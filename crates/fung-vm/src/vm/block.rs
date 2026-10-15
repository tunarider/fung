//! The one place a task waits (stage 3a: 막히는 지점).
//!
//! Every operation that waits for another task goes through `block`:
//! `await`, `sleep`, and later `send`, `recv` and `swap`. It gives the run
//! token up while it sleeps until woken (`Token::wait`), so other tasks
//! run, and takes it back to check its condition. A timeout ends the wait with
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

/// Waits until `ready` gives a value (`Some`), the timeout passes (`None`)
/// or the task is cancelled (E0701). `ready` runs with the token held, at
/// the start and after every wakeup; whoever changes what it reads must
/// hold the token and call `Shared::notify_all` afterwards.
///
/// A condition already true returns at once, even in a cancelled task
/// (Go's fast path); after a wait, cancellation wins over a condition that
/// came true at the same time (Go checks it first).
pub(crate) fn block<T>(
    t: &mut Task<'_>,
    mut ready: impl FnMut(&mut World) -> Option<T>,
    timeout: Option<Duration>,
) -> Result<Option<T>, Stop> {
    token::assert_holds_token();
    if let Some(v) = ready(&mut t.world) {
        return Ok(Some(v));
    }
    // A deadline past what `Instant` can hold is no deadline.
    let deadline = timeout.and_then(|d| Instant::now().checked_add(d));
    let timed = timeout.is_some();
    t.world.blocked += 1;
    if timed {
        t.world.waking += 1;
    }
    let r = loop {
        if t.cancel.is_cancelled() {
            break Err(cancelled(no_pos()));
        }
        let left = match deadline {
            Some(at) => match at.checked_duration_since(Instant::now()) {
                Some(left) if !left.is_zero() => Some(left),
                _ => break Ok(None),
            },
            None => None,
        };
        t.world.wait(left);
        token::assert_holds_token();
        if t.cancel.is_cancelled() {
            break Err(cancelled(no_pos()));
        }
        if let Some(v) = ready(&mut t.world) {
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
#[cfg_attr(not(test), allow(dead_code))] // Task 7 moves stdin and processes onto it.
pub(crate) fn outside<T: Send>(t: &mut Task<'_>, f: impl FnOnce() -> T + Send) -> T {
    token::assert_holds_token();
    t.world.release();
    let r = f();
    t.world.reacquire();
    token::assert_holds_token();
    r
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
