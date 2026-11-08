//! Type aliases (07).

use super::fn_::{is_builtin_type, is_tag_name};
use super::{Resolver, name_error};
use crate::source::Pos;
use crate::tree::{Head, Lit, Node};

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
        // `list[str]`: the names inside are checked, the outer type is the
        // one kept, and only list and map hold elements (07, FE17).
        if let Some((outer, inner)) = element_type(name) {
            self.check_elements(inner, pos);
            let out = self.expand(outer, visiting, pos);
            if out.iter().any(|t| t != "list" && t != "map")
                && self.reported.insert(name.to_string())
            {
                self.errs.push(name_error("E0232", pos, outer));
            }
            return out;
        }
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

impl Resolver<'_> {
    /// The names inside an element type (`str|list[int]`), in one pass:
    /// each must name a type, and one with an element type of its own must
    /// hold elements (E0232). They are not expanded, so a type may name
    /// itself inside (`type tree = list[tree]`), as a shape.
    fn check_elements(&mut self, inner: &str, pos: Pos) {
        let is_name = |c: char| c.is_alphanumeric() || c == '_';
        let mut rest = inner;
        while let Some(start) = rest.find(is_name) {
            rest = &rest[start..];
            let end = rest.find(|c: char| !is_name(c)).unwrap_or(rest.len());
            let name = &rest[..end];
            rest = &rest[end..];
            if rest.starts_with('[') {
                if !self.holds_elements(name) && self.reported.insert(format!("{name}[")) {
                    self.errs.push(name_error("E0232", pos, name));
                }
                continue;
            }
            let known =
                self.aliases.contains_key(name) || is_builtin_type(name) || is_tag_name(name);
            if !known && self.reported.insert(name.to_string()) {
                self.errs.push(name_error("E0212", pos, name));
            }
        }
    }
}

impl Resolver<'_> {
    /// Whether an element type may follow `name`: `list` or `map`, or an
    /// alias whose terms are (one level, so a recursive alias ends).
    fn holds_elements(&self, name: &str) -> bool {
        let container = |t: &str| {
            let base = t.split('[').next().unwrap_or(t);
            base == "list" || base == "map"
        };
        container(name)
            || self
                .aliases
                .get(name)
                .is_some_and(|alts| alts.iter().all(|t| container(t)))
    }
}

/// `list[str|int]` as (`list`, `str|int`); `None` for a plain name.
fn element_type(t: &str) -> Option<(&str, &str)> {
    let open = t.find('[')?;
    Some((&t[..open], t[open + 1..].strip_suffix(']')?))
}
