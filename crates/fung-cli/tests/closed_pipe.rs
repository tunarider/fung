//! Writing to a closed pipe ends the command at once with status 141, the
//! code a shell shows for Go's death by SIGPIPE: no report, no exit
//! handlers. (Go dies by signal 13 itself; the Rust port keeps SIGPIPE
//! ignored so writes to a child's stdin still fail with an error, and
//! exits with 141 instead.)

use std::process::{Command, Stdio};

fn fung() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fung"))
}

/// Runs `code` with standard output (or error) connected to a pipe whose
/// read end is already closed.
fn run_into_closed_pipe(code: &str, on_stderr: bool) -> (Option<i32>, Vec<u8>) {
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
