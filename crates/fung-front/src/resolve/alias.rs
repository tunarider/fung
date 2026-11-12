//! Type aliases (07).

use std::collections::HashSet;

use super::fn_::{is_builtin_type, is_tag_name};
use super::{Resolver, name_error};
use crate::source::Pos;
use crate::tree::{Head, Lit, Node, TypeTerm};

/// The aliases being expanded, to find a cycle (E0218).
type Visiting = HashSet<String>;

/// Adds `terms` to the union `out`, each name once (FE40: an alias used
/// twice would double the union at every level); `seen` holds the names
/// in `out`.
fn union_into(out: &mut Vec<TypeTerm>, seen: &mut HashSet<String>, terms: Vec<TypeTerm>) {
    for t in terms {
        if seen.insert(t.name.clone()) {
            out.push(t);
        }
    }
}

impl Resolver<'_> {
    /// Registers the file's type aliases before anything else is resolved,
    /// so functions above an alias can use it (07). A built-in type name is
    /// E0222 (it would mean something else in this file only), duplicates
    /// are E0217; aliases are dropped from the core tree. Cycles are
    /// checked in declaration order.
    pub(super) fn declare_aliases(&mut self, root: &Node) {
        for k in &root.kids {
            let (Head::Typealias, Lit::Alias(a)) = (k.head, &k.lit) else {
                continue;
            };
            if is_builtin_type(&a.name) {
                self.errs.push(name_error("E0222", a.name_pos, &a.name));
                continue;
            }
            if self.aliases.contains_key(&a.name) {
                self.errs.push(name_error("E0217", a.name_pos, &a.name));
                continue;
            }
            self.aliases.insert(a.name.clone(), a.alts.clone());
            self.alias_pos.insert(a.name.clone(), k.pos);
            self.alias_order.push(a.name.clone());
        }
        for name in self.alias_order.clone() {
            let pos = self.alias_pos[&name];
            self.expand(&TypeTerm::plain(&name, pos), &mut Visiting::default(), pos);
        }
    }

    /// The union behind a type list with aliases replaced, recursively, as
    /// plain terms (the outer names the runtime checks); a cycle is E0218
    /// at `pos`, an unknown name E0212 where it is written.
    pub(super) fn expand_types(&mut self, alts: &[TypeTerm], pos: Pos) -> Vec<TypeTerm> {
        let (mut out, mut seen) = (Vec::new(), HashSet::new());
        for a in alts {
            let terms = self.expand(a, &mut Visiting::default(), pos);
            union_into(&mut out, &mut seen, terms);
        }
        out
    }

    fn expand(&mut self, t: &TypeTerm, visiting: &mut Visiting, pos: Pos) -> Vec<TypeTerm> {
        let name = t.name.as_str();
        if visiting.contains(name) {
            if self.reported.insert(name.to_string()) {
                self.errs.push(name_error("E0218", pos, name));
            }
            return Vec::new();
        }
        if let Some(out) = self.expanded.get(name) {
            return out.clone();
        }
        if let Some(alts) = self.aliases.get(name).cloned() {
            visiting.insert(name.to_string());
            let (mut out, mut seen) = (Vec::new(), HashSet::new());
            for a in &alts {
                let terms = self.expand(a, visiting, pos);
                union_into(&mut out, &mut seen, terms);
            }
            visiting.remove(name);
            self.expanded.insert(name.to_string(), out.clone());
            return out;
        }
        if is_builtin_type(name) || is_tag_name(name) {
            return vec![TypeTerm::plain(name, t.pos)];
        }
        if self.reported.insert(name.to_string()) {
            self.errs.push(name_error("E0212", t.pos, name));
        }
        Vec::new()
    }
}
