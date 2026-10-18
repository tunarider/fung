//! A TLS server for the command-line tests (`http.rs`, `tls.rs`), with
//! rustls (a dev-dependency of fung-cli, `deps.rs`).
//!
//! It serves HTTP/1.1 over TLS on 127.0.0.1: one request per connection
//! (head, then a `Content-Length` body), whose raw bytes go to the test's
//! function; the bytes it returns are the answer. Then it closes the
//! connection with close_notify. A client that rejects the certificate
//! ends only its own connection.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};

/// The answer to a request's raw bytes.
pub type Handler = Arc<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>;

/// Starts a server presenting the chain in `pem` with the key in `key`;
/// returns its port. It serves until the process ends.
pub fn start(pem: &Path, key: &Path, handler: Handler) -> u16 {
    let chain: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(pem)
        .unwrap()
        .map(|c| c.unwrap())
        .collect();
    let key = PrivateKeyDer::from_pem_file(key).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(chain, key)
        .unwrap();
    // No session tickets: a client that closes with them unread makes
    // its kernel answer with a reset.
    cfg.send_tls13_tickets = 0;
    let cfg = Arc::new(cfg);
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(s) = s else { return };
            let (cfg, handler) = (cfg.clone(), handler.clone());
            std::thread::spawn(move || {
                let _ = serve(s, cfg, &handler);
            });
        }
    });
    port
}

fn serve(s: TcpStream, cfg: Arc<ServerConfig>, handler: &Handler) -> std::io::Result<()> {
    s.set_read_timeout(Some(std::time::Duration::from_secs(60)))?;
    let conn = ServerConnection::new(cfg).map_err(std::io::Error::other)?;
    let mut tls = StreamOwned::new(conn, s);
    let mut req = Vec::new();
    let mut buf = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = req.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        let n = tls.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        req.extend_from_slice(&buf[..n]);
    };
    let head = String::from_utf8_lossy(&req[..head_end]).to_lowercase();
    let body_len: usize = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .map_or(0, |v| v.trim().parse().unwrap_or(0));
    while req.len() < head_end + body_len {
        let n = tls.read(&mut buf)?;
        if n == 0 {
            break;
        }
        req.extend_from_slice(&buf[..n]);
    }
    tls.write_all(&handler(&req))?;
    tls.conn.send_close_notify();
    tls.flush()?;
    while tls.conn.wants_write() {
        tls.conn.write_tls(&mut tls.sock)?;
    }
    let _ = tls.sock.shutdown(std::net::Shutdown::Write);
    // Read until the client closes, so closing here never resets a
    // connection with unread data.
    while matches!(tls.sock.read(&mut buf), Ok(n) if n > 0) {}
    Ok(())
}
