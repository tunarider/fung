//! Park requests (concurrency v2 R1a): a native that waits for another
//! task asks its run loop to park it instead of sleeping the thread
//! itself (`cont.rs`, `block.rs`).

use std::time::{Duration, Instant};

use crate::testutil::exec_waiting;

const DEADLOCK: &str = "error[E0702]: deadlock: all tasks are waiting\n";

fn out(src: &str) -> (String, String) {
    let o = exec_waiting(src);
    (o.out, o.err)
}

#[test]
fn sleep_parks() {
    assert_eq!(
        out("sleep(1ms)\nprint(parks() >= 1)"),
        ("true\n".into(), "".into())
    );
}

#[test]
fn await_parks() {
    assert_eq!(
        out(
            "print(scope(fn(s):\n  t = spawn(s, () => sleep(5ms))\n  await(t)\n  parks() >= 1\n;;))"
        ),
        ("true\n".into(), "".into())
    );
}

#[test]
fn a_finished_task_is_awaited_without_parking() {
    assert_eq!(
        out(
            "print(scope(fn(s):\n  t = spawn(s, () => 1)\n  until(() => finished(t))\n  n = parks()\n  await(t)\n  parks() == n\n;;))"
        ),
        ("true\n".into(), "".into())
    );
}

#[test]
fn a_cancelled_sleep_unwinds() {
    let start = Instant::now();
    assert_eq!(
        out("scope(fn(s):\n  t = spawn(s, () => sleep(10s))\n  cancel(t)\n  print(await(t))\n;;)"),
        (
            "Err({kind: 'cancelled, message: \"cancelled\"})\n".into(),
            "".into()
        )
    );
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn parked_tasks_are_judged_like_blocked_ones() {
    // A task awaiting itself, and the entry awaiting it: nobody can run.
    let o = exec_waiting(
        "(tx, rx) = channel(1)\nscope(fn(s):\n  t = spawn(s, fn():\n    Ok(me) = recv(rx)\n    await(me)\n  ;;)\n  send(tx, t)\n  await(t)\n;;)",
    );
    assert!(o.err.starts_with(DEADLOCK), "{}", o.err);
    assert_eq!(o.exit, 1);
    // A sleep alone is not a deadlock: a timer wakes it.
    assert_eq!(out("sleep(20ms)\nprint(1)"), ("1\n".into(), "".into()));
}

#[test]
fn recv_and_send_park() {
    // The receiver waits first.
    assert_eq!(
        out(
            "(tx, rx) = channel()\nprint(scope(fn(s):\n  t = spawn(s, fn(): r = recv(rx); parks() >= 1 ;;)\n  blocked(rx, 1)\n  send(tx, 1)\n  await(t)\n;;))"
        ),
        ("true\n".into(), "".into())
    );
    // The sender waits first.
    assert_eq!(
        out(
            "(tx, rx) = channel()\nprint(scope(fn(s):\n  t = spawn(s, fn(): send(tx, 1); parks() >= 1 ;;)\n  blocked(rx, 1)\n  recv(rx)\n  await(t)\n;;))"
        ),
        ("true\n".into(), "".into())
    );
}

#[test]
fn a_walk_over_a_receiver_parks() {
    for walk in [
        "fold(rx, 0, (a, x) => a + x)",
        "join(rx, \",\")",
        "sort(rx)",
        "sort_by(rx, x => -x)",
        "flatten(rx)",
    ] {
        let src = format!(
            "(tx, rx) = channel()\nprint(scope(fn(s):\n  t = spawn(s, fn(): r = {walk}; [r, parks() >= 1] ;;)\n  blocked(rx, 1)\n  send(tx, 2)\n  send(tx, 1)\n  close(tx)\n  await(t)\n;;))"
        );
        let (o, e) = out(&src);
        assert!(o.ends_with(", true]\n"), "{walk}: {o}{e}");
    }
}

/// The walk's own task is cancelled in its callback: the walk stops before
/// it takes the next element, so everything after the first stays in the
/// channel (05).
#[test]
fn a_cancelled_walk_leaves_the_rest_in_the_channel() {
    assert_eq!(
        out(
            "(tx, rx) = channel(3)\neach([1, 2, 3], x => send(tx, x))\n(htx, hrx) = channel(1)\nscope(fn(s):\n  t = spawn(s, fn():\n    Ok(me) = recv(hrx)\n    each(rx, x => cancel(me))\n  ;;)\n  send(htx, t)\n  print(await(t), buffered(rx))\n;;)"
        ),
        (
            "Err({kind: 'cancelled, message: \"cancelled\"}) 2\n".into(),
            "".into()
        )
    );
    // Cancelled while the walk waits for a value.
    assert_eq!(
        out(
            "(tx, rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => each(rx, x => x))\n  blocked(rx, 1)\n  cancel(t)\n  print(await(t))\n;;)"
        ),
        (
            "Err({kind: 'cancelled, message: \"cancelled\"})\n".into(),
            "".into()
        )
    );
}

#[test]
fn a_scope_waiting_for_children_parks() {
    assert_eq!(
        out("scope(s => spawn(s, () => sleep(5ms)))\nprint(parks() >= 1)"),
        ("true\n".into(), "".into())
    );
}

/// A deadline over a scope cancels its child, whose release still runs to
/// the end before the scope (and the `timeout`) returns.
#[test]
fn an_outer_cancel_waits_for_the_children_then_unwinds() {
    assert_eq!(
        out(
            "(_tx, rx) = channel()\nprint(timeout(20ms, () => scope(fn(s):\n  spawn(s, () => with(() => 1, r => recv(rx), fn(r): sleep(50ms); print(\"released\") ;;))\n  recv(rx)\n;;)))"
        ),
        (
            "released\nErr({kind: 'timeout, message: \"timed out after 20ms\"})\n".into(),
            "".into()
        )
    );
}

fn with_stdin(src: &str, stdin: &'static str) -> crate::testutil::Outcome {
    crate::testutil::exec(
        src,
        crate::testutil::Opts {
            stdin,
            extra: crate::testutil::probes,
            ..crate::testutil::Opts::default()
        },
    )
}

#[test]
fn reading_stdin_parks() {
    let o = with_stdin(
        "n = offloads()\np = parks()\nprint(read_line())\nprint(parks() > p, offloads() == n)",
        "a\n",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        ("Ok(\"a\")\ntrue true\n", "")
    );
}

#[test]
fn reading_a_file_handle_offloads() {
    let dir = crate::testutil::temp_dir();
    let p = dir.join("a.txt");
    std::fs::write(&p, "x\ny\nz\n").unwrap();
    let o = exec_waiting(&format!(
        "import fs\nh = fs.open(\"{}\")!\nn = offloads()\nprint(read_line(h))\nm = offloads()\nprint(read_lines(h))\nprint(m > n, offloads() > m)",
        p.display()
    ));
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        ("Ok(\"x\")\nOk([\"y\", \"z\"])\ntrue true\n", "")
    );
}

#[test]
fn regex_compile_offloads_once() {
    assert_eq!(
        out(
            "import regex\nn = offloads()\nregex.is_match(\"a\", \"a+(park-test)?\")\nm = offloads()\nregex.is_match(\"aa\", \"a+(park-test)?\")\nprint(m - n, offloads() - m)"
        ),
        ("1 0\n".into(), "".into())
    );
}

/// What the release build of the branch's starting point reports.
#[test]
fn offload_errors_point_at_the_call() {
    let o = with_stdin("import regex\nprint(regex.is_match(\"a\", \"(\"))", "");
    assert_eq!(
        o.err,
        "error[E0515]: invalid pattern `(`: unclosed group\n  --> -e:2:21\n  |\n2 | print(regex.is_match(\"a\", \"(\"))\n  |                     ^\n  |\n call path (innermost first):\n   -e:2       <entry>\n"
    );
    let o = with_stdin("print(read_line(1))", "");
    assert_eq!(
        o.err,
        "error[E0403]: argument `f` must be file | nil, got int\n  --> -e:1:16\n  |\n1 | print(read_line(1))\n  |                ^\n  |\n call path (innermost first):\n   -e:1       <entry>\n"
    );
}

/// Only the parking point sleeps a thread, and only the offload helpers
/// leave the run token: every other wait goes through a park request or
/// `nat::offload`, which the scheduler (R1b) takes over.
#[test]
fn nothing_sleeps_a_thread_but_the_parking_point() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![src.clone()];
    let mut found: Vec<(String, &str)> = Vec::new();
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            if name.ends_with("_tests.rs") || name == "tests.rs" || name == "testutil.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap();
            let rel = p.strip_prefix(&src).unwrap().to_string_lossy().into_owned();
            for call in ["block(", "block_until(", "outside(", "outside_watched("] {
                let n = text
                    .match_indices(call)
                    .filter(|(i, _)| {
                        // A call, not a longer name (`try_block(`) or a
                        // definition (`fn outside(`).
                        let before = &text[..*i];
                        !before.ends_with(|c: char| c.is_alphanumeric() || c == '_')
                            && !before.ends_with("fn ")
                    })
                    .count();
                if n > 0 {
                    found.push((rel.clone(), call));
                }
            }
        }
    }
    found.sort();
    let allowed: &[(&str, &str)] = &[];
    let found: Vec<(&str, &str)> = found.iter().map(|(f, c)| (f.as_str(), *c)).collect();
    assert_eq!(found, allowed);
}

/// A computing task gives its turn at every kind of yield point: a call
/// in bytecode (a tail-recursive loop), a call a continuation asks for (a
/// callback without calls of its own), and a plain native a walk would
/// call in place (`map(xs, str)`).
#[test]
fn every_yield_point_lets_others_run() {
    for (src, want) in [
        (
            "fn spin(n) = n == 0 then 0 else spin(n - 1)\nr = spin(30000)",
            "0",
        ),
        ("r = len(map(0..30000, x => x + 1))", "30000"),
        ("r = len(map(0..30000, str))", "30000"),
    ] {
        let o = exec_waiting(&format!("n = yields()\n{src}\nprint(r, yields() - n >= 2)"));
        assert_eq!(
            (o.out.as_str(), o.err.as_str()),
            (format!("{want} true\n").as_str(), ""),
            "{src}"
        );
    }
}

/// A turn that ends on the call that parks is over with the park: the
/// task does not give a second turn as soon as it wakes. `each(ps,
/// fs.read)` ticks once per element, on the read itself, and every read
/// parks for the pool; so every thousandth park ends a turn, and none of
/// them may add a yield.
#[test]
fn a_park_ends_the_turn_it_ended_on() {
    let o = exec_waiting(
        "import fs\nps = map(0..3000, _ => \"/dev/null\")\nn = yields()\np = parks()\neach(ps, fs.read)\nprint(parks() - p >= 3000, yields() - n)",
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("true 0\n", ""));
}

/// The probes plus `this_thread()`, `job_thread()` (the thread an
/// offloaded job runs on) and `slow_job()` (a job that sleeps 50 ms).
fn offload_natives() -> Vec<crate::value::Value> {
    use crate::native::nat;
    use crate::value::{Native, Value};
    use crate::vm::{Flow, Stop, Task};
    fn id() -> String {
        format!("{:?}", std::thread::current().id())
    }
    fn job_thread(t: &mut Task<'_>, _: Vec<Value>) -> Result<Flow, Stop> {
        nat::offload(t, id, |_, s| Ok(Value::str(&s)))
    }
    fn slow_job(t: &mut Task<'_>, _: Vec<Value>) -> Result<Flow, Stop> {
        nat::offload(
            t,
            || std::thread::sleep(Duration::from_millis(50)),
            |_, ()| Ok(Value::Nil),
        )
    }
    let mut v = crate::testutil::probes();
    v.push(Value::Native(Native::leak(
        "this_thread",
        vec![],
        |_: &mut Task, _| Ok(Value::str(&id())),
    )));
    v.push(Value::Native(Native::leak_step(
        "job_thread",
        vec![],
        job_thread,
    )));
    v.push(Value::Native(Native::leak_step(
        "slow_job",
        vec![],
        slow_job,
    )));
    v
}

fn with_offload_natives(src: &str) -> crate::testutil::Outcome {
    crate::testutil::exec(
        src,
        crate::testutil::Opts {
            extra: offload_natives,
            ..crate::testutil::Opts::default()
        },
    )
}

#[test]
fn an_offload_runs_on_another_thread() {
    let o = with_offload_natives("print(job_thread() != this_thread())");
    assert_eq!((o.out.as_str(), o.err.as_str()), ("true\n", ""));
    let o = with_job_natives("print(pool_thread())");
    assert_eq!((o.out.as_str(), o.err.as_str()), ("fung-pool\n", ""));
}

fn with_job_natives(src: &str) -> crate::testutil::Outcome {
    crate::testutil::exec(
        src,
        crate::testutil::Opts {
            extra: crate::testutil::jobs::job_natives,
            ..crate::testutil::Opts::default()
        },
    )
}

/// Offloads one after another reuse pool threads: a job that finds a
/// thread idle runs on it, so 200 jobs take one thread. A job wakes its
/// task just before its thread is idle again, so the task waits for that
/// state (`pool_state()`) before the next job; otherwise the next job may
/// come in that gap and start another thread, as many as the machine is
/// busy (nine were seen on a CI runner).
#[test]
fn offloads_reuse_pool_threads() {
    let s = crate::testutil::setup(
        "fn settle() = match pool_state():\n  [t, i, 0] when i == t => nil\n  _ => settle()\n;;\nfor i in 0..200:\n  settle()\n  count_job(\"reuse\")\n;;",
        crate::testutil::Opts {
            extra: crate::testutil::jobs::job_natives,
            ..crate::testutil::Opts::default()
        },
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    s.rt.run().unwrap();
    assert_eq!(crate::testutil::jobs::count("reuse"), 200);
    assert_eq!(s.rt.shared().pool.threads(), 1);
}

/// A panic in a pool job is E0801 at the call, and the pool thread lives.
#[test]
fn a_panic_in_a_pool_job_is_e0801() {
    let s = crate::testutil::setup(
        "panic_job()",
        crate::testutil::Opts {
            extra: crate::testutil::jobs::job_natives,
            ..crate::testutil::Opts::default()
        },
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    let r = s.rt.run();
    assert!(
        matches!(&r, Err(crate::vm::Stop::Error(d)) if d.code == "E0801"),
        "{r:?}"
    );
    let pool = &s.rt.shared().pool;
    assert!(crate::testutil::jobs::until(|| pool.idle() == 1));
    assert_eq!(pool.threads(), 1);
}

/// A deadline does not cut a running job that is waited for short (spec
/// 15: only jobs that read are abandoned): the job runs to its end, then
/// the deadline is enforced.
/// Here the body returns right after the job, with no call left to see
/// the expired deadline, so `timeout` gives the body's value, as it did
/// before the job left the task's thread.
#[test]
fn an_offload_wait_is_not_cut_by_a_cancel() {
    let start = Instant::now();
    let o = with_offload_natives("print(timeout(10ms, () => slow_job()))");
    assert_eq!((o.out.as_str(), o.err.as_str()), ("nil\n", ""));
    assert!(start.elapsed() >= Duration::from_millis(50));
    // With a call after the job, that call unwinds on the deadline.
    let o = with_offload_natives("print(timeout(10ms, fn(): slow_job(); len(\"x\") ;;))");
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        (
            "Err({kind: 'timeout, message: \"timed out after 10ms\"})\n",
            ""
        )
    );
}

/// An abandoned job's late wake reaches its task while it waits on
/// something else: the task looks again, parks again, and gets only what
/// it waits for.
#[test]
fn a_late_wake_from_an_abandoned_job_is_harmless() {
    let o = with_job_natives(&format!(
        "{}(tx, rx) = channel()\nprint(timeout(10ms, () => abandoning_block_job(\"lw\")))\nprint(scope(fn(s):\n  spawn(s, fn():\n    open_latch(\"lw\")\n    until(() => pool_state()[1] == 1)\n    send(tx, 42)\n  ;;)\n  recv(rx)\n;;))",
        crate::testutil::UNTIL
    ));
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        (
            "Err({kind: 'timeout, message: \"timed out after 10ms\"})\nOk(42)\n",
            ""
        )
    );
}

/// A job that only reads, waiting in a full pool's queue, is dropped at
/// the deadline.
#[test]
fn a_queued_job_is_dropped_at_the_deadline() {
    let o = crate::testutil::exec_prepared(
        &format!(
            "{}print(scope(fn(s):\n  spawn(s, () => block_job(\"qd\"))\n  until(() => pool_state() == [1, 0, 0])\n  r = timeout(20ms, () => abandoning_block_job(\"qd2\"))\n  open_latch(\"qd\")\n  open_latch(\"qd2\")\n  r\n;;))",
            crate::testutil::UNTIL
        ),
        crate::testutil::Opts {
            extra: crate::testutil::jobs::job_natives,
            ..crate::testutil::Opts::default()
        },
        |rt| rt.shared().pool.set_limits(1, Duration::from_secs(10)),
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        (
            "Err({kind: 'timeout, message: \"timed out after 20ms\"})\n",
            ""
        )
    );
}

/// An `await` that a deadline cuts short leaves the awaited task's
/// waiting list (review C3): a server awaiting a shared task under a
/// `timeout` per request used to grow that list with every request, and
/// every later `await` scanned it.
#[test]
fn an_await_cut_short_leaves_the_waiting_list() {
    assert_eq!(
        out(
            "scope(fn(s):\n  bg = spawn(s, () => sleep(1h))\n  scope(c => each(0..100, _ => spawn(c, fn(): _ = timeout(1ms, () => await(bg)); nil ;;)))\n  print(awaiters(bg))\n  cancel(bg)\n;;)"
        ),
        ("0\n".into(), "".into())
    );
}

/// Cancelled waiters leave the list; the ones left wake in the order
/// they began waiting. 40 waiters, so the list is long enough to keep an
/// index by task number, and 26 leave, so it is compacted.
#[test]
fn waiters_left_after_cancellations_wake_in_order() {
    assert_eq!(
        out(
            "(go_tx, go) = channel()\nscope(fn(s):\n  t = spawn(s, () => recv(go))\n  ws = map(0..40, i => spawn(s, fn(): _ = await(t); print(i) ;;))\n  until(() => awaiters(t) == 40)\n  gone = filter(0..40, i => i % 3 != 0) |> map(i => ws[i])\n  each(gone, cancel)\n  until(() => all(gone, finished))\n  print(\"left\", awaiters(t))\n  close(go_tx)\n;;)"
        ),
        (
            "left 14\n0\n3\n6\n9\n12\n15\n18\n21\n24\n27\n30\n33\n36\n39\n".into(),
            "".into()
        )
    );
}
