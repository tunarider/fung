//! SIGINT and SIGTERM through the command (concurrency v2, L6): the
//! first signal is a stop request, the second ends the process at once.
//! Each child is a `fung` held by `Reaped`; a signal is sent only after
//! the program wrote the line the test waits for.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use common::{Reaped, fung_bin};

/// A running `fung` whose standard output is read line by line.
struct Live {
    child: Reaped,
    out: BufReader<ChildStdout>,
    seen: String,
    err: std::thread::JoinHandle<String>,
    done: Arc<AtomicBool>,
}

impl Live {
    fn start(args: &[&str], stdin: Stdio) -> Live {
        let mut child = Reaped::new(
            common::spawn(
                Command::new(fung_bin())
                    .args(args)
                    .stdin(stdin)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped()),
            )
            .unwrap(),
        );
        let out = BufReader::new(child.stdout.take().unwrap());
        let mut e = child.stderr.take().unwrap();
        let err = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = e.read_to_string(&mut s);
            s
        });
        // A watchdog: a child that never writes what the test waits for
        // is killed after 20 s, so the test fails instead of hanging.
        let done = Arc::new(AtomicBool::new(false));
        let (pid, d) = (child.id(), done.clone());
        std::thread::spawn(move || {
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(20) {
                if d.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = common::status(Command::new("kill").args(["-KILL", &pid.to_string()]));
        });
        Live {
            child,
            out,
            seen: String::new(),
            err,
            done,
        }
    }

    /// Reads standard output until a line equal to `line`.
    fn wait_for(&mut self, line: &str) {
        loop {
            let mut l = String::new();
            let n = self.out.read_line(&mut l).unwrap();
            assert!(n > 0, "no line `{line}`; got {:?}", self.seen);
            self.seen.push_str(&l);
            // (the REPL's prompts come before its output on a line)
            if l.trim_end().ends_with(line) {
                return;
            }
        }
    }

    fn signal(&self, sig: &str) {
        let st = common::status(
            Command::new("kill").args([format!("-{sig}"), self.child.id().to_string()]),
        )
        .unwrap();
        assert!(st.success());
    }

    /// The rest of standard output, standard error and the exit code.
    fn finish(mut self) -> (String, String, Option<i32>) {
        let mut rest = String::new();
        let _ = self.out.read_to_string(&mut rest);
        self.seen.push_str(&rest);
        let st = self.child.wait().unwrap();
        self.done.store(true, Ordering::SeqCst);
        (self.seen, self.err.join().unwrap(), st.code())
    }
}

fn run_e(src: &str) -> Live {
    Live::start(&["-e", src], Stdio::null())
}

/// Waits in `use` until cancelled (a sleep: a channel nobody sends on
/// would be a deadlock, reported before any signal comes).
const WAITS_IN_USE: &str =
    "with(() => nil, fn(_):\n  print(\"ready\")\n  sleep(60s)\n;;, _ => print(\"released\"))";

#[test]
fn term_without_a_watcher_runs_the_releases_and_ends_143() {
    let mut l = run_e(WAITS_IN_USE);
    l.wait_for("ready");
    l.signal("TERM");
    assert_eq!(
        l.finish(),
        ("ready\nreleased\n".into(), "".into(), Some(143))
    );
}

#[test]
fn int_without_a_watcher_ends_130() {
    let mut l = run_e(WAITS_IN_USE);
    l.wait_for("ready");
    l.signal("INT");
    assert_eq!(
        l.finish(),
        ("ready\nreleased\n".into(), "".into(), Some(130))
    );
}

/// An entry that only computes gets the signal at its next turn (R1b: the
/// signal is posted to the run thread, which never waits for it), runs its
/// release and ends 130.
#[test]
fn a_signal_during_a_busy_entry_stops_it() {
    let mut l = run_e(
        "fn spin(n) = spin(n + 1)\nwith(() => nil, fn(_):\n  print(\"ready\")\n  spin(0)\n;;, _ => print(\"released\"))",
    );
    l.wait_for("ready");
    let start = Instant::now();
    l.signal("INT");
    assert_eq!(
        l.finish(),
        ("ready\nreleased\n".into(), "".into(), Some(130))
    );
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn a_watcher_finishes_and_ends_0() {
    let mut l = run_e(
        "stop = stop_signal()\nprint(\"ready\")\nwait_closed(stop)\nprint(\"stopping\")\nsleep(50ms)\nprint(\"done\")",
    );
    l.wait_for("ready");
    l.signal("TERM");
    assert_eq!(
        l.finish(),
        ("ready\nstopping\ndone\n".into(), "".into(), Some(0))
    );
}

/// The release waits forever; the second signal does not wait for it.
#[test]
fn a_second_signal_does_not_wait() {
    let mut l = run_e(
        "with(() => nil, fn(_):\n  print(\"ready\")\n  sleep(60s)\n;;, fn(_):\n  print(\"releasing\")\n  sleep(60s)\n;;)",
    );
    l.wait_for("ready");
    l.signal("TERM");
    l.wait_for("releasing");
    let start = Instant::now();
    l.signal("TERM");
    let (_, _, code) = l.finish();
    assert_eq!(code, Some(143));
    assert!(start.elapsed() < Duration::from_secs(2));
}

/// The block fung keeps on the two signals does not reach the commands
/// it runs.
#[test]
fn a_child_starts_with_signals_unblocked() {
    let l = run_e("print(sh(\"trap 'echo got' TERM; kill -TERM $$; echo after\")!.out)");
    assert_eq!(l.finish(), ("got\nafter\n\n".into(), "".into(), Some(0)));
}

/// In the REPL, Ctrl-C cancels the evaluation in progress; the session
/// goes on.
#[test]
fn repl_ctrl_c_cancels_the_evaluation() {
    let mut l = Live::start(&[], Stdio::piped());
    let mut input = l.child.stdin.take().unwrap();
    input
        .write_all(b"fn(): print(\"ready\"); sleep(60s) ;;()\n")
        .unwrap();
    input.flush().unwrap();
    l.wait_for("ready");
    l.signal("INT");
    input.write_all(b"print(\"after\", 1 + 1)\n").unwrap();
    drop(input);
    let (out, _, code) = l.finish();
    assert!(out.contains("after 2\n"), "{out}");
    assert_eq!(code, Some(0));
}

/// A swap's function runs without giving its turn; the first signal
/// still reaches one that never ends (review C6: it took a second).
#[test]
fn a_signal_during_a_swap_stops_it() {
    let mut l = run_e(
        "fn spin(n) = spin(n + 1)\na = atom(0)\nprint(\"ready\")\nswap(a, x => spin(x))\nprint(\"not here\")",
    );
    l.wait_for("ready");
    let start = Instant::now();
    l.signal("TERM");
    assert_eq!(l.finish(), ("ready\n".into(), "".into(), Some(143)));
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "{:?}",
        start.elapsed()
    );
}

/// The REPL's Ctrl-C cancels an evaluation stuck in a swap's function
/// (review C6: no number of Ctrl-Cs did).
#[test]
fn repl_ctrl_c_cancels_a_swap() {
    let mut l = Live::start(&[], Stdio::piped());
    let mut input = l.child.stdin.take().unwrap();
    input
        .write_all(b"fn spin(n) = spin(n + 1)\na = atom(0)\nfn(): print(\"ready\"); swap(a, x => spin(x)) ;;()\n")
        .unwrap();
    input.flush().unwrap();
    l.wait_for("ready");
    l.signal("INT");
    input.write_all(b"print(\"after\", 1 + 1)\n").unwrap();
    drop(input);
    let (out, _, code) = l.finish();
    assert!(out.contains("after 2\n"), "{out}");
    assert_eq!(code, Some(0));
}

/// Each test block starts with no stop requested.
#[test]
fn fung_test_blocks_get_a_fresh_stop() {
    let dir = std::env::temp_dir().join(format!("fung-signals-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("stop_test.fung");
    std::fs::write(
        &file,
        "test \"stops\":\n  stop = stop_signal()\n  request_stop()\n  assert recv(stop) == Err({kind: 'closed, message: \"channel closed\"})\n;;\n\ntest \"not stopped\":\n  stop = stop_signal()\n  assert timeout(20ms, () => recv(stop)) == Err({kind: 'timeout, message: \"timed out after 20ms\"})\n;;\n",
    )
    .unwrap();
    let l = Live::start(&["test", file.to_str().unwrap()], Stdio::null());
    let (out, err, code) = l.finish();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.contains("ok   ") && !out.contains("FAIL"),
        "{out}\n{err}"
    );
    assert_eq!(code, Some(0), "{out}\n{err}");
}

/// SIGTERM to a server: the request in flight is answered, a new
/// connection is refused, `serve` returns Ok(nil) and the run ends with 0.
#[test]
fn a_server_drains_on_sigterm() {
    use std::net::TcpStream;
    let mut l = run_e(
        "import http\nfn h(req) = fn(): print(\"in\"); sleep(300ms); \"slow\" ;;()\nprint(http.serve(\"127.0.0.1:0\", h, ready: a => print(a)))\nprint(\"served\")",
    );
    let mut addr = String::new();
    l.out.read_line(&mut addr).unwrap();
    let addr = addr.trim().to_string();
    let mut c = TcpStream::connect(&addr).unwrap();
    c.write_all(b"GET /slow HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    l.wait_for("in");
    l.signal("TERM");
    // the listener closes on the stop request
    let start = Instant::now();
    while TcpStream::connect(&addr).is_ok() {
        assert!(start.elapsed() < Duration::from_secs(5), "still accepting");
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut answer = String::new();
    c.read_to_string(&mut answer).unwrap();
    assert!(answer.starts_with("HTTP/1.1 200 OK\r\n"), "{answer}");
    assert!(answer.ends_with("\r\n\r\nslow"), "{answer}");
    let (out, err, code) = l.finish();
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("in\nOk(nil)\nserved\n", "", Some(0))
    );
}

/// In the foreground of a terminal a command shares fung's process group,
/// so it can read the terminal (a password prompt) instead of being
/// stopped by SIGTTIN. `script` gives the run a terminal.
#[test]
fn a_command_can_read_the_terminal() {
    let dir = std::env::temp_dir().join(format!("fung-tty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("tty.fung");
    std::fs::write(
        &file,
        "print(sh(\"read x < /dev/tty; echo got $x\")!.out)\n",
    )
    .unwrap();
    let bin = fung_bin();
    let mut cmd = Command::new("script");
    if cfg!(target_os = "macos") {
        cmd.args(["-q", "/dev/null"]).arg(&bin).arg(&file);
    } else {
        cmd.args(["-qec"])
            .arg(format!("{} {}", bin.display(), file.display()))
            .arg("/dev/null");
    }
    let mut child = Reaped::new(
        common::spawn(
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
        )
        .unwrap(),
    );
    child.stdin.take().unwrap().write_all(b"hello\n").unwrap();
    let (o, killed) = child.output_within(Duration::from_secs(10)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(!killed, "the command stopped on the terminal: {out}");
    // what `read` gets (the line, or the end of input `script` passes
    // on first) does not matter: it returned instead of being stopped
    assert!(out.contains("got"), "{out}");
}

/// A long native holds the run thread; the second signal still ends the
/// process at once (the first waits for the run thread's next turn).
#[test]
fn a_second_signal_does_not_wait_for_the_run_token() {
    let mut l = run_e(
        "xs = map(0..2000000, i => (i * 7919) % 1000003)\nprint(\"ready\")\neach(0..20, _ => sort(xs))",
    );
    l.wait_for("ready");
    l.signal("INT");
    std::thread::sleep(Duration::from_millis(100));
    let start = Instant::now();
    l.signal("INT");
    let (_, _, code) = l.finish();
    assert_eq!(code, Some(130));
    assert!(
        start.elapsed() < Duration::from_millis(500),
        "{:?}",
        start.elapsed()
    );
}

/// After the program's own stop request, a signal is the second request:
/// it ends the process at once.
#[test]
fn a_signal_after_request_stop_ends_at_once() {
    let mut l = run_e("stop = stop_signal()\nrequest_stop()\nprint(\"ready\")\nsleep(60s)");
    l.wait_for("ready");
    let start = Instant::now();
    l.signal("INT");
    let (_, _, code) = l.finish();
    assert_eq!(code, Some(130));
    assert!(start.elapsed() < Duration::from_secs(2));
}

/// A stop signal taken at a test file's top level stays open in its
/// blocks; a signal that cuts a file short is named as one.
#[test]
fn fung_test_keeps_a_top_level_stop_and_names_a_signal() {
    let dir = std::env::temp_dir().join(format!("fung-signals-top-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("top_test.fung");
    std::fs::write(
        &file,
        "stop = stop_signal()\n\ntest \"open\":\n  assert timeout(20ms, () => recv(stop)) == Err({kind: 'timeout, message: \"timed out after 20ms\"})\n;;\n",
    )
    .unwrap();
    let file2 = dir.join("slow_test.fung");
    std::fs::write(
        &file2,
        "test \"waits\":\n  print(\"ready\")\n  sleep(60s)\n;;\n\ntest \"after\": assert true ;;\n",
    )
    .unwrap();
    let l = Live::start(&["test", file.to_str().unwrap()], Stdio::null());
    let (out, err, code) = l.finish();
    assert!(
        out.contains("ok   ") && !out.contains("FAIL"),
        "{out}\n{err}"
    );
    assert_eq!(code, Some(0));
    let mut l = Live::start(&["test", file2.to_str().unwrap()], Stdio::null());
    l.wait_for("ready");
    l.signal("TERM");
    let (out, _, code) = l.finish();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.contains("SIGTERM stopped the run"), "{out}");
    assert!(!out.contains("exit(143)"), "{out}");
    assert_eq!(code, Some(143));
}

/// What the REPL says when a second Ctrl-C drops an input stuck in its
/// cleanup.
const ABANDONED: &str =
    "interrupted again: the cleanup did not finish; the input's tasks were dropped";

/// A REPL input whose `release` hangs: the first Ctrl-C cancels the body
/// and the release runs (a cancel does not cut it); a second Ctrl-C drops
/// the input's tasks and the session goes on.
#[test]
fn a_second_ctrl_c_abandons_a_stuck_release() {
    let mut l = Live::start(&[], Stdio::piped());
    let mut input = l.child.stdin.take().unwrap();
    input
        .write_all(b"with(() => nil, fn(_): print(\"ready\"); sleep(60s) ;;, fn(_): print(\"releasing\"); sleep(60s) ;;)\n")
        .unwrap();
    input.flush().unwrap();
    l.wait_for("ready");
    l.signal("INT");
    l.wait_for("releasing");
    l.signal("INT");
    input.write_all(b"print(\"after\", 1 + 1)\n").unwrap();
    drop(input);
    let (out, err, code) = l.finish();
    assert!(out.contains("after 2\n"), "{out}");
    assert!(err.contains(ABANDONED), "{err}");
    assert_eq!(code, Some(0));
}

/// The channel an abandoned input waited on is usable afterwards: its
/// dropped wait left the channel's line.
#[test]
fn an_abandoned_evaluation_leaves_its_channel_usable() {
    let mut l = Live::start(&[], Stdio::piped());
    let mut input = l.child.stdin.take().unwrap();
    input
        .write_all(b"(tx, rx) = channel()\nwith(() => nil, fn(_): print(\"ready\"); sleep(60s) ;;, fn(_): print(\"releasing\"); timeout(60s, () => recv(rx)) ;;)\n")
        .unwrap();
    input.flush().unwrap();
    l.wait_for("ready");
    l.signal("INT");
    l.wait_for("releasing");
    l.signal("INT");
    input
        .write_all(b"scope(fn(s): spawn(s, () => send(tx, 7)); print(\"got\", recv(rx)) ;;)\n")
        .unwrap();
    drop(input);
    let (out, err, code) = l.finish();
    assert!(out.contains("got Ok(7)\n"), "{out}\n{err}");
    assert_eq!(code, Some(0));
}

/// A task of an abandoned input whose handle was kept elsewhere (a
/// channel from an earlier input) awaits as cancelled: it never ends,
/// and its waiters must not wait for it.
#[test]
fn an_abandoned_tasks_handle_in_a_later_input_is_cancelled() {
    let mut l = Live::start(&[], Stdio::piped());
    let mut input = l.child.stdin.take().unwrap();
    input
        .write_all(b"(htx, hrx) = channel(1)\nwith(() => nil, fn(_): print(\"ready\"); sleep(60s) ;;, fn(_): scope(fn(s):\n  send(htx, spawn(s, () => sleep(60s)))\n  print(\"releasing\")\n  sleep(60s)\n;;) ;;)\n")
        .unwrap();
    input.flush().unwrap();
    l.wait_for("ready");
    l.signal("INT");
    l.wait_for("releasing");
    l.signal("INT");
    input
        .write_all(b"print(\"got\", timeout(5s, () => await(recv(hrx)!)))\n")
        .unwrap();
    drop(input);
    let (out, err, code) = l.finish();
    assert!(
        out.contains("got Err({kind: 'cancelled, message: \"cancelled\"})\n"),
        "{out}\n{err}"
    );
    assert_eq!(code, Some(0));
}

/// A command an abandoned input waited on is killed and reaped.
#[test]
fn an_abandoned_command_is_killed_and_reaped() {
    let dir = std::env::temp_dir().join(format!("fung-abandon-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("pid");
    let mut l = Live::start(&[], Stdio::piped());
    let mut input = l.child.stdin.take().unwrap();
    input
        .write_all(
            format!(
                "with(() => nil, fn(_): print(\"ready\"); sleep(60s) ;;, fn(_): print(\"releasing\"); sh(\"echo $$ > {}; exec sleep 60\") ;;)\n",
                f.display()
            )
            .as_bytes(),
        )
        .unwrap();
    input.flush().unwrap();
    l.wait_for("ready");
    l.signal("INT");
    l.wait_for("releasing");
    let start = Instant::now();
    while !f.exists() && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let pid = std::fs::read_to_string(&f).unwrap().trim().to_string();
    l.signal("INT");
    input.write_all(b"print(\"after\")\n").unwrap();
    l.wait_for("after");
    // `kill -0` fails once the process is gone and reaped (a zombie
    // still answers it)
    let start = Instant::now();
    let gone = loop {
        let alive = common::status(
            Command::new("kill")
                .args(["-0", &pid])
                .stderr(Stdio::null()),
        )
        .unwrap()
        .success();
        if !alive {
            break true;
        }
        if start.elapsed() > Duration::from_secs(5) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    drop(input);
    let _ = l.finish();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(gone, "the command {pid} is still there");
}

/// A command whose leader ended while what it left still holds its
/// output: abandoning the wait kills that group too (spec 15).
#[test]
fn an_abandoned_command_group_is_killed_after_its_leader_exited() {
    let dir = std::env::temp_dir().join(format!("fung-abandon-group-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("pid");
    let mut l = Live::start(&[], Stdio::piped());
    let mut input = l.child.stdin.take().unwrap();
    input
        .write_all(
            format!(
                "with(() => nil, fn(_): print(\"ready\"); sleep(60s) ;;, fn(_): print(\"releasing\"); sh(\"sleep 7 & echo $! > {}\") ;;)\n",
                f.display()
            )
            .as_bytes(),
        )
        .unwrap();
    input.flush().unwrap();
    l.wait_for("ready");
    l.signal("INT");
    l.wait_for("releasing");
    let start = Instant::now();
    while !f.exists() && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let pid = std::fs::read_to_string(&f).unwrap().trim().to_string();
    l.signal("INT");
    input.write_all(b"print(\"after\")\n").unwrap();
    l.wait_for("after");
    let start = Instant::now();
    let gone = loop {
        let alive = common::status(
            Command::new("kill")
                .args(["-0", &pid])
                .stderr(Stdio::null()),
        )
        .unwrap()
        .success();
        if !alive {
            break true;
        }
        if start.elapsed() > Duration::from_secs(5) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    drop(input);
    let _ = l.finish();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(gone, "what the command left ({pid}) is still there");
}
