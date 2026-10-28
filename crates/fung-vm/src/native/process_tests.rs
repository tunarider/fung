//! A cancelled task's command (concurrency v2, L6): the command's
//! process group gets SIGTERM, and SIGKILL 2 s later if it is still
//! there. Each program waits for a file the command writes once it has
//! started, so the cancellation never comes before the command runs.

use std::time::{Duration, Instant};

use crate::testutil::{child, run, temp_dir};

/// The tests here check what a group of its own gives (a cancel ends the
/// command's children too): force it, whether or not `cargo test` runs
/// in the foreground of a terminal (then commands share fung's group).
fn own_group() {
    super::force_own_group();
}

/// `wait_file(p)`: sleeps 5 ms at a time until `p` exists.
const WAIT_FILE: &str =
    "import fs\nfn wait_file(p) = fs.exists(p) then nil else fn(): sleep(5ms); wait_file(p) ;;()\n";

/// Whether process `pid` still exists.
fn alive(pid: &str) -> bool {
    child::status(
        std::process::Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(std::process::Stdio::null()),
    )
    .is_ok_and(|s| s.success())
}

/// The bound is above `TERM_GRACE`, as in
/// `an_exit_terminates_running_commands`.
#[test]
fn a_cancelled_command_is_terminated() {
    own_group();
    let start = Instant::now();
    assert_eq!(
        run("print(timeout(100ms, () => sh(\"sleep 5; echo no\")))"),
        (
            "Err({kind: 'timeout, message: \"timed out after 100ms\"})\n".into(),
            "".into(),
            0
        )
    );
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
}

/// What the shell started goes too: the group is terminated.
#[test]
fn the_whole_group_is_terminated() {
    own_group();
    let d = temp_dir();
    let f = d.join("pid");
    let src = format!(
        "{WAIT_FILE}scope(fn(s):\n  t = spawn(s, () => sh(\"sleep 30 & echo $! > {f}.tmp; mv {f}.tmp {f}; wait\"))\n  wait_file(\"{f}\")\n  cancel(t)\n;;)\nprint(\"cancelled\")",
        f = f.display()
    );
    let start = Instant::now();
    assert_eq!(run(&src), ("cancelled\n".into(), "".into(), 0));
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
    let pid = std::fs::read_to_string(&f).unwrap();
    let start = Instant::now();
    while alive(&pid) {
        assert!(start.elapsed() < Duration::from_secs(3), "{pid} lives on");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A command that ignores SIGTERM is killed 2 s later.
#[test]
fn a_command_ignoring_term_is_killed() {
    own_group();
    let d = temp_dir();
    let f = d.join("ready");
    let src = format!(
        "{WAIT_FILE}scope(fn(s):\n  t = spawn(s, () => sh(\"trap '' TERM; touch {f}; sleep 30\"))\n  wait_file(\"{f}\")\n  cancel(t)\n;;)\nprint(\"cancelled\")",
        f = f.display()
    );
    let start = Instant::now();
    assert_eq!(run(&src), ("cancelled\n".into(), "".into(), 0));
    let took = start.elapsed();
    assert!(
        took >= Duration::from_secs(2) && took < Duration::from_secs(5),
        "{took:?}"
    );
}

/// The bound is above `TERM_GRACE` (2 s): a SIGTERM that comes while the
/// shell is starting `sleep` may miss the new child, which then holds
/// the output until the group's SIGKILL (seen once on a CI runner).
#[test]
fn an_exit_terminates_running_commands() {
    own_group();
    let d = temp_dir();
    let f = d.join("ready");
    let src = format!(
        "{WAIT_FILE}scope(fn(s):\n  spawn(s, () => sh(\"touch {f}; sleep 30\"))\n  wait_file(\"{f}\")\n  exit(3)\n;;)",
        f = f.display()
    );
    let start = Instant::now();
    assert_eq!(run(&src).2, 3);
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn an_uncancelled_command_still_gives_its_output() {
    assert_eq!(
        run("r = sh(\"echo hi; echo err >&2; exit 4\")\nprint(sh(\"echo hi\")!.out, r)"),
        (
            "hi\n Err({kind: 'exit, code: 4, out: \"hi\\n\", err: \"err\\n\", message: \"`/bin/sh` exited with code 4\"})\n".into(),
            "".into(),
            0
        )
    );
}

/// The command's own process ended but a child it left in the background
/// still holds its output: a cancel ends that child too, at once.
#[test]
fn a_cancel_after_the_command_itself_ended_ends_what_it_left() {
    own_group();
    let d = temp_dir();
    let f = d.join("pid");
    let src = format!(
        "{WAIT_FILE}scope(fn(s):\n  t = spawn(s, () => sh(\"sleep 30 & echo $! > {f}.tmp; mv {f}.tmp {f}; echo hi\"))\n  wait_file(\"{f}\")\n  cancel(t)\n;;)\nprint(\"cancelled\")",
        f = f.display()
    );
    let start = Instant::now();
    assert_eq!(run(&src), ("cancelled\n".into(), "".into(), 0));
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
    let pid = std::fs::read_to_string(&f).unwrap();
    let start = Instant::now();
    while alive(&pid) {
        assert!(start.elapsed() < Duration::from_secs(3), "{pid} lives on");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// `exec` and `sh` hand the wait for the command off (R1a).
#[test]
fn exec_parks_on_the_loop() {
    for call in ["exec([\"true\"])", "sh(\"true\")"] {
        let o = crate::testutil::exec_waiting(&format!(
            "n = offloads()\nprint({call})\nprint(offloads() - n)"
        ));
        assert_eq!(
            (o.out.as_str(), o.err.as_str()),
            ("Ok({out: \"\", err: \"\", code: 0})\n0\n", ""),
            "{call}"
        );
    }
}

/// A command that writes much to both its output and its error while it
/// reads much input does not stop: the three pipes are watched at once.
#[test]
fn a_command_with_much_input_and_output_on_both_streams() {
    let start = Instant::now();
    assert_eq!(
        run(
            "s = repeat(\"x\", 1000000)\nr = sh(\"tee /dev/stderr\", stdin: s)!\nprint(len(r.out), len(r.err), r.out == s)"
        ),
        ("1000000 1000000 true\n".into(), "".into(), 0)
    );
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
}

/// What a command left in the background keeps its output open: it is
/// read until that closes too.
#[test]
fn a_grandchild_holding_the_output_is_waited_for() {
    assert_eq!(
        run("print(sh(\"(sleep 0.2; echo late) & echo now\")!.out)"),
        ("now\nlate\n\n".into(), "".into(), 0)
    );
}

/// Without pidfd (an old Linux kernel), a pool job waits for the
/// command's end: the same result.
#[cfg(target_os = "linux")]
#[test]
fn a_pool_wait_ends_a_command_without_pidfd() {
    let o = crate::testutil::exec_prepared(
        "print(exec([\"sh\", \"-c\", \"exit 3\"]))",
        crate::testutil::Opts::default(),
        |rt| rt.shared().poller.disable_pidfd(),
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        (
            "Err({kind: 'exit, code: 3, out: \"\", err: \"\", message: \"`sh` exited with code 3\"})\n",
            ""
        )
    );
}

/// Without pidfd and without a signal thread to take SIGCHLD (a test
/// process), the task looks on a short timer until the command has ended
/// (here it closes its output first, so no pipe wakes the task when it
/// ends). No pool thread waits for it (L11).
#[cfg(target_os = "linux")]
#[test]
fn without_pidfd_a_timer_still_sees_the_end() {
    let o = crate::testutil::exec_prepared(
        "print(timeout(5s, () => sh(\"exec >/dev/null 2>&1; sleep 0.1\")!.code))",
        crate::testutil::Opts::default(),
        |rt| {
            rt.shared().poller.disable_pidfd();
            rt.shared().pool.fail_spawns(true);
        },
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("0\n", ""));
}

/// Without pidfd, many commands at once end without a pool thread each:
/// 70 commands (more than the pool's 64 threads) all end, and the pool
/// made no thread (L11 D: the pool's `waitid` per command is gone).
#[cfg(target_os = "linux")]
#[test]
fn without_pidfd_commands_take_no_pool_thread() {
    let o = crate::testutil::exec_prepared(
        "codes = scope(s => map(1..=70, _ => spawn(s, () => sh(\"sleep 0.05\")!.code)) |> map(await))\nprint(sum(codes), len(codes), pool_state()[0])",
        crate::testutil::Opts {
            extra: crate::testutil::jobs::job_natives,
            ..crate::testutil::Opts::default()
        },
        |rt| rt.shared().poller.disable_pidfd(),
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("0 70 0\n", ""));
}

/// A cancel while what the command left ignores SIGTERM and holds its
/// output: the group gets SIGKILL after the grace time, and the call
/// ends then (it does not wait for the grandchild, nor spin meanwhile).
#[test]
fn a_cancel_is_bounded_when_a_grandchild_ignores_term() {
    own_group();
    let start = Instant::now();
    assert_eq!(
        run("print(timeout(300ms, () => sh(\"(trap '' TERM; exec sleep 8) & exec sleep 60\")))"),
        (
            "Err({kind: 'timeout, message: \"timed out after 300ms\"})\n".into(),
            "".into(),
            0
        )
    );
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
}

/// A command whose output comes fast does not hold the run thread: the
/// task reading it moves at most about `PUMP_BUDGET` bytes per look and
/// parks again, so the other tasks get turns while the output is read.
/// Judged by state, not time (a ticker on `sleep` was late on a busy CI
/// runner): 300 MB moved at most 256 KiB plus one 16 KiB read at a time
/// is at least about 1,077 parks, where a reader that drained the pipe
/// until empty parked once or twice (Ubuntu 26.04's fast `head`). The
/// ticker computes without sleeping and gets a turn at each of them.
#[test]
fn a_fast_command_does_not_starve_other_tasks() {
    let o = crate::testutil::exec_waiting(
        "n = atom(0)\ndone = atom(false)\nparked = scope(fn(s):\n  spawn(s, fn():\n    fn lp() = if deref(done): nil else: fn(): swap(n, x => x + 1); lp() ;;() ;;\n    lp()\n  ;;)\n  p0 = parks()\n  _ = sh(\"head -c 300000000 /dev/zero\")\n  p1 = parks()\n  reset(done, true)\n  p1 - p0\n;;)\nprint(parked, deref(n))",
    );
    assert_eq!((o.err.as_str(), o.code.as_str()), ("", ""), "{}", o.out);
    let got: Vec<i64> = o
        .out
        .split_whitespace()
        .map(|w| w.parse().unwrap())
        .collect();
    let (parks, ticks) = (got[0], got[1]);
    assert!(parks >= 1000, "{parks} parks while reading 300 MB");
    assert!(ticks >= parks, "{ticks} ticks over {parks} parks");
}

/// A registration that fails while a cancelled command is being ended
/// (the loop refuses its output pipe after taking its end) ends the call
/// with E0801 and leaves nothing registered.
#[test]
fn a_failed_registration_after_a_cancel_leaves_nothing_registered() {
    own_group();
    let o = crate::testutil::exec(
        "fn kind(r) = match r:\n  Err(e) => e.kind\n  v => v\n;;\nsupervise(fn(sc):\n  t = spawn(sc, fn():\n    fail_register_at(5)\n    timeout(100ms, () => sh(\"trap '' TERM; sleep 5\"))\n  ;;)\n  print(kind(await(t)))\n;;, e => nil)\nprint(waiting_io())",
        crate::testutil::Opts {
            extra: crate::testutil::jobs::job_natives,
            ..crate::testutil::Opts::default()
        },
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("'error\n0\n", ""));
}

/// A variable name holding `=` (or no name at all) cannot be set:
/// the system would read `HOME=z=x` as `HOME`. It is an argument error,
/// not a variable silently set under another name.
#[test]
fn an_env_name_with_an_equals_sign_is_refused() {
    for key in ["HOME=z", ""] {
        let (out, err, code) = run(&format!(
            "print(sh(\"echo $HOME\", env: {{\"{key}\": \"x\"}}))"
        ));
        assert_eq!((out.as_str(), code), ("", 1), "{key}: {err}");
        assert!(
            err.starts_with("error[E0403]: argument `env` must be a map of variable names"),
            "{key}: {err}"
        );
    }
    let (out, err, _) = run("print(sh(\"echo $A_B\", env: {A_B: \"ok\"})!.out)");
    assert_eq!(out, "ok\n\n", "{err}");
}
