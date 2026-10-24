//! Deadlines (concurrency v2, L2; 05 "타임아웃·취소"): `timeout(d, f)`.
//!
//! **A deadline lives on a cancellation node.** `timeout` makes a node
//! whose deadline is `d` from now (or the deadline it inherits, if that
//! is earlier), and every node made below it copies that deadline. No
//! timer thread watches it. Whoever is under the node notices instead:
//! - a task that parks (a channel, a socket on the event loop, standard
//!   input) sleeps only until the deadline (`sched.rs`);
//! - a task that computes looks at the clock when it hands its turn over
//!   (`Task::turn`, every `YIELD_EVERY` calls and jumps).
//!
//! The first to see the deadline pass expires the node that owns it: it
//! marks it expired and cancels it, which cancels everything below. The
//! expiry happens on the run thread: a wait on another thread that
//! ends on the deadline only returns, and the task expires the owner when
//! it runs again (`offload.rs`, `Task::after_park`). A
//! wait with a deadline counts as one a timer will end, so the deadlock
//! check does not judge a task stuck under a deadline (`deadlock.rs`).
//!
//! Offload jobs that only read (a path's contents, compiling a regex) are
//! let go at the deadline. Those that change something (a write) are
//! waited for once they run: the deadline is late by as long as they take,
//! and enforced the moment they end (`offload.rs`).

use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use super::cont::{Cont, Flow, Next};
use super::task::{CancelNode, err_kind, is_cancel};
use super::{Stop, Task};
use crate::value::Value;

/// A node's deadline: when, and the `timeout`'s node it belongs to.
#[derive(Clone)]
pub(crate) struct Deadline {
    pub(crate) at: Instant,
    owner: Weak<CancelNode>,
}

impl Deadline {
    pub(crate) fn new(at: Instant, owner: Weak<CancelNode>) -> Deadline {
        Deadline { at, owner }
    }

    /// Expires the owner; false if it is gone (its `timeout` returned).
    pub(crate) fn expire(&self) -> bool {
        match self.owner.upgrade() {
            Some(o) => {
                o.expire();
                true
            }
            None => false,
        }
    }
}

/// `timeout(d, f)`: runs `f()` under a node with a deadline `d` from now
/// and gives its value, or `Err({kind: 'timeout, message})` once the deadline
/// unwound it. A zero or negative `d` has passed already: `f` unwinds at
/// its first call (as Trio's `move_on_after(0)`). The run loop calls `f`
/// (`cont.rs`).
pub(crate) fn run_timeout(t: &mut Task<'_>, d: i64, f: Value) -> Flow {
    let at = if d <= 0 {
        Some(Instant::now())
    } else {
        Instant::now().checked_add(Duration::from_nanos(d as u64))
    };
    let node = t.cancel.with_deadline(at);
    let outer = std::mem::replace(&mut t.cancel, node.clone());
    Flow::Call(
        Box::new(Timeout {
            node,
            outer: Some(outer),
            d,
        }),
        f,
        0,
    )
}

/// A `timeout` waiting for its body; the task's own node waits in
/// `outer`.
struct Timeout {
    node: Arc<CancelNode>,
    outer: Option<Arc<CancelNode>>,
    /// The duration as given, for the message.
    d: i64,
}

impl Timeout {
    fn end(&mut self, t: &mut Task<'_>, r: Result<Value, Stop>) -> Result<Next, Stop> {
        t.cancel = self.outer.take().expect("the outer node");
        let timed_out = self.node.expired();
        // Out of the opener's node's list; `expired` stays as it was.
        self.node.cancel();
        match r {
            Err(Stop::Error(ref e)) if is_cancel(e) && timed_out && !t.cancel.is_cancelled() => {
                let d = crate::value::format(&Value::Dur(self.d.max(0)));
                Ok(Next::Done(err_kind(
                    "timeout",
                    &format!("timed out after {d}"),
                )))
            }
            r => r.map(Next::Done),
        }
    }
}

impl Cont for Timeout {
    fn resume(&mut self, t: &mut Task<'_>, v: Value) -> Result<Next, Stop> {
        self.end(t, Ok(v))
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        self.end(t, Err(stop))
    }
}
