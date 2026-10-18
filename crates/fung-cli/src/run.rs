//! `fung run`, `fung <script.fung>` and `fung -e`: compile the entry
//! with its import graph, boot the prelude, run (unwinding through every
//! `with`'s release), then the exit code.

use std::io::Write;

use fung_front::diag::{self, Diag, EXIT_COMPILE, EXIT_OK, EXIT_RUNTIME};
use fung_front::loader::{Loader, Stage};
use fung_front::source::SourceMap;
use fung_vm::vm::{Runtime, STDIN_BUFFER, Stop};

/// Ends the command when a write found the reading end of a pipe closed
/// (`fung run x | head -1`): the process dies by SIGPIPE (06 "에러
/// 규칙"), with no report and no further cleanup. Only the
/// standard streams come here; every other write keeps EPIPE as an error.
fn exit_on_broken_pipe<T>(r: std::io::Result<T>) -> std::io::Result<T> {
    if let Err(e) = &r
        && e.kind() == std::io::ErrorKind::BrokenPipe
    {
        fung_vm::sigpipe::die();
    }
    r
}

/// A process stream (standard output or error) as the command writes it:
/// every write goes through at once (a prompt
/// without a newline shows before the program reads its answer), and a
/// closed pipe kills the command (`exit_on_broken_pipe`).
pub struct ProcessOut<W: Write>(pub W);

impl ProcessOut<std::io::Stdout> {
    pub fn stdout() -> Self {
        ProcessOut(std::io::stdout())
    }
}

impl ProcessOut<std::io::Stderr> {
    pub fn stderr() -> Self {
        ProcessOut(std::io::stderr())
    }
}

impl<W: Write> Write for ProcessOut<W> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        exit_on_broken_pipe(self.0.write_all(b))?;
        exit_on_broken_pipe(self.0.flush())?;
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        exit_on_broken_pipe(self.0.flush())
    }
}

/// The status the operating system keeps of a fung exit code: its low 8
/// bits, as the system keeps it (exit(300) is 44, exit(-1) is 255).
pub fn status(code: i64) -> i32 {
    (code & 0xff) as i32
}

/// Compiles and runs one entry file and returns the fung
/// exit code: 0, 1 for a runtime error, 2 for a compile error, or the
/// code of exit(n). Every value the run made is dropped before it
/// returns, so it runs on the big stack together with its caller.
pub fn execute(file: &str, src: &str, args: Vec<String>) -> i64 {
    // Standard error is unbuffered; the runtime and the reports below
    // write to it through separate handles, in order.
    let stdout = Box::new(ProcessOut::stdout());
    let stdin = Box::new(std::io::BufReader::with_capacity(STDIN_BUFFER, raw_stdin()));
    let stderr = &mut ProcessOut::stderr();
    let (b, errs) = Loader::default().compile(file, src, Stage::All);
    if !errs.is_empty() {
        report(stderr, &errs, &b.sources);
        return EXIT_COMPILE.into();
    }
    let entry = b.proto.expect("a compiled entry");
    // The runtime holds the program's files first and boot appends the
    // prelude's after them (see `fung_vm::boot`).
    let rt = Runtime::new(stdout, Box::new(ProcessOut::stderr()), stdin, b.sources);
    if let Err(errs) = rt.boot(args) {
        rt.with_sources(|s| report(stderr, &errs, s));
        return EXIT_COMPILE.into();
    }
    // Links the entry against the booted prelude and registers the
    // modules. No fung value comes back out of the runtime.
    rt.load(&entry, &b.modules);
    // The first SIGINT or SIGTERM is a stop request (`vm/stop.rs`).
    let first = std::sync::Arc::new(std::sync::atomic::AtomicI32::new(0));
    fung_vm::signals::set(Some(fung_vm::signals::stop_requests(
        rt.stop_handle(),
        first,
    )));
    let mut code = i64::from(EXIT_OK);
    let result = rt.run();
    match &result {
        // The entry is done; the tasks it left keep the program alive (05).
        Ok(()) => rt.wait(),
        // An entry error (or an exit) ends the program: cancel what is
        // left and wait for it to unwind.
        Err(_) => {
            rt.cancel_all();
            rt.wait();
        }
    }
    // An entry error is reported even after an exit: a release that
    // failed while the program unwound for `exit(n)` reaches the entry
    // (03 "자원 정리"). A cancellation (an exit's, or the deadlock
    // report's E0702, already told) is not an error to report.
    let mut reported = false;
    if let Err(Stop::Error(d)) = &result
        && d.code != "E0701"
        && d.code != "E0702"
    {
        rt.with_sources(|s| report(stderr, std::slice::from_ref(&**d), s));
        code = EXIT_RUNTIME.into();
        reported = true;
    }
    drop(result);
    let before = rt.exit_code();
    if let Some(n) = before {
        code = n;
    }
    // exit(0) does not hide a reported error: the run fails.
    if reported && code == i64::from(EXIT_OK) {
        code = EXIT_RUNTIME.into();
    }
    // The program has unwound (through its `with` releases). A failure a
    // cancelled `supervise` could not hand to its handler was reported
    // as it closed, and makes the exit code 1 (05).
    match (before, rt.exit_code()) {
        (None, Some(n)) => code = n, // an exit requested while unwinding
        (None, None) if code == i64::from(EXIT_OK) && rt.failed() => {
            code = EXIT_RUNTIME.into(); // a task failure nobody handled (05)
        }
        _ => {}
    }
    fung_vm::signals::set(None);
    drop(rt);
    code
}

/// Standard input without the standard library's own buffer (which
/// would read 8 KiB at a time): the runtime reads it one `STDIN_BUFFER`
/// at a time, so what fung leaves unread stays for the next reader of the
/// same input.
pub(crate) fn raw_stdin() -> Box<dyn std::io::Read + Send> {
    use std::os::fd::AsFd;
    match std::io::stdin().as_fd().try_clone_to_owned() {
        Ok(fd) => Box::new(std::fs::File::from(fd)),
        // No descriptor 0 to share: the buffered handle reads the same.
        Err(_) => Box::new(std::io::stdin()),
    }
}

/// Prints errors with source excerpts.
fn report(w: &mut dyn Write, errs: &[Diag], sources: &SourceMap) {
    for e in errs {
        let _ = w.write_all(diag::format(e, sources).as_bytes());
    }
    let _ = w.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_keeps_the_low_eight_bits() {
        // what `fung -e 'exit(n)'; echo $?` shows
        for (n, want) in [
            (0, 0),
            (4, 4),
            (300, 44),
            (-1, 255),
            (-256, 0),
            (1 << 40, 0),
            ((1 << 40) + 1, 1),
            (1 << 31, 0),
            ((1 << 32) + 1, 1),
            (i64::MAX, 255),
        ] {
            assert_eq!(status(n), want, "exit({n})");
        }
    }
}
