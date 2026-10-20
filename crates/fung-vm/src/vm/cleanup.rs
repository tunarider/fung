//! Cleanup (concurrency v2, L3; 03 "자원 정리"): `with(acquire, use,
//! release)`.
//!
//! `with` calls `acquire()`, then `use(r)` with what it gave, then
//! `release(r)` whatever `use` did: returned a value or an `Err`, died of
//! a runtime error, was cancelled (a `cancel`, a `timeout`, a failing
//! sibling) or called `exit`. A runtime error or a cancellation of
//! `acquire` itself skips `release` (nothing was acquired). Nothing can
//! slip between `acquire` returning and `release` being due: whatever
//! stops `use`, even before its first call, comes back here.
//!
//! **The shield.** `release` runs under a node of its own, a fresh root:
//! the cancellation (or the deadline) that stopped `use` does not reach
//! it, so a release that closes a socket or returns a connection to a
//! pool is not cut short. The node is listed in `World::shields` while it
//! runs, so the deadlock check judges it even after an exit and cancels
//! it if it is stuck (`deadlock.rs`).
//!
//! **What goes up**, by class first, then by source:
//! 1. `use`'s runtime error (with the release's own runtime error, if it
//!    had one, as a note);
//! 2. otherwise the release's runtime error;
//! 3. otherwise the release's own stop: an `exit` it called, or the
//!    deadlock check cutting it short (only that cancels a shield);
//! 4. otherwise `use`'s result (a value, an `Err`, a cancellation, an
//!    exit).
//!
//! So a release's failure is never dropped, nor is an exit or a deadlock
//! inside it; the exit code itself is kept in the world (the first exit
//! requested wins), so it never has to travel in the error.

use std::sync::Arc;

use fung_front::diag::headline;

use super::cont::{Cont, Flow, Next};
use super::task::{CancelNode, is_cancel};
use super::{Stop, Task, discarded};
use crate::value::Value;

/// `with(acquire, use, release)`: see the module doc. The three calls are
/// made by the run loop (`cont.rs`); this is where `with` is between them.
pub(crate) fn run_with(acquire: Value, use_: Value, release: Value) -> Flow {
    let k = With {
        use_,
        release,
        phase: Phase::Acquire,
    };
    Flow::Call(Box::new(k), acquire, 0)
}

struct With {
    use_: Value,
    release: Value,
    phase: Phase,
}

enum Phase {
    Acquire,
    /// `use` runs with what `acquire` gave.
    Use(Value),
    /// `release` runs under `shield`, with what `use` came to; the task's
    /// own node waits in `outer`.
    Release {
        r_use: Result<Value, Stop>,
        shield: Arc<CancelNode>,
        outer: Arc<CancelNode>,
    },
    Ended,
}

impl With {
    /// `use` came to `r_use`: `release` is due, under a shield node.
    fn release(&mut self, t: &mut Task<'_>, res: Value, r_use: Result<Value, Stop>) -> Next {
        let shield = CancelNode::root();
        t.world
            .shields
            .insert(shield_key(&shield), Arc::downgrade(&shield));
        let outer = std::mem::replace(&mut t.cancel, shield.clone());
        self.phase = Phase::Release {
            r_use,
            shield,
            outer,
        };
        t.push_arg(res);
        Next::Call(self.release.clone(), 1)
    }

    /// `release` came to `r_rel`: back under the task's node, and what goes
    /// up (the module doc's order).
    fn released(&mut self, t: &mut Task<'_>, r_rel: Result<Value, Stop>) -> Result<Next, Stop> {
        let Phase::Release {
            r_use,
            shield,
            outer,
        } = std::mem::replace(&mut self.phase, Phase::Ended)
        else {
            unreachable!("released outside the release phase")
        };
        t.cancel = outer;
        t.world.shields.remove(&shield_key(&shield));
        let r_rel = discarded(r_rel);
        let failed = |r: &Result<Value, Stop>| matches!(r, Err(Stop::Error(d)) if !is_cancel(d));
        match (r_use, r_rel) {
            (Err(Stop::Error(mut u)), r_rel) if !is_cancel(&u) => {
                if let Err(Stop::Error(rel)) = &r_rel
                    && !is_cancel(rel)
                {
                    u.notes
                        .push(format!("note: also failed in release: {}", headline(rel)));
                }
                Err(Stop::Error(u))
            }
            (_, r_rel) if failed(&r_rel) => r_rel.map(Next::Done),
            (_, Err(stop)) => Err(stop),
            (r_use, Ok(_)) => r_use.map(Next::Done),
        }
    }
}

/// A shield's key in `World::shields`: its address, unique while the
/// release that holds it runs. An entry left by a release that never
/// ended (its task was dropped) is dead, and a new node at its address
/// takes its place.
fn shield_key(shield: &Arc<CancelNode>) -> usize {
    Arc::as_ptr(shield) as usize
}

impl Cont for With {
    fn resume(&mut self, t: &mut Task<'_>, v: Value) -> Result<Next, Stop> {
        match std::mem::replace(&mut self.phase, Phase::Ended) {
            Phase::Acquire => {
                self.phase = Phase::Use(v.clone());
                t.push_arg(v);
                Ok(Next::Call(self.use_.clone(), 1))
            }
            Phase::Use(res) => Ok(self.release(t, res, Ok(v))),
            phase @ Phase::Release { .. } => {
                self.phase = phase;
                self.released(t, Ok(v))
            }
            Phase::Ended => unreachable!("a with resumed after it ended"),
        }
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        match std::mem::replace(&mut self.phase, Phase::Ended) {
            // Nothing was acquired: nothing to release.
            Phase::Acquire => Err(stop),
            Phase::Use(res) => Ok(self.release(t, res, Err(stop))),
            phase @ Phase::Release { .. } => {
                self.phase = phase;
                self.released(t, Err(stop))
            }
            Phase::Ended => Err(stop),
        }
    }

    fn releasing(&self) -> bool {
        matches!(self.phase, Phase::Release { .. })
    }
}
