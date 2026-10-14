//! The runtime: what every task shares (09) — the streams, the source
//! texts for error reports, the exit handlers and the exit code.

use std::cell::RefCell;
use std::io::{BufRead, Write};
use std::rc::Rc;

use fung_front::diag::{Arg, Diag, error};
use fung_front::source::{FileId, SourceMap, decode_go};

use super::{Globals, RtProto, Stop, Task, no_pos};
use crate::value::Value;

/// Bounds a handler that keeps registering handlers.
const MAX_EXIT_HANDLERS: usize = 10_000;

pub struct Runtime {
    stdout: Box<dyn Write>,
    stderr: Box<dyn Write>,
    stdin: Box<dyn BufRead>,
    sources: SourceMap,
    exit_handlers: Vec<Value>,
    /// The first exit(n) requested anywhere.
    exit: Option<i32>,
    /// The script arguments (the prelude's `args`).
    pub args: Vec<String>,
    /// Every global table the runtime made. A closure stored in a global
    /// holds its table, an `Rc` cycle; dropping the runtime empties the
    /// tables to break it.
    globals: Vec<Globals>,
}

impl Runtime {
    pub fn new(
        stdout: Box<dyn Write>,
        stderr: Box<dyn Write>,
        stdin: Box<dyn BufRead>,
        sources: SourceMap,
    ) -> Runtime {
        Runtime {
            stdout,
            stderr,
            stdin,
            sources,
            exit_handlers: Vec::new(),
            exit: None,
            args: Vec::new(),
            globals: Vec::new(),
        }
    }

    /// The source texts error reports excerpt.
    pub fn sources(&self) -> &SourceMap {
        &self.sources
    }

    /// A file's name for call paths ("" for a file this runtime does not
    /// know, rather than a panic while reporting an error).
    pub(super) fn file_name(&self, id: FileId) -> &str {
        if (id.0 as usize) < self.sources.len() {
            self.sources.name(id)
        } else {
            ""
        }
    }

    /// A fresh global table for a file body.
    pub(super) fn new_globals(&mut self, n: usize) -> Globals {
        let g = Rc::new(RefCell::new(vec![None; n]));
        self.globals.push(g.clone());
        g
    }

    /// Runs a file body on a fresh task and returns its value.
    pub fn run(&mut self, main: Rc<RtProto>) -> Result<Value, Stop> {
        let globals = self.new_globals(main.global_names.len());
        Task::new(self).run_main(main, globals)
    }

    /// The code of the first exit(n) requested, if any.
    pub fn exit_code(&self) -> Option<i32> {
        self.exit
    }

    pub(super) fn request_exit(&mut self, code: i32) {
        self.exit.get_or_insert(code);
    }

    pub(super) fn at_exit(&mut self, f: Value) {
        self.exit_handlers.push(f);
    }

    /// Runs the at_exit handlers in reverse registration order, each on a
    /// fresh task, and returns the errors they raised; one failing handler
    /// does not stop the others. Handlers registered while exiting run too
    /// (like C's atexit), up to a limit (E0806).
    ///
    /// An exit(n) inside a handler ends that handler and records n if no
    /// exit was requested before; it is not an error, so it is not
    /// returned. The handlers after it still run: Go cancels them through
    /// a context whose flag a goroutine sets, so in practice a short
    /// handler runs to the end; Rust makes that the rule.
    pub fn run_exit_handlers(&mut self) -> Vec<Diag> {
        let mut errs = Vec::new();
        let mut ran = 0;
        while let Some(h) = self.exit_handlers.pop() {
            if ran >= MAX_EXIT_HANDLERS {
                errs.push(error(
                    "E0806",
                    no_pos(),
                    vec![
                        Arg::Str("at_exit handlers".into()),
                        Arg::Int(MAX_EXIT_HANDLERS as i64),
                    ],
                ));
                break;
            }
            ran += 1;
            if let Err(Stop::Error(d)) = Task::new(self).call(&h, Vec::new()) {
                errs.push(*d);
            }
        }
        errs
    }

    pub(super) fn stdout(&mut self) -> &mut dyn Write {
        &mut *self.stdout
    }

    pub(super) fn stderr(&mut self) -> &mut dyn Write {
        &mut *self.stderr
    }

    /// One line of standard input with its line ending, `None` at the end.
    /// Bytes that are not UTF-8 become U+FFFD one by one, like source text.
    pub(super) fn read_line(&mut self) -> Option<String> {
        let mut buf = Vec::new();
        let r = self.stdin.read_until(b'\n', &mut buf);
        if buf.is_empty() {
            let _ = r;
            return None;
        }
        Some(decode_go(&buf))
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        for g in &self.globals {
            let old = std::mem::take(&mut *g.borrow_mut());
            drop(old);
        }
    }
}
