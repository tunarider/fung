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
use fung_front::diag::{Arg, Diag};
use fung_front::loader::Loaded;
use fung_front::source::{FileId, SourceMap};

use super::task::CancelNode;
use super::world::{Shared, Token, World};
use super::{Stop, Task, block, internal, link, token};
use crate::value::Value;

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
    /// tree (the entry's place), holding the run token until it drops.
    pub(crate) fn task(&self) -> Task<'_> {
        Task::driven(&self.shared, self.shared.root())
    }

    /// A fresh task below the root: a test block. Cancelling it at the end
    /// ends what it spawned and left.
    pub(super) fn child_task(&self) -> Task<'_> {
        Task::driven(&self.shared, self.shared.root().child())
    }

    /// Holds the runtime open for more code the command will run one
    /// after another (`fung test`: the entry, then each test block). The
    /// command counts as a live task that is not blocked between them,
    /// so a task the entry left waiting for a later test block is not a
    /// deadlock when the entry ends; while a block runs, it stands in for
    /// the command (`Task::driven`), so a deadlock inside it is judged as
    /// usual. Call `release` once the program's tasks are cancelled and
    /// waited for.
    pub fn hold(&self) {
        let mut w = self.lock();
        if !w.held {
            w.held = true;
            w.live += 1;
        }
    }

    /// Ends `hold`. Nothing is judged: the command cancels and waits for
    /// the program's tasks first.
    pub fn release(&self) {
        let mut w = self.lock();
        debug_assert!(!w.hold_taken, "release while a driven task runs");
        if w.held {
            w.held = false;
            w.live -= 1;
        }
    }

    /// Waits until every spawned task has finished.
    pub fn wait(&self) {
        self.wait_until(|w| w.running == 0);
    }

    /// Waits until `done` holds; `idle` is woken whenever the spawned
    /// tasks drop to none.
    fn wait_until(&self, done: fn(&World) -> bool) {
        // On a node of its own: `cancel_all` must not cut this wait short.
        // Not live: the command waiting is not a task (`deadlock.rs`).
        let mut t = Task::unlisted(&self.shared, CancelNode::root());
        let r = block(
            &mut t,
            |w, me| {
                if done(w) {
                    return Some(());
                }
                w.idle.add(me);
                None
            },
            None,
        );
        debug_assert!(matches!(r, Ok(Some(()))));
    }

    /// Cancels every task below the root: an exit, an entry error, the end
    /// of a test run. Blocked tasks wake and unwind.
    pub fn cancel_all(&self) {
        self.shared.cancel_all();
    }

    /// Delivers stop requests from another thread (the command's signal
    /// thread, `stop.rs`).
    pub fn stop_handle(&self) -> super::StopHandle {
        super::StopHandle(self.shared.clone())
    }

    /// After a Ctrl-C (`StopHandle::interrupt`): waits for the input's
    /// tasks to unwind and gives the session a fresh run root; true if
    /// there was one. An exit a task asked for meanwhile stays.
    pub fn recover_interrupt(&self) -> bool {
        if !self.lock().interrupted {
            return false;
        }
        self.wait();
        let mut w = self.lock();
        w.interrupted = false;
        if w.exit.is_none() {
            self.shared.replace_root();
        }
        true
    }

    /// Forgets a stop request (`fung test`, before each test block).
    pub fn reset_stop(&self) {
        super::stop::reset(&mut self.lock());
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
    /// program's modules importable; `run` runs the entry.
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
        let r = t.run_main(main, globals);
        if let Err(Stop::Error(_)) = &r {
            // An entry error ends the program. Cancelled here, while the
            // entry still holds the token: its end is judged for a deadlock
            // when it drops, and the tasks it leaves must not count as
            // stuck by then.
            self.shared.root().cancel();
        }
        let v = r?;
        Ok(f(&v))
    }

    /// Makes the runtime a REPL session's (08): every entry it runs
    /// (`run_repl`) uses one global table, so what an input defines stays
    /// for the next, and reading a global whose assignment failed is E0808
    /// instead of an internal error.
    pub fn prepare_repl(&self) {
        let mut w = self.lock();
        let g = w.new_globals(0);
        w.repl = Some(g);
    }

    /// Appends `sources`' files from `from` on, with their texts: the
    /// REPL compiles each input after the files the runtime has
    /// (`Loader::compile_after`), so the ids agree, and a task failure
    /// report excerpts the input its code came from.
    pub fn add_session_files(&self, sources: &SourceMap, from: usize) {
        let mut w = self.lock();
        debug_assert_eq!(w.sources.len(), from, "the ids would not agree");
        for i in from..sources.len() {
            let id = FileId(i as u32);
            w.sources.add(sources.name(id), sources.text(id));
        }
    }

    /// Recovers a REPL session from a deadlock an input ran into
    /// (decided 2026-10-17). The report cancelled the run root: wait until
    /// every task of the program has unwound (those earlier inputs left
    /// blocked too), then clear the deadlock and the exit(1) it requested
    /// and start a fresh run root, so later inputs run as usual. Globals
    /// (atoms among them) stay.
    ///
    /// Only the report's own exit(1) is cleared: an exit(n) a task asked
    /// for after it stays (`exit_code` gives n), and the root is not
    /// replaced, so the session ends with n.
    /// Returns false, changing nothing, when no deadlock was reported.
    pub fn recover_deadlock(&self) -> bool {
        if !self.lock().deadlocked {
            return false;
        }
        self.wait();
        let mut w = self.lock();
        w.deadlocked = false;
        w.clear_deadlock_exit();
        if w.exit.is_none() {
            self.shared.replace_root();
        }
        true
    }

    /// Runs the loaded entry as a REPL input (`prepare_repl` first) and
    /// returns its value as the REPL prints it. An error does not cancel
    /// the program: the session goes on.
    pub fn run_repl(&self) -> Result<String, Stop> {
        let mut t = self.task();
        let Some(main) = t.world.main.clone() else {
            return Err(internal(vec![Arg::Str("run before load".into())]));
        };
        let Some(globals) = t.world.repl.clone() else {
            return Err(internal(vec![Arg::Str(
                "run_repl before prepare_repl".into(),
            )]));
        };
        let n = main.global_names.len();
        if n > globals.borrow().len() {
            globals.borrow_mut().resize(n, None);
        }
        let r = t.run_main(main, globals);
        r.map(|v| crate::value::format(&v))
    }

    /// Whether a task failure nobody handled was reported (a `supervise`
    /// scope cancelled before its handler took it; 05: the exit code
    /// becomes 1).
    pub fn failed(&self) -> bool {
        let _t = self.lock();
        self.shared.failures.failed()
    }

    /// Whether a deadlock was reported (it also requested exit(1)).
    pub fn deadlocked(&self) -> bool {
        self.lock().deadlocked
    }

    /// The code of the first exit(n) requested, if any.
    pub fn exit_code(&self) -> Option<i64> {
        self.lock().exit
    }

    /// The code of the first exit(n) a task requested, leaving out a
    /// deadlock report's exit(1) (the REPL recovers from that one).
    pub fn exit_requested(&self) -> Option<i64> {
        self.lock().task_exit()
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
            self.shared.root().cancel();
        }
        self.wait_until(|w| w.running == 0);
        let mut w = self.lock();
        token::assert_holds_token();
        w.release_values();
    }
}
