//! Turns a resolved core tree into bytecode. It is a single recursive walk: each core form knows
//! how to emit itself, every emitted instruction carries the position of
//! the node it came from, and a `tail` flag follows the walk so calls in
//! tail position become TAILCALL (03. 꼬리 위치).
//!
//! Recursion follows tree depth, so callers with very deep trees need a
//! large stack (see `parser`).

mod fn_;
mod fold;
mod m4;
mod m6;
mod moves;
mod pattern;
#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::bytecode::{Op, Proto};
use crate::consts::Const;
use crate::diag::{Arg, Diag, error};
use crate::resolve::Resolved;
use crate::source::Pos;
use crate::tree::{CaptureKind, Head, Lit, Node};

/// Configures compilation of one file.
#[derive(Clone, Debug, Default)]
pub struct Options<'s> {
    /// The file's text, for assert and dbg source spans (08), borrowed
    /// from the loader (FE39).
    pub source: &'s str,
    /// The file's name as positions print it (`-e`, `t.fung`): the place
    /// `dbg` reports.
    pub file: &'s str,
    /// Register test blocks (`fung test` / `check`); false drops them.
    pub tests: bool,
    /// The program's last expression is its value; otherwise it is
    /// discarded like a statement (01: 버려지는 자리), and the program's
    /// value is nil.
    pub keep_value: bool,
}

struct Compiler<'a> {
    proto: Proto,
    prelude: &'a BTreeSet<String>,
    opts: &'a Options<'a>,
    /// Byte offsets where the source's lines start (`start_pos`).
    lines: &'a [usize],
    /// The operators that fold to a constant (`fold::constants`).
    folds: &'a HashMap<*const Node, Const>,
    /// A return type annotation disables tail calls (03).
    has_ret: bool,
    /// The reads of locals that are their last (`moves::last_reads`):
    /// they compile to MOVELOCAL.
    moves: HashSet<*const Node>,
    /// The slots read after each statement of a block (`moves::Lives`).
    after: HashMap<*const Node, moves::Live>,
    /// For each store, the slots read after it (`moves::Lives::stored`).
    stored_live: HashMap<*const Node, moves::Live>,
    /// The locals that hold, on the path being compiled, a value read
    /// somewhere after it was stored, and are not let go yet: a
    /// statement's end lets go of those not read after it (`block`, 03:
    /// 값의 수명). A move empties its slot; a value never read is never
    /// held, so it lives on. Each branch starts from what was held before.
    held: BTreeSet<usize>,
    /// The UNWRAPTESTs of `?` and their places, whose returns go after
    /// the function's end (`emit_stubs`).
    stubs: Vec<(usize, Pos)>,
    /// Where a value in tail position is returned: the function body's
    /// place, or a `return`'s. A branch that returns by itself (VM10 f)
    /// checks the return type there, as the function's end would.
    ret_pos: Pos,
}

/// Produces the `<main>` proto for a resolved program. Prelude names
/// become `Const::Builtin(name)` (`args` is `Const::Args`). The main
/// body never uses tail calls: it is the entry frame.
pub fn compile(
    res: &Resolved,
    prelude: &BTreeSet<String>,
    opts: &Options<'_>,
) -> Result<Proto, Diag> {
    let lines = line_starts(opts.source);
    let folds = fold::constants(&res.core);
    let mut c = Compiler {
        proto: Proto {
            name: "<main>".into(),
            num_locals: res.num_locals,
            global_names: res.globals.clone(),
            exports: res.exports.clone(),
            ..Proto::default()
        },
        prelude,
        opts,
        lines: &lines,
        folds: &folds,
        has_ret: true,
        moves: HashSet::new(),
        after: HashMap::new(),
        stored_live: HashMap::new(),
        held: BTreeSet::new(),
        stubs: Vec::new(),
        ret_pos: res.core.pos,
    };
    c.take_lives(moves::last_reads(&res.core));
    c.block(&res.core, false)?;
    if !opts.keep_value
        && let Some(last) = res.core.kids.last()
        && !is_statement(last)
    {
        let pos = c.start_pos(last);
        c.proto.emit(Op::Pop, 0, pos);
        c.emit_const(Const::Nil, pos);
    }
    c.proto.emit(Op::Return, 0, res.core.pos);
    c.emit_stubs();
    c.proto.check_operands()?;
    Ok(c.proto)
}

/// The byte offsets where `src`'s lines start.
fn line_starts(src: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(src.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}

/// Adds the local slots `n` reads in this function's frame to `out`: its
/// `(local i)` leaves, and the locals a function made there captures (a
/// capture copies, so it is a read). A nested function's own code runs in
/// its own frame; a pattern or an assignment's target only writes.
fn local_reads(n: &Node, out: &mut BTreeSet<usize>) {
    match n.head {
        Head::Local => {
            out.insert(lit_index(n));
        }
        Head::Fn => {
            if let Lit::Fn(info) = &n.lit {
                for c in &info.captures {
                    if c.kind == CaptureKind::Local {
                        out.insert(c.index);
                    }
                }
            }
        }
        Head::Let => local_reads(&n.kids[1], out),
        Head::Arm => {
            for k in &n.kids[1..] {
                local_reads(k, out);
            }
        }
        _ => {
            for k in &n.kids {
                local_reads(k, out);
            }
        }
    }
}

/// The items `Compiler::block` compiles as statements (no value).
fn is_statement(n: &Node) -> bool {
    matches!(n.head, Head::Let | Head::Import | Head::Assert | Head::Test)
}

/// An `if` without `else`: desugared with a `nil` else branch.
fn is_if_without_else(n: &Node) -> bool {
    n.head == Head::If && n.kids[2].head == Head::Nil
}

/// `a and b` or `a or b` as desugar writes them: `(if a (bool b) false)`
/// and `(if a true (bool b))`, with `b` the right side, still checked.
enum Logic<'a> {
    And(&'a Node, &'a Node),
    Or(&'a Node, &'a Node),
}

fn logic(n: &Node) -> Option<Logic<'_>> {
    if n.head != Head::If {
        return None;
    }
    let is_bool =
        |k: &Node, b: bool| k.head == Head::Bool && matches!(k.lit, Lit::Bool(x) if x == b);
    let checked = |k: &Node| k.head == Head::Op && lit_str(k) == "bool";
    let [a, then, other] = &n.kids[..] else {
        return None;
    };
    if checked(then) && is_bool(other, false) {
        return Some(Logic::And(a, then));
    }
    if is_bool(then, true) && checked(other) {
        return Some(Logic::Or(a, other));
    }
    None
}

/// Whether `n` always makes a bool, or fails: a comparison, `in`, `not`,
/// `and`, `or` or a bool literal. CHECKBOOL after one is left out (VM10 d).
fn makes_bool(n: &Node) -> bool {
    match n.head {
        Head::Bool => true,
        Head::Op => matches!(
            lit_str(n),
            "==" | "!=" | "<" | ">" | "<=" | ">=" | "in" | "not" | "bool"
        ),
        Head::If => logic(n).is_some(),
        _ => false,
    }
}

/// An E0801 internal error.
fn internal(pos: Pos, msg: String) -> Diag {
    error("E0801", pos, vec![Arg::Str(msg)])
}

/// The constant of a string literal: text, or bytes (L4).
fn str_const(n: &Node) -> Const {
    match &n.lit {
        Lit::Bytes(b) => Const::Bytes(b.clone()),
        _ => Const::Str(lit_str(n).to_string()),
    }
}

fn lit_str(n: &Node) -> &str {
    match &n.lit {
        Lit::Str(s) => s,
        _ => "",
    }
}

fn lit_index(n: &Node) -> usize {
    match n.lit {
        Lit::Int(i) => i as usize,
        _ => 0,
    }
}

/// The constant a prelude name compiles to, as `fung dump` shows it:
/// functions print as `<fn name>`, and `args` is the program's
/// argument list, which is empty when compiling (`fung dump` passes none).
fn prelude_const(name: &str) -> Const {
    if name == "args" {
        return Const::Args;
    }
    Const::Builtin(name.to_string())
}

fn binary_op(op: &str) -> Option<Op> {
    Some(match op {
        "+" => Op::Add,
        "-" => Op::Sub,
        "*" => Op::Mul,
        "/" => Op::Div,
        "//" => Op::FloorDiv,
        "%" => Op::Mod,
        "**" => Op::Pow,
        "==" => Op::Eq,
        "!=" => Op::Ne,
        "<" => Op::Lt,
        ">" => Op::Gt,
        "<=" => Op::Le,
        ">=" => Op::Ge,
        _ => return None,
    })
}

fn unary_op(op: &str) -> Option<Op> {
    Some(match op {
        "-" => Op::Neg,
        "not" => Op::Not,
        "bool" => Op::CheckBool,
        _ => return None,
    })
}

impl Compiler<'_> {
    /// Where `n`'s source text starts: a call's own position is its `(`,
    /// an operator's is the operator. Falls back to `n.pos` without a span.
    fn start_pos(&self, n: &Node) -> Pos {
        let src = self.opts.source;
        if n.end == 0 || n.off > src.len() || !src.is_char_boundary(n.off) {
            return n.pos;
        }
        let line = self.lines.partition_point(|&s| s <= n.off);
        let from = self.lines[line - 1];
        Pos {
            file: n.pos.file,
            line: line as u32,
            col: src[from..n.off].chars().count() as u32 + 1,
        }
    }

    fn emit_const(&mut self, c: Const, pos: Pos) {
        let i = self.proto.add_const(c);
        self.proto.emit(Op::Const, i, pos);
    }

    /// Compiles `(do e…)` leaving the block's value on the stack: the last
    /// expression's value, or nil when the block ends in a statement (01:
    /// 블록이 문장으로 끝나면 nil). Only the last expression is in tail
    /// position.
    fn block(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        if n.kids.is_empty() {
            self.emit_const(Const::Nil, n.pos);
            return Ok(());
        }
        let last_i = n.kids.len() - 1;
        for (i, k) in n.kids.iter().enumerate() {
            let last = i == last_i;
            let stmt = match k.head {
                Head::Let => Some(
                    self.let_stmt(k)
                        .map(|()| self.stored(&k.kids[0], &k.kids[0])),
                ),
                Head::Import => Some(self.import_stmt(k)),
                Head::Assert => Some(self.assert_stmt(k)),
                Head::Test => Some(self.test_stmt(k)),
                _ => None,
            };
            if let Some(r) = stmt {
                r?;
                if last {
                    self.emit_const(Const::Nil, k.pos);
                } else {
                    self.let_go(k);
                }
                continue;
            }
            if !last && is_if_without_else(k) {
                self.if_stmt(k)?;
                self.let_go(k);
                continue;
            }
            self.expr(k, tail && last)?;
            if !last {
                // A discarded Err is reported where the statement starts.
                let pos = self.start_pos(k);
                self.proto.emit(Op::Pop, 0, pos);
                self.let_go(k);
            }
        }
        Ok(())
    }

    /// Ends statement `k` of a block: the locals held on this path and not
    /// read after it are let go (03: 값의 수명). A statement the walk did
    /// not record lets go of nothing.
    fn let_go(&mut self, k: &Node) {
        if let Some(later) = self.after.get(&(k as *const Node)) {
            let later = later.clone();
            let mut held = std::mem::take(&mut self.held);
            let pos = self.start_pos(k);
            self.clear_unread(&mut held, &later, pos);
            self.held = held;
        }
    }

    /// Takes what the walk learned about this frame (`moves::last_reads`):
    /// the parameters read later hold values to let go.
    pub(super) fn take_lives(&mut self, lives: moves::Lives) {
        self.moves = lives.moves;
        self.after = lives.after;
        self.stored_live = lives.stored;
        self.held = lives.entry;
    }

    /// Target or pattern `n` (as the walk saw it, `key`) stored into its
    /// slots: a value read after the store is held; one never read is not,
    /// so it lives on until the frame returns (03: 값의 수명).
    pub(super) fn stored(&mut self, key: &Node, n: &Node) {
        let live = self.stored_live.get(&(key as *const Node)).cloned();
        let mut slots = BTreeSet::new();
        moves::written_slots(n, &mut slots);
        for slot in slots {
            if live.as_ref().is_some_and(|l| l.contains(&slot)) {
                self.held.insert(slot);
            } else {
                self.held.remove(&slot);
            }
        }
    }

    /// Compiles the branches of one choice (`if`, `??`, `match` arms) one
    /// after another: each starts from the locals held before the choice,
    /// and after it a local is held if some branch still holds it.
    pub(super) fn branches(
        &mut self,
        count: usize,
        mut each: impl FnMut(&mut Self, usize) -> Result<(), Diag>,
    ) -> Result<(), Diag> {
        let before = self.held.clone();
        let mut joined = BTreeSet::new();
        for i in 0..count {
            self.held = before.clone();
            each(self, i)?;
            joined.append(&mut self.held);
        }
        self.held = joined;
        Ok(())
    }

    /// Emits CLEARLOCAL for each local in `held` that `later` does not
    /// read, and forgets it.
    fn clear_unread(&mut self, held: &mut BTreeSet<usize>, later: &BTreeSet<usize>, pos: Pos) {
        let gone: Vec<usize> = held.difference(later).copied().collect();
        for slot in gone {
            self.proto.emit(Op::ClearLocal, slot, pos);
            held.remove(&slot);
        }
    }

    /// Compiles an assignment: the value, then a store into the slot or
    /// global that resolve chose — or a MATCHORFAIL for a destructuring
    /// pattern.
    fn let_stmt(&mut self, n: &Node) -> Result<(), Diag> {
        self.expr(&n.kids[1], false)?;
        let target = &n.kids[0];
        match target.head {
            Head::Ptuple
            | Head::Plist
            | Head::Pmap
            | Head::Ptag
            | Head::Palt
            | Head::Plit
            | Head::Pnil
            | Head::Psym
            | Head::Pwild
            | Head::Pbind => {
                let idx = self.add_pattern(target)?;
                self.proto.emit(Op::MatchOrFail, idx, target.pos);
                Ok(())
            }
            _ => self.store_ref(target),
        }
    }

    /// Emits the store into a resolved `(local n)` or `(global n)`.
    fn store_ref(&mut self, target: &Node) -> Result<(), Diag> {
        let op = match target.head {
            Head::Local => Op::StoreLocal,
            Head::Global => Op::StoreGlobal,
            h => {
                return Err(internal(
                    target.pos,
                    format!("compiler: unresolved assignment target {}", h.as_str()),
                ));
            }
        };
        self.proto.emit(op, lit_index(target), target.pos);
        Ok(())
    }

    /// Compiles `(import key binding (member name binding)…)`: the module
    /// value comes from IMPORT (cached after the first run, 04) and is
    /// stored into the module name and each listed member.
    fn import_stmt(&mut self, n: &Node) -> Result<(), Diag> {
        let key = self.proto.add_const(Const::Str(lit_str(n).to_string()));
        if n.kids[0].head != Head::Nil {
            self.proto.emit(Op::Import, key, n.pos);
            self.store_ref(&n.kids[0])?;
        }
        for m in &n.kids[1..] {
            self.proto.emit(Op::Import, key, m.pos);
            let name = self.proto.add_const(Const::Str(lit_str(m).to_string()));
            self.proto.emit(Op::Field, name, m.pos);
            self.store_ref(&m.kids[0])?;
        }
        Ok(())
    }

    /// Compiles an expression; `tail` says whether its value is what the
    /// enclosing function returns.
    fn expr(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        let pos = n.pos;
        match n.head {
            Head::Do => return self.block(n, tail),
            Head::Int | Head::Float | Head::Bool | Head::Nil => {
                let c = match n.lit {
                    Lit::Int(i) => Const::Int(i),
                    Lit::Float(f) => Const::Float(f),
                    Lit::Bool(b) => Const::Bool(b),
                    _ => Const::Nil,
                };
                self.emit_const(c, pos);
            }
            Head::String => self.emit_const(str_const(n), pos),
            Head::Symbol => self.emit_const(Const::Sym(lit_str(n).to_string()), pos),
            Head::Duration => {
                let ns = if let Lit::Dur(d) = n.lit { d } else { 0 };
                self.emit_const(Const::Dur(ns), pos);
            }
            Head::Local => {
                let slot = lit_index(n);
                // A copy leaves the value held; a move empties the slot on
                // this path, so nothing is left to let go.
                let op = if self.moves.contains(&(n as *const Node)) {
                    self.held.remove(&slot);
                    Op::MoveLocal
                } else {
                    Op::LoadLocal
                };
                self.proto.emit(op, slot, pos);
            }
            Head::Global => _ = self.proto.emit(Op::LoadGlobal, lit_index(n), pos),
            Head::Captured => _ = self.proto.emit(Op::LoadCaptured, lit_index(n), pos),
            Head::SelfFn => _ = self.proto.emit(Op::LoadSelf, 0, pos),
            Head::Prelude => {
                let name = lit_str(n);
                if !self.prelude.contains(name) {
                    return Err(internal(
                        pos,
                        format!("compiler: unknown prelude name {name}"),
                    ));
                }
                self.emit_const(prelude_const(name), pos);
            }
            Head::Op => {
                if let Some(c) = self.folds.get(&(n as *const Node)) {
                    self.emit_const(c.clone(), pos);
                } else {
                    return self.op(n);
                }
            }
            Head::If => return self.if_expr(n, tail),
            Head::Concat => {
                self.kids(n)?;
                self.proto.emit(Op::Concat, n.kids.len(), pos);
            }
            Head::List
            | Head::Map
            | Head::Tuple
            | Head::Tag
            | Head::Range
            | Head::Index
            | Head::Slice
            | Head::Field => return self.collection(n),
            Head::Unwrap => return self.unwrap(n),
            Head::Coalesce => return self.coalesce(n, tail),
            Head::Match => return self.match_expr(n, tail),
            Head::Call => return self.call(n, tail),
            Head::Fn => {
                let idx = self.function(n)?;
                self.proto.emit(Op::Closure, idx, pos);
            }
            Head::Return => return self.return_expr(n),
            // Assignment is a statement (01); it cannot appear in
            // expression position.
            Head::Let => return Err(internal(pos, "compiler: let outside a block".into())),
            h => {
                return Err(internal(
                    pos,
                    format!("compiler: unknown core form {}", h.as_str()),
                ));
            }
        }
        Ok(())
    }

    /// `(op name a [b])`: the operands, then the operator. A missing table
    /// entry must be an error, not a wrong opcode: later milestones add
    /// operators to the parser first.
    fn op(&mut self, n: &Node) -> Result<(), Diag> {
        let name = lit_str(n);
        // A value that is a bool already needs no check (VM10 d).
        if name == "bool" && makes_bool(&n.kids[0]) {
            return self.expr(&n.kids[0], false);
        }
        self.kids(n)?;
        if name == "in" {
            self.proto.emit(Op::In, 0, n.pos);
            return Ok(());
        }
        let op = if n.kids.len() == 2 {
            binary_op(name)
        } else {
            unary_op(name)
        };
        let Some(op) = op else {
            return Err(internal(
                n.pos,
                format!("compiler: unknown operator {name}"),
            ));
        };
        self.proto.emit(op, 0, n.pos);
        Ok(())
    }

    fn if_expr(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        let mut to_else = Vec::new();
        self.cond(&n.kids[0], &mut to_else)?;
        let mut to_end = None;
        self.branches(2, |c, i| {
            if i == 0 {
                c.expr(&n.kids[1], tail)?;
                to_end = c.end_branch(tail, n.pos);
                c.patch_all(&to_else);
            } else {
                c.expr(&n.kids[2], tail)?;
            }
            Ok(())
        })?;
        if let Some(j) = to_end {
            self.proto.patch(j, self.proto.code.len());
        }
        Ok(())
    }

    /// A statement `if` without `else` (VM10 e): its value is discarded,
    /// so the branch taken discards its own value and the other makes
    /// none. The discard reports an Err where the statement starts, as a
    /// statement's POP does.
    fn if_stmt(&mut self, k: &Node) -> Result<(), Diag> {
        let mut to_end = Vec::new();
        self.cond(&k.kids[0], &mut to_end)?;
        let pos = self.start_pos(k);
        self.branches(2, |c, i| {
            if i == 0 {
                c.expr(&k.kids[1], false)?;
                c.proto.emit(Op::Pop, 0, pos);
            }
            Ok(())
        })?;
        self.patch_all(&to_end);
        Ok(())
    }

    /// Compiles condition `n` as jumps (VM10 d): it falls through when
    /// true, and each jump it pushes on `fails` goes where false goes. An
    /// `and` or `or` jumps straight to its target instead of making a
    /// bool to test again; JUMPIFFALSE checks the type as CHECKBOOL would,
    /// at the same place.
    fn cond(&mut self, n: &Node, fails: &mut Vec<usize>) -> Result<(), Diag> {
        match logic(n) {
            Some(Logic::And(a, b)) => {
                self.cond(a, fails)?;
                // b runs on one path only: what it lets go is still held
                // on the other (`branches`).
                let before = self.held.clone();
                self.cond(b, fails)?;
                self.held.extend(before);
            }
            Some(Logic::Or(a, b)) => {
                let mut to_b = Vec::new();
                self.cond(a, &mut to_b)?;
                let to_true = self.proto.emit(Op::Jump, 0, n.pos);
                self.patch_all(&to_b);
                let before = self.held.clone();
                self.cond(b, fails)?;
                self.held.extend(before);
                self.proto.patch(to_true, self.proto.code.len());
            }
            None if n.head == Head::Op && lit_str(n) == "bool" => self.cond(&n.kids[0], fails)?,
            None => {
                self.expr(n, false)?;
                // The jump carries the condition's position so a non-bool
                // condition is reported where the condition is.
                fails.push(self.proto.emit(Op::JumpIfFalse, 0, n.pos));
            }
        }
        Ok(())
    }

    /// Points every jump in `jumps` here.
    pub(super) fn patch_all(&mut self, jumps: &[usize]) {
        let here = self.proto.code.len();
        for &j in jumps {
            self.proto.patch(j, here);
        }
    }

    /// Ends a branch whose value is the expression's. In tail position the
    /// value is what the function returns, so the branch returns there
    /// (VM10 f); otherwise it jumps past the other branches: that jump is
    /// returned for patching.
    pub(super) fn end_branch(&mut self, tail: bool, pos: Pos) -> Option<usize> {
        if tail {
            self.end_tail(self.ret_pos);
            return None;
        }
        Some(self.proto.emit(Op::Jump, 0, pos))
    }

    /// Ends code in tail position: returns its value, unless it ended in a
    /// TAILCALL or RETURN already (VM17). Such a call ends the frame by
    /// itself, and nothing in a tail position jumps to its end (every
    /// branch of a choice there returns by itself), so nothing after it
    /// could run.
    pub(super) fn end_tail(&mut self, pos: Pos) {
        let done = self
            .proto
            .code
            .last()
            .is_some_and(|i| matches!(i.op(), Op::TailCall | Op::Return));
        if !done {
            self.emit_return(pos);
        }
    }

    /// Returns the value on the stack, checked against the return type.
    pub(super) fn emit_return(&mut self, pos: Pos) {
        if self.has_ret && self.proto.return_type >= 0 {
            let rt = self.proto.return_type as usize;
            self.proto.emit(Op::CheckReturn, rt, pos);
        }
        self.proto.emit(Op::Return, 0, pos);
    }

    /// The returns of `?` (VM10 f): UNWRAPTEST jumps to these after the
    /// function's own end, so its success path goes straight on. Each has
    /// its `?`'s place.
    fn emit_stubs(&mut self) {
        for (at, pos) in std::mem::take(&mut self.stubs) {
            self.proto.patch(at, self.proto.code.len());
            self.emit_return(pos);
        }
    }

    fn return_expr(&mut self, n: &Node) -> Result<(), Diag> {
        let outer = std::mem::replace(&mut self.ret_pos, n.pos);
        self.expr(&n.kids[0], true)?;
        self.ret_pos = outer;
        self.end_tail(n.pos);
        Ok(())
    }
}
