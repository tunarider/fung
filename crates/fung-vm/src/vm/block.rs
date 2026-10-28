//! Where a task waits (stage 3a: 막히는 지점; since R1b on the scheduler).
//!
//! A native that waits for another task does not sleep itself: it
//! registers the task's `Waiter` on what it waits for and returns a park
//! request (`Next::Park`, `cont.rs`; `park_until` is the common form). The
//! run loop returns to the scheduler (`sched.rs`), which keeps the task
//! aside until a wake, its deadline or a cancellation; then the parked
//! continuation looks again. An event wakes only the waiters registered
//! on what changed. Waits for the operating system are handed off
//! (`nat::offload`, `offload.rs`).
//!
//! Turn handoff (차례 넘기기) lives here too: a task that computes without
//! waiting gives its turn every `YIELD_EVERY` calls (`Task::tick`).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use fung_front::diag::error;
use fung_front::source::Pos;

use super::sched::{FiberId, Ready};
use super::{CancelNode, Stop, Task};
use crate::value::Value;

/// How many calls and backward jumps a task makes before it gives its
/// turn (`Task::tick`): the length of a computing task's turn. Giving the
/// turn returns to the scheduler (`sched.rs`), which is cheap, so the
/// value trades how soon a woken task runs next to a computing one against
/// how often the scheduler is entered. Measured (R1b, release, Apple M2
/// Max, 11-run medians): a task doing 200 `sleep(1ms)` next to a spinner
/// took 0.308 s at 10,000, 0.226 s at 2,000, 0.216 s at 1,000, 0.212 s at
/// 500 and 0.210 s at 200 (0.2 s is no delay); `fib`, `loop`, `list` and
/// two `fib(27)` tasks were the same within noise (±2%) at every value.
/// Below 1,000 a woken task gains under 0.03 ms while the scheduler is
/// entered five times as often, so 1,000.
pub(crate) const YIELD_EVERY: u32 = 1_000;

/// E0701 `task cancelled`, the internal error a cancelled task unwinds
/// with (never printed: its handle turns it into `Err({kind: 'cancelled, …})`).
#[cold]
pub(crate) fn cancelled(pos: Pos) -> Stop {
    Stop::Error(Box::new(error("E0701", pos, Vec::new())))
}

/// A task's wake-up call: its number and the ready queue (`sched.rs`),
/// and a flag that says it was woken since it last looked. Each task has
/// one for its life; a wait registers it on what the task waits for, and
/// whoever changes that thing wakes the waiters registered on it, and
/// only those. Any thread may wake it (an offload's job, the standard
/// input reader).
pub(crate) struct Waiter {
    id: FiberId,
    woken: AtomicBool,
    ready: Arc<Ready>,
}

impl Waiter {
    /// The waiter of task `id`, which `ready` runs.
    pub(crate) fn new(id: FiberId, ready: Arc<Ready>) -> Arc<Waiter> {
        Arc::new(Waiter {
            id,
            woken: AtomicBool::new(false),
            ready,
        })
    }

    pub(crate) fn id(&self) -> FiberId {
        self.id
    }

    /// Wakes the task: it goes on the ready queue, once until it looks
    /// again (`take_woken`).
    pub(crate) fn wake(&self) {
        if !self.woken.swap(true, Ordering::AcqRel) {
            self.ready.push(self.id);
        }
    }

    /// Takes the woken flag (true once per wake). A wait takes it before
    /// it registers the waiter, so a wake after that queues the task.
    pub(crate) fn take_woken(&self) -> bool {
        self.woken.swap(false, Ordering::AcqRel)
    }
}

/// The waiters registered on one thing a task can wait for (a task's
/// result, a cancellation node, the runtime's last running task; a
/// channel keeps one waiter per queued slot instead, `channel.rs`).
/// Changed only on the run thread
/// (or under the node's own lock for cancellation nodes).
///
/// `wake_all` empties it, so a waiter is woken at most once per
/// registration, in the order they were added. A wait that ends early (a
/// cancellation, a deadline) takes its waiter out (`remove`); one that
/// does not is woken spuriously, which the wait absorbs. A waiter is in a
/// list at most once, so a list never holds more entries than there are
/// tasks.
///
/// Adding and removing are O(1) however many tasks wait on one thing
/// (review C3): a removed waiter leaves a hole, and the holes are dropped
/// once they are half the slots; past `INDEXED` slots an index by task
/// number finds a waiter, below that a look along the slots is quicker.
#[derive(Default)]
pub(crate) struct WaitList {
    /// In the order they were added; `None` where one was removed.
    slots: Vec<Option<Arc<Waiter>>>,
    /// The slot of each waiter, by task number, once there are many.
    index: Option<HashMap<FiberId, usize>>,
    /// How many slots are holes.
    holes: usize,
}

/// The slot count past which a [`WaitList`] keeps its index.
const INDEXED: usize = 16;

impl WaitList {
    /// Adds `w`, unless it is in already; true if it was not.
    pub(crate) fn add(&mut self, w: &Arc<Waiter>) -> bool {
        if self.find(w.id()).is_some() {
            return false;
        }
        if let Some(index) = &mut self.index {
            index.insert(w.id(), self.slots.len());
        }
        self.slots.push(Some(w.clone()));
        if self.index.is_none() && self.slots.len() > INDEXED {
            self.reindex();
        }
        true
    }

    /// Takes `w` out, if it is in.
    pub(crate) fn remove(&mut self, w: &Waiter) {
        let Some(i) = self.find(w.id()) else {
            return;
        };
        if let Some(index) = &mut self.index {
            index.remove(&w.id());
        }
        self.slots[i] = None;
        self.holes += 1;
        if self.holes * 2 >= self.slots.len() {
            self.slots.retain(Option::is_some);
            self.holes = 0;
            self.reindex();
        }
    }

    /// How many waiters are in.
    pub(crate) fn len(&self) -> usize {
        self.slots.len() - self.holes
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Wakes every waiter, in the order they were added, and empties the
    /// list.
    pub(crate) fn wake_all(&mut self) {
        self.index = None;
        self.holes = 0;
        for w in self.slots.drain(..).flatten() {
            w.wake();
        }
    }

    fn find(&self, id: FiberId) -> Option<usize> {
        match &self.index {
            Some(index) => index.get(&id).copied(),
            None => self
                .slots
                .iter()
                .position(|s| s.as_ref().is_some_and(|w| w.id() == id)),
        }
    }

    /// Builds the index if the list is long, or drops it if not (the
    /// slots have no holes).
    fn reindex(&mut self) {
        self.index = (self.slots.len() > INDEXED).then(|| {
            self.slots
                .iter()
                .enumerate()
                .filter_map(|(i, s)| s.as_ref().map(|w| (w.id(), i)))
                .collect()
        });
    }
}

/// The poll of a [`park_until`] wait.
type Poll = Box<dyn FnMut(&mut Task<'_>, &Arc<Waiter>) -> Option<Result<Value, Stop>>>;

/// Waits for `poll` to give a result, parking the task between looks:
/// the common wait whose condition one closure checks (`block`'s
/// `ready`). `poll` runs on the run thread, first at once and then
/// after every wake; when it returns `None` it must have registered the
/// waiter on what it waits for.
pub(crate) fn park_until(
    t: &mut Task<'_>,
    mut poll: impl FnMut(&mut Task<'_>, &Arc<Waiter>) -> Option<Result<Value, Stop>> + 'static,
    wake: super::Wake,
) -> Result<super::Flow, Stop> {
    let me = t.waiter.clone();
    me.take_woken();
    if let Some(r) = poll(t, &me) {
        return r.map(super::Flow::Done);
    }
    Ok(super::Flow::Park(
        Box::new(Parked {
            poll: Box::new(poll),
            wake,
            on_stop: None,
        }),
        wake,
    ))
}

/// [`park_until`] with `on_stop` run when a stop (a cancellation, a
/// deadline) ends the wait: to take the waiter off what it waited on.
pub(crate) fn park_until_then(
    t: &mut Task<'_>,
    mut poll: impl FnMut(&mut Task<'_>, &Arc<Waiter>) -> Option<Result<Value, Stop>> + 'static,
    wake: super::Wake,
    on_stop: impl FnMut(&mut Task<'_>) + 'static,
) -> Result<super::Flow, Stop> {
    let me = t.waiter.clone();
    me.take_woken();
    if let Some(r) = poll(t, &me) {
        return r.map(super::Flow::Done);
    }
    Ok(super::Flow::Park(
        Box::new(Parked {
            poll: Box::new(poll),
            wake,
            on_stop: Some(Box::new(on_stop)),
        }),
        wake,
    ))
}

/// What a look of a [`park_io`] wait found.
pub(crate) enum IoStep {
    /// The wait is over.
    Done(Result<Value, Stop>),
    /// Wait until the descriptor is ready for the interest (or the
    /// deadline passes).
    Wait(std::os::fd::RawFd, super::poller::Interest, Option<Instant>),
    /// Wait for a wake only: the waiter is registered elsewhere (a
    /// connection's turn, `net.rs`).
    Park,
    /// Look again at this time: no event says when (a FIFO's write end
    /// waiting for a reader, `pipe.rs`).
    Until(Instant),
}

/// The look of a wait on a descriptor.
type IoPoll = Box<dyn FnMut(&mut Task<'_>, &Arc<Waiter>) -> IoStep>;

/// Parks until a descriptor is ready (R4a): `poll` tries the system
/// call and, when it would block, says on which descriptor and for what
/// (`IoStep::Wait`); the task registers that in the event loop under a
/// fresh token and parks, then looks again when the loop says it is
/// ready, a deadline passes, or something else wakes it. Each look may
/// wait on another descriptor (a TLS handshake reads and writes). Such a
/// park is one the world outside ends (not a deadlock). `on_stop` runs
/// when a stop unwinds the wait.
///
/// The token names this one registration: when the wait ends (its event
/// came, or a timer, a stop, a wake for something else) it leaves the
/// table, so a later event under it wakes nobody (`Sched::poll_io`).
pub(crate) fn park_io(
    t: &mut Task<'_>,
    poll: impl FnMut(&mut Task<'_>, &Arc<Waiter>) -> IoStep + 'static,
    on_stop: impl FnMut(&mut Task<'_>) + 'static,
) -> Result<super::Flow, Stop> {
    let me = t.waiter.clone();
    me.take_woken();
    let mut w = IoWait {
        regs: IoRegs::default(),
        poll: Box::new(poll),
        on_stop: Box::new(on_stop),
    };
    let step = (w.poll)(t, &me);
    Ok(match w.after_or_stop(t, step)? {
        Look::Done(v) => super::Flow::Done(v),
        Look::Park(wake) => super::Flow::Park(Box::new(w), wake),
    })
}

/// Drops the registrations on `fd` (it is being closed) and wakes their
/// tasks, which then see it closed. Done before the descriptor closes, so
/// that no registration outlives it under a number a new descriptor may
/// take.
pub(crate) fn drop_io_waits(t: &mut Task<'_>, fd: std::os::fd::RawFd) {
    let tokens = t.world.io_fds.remove(&fd).unwrap_or_default();
    for k in tokens {
        if let Some((w, _)) = t.world.io.remove(&k) {
            w.wake();
        }
    }
    t.shared
        .poller
        .deregister(fd, super::poller::Interest::Read);
    t.shared
        .poller
        .deregister(fd, super::poller::Interest::Write);
}

impl Task<'_> {
    /// The registrations in the event loop's table and in its fd index
    /// (tests).
    #[cfg(test)]
    pub(crate) fn io_table(&self) -> (usize, usize) {
        let w = &self.world;
        (w.io.len(), w.io_fds.values().map(Vec::len).sum())
    }
}

enum Look {
    Done(Value),
    Park(super::Wake),
}

/// What a registration in the event loop waits on.
#[derive(Clone, Copy, Debug)]
pub(crate) enum IoKey {
    Fd(std::os::fd::RawFd, super::poller::Interest),
    /// A child's end through kqueue's `EVFILT_PROC` (a pidfd is an `Fd`).
    Exit(libc::pid_t),
}

/// Registrations one continuation holds at once (a command waits for
/// its end and its three pipes), each under a fresh token; they leave
/// together (`leave_all`). A token that is no longer in the table had
/// its event (or its descriptor closed): it is not deregistered again.
#[derive(Default)]
pub(crate) struct IoRegs(Vec<(u64, IoKey)>);

impl IoRegs {
    fn add(&mut self, t: &mut Task<'_>, key: IoKey) -> Result<bool, Stop> {
        let token = t.shared.next_token.fetch_add(1, Ordering::Relaxed);
        let r = match key {
            IoKey::Fd(fd, interest) => t.shared.poller.register(fd, interest, token).map(|()| true),
            IoKey::Exit(pid) => t
                .shared
                .poller
                .register_exit(&super::poller::ExitWatch::Pid(pid), token),
        };
        match r {
            Ok(true) => {
                let w = t.waiter.clone();
                t.world.add_io(token, w, key);
                self.0.push((token, key));
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(e) => Err(crate::native::nat::fail(
                "E0801",
                vec![fung_front::diag::Arg::Str(format!(
                    "cannot wait for a descriptor: {e}"
                ))],
            )),
        }
    }

    /// Waits for `fd` to be ready for `interest`.
    pub(crate) fn fd(
        &mut self,
        t: &mut Task<'_>,
        fd: std::os::fd::RawFd,
        interest: super::poller::Interest,
    ) -> Result<(), Stop> {
        self.add(t, IoKey::Fd(fd, interest)).map(drop)
    }

    /// Waits for the end of `w`'s child; false if it has ended already.
    pub(crate) fn exit(
        &mut self,
        t: &mut Task<'_>,
        w: &super::poller::ExitWatch,
    ) -> Result<bool, Stop> {
        use std::os::fd::AsRawFd;
        match w {
            super::poller::ExitWatch::Pid(pid) => self.add(t, IoKey::Exit(*pid)),
            super::poller::ExitWatch::Fd(fd) => {
                self.add(t, IoKey::Fd(fd.as_raw_fd(), super::poller::Interest::Read))
            }
            super::poller::ExitWatch::Unsupported => Ok(true),
        }
    }

    pub(crate) fn leave_all(&mut self, t: &mut Task<'_>) {
        for (token, key) in self.0.drain(..) {
            if t.world.take_io(token).is_some() {
                match key {
                    IoKey::Fd(fd, interest) => t.shared.poller.deregister(fd, interest),
                    IoKey::Exit(pid) => t
                        .shared
                        .poller
                        .deregister_exit(&super::poller::ExitWatch::Pid(pid)),
                }
            }
        }
    }
}

/// A `park_io` wait between looks.
struct IoWait {
    /// The registration in the loop, while there is one.
    regs: IoRegs,
    poll: IoPoll,
    on_stop: OnStop,
}

impl IoWait {
    /// `after`, and when that fails (the loop refused a registration)
    /// the wait ends as a stopped one: it leaves the loop and `on_stop`
    /// runs, before the error unwinds past it.
    fn after_or_stop(&mut self, t: &mut Task<'_>, step: IoStep) -> Result<Look, Stop> {
        let r = self.after(t, step);
        if r.is_err() {
            self.leave(t);
            (self.on_stop)(t);
        }
        r
    }

    fn after(&mut self, t: &mut Task<'_>, step: IoStep) -> Result<Look, Stop> {
        match step {
            IoStep::Done(r) => r.map(Look::Done),
            IoStep::Park => Ok(Look::Park(super::Wake {
                until: None,
                timed: false,
            })),
            IoStep::Wait(fd, interest, until) => {
                self.register(t, fd, interest)?;
                Ok(Look::Park(super::Wake { until, timed: true }))
            }
            IoStep::Until(at) => Ok(Look::Park(super::Wake {
                until: Some(at),
                timed: true,
            })),
        }
    }

    fn register(
        &mut self,
        t: &mut Task<'_>,
        fd: std::os::fd::RawFd,
        interest: super::poller::Interest,
    ) -> Result<(), Stop> {
        self.regs.fd(t, fd, interest)
    }

    /// Ends the registration: its event came (the loop took the token
    /// out of the table), its descriptor closed (`drop_io_waits`), or it
    /// is dropped here.
    fn leave(&mut self, t: &mut Task<'_>) {
        self.regs.leave_all(t);
    }
}

impl super::Cont for IoWait {
    fn resume(&mut self, t: &mut Task<'_>, _: Value) -> Result<super::Next, Stop> {
        self.leave(t);
        let me = t.waiter.clone();
        let step = (self.poll)(t, &me);
        Ok(match self.after_or_stop(t, step)? {
            Look::Done(v) => super::Next::Done(v),
            Look::Park(wake) => super::Next::Park(wake),
        })
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<super::Next, Stop> {
        self.leave(t);
        (self.on_stop)(t);
        Err(stop)
    }
}

/// What a stopped `park_until` wait runs before it unwinds.
type OnStop = Box<dyn FnMut(&mut Task<'_>)>;

/// A `park_until` wait between looks.
struct Parked {
    poll: Poll,
    wake: super::Wake,
    on_stop: Option<OnStop>,
}

impl super::Cont for Parked {
    fn resume(&mut self, t: &mut Task<'_>, _: Value) -> Result<super::Next, Stop> {
        let me = t.waiter.clone();
        match (self.poll)(t, &me) {
            Some(r) => r.map(super::Next::Done),
            None => Ok(super::Next::Park(self.wake)),
        }
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<super::Next, Stop> {
        if let Some(f) = &mut self.on_stop {
            f(t);
        }
        Err(stop)
    }
}

impl Task<'_> {
    /// The event loop (a native that waits on more than descriptors).
    pub(crate) fn shared_poller(&self) -> &super::poller::Poller {
        &self.shared.poller
    }

    /// This task's waiter (a native that wakes it from a job).
    pub(crate) fn waiter_handle(&self) -> Arc<Waiter> {
        self.waiter.clone()
    }

    /// Puts `fresh` root in place of this task's node and gives the node
    /// back (a wait that a stop must not cut, `process.rs`).
    pub(crate) fn swap_in_fresh_root(&mut self) -> Arc<CancelNode> {
        std::mem::replace(&mut self.cancel, CancelNode::root())
    }

    /// Gives this task its node back after `swap_in_fresh_root`; a
    /// deadline that passed meanwhile is enforced.
    pub(crate) fn restore_cancel_node(&mut self, node: Arc<CancelNode>) {
        self.cancel = node;
        if let Some(at) = self.cancel.deadline_at() {
            let now = Instant::now();
            if now >= at {
                self.cancel.expire_due(now);
            }
        }
    }

    /// The runtime's thread pool (a native's `Drop` that hands work off).
    pub(crate) fn pool(&self) -> super::pool::Pool {
        self.shared.pool.clone()
    }

    /// The runtime's closer, for a file handle dropped open (`closer.rs`).
    pub(crate) fn closer(&self) -> super::Closer {
        self.shared.closer.clone()
    }

    /// This task's cancellation node, for a native that waits off the
    /// run thread and must see its cancellation there (`process.rs`).
    pub(crate) fn cancel_node(&self) -> Arc<CancelNode> {
        self.cancel.clone()
    }

    /// Unwinds a cancelled task (its deadline included, once it was
    /// expired): for a native whose wait the cancellation cut short.
    pub(crate) fn check_cancelled(&self) -> Result<(), Stop> {
        if self.cancel.is_cancelled() {
            return Err(self.unwound());
        }
        Ok(())
    }

    /// The check at every call and backward jump: unwinds a cancelled
    /// task (E0701 at `pos`) and, every `YIELD_EVERY` ticks, ends its turn
    /// so the other ready tasks run.
    #[inline(always)]
    pub(super) fn tick(&mut self, pos: Pos) -> Result<(), Stop> {
        if self.cancel.is_cancelled() {
            return Err(cancelled(pos));
        }
        self.budget -= 1;
        if self.budget == 0 {
            self.end_turn();
        }
        Ok(())
    }

    /// The end of a turn: a computing task sees its deadline here
    /// (`deadline.rs`; the next tick unwinds), and the next yield point
    /// leaves the run loop (`Ran::Yielded`) so the other ready tasks run
    /// first (`sched.rs`).
    #[cold]
    #[inline(never)]
    fn end_turn(&mut self) {
        self.budget = YIELD_EVERY;
        if let Some(at) = self.cancel.deadline_at() {
            let now = Instant::now();
            if now >= at {
                self.cancel.expire_due(now);
            }
        }
        // A swap's function runs to its end in one turn: a task that swaps
        // the same atom on every turn would otherwise make it retry forever
        // (it cannot park: effects are E0707). The events other threads
        // posted are made here then, so that a signal or the REPL's Ctrl-C
        // still reaches a function that never ends (review C6). The turn
        // is owed, not dropped: the budget stays at 1, so the first tick
        // after the function ends the turn. Refilled, a loop whose calls a
        // round divide `YIELD_EVERY` ended every turn inside its swap and
        // never gave one.
        if self.pure == 0 {
            self.turn_due = true;
        } else {
            self.budget = 1;
            super::stop::apply_in_swap(&mut self.world, self.shared);
        }
    }
}
