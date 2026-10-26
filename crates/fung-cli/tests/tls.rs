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
            // through the binary's lock: the other test starts children too
            let o = common::output(
                Command::new(bin)
                    .env("TZ", common::TZ)
                    .arg("run")
                    .arg(tmp.join("get.fung"))
                    .arg(&url)
                    .env("SSL_CERT_FILE", data.join(roots))
                    // an empty directory: only SSL_CERT_FILE's roots
                    .env("SSL_CERT_DIR", tmp.join("certs")),
            )
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

/// Where the roots come from when the variables are odd, set on the
/// child alone: a root file whose only block is a broken PEM loads no
/// root (the server is then of an unknown authority; a distinct error for
/// "no roots" is part of `docs/plans/0.5.0/12-sockets-tls.md`), a broken
/// block before a good one skips only itself, and `SSL_CERT_DIR` alone
/// (a directory holding the test CA, no `SSL_CERT_FILE`) is enough.
#[test]
fn odd_root_variables() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/tls");
    let tmp = std::env::temp_dir().join(format!("fung-tls-odd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("dir")).unwrap();
    std::fs::copy(data.join("ca.pem"), tmp.join("dir/ca.pem")).unwrap();
    let ca = std::fs::read_to_string(data.join("ca.pem")).unwrap();
    let broken = ca.replacen("MII", "M!I", 1);
    std::fs::write(tmp.join("broken.pem"), &broken).unwrap();
    std::fs::write(tmp.join("broken-then-good.pem"), format!("{broken}{ca}")).unwrap();
    let hi: common::tls_server::Handler = Arc::new(|_: &[u8]| {
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nhi".to_vec()
    });
    let port =
        common::tls_server::start(&data.join("localhost.pem"), &data.join("localhost.key"), hi);
    let src = format!(
        "import http\nr = http.get(\"https://localhost:{port}/\")\nprint(match r:\n  Ok(v) => \"${{v.status}} ${{v.body}}\"\n  Err(e) => e.message\n;;)"
    );
    let get = |var: &str, path: std::path::PathBuf| {
        let mut cmd = Command::new(common::fung_bin());
        cmd.env("TZ", common::TZ)
            .args(["-e", &src])
            .env_remove("SSL_CERT_FILE")
            .env_remove("SSL_CERT_DIR")
            .env(var, path);
        let o = common::output(&mut cmd).unwrap();
        assert!(
            o.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8_lossy(&o.stdout)
            .trim_end()
            .replace(&port.to_string(), "$PORT")
    };
    assert_eq!(
        get("SSL_CERT_FILE", tmp.join("broken.pem")),
        "cannot request `https://localhost:$PORT/`: unknown certificate authority"
    );
    assert_eq!(
        get("SSL_CERT_FILE", tmp.join("broken-then-good.pem")),
        "200 hi"
    );
    assert_eq!(get("SSL_CERT_DIR", tmp.join("dir")), "200 hi");
    let _ = std::fs::remove_dir_all(&tmp);
}
