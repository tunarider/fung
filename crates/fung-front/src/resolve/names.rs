//! The two rules about names of D38: a name binds once in what it can see
//! (a new name may not hide a visible outer one, E0205/E0240), and every
//! name is read (an unused local, parameter or import is E0239). Names
//! starting with `_` may go unread; hidden desugar names (`$…`) are not
//! the user's.

use super::scope::Scope;
use super::{Resolver, name_error};
use crate::diag::{Arg, Diag, error};
use crate::source::Pos;

/// A visible name that a new binding would hide.
pub(super) enum Outer {
    /// A name of an enclosing block or function (bound at that line).
    Var(Option<Pos>),
    /// A name a top-level statement binds.
    TopName(Option<Pos>),
    /// A top-level fn.
    TopFn(Option<Pos>),
    /// A name an import binds (not a module name).
    Import(Option<Pos>),
    /// An imported module name: E0308, as always (04).
    Module,
    /// The name of a local fn, inside it.
    OwnFn,
    /// A prelude function.
    Prelude,
}

impl Outer {
    /// An outer variable: assigning to it is E0205; the rest are E0240.
    fn is_variable(&self) -> bool {
        matches!(self, Outer::Var(_) | Outer::TopName(_))
    }

    /// What the name is, for E0240's message.
    fn what(&self) -> &'static str {
        match self {
            Outer::Var(_) => "a variable of an outer block",
            Outer::TopName(_) => "a top-level name",
            Outer::TopFn(_) => "a top-level function",
            Outer::Import(_) => "an imported name",
            Outer::Module => "a module",
            Outer::OwnFn => "the name of the function it is in",
            Outer::Prelude => "a prelude function",
        }
    }

    /// Where the hidden name comes from, as a note.
    fn note(&self, name: &str) -> Option<String> {
        let (what, at) = match self {
            Outer::Var(p) => ("is bound", p),
            Outer::TopName(p) => ("is a top-level name bound", p),
            Outer::TopFn(p) => ("is a top-level function defined", p),
            Outer::Import(p) => ("is imported", p),
            Outer::Module | Outer::OwnFn | Outer::Prelude => return None,
        };
        at.map(|p| format!("note: `{name}` {what} on line {}", p.line))
    }
}

impl Resolver<'_> {
    /// Forgets one open binding of `name` (`Resolver::open`).
    pub(super) fn unopen(&mut self, name: &str) {
        if let Some(n) = self.open.get_mut(name) {
            *n -= 1;
            if *n == 0 {
                self.open.remove(name);
            }
        }
    }

    /// The visible name `name` that a binding in the current block would
    /// hide: a name of an enclosing block of this function or of an
    /// enclosing function, a function's own name, the file's names (its
    /// constants, for a top-level fn), or the prelude. Names of ended
    /// blocks and local fns defined later are not visible (E0206, E0208).
    pub(super) fn outer_name(&self, name: &str) -> Option<Outer> {
        let last = self.fns.len() - 1;
        // Only the file's own block can bind a name no open block binds.
        let walked = if self.open.contains_key(name) {
            &self.fns[..]
        } else {
            &self.fns[..1]
        };
        for (i, f) in walked.iter().enumerate().rev() {
            let outer = f.blocks.len() - usize::from(i == last);
            let block = f
                .bound_in
                .get(name)
                .and_then(|at| at.iter().rev().find(|&&b| b < outer));
            if let Some(&b) = block {
                let pos = f.blocks[b].pos.get(name).copied();
                if i > 0 || b > 0 {
                    return Some(Outer::Var(pos));
                }
                let bound = f.blocks[0].names[name];
                return Some(if bound.module {
                    Outer::Module
                } else if self.import_bound.contains_key(name) {
                    Outer::Import(pos)
                } else if bound.constant {
                    Outer::TopFn(pos)
                } else {
                    Outer::TopName(pos)
                });
            }
            if i > 0 && f.self_name == name {
                return Some(Outer::OwnFn);
            }
        }
        if self.in_top_fn() && self.consts.contains_key(name) {
            return Some(Outer::TopName(self.const_pos.get(name).copied()));
        }
        if self.prelude.contains(name) {
            return Some(Outer::Prelude);
        }
        None
    }

    /// Whether a new binding of `name` here (`assign`: an assignment)
    /// hides nothing; otherwise reports it. One rule, a visible name is
    /// not bound again (D88), with two messages: an assignment to an
    /// outer variable is the counter in a loop (E0205, whose help shows
    /// what to write instead); any other binding is E0240. `interface`: the name is part
    /// of a module's interface (a public top-level name of an imported
    /// file, or a parameter of its public fns), which callers see only
    /// qualified (`m.get`) or as an argument name (`timeout:`), so it may
    /// be a prelude name.
    pub(super) fn binds_fresh(
        &mut self,
        name: &str,
        pos: Pos,
        assign: bool,
        interface: bool,
    ) -> bool {
        if crate::tree::is_hidden(name) {
            return true;
        }
        let Some(outer) = self.outer_name(name) else {
            return true;
        };
        if interface && matches!(outer, Outer::Prelude) {
            return true;
        }
        let mut e = match &outer {
            Outer::Module => self.shadow_error(name, pos),
            o if assign && o.is_variable() => name_error("E0205", pos, name),
            o => error(
                "E0240",
                pos,
                vec![Arg::Str(name.into()), Arg::Str(o.what().into())],
            )
            .span(super::rune_len(name)),
        };
        if let Some(note) = outer.note(name) {
            e.notes.push(note);
        }
        self.errs.push(e);
        false
    }

    /// Whether a top-level name or parameter of the file being resolved
    /// is part of its interface (`binds_fresh`): it is public. The rule
    /// is the file's own, whether the file is imported or is the entry
    /// (D86), so a file means the same however it is loaded.
    pub(super) fn interface_name(&self, name: &str) -> bool {
        !name.starts_with('_')
    }

    /// Reports the names of a block that were never read (E0239): not
    /// `_`-prefixed, not hidden desugar names. Sorted by position, then
    /// by name.
    pub(super) fn report_unused(&mut self, sc: &Scope) {
        let mut names: Vec<(&String, Pos)> = sc
            .names
            .keys()
            .filter(|n| !sc.used.contains(*n) && !n.starts_with('_') && !crate::tree::is_hidden(n))
            .filter_map(|n| sc.pos.get(n).map(|p| (n, *p)))
            .collect();
        names.sort_by(|a, b| (a.1.line, a.1.col, a.0).cmp(&(b.1.line, b.1.col, b.0)));
        // A `for` variable is the parameter of the desugared body.
        let for_body = self.cur().for_body;
        for (name, pos) in names {
            let kind = if sc.param.contains(name) && !for_body {
                "parameter"
            } else {
                "variable"
            };
            self.errs.push(unused(kind, name, pos));
        }
    }

    /// The end of the file: unread imports are E0239. A file's top-level
    /// names are not: a public one is its interface, whichever file reads
    /// it (D86), and a `_` one is kept on purpose, like a local. The REPL
    /// keeps every name for later inputs.
    pub(super) fn report_unused_file(&mut self) {
        if self.opts.redefine {
            return;
        }
        let sc = &self.fns[0].blocks[0];
        let mut found: Vec<(Pos, Diag)> = Vec::new();
        for name in sc.names.keys() {
            if sc.used.contains(name) || name.starts_with('_') || crate::tree::is_hidden(name) {
                continue;
            }
            let Some(&pos) = sc.pos.get(name) else {
                continue;
            };
            if let Some(&len) = self.import_bound.get(name) {
                found.push((pos, unused("import", name, pos).span(len)));
            }
        }
        found.sort_by_key(|(p, _)| (p.line, p.col));
        self.errs.extend(found.into_iter().map(|(_, e)| e));
    }
}

fn unused(kind: &str, name: &str, pos: Pos) -> Diag {
    error(
        "E0239",
        pos,
        vec![Arg::Str(kind.into()), Arg::Str(name.into())],
    )
    .span(super::rune_len(name))
}
