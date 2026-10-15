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
//! (the first exit request), and every task cancelled — the program's
//! root, and the exit handlers' root once handlers run. Each blocked task
//! wakes and unwinds with E0702 (its handle turns that into
//! `Err({kind: 'cancelled})`); the command does not report the entry's
//! E0702 again (an exit was requested) and skips it among the handlers'
//! errors, and the exit handlers registered before still run.

use fung_front::diag::{error, headline};

use super::block::cancelled;
use super::{Stop, Task, no_pos, token};

impl Task<'_> {
    /// Reports a deadlock if every live task is blocked with no timer to
    /// wake one; true if it did. Called with the token held, when this
    /// task has counted itself blocked and when a live task ends.
    pub(super) fn judge(&mut self) -> bool {
        token::assert_holds_token();
        let census = &self.shared.census;
        let w = &mut *self.world;
        debug_assert!(census.blocked() <= w.live, "more blocked than live");
        if w.live == 0
            || census.blocked() != w.live
            || census.waking() != 0
            || w.deadlocked
            || w.exit.is_some()
        {
            return false;
        }
        w.deadlocked = true;
        let line = headline(&error("E0702", no_pos(), Vec::new())) + "\n";
        let _ = w.stderr().write_all(line.as_bytes());
        let _ = w.stderr().flush();
        w.request_exit(1);
        let handlers = w.handlers_started;
        self.shared.root.cancel();
        if handlers {
            self.shared.handlers.cancel();
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
