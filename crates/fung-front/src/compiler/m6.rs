//! `assert`, `test` blocks, and the `dbg` rewrite (08).

use super::{Compiler, lit_str};
use crate::bytecode::{Assert, Op};
use crate::consts::Const;
use crate::diag::Diag;
use crate::tree::{Head, Node};

fn is_comparison(op: &str) -> bool {
    matches!(op, "==" | "!=" | "<" | ">" | "<=" | ">=")
}

fn is_literal(h: Head) -> bool {
    matches!(
        h,
        Head::Int
            | Head::Float
            | Head::Bool
            | Head::Nil
            | Head::String
            | Head::Symbol
            | Head::Duration
    )
}

impl Compiler<'_> {
    /// Compiles `(assert e)`. A top-level comparison is split so the
    /// failure report can show each side's value (08); a literal side is
    /// not reported (its value is its text). Each side is evaluated once.
    pub(super) fn assert_stmt(&mut self, n: &Node) -> Result<(), Diag> {
        let e = &n.kids[0];
        let src = &self.opts.source;
        let mut a = Assert {
            text: e.text(src).to_string(),
            ..Assert::default()
        };
        if e.head == Head::Op && is_comparison(lit_str(e)) && e.kids.len() == 2 {
            a.op = lit_str(e).to_string();
            for (i, side) in e.kids.iter().enumerate() {
                self.expr(side, false)?;
                if !is_literal(side.head) {
                    let text = side.text(src).to_string();
                    if i == 0 {
                        a.left = text;
                    } else {
                        a.right = text;
                    }
                }
            }
        } else {
            self.expr(e, false)?;
        }
        self.proto.asserts.push(a);
        let idx = self.proto.asserts.len() - 1;
        self.proto.emit(Op::Assert, idx, n.pos);
        Ok(())
    }

    /// Compiles `(test name fn)`: with `tests` the function is made and
    /// registered on the runtime; without, the block compiles to nothing.
    pub(super) fn test_stmt(&mut self, n: &Node) -> Result<(), Diag> {
        if !self.opts.tests {
            return Ok(());
        }
        let idx = self.function(&n.kids[0])?;
        self.proto.emit(Op::Closure, idx, n.pos);
        let name = self.proto.add_const(Const::Str(lit_str(n).to_string()));
        self.proto.emit(Op::Test, name, n.pos);
        Ok(())
    }

    /// A direct `dbg(e)`: the prelude dbg with one positional argument and
    /// nothing else. Aliases and named arguments get the plain call.
    pub(super) fn is_dbg_call(&self, n: &Node) -> bool {
        let callee = &n.kids[0];
        if callee.head != Head::Prelude || lit_str(callee) != "dbg" || n.kids.len() != 2 {
            return false;
        }
        !matches!(n.kids[1].head, Head::Named | Head::Hole)
    }
}
