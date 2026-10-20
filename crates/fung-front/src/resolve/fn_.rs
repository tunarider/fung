//! Functions: parameters, defaults, bodies and type annotations.

use super::scope::{Binding, FnScope};
use super::{Resolver, fn_info};
use crate::source::Pos;
use crate::tree::{FnInfo, Head, Lit, Node};

/// The type names of 07; tag names (uppercase) are also accepted.
const BUILTIN_TYPES: &[&str] = &[
    "int", "float", "num", "str", "bool", "nil", "sym", "dur", "time", "list", "map", "tuple",
    "range", "fn", "module", "task", "scope", "atom", "sender", "receiver", "file", "any",
];

pub(super) fn is_builtin_type(name: &str) -> bool {
    BUILTIN_TYPES.contains(&name)
}

pub(super) fn is_tag_name(s: &str) -> bool {
    s.chars().next().is_some_and(crate::unicode::is_upper)
}

impl Resolver<'_> {
    /// Resolves an `(fn …)` node in a fresh function scope: params are
    /// slots 0..n-1, each default sees only the parameters before it (03),
    /// the body shares the parameters' block.
    pub(super) fn function(&mut self, n: &Node) -> Node {
        let decl = fn_info(n);
        let mut info = FnInfo {
            name: decl.name.clone(),
            params: decl.params.clone(),
            ret: decl.ret.clone(),
            for_body: false,
            expr_body: decl.expr_body,
            num_locals: 0,
            captures: Vec::new(),
        };
        let mut f = FnScope::new(&decl.name);
        f.for_body = decl.for_body;
        f.map_body = std::mem::take(&mut self.map_arg);
        f.top = std::mem::take(&mut self.top_arg);
        if f.top {
            super::own_bindings(n.kids.last().expect("fn body"), &mut f.own);
        }
        self.fns.push(f);

        self.check_types(&mut info, n.pos);
        let body = n.kids.last().expect("fn body");
        self.push_block(body, false);
        let mut kids = Vec::new();
        let mut di = 0;
        for (i, prm) in decl.params.iter().enumerate() {
            if prm.has_default {
                let d = self.walk(&n.kids[di]);
                kids.push(d);
                di += 1;
            }
            self.check_shadow(&prm.name, prm.pos);
            self.warn_shadow(&prm.name, prm.pos);
            let b = Binding {
                index: i,
                ..Binding::default()
            };
            let sc = self.cur_block();
            sc.names.insert(prm.name.clone(), b);
            sc.pos.insert(prm.name.clone(), prm.pos);
            sc.param.insert(prm.name.clone());
            self.cur_mut().next_slot = i + 1;
        }
        let f = self.cur_mut();
        f.max_slot = f.max_slot.max(f.next_slot);
        let stmts = body.kids.iter().map(|k| self.stmt(k)).collect();
        kids.push(Node::new(Head::Do, body.pos, stmts));
        self.pop_block(false);
        let f = self.fns.pop().expect("function stack");
        info.num_locals = f.max_slot;
        info.captures = f.captures;
        Node::with_lit(Head::Fn, n.pos, Lit::Fn(Box::new(info)), kids)
    }

    /// Expands aliases in a declaration's annotations in place and reports
    /// unknown type names (E0212) and alias cycles (E0218). An annotation
    /// that expands to nothing becomes unchecked.
    fn check_types(&mut self, info: &mut FnInfo, pos: Pos) {
        for p in &mut info.params {
            if let Some(t) = &p.types {
                p.types = Some(self.expand_types(t, p.pos)).filter(|t| !t.is_empty());
            }
            // A rest parameter is always a list (07, T3).
            if p.rest
                && let Some(t) = &p.types
                && t.iter().any(|t| t != "list" && t != "any")
            {
                self.errs.push(super::name_error("E0228", p.pos, &p.name));
            }
        }
        if let Some(t) = &info.ret {
            info.ret = Some(self.expand_types(t, pos)).filter(|t| !t.is_empty());
        }
    }
}
