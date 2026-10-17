//! The read-eval-print loop (08, Go `internal/repl/repl.go` and
//! `cmd/fung/repl.go`): `fung` without arguments. Each input is compiled
//! against the names defined so far and runs on one shared runtime, so
//! definitions persist and functions may be defined again.
//!
//! How values persist: the runtime keeps one global table for the session
//! (`Runtime::prepare_repl`). Each input is compiled with the names of
//! the earlier inputs' globals pre-bound to the same indexes
//! (`Loader::globals`, Go `Options.Globals`) and runs on that table, so
//! the values stay, and a closure from an earlier input (which holds the
//! table) sees a later redefinition.
//!
//! Positions: the inputs are compiled after the files the runtime
//! already has (`Loader::compile_after`), so every position names one
//! file of the session. Go names files by name, and every input is
//! `<repl>`, so Go excerpts an error with the text of the input being
//! run, even when the code that failed came from an earlier one; the
//! port does the same (`excerpts`), and keeps no texts in the runtime,
//! as Go's REPL runtime has none (a task failure report shows no
//! excerpt).
//!
//! Tasks: the session holds the runtime open (`Runtime::hold`), so a task
//! an input leaves waiting for a later input is not a deadlock between
//! inputs (Go's 50 ms grace covers that only while the next line comes
//! quickly, as from a pipe). At the end (`:quit`, the end of input or
//! `exit(n)`) the tasks are cancelled and waited for, then the exit
//! handlers run, then the background tasks (agents) end, then task
//! failures nobody retrieved are reported (stage 3b order).

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
use crate::run::{ProcessOut, raw_stdin, status};
use line::{LineReader, PlainReader, TermReader};

/// The `:help` text (Go `helpText`).
const HELP: &str = "commands:
  :help            show this text
  :quit            leave (Ctrl-D does the same)
  :load file.fung  run a file in this session; its definitions stay
";

/// `fung` without arguments (Go `startRepl`): a terminal gets the line
/// editor, anything else plain line reading.
pub fn start() -> Out {
    let rt = Runtime::new(
        Box::new(ProcessOut::stdout()),
        Box::new(ProcessOut::stderr()),
        Box::new(std::io::BufReader::with_capacity(STDIN_BUFFER, raw_stdin())),
        SourceMap::new(),
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
        None => Box::new(PlainReader::new(raw_stdin(), ProcessOut::stdout())),
    };
    let code = Session::new(reader, rt).run();
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
    /// Every file compiled so far, with its text; the runtime holds the
    /// same ids with names only (`Runtime::add_names`).
    known: SourceMap,
    /// The entry globals defined so far, in index order.
    globals: Vec<String>,
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
        rt.hold();
        let known = rt.with_sources(SourceMap::clone);
        Session {
            input,
            out: ProcessOut::stdout(),
            err: ProcessOut::stderr(),
            rt,
            known,
            globals: Vec::new(),
        }
    }

    /// Reads inputs until `:quit`, the end of input or `exit(n)`, and
    /// returns the exit code: 0, or n for exit(n).
    fn run(mut self) -> i64 {
        loop {
            let Some(line) = self.input.read_line("> ") else {
                let _ = self.out.write_all(b"\n");
                return self.finish(0);
            };
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
                    let _ = self.out.write_all(b"\n");
                    return self.finish(0);
                };
                src.push('\n');
                src.push_str(&more);
            }
            if let Some(code) = self.eval("<repl>", &src) {
                return self.finish(code);
            }
        }
    }

    /// Ends the session (Go `finish`): the tasks still running do not
    /// keep it open (05), then the exit handlers, the background tasks'
    /// end and the failures nobody retrieved.
    fn finish(self, code: i64) -> i64 {
        self.rt.cancel_all();
        self.rt.wait();
        self.rt.release();
        for d in self.rt.run_exit_handlers() {
            if d.code == "E0701" || d.code == "E0702" {
                continue;
            }
            // Go formats these without a source text.
            let none = excerpts(&self.known, self.known.len());
            let _ = ProcessOut::stderr().write_all(diag::format(&d, &none).as_bytes());
        }
        self.rt.end_background();
        self.rt.wait();
        self.rt.report_unretrieved();
        self.rt.exit_code().unwrap_or(code)
    }

    /// The `:` commands (08: `:help`, `:quit` and `:load` only). Returns
    /// the exit code when the session ends.
    fn command(&mut self, line: &str) -> Option<i64> {
        let (name, arg) = line.split_once(' ').unwrap_or((line, ""));
        match name {
            ":help" => {
                let _ = self.out.write_all(HELP.as_bytes());
            }
            ":quit" => return Some(0),
            ":load" => {
                let arg = arg.trim();
                if arg.is_empty() {
                    let _ = self.err.write_all(b":load needs a file path\n");
                    return None;
                }
                match crate::read_file(arg.as_bytes()) {
                    Ok(src) => return self.eval(arg, &src),
                    Err(msg) => {
                        let _ = self.err.write_all(
                            &[b"cannot read ", arg.as_bytes(), b": ", &msg[..], b"\n"].concat(),
                        );
                    }
                }
            }
            _ => {
                let _ = writeln!(self.err, "unknown command {name}");
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
            ..Loader::default()
        };
        let from = self.known.len();
        let prior = std::mem::take(&mut self.known);
        let (b, errs) = loader.compile_after(prior, name, src, Stage::All);
        self.known = b.sources;
        // The runtime learns every file's name, even of a failed compile,
        // so the ids keep agreeing.
        self.rt.add_names(&self.known, from);
        if !errs.is_empty() {
            self.report(&errs, from);
            return None;
        }
        let entry = b.proto.expect("a compiled entry");
        self.rt.load(&entry, &b.modules);
        self.globals.clone_from(&entry.global_names);
        match self.rt.run_repl() {
            Ok(v) => {
                let last = b.surface.as_ref().and_then(|s| s.kids.last());
                if last.is_some_and(|k| !is_statement(k.head)) {
                    let _ = writeln!(self.out, "{v}");
                }
                None
            }
            Err(Stop::Exit(n)) => Some(n),
            Err(Stop::Error(d)) => {
                self.report(std::slice::from_ref(&*d), from);
                None
            }
        }
    }

    /// Prints errors the way Go's REPL does: a position is excerpted from
    /// the file of the same name in the input just compiled (files from
    /// `from` on), whichever input the failing code came from.
    fn report(&self, errs: &[Diag], from: usize) {
        let view = excerpts(&self.known, from);
        let mut err = ProcessOut::stderr();
        for e in errs {
            let _ = err.write_all(diag::format(e, &view).as_bytes());
        }
    }
}

/// The session's files with the texts Go would excerpt (Go formats with
/// `Sources[pos.File]`, a map by name of the current compile): every
/// file gets the text of the first file named like it among those from
/// `from` on, or none.
fn excerpts(known: &SourceMap, from: usize) -> SourceMap {
    let current: Vec<(&str, &str)> = (from..known.len())
        .map(|i| {
            let id = FileId(i as u32);
            (known.name(id), known.text(id))
        })
        .collect();
    let mut view = SourceMap::new();
    for i in 0..known.len() {
        let name = known.name(FileId(i as u32));
        let text = current
            .iter()
            .find(|(n, _)| *n == name)
            .map_or("", |(_, t)| t);
        view.add(name, text);
    }
    view
}

/// Whether the text so far ends before its syntax is complete: the
/// parser failed exactly at the end of the input (Go `needsMore`).
fn needs_more(src: &str) -> bool {
    if src.trim().is_empty() {
        return false;
    }
    let Ok((toks, _)) = lex(FileId(0), src) else {
        return false;
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
        for src in ["", "  ", "1", "x = )", "fn f() = 1", "\"open", "1 +\n2"] {
            assert!(!needs_more(src), "{src:?}");
        }
    }

    #[test]
    fn excerpts_follow_the_current_input_by_name() {
        let mut m = SourceMap::new();
        m.add("prelude", "p");
        m.add("<repl>", "old");
        m.add("lib", "lib text");
        m.add("<repl>", "new");
        let v = excerpts(&m, 3);
        let texts: Vec<_> = (0..4).map(|i| v.text(FileId(i))).collect();
        assert_eq!(texts, ["", "new", "", "new"]);
        assert_eq!(v.name(FileId(2)), "lib");
    }
}
