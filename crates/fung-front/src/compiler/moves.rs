//! When a function's locals hand over and let go of their values (03: 값의
//! 수명, P9, VM19, VM23). The compiler only looks the answers up, by node
//! number (`Node::id`):
//!
//! - which reads of locals are their last: they compile to MOVELOCAL,
//!   which hands the value over and empties the slot, so a callee that
//!   gets the only reference can change a map or list in place
//!   (`Rc::make_mut`) instead of copying it;
//! - which slots to let go (CLEARLOCAL) once a block's statement is done,
//!   as a branch starts (an `if`'s, a `??`'s, an `and`'s or `or`'s right
//!   side, a `match` arm's body once its pattern and guard passed), and
//!   right after a CLOSURE whose capture was a slot's last read;
//! - which always-matching `name =>` arms move the subject into the name.
//!
//! Two walks find them. The language has no loops (repetition is
//! recursion, and a `for` body is a function), so every path through a
//! frame's code goes forward through choices.
//!
//! The first walks back in evaluation order and keeps the slots read
//! after the point it is at (liveness). A `(local i)` whose slot is not
//! among them is a last read; a capture likewise, but a capture copies,
//! so its slot is let go after the CLOSURE. An assignment, and a match
//! arm's pattern, writes its slots: what was read before is not read
//! after (a slot is reused by the names of sibling blocks, and the write
//! tells them apart). At a choice each branch is walked back from the
//! same point; the slots read from the choice on but not from a branch's
//! start are the ones that branch lets go as it starts. A match's
//! subject and guards are compiled more than once, so their reads copy
//! and record nothing. The walk changes one live set in place and undoes
//! a branch's changes before the next (a log), so its cost follows the
//! size of the code, not the code times the live slots (VM24).
//!
//! The second walks forward in the order the compiler emits code and
//! carries the slots known dead but not yet let go: those a path drops
//! where no code runs (the skipped side of a condition's `and`/`or`, an
//! `if` statement without `else`) and those a failed arm's pattern or
//! guard left bound (VM14). The next place that lets go takes them too.
//! A store takes its slots out: a value stored and never read is kept
//! until the frame returns (03), and one read later is live again. Only a
//! call can see a value go (a callee changing it in place, a wait on a
//! channel it holds), so a branch that calls nothing and ends the frame
//! (in tail position, or a `return`) lets go of nothing, nor does a
//! function value that is returned at once: the return does.
//!
//! Where unsure the analysis keeps a value rather than let it go: a
//! slot is let go only where no path reads it before writing it again.

use std::collections::{BTreeSet, HashMap, HashSet};

use super::m4::always_matches;
use super::{
    Logic, is_if_without_else, is_not_of_bool, is_statement, lit_index, lit_str, local_reads, logic,
};
use crate::tree::{CaptureKind, Head, Lit, Node};

/// What the walks learn about one frame's code, by node number.
#[derive(Default, Debug)]
pub(super) struct Lives {
    /// The reads of locals that are their last: MOVELOCAL.
    pub moves: HashSet<u32>,
    /// The slots to let go once a block's statement is done.
    pub after: HashMap<u32, Vec<usize>>,
    /// The slots to let go as a branch starts, by the branch's node: an
    /// `if`'s `then` or `else` node (also the right side of `and`/`or`),
    /// a `??`'s right side, a match arm's body; and for the path on which
    /// a `??`'s left side's value is kept, the left side. Each is a child
    /// of its choice in its own place, so no two branches share a key.
    pub entry: HashMap<u32, Vec<usize>>,
    /// The slots to let go right after a CLOSURE, by the `fn` node: its
    /// capture was their last read.
    pub closure: HashMap<u32, Vec<usize>>,
    /// The always-matching `name =>` arms that move the subject, a local
    /// nothing reads after, into the name.
    pub subject_moves: HashSet<u32>,
    /// The slots a function lets go as its body starts, once its
    /// parameters' types are checked: what a default read last on a path
    /// that skipped later defaults, and a typed parameter whose last read
    /// was in a default (its type check reads it after the defaults).
    /// Both a call that ran the defaults and one that gave every argument
    /// pass there (VM39, VM48).
    pub at_body: Vec<usize>,
}

/// The lifetimes in `body`, a function's body or a program's. `tail` says
/// whether the body's value is returned (a function's).
pub(super) fn lives(body: &Node, tail: bool) -> Lives {
    let mut back = Back::new();
    back.walk(body);
    let mut fwd = Fwd::new(&mut back);
    fwd.expr(body, tail);
    fwd.out
}

/// The lifetimes in a function: its `defaults` (`(parameter, default)`,
/// in order), each run only when its parameter was not given, then the
/// type checks of the `typed` parameters, then its `body`. A default is
/// a branch like an `if` without `else`: its names and its last reads
/// are let go as in the body (VM48), and what it read last is let go on
/// the path that skipped it at the next place that lets go, at the
/// latest as the body starts (`Lives::at_body`).
pub(super) fn fn_lives(defaults: &[(usize, &Node)], typed: &[usize], body: &Node) -> Lives {
    let mut back = Back::new();
    back.walk(body);
    let live_body: BTreeSet<usize> = (0..back.live.len()).filter(|&s| back.live[s]).collect();
    // The type checks read their parameters after the defaults: a
    // default's read of one is not its last.
    for &s in typed {
        back.set(s, true);
    }
    let mut skips = Vec::new();
    for &(slot, d) in defaults.iter().rev() {
        let mark = back.log.len();
        back.set(slot, false);
        back.walk(d);
        let at = back.delta(mark);
        back.undo(mark);
        let branch = |at| Branch {
            id: 0,
            start: HashMap::clone(&at),
            at,
            read_first: Vec::new(),
        };
        let mut dead = back.join(vec![branch(at), branch(HashMap::new())], &[]);
        let skipped = dead.pop().expect("the skipped path");
        // The default's own parameter is empty as it starts: nothing to
        // let go there.
        let ran: Vec<usize> = dead
            .pop()
            .expect("the default")
            .into_iter()
            .filter(|&s| s != slot)
            .collect();
        if !ran.is_empty() && d.id != 0 {
            back.deaths.insert(d.id, ran);
        }
        skips.push(skipped);
    }
    skips.reverse();
    let mut fwd = Fwd::new(&mut back);
    for (&(slot, d), skipped) in defaults.iter().zip(skips) {
        let before = fwd.pending.clone();
        fwd.enter(d.id, false, None);
        fwd.expr(d, false);
        fwd.pending.remove(&slot);
        let ran = std::mem::replace(&mut fwd.pending, before);
        fwd.pending.extend(skipped);
        fwd.meet(ran);
    }
    let mut at_body = std::mem::take(&mut fwd.pending);
    let mut read = BTreeSet::new();
    for &(_, d) in defaults {
        local_reads(d, &mut read);
    }
    at_body.extend(
        typed
            .iter()
            .filter(|s| read.contains(s) && !live_body.contains(s)),
    );
    fwd.out.at_body = at_body.into_iter().collect();
    fwd.expr(body, true);
    fwd.out
}

/// The slots that pattern or assignment target `n` writes.
fn written_slots(n: &Node, out: &mut BTreeSet<usize>) {
    if n.head == Head::Local {
        out.insert(lit_index(n));
    }
    for k in &n.kids {
        written_slots(k, out);
    }
}

fn written(n: &Node) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    written_slots(n, &mut out);
    out
}

/// The arms of a match that are compiled: up to the first that always
/// matches (VM10 c).
fn compiled_arms(n: &Node) -> &[Node] {
    let arms = &n.kids[1..];
    match arms.iter().position(always_matches) {
        Some(i) => &arms[..=i],
        None => arms,
    }
}

/// The backward walk: liveness, last reads, and what each branch lets go
/// as it starts.
struct Back {
    /// Whether each slot is read from the current point on.
    live: Vec<bool>,
    /// Each change to `live`: the slot and its value before.
    log: Vec<(usize, bool)>,
    moves: HashSet<u32>,
    /// For each branch start, the slots read from its choice on and not
    /// from it: dead as it starts.
    deaths: HashMap<u32, Vec<usize>>,
    /// For each `fn` node, the slots whose last read is its capture.
    captures: HashMap<u32, Vec<usize>>,
}

/// How one branch of a choice changed the live set: each slot it changed,
/// with its value at the branch's start (`at`) and where the branch
/// begins, before its pattern and guard (`start`).
struct Branch {
    id: u32,
    at: HashMap<usize, bool>,
    start: HashMap<usize, bool>,
    /// Slots read on the way into the branch, after the choice (a match
    /// arm's guard) and not in it: dead as it starts too.
    read_first: Vec<usize>,
}

impl Back {
    fn new() -> Back {
        Back {
            live: Vec::new(),
            log: Vec::new(),
            moves: HashSet::new(),
            deaths: HashMap::new(),
            captures: HashMap::new(),
        }
    }

    fn get(&self, s: usize) -> bool {
        self.live.get(s).copied().unwrap_or(false)
    }

    fn set(&mut self, s: usize, on: bool) {
        if s >= self.live.len() {
            if !on {
                return;
            }
            self.live.resize(s + 1, false);
        }
        if self.live[s] != on {
            self.log.push((s, self.live[s]));
            self.live[s] = on;
        }
    }

    /// The slots changed since log position `mark`, with their values now.
    fn delta(&self, mark: usize) -> HashMap<usize, bool> {
        let mut was: HashMap<usize, bool> = HashMap::new();
        for &(s, before) in &self.log[mark..] {
            was.entry(s).or_insert(before);
        }
        was.into_iter()
            .filter(|&(s, before)| self.get(s) != before)
            .map(|(s, _)| (s, self.get(s)))
            .collect()
    }

    fn undo(&mut self, mark: usize) {
        while self.log.len() > mark {
            let (s, before) = self.log.pop().expect("a logged change");
            self.live[s] = before;
        }
    }

    /// A read that copies and records nothing: a match's subject and
    /// guards, which are compiled more than once.
    fn copy_reads(&mut self, n: &Node) {
        let mut slots = BTreeSet::new();
        local_reads(n, &mut slots);
        for s in slots {
            self.set(s, true);
        }
    }

    /// Walks `n` back: from the live set after it to the one before it.
    fn walk(&mut self, n: &Node) {
        match n.head {
            Head::Local => {
                let s = lit_index(n);
                if !self.get(s) {
                    if n.id != 0 {
                        self.moves.insert(n.id);
                    }
                    self.set(s, true);
                }
            }
            // A function value copies what it captures; its body is its
            // own frame's.
            Head::Fn => {
                let Lit::Fn(info) = &n.lit else { return };
                let mut last = Vec::new();
                for c in &info.captures {
                    if c.kind == CaptureKind::Local && !self.get(c.index) {
                        last.push(c.index);
                        self.set(c.index, true);
                    }
                }
                if !last.is_empty() && n.id != 0 {
                    last.sort_unstable();
                    last.dedup();
                    self.captures.insert(n.id, last);
                }
            }
            // The value is computed, then stored: the target's slots are
            // not read again before the store.
            Head::Let => {
                for s in written(&n.kids[0]) {
                    self.set(s, false);
                }
                self.walk(&n.kids[1]);
            }
            Head::Import => {}
            // A `test` block is top level only (E0114): it holds no slot
            // of this frame.
            Head::Test => {}
            Head::If | Head::Then => {
                self.choice(&[&n.kids[1], &n.kids[2]], &[n.kids[1].id, n.kids[2].id]);
                self.walk(&n.kids[0]);
            }
            // The left side's value is kept, or the right side runs.
            Head::Coalesce => {
                let rhs = &n.kids[1];
                let mut branches = Vec::new();
                let mark = self.log.len();
                branches.push(Branch {
                    id: n.kids[0].id,
                    at: HashMap::new(),
                    start: HashMap::new(),
                    read_first: Vec::new(),
                });
                self.walk(rhs);
                let d = self.delta(mark);
                branches.push(Branch {
                    id: rhs.id,
                    at: d.clone(),
                    start: d,
                    read_first: Vec::new(),
                });
                self.undo(mark);
                self.join(branches, &[]);
                self.walk(&n.kids[0]);
            }
            // Each compiled arm loads the subject, binds its pattern,
            // tests its guard and runs its body; no arm matching is an
            // error, so nothing follows that path.
            Head::Match => {
                let mut branches = Vec::new();
                for arm in compiled_arms(n) {
                    let mark = self.log.len();
                    self.walk(&arm.kids[2]);
                    let at = self.delta(mark);
                    // What the guard reads last (a name the pattern bound
                    // for it, say) is dead as the body starts.
                    let mut guard = BTreeSet::new();
                    local_reads(&arm.kids[1], &mut guard);
                    let read_first = guard.into_iter().filter(|&s| !self.get(s)).collect();
                    self.copy_reads(&arm.kids[1]);
                    for s in written(&arm.kids[0]) {
                        self.set(s, false);
                    }
                    let start = self.delta(mark);
                    self.undo(mark);
                    branches.push(Branch {
                        id: arm.kids[2].id,
                        at,
                        start,
                        read_first,
                    });
                }
                let mut subject = BTreeSet::new();
                local_reads(&n.kids[0], &mut subject);
                let subject: Vec<usize> = subject.into_iter().collect();
                self.join(branches, &subject);
            }
            // A block's statements, last first.
            Head::Do => {
                for k in n.kids.iter().rev() {
                    self.walk(k);
                }
            }
            // Nothing follows a return on its path.
            Head::Return => {
                for s in 0..self.live.len() {
                    self.set(s, false);
                }
                self.walk(&n.kids[0]);
            }
            _ => {
                for k in n.kids.iter().rev() {
                    self.walk(k);
                }
            }
        }
    }

    /// The branches of an `if`: each walked back from the same point.
    fn choice(&mut self, kids: &[&Node], ids: &[u32]) {
        let mut branches = Vec::new();
        for (k, &id) in kids.iter().zip(ids) {
            let mark = self.log.len();
            self.walk(k);
            let d = self.delta(mark);
            self.undo(mark);
            branches.push(Branch {
                id,
                at: d.clone(),
                start: d,
                read_first: Vec::new(),
            });
        }
        self.join(branches, &[]);
    }

    /// Joins the branches of a choice: the live set before the choice
    /// holds what any branch reads from its start, and `reads` (read
    /// before the choice, on every path: a match's subject). A slot read
    /// from the choice on and not where a branch starts is dead there.
    /// The live set is the one after the choice when called.
    /// Returns what each branch lets go as it starts, in order.
    fn join(&mut self, branches: Vec<Branch>, reads: &[usize]) -> Vec<Vec<usize>> {
        // Slots whose value before the choice differs from after it.
        let mut changed: HashMap<usize, bool> = HashMap::new();
        for b in &branches {
            for &s in b.start.keys() {
                let before = branches
                    .iter()
                    .any(|o| o.start.get(&s).copied().unwrap_or_else(|| self.get(s)));
                if before != self.get(s) {
                    changed.insert(s, before);
                }
            }
        }
        // What is read before the choice is live there whatever the
        // branches do: arms that all return kill every slot, and a subject
        // read again after the match must not be let go before it (VM37).
        for &s in reads {
            if self.get(s) {
                changed.remove(&s);
            } else {
                changed.insert(s, true);
            }
        }
        let before = |s: usize| changed.get(&s).copied().unwrap_or_else(|| self.get(s));
        // The slots read from the choice on and not after it.
        let gained: Vec<usize> = changed.iter().filter(|e| *e.1).map(|e| *e.0).collect();
        let mut found = Vec::new();
        for b in &branches {
            let mut dead: Vec<usize> = gained
                .iter()
                .copied()
                .filter(|s| !b.at.contains_key(s))
                .collect();
            dead.extend(
                b.at.iter()
                    .filter(|&(&s, &on)| !on && before(s))
                    .map(|e| *e.0),
            );
            dead.extend(&b.read_first);
            dead.sort_unstable();
            dead.dedup();
            found.push(dead);
        }
        for (b, dead) in branches.iter().zip(&found) {
            if !dead.is_empty() && b.id != 0 {
                self.deaths.insert(b.id, dead.clone());
            }
        }
        let mut changed: Vec<(usize, bool)> = changed.into_iter().collect();
        changed.sort_unstable();
        for (s, on) in changed {
            self.set(s, on);
        }
        found
    }
}

/// The forward walk, in the order the compiler emits code: the slots
/// dead but not let go yet on the path (`pending`), let go at the next
/// statement end, branch start or capture.
struct Fwd<'a> {
    deaths: &'a HashMap<u32, Vec<usize>>,
    captures: &'a HashMap<u32, Vec<usize>>,
    pending: BTreeSet<usize>,
    out: Lives,
    /// Whether each node's code may call a function (`calls`).
    calls: HashMap<u32, bool>,
}

impl<'a> Fwd<'a> {
    fn new(back: &'a mut Back) -> Fwd<'a> {
        Fwd {
            pending: BTreeSet::new(),
            out: Lives {
                moves: std::mem::take(&mut back.moves),
                ..Lives::default()
            },
            deaths: &back.deaths,
            captures: &back.captures,
            calls: HashMap::new(),
        }
    }

    fn deaths(&self, id: u32) -> &[usize] {
        self.deaths.get(&id).map_or(&[], Vec::as_slice)
    }

    /// Whether running `n` may call a function: only a call can see a
    /// value let go early (a callee changing it in place, a wait on a
    /// channel it holds). A function value's body runs elsewhere.
    fn calls(&mut self, n: &Node) -> bool {
        if let Some(&c) = self.calls.get(&n.id) {
            return c;
        }
        let c = match n.head {
            Head::Call | Head::Import => true,
            Head::Fn => false,
            _ => n.kids.iter().any(|k| self.calls(k)),
        };
        if n.id != 0 {
            self.calls.insert(n.id, c);
        }
        c
    }

    /// Whether branch `n` lets go of nothing as it starts: nothing in it
    /// calls a function, and the frame ends with it (in tail position, or
    /// a `return`), so no one could see the values go any sooner.
    fn quiet(&mut self, n: &Node, tail: bool) -> bool {
        fn returns(n: &Node) -> bool {
            match n.head {
                Head::Return => true,
                Head::Do => n.kids.last().is_some_and(returns),
                _ => false,
            }
        }
        (tail || returns(n)) && !self.calls(n)
    }

    /// A branch with code of its own starts: it lets go of what is dead
    /// there, save `kept` (moved instead). A `quiet` branch lets go of
    /// nothing: the frame's end does, and nothing sees the difference.
    fn enter(&mut self, id: u32, quiet: bool, kept: Option<usize>) {
        let mut go = std::mem::take(&mut self.pending);
        if quiet {
            return;
        }
        go.extend(self.deaths(id));
        if let Some(s) = kept {
            go.remove(&s);
        }
        if !go.is_empty() && id != 0 {
            self.out.entry.insert(id, go.into_iter().collect());
        }
    }

    /// A path that runs no code of its own (the skipped side of an
    /// `and`/`or` in a condition, the missing `else`): what is dead on
    /// it is let go at the next place that lets go.
    fn skip(&mut self, id: u32) {
        let dead = self.deaths(id).to_vec();
        self.pending.extend(dead);
    }

    fn store(&mut self, target: &Node) {
        for s in written(target) {
            self.pending.remove(&s);
        }
    }

    /// Ends statement `k` of a block.
    fn after(&mut self, k: &Node) {
        if !self.pending.is_empty() && k.id != 0 {
            let go = std::mem::take(&mut self.pending);
            self.out.after.insert(k.id, go.into_iter().collect());
        }
    }

    /// Two paths that meet: what either left pending is.
    fn meet(&mut self, other: BTreeSet<usize>) {
        self.pending.extend(other);
    }

    /// Like `Compiler::block`: only the last expression is in tail
    /// position.
    fn block(&mut self, n: &Node, tail: bool) {
        let last = n.kids.len().saturating_sub(1);
        for (i, k) in n.kids.iter().enumerate() {
            if is_statement(k) {
                match k.head {
                    Head::Let => {
                        self.expr(&k.kids[1], false);
                        self.store(&k.kids[0]);
                    }
                    Head::Import => self.store(k),
                    Head::Assert => self.expr(&k.kids[0], false),
                    _ => {}
                }
            } else if i < last && is_if_without_else(k) {
                self.if_stmt(k);
            } else {
                self.expr(k, tail && i == last);
            }
            if i < last {
                self.after(k);
            }
        }
    }

    /// Like `Compiler::expr`, for what lets go.
    fn expr(&mut self, n: &Node, tail: bool) {
        match n.head {
            Head::Do => self.block(n, tail),
            // A function value returned at once lets go of nothing: the
            // return does.
            Head::Fn if tail => {}
            Head::Fn => {
                if let Some(c) = self.captures.get(&n.id) {
                    self.out.closure.insert(n.id, c.clone());
                }
            }
            Head::If | Head::Then => {
                self.cond(&n.kids[0]);
                let before = self.pending.clone();
                let quiet = self.quiet(&n.kids[1], tail);
                self.enter(n.kids[1].id, quiet, None);
                self.expr(&n.kids[1], tail);
                let then = std::mem::replace(&mut self.pending, before);
                let quiet = self.quiet(&n.kids[2], tail);
                self.enter(n.kids[2].id, quiet, None);
                self.expr(&n.kids[2], tail);
                self.meet(then);
            }
            Head::Coalesce => {
                self.expr(&n.kids[0], false);
                let before = self.pending.clone();
                self.enter(n.kids[0].id, tail, None);
                let kept = std::mem::replace(&mut self.pending, before);
                let quiet = self.quiet(&n.kids[1], tail);
                self.enter(n.kids[1].id, quiet, None);
                self.expr(&n.kids[1], tail);
                self.meet(kept);
            }
            Head::Match => self.match_expr(n, tail),
            // The checked right side of an `and`/`or` is the value
            // itself: it returns, or tail calls, where the `and`/`or`
            // does (VM53).
            Head::Op if lit_str(n) == crate::tree::BOOL_CHECK => self.expr(&n.kids[0], tail),
            Head::Return => {
                self.expr(&n.kids[0], true);
                self.pending.clear();
            }
            _ => {
                for k in &n.kids {
                    self.expr(k, false);
                }
            }
        }
    }

    /// Like `Compiler::if_stmt`: the `else` runs no code.
    fn if_stmt(&mut self, k: &Node) {
        self.cond(&k.kids[0]);
        let before = self.pending.clone();
        let quiet = self.quiet(&k.kids[1], false);
        self.enter(k.kids[1].id, quiet, None);
        self.expr(&k.kids[1], false);
        let then = std::mem::replace(&mut self.pending, before);
        self.skip(k.kids[2].id);
        self.meet(then);
    }

    /// Like `Compiler::cond`: `and`/`or` jump instead of making a bool, so
    /// one of their two paths runs no code of its own.
    fn cond(&mut self, n: &Node) {
        match logic(n) {
            Some(Logic::And(a, b)) => {
                self.cond(a);
                let before = self.pending.clone();
                self.enter(n.kids[1].id, false, None);
                self.cond(b);
                let ran = std::mem::replace(&mut self.pending, before);
                self.skip(n.kids[2].id);
                self.meet(ran);
            }
            Some(Logic::Or(a, b)) => {
                self.cond(a);
                let before = self.pending.clone();
                self.skip(n.kids[1].id);
                let skipped = std::mem::replace(&mut self.pending, before);
                self.enter(n.kids[2].id, false, None);
                self.cond(b);
                self.meet(skipped);
            }
            None if n.head == Head::Op && lit_str(n) == crate::tree::BOOL_CHECK => {
                self.cond(&n.kids[0])
            }
            // `not e` of a bool `e` is `e` with the targets swapped: the
            // same code, so the same places let go (VM38).
            None if is_not_of_bool(n) => self.cond(&n.kids[0]),
            None => self.expr(n, false),
        }
    }

    /// Like `Compiler::match_expr`. An arm is reached when the arms
    /// before it failed; a failed arm may have bound its pattern before
    /// its guard failed, and those slots are read nowhere on (VM14).
    fn match_expr(&mut self, n: &Node, tail: bool) {
        let subject = &n.kids[0];
        let mut reached = std::mem::take(&mut self.pending);
        let mut ends = BTreeSet::new();
        for arm in compiled_arms(n) {
            let (pat, body) = (&arm.kids[0], &arm.kids[2]);
            let bound = written(pat);
            self.pending = reached.difference(&bound).copied().collect();
            // The last arm takes the subject when nothing reads it after.
            let mut kept = None;
            if always_matches(arm)
                && pat.head == Head::Pbind
                && subject.head == Head::Local
                && self.deaths(body.id).contains(&lit_index(subject))
            {
                self.out.subject_moves.insert(arm.id);
                kept = Some(lit_index(subject));
            }
            let quiet = self.quiet(body, tail);
            self.enter(body.id, quiet, kept);
            self.expr(body, tail);
            ends.append(&mut self.pending);
            reached.extend(bound);
        }
        self.pending = ends;
    }
}
