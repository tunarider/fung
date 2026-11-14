//! Names bound to literals (D149): a name whose one binding is a literal
//! (`count = "4"`, `c = 1`, `(a, b) = (1, "x")`, `d = count`) is checked
//! at compile time as the literal is, by the checks of what literals
//! settle (D99, `value::literal_type_error`) and of what an annotation
//! takes (D113, `annot`). A name is bound once (D88), so the binding
//! says what the name holds wherever it is read. A name bound to a
//! computed value, and a parameter, are left to the run.

use std::rc::Rc;

use super::call::bound_values;
use super::value::{known_type, literal_type_error, literal_type_error_with};
use super::{Resolver, lit_str};
use crate::source::Pos;
use crate::tree::{Head, Node};

/// Where `target` (a pattern) binds `name`.
pub(super) fn name_pos(target: &Node, name: &str) -> Option<Pos> {
    let mut at = None;
    target.bound_names(&mut |n, p| {
        if n == name && at.is_none() {
            at = Some(p);
        }
    });
    at
}

/// `literal_type_error_with` through what `r` knows names hold.
fn viewed_type_error<'a>(r: &'a Resolver<'_>, n: &'a Node) -> Option<crate::diag::Diag> {
    literal_type_error_with(n, &|k| r.literal_view(k))
}

/// `annot::argument_error` through what `r` knows names hold.
fn viewed_argument_error<'a>(
    r: &'a Resolver<'_>,
    sig: &super::call::Sig,
    args: &'a [Node],
) -> Option<crate::diag::Diag> {
    super::annot::argument_error(sig, args, &|k| r.literal_view(k))
}

impl Resolver<'_> {
    /// The first literal argument the annotations refuse, a name that
    /// holds a literal read as it (D113, D149).
    pub(super) fn annotated_argument_error(
        &self,
        sig: &super::call::Sig,
        args: &[Node],
    ) -> Option<crate::diag::Diag> {
        viewed_argument_error(self, sig, args)
    }

    /// The literal `value` (as written) is: itself, or what the name it
    /// is holds (`d = count`).
    fn literal_value(&self, value: &Node) -> Option<Rc<Node>> {
        if value.head == Head::Name {
            return self.literal_of(value).map(|(lit, _)| lit);
        }
        known_type(value).map(|_| Rc::new(value.clone()))
    }

    /// `literal_value` for a top-level statement before the file's names
    /// are resolved: a name is one the earlier statements noted.
    pub(super) fn file_literal(&self, value: &Node) -> Option<Rc<Node>> {
        if self.opts.redefine {
            return None;
        }
        if value.head == Head::Name {
            let slot = self.slots.get(lit_str(value))?;
            return self.global_lits.get(slot).map(|(lit, _)| lit.clone());
        }
        known_type(value).map(|_| Rc::new(value.clone()))
    }

    /// Notes the names an assignment binds to a literal, in the block
    /// that binds them (and the file's global slots).
    pub(super) fn note_bound_literals(&mut self, target: &Node, value: &Node) {
        if self.opts.redefine {
            return;
        }
        let mut pairs = Vec::new();
        bound_values(target, value, None, &mut pairs);
        for (name, value, _) in pairs {
            let Some(lit) = self.literal_value(value) else {
                continue;
            };
            let at = name_pos(target, name).unwrap_or(target.pos);
            let Some(b) = self.cur_block().names.get(name).copied() else {
                continue;
            };
            if self.cur_block().file {
                self.global_lits.insert(b.index, (lit.clone(), at));
            }
            self.cur_block().lits.insert(name.to_string(), (lit, at));
        }
    }

    /// The literal the name `name` holds where it is read, and where it
    /// was bound, looking outward as a read does; a top-level fn reads
    /// the file's constants.
    fn literal_entry(&self, name: &str) -> Option<&(Rc<Node>, Pos)> {
        for f in self.fns.iter().rev() {
            if let Some(i) = f.innermost(name) {
                return f.blocks[i].lits.get(name);
            }
            if f.self_name == name {
                return None;
            }
        }
        if self.in_top_fn() && self.consts.contains_key(name) {
            return self.global_lits.get(self.slots.get(name)?);
        }
        None
    }

    /// Forgets the literal of the binding a failed binding of `name` hit
    /// (E0238): what the name means there is not sure, so one mistake
    /// gives one error. Other names of that spelling keep theirs.
    pub(super) fn forget_literal(&mut self, name: &str) {
        let last = self.fns.len() - 1;
        for i in (0..=last).rev() {
            if let Some(b) = self.fns[i].innermost(name) {
                self.fns[i].blocks[b].lits.remove(name);
                if i == 0 && b == 0 {
                    let slot = self.slots.get(name).copied();
                    if let Some(s) = slot {
                        self.global_lits.remove(&s);
                    }
                }
                return;
            }
        }
        if let Some(&s) = self.slots.get(name) {
            self.global_lits.remove(&s);
        }
    }

    /// The literal a name (as written) holds, and where it was bound.
    fn literal_of(&self, n: &Node) -> Option<(Rc<Node>, Pos)> {
        if n.head != Head::Name {
            return None;
        }
        self.literal_entry(lit_str(n)).cloned()
    }

    /// The literal `k` stands for: what a name holds, or `k` itself.
    pub(super) fn literal_view<'a>(&'a self, k: &'a Node) -> &'a Node {
        let inner = if k.head == Head::Named { &k.kids[0] } else { k };
        if inner.head != Head::Name {
            return k;
        }
        self.literal_entry(lit_str(inner)).map_or(k, |(lit, _)| lit)
    }

    /// The note for an error at `at` when it stands on a name among
    /// `kids` (or inside a named argument or `and`/`or`) that holds a
    /// literal: where the name was bound.
    pub(super) fn literal_note(&self, kids: &[Node], at: Pos) -> Option<String> {
        let mut found = None;
        let mut look = |k: &Node| {
            let inner = if k.head == Head::Named { &k.kids[0] } else { k };
            if found.is_none()
                && inner.pos == at
                && let Some((_, bound)) = self.literal_of(inner)
            {
                found = Some(format!(
                    "note: `{}` is bound to a literal on line {}",
                    lit_str(inner),
                    bound.line
                ));
            }
        };
        for k in kids {
            look(k);
            if k.head == Head::Op && lit_str(k) == crate::tree::BOOL_CHECK {
                k.kids.iter().for_each(&mut look);
            }
        }
        found.or_else(|| {
            // an operator's error stands at the operator: the first name
            kids.iter().find_map(|k| {
                let (_, bound) = self.literal_of(k)?;
                Some(format!(
                    "note: `{}` is bound to a literal on line {}",
                    lit_str(k),
                    bound.line
                ))
            })
        })
    }

    /// The error a node's literals settle once its names that hold
    /// literals are read as them, when only those names make it one: an
    /// error of the literals as written is the value check's already.
    pub(super) fn check_named_literals(&mut self, n: &Node) {
        if !matches!(
            n.head,
            Head::Call | Head::If | Head::Then | Head::Op | Head::Coalesce
        ) {
            return;
        }
        let names = |k: &Node| {
            k.head == Head::Name
                || (k.head == Head::Op
                    && lit_str(k) == crate::tree::BOOL_CHECK
                    && k.kids.iter().any(|c| c.head == Head::Name))
        };
        if !n.kids.iter().any(names) {
            return;
        }
        let found = viewed_type_error(self, n);
        let Some(mut e) = found else {
            return;
        };
        if literal_type_error(n).is_some() {
            return;
        }
        e.notes.extend(self.literal_note(&n.kids, e.pos));
        self.errs.push(e);
    }
}
