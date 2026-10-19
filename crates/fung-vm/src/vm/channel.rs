//! Channels (05): a queue between
//! tasks, reached through two ends. `channel()` gives `(tx, rx)`: a
//! sending end and a receiving end, each a value of its own that can be
//! copied. `channel()` is unbuffered, so `send` waits until a receiver
//! takes the value (맞교환); `channel(n)` holds up to `n` values. `close`
//! is a flag, not an end of the queue: values buffered before it can
//! still be received, then `recv` gives `Err({kind: 'closed})`, and so
//! does any `send` after it.
//!
//! The state is fung values, so it lives in a `RefCell` touched only by
//! the run thread. A sender that cannot
//! hand its value over
//! queues a `Slot` holding the value; a receiver that finds nothing
//! queues an empty one. The other side completes a slot (moves the value
//! across, marks it done, wakes its task) in one turn of the run thread, so the
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
//! Cancellation and timeouts claim a waiter at once, even though the slot stays queued until its task next runs: the slot carries the task's cancellation node, and pairing skips
//! (and drops from the queue) any slot whose node is cancelled or whose
//! node's deadline (`deadline.rs`) has passed. Pairing happens on the run
//! thread and the flag and the clock only move one way, so that check is
//! the moment the slot is decided: a skipped slot's task finds it not
//! done and returns E0701; a completed slot's task finds it
//! done and succeeds even if it was cancelled just after (the value that
//! moved wins). A wait that ends without
//! completion takes its slot out of the queue (`leave`, a no-op if
//! pairing already dropped it).
//!
//! **The ends close the channel by going away** (concurrency v2, L4).
//! Each end is one `Rc` (`Sender`, `Receiver`) made by `channel()`; a
//! fung copy of an end is a copy of that `Rc`, wherever it is kept (a
//! variable, a closure, a list, another channel's buffer). So the moment
//! the count reaches 0, the `Drop` below, is the moment the last copy
//! went:
//!
//! - the last sending end: the channel closes, as by `close(tx)`; the
//!   receivers still get what is buffered, then `closed`;
//! - the last receiving end: the channel closes and drops its buffer
//!   (`abandon`); nobody can receive any more, so a send fails at once.
//!
//! One `closed` flag serves both: with no receiving end left, a close
//! that keeps the buffer and one that drops it look the same.
//!
//! A channel whose buffer holds one of its own ends would keep itself
//! open forever: an `Rc` cycle that is never freed. So `send` refuses a
//! value that contains an end of the same channel (E0705; `contains`).
//! Two channels holding each other's ends are not caught.

use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use super::block::Waiter;
use super::{CancelNode, Stop, Task, Wake};
use crate::value::{Value, drop_children, keep};

/// The queue both ends of a channel share.
pub struct Channel {
    inner: RefCell<Inner>,
    /// Woken from outside the program (the stop channel, `stop.rs`): a
    /// task waiting on it is not counted stuck by the deadlock check.
    external: bool,
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
}

/// One blocked `send` or `recv`: the value in transit, whether the other
/// side completed it, the task to wake, and what claims the slot before
/// any pairing: the task's cancellation and its deadline. Only
/// the run thread touches it (it holds a fung value).
struct Slot {
    value: RefCell<Option<Value>>,
    done: Cell<bool>,
    /// In one of the channel's queues: a dropped wait must leave it
    /// (`Drop` of `RecvWait`, `SendWait`), one that was taken out need not
    /// scan for it.
    queued: Cell<bool>,
    waiter: Arc<Waiter>,
    cancel: Arc<CancelNode>,
}

impl Slot {
    fn new(t: &Task<'_>, value: Option<Value>) -> Rc<Slot> {
        Rc::new(Slot {
            value: RefCell::new(value),
            done: Cell::new(false),
            queued: Cell::new(false),
            waiter: t.waiter.clone(),
            cancel: t.cancel.clone(),
        })
    }

    /// Whether the slot can no longer be paired: its task was cancelled or
    /// its node's deadline passed. `wait_parked` uses the same deadline
    /// and the same comparison, so a slot skipped for its deadline unwinds.
    fn claimed(&self) -> bool {
        self.cancel.is_cancelled()
            || self
                .cancel
                .deadline_at()
                .is_some_and(|at| Instant::now() >= at)
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
}

impl Channel {
    pub(crate) fn new(cap: usize) -> Channel {
        Channel {
            external: false,
            inner: RefCell::new(Inner {
                cap,
                buf: VecDeque::new(),
                senders: VecDeque::new(),
                receivers: VecDeque::new(),
                closed: false,
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
        if !slot.queued.replace(false) {
            return;
        }
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
            s.queued.set(false);
            s.waiter.wake();
        }
    }

    /// The last receiving end went: closes, and moves the buffered values
    /// out to be dropped (`Receiver`'s `Drop`, `release`). Idempotent.
    pub(crate) fn abandon(&self, stack: &mut Vec<Value>) {
        self.close();
        let buf = std::mem::take(&mut self.inner.borrow_mut().buf);
        for v in buf {
            keep(stack, v);
        }
    }

    /// How many blocked sends and receives wait in the queues (tests:
    /// a test waits for a task to be blocked instead of sleeping and
    /// hoping it is).
    #[cfg(test)]
    pub(crate) fn queued(&self) -> usize {
        let c = self.inner.borrow();
        c.senders.len() + c.receivers.len()
    }

    /// How many values wait in the buffer (tests: a check that does not
    /// block, now that a receive has no timeout of its own).
    #[cfg(test)]
    pub(crate) fn buffered(&self) -> usize {
        self.inner.borrow().buf.len()
    }

    /// Puts `v` in the buffer whatever its size (tests: chains of
    /// channels built without a task).
    #[cfg(test)]
    pub(crate) fn put(&self, v: Value) {
        self.inner.borrow_mut().buf.push_back(v);
    }

    /// Moves every value out for `release` (`value/release.rs`).
    pub(crate) fn spill(&mut self, stack: &mut Vec<Value>) {
        let c = self.inner.get_mut();
        for v in std::mem::take(&mut c.buf) {
            keep(stack, v);
        }
        for s in std::mem::take(&mut c.senders)
            .into_iter()
            .chain(std::mem::take(&mut c.receivers))
        {
            s.queued.set(false);
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
        s.queued.set(false);
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

/// The sending end `channel()` returns; it prints as `<sender>`. When
/// the last copy goes, the channel closes.
pub struct Sender(pub(crate) Rc<Channel>);

/// The receiving end `channel()` returns; it prints as `<receiver>`.
/// When the last copy goes, the channel closes and drops its buffer.
pub struct Receiver(pub(crate) Rc<Channel>);

/// `channel(n)`: the two ends of a new channel.
pub(crate) fn pair(cap: usize) -> (Sender, Receiver) {
    let ch = Rc::new(Channel::new(cap));
    (Sender(ch.clone()), Receiver(ch))
}

/// The ends of a channel woken from outside the program (the stop
/// channel): waiting on it is not a deadlock.
pub(crate) fn pair_external() -> (Sender, Receiver) {
    let mut ch = Channel::new(0);
    ch.external = true;
    let ch = Rc::new(ch);
    (Sender(ch.clone()), Receiver(ch))
}

impl std::fmt::Debug for Sender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<sender>")
    }
}

impl std::fmt::Debug for Receiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<receiver>")
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        self.0.close();
    }
}

/// The buffer goes through `release`: it may hold the receiving end of
/// another channel, which may hold another, and so on.
impl Drop for Receiver {
    fn drop(&mut self) {
        drop_children(|stack| self.0.abandon(stack));
    }
}

/// A channel is a value: it drops only on the run thread, and frees
/// its buffer without recursion (`release`).
impl Drop for Channel {
    fn drop(&mut self) {
        drop_children(|stack| self.spill(stack));
    }
}

/// Whether `v` contains an end of `ch` (E0705, `send` only): `v` is one,
/// or holds one somewhere inside. A value that holds nothing mutable
/// (`Value::holds_mutable`, O(1) for the big collections) is not walked. Cost is linear in the size of `v`, in constant
/// Rust stack.
///
/// - Scanned: lists (their items, so views of a persistent vector are
///   walked through the view), map values (keys are strings), tuples,
///   tag arguments and the values a closure captured.
/// - Not scanned:
///   - other channels' buffers: a cycle through two channels is not
///     caught (spec, documented);
///   - a closure's module globals and module values: a module's globals
///     already hold its closures, which hold the globals, so those
///     cycles exist without channels and are broken when the runtime
///     ends (`World::release_values`) — which frees a channel reached
///     only through them too; walking them would also mean walking the
///     whole program's state on every send;
///   - task handles: a running task's result does not exist yet, so
///     looking at it would make the error depend on timing;
///   - strings, numbers, symbols,
///     durations, ranges, natives and nil.
///
/// Explicit stack, no recursion: a million nested lists must not
/// overflow. A container reached through a shared `Rc` (count above 1)
/// is recorded by address and walked once, so a value with many paths
/// to the same part (`[x, x]` nested) costs its node count, not its
/// path count; an unshared one has one path, so it is not recorded.
pub(crate) fn contains(ch: &Rc<Channel>, v: &Value) -> bool {
    if !v.holds_mutable() {
        return false;
    }
    let mut stack: Vec<&Value> = Vec::new();
    let mut seen: HashSet<*const ()> = HashSet::new();
    if visit(ch, v, &mut stack) {
        return true;
    }
    while let Some(x) = stack.pop() {
        let (ptr, shared) = match x {
            Value::List(rc) => (Rc::as_ptr(rc).cast(), Rc::strong_count(rc) > 1),
            Value::Map(rc) => (Rc::as_ptr(rc).cast(), Rc::strong_count(rc) > 1),
            Value::Tuple(rc) => (Rc::as_ptr(rc).cast(), Rc::strong_count(rc) > 1),
            Value::Tag(rc) => (Rc::as_ptr(rc).cast(), Rc::strong_count(rc) > 1),
            Value::Closure(rc) => (Rc::as_ptr(rc).cast(), Rc::strong_count(rc) > 1),
            _ => continue,
        };
        if shared && !seen.insert(ptr) {
            continue;
        }
        let found = match x {
            Value::List(l) => l.iter().any(|c| visit(ch, c, &mut stack)),
            Value::Map(m) => m.iter().any(|(_, c)| visit(ch, c, &mut stack)),
            Value::Tuple(t) => t.iter().any(|c| visit(ch, c, &mut stack)),
            Value::Tag(t) => t.args.iter().any(|c| visit(ch, c, &mut stack)),
            Value::Closure(f) => f.captured.iter().any(|c| visit(ch, c, &mut stack)),
            _ => false,
        };
        if found {
            return true;
        }
    }
    false
}

/// One value met by `contains`: `true` if it is `ch`; a container is pushed to be walked, anything else is skipped.
fn visit<'a>(ch: &Rc<Channel>, v: &'a Value, stack: &mut Vec<&'a Value>) -> bool {
    match v {
        Value::Sender(e) => Rc::ptr_eq(&e.0, ch),
        Value::Receiver(e) => Rc::ptr_eq(&e.0, ch),
        Value::List(_) | Value::Map(_) | Value::Tuple(_) | Value::Tag(_) | Value::Closure(_) => {
            stack.push(v);
            false
        }
        _ => false,
    }
}

/// What starting a `send` or `recv` gave: the result now, or a wait to
/// park on (`cont.rs`).
pub(crate) enum Try<T, W> {
    Ready(T),
    Wait(W),
}

/// A `recv` whose slot waits in the channel's queue. It holds the
/// receiving end it was called with, as the waiting native's arguments
/// did: a waiting receiver keeps its end alive (`recv(stop_signal())`
/// is a watcher while it waits, `stop.rs`).
pub(crate) struct RecvWait {
    end: Rc<Receiver>,
    slot: Rc<Slot>,
}

/// A `send` whose slot (holding its value) waits in the channel's queue,
/// holding its sending end alive the same way.
pub(crate) struct SendWait {
    end: Rc<Sender>,
    slot: Rc<Slot>,
}

/// `send(tx, v)` on the channel `ch`: `true` once delivered (to a
/// receiver or the buffer), `false` when the channel is closed. Otherwise
/// the sender queues a slot with the value and waits (`SendWait`).
pub(crate) fn begin_send(t: &mut Task<'_>, end: &Rc<Sender>, v: Value) -> Try<bool, SendWait> {
    let ch = &end.0;
    if ch.closed() {
        return Try::Ready(false);
    }
    let v = match ch.try_send(v) {
        Ok(()) => return Try::Ready(true),
        Err(v) => v,
    };
    // The woken flag is taken before the slot is queued: a wake that
    // comes after it is seen (`block.rs`).
    t.waiter.take_woken();
    let slot = Slot::new(t, Some(v));
    slot.queued.set(true);
    ch.inner.borrow_mut().senders.push_back(slot.clone());
    Try::Wait(SendWait {
        end: end.clone(),
        slot,
    })
}

impl SendWait {
    /// After a wake: `Some(true)` once a receiver took the value,
    /// `Some(false)` if the channel closed first (the slot leaves the
    /// queue), `None` to wait again.
    pub(crate) fn poll(&self) -> Option<bool> {
        if self.slot.done.get() {
            Some(true)
        } else if self.end.0.closed() {
            self.end.0.leave(&self.slot);
            Some(false)
        } else {
            None
        }
    }

    /// The wait was cancelled (or reached a deadline): a value a receiver
    /// already took stays sent; otherwise the slot leaves the queue and
    /// `e` goes on.
    pub(crate) fn cancel(self, e: Stop) -> Result<bool, Stop> {
        if self.slot.done.get() {
            return Ok(true);
        }
        self.end.0.leave(&self.slot);
        Err(e)
    }

    pub(crate) fn wake(&self) -> Wake {
        Wake::default()
    }
}

/// `recv(rx)` on the channel `ch`: buffered values come out even after
/// close; then `Closed`. Otherwise the receiver queues an empty slot and
/// waits (`RecvWait`).
pub(crate) fn begin_recv(
    t: &mut Task<'_>,
    end: &Rc<Receiver>,
) -> Result<Try<Received, RecvWait>, Stop> {
    let ch = &end.0;
    // Receiving takes a value out (and may wait): an effect inside a
    // swap's function however it is reached — `recv`, a `for` over the
    // channel, `map` or `fold` of it (`atom.rs`).
    if t.pure > 0 {
        return Err(crate::native::nat::fail(
            "E0707",
            vec![fung_front::diag::Arg::Str("recv".into())],
        ));
    }
    if let Some(v) = ch.try_recv() {
        return Ok(Try::Ready(Received::Value(v)));
    }
    if ch.closed() {
        return Ok(Try::Ready(Received::Closed));
    }
    t.waiter.take_woken();
    let slot = Slot::new(t, None);
    slot.queued.set(true);
    ch.inner.borrow_mut().receivers.push_back(slot.clone());
    Ok(Try::Wait(RecvWait {
        end: end.clone(),
        slot,
    }))
}

/// A dropped wait (its task abandoned, REPL's second Ctrl-C) leaves the
/// channel's queue: no later `send` hands a value to it.
impl Drop for RecvWait {
    fn drop(&mut self) {
        // a scan only for a slot still queued (`Slot::queued`)
        self.end.0.leave(&self.slot);
    }
}

impl Drop for SendWait {
    fn drop(&mut self) {
        self.end.0.leave(&self.slot);
    }
}

impl RecvWait {
    /// After a wake: the value a sender delivered, or, once the channel
    /// closed (the slot leaves the queue), what the buffer still holds or
    /// `Closed`; `None` to wait again.
    pub(crate) fn poll(&self) -> Option<Received> {
        if self.slot.done.get() {
            return Some(self.took());
        }
        let ch = &self.end.0;
        if ch.closed() {
            // A waiting receiver means the buffer was empty and nothing
            // can be sent after the close, but look again.
            ch.leave(&self.slot);
            return Some(ch.try_recv().map_or(Received::Closed, Received::Value));
        }
        None
    }

    /// The wait was cancelled (or reached a deadline): a value a sender
    /// already delivered is still taken; otherwise the slot leaves the
    /// queue and `e` goes on.
    pub(crate) fn cancel(self, e: Stop) -> Result<Value, Stop> {
        if self.slot.done.get() {
            let Received::Value(v) = self.took() else {
                unreachable!("a delivered slot holds a value")
            };
            return Ok(v);
        }
        self.end.0.leave(&self.slot);
        Err(e)
    }

    fn took(&self) -> Received {
        Received::Value(self.slot.value.borrow_mut().take().expect("a value"))
    }

    /// The stop channel is woken from outside: not a deadlock.
    pub(crate) fn wake(&self) -> Wake {
        Wake {
            until: None,
            timed: self.end.0.external,
        }
    }
}
