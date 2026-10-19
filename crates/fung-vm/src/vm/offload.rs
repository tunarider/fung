//! Waits on the operating system, handed to other threads: the machinery
//! behind `nat::offload` and its kin.
//!
//! The job (plain data, `Send + 'static`) runs on another thread; its
//! result lands in a slot the task shares, and that thread wakes the
//! task's waiter. The task parks meanwhile, as one the world outside
//! wakes (not a deadlock). Then, back on the run thread, `finish` turns the
//! result into the native's value.
//!
//! Three kinds of job (spec 15):
//! - **`Wait`** runs on the pool (`pool.rs`). A stop (a cancellation, a
//!   deadline) that comes while the job waits in the queue drops it and
//!   unwinds at once. One that comes while it runs is not let through:
//!   the task waits on under a fresh root node, as a scope waiting for
//!   its children does, until the job ends; then the task's node comes
//!   back, a deadline that passed meanwhile is enforced, and `finish`
//!   runs. A job that changes something (a file written) thus never
//!   lands after the call has given up.
//! - **`Abandon`** runs on the pool and is let go at a stop: the task
//!   unwinds at once and the job, which only reads, runs to its end on
//!   its thread, which drops the result. Its late wake finds a task that
//!   looks at what it waits for, sees nothing, and parks again.
//!
//! A panic in the job is caught on its thread and resumed on the run
//! thread, where the panic fence turns it into E0801. When no thread can
//! take the job, the call is E0801 (`cannot start a job`).

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use super::cont::{Cont, Flow, Next, Wake};
use super::pool::Ticket;
use super::task::CancelNode;
use super::{Stop, Task};
use crate::native::nat;
use crate::value::Value;
use fung_front::diag::Arg;

/// How a job is run, and what a stop does to its wait.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Wait,
    Abandon,
}

/// The job's result, once its thread has stored it.
type Slot<R> = Arc<Mutex<Option<std::thread::Result<R>>>>;

/// How a task waiting for a job parks: the job's thread wakes it.
const JOB_WAKE: Wake = Wake {
    until: None,
    timed: true,
};

/// Runs `job` as `kind` says and parks until it ends; then `finish`.
pub(crate) fn offload_kind<R: Send + 'static>(
    t: &mut Task<'_>,
    kind: Kind,
    job: impl FnOnce() -> R + Send + 'static,
    finish: impl FnOnce(&mut Task<'_>, R) -> Result<Flow, Stop> + 'static,
) -> Result<Flow, Stop> {
    #[cfg(test)]
    {
        t.offloads += 1;
    }
    let slot: Slot<R> = Arc::default();
    let me = t.waiter.clone();
    me.take_woken();
    let ticket = Ticket::new();
    let work = {
        let (slot, ticket) = (slot.clone(), ticket.clone());
        move || {
            let r = catch_unwind(AssertUnwindSafe(job));
            // An abandoned job's result is dropped here, on its thread.
            if !ticket.is_abandoned() {
                *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(r);
            }
            me.wake();
        }
    };
    let started = t.shared.pool.submit_with(ticket.clone(), Box::new(work));
    if let Err(e) = started {
        return Err(nat::fail(
            "E0801",
            vec![Arg::Str(format!("cannot start a job: {e}"))],
        ));
    }
    Ok(Flow::Park(
        Box::new(Job {
            kind,
            slot,
            ticket,
            own: None,
            finish: Some(Box::new(finish)),
            then: None,
        }),
        JOB_WAKE,
    ))
}

fn take<T>(cell: &Mutex<Option<T>>) -> Option<T> {
    cell.lock().unwrap_or_else(PoisonError::into_inner).take()
}

type Finish<R> = Box<dyn FnOnce(&mut Task<'_>, R) -> Result<Flow, Stop>>;

/// A task waiting for its job, then for what `finish` went on with.
struct Job<R> {
    kind: Kind,
    slot: Slot<R>,
    ticket: Arc<Ticket>,
    /// The task's own node while it waits under a fresh root.
    own: Option<Arc<CancelNode>>,
    finish: Option<Finish<R>>,
    /// What `finish` went on with, when that was a call or a park.
    then: Option<Box<dyn Cont>>,
}

impl<R> Job<R> {
    /// The job has ended: the task's node comes back (a deadline that
    /// passed meanwhile is enforced), and `finish` runs.
    fn ended(&mut self, t: &mut Task<'_>, r: std::thread::Result<R>) -> Result<Next, Stop> {
        if let Some(own) = self.own.take() {
            t.cancel = own;
            if let Some(at) = t.cancel.deadline_at() {
                let now = Instant::now();
                if now >= at {
                    t.cancel.expire_due(now);
                }
            }
        }
        let r = match r {
            Ok(r) => r,
            Err(p) => resume_unwind(p),
        };
        let finish = self.finish.take().expect("finish runs once");
        Ok(match finish(t, r)? {
            Flow::Done(v) => Next::Done(v),
            Flow::Call(k, f, n) => {
                self.then = Some(k);
                Next::Call(f, n)
            }
            Flow::Park(k, wake) => {
                self.then = Some(k);
                Next::Park(wake)
            }
        })
    }
}

/// A wait dropped without its job having started (the REPL abandoned
/// the task) takes the job out of the queue: it never runs. One that
/// runs finishes, and its result is dropped.
impl<R> Drop for Job<R> {
    fn drop(&mut self) {
        self.ticket.abandon();
    }
}

impl<R: 'static> Cont for Job<R> {
    fn resume(&mut self, t: &mut Task<'_>, v: Value) -> Result<Next, Stop> {
        if let Some(k) = &mut self.then {
            return k.resume(t, v);
        }
        match take(&self.slot) {
            Some(r) => self.ended(t, r),
            // A wake that was not the job's: wait on.
            None => Ok(Next::Park(JOB_WAKE)),
        }
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        if let Some(k) = &mut self.then {
            return k.unwind(t, stop);
        }
        match self.kind {
            Kind::Abandon => {
                self.ticket.abandon_running();
                Err(stop)
            }
            // Still in the queue: dropped, it never runs.
            Kind::Wait if self.ticket.abandon() => Err(stop),
            // Running: wait for it under a fresh root (the stop comes back
            // through the task's node once the job has ended).
            Kind::Wait => {
                debug_assert!(self.own.is_none(), "one stop reaches a job's wait");
                self.own = Some(std::mem::replace(&mut t.cancel, CancelNode::root()));
                // Take, then look (`park_until`): the job may have ended
                // and woken the task before this.
                t.waiter.take_woken();
                match take(&self.slot) {
                    Some(r) => self.ended(t, r),
                    None => Ok(Next::Park(JOB_WAKE)),
                }
            }
        }
    }
}
