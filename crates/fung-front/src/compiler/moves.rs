//! Which reads of a function's locals are their last (03: 값의 수명, P9).
//! The last read hands the value over (MOVELOCAL): the slot is emptied, so
//! a callee that gets the only reference can change a map or list in place
//! (`Rc::make_mut`) instead of copying it.
//!
//! A backward walk in evaluation order carries the slots read after the
//! point it is at (liveness). A `(local i)` whose slot is not among them is
//! a last read. Writes are not followed: the set only grows, which keeps a
//! read a copy, never a move too early. The language has no loops (repetition
//! is recursion, and a `for` body is a function), so every path through a
//! frame's code is a walk forward with branches.

use std::collections::{BTreeSet, HashSet};

use super::{lit_index, local_reads};
use crate::tree::{Head, Node};

/// Slots read from some point on.
type Live = BTreeSet<usize>;

/// The last reads of locals in function body `body`.
pub(super) fn last_reads(body: &Node) -> HashSet<*const Node> {
    let mut moves = HashSet::new();
    walk(body, Live::new(), &mut moves);
    moves
}

/// Walks `n` back from `after` (the slots read once `n` is done) and
/// returns the slots read from where `n` starts.
fn walk(n: &Node, mut after: Live, moves: &mut HashSet<*const Node>) -> Live {
    match n.head {
        Head::Local => {
            if after.insert(lit_index(n)) {
                moves.insert(n as *const Node);
            }
            after
        }
        // A function value copies what it captures; its body is its own.
        Head::Fn => {
            local_reads(n, &mut after);
            after
        }
        // The target only writes.
        Head::Let => walk(&n.kids[1], after, moves),
        Head::Import => after,
        // Each branch goes on to what follows the `if`.
        Head::If => {
            let then = walk(&n.kids[1], after.clone(), moves);
            let mut live = walk(&n.kids[2], after, moves);
            live.extend(then);
            walk(&n.kids[0], live, moves)
        }
        // The right side may or may not run.
        Head::Coalesce => {
            let rhs = walk(&n.kids[1], after.clone(), moves);
            after.extend(rhs);
            walk(&n.kids[0], after, moves)
        }
        // The subject is loaded again for every arm and the guards run
        // before later arms: those reads copy. Only one body runs, and
        // what follows the match comes after it.
        Head::Match => {
            let mut live = after.clone();
            for arm in &n.kids[1..] {
                live.extend(walk(&arm.kids[2], after.clone(), moves));
                local_reads(&arm.kids[1], &mut live);
            }
            local_reads(&n.kids[0], &mut live);
            live
        }
        // Nothing follows a return on its path.
        Head::Return => walk(&n.kids[0], Live::new(), moves),
        _ => {
            for k in n.kids.iter().rev() {
                after = walk(k, after, moves);
            }
            after
        }
    }
}
