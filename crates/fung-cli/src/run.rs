//! `fung run`, `fung <script.fung>` and `fung -e` (Go `cmd/fung/main.go`
//! `runFile` and `cmd/fung/pipeline.go` `execute`): compile the entry
//! with its import graph, boot the prelude, run, then the exit handlers,
//! then the exit code.

use std::io::Write;

use fung_front::diag::{self, Diag, EXIT_COMPILE, EXIT_OK, EXIT_RUNTIME};
use fung_front::loader::{Loader, Stage};
use fung_front::source::SourceMap;
use fung_vm::boot::{boot, native_module};
use fung_vm::vm::{Runtime, Stop, link};

/// A writer that flushes after every write.
struct Unbuffered<W: Write>(W);

impl<W: Write> Write for Unbuffered<W> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.write_all(b)?;
        self.0.flush()?;
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

/// The status the operating system keeps of a fung exit code: its low 8
/// bits, as for Go's `os.Exit` (exit(300) is 44, exit(-1) is 255).
pub fn status(code: i64) -> i32 {
    (code & 0xff) as i32
}

/// Compiles and runs one entry file (Go `execute`) and returns the fung
/// exit code: 0, 1 for a runtime error, 2 for a compile error, or the
/// code of exit(n). Every value the run made is dropped before it
/// returns, so it runs on the big stack together with its caller.
pub fn execute(file: &str, src: &str, args: Vec<String>) -> i64 {
    // Standard output is written through on every write, as Go's
    // `os.Stdout` is: a prompt without a newline shows before the program
    // reads its answer. Standard error is unbuffered; the runtime and the
    // reports below write to it through separate handles, in order.
    let stdout = Box::new(Unbuffered(std::io::stdout()));
    let stdin = Box::new(std::io::stdin().lock());
    let stderr = &mut std::io::stderr();
    let (b, errs) = Loader::default().compile(file, src, Stage::All);
    if !errs.is_empty() {
        report(stderr, &errs, &b.sources);
        return EXIT_COMPILE.into();
    }
    let entry = b.proto.expect("a compiled entry");
    // The runtime holds the program's files first and boot appends the
    // prelude's after them (see `fung_vm::boot`). Go boots before it
    // compiles; boot cannot fail, so the output is the same.
    let mut rt = Runtime::new(stdout, Box::new(std::io::stderr()), stdin, b.sources);
    let p = match boot(&mut rt, args) {
        Ok(p) => p,
        Err(errs) => {
            report(stderr, &errs, rt.sources());
            return EXIT_COMPILE.into();
        }
    };
    let main = link(&entry, &p);
    // Go `register`. A `_native.*` module stage 2 does not have stays
    // unregistered: importing it is an internal error at the import.
    for m in &b.modules {
        match &m.proto {
            Some(proto) => rt.register_module(&m.key, link(proto, &p)),
            None => {
                if let Some(module) = native_module(&m.key) {
                    rt.register_native(&m.key, module);
                }
            }
        }
    }
    let mut code = i64::from(EXIT_OK);
    let result = rt.run(main);
    if let Err(Stop::Error(d)) = &result
        && rt.exit_code().is_none()
    {
        report(stderr, std::slice::from_ref(&**d), rt.sources());
        code = EXIT_RUNTIME.into();
    }
    drop(result);
    let before = rt.exit_code();
    if let Some(n) = before {
        code = n;
    }
    let handler_errs = rt.run_exit_handlers();
    // An exit(n) inside a handler is not among the errors (the runtime
    // records it instead); every other handler error is reported.
    for d in &handler_errs {
        report(stderr, std::slice::from_ref(d), rt.sources());
        if code == i64::from(EXIT_OK) {
            code = EXIT_RUNTIME.into();
        }
    }
    if let (None, Some(n)) = (before, rt.exit_code()) {
        code = n; // an exit requested inside a handler
    }
    drop(handler_errs);
    drop(rt);
    code
}

/// Prints errors with source excerpts (Go `reportErrors`).
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
    fn status_keeps_the_low_eight_bits_like_go() {
        // Measured with the Go binary: `fung -e 'exit(n)'; echo $?`.
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
