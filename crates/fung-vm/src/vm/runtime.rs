//! The runtime: the handle the command holds. It owns the shared part of
//! the runtime (`World`, behind the run token; see `world.rs`) and starts
//! tasks on it. Every method takes the token for as long as it needs it.
//!
//! **What the public API may pass.** fung values hold `Rc`s, which only
//! the token's holder may touch (constraints: 실행권 규칙). So no public
//! method takes a fung value in or gives one out, nor anything that
//! shares an `Rc` with one (a `Prelude`, an `RtProto`, a module): only
//! `Send` data without an `Rc` crosses it — front-end bundles in, and
//! `()`, `String`s, ints, `Diag`s and `Stop` (which carries a `Diag`)
//! out. The steps that make values (booting the prelude, linking the
//! entry, running it, running a test block) happen inside the runtime
//! with the token held, and what they make stays in the world. This is
//! the invariant the `unsafe impl Send for World` rests on.
//!
//! `Runtime` is `Send` (the command may hand it to another thread) but
//! not `Sync`: one thread drives it.

use std::cell::Cell;
use std::io::{BufRead, Write};
use std::marker::PhantomData;
use std::sync::Arc;

use fung_front::bytecode::Proto;
use fung_front::diag::{Arg, Diag, error};
use fung_front::loader::Loaded;
use fung_front::source::SourceMap;

use super::world::{Shared, Token, World};
use super::{Stop, Task, internal, link, no_pos, token};
use crate::value::Value;

/// Bounds a handler that keeps registering handlers.
const MAX_EXIT_HANDLERS: usize = 10_000;

pub struct Runtime {
    shared: Arc<Shared>,
    /// Makes `Runtime` `!Sync` (`Cell` is `Send` but not `Sync`) while it
    /// stays `Send`: the runtime is driven by one thread at a time. Without
    /// it, `Arc<Shared>` would make it `Sync` by accident. Pinned below.
    _not_sync: PhantomData<Cell<()>>,
}

/// Compile-time check that `Runtime` is not `Sync`, without crates: if it
/// were, both impls below would apply and the item path would be
/// ambiguous, which is a compile error. (`Send` is pinned by the test
/// `the_runtime_can_move_to_another_thread`.)
const _: fn() = || {
    trait AmbiguousIfSync<A> {
        fn some_item() {}
    }
    impl<T: ?Sized> AmbiguousIfSync<()> for T {}
    impl<T: ?Sized + Sync> AmbiguousIfSync<u8> for T {}
    let _ = <Runtime as AmbiguousIfSync<_>>::some_item;
};

impl Runtime {
    pub fn new(
        stdout: Box<dyn Write + Send>,
        stderr: Box<dyn Write + Send>,
        stdin: Box<dyn BufRead + Send>,
        sources: SourceMap,
    ) -> Runtime {
        Runtime {
            shared: Arc::new(Shared::new(World::new(stdout, stderr, stdin, sources))),
            _not_sync: PhantomData,
        }
    }

    /// Takes the run token on this thread.
    pub(super) fn lock(&self) -> Token<'_> {
        self.shared.lock()
    }

    /// A fresh task on this thread under the root of the cancellation
    /// tree (the entry's place, Go `rootTask`), holding the run token
    /// until it drops.
    pub(crate) fn task(&self) -> Task<'_> {
        Task::new(&self.shared, self.shared.root.clone())
    }

    /// A fresh task below the root (Go `newTask(rt.root())`): a test
    /// block. Cancelling it at the end ends what it spawned and left.
    pub(super) fn child_task(&self) -> Task<'_> {
        Task::new(&self.shared, self.shared.root.child())
    }

    /// Waits until every spawned task has finished (Go `Wait`).
    pub fn wait(&self) {
        let mut w = self.lock();
        while w.running > 0 {
            w.wait(None);
        }
    }

    /// Cancels every task below the root (Go `CancelAll`): an exit, an
    /// entry error, the end of a test run. Blocked tasks wake and unwind.
    pub fn cancel_all(&self) {
        self.shared.cancel_all();
    }

    /// The shared part, for tests that cancel from another thread.
    #[cfg(test)]
    pub(crate) fn shared(&self) -> Arc<Shared> {
        self.shared.clone()
    }

    /// Calls `f` with the source texts error reports excerpt, with the
    /// run token held.
    pub fn with_sources<R>(&self, f: impl FnOnce(&SourceMap) -> R) -> R {
        f(&self.lock().sources)
    }

    /// Appends another compile's files after the ones already here and
    /// returns how far its `FileId`s move (the number of files before
    /// them).
    pub fn add_sources(&self, other: &SourceMap) -> u32 {
        self.lock().add_sources(other)
    }

    /// The script arguments (the prelude's `args`).
    pub fn args(&self) -> Vec<String> {
        self.lock().args.clone()
    }

    /// Builds the prelude in the runtime (`crate::boot`); `args` become
    /// the prelude's `args` and the runtime's. Create the runtime with the
    /// program's sources first: boot appends the prelude's after them.
    ///
    /// The errors are prelude.fung's compile errors or the error of its
    /// body. Booting the embedded file does not fail in practice; a
    /// runtime whose boot failed should not be reused.
    pub fn boot(&self, args: Vec<String>) -> Result<(), Vec<Diag>> {
        let mut t = self.task();
        let p = crate::boot::boot(&mut t, args)?;
        t.world.prelude = p;
        Ok(())
    }

    /// Links a compiled entry against the booted prelude and makes the
    /// program's modules importable (Go `register`); `run` runs the entry.
    pub fn load(&self, entry: &Proto, modules: &[Loaded]) {
        let mut w = self.lock();
        let p = std::mem::take(&mut w.prelude);
        let main = link(entry, &p);
        crate::boot::register(&mut w, modules, &p);
        w.prelude = p;
        w.main = Some(main);
    }

    /// Runs the loaded entry on a fresh task.
    pub fn run(&self) -> Result<(), Stop> {
        self.run_with(|_| ())
    }

    /// Runs the loaded entry and hands its value to `f` while the token
    /// is still held (tests format it). `R: Send` keeps values out.
    pub(crate) fn run_with<R: Send>(&self, f: impl FnOnce(&Value) -> R) -> Result<R, Stop> {
        let mut t = self.task();
        let Some(main) = t.world.main.clone() else {
            return Err(internal(vec![Arg::Str("run before load".into())]));
        };
        let globals = t.world.new_globals(main.global_names.len());
        let v = t.run_main(main, globals)?;
        Ok(f(&v))
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
            // value never lives outside the run token. Handlers run below
            // their own root: the program's is cancelled by now.
            let mut t = Task::new(&self.shared, self.shared.handlers.child());
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

/// Test access to the prelude. A test builds a `Prelude` of natives (no
/// `Rc`) on its own thread, so these stay out of the public API.
#[cfg(test)]
impl Runtime {
    pub(crate) fn set_prelude(&self, p: super::Prelude) {
        self.lock().prelude = p;
    }

    pub(crate) fn with_prelude<R>(&self, f: impl FnOnce(&mut super::Prelude) -> R) -> R {
        f(&mut self.lock().prelude)
    }
}

impl Drop for Runtime {
    /// Ends the tasks still running (cancelled, then waited for: they
    /// read the world's tables) and drops every fung value the world
    /// holds while holding the run token (a value boundary: token.rs).
    /// What stays for the `Arc` to drop afterwards, possibly on a task's
    /// thread that is just ending (streams, sources, arguments), holds no
    /// `Rc`.
    fn drop(&mut self) {
        {
            let _t = self.lock();
            self.shared.root.cancel();
            self.shared.handlers.cancel();
            self.shared.notify_all();
        }
        self.wait();
        let mut w = self.lock();
        token::assert_holds_token();
        w.release_values();
    }
}
