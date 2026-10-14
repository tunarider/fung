//! Collections, `!` `?` `??`, and `match`.

use super::{Compiler, lit_str};
use crate::bytecode::Op;
use crate::consts::Const;
use crate::diag::Diag;
use crate::tree::{Head, Node};

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
    /// tail position; falling off the end is E0601 (09).
    pub(super) fn match_expr(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        let subject = &n.kids[0];
        let mut ends = Vec::new();
        for arm in &n.kids[1..] {
            let (pat, guard, body) = (&arm.kids[0], &arm.kids[1], &arm.kids[2]);
            self.expr(subject, false)?;
            let idx = self.add_pattern(pat)?;
            self.proto.emit(Op::Match, idx, pat.pos);
            let mut fails = vec![self.proto.emit(Op::JumpIfFalse, 0, pat.pos)];
            if guard.head != Head::Nil {
                self.expr(guard, false)?;
                fails.push(self.proto.emit(Op::JumpIfFalse, 0, guard.pos));
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
