//! Channels (stage 3a, Task 4). Ports Go `internal/vm/conc_test.go`
//! `TestChannels`, plus the rendezvous, close and cancellation cases Go's
//! `select` decides. Expected outputs come from the Go binary.

use std::time::{Duration, Instant};

use crate::testutil::{Opts, exec, exec_waiting, run};

#[test]
fn a_native_blocked_inside_a_lambda_still_works() {
    let (out, err, code) = run(
        "ch = channel()\nspawn(fn(): each(1..=5, x => send(ch, x)); close(ch) ;;)\nprint(fold(ch, 0, (a, x) => a + x))",
    );
    assert_eq!((out.as_str(), err.as_str(), code), ("15\n", "", 0));
}

#[test]
fn closed_and_buffered_channels_like_go() {
    let (out, _, code) = run(
        "buf = channel(2)\nsend(buf, \"a\")\nclose(buf)\nprint(recv(buf), recv(buf), send(buf, \"b\"), recv(channel(), timeout: 5ms))",
    );
    assert_eq!(
        (out.as_str(), code),
        (
            "Ok(\"a\") Err({kind: 'closed}) Err({kind: 'closed}) Err({kind: 'timeout})\n",
            0
        )
    );
}

#[test]
fn cancelling_a_task_blocked_in_send_inside_each_ends_it() {
    let (out, _, code) = run(
        "ch = channel()\nt = spawn(() => each(1..=5, x => send(ch, x)))\ncancel(t)\nprint(await(t))",
    );
    assert_eq!((out.as_str(), code), ("Err({kind: 'cancelled})\n", 0));
}

/// Go `TestChannels`.
#[test]
fn channels_like_go() {
    let cases = [
        (
            "ch = channel()\nspawn(() => send(ch, 1))\nprint(recv(ch))",
            "Ok(1)\n",
        ),
        (
            "ch = channel(2)\nsend(ch, 1)\nsend(ch, 2)\nclose(ch)\nprint(recv(ch), recv(ch), recv(ch), send(ch, 3))",
            "Ok(1) Ok(2) Err({kind: 'closed}) Err({kind: 'closed})\n",
        ),
        (
            "ch = channel()\nprint(recv(ch, timeout: 10ms))",
            "Err({kind: 'timeout})\n",
        ),
        (
            "ch = channel(3)\neach(1..=3, x => send(ch, x))\nclose(ch)\nprint(for x in ch: x * 10 ;;)",
            "[10, 20, 30]\n",
        ),
        (
            "ch = channel(3)\neach([3, 1, 2], x => send(ch, x))\nclose(ch)\nprint(sort(ch), close(ch))",
            "[1, 2, 3] Ok(nil)\n",
        ),
        (
            "ch = channel(1)\nt = spawn(fn():\n  each(0..100, i => send(ch, i))\n  close(ch)\n;;)\nprint(fold(ch, 0, (a, x) => a + x), await(t))",
            "4950 Ok(nil)\n",
        ),
        (
            "print(type_of(channel()), channel() == channel())",
            "'channel false\n",
        ),
        (
            "ch = channel()\nt = spawn(() => recv(ch))\nsend(ch, 'hi)\nprint(await(t))",
            "Ok('hi)\n",
        ),
        (
            "ch = channel()\nt = spawn(() => send(ch, 1))\ncancel(t)\nprint(await(t) |> is_err)",
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
        ("recv(channel(), timeout: 1)", "E0403"),
    ];
    for (src, want) in errors {
        let o = exec(src, Opts::default());
        assert_eq!(o.code, want, "{src}");
    }
}

#[test]
fn the_argument_errors_read_like_go() {
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
            "error[E0403]: argument `ch` must be channel, got int",
        ),
        (
            "recv(channel(), timeout: 1)",
            "error[E0403]: argument `timeout` must be dur | nil, got int",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        let e = o.error.unwrap_or_else(|| panic!("{src}: no error"));
        assert_eq!(e.headline, want, "{src}");
    }
}

#[test]
fn a_channel_prints_compares_and_types_like_go() {
    let (out, _, code) =
        run("c = channel()\nprint(c, [c], type_of(c), c == c, c == channel(), c != 1)");
    assert_eq!(
        (out.as_str(), code),
        ("<channel> [<channel>] 'channel true false true\n", 0)
    );
    let cases = [
        ("channel() < channel()", "E0501"),
        ("len(channel())", "E0506"),
        ("1 in channel()", "E0512"),
        ("channel(1)[0]", "E0509"),
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
            "ch = channel()\nt = spawn(() => send(ch, 1))\nprint(await(t, timeout: 20ms))\nprint(recv(ch), await(t))",
            "Err({kind: 'timeout})\nOk(1) Ok(nil)\n",
        ),
        (
            "ch = channel(1)\nprint(send(ch, 1))\nt = spawn(() => send(ch, 2))\nprint(await(t, timeout: 20ms), recv(ch), await(t), recv(ch))",
            "Ok(nil)\nErr({kind: 'timeout}) Ok(1) Ok(nil) Ok(2)\n",
        ),
        (
            "ch = channel()\nts = for i in 0..4: spawn(() => send(ch, i)) ;;\nxs = for _ in 0..4: recv(ch)! ;;\nprint(sort(xs), map(ts, await))",
            "[0, 1, 2, 3] [Ok(nil), Ok(nil), Ok(nil), Ok(nil)]\n",
        ),
        (
            "ch = channel(2)\nspawn(fn(): each(0..1000, i => send(ch, i)); close(ch) ;;)\nprint(len(map(ch, x => x)))",
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
            "ch = channel()\nt = spawn(() => send(ch, 1))\nblocked(ch, 1)\nclose(ch)\nprint(await(t), recv(ch))",
            "Err({kind: 'closed}) Err({kind: 'closed})\n",
        ),
        (
            "ch = channel()\nt = spawn(() => recv(ch))\nblocked(ch, 1)\nclose(ch)\nprint(await(t))",
            "Err({kind: 'closed})\n",
        ),
        (
            "ch = channel(1)\nsend(ch, 1)\nt = spawn(() => send(ch, 2))\nblocked(ch, 1)\nclose(ch)\nprint(await(t), recv(ch), recv(ch))",
            "Err({kind: 'closed}) Ok(1) Err({kind: 'closed})\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec_waiting(src);
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", want), "{src}");
    }
}

/// Go's `select` in a blocked send or recv: once the other side has
/// taken (or delivered) the value, the operation succeeded, even if the
/// task is cancelled before it wakes; a cancelled wait that got nothing
/// unwinds with E0701.
#[test]
fn a_delivered_value_wins_over_a_later_cancel() {
    let cases = [
        (
            "ch = channel()\nt = spawn(() => send(ch, 1))\nr = recv(ch)\ncancel(t)\nprint(r, await(t))",
            "Ok(1) Ok(nil)\n",
        ),
        (
            "ch = channel()\nt = spawn(() => recv(ch))\ns = send(ch, 1)\ncancel(t)\nprint(s, await(t))",
            "Ok(nil) Ok(1)\n",
        ),
        (
            "ch = channel()\nt = spawn(() => recv(ch, timeout: 10s))\ns = send(ch, 1)\ncancel(t)\nprint(s, await(t))",
            "Ok(nil) Ok(1)\n",
        ),
        (
            "ch = channel()\nt = spawn(() => recv(ch))\ncancel(t)\nprint(await(t))\nt2 = spawn(() => recv(ch, timeout: 10s))\nblocked(ch, 1)\ncancel(t2)\nprint(await(t2))",
            "Err({kind: 'cancelled})\nErr({kind: 'cancelled})\n",
        ),
    ];
    // Here the task is already blocked when the other side moves the
    // value, and wakes to a cancelled flag. (These waited with a fixed 5 ms
    // sleep, and the buffered one hung under load: a sender that had not
    // started was cancelled before it sent, so the last `recv` waited
    // for a value that never came; Go reports E0702 there.)
    let blocked_first = [
        (
            "ch = channel()\nt = spawn(() => send(ch, 1))\nblocked(ch, 1)\nr = recv(ch)\ncancel(t)\nprint(r, await(t))",
            "Ok(1) Ok(nil)\n",
        ),
        (
            "ch = channel()\nt = spawn(() => recv(ch))\nblocked(ch, 1)\ns = send(ch, 1)\ncancel(t)\nprint(s, await(t))",
            "Ok(nil) Ok(1)\n",
        ),
        (
            "ch = channel()\nt = spawn(() => recv(ch, timeout: 10s))\nblocked(ch, 1)\ns = send(ch, 1)\ncancel(t)\nprint(s, await(t))",
            "Ok(nil) Ok(1)\n",
        ),
        (
            "ch = channel(1)\nsend(ch, 0)\nt = spawn(() => send(ch, 1))\nblocked(ch, 1)\nr = recv(ch)\ncancel(t)\nprint(r, await(t), recv(ch))",
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
        "ch = channel()\nt = spawn(() => send(ch, 1))\nblocked(ch, 1)\ncancel(t)\nprint(await(t), recv(ch, timeout: 20ms))",
    );
    assert_eq!(
        (o.out.as_str(), o.exit),
        ("Err({kind: 'cancelled}) Err({kind: 'timeout})\n", 0)
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}

/// Every iterating native drains a channel until it is closed (Go
/// `nat.Items`), and checks the collection before the function.
#[test]
fn iterating_natives_drain_a_channel() {
    let cases = [
        (
            "c = channel()\nclose(c)\nprint(any(c, x => true), all(c, x => false), group_by(c, x => \"a\"), sort_by(c, x => x), filter(c, x => true), fold(c, 1, (a, b) => a), each(c, x => x))",
            "false true {} [] [] 1 nil\n",
        ),
        (
            "c = channel(1)\nsend(c, [1])\nclose(c)\nprint(flatten(c))",
            "[1]\n",
        ),
        (
            "c = channel(3)\neach([\"a\", 1, nil], x => send(c, x))\nclose(c)\nprint(join(c, \",\"))",
            "a,1,nil\n",
        ),
        (
            "c = channel(3)\neach([1, 2, 3], x => send(c, x))\nclose(c)\nprint(find(c, x => x > 1), recv(c))",
            "2 Err({kind: 'closed})\n",
        ),
        (
            "c = channel(3)\neach([1, 2, 3], x => send(c, x))\nclose(c)\nprint(any(c, x => x == 1), recv(c))",
            "true Err({kind: 'closed})\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", want), "{src}");
    }
    // `map(c, 3)` drains `c` before it reports the bad function, as Go does.
    let o = exec(
        "c = channel(1)\nsend(c, 1)\nclose(c)\nmap(c, 3)",
        Opts::default(),
    );
    assert_eq!(o.code, "E0403");
}

/// A buffer of values that hold the channel itself: the values drop with
/// the token held, and dropping the runtime does not hang or panic.
/// `send` refuses these (E0705); `send_unchecked` still makes them.
#[test]
fn values_in_a_buffer_drop_with_the_runtime() {
    let o = exec(
        "import _native.conc as k\nc = channel(3)\nsend(c, [1, [2, [3]]])\nk.send_unchecked(c, c)\nk.send_unchecked(c, () => c)\nprint(recv(c))",
        Opts {
            internal: true,
            ..Opts::default()
        },
    );
    assert_eq!(
        (o.out.as_str(), o.exit),
        ("Ok([1, [2, [3]]])\n", 0),
        "{}",
        o.err
    );
}

/// A cancelled waiter is never paired: cancellation claims it at once (Go
/// `select`), even though its slot stays queued until its task next holds
/// the token. Here the token holder cancels and pairs without giving the
/// token up in between.
#[test]
fn a_cancelled_waiter_is_never_paired() {
    let cases = [
        // The wrong receiver: r2 must get the value, not the cancelled r1.
        (
            "ch = channel()\nr1 = spawn(() => recv(ch))\nblocked(ch, 1)\nr2 = spawn(() => recv(ch))\nblocked(ch, 2)\ncancel(r1)\nprint(send(ch, 1), await(r1), await(r2))",
            "Ok(nil) Err({kind: 'cancelled}) Ok(1)\n",
        ),
        // A lost value: the send must wait for the receiver that comes
        // after the cancel instead of handing the value to the cancelled one.
        (
            "ch = channel()\nr = spawn(() => recv(ch))\nblocked(ch, 1)\ncancel(r)\nr2 = spawn(() => recv(ch))\nprint(send(ch, 1), await(r), await(r2))",
            "Ok(nil) Err({kind: 'cancelled}) Ok(1)\n",
        ),
        // A cancelled sender delivers nothing.
        (
            "ch = channel()\nt = spawn(() => send(ch, 1))\nblocked(ch, 1)\ncancel(t)\nprint(recv(ch, timeout: 20ms), await(t))",
            "Err({kind: 'timeout}) Err({kind: 'cancelled})\n",
        ),
        (
            "ch = channel()\ns1 = spawn(() => send(ch, 1))\nblocked(ch, 1)\ns2 = spawn(() => send(ch, 2))\nblocked(ch, 2)\ncancel(s1)\nprint(recv(ch), await(s1), await(s2))",
            "Ok(2) Err({kind: 'cancelled}) Ok(nil)\n",
        ),
        // The same with a full buffer: the cancelled sender's value does
        // not move into the freed place.
        (
            "ch = channel(1)\nsend(ch, 0)\nt = spawn(() => send(ch, 1))\nblocked(ch, 1)\ncancel(t)\nprint(recv(ch), recv(ch, timeout: 20ms), await(t))",
            "Ok(0) Err({kind: 'timeout}) Err({kind: 'cancelled})\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec_waiting(src);
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", want), "{src}");
    }
}

/// A receiver whose deadline has passed is never paired, even before its
/// task wakes to see the timeout: `hold(ms)` keeps the token past the
/// deadline, then the send must go to the second receiver.
#[test]
fn a_timed_out_receiver_is_never_paired() {
    // r is blocked (or already timed out, under heavy load) before r2
    // exists, so only r can be the first queued receiver.
    let o = exec_waiting(
        "ch = channel()\nr = spawn(() => recv(ch, timeout: 30ms))\nuntil(() => queued(ch) >= 1 or await(r, timeout: 0s) != Err({kind: 'timeout}))\nr2 = spawn(() => recv(ch))\nhold(60)\nprint(send(ch, 1), await(r), await(r2))",
    );
    assert_eq!(
        (o.code.as_str(), o.out.as_str()),
        ("", "Ok(nil) Err({kind: 'timeout}) Ok(1)\n")
    );
}

/// The interleaving that hung `a_delivered_value_wins_over_a_later_cancel`
/// under load, made deterministic: the entry keeps the run token from the
/// spawn to the cancel, so the sender is cancelled before it ever runs
/// and sends nothing. A last `recv` without a timeout would wait forever
/// (Go reports E0702 there); with one it times out.
#[test]
fn a_sender_cancelled_before_it_runs_sends_nothing() {
    let (out, _, code) = run(
        "ch = channel(1)\nsend(ch, 0)\nt = spawn(() => send(ch, 1))\nr = recv(ch)\ncancel(t)\nprint(r, await(t), recv(ch, timeout: 50ms))",
    );
    assert_eq!(
        (out.as_str(), code),
        ("Ok(0) Err({kind: 'cancelled}) Err({kind: 'timeout})\n", 0)
    );
}

/// Stage 3b Task 5: a channel whose buffer held a value containing the
/// channel itself would never be freed (`Rc`), so `send` refuses it.
#[test]
fn a_channel_inside_what_is_sent_is_e0705() {
    for src in [
        "ch = channel(1)\nsend(ch, ch)",
        "ch = channel(1)\nsend(ch, [1, {k: (2, ch)}])",
        "ch = channel(1)\nf = () => ch\nsend(ch, f)",
        "ch = channel(1)\nsend(ch, Ok([(1..40) |> map(_ => [ch])]))",
        "ch = channel(1)\nm = fold(0..20, {}, (a, i) => put(a, \"k${i}\", i))\nsend(ch, put(m, \"z\", ch))",
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
    let o = exec("ch = channel(1)\nx = 1\n  send(ch, [ch])", Opts::default());
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
        "a = channel(1)\nb = channel(1)\nsend(a, [b])\nfn nest(x, n) = n == 0 then x else nest([x], n - 1)\nsend(b, nest(1, 1000000))\nprint(\"ok\")",
        Opts::default(),
    );
    assert_eq!((o.out.as_str(), o.exit), ("ok\n", 0), "{}", o.err);
}

#[test]
fn shared_sub_values_are_scanned_once() {
    // 2^40 paths through 40 levels of [x, x]: walking each path would
    // never end, walking each node once is instant.
    let o = exec(
        "ch = channel(1)\nfn dup(x, n) = n == 0 then x else dup([x, x], n - 1)\nsend(ch, dup(1, 40))\nprint(\"ok\")\nc2 = channel(1)\nsend(c2, dup(c2, 40))",
        Opts::default(),
    );
    assert_eq!(o.out, "ok\n", "{}", o.err);
    assert!(o.err.starts_with("error[E0705]"), "{}", o.err);
}

#[test]
fn an_agent_handle_over_the_channel_is_e0705() {
    let o = exec(
        "import _native.conc as c\nch = channel(1)\na = c.agent_handle(ch)\nsend(ch, [a])",
        Opts {
            internal: true,
            ..Opts::default()
        },
    );
    assert!(o.err.starts_with("error[E0705]"), "{}", o.err);
}

#[test]
fn send_unchecked_skips_the_e0705_check() {
    let o = exec(
        "import _native.conc as c\nch = channel(1)\nprint(c.send_unchecked(ch, [ch]), type_of(recv(ch)))",
        Opts {
            internal: true,
            ..Opts::default()
        },
    );
    assert_eq!((o.out.as_str(), o.exit), ("Ok(nil) 'Ok\n", 0), "{}", o.err);
}
