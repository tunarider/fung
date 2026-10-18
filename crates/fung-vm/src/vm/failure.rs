//! Task failures nobody retrieved (05 "아무도 확인하지 않은 태스크의 실패").
//!
//! A task that fails (any error but cancellation, E0701/E0702, or `exit`)
//! and was not spawned by a helper that awaits it (`spawn_awaited`) leaves
//! a failure that someone must see. `await` that gets the result
//! retrieves it; a timed-out `await` does not. A failure nobody can
//! retrieve any more is reported on standard error as `task failed: `
//! plus the error with its excerpt and call path, and the program's exit
//! code becomes 1. Nobody can retrieve it any more:
//! - when the task's last handle drops — at once if the task has failed
//!   by then, or at the moment it fails if it is still running;
//! - at the end of the program (`Runtime::report_unretrieved`), for the
//!   handles still held, in the order the tasks failed.
//!
//! Every step runs with the run token held: a handle drops only on the
//! token's holder, and a task finishes holding it. So a failure is
//! reported at most once, and never after `await` retrieved it, without
//! any further synchronisation. The report text is made when the task
//! fails, with the task holding the world's sources, so the handle's
//! drop needs only this record and standard error, which it reaches
//! without the world (`Failures` is shared through an `Arc`; its mutex is
//! never contended, since only the token's holder takes it).

use std::collections::BTreeMap;
use std::io::Write;
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::token;
use super::world::Stream;

/// The failures not yet retrieved or reported, and whether one was
/// reported.
pub(crate) struct Failures {
    inner: Mutex<Inner>,
    /// The world's standard error (the same stream).
    stderr: Stream,
}

#[derive(Default)]
struct Inner {
    /// The report text of each failure by its number, which is the order
    /// the tasks failed in.
    left: BTreeMap<u64, String>,
    next: u64,
    failed: bool,
}

impl Failures {
    pub(crate) fn new(stderr: Stream) -> Failures {
        Failures {
            inner: Mutex::new(Inner::default()),
            stderr,
        }
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Records a failure whose handle is still held; returns its number.
    pub(crate) fn track(&self, text: String) -> u64 {
        token::assert_holds_token();
        let mut g = self.inner();
        g.next += 1;
        let n = g.next;
        g.left.insert(n, text);
        n
    }

    /// `await` got the result: the failure is the caller's to handle.
    pub(crate) fn untrack(&self, n: u64) {
        token::assert_holds_token();
        self.inner().left.remove(&n);
    }

    /// Reports failure `n` if it is still unretrieved and unreported.
    pub(crate) fn report(&self, n: u64) {
        token::assert_holds_token();
        let text = self.inner().left.remove(&n);
        if let Some(text) = text {
            self.write(&text);
        }
    }

    /// Reports a failure whose handle was already gone when it happened.
    pub(crate) fn report_now(&self, text: &str) {
        token::assert_holds_token();
        self.write(text);
    }

    /// Reports every failure left, in the order the tasks failed.
    pub(crate) fn report_all(&self) {
        token::assert_holds_token();
        let left = std::mem::take(&mut self.inner().left);
        for text in left.values() {
            self.write(text);
        }
    }

    fn write(&self, text: &str) {
        self.inner().failed = true;
        let mut w = self.stderr.clone();
        let _ = w.write_all(text.as_bytes());
        let _ = w.flush();
    }

    /// Whether a failure was reported (05: the exit code becomes 1).
    pub(crate) fn failed(&self) -> bool {
        self.inner().failed
    }
}
