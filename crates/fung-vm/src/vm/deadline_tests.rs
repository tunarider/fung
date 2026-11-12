//! Deadlines (concurrency v2, L2): `timeout(f, d)` and the deadline a
//! cancellation node carries.

use std::time::{Duration, Instant};

use crate::testutil::{exec_waiting, run};

fn quick() -> impl FnOnce() {
    let start = Instant::now();
    move || {
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "took {:?}",
            start.elapsed()
        )
    }
}

#[test]
fn timeout_gives_the_value() {
    assert_eq!(
        run("print(timeout(() => 1 + 1, 1s))"),
        ("Ok(2)\n".into(), "".into(), 0)
    );
}

#[test]
fn a_wait_under_a_deadline_is_not_a_deadlock() {
    let done = quick();
    assert_eq!(
        run("(_tx, never) = channel()\nprint(timeout(() => recv(never), 50ms))"),
        (
            "Err({kind: 'timeout, message: \"timed out after 50ms\"})\n".into(),
            "".into(),
            0
        )
    );
    done();
}

#[test]
fn a_wait_outside_any_deadline_is_still_a_deadlock() {
    let done = quick();
    let (_, err, code) = run("(_tx, never) = channel()\ntimeout(() => 1, 10s)\nrecv(never)");
    assert!(err.contains("E0702"), "{err}");
    assert_eq!(code, 1);
    done();
}

#[test]
fn a_computation_is_cut_at_the_deadline() {
    let done = quick();
    assert_eq!(
        run(
            "fn spin(k) = k == 0 then 0 else spin(k - 1)\nprint(timeout(() => spin(1000000000), 50ms))"
        ),
        (
            "Err({kind: 'timeout, message: \"timed out after 50ms\"})\n".into(),
            "".into(),
            0
        )
    );
    done();
}

#[test]
fn sleep_is_cut_at_the_deadline() {
    let done = quick();
    assert_eq!(
        run("print(timeout(() => sleep(10s), 20ms))"),
        (
            "Err({kind: 'timeout, message: \"timed out after 20ms\"})\n".into(),
            "".into(),
            0
        )
    );
    done();
}

#[test]
fn a_zero_timeout_always_times_out() {
    assert_eq!(
        run("print(timeout(() => 1, 0s), timeout(() => sleep(1s), 0s))"),
        (
            "Err({kind: 'timeout, message: \"timed out after 0s\"}) Err({kind: 'timeout, message: \"timed out after 0s\"})\n".into(),
            "".into(),
            0
        )
    );
}

#[test]
fn nested_timeouts_answer_at_the_right_level() {
    let done = quick();
    assert_eq!(
        run("(_tx, never) = channel()\nprint(timeout(() => timeout(() => recv(never), 20ms), 1s))"),
        (
            "Ok(Err({kind: 'timeout, message: \"timed out after 20ms\"}))\n".into(),
            "".into(),
            0
        )
    );
    assert_eq!(
        run(
            "(_tx, never) = channel()\nprint(timeout(fn(): _r = timeout(() => recv(never), 10s); print(\"not here\") ;;, 20ms))"
        ),
        (
            "Err({kind: 'timeout, message: \"timed out after 20ms\"})\n".into(),
            "".into(),
            0
        )
    );
    done();
}

/// The child waits in `inner` under the scope's node, which hangs below
/// the timeout's: it wakes at the deadline and the scope ends with it.
#[test]
fn a_grandchild_in_a_scope_wakes_at_the_deadline() {
    let done = quick();
    let o = exec_waiting(
        "(_inner_tx, inner) = channel()\nprint(timeout(() => scope(s => spawn(s, () => recv(inner))), 50ms), queued(inner))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        (
            "Err({kind: 'timeout, message: \"timed out after 50ms\"}) 0\n",
            "",
            0
        )
    );
    done();
}

#[test]
fn errors_and_exit_pass_through() {
    let (_, err, code) = run("timeout(() => 1 / 0, 1s)");
    assert!(err.contains("E0502"), "{err}");
    assert_eq!(code, 1);
    let (_, _, code) = run("timeout(() => exit(4), 1s)");
    assert_eq!(code, 4);
}

#[test]
fn an_outer_cancel_is_not_a_timeout() {
    assert_eq!(
        run(
            "(_tx, never) = channel()\nr = scope(fn(s): t = spawn(s, () => timeout(() => recv(never), 10s)); cancel(t); await(t) ;;)\nprint(r)"
        ),
        (
            "Ok(Err({kind: 'cancelled, message: \"cancelled\"}))\n".into(),
            "".into(),
            0
        )
    );
}

#[test]
fn timeout_checks_its_arguments() {
    let (_, err, _) = run("timeout(() => 1, 1)");
    assert!(err.contains("argument `d` must be dur"), "{err}");
    let (_, err, _) = run("timeout(2, 1s)");
    assert!(err.contains("argument `f` must be fn"), "{err}");
}

#[test]
fn the_timeout_parameters_are_gone() {
    // prelude functions: caught when the file is compiled (D43)
    let (_, err, code) = run("(_tx, rx) = channel()\nrecv(rx, timeout: 1s)");
    assert!(err.contains("`timeout`"), "{err}");
    assert_eq!(code, 2);
    let (_, err, code) = run("await(scope(s => spawn(s, () => 1)), timeout: 1s)");
    assert!(err.contains("`timeout`"), "{err}");
    assert_eq!(code, 2);
}

// --- final review fixes ---

/// A deadline that passes while the task waits for the event loop on
/// something the deadline cannot cut (a child process) is enforced as
/// soon as the wait returns: nothing after it runs.
#[test]
fn a_deadline_passing_during_a_plain_wait_is_enforced_after_it() {
    let done = quick();
    assert_eq!(
        run("print(timeout(fn(): sh(\"sleep 0.2\"); print(\"not here\"); 1 ;;, 50ms))"),
        (
            "Err({kind: 'timeout, message: \"timed out after 50ms\"})\n".into(),
            "".into(),
            0
        )
    );
    done();
}

/// The outer deadline passed unnoticed (`hold` keeps the run thread without
/// looking at the clock); a zero timeout opened after it answers at the
/// outer level, whose deadline is the one in force.
#[test]
fn a_zero_timeout_inside_a_passed_outer_deadline_answers_outside() {
    let o = exec_waiting(
        "print(timeout(fn(): hold(100); r = timeout(() => 1, 0s); print(\"outer continued\", r); \"outer body value\" ;;, 20ms))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        (
            "Err({kind: 'timeout, message: \"timed out after 20ms\"})\n",
            "",
            0
        )
    );
}
