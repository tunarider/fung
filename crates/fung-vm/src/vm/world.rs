//! What the runtime keeps between tasks (09).
//!
//! The runtime is split in two. `World` is the part every task uses: the
//! streams, the source texts for error reports, the exit code, the
//! script arguments, the global tables, the test blocks and the modules.
//! A `Task` (`vm.rs`) is the per-task part: its value stack, its frames
//! and the native it is calling.
//!
//! One thread runs every task (`sched.rs`, R1b), so the world is no
//! longer behind a lock (R5): the `Runtime` owns it in a `RefCell`, and a
//! run's `Task` borrows it for the whole run. It holds fung values, whose
//! `Rc` counts are not atomic, so it is `!Send`, and so is the runtime:
//! the compiler keeps every value on its thread. What other threads use
//! (the signal thread, the pool's jobs, the standard input reader) is
//! `Shared`, which holds only what is safe to share: the ready queue,
//! the event loop, the pool, the stop flag and the cancellation root.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::rc::Rc;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use fung_front::source::{FileId, SourceMap};

use super::failure::Failures;
use super::module::ModuleState;
use super::poller::Poller;
use super::pool::{POOL_IDLE, POOL_MAX, Pool};
use super::sched::{FiberId, Ready};
use super::stdin::Stdin;
use super::task::CancelNode;
use super::{Globals, Prelude, RtProto, TestCase};

/// The shared part of the runtime. Reachable only through the
/// runtime's `RefCell`, that is, only by the run thread.
pub(crate) struct World {
    stdout: Box<dyn Write + Send>,
    /// Shared with `Failures`, which reports on it when a handle drops.
    stderr: Stream,
    /// Read by a thread of its own (`stdin.rs`).
    pub(super) stdin: Stdin,
    pub(super) sources: SourceMap,
    /// The files `add_sources` added (the boot's embedded extras files),
    /// as a range of ids: the REPL adds its inputs' files after them.
    embedded: Option<std::ops::Range<u32>>,
    /// The first exit(n) requested anywhere.
    pub(super) exit: Option<i64>,
    /// `exit` was set by a deadlock report, not by a task's `exit(n)`.
    pub(super) exit_by_deadlock: bool,
    /// The first exit(n) a task requested after a deadlock report set
    /// `exit`: `Runtime::recover_deadlock` keeps it.
    exit_after_deadlock: Option<i64>,
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
    /// Tasks spawned during the current turn, which the scheduler takes
    /// in after it (`sched.rs`).
    pub(super) new_fibers: Vec<super::Fiber>,
    /// When the current turn began, another task was ready to run or
    /// parked on a timer or the world outside (`Sched::turn`).
    pub(super) others_waiting: bool,
    /// A run is going (`Task::run_root`), which allows no other inside it.
    pub(super) in_run: bool,
    /// The tasks parked on a descriptor, by the token of their
    /// registration in the event loop (`park_io`).
    pub(super) io: std::collections::HashMap<u64, (Arc<super::block::Waiter>, super::block::IoKey)>,
    /// The tokens in `io` by descriptor, so that a close looks only at
    /// its own (`drop_io_waits`). Kept by `add_io` and `take_io`.
    pub(super) io_fds: std::collections::HashMap<std::os::fd::RawFd, Vec<u64>>,
    /// The REPL's entry globals (`Runtime::prepare_repl`): one table every
    /// input runs on, so the values and the closures of earlier inputs
    /// see later definitions (08).
    pub(super) repl: Option<Globals>,
    /// A deadlock was reported (it is reported once).
    pub(super) deadlocked: bool,
    /// The shield nodes of the `with` releases running now
    /// (`cleanup.rs`): a deadlock while one runs cuts it short. Keyed by
    /// the node's address, so a release that ends leaves at O(1) however
    /// many run (review C5).
    pub(super) shields: std::collections::HashMap<usize, Weak<CancelNode>>,
    /// The stop request and its channel (`stop.rs`).
    pub(super) stop: super::stop::StopState,
    /// The REPL's evaluation was interrupted (`StopHandle::interrupt`).
    pub(super) interrupted: bool,
    /// Interrupted again while still running: drop its tasks
    /// (`Sched::abandon`). Reset by `Runtime::take_abandoned`.
    pub(super) abandon: bool,
    /// The last run's tasks were dropped that way.
    pub(super) abandoned: bool,
}

impl World {
    /// Puts a registration in the table (and a descriptor's in the fd
    /// index). One wait at a time per descriptor and interest: a second
    /// registration would replace the first in the loop, and the first
    /// task would never wake.
    pub(super) fn add_io(
        &mut self,
        token: u64,
        waiter: Arc<super::block::Waiter>,
        key: super::block::IoKey,
    ) {
        if let super::block::IoKey::Fd(fd, interest) = key {
            let tokens = self.io_fds.entry(fd).or_default();
            debug_assert!(
                !tokens.iter().any(|t| matches!(
                    self.io.get(t),
                    Some((_, super::block::IoKey::Fd(_, i))) if *i == interest
                )),
                "two waits on one fd and interest"
            );
            tokens.push(token);
        }
        self.io.insert(token, (waiter, key));
    }

    /// Takes a registration out of the table and the fd index.
    pub(super) fn take_io(
        &mut self,
        token: u64,
    ) -> Option<(Arc<super::block::Waiter>, super::block::IoKey)> {
        let e = self.io.remove(&token)?;
        if let super::block::IoKey::Fd(fd, _) = e.1
            && let Some(tokens) = self.io_fds.get_mut(&fd)
        {
            tokens.retain(|t| *t != token);
            if tokens.is_empty() {
                self.io_fds.remove(&fd);
            }
        }
        Some(e)
    }

    /// Takes every registration out (`Sched::abandon`).
    pub(super) fn take_all_io(&mut self) -> Vec<super::block::IoKey> {
        self.io_fds.clear();
        self.io.drain().map(|(_, (_, k))| k).collect()
    }

    pub(super) fn new(
        stdout: Box<dyn Write + Send>,
        stderr: Box<dyn Write + Send>,
        stdin: Box<dyn BufRead + Send>,
        sources: SourceMap,
    ) -> World {
        World {
            stdout,
            stderr: Stream::new(stderr),
            stdin: Stdin::new(stdin),
            sources,
            embedded: None,
            repl: None,
            exit: None,
            args: Vec::new(),
            globals: Vec::new(),
            tests: Vec::new(),
            modules: BTreeMap::new(),
            prelude: Prelude::default(),
            main: None,
            new_fibers: Vec::new(),
            others_waiting: false,
            in_run: false,
            io: std::collections::HashMap::new(),
            io_fds: std::collections::HashMap::new(),
            deadlocked: false,
            shields: std::collections::HashMap::new(),
            exit_by_deadlock: false,
            exit_after_deadlock: None,
            stop: Default::default(),
            interrupted: false,
            abandon: false,
            abandoned: false,
        }
    }

    /// Appends another compile's files after the ones already here and
    /// returns how far its `FileId`s move.
    /// Only the boot adds files this way: the embedded extras files
    /// (`is_embedded`).
    pub(crate) fn add_sources(&mut self, other: &SourceMap) -> u32 {
        let base = self.sources.len() as u32;
        for i in 0..other.len() as u32 {
            let id = FileId(i);
            self.sources.add(other.name(id), other.text(id));
        }
        let end = self.sources.len() as u32;
        let r = self.embedded.get_or_insert(base..end);
        r.end = end;
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

    /// A task's exit(n): the first request wins (05). One that comes
    /// after a deadlock report's own is remembered apart, so a recovered
    /// session does not lose it (`Runtime::recover_deadlock`).
    pub(super) fn request_exit(&mut self, code: i64) {
        match self.exit {
            None => self.exit = Some(code),
            Some(_) if self.exit_by_deadlock => {
                self.exit_after_deadlock.get_or_insert(code);
            }
            Some(_) => {}
        }
    }

    /// A deadlock report's exit(1): the code if none was requested yet.
    pub(super) fn request_deadlock_exit(&mut self) {
        if self.exit.is_none() {
            self.exit = Some(1);
            self.exit_by_deadlock = true;
        }
    }

    /// The exit a task requested (not a deadlock report's), if any.
    pub(super) fn task_exit(&self) -> Option<i64> {
        if self.exit_by_deadlock {
            self.exit_after_deadlock
        } else {
            self.exit
        }
    }

    /// Forgets a deadlock report's exit(1); a task's later request, if
    /// any, takes its place.
    pub(super) fn clear_deadlock_exit(&mut self) {
        if self.exit_by_deadlock {
            self.exit_by_deadlock = false;
            self.exit = self.exit_after_deadlock.take();
        }
    }

    /// The shield nodes of the releases running now; forgets the ended.
    pub(super) fn live_shields(&mut self) -> Vec<Arc<CancelNode>> {
        self.shields.retain(|_, w| w.strong_count() > 0);
        self.shields.values().filter_map(Weak::upgrade).collect()
    }

    pub(super) fn stdout(&mut self) -> &mut dyn Write {
        &mut *self.stdout
    }

    pub(super) fn stderr(&mut self) -> &mut dyn Write {
        &mut self.stderr
    }

    /// Empties everything that holds fung values, dropping them here: the
    /// caller is the run thread. The global tables are emptied first, which
    /// breaks the closure ↔ globals cycles.
    pub(super) fn release_values(&mut self) {
        let globals = std::mem::take(&mut self.globals);
        for g in &globals {
            let old = std::mem::take(&mut *g.borrow_mut());
            drop(old);
        }
        drop(globals);
        drop(std::mem::take(&mut self.tests));
        drop(std::mem::take(&mut self.modules));
        drop(std::mem::take(&mut self.prelude));
        drop(self.main.take());
        super::stop::release(self);
    }
}

/// A stream shared by the world and `Failures` (standard error). Each
/// write locks it for that write only; it is only written on the run
/// thread, so the lock is never contended.
#[derive(Clone)]
pub(crate) struct Stream(Arc<Mutex<Box<dyn Write + Send>>>);

impl Stream {
    fn new(w: Box<dyn Write + Send>) -> Stream {
        Stream(Arc::new(Mutex::new(w)))
    }
}

impl Write for Stream {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap_or_else(recover).write(b)
    }

    fn write_all(&mut self, b: &[u8]) -> std::io::Result<()> {
        self.0.lock().unwrap_or_else(recover).write_all(b)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap_or_else(recover).flush()
    }
}

/// What threads other than the run thread use too: the ready queue the
/// scheduler runs from (`sched.rs`), the event loop, the pool, the stop
/// flag and the root of the cancellation tree. All of it is `Send +
/// Sync` by itself.
pub(crate) struct Shared {
    /// The tasks to run next and the events other threads post.
    pub(super) ready: Arc<Ready>,
    /// The next task's number.
    next_fiber: AtomicU64,
    /// A stop was requested (`stop.rs`): a signal after it ends the
    /// process at once. Read by the signal thread, off the run thread.
    pub(super) stop_requested: AtomicBool,
    /// Failures nobody handled (`failure.rs`).
    pub(super) failures: Arc<Failures>,
    /// Closes what dropped file handles leave (`closer.rs`).
    pub(crate) closer: super::Closer,
    /// The threads offload jobs run on (`pool.rs`).
    pub(crate) pool: Pool,
    /// The event loop the run thread sleeps in (`poller.rs`, `sched.rs`).
    pub(crate) poller: Poller,
    /// Tokens for registrations in the loop, a fresh one per park.
    pub(super) next_token: AtomicU64,
    /// How many times a parked task was resumed (tests count wakeups).
    #[cfg(test)]
    pub(super) wakeups: AtomicUsize,
    /// How many tasks were parked when the scheduler last had nothing to
    /// run (a test probe another thread reads).
    #[cfg(test)]
    pub(crate) parked_probe: AtomicUsize,
    /// How many deadlines the scheduler kept at that moment (tests).
    #[cfg(test)]
    pub(crate) timers_probe: AtomicUsize,
    /// How many times the scheduler looked at the event loop (tests).
    #[cfg(test)]
    pub(crate) io_polls: AtomicUsize,
    /// Every task descends from this node: the entry runs on it,
    /// `cancel_all` cancels it. Replaced only by the REPL's recovery from a
    /// deadlock (`Runtime::recover_deadlock`), on the run thread and with
    /// no task of the program left; read it through `root()`.
    root: Mutex<Arc<CancelNode>>,
}

/// The runtime is gone: its pool's idle threads end at once (`fung test`
/// makes one runtime per file).
impl Drop for Shared {
    fn drop(&mut self) {
        self.pool.shutdown();
    }
}

impl Shared {
    pub(super) fn new(world: &World) -> Shared {
        // Without an event loop (no descriptor left, say) nothing could
        // wait: refuse at once.
        let poller = Poller::new().expect("cannot make the event loop");
        let failures = Arc::new(Failures::new(world.stderr.clone()));
        Shared {
            ready: Arc::new(Ready::new(poller.waker())),
            next_fiber: AtomicU64::new(1),
            stop_requested: AtomicBool::new(false),
            closer: super::Closer::new(failures.clone()),
            failures,
            pool: Pool::new(POOL_MAX, POOL_IDLE),
            poller,
            next_token: AtomicU64::new(1),
            #[cfg(test)]
            wakeups: AtomicUsize::new(0),
            #[cfg(test)]
            parked_probe: AtomicUsize::new(0),
            #[cfg(test)]
            timers_probe: AtomicUsize::new(0),
            #[cfg(test)]
            io_polls: AtomicUsize::new(0),
            root: Mutex::new(CancelNode::root()),
        }
    }

    /// A number for a new task.
    pub(super) fn next_fiber(&self) -> FiberId {
        self.next_fiber.fetch_add(1, Ordering::Relaxed)
    }

    /// How many times a parked task was resumed so far.
    #[cfg(test)]
    pub(crate) fn wakeups(&self) -> usize {
        self.wakeups.load(Ordering::Relaxed)
    }

    /// Cancels every task: the root and everything below it, and wakes the
    /// parked ones. Any thread may call it: a
    /// cancellation node is thread-safe, and a wake goes through the ready
    /// queue (`sched.rs`).
    pub(crate) fn cancel_all(&self) {
        self.root().cancel();
    }

    /// The run root (see the field).
    pub(super) fn root(&self) -> Arc<CancelNode> {
        self.root.lock().unwrap_or_else(recover).clone()
    }

    /// Gives the program a fresh run root. Every task of the program has
    /// ended.
    pub(super) fn replace_root(&self) {
        *self.root.lock().unwrap_or_else(recover) = CancelNode::root();
    }
}

/// A poisoned mutex (a stream's, the root's) means a thread panicked
/// while it held it; what it guards is left whole by every method, so
/// refusing the lock would only wedge the runtime. Take the guard as it
/// is.
fn recover<T>(e: PoisonError<T>) -> T {
    e.into_inner()
}
