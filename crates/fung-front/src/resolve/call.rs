//! Calls whose callee is known at compile time (D43): a top-level fn of
//! the file, a prelude function, a member of an imported module. Their
//! argument count and names are checked here with the codes and messages
//! the call gets at run time (`fung-vm` `vm/call.rs`, `bind_in_place`), so
//! `fung check` finds what running would.

use super::{Resolver, lit_str};
use crate::diag::{Arg, Diag, error};
use crate::source::Pos;
use crate::tree::{FnInfo, Head, Node};

/// One parameter of a known function, as a call binds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SigParam {
    pub name: String,
    pub default: bool,
    pub rest: bool,
    /// The parameter's type annotation with aliases expanded, as the run
    /// checks it (D113); `None` when unchecked.
    pub types: Option<Vec<String>>,
}

/// The parameters of a known function, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sig {
    pub params: Vec<SigParam>,
    /// A builtin known to have no effect (the spec's `pure` mark, D39):
    /// a call of it as a statement is E0234.
    pub pure: bool,
    /// The name the function's own errors give it, when that is not the
    /// name a module exports it under (`adder = (a, b) => a + b` is "the
    /// function"); `None` for a function exported under its own name.
    pub name: Option<String>,
}

/// What the errors of a function made without a name call it (`fung-vm`
/// `vm/call.rs`).
pub const NAMELESS: &str = "the function";

impl Sig {
    /// A parameter list as `spec/builtins.txt` writes it: `xs, sep=?`,
    /// `...args`, or nothing.
    pub fn parse(list: &str) -> Sig {
        let params = list
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(|p| {
                if let Some(name) = p.strip_prefix("...") {
                    SigParam {
                        name: name.to_string(),
                        default: false,
                        rest: true,
                        types: None,
                    }
                } else if let Some(name) = p.strip_suffix("=?") {
                    SigParam {
                        name: name.to_string(),
                        default: true,
                        rest: false,
                        types: None,
                    }
                } else {
                    SigParam {
                        name: p.to_string(),
                        default: false,
                        rest: false,
                        types: None,
                    }
                }
            })
            .collect();
        Sig {
            params,
            pure: false,
            name: None,
        }
    }

    /// `parse`, marked pure when `pure` (D39).
    pub fn parse_marked(list: &str, pure: bool) -> Sig {
        Sig {
            pure,
            ..Sig::parse(list)
        }
    }

    /// The parameters of a fn as written (after desugar: a pattern
    /// parameter has its hidden `$` name), without their types
    /// (`Resolver::sig_of` adds them).
    pub fn of(info: &FnInfo) -> Sig {
        let params = info
            .params
            .iter()
            .map(|p| SigParam {
                name: p.name.clone(),
                default: p.has_default,
                rest: p.rest,
                types: None,
            })
            .collect();
        Sig {
            params,
            pure: false,
            name: None,
        }
    }

    /// The list as [`Sig::parse`] reads it.
    pub fn list(&self) -> String {
        self.params
            .iter()
            .map(|p| {
                if p.rest {
                    format!("...{}", p.name)
                } else if p.default {
                    format!("{}=?", p.name)
                } else {
                    p.name.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The first error binding these arguments gives, as `bind_in_place`
    /// finds it: too many positional arguments (E0402), a named one that
    /// is no parameter (E0406) or fills one already filled (E0405), a
    /// parameter left without a value (E0407). A spread (`...xs`, D45)
    /// leaves the count to the call: only the names are checked then, and
    /// the arguments written before it when they are already too many
    /// (D110).
    fn check(&self, name: &str, args: &[Node], pos: Pos) -> Option<Diag> {
        let n = self.params.len();
        let rest = n > 0 && self.params[n - 1].rest;
        let fixed = if rest { n - 1 } else { n };
        let spread = args.iter().any(|a| a.head == Head::Spread);
        let npos = args.iter().filter(|a| a.head != Head::Named).count();
        let named: Vec<&str> = args
            .iter()
            .filter(|a| a.head == Head::Named)
            .map(lit_str)
            .collect();
        // Each name is looked up in a table made once, and what the names
        // fill is marked once: a call with many named arguments is checked
        // in linear time.
        let slots: std::collections::HashMap<&str, usize> = self.params[..fixed]
            .iter()
            .enumerate()
            .rev()
            .map(|(i, p)| (p.name.as_str(), i))
            .collect();
        let slot_of = |k: &str| slots.get(k).copied();
        if spread {
            if let Some(k) = named.iter().find(|k| slot_of(k).is_none()) {
                return Some(no_such(name, k, pos));
            }
            let written = args
                .iter()
                .filter(|a| a.head != Head::Named && a.head != Head::Spread)
                .count();
            if written > fixed && !rest {
                let args = vec![
                    Arg::Str(name.to_string()),
                    Arg::Int(fixed as i64),
                    Arg::Str(format!("at least {written}")),
                ];
                return Some(error("E0402", pos, args));
            }
            return None;
        }
        if named.is_empty() && npos == n && !rest {
            return None;
        }
        if npos > fixed && !rest {
            let args = vec![
                Arg::Str(name.to_string()),
                Arg::Int(fixed as i64),
                Arg::Int(npos as i64),
            ];
            return Some(error("E0402", pos, args));
        }
        let mut filled = vec![false; fixed];
        for k in &named {
            let Some(i) = slot_of(k) else {
                return Some(no_such(name, k, pos));
            };
            if i < npos {
                return Some(error("E0405", pos, vec![Arg::Str(k.to_string())]));
            }
            filled[i] = true;
        }
        let unfilled = self.params[..fixed].iter().zip(&filled).enumerate();
        for (i, (p, filled)) in unfilled.skip(npos.min(fixed)) {
            if p.default || *filled {
                continue;
            }
            // a parameter with no name in the source (`$`) by its place
            let which = if crate::tree::is_hidden(&p.name) {
                (i + 1).to_string()
            } else {
                format!("`{}`", p.name)
            };
            return Some(error("E0407", pos, vec![Arg::Str(which)]));
        }
        None
    }
}

/// E0406 for the named argument `k` that `name` has no parameter for.
fn no_such(name: &str, k: &str, pos: Pos) -> Diag {
    error(
        "E0406",
        pos,
        vec![Arg::Str(name.to_string()), Arg::Str(k.to_string())],
    )
}

impl Resolver<'_> {
    /// A fn's parameters with their type annotations expanded (D113).
    pub(super) fn sig_of(&mut self, info: &FnInfo) -> Sig {
        let mut sig = Sig::of(info);
        for (s, p) in sig.params.iter_mut().zip(&info.params) {
            if let Some(t) = &p.types {
                let names: Vec<String> = self
                    .expand_types(t, p.pos)
                    .into_iter()
                    .map(|t| t.name)
                    .collect();
                s.types = Some(names).filter(|n| !n.is_empty());
            }
        }
        sig
    }

    /// Notes the top-level names whose one binding is visibly a function
    /// (D110), before the top-level fns are resolved: a fn reads the
    /// file's constants when called, so its calls of one are checked too.
    /// In order, so a name bound to an earlier such name is known. Not in
    /// the REPL, where a later input may bind the name again.
    pub(super) fn note_functions(&mut self, rest: &[&Node]) {
        if self.opts.redefine {
            return;
        }
        for k in rest {
            if k.head != Head::Let {
                continue;
            }
            let mut pairs = Vec::new();
            bound_values(&k.kids[0], &k.kids[1], None, &mut pairs);
            for (name, value, _) in pairs {
                let Some(&slot) = self.slots.get(name) else {
                    continue;
                };
                if let Some(known) = self.function_value(value, None) {
                    self.global_sigs.insert(slot, known);
                }
            }
        }
    }

    /// Notes what the names an assignment binds hold when it is visibly a
    /// function (D110): a call through one is checked as the function's
    /// own call is, in every block. `target` and `value` as written, `got`
    /// the value resolved.
    pub(super) fn note_bound_functions(&mut self, target: &Node, value: &Node, got: &Node) {
        let mut pairs = Vec::new();
        bound_values(target, value, Some(got), &mut pairs);
        for (name, value, got) in pairs {
            let Some(known) = self.function_value(value, got) else {
                continue;
            };
            let Some(b) = self.cur_block().names.get(name).copied() else {
                continue;
            };
            if self.cur_block().file {
                if self.opts.redefine {
                    continue;
                }
                self.global_sigs.insert(b.index, known.clone());
            }
            self.cur_block().sigs.insert(name.to_string(), known);
        }
    }

    /// The function `value` (as written; `got` resolved, or `None` before
    /// the file's names are resolved) is sure to be, with the name its
    /// errors give it: a lambda or an anonymous `fn`, a partial
    /// application (pure when what it calls is), or a known function's
    /// name (`known_callee`).
    pub(super) fn function_value(
        &mut self,
        value: &Node,
        got: Option<&Node>,
    ) -> Option<(String, Sig)> {
        match value.head {
            Head::Fn if super::value::is_lambda(value) => {
                Some((NAMELESS.to_string(), self.sig_of(super::fn_info(value))))
            }
            Head::Do if super::value::partial_shape(value) => {
                let lambda = value.kids.last()?;
                let mut sig = self.sig_of(super::fn_info(lambda));
                let made = lambda.kids.last().and_then(|b| b.kids.first());
                if made.is_some_and(|m| m.head == Head::Call) {
                    // the callee is kept first
                    let callee = &value.kids[0].kids[1];
                    let got = got.and_then(|g| g.kids.first()).and_then(|l| l.kids.get(1));
                    sig.pure = self
                        .function_value(callee, got)
                        .is_some_and(|(_, s)| s.pure);
                }
                Some((NAMELESS.to_string(), sig))
            }
            _ => match got {
                Some(got) => self.known_callee(value, got),
                None => self.file_function(value),
            },
        }
    }

    /// `known_callee` for a value of a top-level statement before the
    /// file's names are resolved: a top-level fn or import, a constant
    /// noted already, a prelude function, a module member.
    fn file_function(&mut self, value: &Node) -> Option<(String, Sig)> {
        match value.head {
            Head::Name => {
                let name = lit_str(value);
                let slot = match self.fns[0].blocks[0].names.get(name) {
                    Some(b) if b.global => Some(b.index),
                    Some(_) => None,
                    None => self.slots.get(name).copied(),
                };
                if let Some(slot) = slot {
                    return self.global_sigs.get(&slot).cloned();
                }
                if !self.prelude.contains(name) {
                    return None;
                }
                let leaf = Node::leaf(
                    Head::Prelude,
                    value.pos,
                    crate::tree::Lit::Str(name.to_string()),
                );
                self.known_callee(value, &leaf)
            }
            Head::Field => self.known_callee(value, value),
            _ => None,
        }
    }

    /// Resolves `(call f args…)` and checks it when `f` is known. A call
    /// of a function known to have no effect is noted for the check of
    /// discarded values (E0234, D39).
    pub(super) fn call(&mut self, n: &Node) -> Node {
        let kids: Vec<Node> = n.kids.iter().map(|k| self.walk(k)).collect();
        if let Some((name, sig)) = self.function_value(&n.kids[0], Some(&kids[0])) {
            if let Some(e) = sig.check(&name, &n.kids[1..], n.pos) {
                self.errs.push(e);
            } else if let Some(e) = super::annot::argument_error(&sig, &n.kids[1..]) {
                // what the call binds is checked as the run does (D113)
                self.errs.push(e);
            }
            if sig.pure {
                // The underline starts where the callee is written.
                // The error shows the callee as written, its help the
                // function's own name (`s = sort; s(xs)`).
                let (shown, at) = match n.kids[0].head {
                    Head::Field => (
                        format!("{}.{name}", lit_str(&n.kids[0].kids[0])),
                        n.kids[0].kids[0].pos,
                    ),
                    Head::Name => (lit_str(&n.kids[0]).to_string(), n.kids[0].pos),
                    _ => (name.clone(), n.kids[0].pos),
                };
                let own = match n.kids[0].head {
                    Head::Field => shown.clone(),
                    _ => name,
                };
                self.pure_calls.insert(
                    (n.pos.line, n.pos.col),
                    super::value::PureCall { shown, own, at },
                );
            }
        }
        Node::with_lit(n.head, n.pos, n.lit.clone(), kids)
    }

    /// The known function the name `callee` holds in the block that binds
    /// it, looking outward through the enclosing functions as a read
    /// does: a local `fn` (itself, inside its body), or what a partial
    /// application keeps (FE65). A name is bound once (D88), so the block
    /// that binds it says what it is.
    fn block_sig(&self, callee: &Node) -> Option<(String, Sig)> {
        if callee.head != Head::Name {
            return None;
        }
        let name = lit_str(callee);
        for f in self.fns.iter().rev() {
            if let Some(i) = f.innermost(name) {
                return f.blocks[i].sigs.get(name).cloned();
            }
            if f.self_name == name {
                return f.self_sig.clone().map(|sig| (name.to_string(), sig));
            }
        }
        None
    }

    /// The name and parameters of the function `callee` (as written) is
    /// sure to be, resolved to `got`: a top-level fn of the file or a
    /// member an import bound by name (`global_sigs`), a prelude
    /// function, or `m.f` of an imported module `m`.
    pub(super) fn known_callee(&self, callee: &Node, got: &Node) -> Option<(String, Sig)> {
        match got.head {
            Head::Global => {
                let crate::tree::Lit::Int(i) = got.lit else {
                    return None;
                };
                let top = self.global_sigs.get(&(i as usize)).cloned();
                top.or_else(|| self.block_sig(callee))
            }
            Head::Local | Head::Captured | Head::SelfFn => self.block_sig(callee),
            Head::Prelude => {
                let name = lit_str(got);
                let list = self.builtins().prelude_signature(name)?;
                let pure = self.builtins().prelude_pure(name);
                Some((name.to_string(), Sig::parse_marked(list, pure)))
            }
            Head::Field if callee.kids[0].head == Head::Name => {
                let info = self.module_names.get(lit_str(&callee.kids[0]))?;
                let member = &*crate::unicode::nfc(lit_str(got));
                let sig = info.sigs.get(member)?;
                let name = sig.name.clone().unwrap_or_else(|| member.to_string());
                Some((name, sig.clone()))
            }
            _ => None,
        }
    }
}

/// The names `target` binds whose values are written in `value`, each
/// with that value (and the value resolved, when `got` is): the name of
/// `f = …`, the names of a tuple or list pattern over a literal of the
/// same shape (`(f, g) = (x => x, len)`).
fn bound_values<'n>(
    target: &'n Node,
    value: &'n Node,
    got: Option<&'n Node>,
    out: &mut Vec<(&'n str, &'n Node, Option<&'n Node>)>,
) {
    match (target.head, value.head) {
        (Head::Name | Head::Pbind, _) if !matches!(lit_str(target), "" | "_") => {
            out.push((lit_str(target), value, got));
        }
        (Head::Ptuple, Head::Tuple) | (Head::Plist, Head::List)
            if target.kids.len() == value.kids.len()
                && got.is_none_or(|g| g.kids.len() == value.kids.len()) =>
        {
            for (i, (t, v)) in target.kids.iter().zip(&value.kids).enumerate() {
                bound_values(t, v, got.map(|g| &g.kids[i]), out);
            }
        }
        _ => {}
    }
}
