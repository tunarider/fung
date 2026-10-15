//! Channels (05, stage 3a; Go `internal/vm/channel.go`): a queue between
//! tasks. `channel()` is unbuffered, so `send` waits until a receiver
//! takes the value (맞교환); `channel(n)` holds up to `n` values. `close`
//! is a flag, not an end of the queue: values buffered before it can
//! still be received, then `recv` gives `Err({kind: 'closed})`, and so
//! does any `send` after it.
//!
//! The state is fung values behind the run token, so it lives in a
//! `RefCell` touched only by the token's holder. The waiting is Go's
//! runtime channel, written out: a sender that cannot hand its value over
//! queues a `Slot` holding the value; a receiver that finds nothing
//! queues an empty one. The other side completes a slot (moves the value
//! across, marks it done, wakes its task) while holding the token, so the
//! hand-over happens exactly once, on one side, at one moment:
//!
//! - `send` gives the value to the first waiting receiver, or puts it in
//!   the buffer if there is room, or queues and blocks;
//! - `recv` takes the oldest buffered value (refilling the buffer from
//!   the first waiting sender), or the first waiting sender's value, or
//!   queues and blocks;
//! - `close` sets the flag, empties both queues and wakes their tasks,
//!   which see `closed` (a sender's value is never delivered).
//!
//! A blocked task waits through `block`; its slot holds its `Waiter`, so
//! completing or closing wakes exactly that task.
//!
//! Cancellation and timeouts claim a waiter at once, as Go's `select`
//! does, even though the slot stays queued until its task next holds the
//! token: the slot carries the task's cancellation node and its deadline,
//! and pairing skips (and drops from the queue) any slot whose node is
//! cancelled or whose deadline has passed. Pairing happens with the token
//! held and the flag and the clock only move one way, so that check is
//! the moment the slot is decided: a skipped slot's task finds it not
//! done and returns E0701 or `timeout`; a completed slot's task finds it
//! done and succeeds even if it was cancelled just after (the `select`
//! case that moved the value wins, as in Go). A wait that ends without
//! completion takes its slot out of the queue (`leave`, a no-op if
//! pairing already dropped it).
//!
//! A channel can hold values that hold the channel itself (`send(c, c)`):
//! an `Rc` cycle that is never freed, where Go's collector would free it.
//! Stage 3b makes sending a channel into itself an error; until then such
//! a channel leaks.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::agent::{Mark, Serving};
use super::block::{Waiter, block_until, deadline_after};
use super::{CancelNode, Stop, Task, token};
use crate::value::{Value, drop_children, keep};

/// The value `channel()` returns. Two channels are equal only when they
/// are the same channel; it prints as `<channel>`.
pub struct Channel {
    inner: RefCell<Inner>,
}

struct Inner {
    /// The buffer size: 0 for a rendezvous channel.
    cap: usize,
    buf: VecDeque<Value>,
    /// Blocked senders, oldest first; each slot holds its value.
    senders: VecDeque<Rc<Slot>>,
    /// Blocked receivers, oldest first; each slot is empty.
    receivers: VecDeque<Rc<Slot>>,
    closed: bool,
    /// For an agent's request channel (stage 3b, `agent.rs`): the task
    /// serving a request right now and its state.
    serving: Option<Serving>,
}

/// One blocked `send` or `recv`: the value in transit, whether the other
/// side completed it, the task to wake, and what claims the slot before
/// any pairing: the task's cancellation and the receive's deadline. Only
/// the token's holder touches it (it holds a fung value).
struct Slot {
    value: RefCell<Option<Value>>,
    done: Cell<bool>,
    waiter: Arc<Waiter>,
    cancel: Arc<CancelNode>,
    deadline: Option<Instant>,
}

impl Slot {
    fn new(t: &Task<'_>, value: Option<Value>, deadline: Option<Instant>) -> Rc<Slot> {
        Rc::new(Slot {
            value: RefCell::new(value),
            done: Cell::new(false),
            waiter: t.waiter.clone(),
            cancel: t.cancel.clone(),
            deadline,
        })
    }

    /// Whether the slot can no longer be paired: its task was cancelled or
    /// its deadline passed. `block_until` uses the same deadline and the
    /// same comparison, so a slot skipped for its deadline times out.
    fn claimed(&self) -> bool {
        self.cancel.is_cancelled() || self.deadline.is_some_and(|at| Instant::now() >= at)
    }

    /// The other side moved the value: mark it and wake the task.
    fn complete(&self) {
        self.done.set(true);
        self.waiter.wake();
    }
}

/// How a receive ended.
pub(crate) enum Received {
    Value(Value),
    Closed,
    Timeout,
}

impl Channel {
    pub(crate) fn new(cap: usize) -> Channel {
        Channel {
            inner: RefCell::new(Inner {
                cap,
                buf: VecDeque::new(),
                senders: VecDeque::new(),
                receivers: VecDeque::new(),
                closed: false,
                serving: None,
            }),
        }
    }

    fn closed(&self) -> bool {
        self.inner.borrow().closed
    }

    /// Hands `v` to a waiting receiver or the buffer; gives it back when
    /// neither can take it.
    fn try_send(&self, v: Value) -> Result<(), Value> {
        let mut c = self.inner.borrow_mut();
        if let Some(r) = first_live(&mut c.receivers) {
            *r.value.borrow_mut() = Some(v);
            r.complete();
            return Ok(());
        }
        if c.buf.len() < c.cap {
            c.buf.push_back(v);
            return Ok(());
        }
        Err(v)
    }

    /// The oldest buffered value (the first waiting sender's value moves
    /// into the freed place), or a waiting sender's value.
    fn try_recv(&self) -> Option<Value> {
        let mut c = self.inner.borrow_mut();
        if let Some(v) = c.buf.pop_front() {
            if let Some(s) = first_live(&mut c.senders) {
                let moved = s.value.borrow_mut().take().expect("a sender's value");
                c.buf.push_back(moved);
                s.complete();
            }
            return Some(v);
        }
        let s = first_live(&mut c.senders)?;
        let v = s.value.borrow_mut().take().expect("a sender's value");
        s.complete();
        Some(v)
    }

    /// Takes a slot whose wait ended without completion out of its queue.
    fn leave(&self, slot: &Rc<Slot>) {
        let mut c = self.inner.borrow_mut();
        c.senders.retain(|s| !Rc::ptr_eq(s, slot));
        c.receivers.retain(|s| !Rc::ptr_eq(s, slot));
    }

    /// `close`: idempotent. Every blocked task wakes and sees `closed`;
    /// the senders' values are dropped by their own tasks (with the slot).
    pub(crate) fn close(&self) {
        let (senders, receivers) = {
            let mut c = self.inner.borrow_mut();
            c.closed = true;
            (
                std::mem::take(&mut c.senders),
                std::mem::take(&mut c.receivers),
            )
        };
        for s in senders.iter().chain(receivers.iter()) {
            s.waiter.wake();
        }
    }

    /// Records the task serving a request on this agent channel (or none)
    /// and returns the previous record, for the caller to drop once the
    /// borrow has ended.
    pub(crate) fn set_serving(&self, s: Option<Serving>) -> Option<Serving> {
        std::mem::replace(&mut self.inner.borrow_mut().serving, s)
    }

    /// The state the request on this channel is served with, if one of
    /// `marks` (a task's, `Task::serving`) is that request's mark.
    pub(crate) fn serving_state(&self, marks: &[Arc<Mark>]) -> Option<Value> {
        let c = self.inner.borrow();
        let s = c.serving.as_ref()?;
        marks
            .iter()
            .any(|m| Arc::ptr_eq(m, &s.mark))
            .then(|| s.state.clone())
    }

    /// How many blocked sends and receives wait in the queues (tests:
    /// a test waits for a task to be blocked instead of sleeping and
    /// hoping it is).
    #[cfg(test)]
    pub(crate) fn queued(&self) -> usize {
        let c = self.inner.borrow();
        c.senders.len() + c.receivers.len()
    }

    /// Moves every value out for `release` (`value/release.rs`).
    pub(crate) fn spill(&mut self, stack: &mut Vec<Value>) {
        let c = self.inner.get_mut();
        for v in std::mem::take(&mut c.buf) {
            keep(stack, v);
        }
        if let Some(s) = c.serving.take() {
            keep(stack, s.state);
        }
        for s in std::mem::take(&mut c.senders)
            .into_iter()
            .chain(std::mem::take(&mut c.receivers))
        {
            if let Some(v) = s.value.borrow_mut().take() {
                keep(stack, v);
            }
        }
    }
}

/// Pops the first slot that can still be paired, dropping the claimed
/// ones before it from the queue (their tasks leave on their own).
fn first_live(q: &mut VecDeque<Rc<Slot>>) -> Option<Rc<Slot>> {
    while let Some(s) = q.pop_front() {
        if !s.claimed() {
            return Some(s);
        }
    }
    None
}

impl std::fmt::Debug for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<channel>")
    }
}

/// A channel is a value: it drops only on the token's holder, and frees
/// its buffer without recursion (`release`).
impl Drop for Channel {
    fn drop(&mut self) {
        token::assert_holds_token();
        drop_children(|stack| self.spill(stack));
    }
}

/// `send(ch, v)`: `true` once delivered (to a receiver or the buffer),
/// `false` when the channel is or becomes closed first. A cancelled wait
/// unwinds with E0701 unless a receiver already took the value.
pub(crate) fn send(t: &mut Task<'_>, ch: &Rc<Channel>, v: Value) -> Result<bool, Stop> {
    if ch.closed() {
        return Ok(false);
    }
    let v = match ch.try_send(v) {
        Ok(()) => return Ok(true),
        Err(v) => v,
    };
    let slot = Slot::new(t, Some(v), None);
    ch.inner.borrow_mut().senders.push_back(slot.clone());
    let r = block_until(
        t,
        |_, _| {
            if slot.done.get() {
                Some(true)
            } else if ch.closed() {
                Some(false)
            } else {
                None
            }
        },
        None,
        false,
    );
    match r {
        Ok(Some(true)) => Ok(true),
        Ok(_) => {
            ch.leave(&slot);
            Ok(false)
        }
        Err(_) if slot.done.get() => Ok(true),
        Err(e) => {
            ch.leave(&slot);
            Err(e)
        }
    }
}

/// `recv(ch, timeout:)`: buffered values come out even after close; then
/// `Closed`. `Timeout` when `timeout` passes first. A cancelled wait
/// unwinds with E0701 unless a sender already delivered a value.
pub(crate) fn recv(
    t: &mut Task<'_>,
    ch: &Rc<Channel>,
    timeout: Option<Duration>,
) -> Result<Received, Stop> {
    if let Some(v) = ch.try_recv() {
        return Ok(Received::Value(v));
    }
    if ch.closed() {
        return Ok(Received::Closed);
    }
    let deadline = deadline_after(timeout);
    let slot = Slot::new(t, None, deadline);
    ch.inner.borrow_mut().receivers.push_back(slot.clone());
    let r = block_until(
        t,
        |_, _| {
            if slot.done.get() {
                Some(true)
            } else if ch.closed() {
                Some(false)
            } else {
                None
            }
        },
        deadline,
        timeout.is_some(),
    );
    let took = |slot: &Slot| Received::Value(slot.value.borrow_mut().take().expect("a value"));
    match r {
        Ok(Some(true)) => Ok(took(&slot)),
        // Closed while waiting. A waiting receiver means the buffer was
        // empty and nothing can be sent after the close, but look again,
        // as Go does.
        Ok(Some(false)) => {
            ch.leave(&slot);
            Ok(ch.try_recv().map_or(Received::Closed, Received::Value))
        }
        // Pairing skips a slot past its deadline, and `block_until` checks
        // the same deadline only right after a check that found the slot
        // not done, so a timed-out slot is never done; Go's order (a value
        // first) is kept anyway.
        Ok(None) | Err(_) if slot.done.get() => Ok(took(&slot)),
        Ok(None) => {
            ch.leave(&slot);
            Ok(Received::Timeout)
        }
        Err(e) => {
            ch.leave(&slot);
            Err(e)
        }
    }
}

/// Receives until the channel is closed (Go `Channel.Drain`): what the
/// iterating natives walk for a channel.
pub(crate) fn drain(t: &mut Task<'_>, ch: &Rc<Channel>) -> Result<Vec<Value>, Stop> {
    let mut out = Vec::new();
    loop {
        match recv(t, ch, None)? {
            Received::Value(v) => out.push(v),
            Received::Closed | Received::Timeout => return Ok(out),
        }
    }
}
