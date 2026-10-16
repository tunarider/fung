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
    s.set_write_timeout(Some(Duration::from_secs(60))).unwrap();
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
        // userinfo becomes Basic authorization unless the user set one
        case(
            r#"print(http.get("http://user:pass@127.0.0.1:PORT/a"))
print(http.get("http://us%20er@127.0.0.1:PORT/b"))
print(http.get("http://only@127.0.0.1:PORT/c"))
print(http.get("http://u:@127.0.0.1:PORT/d"))
print(http.get("http://u:p%3Aw@127.0.0.1:PORT/e", headers: {"authorization": "Bearer t"}))
print(http.get("http://u:p@127.0.0.1:PORT/f", headers: {"Authorization": ""}))
print(http.get("http://%ED%95%9C:%EA%B8%80@127.0.0.1:PORT/g"))"#,
            ok(simple),
        ),
        // an empty port is dropped (Go `removeEmptyPort`)
        case(r#"print(http.get("http://127.0.0.1:/x"))"#, ok(simple)),
        // a chunk size's leading zeros count as digits in Go: 16 digits
        // are fine, 17 are too many
        case(
            r#"print(http.get(URL))"#,
            ok(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0000000000000002\r\nab\r\n0\r\n\r\n",
            ),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n00000000000000002\r\nab\r\n0\r\n\r\n",
            ),
        ),
        // trailer lines are checked as header lines
        case(
            r#"print(http.get(URL))"#,
            ok(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n0\r\nbad trailer\r\n\r\n",
            ),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n0\r\n folded: x\r\n\r\n",
            ),
        ),
        case(
            r#"print(http.get(URL))"#,
            ok(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n0\r\nX: 1\r\nY: 2\r\n",
            ),
        ),
        // folded header lines
        case(
            r#"print(http.get(URL))"#,
            ok("HTTP/1.1 200 OK\r\nX-Fold: a\r\n  b\r\n\tc  \r\nContent-Length: 0\r\n\r\n"),
        ),
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
    r#"print(http.get("http://u:secret@127.0.0.1:REFUSED/"))"#,
    r#"print(http.get("http://us%20er:secret@127.0.0.1:REFUSED/"))"#,
    // an IPv6 zone is kept, shown as %25 (never dialled: the port is bad)
    r#"print(http.get("http://[fe80::1%25en0]:99999/"))"#,
    r#"print(http.get("https://127.0.0.1:/"))"#,
];

/// Argument errors: E0403 at the user's call, in Go's order (the URL is
/// read before the headers' values). `(program, the error and excerpt
/// both print, Go's call path, Rust's call path)`. Go's `http` functions
/// are natives and leave no frame; the Rust ones are fung functions whose
/// frames stay visible, like every fung extras function's (06; a 4d known
/// difference). In Rust's call path `@needle@` stands for the line of
/// `extras/rust/http.fung` that holds `needle`.
const ARG_ERRORS: &[(&str, &str, &str, &str)] = &[
    (
        "print(1)\nhttp.get(1)",
        "error[E0403]: argument `url` must be str, got int\n  --> -e:3:9\n  |\n3 | http.get(1)\n  |         ^\n  |\n",
        "   -e:3       <entry>\n",
        "   extras/rust/http.fung:@fn get(@ get\n   -e:3       <entry>\n",
    ),
    (
        "fn f() = print(http.post(\"x\", 1))\nf()",
        "error[E0403]: argument `body` must be str | nil, got int\n  --> -e:2:25\n  |\n2 | fn f() = print(http.post(\"x\", 1))\n  |                         ^\n  |\n",
        "   -e:2       f\n   -e:3       <entry>\n",
        "   extras/rust/http.fung:@fn post(@ post\n   -e:2       f\n   -e:3       <entry>\n",
    ),
    (
        "http.get(\"http://x\", headers: {a: 1})",
        "error[E0403]: argument `headers` must be a map of strings, got a map holding int\n  --> -e:2:9\n  |\n2 | http.get(\"http://x\", headers: {a: 1})\n  |         ^\n  |\n",
        "   -e:2       <entry>\n",
        "   extras/rust/http.fung:@need_str_map(\"headers\"@ _attempt\n   extras/rust/http.fung:@match _attempt(@ _request (1 tail calls elided)\n   -e:2       <entry>\n",
    ),
    (
        "http.get(\"http://x\", headers: 1)",
        "error[E0403]: argument `headers` must be map, got int\n  --> -e:2:9\n  |\n2 | http.get(\"http://x\", headers: 1)\n  |         ^\n  |\n",
        "   -e:2       <entry>\n",
        "   extras/rust/http.fung:@fn get(@ get\n   -e:2       <entry>\n",
    ),
    (
        "http.get(\"http://x\", timeout: 1)",
        "error[E0403]: argument `timeout` must be dur, got int\n  --> -e:2:9\n  |\n2 | http.get(\"http://x\", timeout: 1)\n  |         ^\n  |\n",
        "   -e:2       <entry>\n",
        "   extras/rust/http.fung:@fn get(@ get\n   -e:2       <entry>\n",
    ),
    (
        "http.request(1, \"x\")",
        "error[E0403]: argument `method` must be str, got int\n  --> -e:2:13\n  |\n2 | http.request(1, \"x\")\n  |             ^\n  |\n",
        "   -e:2       <entry>\n",
        "   extras/rust/http.fung:@fn request(@ request\n   -e:2       <entry>\n",
    ),
    (
        "http.request(\"GET\", 1)",
        "error[E0403]: argument `url` must be str, got int\n  --> -e:2:13\n  |\n2 | http.request(\"GET\", 1)\n  |             ^\n  |\n",
        "   -e:2       <entry>\n",
        "   extras/rust/http.fung:@fn request(@ request\n   -e:2       <entry>\n",
    ),
    (
        "http.request(\"GET\", \"x\", 1)",
        "error[E0403]: argument `body` must be str | nil, got int\n  --> -e:2:13\n  |\n2 | http.request(\"GET\", \"x\", 1)\n  |             ^\n  |\n",
        "   -e:2       <entry>\n",
        "   extras/rust/http.fung:@fn request(@ request\n   -e:2       <entry>\n",
    ),
    // the call itself is wrong: no frame of http.fung yet
    (
        "http.get()",
        "error[E0407]: missing argument `url`\n  --> -e:2:9\n  |\n2 | http.get()\n  |         ^\n  |\n",
        "   -e:2       <entry>\n",
        "   -e:2       <entry>\n",
    ),
    (
        "http.get(\"x\", nope: 1)",
        "error[E0406]: get has no argument named `nope`\n  --> -e:2:9\n  |\n2 | http.get(\"x\", nope: 1)\n  |         ^\n  |\n",
        "   -e:2       <entry>\n",
        "   -e:2       <entry>\n",
    ),
];

/// Programs whose output is the same in both, without a server.
const PLAIN: &[&str] = &[
    "http.get(\"::bad\", headers: {a: 1})",
    "print(http, http.get, http.post, http.request, type_of(http.get))",
];

/// Rust's call path with each `@needle@` replaced by its line number in
/// `extras/rust/http.fung`.
fn rust_frames(template: &str) -> String {
    let src = std::fs::read_to_string(common::repo().join("extras/rust/http.fung")).unwrap();
    let mut out = String::new();
    let mut parts = template.split('@');
    out.push_str(parts.next().unwrap());
    while let (Some(needle), Some(rest)) = (parts.next(), parts.next()) {
        let line = src
            .lines()
            .position(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("no `{needle}` in http.fung"));
        out.push_str(&(line + 1).to_string());
        out.push_str(rest);
    }
    out
}

type Out = (String, String, Option<i32>);

/// How long one run may take: a hang is a failure, not a stuck test.
const CHILD_LIMIT: Duration = Duration::from_secs(30);

fn run(bin: &Path, src: &str) -> Out {
    let mut child = std::process::Command::new(bin)
        .args(["-e", src])
        .current_dir(common::repo())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if start.elapsed() > CHILD_LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{} ran over {CHILD_LIMIT:?} on:\n{src}", bin.display());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let o = child.wait_with_output().unwrap();
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
    for (prog, head, go_frames, rust_template) in ARG_ERRORS {
        let src = format!("import http\n{prog}");
        let path = " call path (innermost first):\n";
        let want_go = ("".to_string(), format!("{head}{path}{go_frames}"), Some(1));
        let want_rust = (
            if prog.starts_with("print(1)") {
                "1\n"
            } else {
                ""
            }
            .to_string(),
            format!("{head}{path}{}", rust_frames(rust_template)),
            Some(1),
        );
        let want_go = (want_rust.0.clone(), want_go.1, want_go.2);
        let (g, r) = (run(&go, &src), run(&rust, &src));
        if g != want_go {
            failures.push(format!("{src}\n  go:   {g:?}\n  want: {want_go:?}"));
        }
        if r != want_rust {
            failures.push(format!("{src}\n  rust: {r:?}\n  want: {want_rust:?}"));
        }
    }
    for prog in PLAIN {
        let src = format!("import http\n{prog}");
        let (g, r) = (run(&go, &src), run(&rust, &src));
        if g != r {
            failures.push(format!("{src}\n  go:   {g:?}\n  rust: {r:?}"));
        }
    }
    // A known difference (4d): a user function whose last call is
    // `http.get` is replaced by it (a tail call to a fung function), so
    // the error points at that function's caller and its frame is gone
    // from the path. Go's native replaces no frame.
    let src = "import http\nfn g(x) = http.get(x)\nfn h() = print(g(1))\nh()";
    let go_want = "error[E0403]: argument `url` must be str, got int\n  --> -e:2:19\n  |\n2 | fn g(x) = http.get(x)\n  |                   ^\n  |\n call path (innermost first):\n   -e:2       g\n   -e:3       h\n   -e:4       <entry>\n";
    let rust_want = format!(
        "error[E0403]: argument `url` must be str, got int\n  --> -e:3:17\n  |\n3 | fn h() = print(g(1))\n  |                 ^\n  |\n call path (innermost first):\n{}",
        rust_frames(
            "   extras/rust/http.fung:@fn get(@ get (1 tail calls elided)\n   -e:3       h\n   -e:4       <entry>\n"
        )
    );
    let (g, r) = (run(&go, src), run(&rust, src));
    if g != (String::new(), go_want.to_string(), Some(1)) {
        failures.push(format!("tail call, go: {g:?}"));
    }
    if r != (String::new(), rust_want.clone(), Some(1)) {
        failures.push(format!("tail call, rust: {r:?}\n  want: {rust_want:?}"));
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
    let r = run(&rust_fung(), src);
    if !g.0.contains("lookup nosuchhost.invalid: no such host\"") {
        // Go's resolver words it differently here (Linux, or no DNS):
        // Rust still names the failed lookup, through the system resolver.
        eprintln!("no_such_host_matches_go: Go comparison skipped, Go said {g:?}");
        assert!(r.0.contains("lookup nosuchhost.invalid: "), "{r:?}");
        return;
    }
    assert_eq!(r, g);
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

/// Release only (a debug build says nothing about the shape): 400,000
/// continuation lines of one header gather in linear time: the pieces are
/// joined once, not grown line by line (that took 7.8 s here; this
/// takes under 1 s).
#[cfg(not(debug_assertions))]
#[test]
fn many_folded_lines_stay_linear() {
    let mut reply = bytes("HTTP/1.1 200 OK\r\nX-Fold: a\r\n");
    reply.extend(b" b\r\n".repeat(400_000));
    reply.extend_from_slice(b"Content-Length: 0\r\n\r\n");
    let fake = Fake::start(Reply::Bytes(reply));
    let src = format!(
        "import http\nr = http.get(\"http://127.0.0.1:{}/\")!\nprint(r.status, len(r.headers[\"x-fold\"]))",
        fake.port
    );
    let start = std::time::Instant::now();
    let got = run(&rust_fung(), &src);
    let took = start.elapsed();
    assert_eq!(got, ("200 800001\n".to_string(), String::new(), Some(0)));
    assert!(took < Duration::from_secs(4), "took {took:?}");
}
