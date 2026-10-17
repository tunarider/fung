//! Deadlock detection (05 "교착"; Go `armWatchdog`, `watchdog` and
//! `deadlock` in `internal/vm/runtime_conc.go`).
//!
//! The program is deadlocked when every live task is blocked in `block`
//! and no timer will wake any of them: nothing can ever run again. The
//! counts (`World::live`, `Census`) change only with the run token held,
//! and each is exact at every moment the token changes hands:
//! - a task is live from the moment it is spawned (counted by its
//!   parent, before its thread exists) until its `Task` drops; a task
//!   waiting for the token, or in `outside` (standard input, a child
//!   process), is live and not blocked;
//! - a task is blocked from the moment it finds its condition false and
//!   decides to sleep until whoever wakes it uncounts it, with the
//!   token held, before releasing it (`Waiter::wake`). So a task that was
//!   woken but has not run yet is not blocked, and no task is counted
//!   blocked while something is on its way to it.
//!
//! So Go's 50 ms grace (which absorbed tasks counted blocked although
//! their wait was already satisfiable) is not needed: the check runs
//! whenever the condition can become true — a live task counting itself
//! blocked, and a live task ending — and reports at once (spec 2절).
//! What nothing in fung can wake is out of the count: `Runtime::wait`'s
//! task is not live (the command waiting for the program is not a task).
//!
//! After the report (Go `deadlock`): E0702 on standard error, exit code 1
//! (the first exit request), and every task of the program cancelled
//! (the root). Each blocked task wakes and unwinds with E0702 (its handle
//! turns that into `Err({kind: 'cancelled})`); the command does not
//! report the entry's E0702 again (an exit was requested) and skips it
//! among the handlers' errors, and the exit handlers still run.
//!
//! While an exit handler runs, the check is the handler's own: a deadlock
//! is reported even after an earlier exit or deadlock, and it cancels
//! only that handler's node, so the handler is cut short and the next one
//! runs; the exit code stays as it was (1 if nothing set one). Go's
//! detector is off once an exit was requested, and Go's runtime then
//! kills the process ("all goroutines are asleep", exit 2); hanging would
//! be worse than both (open-questions).
//!
//! The entry's end is judged too, so an entry error cancels the program
//! before the entry task drops (`Runtime::run_with`): the tasks it leaves
//! blocked are then being cancelled, not deadlocked.
//!
//! **Background tasks** (stage 3b, `spawn_background` and every task
//! spawned below one; agents and what their `f` spawns) are not
//! the program's tasks: they never keep it alive, so they are not in
//! `World::live` nor in `Census::blocked`. They are counted apart
//! (`World::background`, `Census::bg_blocked` / `bg_waking`), by the same
//! rules and with the same exactness, and the check adds one condition:
//! **no deadlock is judged while any background task can still run** —
//! one that is runnable, running, waiting for the token, in `outside`, or
//! blocked with a timer. Such a task may still wake a blocked task of the
//! program (an agent about to answer the request a task waits on), so
//! judging then could report a deadlock that is not one. Once every
//! background task is blocked with no timer, none of them can run again
//! unless some task wakes it, and no task can: every task of the program
//! is blocked too. So the report is exact in both directions with respect
//! to the counts. Since the condition can now also become true when a
//! background task blocks or ends, both are judged as well. What it gives
//! up: a background task that runs forever (a loop that never blocks)
//! hides a deadlock of the program's tasks, as a normal task that spins
//! does today.

use fung_front::diag::{error, headline};

use super::block::cancelled;
use super::{Stop, Task, no_pos, token};

impl Task<'_> {
    /// Reports a deadlock if every live task is blocked with no timer to
    /// wake one, and so is every background task; true if it did. Called
    /// with the token held, when this task (live or background) has
    /// counted itself blocked and when a live or background task ends.
    pub(super) fn judge(&mut self) -> bool {
        token::assert_holds_token();
        let census = &self.shared.census;
        let w = &mut *self.world;
        debug_assert!(census.blocked() <= w.live, "more blocked than live");
        debug_assert!(
            census.bg_blocked() <= w.background,
            "more background tasks blocked than running"
        );
        if w.live == 0 || census.blocked() != w.live || census.waking() != 0 {
            return false;
        }
        // A background task that can still run may wake one of them.
        if census.bg_blocked() != w.background || census.bg_waking() != 0 {
            return false;
        }
        // During an exit handler, only the handler and what it spawned
        // run, all under the handler's node: a deadlock there is judged
        // on its own, whatever happened before the handlers.
        let handler = w.handler.clone();
        if handler.is_none() && (w.deadlocked || w.exit.is_some()) {
            return false;
        }
        w.deadlocked = true;
        let line = headline(&error("E0702", no_pos(), Vec::new())) + "\n";
        let _ = w.stderr().write_all(line.as_bytes());
        let _ = w.stderr().flush();
        w.request_deadlock_exit();
        match handler {
            Some(node) => node.cancel(),
            None => self.shared.root().cancel(),
        }
        true
    }

    /// The error a cancelled blocked task unwinds with: E0702 once a
    /// deadlock was reported (Go `block`), E0701 otherwise.
    pub(super) fn unwound(&self) -> Stop {
        if self.world.deadlocked {
            Stop::Error(Box::new(error("E0702", no_pos(), Vec::new())))
        } else {
            cancelled(no_pos())
        }
    }
}
