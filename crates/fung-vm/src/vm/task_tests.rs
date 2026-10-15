//! Tasks (stage 3a, Task 3): `spawn`, `await`, `cancel`, `sleep`, the
//! blocking point and turn handoff. Ports of Go `internal/vm/conc_test.go`
//! `TestSleepWaitsAndIsCancellable`, `TestSpawnAwaitAndResults`, the
//! output half of `TestAwaitTimeoutDoesNotHideALaterFailure`, and
//! `context_test.go` `TestRunTaskRunsOnAFreshTask`. Expected outputs come
//! from the Go binary.

use std::time::{Duration, Instant};

use crate::native::nat;
use crate::testutil::{Opts, exec, on_big_stack, run, setup};
use crate::value::{Native, Value};
use crate::vm::{Stop, Task};

#[test]
fn spawn_and_await_like_go() {
    let (out, err, code) = run(
        "ts = for i in 1..=3: spawn(() => i * 2) ;;\nprint(ts |> map(await), await(spawn(() => Err('e))), await(spawn(() => 1 / 0)) |> is_err)",
    );
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("[2, 4, 6] Err('e) true\n", "", 0)
    );
}

#[test]
fn cancel_reaches_a_sleeping_task_and_its_children() {
    let start = Instant::now();
    let (out, _, code) = run(
        "t = spawn(fn(): spawn(() => sleep(10s)); sleep(10s) ;;)\ncancel(t)\nprint(await(t), await(spawn(() => sleep(10s)), timeout: 10ms))",
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
        "fn spin(n) = n == 0 then 0 else spin(n - 1)\nt = spawn(() => spin(50000000))\nprint(\"other ran\")\ncancel(t)",
    );
    assert_eq!((out.as_str(), code), ("other ran\n", 0));
}

/// The brief's test above never lets the spinner run (the entry keeps
/// the token until it cancels it). Here the entry gives the token up
/// (`sleep`), the spinner takes it, and the entry gets it back only if
/// the spinner hands its turn over.
#[test]
fn a_busy_task_hands_the_token_back() {
    let start = Instant::now();
    let (out, _, code) = run(
        "fn spin(n) = n == 0 then 0 else spin(n - 1)\nt = spawn(() => spin(500000000))\nsleep(10ms)\nprint(\"other ran\")\ncancel(t)\nprint(await(t))",
    );
    assert_eq!(
        (out.as_str(), code),
        ("other ran\nErr({kind: 'cancelled})\n", 0)
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn two_thousand_tasks_run() {
    let (out, _, code) =
        run("ts = for i in 0..2000: spawn(() => i) ;;\nprint(fold(ts, 0, (a, t) => a + await(t)))");
    assert_eq!((out.as_str(), code), ("1999000\n", 0));
}

// --- Go TestSpawnAwaitAndResults ---

#[test]
fn spawn_await_and_results() {
    let cases = [
        ("t = spawn(() => 1 + 1)\nprint(await(t), await(t))", "2 2\n"),
        (
            "t = spawn(() => Err(\"e\"))\nprint(await(t))",
            "Err(\"e\")\n",
        ),
        (
            "t = spawn(() => 1 / 0)\nprint(await(t))",
            "Err({kind: 'error, message: \"error[E0502]: division by zero\"})\n",
        ),
        (
            "t = spawn(() => sleep(10s))\ncancel(t)\nprint(await(t))",
            "Err({kind: 'cancelled})\n",
        ),
        (
            "t = spawn(() => sleep(10s))\nprint(await(t, timeout: 10ms))\ncancel(t)",
            "Err({kind: 'timeout})\n",
        ),
        (
            "ts = for i in 0..5: spawn(() => i * i) ;;\nprint(ts |> map(await))",
            "[0, 1, 4, 9, 16]\n",
        ),
        (
            "print(type_of(spawn(() => 1)), spawn(() => 1) == spawn(() => 1))",
            "'task false\n",
        ),
        (
            "outer = spawn(fn():\n  inner = spawn(() => sleep(10s))\n  await(inner)\n;;)\nsleep(10ms)\ncancel(outer)\nprint(await(outer))",
            "Err({kind: 'cancelled})\n",
        ),
        // Cancelling a finished task changes nothing.
        (
            "t = spawn(() => 1)\nprint(await(t), cancel(t), await(t))",
            "1 nil 1\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", want), "{src}");
    }
    for (src, want) in [
        ("spawn(1)", "E0403"),
        ("await(1)", "E0403"),
        ("cancel(1)", "E0403"),
        ("await(spawn(() => 1), timeout: 1)", "E0403"),
    ] {
        assert_eq!(exec(src, Opts::default()).code, want, "{src}");
    }
}

#[test]
fn a_task_handle_prints_compares_and_types_like_go() {
    let (out, err, code) = run(
        "t = spawn(() => 1)\nprint(t, [t], {a: t}, t == t, t != t, type_of(t), str(t), \"${t}\")\nprint(t < t)",
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

/// A finished task cancels what it spawned and left running (Go `defer
/// child.cancel()`).
#[test]
fn a_finished_task_cancels_its_children() {
    let start = Instant::now();
    let (out, _, code) =
        run("t = spawn(fn(): spawn(() => sleep(10s)) ;;)\ninner = await(t)\nprint(await(inner))");
    assert_eq!((out.as_str(), code), ("Err({kind: 'cancelled})\n", 0));
    assert!(start.elapsed() < Duration::from_secs(2));
}

/// The failure of a task stays retrievable after a timed-out `await`
/// (the output half of Go `TestAwaitTimeoutDoesNotHideALaterFailure`;
/// the report of a never-retrieved failure is Task 6).
#[test]
fn an_await_timeout_does_not_hide_a_later_failure() {
    let (out, _, code) = run(
        "t = spawn(fn(): sleep(20ms); fail(\"later\") ;;)\nprint(await(t, timeout: 1ms))\nprint(await(t))",
    );
    assert_eq!(
        (out.as_str(), code),
        (
            "Err({kind: 'timeout})\nErr({kind: 'error, message: \"error[E0803]: later\"})\n",
            0
        )
    );
}

// --- Go TestSleepWaitsAndIsCancellable ---

#[test]
fn sleep_waits_and_is_cancellable() {
    let start = Instant::now();
    let (out, _, code) = run("sleep(20ms)\nprint(1)");
    assert_eq!((out.as_str(), code), ("1\n", 0));
    assert!(start.elapsed() >= Duration::from_millis(15));

    // Cancelled from another thread while it sleeps (Go: a goroutine
    // calls rt.CancelAll()).
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

// --- Go TestRunTaskRunsOnAFreshTask ---

/// `run_twice(f)`: f(1) + f(2), each on a fresh child task, as a list
/// `[sum, cancelled]` so the test sees what the native saw; `run_once(f)`:
/// f(1) on a fresh child task.
fn run_natives() -> Vec<Value> {
    vec![
        Value::Native(Native::leak(
            "run_twice",
            vec![nat::param("f")],
            |t: &mut Task, a| {
                let seen = t.cancelled();
                let x = t.run_task(&a[0], vec![Value::Int(1)])?;
                let y = t.run_task(&a[0], vec![Value::Int(2)])?;
                let sum = crate::value::add(&x, &y).map_err(Stop::from)?;
                Ok(nat::list(vec![sum, Value::Bool(seen)]))
            },
        )),
        Value::Native(Native::leak(
            "run_once",
            vec![nat::param("f")],
            |t: &mut Task, a| t.run_task(&a[0], vec![Value::Int(1)]),
        )),
    ]
}

#[test]
fn run_task_runs_on_a_fresh_task() {
    let opts = Opts {
        extra: run_natives,
        ..Opts::default()
    };
    let o = exec("print(run_twice(x => x * 10))", opts);
    assert_eq!((o.code.as_str(), o.out.as_str()), ("", "[30, false]\n"));
    let o = exec("run_twice(x => fail(\"boom\"))", opts);
    assert_eq!(o.code, "E0803");
    // A task the child spawned and left running ends with the child.
    let start = Instant::now();
    let o = exec(
        "h = run_once(x => spawn(() => sleep(10s)))\nprint(await(h))",
        opts,
    );
    assert_eq!(
        (o.code.as_str(), o.out.as_str()),
        ("", "Err({kind: 'cancelled})\n")
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}

/// `outside` gives the token up for a wait on the operating system, so
/// another task runs meanwhile.
#[test]
fn outside_lets_other_tasks_run() {
    fn os_wait() -> Vec<Value> {
        vec![Value::Native(Native::leak(
            "os_wait",
            vec![],
            |t: &mut Task, _| {
                super::block::outside(t, || std::thread::sleep(Duration::from_millis(50)));
                Ok(Value::Nil)
            },
        ))]
    }
    let opts = Opts {
        extra: os_wait,
        ..Opts::default()
    };
    let o = exec(
        "t = spawn(() => print(\"ran\"))\nos_wait()\nprint(\"after\")",
        opts,
    );
    assert_eq!((o.code.as_str(), o.out.as_str()), ("", "ran\nafter\n"));
}

/// An exit handler still runs after the program's tasks were cancelled:
/// handlers have their own root.
#[test]
fn exit_handlers_run_after_cancel_all() {
    let o = on_big_stack(|| {
        let s = setup(
            "at_exit(() => print(await(spawn(() => \"h\"))))\nprint(1)",
            Opts::default(),
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
        s.rt.run().unwrap();
        s.rt.cancel_all();
        s.rt.wait();
        let errs = s.rt.run_exit_handlers();
        assert!(errs.is_empty());
        s.out.text()
    });
    assert_eq!(o, "1\nh\n");
}
