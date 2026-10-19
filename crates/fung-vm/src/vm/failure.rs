//! Task failures nobody handled (05 "범위").
//!
//! Tasks start only inside a scope, so a failure goes to the code that
//! opened the scope (`scope`) or to its handler (`supervise`). The one
//! failure left for nobody is one a `supervise` scope queued for its
//! handler and could not hand over because the scope was cancelled first
//! (`vm/scope.rs`): it is reported on standard error as `task failed: `
//! plus the error with its excerpt and call path, and the program's exit
//! code becomes 1.
//!
//! Every report is written on the run thread. The report text is
//! made when the task fails, with the task holding the world's sources,
//! so writing it needs only standard error, which this reaches without
//! the world (`Failures` is shared through an `Arc`; its mutex is never
//! contended, since only the run thread takes it).

use std::io::Write;
use std::sync::{Mutex, PoisonError};

use super::world::Stream;

/// Whether a failure was reported, and where reports go.
pub(crate) struct Failures {
    failed: Mutex<bool>,
    /// The world's standard error (the same stream).
    stderr: Stream,
}

impl Failures {
    pub(crate) fn new(stderr: Stream) -> Failures {
        Failures {
            failed: Mutex::new(false),
            stderr,
        }
    }

    /// Reports a failure nobody will handle.
    pub(crate) fn report_now(&self, text: &str) {
        *self.failed.lock().unwrap_or_else(PoisonError::into_inner) = true;
        let mut w = self.stderr.clone();
        let _ = w.write_all(text.as_bytes());
        let _ = w.flush();
    }

    /// Whether a failure was reported (05: the exit code becomes 1).
    pub(crate) fn failed(&self) -> bool {
        *self.failed.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
