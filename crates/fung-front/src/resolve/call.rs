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
}

/// The parameters of a known function, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sig {
    pub params: Vec<SigParam>,
    /// A builtin known to have no effect (the spec's `pure` mark, D39):
    /// a call of it as a statement is E0234.
    pub pure: bool,
}

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
                    }
                } else if let Some(name) = p.strip_suffix("=?") {
                    SigParam {
                        name: name.to_string(),
                        default: true,
                        rest: false,
                    }
                } else {
                    SigParam {
                        name: p.to_string(),
                        default: false,
                        rest: false,
                    }
                }
            })
            .collect();
        Sig {
            params,
            pure: false,
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
    /// parameter has its hidden `$` name).
    pub fn of(info: &FnInfo) -> Sig {
        let params = info
            .params
            .iter()
            .map(|p| SigParam {
                name: p.name.clone(),
                default: p.has_default,
                rest: p.rest,
            })
            .collect();
        Sig {
            params,
            pure: false,
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
    /// leaves the count to the call; only the names are checked then.
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
            let k = named.iter().find(|k| slot_of(k).is_none())?;
            return Some(no_such(name, k, pos));
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
    /// Notes the top-level names that hold a known function: `read =
    /// _n.read`, a member of an imported module under a name of the file
    /// (a top-level name is bound once, D53). A call of one gets the
    /// member's name in its errors, as the function itself names them.
    pub(super) fn note_aliases(&mut self, rest: &[&Node]) {
        if self.opts.redefine {
            return;
        }
        for k in rest {
            let (target, value) = match (k.head, k.kids.as_slice()) {
                (Head::Let, [t, v]) if t.head == Head::Name => (t, v),
                _ => continue,
            };
            if value.head != Head::Field || value.kids[0].head != Head::Name {
                continue;
            }
            let Some(info) = self.module_names.get(lit_str(&value.kids[0])) else {
                continue;
            };
            let member = lit_str(value);
            let (Some(sig), Some(&slot)) = (info.sigs.get(member), self.slots.get(lit_str(target)))
            else {
                continue;
            };
            let entry = (member.to_string(), sig.clone());
            self.global_sigs.insert(slot, entry);
        }
    }

    /// Resolves `(call f args…)` and checks it when `f` is known. A call
    /// of a function known to have no effect is noted for the check of
    /// discarded values (E0234, D39).
    pub(super) fn call(&mut self, n: &Node) -> Node {
        let kids: Vec<Node> = n.kids.iter().map(|k| self.walk(k)).collect();
        if let Some((name, sig)) = self.known_callee(&n.kids[0], &kids[0]) {
            if let Some(e) = sig.check(&name, &n.kids[1..], n.pos) {
                self.errs.push(e);
            }
            if sig.pure {
                // The underline starts where the callee is written.
                let (shown, at) = match n.kids[0].head {
                    Head::Field => (
                        format!("{}.{name}", lit_str(&n.kids[0].kids[0])),
                        n.kids[0].kids[0].pos,
                    ),
                    _ => (name, n.kids[0].pos),
                };
                self.pure_calls.insert((n.pos.line, n.pos.col), (shown, at));
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
                Some((member.to_string(), sig.clone()))
            }
            _ => None,
        }
    }
}
