//! Scopes (concurrency v2, L1): `scope(f)`, `spawn(s, g)`, and what a
//! scope waits for.

use crate::testutil::{Opts, exec_waiting, on_big_stack, run, setup};

#[test]
fn scope_returns_the_body_value() {
    assert_eq!(
        run("print(scope(s => 1 + 1))"),
        ("2\n".into(), "".into(), 0)
    );
}

/// The child holds the run thread while it sleeps (`hold`), so the body
/// has long returned when it sends: the scope must still wait for it.
#[test]
fn scope_waits_for_its_children() {
    let o = exec_waiting(
        "(ch_tx, ch) = channel(1)\nscope(fn(s): spawn(s, fn(): hold(20); send(ch_tx, 'done) ;;) ;;)\nprint(recv(ch))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Ok('done)\n", "", 0)
    );
}

#[test]
fn await_inside_a_scope() {
    assert_eq!(
        run(
            "print(scope(fn(s): a = spawn(s, () => 2); b = spawn(s, () => Err('e)); [await(a), await(b)] ;;))"
        ),
        ("[2, Err('e)]\n".into(), "".into(), 0)
    );
}

#[test]
fn spawn_into_a_closed_scope_is_e0706() {
    let (out, err, code) = run("s2 = scope(s => s)\nspawn(s2, () => 1)\nprint(\"not here\")");
    assert_eq!(out, "");
    assert!(err.contains("E0706"), "{err}");
    assert!(err.contains("spawn into a closed scope"), "{err}");
    assert_eq!(code, 1);
}

/// The body returns at once; while the scope waits for its child, the
/// child spawns a grandchild into the same scope, which is still open.
#[test]
fn a_child_may_spawn_while_the_scope_drains() {
    let o = exec_waiting(
        "(ch_tx, ch) = channel(1)\nscope(fn(s): spawn(s, fn(): hold(10); spawn(s, () => send(ch_tx, 'grandchild)) ;;) ;;)\nprint(recv(ch))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Ok('grandchild)\n", "", 0)
    );
}

#[test]
fn scope_values() {
    assert_eq!(
        run("fn f(x: scope) = type_of(x)\nscope(fn(s): print(s, type_of(s), s == s, f(s)) ;;)"),
        ("<scope> 'scope true 'scope\n".into(), "".into(), 0)
    );
}

#[test]
fn a_scope_waiting_on_a_stuck_child_is_a_deadlock() {
    let (out, err, code) = run(
        "(_tx, never) = channel()\nscope(s => spawn(s, () => recv(never)))\nprint(\"not here\")",
    );
    assert_eq!(out, "");
    assert!(err.contains("E0702"), "{err}");
    assert_eq!(code, 1);
}

/// A thousand scopes whose children hold the scope leave no cancellation
/// node behind once they have closed.
#[test]
fn a_scope_captured_by_its_children_is_freed() {
    let src = "each(0..1000, _ => scope(fn(s): spawn(s, () => spawn(s, () => s)) ;;))";
    let left = on_big_stack(move || {
        let s = setup(src, Opts::default()).unwrap_or_else(|e| panic!("{e:?}"));
        s.rt.run().unwrap();
        assert_eq!(s.err.text(), "");
        s.rt.shared().root().descendant_count()
    });
    assert_eq!(left, 0, "{left} cancellation nodes left");
}

// --- Task 2: failures and cancellation ---

#[test]
fn a_child_failure_is_raised_at_the_scope() {
    let (out, err, code) = run("scope(s => spawn(s, () => 1 / 0))\nprint(\"not here\")");
    assert_eq!(out, "");
    assert!(err.contains("1 / 0"), "the child's position: {err}");
    assert!(err.contains("note: scope opened at -e:1"), "{err}");
    assert_eq!(code, 1);
}

/// The body waits for `b` when `a` fails: the body unwinds on the
/// cancellation, but what the scope raises is `a`'s error.
#[test]
fn the_child_error_wins_over_the_cancellation() {
    let o = exec_waiting(
        "(_gate_tx, gate) = channel()\nscope(fn(s): b = spawn(s, () => recv(gate)); spawn(s, fn(): blocked(gate, 1); fail(\"boom\") ;;); await(b) ;;)",
    );
    assert!(o.err.contains("error[E0803]: boom"), "{}", o.err);
    assert!(!o.err.contains("E0701"), "{}", o.err);
    assert_eq!(o.exit, 1);
}

#[test]
fn a_failure_cancels_the_siblings() {
    let (_, err, code) = run(
        "(_tx, never) = channel()\nscope(fn(s): spawn(s, () => recv(never)); spawn(s, () => 1 / 0) ;;)",
    );
    assert!(err.contains("1 / 0"), "{err}");
    assert!(!err.contains("E0702"), "not a deadlock: {err}");
    assert_eq!(code, 1);
}

#[test]
fn an_err_value_is_not_a_failure() {
    assert_eq!(
        run("print(scope(s => await(spawn(s, () => Err('x)))))"),
        ("Err('x)\n".into(), "".into(), 0)
    );
}

#[test]
fn a_body_error_cancels_and_waits_for_the_children() {
    let start = std::time::Instant::now();
    let (_, err, code) =
        run("(_tx, never) = channel()\nscope(fn(s): spawn(s, () => recv(never)); x = 1 / 0; x ;;)");
    assert!(err.contains("1 / 0"), "{err}");
    assert!(!err.contains("E0702"), "{err}");
    assert_eq!(code, 1);
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}

/// `t`, a child of the outer scope `o`, is cancelled from outside while
/// its own scope's child waits in `inner`: by the time `await(t)`
/// returns, the child has unwound and left `inner`.
#[test]
fn outer_cancel_drains_children_first() {
    let o = exec_waiting(
        "(_inner_tx, inner) = channel()\n(_gate_tx, gate) = channel()\n\
         scope(fn(o):\n  t = spawn(o, () => scope(fn(s): spawn(s, () => recv(inner)); recv(gate) ;;))\n  \
         blocked(inner, 1)\n  blocked(gate, 1)\n  cancel(t)\n  print(await(t), queued(inner))\n;;)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err({kind: 'cancelled, message: \"cancelled\"}) 0\n", "", 0)
    );
}

#[test]
fn cancel_of_the_scope_returns_err_cancelled() {
    assert_eq!(
        run(
            "(_tx, never) = channel()\nr = scope(fn(s): spawn(s, () => cancel(s)); recv(never) ;;)\nprint(r, \"after\")"
        ),
        (
            "Err({kind: 'cancelled, message: \"cancelled\"}) after\n".into(),
            "".into(),
            0
        )
    );
}

#[test]
fn cancelling_one_child_is_not_a_failure() {
    assert_eq!(
        run(
            "(_tx, never) = channel()\nscope(fn(s): t = spawn(s, () => recv(never)); cancel(t); print(await(t)) ;;)"
        ),
        (
            "Err({kind: 'cancelled, message: \"cancelled\"})\n".into(),
            "".into(),
            0
        )
    );
}

#[test]
fn exit_in_a_child_ends_the_program() {
    let (out, _, code) = run("scope(s => spawn(s, () => exit(3)))\nprint(\"not here\")");
    assert_eq!((out.as_str(), code), ("", 3));
}

/// Two genuine failures cannot be ordered from fung code (the second
/// child is cancelled at its next call), so the scope's record is
/// checked directly.
#[test]
fn two_failures_are_both_shown() {
    use super::scope::ScopeState;
    use super::task::CancelNode;
    use fung_front::diag::{Arg, error};
    let fail = |m: &str| {
        Err(crate::vm::Stop::Error(Box::new(error(
            "E0803",
            super::no_pos(),
            vec![Arg::Str(m.into())],
        ))))
    };
    let s = ScopeState::fail_fast(CancelNode::root());
    s.child_started();
    s.child_started();
    s.child_done(&fail("a"));
    s.child_done(&fail("b"));
    let Some(d) = s.take_failure("-e:1") else {
        panic!("no failure recorded")
    };
    assert_eq!(fung_front::diag::headline(&d), "error[E0803]: a");
    assert_eq!(
        d.notes,
        vec![
            "note: scope opened at -e:1".to_string(),
            "note: also failed: error[E0803]: b".to_string()
        ]
    );
}

// --- Task 3: supervise ---

#[test]
fn a_supervised_failure_goes_to_on_fail_and_the_rest_run_on() {
    let (out, err, code) = run(
        "r = supervise(fn(s):\n  a = spawn(s, () => 1)\n  b = spawn(s, () => fail(\"b\"))\n  c = spawn(s, () => 3)\n  [await(a), await(c)]\n;;, e => print(\"handled: ${e.message}\"))\nprint(r)",
    );
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("handled: b\n[1, 3]\n", "", 0)
    );
}

#[test]
fn await_sees_the_failure_too() {
    let (out, err, code) = run(
        "supervise(fn(s): t = spawn(s, () => 1 / 0); print(await(t) |> is_err) ;;, e => print(\"handled\"))",
    );
    assert!(out.contains("handled\n") && out.contains("true\n"), "{out}");
    assert_eq!((err.as_str(), code), ("", 0));
}

/// `on_fail` waits on a channel the body feeds: one handler at a time,
/// every failure handled once, and the scope ends after all three.
#[test]
fn an_on_fail_that_waits_still_handles_in_order() {
    let (out, _, code) = run(
        "(tx, rx) = channel(3)\nsupervise(fn(s):\n  each([\"a\", \"b\", \"c\"], m => spawn(s, () => fail(m)))\n  each(0..3, i => send(tx, i))\n;;, fn(e): print(\"start\"); recv(rx); print(\"end ${e.message}\") ;;)\nprint(\"done\")",
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 7, "{out}");
    for pair in lines[..6].chunks(2) {
        assert_eq!(pair[0], "start", "{out}");
        assert!(pair[1].starts_with("end "), "{out}");
    }
    assert_eq!(lines[6], "done", "{out}");
    assert_eq!(code, 0);
}

/// `on_fail` blocks (`sleep`) between its two prints: if a second
/// handler ran meanwhile, the prints would interleave.
#[test]
fn on_fail_runs_one_at_a_time_in_order() {
    let (out, _, code) = run(
        "supervise(fn(s): each([\"a\", \"b\", \"c\"], m => spawn(s, () => fail(m))) ;;, fn(e): print(\"start\"); sleep(1ms); print(\"end ${e.message}\") ;;)",
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 6, "{out}");
    for pair in lines.chunks(2) {
        assert_eq!(pair[0], "start", "{out}");
        assert!(pair[1].starts_with("end "), "{out}");
    }
    let mut ms: Vec<&str> = lines
        .iter()
        .skip(1)
        .step_by(2)
        .map(|l| &l[l.len() - 1..])
        .collect();
    ms.sort();
    assert_eq!(ms, ["a", "b", "c"], "{out}");
    assert_eq!(code, 0);
}

#[test]
fn an_on_fail_error_fails_the_supervise() {
    let (out, err, code) =
        run("supervise(s => spawn(s, () => fail(\"x\")), e => 1 / 0)\nprint(\"not here\")");
    assert_eq!(out, "");
    assert!(err.contains("1 / 0"), "{err}");
    assert_eq!(code, 1);
}

/// `a` fails first and its handler waits in `gate`; `b` fails while it
/// waits, so its failure is queued; then the scope is cancelled. `b`
/// never reached `on_fail`, so it is reported; `a` did (05 "범위": only
/// the failures the scope could not handle are reported).
#[test]
fn queued_failures_of_a_cancelled_supervise_are_reported() {
    let o = exec_waiting(
        "(_gate_tx, gate) = channel()\n(note_tx, note) = channel(1)\n\
         r = supervise(fn(s):\n  spawn(s, () => fail(\"a\"))\n  spawn(s, fn(): blocked(gate, 1); send(note_tx, 1); fail(\"b\") ;;)\n  spawn(s, fn(): recv(note); cancel(s) ;;)\n;;, e => recv(gate))\nprint(r)",
    );
    assert_eq!(o.out, "Err({kind: 'cancelled, message: \"cancelled\"})\n");
    assert_eq!(o.err.matches("task failed:").count(), 1, "{}", o.err);
    assert!(!o.err.contains("error[E0803]: a"), "{}", o.err);
    assert!(o.err.contains("error[E0803]: b"), "{}", o.err);
    assert_eq!(o.exit, 1);
}

#[test]
fn a_thousand_supervised_failures_leave_no_node() {
    let src = "each(0..1000, _ => supervise(s => spawn(s, () => 1 / 0), e => nil))";
    let left = on_big_stack(move || {
        let s = setup(src, Opts::default()).unwrap_or_else(|e| panic!("{e:?}"));
        s.rt.run().unwrap();
        assert_eq!(s.err.text(), "");
        s.rt.shared().root().descendant_count()
    });
    assert_eq!(left, 0, "{left} cancellation nodes left");
}

// --- Task 6: spawn needs a scope ---

#[test]
fn spawn_needs_a_scope() {
    let (out, err, code) = run("spawn(1, () => 1)\nprint(\"not here\")");
    assert_eq!(out, "");
    assert!(
        err.contains("error[E0403]: argument `s` must be scope"),
        "{err}"
    );
    assert_eq!(code, 1);
}

/// The old one-argument form fails the arity check before the native
/// runs.
#[test]
fn the_old_spawn_form_is_an_arity_error() {
    let (_, err, code) = run("spawn(() => 1)");
    assert!(err.contains("missing argument `f`"), "{err}");
    assert_eq!(code, 1);
}

#[test]
fn spawn_needs_a_function() {
    let (_, err, code) = run("scope(s => spawn(s))");
    assert!(err.contains("missing argument `f`"), "{err}");
    assert_ne!(code, 0);
}

// --- final review fixes ---

/// Cancelling a task that has finished does nothing, even while its
/// task still runs `on_fail` for its failure.
#[test]
fn cancelling_a_finished_supervised_task_does_not_stop_on_fail() {
    let (out, err, code) = run(
        "r = supervise(fn(s):\n  t = spawn(s, () => fail(\"a\"))\n  print(await(t))\n  cancel(t)\n;;, fn(e): sleep(20ms); print(\"handled\", e.message) ;;)\nprint(r)",
    );
    assert!(out.contains("handled a\n"), "{out}");
    assert_eq!((err.as_str(), code), ("", 0));
}

/// The failure `on_fail` was handling when the scope was cancelled has
/// reached `on_fail`: it is not reported again and the exit code stays
/// 0 (05 "범위").
#[test]
fn a_failure_whose_handler_was_cancelled_is_not_reported_again() {
    let o = exec_waiting(
        "(_gate_tx, gate) = channel()\nr = supervise(fn(s):\n  spawn(s, () => fail(\"a\"))\n  blocked(gate, 1)\n  cancel(s)\n;;, e => recv(gate))\nprint(r)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err({kind: 'cancelled, message: \"cancelled\"})\n", "", 0)
    );
}

/// A cancellation from outside the `supervise` (here an outer scope's;
/// a `timeout` is the same) while `on_fail` runs: the failure was
/// handed over, so nothing is reported.
#[test]
fn an_outer_cancel_while_on_fail_runs_reports_nothing() {
    let o = exec_waiting(
        "(_gate_tx, gate) = channel()\n(_h_tx, h) = channel()\nr = scope(fn(o):\n  spawn(o, fn(): blocked(h, 1); cancel(o) ;;)\n  supervise(fn(s):\n    spawn(s, () => fail(\"a\"))\n    recv(gate)\n  ;;, e => recv(h))\n;;)\nprint(r)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err({kind: 'cancelled, message: \"cancelled\"})\n", "", 0)
    );
}

/// `on_fail` cancels its own scope: whether a statement follows the
/// cancellation point does not change the result.
#[test]
fn cancel_inside_on_fail_reports_nothing() {
    for after in ["", "\n  print(\"not here\")"] {
        let o = exec_waiting(&format!(
            "(stx, srx) = channel(1)\n(_gate_tx, gate) = channel()\nr = supervise(fn(s):\n  send(stx, s)\n  spawn(s, () => fail(\"a\"))\n  recv(gate)\n;;, fn(e):\n  print(\"handling\")\n  cancel(recv(srx)!){after}\n;;)\nprint(r)"
        ));
        assert_eq!(
            (o.out.as_str(), o.err.as_str(), o.exit),
            (
                "handling\nErr({kind: 'cancelled, message: \"cancelled\"})\n",
                "",
                0
            ),
            "after: {after:?}"
        );
    }
}

/// `exit` inside `on_fail` ends the program with its code; the failure
/// being handled is not reported.
#[test]
fn exit_inside_on_fail_reports_nothing() {
    let o = exec_waiting(
        "(_gate_tx, gate) = channel()\nsupervise(fn(s):\n  spawn(s, () => fail(\"a\"))\n  recv(gate)\n;;, e => exit(7))",
    );
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("", "", 7));
}

/// The entry's error after a deadlock inside a scope's body is the
/// body's own, stopped at the blocked call, not a new one at `scope(`
/// (the REPL prints it with that position).
#[test]
fn a_deadlock_in_a_scope_body_keeps_its_position() {
    let o = crate::testutil::exec(
        "(_c_tx, c) = channel()\nscope(fn(s):\n  spawn(s, fn(): (_tx, rx) = channel(); recv(rx) ;;)\n  recv(c)\n;;)",
        Opts::default(),
    );
    let e = o.error.expect("an entry error");
    assert_eq!((e.code.as_str(), e.line), ("E0702", 4), "{e:?}");
    assert_eq!(o.exit, 1);
}

/// The note names the user's line, not the scope `pmap` opens inside
/// `extras/conc.fung`.
#[test]
fn the_scope_note_names_the_users_line() {
    let (_, err, _) = run("import conc\nconc.pmap([1], x => 1 / 0)");
    assert!(err.contains("note: scope opened at -e:2"), "{err}");
}

/// A body error and a child failure together: the child's is raised and
/// the body's is kept as a note.
#[test]
fn a_body_error_beside_a_child_failure_is_kept() {
    use super::scope::{ScopeState, settle};
    use super::task::CancelNode;
    use fung_front::diag::{Arg, error};
    let fail = |m: &str| {
        crate::vm::Stop::Error(Box::new(error(
            "E0803",
            super::no_pos(),
            vec![Arg::Str(m.into())],
        )))
    };
    let s = ScopeState::fail_fast(CancelNode::root());
    s.child_started();
    s.child_done(&Err(fail("child")));
    let Err(crate::vm::Stop::Error(d)) = settle(&s, Err(fail("body")), "-e:1", None) else {
        panic!("not an error")
    };
    assert_eq!(fung_front::diag::headline(&d), "error[E0803]: child");
    assert!(
        d.notes
            .contains(&"note: also failed: error[E0803]: body".to_string()),
        "{:?}",
        d.notes
    );
}

/// Decided 2026-10-18 (05): a scope cancelled after its body returned
/// still gives `Err({kind: 'cancelled, …})`, not the body's value.
#[test]
fn a_scope_cancelled_after_its_body_returned_is_cancelled() {
    assert_eq!(
        run("print(scope(fn(s): spawn(s, () => cancel(s)); 42 ;;))"),
        (
            "Err({kind: 'cancelled, message: \"cancelled\"})\n".into(),
            "".into(),
            0
        )
    );
}

/// L1: an Err a task's function returned that nobody can read any more
/// fails the scope with E0607 at the `spawn`, however it became unreadable:
/// no handle when the task ends, the last handle dropped unread after the
/// end, or a handle that left the scope unread.
#[test]
fn an_unawaited_err_fails_the_scope() {
    for (what, src) in [
        (
            "no handle",
            "scope(s => spawn(s, fn(): Err(\"x\") ;;))\nprint(\"not here\")",
        ),
        (
            "dropped unread",
            "scope(fn(s):\n  t = spawn(s, fn(): Err(\"x\") ;;)\n  until(() => finished(t))\n  hold(1)\n;;)\nprint(\"not here\")",
        ),
        (
            "left the scope",
            "t = scope(s => spawn(s, fn(): Err(\"x\") ;;))\nprint(await(t))",
        ),
    ] {
        let o = exec_waiting(src);
        let (out, err, code) = (o.out, o.err, o.exit);
        assert_eq!(out, "", "{what}");
        assert!(
            err.starts_with("error[E0607]: task's Err was never awaited: \"x\"\n"),
            "{what}: {err}"
        );
        assert!(err.contains("note: scope opened at -e:"), "{what}: {err}");
        assert_eq!(code, 1, "{what}");
    }
    // A list holding an Err is looked into, as a discarded one is (02).
    let (_, err, code) = run("scope(s => spawn(s, () => [1, Err(2)]))");
    assert!(
        err.starts_with("error[E0607]: task's Err was never awaited: 2\n"),
        "{err}"
    );
    assert!(
        err.contains("note: element 2 of the discarded list (of 2)"),
        "{err}"
    );
    assert_eq!(code, 1);
}

/// What an E0607 leaves alone: an Err that was awaited (before or after
/// the end), a cancelled task's Err (the runtime's, not the function's),
/// a task that ends in nil, and an awaited Err discarded on purpose.
#[test]
fn a_read_or_runtime_err_is_no_e0607() {
    let src = "(_tx, rx) = channel()\nscope(fn(s):\n  a = spawn(s, fn(): Err(\"a\") ;;)\n  print(await(a))\n  b = spawn(s, fn(): Err(\"b\") ;;)\n  until(() => finished(b))\n  print(await(b))\n  c = spawn(s, () => recv(rx))\n  cancel(c)\n  spawn(s, fn():\n    _ = Err(\"d\")\n    nil\n  ;;)\n  e = spawn(s, fn(): Err(\"e\") ;;)\n  _ = await(e)\n  nil\n;;)\nprint(\"done\")";
    let o = exec_waiting(src);
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err(\"a\")\nErr(\"b\")\ndone\n", "", 0)
    );
}

/// In a supervise scope an unread Err goes to `on_fail` as a failure,
/// from each of the three places.
#[test]
fn supervise_hands_an_unawaited_err_to_on_fail() {
    for src in [
        "supervise(s => spawn(s, fn(): Err(\"x\") ;;), e => print(e.code, e.message))",
        "supervise(fn(s):\n  t = spawn(s, fn(): Err(\"x\") ;;)\n  until(() => finished(t))\n  hold(1)\n;;, e => print(e.code, e.message))",
        "t = supervise(s => spawn(s, fn(): Err(\"x\") ;;), e => print(e.code, e.message))\nprint(\"after\")",
    ] {
        let o = exec_waiting(src);
        let (out, err, code) = (o.out, o.err, o.exit);
        assert!(
            out.starts_with("E0607 task's Err was never awaited: \"x\"\n"),
            "{src}: {out} {err}"
        );
        assert_eq!((err.as_str(), code), ("", 0), "{src}");
    }
}
