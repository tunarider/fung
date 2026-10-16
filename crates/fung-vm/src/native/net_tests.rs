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
use crate::testutil::{Opts, Outcome, UNTIL, exec, probes};
use crate::value::{Native, Value};
use crate::vm::{Stop, Task, live_wakes, watch_serial};

/// One net test at a time: the counters are process-wide (the lock is
/// shared with the wake-pipe tests of `vm/watch.rs`).
fn serial() -> MutexGuard<'static, ()> {
    watch_serial()
}

/// The probes plus the net ones: `net_waiting()` (tasks inside a socket
/// wait now), `read_waits()` / `write_waits()` (waits for reading or
/// writing so far), `sockets()` (live sockets).
fn net_probes() -> Vec<Value> {
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
const HELPERS: &str = "fn msg(r) = match r:\n  Err(e) => e.message\n  v => v\n;;\nfn kind(r) = match r:\n  Err(e) => e.kind\n  v => v\n;;\nfn host(a) = join(reverse(drop(reverse(split(a, \":\")), 1)), \":\")\n";

/// Runs `src` with `_native.net` imported as `net`, the probes and
/// `until`; then checks that nothing is left open: no socket, no wake
/// pipe, no lookup thread (one may still be finishing: it is waited for
/// by state, with a generous bound).
fn run(src: &str) -> Outcome {
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

fn out(o: &Outcome) -> (&str, &str, i32) {
    (o.out.as_str(), o.err.as_str(), o.exit)
}

/// Waits (by state) until `f` holds.
fn wait_for(mut f: impl FnMut() -> bool) {
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
            "net.connect(\"nosuchhost.invalid\", 80)".into(),
            "Err({kind: 'network, message: \"dial tcp: lookup nosuchhost.invalid: no such host\"})"
                .into(),
        ),
        (
            "net.connect(\"127.0.0.1\", 65536)".into(),
            "Err({kind: 'network, message: \"dial tcp: address 65536: invalid port\"})".into(),
        ),
        (
            "net.connect(\"127.0.0.1\", 80, tls: true)".into(),
            "Err({kind: 'network, message: \"tls is not supported yet\"})".into(),
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
        (
            "net.listen(\"nosuchhost.invalid:0\")".into(),
            "Err({kind: 'network, message: \"listen tcp: lookup nosuchhost.invalid: no such host\"})"
                .into(),
        ),
    ];
    let src: String = cases.iter().map(|(e, _)| format!("print({e})\n")).collect();
    let want: String = cases.iter().map(|(_, w)| format!("{w}\n")).collect();
    let o = run(&src);
    drop(busy);
    assert_eq!(out(&o), (want.as_str(), "", 0));
}

/// Go: `:0` listens on `[::]`, `[::1]:0` on `[::1]`, `[127.0.0.1]:0`
/// and `localhost:0` on 127.0.0.1, and a missing port is port 0.
#[test]
fn listen_addresses_read_like_go() {
    let _g = serial();
    let o = run(
        "each([\":0\", \"[::1]:0\", \"[127.0.0.1]:0\", \"localhost:0\", \"127.0.0.1:\"], fn(a):\n  l = net.listen(a)!\n  addr = net.local_addr(l)\n  print(host(addr), parse_int(last(split(addr, \":\")))! > 0)\n;;)",
    );
    assert_eq!(
        out(&o),
        (
            "[::] true\n[::1] true\n127.0.0.1 true\n127.0.0.1 true\n127.0.0.1 true\n",
            "",
            0
        )
    );
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
        "c = net.connect(\"127.0.0.1\", {port})!\nname = net.local_addr(c) + \"->127.0.0.1:{port}\"\nshow = (r) => replace(msg(r), name, \"A->B\")\nprint(show(net.read_line(c, 10)))\nprint(show(net.write(c, \"x\")))\nprint(net.close(c))\nprint(show(net.read_line(c, 10)))\nprint(show(net.write(c, \"x\")))\nprint(show(net.close(c)))\nprint(show(net.set_deadline(c, 1s)))"
    ));
    h.join().unwrap();
    assert_eq!(
        out(&o),
        (
            "read tcp A->B: read: connection reset by peer\nwrite tcp A->B: write: broken pipe\nOk(nil)\nread tcp A->B: use of closed network connection\nwrite tcp A->B: use of closed network connection\nclose tcp A->B: use of closed network connection\nset tcp A->B: use of closed network connection\n",
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
fn run_then(src: &str, release: impl FnOnce()) -> Outcome {
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
/// the entry awaiting it is no deadlock.
#[test]
fn awaiting_a_task_in_accept_is_no_deadlock() {
    let _g = serial();
    let o = run(
        "l = net.listen(\"127.0.0.1:0\")!\nport = parse_int(split(net.local_addr(l), \":\")[1])!\nt = spawn(() => net.read_line(net.accept(l)!, 10))\nspawn(fn(): until(() => net_waiting() == 1); c = net.connect(\"127.0.0.1\", port)!; net.write(c, \"ok\\n\")! ;;)\nprint(await(t))",
    );
    assert_eq!(out(&o), ("Ok(\"ok\")\n", "", 0));
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
