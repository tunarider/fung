//! The M:1 scheduler (concurrency v2 R1b, `sched.rs`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::native::nat;
use crate::testutil::{Opts, exec, exec_waiting, probes, setup};
use crate::value::{Native, Value};
use crate::vm::{Cont, Flow, Next, Stop, Task, Wake};

const DEADLOCK: &str = "error[E0702]: deadlock: all tasks are waiting\n";

fn id() -> String {
    format!("{:?}", std::thread::current().id())
}

/// `this_thread()`; `wake_early()`: parks, after a thread of its own woke
/// the task (and was joined) before the park was decided; `slow_job()`: an
/// offload that sleeps 50 ms.
fn natives() -> Vec<Value> {
    struct Flag(Arc<AtomicBool>);
    impl Cont for Flag {
        fn resume(&mut self, _: &mut Task<'_>, _: Value) -> Result<Next, Stop> {
            Ok(match self.0.load(Ordering::SeqCst) {
                true => Next::Done(Value::str("woken")),
                false => Next::Park(Wake::default()),
            })
        }
    }
    fn wake_early(t: &mut Task<'_>, _: Vec<Value>) -> Result<Flow, Stop> {
        let me = t.waiter.clone();
        me.take_woken();
        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        std::thread::spawn(move || {
            f.store(true, Ordering::SeqCst);
            me.wake();
        })
        .join()
        .unwrap();
        Ok(Flow::Park(Box::new(Flag(flag)), Wake::default()))
    }
    fn slow_job(t: &mut Task<'_>, _: Vec<Value>) -> Result<Flow, Stop> {
        nat::offload(
            t,
            || std::thread::sleep(Duration::from_millis(50)),
            |_, ()| Ok(Value::Nil),
        )
    }
    let mut v = probes();
    v.push(Value::Native(Native::leak(
        "this_thread",
        vec![],
        |_: &mut Task, _| Ok(Value::str(&id())),
    )));
    v.push(Value::Native(Native::leak_step(
        "wake_early",
        vec![],
        wake_early,
    )));
    v.push(Value::Native(Native::leak_step(
        "slow_job",
        vec![],
        slow_job,
    )));
    v
}

fn run(src: &str) -> crate::testutil::Outcome {
    exec(
        src,
        Opts {
            extra: natives,
            ..Opts::default()
        },
    )
}

/// A thousand tasks all run on the thread that runs the entry.
#[test]
fn tasks_run_on_one_thread() {
    let o = run(
        "me = this_thread()\nprint(scope(fn(s):\n  ts = map(0..1000, _ => spawn(s, fn(): sleep(1ms); this_thread() ;;))\n  all(map(ts, await), x => x == me)\n;;))",
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("true\n", ""));
}

/// A wake that came before the task decided to park is not lost.
#[test]
fn a_wake_from_another_thread_before_the_park_is_kept() {
    let start = Instant::now();
    let o = run("print(wake_early())");
    assert_eq!((o.out.as_str(), o.err.as_str()), ("woken\n", ""));
    assert!(start.elapsed() < Duration::from_secs(5));
}

/// The scheduler judges a deadlock exactly when every task is parked and
/// none is one a timer or the world outside ends.
#[test]
fn deadlock_is_judged_by_the_scheduler() {
    let o = run("(_tx, rx) = channel()\nrecv(rx)");
    assert!(o.err.starts_with(DEADLOCK), "{}", o.err);
    for src in [
        // A sleeper: a timer ends it.
        "(_tx, rx) = channel()\nscope(fn(s):\n  spawn(s, () => sleep(20ms))\n  _ = timeout(40ms, () => recv(rx))\n  nil\n;;)\nprint(\"ok\")",
        // An offload's job: the world outside ends it.
        "scope(fn(s):\n  t = spawn(s, () => slow_job())\n  await(t)\n;;)\nprint(\"ok\")",
    ] {
        let o = run(src);
        assert_eq!((o.out.as_str(), o.err.as_str()), ("ok\n", ""), "{src}");
    }
}

/// A stop request made while nothing runs waits for the next run: the
/// entry is cancelled at its first call and the exit code is 130.
#[test]
fn a_stop_while_idle_is_kept_for_the_next_run() {
    let s = setup("print(\"started\")\nfn f() = f()\nf()", Opts::default())
        .unwrap_or_else(|e| panic!("{e:?}"));
    s.rt.stop_handle().deliver(2);
    let r = s.rt.run();
    assert!(
        matches!(&r, Err(Stop::Error(d)) if d.code == "E0701"),
        "{r:?}"
    );
    assert_eq!(s.rt.exit_code(), Some(130));
    assert_eq!(s.out.text(), "");
}

/// A deadline of a wait that ended early does not hold back the deadlock
/// report until it passes.
#[test]
fn a_stale_deadline_does_not_delay_a_deadlock() {
    let start = Instant::now();
    let o = run(
        "(tx, rx) = channel()\nr = scope(fn(s):\n  spawn(s, () => send(tx, 1))\n  timeout(1h, () => recv(rx))\n;;)\nprint(r)\n(_tx2, rx2) = channel()\nrecv(rx2)",
    );
    assert_eq!(o.out, "Ok(1)\n");
    assert!(o.err.starts_with(DEADLOCK), "{}", o.err);
    assert!(start.elapsed() < Duration::from_secs(5));
}

/// The deadlines of waits that ended early do not pile up.
#[test]
fn stale_deadlines_do_not_pile_up() {
    let s = setup(
        "(tx, rx) = channel()\nscope(fn(s):\n  spawn(s, fn():\n    for i in 0..5000: send(tx, i) ;;\n  ;;)\n  for i in 0..5000: timeout(1h, () => recv(rx)) ;;\n;;)\n(_tx2, rx2) = channel()\nrecv(rx2)",
        Opts::default(),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    let _ = s.rt.run();
    let n = s.rt.shared().timers_probe.load(Ordering::SeqCst);
    assert!(n < 200, "{n} deadlines kept");
}

/// A swap whose function runs long is not starved by a task that swaps
/// the same atom on every turn: the function runs to its end in one turn.
#[test]
fn a_long_swap_is_not_starved() {
    let o = run(
        "a = atom(0)\ndone = atom(false)\nfn slow(n, k):\n  if k == 0: n else: slow(n, k - 1) ;;\n;;\nr = timeout(5s, () => scope(fn(s):\n  spawn(s, fn():\n    fn loop(i):\n      swap(a, n => n + 1)\n      if deref(done): i else: loop(i + 1) ;;\n    ;;\n    loop(0)\n  ;;)\n  swap(a, n => slow(n, 5000))\n  reset(done, true)\n  \"slow done\"\n;;))\nprint(r)",
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("slow done\n", ""));
}

fn job_opts() -> Opts {
    Opts {
        extra: crate::testutil::jobs::job_natives,
        ..Opts::default()
    }
}

/// A task whose job waits in the pool's queue is not deadlocked.
#[test]
fn a_task_waiting_in_the_pool_queue_is_not_deadlocked() {
    use crate::testutil::jobs;
    let o = crate::testutil::exec_prepared(
        "print(scope(fn(s):\n  a = spawn(s, () => block_job(\"q1\"))\n  b = spawn(s, () => block_job(\"q1\"))\n  [await(a), await(b)]\n;;))",
        job_opts(),
        |rt| {
            let sh = rt.shared();
            sh.pool.set_limits(1, Duration::from_secs(10));
            std::thread::spawn(move || {
                if jobs::until(|| sh.pool.queued() == 1 && sh.pool.idle() == 0) {
                    jobs::mark("q1-seen");
                }
                jobs::open("q1");
            });
        },
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        ("[\"done\", \"done\"]\n", "")
    );
    assert_eq!(crate::testutil::jobs::count("q1-seen"), 1);
}

/// A job that is waited for is still dropped while it waits in the
/// queue: the deadline ends the call at once and the job never runs.
#[test]
fn a_queued_wait_job_is_dropped_on_cancel() {
    let s = setup(
        &format!(
            "{}print(scope(fn(s):\n  spawn(s, () => block_job(\"q2\"))\n  until(() => pool_state() == [1, 0, 0])\n  r = timeout(20ms, () => count_job(\"q2c\"))\n  open_latch(\"q2\")\n  r\n;;))",
            crate::testutil::UNTIL
        ),
        job_opts(),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    let pool = &s.rt.shared().pool;
    pool.set_limits(1, Duration::from_secs(10));
    s.rt.run().unwrap();
    assert_eq!(
        s.out.text(),
        "Err({kind: 'timeout, message: \"timed out after 20ms\"})\n"
    );
    assert!(crate::testutil::jobs::until(
        || pool.queued() == 0 && pool.idle() == 1
    ));
    assert_eq!(crate::testutil::jobs::count("q2c"), 0);
}

/// A run cannot start inside a run (it would drop the outer run's queued
/// wakes and take its running task's state), in release builds too: the
/// panic fence reports it as E0801, and once the outer run has ended a
/// new one may start.
#[test]
fn a_nested_run_root_is_refused() {
    fn natives() -> Vec<Value> {
        vec![Value::Native(Native::leak(
            "nested",
            vec![],
            |t: &mut Task, _| t.run_root(crate::vm::Todo::Start(Value::Nil, 0)),
        ))]
    }
    let s = setup(
        "nested()",
        Opts {
            extra: natives,
            ..Opts::default()
        },
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    let r = s.rt.run();
    match &r {
        Err(Stop::Error(d)) if d.code == "E0801" => {
            let text = format!("{d:?}");
            assert!(text.contains("run_root during a run"), "{text}");
        }
        r => panic!("{r:?}"),
    }
    // The outer run ended: another may start.
    assert!(!s.rt.world().in_run);
}

/// Runs `src` with the job natives; once `settle_mark(name)` ran and
/// `parked` tasks are parked after it, a test thread opens latch `name`.
/// Bounded: (stdout, stderr, the run's result).
fn open_when_parked(
    src: &str,
    name: &'static str,
    parked: usize,
) -> (String, String, Result<Value, Stop>) {
    use crate::testutil::jobs;
    let src = src.to_string();
    let (shared_tx, shared_rx) = std::sync::mpsc::channel();
    let (tx, rx) = std::sync::mpsc::channel();
    // The runtime stays on the thread that makes it (`!Send`).
    std::thread::spawn(move || {
        let s = setup(&src, job_opts()).unwrap_or_else(|e| panic!("{e:?}"));
        let _ = shared_tx.send(s.rt.shared());
        let r = s.rt.run();
        let _ = tx.send((s.out.text(), s.err.text(), format!("{r:?}")));
    });
    let sh = shared_rx.recv().unwrap();
    std::thread::spawn(move || {
        let seen = jobs::until(|| {
            jobs::count(name) == 1 && sh.parked_probe.load(Ordering::SeqCst) == parked
        });
        assert!(seen, "{name}: the state never came");
        jobs::open(name);
    });
    let (out, err, r) = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the run hung");
    (
        out,
        err,
        if r.starts_with("Ok") {
            Ok(Value::Nil)
        } else {
            Err(Stop::Exit(0))
        },
    )
}

/// A cancel while a job that is waited for runs: the task waits for the
/// job, then unwinds; the job's wake after the cancel's is not lost.
#[test]
fn a_cancel_while_a_waited_job_runs_unwinds_after_it() {
    let (out, err, _) = open_when_parked(
        &format!(
            "{}print(scope(fn(s):\n  spawn(s, () => block_job(\"cw\"))\n  until(() => pool_state() == [1, 0, 0])\n  settle_mark(\"cw\")\n  cancel(s)\n;;))",
            crate::testutil::UNTIL
        ),
        "cw",
        2,
    );
    assert_eq!(
        (out.as_str(), err.as_str()),
        ("Err({kind: 'cancelled, message: \"cancelled\"})\n", "")
    );
}

/// The same for `exit`: the program ends with its code.
#[test]
fn an_exit_while_a_waited_job_runs_ends_after_it() {
    let (out, err, r) = open_when_parked(
        &format!(
            "{}scope(fn(s):\n  spawn(s, () => block_job(\"ew\"))\n  until(() => pool_state() == [1, 0, 0])\n  settle_mark(\"ew\")\n  exit(3)\n;;)",
            crate::testutil::UNTIL
        ),
        "ew",
        2,
    );
    assert_eq!((out.as_str(), err.as_str()), ("", ""));
    assert!(r.is_err());
}

fn run_jobs(src: &str) -> crate::testutil::Outcome {
    exec(&format!("{}{src}", crate::testutil::UNTIL), job_opts())
}

/// A task parked on a descriptor wakes when it is ready; no thread waits
/// for it (no offload).
#[test]
fn a_task_parked_on_io_wakes_when_ready() {
    let o = run_jobs(
        "fd = pipe_pair(\"pr\")\nprint(scope(fn(s):\n  t = spawn(s, fn(): r = pipe_read(fd); [r, parks() > 0, offloads()] ;;)\n  until(() => waiting_io() == 1)\n  pipe_put(\"pr\", \"x\")\n  await(t)\n;;))",
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("[\"x\", true, 0]\n", ""));
}

/// A wake from another thread while the run thread sleeps in the event
/// loop ends the sleep at once.
#[test]
fn a_push_while_the_scheduler_sleeps_wakes_it() {
    use crate::testutil::jobs;
    let s = setup("print(block_job(\"ps\"))", job_opts()).unwrap_or_else(|e| panic!("{e:?}"));
    let sh = s.rt.shared();
    std::thread::spawn(move || {
        jobs::until(|| sh.parked_probe.load(Ordering::SeqCst) == 1);
        jobs::open("ps");
    });
    let start = Instant::now();
    s.rt.run().unwrap();
    assert_eq!(s.out.text(), "done\n");
    assert!(start.elapsed() < Duration::from_secs(5));
}

/// A wait that ended (here at its deadline) leaves no registration: data
/// that comes later wakes nobody, and the next reader gets it.
#[test]
fn a_stale_token_is_ignored() {
    let o = run_jobs(
        "fd = pipe_pair(\"st\")\nprint(timeout(10ms, () => pipe_read(fd)))\nprint(waiting_io())\npipe_put(\"st\", \"y\")\nprint(pipe_read(fd))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        (
            "Err({kind: 'timeout, message: \"timed out after 10ms\"})\n0\ny\n",
            ""
        )
    );
}

/// The loop is seen between the turns of a computing task: a reader wakes
/// while the computation goes on.
#[test]
fn io_is_seen_between_turns_of_a_computing_task() {
    use crate::testutil::jobs;
    let s = setup(
        &format!(
            "{}fd = pipe_pair(\"cp\")\nfn fib(n) = if n < 2: n else: fib(n - 1) + fib(n - 2) ;;\nscope(fn(s):\n  spawn(s, fn(): print(pipe_read(fd)) ;;)\n  spawn(s, fn():\n    until(() => waiting_io() == 1)\n    settle_mark(\"cp\")\n    fib(30)\n    print(\"fib done\")\n  ;;)\n;;)",
            crate::testutil::UNTIL
        ),
        job_opts(),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    std::thread::spawn(|| {
        jobs::until(|| jobs::count("cp") == 1);
        jobs::pipe_write("cp", "z");
    });
    s.rt.run().unwrap();
    assert_eq!(s.out.text(), "z\nfib done\n");
}

/// A task parked on a descriptor alone is not a deadlock: the world
/// outside may make it ready.
#[test]
fn a_task_parked_on_io_is_not_deadlocked() {
    use crate::testutil::jobs;
    let s = setup(
        "fd = pipe_pair(\"nd\")\nsettle_mark(\"nd\")\nprint(pipe_read(fd))",
        job_opts(),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    let sh = s.rt.shared();
    std::thread::spawn(move || {
        jobs::until(|| jobs::count("nd") == 1 && sh.parked_probe.load(Ordering::SeqCst) == 1);
        jobs::pipe_write("nd", "w");
    });
    s.rt.run().unwrap();
    assert_eq!(
        (s.out.text(), s.err.text()),
        ("w\n".to_string(), String::new())
    );
}

/// A task another thread woke (its pool job ended) runs beside a task
/// that keeps computing, not after it.
#[test]
fn a_task_woken_from_another_thread_runs_beside_a_computing_task() {
    use crate::testutil::jobs;
    let s = setup(
        &format!(
            "{}fn fib(n) = if n < 2: n else: fib(n - 1) + fib(n - 2) ;;\nscope(fn(s):\n  spawn(s, fn(): print(block_job(\"wb\")) ;;)\n  spawn(s, fn():\n    until(() => pool_state()[0] - pool_state()[1] == 1)\n    settle_mark(\"wb\")\n    fib(30)\n    print(\"fib done\")\n  ;;)\n;;)",
            crate::testutil::UNTIL
        ),
        job_opts(),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    std::thread::spawn(|| {
        jobs::until(|| jobs::count("wb") == 1);
        jobs::open("wb");
    });
    s.rt.run().unwrap();
    assert_eq!(s.out.text(), "done\nfib done\n");
}

/// Channel traffic next to a task parked on a descriptor does not look
/// at the event loop every round (spec 15: when a turn is given, and
/// now and then): two tasks pass 10,000 values while a third waits on a
/// pipe.
#[test]
fn channel_traffic_beside_io_does_not_poll_every_round() {
    let s = setup(
        "fd = pipe_pair(\"cp2\")\nscope(fn(s):\n  r = spawn(s, () => pipe_read(fd))\n  (tx, rx) = channel()\n  spawn(s, fn(): each(0..10000, i => send(tx, i)); close(tx) ;;)\n  print(fold(rx, 0, (a, x) => a + x))\n  pipe_put(\"cp2\", \"x\")\n  await(r)\n;;)",
        job_opts(),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    s.rt.run().unwrap();
    assert_eq!(s.out.text(), "49995000\n");
    let polls = s.rt.shared().io_polls.load(Ordering::SeqCst);
    assert!(polls < 1000, "{polls} loop looks");
}

/// Two Ctrl-Cs while nothing runs (the REPL between inputs) abandon
/// nothing: the next input runs.
#[test]
fn interrupts_while_idle_do_not_abandon_the_next_run() {
    let s = setup("print(\"hi\")", Opts::default()).unwrap_or_else(|e| panic!("{e:?}"));
    let h = s.rt.stop_handle();
    h.interrupt();
    h.interrupt();
    s.rt.recover_interrupt();
    s.rt.take_abandoned();
    assert!(s.rt.run().is_ok());
    assert_eq!(s.out.text(), "hi\n");
}

/// A runtime that goes ends its pool's idle threads (`fung test` makes
/// one runtime per file). A task alone calls `exists` on the run thread
/// (P8), so the job is a listing.
#[test]
fn a_dropped_runtime_ends_its_pool_threads() {
    let s = setup(
        "import fs\nprint(len(fs.list(\"/\")!) > 0)",
        Opts::default(),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    s.rt.run().unwrap();
    let pool = s.rt.shared().pool.clone();
    assert!(crate::testutil::jobs::until(|| pool.idle() == 1));
    drop(s);
    assert!(crate::testutil::jobs::until(|| pool.threads() == 0));
}

/// A job queued from a `release` that a second Ctrl-C abandons never
/// runs: the abandoned input's jobs that have not started go with it
/// (one that is running finishes).
#[test]
fn an_abandoned_inputs_queued_pool_jobs_do_not_run() {
    use crate::testutil::jobs;
    let s = setup(
        &format!(
            "{}scope(fn(s):\n  spawn(s, () => block_job(\"q3\"))\n  until(() => pool_state() == [1, 0, 0])\n  with(() => nil, fn(_): settle_mark(\"q3-body\"); sleep(60s) ;;, fn(_): count_job(\"q3c\") ;;)\n;;)",
            crate::testutil::UNTIL
        ),
        job_opts(),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    let sh = s.rt.shared().clone();
    sh.pool.set_limits(1, Duration::from_secs(10));
    let h = s.rt.stop_handle();
    let t = std::thread::spawn(move || {
        assert!(jobs::until(|| jobs::count("q3-body") == 1));
        h.interrupt();
        assert!(jobs::until(|| sh.pool.queued() == 1));
        h.interrupt();
    });
    let _ = s.rt.run();
    t.join().unwrap();
    assert!(s.rt.take_abandoned());
    jobs::open("q3");
    let pool = s.rt.shared().pool.clone();
    assert!(jobs::until(|| pool.queued() == 0 && pool.idle() == 1));
    assert_eq!(jobs::count("q3c"), 0);
}

// --- the deadlock report (review C7, decided 2026-10-20) ---

/// The report shows where the entry waits, as a runtime error does (its
/// line and call path), then the other tasks: one line per kind of wait
/// and place, with how many wait there.
#[test]
fn a_deadlock_report_shows_where_the_tasks_wait() {
    let o = exec(
        "fn worker(rx) = recv(rx)\nfn main() = scope(fn(s):\n  (_tx, rx) = channel()\n  (_tx2, rx2) = channel()\n  a = spawn(s, () => worker(rx))\n  spawn(s, () => worker(rx2))\n  await(a)\n;;)\nmain()",
        Opts::default(),
    );
    assert_eq!(
        o.err,
        "error[E0702]: deadlock: all tasks are waiting\n  --> -e:7:8\n  |\n7 |   await(a)\n  |        ^\n  |\n call path (innermost first):\n   -e:7       <lambda>\n   -e:2       main\n   -e:9       <entry>\n 2 other tasks are waiting:\n   recv at -e:1:21 (worker) ×2\n"
    );
    assert_eq!(o.exit, 1);
}

/// Each line says what the task waits in: inside a built-in module's
/// function, the one the user called (and its place in the user's code);
/// inside a `with`'s release, that too. The lines keep the order the
/// tasks were spawned in.
#[test]
fn a_deadlock_report_names_the_function_and_the_release() {
    let o = exec(
        "(_tx, rx) = channel()\nscope(fn(s):\n  spawn(s, fn(): take(rx, 1); nil ;;)\n  spawn(s, () => with(() => nil, _ => nil, _ => recv(rx)))\n  spawn(s, fn(): (tx, _rx) = channel(); send(tx, 1) ;;)\n  spawn(s, () => take(rx, 1))\n;;)",
        Opts::default(),
    );
    let (_, others) = o
        .err
        .split_once(" 4 other tasks are waiting:\n")
        .unwrap_or_else(|| panic!("{}", o.err));
    let lines: Vec<&str> = others.lines().collect();
    assert_eq!(
        lines[..3],
        // `rx`'s sending end is a global, which every task holds (L7).
        [
            "   recv in take at -e:3:22 (<lambda>, holds its own sending end)",
            "   recv at -e:4:53 (<lambda>, in release, holds its own sending end)",
            "   send at -e:5:45 (<lambda>)",
        ],
        "{}",
        o.err
    );
    // The lambda tail-called `take`: the frame it left behind gives the
    // place and the name (L8).
    assert_eq!(
        lines[3], "   recv in take at -e:6:22 (<lambda>)",
        "{}",
        o.err
    );
}

/// A task a built-in module spawned on a lambda of its own has no named
/// function to report: the line names the module alone (`scope in conc`),
/// not an empty member (`conc.`).
#[test]
fn a_deadlock_report_names_the_module_of_its_own_task() {
    let o = exec(
        "import conc\n(_tx, rx) = channel()\nscope(fn(s):\n  out = conc.fan_in(s, [rx])\n  print(recv(out))\n;;)",
        Opts::default(),
    );
    assert!(
        o.err.contains("   scope in conc at extras/conc.fung:"),
        "{}",
        o.err
    );
    assert!(!o.err.contains("conc. at"), "{}", o.err);
}

/// A task that waits to receive while it holds that channel's sending
/// end itself can never be woken: the report says so (review L7), for a
/// variable of a function, a global, and another task's line.
#[test]
fn a_deadlock_report_names_a_sending_end_the_receiver_holds() {
    let o = exec(
        // `_held` is never read, so it lives on (03: 값의 수명).
        "fn main() = scope(fn(s):\n  (tx, rx) = channel()\n  _held = tx\n  spawn(s, () => each(0..3, i => send(tx, i)))\n  for x in rx: x * 10 ;;\n;;)\nmain()",
        Opts::default(),
    );
    assert!(
        o.err.contains(
            " help: this task waits to receive, but it still holds that channel's sending end itself (a variable of `<lambda>`), so the channel cannot close: close it with `close` first, or leave the sending end to the producers only\n"
        ),
        "{}",
        o.err
    );
    let o = exec("(tx, rx) = channel()\nfor x in rx: x ;;", Opts::default());
    assert!(
        o.err.contains("sending end itself (the global `tx`)"),
        "{}",
        o.err
    );
    let o = exec(
        "scope(fn(s):\n  t = spawn(s, fn(): (_tx, rx) = channel(); recv(rx) ;;)\n  await(t)\n;;)",
        Opts::default(),
    );
    assert!(
        o.err
            .contains("   recv at -e:2:49 (<lambda>, holds its own sending end)\n"),
        "{}",
        o.err
    );
    // A receiver whose sending end another task's variable holds gets no
    // hint (a global is every task's).
    let o = exec(
        "fn worker(rx) = recv(rx)\nfn main() = scope(fn(s):\n  (_tx, rx) = channel()\n  a = spawn(s, () => worker(rx))\n  await(a)\n;;)\nmain()",
        Opts::default(),
    );
    assert!(!o.err.contains("sending end"), "{}", o.err);
}

/// At most 10 lines; the tasks past them are counted.
#[test]
fn a_deadlock_report_lists_ten_places_at_most() {
    let fns: String = (0..12)
        .map(|i| format!("fn w{i}(rx) = recv(rx)\n"))
        .collect();
    let spawns: String = (0..12)
        .map(|i| format!("  spawn(s, () => w{i}(rx))\n"))
        .collect();
    let o = exec(
        &format!(
            "{fns}(_tx, rx) = channel()\nscope(fn(s):\n  each(0..5, _ => spawn(s, () => w0(rx)))\n{spawns};;)"
        ),
        Opts::default(),
    );
    let others = o
        .err
        .split_once(" 17 other tasks are waiting:\n")
        .map(|(_, r)| r);
    // The sending end is a global, which every task holds (L7).
    let mut want: String = "   recv at -e:1:17 (w0, holds its own sending end) ×6\n".into();
    for i in 1..10 {
        want += &format!(
            "   recv at -e:{}:17 (w{i}, holds its own sending end)\n",
            i + 1
        );
    }
    want += "   … 2 more\n";
    assert_eq!(others, Some(want.as_str()), "{}", o.err);
}

/// L14: a task is in the ready queue at most once, so it gets one turn a
/// round. A gives its turn (a long loop) after a wake of its own waiter
/// (a stale wake); B, next in the round, sees the queue.
#[test]
fn a_task_is_queued_once() {
    let o = exec_waiting(
        "fn spin(n) = n == 0 then 0 else spin(n - 1) + 0\nscope(fn(s):\n  a = spawn(s, fn():\n    wake_self()\n    spin(3000)\n  ;;)\n  b = spawn(s, () => queue_has_dupes())\n  print(await(b), await(a))\n;;)",
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("false 0\n", ""));
}

fn seed_1(rt: &crate::vm::Runtime) {
    rt.shuffle_schedule(1);
}

fn seed_2(rt: &crate::vm::Runtime) {
    rt.shuffle_schedule(2);
}

fn seed_3(rt: &crate::vm::Runtime) {
    rt.shuffle_schedule(3);
}

/// L14: with a seed (`fung test`), the tasks started or woken in a round
/// run in an order drawn from it: the same seed, the same order; without
/// one, the order they were spawned in.
#[test]
fn the_schedule_shuffles_by_its_seed() {
    let src = "scope(fn(s): each(0..8, i => spawn(s, () => print(i))) ;;)";
    let plain = exec(src, Opts::default()).out;
    assert_eq!(plain, "0\n1\n2\n3\n4\n5\n6\n7\n");
    let run =
        |p: fn(&crate::vm::Runtime)| crate::testutil::exec_prepared(src, Opts::default(), p).out;
    assert_eq!(run(seed_1), run(seed_1));
    let orders = [run(seed_1), run(seed_2), run(seed_3)];
    assert!(orders.iter().any(|o| *o != plain), "{orders:?}");
    for o in &orders {
        let mut lines: Vec<&str> = o.lines().collect();
        lines.sort_unstable();
        assert_eq!(lines, ["0", "1", "2", "3", "4", "5", "6", "7"]);
    }
}
