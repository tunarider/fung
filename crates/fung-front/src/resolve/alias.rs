//! Type aliases (07).

use super::fn_::{is_builtin_type, is_tag_name};
use super::{Resolver, name_error};
use crate::source::Pos;
use crate::tree::{Head, Lit, Node};

impl Resolver<'_> {
    /// Registers the file's type aliases before anything else is resolved,
    /// so functions above an alias can use it (07). Duplicates are E0217;
    /// aliases are dropped from the core tree. Cycles are checked in
    /// declaration order.
    pub(super) fn declare_aliases(&mut self, root: &Node) {
        for k in &root.kids {
            let (Head::Typealias, Lit::Alias(a)) = (k.head, &k.lit) else {
                continue;
            };
            if self.aliases.contains_key(&a.name) {
                self.errs.push(name_error("E0217", k.pos, &a.name));
                continue;
            }
            self.aliases.insert(a.name.clone(), a.alts.clone());
            self.alias_pos.insert(a.name.clone(), k.pos);
            self.alias_order.push(a.name.clone());
        }
        for name in self.alias_order.clone() {
            let pos = self.alias_pos[&name];
            self.expand(&name, &mut Vec::new(), pos);
        }
    }

    /// The union behind a type list with aliases replaced, recursively; a
    /// cycle is E0218 and an unknown name E0212.
    pub(super) fn expand_types(&mut self, alts: &[String], pos: Pos) -> Vec<String> {
        let mut out = Vec::new();
        for a in alts {
            out.extend(self.expand(a, &mut Vec::new(), pos));
        }
        out
    }

    fn expand(&mut self, name: &str, visiting: &mut Vec<String>, pos: Pos) -> Vec<String> {
        if visiting.iter().any(|v| v == name) {
            if self.reported.insert(name.to_string()) {
                self.errs.push(name_error("E0218", pos, name));
            }
            return Vec::new();
        }
        if let Some(alts) = self.aliases.get(name).cloned() {
            visiting.push(name.to_string());
            let mut out = Vec::new();
            for a in &alts {
                out.extend(self.expand(a, visiting, pos));
            }
            visiting.pop();
            return out;
        }
        if is_builtin_type(name) || is_tag_name(name) {
            return vec![name.to_string()];
        }
        if self.reported.insert(name.to_string()) {
            self.errs.push(name_error("E0212", pos, name));
        }
        Vec::new()
    }
}
