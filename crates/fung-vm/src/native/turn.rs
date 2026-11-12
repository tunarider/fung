//! Turns: one task at a time on one side of a shared descriptor (a
//! connection's read or write side, `net.rs`; a FIFO handle's, `fs.rs`
//! and `prelude/io.rs`).
//!
//! The event loop keeps one wait per descriptor and interest (`World::add_io`):
//! a second registration would replace the first, and the first task
//! would never wake. So an operation on a shared descriptor first takes
//! that side's turn, holds it for its whole run (the waits in it
//! included) and gives it up at its end. The others wait in line, parked
//! with no registration of their own (`IoStep::Park`), first come first
//! served.

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::vm::{FiberId, Waiter};

/// One task at a time on one side of a descriptor, for a whole operation
/// and the waits in it. The others park in line, first come first served:
/// a released turn goes to the first in line (L14), so a task that starts
/// its next operation at once cannot keep it while others wait. A
/// cancellation or deadline ends a wait in line too; a task the turn was
/// handed to that leaves without taking it hands it on.
#[derive(Default)]
pub(crate) struct Turnstile {
    state: Mutex<TurnState>,
    /// Counts the tasks in line, with those of the other turnstiles that
    /// share it (a test probe: the connections' lines).
    counter: Option<&'static AtomicUsize>,
}

#[derive(Default)]
struct TurnState {
    /// Held, or handed to `handed` and not yet taken.
    busy: bool,
    /// The task the turn was handed to, which has not taken it yet.
    handed: Option<FiberId>,
    /// The tasks waiting, oldest first.
    line: std::collections::VecDeque<Arc<Waiter>>,
}

fn lock(m: &Mutex<TurnState>) -> MutexGuard<'_, TurnState> {
    // The state is plain data that no panic leaves half-changed.
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Turnstile {
    /// A turnstile whose line `counter` counts too.
    pub(crate) fn counted(counter: &'static AtomicUsize) -> Turnstile {
        Turnstile {
            state: Mutex::default(),
            counter: Some(counter),
        }
    }

    fn count(&self, up: bool, n: usize) {
        if let Some(c) = self.counter {
            if up {
                c.fetch_add(n, Ordering::SeqCst);
            } else {
                c.fetch_sub(n, Ordering::SeqCst);
            }
        }
    }

    /// How many tasks wait in line now (tests).
    #[cfg(test)]
    pub(crate) fn in_line(&self) -> usize {
        lock(&self.state).line.len()
    }

    pub(crate) fn is_busy(&self) -> bool {
        lock(&self.state).busy
    }

    /// Takes the turn (free, or handed to `me`): true. Otherwise lines
    /// `me` up (once) and false: the caller parks until woken.
    pub(crate) fn take(&self, me: &Arc<Waiter>) -> bool {
        let mut s = lock(&self.state);
        let id = me.id();
        if !s.busy {
            s.busy = true;
            return true;
        }
        if s.handed == Some(id) {
            s.handed = None;
            return true;
        }
        if !s.line.iter().any(|w| w.id() == id) {
            s.line.push_back(me.clone());
            self.count(true, 1);
        }
        false
    }

    /// Gives the turn up: to the first in line, if any.
    pub(crate) fn release(&self) {
        let mut s = lock(&self.state);
        self.hand_on(&mut s);
    }

    /// Task `me` stops waiting without taking the turn (its wait ended
    /// some other way): it leaves the line, and a turn handed to it goes
    /// on to the next.
    pub(crate) fn leave(&self, me: FiberId) {
        let mut s = lock(&self.state);
        let before = s.line.len();
        s.line.retain(|w| w.id() != me);
        self.count(false, before - s.line.len());
        if s.handed == Some(me) {
            self.hand_on(&mut s);
        }
    }

    fn hand_on(&self, s: &mut TurnState) {
        match s.line.pop_front() {
            Some(w) => {
                self.count(false, 1);
                s.busy = true;
                s.handed = Some(w.id());
                w.wake();
            }
            None => {
                s.busy = false;
                s.handed = None;
            }
        }
    }
}

/// What one operation holds of a turnstile between its tries: the turn,
/// or its place in line. `end` gives back whichever it has, once.
#[derive(Default)]
pub(crate) struct Hold {
    held: Cell<bool>,
    /// Waiting in the line, as this task.
    in_line: Cell<Option<FiberId>>,
}

impl Hold {
    /// Whether the operation holds the turn now, taking it if it can;
    /// false: it waits in line (park until woken).
    pub(crate) fn take(&self, ts: &Turnstile, me: &Arc<Waiter>) -> bool {
        if self.held.get() {
            return true;
        }
        if !ts.take(me) {
            self.in_line.set(Some(me.id()));
            return false;
        }
        self.in_line.set(None);
        self.held.set(true);
        true
    }

    /// The operation ended (done, stopped or dropped): the turn goes to
    /// the next in line, or this task leaves the line.
    pub(crate) fn end(&self, ts: &Turnstile) {
        if self.held.replace(false) {
            ts.release();
        } else if let Some(me) = self.in_line.take() {
            ts.leave(me);
        }
    }
}
