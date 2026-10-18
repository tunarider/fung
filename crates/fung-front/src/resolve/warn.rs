//! Warnings (08, W01xx), collected alongside errors and shown only by
//! `fung check` and `fung test`.

use super::scope::Scope;
use super::{Resolver, lit_str, name_error};
use crate::diag::Diag;
use crate::source::Pos;
use crate::tree::{Head, Node};

impl Resolver<'_> {
    /// Reports a new name that hides a prelude function (W0103). The rule
    /// is the same however a file is loaded; the loader hides the embedded
    /// stdlib's own warnings.
    pub(super) fn warn_shadow(&mut self, name: &str, pos: Pos) {
        if self.prelude.contains(name) {
            self.warnings.push(name_error("W0103", pos, name));
        }
    }

    /// Reports the block-local names that were never read (W0102): not
    /// parameters, not `_`-prefixed, not hidden desugar names. Every way
    /// of binding a name records its position. The warnings are sorted
    /// by position and then by name (08).
    pub(super) fn warn_unused(&mut self, sc: &Scope) {
        let mut unused: Vec<(&String, Pos)> = sc
            .names
            .keys()
            .filter(|n| {
                !sc.used.contains(*n)
                    && !sc.param.contains(*n)
                    && !n.starts_with('_')
                    && !n.starts_with('$')
            })
            .map(|n| (n, sc.pos[n]))
            .collect();
        unused.sort_by(|a, b| (a.1.line, a.1.col, a.0).cmp(&(b.1.line, b.1.col, b.0)));
        for (name, pos) in unused {
            self.warnings.push(name_error("W0102", pos, name));
        }
    }

    /// Reports import-bound names never read in the file (W0101), in the
    /// order of the globals.
    pub(super) fn warn_unused_imports(&mut self) {
        let sc = &self.fns[0].blocks[0];
        let found: Vec<Diag> = self
            .globals
            .iter()
            .filter(|g| self.import_bound.contains(*g) && !sc.used.contains(*g))
            .map(|g| name_error("W0101", sc.pos[g], g))
            .collect();
        self.warnings.extend(found);
    }

    /// Reports a block in a value position (the right side of an
    /// assignment, a call argument, a return) whose last statement is an
    /// assignment: its value is nil, which is almost never what was meant
    /// (W0104, 08). Only if/match/do blocks are inspected.
    pub(super) fn warn_block_value(&mut self, n: &Node) {
        if !matches!(n.head, Head::If | Head::Match | Head::Do) {
            return;
        }
        if let Some(a) = last_assign(n) {
            let t = &a.kids[0];
            let name = if t.head == Head::Name {
                lit_str(t)
            } else {
                "this pattern"
            };
            self.warnings.push(name_error("W0104", a.pos, name));
        }
    }
}

/// Orders warnings by position (stable), so output does not depend on
/// the order they were found in.
pub(super) fn sort_warnings(ws: &mut [Diag]) {
    ws.sort_by_key(|w| (w.pos.line, w.pos.col));
}

/// An assignment that would be the last statement of `n` when `n` is
/// evaluated as a value: the end of a do, of any if branch, or of any
/// match arm.
fn last_assign(n: &Node) -> Option<&Node> {
    match n.head {
        Head::Let => Some(n),
        Head::Do => n.kids.last().and_then(last_assign),
        Head::If => n.kids[1..].iter().find_map(last_assign),
        Head::Match => n.kids[1..]
            .iter()
            .find_map(|arm| arm.kids.last().and_then(last_assign)),
        _ => None,
    }
}
