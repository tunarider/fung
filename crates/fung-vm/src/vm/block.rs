//! The one place a task waits (stage 3a: 막히는 지점).
//!
//! Every operation that waits for another task goes through `block`:
//! `await`, `sleep`, `send`, `recv` and `swap`. It registers the
//! task's `Waiter` on what it waits for, gives the run token up while it
//! sleeps until woken (`Token::sleep`), so other tasks run, and takes it
//! back to check its condition. An event wakes only the waiters
//! registered on what changed. A timeout ends the wait with
//! `None`; cancellation ends it with E0701. Waits for the operating system
//! (standard input, a child process) go through `outside` instead: the
//! token is given up for the call, and the wait is not a blocked task for
//! the deadlock check.
//!
//! Turn handoff (차례 넘기기) lives here too: a task that computes without
//! blocking gives the token to the tasks waiting for it every
//! `YIELD_EVERY` calls (`Task::tick`).
//!
//! When tasks become green threads, this file is what changes: `block`
//! parks the task and returns to a scheduler.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::Thread;
use std::time::{Duration, Instant};

use fung_front::diag::error;
use fung_front::source::Pos;

use super::{Stop, Task, World, no_pos, token};

/// How many calls and backward jumps a task makes between checks whether
/// another task waits for the token. With no other task waiting, the
/// check is one atomic load, so the value only matters under contention:
/// it is the length of a computing task's turn. Measured (release, two
/// spinning tasks against one doing the same work alone; and a task
/// doing 200 `sleep(1ms)` next to a spinner): a handoff costs about 3.5
/// µs, so 2,000 cost 8% of the throughput of two spinners and 10,000 1%,
/// while a woken task waits about half a millisecond for its turn at
/// 10,000 (0.7 ms over its sleep at 20,000, 8.6 ms at 200,000).
pub(crate) const YIELD_EVERY: u32 = 10_000;

/// E0701 `task cancelled`, the internal error a cancelled task unwinds
/// with (never printed: its handle turns it into `Err({kind: 'cancelled})`).
#[cold]
pub(crate) fn cancelled(pos: Pos) -> Stop {
    Stop::Error(Box::new(error("E0701", pos, Vec::new())))
}

/// A blocked task's wake-up call: the thread to unpark and a flag that
/// says it was woken (park can return spuriously; the flag is the truth).
/// Each `Task` has one for its life; `block` registers it on what the
/// task waits for, and whoever changes that thing wakes the waiters
/// registered on it, and only those.
pub(crate) struct Waiter {
    thread: Thread,
    woken: AtomicBool,
}

impl Waiter {
    /// A waiter for the current thread.
    pub(crate) fn new() -> Arc<Waiter> {
        Arc::new(Waiter {
            thread: std::thread::current(),
            woken: AtomicBool::new(false),
        })
    }

    pub(crate) fn wake(&self) {
        self.woken.store(true, Ordering::Release);
        self.thread.unpark();
    }

    /// Takes the woken flag (true once per wake).
    pub(super) fn take_woken(&self) -> bool {
        self.woken.swap(false, Ordering::Acquire)
    }
}

/// The waiters registered on one thing a task can wait for (a task's
/// result, a cancellation node, the runtime's last running task; a
/// channel keeps one waiter per queued slot instead, `channel.rs`).
/// Changed only with the token held
/// (or under the node's own lock for cancellation nodes).
///
/// `wake_all` empties it, so a waiter is woken at most once per
/// registration; a waiter that stopped waiting (a timeout) may still be
/// in a list, and its wake is then a spurious one, which `block` absorbs.
/// A waiter is in a list at most once, so a list never holds more
/// entries than there are tasks.
#[derive(Default)]
pub(crate) struct WaitList(Vec<Arc<Waiter>>);

impl WaitList {
    pub(crate) fn add(&mut self, w: &Arc<Waiter>) {
        if !self.0.iter().any(|x| Arc::ptr_eq(x, w)) {
            self.0.push(w.clone());
        }
    }

    pub(crate) fn wake_all(&mut self) {
        for w in self.0.drain(..) {
            w.wake();
        }
    }
}

/// Waits until `ready` gives a value (`Some`), the timeout passes (`None`)
/// or the task is cancelled (E0701).
///
/// `ready(world, me)` runs with the token held, at the start and after
/// every wakeup. When it returns `None` it must register `me` on what it
/// waits for (`WaitList::add`); whoever changes that thing, holding the
/// token, calls `wake_all` on the list. `block` itself registers `me` on
/// the task's cancellation node.
///
/// No wakeup is lost: the woken flag is cleared and `me` registered
/// before the check, all with the token held; a change needs the token,
/// so it comes after the check and finds `me` registered, and its wake
/// sets the flag the sleep below looks at before it parks.
///
/// A condition already true returns at once, even in a cancelled task
/// (Go's fast path); after a wait, cancellation wins over a condition that
/// came true at the same time (Go checks it first).
pub(crate) fn block<T>(
    t: &mut Task<'_>,
    ready: impl FnMut(&mut World, &Arc<Waiter>) -> Option<T>,
    timeout: Option<Duration>,
) -> Result<Option<T>, Stop> {
    block_until(t, ready, deadline_after(timeout), timeout.is_some())
}

/// The deadline `timeout` from now; a deadline past what `Instant` can
/// hold is no deadline.
pub(crate) fn deadline_after(timeout: Option<Duration>) -> Option<Instant> {
    timeout.and_then(|d| Instant::now().checked_add(d))
}

/// `block` with the deadline already computed, for a waiter whose
/// deadline others must see too (a channel receiver's slot): the wait
/// times out exactly when `Instant::now() >= deadline`. `timed` says a
/// timeout was given (it counts for the deadlock check even when the
/// deadline is too far to hold).
pub(crate) fn block_until<T>(
    t: &mut Task<'_>,
    mut ready: impl FnMut(&mut World, &Arc<Waiter>) -> Option<T>,
    deadline: Option<Instant>,
    timed: bool,
) -> Result<Option<T>, Stop> {
    token::assert_holds_token();
    let me = t.waiter.clone();
    me.take_woken();
    if let Some(v) = ready(&mut t.world, &me) {
        return Ok(Some(v));
    }
    t.cancel.watch(&me);
    t.world.blocked += 1;
    if timed {
        t.world.waking += 1;
    }
    let r = loop {
        if t.cancel.is_cancelled() {
            break Err(cancelled(no_pos()));
        }
        if deadline.is_some_and(|at| Instant::now() >= at) {
            break Ok(None);
        }
        t.world.sleep(&me, deadline);
        token::assert_holds_token();
        if t.cancel.is_cancelled() {
            break Err(cancelled(no_pos()));
        }
        me.take_woken();
        t.cancel.watch(&me);
        if let Some(v) = ready(&mut t.world, &me) {
            break Ok(Some(v));
        }
    };
    t.world.blocked -= 1;
    if timed {
        t.world.waking -= 1;
    }
    token::assert_holds_token();
    r
}

/// Runs `f` without the run token: for a wait on the operating system
/// (reading standard input, waiting for a child process) during which
/// other tasks should run. `f` and its result are `Send`, so they can
/// hold no fung value (`Value` is not `Send`, nor is a reference to one):
/// nothing can touch values while the token is away.
///
/// A panic in `f` is caught, the token taken back, and the panic resumed:
/// unwinding then drops the values of the native frames above with the
/// token held, and the panic fence reports it as E0801.
#[cfg_attr(not(test), allow(dead_code))] // Task 7 moves stdin and processes onto it.
pub(crate) fn outside<T: Send>(t: &mut Task<'_>, f: impl FnOnce() -> T + Send) -> T {
    token::assert_holds_token();
    t.world.release();
    let r = catch_unwind(AssertUnwindSafe(f));
    t.world.reacquire();
    token::assert_holds_token();
    match r {
        Ok(v) => v,
        Err(p) => resume_unwind(p),
    }
}

impl Task<'_> {
    /// The check at every call and backward jump: unwinds a cancelled
    /// task (E0701 at `pos`) and, every `YIELD_EVERY` ticks, lets the
    /// tasks waiting for the token run.
    #[inline(always)]
    pub(super) fn tick(&mut self, pos: Pos) -> Result<(), Stop> {
        if self.cancel.is_cancelled() {
            return Err(cancelled(pos));
        }
        self.budget -= 1;
        if self.budget == 0 {
            self.turn();
        }
        Ok(())
    }

    #[cold]
    #[inline(never)]
    fn turn(&mut self) {
        self.budget = YIELD_EVERY;
        token::assert_holds_token();
        self.world.yield_turn();
        token::assert_holds_token();
    }
}
