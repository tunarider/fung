//! `http.get` over TLS: a server presenting the test certificates
//! (`tests/data/tls`, `scripts/gen-test-certs.sh`), with `SSL_CERT_FILE`
//! naming the roots the client trusts. The answers and the verification
//! errors are kept in `expected/tls/gets.txt`.
//!
//! The texts are also checked in `fung-vm` (`native/tls_tests.rs`, a
//! rustls server in the test process) through `_native.net`, which a
//! user program cannot reach; the constants below are the same texts.

mod common;

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use common::expected::{self, Entry};

/// The same texts as `fung-vm`'s `native/tls_tests.rs`.
const UNKNOWN_AUTHORITY: &str = "unknown certificate authority";
const WRONG_NAME: &str = "certificate is not valid for `localhost`";
const NO_IP_SANS: &str = "certificate is not valid for `127.0.0.1`";
const EXPIRED: &str = "certificate has expired (valid until 2001-01-01T00:00:00Z)";

/// (certificate, roots, host, what the client must say)
const CASES: &[(&str, &str, &str, &str)] = &[
    ("localhost", "ca.pem", "localhost", "200 hi"),
    ("localhost", "ca.pem", "127.0.0.1", "200 hi"),
    ("localhost", "other-ca.pem", "localhost", UNKNOWN_AUTHORITY),
    ("wrongname", "ca.pem", "localhost", WRONG_NAME),
    // the chain is checked before the name
    ("wrongname", "other-ca.pem", "localhost", UNKNOWN_AUTHORITY),
    ("wrongname", "ca.pem", "127.0.0.1", NO_IP_SANS),
    ("expired", "ca.pem", "localhost", EXPIRED),
    ("expired", "other-ca.pem", "localhost", EXPIRED),
];

const HEADER: &str =
    "`http.get` over TLS with the test certificates (crates/fung-cli/tests/tls.rs).
Labels: certificate, trusted roots, host. `$PORT` is the server's port.
After a deliberate change: FUNG_EXPECTED=bless cargo test -p fung-cli --test tls";

#[test]
fn tls_gets_match_the_expected_output() {
    assert_eq!(CASES.len(), 8, "the number of cases");
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/tls");
    let tmp = std::env::temp_dir().join(format!("fung-tls-{}", std::process::id()));
    std::fs::create_dir_all(tmp.join("certs")).unwrap();
    std::fs::write(
        tmp.join("get.fung"),
        "import http\nfn show(r) = match r:\n  Ok(v) => \"${v.status} ${v.body}\"\n  Err(e) => e.message\n;;\nprint(show(http.get(args[0])))\n",
    )
    .unwrap();
    let hi: common::tls_server::Handler = Arc::new(|_: &[u8]| {
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nhi".to_vec()
    });
    let mut got = Vec::new();
    for (cert, roots, host, text) in CASES {
        let port = common::tls_server::start(
            &data.join(format!("{cert}.pem")),
            &data.join(format!("{cert}.key")),
            hi.clone(),
        );
        let url = format!("https://{host}:{port}/");
        let get = |bin: &Path| {
            let o = Command::new(bin)
                .env("TZ", common::TZ)
                .arg("run")
                .arg(tmp.join("get.fung"))
                .arg(&url)
                .env("SSL_CERT_FILE", data.join(roots))
                // an empty directory: only SSL_CERT_FILE's roots
                .env("SSL_CERT_DIR", tmp.join("certs"))
                .output()
                .unwrap();
            let ports = [(port, "$PORT")];
            Entry::new(format!("{cert} {roots} {host}"))
                .field("stdout", expected::replace_ports(&o.stdout, &ports))
                .field("stderr", expected::replace_ports(&o.stderr, &ports))
                .field("exit", expected::exit_text(&format!("{:?}", o.status)))
        };
        let r = get(&common::fung_bin());
        // What the expected output says, checked here too.
        let out = String::from_utf8_lossy(r.get("stdout").unwrap()).into_owned();
        let out = out.trim_end();
        let case = format!("{cert} {roots} {host}: {out:?}");
        if text.starts_with("200") {
            assert_eq!(out, *text, "{case}");
        } else {
            // cannot request `<url>`: <reason>
            let shown = "https://".to_string() + host + ":$PORT/";
            let prefix = format!("cannot request `{shown}`: ");
            let reason = out
                .strip_prefix(&prefix)
                .unwrap_or_else(|| panic!("{case}"));
            assert_eq!(reason, *text, "{case}");
        }
        got.push(r);
    }
    let _ = std::fs::remove_dir_all(&tmp);
    expected::verify("tls/gets.txt", HEADER, 64 * 1024, &got);
}
