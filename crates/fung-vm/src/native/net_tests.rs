//! `_native.net` (stage 4d, Task 1): the TCP primitives, driven by fung
//! programs that import the bridge as an embedded file would
//! (`Opts::internal`), against fung or Rust peers on 127.0.0.1.
//!
//! The tests share process-wide counters (live sockets, wake pipes,
//! lookup threads, waits), so they run one at a time under `NET`.
//! Concurrent orderings use state (the waits counted in `probe`), never
//! sleeps. Expected error texts come from the Go 1.27 `net` package
//! (small Go programs; the texts are copied in).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::MutexGuard;
use std::time::{Duration, Instant};

use super::probe;
use crate::testutil::{Opts, Outcome, UNTIL, exec, exec_prepared, probes};
use crate::value::{Native, Value};
use crate::vm::{CancelNodeForTests as CancelNode, Stop, Task, Watch, live_wakes, watch_serial};

/// One net test at a time: the counters are process-wide (the lock is
/// shared with the wake-pipe tests of `vm/watch.rs`).
pub(super) fn serial() -> MutexGuard<'static, ()> {
    watch_serial()
}

/// The probes plus the net ones: `net_waiting()` (tasks inside a socket
/// wait now), `read_waits()` / `write_waits()` (waits for reading or
/// writing so far), `sockets()` (live sockets).
pub(super) fn net_probes() -> Vec<Value> {
    type F = fn(&mut Task<'_>, Vec<Value>) -> Result<Value, Stop>;
    let natives: [(&'static str, F); 4] = [
        ("net_waiting", |_, _| {
            Ok(Value::Int(probe::waiting() as i64))
        }),
        ("read_waits", |_, _| {
            Ok(Value::Int(probe::read_waits() as i64))
        }),
        ("write_waits", |_, _| {
            Ok(Value::Int(probe::write_waits() as i64))
        }),
        ("sockets", |_, _| Ok(Value::Int(probe::sockets() as i64))),
    ];
    let mut v = probes();
    for (name, f) in natives {
        v.push(Value::Native(Native::leak(name, vec![], f)));
    }
    v
}

/// fung helpers: `msg(r)` is the message of an `Err` (anything else as
/// it is), `host(addr)` an address without its port.
pub(super) const HELPERS: &str = "fn msg(r) = match r:\n  Err(e) => e.message\n  v => v\n;;\nfn kind(r) = match r:\n  Err(e) => e.kind\n  v => v\n;;\nfn host(a) = join(reverse(drop(reverse(split(a, \":\")), 1)), \":\")\n";

/// Runs `src` with `_native.net` imported as `net`, the probes and
/// `until`; then checks that nothing is left open: no socket, no wake
/// pipe, no lookup thread (one may still be finishing: it is waited for
/// by state, with a generous bound).
pub(super) fn run(src: &str) -> Outcome {
    let o = exec(
        &format!("import _native.net as net\n{UNTIL}{HELPERS}{src}"),
        Opts {
            internal: true,
            extra: net_probes,
            ..Opts::default()
        },
    );
    assert_eq!(probe::sockets(), 0, "sockets left open\n{o:?}");
    assert_eq!(live_wakes(), 0, "wake pipes left open\n{o:?}");
    let until = Instant::now() + Duration::from_secs(10);
    while probe::lookups() != 0 && Instant::now() < until {
        std::thread::yield_now();
    }
    assert_eq!(probe::lookups(), 0, "lookup threads left");
    o
}

pub(super) fn out(o: &Outcome) -> (&str, &str, i32) {
    (o.out.as_str(), o.err.as_str(), o.exit)
}

/// Waits (by state) until `f` holds.
pub(super) fn wait_for(mut f: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(20);
    while !f() {
        assert!(Instant::now() < until, "state not reached");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A Rust peer on 127.0.0.1: the port and the thread running `serve`
/// on the first connection. A program that never connects fails the
/// test (after 20 s) instead of hanging it.
fn peer<T: Send + 'static>(
    serve: impl FnOnce(TcpStream) -> T + Send + 'static,
) -> (u16, std::thread::JoinHandle<T>) {
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
        serve(s)
    });
    (port, h)
}

/// A port nothing listens on (bound, then closed).
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Program lines that listen on 127.0.0.1, connect a client `c` to it
/// and accept the server side `s`.
const PAIR: &str = "l = net.listen(\"127.0.0.1:0\")!\nport = parse_int(split(net.local_addr(l), \":\")[1])!\nc = net.connect(\"127.0.0.1\", port)!\ns = net.accept(l)!\n";

#[test]
fn a_line_goes_there_and_back() {
    let _g = serial();
    let o = run(
        "l = net.listen(\"127.0.0.1:0\")!\nport = parse_int(split(net.local_addr(l), \":\")[1])!\nsrv = spawn(fn():\n  s = net.accept(l)!\n  line = net.read_line(s, 100)!\n  net.write(s, \"got ${line}\\n\")!\n  net.close(s)!\n;;)\nc = net.connect(\"127.0.0.1\", port)!\nprint(net.write(c, \"héllo\\r\\n\"))\nprint(net.read_line(c, 100))\nprint(net.read_line(c, 100))\nprint(await(srv))",
    );
    assert_eq!(
        out(&o),
        ("Ok(nil)\nOk(\"got héllo\")\nOk(nil)\nnil\n", "", 0)
    );
}

#[test]
fn values_print_type_and_compare_by_identity() {
    let _g = serial();
    let o = run(&format!(
        "{PAIR}print(c, l, type_of(c), type_of(l), c == c, c == s, l == l, [c])"
    ));
    assert_eq!(
        out(&o),
        (
            "<conn> <listener> 'conn 'listener true false true [<conn>]\n",
            "",
            0
        )
    );
}

#[test]
fn reads_gather_what_arrives_in_pieces() {
    let _g = serial();
    let base = probe::read_waits();
    let (port, h) = peer(move |mut s| {
        // each piece is written once the reader waits for more
        wait_for(|| probe::read_waits() > base);
        s.write_all(b"hel").unwrap();
        wait_for(|| probe::read_waits() > base + 1);
        s.write_all(b"lo\r\n12").unwrap();
        wait_for(|| probe::read_waits() > base + 2);
        s.write_all("3한".as_bytes()).unwrap();
    });
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port})!\nprint(net.read_line(c, 100), net.read_bytes(c, 6), net.read_line(c, 100))"
    ));
    h.join().unwrap();
    assert_eq!(out(&o), ("Ok(\"hello\") Ok(\"123한\") Ok(nil)\n", "", 0));
}

#[test]
fn a_long_write_goes_out_whole() {
    let _g = serial();
    let base = probe::write_waits();
    // 3 MB, more than the socket buffers hold: the writer waits
    let (port, h) = peer(move |mut s| {
        wait_for(|| probe::write_waits() > base);
        let mut got = Vec::new();
        s.read_to_end(&mut got).unwrap();
        got
    });
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port})!\ns = repeat(\"한ab\", 600000)\nprint(net.byte_len(s), net.write(c, s), net.close(c))"
    ));
    let got = h.join().unwrap();
    assert_eq!(out(&o), ("3000000 Ok(nil) Ok(nil)\n", "", 0));
    assert_eq!(got, "한ab".repeat(600_000).into_bytes());
}

#[test]
fn line_ends_eof_and_partial_lines() {
    let _g = serial();
    let (port, h) = peer(|mut s| s.write_all(b"a\nb\r\n\r\n\nlast").unwrap());
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port})!\nprint(net.read_line(c, 10), net.read_line(c, 10), net.read_line(c, 10), net.read_line(c, 10), net.read_line(c, 10), net.read_line(c, 10), net.read_line(c, 10))"
    ));
    h.join().unwrap();
    assert_eq!(
        out(&o),
        (
            "Ok(\"a\") Ok(\"b\") Ok(\"\") Ok(\"\") Ok(\"last\") Ok(nil) Ok(nil)\n",
            "",
            0
        )
    );
}

#[test]
fn the_line_limit_counts_bytes_without_the_line_end() {
    let _g = serial();
    // "한글" is 6 bytes; a 7-byte line is too long for 6 even before its
    // end arrives (the peer keeps the connection open)
    let (port, h) = peer(|mut s| {
        s.write_all("한글\r\n한글\n1234567".as_bytes()).unwrap();
        let mut b = [0u8; 1];
        let _ = s.read(&mut b);
    });
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port})!\nprint(net.read_line(c, 6))\nprint(net.read_line(c, 6))\nprint(net.read_line(c, 6))\nnet.close(c)"
    ));
    h.join().unwrap();
    assert_eq!(
        out(&o),
        (
            "Ok(\"한글\")\nOk(\"한글\")\nErr({kind: 'limit, message: \"line longer than 6 bytes\"})\n",
            "",
            0
        )
    );
}

/// A last line without its end is held to the limit too, `\r` and all.
#[test]
fn a_last_line_without_its_end_keeps_the_limit() {
    let _g = serial();
    for (sent, want) in [
        (
            "abc\r",
            "Err({kind: 'limit, message: \"line longer than 3 bytes\"})",
        ),
        ("ab\r", "Ok(\"ab\\r\")"),
    ] {
        let (port, h) = peer(move |mut s| s.write_all(sent.as_bytes()).unwrap());
        let o = run(&format!(
            "c = net.connect(\"127.0.0.1\", {port})!\nprint(net.read_line(c, 3))"
        ));
        h.join().unwrap();
        assert_eq!(out(&o), (format!("{want}\n").as_str(), "", 0), "{sent:?}");
    }
}

#[test]
fn read_bytes_needs_all_of_them_and_read_all_reads_to_the_end() {
    let _g = serial();
    let (port, h) = peer(|mut s| s.write_all(b"abcdefgh").unwrap());
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port})!\nprint(net.read_bytes(c, 0), net.read_bytes(c, 3), net.read_bytes(c, 9))"
    ));
    h.join().unwrap();
    assert_eq!(
        out(&o),
        (
            "Ok(\"\") Ok(\"abc\") Err({kind: 'network, message: \"unexpected EOF\"})\n",
            "",
            0
        )
    );
    // after a limit error what was read stays unread
    for (limit, want) in [
        (8, "Ok(\"abcdefgh\") Ok(\"\")"),
        (
            7,
            "Err({kind: 'limit, message: \"more than 7 bytes\"}) Err({kind: 'limit, message: \"more than 0 bytes\"})",
        ),
    ] {
        let (port, h) = peer(|mut s| s.write_all(b"abcdefgh").unwrap());
        let o = run(&format!(
            "c = net.connect(\"127.0.0.1\", {port})!\nprint(net.read_all(c, {limit}), net.read_all(c, 0))"
        ));
        h.join().unwrap();
        assert_eq!(out(&o), (format!("{want}\n").as_str(), "", 0));
    }
}

#[test]
fn bytes_that_are_not_utf8_read_as_replacement_characters() {
    let _g = serial();
    let (port, h) = peer(|mut s| s.write_all(b"a\xffb\xe2\x82\nz\xc3").unwrap());
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port})!\nprint(net.read_line(c, 10), net.read_all(c, 10))"
    ));
    h.join().unwrap();
    assert_eq!(
        out(&o),
        (
            "Ok(\"a\u{fffd}b\u{fffd}\u{fffd}\") Ok(\"z\u{fffd}\")\n",
            "",
            0
        )
    );
}

#[test]
fn byte_len_counts_utf8_bytes() {
    let _g = serial();
    let o = run("print(net.byte_len(\"한a\"), net.byte_len(\"\"))");
    assert_eq!(out(&o), ("4 0\n", "", 0));
}

/// `utf8_bytes` gives what `write` would send, one int per byte (the
/// `http` module percent-encodes URL paths with it).
#[test]
fn utf8_bytes_lists_the_bytes() {
    let _g = serial();
    let o =
        run("print(net.utf8_bytes(\"한a\"), net.utf8_bytes(\"\"), net.utf8_bytes(\"\\u{0}~\"))");
    assert_eq!(out(&o), ("[237, 149, 156, 97] [] [0, 126]\n", "", 0));
}

/// `need_str_map(name, m)` is Go's check of a header map: nil when every
/// value is a str, else E0403 naming the first value that is not (in the
/// map's order), at the user's call.
#[test]
fn need_str_map_checks_every_value() {
    let _g = serial();
    let o = run(
        "print(net.need_str_map(\"headers\", {}), net.need_str_map(\"headers\", {a: \"1\", b: \"2\"}))",
    );
    assert_eq!(out(&o), ("nil nil\n", "", 0));
    let o = run("print(1)\nnet.need_str_map(\"headers\", {a: \"1\", b: 2, c: nil})");
    assert!(
        o.err.starts_with(
            "error[E0403]: argument `headers` must be a map of strings, got a map holding int\n"
        ),
        "{}",
        o.err
    );
    let o = run("net.need_str_map(\"headers\", 1)");
    assert!(
        o.err
            .starts_with("error[E0403]: argument `headers` must be map, got int\n"),
        "{}",
        o.err
    );
}

/// The Go 1.27 texts (darwin; Linux has the same apart from the errno
/// wording, which `oserr` covers): `dial tcp 127.0.0.1:1: connect:
/// connection refused`, `dial tcp: lookup nosuchhost.invalid: no such
/// host`, `listen tcp 127.0.0.1:55925: bind: address already in use`,
/// `listen tcp: address a:b:c: too many colons in address`, ...
#[test]
fn errors_read_like_go() {
    let _g = serial();
    let closed = free_port();
    let busy = TcpListener::bind("127.0.0.1:0").unwrap();
    let busy_port = busy.local_addr().unwrap().port();
    let cases: Vec<(String, String)> = vec![
        (
            format!("net.connect(\"127.0.0.1\", {closed})"),
            format!(
                "Err({{kind: 'network, message: \"dial tcp 127.0.0.1:{closed}: connect: connection refused\"}})"
            ),
        ),
        (
            "net.connect(\"127.0.0.1\", 65536)".into(),
            "Err({kind: 'network, message: \"dial tcp: address 65536: invalid port\"})".into(),
        ),
        // with TLS, the TCP error comes first (Go's dial error)
        (
            format!("net.connect(\"127.0.0.1\", {closed}, tls: true)"),
            format!(
                "Err({{kind: 'network, message: \"dial tcp 127.0.0.1:{closed}: connect: connection refused\"}})"
            ),
        ),
        (
            format!("net.listen(\"127.0.0.1:{busy_port}\")"),
            format!(
                "Err({{kind: 'network, message: \"listen tcp 127.0.0.1:{busy_port}: bind: address already in use\"}})"
            ),
        ),
        (
            "net.listen(\"a:b:c\")".into(),
            "Err({kind: 'network, message: \"listen tcp: address a:b:c: too many colons in address\"})"
                .into(),
        ),
        (
            "net.listen(\"::1:0\")".into(),
            "Err({kind: 'network, message: \"listen tcp: address ::1:0: too many colons in address\"})"
                .into(),
        ),
        (
            "net.listen(\"127.0.0.1\")".into(),
            "Err({kind: 'network, message: \"listen tcp: address 127.0.0.1: missing port in address\"})"
                .into(),
        ),
        (
            "net.listen(\"127.0.0.1:99999\")".into(),
            "Err({kind: 'network, message: \"listen tcp: address 99999: invalid port\"})".into(),
        ),
        (
            "net.listen(\"127.0.0.1:-1\")".into(),
            "Err({kind: 'network, message: \"listen tcp: address -1: invalid port\"})".into(),
        ),
        (
            "net.listen(\"127.0.0.1:abc\")".into(),
            "Err({kind: 'network, message: \"listen tcp: lookup tcp/abc: unknown port\"})".into(),
        ),
    ];
    let src: String = cases.iter().map(|(e, _)| format!("print({e})\n")).collect();
    let want: String = cases.iter().map(|(_, w)| format!("{w}\n")).collect();
    let o = run(&src);
    drop(busy);
    assert_eq!(out(&o), (want.as_str(), "", 0));
}

/// Go's lookup failure text (`lookup <host>: no such host`) for a
/// resolver answer of `EAI_NONAME`, given deterministically by the test
/// hook (`gate.invalid`, not held: it answers at once).
#[test]
fn a_name_that_does_not_exist_reads_like_go() {
    let _g = serial();
    let o = run("print(net.connect(\"gate.invalid\", 80))\nprint(net.listen(\"gate.invalid:0\"))");
    assert_eq!(
        out(&o),
        (
            "Err({kind: 'network, message: \"dial tcp: lookup gate.invalid: no such host\"})\nErr({kind: 'network, message: \"listen tcp: lookup gate.invalid: no such host\"})\n",
            "",
            0
        )
    );
}

/// The same through the system resolver, where it says so: a machine
/// without DNS answers otherwise (`Temporary failure in name resolution`),
/// and the test is skipped there.
#[test]
fn a_real_lookup_failure_reads_like_go() {
    let _g = serial();
    if super::sock::lookup("nosuchhost.invalid") != Err("no such host".to_string()) {
        eprintln!(
            "skipping: the system resolver does not answer `no such host` for nosuchhost.invalid"
        );
        return;
    }
    let o = run(
        "print(net.connect(\"nosuchhost.invalid\", 80))\nprint(net.listen(\"nosuchhost.invalid:0\"))",
    );
    assert_eq!(
        out(&o),
        (
            "Err({kind: 'network, message: \"dial tcp: lookup nosuchhost.invalid: no such host\"})\nErr({kind: 'network, message: \"listen tcp: lookup nosuchhost.invalid: no such host\"})\n",
            "",
            0
        )
    );
}

/// Go: `[127.0.0.1]:0` and `localhost:0` listen on 127.0.0.1, and a
/// missing port is port 0.
#[test]
fn listen_addresses_read_like_go() {
    let _g = serial();
    let o = run(
        "each([\"[127.0.0.1]:0\", \"localhost:0\", \"127.0.0.1:\"], fn(a):\n  l = net.listen(a)!\n  addr = net.local_addr(l)\n  print(host(addr), parse_int(last(split(addr, \":\")))! > 0)\n;;)",
    );
    assert_eq!(
        out(&o),
        ("127.0.0.1 true\n127.0.0.1 true\n127.0.0.1 true\n", "", 0)
    );
}

/// Go: `:0` listens on `[::]` (both families), `[::1]:0` on `[::1]`.
/// Skipped where IPv6 is missing.
#[test]
fn ipv6_listen_addresses_read_like_go() {
    let _g = serial();
    if TcpListener::bind("[::1]:0").is_err() {
        eprintln!("skipping: no IPv6 loopback");
        return;
    }
    let o = run(
        "each([\":0\", \"[::1]:0\"], fn(a):\n  l = net.listen(a)!\n  addr = net.local_addr(l)\n  print(host(addr), parse_int(last(split(addr, \":\")))! > 0)\n;;)",
    );
    assert_eq!(out(&o), ("[::] true\n[::1] true\n", "", 0));
}

/// The loopback interface's name, for an IPv6 zone.
#[cfg(target_os = "linux")]
const LOOPBACK: &str = "lo";
#[cfg(not(target_os = "linux"))]
const LOOPBACK: &str = "lo0";

/// An IPv6 literal with a zone keeps it: Go names the address with the
/// zone as written (`dial tcp [::1%lo0]:P: connect: connection refused`,
/// also for a numeric or unknown zone, which mean no scope), and a
/// link-local address dials through its zone's interface. Skipped where
/// IPv6 is missing.
#[test]
fn an_ipv6_zone_is_kept() {
    let _g = serial();
    let Ok(l) = TcpListener::bind("[::1]:0") else {
        eprintln!("skipping: no IPv6 loopback");
        return;
    };
    let closed = l.local_addr().unwrap().port();
    drop(l);
    let o = run(&format!(
        "each([\"::1%{LOOPBACK}\", \"::1%1\", \"::1%nosuch\"], h => print(msg(net.connect(h, {closed}))))\nl = net.listen(\"[::1%{LOOPBACK}]:0\")!\np = parse_int(last(split(net.local_addr(l), \":\")))!\nc = net.connect(\"::1%{LOOPBACK}\", p)!\nprint(host(net.local_addr(l)), host(net.local_addr(c)))\nnet.close(c)\nnet.close(l)"
    ));
    assert_eq!(
        out(&o),
        (
            format!(
                "dial tcp [::1%{LOOPBACK}]:{closed}: connect: connection refused\ndial tcp [::1%1]:{closed}: connect: connection refused\ndial tcp [::1%nosuch]:{closed}: connect: connection refused\n[::1] [::1]\n"
            )
            .as_str(),
            "",
            0
        )
    );
}

/// macOS gives `lo0` the link-local `fe80::1`, which only its scope
/// reaches (without it: `connect: no route to host`). Go 1.27 prints the
/// zone in every address (`[fe80::1%lo0]:P`).
#[cfg(target_os = "macos")]
#[test]
fn a_link_local_address_dials_through_its_zone() {
    let _g = serial();
    // SAFETY: a NUL-terminated name.
    let lo0 = unsafe { libc::if_nametoindex(c"lo0".as_ptr()) };
    let a = std::net::SocketAddrV6::new(
        std::net::Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1),
        0,
        0,
        lo0,
    );
    if lo0 == 0 || TcpListener::bind(a).is_err() {
        eprintln!("skipping: no fe80::1 on lo0");
        return;
    }
    let o = run(
        "l = net.listen(\"[fe80::1%lo0]:0\")!\na = net.local_addr(l)\np = parse_int(last(split(a, \":\")))!\nc = net.connect(\"fe80::1%lo0\", p)!\nprint(host(a), host(net.local_addr(c)))\nprint(msg(net.listen(a)) == \"listen tcp ${a}: bind: address already in use\")\nnet.close(c)\nnet.close(l)",
    );
    assert_eq!(out(&o), ("[fe80::1%lo0] [fe80::1%lo0]\ntrue\n", "", 0));
}

/// Go: `read tcp 127.0.0.1:55927->127.0.0.1:55925: read: connection
/// reset by peer`, then `write tcp …: write: broken pipe`; after
/// `Close`, `… use of closed network connection` for read, write and a
/// second close.
#[test]
fn reset_and_closed_connections_read_like_go() {
    let _g = serial();
    let (port, h) = peer(|s| {
        // SO_LINGER 0: close sends a reset
        let l = libc::linger {
            l_onoff: 1,
            l_linger: 0,
        };
        // SAFETY: a valid socket and a correctly sized option value.
        let rc = unsafe {
            libc::setsockopt(
                std::os::fd::AsRawFd::as_raw_fd(&s),
                libc::SOL_SOCKET,
                libc::SO_LINGER,
                (&l as *const libc::linger).cast(),
                std::mem::size_of::<libc::linger>() as libc::socklen_t,
            )
        };
        assert_eq!(rc, 0);
        drop(s);
    });
    // the read waits for the reset
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port})!\nname = net.local_addr(c) + \"->127.0.0.1:{port}\"\nlocal = net.local_addr(c)\nshow = (r) => replace(replace(msg(r), name, \"A->B\"), local, \"A\")\nprint(show(net.read_line(c, 10)))\nprint(show(net.write(c, \"x\")))\nprint(net.close(c))\nprint(show(net.read_line(c, 10)))\nprint(show(net.write(c, \"x\")))\nprint(show(net.close(c)))\nprint(show(net.set_deadline(c, 1s)))"
    ));
    h.join().unwrap();
    assert_eq!(
        out(&o),
        (
            "read tcp A->B: read: connection reset by peer\nwrite tcp A->B: write: broken pipe\nOk(nil)\nread tcp A->B: use of closed network connection\nwrite tcp A->B: use of closed network connection\nclose tcp A->B: use of closed network connection\nset tcp A: use of closed network connection\n",
            "",
            0
        )
    );
}

/// Go: `read tcp 127.0.0.1:55927->127.0.0.1:55925: i/o timeout`.
#[test]
fn a_deadline_ends_reads_with_a_timeout() {
    let _g = serial();
    let o = run(&format!(
        "{PAIR}name = net.local_addr(c) + \"->\" + net.local_addr(l)\nprint(net.set_deadline(c, 30ms))\nr = net.read_line(c, 10)\nprint(kind(r), replace(msg(r), name, \"A->B\"))\nnet.set_deadline(c, 0s)!\nnet.write(s, \"late\\n\")!\nprint(net.read_line(c, 10))\nnet.set_deadline(c, -1s)!\nnet.write(s, \"x\\n\")!\nprint(kind(net.read_bytes(c, 2)))"
    ));
    assert_eq!(
        out(&o),
        (
            "Ok(nil)\n'timeout read tcp A->B: i/o timeout\nOk(\"late\")\n'timeout\n",
            "",
            0
        )
    );
}

#[test]
fn the_last_copy_closes_the_connection() {
    let _g = serial();
    let (port, h) = peer(|mut s| {
        let mut got = String::new();
        s.read_to_string(&mut got).unwrap();
        got
    });
    // the task's copy is the only one: the peer sees the end once it is
    // gone, while the program still runs
    let o = run(&format!(
        "t = spawn(fn(): c = net.connect(\"127.0.0.1\", {port})!; net.write(c, \"hi\")! ;;)\nawait(t)\nprint(sockets())"
    ));
    assert_eq!(h.join().unwrap(), "hi");
    assert_eq!(out(&o), ("0\n", "", 0));
    // fung on both ends: the server reads to the end once the client's
    // last copy is gone
    let o = run(&format!("{PAIR}c = nil\nprint(net.read_all(s, 10))"));
    assert_eq!(out(&o), ("Ok(\"\")\n", "", 0));
}

/// Go: a `Close` wakes a `Read` or `Accept` blocked on the same
/// connection or listener with `use of closed network connection`.
#[test]
fn close_wakes_a_task_blocked_on_the_same_handle() {
    let _g = serial();
    let o = run(&format!(
        "{PAIR}name = net.local_addr(s) + \"->\" + net.local_addr(c)\nlname = net.local_addr(l)\nr = spawn(() => net.read_line(s, 10))\na = spawn(() => net.accept(l))\nuntil(() => net_waiting() == 2)\nnet.close(s)!\nnet.close(l)!\nprint(replace(msg(await(r)), name, \"A->B\"))\nprint(replace(msg(await(a)), lname, \"A\"))\nprint(replace(msg(net.close(l)), lname, \"A\"))"
    ));
    assert_eq!(
        out(&o),
        (
            "read tcp A->B: use of closed network connection\naccept tcp A: use of closed network connection\nclose tcp A: use of closed network connection\n",
            "",
            0
        )
    );
}

/// Review Focus 1: a task blocked in `accept` ends with E0701 when
/// cancelled (its handle gives `Err({kind: 'cancelled})`), at once, and
/// leaves no socket, pipe or thread behind (`run` checks).
#[test]
fn cancel_ends_a_blocked_accept() {
    let _g = serial();
    let start = Instant::now();
    let o = run(
        "l = net.listen(\"127.0.0.1:0\")!\nt = spawn(() => net.accept(l))\nuntil(() => net_waiting() == 1)\ncancel(t)\nprint(await(t))\nprint(net_waiting())",
    );
    assert_eq!(out(&o), ("Err({kind: 'cancelled})\n0\n", "", 0));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn cancel_ends_a_blocked_read() {
    let _g = serial();
    let o = run(&format!(
        "{PAIR}t = spawn(() => net.read_line(s, 10))\nuntil(() => net_waiting() == 1)\ncancel(t)\nprint(await(t))\nnet.write(c, \"still\\n\")!\nprint(net.read_line(s, 10))"
    ));
    assert_eq!(out(&o), ("Err({kind: 'cancelled})\nOk(\"still\")\n", "", 0));
}

#[test]
fn cancel_ends_a_blocked_write() {
    let _g = serial();
    let o = run(&format!(
        "{PAIR}t = spawn(() => net.write(c, repeat(\"x\", 50000000)))\nuntil(() => net_waiting() == 1)\ncancel(t)\nprint(await(t))"
    ));
    assert_eq!(out(&o), ("Err({kind: 'cancelled})\n", "", 0));
}

/// `connect` cancelled while its name lookup runs. The lookup is held
/// by the test gate (`probe::LOOKUP_GATE`), so the wait is certain; the
/// connection phase waits through the same `Watch` (`cancel_wakes_a_
/// socket_wait` in `vm/watch.rs` covers the wait itself). An address
/// that never answers would depend on the network the tests run in.
#[test]
fn cancel_ends_a_blocked_connect() {
    let _g = serial();
    let gate = probe::hold_lookups();
    let o = run_then(
        "t = spawn(() => net.connect(\"gate.invalid\", 80))\nuntil(() => net_waiting() == 1)\ncancel(t)\nprint(await(t))",
        move || drop(gate),
    );
    assert_eq!(out(&o), ("Err({kind: 'cancelled})\n", "", 0));
}

/// A lookup that outlives the timeout: `dial tcp: lookup <host>: i/o
/// timeout` (Go's text for a lookup cut by the dial deadline).
#[test]
fn a_connect_timeout_covers_the_lookup() {
    let _g = serial();
    let gate = probe::hold_lookups();
    let o = run_then(
        "print(net.connect(\"gate.invalid\", 80, timeout: 30ms))",
        move || drop(gate),
    );
    assert_eq!(
        out(&o),
        (
            "Err({kind: 'timeout, message: \"dial tcp: lookup gate.invalid: i/o timeout\"})\n",
            "",
            0
        )
    );
}

/// `run`, with `release` called after the program (the lookup gate is
/// opened before the leak check, so the held thread can finish).
pub(super) fn run_then(src: &str, release: impl FnOnce()) -> Outcome {
    let o = exec(
        &format!("import _native.net as net\n{UNTIL}{HELPERS}{src}"),
        Opts {
            internal: true,
            extra: net_probes,
            ..Opts::default()
        },
    );
    release();
    assert_eq!(probe::sockets(), 0, "sockets left open\n{o:?}");
    assert_eq!(live_wakes(), 0, "wake pipes left open\n{o:?}");
    wait_for(|| probe::lookups() == 0);
    o
}

#[test]
fn exit_from_another_task_ends_a_blocked_accept() {
    let _g = serial();
    let o = run(
        "l = net.listen(\"127.0.0.1:0\")!\nt = spawn(() => net.accept(l))\nspawn(fn(): until(() => net_waiting() == 1); exit(3) ;;)\nprint(await(t))",
    );
    assert_eq!((o.out.as_str(), o.exit), ("", 3));
}

/// A task waiting on the network is not blocked for the deadlock check:
/// the entry awaiting it is no deadlock, though no fung task can run
/// meanwhile (the client is a Rust thread that connects only once the
/// task waits in `accept`).
#[test]
fn awaiting_a_task_in_accept_is_no_deadlock() {
    let _g = serial();
    let port = free_port();
    let client = std::thread::spawn(move || {
        wait_for(|| probe::waiting() == 1);
        let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
        c.write_all(b"ok\n").unwrap();
    });
    let o = run(&format!(
        "l = net.listen(\"127.0.0.1:{port}\")!\nt = spawn(() => net.read_line(net.accept(l)!, 10))\nprint(await(t))"
    ));
    client.join().unwrap();
    assert_eq!(out(&o), ("Ok(\"ok\")\n", "", 0));
}

/// One reader at a time per connection: a second reader waits for the
/// first (on the read lock, not cancellable meanwhile) and then reads
/// what follows.
#[test]
fn a_second_reader_waits_for_the_first() {
    let _g = serial();
    let o = run(&format!(
        "{PAIR}r1 = spawn(() => net.read_line(s, 10))\nuntil(() => net_waiting() == 1)\nr2 = spawn(() => net.read_line(s, 10))\nnet.write(c, \"one\\ntwo\\n\")!\nprint(await(r1), await(r2))"
    ));
    assert_eq!(out(&o), ("Ok(\"one\") Ok(\"two\")\n", "", 0));
}

/// A body read in raw pieces is decoded once: a character split between
/// two reads comes out whole. The cap counts what the buffer holds.
#[test]
fn raw_reads_gather_a_body_decoded_once() {
    let _g = serial();
    let (port, h) = peer(|mut s| s.write_all("a한b한!".as_bytes()).unwrap());
    // "a" + first byte of 한 | other two bytes + "b" + 한, then 1 too many
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port})!\nprint(net.read_bytes_into(c, 2, 10), net.read_bytes_into(c, 6, 10))\nprint([net.take_body(c), net.take_body(c)])\nprint(net.read_bytes_into(c, 1, 0))\nprint(net.read_bytes_into(c, 1, 1), net.read_bytes_into(c, 1, 2))"
    ));
    h.join().unwrap();
    assert_eq!(
        out(&o),
        (
            "Ok(nil) Ok(nil)\n[\"a한b한\", \"\"]\nErr({kind: 'limit, message: \"body longer than 0 bytes\"})\nOk(nil) Err({kind: 'network, message: \"EOF\"})\n",
            "",
            0
        )
    );
}

#[test]
fn a_raw_read_past_the_cap_reads_nothing() {
    let _g = serial();
    let (port, h) = peer(|mut s| s.write_all(b"abcdef").unwrap());
    let o = run(&format!(
        "c = net.connect(\"127.0.0.1\", {port})!\nprint(net.read_bytes_into(c, 4, 5), net.read_bytes_into(c, 2, 5))\nprint([net.take_body(c)], net.read_bytes(c, 2))\nprint(net.read_bytes_into(c, 1, 5))"
    ));
    h.join().unwrap();
    assert_eq!(
        out(&o),
        (
            "Ok(nil) Err({kind: 'limit, message: \"body longer than 5 bytes\"})\n[\"abcd\"] Ok(\"ef\")\nErr({kind: 'network, message: \"EOF\"})\n",
            "",
            0
        )
    );
}

#[test]
fn wrong_arguments_are_e0403() {
    let _g = serial();
    for (src, want) in [
        ("net.read_line(1, 2)", "argument `c` must be conn, got int"),
        ("net.accept(1)", "argument `l` must be listener, got int"),
        ("net.connect(1, 2)", "argument `host` must be str, got int"),
        (
            "net.connect(\"h\", \"2\")",
            "argument `port` must be int, got str",
        ),
        (
            "net.connect(\"h\", 2, tls: 1)",
            "argument `tls` must be bool, got int",
        ),
        (
            "net.connect(\"h\", 2, timeout: 1)",
            "argument `timeout` must be dur, got int",
        ),
        (
            "net.set_deadline(1, 1s)",
            "argument `c` must be conn, got int",
        ),
        ("net.byte_len(1)", "argument `s` must be str, got int"),
        ("net.utf8_bytes(1)", "argument `s` must be str, got int"),
        (
            "net.need_str_map(1, {})",
            "argument `name` must be str, got int",
        ),
        ("net.take_body(1)", "argument `c` must be conn, got int"),
        (
            "net.read_bytes_into(1, 1, 1)",
            "argument `c` must be conn, got int",
        ),
        (
            "net.local_addr(1)",
            "argument `x` must be conn | listener, got int",
        ),
    ] {
        let o = run(src);
        assert!(
            o.err.starts_with(&format!("error[E0403]: {want}")),
            "{src}: {}",
            o.err
        );
    }
    let o = run(&format!("{PAIR}net.read_bytes(c, -1)"));
    assert!(
        o.err
            .starts_with("error[E0403]: argument `n` must be a non-negative int"),
        "{}",
        o.err
    );
}

/// A cancelled task stops between reads and between writes even when
/// the socket never makes it wait (a fast peer): the loops check.
#[test]
fn reads_and_writes_stop_on_cancel_without_waiting() {
    let _g = serial();
    let (port, h) = peer(|mut s| {
        s.write_all(b"data").unwrap();
        let mut rest = Vec::new();
        let _ = s.read_to_end(&mut rest);
    });
    let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_nonblocking(true).unwrap();
    let peer_addr = s.peer_addr().unwrap();
    let c = super::Conn::new(s, peer_addr);
    let node = CancelNode::root();
    node.cancel();
    let mut w = Watch::new(node);
    assert!(matches!(
        c.read_all(&mut w, 100),
        Err(super::Fail::Cancelled)
    ));
    assert!(matches!(c.write(&mut w, b"x"), Err(super::Fail::Cancelled)));
    drop(c);
    drop(w);
    h.join().unwrap();
    assert_eq!(probe::sockets(), 0);
    assert_eq!(live_wakes(), 0);
}

/// `hold(l, grace)` (Task 5, the server's end, Go `Server.Shutdown`):
/// waits until its task is cancelled, then closes `l` and waits until
/// every connection accepted from `l` is closed, at most `grace`, and
/// ends with E0701. Here the open connection closes long before the
/// grace is over: the cancelled task ends then, not at the grace.
#[test]
fn hold_waits_for_the_connections_of_its_listener() {
    let _g = serial();
    let start = Instant::now();
    let o = run(&format!(
        "{PAIR}t = spawn(() => net.hold(l, 20s))\nuntil(() => net_waiting() == 1)\ncancel(t)\nuntil(() => kind(net.connect(\"127.0.0.1\", port)) == 'network)\nprint(await(t, timeout: 0s))\nnet.close(s)!\nprint(await(t))\nprint(net_waiting())"
    ));
    assert_eq!(
        out(&o),
        ("Err({kind: 'timeout})\nErr({kind: 'cancelled})\n0\n", "", 0)
    );
    assert!(start.elapsed() < Duration::from_secs(10));
}

/// A connection still open when the grace is over is left as it is.
/// A connection whose last copy goes counts as closed.
#[test]
fn hold_gives_up_after_the_grace() {
    let _g = serial();
    let start = Instant::now();
    let o = run(&format!(
        "{PAIR}t = spawn(() => net.hold(l, 200ms))\nuntil(() => net_waiting() == 1)\ncancel(t)\nprint(await(t))\nprint(net.write(s, \"x\\n\"), net.read_line(c, 5))"
    ));
    assert_eq!(
        out(&o),
        ("Err({kind: 'cancelled})\nOk(nil) Ok(\"x\")\n", "", 0)
    );
    assert!(start.elapsed() >= Duration::from_millis(200));
    let o = run(
        "l = net.listen(\"127.0.0.1:0\")!\nport = parse_int(split(net.local_addr(l), \":\")[1])!\nc = net.connect(\"127.0.0.1\", port)!\nr = await(spawn(() => net.accept(l)!))\nt = spawn(() => net.hold(l, 20s))\nuntil(() => net_waiting() == 1)\nr = nil\ncancel(t)\nprint(await(t))",
    );
    assert_eq!(out(&o), ("Err({kind: 'cancelled})\n", "", 0));
}

/// `hold` waits for nothing while its task is not cancelled, and wrong
/// arguments are E0403.
#[test]
fn hold_checks_its_arguments() {
    let _g = serial();
    for (src, want) in [
        ("net.hold(1, 1s)", "argument `l` must be listener, got int"),
        (
            "net.hold(net.listen(\"127.0.0.1:0\")!, 1)",
            "argument `grace` must be dur, got int",
        ),
        ("net.utf8_string(1)", "argument `bs` must be list, got int"),
        (
            "net.utf8_string([1, 256])",
            "argument `bs` must be a list of bytes (ints from 0 to 255), got int",
        ),
    ] {
        let o = run(src);
        assert!(
            o.err.starts_with(&format!("error[E0403]: {want}")),
            "{src}: {}",
            o.err
        );
    }
}

/// `utf8_string(bs)` is `utf8_bytes` backwards: the text of the bytes, a
/// byte that is not UTF-8 becoming U+FFFD (the `http` server decodes
/// percent-escapes with it).
#[test]
fn utf8_string_decodes_bytes() {
    let _g = serial();
    let o = run(
        "print([net.utf8_string([237, 149, 156, 97]), net.utf8_string([]), net.utf8_string([97, 255, 98]), net.utf8_string(net.utf8_bytes(\"é~\"))])",
    );
    assert_eq!(
        out(&o),
        ("[\"한a\", \"\", \"a\u{fffd}b\", \"é~\"]\n", "", 0)
    );
}

// ---- `http.serve` (Task 5): its tasks, its end, what it leaves --------
//
// The server is fung (`extras/rust/http.fung`); these runs check what
// the Go comparison cannot see: the order of concurrent requests (by
// state: a handler waiting on a channel), the end of the server task,
// and that no socket, wake pipe or task is left (`run` checks).

/// The start of a program with a server on `h` (a fung function defined
/// before): `s` is its task, `base` its URL.
const SERVE: &str = "port = channel(1)\ns = spawn(() => http.serve(\"127.0.0.1:0\", h, ready: a => send(port, a)))\naddr = recv(port)!\nbase = \"http://\" + addr\n";

/// A handler that waits does not hold up other requests: each request
/// runs on a task of its own (Review Focus 5).
#[test]
fn a_slow_handler_does_not_hold_up_the_others() {
    let _g = serial();
    let o = run(&format!(
        "import http\ngate = channel()\nh = fn(req):\n  if req.path == \"/slow\":\n    recv(gate)!\n  ;;\n  req.path == \"/slow\" then \"slow\" else \"fast\"\n;;\n{SERVE}t = spawn(() => http.get(base + \"/slow\")!.body)\nblocked(gate, 1)\nprint(http.get(base + \"/fast\")!.body)\nprint(http.get(base + \"/fast\")!.body)\nsend(gate, 1)\nprint(await(t))\ncancel(s)\nprint(await(s))\nuntil(() => background() == 0 and sockets() == 0)\nprint(\"no task left\")"
    ));
    assert_eq!(
        out(&o),
        (
            "fast\nfast\nslow\nErr({kind: 'cancelled})\nno task left\n",
            "",
            0
        )
    );
}

/// Cancelling the server cancels a handler that is running: its request
/// gets 503 (not reported), and the server task ends once that answer
/// is out, well before the 2 s grace (Go `Shutdown`; Review Focus 1).
#[test]
fn cancelling_the_server_answers_running_requests_503() {
    let _g = serial();
    let start = Instant::now();
    let o = run(&format!(
        "import http\ngate = channel()\nh = fn(req): recv(gate); \"never\" ;;\n{SERVE}t = spawn(() => http.get(base + \"/slow\"))\nblocked(gate, 1)\ncancel(s)\nprint(await(s))\nr = await(t)!\nprint(r.status, r.body)\nprint(kind(net.connect(\"127.0.0.1\", parse_int(split(addr, \":\")[1])!)))"
    ));
    assert_eq!(
        out(&o),
        (
            "Err({kind: 'cancelled})\n503 server shutting down\n\n'network\n",
            "",
            0
        )
    );
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
}

/// A connection that never sends its request keeps the server task
/// waiting the whole 2 s grace after a cancel, no longer; the task
/// reading it ends with the program (`run` checks nothing is left).
#[test]
fn the_server_waits_two_seconds_at_most() {
    let _g = serial();
    let o = run(&format!(
        "import http\nimport time\nfn h(req) = \"x\"\n{SERVE}until(() => net_waiting() == 2)\nc = net.connect(\"127.0.0.1\", parse_int(split(addr, \":\")[1])!)!\nuntil(() => net_waiting() == 3)\nt0 = time.now()\ncancel(s)\nprint(await(s))\ntook = time.now() - t0\nprint(took >= 2s, took < 4s)"
    ));
    assert_eq!(out(&o), ("Err({kind: 'cancelled})\ntrue true\n", "", 0));
}

/// `hold` also ends when another task closes the listener: then it
/// waits for the connections the same way and returns nil (the `http`
/// server's accept task closes it when it fails).
#[test]
fn hold_returns_when_the_listener_is_closed_elsewhere() {
    let _g = serial();
    let o = run(
        "l = net.listen(\"127.0.0.1:0\")!\nt = spawn(() => net.hold(l, 5s))\nuntil(() => net_waiting() == 1)\nnet.close(l)!\nprint(await(t))",
    );
    assert_eq!(out(&o), ("nil\n", "", 0));
}

/// The server task cancelled at any tick between starting its accept task
/// and waiting in `hold`: the listener still closes (the accept task runs
/// below the server's task). `burn(k)` in `ready` moves where the server
/// task yields its turn; with `k` such that the yield falls right after
/// the accept task starts, the listener used to stay open (the accept
/// task kept it and answered 503). The window is one tick wide; this
/// range holds it for the current `_serve` (the release build sweeps a
/// whole yield period: `cancel_at_every_tick_closes_the_listener`).
#[test]
fn a_cancel_before_hold_closes_the_listener() {
    let _g = serial();
    let o = run(&cancel_sweep(9900, 10001));
    assert_eq!(out(&o), ("done\n", "", 0));
}

#[cfg(not(debug_assertions))]
#[test]
fn cancel_at_every_tick_closes_the_listener() {
    let _g = serial();
    let o = run(&cancel_sweep(0, crate::vm::YIELD_EVERY as i64 + 1));
    assert_eq!(out(&o), ("done\n", "", 0));
}

/// A chunk-size line is read in one native call, not a byte at a time:
/// 2,000 chunks of 1,000 bytes whose size lines carry a 1,000-byte
/// extension (Go ignores it) read in well under half a second. A byte
/// per call took about 1.8 s in release on an Apple M-series machine;
/// one call per line, about 0.06 s.
#[cfg(not(debug_assertions))]
#[test]
fn a_chunk_size_line_is_read_whole() {
    let _g = serial();
    let (port, h) = peer(|mut s| {
        let mut b = [0u8; 4096];
        let _ = s.read(&mut b);
        let mut res = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
        for _ in 0..2000 {
            res.extend_from_slice(b"3e8;");
            res.extend_from_slice(&[b'e'; 1000]);
            res.extend_from_slice(b"\r\n");
            res.extend_from_slice(&[b'x'; 1000]);
            res.extend_from_slice(b"\r\n");
        }
        res.extend_from_slice(b"0\r\n\r\n");
        s.write_all(&res).unwrap();
    });
    let start = Instant::now();
    let o = run(&format!(
        "import http\nprint(len(http.get(\"http://127.0.0.1:{port}/\")!.body))"
    ));
    let took = start.elapsed();
    h.join().unwrap();
    assert_eq!(out(&o), ("2000000\n", "", 0));
    assert!(took < Duration::from_millis(500), "took {took:?}");
}

/// A program that starts a server, burns `k` ticks in `ready`, cancels
/// it, and checks that nothing answers HTTP on its port afterwards, for
/// each `k` in `from..to`. (Only an HTTP answer counts: in a busy test
/// process another test may bind the freed port number at once.)
fn cancel_sweep(from: i64, to: i64) -> String {
    // What keeps a reused port from a false positive is the caller's
    // `serial()` lock: no other HTTP server in this process runs at the
    // same time, so an answer on the freed port can only come from the
    // server that should be closed. (Another process could still bind
    // the port, but would hardly answer HTTP within the 2 s deadline.)
    format!(
        "import http\nfn burn(n) = n == 0 then nil else burn(n - 1)\nfn answers(c) = match c:\n  Ok(c) => fn():\n    net.set_deadline(c, 2s)\n    net.write(c, \"GET / HTTP/1.1\\r\\nHost: x\\r\\n\\r\\n\")\n    match net.read_line(c, 100):\n      Ok(line) when line != nil => starts_with(line, \"HTTP/\")\n      _                         => false\n    ;;\n  ;;()\n  _ => false\n;;\nfn attempt(k):\n  port = channel(1)\n  s = spawn(() => http.serve(\"127.0.0.1:0\", r => \"x\", ready: fn(a): send(port, a); burn(k) ;;))\n  addr = recv(port)!\n  cancel(s)\n  r = await(s)\n  c = net.connect(\"127.0.0.1\", parse_int(split(addr, \":\")[1])!)\n  if r != Err({{kind: 'cancelled}}) or answers(c):\n    print(\"k=${{k}}\", r, c)\n  ;;\n;;\neach({from}..{to}, attempt)\nprint(\"done\")"
    )
}

/// `_native.net` with `member` replaced by a native that fails with
/// E0803 `injected`: a bug in the server's own code, which no request can
/// cause.
fn failing_net(rt: &crate::vm::Runtime, member: &'static str, params: Vec<crate::vm::NativeParam>) {
    let real = super::module();
    let mut m = crate::value::Module::new("net", "_native.net");
    for name in real.names() {
        let v = if name == member {
            Value::Native(Native::leak(member, params.clone(), |_, _| {
                Err(crate::native::nat::fail(
                    "E0803",
                    vec![crate::native::nat::s("injected")],
                ))
            }))
        } else {
            real.get(name).unwrap().clone()
        };
        m.set(name, v);
    }
    rt.register_native("_native.net", std::rc::Rc::new(m));
}

/// A failure in the code answering a connection (here `set_deadline`,
/// its first call) is reported on stderr, the connection is closed, and
/// the server goes on (Go logs `http: panic serving …` the same way).
#[test]
fn a_failure_answering_a_connection_is_reported() {
    let _g = serial();
    let o = exec_prepared(
        &format!(
            "import _native.net as net\nimport http\n{UNTIL}{HELPERS}fn h(req) = \"x\"\n{SERVE}p = parse_int(split(addr, \":\")[1])!\nask = fn():\n  c = net.connect(\"127.0.0.1\", p)!\n  net.write(c, \"GET / HTTP/1.1\\r\\nHost: x\\r\\n\\r\\n\")!\n  net.read_line(c, 100)\n;;\nprint(ask(), ask())\ncancel(s)\nprint(await(s))"
        ),
        Opts {
            internal: true,
            extra: net_probes,
            ..Opts::default()
        },
        |rt| {
            failing_net(
                rt,
                "set_deadline",
                vec![
                    crate::native::nat::param("c"),
                    crate::native::nat::param("d"),
                ],
            )
        },
    );
    assert_eq!(
        out(&o),
        (
            "Ok(nil) Ok(nil)\nErr({kind: 'cancelled})\n",
            "http: internal error: error[E0803]: injected\nhttp: internal error: error[E0803]: injected\n",
            0
        )
    );
    assert_eq!(probe::sockets(), 0);
}

/// A failure in the accept loop ends the server: the listener closes,
/// and the server task fails with that error (it would otherwise wait,
/// deaf, until cancelled).
#[test]
fn a_failure_accepting_ends_the_server() {
    let _g = serial();
    let o = exec_prepared(
        &format!(
            "import _native.net as net\nimport http\n{UNTIL}{HELPERS}fn h(req) = \"x\"\n{SERVE}r = await(s)\nprint(r)\nprint(kind(net.connect(\"127.0.0.1\", parse_int(split(addr, \":\")[1])!)))"
        ),
        Opts {
            internal: true,
            extra: net_probes,
            ..Opts::default()
        },
        |rt| failing_net(rt, "accept", vec![crate::native::nat::param("l")]),
    );
    assert_eq!(
        out(&o),
        (
            "Err({kind: 'error, message: \"error[E0803]: injected\"})\n'network\n",
            "",
            0
        )
    );
    assert_eq!(probe::sockets(), 0);
}

/// Calls of `spawn_background` so far, for `failing_spawn`.
static SPAWNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// `_native.conc` with a `spawn_background` whose second call (the
/// server's first connection: the first starts the accept task) fails
/// with E0801 `cannot start a task`, as when no thread can be made.
fn failing_spawn(rt: &crate::vm::Runtime) {
    use std::sync::atomic::Ordering;
    SPAWNS.store(0, Ordering::SeqCst);
    let real = crate::conc::module();
    let mut m = crate::value::Module::new("conc", "_native.conc");
    for name in real.names() {
        let v = if name == "spawn_background" {
            Value::Native(Native::leak(
                "spawn_background",
                vec![crate::native::nat::param("f")],
                |t, args| {
                    if SPAWNS.fetch_add(1, Ordering::SeqCst) == 1 {
                        return Err(crate::vm::internal(vec![fung_front::diag::Arg::Str(
                            "cannot start a task: injected".into(),
                        )]));
                    }
                    crate::vm::agent::spawn_background(t, args)
                },
            ))
        } else {
            real.get(name).unwrap().clone()
        };
        m.set(name, v);
    }
    rt.register_native("_native.conc", std::rc::Rc::new(m));
}

/// A connection whose task cannot start (no thread left) is closed and
/// reported, and the server keeps accepting (it used to stop serving).
#[test]
fn a_connection_task_that_cannot_start_is_dropped() {
    let _g = serial();
    let o = exec_prepared(
        &format!(
            "import _native.net as net\nimport http\n{UNTIL}{HELPERS}fn h(req) = \"x\"\n{SERVE}p = parse_int(split(addr, \":\")[1])!\nc = net.connect(\"127.0.0.1\", p)!\nnet.set_deadline(c, 5s)\nprint(net.read_line(c, 100))\nnet.close(c)\nprint(http.get(base)!.body)\ncancel(s)\nprint(await(s))"
        ),
        Opts {
            internal: true,
            extra: net_probes,
            ..Opts::default()
        },
        failing_spawn,
    );
    assert_eq!(
        out(&o),
        (
            "Ok(nil)\nx\nErr({kind: 'cancelled})\n",
            "http: cannot start a connection task: error[E0801]: internal error: cannot start a task: injected\n",
            0
        )
    );
    assert_eq!(probe::sockets(), 0);
}

/// `close_on_cancel(l)`: the listener closes inside `cancel` itself (the
/// connection right after is refused, before the task has even run
/// again), and when the task ends; arguments are checked.
#[test]
fn close_on_cancel_closes_the_listener_at_once() {
    let _g = serial();
    let o = run(
        "l = net.listen(\"127.0.0.1:0\")!\nport = parse_int(split(net.local_addr(l), \":\")[1])!\nch = channel()\nt = spawn(fn(): net.close_on_cancel(l); recv(ch) ;;)\nblocked(ch, 1)\nprint(kind(net.connect(\"127.0.0.1\", port)))\ncancel(t)\nprint(kind(net.connect(\"127.0.0.1\", port)), kind(net.close(l)))\nl2 = net.listen(\"127.0.0.1:0\")!\nawait(spawn(() => net.close_on_cancel(l2)))\nprint(kind(net.accept(l2)))",
    );
    assert_eq!(
        (o.out.lines().collect::<Vec<_>>(), o.err.as_str(), o.exit),
        (vec!["Ok(<conn>)", "'network 'network", "'network"], "", 0),
        "{o:?}"
    );
    let o = run("net.close_on_cancel(1)");
    assert!(
        o.err
            .starts_with("error[E0403]: argument `l` must be listener, got int"),
        "{}",
        o.err
    );
}
