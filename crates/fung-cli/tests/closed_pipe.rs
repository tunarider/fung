//! Writing to a closed pipe ends the command at once with status 141, the
//! code a shell shows for Go's death by SIGPIPE: no report, no exit
//! handlers. (Go dies by signal 13 itself; the Rust port keeps SIGPIPE
//! ignored so writes to a child's stdin still fail with an error, and
//! exits with 141 instead.)

use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard, PoisonError};

fn fung() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fung"))
}

/// Serialises every test here that creates a pipe or spawns a child.
///
/// The tests run on parallel threads of one process. A child spawned by
/// one test inherits whatever descriptors are open in the process at the
/// moment it forks, unless they are close-on-exec, and macOS sets that
/// flag on a new pipe in a second step after creating it. So a sibling's
/// spawn can land between `pipe()` and `drop(r)` here and its child keeps
/// a copy of the read end open for as long as it runs: the pipe is not
/// closed after all, the write does not fail with EPIPE, and the test
/// sees an ordinary run instead of 141. Holding this lock from creating
/// the pipe until the child has been waited for (and around every other
/// spawn in the file) leaves no sibling spawn to inherit the read end.
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    // A failed assertion in another test poisons the lock; the `()` it
    // guards cannot be left inconsistent, so go on.
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs `code` with standard output (or error) connected to a pipe whose
/// read end is already closed.
fn run_into_closed_pipe(code: &str, on_stderr: bool) -> (Option<i32>, Vec<u8>) {
    let _serial = serial();
    let (r, w) = std::io::pipe().unwrap();
    drop(r);
    let mut cmd = fung();
    cmd.args(["-e", code]).stdin(Stdio::null());
    if on_stderr {
        cmd.stderr(Stdio::from(w)).stdout(Stdio::piped());
    } else {
        cmd.stdout(Stdio::from(w)).stderr(Stdio::piped());
    }
    let o = cmd.output().unwrap();
    let other = if on_stderr { o.stdout } else { o.stderr };
    (o.status.code(), other)
}

#[test]
fn print_to_a_closed_stdout_exits_141_quietly() {
    let (code, err) = run_into_closed_pipe(
        "at_exit(() => eprint(\"handler ran\"))\neach(0..1000000, (i) => print(i))",
        false,
    );
    assert_eq!(code, Some(141));
    assert_eq!(String::from_utf8_lossy(&err), "");
}

#[test]
fn eprint_to_a_closed_stderr_exits_141_quietly() {
    let (code, out) = run_into_closed_pipe(
        "at_exit(() => print(\"handler ran\"))\neach(0..1000000, (i) => eprint(i))",
        true,
    );
    assert_eq!(code, Some(141));
    assert_eq!(String::from_utf8_lossy(&out), "");
}

#[test]
fn a_report_to_a_closed_stderr_exits_141() {
    let (code, out) = run_into_closed_pipe("at_exit(() => print(\"handler ran\"))\n1 / 0", true);
    // The report is written first; the handler would print after it.
    assert_eq!(code, Some(141));
    assert_eq!(String::from_utf8_lossy(&out), "");
}

#[test]
fn a_closed_child_stdin_is_still_an_error_value() {
    let _serial = serial();
    let o = fung()
        .args([
            "-e",
            "r = exec([\"true\"], stdin: join(map(0..200000, (i) => \"abcdefghij\"), \"\"))\nprint(r)",
        ])
        .output()
        .unwrap();
    // `true` exits without reading, so the write to its stdin fails;
    // Go ignores that error, and the Rust port must not exit with 141.
    assert_eq!(
        String::from_utf8_lossy(&o.stdout),
        "Ok({out: \"\", err: \"\", code: 0})\n"
    );
    assert_eq!(o.status.code(), Some(0));
}
