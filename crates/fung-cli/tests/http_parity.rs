//! The `http` client run by the Go and the Rust binaries against the same
//! fake servers (stage 4d, Task 3): status, headers and body, the three
//! ways a body ends (`Content-Length`, chunked, the connection closing),
//! responses without a body, multi-byte text split across reads, the
//! request each client writes, timeouts, refused connections, bad URLs,
//! argument errors, and malformed responses.
//!
//! The fake server is plain `std::net` code in this test: it reads one
//! request (headers and a `Content-Length` body), records its raw bytes,
//! and answers with a scripted reply. Both binaries meet the same server,
//! so their output (stdout, stderr, exit status) and the requests they
//! wrote are compared. Go's transport adds `Accept-Encoding: gzip` to
//! every request (it then unzips the answer itself); the Rust client asks
//! for no compression (06 "fung HTTP 약속"), so that one line is removed
//! from Go's request before the comparison. No reply here is compressed.

mod common;

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{go_fung, rust_fung};

/// What the fake server does once it has read a request.
#[derive(Clone)]
enum Reply {
    /// Writes the bytes, then ends its side of the connection.
    Bytes(Vec<u8>),
    /// Writes the bytes, then waits for the client to give up (it never
    /// sends the rest).
    Hold(Vec<u8>),
    /// Writes the bytes, then the byte `a` until the client goes away.
    Endless(Vec<u8>),
}

/// A fake server on a port of its own; it serves until the test ends.
struct Fake {
    port: u16,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl Fake {
    fn start(reply: Reply) -> Fake {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(s) = s else { return };
                let (reply, seen) = (reply.clone(), seen.clone());
                std::thread::spawn(move || answer(s, &reply, &seen));
            }
        });
        Fake { port, requests }
    }

    /// The requests read so far, emptied.
    fn take(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.requests.lock().unwrap())
    }
}

/// Reads one request, records it, and replies. A client that hangs never
/// holds a thread past 60 s.
fn answer(mut s: TcpStream, reply: &Reply, seen: &Mutex<Vec<Vec<u8>>>) {
    s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
    let mut req = Vec::new();
    let mut buf = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = req.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        match s.read(&mut buf) {
            Ok(0) | Err(_) => {
                seen.lock().unwrap().push(req);
                return;
            }
            Ok(n) => req.extend_from_slice(&buf[..n]),
        }
    };
    let head = String::from_utf8_lossy(&req[..head_end]).to_lowercase();
    let body_len: usize = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .map_or(0, |v| v.trim().parse().unwrap());
    while req.len() < head_end + body_len {
        match s.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => req.extend_from_slice(&buf[..n]),
        }
    }
    seen.lock().unwrap().push(req);
    match reply {
        Reply::Bytes(b) => {
            let _ = s.write_all(b);
            let _ = s.shutdown(Shutdown::Write);
            drain(&mut s);
        }
        Reply::Hold(b) => {
            let _ = s.write_all(b);
            drain(&mut s);
        }
        Reply::Endless(b) => {
            let _ = s.write_all(b);
            let chunk = vec![b'a'; 64 * 1024];
            while s.write_all(&chunk).is_ok() {}
        }
    }
}

/// Reads until the client closes (so closing here never resets a
/// connection with unread data).
fn drain(s: &mut TcpStream) {
    let mut buf = [0u8; 4096];
    while matches!(s.read(&mut buf), Ok(n) if n > 0) {}
}

fn bytes(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

/// A program with `URL` standing for `http://127.0.0.1:<port>`, and the
/// server's reply.
struct Case {
    prog: &'static str,
    reply: Reply,
}

fn case(prog: &'static str, reply: Reply) -> Case {
    Case { prog, reply }
}

fn ok(s: &str) -> Reply {
    Reply::Bytes(bytes(s))
}

/// "한글" is ED 95 9C EA B8 80: each chunk below cuts a character.
fn hangul_chunks() -> Vec<u8> {
    let mut b = bytes("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n");
    for part in [
        &[0xED, 0x95][..],
        &[0x9C, 0xEA][..],
        &[0xB8][..],
        &[0x80, b'!'][..],
    ] {
        b.extend_from_slice(format!("{:x}\r\n", part.len()).as_bytes());
        b.extend_from_slice(part);
        b.extend_from_slice(b"\r\n");
    }
    b.extend_from_slice(b"0\r\n\r\n");
    b
}

fn cases() -> Vec<Case> {
    let simple = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
    vec![
        // status, headers (lower case, sorted, repeated names joined), body
        case(
            r#"print(http.get(URL + "/a/b?x=1&y=2#frag"))"#,
            ok(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nX-Multi: a\r\nx-multi: b\r\nX-Sp:   v w  \r\nx-lower:x\r\nContent-Length: 5\r\n\r\nhello",
            ),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nnot found"),
        ),
        case(
            r#"print(http.get(URL + "/"))"#,
            ok("HTTP/1.1 200\r\nContent-Length: 0\r\n\r\n"),
        ),
        case(
            r#"print(http.get(URL + "/"))"#,
            ok("HTTP/1.1 599 Odd\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\nx"),
        ),
        // chunked, with an extension and a trailer
        case(
            r#"print(http.get(URL))"#,
            ok(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 99\r\n\r\n5\r\nhello\r\n7;ext=1\r\n, world\r\n0\r\nX-Trailer: t\r\n\r\n",
            ),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nTransfer-Encoding: Chunked\r\n\r\nA\r\n0123456789\r\n0\r\n\r\n"),
        ),
        case(r#"print(http.get(URL))"#, Reply::Bytes(hangul_chunks())),
        // until the connection closes
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nX: 1\r\n\r\n한글 body until close"),
        ),
        case(r#"print(http.get(URL))"#, ok("HTTP/1.0 200 OK\r\n\r\nold")),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 2\r\n\r\nok"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.0 200 OK\r\nConnection: keep-alive\r\nContent-Length: 2\r\n\r\nok"),
        ),
        // no body: HEAD, 1xx, 204, 304
        case(
            r#"print(http.request("HEAD", URL))"#,
            ok("HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n"),
        ),
        case(
            r#"print(http.request("head", URL))"#,
            ok("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 204 No Content\r\nX: y\r\n\r\n"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 304 Not Modified\r\nContent-Length: 10\r\n\r\n"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok(
                "HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 103 Early Hints\r\nLink: x\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok",
            ),
        ),
        // what the client writes: Host, User-Agent, Content-Length (bytes),
        // the user's headers in canonical form, sorted, overriding defaults
        case(
            r#"print(http.post(URL + "/up", "한글 본문", headers: {"content-type": "text/plain; charset=utf-8", "user-agent": "fung-test", "x-b": "2", "X-A": " 1 ", "host": "evil.example", "content-length": "1", "x-yz_w": "u"}))"#,
            ok(simple),
        ),
        case(
            r#"print(http.post(URL, nil), http.post(URL, ""))"#,
            ok(simple),
        ),
        case(
            r#"print(http.request("put", URL + "/x", "body"))
print(http.request("DELETE", URL))
print(http.request("patch", URL))
print(http.request("PURGE", URL, "", headers: {"User-Agent": ""}))
print(http.request("", URL, headers: {"accept-encoding": "identity"}))
print(http.request("GET", URL, "x"))"#,
            ok(simple),
        ),
        case(
            r#"print(http.get(URL + "/검색/a b?q=한글&r=a b"))"#,
            ok(simple),
        ),
        case(r#"print(http.get(URL + "/a%20b/%41%zz"))"#, ok(simple)),
        case(r#"print(http.get(URL + "/a%2Fb/c(d)!*'~"))"#, ok(simple)),
        case(r#"print(http.get("HTTP://127.0.0.1:PORT?"))"#, ok(simple)),
        // malformed responses: an Err like Go's, never a hang or a panic
        case(r#"print(http.get(URL))"#, ok("garbage\r\n\r\n")),
        case(r#"print(http.get(URL))"#, ok("HTTP/1.1 abc OK\r\n\r\n")),
        case(r#"print(http.get(URL))"#, ok("HTTP/1.1 20 OK\r\n\r\n")),
        case(r#"print(http.get(URL))"#, ok("HTP/1.1 200 OK\r\n\r\n")),
        case(r#"print(http.get(URL))"#, ok("")),
        case(r#"print(http.get(URL))"#, ok("HTTP/1.1 200 OK\r\n")),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nBad header\r\n\r\n"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\n Folded: x\r\n\r\n"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\nab"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nContent-Length: abc\r\n\r\nab"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nContent-Length: +2\r\n\r\nab"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\nab"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\nab\r\n0\r\n\r\n"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n\r\nab\r\n0\r\n\r\n"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n11111111111111111\r\nab\r\n0\r\n\r\n",
            ),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabcX\r\n0\r\n\r\n"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\nab\r\n0\r\n\r\n"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nab"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n0\r\n"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nabc"),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n"),
        ),
        case(
            r#"print(http.get(URL))"#,
            Reply::Endless(bytes("HTTP/1.1 200 OK\r\nX-Endless: ")),
        ),
        case(
            r#"print(http.get(URL))"#,
            Reply::Endless(bytes(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n",
            )),
        ),
        // the timeout covers the whole request, the body included
        case(
            r#"print(http.get(URL, timeout: 300ms))"#,
            Reply::Hold(Vec::new()),
        ),
        case(
            r#"print(http.get(URL, timeout: 300ms))"#,
            Reply::Hold(bytes("HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nabc")),
        ),
        case(
            r#"print(http.get(URL, timeout: 300ms))"#,
            Reply::Hold(bytes(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n",
            )),
        ),
        case(r#"print(http.get(URL, timeout: 0s))"#, ok(simple)),
        // a task blocked in a request is cancelled; exit ends a blocked one
        case(
            r#"t = spawn(() => http.get(URL))
print(await(t, timeout: 200ms))
cancel(t)
print(await(t))
t2 = spawn(() => http.get(URL, timeout: 0s))
print(await(t2, timeout: 100ms))
exit(3)"#,
            Reply::Hold(Vec::new()),
        ),
        // the request is checked before anything is sent
        case(
            r#"print(http.request("GE T", URL))
print(http.get(URL, headers: {"a b": "x"}))
print(http.get(URL, headers: {"x": "a\r\nb"}))
print(http.get(URL, headers: {"x": "tab\tok"}))"#,
            ok(simple),
        ),
    ]
}

/// Programs that need no server: bad URLs and refused connections
/// (`REFUSED` is a port nothing listens on).
const NO_SERVER: &[&str] = &[
    r#"print(http.get("http://127.0.0.1:REFUSED/"))"#,
    r#"print(http.post("http://127.0.0.1:REFUSED/x", "b", timeout: 5s))"#,
    r#"print(http.request("delete", "http://127.0.0.1:REFUSED/"))"#,
    r#"print(http.get("ftp://x/"))"#,
    r#"print(http.get("::bad"))"#,
    r#"print(http.get("http://[::1"))"#,
    r#"print(http.get("x"))"#,
    r#"print(http.get(""))"#,
    r#"print(http.get("http://a b/"))"#,
    r#"print(http.get("http:///path"))"#,
    r#"print(http.get("http:foo"))"#,
    r#"print(http.get("http://x:y/"))"#,
    r#"print(http.get("http://127.0.0.1:99999/"))"#,
    r#"print(http.get("http://127.0.0.1:99999999999999999999999/"))"#,
    r#"print(http.get("http://%zz/"))"#,
    r#"print(http.get("http://x/%zz"))"#,
    r#"print(http.get("http://x/#%zz"))"#,
    r#"print(http.get("http://x/\u{1}"))"#,
    r#"print(http.get("a:b/c"))"#,
    r#"print(http.get("1http://x/"))"#,
    r#"print(http.get("/relative"))"#,
    r#"print(http.get("http://[::1]x/"))"#,
    r#"print(http.get("http://a[::1]/"))"#,
    r#"print(http.get("http://[nope]/"))"#,
    r#"print(http.get("http://[127.0.0.1]/"))"#,
    r#"print(http.get("http://u@[::1]:REFUSED/"))"#,
];

/// Argument errors: E0403 at the user's call, in Go's order (the URL is
/// read before the headers' values).
const ARG_ERRORS: &[&str] = &[
    "print(1)\nhttp.get(1)",
    // not in tail position: a tail call to a fung function replaces the
    // caller's frame, so the error would point at the caller's caller
    // (Go's natives replace no frame; 4d known differences)
    "fn f() = print(http.post(\"x\", 1))\nf()",
    "http.get(\"http://x\", headers: {a: 1})",
    "http.get(\"::bad\", headers: {a: 1})",
    "http.get(\"http://x\", headers: 1)",
    "http.get(\"http://x\", timeout: 1)",
    "http.request(1, \"x\")",
    "http.request(\"GET\", 1)",
    "http.request(\"GET\", \"x\", 1)",
    "http.get()",
    "http.get(\"x\", nope: 1)",
    "print(http, http.get, http.post, http.request, type_of(http.get))",
];

type Out = (String, String, Option<i32>);

fn run(bin: &Path, src: &str) -> Out {
    let o = std::process::Command::new(bin)
        .args(["-e", src])
        .current_dir(common::repo())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        o.status.code(),
    )
}

/// Go's request without the `Accept-Encoding: gzip` line its transport adds.
fn without_gzip(req: &[u8]) -> String {
    String::from_utf8_lossy(req).replace("Accept-Encoding: gzip\r\n", "")
}

/// The call path lines of frames inside the embedded `http.fung`: Go's
/// `http` functions are natives and leave no frame, the Rust ones are
/// fung functions whose frames stay visible in the call path, like those
/// of every fung extras function (06).
fn without_extras_frames(err: &str) -> String {
    err.lines()
        .filter(|l| !l.trim_start().starts_with("extras/http.fung:"))
        .map(|l| format!("{l}\n"))
        .collect()
}

#[test]
fn http_client_matches_go() {
    let Some(go) = go_fung("http") else {
        return;
    };
    let rust = rust_fung();
    let mut failures = Vec::new();
    for c in cases() {
        let fake = Fake::start(c.reply.clone());
        let src = format!(
            "import http\n{}",
            c.prog
                .replace("URL", &format!("\"http://127.0.0.1:{}\"", fake.port))
                .replace("PORT", &fake.port.to_string())
        );
        let g = run(&go, &src);
        let g_reqs: Vec<String> = fake.take().iter().map(|r| without_gzip(r)).collect();
        let r = run(&rust, &src);
        let r_reqs: Vec<String> = fake
            .take()
            .iter()
            .map(|r| String::from_utf8_lossy(r).into_owned())
            .collect();
        if g != r {
            failures.push(format!("{src}\n  go:   {g:?}\n  rust: {r:?}"));
        } else if g_reqs != r_reqs {
            failures.push(format!(
                "{src}\n  go wrote:   {g_reqs:?}\n  rust wrote: {r_reqs:?}"
            ));
        }
    }
    let refused = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    for prog in NO_SERVER {
        let src = format!(
            "import http\n{}",
            prog.replace("REFUSED", &refused.to_string())
        );
        let (g, r) = (run(&go, &src), run(&rust, &src));
        if g != r {
            failures.push(format!("{src}\n  go:   {g:?}\n  rust: {r:?}"));
        }
    }
    for prog in ARG_ERRORS {
        let src = format!("import http\n{prog}");
        let g = run(&go, &src);
        let r = run(&rust, &src);
        let r = (r.0, without_extras_frames(&r.1), r.2);
        if g != r {
            failures.push(format!("{src}\n  go:   {g:?}\n  rust: {r:?}"));
        }
    }
    let _ = std::fs::remove_file(go);
    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A name that does not resolve: compared only where Go's resolver says
/// `no such host` (a box without DNS, or Go's own resolver on Linux,
/// words it differently; 4d known differences).
#[test]
fn no_such_host_matches_go() {
    let Some(go) = go_fung("http-dns") else {
        return;
    };
    let src = "import http\nprint(http.get(\"http://nosuchhost.invalid/x\"))";
    let g = run(&go, src);
    let _ = std::fs::remove_file(&go);
    if !g.0.contains("lookup nosuchhost.invalid: no such host\"") {
        eprintln!("no_such_host_matches_go: skipped, Go said {g:?}");
        return;
    }
    assert_eq!(run(&rust_fung(), src), g);
}

/// Rust only: a body may hold at most 64 MB (06; Go's client has no
/// limit, so it would read an endless body forever). The limit holds for
/// a body read to the end of the connection, a declared length and a
/// chunk size, without reading what is past it.
#[test]
fn bodies_past_the_limit_are_errors() {
    let rust = rust_fung();
    let cases = [
        Reply::Endless(bytes("HTTP/1.1 200 OK\r\n\r\n")),
        Reply::Hold(bytes("HTTP/1.1 200 OK\r\nContent-Length: 67108865\r\n\r\n")),
        Reply::Hold(bytes(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\na\r\n4000000\r\n",
        )),
    ];
    for reply in cases {
        let fake = Fake::start(reply);
        let url = format!("http://127.0.0.1:{}/", fake.port);
        let got = run(&rust, &format!("import http\nprint(http.get(\"{url}\"))"));
        let want = format!(
            "Err({{kind: 'network, message: \"cannot request `{url}`: http: response body longer than 67108864 bytes\"}})\n"
        );
        assert_eq!(got, (want, String::new(), Some(0)));
    }
}
