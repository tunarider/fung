//! Closing what dropped file handles leave (L11 B; 06 `fs`).
//!
//! When the last copy of a handle goes without `fs.close`, its buffer
//! must still be flushed and its file closed. Doing that where the handle
//! drops (most often the run thread) would stop every task while a file
//! system that hangs on close (NFS) waits. So the handle gives its
//! buffer and file to the runtime's closer, whose thread flushes and
//! closes them in the order they came. The thread starts when there is
//! work and ends when there is none, and it is not a pool thread: a pool
//! full of reads that wait cannot hold up a close.
//!
//! When the data reaches the file is no longer said; the runtime's end
//! waits for every close (`Runtime::release`), so nothing written is lost.
//! A flush or close that fails has no value to give its error to: it is
//! reported on standard error (E0810) and the exit code becomes 1, as an
//! unhandled task failure is (`failure.rs`).

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use fung_front::diag::{self, Arg, error};
use fung_front::source::SourceMap;

use super::failure::Failures;
use crate::native::file::{self, Io};

/// The runtime's closer; copies share it.
#[derive(Clone)]
pub(crate) struct Closer(Arc<Inner>);

struct Inner {
    state: Mutex<State>,
    /// Signalled when the last job is done.
    idle: Condvar,
    failures: Arc<Failures>,
}

#[derive(Default)]
struct State {
    /// The dropped handles' paths and what they left, oldest first.
    queue: VecDeque<(String, Io)>,
    /// Jobs queued or running.
    pending: usize,
    /// Whether the thread runs.
    running: bool,
}

impl Closer {
    pub(crate) fn new(failures: Arc<Failures>) -> Closer {
        Closer(Arc::new(Inner {
            state: Mutex::new(State::default()),
            idle: Condvar::new(),
            failures,
        }))
    }

    /// Flushes and closes what the dropped handle of `path` left, on the
    /// closer's thread; here if no thread can be made.
    pub(crate) fn submit(&self, path: String, io: Io) {
        let mut s = self.0.state();
        s.pending += 1;
        s.queue.push_back((path, io));
        if s.running {
            return;
        }
        s.running = true;
        drop(s);
        let inner = self.0.clone();
        let spawned = std::thread::Builder::new()
            .name("fung-closer".into())
            .spawn(move || inner.work());
        if spawned.is_err() {
            self.0.state().running = false;
            self.0.work();
        }
    }

    /// Waits up to `limit` for the closes pending now; whether any was
    /// pending. An open that ran out of descriptors calls this before it
    /// tries again: the closer may lag behind a program that drops
    /// handles fast. The limit keeps a close that hangs (NFS) from
    /// holding the open forever; then the open fails as it would have.
    pub(crate) fn settle(&self, limit: std::time::Duration) -> bool {
        let mut s = self.0.state();
        if s.pending == 0 {
            return false;
        }
        let until = std::time::Instant::now() + limit;
        while s.pending > 0 {
            let left = until.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                break;
            }
            s = self
                .0
                .idle
                .wait_timeout(s, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        true
    }

    /// Waits until every dropped handle is closed (the runtime's end).
    pub(crate) fn wait(&self) {
        let mut s = self.0.state();
        while s.pending > 0 {
            s = self.0.idle.wait(s).unwrap_or_else(PoisonError::into_inner);
        }
    }
}

impl Inner {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Closes the queued handles until none is left.
    fn work(&self) {
        loop {
            let Some((path, io)) = self.state().queue.pop_front() else {
                let mut s = self.state();
                // A job queued between the look and the lock is taken on.
                if s.queue.is_empty() {
                    s.running = false;
                    return;
                }
                continue;
            };
            if let Err(e) = file::finish(io, &path) {
                self.report(&path, &e);
            }
            let mut s = self.state();
            s.pending -= 1;
            if s.pending == 0 {
                self.idle.notify_all();
            }
        }
    }

    /// E0810 on standard error; the exit code becomes 1.
    fn report(&self, path: &str, e: &std::io::Error) {
        let why = format!("cannot close `{path}`: {}", crate::native::oserr::reason(e));
        let d = error("E0810", super::no_pos(), vec![Arg::Str(why)]);
        self.failures
            .report_now(&diag::format(&d, &SourceMap::default()));
    }
}
