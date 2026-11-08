//! Collections, `!` `?` `??`, and `match`.

use super::{Compiler, lit_str, str_const};
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
        // A list, map, tuple or tag value of constants is one constant
        // (P7, L5): values are immutable, so every evaluation can share it.
        if matches!(
            n.head,
            Head::List | Head::Map | Head::Tuple | Head::Tag | Head::Range
        ) && let Some(c) = constant(n)
        {
            self.emit_const(c, pos);
            return Ok(());
        }
        match n.head {
            Head::List => {
                self.kids(n)?;
                self.proto.emit(Op::List, n.kids.len(), pos);
            }
            Head::Map => {
                for entry in &n.kids {
                    self.emit_const(str_const(entry), entry.pos);
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

    /// Compiles `a ?? b`: COALESCE jumps to b when a is nil or Err.
    pub(super) fn coalesce(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        self.expr(&n.kids[0], false)?;
        let to_rhs = self.proto.emit(Op::Coalesce, 0, n.pos);
        let to_end = self.end_branch(tail, n.pos);
        self.proto.patch(to_rhs, self.proto.code.len());
        // The right side may not run: what it lets go is still held after.
        self.branches(2, |c, i| {
            if i == 0 {
                Ok(())
            } else {
                c.expr(&n.kids[1], tail)
            }
        })?;
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
        self.branches(count, |c, a| {
            c.match_arm(subject, &arms[a], tail, &mut ends)
        })?;
        if last.is_none() {
            self.expr(subject, false)?;
            self.proto.emit(Op::NoMatch, 0, n.pos);
        }
        self.patch_all(&ends);
        Ok(())
    }

    /// One arm of `match_expr`: its tries, its body, and the jump past the
    /// other arms (pushed on `ends`), or its return in tail position. Once
    /// the arm is taken, a subject in a local that nothing reads after is
    /// let go before a body that calls a function (03: 값의 수명): what the
    /// pattern bound is then the only holder of the value, which a callee
    /// may change in place.
    fn match_arm(
        &mut self,
        subject: &Node,
        arm: &Node,
        tail: bool,
        ends: &mut Vec<usize>,
    ) -> Result<(), Diag> {
        let (pat, guard, body) = (&arm.kids[0], &arm.kids[1], &arm.kids[2]);
        let done = self.subject_done(subject, pat);
        if always_matches(arm) {
            if pat.head == Head::Pbind {
                match done {
                    Some(slot) => {
                        self.held.remove(&slot);
                        self.proto.emit(Op::MoveLocal, slot, subject.pos);
                    }
                    None => self.expr(subject, false)?,
                }
                self.store_ref(&pat.kids[0])?;
            } else if let Some(slot) = done
                && has_call(body)
            {
                self.held.remove(&slot);
                self.proto.emit(Op::ClearLocal, slot, arm.pos);
            }
            self.stored(pat, pat);
            // Nothing follows this arm in the match: the body's value is
            // the match's, and a body in tail position returns by itself
            // where the match ends.
            return self.expr(body, tail);
        }
        let tries = if guard.head == Head::Nil {
            vec![pat.clone()]
        } else {
            super::pattern::alternatives(pat)?
        };
        let mut fails = Vec::new();
        let mut taken = Vec::new();
        for (i, alt) in tries.iter().enumerate() {
            self.patch_all(&fails);
            fails.clear();
            self.expr(subject, false)?;
            let idx = self.add_pattern(alt)?;
            self.proto.emit(Op::Match, idx, alt.pos);
            self.stored(pat, alt);
            fails.push(self.proto.emit(Op::JumpIfFalse, 0, alt.pos));
            if guard.head != Head::Nil {
                self.cond(guard, &mut fails)?;
            }
            if i + 1 < tries.len() {
                taken.push(self.proto.emit(Op::Jump, 0, arm.pos));
            }
        }
        self.patch_all(&taken);
        if let Some(slot) = done
            && has_call(body)
        {
            self.held.remove(&slot);
            self.proto.emit(Op::ClearLocal, slot, arm.pos);
        }
        self.expr(body, tail)?;
        if let Some(e) = self.end_branch(tail, arm.pos) {
            ends.push(e);
        }
        self.patch_all(&fails);
        Ok(())
    }

    /// The subject's slot when it is a local held now that nothing reads
    /// once arm pattern `pat` has matched: the arm lets go of it.
    fn subject_done(&self, subject: &Node, pat: &Node) -> Option<usize> {
        if subject.head != Head::Local {
            return None;
        }
        let slot = super::lit_index(subject);
        let later = self.stored_live.get(&(pat as *const Node))?;
        (self.held.contains(&slot) && !later.contains(&slot)).then_some(slot)
    }
}

/// Whether running `n` calls a function: only a call can make use of
/// the subject being let go (a callee changing a value in place, a wait
/// on a channel the subject holds), so a body without one does not let
/// it go before it runs.
fn has_call(n: &Node) -> bool {
    match n.head {
        Head::Call => true,
        Head::Fn => false,
        _ => n.kids.iter().any(has_call),
    }
}

/// An arm that matches every value: `_` or a name, without a guard.
fn always_matches(arm: &Node) -> bool {
    arm.kids[1].head == Head::Nil && matches!(arm.kids[0].head, Head::Pwild | Head::Pbind)
}

/// The value of a literal whose parts are all constants (L5): a scalar,
/// or a list, map, tuple or tag value of such literals, folded into one
/// constant so the VM loads it instead of building it each time.
/// `None` for anything else (names, calls, spreads, interpolation).
fn constant(n: &Node) -> Option<Const> {
    let all = |kids: &[Node]| kids.iter().map(constant).collect::<Option<Vec<_>>>();
    Some(match (n.head, &n.lit) {
        (Head::Int, Lit::Int(i)) => Const::Int(*i),
        (Head::Float, Lit::Float(f)) => Const::Float(*f),
        (Head::Bool, Lit::Bool(b)) => Const::Bool(*b),
        (Head::Nil, _) => Const::Nil,
        (Head::String, Lit::Str(s)) => Const::Str(s.clone()),
        (Head::String, Lit::Bytes(b)) => Const::Bytes(b.clone()),
        (Head::Symbol, Lit::Str(s)) => Const::Sym(s.clone()),
        (Head::Duration, Lit::Dur(d)) => Const::Dur(*d),
        (Head::List, _) => Const::List(all(&n.kids)?),
        (Head::Tuple, _) => Const::Tuple(all(&n.kids)?),
        (Head::Tag, Lit::Str(name)) => Const::Tag(name.clone(), all(&n.kids)?),
        (Head::Op, _) => super::fold::fold(n)?,
        (Head::Range, _) => match (constant(&n.kids[0])?, constant(&n.kids[1])?) {
            (Const::Int(a), Const::Int(b)) => Const::Range(a, b, lit_str(n) == "..="),
            _ => return None,
        },
        (Head::Map, _) => Const::Map(
            n.kids
                .iter()
                .map(|e| Some((str_const(e), constant(&e.kids[0])?)))
                .collect::<Option<_>>()?,
        ),
        _ => return None,
    })
}
