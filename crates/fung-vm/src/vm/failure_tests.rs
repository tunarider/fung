//! Deadlock detection (stage 3a, Task 6; tasks in scopes since
//! concurrency v2 L1). Expected texts are what `fung -e` prints (so the
//! file is `-e`).
//!
//! Tests that need a task in a given state wait for that state
//! (`testutil` probes), not for a time.

use std::time::{Duration, Instant};

use crate::testutil::{Opts, Outcome, exec, exec_waiting, on_big_stack, report_heads, run, setup};
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

#[test]
fn a_channel_nobody_sends_on_is_a_deadlock() {
    let start = std::time::Instant::now();
    let (out, err, code) =
        run_within("(_tx, ch) = channel()\nwith(() => nil, _ => recv(ch), _ => print(\"bye\"))");
    assert!(
        err.starts_with("error[E0702]: deadlock: all tasks are waiting"),
        "{err}"
    );
    assert_eq!(code, 1);
    assert!(start.elapsed() < std::time::Duration::from_millis(500));
    // The report, then the release runs; nothing else.
    assert_eq!(
        (out.as_str(), report_heads(&err).as_str()),
        ("bye\n", DEADLOCK)
    );
}

// --- failures and deadlocks ---

/// A sleeper keeps the others alive, so there is no deadlock until the
/// program is cancelled from outside.
#[test]
fn a_sleeper_keeps_a_waiting_chain_alive() {
    let (err, code) = on_big_stack(|| {
        let s = setup(
            "scope(fn(s):\n  t = spawn(s, fn(): sleep(10s) ;;)\n  u = spawn(s, () => await(t))\n  await(u)\n;;)\n",
            Opts::default(),
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
        let shared = s.rt.shared();
        // Cancel only once the chain is in place: the sleeper, `u` and
        // the entry all blocked (the sleeper with its timer).
        let canceller = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(10);
            while shared
                .parked_probe
                .load(std::sync::atomic::Ordering::SeqCst)
                < 3
                && Instant::now() < until
            {
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(
                shared
                    .parked_probe
                    .load(std::sync::atomic::Ordering::SeqCst),
                3,
                "the chain never parked"
            );
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

#[test]
fn a_deadlock_is_detected() {
    let o = within(10, || {
        exec("(_tx, ch) = channel()\nrecv(ch)", Opts::default())
    });
    assert_eq!(
        (o.code.as_str(), report_heads(&o.err).as_str(), o.exit),
        ("E0702", DEADLOCK, 1)
    );
    let o = within(10, || {
        exec(
            "(_tx, ch) = channel()\nscope(fn(s):\n  t = spawn(s, () => recv(ch))\n  await(t)\n;;)",
            Opts::default(),
        )
    });
    assert!(o.code == "E0702" || o.code == "E0701", "{}", o.code);
    assert_eq!((report_heads(&o.err).as_str(), o.exit), (DEADLOCK, 1));
    // A sleeper keeps the program alive: a recv with a timeout returns
    // instead of a false deadlock, and an unbuffered send meets its
    // receiver.
    let (out, err, code) = run_within(
        "(_tx, ch) = channel()\nscope(fn(s):\n  t = spawn(s, () => sleep(30ms))\n  print(timeout(10ms, () => recv(ch)))\n  await(t)\n;;)",
    );
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        (
            "Err({kind: 'timeout, message: \"timed out after 10ms\"})\n",
            "",
            0
        )
    );
    let (out, err, code) = run_within(
        "(tx, ch) = channel()\nscope(fn(s):\n  spawn(s, fn(): sleep(10ms); send(tx, 1) ;;)\n  print(recv(ch))\n;;)",
    );
    assert_eq!((out.as_str(), err.as_str(), code), ("Ok(1)\n", "", 0));
}

#[test]
fn a_deadlock_after_the_last_runner_finishes() {
    // The sender finishes without sending: the receiver is left alone.
    let o = within(10, || {
        exec(
            "(_tx, ch) = channel()\nscope(fn(s):\n  t = spawn(s, () => 1)\n  print(recv(ch))\n;;)",
            Opts::default(),
        )
    });
    assert_eq!(
        (o.code.as_str(), report_heads(&o.err).as_str(), o.exit),
        ("E0702", DEADLOCK, 1)
    );
    // A body that is done while its child is blocked for good: the scope
    // waits on the child, and the report cancels both.
    let o = within(10, || {
        exec(
            "(_tx, never) = channel()\nscope(s => spawn(s, () => recv(never)))\nprint(\"entry done\")",
            Opts::default(),
        )
    });
    assert_eq!(
        (o.out.as_str(), report_heads(&o.err).as_str(), o.exit),
        ("", DEADLOCK, 1)
    );
    // A test body counts as live: awaiting a worker is no deadlock.
    let ok = within(10, || {
        on_big_stack(|| {
            let s = setup(
                "fn spin(n) = n == 0 then 0 else spin(n - 1)\ntest \"waits for a worker\":\n  assert scope(s => await(spawn(s, () => spin(300000)))) == 0\n;;",
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

// --- more cases ---

/// The count is exact: a task that was woken but has not run yet is not
/// blocked. Here the scope's body wakes `t` and ends while still holding
/// the run thread, so `t` has not run when the scope starts to wait for it.
#[test]
fn a_woken_task_that_has_not_run_is_not_blocked() {
    let o = within(10, || {
        exec_waiting(
            "(tx, ch) = channel()\nscope(fn(s):\n  t = spawn(s, () => recv(ch))\n  blocked(ch, 1)\n  send(tx, 1)\n;;)",
        )
    });
    assert_eq!((o.err.as_str(), o.exit), ("", 0));
    let o = within(10, || {
        exec_waiting(
            "(_tx, ch) = channel()\nscope(fn(s):\n  t = spawn(s, () => recv(ch))\n  blocked(ch, 1)\n  cancel(t)\n  print(await(t))\n;;)",
        )
    });
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err({kind: 'cancelled, message: \"cancelled\"})\n", "", 0)
    );
}

/// A wait on the operating system (an offload job) is not a
/// blocked task: the entry waits for a value the task sends after it.
#[test]
fn a_wait_outside_the_token_is_not_blocked() {
    fn natives() -> Vec<Value> {
        fn os_sleep(t: &mut Task<'_>, _: Vec<Value>) -> Result<crate::vm::Flow, Stop> {
            crate::native::nat::offload(
                t,
                || std::thread::sleep(Duration::from_millis(30)),
                |_, ()| Ok(Value::Nil),
            )
        }
        vec![Value::Native(Native::leak_step(
            "os_sleep",
            vec![],
            os_sleep,
        ))]
    }
    let o: Outcome = within(10, || {
        exec(
            "(tx, ch) = channel()\nscope(fn(s):\n  spawn(s, fn(): os_sleep(); send(tx, 1) ;;)\n  print(recv(ch))\n;;)",
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
        "(a_tx, a) = channel()\n(b_tx, b) = channel()\nscope(fn(s):\n  t = spawn(s, fn(): recv(a); send(b_tx, 1) ;;)\n  recv(b)\n  send(a_tx, 1)\n;;)",
    );
    assert_eq!((report_heads(&err).as_str(), code), (DEADLOCK, 1));
    assert!(start.elapsed() < Duration::from_millis(500));
}

/// A long chain of task results, each holding the next handle, drops in
/// constant stack (`value/release.rs`).
#[test]
fn a_long_chain_of_task_results_drops_without_recursion() {
    use super::task::{CancelNode, TaskHandle};
    std::thread::Builder::new()
        .stack_size(256 << 10)
        .spawn(move || {
            let mut v = Value::Nil;
            for _ in 0..200_000 {
                v = TaskHandle::finished(v, CancelNode::root());
            }
            drop(v);
        })
        .unwrap()
        .join()
        .unwrap();
}

// --- fix round 1 ---

/// An entry error while another task is blocked is the entry's error, not
/// a deadlock: the entry's end must not be judged before the program is
/// cancelled (an entry error cancels the program at once).
#[test]
fn an_entry_error_beside_a_blocked_task_is_not_a_deadlock() {
    let (out, err, code) = run_within(
        "scope(fn(s):\n  spawn(s, fn(): (_tx, rx) = channel(); recv(rx) ;;)\n  sleep(5ms)\n  fail(\"real error\")\n;;)",
    );
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        (
            "",
            "error[E0803]: real error\n  --> -e:4:7\n  |\n4 |   fail(\"real error\")\n  |       ^\n  |\n call path (innermost first):\n   -e:4       <lambda>\n   -e:1       <entry>\n",
            1
        )
    );
    // The same for exit: the exit code, no deadlock report.
    let (_, err, code) = run_within(
        "scope(fn(s):\n  spawn(s, fn(): (_tx, rx) = channel(); recv(rx) ;;)\n  sleep(5ms)\n  exit(3)\n;;)",
    );
    assert_eq!((err.as_str(), code), ("", 3));
}

/// The variant with a task still running when the entry fails: it is
/// queued to run (its 1 ms sleep ran out during `hold`), so it
/// runs right after the entry, before the command can cancel
/// anything, and then blocks for good.
#[test]
fn an_entry_error_beside_a_task_about_to_block_is_not_a_deadlock() {
    let o = within(10, || {
        exec_waiting(
            "(go_tx, go) = channel(1)\n(ready_tx, ready) = channel(1)\n(_tx, never) = channel()\nscope(fn(s):\n  spawn(s, fn(): send(ready_tx, nil); until(() => buffered(go) > 0); recv(never) ;;)\n  recv(ready)\n  send(go_tx, true)\n  hold(5)\n  fail(\"real error\")\n;;)",
        )
    });
    assert!(o.err.starts_with("error[E0803]: real error\n"), "{}", o.err);
    assert!(!o.err.contains("E0702"), "{}", o.err);
    assert_eq!(o.exit, 1);
}
