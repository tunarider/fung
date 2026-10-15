//! Unretrieved task failures and deadlock detection (stage 3a, Task 6).
//! Ports Go `internal/vm/unretrieved_test.go` (all of it) and
//! `conc_test.go` `TestUnawaitedFailureIsReportedOnce`,
//! `TestAwaitOnSelfIsADeadlock`, `TestDeadlockIsDetected`,
//! `TestDeadlockAfterTheLastRunnerFinishes` and
//! `TestAwaitTimeoutDoesNotHideALaterFailure`. Expected texts come from
//! the Go binary (`fung -e`, so the file is `-e`).
//!
//! Go reports a dropped handle's failure when the garbage collector finds
//! the handle; Rust reports it when the last handle drops (or, if the task
//! is still running then, when it fails). Tests that need a task in a
//! given state wait for that state (`testutil` probes), not for a time.

use std::time::{Duration, Instant};

use crate::testutil::{Opts, Outcome, exec, exec_waiting, on_big_stack, probes, run, setup};
use crate::value::{Native, Value};
use crate::vm::{Stop, Task};

/// Runs `f` on its own thread and fails the test if it takes longer than
/// `secs` (a hang becomes a failure; the thread is left behind).
fn within<R: Send + 'static>(secs: u64, f: impl FnOnce() -> R + Send + 'static) -> R {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs))
        .unwrap_or_else(|_| panic!("did not end within {secs}s"))
}

fn run_within(src: &'static str) -> (String, String, i32) {
    within(10, move || run(src))
}

const DEADLOCK: &str = "error[E0702]: deadlock: all tasks are waiting\n";

// --- the brief's tests ---

#[test]
fn a_dropped_failed_task_is_reported_once_and_fails_the_run() {
    let (out, err, code) = run("spawn(() => fail(\"x\"))\nsleep(20ms)\nprint(\"entry done\")");
    assert_eq!(out, "entry done\n");
    assert_eq!(err.matches("error[E0803]").count(), 1, "{err}");
    assert_eq!(code, 1);
    // The whole text, as the Go binary prints it.
    assert_eq!(
        err,
        "task failed: error[E0803]: x\n  --> -e:1:17\n  |\n1 | spawn(() => fail(\"x\"))\n  |                 ^\n  |\n call path (innermost first):\n   -e:1       <lambda>\n"
    );
}

#[test]
fn awaited_and_helper_failures_are_not_reported() {
    let (out, err, code) = run(
        "t = spawn(() => fail(\"x\"))\nr = pmap([1], x => 1 / 0)\nprint(await(t) |> is_err, type_of(r))",
    );
    assert_eq!((out.as_str(), err.as_str(), code), ("true 'list\n", "", 0));
}

#[test]
fn a_channel_nobody_sends_on_is_a_deadlock() {
    let start = std::time::Instant::now();
    let (out, err, code) = run_within("at_exit(() => print(\"bye\"))\nch = channel()\nrecv(ch)");
    assert!(
        err.starts_with("error[E0702]: deadlock: all tasks are waiting"),
        "{err}"
    );
    assert_eq!(code, 1);
    assert!(start.elapsed() < std::time::Duration::from_millis(500));
    // Go: the report, then the handler runs; nothing else.
    assert_eq!((out.as_str(), err.as_str()), ("bye\n", DEADLOCK));
}

// --- Go unretrieved_test.go ---

/// Go `TestAwaitAfterTheTaskDiedIsQuiet`.
#[test]
fn an_await_after_the_task_died_is_quiet() {
    let (out, err, code) =
        run("t = spawn(() => fail(\"boom\"))\nsleep(20ms)\nprint(await(t) |> is_err)");
    assert_eq!((out.as_str(), err.as_str(), code), ("true\n", "", 0));
}

/// Go `TestHeldFailureIsReportedAtTheEnd`: a handle still held can be
/// awaited later, so its failure waits for the end of the program, and
/// the sweep reports it once however often it runs.
#[test]
fn a_held_failure_is_reported_at_the_end() {
    on_big_stack(|| {
        let s = setup(
            "t = spawn(() => fail(\"boom\"))\nprint(\"after\")",
            Opts::default(),
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
        s.rt.run().expect("the entry runs");
        s.rt.wait();
        assert_eq!(s.err.text(), "", "reported while the handle is held");
        assert!(!s.rt.failed());
        s.rt.report_unretrieved();
        s.rt.report_unretrieved();
        assert_eq!(s.out.text(), "after\n");
        let err = s.err.text();
        assert_eq!(
            err,
            "task failed: error[E0803]: boom\n  --> -e:1:21\n  |\n1 | t = spawn(() => fail(\"boom\"))\n  |                     ^\n  |\n call path (innermost first):\n   -e:1       <lambda>\n"
        );
        assert!(s.rt.failed());
    });
}

/// `reported()`: whether a failure was reported so far.
fn reported() -> Vec<Value> {
    let mut v = probes();
    v.push(Value::Native(Native::leak(
        "reported",
        vec![],
        |t: &mut Task, _| Ok(Value::Bool(t.shared.failures.failed())),
    )));
    v
}

/// Go `TestDroppedHandleIsReportedByTheGC`, with its intent in Rust: a
/// dropped handle can never be awaited, so its failure is reported while
/// the program still runs: here when the handle's last reference (a
/// local of `kick`, deeper than the entry's later calls reach) goes,
/// or when the task fails if that comes later. The entry waits for the
/// report and then goes on.
#[test]
fn a_dropped_handle_is_reported_when_it_drops() {
    let o = within(10, || {
        exec(
            &format!(
                "{}fn kick():\n  a = 1\n  b = 2\n  t = spawn(() => fail(\"boom\"))\n  a\n;;\nkick()\nuntil(reported)\nprint(\"seen\")",
                crate::testutil::UNTIL
            ),
            Opts {
                extra: reported,
                ..Opts::default()
            },
        )
    });
    assert_eq!(o.out, "seen\n");
    assert_eq!(o.err.matches("task failed").count(), 1, "{}", o.err);
    assert!(
        o.err.contains("task failed: error[E0803]: boom\n"),
        "{}",
        o.err
    );
    assert_eq!(o.exit, 1);
}

/// `finished(t)`: whether the task has a result, without retrieving it.
fn finished() -> Vec<Value> {
    let mut v = reported();
    v.push(Value::Native(Native::leak(
        "finished",
        vec![crate::native::nat::param("t")],
        |_: &mut Task, a| {
            let Value::Task(h) = &a[0] else {
                panic!("a task argument")
            };
            Ok(Value::Bool(h.state.result().is_some()))
        },
    )));
    v
}

/// The other order: the task fails while its handle is held (nothing is
/// reported), and the report comes when the handle drops.
#[test]
fn a_handle_dropped_after_the_failure_is_reported_then() {
    let o = within(10, || {
        exec(
            &format!(
                "{}fn kick():\n  t = spawn(() => fail(\"boom\"))\n  until(() => finished(t))\n  print(reported())\n;;\nkick()\nprint(reported())",
                crate::testutil::UNTIL
            ),
            Opts {
                extra: finished,
                ..Opts::default()
            },
        )
    });
    assert_eq!(o.out, "false\ntrue\n", "{}", o.err);
    assert_eq!(o.err.matches("task failed").count(), 1, "{}", o.err);
    assert_eq!(o.exit, 1);
}

/// Go `TestTimedOutAwaitDoesNotRetrieve` (and the rest of
/// `TestAwaitTimeoutDoesNotHideALaterFailure`): a timed-out await does not
/// retrieve the result, so the later failure is reported.
#[test]
fn a_timed_out_await_does_not_retrieve() {
    let o = within(10, || {
        exec(
            "go = channel()\nt = spawn(fn(): recv(go); fail(\"late\") ;;)\nprint(await(t, timeout: 1ms))\nsend(go, nil)",
            Opts::default(),
        )
    });
    assert_eq!(o.out, "Err({kind: 'timeout})\n");
    assert!(
        o.err.starts_with("task failed: error[E0803]: late\n"),
        "{}",
        o.err
    );
    assert_eq!(o.exit, 1);
}

/// Go `TestHelpersThatAwaitNeverReport`.
#[test]
fn helpers_that_await_never_report() {
    for _ in 0..20 {
        let (out, err, code) =
            run("r = pmap([1, 2, 3], x => fail(\"boom\"))\nprint(r |> map(is_err))");
        assert_eq!(
            (out.as_str(), err.as_str(), code),
            ("[true, true, true]\n", "", 0)
        );
    }
}

// --- Go conc_test.go ---

/// Go `TestUnawaitedFailureIsReportedOnce`.
#[test]
fn an_unawaited_failure_is_reported_once() {
    let (out, err, code) = run("spawn(() => fail(\"boom\"))\nsleep(20ms)\nprint(\"after\")");
    assert_eq!(out, "after\n");
    assert!(
        err.starts_with("task failed: error[E0803]: boom\n  --> -e:1:"),
        "{err}"
    );
    assert_eq!(err.matches("task failed").count(), 1, "{err}");
    assert_eq!(code, 1);

    let (out, err, code) = run("t = spawn(() => fail(\"boom\"))\nprint(await(t) |> is_err)");
    assert_eq!((out.as_str(), err.as_str(), code), ("true\n", "", 0));

    // A cancelled task is not a failure.
    let (_, err, code) = run("t = spawn(() => sleep(10s))\ncancel(t)\nsleep(10ms)");
    assert_eq!((err.as_str(), code), ("", 0));
}

/// Go `TestAwaitOnSelfIsADeadlock`: a sleeper keeps the others alive, so
/// there is no deadlock until the program is cancelled from outside.
#[test]
fn a_sleeper_keeps_a_waiting_chain_alive() {
    let (err, code) = on_big_stack(|| {
        let s = setup(
            "t = spawn(fn(): sleep(10s) ;;)\nu = spawn(() => await(t))\nawait(u)\n",
            Opts::default(),
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
        let shared = s.rt.shared();
        // Cancel only once the chain is in place: the sleeper, `u` and
        // the entry all blocked (the sleeper with its timer).
        let canceller = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(10);
            while shared.census.blocked() < 3 && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(shared.census.blocked(), 3, "the chain never blocked");
            shared.cancel_all();
        });
        let code = match s.rt.run() {
            Ok(()) => String::new(),
            Err(Stop::Error(d)) => d.code.clone(),
            Err(Stop::Exit(_)) => "exit".into(),
        };
        s.rt.wait();
        canceller.join().unwrap();
        (s.err.text(), code)
    });
    assert!(!err.contains("deadlock"), "false deadlock: {err}");
    assert!(code.is_empty() || code == "E0701", "{code}");
}

/// Go `TestDeadlockIsDetected`.
#[test]
fn a_deadlock_is_detected() {
    let o = within(10, || exec("ch = channel()\nrecv(ch)", Opts::default()));
    assert_eq!(
        (o.code.as_str(), o.err.as_str(), o.exit),
        ("E0702", DEADLOCK, 1)
    );
    let o = within(10, || {
        exec(
            "ch = channel()\nt = spawn(() => recv(ch))\nawait(t)",
            Opts::default(),
        )
    });
    assert!(o.code == "E0702" || o.code == "E0701", "{}", o.code);
    assert_eq!((o.err.as_str(), o.exit), (DEADLOCK, 1));
    // A sleeper keeps the program alive: a recv with a timeout returns
    // instead of a false deadlock, and an unbuffered send meets its
    // receiver.
    let (out, err, code) = run_within(
        "t = spawn(() => sleep(30ms))\nch = channel()\nprint(recv(ch, timeout: 10ms))\nawait(t)",
    );
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("Err({kind: 'timeout})\n", "", 0)
    );
    let (out, err, code) =
        run_within("ch = channel()\nspawn(fn(): sleep(10ms); send(ch, 1) ;;)\nprint(recv(ch))");
    assert_eq!((out.as_str(), err.as_str(), code), ("Ok(1)\n", "", 0));
}

/// Go `TestDeadlockAfterTheLastRunnerFinishes`.
#[test]
fn a_deadlock_after_the_last_runner_finishes() {
    // The sender finishes without sending: the receiver is left alone.
    let o = within(10, || {
        exec(
            "ch = channel()\nt = spawn(() => 1)\nprint(recv(ch))",
            Opts::default(),
        )
    });
    assert_eq!(
        (o.code.as_str(), o.err.as_str(), o.exit),
        ("E0702", DEADLOCK, 1)
    );
    // An orphan blocked after the entry is done: `wait` returns because
    // the deadlock report cancels it.
    let (out, err, exit) = within(3, || {
        on_big_stack(|| {
            let s = setup(
                "spawn(() => recv(channel()))\nprint(\"entry done\")",
                Opts::default(),
            )
            .unwrap_or_else(|e| panic!("{e:?}"));
            s.rt.run().expect("the entry runs");
            s.rt.wait();
            (s.out.text(), s.err.text(), s.rt.exit_code())
        })
    });
    assert_eq!(
        (out.as_str(), err.as_str(), exit),
        ("entry done\n", DEADLOCK, Some(1))
    );
    // A test body counts as live: awaiting a worker is no deadlock.
    let ok = within(10, || {
        on_big_stack(|| {
            let s = setup(
                "fn spin(n) = n == 0 then 0 else spin(n - 1)\ntest \"waits for a worker\":\n  t = spawn(() => spin(300000))\n  assert await(t) == 0\n;;",
                Opts {
                    tests: true,
                    ..Opts::default()
                },
            )
            .unwrap_or_else(|e| panic!("{e:?}"));
            s.rt.run().expect("the body runs");
            let r = s.rt.call_test(0);
            (r.is_ok(), s.err.text())
        })
    });
    assert_eq!(ok, (true, String::new()));
}

// --- more cases, checked against the Go binary ---

/// An exit handler counts as live, and a deadlock inside one is reported
/// and ends it (its E0702 is not reported again).
#[test]
fn a_deadlock_inside_an_exit_handler() {
    let (out, err, code) = run_within("at_exit(fn(): recv(channel()); print(\"no\") ;;)\nprint(1)");
    assert_eq!((out.as_str(), err.as_str(), code), ("1\n", DEADLOCK, 1));
}

/// The count is exact: a task that was woken but has not run yet is not
/// blocked. Here the entry wakes `t` and ends while still holding the run
/// token, so `t` has not run when the entry's end is judged.
#[test]
fn a_woken_task_that_has_not_run_is_not_blocked() {
    let o = within(10, || {
        exec_waiting("ch = channel()\nt = spawn(() => recv(ch))\nblocked(ch, 1)\nsend(ch, 1)")
    });
    assert_eq!((o.err.as_str(), o.exit), ("", 0));
    let o = within(10, || {
        exec_waiting(
            "ch = channel()\nt = spawn(() => recv(ch))\nblocked(ch, 1)\ncancel(t)\nprint(await(t))",
        )
    });
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err({kind: 'cancelled})\n", "", 0)
    );
}

/// A wait outside the token (standard input, a child process) is not a
/// blocked task: the entry waits for a value the task sends after it.
#[test]
fn a_wait_outside_the_token_is_not_blocked() {
    fn natives() -> Vec<Value> {
        vec![Value::Native(Native::leak(
            "os_sleep",
            vec![],
            |t: &mut Task, _| {
                super::block::outside(t, || std::thread::sleep(Duration::from_millis(30)));
                Ok(Value::Nil)
            },
        ))]
    }
    let o: Outcome = within(10, || {
        exec(
            "ch = channel()\nspawn(fn(): os_sleep(); send(ch, 1) ;;)\nprint(recv(ch))",
            Opts {
                extra: natives,
                ..Opts::default()
            },
        )
    });
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("Ok(1)\n", "", 0));
}

/// Every task waits on another: reported at once, without a grace period.
#[test]
fn a_mutual_wait_is_reported_at_once() {
    let start = Instant::now();
    let (_, err, code) = run_within(
        "a = channel()\nb = channel()\nt = spawn(fn(): recv(a); send(b, 1) ;;)\nrecv(b)\nsend(a, 1)",
    );
    assert_eq!((err.as_str(), code), (DEADLOCK, 1));
    assert!(start.elapsed() < Duration::from_millis(500));
}

/// A long chain of task results, each holding the next handle, drops in
/// constant stack (`value/release.rs`).
#[test]
fn a_long_chain_of_task_results_drops_without_recursion() {
    use super::task::{CancelNode, TaskHandle};
    let rt = on_big_stack(|| {
        setup("nil", Opts::default())
            .unwrap_or_else(|e| panic!("{e:?}"))
            .rt
    });
    std::thread::Builder::new()
        .stack_size(256 << 10)
        .spawn(move || {
            let failures = rt.shared().failures.clone();
            let t = rt.lock();
            let mut v = Value::Nil;
            for _ in 0..200_000 {
                v = TaskHandle::finished(v, CancelNode::root(), failures.clone());
            }
            drop(v);
            drop(t);
        })
        .unwrap()
        .join()
        .unwrap();
}

// --- fix round 1 ---

/// An entry error while another task is blocked is the entry's error, not
/// a deadlock: the entry's end must not be judged before the program is
/// cancelled (Go `execute` cancels right after an entry error). Expected
/// text from the Go binary.
#[test]
fn an_entry_error_beside_a_blocked_task_is_not_a_deadlock() {
    let (out, err, code) =
        run_within("spawn(() => recv(channel()))\nsleep(5ms)\nfail(\"real error\")");
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        (
            "",
            "error[E0803]: real error\n  --> -e:3:5\n  |\n3 | fail(\"real error\")\n  |     ^\n  |\n call path (innermost first):\n   -e:3       <entry>\n",
            1
        )
    );
    // The same for exit: the exit code, no deadlock report.
    let (_, err, code) = run_within("spawn(() => recv(channel()))\nsleep(5ms)\nexit(3)");
    assert_eq!((err.as_str(), code), ("", 3));
}

/// The variant with a task still running when the entry fails: it is
/// queued for the token (its 1 ms sleep ran out during `hold`), so it
/// gets the token right after the entry, before the command can cancel
/// anything, and then blocks for good.
#[test]
fn an_entry_error_beside_a_task_about_to_block_is_not_a_deadlock() {
    let o = within(10, || {
        exec_waiting(
            "go = channel(1)\nready = channel(1)\nspawn(fn(): send(ready, nil); until(() => is_ok(recv(go, timeout: 0s))); recv(channel()) ;;)\nrecv(ready)\nsend(go, true)\nhold(5)\nfail(\"real error\")",
        )
    });
    assert!(o.err.starts_with("error[E0803]: real error\n"), "{}", o.err);
    assert!(!o.err.contains("E0702"), "{}", o.err);
    assert_eq!(o.exit, 1);
}

/// A deadlock inside an exit handler is judged on its own, even after an
/// exit or a deadlock before the handlers: it is reported, the handler is
/// cut short, and the exit code stays what it was. (Go's own runtime
/// detector kills the process with exit 2 here; see open-questions.)
#[test]
fn a_deadlock_in_a_handler_after_an_exit_or_a_deadlock() {
    let (out, err, code) =
        run_within("at_exit(() => print(\"second\"))\nat_exit(() => recv(channel()))\nexit(0)");
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("second\n", DEADLOCK, 0)
    );
    let (out, err, code) = run_within(
        "at_exit(() => print(\"second\"))\nat_exit(() => recv(channel()))\nrecv(channel())",
    );
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        (
            "second\n",
            "error[E0702]: deadlock: all tasks are waiting\nerror[E0702]: deadlock: all tasks are waiting\n",
            1
        )
    );
    // Its tasks are cut short with it.
    let (out, err, code) = run_within(
        "at_exit(() => print(\"second\"))\nat_exit(fn(): ch = channel(); spawn(() => recv(ch)); recv(ch) ;;)\nexit(4)",
    );
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("second\n", DEADLOCK, 4)
    );
}
