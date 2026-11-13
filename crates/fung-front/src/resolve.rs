//! Classifies every name in a core tree and reports scope errors (09):
//! block locals become `(local n)`, captured values `(captured n)`, a named
//! local fn's own name `(self)`, file globals `(global n)`, prelude names
//! `(prelude name)`. It walks once, rewrites name leaves into what the
//! compiler needs, and collects every error (08), in the order found.
//!
//! The input is borrowed and never modified. Recursion follows tree depth,
//! so callers with very deep trees need a large stack (see `parser`).

mod alias;
mod call;
mod fn_;
mod module;
mod names;
mod pattern;
mod scope;
mod similar;
#[cfg(test)]
mod tests;
mod value;

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::diag::{Arg, Diag, error};
use crate::source::Pos;
use crate::tree::{Capture, CaptureKind, FnInfo, Head, Lit, Node, TypeTerm};

use scope::{Binding, FnScope, Made, Scope};

pub use call::{Sig, SigParam};
pub use fn_::BUILTIN_TYPES;

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
    /// The parameters of the exported functions whose calls are checked
    /// at compile time (D43): top-level fns of a fung module, members of
    /// a `_native.*` bridge.
    pub sigs: BTreeMap<String, Sig>,
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
    /// The file's last expression is its value (the REPL prints it), as
    /// `desugar`'s `keep_value`; otherwise that place is discarded.
    pub keep_value: bool,
    /// The built-in names whose parameters prelude calls are checked
    /// against (D43); `None` is `spec/builtins.txt`.
    pub spec: Option<&'static crate::builtins::Builtins>,
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
    /// The parameters of the exported top-level fns (D43).
    pub sigs: BTreeMap<String, Sig>,
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
        expanded: HashMap::new(),
        reported: HashSet::new(),
        opts,
        imports: Vec::new(),
        module_names: HashMap::new(),
        module_specs: HashMap::new(),
        import_bound: HashMap::new(),
        errs: Vec::new(),
        consts: HashMap::new(),
        const_pos: HashMap::new(),
        open: HashMap::new(),
        pattern_bound: HashSet::new(),
        global_sigs: HashMap::new(),
        lambda_targets: Vec::new(),
        similar_index: None,
        similar_indexed: 0,
        slots: HashMap::new(),
        pure_calls: HashMap::new(),
    };
    let mut out = r.root_block(core);
    out.number();
    // The file's last item is discarded (01), or printed by the REPL.
    let pure_calls = std::mem::take(&mut r.pure_calls);
    value::check_file(core, opts.keep_value, &pure_calls, &mut r.errs);
    // An unused name is often another error's doing (a misspelt read, a
    // binding that hides): it is reported once the rest is fixed (D38).
    if r.errs.iter().any(|e| e.code != "E0239") {
        r.errs.retain(|e| e.code != "E0239");
    }
    // In source order, whichever pass found them (FE73).
    r.errs.sort_by_key(|e| (e.pos.line, e.pos.col));
    if !r.errs.is_empty() {
        return Err(r.errs);
    }
    let exports: Vec<String> = r
        .globals
        .iter()
        .filter(|g| {
            !g.starts_with('_') && !crate::tree::is_hidden(g) && !r.import_bound.contains_key(*g)
        })
        .cloned()
        .collect();
    let exported: std::collections::HashSet<&String> = exports.iter().collect();
    let sigs = r
        .global_sigs
        .iter()
        .filter(|(i, _)| exported.contains(&r.globals[**i]))
        .map(|(i, (_, sig))| (r.globals[*i].clone(), sig.clone()))
        .collect();
    Ok(Resolved {
        core: out,
        num_locals: r.fns[0].max_slot,
        globals: r.globals,
        imports: r.imports,
        exports,
        sigs,
    })
}

struct Resolver<'a> {
    /// The calls of functions known to have no effect, by position, with
    /// the name the error shows (D39; `value::check_file`).
    pure_calls: value::Pure,
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
    aliases: HashMap<String, Vec<TypeTerm>>,
    /// Alias names in declaration order.
    alias_order: Vec<String>,
    alias_pos: HashMap<String, Pos>,
    /// Each alias's union once expanded (FE40: a later use reuses it).
    expanded: HashMap<String, Vec<TypeTerm>>,
    /// Type names already reported, to avoid repeats.
    reported: HashSet<String>,
    opts: &'a Options,
    imports: Vec<String>,
    /// Names bound to modules (unshadowable).
    module_names: HashMap<String, ModuleInfo>,
    /// How each module name's import names its module (`fs`,
    /// `"./lib/x"`), for E0308's help.
    module_specs: HashMap<String, String>,
    /// Every name an import bound, with the length of where it is written
    /// (for E0239's underline): never re-exported (04).
    import_bound: HashMap<String, usize>,
    errs: Vec<Diag>,
    /// The file's constants: every name a top-level statement binds, once
    /// (03, L9, D53; every name an input binds in the REPL), with their
    /// global slots taken before the top-level fns are resolved.
    consts: HashMap<String, usize>,
    /// Where each constant is first bound, for E0240's note.
    const_pos: HashMap<String, Pos>,
    /// How many open blocks below the file's own (in any function) bind
    /// each name, its own names of named local fns included: a name none
    /// binds is found in the file's block or nowhere (`outer_name`, so a
    /// deep nest of functions stays linear).
    open: HashMap<String, usize>,
    /// The names the pattern being bound has bound so far: a later
    /// alternative of it binds them again (`bind_pattern`).
    pattern_bound: HashSet<String>,
    /// The global slots that hold a known function (D43): a top-level fn
    /// of the file or a name an import bound, with its name and
    /// parameters. Empty in the REPL, where a later input may define the
    /// name again.
    global_sigs: HashMap<usize, (String, Sig)>,
    /// The "similar name" index (FE36), made at the first undefined name,
    /// and how many of the file's lasting names it holds.
    similar_index: Option<similar::Index>,
    similar_indexed: usize,
    /// The names being assigned a lambda, innermost last: a read of one
    /// inside its lambda is the lambda calling itself (E0204's help).
    lambda_targets: Vec<String>,
    /// The global slot of every name a top-level statement binds, taken in
    /// the order the names first appear (as binding them in turn would).
    slots: HashMap<String, usize>,
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
    /// E0237 for `return` or `?` (`what`) in a lambda `=>` or a `for`
    /// body: neither can return early (D42). A `for` body is a lambda
    /// that builds the list, so it is the same rule.
    fn early_return(&self, what: &str, pos: Pos) -> Option<Diag> {
        let f = self.cur();
        let place = if f.for_body {
            "a `for` body"
        } else if f.lambda {
            "a lambda `=>`"
        } else {
            return None;
        };
        let len = if what == "?" { 1 } else { 6 };
        Some(error("E0237", pos, vec![str_arg(what), str_arg(place)]).span(len))
    }

    /// The built-in names prelude calls are checked against.
    fn builtins(&self) -> &'static crate::builtins::Builtins {
        self.opts.spec.unwrap_or_else(crate::builtins::builtins)
    }

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

    /// Binds name in the innermost block of the current function.
    fn bind_in_block(&mut self, name: &str, b: Binding) {
        let root = self.fns.len() == 1 && self.cur().blocks.len() == 1;
        let f = self.cur_mut();
        let i = f.blocks.len() - 1;
        if f.bind(i, name, b) && !root {
            *self.open.entry(name.to_string()).or_default() += 1;
        }
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
            self.fns[0].bind(0, g, b);
            self.root_scope().used.insert(g.clone());
            self.globals.push(g.clone());
        }
        self.declare_aliases(n);
        self.check_test_names(n);
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
            let at = fn_info(k).name_pos;
            if let Some(b) = self.root_scope().names.get(name).copied() {
                if self.opts.redefine && !b.module {
                    // REPL: the fn takes the name's storage over (08).
                    let b = Binding {
                        constant: true,
                        ..b
                    };
                    self.fns[0].bind(0, name, b);
                    fns.push(k);
                    continue;
                }
                let e = if b.module {
                    self.shadow_error(name, at)
                } else {
                    let first = self.root_scope().pos.get(name).copied();
                    defined_twice(name, at, first)
                };
                self.errs.push(e);
                continue;
            }
            let interface = self.interface_name(name);
            self.binds_fresh(name, at, false, interface);
            let b = Binding {
                global: true,
                index: self.globals.len(),
                constant: true,
                module: false,
            };
            self.fns[0].bind(0, name, b);
            self.root_scope().pos.insert(name.to_string(), at);
            self.globals.push(name.to_string());
            if !self.opts.redefine {
                let sig = call::Sig::of(fn_info(k));
                self.global_sigs.insert(b.index, (name.to_string(), sig));
            }
            fns.push(k);
        }
        for k in &rest {
            if k.head == Head::Let && k.kids[0].head == Head::Name {
                self.file_var_pos
                    .entry(lit_str(&k.kids[0]).to_string())
                    .or_insert(k.kids[0].pos);
            }
        }
        self.declare_consts(&rest);
        self.note_aliases(&rest);
        for k in fns {
            let b = self.root_scope().names[fn_info(k).name.as_str()];
            let f = self.function(k, true);
            kids.push(Node::new(Head::Let, k.pos, vec![ref_node(k.pos, b), f]));
        }
        for k in rest {
            // `_ = e` keeps no value either: it is a bare statement under
            // another spelling (FE69).
            let discards = k.head == Head::Let && k.kids[0].head == Head::Pwild;
            if self.opts.library && ((k.head != Head::Let && k.head != Head::Test) || discards) {
                // At the statement's start, not a call's `(` (FE29).
                self.errs.push(error("E0306", k.start_pos(), vec![]));
                continue;
            }
            let w = self.walk(k);
            kids.push(w);
        }
        self.report_unused_file();
        self.pop_block(true);
        Node::new(Head::Do, n.pos, kids)
    }

    /// Takes the global slots of the file's constants (03, L9): every name
    /// a top-level assignment binds, since each is bound once (D53; a
    /// second binding is E0238 where it stands), and in the REPL every name
    /// an input binds. A name the file already has (a fn, a module, an
    /// earlier input's global) keeps its own binding.
    fn declare_consts(&mut self, rest: &[&Node]) {
        let mut seen = HashSet::new();
        for k in rest {
            if k.head != Head::Let || lit_str(k) == crate::tree::FRESH {
                continue;
            }
            let mut names: Vec<(String, Pos)> = Vec::new();
            k.kids[0].bound_names(&mut |name, pos| names.push((name.to_string(), pos)));
            for (name, pos) in names {
                if !seen.insert(name.clone()) || self.root_scope().names.contains_key(&name) {
                    continue;
                }
                self.const_pos.insert(name.clone(), pos);
                let i = self.globals.len();
                self.globals.push(name.clone());
                self.slots.insert(name.clone(), i);
                self.consts.insert(name, i);
            }
        }
    }

    /// Whether the function being resolved is a top-level fn or inside one.
    fn in_top_fn(&self) -> bool {
        self.fns.get(1).is_some_and(|f| f.top)
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
        let sc = f.pop_scope();
        f.later.pop();
        if !file {
            f.next_slot = sc.base;
            for name in sc.names.keys() {
                self.unopen(name);
            }
            self.report_unused(&sc);
            let f = self.cur_mut();
            for name in sc.names.into_keys() {
                f.close(name, sc.made);
            }
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
            Head::Let => return self.let_(n),
            Head::Name => return self.name(n),
            Head::Fn => return self.fn_node(n),
            Head::Match => return self.match_expr(n),
            Head::Test => {
                // The test body is a function made where it stands (08), not
                // a named local fn: nothing binds its name.
                let f = self.function(&n.kids[0], false);
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
            Head::Return => {
                if !self.in_function() {
                    let args = vec![str_arg("return"), str_arg("outside a function")];
                    self.errs.push(error("E0211", n.pos, args));
                } else if self.cur().in_default {
                    self.errs.push(return_in_default("return", n.pos));
                } else if let Some(e) = self.early_return("return", n.pos) {
                    self.errs.push(e);
                }
            }
            Head::Unwrap if lit_str(n) == "?" => {
                if !self.in_function() {
                    self.errs.push(error("E0214", n.pos, vec![]));
                } else if self.cur().in_default {
                    self.errs.push(return_in_default("`?`", n.pos));
                } else if let Some(e) = self.early_return("?", n.pos) {
                    self.errs.push(e);
                }
            }
            Head::Field => self.check_member(&n.kids[0], lit_str(n), n.pos),
            Head::Call => return self.call(n),
            _ => {}
        }
        let kids = n.kids.iter().map(|k| self.walk(k)).collect();
        Node::with_lit(n.head, n.pos, n.lit.clone(), kids)
    }

    /// A named fn inside a block: resolve the body first (inside it the
    /// name is `(self)`), then bind the name as a local.
    fn fn_node(&mut self, n: &Node) -> Node {
        let f = self.function(n, false);
        let Some(name) = fn_stmt(n) else {
            return f;
        };
        let at = fn_info(n).name_pos;
        // a block defines a fn once, as the file does (FE61)
        if self.cur_block().fns.contains(name) {
            let first = self.cur_block().pos.get(name).copied();
            self.errs.push(defined_twice(name, at, first));
            return f;
        }
        match self.bind_new(name, at, false) {
            Some(b) => {
                self.cur_block().fns.insert(name.to_string());
                let known = (name.to_string(), call::Sig::of(fn_info(n)));
                self.cur_block().sigs.insert(name.to_string(), known);
                Node::new(Head::Let, n.pos, vec![ref_node(n.pos, b), f])
            }
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

    /// Binds a name in the current block for an assignment (`assign`) or
    /// a local fn. A name is bound once (D88): a second binding in the
    /// block is E0238, whatever the two are (a variable, a fn, an imported
    /// name), and a visible outer name is E0240 (D38). Only the REPL's
    /// top level binds a name again. Returns `None` when the binding is
    /// an error.
    fn bind_new(&mut self, name: &str, pos: Pos, assign: bool) -> Option<Binding> {
        if let Some(b) = self.cur_block().names.get(name).copied() {
            let again = self.cur_block().file && self.opts.redefine;
            if b.module && !again {
                let e = self.shadow_error(name, pos);
                self.errs.push(e);
                return None;
            }
            // What desugar names itself it may bind as it needs.
            if !again && !crate::tree::is_hidden(name) {
                let mut e = name_error("E0238", pos, name);
                if let Some(at) = self.cur_block().pos.get(name).copied() {
                    let how = if self.cur_block().file && self.import_bound.contains_key(name) {
                        "imported"
                    } else {
                        "first bound"
                    };
                    e.notes
                        .push(format!("note: `{name}` is {how} on line {}", at.line));
                }
                self.errs.push(e);
                return None;
            }
            if b.constant || b.module {
                // REPL: a plain variable replaces the fn or module (08).
                let b = Binding {
                    global: b.global,
                    index: b.index,
                    ..Binding::default()
                };
                self.bind_in_block(name, b);
                self.module_names.remove(name);
                return Some(b);
            }
            return Some(b);
        }
        let interface = self.cur_block().file && self.interface_name(name);
        if !self.binds_fresh(name, pos, assign, interface) {
            return None;
        }
        Some(self.bind_here(name, pos))
    }

    /// Binds name as a new global (file scope) or slot (block) in the
    /// current block, bound at `pos` (for warnings).
    fn bind_here(&mut self, name: &str, pos: Pos) -> Binding {
        let b = if self.cur_block().file {
            // The slot was taken before the fns (`declare_consts`).
            let index = match self.slots.get(name) {
                Some(&i) => i,
                None => {
                    self.globals.push(name.to_string());
                    self.globals.len() - 1
                }
            };
            Binding {
                global: true,
                index,
                ..Binding::default()
            }
        } else {
            Binding {
                index: self.cur_mut().alloc_slot(),
                ..Binding::default()
            }
        };
        self.bind_in_block(name, b);
        self.cur_block().pos.insert(name.to_string(), pos);
        b
    }

    /// Resolves the right-hand side first (`x = x + 1` reads the old x),
    /// then binds the name — or, for destructuring, every name of the
    /// pattern under the same assignment rules (03).
    fn let_(&mut self, n: &Node) -> Node {
        let target = &n.kids[0];
        // `f = n => … f(n - 1)`: the lambda cannot see `f` (E0204's help).
        let lambda = target.head == Head::Name
            && n.kids[1].head == Head::Fn
            && fn_info(&n.kids[1]).name.is_empty();
        if lambda {
            self.lambda_targets.push(lit_str(target).to_string());
        }
        let rhs = self.walk(&n.kids[1]);
        if lambda {
            self.lambda_targets.pop();
        }
        if target.head != Head::Name {
            // Destructuring assignment binds like assignment; the destructure
            // desugar emits for a `for` pattern binds fresh names (03).
            let mode = if lit_str(n) == crate::tree::FRESH {
                pattern::Mode::Fresh
            } else {
                pattern::Mode::Assign
            };
            let pat = self.bind_pattern(target, mode);
            return Node::new(Head::Let, n.pos, vec![pat, rhs]);
        }
        // The callee a partial application keeps in a hidden variable is
        // still the function it was written as (FE65).
        let kept = match crate::tree::is_hidden(lit_str(target)) {
            true => self.known_callee(&n.kids[1], &rhs),
            false => None,
        };
        let lhs = match self.bind_new(lit_str(target), target.pos, true) {
            Some(b) => {
                if let Some(known) = kept {
                    self.cur_block()
                        .sigs
                        .insert(lit_str(target).to_string(), known);
                }
                ref_node(target.pos, b)
            }
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
        // A name that left its block or is defined later in it is an error
        // before it is the prelude's: reading the builtin of the same name
        // would hide the mistake (03, rule 4).
        // Closed in this function or one around it: a lambda or an inner
        // fn reads the same name the enclosing code would (FE3).
        if let Some(made) = self.fns.iter().rev().find_map(|f| f.closed.get(name)) {
            let help = match made {
                Made::Block => format!("assign the block's value instead: `{name} = if … ;;`"),
                Made::Arm => format!(
                    "`{name}` is bound in a `match` arm, only there; use the match's value instead: `{name} = match … ;;`"
                ),
                Made::For => format!(
                    "`{name}` is bound in a `for` body, only there; use the for's value instead: `xs = for … ;;`"
                ),
            };
            self.errs
                .push(name_error("E0206", n.pos, name).with_help(&help));
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
        if self.prelude.contains(name) {
            return Node::leaf(Head::Prelude, n.pos, Lit::Str(name.to_string()));
        }
        let mut e = name_error("E0204", n.pos, name);
        if self.lambda_targets.iter().any(|t| t == name) {
            e = e.with_help(&format!(
                "a lambda copies the names that exist when it is made, so it cannot call itself: define `{name}` with `fn {name}(n) = …`"
            ));
        } else if let (Some(p), true) = (self.file_var_pos.get(name), self.in_function()) {
            e = e.with_help(&format!(
                "`{name}` is a file-level variable (assigned on line {}) that this function cannot see: a lambda or a function made in a block copies only the variables that exist when it is made; assign `{name}` first, or pass it as an argument",
                p.line
            ));
        } else if let Some((_, module)) = MOVED_TO_MODULE.iter().find(|(n, _)| *n == name) {
            e = e.with_help(&format!(
                "moved to module {module}: import {module}, then {module}.{name}"
            ));
        } else if let Some(h) = crate::habits::value_word(name) {
            // `null` of another language (FE43)
            e = e.with_help(h);
        } else if let Some(s) = self.similar_name(name) {
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
        if let Some(i) = f.innermost(name) {
            let sc = &mut f.blocks[i];
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
        let repl_top = self.opts.redefine && pi == 0 && self.in_top_fn();
        let p = &mut self.fns[pi];
        let mut src = None;
        let found = p.innermost(name).map(|i| {
            let sc = &mut p.blocks[i];
            sc.used.insert(name.to_string());
            sc.names[name]
        });
        if let Some(b) = found {
            if b.global && (b.constant || repl_top) {
                // Another top-level fn: it never changes, so a direct
                // reference is indistinguishable from a copy. In the REPL a
                // top-level fn reads every global when called (03, L9).
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
        if src.is_none()
            && pi == 0
            && self.in_top_fn()
            && let Some(&i) = self.consts.get(name)
        {
            // A constant of the file, read when the fn is called (03, L9).
            self.fns[0].blocks[0].used.insert(name.to_string());
            return Some(Found::Global(i));
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

    /// The visible name closest to `name` (`similar`): every block of every
    /// enclosing function, the names closed in them, and the prelude. The
    /// prelude and the file's lasting names are indexed once, as they come
    /// (FE36); the open blocks and functions inside the file are few and
    /// are measured one by one.
    fn similar_name(&mut self, name: &str) -> Option<String> {
        if similar::edit_budget(name.chars().count()) == 0 {
            return None;
        }
        let index = self.similar_index.get_or_insert_with(|| {
            let mut index = similar::Index::default();
            for p in self.prelude_list {
                index.add(p);
            }
            index
        });
        let lasting = &self.fns[0].lasting;
        for n in &lasting[self.similar_indexed..] {
            index.add(n);
        }
        self.similar_indexed = lasting.len();
        let outer = self.fns[0].blocks.iter().skip(1);
        let inner = self.fns[1..].iter().flat_map(|f| &f.blocks);
        let open = outer.chain(inner).flat_map(|sc| sc.names.keys());
        let closed = self.fns[1..].iter().flat_map(|f| f.closed.keys());
        similar::similar(name, index, open.chain(closed).map(String::as_str))
    }
}

/// E0210 at the second definition of fn `name`, with where the first
/// one is (FE61).
fn defined_twice(name: &str, at: Pos, first: Option<Pos>) -> Diag {
    let mut e = name_error("E0210", at, name);
    if let Some(p) = first {
        e.notes.push(format!(
            "note: `{name}` is first defined on line {}",
            p.line
        ));
    }
    e
}

/// E0211 for `return` or `?` in a parameter default: the default is
/// computed for the call before the body runs, so returning there would
/// skip the body (FE38).
fn return_in_default(what: &str, pos: Pos) -> Diag {
    error(
        "E0211",
        pos,
        vec![str_arg(what), str_arg("inside a parameter default")],
    )
    .with_help(
        "compute the value in the body: give the parameter a `nil` default and test it there",
    )
}

impl Resolver<'_> {
    /// A test's name tells which one failed, so a file uses each once and
    /// none is empty (FE20, E0233), as it does a top-level fn's (E0210).
    fn check_test_names(&mut self, root: &Node) {
        let mut seen = std::collections::HashSet::new();
        for k in &root.kids {
            if k.head != Head::Test {
                continue;
            }
            let name = lit_str(k);
            let why = if name.trim().is_empty() {
                "is empty"
            } else if !seen.insert(name.to_string()) {
                "is used by another test in this file"
            } else {
                continue;
            };
            self.errs.push(error(
                "E0233",
                k.pos,
                vec![Arg::Str(name.to_string()), Arg::Str(why.into())],
            ));
        }
    }
}
