//! Cleanup (concurrency v2, L3): `with(acquire, use, release)`.

use std::time::{Duration, Instant};

use crate::testutil::{Opts, boom, exec, exec_waiting, run};

const DEADLOCK: &str = "error[E0702]: deadlock: all tasks are waiting\n";

/// `run` on its own thread, failing the test if it takes longer than 10 s
/// (a hang becomes a failure; the thread is left behind).
fn run_within(src: &'static str) -> (String, String, i32) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run(src));
    });
    rx.recv_timeout(Duration::from_secs(10))
        .unwrap_or_else(|_| panic!("did not end within 10s"))
}

#[test]
fn with_gives_the_use_value_and_releases() {
    assert_eq!(
        run("print(with(() => \"r\", r => r + \"!\", r => print(\"released\", r)))"),
        ("released r\nr!\n".into(), "".into(), 0)
    );
}

#[test]
fn release_runs_after_an_err_value() {
    assert_eq!(
        run("print(with(() => 1, r => Err('x), r => print(\"released\")))"),
        ("released\nErr('x)\n".into(), "".into(), 0)
    );
}

#[test]
fn release_runs_after_a_runtime_error() {
    let (out, err, code) = run("with(() => 1, r => 1 / 0, r => print(\"released\"))");
    assert_eq!(out, "released\n");
    assert!(err.contains("E0502"), "{err}");
    assert_eq!(code, 1);
}

#[test]
fn release_runs_after_a_cancel() {
    let o = exec_waiting(
        "(_gate_tx, gate) = channel()\nscope(fn(s):\n  t = spawn(s, () => with(() => 1, r => recv(gate), r => print(\"released\")))\n  blocked(gate, 1)\n  cancel(t)\n  print(await(t))\n;;)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("released\nErr({kind: 'cancelled})\n", "", 0)
    );
}

/// An exit from one task unwinds every `with`: the entry's and the other
/// tasks'.
#[test]
fn exit_runs_every_release() {
    let o = exec_waiting(
        "(_gate_tx, gate) = channel()\nwith(() => 0, _ => scope(fn(s):\n  spawn(s, () => with(() => 1, r => recv(gate), r => print(\"released\", r)))\n  spawn(s, () => with(() => 2, r => recv(gate), r => print(\"released\", r)))\n  blocked(gate, 2)\n  spawn(s, () => exit(3))\n;;), r => print(\"released\", r))",
    );
    for r in ["released 0", "released 1", "released 2"] {
        assert!(o.out.contains(r), "{r}: {}", o.out);
    }
    assert_eq!(o.exit, 3);
}

#[test]
fn a_failing_sibling_still_lets_release_run() {
    let o = exec_waiting(
        "(_gate_tx, gate) = channel()\nscope(fn(s):\n  spawn(s, () => with(() => 1, r => recv(gate), r => print(\"released\")))\n  spawn(s, fn(): blocked(gate, 1); fail(\"boom\") ;;)\n;;)",
    );
    assert_eq!(o.out, "released\n");
    assert!(o.err.contains("boom"), "{}", o.err);
    assert_eq!(o.exit, 1);
}

#[test]
fn a_deadline_does_not_cut_release() {
    assert_eq!(
        run(
            "(_tx, never) = channel()\nprint(timeout(20ms, () => with(() => 1, r => recv(never), fn(r): sleep(50ms); print(\"released\") ;;)))"
        ),
        ("released\nErr({kind: 'timeout})\n".into(), "".into(), 0)
    );
}

#[test]
fn acquire_failing_skips_release() {
    let (out, err, _) = run("with(() => 1 / 0, r => 1, r => print(\"released\"))");
    assert_eq!(out, "");
    assert!(err.contains("E0502"), "{err}");
}

#[test]
fn a_release_error_replaces_a_value() {
    let (_, err, code) = run("with(() => 1, r => 2, r => fail(\"rel\"))");
    assert!(err.contains("error[E0803]: rel"), "{err}");
    assert_eq!(code, 1);
}

#[test]
fn a_use_error_keeps_the_release_error_as_a_note() {
    let (_, err, code) = run("with(() => 1, r => fail(\"use\"), r => fail(\"rel\"))");
    assert!(err.contains("error[E0803]: use"), "{err}");
    assert!(
        err.contains("note: also failed in release: error[E0803]: rel"),
        "{err}"
    );
    assert_eq!(code, 1);
}

#[test]
fn a_release_stuck_after_exit_is_cut_by_the_deadlock_check() {
    let start = Instant::now();
    let (_, err, code) =
        run("(_tx, never) = channel()\nwith(() => 1, r => exit(3), r => recv(never))");
    assert!(err.contains("E0702"), "{err}");
    assert_eq!(code, 3);
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn a_release_stuck_without_exit_is_a_deadlock() {
    let (_, err, code) = run("(_tx, never) = channel()\nwith(() => 1, r => 2, r => recv(never))");
    assert!(err.contains("E0702"), "{err}");
    assert_eq!(code, 1);
}

#[test]
fn with_checks_its_arguments() {
    let (_, err, _) = run("with(1, r => r, r => r)");
    assert!(err.contains("argument `acquire` must be fn"), "{err}");
    let (_, err, _) = run("with(() => 1, 2, r => r)");
    assert!(err.contains("argument `use` must be fn"), "{err}");
    let (_, err, _) = run("with(() => 1, r => r, 3)");
    assert!(err.contains("argument `release` must be fn"), "{err}");
}

#[test]
fn an_exit_in_release_ends_the_program() {
    assert_eq!(
        run("print(with(() => 1, r => 2, r => exit(5)))\nprint(\"not here\")"),
        ("".into(), "".into(), 5)
    );
}

#[test]
fn at_exit_is_gone() {
    let (_, err, _) = run("at_exit(() => 1)");
    assert!(err.contains("E0204"), "{err}");
}

// --- cleanup at the end of the program (what `at_exit` used to do) ---

/// A release stuck in a deadlock is reported once and cut short; the
/// outer release still runs, and what follows the stuck `recv` does not.
#[test]
fn a_deadlock_in_a_release_is_reported_once() {
    assert_eq!(
        run_within(
            "(_tx, never) = channel()\nwith(() => nil, _ => with(() => nil, _ => print(1), fn(_): recv(never); print(\"no\") ;;), _ => print(\"second\"))"
        ),
        ("1\nsecond\n".into(), DEADLOCK.into(), 1)
    );
}

/// A release stuck after an exit is judged on its own: reported, cut
/// short, the exit code kept, and the outer release still runs.
#[test]
fn a_stuck_release_after_an_exit_lets_the_outer_release_run() {
    assert_eq!(
        run_within(
            "(_tx, never) = channel()\nwith(() => nil, _ => with(() => nil, _ => exit(0), _ => recv(never)), _ => print(\"second\"))"
        ),
        ("second\n".into(), DEADLOCK.into(), 0)
    );
}

/// A deadlock in `use`, then a release stuck too: each is reported.
#[test]
fn a_deadlock_then_a_stuck_release_are_both_reported() {
    assert_eq!(
        run_within(
            "(_tx, never) = channel()\nwith(() => nil, _ => with(() => nil, _ => recv(never), _ => recv(never)), _ => print(\"second\"))"
        ),
        ("second\n".into(), format!("{DEADLOCK}{DEADLOCK}"), 1)
    );
}

/// The tasks a stuck release spawned are cut short with it.
#[test]
fn the_tasks_of_a_stuck_release_are_cut_with_it() {
    assert_eq!(
        run_within(
            "with(() => nil, _ => with(() => nil, _ => exit(4), fn(_): (_tx, ch) = channel(); scope(fn(s): spawn(s, () => recv(ch)); recv(ch) ;;) ;;), _ => print(\"second\"))"
        ),
        ("second\n".into(), DEADLOCK.into(), 4)
    );
}

/// A release after an exit can still spawn tasks and wait for them: its
/// shield is a root of its own, not the cancelled program's.
#[test]
fn a_release_runs_tasks_after_an_exit() {
    assert_eq!(
        run_within(
            "with(() => nil, _ => exit(2), _ => print(scope(s => await(spawn(s, () => \"h\")))))"
        ),
        ("h\n".into(), "".into(), 2)
    );
}

/// An exit inside a release ends that release (the sleeper its scope
/// spawned is cancelled at once); the outer releases still run.
#[test]
fn an_exit_in_a_release_still_runs_the_outer_releases() {
    let start = Instant::now();
    assert_eq!(
        run_within(
            "with(() => nil, _ => with(() => nil, _ => 1, _ => scope(fn(s): spawn(s, () => sleep(10s)); exit(2) ;;)), _ => print(\"second\"))"
        ),
        ("second\n".into(), "".into(), 2)
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}

/// An exit while 200 tasks wait inside `with`: every release runs.
#[test]
fn an_exit_runs_the_releases_of_many_tasks() {
    let o = exec_waiting(
        "(_gate_tx, gate) = channel()\nscope(fn(s):\n  for _ in 0..200: spawn(s, () => with(() => nil, _ => recv(gate), _ => print(\"h\"))) ;;\n  blocked(gate, 200)\n  exit(3)\n;;)",
    );
    assert_eq!((o.err.as_str(), o.exit), ("", 3));
    assert_eq!(o.out.matches("h\n").count(), 200, "{}", o.out);
}

/// A Rust panic inside a release becomes an internal error (the panic
/// fence), and the program fails.
#[test]
fn a_rust_panic_in_a_release_becomes_an_internal_error() {
    let o = exec(
        "with(() => nil, _ => 1, _ => boom())",
        Opts {
            extra: boom,
            ..Opts::default()
        },
    );
    assert_eq!(o.code, "E0801", "{}", o.err);
    assert_eq!(o.exit, 1);
}

// --- final review fixes ---

/// An exit in a release goes up from that release: nothing after the
/// inner `with` runs.
#[test]
fn code_after_an_exit_in_an_inner_release_does_not_run() {
    assert_eq!(
        run(
            "with(() => nil, _ => 1, fn(_): with(() => nil, _ => 1, _ => exit(3)); print(\"after inner exit\") ;;)"
        ),
        ("".into(), "".into(), 3)
    );
}

/// A release's failure after `exit(n)` is reported; the code stays `n`.
#[test]
fn a_release_error_after_an_exit_is_reported() {
    let (_, err, code) = run("with(() => nil, _ => exit(5), _ => 1 / 0)");
    assert!(err.contains("E0502"), "{err}");
    assert_eq!(code, 5);
}

/// After `exit(0)` a failing release makes the run fail: a run never
/// ends with 0 over a reported error.
#[test]
fn a_release_error_after_exit_0_fails_the_run() {
    let (_, err, code) = run("with(() => nil, _ => exit(0), _ => 1 / 0)");
    assert!(err.contains("E0502"), "{err}");
    assert_eq!(code, 1);
}

/// `use`'s runtime error is not hidden by an exit in its release.
#[test]
fn a_use_error_beside_an_exit_in_release_is_reported() {
    let (_, err, code) = run("with(() => nil, _ => 1 / 0, _ => exit(3))");
    assert!(err.contains("E0502"), "{err}");
    assert_eq!(code, 3);
}

/// `acquire` cancels its own task: the cancellation is seen when `use`
/// is called, right after `acquire` returned, and `release` still runs.
#[test]
fn a_cancel_before_use_still_releases() {
    let o = exec_waiting(
        "(tx, rx) = channel(1)\nscope(fn(s):\n  t = spawn(s, fn():\n    Ok(me) = recv(rx)\n    with(() => cancel(me), r => print(\"used\"), r => print(\"released\"))\n  ;;)\n  send(tx, t)\n  print(await(t))\n;;)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        ("released\nErr({kind: 'cancelled})\n", "")
    );
}

#[test]
fn exit_inside_a_callback_releases() {
    assert_eq!(
        run("with(() => 1, r => each([1], x => exit(3)), r => print(\"released\"))"),
        ("released\n".into(), "".into(), 3)
    );
}
