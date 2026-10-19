//! Stop requests (concurrency v2, L6; 05 "정지 요청"): "finish what you
//! are doing, then end", apart from a cancellation ("stop now").
//!
//! The runtime holds one stop channel. `stop_signal()` gives its
//! receiving end; the runtime keeps the sending end and only a `Weak`
//! to the receiving end. So whether anyone watches is whether that
//! `Weak` still upgrades: some copy of the receiving end is alive
//! (L4's counts, `channel.rs`).
//!
//! A stop request (the first SIGINT or SIGTERM, or `request_stop()`):
//! - someone watches: the runtime drops its sending end, so the channel
//!   closes and every watcher wakes; the program ends when it ends;
//! - nobody watches: like `exit`, the program's root is cancelled, with
//!   exit code 128 + the signal's number (0 for `request_stop()`).
//!
//! The stop channel is woken from outside the program (`Channel::
//! external`): a task waiting on it alone is not deadlocked.

use std::rc::{Rc, Weak};
use std::sync::Arc;

use super::channel::{self, Receiver, Sender};
use super::sched::Event;
use super::world::{Shared, World};
use super::{Stop, Task};
use crate::value::Value;

/// What the runtime knows of the stop request.
#[derive(Default)]
pub(crate) struct StopState {
    /// A stop was requested: `stop_signal()` gives a closed end now.
    requested: bool,
    /// The stop channel's sending end, while no stop was requested.
    tx: Option<Rc<Sender>>,
    /// The receiving end `stop_signal()` handed out, if any copy lives.
    rx: Weak<Receiver>,
}

/// `stop_signal()`: the receiving end of the stop channel, closed once a
/// stop was requested. Every call gives the same end while one lives.
pub(crate) fn stop_signal(t: &mut Task<'_>, _: Vec<Value>) -> Result<Value, Stop> {
    let s = &mut t.world.stop;
    if let Some(rx) = s.rx.upgrade() {
        return Ok(Value::Receiver(rx));
    }
    let (tx, rx) = channel::pair_external();
    let rx = Rc::new(rx);
    s.rx = Rc::downgrade(&rx);
    // A sending end left from watchers that are gone closes nothing
    // anyone reads; after a stop the new channel closes at once.
    let old = if s.requested {
        drop(tx);
        None
    } else {
        s.tx.replace(Rc::new(tx))
    };
    drop(old);
    Ok(Value::Receiver(rx))
}

/// Marks the stop requested and closes the stop channel; whether anyone
/// watched it.
fn request(w: &mut World, shared: &Shared) -> bool {
    w.stop.requested = true;
    // For the signal thread, which decides on a second signal without
    // the run thread (`StopHandle::deliver`).
    shared
        .stop_requested
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let watched = w.stop.rx.strong_count() > 0;
    let tx = w.stop.tx.take();
    drop(tx);
    watched
}

/// `request_stop()`: a stop request from the program, as the first
/// signal would make. With nobody watching it ends the program as
/// `exit(0)` does.
pub(crate) fn request_stop(t: &mut Task<'_>, _: Vec<Value>) -> Result<Value, Stop> {
    if request(&mut t.world, t.shared) {
        Ok(Value::Nil)
    } else {
        Err(Stop::Exit(0))
    }
}

/// Forgets a stop request (`fung test`: each test block starts afresh).
/// Without one nothing changes: a stop signal taken earlier (at the
/// file's top level) stays open.
pub(super) fn reset(w: &mut World, shared: &Shared) {
    if w.stop.requested {
        shared
            .stop_requested
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let old = std::mem::take(&mut w.stop);
        drop(old);
    }
}

/// Drops the stop state with the rest of the world's values.
pub(super) fn release(w: &mut World) {
    let old = std::mem::take(&mut w.stop);
    drop(old);
}

/// Delivers stop requests from outside the program (the command's
/// signal thread). It holds no fung value, so it may live on any thread.
#[derive(Clone)]
pub struct StopHandle(pub(super) Arc<Shared>);

impl StopHandle {
    /// The handle of the runtime `t` runs in (tests).
    #[cfg(test)]
    pub(crate) fn of(t: &Task<'_>) -> StopHandle {
        StopHandle(Arc::clone(t.shared))
    }

    /// A stop request for signal `sig` (0: as `request_stop()`): the
    /// watchers wake, or with none the program is cancelled with exit
    /// code 128 + `sig`. It is posted to the run thread (`sched.rs`), so it
    /// never waits for the run thread: the request is made at the next
    /// turn, or when the runtime is next used if nothing runs now.
    ///
    /// A signal after a stop was already requested (by `request_stop()`)
    /// is a second request: the process ends at once, as on a second
    /// signal.
    pub fn deliver(&self, sig: i32) {
        if sig != 0
            && self
                .0
                .stop_requested
                .load(std::sync::atomic::Ordering::SeqCst)
        {
            crate::signals::exit_now(sig);
        }
        self.0.ready.post(Event::Stop(sig));
    }

    /// Cancels what the REPL's input is running (Ctrl-C during an
    /// evaluation); `Runtime::recover_interrupt` then gives the session a
    /// fresh root.
    pub fn interrupt(&self) {
        self.0.ready.post(Event::Interrupt);
    }

    /// `deliver` once a task waits on the stop signal (tests: the order
    /// is decided by state, not by time).
    #[cfg(test)]
    pub(crate) fn deliver_when_watched(&self, sig: i32) {
        self.0.ready.post(Event::StopWhenWatched(sig));
    }
}

/// Whether a task waits on the stop signal (`Event::StopWhenWatched`).
#[cfg(test)]
pub(super) fn watched(w: &World) -> bool {
    w.stop.rx.upgrade().is_some_and(|rx| rx.0.queued() > 0)
}

/// Makes what an event asks for, on the run thread.
pub(super) fn apply(w: &mut World, shared: &Shared, e: Event) {
    match e {
        Event::Stop(sig) => stop(w, shared, sig),
        #[cfg(test)]
        Event::StopWhenWatched(sig) => stop(w, shared, sig),
        Event::Interrupt => {
            // Again while the input is cut short already (stuck in a
            // `release`): its tasks are dropped (`Sched::abandon`).
            if w.interrupted && w.in_run {
                w.abandon = true;
            }
            w.interrupted = true;
            // The root of the input running now, not a fresh one the REPL
            // puts in place after (`Runtime::recover_interrupt`).
            shared.root().cancel();
        }
    }
}

/// The stop request of signal `sig` (see `StopHandle::deliver`).
fn stop(w: &mut World, shared: &Shared, sig: i32) {
    if sig != 0 && w.stop.requested {
        crate::signals::exit_now(sig);
    }
    if !request(w, shared) {
        let code = if sig == 0 { 0 } else { 128 + i64::from(sig) };
        w.request_exit(code);
        shared.root().cancel();
    }
}

/// Applies the events posted while nothing ran (`Runtime`'s calls).
pub(super) fn apply_posted(w: &mut World, shared: &Shared) {
    for e in shared.ready.take_events() {
        apply(w, shared, e);
    }
}

impl Task<'_> {
    /// Makes what an event asks for (`sched.rs`).
    pub(super) fn apply_event(&mut self, e: Event) {
        apply(&mut self.world, self.shared, e);
    }
}
