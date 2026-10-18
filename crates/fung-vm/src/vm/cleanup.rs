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

use super::task::{CancelNode, is_cancel};
use super::{Stop, Task};
use crate::value::Value;

/// `with(acquire, use, release)`: see the module doc.
pub(crate) fn run_with(
    t: &mut Task<'_>,
    acquire: Value,
    use_: Value,
    release: Value,
) -> Result<Value, Stop> {
    let res = t.call(&acquire, Vec::new())?;
    let r_use = t.call(&use_, vec![res.clone()]);
    let shield = CancelNode::root();
    t.world.shields.push(Arc::downgrade(&shield));
    let outer = std::mem::replace(&mut t.cancel, shield.clone());
    let r_rel = t.call(&release, vec![res]);
    t.cancel = outer;
    t.world
        .shields
        .retain(|w| !std::ptr::eq(w.as_ptr(), Arc::as_ptr(&shield)));
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
        (_, r_rel) if failed(&r_rel) => r_rel,
        (_, Err(stop)) => Err(stop),
        (r_use, Ok(_)) => r_use,
    }
}
