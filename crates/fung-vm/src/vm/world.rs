//! What every task shares (09, stage 3a), behind the run token.
//!
//! The runtime is split in two. `World` is the shared part: the streams,
//! the source texts for error reports, the exit handlers and exit code,
//! the script arguments, the global tables, the test blocks and the
//! modules. A `Task` (`vm.rs`) is the per-task part: its value stack, its
//! frames and the native it is calling.
//!
//! `World` sits in one `Mutex`, shared through an `Arc` with one `Condvar`
//! (stage 3a: 실행권 잠금). Holding that mutex is holding the run token:
//! only the holder runs fung code, so only the holder touches fung values,
//! whose `Rc` counts are not atomic. A task takes the token when it starts
//! and holds it for its whole run; the run loop never locks per
//! instruction. `Token` is the guard: it keeps the debug mark of
//! `token.rs` in step with the mutex, and it can give the mutex up and
//! take it back (waiting on the condvar, from Task 3 on) without ever
//! being a guard whose mark is wrong.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use fung_front::source::{FileId, SourceMap, decode_go};

use super::module::ModuleState;
use super::{Globals, Prelude, RtProto, TestCase, token};
use crate::value::Value;

/// The shared part of the runtime. Reachable only through `Shared`'s
/// mutex, that is, only by the thread holding the run token.
pub(crate) struct World {
    stdout: Box<dyn Write + Send>,
    stderr: Box<dyn Write + Send>,
    stdin: Box<dyn BufRead + Send>,
    pub(super) sources: SourceMap,
    pub(super) exit_handlers: Vec<Value>,
    /// The first exit(n) requested anywhere.
    pub(super) exit: Option<i64>,
    /// The script arguments (the prelude's `args`).
    pub(crate) args: Vec<String>,
    /// Every global table the runtime made. A closure stored in a global
    /// holds its table, an `Rc` cycle; dropping the runtime empties the
    /// tables to break it.
    pub(super) globals: Vec<Globals>,
    /// The test blocks the body registered (TEST).
    pub(super) tests: Vec<TestCase>,
    /// The importable modules by key (IMPORT).
    pub(super) modules: BTreeMap<String, ModuleState>,
    /// The booted prelude the entry and its modules link against.
    pub(crate) prelude: Prelude,
    /// The linked entry (`Runtime::load`), run by `Runtime::run`.
    pub(super) main: Option<Rc<RtProto>>,
}

// SAFETY: `World` holds fung values (`Rc`s with non-atomic counts). The
// invariant that makes sending it sound: every fung value that shares an
// `Rc` with a value reachable from the world is created, cloned and
// dropped only by the thread that holds `Shared::world`'s mutex (the run
// token). It holds because
// - `World` is reachable only through that mutex (`Shared` owns it, and
//   only `Token`, which marks the holder, derefs to it);
// - values leave the world only onto a `Task`'s stack and frames or into
//   locals of a function that holds a `Token` or `Task`, and the `Task` /
//   `Token` drops them before it releases the mutex (`Task`'s `Drop`
//   empties the stack and frames first; `Runtime`'s `Drop` empties the
//   world under the token);
// - the public API of `Runtime` (see `runtime.rs`) takes no fung value in
//   and gives none out: only `Send` data without an `Rc` crosses it, and
//   `Value` is itself `!Send`, so no value can be carried to another
//   thread around the API either.
// The streams are `Send` themselves.
unsafe impl Send for World {}

impl World {
    pub(super) fn new(
        stdout: Box<dyn Write + Send>,
        stderr: Box<dyn Write + Send>,
        stdin: Box<dyn BufRead + Send>,
        sources: SourceMap,
    ) -> World {
        World {
            stdout,
            stderr,
            stdin,
            sources,
            exit_handlers: Vec::new(),
            exit: None,
            args: Vec::new(),
            globals: Vec::new(),
            tests: Vec::new(),
            modules: BTreeMap::new(),
            prelude: Prelude::default(),
            main: None,
        }
    }

    /// Appends another compile's files after the ones already here and
    /// returns how far its `FileId`s move.
    pub(crate) fn add_sources(&mut self, other: &SourceMap) -> u32 {
        let base = self.sources.len() as u32;
        for i in 0..other.len() as u32 {
            let id = FileId(i);
            self.sources.add(other.name(id), other.text(id));
        }
        base
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

    pub(super) fn request_exit(&mut self, code: i64) {
        self.exit.get_or_insert(code);
    }

    pub(super) fn at_exit(&mut self, f: Value) {
        self.exit_handlers.push(f);
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

    /// Empties everything that holds fung values, dropping them here: the
    /// caller holds the token. The global tables are emptied first, which
    /// breaks the closure ↔ globals cycles.
    pub(super) fn release_values(&mut self) {
        token::assert_holds_token();
        let globals = std::mem::take(&mut self.globals);
        for g in &globals {
            let old = std::mem::take(&mut *g.borrow_mut());
            drop(old);
        }
        drop(globals);
        drop(std::mem::take(&mut self.exit_handlers));
        drop(std::mem::take(&mut self.tests));
        drop(std::mem::take(&mut self.modules));
        drop(std::mem::take(&mut self.prelude));
        drop(self.main.take());
    }
}

/// The run token's mutex and the condvar blocked tasks wait on.
pub(crate) struct Shared {
    world: Mutex<World>,
    /// Signalled when a blocked task may go on (Task 3 on).
    #[cfg_attr(not(test), allow(dead_code))]
    cv: Condvar,
}

impl Shared {
    pub(super) fn new(world: World) -> Shared {
        Shared {
            world: Mutex::new(world),
            cv: Condvar::new(),
        }
    }

    /// Takes the run token, waiting for the thread that holds it.
    pub(super) fn lock(&self) -> Token<'_> {
        token::assert_not_held();
        let guard = self.world.lock().unwrap_or_else(recover);
        token::mark_taken();
        Token {
            shared: self,
            guard: Some(guard),
        }
    }
}

/// A poisoned mutex means a thread panicked while it held the token. The
/// VM's panic fence already turned that panic into E0801 (or the panic is
/// a test's), and the world's own fields are plain containers that no
/// method leaves half-changed across a call that can panic, so refusing
/// the lock would only wedge the runtime: every later task, the exit
/// handlers and the runtime's drop would fail on a lock nobody can clear.
/// Take the guard as it is.
fn recover<T>(e: PoisonError<T>) -> T {
    e.into_inner()
}

/// The run token: the guard of `Shared::world` while this thread holds it.
/// The debug mark (`token.rs`) is set right after the mutex is locked and
/// cleared right before it is unlocked, in every path: `Shared::lock`,
/// `release`, `wait_timeout` and the drop.
///
/// The guard is an `Option` so the token can be given up and taken back
/// without dropping the `Token` (a task keeps one for its whole life): a
/// blocking point waits with `wait_timeout`, which hands the guard to the
/// condvar and puts back the one it returns.
pub(crate) struct Token<'a> {
    shared: &'a Shared,
    guard: Option<MutexGuard<'a, World>>,
}

impl Token<'_> {
    /// Gives the token up; the world is unreachable until `reacquire`.
    pub(super) fn release(&mut self) {
        if let Some(g) = self.guard.take() {
            token::mark_released();
            drop(g);
        }
    }

    /// Takes the token back if this `Token` gave it up (a no-op while it
    /// is held).
    pub(super) fn reacquire(&mut self) {
        if self.guard.is_none() {
            token::assert_not_held();
            let g = self.shared.world.lock().unwrap_or_else(recover);
            token::mark_taken();
            self.guard = Some(g);
        }
    }

    /// Waits on the condvar for at most `d`, giving the token up while it
    /// sleeps (atomically with starting to wait, so no signal is lost) and
    /// holding it again when it returns. True if the wait timed out.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn wait_timeout(&mut self, d: Duration) -> bool {
        let g = self.guard.take().expect("waiting without the run token");
        token::mark_released();
        let (g, r) = self.shared.cv.wait_timeout(g, d).unwrap_or_else(recover);
        token::mark_taken();
        self.guard = Some(g);
        r.timed_out()
    }
}

impl Deref for Token<'_> {
    type Target = World;

    fn deref(&self) -> &World {
        self.guard
            .as_deref()
            .expect("the world reached without the run token")
    }
}

impl DerefMut for Token<'_> {
    fn deref_mut(&mut self) -> &mut World {
        self.guard
            .as_deref_mut()
            .expect("the world reached without the run token")
    }
}

impl Drop for Token<'_> {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::Arc;

    fn shared() -> Arc<Shared> {
        Arc::new(Shared::new(World::new(
            Box::new(std::io::sink()),
            Box::new(std::io::sink()),
            Box::new(Cursor::new(Vec::new())),
            SourceMap::default(),
        )))
    }

    #[cfg(debug_assertions)]
    #[test]
    fn the_mark_follows_the_guard() {
        let s = shared();
        let mut t = s.lock();
        token::assert_holds_token();
        t.release();
        assert!(std::panic::catch_unwind(token::assert_holds_token).is_err());
        t.reacquire();
        token::assert_holds_token();
        drop(t);
        assert!(std::panic::catch_unwind(token::assert_holds_token).is_err());
    }

    #[test]
    fn a_wait_gives_the_token_to_another_thread() {
        let s = shared();
        let mut t = s.lock();
        t.exit = Some(1);
        let other = {
            let s = s.clone();
            std::thread::spawn(move || {
                let mut t = s.lock();
                token::assert_holds_token();
                t.exit = Some(2);
                s.cv.notify_all();
            })
        };
        while t.exit != Some(2) {
            t.wait_timeout(Duration::from_millis(10));
        }
        token::assert_holds_token();
        drop(t);
        other.join().unwrap();
    }

    #[test]
    fn a_poisoned_token_is_taken_as_it_is() {
        let s = shared();
        let s2 = s.clone();
        let r = std::thread::spawn(move || {
            let mut t = s2.lock();
            t.exit = Some(7);
            panic!("boom");
        })
        .join();
        assert!(r.is_err());
        assert_eq!(s.lock().exit, Some(7));
    }
}
