//! Imports and module member checks (04).

use super::scope::Binding;
use super::{Resolver, lit_str, name_error, ref_node, rune_len, str_arg};
use crate::diag::{Diag, error};
use crate::source::Pos;
use crate::tree::{Head, Lit, Node};

use super::ModuleInfo;

impl Resolver<'_> {
    /// E0308 for hiding the module bound to `name`; the help writes its
    /// import with the module as imported, not the name it is bound to.
    pub(super) fn shadow_error(&self, name: &str, pos: Pos) -> Diag {
        let target = self.module_specs.get(name).map_or(name, String::as_str);
        name_error("E0308", pos, name).with_help(&format!(
            "import it under another name: `import {target} as m`"
        ))
    }

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
        if !mod_name.is_empty() {
            let target = if im.is_path {
                format!("\"{}\"", im.spec)
            } else {
                im.spec.clone()
            };
            self.module_specs.insert(mod_name.to_string(), target);
        }
        let mut kids = vec![
            match self.bind_module(mod_name, im.bind_pos, im.bind_len, info) {
                Some(b) => ref_node(n.pos, b),
                None => Node::new(Head::Nil, n.pos, vec![]),
            },
        ];
        for (i, name) in im.names.iter().flatten().enumerate() {
            let pos = im.name_pos[i];
            let e = if name.starts_with('_') {
                Some(error(
                    "E0304",
                    pos,
                    vec![str_arg(name), str_arg(&info.name)],
                ))
            } else if !info.exports.contains(name) {
                Some(missing_member(&info.name, name, pos, &info.exports))
            } else {
                None
            };
            if let Some(e) = e {
                self.errs.push(e.span(rune_len(name)));
                // The name is bound all the same, so its uses are not
                // reported again as undefined (FE61).
                if self.bind_const(name, pos).is_some() {
                    self.root_scope().used.insert(name.clone());
                }
                continue;
            }
            // A constant global, like a hoisted fn.
            if let Some(b) = self.bind_const(name, pos) {
                if let Some(sig) = info.sigs.get(name)
                    && !self.opts.redefine
                {
                    self.global_sigs
                        .insert(b.index, (name.clone(), sig.clone()));
                }
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
    /// name (only a name list was written) binds nothing. `pos` and `len`
    /// are where the name is written, for diagnostics.
    fn bind_module(
        &mut self,
        name: &str,
        pos: Pos,
        len: usize,
        info: &ModuleInfo,
    ) -> Option<Binding> {
        if name.is_empty() {
            return None;
        }
        if let Some(b) = self.root_scope().names.get(name).copied() {
            if self.opts.redefine && (b.module || !b.constant) {
                // REPL: the import takes the name's storage over (08).
                let b = Binding {
                    constant: true,
                    module: true,
                    ..b
                };
                self.fns[0].bind(0, name, b);
                self.module_names.insert(name.to_string(), info.clone());
                self.import_bound.insert(name.to_string(), len);
                return Some(b);
            }
            let e = if b.module {
                name_error("E0309", pos, name)
            } else {
                self.shadow_error(name, pos)
            };
            self.errs.push(e.span(len));
            return None;
        }
        self.binds_fresh(name, pos, false, false);
        let b = Binding {
            global: true,
            index: self.globals.len(),
            constant: true,
            module: true,
        };
        self.bind_import(name, pos, len, b);
        self.module_names.insert(name.to_string(), info.clone());
        Some(b)
    }

    /// Binds a listed import name as a constant global: hoisted fns see
    /// it, nothing may reassign it (E0209).
    fn bind_const(&mut self, name: &str, pos: Pos) -> Option<Binding> {
        if let Some(b) = self.root_scope().names.get(name).copied() {
            let e = if b.module {
                self.shadow_error(name, pos)
            } else {
                name_error("E0209", pos, name)
            };
            self.errs.push(e);
            return None;
        }
        self.binds_fresh(name, pos, false, false);
        let b = Binding {
            global: true,
            index: self.globals.len(),
            constant: true,
            module: false,
        };
        self.bind_import(name, pos, rune_len(name), b);
        Some(b)
    }

    fn bind_import(&mut self, name: &str, pos: Pos, len: usize, b: Binding) {
        self.globals.push(name.to_string());
        self.fns[0].bind(0, name, b);
        self.root_scope().pos.insert(name.to_string(), pos);
        self.import_bound.insert(name.to_string(), len);
    }

    /// Reports a static member access on a module binding: private names
    /// are E0304, unknown names E0303 (04).
    pub(super) fn check_member(&mut self, target: &Node, member: &str, pos: Pos) {
        if target.head != Head::Name {
            return;
        }
        // A field is written as is (a map key, 01); a member is a name.
        let member = &*crate::unicode::nfc(member);
        let Some(info) = self.module_names.get(lit_str(target)) else {
            return;
        };
        let e = if member.starts_with('_') {
            error("E0304", pos, vec![str_arg(member), str_arg(&info.name)])
        } else if !info.exports.iter().any(|x| x == member) {
            missing_member(&info.name, member, pos, &info.exports)
        } else {
            return;
        };
        self.errs.push(e.span(rune_len(member)));
    }
}

/// E0303 for `member` missing from module `module`, with the member of a
/// similar name if there is one, as an undefined variable gets (FE61).
fn missing_member(module: &str, member: &str, pos: Pos, exports: &[String]) -> Diag {
    let e = error("E0303", pos, vec![str_arg(module), str_arg(member)]);
    let index = super::similar::Index::default();
    match super::similar::similar(member, &index, exports.iter().map(String::as_str)) {
        Some(s) => e.with_help(&format!("a similar name exists: `{s}`")),
        None => e,
    }
}
