//! Channels (stage 3a, Task 4): sending, receiving and closing, plus the
//! rendezvous, close and cancellation cases a blocked send or recv must
//! decide.

use std::time::{Duration, Instant};

use crate::testutil::{Opts, exec, exec_waiting, run};

#[test]
fn a_native_blocked_inside_a_lambda_still_works() {
    let (out, err, code) = run(
        "(tx, rx) = channel()\nscope(fn(s):\n  spawn(s, fn(): each(1..=5, x => send(tx, x)); close(tx) ;;)\n  print(fold(rx, 0, (a, x) => a + x))\n;;)",
    );
    assert_eq!((out.as_str(), err.as_str(), code), ("15\n", "", 0));
}

#[test]
fn closed_and_buffered_channels() {
    let (out, _, code) = run(
        "(buf_tx, buf) = channel(2)\nsend(buf_tx, \"a\")\nclose(buf_tx)\n(_never_tx, never) = channel()\nprint(recv(buf), recv(buf), send(buf_tx, \"b\"), timeout(5ms, () => recv(never)))",
    );
    assert_eq!(
        (out.as_str(), code),
        (
            "Ok(\"a\") Err({kind: 'closed, message: \"channel closed\"}) Err({kind: 'closed, message: \"channel closed\"}) Err({kind: 'timeout, message: \"timed out after 5ms\"})\n",
            0
        )
    );
}

#[test]
fn cancelling_a_task_blocked_in_send_inside_each_ends_it() {
    let (out, _, code) = run(
        "(tx, _rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => each(1..=5, x => send(tx, x)))\n  cancel(t)\n  print(await(t))\n;;)",
    );
    assert_eq!(
        (out.as_str(), code),
        ("Err({kind: 'cancelled, message: \"cancelled\"})\n", 0)
    );
}

#[test]
fn channels_send_receive_and_close() {
    let cases = [
        (
            "(tx, rx) = channel()\nscope(fn(s):\n  spawn(s, () => send(tx, 1))\n  print(recv(rx))\n;;)",
            "Ok(1)\n",
        ),
        (
            "(tx, rx) = channel(2)\nsend(tx, 1)\nsend(tx, 2)\nclose(tx)\nprint(recv(rx), recv(rx), recv(rx), send(tx, 3))",
            "Ok(1) Ok(2) Err({kind: 'closed, message: \"channel closed\"}) Err({kind: 'closed, message: \"channel closed\"})\n",
        ),
        (
            "(_tx, rx) = channel()\nprint(timeout(10ms, () => recv(rx)))",
            "Err({kind: 'timeout, message: \"timed out after 10ms\"})\n",
        ),
        (
            "(tx, rx) = channel(3)\neach(1..=3, x => send(tx, x))\nclose(tx)\nprint(for x in rx: x * 10 ;;)",
            "[10, 20, 30]\n",
        ),
        (
            "(tx, rx) = channel(3)\neach([3, 1, 2], x => send(tx, x))\nclose(tx)\nprint(sort(rx), close(tx))",
            "[1, 2, 3] Ok(nil)\n",
        ),
        (
            "(tx, rx) = channel(1)\nscope(fn(s):\n  t = spawn(s, fn():\n    each(0..100, i => send(tx, i))\n    close(tx)\n  ;;)\n  print(fold(rx, 0, (a, x) => a + x), await(t))\n;;)",
            "4950 Ok(nil)\n",
        ),
        (
            "(tx, rx) = channel()\n(tx2, rx2) = channel()\nprint(type_of(tx), type_of(rx), tx == tx2, rx == rx2)",
            "'sender 'receiver false false\n",
        ),
        (
            "(tx, rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => recv(rx))\n  send(tx, 'hi)\n  print(await(t))\n;;)",
            "Ok('hi)\n",
        ),
        (
            "(tx, _rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => send(tx, 1))\n  cancel(t)\n  print(await(t) |> is_err)\n;;)",
            "true\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", want), "{src}");
    }
    let errors = [
        ("channel(-1)", "E0403"),
        ("channel(\"a\")", "E0403"),
        ("send(1, 2)", "E0403"),
        ("recv(1)", "E0403"),
        ("close(1)", "E0403"),
    ];
    for (src, want) in errors {
        let o = exec(src, Opts::default());
        assert_eq!(o.code, want, "{src}");
    }
}

#[test]
fn the_argument_errors_name_the_argument() {
    let cases = [
        (
            "channel(-1)",
            "error[E0403]: argument `n` must be a non-negative int, got -1",
        ),
        (
            "channel(1.5)",
            "error[E0403]: argument `n` must be a non-negative int, got 1.5",
        ),
        (
            "send(1, 2)",
            "error[E0403]: argument `tx` must be sender, got int",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        let e = o.error.unwrap_or_else(|| panic!("{src}: no error"));
        assert_eq!(e.headline, want, "{src}");
    }
}

#[test]
fn a_channel_prints_compares_and_types() {
    let (out, _, code) = run(
        "(t, c) = channel()\n(_t2, c2) = channel()\nprint(t, c, [c], type_of(t), type_of(c), c == c, c == c2, c != 1)",
    );
    assert_eq!(
        (out.as_str(), code),
        (
            "<sender> <receiver> [<receiver>] 'sender 'receiver true false true\n",
            0
        )
    );
    let cases = [
        (
            "(_a, ra) = channel()\n(_b, rb) = channel()\nra < rb",
            "E0501",
        ),
        ("(a, _ra) = channel()\n(b, _rb) = channel()\na < b", "E0501"),
        ("(_tx, rx) = channel()\nlen(rx)", "E0506"),
        ("(_tx, rx) = channel()\n1 in rx", "E0512"),
        ("(_tx, rx) = channel(1)\nrx[0]", "E0509"),
    ];
    for (src, want) in cases {
        assert_eq!(exec(src, Opts::default()).code, want, "{src}");
    }
}

/// An unbuffered send waits until a receiver takes the value; a send to
/// a full buffer waits for room.
#[test]
fn a_send_waits_for_a_receiver_or_for_room() {
    let cases = [
        (
            "(tx, rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => send(tx, 1))\n  print(timeout(20ms, () => await(t)))\n  print(recv(rx), await(t))\n;;)",
            "Err({kind: 'timeout, message: \"timed out after 20ms\"})\nOk(1) Ok(nil)\n",
        ),
        (
            "(tx, rx) = channel(1)\nscope(fn(s):\n  print(send(tx, 1))\n  t = spawn(s, () => send(tx, 2))\n  print(timeout(20ms, () => await(t)), recv(rx), await(t), recv(rx))\n;;)",
            "Ok(nil)\nErr({kind: 'timeout, message: \"timed out after 20ms\"}) Ok(1) Ok(nil) Ok(2)\n",
        ),
        (
            "(tx, rx) = channel()\nscope(fn(s):\n  ts = for i in 0..4: spawn(s, () => send(tx, i)) ;;\n  xs = for _ in 0..4: recv(rx)! ;;\n  print(sort(xs), map(ts, await))\n;;)",
            "[0, 1, 2, 3] [Ok(nil), Ok(nil), Ok(nil), Ok(nil)]\n",
        ),
        (
            "(tx, rx) = channel(2)\nscope(fn(s):\n  spawn(s, fn(): each(0..1000, i => send(tx, i)); close(tx) ;;)\n  print(len(map(rx, x => x)))\n;;)",
            "1000\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", want), "{src}");
    }
}

/// `close` wakes everyone: a waiting sender gets `closed` and its value
/// is never delivered; a waiting receiver gets `closed`.
#[test]
fn close_wakes_waiting_senders_and_receivers() {
    let cases = [
        (
            "(tx, rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => send(tx, 1))\n  blocked(rx, 1)\n  close(tx)\n  print(await(t), recv(rx))\n;;)",
            "Err({kind: 'closed, message: \"channel closed\"}) Err({kind: 'closed, message: \"channel closed\"})\n",
        ),
        (
            "(tx, rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => recv(rx))\n  blocked(rx, 1)\n  close(tx)\n  print(await(t))\n;;)",
            "Err({kind: 'closed, message: \"channel closed\"})\n",
        ),
        (
            "(tx, rx) = channel(1)\nscope(fn(s):\n  send(tx, 1)\n  t = spawn(s, () => send(tx, 2))\n  blocked(rx, 1)\n  close(tx)\n  print(await(t), recv(rx), recv(rx))\n;;)",
            "Err({kind: 'closed, message: \"channel closed\"}) Ok(1) Err({kind: 'closed, message: \"channel closed\"})\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec_waiting(src);
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", want), "{src}");
    }
}

/// A blocked send or recv: once the other side has taken (or delivered) the
/// value, the operation succeeded, even if the task is cancelled before it
/// wakes; a cancelled wait that got nothing unwinds with E0701.
#[test]
fn a_delivered_value_wins_over_a_later_cancel() {
    let cases = [
        (
            "(tx, rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => send(tx, 1))\n  r = recv(rx)\n  cancel(t)\n  print(r, await(t))\n;;)",
            "Ok(1) Ok(nil)\n",
        ),
        (
            "(tx, rx) = channel()\nscope(fn(sc):\n  t = spawn(sc, () => recv(rx))\n  s = send(tx, 1)\n  cancel(t)\n  print(s, await(t))\n;;)",
            "Ok(nil) Ok(1)\n",
        ),
        (
            "(tx, rx) = channel()\nscope(fn(sc):\n  t = spawn(sc, () => timeout(10s, () => recv(rx)))\n  s = send(tx, 1)\n  cancel(t)\n  print(s, await(t))\n;;)",
            "Ok(nil) Ok(1)\n",
        ),
        (
            "(_tx, rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => recv(rx))\n  cancel(t)\n  print(await(t))\n  t2 = spawn(s, () => timeout(10s, () => recv(rx)))\n  blocked(rx, 1)\n  cancel(t2)\n  print(await(t2))\n;;)",
            "Err({kind: 'cancelled, message: \"cancelled\"})\nErr({kind: 'cancelled, message: \"cancelled\"})\n",
        ),
    ];
    // Here the task is already blocked when the other side moves the
    // value, and wakes to a cancelled flag. (These waited with a fixed 5 ms
    // sleep, and the buffered one hung under load: a sender that had not
    // started was cancelled before it sent, so the last `recv` waited
    // for a value that never came, a deadlock: E0702.)
    let blocked_first = [
        (
            "(tx, rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => send(tx, 1))\n  blocked(rx, 1)\n  r = recv(rx)\n  cancel(t)\n  print(r, await(t))\n;;)",
            "Ok(1) Ok(nil)\n",
        ),
        (
            "(tx, rx) = channel()\nscope(fn(sc):\n  t = spawn(sc, () => recv(rx))\n  blocked(rx, 1)\n  s = send(tx, 1)\n  cancel(t)\n  print(s, await(t))\n;;)",
            "Ok(nil) Ok(1)\n",
        ),
        (
            "(tx, rx) = channel()\nscope(fn(sc):\n  t = spawn(sc, () => timeout(10s, () => recv(rx)))\n  blocked(rx, 1)\n  s = send(tx, 1)\n  cancel(t)\n  print(s, await(t))\n;;)",
            "Ok(nil) Ok(1)\n",
        ),
        (
            "(tx, rx) = channel(1)\nscope(fn(s):\n  send(tx, 0)\n  t = spawn(s, () => send(tx, 1))\n  blocked(rx, 1)\n  r = recv(rx)\n  cancel(t)\n  print(r, await(t), recv(rx))\n;;)",
            "Ok(0) Ok(nil) Ok(1)\n",
        ),
    ];
    for _ in 0..10 {
        for (src, want) in cases.iter().chain(blocked_first.iter()) {
            let o = exec_waiting(src);
            assert_eq!((o.code.as_str(), o.out.as_str()), ("", *want), "{src}");
        }
    }
}

/// A cancelled sender leaves the channel: a later receiver does not get
/// its value.
#[test]
fn a_cancelled_sender_takes_its_value_back() {
    let start = Instant::now();
    let o = exec_waiting(
        "(tx, rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => send(tx, 1))\n  blocked(rx, 1)\n  cancel(t)\n  print(await(t), timeout(20ms, () => recv(rx)))\n;;)",
    );
    assert_eq!(
        (o.out.as_str(), o.exit),
        (
            "Err({kind: 'cancelled, message: \"cancelled\"}) Err({kind: 'timeout, message: \"timed out after 20ms\"})\n",
            0
        )
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}

/// Every iterating native walks a channel as its values arrive (05),
/// stops where it stops, and checks the collection before the function.
#[test]
fn iterating_natives_walk_a_channel() {
    let cases = [
        (
            "(t, c) = channel()\nclose(t)\nprint(any(c, x => true), all(c, x => false), group_by(c, x => \"a\"), sort_by(c, x => x), filter(c, x => true), fold(c, 1, (a, b) => a), each(c, x => x))",
            "false true {} [] [] 1 nil\n",
        ),
        (
            "(t, c) = channel(1)\nsend(t, [1])\nclose(t)\nprint(flatten(c))",
            "[1]\n",
        ),
        (
            "(t, c) = channel(3)\neach([\"a\", 1, nil], x => send(t, x))\nclose(t)\nprint(join(c, \",\"))",
            "a,1,nil\n",
        ),
        (
            "(t, c) = channel(3)\neach([1, 2, 3], x => send(t, x))\nclose(t)\nprint(find(c, x => x > 1), recv(c))",
            "2 Ok(3)\n",
        ),
        (
            "(t, c) = channel(3)\neach([1, 2, 3], x => send(t, x))\nclose(t)\nprint(any(c, x => x == 1), recv(c))",
            "true Ok(2)\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", want), "{src}");
    }
    // `map(c, 3)` reports the bad function before it receives anything.
    let o = exec(
        "(t, c) = channel(1)\nsend(t, 1)\nclose(t)\nmap(c, 3)",
        Opts::default(),
    );
    assert_eq!(o.code, "E0403");
}

/// A buffer of values: the values drop on the run thread, and
/// dropping the runtime does not hang or panic. (A value holding an end
/// of the channel itself cannot be buffered: `send` refuses it, E0705.)
#[test]
fn values_in_a_buffer_drop_with_the_runtime() {
    let o = exec(
        "(t, c) = channel(3)\nsend(t, [1, [2, [3]]])\nsend(t, () => 1)\nsend(t, [c])",
        Opts::default(),
    );
    assert!(o.err.starts_with("error[E0705]"), "{}", o.err);
    let o = exec(
        "(t, c) = channel(3)\nsend(t, [1, [2, [3]]])\nsend(t, () => 1)\nsend(t, {k: channel()})\nprint(recv(c))",
        Opts::default(),
    );
    assert_eq!(
        (o.out.as_str(), o.exit),
        ("Ok([1, [2, [3]]])\n", 0),
        "{}",
        o.err
    );
}

/// A cancelled waiter is never paired: cancellation claims it at once, even
/// though its slot stays queued until its task next runs. Here
/// one task cancels and pairs without giving its turn up in
/// between.
#[test]
fn a_cancelled_waiter_is_never_paired() {
    let cases = [
        // The wrong receiver: r2 must get the value, not the cancelled r1.
        (
            "(tx, rx) = channel()\nscope(fn(s):\n  r1 = spawn(s, () => recv(rx))\n  blocked(rx, 1)\n  r2 = spawn(s, () => recv(rx))\n  blocked(rx, 2)\n  cancel(r1)\n  print(send(tx, 1), await(r1), await(r2))\n;;)",
            "Ok(nil) Err({kind: 'cancelled, message: \"cancelled\"}) Ok(1)\n",
        ),
        // A lost value: the send must wait for the receiver that comes
        // after the cancel instead of handing the value to the cancelled one.
        (
            "(tx, rx) = channel()\nscope(fn(s):\n  r = spawn(s, () => recv(rx))\n  blocked(rx, 1)\n  cancel(r)\n  r2 = spawn(s, () => recv(rx))\n  print(send(tx, 1), await(r), await(r2))\n;;)",
            "Ok(nil) Err({kind: 'cancelled, message: \"cancelled\"}) Ok(1)\n",
        ),
        // A cancelled sender delivers nothing.
        (
            "(tx, rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => send(tx, 1))\n  blocked(rx, 1)\n  cancel(t)\n  print(timeout(20ms, () => recv(rx)), await(t))\n;;)",
            "Err({kind: 'timeout, message: \"timed out after 20ms\"}) Err({kind: 'cancelled, message: \"cancelled\"})\n",
        ),
        (
            "(tx, rx) = channel()\nscope(fn(s):\n  s1 = spawn(s, () => send(tx, 1))\n  blocked(rx, 1)\n  s2 = spawn(s, () => send(tx, 2))\n  blocked(rx, 2)\n  cancel(s1)\n  print(recv(rx), await(s1), await(s2))\n;;)",
            "Ok(2) Err({kind: 'cancelled, message: \"cancelled\"}) Ok(nil)\n",
        ),
        // The same with a full buffer: the cancelled sender's value does
        // not move into the freed place.
        (
            "(tx, rx) = channel(1)\nscope(fn(s):\n  send(tx, 0)\n  t = spawn(s, () => send(tx, 1))\n  blocked(rx, 1)\n  cancel(t)\n  print(recv(rx), timeout(20ms, () => recv(rx)), await(t))\n;;)",
            "Ok(0) Err({kind: 'timeout, message: \"timed out after 20ms\"}) Err({kind: 'cancelled, message: \"cancelled\"})\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec_waiting(src);
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", want), "{src}");
    }
}

/// A receiver whose deadline has passed is never paired, even before its
/// task wakes to see the timeout: `hold(ms)` keeps the run thread past
/// the deadline, then the send must go to the second receiver.
#[test]
fn a_timed_out_receiver_is_never_paired() {
    // r is blocked (or already timed out, under heavy load) before r2
    // exists, so only r can be the first queued receiver.
    let o = exec_waiting(
        "(tx, rx) = channel()\nscope(fn(s):\n  r = spawn(s, () => timeout(30ms, () => recv(rx)))\n  until(() => queued(rx) >= 1 or finished(r))\n  r2 = spawn(s, () => recv(rx))\n  hold(60)\n  print(send(tx, 1), await(r), await(r2))\n;;)",
    );
    assert_eq!(
        (o.code.as_str(), o.out.as_str()),
        (
            "",
            "Ok(nil) Err({kind: 'timeout, message: \"timed out after 30ms\"}) Ok(1)\n"
        )
    );
}

/// The interleaving that hung `a_delivered_value_wins_over_a_later_cancel`
/// under load, made deterministic: the entry keeps the run thread from the
/// spawn to the cancel, so the sender is cancelled before it ever runs
/// and sends nothing. A last `recv` without a timeout would wait forever
/// (a deadlock, E0702); with one it times out.
#[test]
fn a_sender_cancelled_before_it_runs_sends_nothing() {
    let (out, _, code) = run(
        "(tx, rx) = channel(1)\nscope(fn(s):\n  send(tx, 0)\n  t = spawn(s, () => send(tx, 1))\n  r = recv(rx)\n  cancel(t)\n  print(r, await(t), timeout(50ms, () => recv(rx)))\n;;)",
    );
    assert_eq!(
        (out.as_str(), code),
        (
            "Ok(0) Err({kind: 'cancelled, message: \"cancelled\"}) Err({kind: 'timeout, message: \"timed out after 50ms\"})\n",
            0
        )
    );
}

/// Stage 3b Task 5: a channel whose buffer held a value containing one of
/// its own ends would never be freed (`Rc`), so `send` refuses it.
#[test]
fn a_channel_inside_what_is_sent_is_e0705() {
    for src in [
        "(tx, _rx) = channel(1)\nsend(tx, tx)",
        "(tx, rx) = channel(1)\nsend(tx, [1, {k: (2, rx)}])",
        "(tx, rx) = channel(1)\nf = () => rx\nsend(tx, f)",
        "(tx, rx) = channel(1)\nsend(tx, Ok([(1..40) |> map(_ => [rx])]))",
        "(tx, rx) = channel(1)\nm = fold(0..20, {}, (a, i) => put(a, \"k${i}\", i))\nsend(tx, put(m, \"z\", rx))",
    ] {
        let o = exec(src, Opts::default());
        assert!(
            o.err.starts_with("error[E0705]: channel sent into itself"),
            "{src}: {}",
            o.err
        );
        assert_eq!(o.exit, 1, "{src}");
    }
}

#[test]
fn e0705_is_reported_at_the_send_call() {
    let o = exec(
        "(tx, rx) = channel(1)\nx = 1\n  send(tx, [rx])",
        Opts::default(),
    );
    let e = o.error.expect("an error");
    assert_eq!(
        (e.code.as_str(), e.line, e.col),
        ("E0705", 3, 7),
        "{}",
        o.err
    );
}

#[test]
fn other_channels_and_deep_values_are_fine() {
    let o = exec(
        "(a, _ra) = channel(1)\n(b, rb) = channel(1)\nsend(a, [b, rb])\nfn nest(x, n) = n == 0 then x else nest([x], n - 1)\nsend(b, nest(1, 1000000))\nprint(\"ok\")",
        Opts::default(),
    );
    assert_eq!((o.out.as_str(), o.exit), ("ok\n", 0), "{}", o.err);
}

#[test]
fn shared_sub_values_are_scanned_once() {
    // 2^40 paths through 40 levels of [x, x]: walking each path would
    // never end, walking each node once is instant. The leaf is an end
    // of another channel, so the value is walked.
    let o = exec(
        "(tx, _rx) = channel(1)\n(other, _other_rx) = channel()\nfn dup(x, n) = n == 0 then x else dup([x, x], n - 1)\nsend(tx, dup(other, 40))\nprint(\"ok\")\n(t2, c2) = channel(1)\nsend(t2, dup(c2, 40))",
        Opts::default(),
    );
    assert_eq!(o.out, "ok\n", "{}", o.err);
    assert!(o.err.starts_with("error[E0705]"), "{}", o.err);
}

// --- L4: ends ---

#[test]
fn channel_gives_two_ends() {
    assert_eq!(
        run(
            "(tx, rx) = channel(1)\nprint(tx, rx, type_of(tx), type_of(rx), tx == tx, tx == rx, send(tx, 5), recv(rx))"
        ),
        (
            "<sender> <receiver> 'sender 'receiver true false Ok(nil) Ok(5)\n".into(),
            "".into(),
            0
        )
    );
}

#[test]
fn the_wrong_end_is_a_type_error() {
    for (src, want) in [
        (
            "(tx, rx) = channel(1)\nsend(rx, 1)",
            "argument `tx` must be sender",
        ),
        (
            "(tx, rx) = channel(1)\nrecv(tx)",
            "argument `rx` must be receiver",
        ),
        (
            "(tx, rx) = channel(1)\nclose(rx)",
            "argument `tx` must be sender",
        ),
        ("(tx, rx) = channel(1)\nfor x in tx:\n  x\n;;", "E0506"),
    ] {
        let (_, err, code) = run(src);
        assert!(err.contains(want), "{src}: {err}");
        assert_eq!(code, 1, "{src}");
    }
}

#[test]
fn end_types_in_annotations() {
    assert_eq!(
        run(
            "fn f(r: receiver) = recv(r)\nfn g(t: sender) = send(t, 1)\n(tx, rx) = channel(1)\ng(tx)\nprint(f(rx))"
        ),
        ("Ok(1)\n".into(), "".into(), 0)
    );
    let (_, err, _) = run("fn h(c: channel) = c");
    assert!(
        err.contains("error[E0212]: unknown type `channel`"),
        "{err}"
    );
}

/// The function that makes the channel returns only the receiving end:
/// its own `tx` goes when it returns, the task's copy when the task ends,
/// and then the channel closes.
#[test]
fn a_producer_that_ends_closes_its_channel() {
    assert_eq!(
        run(
            "fn numbers(s, n):\n  (tx, rx) = channel()\n  spawn(s, () => each(0..n, i => send(tx, i)))\n  rx\n;;\nscope(fn(s):\n  for x in numbers(s, 3):\n    print(x)\n  ;;\n  print(\"done\")\n;;)"
        ),
        ("0\n1\n2\ndone\n".into(), "".into(), 0)
    );
}

/// A top-level variable keeps its end until the program ends: waiting on
/// the other end is a deadlock, reported instead of hanging.
#[test]
fn a_live_sender_at_top_level_is_a_deadlock() {
    let (_, err, code) = run("(tx, rx) = channel()\nfor x in rx:\n  print(x)\n;;");
    assert!(err.contains("E0702"), "{err}");
    assert_eq!(code, 1);
}

#[test]
fn close_closes_for_every_copy() {
    assert_eq!(
        run(
            "(tx, rx) = channel(2)\nt2 = tx\nsend(tx, 1)\nclose(t2)\nprint(send(tx, 2), recv(rx), recv(rx))"
        ),
        (
            "Err({kind: 'closed, message: \"channel closed\"}) Ok(1) Err({kind: 'closed, message: \"channel closed\"})\n".into(),
            "".into(),
            0
        )
    );
}

/// The last receiving end goes while a send waits on an unbuffered
/// channel: the send wakes with `closed`.
#[test]
fn dropping_the_receiver_wakes_a_blocked_sender() {
    let o = exec_waiting(
        "fn sender_only(s):\n  (tx, rx) = channel()\n  t = spawn(s, () => send(tx, 1))\n  blocked(rx, 1)\n  t\n;;\nprint(scope(s => await(sender_only(s))))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err({kind: 'closed, message: \"channel closed\"})\n", "", 0)
    );
}

#[test]
fn dropping_the_receiver_fails_later_sends() {
    assert_eq!(
        run("fn mk() = channel(1)\n(tx, _) = mk()\nprint(send(tx, 1))"),
        (
            "Err({kind: 'closed, message: \"channel closed\"})\n".into(),
            "".into(),
            0
        )
    );
}

#[test]
fn many_workers_share_a_receiver() {
    assert_eq!(
        run(
            "(jobs_tx, jobs) = channel(6)\n(out_tx, out) = channel(6)\neach(1..=6, i => send(jobs_tx, i))\nclose(jobs_tx)\nscope(s => each(0..3, _ => spawn(s, () => each(jobs, j => send(out_tx, j)))))\nclose(out_tx)\nprint(fold(out, 0, (a, x) => a + x))"
        ),
        ("21\n".into(), "".into(), 0)
    );
}

#[test]
fn sending_an_end_into_its_own_channel_is_e0705() {
    for v in ["tx", "rx", "[1, rx]", "() => tx", "(1, {k: rx})"] {
        let (_, err, code) = run(&format!("(tx, rx) = channel(1)\nsend(tx, {v})"));
        assert!(err.contains("error[E0705]"), "{v}: {err}");
        assert_eq!(code, 1, "{v}");
    }
}

#[test]
fn reply_channels_work() {
    assert_eq!(
        run(
            "(req_tx, req) = channel()\nscope(fn(s):\n  spawn(s, fn(): for (x, reply) in req:\n    send(reply, x * 2)\n  ;; ;;)\n  (reply_tx, reply) = channel(1)\n  send(req_tx, (21, reply_tx))\n  print(recv(reply))\n  close(req_tx)\n;;)"
        ),
        ("Ok(42)\n".into(), "".into(), 0)
    );
}

/// A million channels, each buffering the receiving end of the one
/// before: dropping the last frees them all without recursion.
#[test]
fn a_deep_chain_of_channels_frees_without_recursion() {
    let o = exec(
        "fn chain(n, rx):\n  (tx, r2) = channel(1)\n  send(tx, rx)\n  n == 0 then r2 else chain(n - 1, r2)\n;;\n(_t0, r0) = channel(1)\nprint(type_of(chain(1000000, r0)))\nprint(\"freed\")",
        Opts::default(),
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("'receiver\nfreed\n", "", 0)
    );
}

#[test]
fn ends_cannot_go_into_atoms() {
    for src in [
        "(tx, _rx) = channel()\natom(tx)",
        "(_tx, rx) = channel()\natom([rx])",
    ] {
        let (_, err, code) = run(src);
        assert!(err.contains("error[E0708]"), "{src}: {err}");
        assert_eq!(code, 1, "{src}");
    }
}

/// `send` and `atom` on a value sharing its parts (`[a, a]` forty levels
/// deep, nothing mutable inside) look at each part once: they end at once.
#[test]
fn shared_values_without_ends_are_sent_and_held_at_once() {
    for src in [
        "(tx, rx) = channel(1)\nx = fold(1..=40, 0, (a, i) => [a, a])\nprint(send(tx, x))",
        "x = fold(1..=40, 0, (a, i) => (a, a))\nprint(type_of(atom(x)))",
    ] {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(run(src));
        });
        let (out, err, code) = rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or_else(|_| panic!("did not end within 10s: {src}"));
        assert!(out == "Ok(nil)\n" || out == "'atom\n", "{src}: {out}{err}");
        assert_eq!(code, 0, "{src}");
    }
}

/// Iterating a sending end (a `for` over `tx`) is E0506 with a help line
/// pointing at the receiving end (`for` is `map` underneath).
#[test]
fn iterating_a_sender_says_to_use_the_receiving_end() {
    let (_, err, _) = crate::testutil::run("(tx, rx) = channel()\nfor x in tx: print(x) ;;");
    assert!(err.contains("error[E0506]"), "{err}");
    assert!(
        err.contains("a sending end (`tx`) cannot be read; go over the receiving end (`rx`)"),
        "{err}"
    );
}

/// When the merged receiving end is gone, `conc.fan_in` stops forwarding
/// and lets go of its inputs, so the producers' `send` sees
/// `Err({kind: 'closed, …})` and they end (05 "채널의 두 끝"). It used to
/// drain the inputs forever, busy, so the scope never closed. The
/// `timeout` only bounds the test.
#[test]
fn fan_in_stops_when_its_output_is_gone() {
    let (out, err, code) = run("import conc\n\
         fn ticker(s, name):\n  (tx, rx) = channel()\n  fn go(i) = match send(tx, i):\n    Ok(_) => go(i + 1)\n    Err(_) => print(\"stopped ${name}\")\n  ;;\n  spawn(s, () => go(0))\n  rx\n;;\n\
         print(timeout(10s, () => scope(fn(s):\n  out = conc.fan_in(s, [ticker(s, \"a\"), ticker(s, \"b\")])\n  len(take(out, 2))\n;;)))");
    let mut lines: Vec<&str> = out.lines().collect();
    lines.sort();
    assert_eq!(
        (lines, err.as_str(), code),
        (vec!["2", "stopped a", "stopped b"], "", 0)
    );
}

/// An input that never sends nor closes does not keep `fan_in` alive
/// once its output is gone: the other inputs' forwarders find that out
/// and stop the idle one too.
#[test]
fn fan_in_lets_an_idle_input_go_when_its_output_is_gone() {
    let (out, err, code) = run("import conc\n(_idle_tx, idle) = channel()\n\
         fn ticker(s):\n  (tx, rx) = channel()\n  fn go(i) = match send(tx, i):\n    Ok(_) => go(i + 1)\n    Err(_) => print(\"stopped\")\n  ;;\n  spawn(s, () => go(0))\n  rx\n;;\n\
         print(timeout(10s, () => scope(fn(s):\n  out = conc.fan_in(s, [ticker(s), idle])\n  len(take(out, 2))\n;;)))");
    assert_eq!((out.as_str(), err.as_str(), code), ("stopped\n2\n", "", 0));
}

/// The forwarding loop is a tail call: a long stream runs in constant
/// stack.
#[test]
fn fan_in_forwards_a_long_stream() {
    let (out, err, code) = run(
        "import conc\nprint(scope(fn(s):\n  (tx, rx) = channel()\n  spawn(s, () => each(1..=200000, i => send(tx, i)))\n  sum(conc.fan_in(s, [rx]))\n;;))",
    );
    assert_eq!((out.as_str(), err.as_str(), code), ("20000100000\n", "", 0));
}

/// Receivers that stop waiting leave the queue (`queued` counts only
/// the ones still waiting) and the rest are paired in the order they
/// began waiting (review C4: a wait that ends marks its slot instead of
/// scanning the queue). 40 receivers, 26 cancelled.
#[test]
fn receivers_left_after_cancellations_are_paired_in_order() {
    let o = exec_waiting(
        "(tx, rx) = channel()\nscope(fn(s):\n  ws = map(0..40, i => spawn(s, fn(): print(i, recv(rx)!) ;;))\n  blocked(rx, 40)\n  gone = filter(0..40, i => i % 3 != 0) |> map(i => ws[i])\n  each(gone, cancel)\n  until(() => all(gone, finished))\n  print(\"left\", queued(rx))\n  each(0..14, i => send(tx, i))\n;;)",
    );
    let want: String = std::iter::once("left 14\n".to_string())
        .chain((0..14).map(|k| format!("{} {k}\n", k * 3)))
        .collect();
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        (want.as_str(), "", 0)
    );
}

/// A send that stops waiting drops its value at once, even while
/// other sends still wait in the queue: here the value is the only
/// receiving end of another channel, which closes then (05 "채널의 두 끝").
#[test]
fn a_cancelled_send_drops_its_value_at_once() {
    let o = exec_waiting(
        "(a_tx, a) = channel()\n(out_tx, out) = channel(1)\nscope(fn(s):\n  each(0..2, i => spawn(s, () => send(a_tx, i)))\n  t = spawn(s, fn():\n    (b_tx, b) = channel(1)\n    send(out_tx, b_tx)\n    send(a_tx, b)\n  ;;)\n  b_tx = recv(out)!\n  blocked(a, 3)\n  cancel(t)\n  until(() => finished(t))\n  print(queued(a), send(b_tx, 1))\n  print(recv(a), recv(a))\n;;)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        (
            "2 Err({kind: 'closed, message: \"channel closed\"})\nOk(0) Ok(1)\n",
            "",
            0
        )
    );
}

/// L3: `try_recv` takes a value without waiting: the oldest buffered one,
/// or an empty or closed channel's Err.
#[test]
fn try_recv_looks_once() {
    let (out, err, code) = run(
        "(tx, rx) = channel(2)\nprint(try_recv(rx))\nsend(tx, 5)!\nprint(try_recv(rx))\nsend(tx, 6)!\nclose(tx)\nprint(try_recv(rx), try_recv(rx))",
    );
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        (
            "Err({kind: 'empty, message: \"channel empty\"})\nOk(5)\nOk(6) Err({kind: 'closed, message: \"channel closed\"})\n",
            "",
            0
        )
    );
}

/// On a rendezvous channel `try_recv` takes a blocked sender's value, and
/// it never parks: the task does not give up its turn.
#[test]
fn try_recv_takes_from_a_blocked_sender_without_parking() {
    let o = exec_waiting(
        "(tx, rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => send(tx, 1))\n  blocked(rx, 1)\n  n = parks()\n  print(try_recv(rx), try_recv(rx), parks() == n)\n  print(await(t))\n;;)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        (
            "Ok(1) Err({kind: 'empty, message: \"channel empty\"}) true\nOk(nil)\n",
            "",
            0
        )
    );
}

/// `try_send` hands a value over without waiting: to a waiting receiver or
/// the buffer, else a full channel's Err; a closed channel's as `send`.
#[test]
fn try_send_gives_up_on_a_full_channel() {
    let o = exec_waiting(
        "(tx, rx) = channel(1)\nprint(try_send(tx, 1), try_send(tx, 2), recv(rx))\n(rtx, rrx) = channel()\nprint(try_send(rtx, 1))\nscope(fn(s):\n  t = spawn(s, () => recv(rrx))\n  blocked(rrx, 1)\n  print(try_send(rtx, 2), await(t))\n;;)\nclose(tx)\nprint(try_send(tx, 3))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        (
            "Ok(nil) Err({kind: 'full, message: \"channel full\"}) Ok(1)\nErr({kind: 'full, message: \"channel full\"})\nOk(nil) Ok(2)\nErr({kind: 'closed, message: \"channel closed\"})\n",
            "",
            0
        )
    );
}

/// `wait_closed` waits for the close and takes nothing: what is buffered
/// stays. A cancellation or a deadline cuts the wait short.
#[test]
fn wait_closed_waits_for_the_close_only() {
    let o = exec_waiting(
        "(tx, rx) = channel(2)\nscope(fn(s):\n  w = spawn(s, () => wait_closed(rx))\n  blocked(rx, 1)\n  send(tx, 1)!\n  print(finished(w))\n  close(tx)\n  print(await(w), recv(rx))\n;;)\nprint(wait_closed(rx))\n(_tx2, rx2) = channel()\nprint(timeout(5ms, () => wait_closed(rx2)))\nscope(fn(s):\n  w = spawn(s, () => wait_closed(rx2))\n  blocked(rx2, 1)\n  cancel(w)\n  print(await(w))\n;;)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        (
            "false\nnil Ok(1)\nnil\nErr({kind: 'timeout, message: \"timed out after 5ms\"})\nErr({kind: 'cancelled, message: \"cancelled\"})\n",
            "",
            0
        )
    );
}

/// The channel closes when its last sending end goes, which wakes
/// `wait_closed` too; waiting for a close only the waiter could make is a
/// deadlock reported under its own name.
#[test]
fn wait_closed_wakes_when_the_senders_go_and_deadlocks_by_name() {
    let (out, err, code) = run(
        "scope(fn(s):\n  (tx, rx) = channel()\n  spawn(s, () => wait_closed(rx))\n  spawn(s, fn():\n    _ = tx\n    nil\n  ;;)\n;;)\nprint(\"woken\")",
    );
    assert_eq!((out.as_str(), err.as_str(), code), ("woken\n", "", 0));
    let (_, err, code) =
        run("(tx, rx) = channel()\nscope(s => spawn(s, () => wait_closed(rx)))\nprint(tx)");
    assert!(
        err.starts_with("error[E0702]: deadlock: all tasks are waiting\n"),
        "{err}"
    );
    assert!(err.contains("   wait_closed at -e:2:"), "{err}");
    assert_eq!(code, 1);
}
