//! The thread pool offload jobs run on (concurrency v2 R3, spec 15).
//!
//! Threads start when a job finds none idle, up to `POOL_MAX`; one that
//! finds no job for `POOL_IDLE` ends. When all are busy a job waits in a
//! FIFO queue. A job is plain data (`Send + 'static`), never a fung value.
//!
//! Every job has a `Ticket`. A job still in the queue can always be
//! dropped (`Ticket::abandon`): the thread that takes it skips it. A job
//! already running cannot be stopped (an operating system thread cannot
//! be cut short), so abandoning it only tells the job to drop its result
//! (`Ticket::abandon_running`); `offload.rs` decides which jobs may be
//! left like that.
//!
//! The queue, the thread count and the idle count change under one lock,
//! so no job is left in the queue while a thread could take it: a job
//! starts a thread whenever the queue holds more jobs than there are idle
//! threads to take them. A thread that is finishing a job is not idle
//! yet, so a task that submits its next job right after the last one
//! woke it may start a second thread (that one ends when it idles).

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// The most threads the pool runs at once.
pub(crate) const POOL_MAX: usize = 64;

/// How long a thread with nothing to do lives.
pub(crate) const POOL_IDLE: Duration = Duration::from_secs(10);

/// A job.
pub(crate) type Work = Box<dyn FnOnce() + Send>;

const QUEUED: u8 = 0;
const RUNNING: u8 = 1;
const ABANDONED: u8 = 2;

/// A job's state: queued, running, or abandoned.
pub(crate) struct Ticket(AtomicU8);

impl Ticket {
    pub(crate) fn new() -> Arc<Ticket> {
        Arc::new(Ticket(AtomicU8::new(QUEUED)))
    }

    /// Drops the job if it has not started; whether it had not.
    pub(crate) fn abandon(&self) -> bool {
        self.0
            .compare_exchange(QUEUED, ABANDONED, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Drops the job, or its result if it is running already.
    pub(crate) fn abandon_running(&self) {
        self.0.store(ABANDONED, Ordering::SeqCst);
    }

    pub(crate) fn is_abandoned(&self) -> bool {
        self.0.load(Ordering::SeqCst) == ABANDONED
    }

    /// The pool takes the job: false if it was abandoned in the queue.
    fn start(&self) -> bool {
        self.0
            .compare_exchange(QUEUED, RUNNING, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

struct State {
    queue: VecDeque<(Arc<Ticket>, Work)>,
    threads: usize,
    idle: usize,
    max: usize,
    idle_for: Duration,
    /// The queue length at which abandoned jobs are dropped from it.
    compact_at: usize,
    /// The runtime went (`Pool::shutdown`): idle threads end at once.
    shutdown: bool,
}

/// The least `compact_at`.
const COMPACT_MIN: usize = 64;

struct Inner {
    state: Mutex<State>,
    work: Condvar,
    /// Thread starts fail (tests).
    #[cfg(test)]
    fail_spawns: std::sync::atomic::AtomicBool,
}

impl Inner {
    fn state(&self) -> MutexGuard<'_, State> {
        // Plain counts and a queue that no panic leaves half-changed (jobs
        // run outside the lock).
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A handle on the pool; clones share it.
#[derive(Clone)]
pub(crate) struct Pool {
    inner: Arc<Inner>,
}

impl Pool {
    pub(crate) fn new(max: usize, idle_for: Duration) -> Pool {
        Pool {
            inner: Arc::new(Inner {
                state: Mutex::new(State {
                    queue: VecDeque::new(),
                    threads: 0,
                    idle: 0,
                    max,
                    idle_for,
                    compact_at: COMPACT_MIN,
                    shutdown: false,
                }),
                work: Condvar::new(),
                #[cfg(test)]
                fail_spawns: std::sync::atomic::AtomicBool::new(false),
            }),
        }
    }

    /// Queues `work`, starting a thread for it if none is idle and the
    /// pool is not full. Fails only when no thread runs and none starts.
    #[cfg(test)]
    pub(crate) fn submit(&self, work: Work) -> std::io::Result<Arc<Ticket>> {
        let ticket = Ticket::new();
        self.submit_with(ticket.clone(), work)?;
        Ok(ticket)
    }

    /// [`Pool::submit`] with a ticket the job also holds.
    pub(crate) fn submit_with(&self, ticket: Arc<Ticket>, work: Work) -> std::io::Result<()> {
        let mut s = self.inner.state();
        s.queue.push_back((ticket.clone(), work));
        // Behind busy threads (a stuck read, say) abandoned jobs would wait
        // in the queue until a thread takes them: drop them once the queue
        // has doubled since the last time.
        if s.queue.len() > s.compact_at {
            s.queue.retain(|(t, _)| !t.is_abandoned());
            s.compact_at = COMPACT_MIN.max(2 * s.queue.len());
        }
        if s.queue.len() > s.idle && s.threads < s.max {
            match self.start_thread() {
                Ok(()) => s.threads += 1,
                Err(e) if s.threads == 0 => {
                    s.queue.pop_back();
                    return Err(e);
                }
                // A running thread takes it later.
                Err(_) => {}
            }
        }
        self.inner.work.notify_one();
        Ok(())
    }

    fn start_thread(&self) -> std::io::Result<()> {
        #[cfg(test)]
        if self.inner.fail_spawns.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("thread starts fail (test)"));
        }
        let inner = self.inner.clone();
        std::thread::Builder::new()
            .name("fung-pool".into())
            .spawn(move || run(&inner))
            .map(drop)
    }

    /// The runtime is gone: idle threads end now, not after their idle
    /// time. Jobs running or queued still run to their end (a job that is
    /// waited for is not dropped), and a job submitted later still gets a
    /// thread, which ends once the queue is empty.
    pub(crate) fn shutdown(&self) {
        self.inner.state().shutdown = true;
        self.inner.work.notify_all();
    }

    /// Changes the limits (tests).
    #[cfg(test)]
    pub(crate) fn set_limits(&self, max: usize, idle_for: Duration) {
        let mut s = self.inner.state();
        s.max = max;
        s.idle_for = idle_for;
    }

    #[cfg(test)]
    pub(crate) fn fail_spawns(&self, on: bool) {
        self.inner.fail_spawns.store(on, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn threads(&self) -> usize {
        self.inner.state().threads
    }

    #[cfg(test)]
    pub(crate) fn idle(&self) -> usize {
        self.inner.state().idle
    }

    #[cfg(test)]
    pub(crate) fn queued(&self) -> usize {
        self.inner.state().queue.len()
    }
}

/// A pool thread: takes jobs until none comes for the idle time.
fn run(inner: &Inner) {
    let mut s = inner.state();
    loop {
        if let Some((ticket, work)) = s.queue.pop_front() {
            drop(s);
            if ticket.start() {
                // A panic is the job's to report (`offload.rs` catches it
                // into its result); here it only must not end the thread.
                let _ = catch_unwind(AssertUnwindSafe(work));
            }
            s = inner.state();
            continue;
        }
        if s.shutdown {
            s.threads -= 1;
            return;
        }
        s.idle += 1;
        let idle_for = s.idle_for;
        let (next, timeout) = inner
            .work
            .wait_timeout(s, idle_for)
            .unwrap_or_else(PoisonError::into_inner);
        s = next;
        s.idle -= 1;
        if (s.shutdown || timeout.timed_out()) && s.queue.is_empty() {
            s.threads -= 1;
            return;
        }
    }
}
