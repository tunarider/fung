//! Deadlines (concurrency v2, L2; 05 "타임아웃·취소"): `timeout(d, f)`.
//!
//! **A deadline lives on a cancellation node.** `timeout` makes a node
//! whose deadline is `d` from now (or the deadline it inherits, if that
//! is earlier), and every node made below it copies that deadline. No
//! timer thread watches it. Whoever is under the node notices instead:
//! - a task that waits (`block_until`, a socket wait in `Watch`, a wait
//!   for standard input) sleeps only until the deadline;
//! - a task that computes looks at the clock when it hands its turn over
//!   (`Task::turn`, every `YIELD_EVERY` calls and jumps).
//!
//! The first to see the deadline pass expires the node that owns it: it
//! marks it expired and cancels it, which cancels everything below. The
//! expiry happens with the run token held: a wait outside the token that
//! ends on the deadline only returns, and `outside` expires the owner
//! when the token is back. A
//! wait with a deadline counts as one a timer will end, so the deadlock
//! check does not judge a task stuck under a deadline (`deadlock.rs`).
//!
//! Waits outside the run token through plain `outside` (files, child
//! processes, compiling a regex) are not cut short: the deadline is late
//! by as long as they take, and enforced the moment they return (known
//! limit until the thread pool, R7).

use std::sync::Weak;
use std::time::{Duration, Instant};

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
/// and gives its value, or `Err({kind: 'timeout})` once the deadline
/// unwound it. A zero or negative `d` has passed already: `f` unwinds at
/// its first call (as Trio's `move_on_after(0)`).
pub(crate) fn run_timeout(t: &mut Task<'_>, d: i64, f: Value) -> Result<Value, Stop> {
    let at = if d <= 0 {
        Some(Instant::now())
    } else {
        Instant::now().checked_add(Duration::from_nanos(d as u64))
    };
    let node = t.cancel.with_deadline(at);
    let outer = std::mem::replace(&mut t.cancel, node.clone());
    let r = t.call(&f, Vec::new());
    t.cancel = outer;
    let timed_out = node.expired();
    // Out of the opener's node's list; `expired` stays as it was.
    node.cancel();
    match r {
        Err(Stop::Error(ref e)) if is_cancel(e) && timed_out && !t.cancel.is_cancelled() => {
            Ok(err_kind("timeout"))
        }
        r => r,
    }
}
