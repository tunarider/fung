//! Atoms (stage 3a, Task 5; removed in stage 3b). Ports Go
//! `internal/vm/conc_test.go` `TestAtoms` and `TestDerefAndPrintInsideSwap`,
//! plus the release and waiting cases. Expected outputs come from the Go
//! binary.

use crate::testutil::{Opts, boom, exec, run};

#[test]
fn concurrent_swaps_do_not_lose_updates() {
    let (out, _, code) = run(
        "counter = atom(0)\neach(for _ in 0..8: spawn(() => each(0..50, _ => swap(counter, n => n + 1))) ;;, await)\nprint(deref(counter))",
    );
    assert_eq!((out.as_str(), code), ("400\n", 0));
}

#[test]
fn reentrant_swap_is_e0703() {
    let (_, err, code) = run("a = atom(0)\nswap(a, x => swap(a, y => y))");
    assert!(
        err.starts_with("error[E0703]: atom swapped inside its own swap"),
        "{err}"
    );
    assert_eq!(code, 1);
}

#[test]
fn a_swap_that_sleeps_makes_the_others_wait() {
    let (out, _, code) = run(
        "a = atom(0)\nt = spawn(() => swap(a, fn(x): sleep(20ms); x + 1 ;;))\nsleep(5ms)\nswap(a, x => x * 10)\nawait(t)\nprint(deref(a))",
    );
    assert_eq!((out.as_str(), code), ("10\n", 0));
}

fn outputs(cases: &[(&str, &str)]) {
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!(
            (o.code.as_str(), o.out.as_str()),
            ("", *want),
            "{src}\n{}",
            o.err
        );
    }
}

/// Go `TestAtoms`.
#[test]
fn atoms_like_go() {
    outputs(&[
        (
            "a = atom(0)\nprint(swap(a, x => x + 1), deref(a), a)",
            "1 1 <atom 1>\n",
        ),
        (
            "a = atom(0)\nts = for _ in 0..10: spawn(() => each(0..100, _ => swap(a, x => x + 1))) ;;\neach(ts, await)\nprint(deref(a))",
            "1000\n",
        ),
        (
            "a = atom([])\nswap(a, xs => xs + [1])\nprint(deref(a), type_of(a), a == a, atom(1) == atom(1))",
            "[1] 'atom true false\n",
        ),
        // A different atom inside a swap is fine.
        (
            "a = atom(0)\nb = atom(0)\nprint(swap(a, x => swap(b, y => y + 1) + x))",
            "1\n",
        ),
    ]);
    let o = exec(
        "a = atom(0)\nswap(a, x => swap(a, y => y))",
        Opts::default(),
    );
    assert_eq!(o.code, "E0703");
    for (src, want) in [
        ("swap(1, x => x)", "E0403"),
        ("a = atom(0)\nswap(a, 1)", "E0403"),
        ("deref(1)", "E0403"),
    ] {
        assert_eq!(exec(src, Opts::default()).code, want, "{src}");
    }
    // A failing f leaves the atom unlocked and unchanged.
    let o = exec(
        "a = atom(0)\nswap(a, x => 1 / 0)\nprint(deref(a))",
        Opts::default(),
    );
    assert_eq!(o.code, "E0502");
    outputs(&[(
        "a = atom(5)\nt = spawn(() => swap(a, x => 1 / 0))\nawait(t)\nprint(swap(a, x => x + 1))",
        "6\n",
    )]);
}

/// Go `TestDerefAndPrintInsideSwap`: `deref` never waits and sees the old
/// value while a swap runs.
#[test]
fn deref_and_print_inside_swap() {
    outputs(&[
        ("a = atom(1)\nprint(swap(a, x => deref(a) + x))", "2\n"),
        (
            "a = atom(1)\nprint(swap(a, fn(x): print(a); x + 1 ;;))",
            "<atom 1>\n2\n",
        ),
        (
            "a = atom(1)\nprint(swap(a, fn(x): print(deref(a), a, \"${a}\", [a]); x + 1 ;;), deref(a), a)",
            "1 <atom 1> <atom 1> [<atom 1>]\n2 2 <atom 2>\n",
        ),
    ]);
}

#[test]
fn an_atom_prints_compares_and_types_like_go() {
    outputs(&[
        (
            "a = atom([])\nswap(a, xs => xs + [1])\nprint(deref(a), type_of(a), a == a, atom(1) == atom(1), [a], atom(\"s\"))",
            "[1] 'atom true false [<atom [1]>] <atom \"s\">\n",
        ),
        (
            "a = atom(1)\nprint(swap(a, x => nil), deref(a), swap(a, x => x))",
            "nil nil nil\n",
        ),
    ]);
    for (src, want) in [
        ("a = atom(1)\na < a", "E0501"),
        ("print(len(atom(1)))", "E0506"),
    ] {
        assert_eq!(exec(src, Opts::default()).code, want, "{src}");
    }
}

#[test]
fn the_argument_errors_read_like_go() {
    let cases = [
        (
            "swap(1, x => x)",
            "error[E0403]: argument `a` must be atom, got int",
            5,
        ),
        (
            "a = atom(0)\nswap(a, 1)",
            "error[E0403]: argument `f` must be fn, got int",
            5,
        ),
        (
            "deref(1)",
            "error[E0403]: argument `a` must be atom, got int",
            6,
        ),
        (
            "a = atom(0)\nswap(a, x => swap(a, y => y))",
            "error[E0703]: atom swapped inside its own swap",
            18,
        ),
    ];
    for (src, want, col) in cases {
        let o = exec(src, Opts::default());
        let e = o.error.unwrap_or_else(|| panic!("{src}: no error"));
        assert_eq!((e.headline.as_str(), e.col), (want, col), "{src}");
        assert_eq!(o.exit, 1, "{src}");
    }
}

/// The atom is released on every way out of `f`: a fung error, `fail`,
/// cancellation, `exit` and a Rust panic caught by the fence.
#[test]
fn every_way_out_of_f_releases_the_atom() {
    outputs(&[
        (
            "a = atom(0)\nt = spawn(() => swap(a, x => fail(\"boom\")))\nprint(await(t) |> is_err, swap(a, x => x + 1))",
            "true 1\n",
        ),
        (
            "a = atom(0)\nt = spawn(() => swap(a, fn(x): sleep(10s); x ;;))\nsleep(5ms)\ncancel(t)\nprint(await(t), swap(a, x => x + 1))",
            "Err({kind: 'cancelled}) 1\n",
        ),
    ]);
    let o = exec(
        "a = atom(0)\nat_exit(() => print(swap(a, x => x + 1)))\nswap(a, x => exit(3))",
        Opts::default(),
    );
    assert_eq!((o.out.as_str(), o.exit), ("1\n", 3));
    let o = exec(
        "a = atom(0)\nt = spawn(() => swap(a, x => boom()))\nprint(await(t) |> is_err, swap(a, x => x + 1), deref(a))",
        Opts {
            extra: boom,
            ..Opts::default()
        },
    );
    assert_eq!(
        (o.code.as_str(), o.out.as_str()),
        ("", "true 1 1\n"),
        "{}",
        o.err
    );
}

/// Waiting swappers get the atom in the order they came; one cancelled
/// while queued never gets it and its `f` never runs.
#[test]
fn waiters_are_served_in_order_and_cancelled_ones_skipped() {
    outputs(&[
        (
            "a = atom([])\nheld = channel()\nh = spawn(() => swap(a, fn(xs): send(held, nil); sleep(150ms); xs + [\"h\"] ;;))\nrecv(held)\nws = for i in 1..=4: spawn(fn(): sleep(i * 20ms); swap(a, xs => xs + [i]) ;;) ;;\neach(ws, await)\nawait(h)\nprint(deref(a))",
            "[\"h\", 1, 2, 3, 4]\n",
        ),
        (
            "a = atom(0)\nheld = channel()\nh = spawn(() => swap(a, fn(x): send(held, nil); sleep(100ms); x + 1 ;;))\nrecv(held)\nw1 = spawn(() => swap(a, fn(x): print(\"w1 ran\"); x + 100 ;;))\nsleep(20ms)\nw2 = spawn(() => swap(a, x => x * 10))\nsleep(20ms)\ncancel(w1)\nprint(await(h), await(w1), await(w2), deref(a))",
            "1 Err({kind: 'cancelled}) 10 10\n",
        ),
        // The holder cancels a queued waiter and releases in the same
        // turn, before that waiter can leave the queue: it is skipped.
        (
            "a = atom(0)\nw1 = spawn(() => swap(a, fn(x): print(\"w1 ran\"); x + 100 ;;))\nw2 = spawn(() => swap(a, x => x + 1))\nprint(swap(a, fn(x): sleep(20ms); cancel(w1); x ;;), await(w1), await(w2), deref(a))",
            "0 Err({kind: 'cancelled}) 1 1\n",
        ),
        // A waiter granted the atom and cancelled before it runs passes
        // the atom on (the fallback in `acquire`); without that the last
        // swap would wait forever.
        (
            "a = atom(0)\nw1 = spawn(() => swap(a, fn(x): print(\"w1 ran\"); x + 100 ;;))\nswap(a, fn(x): sleep(20ms); x ;;)\ncancel(w1)\nprint(await(w1), swap(a, x => x + 1))",
            "Err({kind: 'cancelled}) 1\n",
        ),
    ]);
}

/// A waiting swap is a blocking point: cancelling the waiter ends its wait
/// at once (Go waits for the lock first; see the Task 5 report).
#[test]
fn a_waiting_swap_can_be_cancelled() {
    let start = std::time::Instant::now();
    outputs(&[(
        "a = atom(0)\nheld = channel()\nt = spawn(() => swap(a, fn(x): send(held, nil); sleep(10s); x + 1 ;;))\nrecv(held)\nw = spawn(() => swap(a, x => x + 10))\nsleep(20ms)\ncancel(w)\nprint(await(w), deref(a))\ncancel(t)\nprint(await(t), swap(a, x => x + 2))",
        "Err({kind: 'cancelled}) 0\nErr({kind: 'cancelled}) 2\n",
    )]);
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}
