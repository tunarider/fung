//! Pattern bindings: destructuring assignments and match arms.

use std::collections::HashSet;

use super::scope::{Binding, Made};
use super::{Resolver, lit_str, name_error, ref_node, rune_len};
use crate::diag::{Diag, error};
use crate::source::Pos;
use crate::tree::{Head, Lit, Node};

/// How a pattern binds its names.
#[derive(Clone, Copy)]
pub(super) enum Mode {
    /// Like assignment (`bind_new`).
    Assign,
    /// Always a new name in the current block: pattern names shadow outer
    /// ones (04).
    Fresh,
}

fn binds(n: &Node) -> bool {
    matches!(n.head, Head::Pbind | Head::Prest) && matches!(n.lit, Lit::Str(_))
}

/// The names a pattern binds, sorted (`Node::bound_names`).
fn pattern_names(n: &Node) -> Vec<String> {
    let mut names = Vec::new();
    n.bound_names(&mut |name, _| names.push(name.to_string()));
    names.sort();
    names
}

impl Resolver<'_> {
    /// Always binds a new name in the current block (or reuses the block's
    /// own non-constant binding of that name).
    fn bind_fresh(&mut self, name: &str, pos: Pos) -> Option<Binding> {
        if !self.check_shadow(name, pos) {
            return None;
        }
        if let Some(b) = self.cur_block().names.get(name)
            && !b.constant
        {
            return Some(*b);
        }
        self.warn_shadow(name, pos);
        Some(self.bind_here(name, pos))
    }

    /// Rewrites the names of a pattern into `(local n)`/`(global n)`
    /// references. Alternatives must bind the same names (03, E0216); the
    /// first alternative binds and the others reuse its slots. A name bound
    /// twice in one pattern is E0221 (03).
    pub(super) fn bind_pattern(&mut self, n: &Node, mode: Mode) -> Node {
        self.bind_pat(n, mode, &mut HashSet::new())
    }

    fn bind_pat(&mut self, n: &Node, mode: Mode, seen: &mut HashSet<String>) -> Node {
        match n.head {
            Head::Pbind | Head::Prest => {
                if !binds(n) {
                    return n.clone();
                }
                let name = lit_str(n);
                if !seen.insert(name.to_string()) {
                    self.errs.push(name_error("E0221", n.pos, name));
                    return n.clone();
                }
                let b = match mode {
                    Mode::Assign => self.bind_new(name, n.pos),
                    Mode::Fresh => self.bind_fresh(name, n.pos),
                };
                match b {
                    Some(b) => Node::new(n.head, n.pos, vec![ref_node(n.pos, b)]),
                    None => n.clone(),
                }
            }
            Head::Palt => {
                let first = pattern_names(&n.kids[0]);
                if let Some(alt) = n.kids[1..].iter().find(|a| pattern_names(a) != first) {
                    self.errs.push(error("E0216", alt.pos, vec![]));
                    // Bind every alternative's names anyway, so their uses
                    // are not reported again as undefined (08).
                    for alt in &n.kids {
                        for name in pattern_names(alt) {
                            if seen.insert(name.clone()) {
                                match mode {
                                    Mode::Assign => self.bind_new(&name, alt.pos),
                                    Mode::Fresh => self.bind_fresh(&name, alt.pos),
                                };
                            }
                        }
                    }
                    return n.clone();
                }
                // Each alternative binds the same names once; start every
                // alternative from the names seen before the `|`.
                let kids = n
                    .kids
                    .iter()
                    .map(|alt| self.bind_pat(alt, mode, &mut seen.clone()))
                    .collect();
                seen.extend(first);
                Node::new(Head::Palt, n.pos, kids)
            }
            _ => {
                let kids = n
                    .kids
                    .iter()
                    .map(|k| self.bind_pat(k, mode, seen))
                    .collect();
                Node::with_lit(n.head, n.pos, n.lit.clone(), kids)
            }
        }
    }

    /// Resolves `(match subject (arm pat guard body)…)`: each arm is its
    /// own block scope whose pattern names are fresh bindings. The body's
    /// statements share that block, so the arm may rebind its names like a
    /// `for` body (03). An arm after one that matches everything without
    /// a guard never runs: E0235 (FE25).
    pub(super) fn match_expr(&mut self, n: &Node) -> Node {
        let mut kids = vec![self.walk(&n.kids[0])];
        let arms = &n.kids[1..];
        if let Some(i) = arms[..arms.len().saturating_sub(1)]
            .iter()
            .position(|a| a.kids[1].head == Head::Nil && matches_all(&a.kids[0]))
        {
            let prev = i.checked_sub(1).map(|j| &arms[j].kids[0]);
            self.errs
                .push(self.unreachable_after(&arms[i].kids[0], prev));
        }
        for arm in arms {
            let body = &arm.kids[2];
            self.push_block(body, false);
            self.cur_block().made = Made::Arm;
            let pat = self.bind_pattern(&arm.kids[0], Mode::Fresh);
            let guard = self.walk(&arm.kids[1]);
            let body = if body.head == Head::Do {
                let stmts = body.kids.iter().map(|k| self.stmt(k)).collect();
                let mut d = Node::new(Head::Do, body.pos, stmts);
                d.off = body.off;
                d.end = body.end;
                d
            } else {
                self.walk(body)
            };
            self.pop_block(false);
            kids.push(Node::new(Head::Arm, arm.pos, vec![pat, guard, body]));
        }
        Node::new(Head::Match, n.pos, kids)
    }

    /// E0235 at the arm pattern that matches everything. Indented under
    /// the arm before (`prev`), it was most likely a lambda statement in
    /// that arm's body; a bare name that is also a visible variable was
    /// most likely meant to compare with it.
    fn unreachable_after(&self, pat: &Node, prev: Option<&Node>) -> Diag {
        let name = lit_str(pat);
        let e = error("E0235", pat.pos, vec![]).span(rune_len(name).max(1));
        if prev.is_some_and(|p| pat.pos.line > p.pos.line && pat.pos.col > p.pos.col) {
            return e.with_help(
                "a line of the form `pattern =>` starts a new arm; a lambda as the value of the arm above goes in parentheses: `(y => y + 1)`",
            );
        }
        if pat.head != Head::Pbind {
            return e;
        }
        let visible = self.fns.iter().any(|f| f.innermost(name).is_some())
            || self.file_var_pos.contains_key(name);
        if !visible {
            return e;
        }
        e.with_help(&format!(
            "a name in a pattern binds a new name, so `{name}` here is not the variable; compare with a guard: `v when v == {name} =>`"
        ))
    }
}

/// A pattern that every value matches: `_`, a bare name, or alternatives
/// one of which is such.
fn matches_all(p: &Node) -> bool {
    match p.head {
        Head::Pwild | Head::Pbind => true,
        Head::Palt => p.kids.iter().any(matches_all),
        _ => false,
    }
}
