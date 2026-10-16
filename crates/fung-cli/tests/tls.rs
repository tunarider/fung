//! TLS error texts against Go (stage 4d, Task 2), and since Task 3 the
//! Rust `http` client against the same Go TLS server.
//!
//! The Rust texts are checked in `fung-vm` (`native/tls_tests.rs`, a
//! rustls server in the test process), since `_native.net` is not
//! reachable from a user program and this crate may not depend on rustls
//! (`deps.rs`). Here Go's `http.get` meets a Go TLS server presenting the
//! test certificates (`tests/data/tls`, `scripts/gen-test-certs.sh`) with
//! `SSL_CERT_FILE` naming the roots, and its reason must be the text the
//! Rust tests expect: the two agree through these constants.
//!
//! With `SSL_CERT_FILE` (or `SSL_CERT_DIR`) set, Go verifies with its own
//! code on Linux and, since Go 1.27, on macOS too (where it otherwise asks
//! the platform verifier, which words its errors differently; Task 2
//! report), so the comparison runs on both.

mod common;

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

/// The same texts as `fung-vm`'s `native/tls_tests.rs`.
const UNKNOWN_AUTHORITY: &str =
    "tls: failed to verify certificate: x509: certificate signed by unknown authority";
const WRONG_NAME: &str = "tls: failed to verify certificate: x509: certificate is valid for wrong.example, not localhost";
const NO_IP_SANS: &str = "tls: failed to verify certificate: x509: cannot validate certificate for 127.0.0.1 because it doesn't contain any IP SANs";
const EXPIRED_PREFIX: &str = "tls: failed to verify certificate: x509: certificate has expired or is not yet valid: current time ";
const EXPIRED_SUFFIX: &str = " is after 2001-01-01T00:00:00Z";

/// A TLS server answering every request with `hi`: `<cert> <key>` as
/// arguments, its port as the first line of output.
const SERVER: &str = r#"package main

import (
	"crypto/tls"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"os"
)

func main() {
	cert, err := tls.LoadX509KeyPair(os.Args[1], os.Args[2])
	if err != nil {
		panic(err)
	}
	l, err := tls.Listen("tcp", "127.0.0.1:0", &tls.Config{Certificates: []tls.Certificate{cert}})
	if err != nil {
		panic(err)
	}
	fmt.Println(l.Addr().(*net.TCPAddr).Port)
	srv := &http.Server{
		Handler:  http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { io.WriteString(w, "hi") }),
		ErrorLog: log.New(io.Discard, "", 0),
	}
	srv.Serve(l)
}
"#;

#[test]
fn go_gives_the_tls_texts_the_rust_tests_expect() {
    let Some(go_fung) = common::go_fung("tls") else {
        return;
    };
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/tls");
    let tmp = std::env::temp_dir().join(format!("fung-tls-{}", std::process::id()));
    std::fs::create_dir_all(tmp.join("certs")).unwrap();
    std::fs::write(tmp.join("server.go"), SERVER).unwrap();
    let built = Command::new("go")
        .args(["build", "-o", "server", "server.go"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    std::fs::write(
        tmp.join("get.fung"),
        "import http\nfn show(r) = match r:\n  Ok(v) => \"${v.status} ${v.body}\"\n  Err(e) => e.message\n;;\nprint(show(http.get(args[0])))\n",
    )
    .unwrap();
    // (certificate, roots, host, what Go must say)
    let cases = [
        ("localhost", "ca.pem", "localhost", "200 hi"),
        ("localhost", "ca.pem", "127.0.0.1", "200 hi"),
        ("localhost", "other-ca.pem", "localhost", UNKNOWN_AUTHORITY),
        ("wrongname", "ca.pem", "localhost", WRONG_NAME),
        ("wrongname", "other-ca.pem", "localhost", WRONG_NAME),
        ("wrongname", "ca.pem", "127.0.0.1", NO_IP_SANS),
        ("expired", "ca.pem", "localhost", "expired"),
        ("expired", "other-ca.pem", "localhost", "expired"),
    ];
    for (cert, roots, host, want) in cases {
        let mut server = Command::new(tmp.join("server"))
            .arg(data.join(format!("{cert}.pem")))
            .arg(data.join(format!("{cert}.key")))
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut port = String::new();
        BufReader::new(server.stdout.take().unwrap())
            .read_line(&mut port)
            .unwrap();
        let url = format!("https://{host}:{}/", port.trim());
        let o = Command::new(&go_fung)
            .arg("run")
            .arg(tmp.join("get.fung"))
            .arg(&url)
            .env("SSL_CERT_FILE", data.join(roots))
            // an empty directory: only SSL_CERT_FILE's roots
            .env("SSL_CERT_DIR", tmp.join("certs"))
            .output()
            .unwrap();
        // Stage 4d Task 3: the Rust `http` meets the same server, and its
        // output must be Go's (the expiry check's "current time" aside).
        let r = Command::new(common::rust_fung())
            .arg("run")
            .arg(tmp.join("get.fung"))
            .arg(&url)
            .env("SSL_CERT_FILE", data.join(roots))
            .env("SSL_CERT_DIR", tmp.join("certs"))
            .output()
            .unwrap();
        let _ = server.kill();
        let _ = server.wait();
        assert_eq!(
            (
                without_now(&String::from_utf8_lossy(&r.stdout)),
                String::from_utf8_lossy(&r.stderr),
                r.status.code()
            ),
            (
                without_now(&String::from_utf8_lossy(&o.stdout)),
                String::from_utf8_lossy(&o.stderr),
                o.status.code()
            ),
            "{cert} {roots} {host}: rust and go differ"
        );
        let got = String::from_utf8_lossy(&o.stdout);
        let got = got.trim_end();
        let case = format!("{cert} {roots} {host}: {got:?} {:?}", o.stderr);
        if want.starts_with("200") {
            assert_eq!(got, want, "{case}");
            continue;
        }
        // cannot request `<url>`: Get "<url>": <reason>
        let prefix = format!("cannot request `{url}`: Get \"{url}\": ");
        let reason = got
            .strip_prefix(&prefix)
            .unwrap_or_else(|| panic!("{case}"));
        if want == "expired" {
            assert!(
                reason.starts_with(EXPIRED_PREFIX) && reason.ends_with(EXPIRED_SUFFIX),
                "{case}"
            );
        } else {
            assert_eq!(reason, want, "{case}");
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::remove_file(go_fung);
}

/// The text with the time of an expiry error ("current time <t> is
/// after") left out: the two runs happen at different seconds.
fn without_now(s: &str) -> String {
    match (s.find("current time "), s.find(" is after")) {
        (Some(a), Some(b)) if a < b => format!("{}{}", &s[..a + 13], &s[b..]),
        _ => s.to_string(),
    }
}
