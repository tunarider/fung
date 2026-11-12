//! Collections, `!` `?` `??`, and `match`.

use super::{Compiler, lit_str, str_const};
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
        // A list, map, tuple or tag value of constants is one constant
        // (P7, L5): values are immutable, so every evaluation can share it.
        if matches!(
            n.head,
            Head::List | Head::Map | Head::Tuple | Head::Tag | Head::Range
        ) && let Some(c) = self.folds.get(&n.id)
        {
            self.emit_const(c.clone(), pos);
            return Ok(());
        }
        match n.head {
            Head::List => {
                self.kids(n)?;
                self.proto.emit(Op::List, n.kids.len(), pos);
            }
            // The keys are one constant, the MAP's second operand (VM30).
            Head::Map => {
                let keys = n.kids.iter().map(str_const).collect();
                let keys = self.proto.add_const(Const::List(keys));
                for entry in &n.kids {
                    self.expr(&entry.kids[0], false)?;
                }
                self.proto.emit_with(Op::Map, n.kids.len(), keys, pos);
            }
            Head::Tuple => {
                self.kids(n)?;
                self.proto.emit(Op::Tuple, n.kids.len(), pos);
            }
            // The name is the TAG's second operand (VM30).
            Head::Tag => {
                let name = self.proto.add_const(Const::Sym(lit_str(n).to_string()));
                self.kids(n)?;
                self.proto.emit_with(Op::Tag, n.kids.len(), name, pos);
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
            // A missing end (parsed as `missing`, not `nil`) is open; a
            // written `nil` is compiled as a value (VA3).
            Head::Slice => {
                self.expr(&n.kids[0], false)?;
                let mut flags = 0;
                if n.kids[1].head != Head::Missing {
                    flags |= 1;
                    self.expr(&n.kids[1], false)?;
                }
                if n.kids[2].head != Head::Missing {
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

    /// Compiles `!` and `?` (02). On an Err, `?`'s UNWRAPTEST leaves it on
    /// the stack and jumps to a RETURN after the function's end
    /// (`emit_stubs`, VM10 f): the success path goes straight on.
    pub(super) fn unwrap(&mut self, n: &Node) -> Result<(), Diag> {
        self.expr(&n.kids[0], false)?;
        if lit_str(n) == "!" {
            self.proto.emit(Op::UnwrapAbort, 0, n.pos);
            return Ok(());
        }
        let to_return = self.proto.emit(Op::UnwrapTest, 0, n.pos);
        self.stubs.push((to_return, n.pos));
        Ok(())
    }

    /// Compiles `a ?? b`: COALESCE goes on with a's value (an Ok's
    /// payload) past b, and falls into b when a is nil or Err, so the
    /// common path takes no other jump (VM26). Each path lets go of what
    /// only the other reads as it starts (VM19); when a's path has some
    /// to let go of, they come after b, which jumps over them.
    pub(super) fn coalesce(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        self.expr(&n.kids[0], false)?;
        let to_left = self.proto.emit(Op::Coalesce, 0, n.pos);
        self.enter(&n.kids[1]);
        self.expr(&n.kids[1], tail)?;
        let left_clears = self
            .lives
            .entry
            .get(&n.kids[0].id)
            .is_some_and(|c| !c.is_empty());
        if tail {
            self.end_tail(self.ret_pos);
        } else if !left_clears {
            self.proto.patch(to_left, self.proto.code.len());
            return Ok(());
        }
        let to_end = if tail {
            None
        } else {
            self.end_branch(false, n.pos)
        };
        self.proto.patch(to_left, self.proto.code.len());
        self.enter_at(n.kids[0].id, n.pos);
        if tail {
            // Its own return: the code before it is b's, which may end
            // in one already.
            self.emit_return(self.ret_pos);
        }
        if let Some(j) = to_end {
            self.proto.patch(j, self.proto.code.len());
        }
        Ok(())
    }

    /// Compiles `(match subject (arm pat guard body)…)`: each arm loads the
    /// subject, tries its pattern, checks its guard, and runs its body in
    /// tail position; falling off the end is E0601 (09). A guarded arm
    /// with alternatives tries the pattern and the guard once per
    /// alternative (`pattern::alternatives`, 03) before the next arm; its
    /// body is compiled once. An arm that always matches (`_` or a name,
    /// no guard) needs no test, and nothing after it can run: the arms
    /// after it and the E0601 are left out (VM10 c).
    pub(super) fn match_expr(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        let subject = &n.kids[0];
        let arms = &n.kids[1..];
        let last = arms.iter().position(always_matches);
        let count = last.map_or(arms.len(), |i| i + 1);
        let mut ends = Vec::new();
        for arm in &arms[..count] {
            self.match_arm(subject, arm, tail, &mut ends)?;
        }
        if last.is_none() {
            self.expr(subject, false)?;
            self.proto.emit(Op::NoMatch, 0, n.pos);
        }
        self.patch_all(&ends);
        Ok(())
    }

    /// One arm of `match_expr`: its tries, its body, and the jump past the
    /// other arms (pushed on `ends`), or its return in tail position. Once
    /// the arm is taken, its body lets go of what it does not read (the
    /// subject, what only the guard or other arms read; `moves`, VM19): what
    /// the pattern bound is then the only holder of the value, which a
    /// callee may change in place. The last arm, a name, takes a subject
    /// nothing reads after (MOVELOCAL).
    fn match_arm(
        &mut self,
        subject: &Node,
        arm: &Node,
        tail: bool,
        ends: &mut Vec<usize>,
    ) -> Result<(), Diag> {
        let (pat, guard, body) = (&arm.kids[0], &arm.kids[1], &arm.kids[2]);
        if always_matches(arm) {
            if pat.head == Head::Pbind {
                if self.lives.subject_moves.contains(&arm.id) {
                    self.proto
                        .emit(Op::MoveLocal, super::lit_index(subject), subject.pos);
                } else {
                    self.expr(subject, false)?;
                }
                self.store_ref(&pat.kids[0])?;
            }
            self.enter(body);
            // Nothing follows this arm in the match: the body's value is
            // the match's, and a body in tail position returns by itself
            // where the match ends.
            return self.expr(body, tail);
        }
        let mut fails = Vec::new();
        let mut taken = Vec::new();
        let tries = if matches!(pat.head, Head::Pwild | Head::Pbind) {
            // A name or `_` with a guard: the pattern always matches, so
            // only the guard is tested (VM17).
            if pat.head == Head::Pbind {
                self.expr(subject, false)?;
                self.store_ref(&pat.kids[0])?;
            }
            self.cond(guard, &mut fails)?;
            Vec::new()
        } else if guard.head == Head::Nil {
            vec![pat.clone()]
        } else {
            super::pattern::alternatives(pat)?
        };
        for (i, alt) in tries.iter().enumerate() {
            self.patch_all(&fails);
            fails.clear();
            self.expr(subject, false)?;
            let idx = self.add_pattern(alt)?;
            // MATCH goes to the next try itself when the pattern fails
            // (VM29).
            fails.push(self.proto.emit_with(Op::Match, 0, idx, alt.pos));
            if guard.head != Head::Nil {
                self.cond(guard, &mut fails)?;
            }
            if i + 1 < tries.len() {
                taken.push(self.proto.emit(Op::Jump, 0, arm.pos));
            }
        }
        self.patch_all(&taken);
        self.enter(body);
        self.expr(body, tail)?;
        if let Some(e) = self.end_branch(tail, arm.pos) {
            ends.push(e);
        }
        self.patch_all(&fails);
        Ok(())
    }
}

/// An arm that matches every value: `_` or a name, without a guard.
pub(super) fn always_matches(arm: &Node) -> bool {
    arm.kids[1].head == Head::Nil && matches!(arm.kids[0].head, Head::Pwild | Head::Pbind)
}
