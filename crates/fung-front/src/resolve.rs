//! Classifies every name in a core tree and reports scope errors (09):
//! block locals become `(local n)`, captured values `(captured n)`, a named
//! local fn's own name `(self)`, file globals `(global n)`, prelude names
//! `(prelude name)`. It walks once, rewrites name leaves into what the
//! compiler needs, and collects every error (08), in the order found.
//!
//! The input is borrowed and never modified. Recursion follows tree depth,
//! so callers with very deep trees need a large stack (see `parser`).

mod alias;
mod fn_;
mod module;
mod pattern;
mod scope;
mod similar;
#[cfg(test)]
mod tests;
mod warn;

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::diag::{Arg, Diag, error};
use crate::source::Pos;
use crate::tree::{Capture, CaptureKind, FnInfo, Head, Lit, Node};

use scope::{Binding, FnScope, Scope};

/// Former prelude names and the module that has them now (stage 6a): an
/// undefined use of one gets a help line that names the module.
pub const MOVED_TO_MODULE: &[(&str, &str)] =
    &[("fan_in", "conc"), ("pmap", "conc"), ("retry", "conc")];

/// What resolve knows about an imported module: enough to check member
/// names at compile time (04).
#[derive(Clone, Debug, Default)]
pub struct ModuleInfo {
    pub key: String,
    pub name: String,
    pub exports: Vec<String>,
}

/// Configures a file's resolution.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Import spec as written → module.
    pub imports: BTreeMap<String, ModuleInfo>,
    /// An imported file: top-level restrictions (04).
    pub library: bool,
    /// REPL: the globals earlier inputs defined, in index order. They
    /// keep their indexes and count as read.
    pub globals: Vec<String>,
    /// REPL: fns and imports may be defined again.
    pub redefine: bool,
}

/// The resolved tree plus what the compiler needs to lay out storage.
#[derive(Debug)]
pub struct Resolved {
    pub core: Node,
    /// Block-local slots the main frame needs.
    pub num_locals: usize,
    /// The file's globals, in index order.
    pub globals: Vec<String>,
    /// Module keys this file imports, in order.
    pub imports: Vec<String>,
    /// Public globals: not private, not hidden, not bound by import (04).
    pub exports: Vec<String>,
    /// W01xx warnings, sorted by position (08).
    pub warnings: Vec<Diag>,
}

/// Resolves a core tree against the prelude names (sorted) and the modules
/// it may import.
pub fn resolve(core: &Node, prelude: &[String], opts: &Options) -> Result<Resolved, Vec<Diag>> {
    let mut r = Resolver {
        prelude: prelude.iter().map(String::as_str).collect(),
        prelude_list: prelude,
        fns: vec![FnScope::new("")],
        globals: Vec::new(),
        file_var_pos: HashMap::new(),
        aliases: HashMap::new(),
        alias_order: Vec::new(),
        alias_pos: HashMap::new(),
        reported: HashSet::new(),
        opts,
        imports: Vec::new(),
        module_names: HashMap::new(),
        import_bound: HashSet::new(),
        errs: Vec::new(),
        warnings: Vec::new(),
    };
    let out = r.root_block(core);
    warn::sort_warnings(&mut r.warnings);
    if !r.errs.is_empty() {
        return Err(r.errs);
    }
    let exports = r
        .globals
        .iter()
        .filter(|g| !g.starts_with('_') && !g.starts_with('$') && !r.import_bound.contains(*g))
        .cloned()
        .collect();
    Ok(Resolved {
        core: out,
        num_locals: r.fns[0].max_slot,
        globals: r.globals,
        imports: r.imports,
        exports,
        warnings: r.warnings,
    })
}

struct Resolver<'a> {
    prelude: HashSet<&'a str>,
    prelude_list: &'a [String],
    /// The functions being resolved, outermost (the file) first.
    fns: Vec<FnScope>,
    globals: Vec<String>,
    /// Where each file-level variable is first assigned, only to explain
    /// why a function cannot see it (03: functions copy the variables
    /// visible when they are created; top-level fns are created before any
    /// variable exists).
    file_var_pos: HashMap<String, Pos>,
    aliases: HashMap<String, Vec<String>>,
    /// Alias names in declaration order.
    alias_order: Vec<String>,
    alias_pos: HashMap<String, Pos>,
    /// Type names already reported, to avoid repeats.
    reported: HashSet<String>,
    opts: &'a Options,
    imports: Vec<String>,
    /// Names bound to modules (unshadowable).
    module_names: HashMap<String, ModuleInfo>,
    /// Every name an import bound: never re-exported (04).
    import_bound: HashSet<String>,
    errs: Vec<Diag>,
    warnings: Vec<Diag>,
}

/// Where a captured name was found.
enum Found {
    /// A top-level fn: referenced directly as `(global n)`, not copied.
    Global(usize),
    /// The capture's index in the function.
    Captured(usize),
}

fn str_arg(s: &str) -> Arg {
    Arg::Str(s.to_string())
}

fn rune_len(s: &str) -> usize {
    s.chars().count()
}

/// An error about a name, underlining the whole name.
fn name_error(code: &str, pos: Pos, name: &str) -> Diag {
    error(code, pos, vec![str_arg(name)]).span(rune_len(name))
}

fn lit_str(n: &Node) -> &str {
    match &n.lit {
        Lit::Str(s) => s,
        _ => "",
    }
}

fn fn_info(n: &Node) -> &FnInfo {
    match &n.lit {
        Lit::Fn(f) => f,
        _ => unreachable!("fn node without FnInfo"),
    }
}

/// The name of a named fn definition statement.
fn fn_stmt(n: &Node) -> Option<&str> {
    if n.head != Head::Fn {
        return None;
    }
    let info = fn_info(n);
    (!info.name.is_empty()).then_some(info.name.as_str())
}

fn ref_node(pos: Pos, b: Binding) -> Node {
    let head = if b.global { Head::Global } else { Head::Local };
    Node::leaf(head, pos, Lit::Int(b.index as i64))
}

impl Resolver<'_> {
    fn cur(&self) -> &FnScope {
        self.fns.last().expect("function stack")
    }

    fn cur_mut(&mut self) -> &mut FnScope {
        self.fns.last_mut().expect("function stack")
    }

    /// The innermost block of the current function.
    fn cur_block(&mut self) -> &mut Scope {
        self.cur_mut().blocks.last_mut().expect("block stack")
    }

    /// The file scope.
    fn root_scope(&mut self) -> &mut Scope {
        &mut self.fns[0].blocks[0]
    }

    /// Whether the current function is a real function, not the file.
    fn in_function(&self) -> bool {
        self.fns.len() > 1
    }

    /// Resolves the file scope. Imports and top-level fn statements are
    /// bound first, in source order, as constant globals and emitted first
    /// (03, 04), then the remaining statements in order. In a library file
    /// those statements may only be assignments (04, E0306).
    fn root_block(&mut self, n: &Node) -> Node {
        self.push_block(n, true);
        // REPL: the globals of earlier inputs keep their indexes (08).
        for (i, g) in self.opts.globals.iter().enumerate() {
            let b = Binding {
                global: true,
                index: i,
                ..Binding::default()
            };
            let sc = self.root_scope();
            sc.names.insert(g.clone(), b);
            sc.used.insert(g.clone());
            self.globals.push(g.clone());
        }
        self.declare_aliases(n);
        let mut kids = Vec::new();
        let mut fns: Vec<&Node> = Vec::new();
        let mut rest: Vec<&Node> = Vec::new();
        for k in &n.kids {
            if k.head == Head::Typealias {
                continue; // consumed by declare_aliases
            }
            if k.head == Head::Import {
                let im = self.import_stmt(k);
                kids.push(im);
                continue;
            }
            let Some(name) = fn_stmt(k) else {
                rest.push(k);
                continue;
            };
            if let Some(b) = self.root_scope().names.get(name).copied() {
                if self.opts.redefine && !b.module {
                    // REPL: the fn takes the name's storage over (08).
                    let b = Binding {
                        constant: true,
                        ..b
                    };
                    self.root_scope().names.insert(name.to_string(), b);
                    fns.push(k);
                    continue;
                }
                let e = if b.module {
                    module::shadow_error(name, k.pos)
                } else {
                    name_error("E0210", k.pos, name)
                };
                self.errs.push(e);
                continue;
            }
            self.warn_shadow(name, k.pos);
            let b = Binding {
                global: true,
                index: self.globals.len(),
                constant: true,
                module: false,
            };
            self.root_scope().names.insert(name.to_string(), b);
            self.root_scope().pos.insert(name.to_string(), k.pos);
            self.globals.push(name.to_string());
            fns.push(k);
        }
        for k in &rest {
            if k.head == Head::Let && k.kids[0].head == Head::Name {
                self.file_var_pos
                    .entry(lit_str(&k.kids[0]).to_string())
                    .or_insert(k.kids[0].pos);
            }
        }
        for k in fns {
            let b = self.root_scope().names[fn_info(k).name.as_str()];
            let f = self.function(k);
            kids.push(Node::new(Head::Let, k.pos, vec![ref_node(k.pos, b), f]));
        }
        for k in rest {
            if self.opts.library && k.head != Head::Let && k.head != Head::Test {
                self.errs.push(error("E0306", k.pos, vec![]));
                continue;
            }
            let w = self.walk(k);
            kids.push(w);
        }
        self.warn_unused_imports();
        self.pop_block(true);
        Node::new(Head::Do, n.pos, kids)
    }

    /// Opens a block scope in the current function and notes which local
    /// fns the block declares later (for E0208).
    fn push_block(&mut self, n: &Node, file: bool) {
        let later: HashSet<String> = if file {
            HashSet::new()
        } else {
            n.kids
                .iter()
                .filter_map(fn_stmt)
                .map(str::to_string)
                .collect()
        };
        let f = self.cur_mut();
        let sc = Scope::new(file, f.next_slot);
        f.blocks.push(sc);
        f.later.push(later);
    }

    /// Closes a block: its names become "closed" for E0206 and its slots
    /// are released for reuse (03).
    fn pop_block(&mut self, file: bool) {
        let f = self.cur_mut();
        let sc = f.blocks.pop().expect("block stack");
        f.later.pop();
        if !file {
            f.next_slot = sc.base;
            self.warn_unused(&sc);
            self.cur_mut().closed.extend(sc.names.into_keys());
        }
    }

    /// Resolves one node; a rewritten node inherits the source span of the
    /// node it came from (assert and dbg show source text, 08).
    fn walk(&mut self, n: &Node) -> Node {
        let mut out = self.walk_node(n);
        if out.end == 0 {
            out.off = n.off;
            out.end = n.end;
        }
        out
    }

    fn walk_node(&mut self, n: &Node) -> Node {
        match n.head {
            Head::Do => return self.block(n),
            Head::Let => {
                self.warn_block_value(&n.kids[1]);
                return self.let_(n);
            }
            Head::Name => return self.name(n),
            Head::Fn => return self.fn_node(n),
            Head::Match => return self.match_expr(n),
            Head::Test => {
                // The test body is a function made where it stands (08), not
                // a named local fn: nothing binds its name.
                let f = self.function(&n.kids[0]);
                return Node::with_lit(Head::Test, n.pos, n.lit.clone(), vec![f]);
            }
            Head::Hole => {
                self.errs.push(error("E0215", n.pos, vec![]));
                return n.clone();
            }
            Head::Typealias => {
                self.errs.push(error("E0219", n.pos, vec![]));
                return Node::new(Head::Nil, n.pos, vec![]);
            }
            Head::Import => {
                // root_block consumes top-level imports; any other is nested.
                self.errs.push(error("E0311", n.pos, vec![]));
                return Node::new(Head::Nil, n.pos, vec![]);
            }
            Head::Call => {
                for k in &n.kids[1..] {
                    self.warn_block_value(k);
                }
            }
            Head::Return => {
                if let Some(k) = n.kids.first() {
                    self.warn_block_value(k);
                }
                if !self.in_function() {
                    self.errs.push(error("E0211", n.pos, vec![]));
                } else if self.cur().for_body {
                    self.errs.push(error("E0213", n.pos, vec![]));
                }
            }
            Head::Unwrap if lit_str(n) == "?" => {
                if !self.in_function() {
                    self.errs.push(error("E0214", n.pos, vec![]));
                } else if self.cur().for_body {
                    // The for body is a lambda, so `?` would return from it
                    // and drop an Err into the list: the silent bug E0213
                    // prevents.
                    self.errs.push(error("E0220", n.pos, vec![]));
                }
            }
            Head::Field => self.check_member(&n.kids[0], lit_str(n), n.pos),
            _ => {}
        }
        let kids = n.kids.iter().map(|k| self.walk(k)).collect();
        Node::with_lit(n.head, n.pos, n.lit.clone(), kids)
    }

    /// A named fn inside a block: resolve the body first (inside it the
    /// name is `(self)`), then bind the name as a local.
    fn fn_node(&mut self, n: &Node) -> Node {
        let f = self.function(n);
        let Some(name) = fn_stmt(n) else {
            return f;
        };
        match self.bind_new(name, n.pos) {
            Some(b) => Node::new(Head::Let, n.pos, vec![ref_node(n.pos, b), f]),
            None => f,
        }
    }

    fn block(&mut self, n: &Node) -> Node {
        self.push_block(n, false);
        let kids = n.kids.iter().map(|k| self.stmt(k)).collect();
        self.pop_block(false);
        Node::new(Head::Do, n.pos, kids)
    }

    /// Resolves one statement of the current block, marking a local fn as
    /// no longer "later" once its statement is reached.
    fn stmt(&mut self, k: &Node) -> Node {
        if let Some(name) = fn_stmt(k)
            && let Some(later) = self.cur_mut().later.last_mut()
        {
            later.remove(name);
        }
        self.walk(k)
    }

    /// Binds a name in the current block for an assignment: rebinding in
    /// the same block reuses the storage; a name from an enclosing scope is
    /// E0205 (01); a hoisted fn is E0209. Returns `None` when the
    /// assignment is an error.
    fn bind_new(&mut self, name: &str, pos: Pos) -> Option<Binding> {
        if let Some(b) = self.cur_block().names.get(name).copied() {
            if b.module && !self.opts.redefine {
                self.errs.push(module::shadow_error(name, pos));
                return None;
            }
            if b.constant && !self.opts.redefine {
                self.errs.push(name_error("E0209", pos, name));
                return None;
            }
            if b.constant || b.module {
                // REPL: a plain variable replaces the fn or module (08).
                let b = Binding {
                    global: b.global,
                    index: b.index,
                    ..Binding::default()
                };
                self.cur_block().names.insert(name.to_string(), b);
                self.module_names.remove(name);
                return Some(b);
            }
            return Some(b);
        }
        if !self.check_shadow(name, pos) {
            return None;
        }
        self.warn_shadow(name, pos);
        if self.defined_outside(name) {
            self.errs.push(name_error("E0205", pos, name));
            return None;
        }
        Some(self.bind_here(name, pos))
    }

    /// Binds name as a new global (file scope) or slot (block) in the
    /// current block, bound at `pos` (for warnings).
    fn bind_here(&mut self, name: &str, pos: Pos) -> Binding {
        let b = if self.cur_block().file {
            let b = Binding {
                global: true,
                index: self.globals.len(),
                ..Binding::default()
            };
            self.globals.push(name.to_string());
            b
        } else {
            Binding {
                index: self.cur_mut().alloc_slot(),
                ..Binding::default()
            }
        };
        let sc = self.cur_block();
        sc.names.insert(name.to_string(), b);
        sc.pos.insert(name.to_string(), pos);
        b
    }

    /// Whether name is bound in an enclosing block of this function, in any
    /// enclosing function, or as a function's own name. Only names bound so
    /// far count: scopes are read top-down, and a hoisted top-level fn is
    /// resolved before any file variable exists.
    fn defined_outside(&self, name: &str) -> bool {
        let last = self.fns.len() - 1;
        self.fns.iter().enumerate().rev().any(|(i, f)| {
            let skip = usize::from(i == last);
            let outer = &f.blocks[..f.blocks.len().saturating_sub(skip)];
            outer.iter().any(|b| b.names.contains_key(name)) || f.self_name == name
        })
    }

    /// Resolves the right-hand side first (`x = x + 1` reads the old x),
    /// then binds the name — or, for destructuring, every name of the
    /// pattern under the same assignment rules (03).
    fn let_(&mut self, n: &Node) -> Node {
        let target = &n.kids[0];
        let rhs = self.walk(&n.kids[1]);
        if target.head != Head::Name {
            // Destructuring assignment binds like assignment; the destructure
            // desugar emits for a `for` pattern binds fresh names (03).
            let mode = if lit_str(n) == "fresh" {
                pattern::Mode::Fresh
            } else {
                pattern::Mode::Assign
            };
            let pat = self.bind_pattern(target, mode);
            return Node::new(Head::Let, n.pos, vec![pat, rhs]);
        }
        let lhs = match self.bind_new(lit_str(target), target.pos) {
            Some(b) => ref_node(target.pos, b),
            None => target.clone(),
        };
        Node::new(Head::Let, n.pos, vec![lhs, rhs])
    }

    /// Resolves a read.
    fn name(&mut self, n: &Node) -> Node {
        let name = lit_str(n);
        if name == "_" {
            self.errs.push(error("E0215", n.pos, vec![]));
            return n.clone();
        }
        if let Some(node) = self.lookup(name, n.pos) {
            return node;
        }
        if self.prelude.contains(name) {
            return Node::leaf(Head::Prelude, n.pos, Lit::Str(name.to_string()));
        }
        if self.cur().closed.contains(name) {
            let e = name_error("E0206", n.pos, name).with_help(&format!(
                "assign the block's value instead: `{name} = if … ;;`"
            ));
            self.errs.push(e);
            return n.clone();
        }
        if self
            .fns
            .iter()
            .any(|f| f.later.iter().any(|l| l.contains(name)))
        {
            self.errs.push(name_error("E0208", n.pos, name));
            return n.clone();
        }
        let mut e = name_error("E0204", n.pos, name);
        if let (Some(p), true) = (self.file_var_pos.get(name), self.in_function()) {
            e = e.with_help(&format!(
                "`{name}` is a file-level variable (assigned on line {}) that this function cannot see: functions copy only the variables that exist when they are created, and top-level fns are created first; pass it as an argument",
                p.line
            ));
        } else if let Some((_, module)) = MOVED_TO_MODULE.iter().find(|(n, _)| *n == name) {
            e = e.with_help(&format!(
                "moved to module {module}: import {module}, then {module}.{name}"
            ));
        } else if let Some(s) = similar::similar(name, &self.visible()) {
            e = e.with_help(&format!("a similar name exists: `{s}`"));
        }
        self.errs.push(e);
        n.clone()
    }

    /// Finds name in the current function: its blocks, its own name, then
    /// the enclosing functions (captured through every function in
    /// between). A top-level fn (a constant) is referenced directly; a file
    /// variable is copied like any other captured value (03).
    fn lookup(&mut self, name: &str, pos: Pos) -> Option<Node> {
        let f = self.cur_mut();
        if let Some(sc) = f
            .blocks
            .iter_mut()
            .rev()
            .find(|b| b.names.contains_key(name))
        {
            sc.used.insert(name.to_string());
            return Some(ref_node(pos, sc.names[name]));
        }
        if f.self_name == name {
            return Some(Node::new(Head::SelfFn, pos, vec![]));
        }
        if !self.in_function() {
            return None;
        }
        let (head, index) = match self.capture_from(name, self.fns.len() - 1)? {
            Found::Global(i) => (Head::Global, i),
            Found::Captured(i) => (Head::Captured, i),
        };
        Some(Node::leaf(head, pos, Lit::Int(index as i64)))
    }

    /// Makes name available inside function `fi` by capturing it from its
    /// parent (recursively through the chain).
    fn capture_from(&mut self, name: &str, fi: usize) -> Option<Found> {
        if let Some(&idx) = self.fns[fi].cap_index.get(name) {
            return Some(Found::Captured(idx));
        }
        let pi = fi - 1;
        let p = &mut self.fns[pi];
        let mut src = None;
        let found = p.blocks.iter_mut().rev().find_map(|sc| {
            let b = *sc.names.get(name)?;
            sc.used.insert(name.to_string());
            Some(b)
        });
        if let Some(b) = found {
            if b.global && b.constant {
                // Another top-level fn: it never changes, so a direct
                // reference is indistinguishable from a copy.
                return Some(Found::Global(b.index));
            }
            let kind = if b.global {
                CaptureKind::Global
            } else {
                CaptureKind::Local
            };
            src = Some(Capture {
                kind,
                index: b.index,
            });
        }
        if src.is_none() && p.self_name == name {
            src = Some(Capture {
                kind: CaptureKind::SelfFn,
                index: 0,
            });
        }
        if src.is_none() && pi > 0 {
            match self.capture_from(name, pi)? {
                Found::Global(i) => return Some(Found::Global(i)),
                Found::Captured(i) => {
                    src = Some(Capture {
                        kind: CaptureKind::Captured,
                        index: i,
                    })
                }
            }
        }
        let src = src?;
        let f = &mut self.fns[fi];
        f.captures.push(src);
        let idx = f.captures.len() - 1;
        f.cap_index.insert(name.to_string(), idx);
        Some(Found::Captured(idx))
    }

    /// Every name that could have been meant: all blocks of all enclosing
    /// functions, closed block names, and the prelude.
    fn visible(&self) -> Vec<String> {
        let mut names = Vec::new();
        for f in self.fns.iter().rev() {
            for sc in &f.blocks {
                names.extend(sc.names.keys().cloned());
            }
            names.extend(f.closed.iter().cloned());
        }
        names.extend(self.prelude_list.iter().cloned());
        names
    }
}
