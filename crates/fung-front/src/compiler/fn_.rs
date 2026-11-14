//! Calls and function bodies (09. 호출 규약).

use super::{Compiler, lit_str};
use crate::bytecode::{Op, Param, Proto};
use crate::consts::Const;
use crate::diag::Diag;
use crate::tree::{Head, Lit, Node};

impl Compiler<'_> {
    /// Compiles `(call f pos… (named k e)…)`: callee, positional arguments,
    /// then each named argument as its symbol followed by its value, then
    /// CALL — or TAILCALL in tail position. A spread `...xs` (the last
    /// positional) is its list, which CALLSPREAD lays out (D45).
    pub(super) fn call(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        self.call_checked(n, tail, None)
    }

    /// `call`, for a tail call whose value must be a bool (`bool_at`: the
    /// place of the `and`/`or` it is the right side of, D97): the frame
    /// takes that check along, as it takes a return type's.
    pub(super) fn call_checked(
        &mut self,
        n: &Node,
        tail: bool,
        bool_at: Option<crate::source::Pos>,
    ) -> Result<(), Diag> {
        if self.is_dbg_call(n) {
            return self.dbg_args(n);
        }
        self.expr(&n.kids[0], false)?;
        let (mut positional, mut named, mut spread) = (0, 0, false);
        for k in &n.kids[1..] {
            if k.head == Head::Named {
                named += 1;
                self.emit_const(Const::Sym(lit_str(k).to_string()), k.pos);
                self.expr(&k.kids[0], false)?;
                continue;
            }
            positional += 1;
            if k.head == Head::Spread {
                spread = true;
                self.expr(&k.kids[0], false)?;
                continue;
            }
            self.expr(k, false)?;
        }
        // A function with a return type reuses its frame for every tail
        // call too: the frame takes the check along, to run when it
        // returns (D44). A call to itself needs none: its own check runs
        // where its self calls end (03). The checks run the latest first,
        // and the bool check of an `and`/`or` comes before the return
        // type's, as when the value is named first (VM49): it goes last.
        if tail && self.proto.return_type >= 0 && n.kids[0].head != Head::SelfFn {
            let rt = self.proto.return_type as usize;
            self.proto.emit(Op::TailCheck, rt, self.ret_pos);
        }
        if let Some(at) = bool_at {
            self.proto.emit(Op::TailCheckBool, 0, at);
        }
        let op = match (tail, spread) {
            (false, false) => Op::Call,
            (true, false) => Op::TailCall,
            (false, true) => Op::CallSpread,
            (true, true) => Op::TailCallSpread,
        };
        // A count past an operand's 24 bits is E0230 (`check_operands`),
        // the limit of every count (VM43).
        self.proto.emit_with(op, positional, named, n.pos);
        Ok(())
    }

    /// `dbg(e)` calls the `dbg` that also takes e's source text and place
    /// (08), which a program cannot pass itself (SP11).
    fn dbg_args(&mut self, n: &Node) -> Result<(), Diag> {
        self.emit_const(Const::Dbg, n.kids[0].pos);
        let arg = &n.kids[1];
        self.expr(arg, false)?;
        self.emit_const(Const::Str(arg.text(self.opts.source).to_string()), arg.pos);
        let place = format!("{}:{}", self.opts.file, n.pos.line);
        self.emit_const(Const::Str(place), n.pos);
        self.proto.emit(Op::Call, 3, n.pos);
        Ok(())
    }

    /// Compiles an `(fn …)` node into a child proto and returns its index
    /// in the current proto's `protos`. The prologue fills defaults for
    /// unassigned slots (JUMPIFSET sees the empty slot the VM leaves) and
    /// checks parameter types; the body's last value is checked against
    /// the return type when there is one.
    pub(super) fn function(&mut self, n: &Node) -> Result<usize, Diag> {
        let Lit::Fn(info) = &n.lit else {
            return Err(super::internal(n.pos, "compiler: fn without FnInfo".into()));
        };
        let mut child = Proto {
            name: info.name.clone(),
            num_locals: info.num_locals,
            captures: info.captures.clone(),
            ..Proto::default()
        };
        for prm in &info.params {
            let ty = match &prm.types {
                Some(t) => child.add_type(type_names(t)) as i32,
                None => -1,
            };
            child.params.push(Param {
                name: prm.name.clone(),
                has_default: prm.has_default,
                rest: prm.rest,
                ty,
            });
        }
        if let Some(r) = &info.ret {
            child.return_type = child.add_type(type_names(r)) as i32;
        }
        let mut cc = Compiler {
            proto: child,
            prelude: self.prelude,
            opts: self.opts,
            lines: self.lines,
            folds: self.folds,
            lives: fn_lives(n, info),
            stubs: Vec::new(),
            ret_pos: n.kids.last().expect("fn has a body").pos,
        };

        // Prologue: defaults, then type checks.
        let mut di = 0;
        for i in 0..cc.proto.params.len() {
            if !cc.proto.params[i].has_default {
                continue;
            }
            let skip = cc.proto.emit_with(Op::JumpIfSet, 0, i, n.pos);
            cc.enter(&n.kids[di]);
            cc.expr(&n.kids[di], false)?;
            di += 1;
            cc.proto.emit(Op::StoreLocal, i, n.pos);
            cc.proto.patch(skip, cc.proto.code.len());
        }
        cc.proto.body = cc.proto.code.len();
        for i in 0..cc.proto.params.len() {
            let ty = cc.proto.params[i].ty;
            if ty >= 0 {
                cc.proto
                    .emit_with(Op::CheckType, i, ty as usize, info.params[i].pos);
            }
        }

        // What the defaults read last lets go here, once the types are
        // checked: a call that gave every argument skips the defaults, so
        // this is the first place both paths pass (VM39, VM48).
        let at_body = std::mem::take(&mut cc.lives.at_body);
        for slot in at_body {
            cc.proto.emit(Op::ClearLocal, slot, n.pos);
        }
        let body = n.kids.last().expect("fn has a body");
        cc.block(body, true)?;
        cc.end_tail(body.pos);
        cc.emit_stubs();
        cc.proto.thread_jumps();
        cc.proto.check_operands()?;
        self.proto.protos.push(cc.proto);
        Ok(self.proto.protos.len() - 1)
    }
}

/// The lifetimes in function `n`: its defaults, its type checks and its
/// body (`moves::fn_lives`).
fn fn_lives(n: &Node, info: &crate::tree::FnInfo) -> super::moves::Lives {
    let mut defaults = Vec::new();
    let mut typed = Vec::new();
    for (i, prm) in info.params.iter().enumerate() {
        if prm.has_default {
            defaults.push((i, &n.kids[defaults.len()]));
        }
        if prm.types.is_some() {
            typed.push(i);
        }
    }
    let body = n.kids.last().expect("fn has a body");
    super::moves::fn_lives(&defaults, &typed, body)
}

/// The names a resolved annotation checks (aliases expanded, element
/// types dropped by resolve).
fn type_names(t: &[crate::tree::TypeTerm]) -> Vec<String> {
    t.iter().map(|t| t.name.clone()).collect()
}
