//! The runtime: the handle the command holds. It owns the shared part of
//! the runtime (`World`, behind the run token; see `world.rs`) and starts
//! tasks on it. Every method takes the token for as long as it needs it.

use std::io::{BufRead, Write};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

use fung_front::diag::{Arg, Diag, error};
use fung_front::source::SourceMap;

use super::world::{Shared, Token, World};
use super::{RtProto, Stop, Task, no_pos, token};
use crate::value::Value;

/// Bounds a handler that keeps registering handlers.
const MAX_EXIT_HANDLERS: usize = 10_000;

pub struct Runtime {
    shared: Arc<Shared>,
}

/// The source texts, borrowed with the run token held (`Runtime::sources`).
pub struct Sources<'a>(Token<'a>);

impl Deref for Sources<'_> {
    type Target = SourceMap;

    fn deref(&self) -> &SourceMap {
        &self.0.sources
    }
}

impl Runtime {
    pub fn new(
        stdout: Box<dyn Write + Send>,
        stderr: Box<dyn Write + Send>,
        stdin: Box<dyn BufRead + Send>,
        sources: SourceMap,
    ) -> Runtime {
        Runtime {
            shared: Arc::new(Shared::new(World::new(stdout, stderr, stdin, sources))),
        }
    }

    /// Takes the run token on this thread.
    pub(super) fn lock(&self) -> Token<'_> {
        self.shared.lock()
    }

    /// A fresh task on this thread, holding the run token until it drops.
    pub(crate) fn task(&self) -> Task<'_> {
        Task::new(&self.shared)
    }

    /// The source texts error reports excerpt. The run token is held
    /// while the result lives: drop it before running fung code.
    pub fn sources(&self) -> Sources<'_> {
        Sources(self.lock())
    }

    /// Appends another compile's files after the ones already here and
    /// returns how far its `FileId`s move (the number of files before
    /// them). `boot` uses it for prelude.fung.
    pub fn add_sources(&self, other: &SourceMap) -> u32 {
        self.lock().add_sources(other)
    }

    /// The script arguments (the prelude's `args`).
    pub fn args(&self) -> Vec<String> {
        self.lock().args.clone()
    }

    pub fn set_args(&self, args: Vec<String>) {
        self.lock().args = args;
    }

    /// Runs a file body on a fresh task and returns its value.
    pub fn run(&self, main: Rc<RtProto>) -> Result<Value, Stop> {
        let mut t = self.task();
        let globals = t.world.new_globals(main.global_names.len());
        t.run_main(main, globals)
    }

    /// The code of the first exit(n) requested, if any.
    pub fn exit_code(&self) -> Option<i64> {
        self.lock().exit
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
    pub fn run_exit_handlers(&self) -> Vec<Diag> {
        let mut errs = Vec::new();
        let mut ran = 0;
        loop {
            // The handler is popped by the task that runs it, so the
            // value never lives outside the run token.
            let mut t = self.task();
            let Some(h) = t.world.exit_handlers.pop() else {
                break;
            };
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
            if let Err(Stop::Error(d)) = t.call(&h, Vec::new()) {
                errs.push(*d);
            }
        }
        errs
    }
}

impl Drop for Runtime {
    /// Drops every fung value the world holds while holding the run
    /// token (a value boundary: token.rs). What stays for the `Arc` to
    /// drop afterwards (streams, sources, arguments) holds no `Rc`.
    fn drop(&mut self) {
        let mut w = self.lock();
        token::assert_holds_token();
        w.release_values();
    }
}
