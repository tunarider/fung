//! A cancelled task's command (concurrency v2, L6): the command's
//! process group gets SIGTERM, and SIGKILL 2 s later if it is still
//! there. Each program waits for a file the command writes once it has
//! started, so the cancellation never comes before the command runs.

use std::time::{Duration, Instant};

use crate::testutil::{run, temp_dir};

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
    std::process::Command::new("kill")
        .args(["-0", pid.trim()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[test]
fn a_cancelled_command_is_terminated() {
    own_group();
    let start = Instant::now();
    assert_eq!(
        run("print(timeout(100ms, () => sh(\"sleep 5; echo no\")))"),
        ("Err({kind: 'timeout})\n".into(), "".into(), 0)
    );
    assert!(
        start.elapsed() < Duration::from_secs(2),
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
        start.elapsed() < Duration::from_secs(2),
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
