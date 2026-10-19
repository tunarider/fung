//! Deadlock detection (05 "교착").
//!
//! The program is deadlocked when every task is parked and none of them
//! is one a timer or the world outside ends: nothing can ever run again.
//! The scheduler knows that exactly (`sched.rs`): every task is running,
//! in the ready queue or parked with it, and the counts change only on
//! the run thread. So it calls `judge` when the ready queue is empty and
//! every parked task is stuck, and the report is at once, with no grace
//! period (spec 2절). A parked task woken but not run yet is in the ready
//! queue, so it is never counted stuck.
//!
//! After the report: E0702 on standard error, exit code 1 (the first exit
//! request), and every task of the program cancelled (the root). Each
//! parked task wakes and unwinds with E0702 (its handle turns that into
//! `Err({kind: 'cancelled})`); the command does not report the entry's
//! E0702 again (an exit was requested).
//!
//! While a `with`'s release runs (`cleanup.rs`), the check is judged too
//! even after an exit or a deadlock: the release runs under a shield node
//! nothing else cancels, so a deadlock there cancels the shield nodes of
//! the releases running then (and the root), and they are cut short.

use fung_front::diag::{error, headline};

use super::block::cancelled;
use super::{Stop, Task, no_pos};

impl Task<'_> {
    /// Reports a deadlock: every task is parked and stuck (the scheduler
    /// checked); true if it reported. After an exit or an earlier report
    /// the program is unwinding, so it is judged only while a release
    /// runs.
    pub(super) fn judge(&mut self) -> bool {
        let w = &mut *self.world;
        let shields = w.live_shields();
        if shields.is_empty() && (w.deadlocked || w.exit.is_some()) {
            return false;
        }
        w.deadlocked = true;
        let line = headline(&error("E0702", no_pos(), Vec::new())) + "\n";
        let _ = w.stderr().write_all(line.as_bytes());
        let _ = w.stderr().flush();
        w.request_deadlock_exit();
        for s in &shields {
            s.cancel();
        }
        self.shared.root().cancel();
        true
    }

    /// The error a cancelled blocked task unwinds with: E0702 once a
    /// deadlock was reported, E0701 otherwise.
    pub(super) fn unwound(&self) -> Stop {
        if self.world.deadlocked {
            Stop::Error(Box::new(error("E0702", no_pos(), Vec::new())))
        } else {
            cancelled(no_pos())
        }
    }
}
