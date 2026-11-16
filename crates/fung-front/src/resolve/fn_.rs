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
            held: Vec::new(),
            captures: Vec::new(),
            locals: Vec::new(),
            capture_names: Vec::new(),
        };
        let mut f = FnScope::new(&decl.name);
        if !decl.name.is_empty() {
            f.self_sig = Some(self.sig_of_node(n));
        }
        f.nparams = decl.params.len();
        f.key = super::effect::key(n);
        f.for_body = decl.for_body;
        f.top = top;
        self.fns.push(f);
        if !decl.name.is_empty() {
            *self.open.entry(decl.name.clone()).or_default() += 1;
        }

        self.check_types(&mut info, n.pos);
        // Literals the annotations settle (D113).
        let defaults = decl.params.iter().filter(|p| p.has_default);
        for (d, p) in n.kids.iter().zip(defaults) {
            let p = info
                .params
                .iter()
                .find(|q| q.name == p.name)
                .expect("param");
            if let Some(e) = super::annot::default_error(p, d) {
                self.errs.push(e);
            }
        }
        if let Some(ret) = &info.ret {
            super::annot::return_errors(ret, n.kids.last().expect("fn body"), &mut self.errs);
        }
        let body = n.kids.last().expect("fn body");
        self.push_block(body, false);
        if decl.for_body {
            self.cur_block().made = Made::For;
        }
        // The parameters are names of the body's block, and the defaults
        // are inside it: a prelude name a parameter binds is hidden in
        // every default before it is bound, its own too (D168), as a
        // statement's is in the lines before it (D139).
        for prm in &decl.params {
            if self.prelude.contains(prm.name.as_str())
                && let Some(h) = self.cur_mut().hiding.last_mut()
            {
                h.entry(prm.name.clone()).or_insert(prm.pos);
            }
        }
        let mut kids = Vec::new();
        let mut di = 0;
        // The parameters own slots 0..n from the start: a default's names
        // and hidden temporaries come after them, never in a later
        // parameter's slot (VM48).
        // A `_` name takes a slot no name had (`bind_here`), counted from
        // the highest used: the parameters' are used from the start.
        let f = self.cur_mut();
        f.next_slot = decl.params.len();
        f.max_slot = f.max_slot.max(f.next_slot);
        for (i, prm) in decl.params.iter().enumerate() {
            if prm.has_default {
                self.cur_mut().in_default = true;
                let d = self.walk(&n.kids[di]);
                self.cur_mut().in_default = false;
                kids.push(d);
                di += 1;
            }
            self.binds_fresh(&prm.name, prm.pos, false);
            let b = Binding {
                index: i,
                ..Binding::default()
            };
            self.bind_in_block(&prm.name, b);
            let sc = self.cur_block();
            sc.pos.insert(prm.name.clone(), prm.pos);
            sc.param.insert(prm.name.clone());
        }
        let f = self.cur_mut();
        f.max_slot = f.max_slot.max(f.next_slot);
        let stmts = body.kids.iter().map(|k| self.stmt(k)).collect();
        kids.push(Node::new(Head::Do, body.pos, stmts));
        self.pop_block(false);
        let mut f = self.fns.pop().expect("function stack");
        self.bodies
            .insert(super::effect::key(n), std::mem::take(&mut f.effects));
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
        info.held = f.held;
        info.capture_names = vec![String::new(); f.captures.len()];
        for (name, i) in f.cap_index {
            info.capture_names[i] = name;
        }
        info.captures = f.captures;
        info.locals = f.decls;
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
            // A rest parameter is always a list: `list` or nothing
            // (07, T3; D161 took `any` out).
            if p.rest
                && let Some(t) = &p.types
                && t.iter().any(|t| t.name != "list")
            {
                self.errs.push(super::name_error("E0228", p.pos, &p.name));
            }
        }
        if let Some(t) = &info.ret {
            info.ret = Some(self.expand_types(t, pos)).filter(|t| !t.is_empty());
        }
    }
}
