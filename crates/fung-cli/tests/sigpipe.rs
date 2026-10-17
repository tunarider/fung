//! Writing to a closed pipe. On standard output or error (file
//! descriptor 1 or 2) fung dies by SIGPIPE (signal 13) at once, from any
//! task and from an exit handler, without running the exit handlers or
//! reporting anything. Every other descriptor (a file handle on a fifo, a
//! child's standard input) gives an error value. fung dies even when it
//! was started with SIGPIPE ignored, and a child process starts with the
//! default SIGPIPE action.

#![cfg(unix)]

mod common;

use std::io::{BufRead, BufReader};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Mutex, MutexGuard, PoisonError};

use common::rust_fung;

/// Serialises every test here that creates a pipe or spawns a child.
///
/// The tests run on parallel threads of one process. A child spawned by
/// one test inherits whatever descriptors are open in the process at the
/// moment it forks, unless they are close-on-exec, and macOS sets that
/// flag on a new pipe in a second step after creating it. So a sibling's
/// spawn can land between `pipe()` and `drop(r)` here and its child keeps
/// a copy of the read end open for as long as it runs: the pipe is not
/// closed after all and the write does not fail. Holding this lock from
/// creating the pipe until the child has been waited for (and around
/// every other spawn in the file) leaves no
/// sibling spawn to inherit the read end.
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    // A failed assertion in another test poisons the lock; the `()` it
    // guards cannot be left inconsistent, so go on.
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs `fung -e code` with standard output (or error, `on_stderr`)
/// connected to a pipe whose read end is already closed, and returns the
/// status and what the other stream got. Standard input is a pipe the
/// test keeps open until the child ends, so `read_line()` blocks.
/// With `ignored`, fung starts with SIGPIPE ignored (a shell's `trap ""
/// PIPE` is inherited across `exec`).
fn into_closed_pipe(
    bin: &Path,
    code: &str,
    on_stderr: bool,
    ignored: bool,
) -> (ExitStatus, String) {
    let (r, w) = std::io::pipe().unwrap();
    drop(r);
    let mut cmd = if ignored {
        let mut c = Command::new("sh");
        c.args(["-c", "trap '' PIPE; exec \"$0\" -e \"$1\""])
            .arg(bin)
            .arg(code);
        c
    } else {
        let mut c = Command::new(bin);
        c.args(["-e", code]);
        c
    };
    cmd.stdin(Stdio::piped());
    if on_stderr {
        cmd.stderr(Stdio::from(w)).stdout(Stdio::piped());
    } else {
        cmd.stdout(Stdio::from(w)).stderr(Stdio::piped());
    }
    // killed and waited for if anything below panics
    let mut child = common::Reaped::new(cmd.spawn().unwrap());
    let stdin = child.stdin.take();
    let o = child.output().unwrap();
    drop(stdin);
    let other = if on_stderr { o.stdout } else { o.stderr };
    (o.status, String::from_utf8_lossy(&other).into_owned())
}

/// fung dies by signal 13 and writes `other` to the other stream.
fn dies_by_sigpipe(code: &str, on_stderr: bool, other: &str) {
    let _serial = serial();
    for ignored in [false, true] {
        let (status, got) = into_closed_pipe(&rust_fung(), code, on_stderr, ignored);
        let how = if ignored {
            " (started ignoring SIGPIPE)"
        } else {
            ""
        };
        assert_eq!(status.signal(), Some(13), "{how}: {status:?}");
        assert_eq!(got, other, "{how}");
    }
}

#[test]
fn print_to_a_closed_stdout_dies_by_sigpipe() {
    dies_by_sigpipe(
        "at_exit(() => eprint(\"handler ran\"))\neach(0..1000000, (i) => print(i))",
        false,
        "",
    );
}

#[test]
fn print_from_an_awaited_task_dies_by_sigpipe() {
    dies_by_sigpipe(
        "at_exit(() => eprint(\"handler ran\"))\nt = spawn(() => each(0..1000000, (i) => print(i)))\nawait(t)\neprint(\"after\")",
        false,
        "",
    );
}

#[test]
fn print_from_a_task_while_the_entry_waits_for_input_dies_by_sigpipe() {
    // The entry task blocks in `read_line` (standard input stays open),
    // so only the spawned task's write can end the process.
    dies_by_sigpipe(
        "at_exit(() => eprint(\"handler ran\"))\nt = spawn(() => each(0..1000000, (i) => print(i)))\nread_line()\neprint(\"after\")",
        false,
        "",
    );
}

#[test]
fn eprint_to_a_closed_stderr_dies_by_sigpipe() {
    dies_by_sigpipe(
        "at_exit(() => print(\"handler ran\"))\neach(0..1000000, (i) => eprint(i))",
        true,
        "",
    );
}

#[test]
fn a_report_to_a_closed_stderr_dies_by_sigpipe() {
    // The report is written first; the handler would print after it.
    dies_by_sigpipe("at_exit(() => print(\"handler ran\"))\n1 / 0", true, "");
}

#[test]
fn print_from_an_exit_handler_dies_by_sigpipe() {
    // Handlers run last first: the second one dies, the first never runs.
    dies_by_sigpipe(
        "at_exit(() => eprint(\"first handler ran\"))\nat_exit(() => print(\"second handler\"))\neprint(\"main\")",
        false,
        "main\n",
    );
}

/// A fresh directory with a fifo `f` in it.
fn fifo(name: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("fung-sigpipe-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("f");
    let st = Command::new("mkfifo").arg(&f).status().unwrap();
    assert!(st.success(), "mkfifo failed");
    (dir, f)
}

/// Runs `fung -e code <fifo>` while another thread opens the fifo for
/// reading and closes it again at once (the open waits for fung's open
/// for writing). Standard input is closed only after the reader has
/// closed, so a program can `read_line()` to wait for that. Returns
/// standard output and error (the fifo's directory replaced by `D`) and
/// the status.
fn into_closed_fifo(bin: &Path, f: &Path, dir: &Path, code: &str) -> (String, String, ExitStatus) {
    // killed and waited for if anything below panics
    let mut child = common::Reaped::new(
        Command::new(bin)
            .args(["-e", code])
            .arg(f)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let stdin = child.stdin.take();
    let reader = {
        let f = f.to_path_buf();
        // Blocks until fung opens the fifo for writing. Left blocked if
        // fung fails before that; the assertions below then fail.
        std::thread::spawn(move || drop(std::fs::File::open(f).unwrap()))
    };
    // Wait for the reader to have come and gone before letting fung on
    // past its `read_line`, or until fung has ended (it may never open).
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut first = String::new();
    loop {
        let mut line = String::new();
        if stdout.read_line(&mut line).unwrap() == 0 {
            break;
        }
        first.push_str(&line);
        if line == "opened\n" {
            reader.join().unwrap();
            break;
        }
    }
    drop(stdin);
    let mut rest = String::new();
    std::io::Read::read_to_string(&mut stdout, &mut rest).unwrap();
    let o = child.output().unwrap();
    let d = dir.to_str().unwrap();
    (
        (first + &rest).replace(d, "D"),
        String::from_utf8_lossy(&o.stderr).replace(d, "D"),
        o.status,
    )
}

/// fung gives `want` on standard output and exits 0.
fn fifo_case(name: &str, code: &str, want: &str) {
    let _serial = serial();
    let (dir, f) = fifo(name);
    let (out, err, status) = into_closed_fifo(&rust_fung(), &f, &dir, code);
    assert_eq!((out.as_str(), err.as_str()), (want, ""));
    assert_eq!(status.code(), Some(0), "{status:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fs_write_to_a_closed_fifo_is_an_error_value() {
    fifo_case(
        "write",
        "import fs\nbig = join(map(0..100000, (i) => \"abcdefghij\"), \"\")\nprint(\"opened\")\nprint(fs.write(args[0], big))\nprint(\"alive\")",
        "opened\nErr({kind: 'io, message: \"cannot write `D/f`: broken pipe\"})\nalive\n",
    );
}

#[test]
fn a_file_handle_on_a_closed_fifo_gives_error_values() {
    fifo_case(
        "handle",
        "import fs\nbig = join(map(0..100000, (i) => \"abcdefghij\"), \"\")\nh = fs.open(args[0], 'append)!\nprint(\"opened\")\nread_line()\nprint(fs.write_line(h, big))\nprint(fs.write_line(h, big))\nprint(fs.close(h))\nprint(\"alive\")",
        "opened\nErr({kind: 'io, message: \"cannot write `D/f`: broken pipe\"})\nErr({kind: 'io, message: \"cannot write `D/f`: broken pipe\"})\nErr({kind: 'io, message: \"cannot close `D/f`: broken pipe\"})\nalive\n",
    );
}

#[test]
fn a_buffered_write_to_a_closed_fifo_fails_at_close() {
    fifo_case(
        "flush",
        "import fs\nh = fs.open(args[0], 'append)!\nprint(fs.write_line(h, \"small\"))\nprint(\"opened\")\nread_line()\nprint(fs.close(h))\nprint(\"alive\")",
        "Ok(nil)\nopened\nErr({kind: 'io, message: \"cannot close `D/f`: broken pipe\"})\nalive\n",
    );
}

/// fung runs `code` with no standard input and gives `want` on standard
/// output and status 0.
fn runs_with(code: &str, want: &str) {
    let _serial = serial();
    let o = Command::new(rust_fung())
        .args(["-e", code])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), want);
    assert_eq!(String::from_utf8_lossy(&o.stderr), "");
    assert_eq!(o.status.code(), Some(0), "{:?}", o.status);
}

#[test]
fn a_closed_child_stdin_is_still_an_error_value() {
    // `true` exits without reading, so the write to its stdin fails;
    // the error is ignored, and fung does not die.
    runs_with(
        "r = exec([\"true\"], stdin: join(map(0..200000, (i) => \"abcdefghij\"), \"\"))\nprint(r)",
        "Ok({out: \"\", err: \"\", code: 0})\n",
    );
}

#[test]
fn a_child_starts_with_the_default_sigpipe_action() {
    // `yes` dies by SIGPIPE (status 141 in the shell) once `head` has
    // gone; with SIGPIPE ignored it would print an error and exit 1.
    runs_with(
        "print(exec([\"sh\", \"-c\", \"(yes; echo yes-exit $? >&2) | head -1\"]))",
        "Ok({out: \"y\\n\", err: \"yes-exit 141\\n\", code: 0})\n",
    );
}
