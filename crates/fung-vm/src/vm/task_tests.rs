//! Tasks (stage 3a, Task 3): `spawn`, `await`, `cancel`, `sleep`, the
//! blocking point and turn handoff, and `run_task` for natives.

use std::time::{Duration, Instant};

use crate::native::nat;
use crate::testutil::{Opts, exec, on_big_stack, run, setup};
use crate::value::{Native, Value};
use crate::vm::{Stop, Task};

#[test]
fn spawn_and_await_basics() {
    let (out, err, code) = run(
        "supervise(fn(s):\n  ts = for i in 1..=3: spawn(s, () => i * 2) ;;\n  print(ts |> map(await), await(spawn(s, () => Err('e))), await(spawn(s, () => 1 / 0)) |> is_err)\n;;, e => nil)",
    );
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("[2, 4, 6] Err('e) true\n", "", 0)
    );
}

#[test]
fn cancel_reaches_a_sleeping_task_and_its_children() {
    let start = Instant::now();
    // The scope waits for its children, so the sleeper the timed-out
    // await leaves is cancelled before the scope ends.
    let (out, _, code) = run(
        "scope(fn(s):\n  t = spawn(s, () => scope(fn(c): spawn(c, () => sleep(10s)); sleep(10s) ;;))\n  cancel(t)\n  u = spawn(s, () => sleep(10s))\n  print(await(t), timeout(10ms, () => await(u)))\n  cancel(u)\n;;)",
    );
    assert_eq!(
        (out.as_str(), code),
        ("Err({kind: 'cancelled}) Err({kind: 'timeout})\n", 0)
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn a_busy_task_does_not_starve_the_others() {
    let (out, _, code) = run(
        "fn spin(n) = n == 0 then 0 else spin(n - 1)\nscope(fn(s):\n  t = spawn(s, () => spin(50000000))\n  print(\"other ran\")\n  cancel(t)\n;;)",
    );
    assert_eq!((out.as_str(), code), ("other ran\n", 0));
}

/// The brief's test above never lets the spinner run (the entry keeps
/// its turn until it cancels it). Here the entry parks
/// (`sleep`), the spinner runs, and the entry gets its turn back only if
/// the spinner hands its turn over.
#[test]
fn a_busy_task_hands_the_token_back() {
    let start = Instant::now();
    let (out, _, code) = run(
        "fn spin(n) = n == 0 then 0 else spin(n - 1)\nscope(fn(s):\n  t = spawn(s, () => spin(500000000))\n  sleep(10ms)\n  print(\"other ran\")\n  cancel(t)\n  print(await(t))\n;;)",
    );
    assert_eq!(
        (out.as_str(), code),
        ("other ran\nErr({kind: 'cancelled})\n", 0)
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn two_thousand_tasks_run() {
    let (out, _, code) = run(
        "scope(fn(s):\n  ts = for i in 0..2000: spawn(s, () => i) ;;\n  print(fold(ts, 0, (a, t) => a + await(t)))\n;;)",
    );
    assert_eq!((out.as_str(), code), ("1999000\n", 0));
}

// --- spawn, await and results ---

#[test]
fn spawn_await_and_results() {
    let cases = [
        (
            "scope(fn(s):\n  t = spawn(s, () => 1 + 1)\n  print(await(t), await(t))\n;;)",
            "2 2\n",
        ),
        (
            "scope(fn(s):\n  t = spawn(s, () => Err(\"e\"))\n  print(await(t))\n;;)",
            "Err(\"e\")\n",
        ),
        // A supervised child's runtime error comes back from `await`.
        (
            "supervise(fn(s):\n  t = spawn(s, () => 1 / 0)\n  print(await(t))\n;;, e => nil)",
            "Err({kind: 'error, message: \"error[E0502]: division by zero\"})\n",
        ),
        (
            "scope(fn(s):\n  t = spawn(s, () => sleep(10s))\n  cancel(t)\n  print(await(t))\n;;)",
            "Err({kind: 'cancelled})\n",
        ),
        (
            "scope(fn(s):\n  t = spawn(s, () => sleep(10s))\n  print(timeout(10ms, () => await(t)))\n  cancel(t)\n;;)",
            "Err({kind: 'timeout})\n",
        ),
        (
            "scope(fn(s):\n  ts = for i in 0..5: spawn(s, () => i * i) ;;\n  print(ts |> map(await))\n;;)",
            "[0, 1, 4, 9, 16]\n",
        ),
        (
            "scope(s => print(type_of(spawn(s, () => 1)), spawn(s, () => 1) == spawn(s, () => 1)))",
            "'task false\n",
        ),
        (
            "scope(fn(s):\n  outer = spawn(s, () => scope(fn(c):\n    inner = spawn(c, () => sleep(10s))\n    await(inner)\n  ;;))\n  sleep(10ms)\n  cancel(outer)\n  print(await(outer))\n;;)",
            "Err({kind: 'cancelled})\n",
        ),
        // Cancelling a finished task changes nothing.
        (
            "scope(fn(s):\n  t = spawn(s, () => 1)\n  print(await(t), cancel(t), await(t))\n;;)",
            "1 nil 1\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", want), "{src}");
    }
    for (src, want) in [
        ("scope(s => spawn(s, 1))", "E0403"),
        ("spawn(1, () => 1)", "E0403"),
        ("await(1)", "E0403"),
        ("cancel(1)", "E0403"),
    ] {
        assert_eq!(exec(src, Opts::default()).code, want, "{src}");
    }
}

#[test]
fn a_task_handle_prints_compares_and_types() {
    let (out, err, code) = run(
        "t = scope(s => spawn(s, () => 1))\nprint(t, [t], {a: t}, t == t, t != t, type_of(t), str(t), \"${t}\")\nprint(t < t)",
    );
    assert_eq!(
        out,
        "<task> [<task>] {a: <task>} true false 'task <task> <task>\n"
    );
    assert!(
        err.starts_with("error[E0501]: cannot compare task and task\n  --> -e:3:9\n"),
        "{err}"
    );
    assert_eq!(code, 1);
}

/// A task that cancels itself stops at its next call (05: the flag is
/// checked at every call), so it never runs on to return 1 (R3a
/// note).
#[test]
fn a_task_that_cancels_itself_stops_at_its_next_call() {
    let (out, _, code) = run(
        "(tx, ch) = channel(1)\nscope(fn(s):\n  t = spawn(s, fn(): me = recv(ch)!; cancel(me); print(\"after self cancel\"); 1 ;;)\n  send(tx, t)\n  print(await(t))\n;;)",
    );
    assert_eq!((out.as_str(), code), ("Err({kind: 'cancelled})\n", 0));
}

/// The failure of a supervised task stays retrievable after a timed-out
/// `await`.
#[test]
fn an_await_timeout_does_not_hide_a_later_failure() {
    let (out, _, code) = run(
        "(go_tx, go) = channel()\nsupervise(fn(s):\n  t = spawn(s, fn(): recv(go); fail(\"later\") ;;)\n  print(timeout(1ms, () => await(t)))\n  send(go_tx, nil)\n  print(await(t))\n;;, e => nil)",
    );
    assert_eq!(
        (out.as_str(), code),
        (
            "Err({kind: 'timeout})\nErr({kind: 'error, message: \"error[E0803]: later\"})\n",
            0
        )
    );
}

// --- sleep ---

#[test]
fn sleep_waits_and_is_cancellable() {
    let start = Instant::now();
    let (out, _, code) = run("sleep(20ms)\nprint(1)");
    assert_eq!((out.as_str(), code), ("1\n", 0));
    assert!(start.elapsed() >= Duration::from_millis(15));

    // Cancelled from another thread while it sleeps (`cancel_all`).
    let start = Instant::now();
    let code = on_big_stack(|| {
        let s = setup("sleep(10s)\nprint(1)", Opts::default()).unwrap_or_else(|e| panic!("{e:?}"));
        let shared = s.rt.shared();
        let canceller = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            shared.cancel_all();
        });
        let r = s.rt.run();
        canceller.join().unwrap();
        let out = s.out.text();
        match r {
            Err(Stop::Error(d)) => (d.code.clone(), out),
            other => (format!("{other:?}"), out),
        }
    });
    assert_eq!(code, ("E0701".to_string(), String::new()));
    assert!(start.elapsed() < Duration::from_secs(2));

    assert_eq!(exec("sleep(1)", Opts::default()).code, "E0403");
    assert_eq!(exec("sleep(-1s)", Opts::default()).code, "");
}

/// Set by `mark()` in `outside_lets_other_tasks_run`.
static MARKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A wait on the operating system is handed off (`nat::offload`), so
/// another task runs meanwhile: `os_wait`'s job waits until the spawned
/// task has called `mark()`, which it can only do if it got a turn (no
/// timing: without the handoff this never ends, and the test's own bound
/// reports it).
#[test]
fn an_offload_lets_other_tasks_run() {
    use std::sync::atomic::Ordering;
    fn natives() -> Vec<Value> {
        vec![
            Value::Native(Native::leak_step("os_wait", vec![], |t: &mut Task, _| {
                let wait = || {
                    let until = Instant::now() + Duration::from_secs(10);
                    while !MARKED.load(Ordering::SeqCst) && Instant::now() < until {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    MARKED.load(Ordering::SeqCst)
                };
                crate::native::nat::offload(t, wait, |_, seen| Ok(Value::Bool(seen)))
            })),
            Value::Native(Native::leak("mark", vec![], |_: &mut Task, _| {
                MARKED.store(true, Ordering::SeqCst);
                Ok(Value::Nil)
            })),
        ]
    }
    MARKED.store(false, Ordering::SeqCst);
    let opts = Opts {
        extra: natives,
        ..Opts::default()
    };
    let o = exec(
        "scope(fn(s):\n  spawn(s, fn(): print(\"ran\"); mark() ;;)\n  print(os_wait())\n  print(\"after\")\n;;)",
        opts,
    );
    assert_eq!(
        (o.code.as_str(), o.out.as_str()),
        ("", "ran\ntrue\nafter\n")
    );
}

/// A panic in an offload job is resumed on the run thread and unwinds
/// through the native's frames (which hold values; here a task handle):
/// the panic fence reports E0801.
#[test]
fn a_panic_in_an_offload_job_is_e0801() {
    fn natives() -> Vec<Value> {
        fn os_panic(t: &mut Task<'_>, _: Vec<Value>) -> Result<crate::vm::Flow, Stop> {
            nat::offload(t, || panic!("outside"), |_, ()| Ok(Value::Nil))
        }
        vec![Value::Native(Native::leak_step(
            "os_panic",
            vec![nat::param("x")],
            os_panic,
        ))]
    }
    let opts = Opts {
        extra: natives,
        ..Opts::default()
    };
    let o = exec("scope(s => os_panic(spawn(s, () => 1)))", opts);
    assert_eq!(o.code, "E0801");
    assert!(o.err.contains("outside"), "{}", o.err);
}

/// A node below one whose task and handle are gone stays reachable from
/// the root: cancelling the root reaches it.
#[test]
fn a_grandchild_stays_cancellable_after_its_parent_is_gone() {
    let root = super::task::CancelNode::root();
    let mid = root.child();
    let leaf = mid.child();
    drop(mid);
    root.cancel();
    assert!(leaf.is_cancelled());
    // A cancelled node leaves its parent's list.
    let a = root.child();
    assert!(a.is_cancelled());
    let fresh = super::task::CancelNode::root();
    let b = fresh.child();
    b.cancel();
    assert_eq!(fresh.child_count(), 0);
}

/// A task event wakes only the tasks waiting on it: 200 tasks sleeping
/// for different times, each awaited in turn, wake about once per sleep
/// and once per await, not once per event per blocked task.
#[test]
fn an_event_wakes_only_its_waiters() {
    let (wakeups, out) = on_big_stack(|| {
        let s = setup(
            "scope(fn(s):\n  ts = for i in 0..200: spawn(s, () => sleep(i * 1ms)) ;;\n  print(len(map(ts, await)))\n;;)",
            Opts::default(),
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
        s.rt.run().unwrap();
        (s.rt.shared().wakeups(), s.out.text())
    });
    assert_eq!(out, "200\n");
    assert!(wakeups < 1_000, "{wakeups} wakeups");
}
