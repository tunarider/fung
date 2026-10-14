//! Imports and module member checks (04).

use super::scope::Binding;
use super::{Resolver, lit_str, name_error, ref_node, rune_len, str_arg};
use crate::diag::{Diag, error};
use crate::source::Pos;
use crate::tree::{Head, Lit, Node};

use super::ModuleInfo;

pub(super) fn shadow_error(name: &str, pos: Pos) -> Diag {
    name_error("E0308", pos, name).with_help(&format!(
        "import it under another name: `import {name} as m`"
    ))
}

impl Resolver<'_> {
    /// Binds an import's names as constant globals — module names marked
    /// as modules so they cannot be shadowed (04) — and rewrites the
    /// statement to `(import key (global n)|(nil) (member name (global m))…)`.
    pub(super) fn import_stmt(&mut self, n: &Node) -> Node {
        let Lit::Import(im) = &n.lit else {
            unreachable!("import node without Import");
        };
        let Some(info) = self.opts.imports.get(&im.spec) else {
            self.errs
                .push(error("E0301", n.pos, vec![str_arg(&im.spec)]));
            return Node::new(Head::Nil, n.pos, vec![]);
        };
        self.imports.push(info.key.clone());
        let mod_name = if im.alias.is_empty() && im.names.is_none() {
            info.name.as_str()
        } else {
            im.alias.as_str()
        };
        let mut kids = vec![match self.bind_module(mod_name, n.pos, info) {
            Some(b) => ref_node(n.pos, b),
            None => Node::new(Head::Nil, n.pos, vec![]),
        }];
        for (i, name) in im.names.iter().flatten().enumerate() {
            let pos = im.name_pos[i];
            if name.starts_with('_') {
                self.errs.push(
                    error("E0304", pos, vec![str_arg(name), str_arg(&info.name)])
                        .span(rune_len(name)),
                );
                continue;
            }
            if !info.exports.contains(name) {
                self.errs.push(
                    error("E0303", pos, vec![str_arg(&info.name), str_arg(name)])
                        .span(rune_len(name)),
                );
                continue;
            }
            // A constant global, like a hoisted fn.
            if let Some(b) = self.bind_const(name, pos) {
                kids.push(Node::with_lit(
                    Head::Member,
                    pos,
                    Lit::Str(name.clone()),
                    vec![ref_node(pos, b)],
                ));
            }
        }
        Node::with_lit(Head::Import, n.pos, Lit::Str(info.key.clone()), kids)
    }

    /// Binds a module name: constant, unshadowable, once (E0309). An empty
    /// name (only a name list was written) binds nothing.
    fn bind_module(&mut self, name: &str, pos: Pos, info: &ModuleInfo) -> Option<Binding> {
        if name.is_empty() {
            return None;
        }
        if let Some(b) = self.root_scope().names.get(name).copied() {
            let e = if b.module {
                name_error("E0309", pos, name)
            } else {
                shadow_error(name, pos)
            };
            self.errs.push(e);
            return None;
        }
        let b = Binding {
            global: true,
            index: self.globals.len(),
            constant: true,
            module: true,
        };
        self.bind_import(name, b);
        self.module_names.insert(name.to_string(), info.clone());
        Some(b)
    }

    /// Binds a listed import name as a constant global: hoisted fns see
    /// it, nothing may reassign it (E0209).
    fn bind_const(&mut self, name: &str, pos: Pos) -> Option<Binding> {
        if let Some(b) = self.root_scope().names.get(name).copied() {
            let e = if b.module {
                shadow_error(name, pos)
            } else {
                name_error("E0209", pos, name)
            };
            self.errs.push(e);
            return None;
        }
        let b = Binding {
            global: true,
            index: self.globals.len(),
            constant: true,
            module: false,
        };
        self.bind_import(name, b);
        Some(b)
    }

    fn bind_import(&mut self, name: &str, b: Binding) {
        self.globals.push(name.to_string());
        self.root_scope().names.insert(name.to_string(), b);
        self.import_bound.insert(name.to_string());
    }

    /// Reports E0308 when a new name would hide an imported module name
    /// (04). Every place that creates a name calls it: assignment, fn,
    /// params, patterns.
    pub(super) fn check_shadow(&mut self, name: &str, pos: Pos) -> bool {
        if self.module_names.contains_key(name) {
            self.errs.push(shadow_error(name, pos));
            return false;
        }
        true
    }

    /// Reports a static member access on a module binding: private names
    /// are E0304, unknown names E0303 (04).
    pub(super) fn check_member(&mut self, target: &Node, member: &str, pos: Pos) {
        if target.head != Head::Name {
            return;
        }
        let Some(info) = self.module_names.get(lit_str(target)) else {
            return;
        };
        let e = if member.starts_with('_') {
            error("E0304", pos, vec![str_arg(member), str_arg(&info.name)])
        } else if !info.exports.iter().any(|x| x == member) {
            error("E0303", pos, vec![str_arg(&info.name), str_arg(member)])
        } else {
            return;
        };
        self.errs.push(e.span(rune_len(member)));
    }
}
