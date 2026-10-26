//! The M:1 scheduler (concurrency v2 R1b): one thread runs every task.
//!
//! A task between turns is a `Fiber`: its stack, frames, continuations,
//! cancellation node and waiter, all on the heap. The thread that called
//! the runtime (the command's) is the run thread for the whole run and
//! runs the task in front of the ready queue: it swaps that task's fiber
//! into its `Task` (`Task::exchange`), lets the run loop go (`drive`)
//! until the task is done, parks or gives its turn, and swaps it out
//! again. Nothing else touches fung values, so they stay `Rc`.
//!
//! **Waking.** A parked task registered its waiter on what it waits for;
//! whoever changes that thing calls `Waiter::wake`, which puts the task's
//! id on the ready queue (`Ready`) the first time since the task last
//! looked (`take_woken`). The queue sits behind a mutex, because other
//! threads wake tasks too (an offload's job, the standard input reader)
//! and the signal thread posts events (`Event`); a wake from another
//! thread rouses the event loop. When nothing is ready the run thread
//! sleeps in the event loop (`poll_io`), at most until the earliest timer.
//!
//! **No wake is lost.** A task takes its woken flag and registers its
//! waiter before it checks its condition (`park_until`). A change after
//! the check comes after the registration, so it wakes the task: the
//! flag is set and the id queued, whether the task has parked yet or not.
//! A queued id of a task that is not parked (it gave its turn, or the
//! wake was not for the wait it is in) only resumes it to look again.
//!
//! **Deadlock.** Every task is either running, in the ready queue, or
//! parked here, and every count below changes on this thread only. So
//! when the queue is empty and nothing runs, the program is stuck exactly
//! when no parked task is one a timer or the world outside will end
//! (`Wake::timed`: a sleep, a deadline, an offload's job, standard input,
//! the stop channel). Then `Task::judge` reports E0702 and cancels; the
//! cancellation wakes the parked tasks, which unwind. No atomic counts.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use super::poller::Waker;
use super::{Fiber, Stop, Task, Todo, Turn, Wake};
use crate::value::Value;

/// Rounds of parking tasks between looks at the event loop while tasks
/// wait on descriptors (`Sched::run`).
const POLL_EVERY: u32 = 256;

/// A task's number, unique in the runtime.
pub(crate) type FiberId = u64;

/// What other threads tell the run thread.
pub(crate) enum Event {
    /// A stop request for signal `sig` (0: `request_stop`), `stop.rs`.
    Stop(i32),
    /// The REPL's Ctrl-C: cancel the evaluation.
    Interrupt,
    /// A stop request once a task waits on the stop signal (tests).
    #[cfg(test)]
    StopWhenWatched(i32),
}

/// The ready queue and the events, shared with other threads. When the
/// run thread has nothing to do it sleeps in the event loop
/// (`Sched::sleep`); a push or a post then pulls the loop's waker.
pub(crate) struct Ready {
    state: Mutex<ReadyState>,
    waker: Waker,
}

#[derive(Default)]
struct ReadyState {
    ids: VecDeque<FiberId>,
    events: Vec<Event>,
    /// The run thread is about to wait, or waits, in the event loop.
    sleeping: bool,
}

impl Ready {
    fn state(&self) -> MutexGuard<'_, ReadyState> {
        // Plain containers that no panic leaves half-changed.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn new(waker: Waker) -> Ready {
        Ready {
            state: Mutex::default(),
            waker,
        }
    }

    /// Puts task `id` on the ready queue (`Waiter::wake`).
    pub(crate) fn push(&self, id: FiberId) {
        let mut s = self.state();
        s.ids.push_back(id);
        if s.sleeping {
            self.waker.wake();
        }
    }

    /// Posts an event for the run thread.
    pub(crate) fn post(&self, e: Event) {
        let mut s = self.state();
        s.events.push(e);
        if s.sleeping {
            self.waker.wake();
        }
    }

    /// Whether an event is posted or a task other than `me` queued (a
    /// task's own id may be queued twice, by a wake that came early).
    pub(crate) fn has_others(&self, me: FiberId) -> bool {
        let s = self.state();
        !s.events.is_empty() || s.ids.iter().any(|&id| id != me)
    }

    /// The events posted so far.
    pub(crate) fn take_events(&self) -> Vec<Event> {
        std::mem::take(&mut self.state().events)
    }

    /// Moves the queued ids to `into`.
    fn take_ids(&self, into: &mut VecDeque<FiberId>) {
        into.extend(self.state().ids.drain(..));
    }

    /// Whether the run thread may sleep: nothing queued or posted. If so
    /// it is marked sleeping, so that what comes next pulls the waker.
    fn may_sleep(&self) -> bool {
        let mut s = self.state();
        s.sleeping = s.ids.is_empty() && s.events.is_empty();
        s.sleeping
    }

    /// The run thread is awake again: moves the queued ids to `into`.
    fn woke(&self, into: &mut VecDeque<FiberId>) {
        let mut s = self.state();
        s.sleeping = false;
        into.extend(s.ids.drain(..));
    }

    /// Forgets the queued ids (a run starts: tasks of an earlier run are
    /// gone).
    pub(super) fn clear_ids(&self) {
        self.state().ids.clear();
    }
}

/// A task between turns.
struct Slot {
    fiber: Fiber,
    /// Parked since its `seq`-th park, a timer or the outside world ending
    /// it if `timed`; `None` while it waits in the ready queue.
    parked: Option<(u64, bool)>,
}

/// One run's tasks: the root (the entry, a test block, a module body)
/// and every task spawned under it.
pub(super) struct Sched {
    slots: HashMap<FiberId, Slot>,
    queue: VecDeque<FiberId>,
    /// Parked tasks' deadlines: (when, task, which park).
    timers: BinaryHeap<Reverse<(Instant, FiberId, u64)>>,
    seq: u64,
    /// Parked tasks, and those of them a timer or the outside ends.
    parked: usize,
    parked_timed: usize,
    root: FiberId,
    result: Option<Result<Value, Stop>>,
    /// The tokens a loop wait gave, until their tasks are woken.
    tokens: Vec<u64>,
    /// A task gave its turn since the loop was last looked at.
    poll_due: bool,
    /// Rounds since the loop was last looked at.
    since_poll: u32,
    /// Events that wait for a state (tests).
    #[cfg(test)]
    pending: Vec<Event>,
}

impl Sched {
    pub(super) fn new(root: Fiber) -> Sched {
        let id = root.id();
        let mut slots = HashMap::new();
        slots.insert(
            id,
            Slot {
                fiber: root,
                parked: None,
            },
        );
        Sched {
            slots,
            queue: VecDeque::from([id]),
            timers: BinaryHeap::new(),
            seq: 0,
            parked: 0,
            parked_timed: 0,
            root: id,
            result: None,
            tokens: Vec::new(),
            poll_due: false,
            since_poll: 0,
            #[cfg(test)]
            pending: Vec::new(),
        }
    }

    /// Runs until every task has ended; the root's result, and its fiber
    /// back.
    pub(super) fn run(mut self, t: &mut Task<'_>) -> (Result<Value, Stop>, Fiber) {
        let mut root = None;
        loop {
            self.events(t);
            if t.world.abandon {
                return self.abandon(t);
            }
            self.adopt(t);
            self.timers_due();
            // Woken tasks join the queue every round, not only once it is
            // empty: a computing task that keeps giving its turn would
            // otherwise hold back every task another thread or a channel
            // woke.
            t.shared.ready.take_ids(&mut self.queue);
            // Tasks wait on descriptors: look at the loop without waiting
            // when a task gave its turn (spec 15), and every
            // `POLL_EVERY` rounds of tasks that park and wake each other
            // (channel traffic), so that neither holds them back. Not
            // every round: a look is a system call.
            if !t.world.io.is_empty() && !self.queue.is_empty() {
                self.since_poll += 1;
                if self.poll_due || self.since_poll >= POLL_EVERY {
                    self.poll_due = false;
                    self.since_poll = 0;
                    self.poll_io(t, Some(std::time::Duration::ZERO));
                    self.wake_ready(t);
                    t.shared.ready.take_ids(&mut self.queue);
                }
            }
            if let Some(id) = self.queue.pop_front() {
                if let Some(f) = self.turn(t, id) {
                    root = Some(f);
                }
                continue;
            }
            t.shared.ready.take_ids(&mut self.queue);
            if !self.queue.is_empty() {
                continue;
            }
            if self.slots.is_empty() {
                let r = self.result.take().expect("the root ended");
                return (r, root.expect("the root's fiber"));
            }
            // Every task is parked.
            #[cfg(test)]
            t.shared
                .parked_probe
                .store(self.parked, std::sync::atomic::Ordering::SeqCst);
            #[cfg(test)]
            t.shared
                .timers_probe
                .store(self.timers.len(), std::sync::atomic::Ordering::SeqCst);
            // `parked_timed` counts every park a deadline ends, so with it
            // at zero any deadline left in `timers` is stale.
            if self.parked_timed == 0 && t.judge(|t| self.stuck(t)) {
                continue;
            }
            let until = self.timers.peek().map(|Reverse((at, _, _))| *at);
            self.sleep(t, until);
        }
    }

    /// Where every parked task waits, for the deadlock report: each
    /// fiber is put in `t` for a moment, so the report reads its frames
    /// as an error would. The order is the tasks' numbers, so the order
    /// they were spawned in.
    fn stuck(&mut self, t: &mut Task<'_>) -> super::deadlock::Stuck {
        let mut ids: Vec<FiberId> = self.slots.keys().copied().collect();
        ids.sort_unstable();
        let mut s = super::deadlock::Stuck::default();
        for id in ids {
            let slot = self.slots.get_mut(&id).expect("a parked task");
            t.exchange(&mut slot.fiber);
            if id == self.root {
                s.root = Some(t.stuck_root());
                s.root_place = Some(t.stuck_place());
            } else {
                s.others.push(t.stuck_place());
            }
            t.exchange(&mut slot.fiber);
        }
        s
    }

    /// Waits in the event loop until a descriptor is ready, another
    /// thread pushes or posts, or `until` passes. A push between the
    /// check and the wait is kept: it pulled the waker, which ends the
    /// wait at once.
    fn sleep(&mut self, t: &mut Task<'_>, until: Option<Instant>) {
        if t.shared.ready.may_sleep() {
            let timeout = until.map(|at| at.saturating_duration_since(Instant::now()));
            self.poll_io(t, timeout);
        }
        // Awake first: the wakes below then pull no waker.
        t.shared.ready.woke(&mut self.queue);
        self.wake_ready(t);
        t.shared.ready.take_ids(&mut self.queue);
    }

    /// Waits in the loop at most `timeout` for descriptors to be ready;
    /// their tokens wait in `tokens` for `wake_ready`.
    fn poll_io(&mut self, t: &mut Task<'_>, timeout: Option<std::time::Duration>) {
        #[cfg(test)]
        t.shared
            .io_polls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // A failed wait (it cannot fail on a valid loop) only ends this
        // sleep early.
        let _ = t.shared.poller.wait(&mut self.tokens, timeout);
    }

    /// Wakes the tasks whose descriptors the loop said are ready. A token
    /// no longer in the table is a wait that ended (`park_io`): its
    /// event is dropped.
    fn wake_ready(&mut self, t: &mut Task<'_>) {
        for tok in self.tokens.drain(..) {
            if let Some((w, _)) = t.world.take_io(tok) {
                w.wake();
            }
        }
    }

    /// The REPL's second Ctrl-C (`stop.rs`): every task of this run is
    /// dropped as it stands, without unwinding (its `release`s do not
    /// run). Their values drop here, on the run thread; their waits
    /// leave the queues they stood in as they drop (`Drop` of a channel's
    /// wait, a socket wait's turn, a command, which is killed). The
    /// event loop's registrations go too. The run ends cancelled.
    fn abandon(mut self, t: &mut Task<'_>) -> (Result<Value, Stop>, Fiber) {
        t.world.abandon = false;
        t.world.abandoned = true;
        self.queue.clear();
        self.timers.clear();
        t.world.new_fibers.clear();
        let slots = std::mem::take(&mut self.slots);
        drop(slots);
        let regs = t.world.take_all_io();
        for k in regs {
            match k {
                super::block::IoKey::Fd(fd, i) => t.shared.poller.deregister(fd, i),
                super::block::IoKey::Exit(pid) => t
                    .shared
                    .poller
                    .deregister_exit(&super::poller::ExitWatch::Pid(pid)),
            }
        }
        (
            Err(super::block::cancelled(super::no_pos())),
            Fiber::empty(t.shared),
        )
    }

    /// Applies the events other threads posted.
    fn events(&mut self, t: &mut Task<'_>) {
        #[cfg(test)]
        let mut events = std::mem::take(&mut self.pending);
        #[cfg(not(test))]
        let mut events = Vec::new();
        events.extend(t.shared.ready.take_events());
        for e in events {
            match e {
                #[cfg(test)]
                Event::StopWhenWatched(sig) if !super::stop::watched(&t.world) => {
                    self.pending.push(Event::StopWhenWatched(sig));
                }
                e => t.apply_event(e),
            }
        }
    }

    /// Takes in the tasks spawned during the last turn.
    fn adopt(&mut self, t: &mut Task<'_>) {
        for f in t.world.new_fibers.drain(..) {
            let id = f.id();
            self.slots.insert(
                id,
                Slot {
                    fiber: f,
                    parked: None,
                },
            );
            self.queue.push_back(id);
        }
    }

    /// Queues the parked tasks whose deadline has come.
    fn timers_due(&mut self) {
        let now = Instant::now();
        while let Some(Reverse((at, id, seq))) = self.timers.peek().copied() {
            if at > now {
                break;
            }
            self.timers.pop();
            if self
                .slots
                .get(&id)
                .is_some_and(|s| s.parked.is_some_and(|(n, _)| n == seq))
            {
                self.queue.push_back(id);
            }
        }
    }

    /// Gives task `id` a turn. The root's fiber comes back when it ends.
    fn turn(&mut self, t: &mut Task<'_>, id: FiberId) -> Option<Fiber> {
        let slot = self.slots.get_mut(&id)?;
        if let Some((_, timed)) = slot.parked.take() {
            self.parked -= 1;
            if timed {
                self.parked_timed -= 1;
            }
            #[cfg(test)]
            t.shared
                .wakeups
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        t.world.others_waiting = self.parked_timed > 0 || self.queue.iter().any(|&q| q != id);
        t.exchange(&mut slot.fiber);
        let turn = t.turn();
        t.exchange(&mut slot.fiber);
        match turn {
            Turn::Yielded => {
                self.poll_due = true;
                self.queue.push_back(id);
            }
            Turn::Parked(w) => self.park(id, w),
            Turn::Done(r) => {
                let slot = self.slots.remove(&id).expect("the task's slot");
                if id == self.root {
                    self.result = Some(r);
                    return Some(slot.fiber);
                }
                // A spawned task's end has done its work (`TaskEnd`); its
                // values drop here, on the run thread.
                drop(slot);
            }
        }
        None
    }

    /// Parks task `id` (its waiter already registered), or queues it at
    /// once if its wait is already over.
    fn park(&mut self, id: FiberId, w: Wake) {
        let slot = self.slots.get_mut(&id).expect("the task's slot");
        let f = &slot.fiber;
        // The cancellation node's deadline ends the wait too, as a
        // cancellation (`deadline.rs`).
        let over = f.cancel.deadline_at();
        let until = match (w.until, over) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        f.cancel.watch(&f.waiter);
        let now = Instant::now();
        if f.cancel.is_cancelled() || until.is_some_and(|at| now >= at) {
            self.queue.push_back(id);
            return;
        }
        let timed = w.timed || until.is_some();
        self.seq += 1;
        slot.parked = Some((self.seq, timed));
        self.parked += 1;
        if timed {
            self.parked_timed += 1;
        }
        if let Some(at) = until {
            self.timers.push(Reverse((at, id, self.seq)));
        }
        // A wait that ended early leaves its deadline behind: drop the
        // stale ones before they pile up.
        if self.timers.len() > 2 * self.parked_timed + 64 {
            let slots = &self.slots;
            self.timers.retain(|Reverse((_, id, seq))| {
                slots
                    .get(id)
                    .is_some_and(|s| s.parked.is_some_and(|(n, _)| n == *seq))
            });
        }
    }
}

impl Task<'_> {
    /// Runs this task's current state as the root of a run (the entry, a
    /// test block, a module body), from `start`, with every task it
    /// spawns, until all have ended; the root's result.
    ///
    /// A run inside a run would drop the outer run's queued wakes and take
    /// its running task's state, so it panics (the panic fence makes that
    /// E0801), in release builds too.
    pub(crate) fn run_root(&mut self, start: Todo) -> Result<Value, Stop> {
        assert!(!self.world.in_run, "run_root during a run");
        self.world.in_run = true;
        self.shared.ready.clear_ids();
        self.todo = Some(start);
        let mut root = Fiber::empty(self.shared);
        self.exchange(&mut root);
        let ran = catch_unwind(AssertUnwindSafe(|| Sched::new(root).run(self)));
        self.world.in_run = false;
        let (r, mut root) = ran.unwrap_or_else(|p| resume_unwind(p));
        self.exchange(&mut root);
        r
    }
}
