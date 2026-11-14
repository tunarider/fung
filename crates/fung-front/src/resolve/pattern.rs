//! Pattern bindings: destructuring assignments and match arms.

use std::collections::HashSet;

use super::scope::{Binding, Made};
use super::{Resolver, lit_str, name_error, ref_node, rune_len};
use crate::diag::{Arg, Diag, error};
use crate::source::Pos;
use crate::tree::{Head, Lit, Node};

/// How a pattern binds its names.
#[derive(Clone, Copy)]
pub(super) enum Mode {
    /// Like assignment (`bind_new`).
    Assign,
    /// Always a new name in the current block (a `match` arm, a `for`
    /// or parameter pattern); hiding an outer name is E0238 (D38, D122).
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
        if let Some(b) = self.cur_block().names.get(name)
            && !b.constant
            && !b.module
        {
            return Some(*b);
        }
        if !self.binds_fresh(name, pos, false, false) {
            return None;
        }
        Some(self.bind_here(name, pos))
    }

    /// Rewrites the names of a pattern into `(local n)`/`(global n)`
    /// references. Alternatives must bind the same names (03, E0216); the
    /// first alternative binds and the others reuse its slots. A name bound
    /// twice in one pattern is E0221 (03).
    pub(super) fn bind_pattern(&mut self, n: &Node, mode: Mode) -> Node {
        let outer = std::mem::take(&mut self.pattern_bound);
        let out = self.bind_pat(n, mode, &mut HashSet::new());
        self.pattern_bound = outer;
        out
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
                // A later alternative binds the name an earlier one did:
                // the same binding, not a second one (D53 at the top level).
                let again = self.pattern_bound.contains(name);
                let b = match mode {
                    Mode::Assign if again => self.cur_block().names.get(name).copied(),
                    Mode::Assign => self.bind_new(name, n.pos, true),
                    Mode::Fresh => self.bind_fresh(name, n.pos),
                };
                self.pattern_bound.insert(name.to_string());
                match b {
                    Some(b) => Node::new(n.head, n.pos, vec![ref_node(n.pos, b)]),
                    None => n.clone(),
                }
            }
            Head::Palt => {
                self.dead_alternatives(n);
                let first = pattern_names(&n.kids[0]);
                if let Some(alt) = n.kids[1..].iter().find(|a| pattern_names(a) != first) {
                    self.errs.push(error("E0216", alt.pos, vec![]));
                    // Bind every alternative's names anyway, so their uses
                    // are not reported again as undefined (08).
                    for alt in &n.kids {
                        for name in pattern_names(alt) {
                            if seen.insert(name.clone()) {
                                match mode {
                                    Mode::Assign => self.bind_new(&name, alt.pos, true),
                                    Mode::Fresh => self.bind_fresh(&name, alt.pos),
                                };
                            }
                        }
                    }
                    return n.clone();
                }
                // Each alternative binds the same names once; start every
                // alternative from the names seen before the `|`, by
                // taking out what the one before added (FE75).
                let added: Vec<&String> = first.iter().filter(|n| !seen.contains(*n)).collect();
                let mut kids = Vec::with_capacity(n.kids.len());
                for alt in &n.kids {
                    kids.push(self.bind_pat(alt, mode, seen));
                    for name in &added {
                        seen.remove(*name);
                    }
                }
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
            let prev = i.checked_sub(1).map(|j| &arms[j]);
            self.errs
                .push(self.unreachable_after(&arms[i].kids[0], prev));
        }
        self.covered_arms(arms);
        for arm in arms {
            if arm.kids[0].head == Head::Pbind {
                let at = arm.kids[0].pos;
                self.catch_all.insert((at.line, at.col));
            }
            let body = &arm.kids[2];
            self.push_block(body, false);
            self.cur_block().made = Made::Arm;
            self.guarded = arm.kids[1].head != Head::Nil;
            let pat = self.bind_pattern(&arm.kids[0], Mode::Fresh);
            self.guarded = false;
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
        let what = "matches everything, so the arms after it never run";
        let e = error("E0235", pat.pos, vec![Arg::Str(what.into())]).span(rune_len(name).max(1));
        let prev_pat = prev.map(|a| &a.kids[0]);
        if prev_pat.is_some_and(|p| pat.pos.line > p.pos.line && pat.pos.col > p.pos.col) {
            return e.with_help(
                "a line of the form `pattern =>` starts a new arm; a lambda as the value of the arm above goes in parentheses: `(y => y + 1)`",
            );
        }
        // `1 => y => y + 1`: `y =>` was read as the next arm, so the arm
        // before it is empty (FE25, D121).
        if pat.head == Head::Pbind
            && prev.is_some_and(|a| a.kids[2].kids.is_empty() && a.kids[0].pos.line == pat.pos.line)
        {
            return e
                .with_help("a lambda as an arm's value goes in parentheses: `1 => (y => y + 1)`");
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

impl Resolver<'_> {
    /// E0241 at an alternative an earlier one of the same `|` covers
    /// (D112): one after an alternative that matches everything, the same
    /// alternative written again, or one whose every value an earlier one
    /// matches.
    fn dead_alternatives(&mut self, alt: &Node) {
        // Under a guard each alternative is tried with the guard, so an
        // earlier one covers nothing (a guarded pattern never covers).
        if self.guarded {
            return;
        }
        for (i, a) in alt.kids.iter().enumerate() {
            let Some(by) = alt.kids[..i].iter().find(|b| covers(b, a)) else {
                continue;
            };
            let what = if matches_all(by) {
                "an alternative before it matches everything"
            } else if by.to_string() == a.to_string() || (is_literal(by) && is_literal(a)) {
                "it repeats an alternative before it"
            } else {
                "an alternative before it matches all it matches"
            };
            self.errs
                .push(error("E0241", a.pos, vec![Arg::Str(what.into())]));
            return;
        }
    }

    /// E0235 at an arm an earlier arm without a guard covers (D112): it
    /// never runs. A literal arm after the same literal names that arm's
    /// line (D51), so does any other. An arm after one that matches
    /// everything is reported there instead (`unreachable_after`). An arm
    /// of alternatives only some of which are covered keeps the others:
    /// E0241 at the first covered one.
    fn covered_arms(&mut self, arms: &[Node]) {
        for (j, arm) in arms.iter().enumerate() {
            let pat = &arm.kids[0];
            let earlier = arms[..j].iter().filter(|a| a.kids[1].head == Head::Nil);
            if let Some(by) = earlier.clone().find(|a| covers(&a.kids[0], pat)) {
                if matches_all(&by.kids[0]) {
                    continue; // reported at the arm that matches everything
                }
                let line = by.kids[0].pos.line;
                let what = if is_literal(pat) && is_literal(&by.kids[0]) {
                    format!("never runs: the arm on line {line} matches the same literal first")
                } else {
                    format!("never runs: the arm on line {line} matches all it matches first")
                };
                self.errs.push(
                    error("E0235", pat.pos, vec![Arg::Str(what)])
                        .with_help("remove one of the two arms, or join their bodies in one"),
                );
                continue;
            }
            if pat.head != Head::Palt {
                continue;
            }
            let dead = pat.kids.iter().find_map(|a| {
                let by = earlier.clone().find(|b| covers(&b.kids[0], a))?;
                Some((a, by.kids[0].pos.line))
            });
            if let Some((a, line)) = dead {
                let what = format!("the arm on line {line} matches all it matches first");
                self.errs.push(error("E0241", a.pos, vec![Arg::Str(what)]));
            }
        }
    }
}

/// Whether every value `b` matches, `a` matches too (D112), seen from the
/// patterns alone: `_` or a name covers anything; a literal an equal
/// literal; a tuple, a tag value, a list or a map one of the same shape
/// whose parts it covers (a list or map pattern opened with `...` covers
/// longer lists, or maps with more keys); `a | b` when one alternative
/// does; and a pattern covers alternatives when it covers each. Several
/// patterns that together cover one are not looked at.
fn covers(a: &Node, b: &Node) -> bool {
    if b.head == Head::Palt {
        return b.kids.iter().all(|x| covers(a, x));
    }
    match a.head {
        Head::Pwild | Head::Pbind => true,
        Head::Palt => a.kids.iter().any(|x| covers(x, b)),
        Head::Plit | Head::Pnil | Head::Psym => is_literal(b) && literal_key(a) == literal_key(b),
        Head::Ptuple => b.head == Head::Ptuple && all_cover(&a.kids, &b.kids),
        Head::Ptag => {
            b.head == Head::Ptag && lit_str(a) == lit_str(b) && all_cover(&a.kids, &b.kids)
        }
        Head::Plist => b.head == Head::Plist && list_covers(a, b),
        Head::Pmap => b.head == Head::Pmap && map_covers(a, b),
        _ => false,
    }
}

/// Patterns of the same count, each covering its place.
fn all_cover(a: &[Node], b: &[Node]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| covers(x, y))
}

/// A map pattern entry's key as written (a string or, not valid UTF-8,
/// its bytes).
fn key(e: &Node) -> Option<&[u8]> {
    match &e.lit {
        Lit::Str(s) => Some(s.as_bytes()),
        Lit::Bytes(b) => Some(b),
        _ => None,
    }
}

/// A pattern's items and whether it ends in `...` (a list or a map).
fn split_rest(p: &Node) -> (&[Node], bool) {
    match p.kids.split_last() {
        Some((last, items)) if last.head == Head::Prest => (items, true),
        _ => (&p.kids[..], false),
    }
}

/// A closed list pattern covers one of the same length; an open one with
/// `k` items covers any whose first `k` items it covers (so at least that
/// many), open or not.
fn list_covers(a: &Node, b: &Node) -> bool {
    let ((ai, ar), (bi, br)) = (split_rest(a), split_rest(b));
    if !ar {
        return !br && all_cover(ai, bi);
    }
    bi.len() >= ai.len() && all_cover(ai, &bi[..ai.len()])
}

/// A closed map pattern covers a closed one with the same keys; an open
/// one covers any (open or closed) that has its keys; the values of its
/// keys covering in each case. A closed one never covers an open one.
fn map_covers(a: &Node, b: &Node) -> bool {
    let ((ai, ar), (bi, br)) = (split_rest(a), split_rest(b));
    if !ar && (br || ai.len() != bi.len()) {
        return false;
    }
    ai.iter().all(|e| {
        bi.iter()
            .find(|f| key(f) == key(e))
            .is_some_and(|f| covers(&e.kids[0], &f.kids[0]))
    })
}

/// What a literal pattern matches, as a key: a number by its value (an
/// int and the float of the same value are one, and so are 0.0 and
/// -0.0), anything else as it is written.
fn literal_key(p: &Node) -> String {
    match &p.lit {
        Lit::Int(i) => format!("n{i}"),
        Lit::Float(f) if f.fract() == 0.0 && f.abs() < 9.0e18 => format!("n{}", *f as i64),
        Lit::Float(f) => format!("n{f:?}"),
        _ => p.to_string(),
    }
}

/// A literal pattern: a number, string, duration, `true`/`false`, `nil`,
/// or a symbol (a bare tag).
fn is_literal(p: &Node) -> bool {
    matches!(p.head, Head::Plit | Head::Pnil | Head::Psym)
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
