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
//!
//! Task 4 adds redirects and proxies. Their fake servers route by the
//! request target, so one server can redirect several times, and a fake
//! proxy can play many hosts: names such as `example.test` are never
//! resolved, only written into the request line the proxy reads. Proxy
//! variables are set on the child processes alone (`run_env`, which also
//! removes any the test process has). https through a proxy goes through
//! a fake proxy's tunnel to a Go TLS server (`TLS_SERVER`).

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
    /// Waits, then gives the reply (Task 4: redirects share the timeout).
    Slow(Duration, Box<Reply>),
    /// The reply for the request target (the second word of the request
    /// line: a path, an absolute URL through a proxy, or `host:port` of a
    /// CONNECT); `*` is the reply for any other target (Task 4).
    Routes(Arc<Vec<(String, Reply)>>),
    /// A proxy's tunnel: answers the CONNECT with 200, then carries bytes
    /// both ways between the client and 127.0.0.1:<port> (Task 4).
    Tunnel(u16),
}

/// A fake server on a port of its own; it serves until the test ends.
struct Fake {
    port: u16,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    /// Words replaced in the replies (Task 4: other servers' ports).
    words: Words,
}

type Words = Arc<Mutex<Vec<(String, String)>>>;

impl Fake {
    fn start(reply: Reply) -> Fake {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        let words: Words = Arc::new(Mutex::new(vec![("SELFPORT".to_string(), port.to_string())]));
        let w = words.clone();
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(s) = s else { return };
                let (reply, seen, w) = (reply.clone(), seen.clone(), w.clone());
                std::thread::spawn(move || answer(s, &reply, &seen, &w));
            }
        });
        Fake {
            port,
            requests,
            words,
        }
    }

    /// The requests read so far, emptied.
    fn take(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.requests.lock().unwrap())
    }
}

/// Reads one request, records it, and replies (with the server's words
/// replaced: `SELFPORT` is its own port). A client that hangs never holds
/// a thread past 60 s.
fn answer(mut s: TcpStream, reply: &Reply, seen: &Mutex<Vec<Vec<u8>>>, words: &Words) {
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
    let target = String::from_utf8_lossy(&req)
        .split(' ')
        .nth(1)
        .unwrap_or("")
        .to_string();
    seen.lock().unwrap().push(req);
    give(&mut s, reply, &target, words);
}

/// `b` with the words replaced.
fn local(b: &[u8], words: &Words) -> Vec<u8> {
    let mut s = String::from_utf8_lossy(b).into_owned();
    for (k, v) in words.lock().unwrap().iter() {
        s = s.replace(k, v);
    }
    s.into_bytes()
}

/// Writes the reply to a request for `target`.
fn give(s: &mut TcpStream, reply: &Reply, target: &str, words: &Words) {
    match reply {
        Reply::Slow(d, r) => {
            std::thread::sleep(*d);
            give(s, r, target, words);
        }
        Reply::Routes(rs) => {
            let r = rs
                .iter()
                .find(|(t, _)| t == target)
                .or_else(|| rs.iter().find(|(t, _)| t == "*"))
                .map(|(_, r)| r.clone())
                .unwrap_or_else(|| ok("HTTP/1.1 500 No Route\r\nContent-Length: 0\r\n\r\n"));
            give(s, &r, target, words);
        }
        Reply::Tunnel(to) => {
            let _ = s.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n");
            let Ok(up) = TcpStream::connect(("127.0.0.1", *to)) else {
                return;
            };
            let (mut a, mut b) = (s.try_clone().unwrap(), up.try_clone().unwrap());
            let t = std::thread::spawn(move || {
                let _ = std::io::copy(&mut a, &mut b);
                let _ = b.shutdown(Shutdown::Write);
            });
            let (mut up, mut down) = (up, s.try_clone().unwrap());
            let _ = std::io::copy(&mut up, &mut down);
            let _ = down.shutdown(Shutdown::Write);
            let _ = t.join();
        }
        Reply::Bytes(b) => {
            let b = &local(b, words);
            let _ = s.write_all(b);
            let _ = s.shutdown(Shutdown::Write);
            drain(s);
        }
        Reply::Hold(b) => {
            let _ = s.write_all(&local(b, words));
            drain(s);
        }
        Reply::Endless(b) => {
            let _ = s.write_all(&local(b, words));
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
    let o = run_env(bin, src, &[]);
    assert!(
        o.1 != RAN_OVER,
        "{} ran over {CHILD_LIMIT:?} on:\n{src}",
        bin.display()
    );
    o
}

/// The standard error of a run killed at `CHILD_LIMIT`.
const RAN_OVER: &str = "<killed: ran over the limit>";

/// The variables the client reads to choose a proxy (Go
/// `httpproxy.FromEnvironment`): a run sees only those it is given.
const PROXY_VARS: &[&str] = &[
    "HTTP_PROXY",
    "http_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "NO_PROXY",
    "no_proxy",
    "REQUEST_METHOD",
];

/// `run` with environment variables set on the child alone.
fn run_env(bin: &Path, src: &str, env: &[(String, String)]) -> Out {
    let mut cmd = std::process::Command::new(bin);
    for v in PROXY_VARS {
        cmd.env_remove(v);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd
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
            return (String::new(), RAN_OVER.to_string(), None);
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

// ---- Task 4: redirects and proxies ------------------------------------

/// A redirect (`Connection: close`: one request per connection, so Go
/// never reuses one the fake server is closing).
fn redirect(code: u16, loc: &str) -> Reply {
    ok(&format!(
        "HTTP/1.1 {code} Moved\r\nLocation: {loc}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
    ))
}

/// A 200 answer whose body is `name`.
fn named(name: &str) -> Reply {
    ok(&format!(
        "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{name}",
        name.len()
    ))
}

fn routes(rs: Vec<(&str, Reply)>) -> Reply {
    Reply::Routes(Arc::new(
        rs.into_iter().map(|(t, r)| (t.to_string(), r)).collect(),
    ))
}

/// A program run against several fake servers with environment variables
/// set on the child alone. In the program and the variables' values,
/// `PORTn` is server n's port, `URL` is `"http://127.0.0.1:PORT0"`, and
/// the test's own words (`REFUSED`, `TLSPORT`, ...) are filled in first.
struct Multi {
    prog: String,
    servers: Vec<Reply>,
    env: Vec<(String, String)>,
}

fn multi(prog: &str, servers: Vec<Reply>, env: &[(&str, &str)]) -> Multi {
    Multi {
        prog: prog.to_string(),
        servers,
        env: env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

fn fill(s: &str, ports: &[u16], words: &[(&str, String)]) -> String {
    let mut s = s.replace("URL", "\"http://127.0.0.1:PORT0\"");
    for (k, v) in words {
        s = s.replace(k, v);
    }
    for (i, p) in ports.iter().enumerate().rev() {
        s = s.replace(&format!("PORT{i}"), &p.to_string());
    }
    s
}

/// A port nothing listens on.
fn refused_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// Runs each case with both binaries: the outputs and the requests every
/// server got must be the same. The differences, described.
fn compare(go: &Path, cases: Vec<Multi>, words: &[(&str, String)]) -> Vec<String> {
    let rust = rust_fung();
    let mut words = words.to_vec();
    words.push(("REFUSED", refused_port().to_string()));
    let mut failures = Vec::new();
    for c in cases {
        let fakes: Vec<Fake> = c.servers.into_iter().map(Fake::start).collect();
        let ports: Vec<u16> = fakes.iter().map(|f| f.port).collect();
        for f in &fakes {
            let mut w = f.words.lock().unwrap();
            w.extend(words.iter().map(|(k, v)| (k.to_string(), v.clone())));
            for (i, p) in ports.iter().enumerate().rev() {
                w.push((format!("PORT{i}"), p.to_string()));
            }
        }
        let src = format!("import http\n{}", fill(&c.prog, &ports, &words));
        let env: Vec<(String, String)> = c
            .env
            .iter()
            .map(|(k, v)| (k.clone(), fill(v, &ports, &words)))
            .chain(
                words
                    .iter()
                    .filter(|(k, _)| k.starts_with("SSL_"))
                    .map(|(k, v)| (k.to_string(), v.clone())),
            )
            .collect();
        let g = run_env(go, &src, &env);
        let g_reqs: Vec<Vec<String>> = fakes
            .iter()
            .map(|f| f.take().iter().map(|r| without_gzip(r)).collect())
            .collect();
        let r = run_env(&rust, &src, &env);
        let r_reqs: Vec<Vec<String>> = fakes
            .iter()
            .map(|f| {
                f.take()
                    .iter()
                    .map(|r| String::from_utf8_lossy(r).into_owned())
                    .collect()
            })
            .collect();
        if g != r || g.1 == RAN_OVER {
            failures.push(format!(
                "{src}\n  env: {env:?}\n  go:   {g:?}\n  rust: {r:?}"
            ));
        } else if g_reqs != r_reqs {
            failures.push(format!(
                "{src}\n  env: {env:?}\n  go wrote:   {g_reqs:?}\n  rust wrote: {r_reqs:?}"
            ));
        }
    }
    failures
}

fn report(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The whole matrix of methods each redirect code is tried with: what
/// the second request is (method, body, `Content-Length`, the body's
/// headers) shows in the requests the server got.
const METHODS: &str = r#"print(http.get(URL + "/start"))
print(http.post(URL + "/start", "한글 data", headers: {"content-type": "text/plain", "content-language": "ko", "content-encoding": "identity", "content-location": "/x", "x-keep": "1"}))
print(http.request("HEAD", URL + "/start"))
print(http.request("put", URL + "/start", "b"))
print(http.request("DELETE", URL + "/start"))
print(http.request("POST", URL + "/start"))
print(http.request("GET", URL + "/start", "get body"))"#;

/// `/r/0` redirects to `/r/1` and so on up to `/r/<last>`, which answers.
fn chain(last: usize) -> Reply {
    let mut rs: Vec<(String, Reply)> = (0..last)
        .map(|i| (format!("/r/{i}"), redirect(302, &format!("/r/{}", i + 1))))
        .collect();
    rs.push((format!("/r/{last}"), named("end")));
    Reply::Routes(Arc::new(rs))
}

/// Locations resolved against `/a/b/s<i>?k=v` (Go `URL.Parse` and
/// `ResolveReference`).
const LOCATIONS: &[&str] = &[
    "end",
    "../x",
    "../../../../y",
    "./z?q=1",
    "?only=query",
    "/abs/./p/../q",
    "//127.0.0.1:SELFPORT/net",
    "http://127.0.0.1:SELFPORT/full?a=b",
    "HTTP://127.0.0.1:SELFPORT/upper",
    "/a b/검색?q=한 글",
    ".",
    "..",
    "/x/.",
    "c/..",
    "/%41%2F/",
    "http://u:p%20w@127.0.0.1:SELFPORT/userinfo",
    "#frag",
];

fn redirect_cases() -> Vec<Multi> {
    let mut v = Vec::new();
    for code in [301, 302, 303, 307, 308] {
        v.push(multi(
            METHODS,
            vec![routes(vec![
                ("/start", redirect(code, "/end")),
                ("*", named("end")),
            ])],
            &[],
        ));
    }
    // codes Go does not follow, and 3xx answers without a Location
    v.push(multi(
        r#"print(http.get(URL + "/300"))
print(http.get(URL + "/304"))
print(http.get(URL + "/305"))
print(http.get(URL + "/306"))
print(http.get(URL + "/309"))
print(http.get(URL + "/none"))
print(http.get(URL + "/empty"))
print(http.request("HEAD", URL + "/none"))"#,
        vec![routes(vec![
            ("/300", redirect(300, "/end")),
            ("/304", redirect(304, "/end")),
            ("/305", redirect(305, "/end")),
            ("/306", redirect(306, "/end")),
            ("/309", redirect(309, "/end")),
            (
                "/none",
                ok("HTTP/1.1 302 Found\r\nContent-Length: 5\r\n\r\nfound"),
            ),
            (
                "/empty",
                ok("HTTP/1.1 301 Moved\r\nLocation: \r\nX: y\r\nContent-Length: 2\r\n\r\nno"),
            ),
            ("*", named("end")),
        ])],
        &[],
    ));
    // relative and absolute Locations
    let mut rs: Vec<(String, Reply)> = LOCATIONS
        .iter()
        .enumerate()
        .map(|(i, l)| (format!("/a/b/s{i}?k=v"), redirect(302, l)))
        .collect();
    rs.push(("*".to_string(), named("end")));
    let prog: Vec<String> = (0..LOCATIONS.len())
        .map(|i| format!("print(http.get(URL + \"/a/b/s{i}?k=v\"))"))
        .collect();
    v.push(Multi {
        prog: prog.join("\n"),
        servers: vec![Reply::Routes(Arc::new(rs))],
        env: Vec::new(),
    });
    // Locations that fail: they do not parse, or the next request cannot
    // be made
    v.push(multi(
        r#"print(http.get(URL + "/bad1"))
print(http.get("http://u:secret@127.0.0.1:PORT0/bad2"))
print(http.post(URL + "/bad3", "x"))
print(http.get(URL + "/bad4"))
print(http.get(URL + "/bad5"))"#,
        vec![routes(vec![
            ("/bad1", redirect(302, "http://[::1")),
            ("/bad2", redirect(302, "/%zz")),
            ("/bad3", redirect(307, "ftp://x/y")),
            ("/bad4", redirect(302, "http:opaque")),
            ("/bad5", redirect(302, "http://127.0.0.1:REFUSED/gone")),
            ("*", named("end")),
        ])],
        &[],
    ));
    // the body of a redirect is never read: an endless one, one cut short
    v.push(multi(
        r#"print(http.get(URL + "/endless"))
print(http.get(URL + "/short"))
print(http.get(URL + "/chunks"))"#,
        vec![routes(vec![
            (
                "/endless",
                Reply::Endless(bytes("HTTP/1.1 302 Found\r\nLocation: /end\r\n\r\n")),
            ),
            (
                "/short",
                ok("HTTP/1.1 301 Moved\r\nLocation: /end\r\nContent-Length: 10\r\n\r\nabc"),
            ),
            (
                "/chunks",
                ok(
                    "HTTP/1.1 303 See\r\nLocation: /end\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n",
                ),
            ),
            ("*", named("end")),
        ])],
        &[],
    ));
    // ten requests at most: nine redirects pass, the tenth fails; a loop
    v.push(multi(
        r#"print(http.get(URL + "/r/0"))"#,
        vec![chain(9)],
        &[],
    ));
    v.push(multi(
        r#"print(http.request("HEAD", URL + "/r/0"))"#,
        vec![chain(10)],
        &[],
    ));
    v.push(multi(
        r#"print(http.post(URL + "/loop", "x"))"#,
        vec![routes(vec![("*", redirect(307, "/loop"))])],
        &[],
    ));
    // Referer: the last URL without its userinfo (the fragment kept), a
    // Referer the user set wins; userinfo becomes Basic authorization on
    // every request whose URL holds it
    v.push(multi(
        r#"print(http.get("http://u:p@127.0.0.1:PORT0/start?q=1#frag"))
print(http.get(URL + "/start?q=1", headers: {"referer": "mine"}))
print(http.get(URL + "/start?q=1", headers: {"Referer": ""}))"#,
        vec![routes(vec![
            ("/start?q=1", redirect(302, "/next")),
            ("/next", redirect(301, "/last")),
            ("*", named("end")),
        ])],
        &[],
    ));
    // the timeout covers every request together: each answer comes in
    // time for it alone, not both
    v.push(multi(
        r#"print(http.get(URL + "/slow1", timeout: 600ms))
print(http.get(URL + "/slow1", timeout: 5s))"#,
        vec![routes(vec![
            (
                "/slow1",
                Reply::Slow(
                    Duration::from_millis(400),
                    Box::new(redirect(302, "/slow2")),
                ),
            ),
            (
                "*",
                Reply::Slow(Duration::from_millis(400), Box::new(named("late"))),
            ),
        ])],
        &[],
    ));
    // Another port of the same host keeps the headers; another host
    // (localhost is not 127.0.0.1) drops the sensitive ones, also when
    // a later redirect comes back.
    v.push(multi(
        r#"h = {"authorization": "Bearer t", "cookie": "c=1", "cookie2": "x", "www-authenticate": "w", "proxy-authorization": "p", "proxy-authenticate": "pa", "x-other": "o"}
print(http.get(URL + "/port", headers: h))
print(http.get(URL + "/host", headers: h))"#,
        vec![
            routes(vec![
                ("/port", redirect(302, "http://127.0.0.1:PORT1/p")),
                ("/host", redirect(307, "http://localhost:PORT1/h")),
                ("*", named("back")),
            ]),
            routes(vec![
                ("/h", redirect(302, "http://127.0.0.1:PORT0/back")),
                ("*", named("other")),
            ]),
        ],
        &[],
    ));
    // Host names through a proxy (it never resolves them): subdomains and
    // ports keep the headers, case does not matter, other hosts drop them
    let hs = r#"{"authorization": "Bearer t", "cookie": "c=1", "content-type": "text/plain", "x-other": "o"}"#;
    v.push(multi(
        &format!(
            r#"h = {hs}
print(http.post("http://example.test/start", "body", headers: h))
print(http.get("http://EXAMPLE.test/s2", headers: h))
print(http.get("http://example.test/s3", headers: h))
print(http.get("http://u:p@example.test/s4", headers: h))
print(http.get("http://192.0.2.1/s5", headers: h))
print(http.request("PUT", "http://example.test/s6", "b", headers: h))"#
        ),
        vec![routes(vec![
            (
                "http://example.test/start",
                redirect(307, "http://sub.example.test/a"),
            ),
            (
                "http://sub.example.test/a",
                redirect(308, "http://example.test:8080/b"),
            ),
            (
                "http://example.test:8080/b",
                redirect(307, "http://other.test/c"),
            ),
            (
                "http://other.test/c",
                redirect(307, "http://example.test/d"),
            ),
            (
                "http://EXAMPLE.test/s2",
                redirect(302, "http://example.TEST/x"),
            ),
            (
                "http://example.test/s3",
                redirect(302, "http://notexample.test/x"),
            ),
            (
                "http://example.test/s4",
                redirect(302, "http://u2:p2@other.test/x"),
            ),
            (
                "http://192.0.2.1/s5",
                redirect(302, "http://192.0.2.1:8080/x"),
            ),
            (
                "http://example.test/s6",
                redirect(307, "http://xexample.test/x"),
            ),
            ("*", named("end")),
        ])],
        &[("HTTP_PROXY", "http://127.0.0.1:PORT0")],
    ));
    v
}

#[test]
fn redirects_match_go() {
    let Some(go) = go_fung("http-redirect") else {
        return;
    };
    let failures = compare(&go, redirect_cases(), &[]);
    let _ = std::fs::remove_file(go);
    report(failures);
}

/// A proxy that names itself in its answer: a 403 whose reason and body
/// are `name` (a request through it is `Ok` with that body, a CONNECT
/// through it fails with that reason).
fn proxy_named(name: &str) -> Reply {
    ok(&format!(
        "HTTP/1.1 403 {name}\r\nContent-Length: {}\r\n\r\n{name}",
        name.len()
    ))
}

fn proxy_cases() -> Vec<Multi> {
    let through = r#"print(http.get("http://u:p@example.test/a?b#c", headers: {"cookie": "x", "x-z": "z"}))
print(http.post("http://example.test:8080/p", "한"))
print(http.get("http://[2001:db8::1]:81/"))
print(http.get("http://[fe80::1%25en0]:81/z"))
print(http.get("http://example.test/검색?q=a b"))"#;
    let echo = || routes(vec![("*", named("via proxy"))]);
    vec![
        // http: the absolute URL goes to the proxy
        multi(
            through,
            vec![echo()],
            &[("HTTP_PROXY", "http://127.0.0.1:PORT0")],
        ),
        // the proxy's userinfo becomes Proxy-Authorization
        multi(
            through,
            vec![echo()],
            &[("HTTP_PROXY", "http://pu:p%20w@127.0.0.1:PORT0")],
        ),
        // a proxy without a scheme is http; its path is not used
        multi(
            r#"print(http.get("http://example.test/a"))"#,
            vec![echo()],
            &[("http_proxy", "127.0.0.1:PORT0")],
        ),
        multi(
            r#"print(http.get("http://example.test/a"))"#,
            vec![echo()],
            &[("HTTP_PROXY", "http://127.0.0.1:PORT0/ignored?x=1")],
        ),
        // failures: refused, a bad port, a CGI program
        multi(
            r#"print(http.get("http://example.test/a"))"#,
            vec![],
            &[("HTTP_PROXY", "http://127.0.0.1:REFUSED")],
        ),
        multi(
            r#"print(http.get("http://example.test/a"))"#,
            vec![],
            &[("HTTP_PROXY", "http://127.0.0.1:99999")],
        ),
        multi(
            r#"print(http.get("http://example.test/a"))
print(http.get(URL + "/loopback"))
print(http.get("https://example.test/a"))"#,
            vec![named("direct"), proxy_named("Forbidden")],
            &[
                ("HTTP_PROXY", "http://127.0.0.1:PORT1"),
                ("HTTPS_PROXY", "http://127.0.0.1:PORT1"),
                ("REQUEST_METHOD", "GET"),
            ],
        ),
        // loopback hosts and localhost never use the proxy; LOCALHOST does
        // (Go compares the name as written)
        multi(
            r#"print(http.get("http://127.0.0.1:PORT1/a"))
print(http.get("http://localhost:PORT1/b"))
print(http.get("http://[::1]:REFUSED/c"))
print(http.get("http://LOCALHOST:PORT1/d"))"#,
            vec![proxy_named("proxy"), named("direct")],
            &[("HTTP_PROXY", "http://127.0.0.1:PORT0"), ("NO_PROXY", "")],
        ),
        // upper case first; an empty upper-case value counts as unset
        multi(
            r#"print(http.get("http://example.test/"), http.get("https://example.test/"))"#,
            vec![proxy_named("A"), proxy_named("B")],
            &[
                ("HTTP_PROXY", "http://127.0.0.1:PORT0"),
                ("http_proxy", "http://127.0.0.1:PORT1"),
                ("HTTPS_PROXY", "http://127.0.0.1:PORT1"),
                ("https_proxy", "http://127.0.0.1:PORT0"),
            ],
        ),
        multi(
            r#"print(http.get("http://example.test/"), http.get("https://example.test/"))"#,
            vec![proxy_named("A"), proxy_named("B")],
            &[
                ("HTTP_PROXY", ""),
                ("http_proxy", "http://127.0.0.1:PORT1"),
                ("https_proxy", "http://127.0.0.1:PORT0"),
            ],
        ),
        multi(
            r#"print(http.get("http://example.test/"), http.get("http://other.test/"))"#,
            vec![proxy_named("A")],
            &[
                ("HTTP_PROXY", "http://127.0.0.1:PORT0"),
                ("NO_PROXY", "other.test:1"),
                ("no_proxy", "example.test"),
            ],
        ),
        // https: CONNECT host:port (with Proxy-Authorization), and how a
        // proxy's refusal reads
        multi(
            r#"print(http.get("https://example.test/a"))
print(http.get("https://[2001:db8::1]:8443/b"))
print(http.get("https://u:p@example.test:444/c"))"#,
            vec![proxy_named("Forbidden")],
            &[("HTTPS_PROXY", "http://pu:pp@127.0.0.1:PORT0")],
        ),
        multi(
            r#"print(http.get("https://example.test/a"))
print(http.get("https://example.test/b"))
print(http.get("https://example.test/c"))
print(http.get("https://example.test/d"))
print(http.get("https://example.test/e"))
print(http.get("https://example.test/f"))
print(http.get("https://example.test/g", timeout: 300ms))"#,
            vec![routes(vec![
                ("example.test:443", ok("HTTP/1.1 407\r\n\r\n")),
                ("*", ok("")),
            ])],
            &[("HTTPS_PROXY", "127.0.0.1:PORT0")],
        ),
        multi(
            r#"print(http.get("https://example.test/a"))"#,
            vec![ok("garbage\r\n\r\n")],
            &[("HTTPS_PROXY", "127.0.0.1:PORT0")],
        ),
        multi(
            r#"print(http.get("https://example.test/a"))"#,
            vec![ok("HTTP/1.1 200 OK\r\nBad header\r\n\r\n")],
            &[("HTTPS_PROXY", "127.0.0.1:PORT0")],
        ),
        multi(
            r#"print(http.get("https://example.test/a"))"#,
            vec![ok(
                "HTTP/1.1 502 Bad Gateway\r\nContent-Length: 3\r\n\r\nbad",
            )],
            &[("HTTPS_PROXY", "127.0.0.1:PORT0")],
        ),
        multi(
            r#"print(http.get("https://example.test/a", timeout: 300ms))"#,
            vec![Reply::Hold(Vec::new())],
            &[("HTTPS_PROXY", "127.0.0.1:PORT0")],
        ),
        multi(
            r#"print(http.get("https://example.test/a"))"#,
            vec![],
            &[("HTTPS_PROXY", "http://127.0.0.1:REFUSED")],
        ),
    ]
}

#[test]
fn proxies_match_go() {
    let Some(go) = go_fung("http-proxy") else {
        return;
    };
    let failures = compare(&go, proxy_cases(), &[]);
    let _ = std::fs::remove_file(go);
    report(failures);
}

/// Variables, then (url, "proxy" | "direct") pairs.
type NoProxyRow = (
    &'static [(&'static str, &'static str)],
    &'static [(&'static str, &'static str)],
);

/// Review Focus 4: which requests go through the proxy for a NO_PROXY
/// value (Go `httpproxy`): `(variables, [(url, "proxy" | "direct")])`.
/// Each table row is one run; its requests run at once, each telling by
/// the proxy's answer whether it went through the proxy (a direct one to
/// these hosts fails or times out). `HTTP_PROXY` and `HTTPS_PROXY` are
/// always the proxy.
const NO_PROXY_TABLE: &[NoProxyRow] = &[
    (
        &[],
        &[
            ("http://example.test/", "proxy"),
            ("https://example.test/", "proxy"),
            ("http://192.0.2.1/", "proxy"),
            ("http://LOCALHOST:REFUSED/", "proxy"),
            ("http://127.0.0.1:REFUSED/", "direct"),
            ("http://127.1.2.3:REFUSED/", "direct"),
            ("http://[::1]:REFUSED/", "direct"),
            ("http://[::ffff:127.0.0.1]:REFUSED/", "direct"),
            ("http://localhost:REFUSED/", "direct"),
        ],
    ),
    (
        &[("NO_PROXY", "*")],
        &[
            ("http://example.test/", "direct"),
            ("https://example.test/", "direct"),
            ("http://192.0.2.1/", "direct"),
        ],
    ),
    (
        &[("NO_PROXY", "example.test")],
        &[
            ("http://example.test/", "direct"),
            ("http://sub.example.test/", "direct"),
            ("http://notexample.test/", "proxy"),
            ("https://example.test:8443/", "direct"),
            ("http://192.0.2.1/", "proxy"),
        ],
    ),
    (
        &[("NO_PROXY", ".example.test")],
        &[
            ("http://example.test/", "proxy"),
            ("http://sub.example.test/", "direct"),
            ("http://a.b.example.test/", "direct"),
        ],
    ),
    (
        &[("NO_PROXY", "*.example.test")],
        &[
            ("http://example.test/", "proxy"),
            ("http://sub.example.test/", "direct"),
        ],
    ),
    (
        &[("NO_PROXY", "example.test:8080")],
        &[
            ("http://example.test:8080/", "direct"),
            ("http://example.test/", "proxy"),
            ("http://sub.example.test:8080/", "direct"),
        ],
    ),
    (
        &[("NO_PROXY", "example.test:80")],
        &[
            ("http://example.test/", "direct"),
            ("https://example.test/", "proxy"),
            ("http://example.test:80/", "direct"),
        ],
    ),
    (
        &[("NO_PROXY", "EXAMPLE.Test")],
        &[("http://Example.TEST/", "direct")],
    ),
    (
        &[("NO_PROXY", " other.test , example.test ,")],
        &[
            ("http://example.test/", "direct"),
            ("http://other.test/", "direct"),
            ("http://third.test/", "proxy"),
        ],
    ),
    (
        &[("NO_PROXY", "192.0.2.1")],
        &[
            ("http://192.0.2.1/", "direct"),
            ("http://192.0.2.10/", "proxy"),
            ("http://[::ffff:192.0.2.1]/", "direct"),
            ("http://example.test/", "proxy"),
        ],
    ),
    (
        &[("NO_PROXY", "192.0.2.1:8080")],
        &[
            ("http://192.0.2.1:8080/", "direct"),
            ("http://192.0.2.1/", "proxy"),
        ],
    ),
    (
        &[("NO_PROXY", "192.0.2.0/24")],
        &[
            ("http://192.0.2.77/", "direct"),
            ("http://198.51.100.1/", "proxy"),
            ("http://[::ffff:192.0.2.5]/", "direct"),
        ],
    ),
    (
        &[("NO_PROXY", "2001:db8::1")],
        &[
            ("http://[2001:db8::1]/", "direct"),
            ("http://[2001:db8::2]/", "proxy"),
            ("http://[2001:db8:0:0:0:0:0:1]:81/", "direct"),
        ],
    ),
    (
        &[("NO_PROXY", "[2001:db8::1]:8080")],
        &[
            ("http://[2001:db8::1]:8080/", "direct"),
            ("http://[2001:db8::1]/", "proxy"),
        ],
    ),
    (
        &[("NO_PROXY", "2001:db8::/32")],
        &[
            ("http://[2001:db8:1::5]/", "direct"),
            ("http://[2001:db9::1]/", "proxy"),
            ("http://192.0.2.1/", "proxy"),
        ],
    ),
    (
        &[("NO_PROXY", "::ffff:192.0.2.0/120,198.51.100.0/24")],
        &[
            ("http://192.0.2.5/", "direct"),
            ("http://[::ffff:198.51.100.7]/", "direct"),
            ("http://[2001:db8::5]/", "proxy"),
        ],
    ),
    (
        &[("NO_PROXY", "::/0")],
        &[
            ("http://192.0.2.1/", "proxy"),
            ("http://[2001:db8::5]/", "direct"),
        ],
    ),
    (
        &[("NO_PROXY", ":80,192.0.2.01,example.test.,[2001:db8::1]")],
        &[
            ("http://example.test/", "proxy"),
            ("http://192.0.2.1/", "proxy"),
            ("http://[2001:db8::1]/", "proxy"),
        ],
    ),
    (
        &[("NO_PROXY", "localhost")],
        &[("http://LOCALHOST:REFUSED/", "direct")],
    ),
    (
        &[("no_proxy", "example.test")],
        &[("http://example.test/", "direct")],
    ),
    (
        &[("NO_PROXY", ""), ("no_proxy", "example.test")],
        &[("http://example.test/", "direct")],
    ),
    (
        &[("NO_PROXY", "other.test"), ("no_proxy", "example.test")],
        &[
            ("http://example.test/", "proxy"),
            ("http://other.test/", "direct"),
        ],
    ),
];

#[test]
fn no_proxy_patterns_match_go() {
    let Some(go) = go_fung("http-noproxy") else {
        return;
    };
    let rust = rust_fung();
    let refused = refused_port().to_string();
    let mut failures = Vec::new();
    for (vars, urls) in NO_PROXY_TABLE {
        let proxy = Fake::start(proxy_named("via-proxy"));
        let list: Vec<String> = urls
            .iter()
            .map(|(u, _)| format!("{:?}", u.replace("REFUSED", &refused)))
            .collect();
        let src = format!(
            "import http\nfn probe(u) = match http.get(u, timeout: 1s):\n  Ok(r)  => r.body == \"via-proxy\" then \"proxy\" else \"direct\"\n  Err(e) => \"via-proxy\" in e.message then \"proxy\" else \"direct\"\n;;\nts = map([{}], u => spawn(() => probe(u)))\nprint(join(map(ts, t => await(t)), \" \"))",
            list.join(", ")
        );
        let addr = format!("http://127.0.0.1:{}", proxy.port);
        let mut env = vec![
            ("HTTP_PROXY".to_string(), addr.clone()),
            ("HTTPS_PROXY".to_string(), addr),
        ];
        env.extend(vars.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        let want: Vec<&str> = urls.iter().map(|(_, w)| *w).collect();
        let want = (format!("{}\n", want.join(" ")), String::new(), Some(0));
        let g = run_env(&go, &src, &env);
        let r = run_env(&rust, &src, &env);
        if g != want {
            failures.push(format!(
                "{vars:?} {urls:?}\n  go:   {g:?}\n  want: {want:?}"
            ));
        }
        if r != g {
            failures.push(format!("{vars:?} {urls:?}\n  rust: {r:?}\n  go:   {g:?}"));
        }
    }
    let _ = std::fs::remove_file(go);
    report(failures);
}

/// A Go TLS server (`<cert> <key>` as arguments, its port the first line
/// of output). It redirects to `?to=`, else answers with what it got:
/// the request line, the Host, the headers but Accept-Encoding (sorted),
/// and the body. It closes each connection after one answer.
const TLS_SERVER: &str = r#"package main

import (
	"crypto/tls"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"os"
	"sort"
	"strings"
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
	srv := &http.Server{Handler: http.HandlerFunc(serve), ErrorLog: log.New(io.Discard, "", 0)}
	srv.Serve(l)
}

func serve(w http.ResponseWriter, r *http.Request) {
	// one request per connection, as the Rust client does: Go then
	// opens a tunnel for each request too
	w.Header().Set("Connection", "close")
	if to := r.URL.Query().Get("to"); to != "" {
		w.Header().Set("Location", to)
		w.WriteHeader(302)
		return
	}
	keys := []string{}
	for k := range r.Header {
		if k != "Accept-Encoding" {
			keys = append(keys, k)
		}
	}
	sort.Strings(keys)
	body, _ := io.ReadAll(r.Body)
	fmt.Fprintf(w, "%s %s %s host=%s\n", r.Method, r.RequestURI, r.Proto, r.Host)
	for _, k := range keys {
		fmt.Fprintf(w, "%s: %s\n", k, strings.Join(r.Header[k], ", "))
	}
	w.Write(body)
}
"#;

/// A running Go TLS server, killed when dropped.
struct TlsServer {
    child: std::process::Child,
    port: u16,
}

impl Drop for TlsServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn tls_server(bin: &Path, cert: &str) -> TlsServer {
    use std::io::{BufRead, BufReader};
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/tls");
    let mut child = std::process::Command::new(bin)
        .arg(data.join(format!("{cert}.pem")))
        .arg(data.join(format!("{cert}.key")))
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut port = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut port)
        .unwrap();
    TlsServer {
        port: port.trim().parse().unwrap(),
        child,
    }
}

/// https through a proxy's CONNECT tunnel: the fake proxy records the
/// CONNECT and carries the bytes to a Go TLS server for `wrong.example`
/// (a name no one resolves: only the proxy's tunnel reaches it), trusted
/// through `SSL_CERT_FILE` (Go uses it on Linux and, since 1.27, macOS).
/// What the server got comes back in the body. Also: redirects inside
/// the tunnel (no Referer from https to http), a name the certificate
/// does not hold, and an http request through an https proxy (a TLS
/// server for localhost answering the absolute URL).
#[test]
fn https_through_a_proxy_matches_go() {
    let Some(go) = go_fung("http-tunnel") else {
        return;
    };
    let tmp = std::env::temp_dir().join(format!("fung-http-tunnel-{}", std::process::id()));
    std::fs::create_dir_all(tmp.join("certs")).unwrap();
    std::fs::write(tmp.join("server.go"), TLS_SERVER).unwrap();
    let built = std::process::Command::new("go")
        .args(["build", "-o", "server", "server.go"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let wrong = tls_server(&tmp.join("server"), "wrongname");
    let local = tls_server(&tmp.join("server"), "localhost");
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/tls");
    let words = [
        ("TLSPORT", wrong.port.to_string()),
        ("LOCALTLS", local.port.to_string()),
        (
            "SSL_CERT_FILE",
            data.join("ca.pem").to_string_lossy().into_owned(),
        ),
        (
            "SSL_CERT_DIR",
            tmp.join("certs").to_string_lossy().into_owned(),
        ),
    ];
    let show =
        "fn show(r) = match r:\n  Ok(v)  => \"${v.status} ${v.body}\"\n  Err(e) => e.message\n;;\n";
    let tunnel = || Reply::Tunnel(wrong.port);
    let cases = vec![
        multi(
            &format!(
                r#"{show}print(show(http.get("https://wrong.example:TLSPORT/a?b=1", headers: {{"x-h": "1", "cookie": "c"}})))
print(show(http.post("https://u:p@wrong.example/p", "한글")))
print(show(http.get("https://wrong.example/r?to=/final")))
print(show(http.get("https://wrong.example/r?to=http://127.0.0.1:PORT1/x")))
print(show(http.get("https://wrong.example/r?to=https://wrong.example:8443/y", headers: {{"authorization": "a"}})))"#
            ),
            vec![tunnel(), named("plain")],
            &[("HTTPS_PROXY", "http://pu:pp@127.0.0.1:PORT0")],
        ),
        multi(
            &format!(r#"{show}print(show(http.get("https://other.example/")))"#),
            vec![tunnel()],
            &[("HTTPS_PROXY", "127.0.0.1:PORT0")],
        ),
        multi(
            &format!(
                r#"{show}print(show(http.get("http://example.test/a?b", headers: {{"x-h": "1"}})))"#
            ),
            vec![],
            &[("HTTP_PROXY", "https://pu:pp@localhost:LOCALTLS")],
        ),
    ];
    let failures = compare(&go, cases, &words);
    drop((wrong, local));
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::remove_file(go);
    report(failures);
}

/// Rust only (4d known differences): SOCKS proxies and https through an
/// https proxy (TLS inside TLS) are not supported; Go supports both.
#[test]
fn unsupported_proxies_are_errors() {
    let rust = rust_fung();
    let cases = [
        (
            "HTTP_PROXY",
            "socks5://127.0.0.1:1080",
            "http://example.test/",
            "socks5 proxies are not supported",
        ),
        (
            "HTTPS_PROXY",
            "https://127.0.0.1:1",
            "https://example.test/",
            "https through an https proxy is not supported",
        ),
    ];
    for (var, proxy, url, why) in cases {
        let src = format!("import http\nprint(http.get(\"{url}\"))");
        let got = run_env(&rust, &src, &[(var.to_string(), proxy.to_string())]);
        let want = format!(
            "Err({{kind: 'network, message: \"cannot request `{url}`: Get \\\"{url}\\\": proxyconnect tcp: {why}\"}})\n"
        );
        assert_eq!(got, (want, String::new(), Some(0)));
    }
}
