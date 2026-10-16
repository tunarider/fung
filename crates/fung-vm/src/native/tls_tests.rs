//! `_native.net` TLS connections (stage 4d, Task 2): `connect(tls:
//! true)` against a rustls server in the test process, with the test
//! certificates of `crates/fung-cli/tests/data/tls` (made by
//! `scripts/gen-test-certs.sh`).
//!
//! Trust is given in process through `probe::trust` (a root store the
//! connections use instead of the process-wide one), so no test changes
//! the environment; `ssl_cert_file_gives_the_roots` checks the real loader
//! in a child process with `SSL_CERT_FILE` set. Expected error texts are
//! Go 1.27's pure-Go verifier's (Linux, and macOS with `SSL_CERT_FILE`
//! set; small Go programs, texts copied in). `crates/fung-cli/tests/tls.rs`
//! checks that Go prints the same texts.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};

use super::tests::{HELPERS, net_probes, out, run, serial, wait_for};
use super::{probe, tls};
use crate::testutil::{Opts, UNTIL, exec};

const UNKNOWN_AUTHORITY: &str =
    "tls: failed to verify certificate: x509: certificate signed by unknown authority";
const WRONG_NAME: &str = "tls: failed to verify certificate: x509: certificate is valid for wrong.example, not localhost";
const NO_IP_SANS: &str = "tls: failed to verify certificate: x509: cannot validate certificate for 127.0.0.1 because it doesn't contain any IP SANs";

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fung-cli/tests/data/tls")
        .join(name)
}

/// A server configuration presenting `<cert>.pem` (leaf and CA).
fn server_config(cert: &str) -> Arc<ServerConfig> {
    let chain: Vec<CertificateDer<'static>> =
        CertificateDer::pem_file_iter(data(&format!("{cert}.pem")))
            .unwrap()
            .map(|c| c.unwrap())
            .collect();
    let key = PrivateKeyDer::from_pem_file(data(&format!("{cert}.key"))).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut c = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(chain, key)
        .unwrap();
    // No session tickets after the handshake: a client that closes with
    // them unread makes its kernel answer with a reset instead of an end
    // of stream, which would make the server's view of a close racy.
    c.send_tls13_tickets = 0;
    Arc::new(c)
}

type Tls = StreamOwned<ServerConnection, TcpStream>;

/// A TLS server on 127.0.0.1 presenting `cert`: the port and the thread
/// running `serve` on the first connection (the handshake happens on its
/// first read or write, or `handshake`).
fn tls_peer<T: Send + 'static>(
    cert: &str,
    serve: impl FnOnce(Tls) -> T + Send + 'static,
) -> (u16, JoinHandle<T>) {
    let cfg = server_config(cert);
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    l.set_nonblocking(true).unwrap();
    let h = std::thread::spawn(move || {
        let mut s = None;
        wait_for(|| {
            s = l.accept().ok().map(|(s, _)| s);
            s.is_some()
        });
        let s = s.unwrap();
        s.set_nonblocking(false).unwrap();
        serve(StreamOwned::new(ServerConnection::new(cfg).unwrap(), s))
    });
    (port, h)
}

/// Runs the handshake; its result (a client that rejects the certificate
/// makes it fail).
fn handshake(s: &mut Tls) -> Result<(), String> {
    while s.conn.is_handshaking() {
        s.conn.complete_io(&mut s.sock).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Reads one line (without its `\n`).
fn line(s: &mut Tls) -> String {
    let mut b = [0u8; 1];
    let mut v = Vec::new();
    while s.read(&mut b).unwrap() == 1 && b[0] != b'\n' {
        v.push(b[0]);
    }
    String::from_utf8(v).unwrap()
}

/// Sends close_notify and everything pending.
fn close_notify(s: &mut Tls) {
    s.conn.send_close_notify();
    while s.conn.wants_write() {
        s.conn.write_tls(&mut s.sock).unwrap();
    }
}

/// Answers one line with `got <line>`, then closes with close_notify.
fn echo(mut s: Tls) {
    let l = line(&mut s);
    s.write_all(format!("got {l}\n").as_bytes()).unwrap();
    close_notify(&mut s);
}

/// The handshake only, its result.
fn reject(mut s: Tls) -> Result<(), String> {
    handshake(&mut s)
}

/// Trusts the test CA for the guard's life.
fn trust_ca() -> tls::probe::Trust {
    tls::probe::trust(&data("ca.pem"))
}

/// Trusts only a CA that signed none of the test certificates.
fn trust_other() -> tls::probe::Trust {
    tls::probe::trust(&data("other-ca.pem"))
}

#[test]
fn a_trusted_server_gets_a_line_there_and_back() {
    let _g = serial();
    let _t = trust_ca();
    for host in ["localhost", "127.0.0.1"] {
        let (port, h) = tls_peer("localhost", echo);
        let o = run(&format!(
            "c = net.connect(\"{host}\", {port}, tls: true)!\nprint(net.write(c, \"héllo\\n\"))\nprint(net.read_line(c, 100))\nprint(net.read_line(c, 100))\nprint(net.close(c))"
        ));
        h.join().unwrap();
        assert_eq!(
            out(&o),
            ("Ok(nil)\nOk(\"got héllo\")\nOk(nil)\nOk(nil)\n", "", 0),
            "{host}"
        );
    }
}

#[test]
fn the_server_name_is_sent_for_a_name_only() {
    let _g = serial();
    let _t = trust_ca();
    for (host, want) in [("localhost", Some("localhost")), ("127.0.0.1", None)] {
        let (port, h) = tls_peer("localhost", |mut s| {
            handshake(&mut s).unwrap();
            let name = s.conn.server_name().map(str::to_string);
            close_notify(&mut s);
            name
        });
        let o = run(&format!(
            "c = net.connect(\"{host}\", {port}, tls: true)!\nprint(net.read_all(c, 10))"
        ));
        assert_eq!(h.join().unwrap().as_deref(), want);
        assert_eq!(out(&o), ("Ok(\"\")\n", "", 0));
    }
}

#[test]
fn an_unknown_authority_reads_like_go() {
    let _g = serial();
    let _t = trust_other();
    let (port, h) = tls_peer("localhost", reject);
    let o = run(&format!(
        "print(net.connect(\"localhost\", {port}, tls: true))"
    ));
    assert!(h.join().unwrap().is_err(), "the client sends an alert");
    assert_eq!(
        out(&o),
        (
            format!("Err({{kind: 'network, message: \"{UNKNOWN_AUTHORITY}\"}})\n").as_str(),
            "",
            0
        )
    );
}

/// Go checks the name before looking for the issuer: a certificate for
/// another name says so whether or not its issuer is trusted.
#[test]
fn a_name_mismatch_reads_like_go() {
    let _g = serial();
    for trusted in [true, false] {
        let _t = if trusted { trust_ca() } else { trust_other() };
        for (host, want) in [("localhost", WRONG_NAME), ("127.0.0.1", NO_IP_SANS)] {
            let (port, h) = tls_peer("wrongname", reject);
            let o = run(&format!(
                "print(msg(net.connect(\"{host}\", {port}, tls: true)))"
            ));
            let _ = h.join().unwrap();
            assert_eq!(out(&o), (format!("{want}\n").as_str(), "", 0), "{host}");
        }
    }
}

/// `current time <local, RFC 3339> is after <not after, UTC>`; Go checks
/// the leaf's dates first of all.
#[test]
fn an_expired_certificate_reads_like_go() {
    let _g = serial();
    for trusted in [true, false] {
        let _t = if trusted { trust_ca() } else { trust_other() };
        let (port, h) = tls_peer("expired", reject);
        let o = run(&format!(
            "print(msg(net.connect(\"localhost\", {port}, tls: true)))"
        ));
        let _ = h.join().unwrap();
        let text = o.out.trim_end();
        let prefix = "tls: failed to verify certificate: x509: certificate has expired or is not yet valid: current time ";
        let suffix = " is after 2001-01-01T00:00:00Z";
        assert!(text.starts_with(prefix) && text.ends_with(suffix), "{o:?}");
        let now = &text[prefix.len()..text.len() - suffix.len()];
        // 2026-10-16T17:51:08+09:00 or 2026-10-16T08:51:08Z
        let b = now.as_bytes();
        assert!(
            b.len() >= 20 && b[4] == b'-' && b[10] == b'T' && b[13] == b':',
            "{now}"
        );
        assert!(now.ends_with('Z') || b[b.len() - 3] == b':', "{now}");
    }
}

/// Something that is not a TLS server answering: Go's text, at once.
#[test]
fn a_server_that_does_not_speak_tls_reads_like_go() {
    let _g = serial();
    let _t = trust_ca();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let h = std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut b = [0u8; 4096];
        let _ = s.read(&mut b);
        s.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").unwrap();
        // keep it open: the client must not wait for more
        let _ = s.read(&mut b);
    });
    let o = run(&format!(
        "print(msg(net.connect(\"127.0.0.1\", {port}, tls: true)))"
    ));
    h.join().unwrap();
    assert_eq!(
        out(&o),
        (
            "tls: first record does not look like a TLS handshake\n",
            "",
            0
        )
    );
}

/// A fatal alert from the server: Go's `remote error: tls: <alert>`.
#[test]
fn an_alert_from_the_server_reads_like_go() {
    let _g = serial();
    let _t = trust_ca();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let h = std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut b = [0u8; 4096];
        let _ = s.read(&mut b);
        // a fatal handshake_failure alert record
        s.write_all(&[21, 3, 3, 0, 2, 2, 40]).unwrap();
        let _ = s.read(&mut b);
    });
    let o = run(&format!(
        "print(msg(net.connect(\"127.0.0.1\", {port}, tls: true)))"
    ));
    h.join().unwrap();
    assert_eq!(out(&o), ("remote error: tls: handshake failure\n", "", 0));
}

/// No name to verify (the local system, `""`): Go's `tls.Dialer` text,
/// once connected.
#[test]
fn no_host_name_reads_like_go() {
    let _g = serial();
    let _t = trust_ca();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let o = run(&format!("print(msg(net.connect(\"\", {port}, tls: true)))"));
    drop(l);
    assert_eq!(
        out(&o),
        (
            "tls: either ServerName or InsecureSkipVerify must be specified in the tls.Config\n",
            "",
            0
        )
    );
}

/// The server reads the hello and hangs up: Go's `EOF`.
#[test]
fn a_server_that_hangs_up_in_the_handshake_reads_like_go() {
    let _g = serial();
    let _t = trust_ca();
    let base = probe::read_waits();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let h = std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        // the hello is in once the client waits for the answer
        wait_for(|| probe::read_waits() > base);
        s.set_nonblocking(true).unwrap();
        let mut b = [0u8; 65536];
        while s.read(&mut b).is_ok_and(|n| n > 0) {}
        s.shutdown(Shutdown::Both).unwrap();
    });
    let o = run(&format!(
        "print(net.connect(\"127.0.0.1\", {port}, tls: true))"
    ));
    h.join().unwrap();
    assert_eq!(
        out(&o),
        ("Err({kind: 'network, message: \"EOF\"})\n", "", 0)
    );
}

/// Review Focus 1: a task waiting in the handshake (the hello sent, no
/// answer) ends with E0701 when cancelled, at once, leaving nothing open
/// (`run` checks). The order is by state: the cancel comes once the task
/// waits to read, which only the handshake does.
#[test]
fn cancel_ends_a_handshake() {
    let _g = serial();
    let _t = trust_ca();
    // a listener that never answers: the kernel accepts for it
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let start = Instant::now();
    let base = probe::read_waits();
    let o = run(&format!(
        "t = spawn(() => net.connect(\"127.0.0.1\", {port}, tls: true))\nuntil(() => read_waits() > {base} and net_waiting() == 1)\ncancel(t)\nprint(await(t))\nprint(net_waiting())"
    ));
    drop(l);
    assert_eq!(out(&o), ("Err({kind: 'cancelled})\n0\n", "", 0));
    assert!(start.elapsed() < Duration::from_secs(5));
}

/// The connect timeout covers the handshake: Go's `tls.Dialer` text.
#[test]
fn a_timeout_ends_a_handshake() {
    let _g = serial();
    let _t = trust_ca();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let o = run(&format!(
        "print(net.connect(\"127.0.0.1\", {port}, tls: true, timeout: 50ms))"
    ));
    drop(l);
    assert_eq!(
        out(&o),
        (
            "Err({kind: 'timeout, message: \"context deadline exceeded\"})\n",
            "",
            0
        )
    );
}

/// Reading until the server closes: close_notify ends the stream, and so
/// does a TCP end between records (Go accepts it, as popular sites do
/// that); a TCP end inside a record is `unexpected EOF` (Go).
#[test]
fn reading_until_close_reads_like_go() {
    let _g = serial();
    let _t = trust_ca();
    let program = |port| {
        format!(
            "c = net.connect(\"127.0.0.1\", {port}, tls: true)!\nprint(net.read_all(c, 100))\nprint(net.read_all(c, 100))"
        )
    };
    // close_notify
    let (port, h) = tls_peer("localhost", |mut s| {
        s.write_all("a\nb한".as_bytes()).unwrap();
        close_notify(&mut s);
    });
    let o = run(&program(port));
    h.join().unwrap();
    assert_eq!(out(&o), ("Ok(\"a\\nb한\")\nOk(\"\")\n", "", 0));
    // a TCP end after a whole record, no close_notify
    let (port, h) = tls_peer("localhost", |mut s| {
        s.write_all(b"a\nb").unwrap();
        s.flush().unwrap();
        s.sock.shutdown(Shutdown::Both).unwrap();
    });
    let o = run(&program(port));
    h.join().unwrap();
    assert_eq!(out(&o), ("Ok(\"a\\nb\")\nOk(\"\")\n", "", 0));
    // a TCP end inside a record
    let (port, h) = tls_peer("localhost", |mut s| {
        handshake(&mut s).unwrap();
        s.conn.writer().write_all(b"a\nb").unwrap();
        let mut wire = Vec::new();
        while s.conn.wants_write() {
            s.conn.write_tls(&mut wire).unwrap();
        }
        s.sock.write_all(&wire[..wire.len() - 2]).unwrap();
        s.sock.shutdown(Shutdown::Both).unwrap();
    });
    let o = run(&program(port));
    h.join().unwrap();
    assert_eq!(
        out(&o),
        (
            "Err({kind: 'network, message: \"unexpected EOF\"})\nErr({kind: 'network, message: \"unexpected EOF\"})\n",
            "",
            0
        )
    );
}

/// The other primitives work the same over TLS: exact reads, raw body
/// reads decoded once, deadlines, and `close` (which sends close_notify,
/// so the server sees a clean end; a second close is Go's error).
#[test]
fn the_primitives_work_over_tls() {
    let _g = serial();
    let _t = trust_ca();
    let (port, h) = tls_peer("localhost", |mut s| {
        // "한" split across two records
        s.write_all(b"12345\xed").unwrap();
        s.flush().unwrap();
        s.write_all(b"\x95\x9c!").unwrap();
        s.flush().unwrap();
        let l = line(&mut s);
        let mut rest = Vec::new();
        let r = s.read_to_end(&mut rest).map_err(|e| e.to_string());
        (l, r)
    });
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port}, tls: true)!\nprint(net.read_bytes(c, 3))\nprint(net.read_bytes_into(c, 3, 10), net.read_bytes_into(c, 3, 10), [net.take_body(c)])\nnet.set_deadline(c, 20ms)!\nprint(kind(net.read_line(c, 10)))\nnet.set_deadline(c, 0s)!\nprint(net.write(c, \"bye\\n\"), net.close(c))\nprint(msg(net.close(c)) == \"close tcp ${{net.local_addr(c)}}->127.0.0.1:{port}: use of closed network connection\")"
    ));
    let (l, r) = h.join().unwrap();
    assert_eq!(
        out(&o),
        (
            "Ok(\"123\")\nOk(nil) Ok(nil) [\"45한!\"]\n'timeout\nOk(nil) Ok(nil)\ntrue\n",
            "",
            0
        )
    );
    assert_eq!((l.as_str(), r), ("bye", Ok(0)));
}

/// One reader and one writer at once over TLS: the rustls lock is never
/// held across a wait, so a reader waiting for the answer does not stop a
/// long write (which waits for room in the socket) from going out.
#[test]
fn a_reader_and_a_writer_run_at_once_over_tls() {
    let _g = serial();
    let _t = trust_ca();
    let base = probe::read_waits();
    let (port, h) = tls_peer("localhost", move |mut s| {
        let mut got = vec![0u8; 3_000_000];
        s.read_exact(&mut got).unwrap();
        s.write_all(b"done\n").unwrap();
        close_notify(&mut s);
        got
    });
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port}, tls: true)!\nr = spawn(() => net.read_line(c, 10))\nuntil(() => read_waits() > {base} and net_waiting() == 1)\nprint(net.write(c, repeat(\"한ab\", 600000)))\nprint(await(r))"
    ));
    let got = h.join().unwrap();
    assert_eq!(out(&o), ("Ok(nil)\nOk(\"done\")\n", "", 0));
    assert_eq!(got, "한ab".repeat(600_000).into_bytes());
}

/// The last copy of a TLS connection closes it (no close_notify, as Go:
/// nothing is sent when a connection is collected).
#[test]
fn the_last_copy_closes_a_tls_connection() {
    let _g = serial();
    let _t = trust_ca();
    let (port, h) = tls_peer("localhost", |mut s| {
        handshake(&mut s).unwrap();
        let mut rest = Vec::new();
        s.read_to_end(&mut rest).map_err(|e| e.kind())
    });
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port}, tls: true)!\nprint(sockets())"
    ));
    assert_eq!(out(&o), ("1\n", "", 0));
    assert_eq!(h.join().unwrap(), Err(std::io::ErrorKind::UnexpectedEof));
}

/// The real root store comes from `SSL_CERT_FILE` (rustls-native-certs)
/// and is read once per process. Run in a child process (this test
/// again, with the variable set) so that no test changes this process's
/// environment.
#[test]
fn ssl_cert_file_gives_the_roots() {
    if let Ok(ports) = std::env::var("FUNG_TLS_TEST_PORTS") {
        // the child: no `probe::trust`, the process-wide store
        let (p1, p2) = ports.split_once(' ').unwrap();
        let o = exec(
            &format!(
                "import _native.net as net\n{UNTIL}{HELPERS}print(msg(net.connect(\"localhost\", {p1}, tls: true)))\nprint(msg(net.connect(\"localhost\", {p2}, tls: true)))"
            ),
            Opts {
                internal: true,
                extra: net_probes,
                ..Opts::default()
            },
        );
        println!(
            "CHILD {:?} {:?} loads={}",
            o.out,
            o.err,
            tls::probe::loads()
        );
        return;
    }
    let _g = serial();
    for (roots, want) in [
        ("ca.pem", "Ok(<conn>)\\nOk(<conn>)\\n".to_string()),
        (
            "other-ca.pem",
            format!("{UNKNOWN_AUTHORITY}\\n{UNKNOWN_AUTHORITY}\\n"),
        ),
    ] {
        let serve = |mut s: Tls| {
            let _ = handshake(&mut s);
        };
        let (p1, h1) = tls_peer("localhost", serve);
        let (p2, h2) = tls_peer("localhost", serve);
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "native::net::tls_tests::ssl_cert_file_gives_the_roots",
                "--exact",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("FUNG_TLS_TEST_PORTS", format!("{p1} {p2}"))
            .env("SSL_CERT_FILE", data(roots))
            .env_remove("SSL_CERT_DIR")
            .output()
            .unwrap();
        h1.join().unwrap();
        h2.join().unwrap();
        let stdout = String::from_utf8_lossy(&child.stdout);
        assert!(
            stdout.contains(&format!("CHILD \"{want}\" \"\" loads=1")),
            "{roots}: {stdout}\n{}",
            String::from_utf8_lossy(&child.stderr)
        );
    }
}

/// A plain server that reads one line, answers `ready`, then speaks TLS
/// (presenting `cert`) on the same socket, as an HTTP proxy's CONNECT
/// tunnel does: the port, and the thread giving the plain line read.
fn tunnel_peer(cert: &str, serve: impl FnOnce(Tls) + Send + 'static) -> (u16, JoinHandle<String>) {
    let cfg = server_config(cert);
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    l.set_nonblocking(true).unwrap();
    let h = std::thread::spawn(move || {
        let mut s = None;
        wait_for(|| {
            s = l.accept().ok().map(|(s, _)| s);
            s.is_some()
        });
        let mut s = s.unwrap();
        s.set_nonblocking(false).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut b = [0u8; 1];
        let mut v = Vec::new();
        while s.read(&mut b).unwrap() == 1 && b[0] != b'\n' {
            v.push(b[0]);
        }
        s.write_all(b"ready\n").unwrap();
        serve(StreamOwned::new(ServerConnection::new(cfg).unwrap(), s));
        String::from_utf8(v).unwrap()
    });
    (port, h)
}

/// Stage 4d Task 4: `start_tls` makes a plain connection a TLS client of
/// a name over the same socket (an HTTP proxy's CONNECT tunnel); the old
/// value is closed without ending the socket. Errors read like connect's.
#[test]
fn start_tls_upgrades_a_plain_connection() {
    let _g = serial();
    let _t = trust_ca();
    let (port, h) = tunnel_peer("localhost", echo);
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port})!\nnet.write(c, \"hello\\n\")!\nprint(net.read_line(c, 100))\nt = net.start_tls(c, \"localhost\", timeout: 5s)!\nprint(msg(net.write(c, \"x\")) == \"write tcp ${{net.local_addr(c)}}->127.0.0.1:{port}: use of closed network connection\")\nprint(net.write(t, \"secret\\n\"))\nprint(net.read_line(t, 100))\nprint(net.read_line(t, 100))\nprint(net.close(t))\nprint(msg(net.start_tls(c, \"localhost\")) == \"read tcp ${{net.local_addr(c)}}->127.0.0.1:{port}: use of closed network connection\")"
    ));
    assert_eq!(h.join().unwrap(), "hello");
    assert_eq!(
        out(&o),
        (
            "Ok(\"ready\")\ntrue\nOk(nil)\nOk(\"got secret\")\nOk(nil)\nOk(nil)\ntrue\n",
            "",
            0
        )
    );
    let (port, h) = tunnel_peer("wrongname", |mut s| {
        let _ = handshake(&mut s);
    });
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port})!\nnet.write(c, \"hi\\n\")!\nnet.read_line(c, 100)!\nprint(msg(net.start_tls(c, \"localhost\")))\nprint(type_of(net.start_tls(c, \"localhost\", timeout: 1)))"
    ));
    h.join().unwrap();
    assert_eq!(out(&o).0, format!("{WRONG_NAME}\n"));
    assert!(
        out(&o)
            .1
            .contains("argument `timeout` must be dur, got int")
    );
}
