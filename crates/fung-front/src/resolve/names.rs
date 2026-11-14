//! The two rules about names of D38: a name binds once in what it can see
//! (a new name may not hide a visible outer one, E0238, D122), and every
//! name is read (an unused local, parameter or import is E0239). Names
//! starting with `_` may go unread; hidden desugar names (`$…`) are not
//! the user's.

use super::scope::Scope;
use super::{Resolver, name_error};
use crate::diag::{Arg, Diag, error};
use crate::source::Pos;

/// E0238's help for a name of another block bound with `=`: the counter
/// of a loop, most often.
const LOOP_HELP: &str = "a name is bound once, and a block cannot bind an outer one again; build the value with an expression instead, e.g. `x = if … ;;` or `fold`";
/// E0238's help for any other binding of a visible name.
const FRESH_HELP: &str = "a visible name means one thing: give the new one a name of its own";
/// E0238's help for a second `fn` of one name.
pub(super) const FN_HELP: &str = "fung has no overloading: give each function a name of its own";

/// What the name bound first in the same block is, for E0238's note.
pub(super) enum Here {
    /// An imported module name.
    Module,
    /// A name an import binds.
    Import,
    /// A `fn`.
    Fn,
    /// Any other name.
    Name,
}

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
    /// An imported module name (04).
    Module(Option<Pos>),
    /// The name of a local fn, inside it.
    OwnFn,
    /// A prelude function.
    Prelude,
}

impl Outer {
    /// An outer variable: assigning to it is the counter in a loop, whose
    /// help shows what to write instead.
    fn is_variable(&self) -> bool {
        matches!(self, Outer::Var(_) | Outer::TopName(_))
    }

    /// Where the name comes from, as E0238's note.
    fn note(&self, name: &str) -> String {
        let (what, at) = match self {
            Outer::Var(p) => ("is bound outside this block, on line", p),
            Outer::TopName(p) => ("is a top-level name bound on line", p),
            Outer::TopFn(p) => ("is a function defined on line", p),
            Outer::Import(p) => ("is imported on line", p),
            Outer::Module(p) => ("is the module imported on line", p),
            Outer::OwnFn => return format!("note: `{name}` is the name of the function it is in"),
            Outer::Prelude => return format!("note: `{name}` is a prelude function"),
        };
        match at {
            Some(p) => format!("note: `{name}` {what} {}", p.line),
            // without a place: the words up to "on line"
            None => format!(
                "note: `{name}` {}",
                what.trim_end_matches(" on line").trim_end_matches(',')
            ),
        }
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
                    Outer::Module(pos)
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
    /// not bound again (D88), one error, E0238 (D122): the note says where
    /// the name is from, and the help what to write instead (for an
    /// assignment to an outer variable, the counter in a loop, an
    /// expression or `fold`). `interface`: the name is part
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
        let e = match &outer {
            Outer::Module(_) => self.module_again(name, pos),
            o => {
                let help = if assign && o.is_variable() {
                    LOOP_HELP
                } else {
                    FRESH_HELP
                };
                let mut e = name_error("E0238", pos, name).with_help(help);
                e.notes.push(o.note(name));
                e
            }
        };
        self.errs.push(e);
        false
    }

    /// E0238 for binding `name` again in a block that bound it already
    /// (`root`: the file's block), as `what` the first binding is; a
    /// second `fn` says there is no overloading (D122, FE61).
    pub(super) fn bound_again(
        &mut self,
        name: &str,
        pos: Pos,
        root: bool,
        defining_fn: bool,
    ) -> Diag {
        let sc = if root {
            &self.fns[0].blocks[0]
        } else {
            self.fns
                .last()
                .and_then(|f| f.blocks.last())
                .expect("block stack")
        };
        let first = sc.pos.get(name).copied();
        let here = match sc.names.get(name) {
            Some(b) if b.module => Here::Module,
            _ if sc.file && self.import_bound.contains_key(name) => Here::Import,
            _ if sc.fns.contains(name) => Here::Fn,
            _ => Here::Name,
        };
        if matches!(here, Here::Module) {
            return self.module_again(name, pos);
        }
        let what = match here {
            Here::Import => "is imported on line",
            Here::Fn => "is a function defined on line",
            _ => "is first bound on line",
        };
        let mut e = name_error("E0238", pos, name);
        if let Some(p) = first {
            e.notes.push(format!("note: `{name}` {what} {}", p.line));
        }
        if defining_fn {
            e = e.with_help(FN_HELP);
        }
        e
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
            let mut e = unused(kind, name, pos);
            if self.catch_all.contains(&(pos.line, pos.col)) {
                e.help = Some(format!(
                    "the name `{name}` matches every value and binds it; an arm for any other value is written `_ =>`"
                ));
            }
            self.errs.push(e);
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
