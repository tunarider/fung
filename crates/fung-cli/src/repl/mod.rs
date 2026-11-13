//! The read-eval-print loop (08): `fung` without arguments. Each input is
//! compiled against the names defined so far and runs on one shared
//! runtime, so definitions persist and functions may be defined again.
//!
//! How values persist: the runtime keeps one global table for the session
//! (`Runtime::prepare_repl`). Each input is compiled with the names of
//! the earlier inputs' globals pre-bound to the same indexes
//! (`Loader::globals`) and runs on that table, so
//! the values stay, and a closure from an earlier input (which holds the
//! table) sees a later redefinition.
//!
//! Positions: the inputs are compiled after the files the runtime
//! already has (`Loader::compile_after`), so every position names one
//! file of the session, and that file keeps its text, here and in the
//! runtime (`Runtime::add_session_files`): an error is excerpted from the input
//! its code came from, also in a task failure report.
//!
//! After an input poisons the program (decided 2026-10-17): a deadlock
//! fails only the input it happened in, with the deadlock report, and the
//! session goes on on a fresh run root (`Runtime::recover_deadlock`: the
//! input's tasks are cancelled; globals, atoms among them, stay; the exit
//! code stays 0). A spawned task's `exit(n)` ends the session as
//! `exit(n)` typed at the prompt does.
//!
//! Tasks: an input's tasks run in its scopes and end with it; each input
//! is its own run, so waiting for the next line is no deadlock. At the end
//! (`:quit`, the end of input or `exit(n)`) the tasks are cancelled and
//! waited for (unwinding through their `with` releases).

mod editor;
mod history;
mod line;

use std::io::Write;
use std::path::PathBuf;

use fung_front::diag::{self, Diag, EXIT_COMPILE};
use fung_front::lexer::lex;
use fung_front::loader::{Loader, Stage};
use fung_front::parser::parse;
use fung_front::source::{FileId, SourceMap};
use fung_front::tree::Head;
use fung_vm::vm::{Runtime, STDIN_BUFFER, Stop};

use crate::Out;

/// What the REPL says when a second Ctrl-C dropped an input stuck in
/// its cleanup (`Runtime::take_abandoned`).
const ABANDONED: &str =
    "interrupted again: the cleanup did not finish; the input's tasks were dropped";
use crate::run::{ProcessOut, raw_stdin, status};
use line::{LineReader, PlainReader, TermReader};

/// The `:help` text.
const HELP: &str = "commands:
  :help            show this text
  :quit            leave (Ctrl-D does the same)
  :load file.fung  run a file in this session; its definitions stay
";

/// `fung` without arguments: a terminal gets the line
/// editor, anything else plain line reading.
pub fn start() -> Out {
    let rt = Runtime::with_streams(
        crate::run::RuntimeOut::stdout(),
        crate::run::RuntimeOut::stderr(),
        Box::new(std::io::BufReader::with_capacity(STDIN_BUFFER, raw_stdin())),
        SourceMap::new(),
        fung_vm::vm::std_streams_apart(),
    );
    if let Err(errs) = rt.boot(Vec::new()) {
        let text: String = rt.with_sources(|s| errs.iter().map(|e| diag::format(e, s)).collect());
        return Out {
            stderr: text.into_bytes(),
            code: EXIT_COMPILE,
            ..Out::default()
        };
    }
    let reader: Box<dyn LineReader> = match TermReader::new(raw_stdin_file(), ProcessOut::stdout())
    {
        Some(mut r) => {
            // The history file lives in the home directory (08); without
            // one, history stays in memory.
            if let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
                r.use_history_file(&PathBuf::from(home).join(".fung_history"));
            }
            Box::new(r)
        }
        None => Box::new(PlainReader::new(raw_stdin())),
    };
    // Ctrl-C during an evaluation cancels it (`Session::eval`).
    fung_vm::signals::set(Some(fung_vm::signals::repl(rt.stop_handle())));
    let code = Session::new(reader, rt).run();
    fung_vm::signals::set(None);
    Out {
        code: status(code),
        ..Out::default()
    }
}

/// Standard input as a file of its own (a duplicate of descriptor 0),
/// for the line editor, which needs the descriptor for raw mode.
fn raw_stdin_file() -> std::fs::File {
    use std::os::fd::AsFd;
    match std::io::stdin().as_fd().try_clone_to_owned() {
        Ok(fd) => std::fs::File::from(fd),
        // No descriptor 0: a file that reads nothing and is no terminal.
        Err(_) => std::fs::File::open("/dev/null").expect("open /dev/null"),
    }
}

/// One REPL run.
struct Session {
    input: Box<dyn LineReader>,
    out: ProcessOut<std::io::Stdout>,
    err: ProcessOut<std::io::Stderr>,
    rt: Runtime,
    /// Every file compiled so far, with its text; the runtime shares the
    /// same texts (`Runtime::add_session_files`).
    known: SourceMap,
    /// The entry globals defined so far, in index order.
    globals: Vec<String>,
    /// An error was reported (TL3): the session ends with 1, unless
    /// `exit(n)` names its code.
    failed: bool,
}

/// Surface forms whose value is not printed (08).
fn is_statement(h: Head) -> bool {
    matches!(
        h,
        Head::Assign | Head::Fn | Head::Import | Head::Typealias | Head::Test | Head::Assert
    )
}

impl Session {
    fn new(input: Box<dyn LineReader>, rt: Runtime) -> Session {
        rt.prepare_repl();
        let known = rt.with_sources(SourceMap::clone);
        Session {
            input,
            out: ProcessOut::stdout(),
            err: ProcessOut::stderr(),
            rt,
            known,
            globals: Vec::new(),
            failed: false,
        }
    }

    /// Reads inputs until `:quit`, the end of input or `exit(n)`, and
    /// returns the exit code: n for exit(n); otherwise 1 if an error was
    /// reported (TL3: a session fed from a pipe fails as a script does),
    /// else 0.
    fn run(mut self) -> i64 {
        loop {
            // A task an earlier input spawned called exit(n): the session
            // ends as for exit(n) typed at the prompt (decided 2026-10-17).
            if let Some(n) = self.rt.exit_code() {
                return self.finish(Some(n));
            }
            let Some(line) = self.input.read_line("> ") else {
                return self.end_of_input();
            };
            // Or it came while the line was read: the line does not run,
            // a command neither.
            if let Some(n) = self.rt.exit_code() {
                return self.finish(Some(n));
            }
            if line.trim().starts_with(':') {
                if let Some(code) = self.command(line.trim()) {
                    return self.finish(code);
                }
                continue;
            }
            // Keep reading while the text ends mid-block or
            // mid-expression.
            let mut src = line;
            while needs_more(&src) {
                let Some(more) = self.input.read_line(". ") else {
                    // The input ended inside a form: it is compiled
                    // anyway, so its error is reported, not dropped.
                    if let Some(code) = self.eval("<repl>", &src) {
                        return self.finish(Some(code));
                    }
                    return self.end_of_input();
                };
                src.push('\n');
                src.push_str(&more);
            }
            if let Some(n) = self.rt.exit_code() {
                return self.finish(Some(n)); // it came while the rest was read
            }
            if let Some(code) = self.eval("<repl>", &src) {
                return self.finish(Some(code));
            }
        }
    }

    /// The input ended: a terminal's prompt line is closed.
    fn end_of_input(mut self) -> i64 {
        if self.input.interactive() {
            let _ = self.out.write_all(b"\n");
        }
        self.finish(None)
    }

    /// Ends the session: the tasks still running do not keep it open
    /// (05). Without `exit(n)`, an error reported in the session (here,
    /// or a task failure the runtime reported) makes the code 1.
    fn finish(self, exit: Option<i64>) -> i64 {
        self.rt.cancel_all();
        self.rt.wait();
        match self.rt.exit_code().or(exit) {
            Some(n) => n,
            None if self.failed || self.rt.failed() => 1,
            None => 0,
        }
    }

    /// The `:` commands (08: `:help`, `:quit` and `:load` only). `Some`
    /// when the session ends: `Some(None)` for `:quit`, `Some(Some(n))`
    /// for an `exit(n)` in a loaded file.
    fn command(&mut self, line: &str) -> Option<Option<i64>> {
        let (name, arg) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let arg = arg.trim();
        // Leftover arguments are an error, as in every command (TL7).
        if matches!(name, ":help" | ":quit") && !arg.is_empty() {
            let _ = writeln!(self.err, "error: {name} takes no arguments");
            self.failed = true;
            return None;
        }
        match name {
            ":help" => {
                let _ = self.out.write_all(HELP.as_bytes());
            }
            ":quit" => return Some(None),
            ":load" => {
                if arg.is_empty() {
                    let _ = self.err.write_all(b"error: :load needs a file path\n");
                    self.failed = true;
                    return None;
                }
                match crate::read_file(arg.as_bytes()) {
                    Ok(src) => return self.eval(arg, &src).map(Some),
                    Err(fail) => {
                        let _ = self.err.write_all(
                            &[
                                b"error: ",
                                &crate::cannot_read(arg.as_bytes(), &fail.reason)[..],
                                b"\n",
                            ]
                            .concat(),
                        );
                        self.failed = true;
                    }
                }
            }
            _ => {
                let _ = writeln!(self.err, "error: unknown command {name}");
                self.failed = true;
            }
        }
        None
    }

    /// Compiles and runs one input against the session's globals and
    /// prints the value of a final expression. Returns the exit code when
    /// the input called `exit(n)`.
    fn eval(&mut self, name: &str, src: &str) -> Option<i64> {
        let loader = Loader {
            globals: self.globals.clone(),
            redefine: true,
            keep_value: true,
            ..Loader::default()
        };
        let from = self.known.len();
        let prior = std::mem::take(&mut self.known);
        let (b, errs) = loader.compile_after(prior, name, src, Stage::All);
        self.known = b.sources;
        // The runtime learns every file, even of a failed compile, so the
        // ids keep agreeing.
        self.rt.add_session_files(&self.known, from);
        if !errs.is_empty() {
            self.report(&errs);
            return None;
        }
        let entry = b.proto.expect("a compiled entry");
        self.rt.load(&entry, &b.modules);
        self.globals.clone_from(&entry.global_names);
        // A Ctrl-C that came while no input ran cancels nothing.
        self.rt.recover_interrupt();
        let r = self.rt.run_repl();
        // A Ctrl-C cut the input short: nothing is reported or printed,
        // and the session goes on on a fresh root. A second one dropped
        // its tasks while their cleanup still ran: one line says so.
        let abandoned = self.rt.take_abandoned();
        if self.rt.recover_interrupt() {
            // A deadlock reported before the Ctrl-C fails only this input,
            // as below (CC18).
            self.rt.recover_deadlock();
            if abandoned {
                let _ = writeln!(self.err, "{ABANDONED}");
            }
            return self.rt.exit_code();
        }
        // A spawned task's exit(n) ends the session like a direct one:
        // what it cut the input short with (E0701) is not reported, and
        // no value is printed. A deadlock's report asks for exit(1) too,
        // but only fails this input (below); a task's exit(n) after it
        // still ends the session (`recover_deadlock` keeps it).
        let deadlocked = self.rt.deadlocked();
        let exited = self.rt.exit_requested().is_some();
        match r {
            Ok(v) => {
                let last = b.surface.as_ref().and_then(|s| s.kids.last());
                if !exited && last.is_some_and(|k| !is_statement(k.head)) {
                    let _ = writeln!(self.out, "{v}");
                }
            }
            Err(Stop::Exit(n)) => return Some(n),
            Err(Stop::Error(d)) => {
                // The deadlock report showed where the input waits already.
                let told = deadlocked && d.code == "E0702";
                if !(exited || told) {
                    self.report(std::slice::from_ref(&*d));
                }
                if !exited {
                    self.failed = true;
                }
            }
        }
        if deadlocked {
            // The input failed with the deadlock report; the session goes
            // on on a fresh run root (decided 2026-10-17), unless a task
            // asked for an exit meanwhile (returned below).
            self.rt.recover_deadlock();
        }
        self.rt.exit_code()
    }

    /// Prints errors, each excerpted from the input its position names.
    fn report(&mut self, errs: &[Diag]) {
        self.failed = true;
        let mut err = ProcessOut::stderr();
        for e in errs {
            let _ = err.write_all(diag::format(e, &self.known).as_bytes());
        }
    }
}

/// Whether the text so far ends before its syntax is complete: the
/// parser failed exactly at the end of the input, or a raw string is
/// still open there (TL2: it may span lines, 01). A raw string is what
/// is open when closing one at the end ends the unterminated string.
fn needs_more(src: &str) -> bool {
    if src.trim().is_empty() {
        return false;
    }
    let toks = match lex(FileId(0), src) {
        Ok((toks, _)) => toks,
        // Open at the end: a raw string goes on on the next line (TL2); a
        // plain string cannot, so one still open once a closing backtick
        // is added (outside, or around an interpolation) is the error.
        Err(e) if e.code == "E0104" => {
            return lex(FileId(0), &format!("{src}`"))
                .err()
                .is_none_or(|e2| e2.code != "E0104");
        }
        Err(_) => return false,
    };
    let Err(e) = parse(&toks) else {
        return false;
    };
    toks.last().is_some_and(|eof| eof.pos == e.pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_more_when_the_parser_stops_at_the_end() {
        for src in ["fn f():", "1 +", "[1,", "if x:\n  1", "f(", "\"a\" +"] {
            assert!(needs_more(src), "{src:?}");
        }
        // A raw string open at the end goes on on the next line (TL2).
        for src in ["x = `a", "x = `a\nb", "`a` + `b"] {
            assert!(needs_more(src), "{src:?}");
        }
        for src in [
            "",
            "  ",
            "1",
            "x = )",
            "fn f() = 1",
            "\"open",
            "1 +\n2",
            "x = `a\nb`",
            "\"a${`x",
        ] {
            assert!(!needs_more(src), "{src:?}");
        }
    }
}
