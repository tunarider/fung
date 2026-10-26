//! Collections, `!` `?` `??`, and `match`.

use super::{Compiler, lit_str};
use crate::bytecode::Op;
use crate::consts::Const;
use crate::diag::Diag;
use crate::tree::{Head, Lit, Node};

impl Compiler<'_> {
    /// Compiles every child as a non-tail expression.
    pub(super) fn kids(&mut self, n: &Node) -> Result<(), Diag> {
        for k in &n.kids {
            self.expr(k, false)?;
        }
        Ok(())
    }

    /// Compiles list/map/tuple/tag/range/index/slice/field forms.
    pub(super) fn collection(&mut self, n: &Node) -> Result<(), Diag> {
        let pos = n.pos;
        match n.head {
            Head::List => {
                // A list of constants is one constant (P7): values are
                // immutable, so every evaluation can share it.
                if let Some(c) = constant_list(n) {
                    self.emit_const(c, pos);
                    return Ok(());
                }
                self.kids(n)?;
                self.proto.emit(Op::List, n.kids.len(), pos);
            }
            Head::Map => {
                for entry in &n.kids {
                    self.emit_const(Const::Str(lit_str(entry).to_string()), entry.pos);
                    self.expr(&entry.kids[0], false)?;
                }
                self.proto.emit(Op::Map, n.kids.len(), pos);
            }
            Head::Tuple => {
                self.kids(n)?;
                self.proto.emit(Op::Tuple, n.kids.len(), pos);
            }
            Head::Tag => {
                self.emit_const(Const::Sym(lit_str(n).to_string()), pos);
                self.kids(n)?;
                self.proto.emit(Op::Tag, n.kids.len(), pos);
            }
            Head::Range => {
                self.kids(n)?;
                let inclusive = usize::from(lit_str(n) == "..=");
                self.proto.emit(Op::Range, inclusive, pos);
            }
            Head::Index => {
                self.kids(n)?;
                self.proto.emit(Op::Index, 0, pos);
            }
            Head::Slice => {
                self.expr(&n.kids[0], false)?;
                let mut flags = 0;
                if n.kids[1].head != Head::Nil {
                    flags |= 1;
                    self.expr(&n.kids[1], false)?;
                }
                if n.kids[2].head != Head::Nil {
                    flags |= 2;
                    self.expr(&n.kids[2], false)?;
                }
                if lit_str(n) == "..=" {
                    flags |= 4;
                }
                self.proto.emit(Op::Slice, flags, pos);
            }
            Head::Field => {
                self.expr(&n.kids[0], false)?;
                let name = self.proto.add_const(Const::Str(lit_str(n).to_string()));
                self.proto.emit(Op::Field, name, pos);
            }
            _ => unreachable!("collection called with {}", n.head.as_str()),
        }
        Ok(())
    }

    /// Compiles `!` and `?` (02). `?` shares the function's return path:
    /// UNWRAPTEST leaves the Err on the stack and jumps to a RETURN.
    pub(super) fn unwrap(&mut self, n: &Node) -> Result<(), Diag> {
        self.expr(&n.kids[0], false)?;
        let p = &mut self.proto;
        if lit_str(n) == "!" {
            p.emit(Op::UnwrapAbort, 0, n.pos);
            return Ok(());
        }
        let to_return = p.emit(Op::UnwrapTest, 0, n.pos);
        let to_cont = p.emit(Op::Jump, 0, n.pos);
        p.patch(to_return, p.code.len());
        if self.has_ret && p.return_type >= 0 {
            let rt = p.return_type as usize;
            p.emit(Op::CheckReturn, rt, n.pos);
        }
        p.emit(Op::Return, 0, n.pos);
        p.patch(to_cont, p.code.len());
        Ok(())
    }

    /// Compiles `a ?? b`: COALESCE jumps to b when a is nil or Err.
    pub(super) fn coalesce(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        self.expr(&n.kids[0], false)?;
        let to_rhs = self.proto.emit(Op::Coalesce, 0, n.pos);
        let to_end = self.proto.emit(Op::Jump, 0, n.pos);
        self.proto.patch(to_rhs, self.proto.code.len());
        self.expr(&n.kids[1], tail)?;
        self.proto.patch(to_end, self.proto.code.len());
        Ok(())
    }

    /// Compiles `(match subject (arm pat guard body)…)`: each arm loads the
    /// subject, tries its pattern, checks its guard, and runs its body in
    /// tail position; falling off the end is E0601 (09). A guarded arm
    /// with alternatives tries the pattern and the guard once per
    /// alternative (`pattern::alternatives`, 03) before the next arm; its
    /// body is compiled once.
    pub(super) fn match_expr(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        let subject = &n.kids[0];
        let mut ends = Vec::new();
        for arm in &n.kids[1..] {
            let (pat, guard, body) = (&arm.kids[0], &arm.kids[1], &arm.kids[2]);
            let tries = if guard.head == Head::Nil {
                vec![pat.clone()]
            } else {
                super::pattern::alternatives(pat)?
            };
            let mut fails = Vec::new();
            let mut taken = Vec::new();
            for (i, alt) in tries.iter().enumerate() {
                let here = self.proto.code.len();
                for f in fails.drain(..) {
                    self.proto.patch(f, here);
                }
                self.expr(subject, false)?;
                let idx = self.add_pattern(alt)?;
                self.proto.emit(Op::Match, idx, alt.pos);
                fails.push(self.proto.emit(Op::JumpIfFalse, 0, alt.pos));
                if guard.head != Head::Nil {
                    self.expr(guard, false)?;
                    fails.push(self.proto.emit(Op::JumpIfFalse, 0, guard.pos));
                }
                if i + 1 < tries.len() {
                    taken.push(self.proto.emit(Op::Jump, 0, arm.pos));
                }
            }
            let start = self.proto.code.len();
            for t in taken {
                self.proto.patch(t, start);
            }
            self.expr(body, tail)?;
            ends.push(self.proto.emit(Op::Jump, 0, arm.pos));
            let here = self.proto.code.len();
            for f in fails {
                self.proto.patch(f, here);
            }
        }
        self.expr(subject, false)?;
        self.proto.emit(Op::NoMatch, 0, n.pos);
        let here = self.proto.code.len();
        for e in ends {
            self.proto.patch(e, here);
        }
        Ok(())
    }
}

/// The constant a non-empty list literal of constants stands for (P7).
/// An empty list is left to LIST: the runtime reads an empty list
/// constant as the prelude's `args` (`fung_vm::vm::link`).
fn constant_list(n: &Node) -> Option<Const> {
    if n.head != Head::List || n.kids.is_empty() {
        return None;
    }
    let mut items = Vec::with_capacity(n.kids.len());
    for k in &n.kids {
        items.push(match (k.head, &k.lit) {
            (Head::Int, Lit::Int(i)) => Const::Int(*i),
            (Head::Float, Lit::Float(f)) => Const::Float(*f),
            (Head::Bool, Lit::Bool(b)) => Const::Bool(*b),
            (Head::Nil, _) => Const::Nil,
            (Head::String, Lit::Str(s)) => Const::Str(s.clone()),
            (Head::Symbol, Lit::Str(s)) => Const::Sym(s.clone()),
            (Head::Duration, Lit::Dur(d)) => Const::Dur(*d),
            (Head::List, _) => constant_list(k)?,
            _ => return None,
        });
    }
    Some(Const::List(items))
}
