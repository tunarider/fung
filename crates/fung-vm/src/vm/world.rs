//! What every task shares (09, stage 3a), behind the run token.
//!
//! The runtime is split in two. `World` is the shared part: the streams,
//! the source texts for error reports, the exit handlers and exit code,
//! the script arguments, the global tables, the test blocks and the
//! modules. A `Task` (`vm.rs`) is the per-task part: its value stack, its
//! frames and the native it is calling.
//!
//! `World` sits in one `Mutex`, shared through an `Arc` (stage 3a:
//! 실행권 잠금), with a fair queue in front of it (`Gate`). A blocked task
//! sleeps on its own `Waiter` (`block.rs`) without the token, and only
//! what it waits for wakes it. Owning the gate and holding that
//! mutex is holding the run token: only the holder runs fung code, so
//! only the holder touches fung values, whose `Rc` counts are not atomic.
//! A task takes the token when it starts and holds it while it runs,
//! giving it up only to wait (`block`), to let waiting tasks have a turn
//! (`yield_turn`) or to call the operating system (`outside`); the run
//! loop never locks per instruction. `Token` is the guard: it keeps the
//! debug mark of `token.rs` in step with the mutex, and it can give the
//! mutex up and take it back without ever being a guard whose mark is
//! wrong.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, Write};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::Thread;
use std::time::Instant;

use fung_front::source::{FileId, SourceMap};

use super::block::{Census, WaitList, Waiter};
use super::failure::Failures;
use super::module::ModuleState;
use super::stdin::Stdin;
use super::task::{CancelNode, Kind};
use super::{Globals, Prelude, RtProto, TestCase, token};
use crate::value::Value;

/// The shared part of the runtime. Reachable only through `Shared`'s
/// mutex, that is, only by the thread holding the run token.
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
    /// Spawned tasks that have not finished (`Runtime::wait`).
    pub(super) running: usize,
    /// Woken when `running` drops to 0.
    pub(super) idle: WaitList,
    /// Tasks running fung code (Go `concState.live`): the entry, spawned
    /// tasks from the moment they are spawned, test blocks, exit handlers
    /// and the prelude's boot; not `Runtime::wait`'s observer. The
    /// deadlock check (`deadlock.rs`) compares it with `Census::blocked`.
    pub(super) live: usize,
    /// Background tasks that have not finished (stage 3b): spawned by
    /// `spawn_background` or by another background task, counted from
    /// the spawn until their `Task` drops. Not in `running` or `live`;
    /// the deadlock check counts them apart (`deadlock.rs`), and `idle`
    /// is woken when this drops to 0 too (`Runtime::end_background`).
    pub(super) background: usize,
    /// The command holds one count of `live` between the tasks it runs
    /// one after another (`Runtime::hold`, `fung test`): it is about to
    /// run more fung code, so it is live and not blocked. The task it
    /// runs takes that count over while it runs (`Task::driven`).
    pub(super) held: bool,
    /// A driven task has taken the held count over and has not dropped
    /// yet. There is at most one: `Task::driven` asserts it.
    pub(super) hold_taken: bool,
    /// The REPL's entry globals (`Runtime::prepare_repl`): one table every
    /// input runs on, so the values and the closures of earlier inputs
    /// see later definitions (08).
    pub(super) repl: Option<Globals>,
    /// A deadlock was reported (it is reported once).
    pub(super) deadlocked: bool,
    /// The cancellation node of the exit handler running now: a deadlock
    /// while it runs cuts that handler short (`deadlock.rs`).
    pub(super) handler: Option<Arc<CancelNode>>,
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
// Tasks on other threads (stage 3a) keep to the same rule: a spawned
// task's thread touches values only between taking the token and
// giving it up (`task.rs`).
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
            stderr: Stream::new(stderr),
            stdin: Stdin::new(stdin),
            sources,
            embedded: None,
            repl: None,
            exit_handlers: Vec::new(),
            exit: None,
            args: Vec::new(),
            globals: Vec::new(),
            tests: Vec::new(),
            modules: BTreeMap::new(),
            prelude: Prelude::default(),
            main: None,
            running: 0,
            idle: WaitList::default(),
            live: 0,
            held: false,
            hold_taken: false,
            background: 0,
            deadlocked: false,
            handler: None,
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

    /// Whether a file is one of the embedded extras files the boot added
    /// (`agent.rs`: a position there is never shown for E0704).
    pub(super) fn is_embedded(&self, id: FileId) -> bool {
        self.embedded.as_ref().is_some_and(|r| r.contains(&id.0))
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

    /// Counts a task being spawned (`up`) or one whose spawn failed: a
    /// normal task is running and live, a background task is counted
    /// apart.
    pub(super) fn count(&mut self, kind: Kind, up: bool) {
        match (kind.background, up) {
            (false, true) => {
                self.running += 1;
                self.live += 1;
            }
            (false, false) => {
                self.running -= 1;
                self.live -= 1;
            }
            (true, true) => self.background += 1,
            (true, false) => self.background -= 1,
        }
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
        &mut self.stderr
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

/// A stream shared by the world and `Failures` (standard error). Each
/// write locks it for that write only; it is only written with the run
/// token held, so the lock is never contended.
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

/// The run token's mutex, the fair queue in front of it, and the roots of
/// the cancellation tree. (Blocked tasks sleep on their own `Waiter`.)
pub(crate) struct Shared {
    world: Mutex<World>,
    /// How many tasks are blocked, for the deadlock check (`block.rs`).
    pub(super) census: Arc<Census>,
    /// Failures nobody retrieved yet (`failure.rs`).
    pub(super) failures: Arc<Failures>,
    /// How many times a sleeping task woke up (tests count wakeups).
    #[cfg(test)]
    wakeups: AtomicUsize,
    gate: Gate,
    /// Every task descends from this node (Go `rt.root()`): the entry
    /// runs on it, `cancel_all` cancels it.
    pub(super) root: Arc<CancelNode>,
    /// Exit handlers run under this node, not the root: the root is
    /// cancelled by then (Go `handlerContext`).
    pub(super) handlers: Arc<CancelNode>,
    /// Background tasks (stage 3b) hang from this node, apart from the
    /// program's root: an exit or an entry error cancels the program,
    /// and the background tasks are cancelled only after the exit
    /// handlers (`Runtime::end_background`).
    pub(super) background: Arc<CancelNode>,
}

impl Shared {
    pub(super) fn new(world: World) -> Shared {
        let failures = Arc::new(Failures::new(world.stderr.clone()));
        Shared {
            world: Mutex::new(world),
            census: Arc::default(),
            failures,
            #[cfg(test)]
            wakeups: AtomicUsize::new(0),
            gate: Gate::default(),
            root: CancelNode::root(),
            handlers: CancelNode::root(),
            background: CancelNode::root(),
        }
    }

    /// Takes the run token, waiting for the thread that holds it. Waiters
    /// get the token in the order they asked for it (`Gate`).
    pub(super) fn lock(&self) -> Token<'_> {
        token::assert_not_held();
        self.gate.enter();
        let guard = self.world.lock().unwrap_or_else(recover);
        token::mark_taken();
        Token {
            shared: self,
            guard: Some(guard),
        }
    }

    /// How many times a sleeping task woke up so far.
    #[cfg(test)]
    pub(crate) fn wakeups(&self) -> usize {
        self.wakeups.load(Ordering::Relaxed)
    }

    /// Cancels every task (Go `CancelAll`): the root and everything below
    /// it, and wakes the blocked ones. Takes the token.
    pub(crate) fn cancel_all(&self) {
        let _t = self.lock();
        self.root.cancel();
    }
}

/// A poisoned mutex means a thread panicked while it held the token. The
/// VM's panic fence already turned that panic into E0801 (or the panic is
/// a test's), and the world's own fields are plain containers that no
/// method leaves half-changed across a call that can panic, so refusing
/// the lock would only wedge the runtime: every later task, the exit
/// handlers and the runtime's drop would fail on a lock nobody can clear.
/// Take the guard as it is. (The gate's state is changed only in short
/// sections that do not panic; the same reasoning applies.)
fn recover<T>(e: PoisonError<T>) -> T {
    e.into_inner()
}

/// The fair queue in front of the run token (stage 3a: 차례 넘기기).
///
/// `std::sync::Mutex` is not fair: a thread that unlocks and locks again
/// at once usually gets the lock back, and the others starve. So a thread
/// may lock the world mutex only while it owns the gate, and the gate
/// passes ownership in first-come order: a thread that finds the gate
/// owned queues a `Turn` and parks until the owner, leaving, hands the
/// turn to the first in the queue. Ownership passes directly, so a thread
/// that leaves and enters again goes to the back of the queue.
///
/// The gate's own mutex is held only for a few instructions (to queue or
/// to hand over), never while fung code runs.
#[derive(Default)]
struct Gate {
    state: Mutex<GateState>,
    /// How many threads are queued. The token's holder reads it without
    /// any lock to decide whether yielding would help anyone.
    queued: AtomicUsize,
}

#[derive(Default)]
struct GateState {
    /// Whether some thread owns the gate (holds or is about to take the
    /// token).
    owned: bool,
    queue: VecDeque<Arc<Turn>>,
}

/// A queued thread's ticket: set ready when the gate is handed to it.
struct Turn {
    thread: Thread,
    ready: AtomicBool,
}

impl Turn {
    fn new() -> Arc<Turn> {
        Arc::new(Turn {
            thread: std::thread::current(),
            ready: AtomicBool::new(false),
        })
    }

    /// Parks until the gate is handed to this thread (park can wake
    /// spuriously; the flag is the truth).
    fn wait(&self) {
        while !self.ready.load(Ordering::Acquire) {
            std::thread::park();
        }
    }

    fn give(&self) {
        self.ready.store(true, Ordering::Release);
        self.thread.unpark();
    }
}

impl Gate {
    fn state(&self) -> MutexGuard<'_, GateState> {
        self.state.lock().unwrap_or_else(recover)
    }

    /// Becomes the owner, waiting in line if another thread owns it.
    fn enter(&self) {
        let mut g = self.state();
        if !g.owned {
            g.owned = true;
            return;
        }
        let turn = Turn::new();
        g.queue.push_back(turn.clone());
        self.queued.fetch_add(1, Ordering::Relaxed);
        drop(g);
        turn.wait();
    }

    /// Stops owning: hands the gate to the first thread in line, if any.
    fn leave(&self) {
        let mut g = self.state();
        match g.queue.pop_front() {
            Some(next) => {
                self.queued.fetch_sub(1, Ordering::Relaxed);
                next.give();
            }
            None => g.owned = false,
        }
    }

    /// Hands the gate to the first thread in line and queues this one
    /// behind the rest, in one step. Returns the turn to wait for, or
    /// `None` (still the owner) when nobody is in line.
    fn pass(&self) -> Option<Arc<Turn>> {
        let mut g = self.state();
        let next = g.queue.pop_front()?;
        let turn = Turn::new();
        g.queue.push_back(turn.clone());
        next.give();
        Some(turn)
    }
}

/// The run token: the guard of `Shared::world` while this thread holds it.
/// The debug mark (`token.rs`) is set right after the mutex is locked and
/// cleared right before it is unlocked, in every path: `Shared::lock`,
/// `release`, `reacquire`, `wait`, `yield_turn` and the drop.
///
/// The guard is an `Option` so the token can be given up and taken back
/// without dropping the `Token` (a task keeps one for its whole life).
///
/// Holding the token means owning the gate and holding the mutex. Giving
/// it up drops the mutex guard first and then leaves the gate, so the
/// next owner finds the mutex free (or about to be).
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
            self.shared.gate.leave();
        }
    }

    /// Takes the token back if this `Token` gave it up (a no-op while it
    /// is held).
    pub(super) fn reacquire(&mut self) {
        if self.guard.is_none() {
            token::assert_not_held();
            self.shared.gate.enter();
            let g = self.shared.world.lock().unwrap_or_else(recover);
            token::mark_taken();
            self.guard = Some(g);
        }
    }

    /// Gives the token up until `w` is woken or `deadline` passes, then
    /// takes it back in line. The sleep parks the thread; it never needs
    /// the world mutex (which a computing task holds), so a woken task
    /// queues at the gate, where `yield_turn` sees it. A wake that came
    /// before the sleep started is not lost: it set `w`'s flag, which is
    /// checked before every park.
    pub(super) fn sleep(&mut self, w: &Waiter, deadline: Option<Instant>) {
        self.release();
        loop {
            if w.take_woken() {
                break;
            }
            match deadline {
                Some(at) => {
                    let now = Instant::now();
                    if now >= at {
                        break;
                    }
                    std::thread::park_timeout(at - now);
                }
                None => std::thread::park(),
            }
        }
        #[cfg(test)]
        self.shared.wakeups.fetch_add(1, Ordering::Relaxed);
        self.reacquire();
    }

    /// Whether another thread is waiting for the token. Reads one atomic,
    /// without any lock.
    #[inline]
    pub(super) fn contended(&self) -> bool {
        self.shared.gate.queued.load(Ordering::Relaxed) > 0
    }

    /// Lets every thread already waiting for the token run first, then
    /// takes it back (turn handoff). Does nothing when nobody waits.
    pub(super) fn yield_turn(&mut self) {
        if !self.contended() {
            return;
        }
        let Some(turn) = self.shared.gate.pass() else {
            return;
        };
        let g = self.guard.take().expect("yielding without the run token");
        token::mark_released();
        drop(g);
        turn.wait();
        let g = self.shared.world.lock().unwrap_or_else(recover);
        token::mark_taken();
        self.guard = Some(g);
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
    use std::time::Duration;

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
    fn a_sleep_gives_the_token_to_another_thread() {
        let s = shared();
        let mut t = s.lock();
        t.exit = Some(1);
        let me = Waiter::new(Arc::default());
        let other = {
            let (s, me) = (s.clone(), me.clone());
            std::thread::spawn(move || {
                let mut t = s.lock();
                token::assert_holds_token();
                t.exit = Some(2);
                me.wake();
            })
        };
        while t.exit != Some(2) {
            t.sleep(&me, Some(Instant::now() + Duration::from_millis(10)));
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

    /// Threads waiting for the token get it in the order they asked, and
    /// a yield lets every thread already waiting run first.
    #[test]
    fn the_token_goes_round_in_line() {
        let s = shared();
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut t = s.lock();
        let mut threads = Vec::new();
        for i in 0..4 {
            let (s2, order) = (s.clone(), order.clone());
            threads.push(std::thread::spawn(move || {
                let _t = s2.lock();
                order.lock().unwrap().push(i);
            }));
            // Wait until thread i is in line before starting the next.
            while s.gate.queued.load(Ordering::Relaxed) <= i {
                std::thread::yield_now();
            }
        }
        assert!(t.contended());
        t.yield_turn();
        assert_eq!(*order.lock().unwrap(), vec![0, 1, 2, 3]);
        assert!(!t.contended());
        t.yield_turn(); // nobody waits: keeps the token
        token::assert_holds_token();
        drop(t);
        for th in threads {
            th.join().unwrap();
        }
    }
}
