//! The runtime: the handle the command holds. It owns the part every
//! task uses (`World`, see `world.rs`) and starts runs on it; each method
//! borrows the world for as long as it needs it.
//!
//! **What the public API passes.** No public method takes a fung value
//! in or gives one out, nor anything that shares an `Rc` with one (a
//! `Prelude`, an `RtProto`, a module): front-end bundles go in, and `()`,
//! `String`s, ints, `Diag`s and `Stop` (which carries a `Diag`) come out.
//! The steps that make values (booting the prelude, linking the entry,
//! running it, running a test block) happen inside the runtime, and what
//! they make stays in the world.
//!
//! The runtime is `!Send` (R5): fung values hold `Rc`s with non-atomic
//! counts, so it stays on the thread that made it, and the compiler
//! checks that. Other threads get `Shared` (the stop handle, the pool's
//! jobs), which holds no value.

use std::cell::{RefCell, RefMut};
use std::io::{BufRead, Write};
use std::sync::Arc;

use fung_front::bytecode::Proto;
use fung_front::diag::{Arg, Diag};
use fung_front::loader::Loaded;
use fung_front::source::{FileId, SourceMap};

use super::world::{Shared, World};
use super::{Stop, Task, internal, link};
use crate::value::Value;

pub struct Runtime {
    /// Borrowed by a run's `Task` for the whole run, and by the methods
    /// below between runs. It holds fung values, so the runtime is
    /// `!Send`: it stays on the thread that made it (`tests.rs` pins it).
    world: RefCell<World>,
    shared: Arc<Shared>,
}

impl Runtime {
    pub fn new(
        stdout: Box<dyn Write + Send>,
        stderr: Box<dyn Write + Send>,
        stdin: Box<dyn BufRead + Send>,
        sources: SourceMap,
    ) -> Runtime {
        let world = World::new(stdout, stderr, stdin, sources);
        Runtime {
            shared: Arc::new(Shared::new(&world)),
            world: RefCell::new(world),
        }
    }

    /// The world, between runs.
    pub(super) fn world(&self) -> RefMut<'_, World> {
        self.world.borrow_mut()
    }

    /// A fresh task on this thread under the root of the cancellation
    /// tree (the entry's place), on this run thread.
    pub(crate) fn task(&self) -> Task<'_> {
        Task::new(&self.world, &self.shared, self.shared.root())
    }

    /// A fresh task below the root: a test block. Cancelling it at the end
    /// ends what it spawned and left.
    pub(super) fn child_task(&self) -> Task<'_> {
        Task::new(&self.world, &self.shared, self.shared.root().child())
    }

    /// Waits until every spawned task has finished. Every run ends only
    /// once all its tasks have (`sched.rs`), so there is nothing left to
    /// wait for when the command calls this; it stays so the command's
    /// steps read as before (cancel what is left, wait, report).
    pub fn wait(&self) {}

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
        let mut w = self.world();
        super::stop::apply_posted(&mut w, &self.shared);
        // A second Ctrl-C that came after the run ended drops nothing.
        w.abandon = false;
        if !w.interrupted {
            return false;
        }
        w.interrupted = false;
        if w.exit.is_none() {
            self.shared.replace_root();
        }
        true
    }

    /// Whether the last run's tasks were dropped by a second Ctrl-C
    /// (`Sched::abandon`); reading it forgets it.
    pub fn take_abandoned(&self) -> bool {
        std::mem::take(&mut self.world().abandoned)
    }

    /// Forgets a stop request (`fung test`, before each test block).
    pub fn reset_stop(&self) {
        let mut w = self.world();
        super::stop::apply_posted(&mut w, &self.shared);
        super::stop::reset(&mut w, &self.shared);
    }

    /// The shared part, for tests that cancel from another thread.
    #[cfg(test)]
    pub(crate) fn shared(&self) -> Arc<Shared> {
        self.shared.clone()
    }

    /// Calls `f` with the source texts error reports excerpt, with the
    /// world borrowed.
    pub fn with_sources<R>(&self, f: impl FnOnce(&SourceMap) -> R) -> R {
        f(&self.world().sources)
    }

    /// Appends another compile's files after the ones already here and
    /// returns how far its `FileId`s move (the number of files before
    /// them).
    pub fn add_sources(&self, other: &SourceMap) -> u32 {
        self.world().add_sources(other)
    }

    /// The script arguments (the prelude's `args`).
    pub fn args(&self) -> Vec<String> {
        self.world().args.clone()
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
        let mut w = self.world();
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

    /// Runs the loaded entry and hands its value to `f` while the task
    /// is still alive (tests format it). `R: Send` keeps values out.
    pub(crate) fn run_with<R: Send>(&self, f: impl FnOnce(&Value) -> R) -> Result<R, Stop> {
        let mut t = self.task();
        let Some(main) = t.world.main.clone() else {
            return Err(internal(vec![Arg::Str("run before load".into())]));
        };
        let globals = t.world.new_globals(main.global_names.len());
        let r = t.run_main(main, globals);
        if let Err(Stop::Error(_)) = &r {
            // An entry error ends the program. Cancelled here, while the
            // entry is still alive: its end is judged for a deadlock
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
        let mut w = self.world();
        let g = w.new_globals(0);
        w.repl = Some(g);
    }

    /// Appends `sources`' files from `from` on, sharing their texts (no
    /// second copy per input, `SourceMap::add_from`): the
    /// REPL compiles each input after the files the runtime has
    /// (`Loader::compile_after`), so the ids agree, and a task failure
    /// report excerpts the input its code came from.
    pub fn add_session_files(&self, sources: &SourceMap, from: usize) {
        let mut w = self.world();
        debug_assert_eq!(w.sources.len(), from, "the ids would not agree");
        for i in from..sources.len() {
            w.sources.add_from(sources, FileId(i as u32));
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
        let mut w = self.world();
        super::stop::apply_posted(&mut w, &self.shared);
        if !w.deadlocked {
            return false;
        }
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
        let mut w = self.world();
        super::stop::apply_posted(&mut w, &self.shared);
        self.shared.failures.failed()
    }

    /// Whether a deadlock was reported (it also requested exit(1)).
    pub fn deadlocked(&self) -> bool {
        let mut w = self.world();
        super::stop::apply_posted(&mut w, &self.shared);
        w.deadlocked
    }

    /// The code of the first exit(n) requested, if any.
    pub fn exit_code(&self) -> Option<i64> {
        let mut w = self.world();
        super::stop::apply_posted(&mut w, &self.shared);
        w.exit
    }

    /// The code of the first exit(n) a task requested, leaving out a
    /// deadlock report's exit(1) (the REPL recovers from that one).
    pub fn exit_requested(&self) -> Option<i64> {
        let mut w = self.world();
        super::stop::apply_posted(&mut w, &self.shared);
        w.task_exit()
    }
}

/// Test access to the prelude. A test builds a `Prelude` of natives (no
/// `Rc`) on its own thread, so these stay out of the public API.
#[cfg(test)]
impl Runtime {
    pub(crate) fn set_prelude(&self, p: super::Prelude) {
        self.world().prelude = p;
    }

    pub(crate) fn with_prelude<R>(&self, f: impl FnOnce(&mut super::Prelude) -> R) -> R {
        f(&mut self.world().prelude)
    }
}

impl Drop for Runtime {
    /// Drops every fung value the world holds, on the run thread (`Runtime`
    /// is `!Send`). No task is left: every run ends only
    /// once its tasks have. What stays for the `Arc` to drop afterwards
    /// (streams, sources, arguments) holds no `Rc`.
    fn drop(&mut self) {
        let mut w = self.world();
        w.release_values();
    }
}
