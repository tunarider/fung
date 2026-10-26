//! Calls and function bodies (09. 호출 규약).

use super::{Compiler, lit_str};
use crate::bytecode::{Op, Param, Proto, encode_call};
use crate::consts::Const;
use crate::diag::{Arg, Diag, error};
use crate::tree::{Head, Lit, Node};

/// What the 12-bit CALL operand fields and CHECKTYPE packing hold.
const MAX_ARGS: usize = 4095;

fn too_many(n: &Node, what: &str) -> Diag {
    error(
        "E0230",
        n.pos,
        vec![Arg::Str(what.into()), Arg::Int(MAX_ARGS as i64)],
    )
}

impl Compiler<'_> {
    /// Compiles `(call f pos… (named k e)…)`: callee, positional arguments,
    /// then each named argument as its symbol followed by its value, then
    /// CALL — or TAILCALL in tail position.
    pub(super) fn call(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        self.expr(&n.kids[0], false)?;
        if self.is_dbg_call(n) {
            return self.dbg_args(n);
        }
        let (mut positional, mut named) = (0, 0);
        for k in &n.kids[1..] {
            if k.head == Head::Named {
                named += 1;
                self.emit_const(Const::Sym(lit_str(k).to_string()), k.pos);
                self.expr(&k.kids[0], false)?;
                continue;
            }
            positional += 1;
            self.expr(k, false)?;
        }
        if positional > MAX_ARGS || named > MAX_ARGS {
            return Err(too_many(n, "arguments"));
        }
        // A function with a return type keeps its frame to check the value,
        // except for a tail call to itself: the same check runs where its
        // self calls end (03, T4).
        let tail = tail && (!self.has_ret || n.kids[0].head == Head::SelfFn);
        let op = if tail { Op::TailCall } else { Op::Call };
        self.proto.emit(op, encode_call(positional, named), n.pos);
        Ok(())
    }

    /// `dbg(e)` also receives e's source text and place (08).
    fn dbg_args(&mut self, n: &Node) -> Result<(), Diag> {
        let arg = &n.kids[1];
        self.expr(arg, false)?;
        self.emit_const(Const::Str(arg.text(&self.opts.source).to_string()), arg.pos);
        let place = format!("{}:{}", self.opts.file, n.pos.line);
        self.emit_const(Const::Str(place), n.pos);
        self.proto.emit(Op::Call, encode_call(3, 0), n.pos);
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
        if info.params.len() > MAX_ARGS {
            return Err(too_many(n, "parameters"));
        }
        let mut child = Proto {
            name: info.name.clone(),
            num_locals: info.num_locals,
            captures: info.captures.clone(),
            ..Proto::default()
        };
        for prm in &info.params {
            let ty = match &prm.types {
                Some(t) => child.add_type(t.clone()) as i32,
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
            child.return_type = child.add_type(r.clone()) as i32;
        }
        let mut cc = Compiler {
            has_ret: child.return_type >= 0,
            proto: child,
            prelude: self.prelude,
            opts: self.opts,
            lines: self.lines,
            body_block: false,
            moves: super::moves::last_reads(n.kids.last().expect("fn has a body")),
        };

        // Prologue: defaults, then type checks.
        let mut di = 0;
        for i in 0..cc.proto.params.len() {
            if !cc.proto.params[i].has_default {
                continue;
            }
            cc.proto.emit(Op::LoadLocal, i, n.pos);
            let skip = cc.proto.emit(Op::JumpIfSet, 0, n.pos);
            cc.expr(&n.kids[di], false)?;
            di += 1;
            cc.proto.emit(Op::StoreLocal, i, n.pos);
            cc.proto.patch(skip, cc.proto.code.len());
        }
        for i in 0..cc.proto.params.len() {
            let ty = cc.proto.params[i].ty;
            if ty >= 0 {
                cc.proto
                    .emit(Op::CheckType, i | (ty as usize) << 12, info.params[i].pos);
            }
        }

        let body = n.kids.last().expect("fn has a body");
        cc.body_block = true;
        cc.block(body, true)?;
        if cc.has_ret {
            let rt = cc.proto.return_type as usize;
            cc.proto.emit(Op::CheckReturn, rt, body.pos);
        }
        cc.proto.emit(Op::Return, 0, body.pos);
        self.proto.protos.push(cc.proto);
        Ok(self.proto.protos.len() - 1)
    }
}
