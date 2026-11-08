//! Which reads of a function's locals are their last (03: 값의 수명, P9).
//! The last read hands the value over (MOVELOCAL): the slot is emptied, so
//! a callee that gets the only reference can change a map or list in place
//! (`Rc::make_mut`) instead of copying it.
//!
//! A backward walk in evaluation order carries the slots read after the
//! point it is at (liveness). A `(local i)` whose slot is not among them is
//! a last read. An assignment, and a match arm's pattern, writes its slots:
//! what was read there before is not read after it, so it leaves the set
//! (a slot is reused by the names of sibling blocks, and the write tells
//! them apart). A write on one branch only leaves the other branches'
//! reads in. The language has no loops (repetition
//! is recursion, and a `for` body is a function), so every path through a
//! frame's code is a walk forward with branches.
//!
//! The same walk records, for every statement of every block it passes
//! (all but a block's last expression), the slots read after it: once that
//! statement is done, a local read so far and not among them is let go
//! (CLEARLOCAL, 03: 값의 수명). A block the walk does not enter (a match
//! subject or guard, read with `local_reads`) has no entry and lets go of
//! nothing.

use std::collections::{BTreeSet, HashMap, HashSet};

use super::{lit_index, local_reads};
use crate::tree::{Head, Node};

/// Slots read from some point on.
pub(super) type Live = BTreeSet<usize>;

/// What the walk learns about one frame's code.
#[derive(Default)]
pub(super) struct Lives {
    /// The reads of locals that are their last: they compile to MOVELOCAL.
    pub moves: HashSet<*const Node>,
    /// For each statement of a block, the slots read after it.
    pub after: HashMap<*const Node, Live>,
    /// For each assignment target and match arm pattern, the slots read
    /// after it stores: a value stored where its slot is not among them is
    /// never read, so it is kept until the frame returns.
    pub stored: HashMap<*const Node, Live>,
    /// The slots read from the start: the parameters that are read.
    pub entry: Live,
}

/// The last reads of locals in `body`, a function's body or a program's,
/// and the slots read after each statement and each store in it.
pub(super) fn last_reads(body: &Node) -> Lives {
    let mut lives = Lives::default();
    lives.entry = walk(body, Live::new(), &mut lives);
    lives
}

/// Walks `n` back from `after` (the slots read once `n` is done) and
/// returns the slots read from where `n` starts.
fn walk(n: &Node, mut after: Live, lives: &mut Lives) -> Live {
    match n.head {
        Head::Local => {
            if after.insert(lit_index(n)) {
                lives.moves.insert(n as *const Node);
            }
            after
        }
        // A function value copies what it captures; its body is its own.
        Head::Fn => {
            local_reads(n, &mut after);
            after
        }
        // The value is computed, then stored: the target's slots are not
        // read again before the store.
        Head::Let => {
            lives
                .stored
                .insert(&n.kids[0] as *const Node, after.clone());
            written(&n.kids[0], &mut after);
            walk(&n.kids[1], after, lives)
        }
        Head::Import => after,
        // Each branch goes on to what follows the `if`.
        Head::If => {
            let then = walk(&n.kids[1], after.clone(), lives);
            let mut live = walk(&n.kids[2], after, lives);
            live.extend(then);
            walk(&n.kids[0], live, lives)
        }
        // The right side may or may not run.
        Head::Coalesce => {
            let rhs = walk(&n.kids[1], after.clone(), lives);
            after.extend(rhs);
            walk(&n.kids[0], after, lives)
        }
        // The subject is loaded again for every arm and the guards run
        // before later arms: those reads copy. Only one body runs, and
        // what follows the match comes after it.
        // An arm's pattern binds its slots before its guard and body. No
        // arm matching is an error, so nothing follows that path.
        Head::Match => {
            let mut live = Live::new();
            for arm in &n.kids[1..] {
                let mut here = walk(&arm.kids[2], after.clone(), lives);
                local_reads(&arm.kids[1], &mut here);
                lives
                    .stored
                    .insert(&arm.kids[0] as *const Node, here.clone());
                written(&arm.kids[0], &mut here);
                live.extend(here);
            }
            local_reads(&n.kids[0], &mut live);
            live
        }
        // A block's statements: what follows each one is recorded.
        Head::Do => {
            let last = n.kids.len().saturating_sub(1);
            for (i, k) in n.kids.iter().enumerate().rev() {
                if i < last {
                    lives.after.insert(k as *const Node, after.clone());
                }
                after = walk(k, after, lives);
            }
            after
        }
        // Nothing follows a return on its path.
        Head::Return => walk(&n.kids[0], Live::new(), lives),
        _ => {
            for k in n.kids.iter().rev() {
                after = walk(k, after, lives);
            }
            after
        }
    }
}

/// Takes out of `live` the slots that target or pattern `n` writes: its
/// `(local i)` leaves (a pattern holds no expressions).
fn written(n: &Node, live: &mut Live) {
    let mut slots = Live::new();
    written_slots(n, &mut slots);
    for s in slots {
        live.remove(&s);
    }
}

/// Adds to `out` the slots that target or pattern `n` writes.
pub(super) fn written_slots(n: &Node, out: &mut Live) {
    if n.head == Head::Local {
        out.insert(lit_index(n));
    }
    for k in &n.kids {
        written_slots(k, out);
    }
}
