//! TLS client connections for `_native.net` (stage 4d): `connect(host,
//! port, tls: true)` with rustls (ring), verified against the operating
//! system's roots, or `SSL_CERT_FILE` / `SSL_CERT_DIR` when set
//! (rustls-native-certs: either variable replaces the system store).
//!
//! **Roots.** Read once per process, at the first TLS connection, into a
//! `ClientConfig` (plain Rust values) kept in a `OnceLock`; a root that
//! does not parse is skipped, as Go does. No session resumption (Go's
//! client has no session cache by default) and no ALPN (`_native.net` is
//! not HTTP-specific; the server then speaks HTTP/1.1).
//!
//! **Driving rustls without blocking.** The socket stays non-blocking. The
//! handshake, reads and writes move bytes between rustls and the socket
//! with `read_tls` / `write_tls` and, when the socket would block, wait in
//! the task's `Watch` (`wait_io`) like the TCP calls: a cancellation of
//! the task ends a wait in the handshake, a read or a write with E0701,
//! and the connect deadline (`timeout`) covers the handshake.
//!
//! **Locks.** A TLS connection has one more lock, around the rustls state
//! (`Session`), since reading and writing both change it. It is held only
//! for non-blocking steps (moving bytes, decrypting), never across a wait,
//! so the one reader and the one writer a connection allows (net.rs) still
//! run at once: a writer waiting for room in the socket does not keep a
//! reader from decrypting what arrived, and the other way round. Each one
//! keeps its own lock (`rd`, `wr`) for its whole call as for TCP.
//!
//! **The end of the stream** follows Go's `crypto/tls`: close_notify ends
//! it; a TCP end without close_notify is an end too when it falls between
//! records (Go accepts it: popular sites close that way), and `unexpected
//! EOF` inside a record. rustls reports both as `UnexpectedEof`, so the
//! record framing of the bytes read is followed here (`Records`) to tell
//! them apart. `close` sends close_notify (best effort, without waiting);
//! the last copy going away does not (neither does Go's collector).
//!
//! **Errors** are Go's texts (open-questions "Go 동등을 위한 과도기
//! 잔가지"): certificate errors as Go's own verifier words them (Linux;
//! macOS when `SSL_CERT_FILE` or `SSL_CERT_DIR` is set, since Go 1.27;
//! otherwise Go on macOS asks the platform, whose texts differ), checked
//! in Go's order (the leaf's dates, then its name, then the
//! chain: `GoOrder`); remote alerts as `remote error: tls: <alert>`; the
//! first record of something that is not a TLS server; `EOF` when the
//! server hangs up in the handshake; `context deadline exceeded` (Go's
//! `tls.Dialer`) when the timeout passes in the handshake. Other rustls
//! errors are `tls: <rustls's text>` (a known difference).

use std::io::{self, Read};
use std::net::{IpAddr, TcpStream};
use std::os::fd::AsRawFd;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::{Resumption, verify_server_cert_signed_by_trust_anchor, verify_server_name};
use rustls::crypto::{WebPkiSupportedAlgorithms, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::ParsedCertificate;
use rustls::{
    CertificateError, ClientConfig, ClientConnection, DigitallySignedStruct, Error as TlsError,
    RootCertStore, SignatureScheme,
};

use super::{CHUNK, Conn, Fail, lock, wait_io};
use crate::native::oserr;
use crate::native::tz;
use crate::value::{Loc, Time};
use crate::vm::{Interest, Waited, Watch};

/// The rustls state of one connection and the framing of what was read.
pub(super) struct Session {
    conn: ClientConnection,
    rec: Records,
}

impl Session {
    /// `read_tls` from the socket, following the record framing.
    fn read_tls(&mut self, s: &TcpStream) -> io::Result<usize> {
        let Session { conn, rec } = self;
        conn.read_tls(&mut Tap { s, rec })
    }

    /// Writes what rustls has to send (an alert, close_notify, a key
    /// update answer) as far as the socket takes it without waiting.
    fn flush_quietly(&mut self, s: &TcpStream) {
        while self.conn.wants_write() {
            match self.conn.write_tls(&mut &*s) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    }

    /// A failure rustls reported: its alert goes out, then Go's text.
    fn failed(&mut self, s: &TcpStream, e: &TlsError) -> Fail {
        self.flush_quietly(s);
        Fail::Network(reason(e))
    }
}

/// The socket as rustls reads it, each byte also given to `rec`.
struct Tap<'a> {
    s: &'a TcpStream,
    rec: &'a mut Records,
}

impl Read for Tap<'_> {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let n = (&*self.s).read(b)?;
        self.rec.feed(&b[..n]);
        Ok(n)
    }
}

/// Where the bytes read so far stand in the TLS record framing (a 5-byte
/// header holding the body's length, then the body): enough to tell a
/// stream that ends between records from one that ends inside a record,
/// and to look at the first header as Go does.
#[derive(Default)]
struct Records {
    /// The header being read and how much of it is in.
    hdr: [u8; 5],
    have: usize,
    /// Body bytes of the current record still to come.
    left: usize,
    /// The first header is complete.
    first: bool,
    /// The first header does not look like TLS (Go's check).
    not_tls: bool,
}

impl Records {
    fn feed(&mut self, mut b: &[u8]) {
        while !b.is_empty() {
            if self.left > 0 {
                let k = self.left.min(b.len());
                self.left -= k;
                b = &b[k..];
                continue;
            }
            let k = (5 - self.have).min(b.len());
            self.hdr[self.have..self.have + k].copy_from_slice(&b[..k]);
            self.have += k;
            b = &b[k..];
            if self.have == 5 {
                if !self.first {
                    self.first = true;
                    // Go: not an alert or a handshake, or a version past
                    // anything real
                    let vers = u16::from_be_bytes([self.hdr[1], self.hdr[2]]);
                    self.not_tls = !matches!(self.hdr[0], 21 | 22) || vers >= 0x1000;
                }
                self.left = usize::from(u16::from_be_bytes([self.hdr[3], self.hdr[4]]));
                self.have = 0;
            }
        }
    }

    fn inside_a_record(&self) -> bool {
        self.have > 0 || self.left > 0
    }
}

/// `EOF` between records, `unexpected EOF` inside one (Go `io.EOF`,
/// `io.ErrUnexpectedEOF`).
fn eof_text(rec: &Records) -> &'static str {
    if rec.inside_a_record() {
        "unexpected EOF"
    } else {
        "EOF"
    }
}

/// Makes `c` (just connected, not shared yet) a TLS client of `host`:
/// the session once the handshake is done. `deadline` is the connect's.
pub(super) fn handshake(
    w: &mut Watch,
    c: &Conn,
    host: &str,
    deadline: Option<Instant>,
) -> Result<Session, Fail> {
    if host.is_empty() {
        // Go's `tls.Dialer` with no name to verify
        return Err(Fail::Network(
            "tls: either ServerName or InsecureSkipVerify must be specified in the tls.Config"
                .into(),
        ));
    }
    let name = ServerName::try_from(host.to_string())
        .map_err(|_| Fail::Network(format!("tls: invalid server name {host}")))?;
    let conn = ClientConnection::new(config(), name).map_err(|e| Fail::Network(reason(&e)))?;
    let mut st = Session {
        conn,
        rec: Records::default(),
    };
    let s = c.sock("read")?;
    let s = &s.0;
    let timeout = || Fail::Timeout("context deadline exceeded".into());
    loop {
        if w.cancelled() {
            return Err(Fail::Cancelled);
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return Err(timeout());
        }
        if st.conn.wants_write() {
            match st.conn.write_tls(&mut &*s) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if wait_io(w, &[(s.as_raw_fd(), Interest::Write)], deadline)?
                        == Waited::TimedOut
                    {
                        return Err(timeout());
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    return Err(Fail::Network(format!(
                        "write tcp {}: write: {}",
                        c.name,
                        oserr::reason(&e)
                    )));
                }
            }
            continue;
        }
        if !st.conn.is_handshaking() {
            return Ok(st);
        }
        match st.read_tls(s) {
            Ok(0) => return Err(Fail::Network(eof_text(&st.rec).into())),
            Ok(_) => {
                if st.rec.not_tls {
                    return Err(Fail::Network(
                        "tls: first record does not look like a TLS handshake".into(),
                    ));
                }
                if let Err(e) = st.conn.process_new_packets() {
                    return Err(st.failed(s, &e));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if wait_io(w, &[(s.as_raw_fd(), Interest::Read)], deadline)? == Waited::TimedOut {
                    return Err(timeout());
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => {
                return Err(Fail::Network(format!(
                    "read tcp {}: read: {}",
                    c.name,
                    oserr::reason(&e)
                )));
            }
        }
    }
}

/// `Conn::fill` over TLS: decrypted bytes into `buf` (waiting for
/// some); 0 at the end of the stream.
pub(super) fn fill(
    c: &Conn,
    t: &Mutex<Session>,
    w: &mut Watch,
    s: &TcpStream,
    buf: &mut Vec<u8>,
) -> Result<usize, Fail> {
    let mut chunk = [0u8; CHUNK];
    loop {
        if w.cancelled() {
            return Err(Fail::Cancelled);
        }
        if c.is_closed() {
            return Err(c.closed("read"));
        }
        let deadline = c.deadline();
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return Err(c.timed_out("read"));
        }
        let mut st = lock(t);
        match st.conn.reader().read(&mut chunk) {
            // 0: close_notify came
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                return Ok(n);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            // the TCP stream ended without close_notify
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                return match eof_text(&st.rec) {
                    "EOF" => Ok(0),
                    text => Err(Fail::Network(text.into())),
                };
            }
            Err(e) => return Err(Fail::Network(format!("tls: {e}"))),
        }
        match st.read_tls(s) {
            // a close from another task shuts the socket down: that end
            // of stream is the close; otherwise rustls now knows of the
            // end, and the next round reports it
            Ok(0) if c.is_closed() => return Err(c.closed("read")),
            Ok(0) => {}
            Ok(_) => {
                let r = st.conn.process_new_packets();
                if let Err(e) = r {
                    return Err(st.failed(s, &e));
                }
                // an answer rustls owes (a key update): sent if it fits
                st.flush_quietly(s);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                drop(st);
                if wait_io(w, &[(s.as_raw_fd(), Interest::Read)], deadline)? == Waited::TimedOut {
                    return Err(c.timed_out("read"));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) if c.is_closed() => return Err(c.closed("read")),
            Err(e) => {
                return Err(Fail::Network(format!(
                    "read tcp {}: read: {}",
                    c.name,
                    oserr::reason(&e)
                )));
            }
        }
    }
}

/// `Conn::write` over TLS: all of `b`, encrypted and sent.
pub(super) fn write(
    c: &Conn,
    t: &Mutex<Session>,
    w: &mut Watch,
    s: &TcpStream,
    b: &[u8],
) -> Result<(), Fail> {
    let mut off = 0;
    loop {
        if w.cancelled() {
            return Err(Fail::Cancelled);
        }
        if c.is_closed() {
            return Err(c.closed("write"));
        }
        let deadline = c.deadline();
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return Err(c.timed_out("write"));
        }
        let mut st = lock(t);
        if off < b.len() {
            // takes what fits in rustls's send buffer
            off += io::Write::write(&mut st.conn.writer(), &b[off..])
                .map_err(|e| Fail::Network(format!("tls: {e}")))?;
        }
        if !st.conn.wants_write() {
            if off >= b.len() {
                return Ok(());
            }
            continue;
        }
        match st.conn.write_tls(&mut &*s) {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                drop(st);
                if wait_io(w, &[(s.as_raw_fd(), Interest::Write)], deadline)? == Waited::TimedOut {
                    return Err(c.timed_out("write"));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) if c.is_closed() => return Err(c.closed("write")),
            Err(e) => {
                return Err(Fail::Network(format!(
                    "write tcp {}: write: {}",
                    c.name,
                    oserr::reason(&e)
                )));
            }
        }
    }
}

/// Sends close_notify as far as the socket takes it now (Go waits up to
/// 5 s for it; here a full send buffer drops it).
pub(super) fn close_notify(t: &Mutex<Session>, s: &TcpStream) {
    let mut st = lock(t);
    st.conn.send_close_notify();
    st.flush_quietly(s);
}

/// The process's client configuration: roots read once (the test probe
/// can put another in front).
fn config() -> Arc<ClientConfig> {
    #[cfg(test)]
    if let Some(c) = lock(&probe::TRUST).clone() {
        return c;
    }
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            #[cfg(test)]
            probe::LOADS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // Errors (an unreadable file among several) leave out what
            // they name; what was read is used, as Go does.
            let found = rustls_native_certs::load_native_certs();
            let mut roots = RootCertStore::empty();
            roots.add_parsable_certificates(found.certs);
            client_config(roots)
        })
        .clone()
}

fn client_config(roots: RootCertStore) -> Arc<ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let algs = provider.signature_verification_algorithms;
    let mut c = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("ring supports TLS 1.2 and 1.3")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(GoOrder {
            roots: Arc::new(roots),
            algs,
        }))
        .with_no_client_auth();
    c.resumption = Resumption::disabled();
    Arc::new(c)
}

/// rustls's WebPKI verification, in Go's order: rustls checks the chain
/// (the leaf's dates first) and then the name; Go checks the leaf's
/// dates, then the name, then the chain, so a certificate for another
/// name signed by an unknown issuer is a name error in Go.
#[derive(Debug)]
struct GoOrder {
    roots: Arc<RootCertStore>,
    algs: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for GoOrder {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let cert = ParsedCertificate::try_from(end_entity)?;
        let chain = verify_server_cert_signed_by_trust_anchor(
            &cert,
            &self.roots,
            intermediates,
            now,
            self.algs.all,
        );
        match chain {
            Ok(()) => {}
            Err(
                e @ TlsError::InvalidCertificate(
                    CertificateError::Expired
                    | CertificateError::ExpiredContext { .. }
                    | CertificateError::NotValidYet
                    | CertificateError::NotValidYetContext { .. },
                ),
            ) => return Err(e),
            Err(e) => {
                verify_server_name(&cert, server_name)?;
                return Err(e);
            }
        }
        verify_server_name(&cert, server_name)?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

/// Go's text for a rustls error.
pub(super) fn reason(e: &TlsError) -> String {
    match e {
        TlsError::InvalidCertificate(ce) => {
            format!("tls: failed to verify certificate: {}", x509(ce))
        }
        TlsError::AlertReceived(a) => format!("remote error: {}", alert(u8::from(*a))),
        TlsError::DecryptError => "local error: tls: bad record MAC".into(),
        e => format!("tls: {e}"),
    }
}

/// Go `crypto/x509`'s text for a certificate error.
fn x509(e: &CertificateError) -> String {
    let dates = "x509: certificate has expired or is not yet valid";
    match e {
        CertificateError::UnknownIssuer => "x509: certificate signed by unknown authority".into(),
        CertificateError::NotValidForNameContext {
            expected,
            presented,
        } => hostname_error(expected, presented),
        CertificateError::ExpiredContext { time, not_after } => format!(
            "{dates}: current time {} is after {}",
            rfc3339(time, true),
            rfc3339(not_after, false)
        ),
        CertificateError::NotValidYetContext { time, not_before } => format!(
            "{dates}: current time {} is before {}",
            rfc3339(time, true),
            rfc3339(not_before, false)
        ),
        CertificateError::Expired | CertificateError::NotValidYet => dates.into(),
        e => format!("x509: {e}"),
    }
}

/// Go `x509.HostnameError`: for an IP address the certificate's IP
/// names, else its DNS names. webpki gives the names as `DnsName("x")`
/// and `IpAddress(1.2.3.4)` (rustls-webpki 0.103; a change shows in the
/// tests). Go's message for a certificate with no SAN whose common name
/// matches is not made.
fn hostname_error(expected: &ServerName<'_>, presented: &[String]) -> String {
    let (want_ip, host) = match expected {
        ServerName::IpAddress(ip) => (true, IpAddr::from(*ip).to_string()),
        ServerName::DnsName(d) => (false, d.as_ref().to_string()),
        other => (false, format!("{other:?}")),
    };
    let names: Vec<String> = presented
        .iter()
        .filter_map(|p| {
            if want_ip {
                let ip = p.strip_prefix("IpAddress(")?.strip_suffix(')')?;
                Some(
                    ip.parse::<IpAddr>()
                        .map_or(ip.to_string(), |a| a.to_string()),
                )
            } else {
                Some(
                    p.strip_prefix("DnsName(\"")?
                        .strip_suffix("\")")?
                        .to_string(),
                )
            }
        })
        .collect();
    if want_ip && names.is_empty() {
        return format!(
            "x509: cannot validate certificate for {host} because it doesn't contain any IP SANs"
        );
    }
    if names.is_empty() {
        return format!("x509: certificate is not valid for any names, but wanted to match {host}");
    }
    format!(
        "x509: certificate is valid for {}, not {host}",
        names.join(", ")
    )
}

/// Go `Format(time.RFC3339)`: local time with its offset (`Z` for UTC)
/// for `local`, else UTC.
fn rfc3339(t: &UnixTime, local: bool) -> String {
    let secs = i64::try_from(t.as_secs()).unwrap_or(i64::MAX);
    let loc = if local {
        Loc::Zone(tz::local())
    } else {
        Loc::Utc
    };
    let t = Time::from_unix(secs, 0, loc);
    let (y, mo, d) = t.date();
    let (h, mi, s) = t.clock();
    let (_, off) = t.zone();
    let zone = if off == 0 {
        "Z".to_string()
    } else {
        let (sign, off) = if off < 0 { ('-', -off) } else { ('+', off) };
        format!("{sign}{:02}:{:02}", off / 3600, off % 3600 / 60)
    };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}{zone}")
}

/// Go's `alert.String()`.
fn alert(a: u8) -> String {
    let text = match a {
        0 => "close notify",
        10 => "unexpected message",
        20 => "bad record MAC",
        21 => "decryption failed",
        22 => "record overflow",
        30 => "decompression failure",
        40 => "handshake failure",
        42 => "bad certificate",
        43 => "unsupported certificate",
        44 => "revoked certificate",
        45 => "expired certificate",
        46 => "unknown certificate",
        47 => "illegal parameter",
        48 => "unknown certificate authority",
        49 => "access denied",
        50 => "error decoding message",
        51 => "error decrypting message",
        60 => "export restriction",
        70 => "protocol version not supported",
        71 => "insufficient security level",
        80 => "internal error",
        86 => "inappropriate fallback",
        90 => "user canceled",
        100 => "no renegotiation",
        109 => "missing extension",
        110 => "unsupported extension",
        111 => "certificate unobtainable",
        112 => "unrecognized name",
        113 => "bad certificate status response",
        114 => "bad certificate hash value",
        115 => "unknown PSK identity",
        116 => "certificate required",
        120 => "no application protocol",
        121 => "encrypted client hello required",
        n => return format!("tls: alert({n})"),
    };
    format!("tls: {text}")
}

/// Test probes: trust given in process, and how often the process-wide
/// roots were read.
#[cfg(test)]
pub(crate) mod probe {
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use rustls::ClientConfig;
    use rustls::RootCertStore;
    use rustls::pki_types::CertificateDer;
    use rustls::pki_types::pem::PemObject;

    use super::lock;

    pub(super) static TRUST: Mutex<Option<Arc<ClientConfig>>> = Mutex::new(None);
    pub(super) static LOADS: AtomicUsize = AtomicUsize::new(0);

    /// Connections trust only the certificates of `pem` while the result
    /// lives (tests run one at a time: `serial`).
    pub(crate) fn trust(pem: &Path) -> Trust {
        let mut roots = RootCertStore::empty();
        let certs = CertificateDer::pem_file_iter(pem)
            .unwrap()
            .map(|c| c.unwrap());
        roots.add_parsable_certificates(certs);
        *lock(&TRUST) = Some(super::client_config(roots));
        Trust
    }

    pub(crate) struct Trust;

    impl Drop for Trust {
        fn drop(&mut self) {
            *lock(&TRUST) = None;
        }
    }

    /// How many times the process-wide roots were read.
    pub(crate) fn loads() -> usize {
        LOADS.load(Ordering::SeqCst)
    }
}
