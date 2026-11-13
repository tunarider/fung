//! Functions: parameters, defaults, bodies and type annotations.

use super::scope::{Binding, FnScope, Made};
use super::{Resolver, fn_info};
use crate::source::Pos;
use crate::tree::{FnInfo, Head, Lit, Node};

/// The type names of 07; tag names (uppercase) are also accepted. The
/// VM's type checks know the same names (`fung_vm` `value/types.rs`).
pub const BUILTIN_TYPES: &[&str] = &[
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
    /// the body shares the parameters' block. `top`: a hoisted top-level
    /// fn (FE62: passed, not left in a field for the next call).
    pub(super) fn function(&mut self, n: &Node, top: bool) -> Node {
        let decl = fn_info(n);
        let mut info = FnInfo {
            name: decl.name.clone(),
            name_pos: decl.name_pos,
            params: decl.params.clone(),
            ret: decl.ret.clone(),
            for_body: false,
            lambda: false,
            num_locals: 0,
            captures: Vec::new(),
        };
        let mut f = FnScope::new(&decl.name);
        if !decl.name.is_empty() {
            f.self_sig = Some(super::call::Sig::of(decl));
        }
        f.for_body = decl.for_body;
        f.lambda = decl.lambda;
        f.top = top;
        // The parameters of a module's public fns are its interface: callers
        // name them (`timeout: 5s`), so they may be prelude names (D38).
        let interface = f.top && self.interface_name(&decl.name);
        self.fns.push(f);
        if !decl.name.is_empty() {
            *self.open.entry(decl.name.clone()).or_default() += 1;
        }

        self.check_types(&mut info, n.pos);
        let body = n.kids.last().expect("fn body");
        self.push_block(body, false);
        if decl.for_body {
            self.cur_block().made = Made::For;
        }
        let mut kids = Vec::new();
        let mut di = 0;
        for (i, prm) in decl.params.iter().enumerate() {
            if prm.has_default {
                self.cur_mut().in_default = true;
                let d = self.walk(&n.kids[di]);
                self.cur_mut().in_default = false;
                kids.push(d);
                di += 1;
            }
            self.binds_fresh(&prm.name, prm.pos, false, interface);
            let b = Binding {
                index: i,
                ..Binding::default()
            };
            self.bind_in_block(&prm.name, b);
            let sc = self.cur_block();
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
        if !decl.name.is_empty() {
            self.unopen(&decl.name);
        }
        if f.for_body {
            // A `for` binds in the block it stands in, like `if` and
            // `match`: its names are closed there after it (FE33).
            let parent = self.cur_mut();
            for (name, made) in f.closed {
                if !crate::tree::is_hidden(&name) {
                    parent.close(name, made);
                }
            }
        }
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
                && t.iter().any(|t| t.name != "list" && t.name != "any")
            {
                self.errs.push(super::name_error("E0228", p.pos, &p.name));
            }
        }
        if let Some(t) = &info.ret {
            info.ret = Some(self.expand_types(t, pos)).filter(|t| !t.is_empty());
        }
    }
}
