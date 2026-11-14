//! Scope bookkeeping: where a name lives, block scopes, function scopes.

use std::collections::{HashMap, HashSet};

use crate::source::Pos;
use crate::tree::Capture;

/// Where a name lives.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Binding {
    pub(super) global: bool,
    /// Global index or local slot.
    pub(super) index: usize,
    /// A hoisted top-level fn or an import: cannot be reassigned.
    pub(super) constant: bool,
    /// An imported module name: cannot be shadowed either (04).
    pub(super) module: bool,
}

/// What made a block, for the help of E0206 when one of its names is
/// read after it (FE33).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Made {
    /// An `if` branch, or a block desugar made.
    #[default]
    Block,
    /// A `match` arm.
    Arm,
    /// A `for` body (its desugared function).
    For,
}

/// One lexical block: the file (globals) or a block (slots).
pub(super) struct Scope {
    pub(super) names: HashMap<String, Binding>,
    pub(super) file: bool,
    pub(super) made: Made,
    /// First slot this block may use; restored on exit.
    pub(super) base: usize,
    /// For E0239 (D38): where each name was bound, which were read, and
    /// which are parameters.
    pub(super) pos: HashMap<String, Pos>,
    pub(super) used: HashSet<String>,
    pub(super) param: HashSet<String>,
    /// The names of the fns this block defines (`fn a() …`): a second
    /// one is E0238 (FE61, D122).
    pub(super) fns: HashSet<String>,
    /// The names of this block sure to hold a known function (a local
    /// `fn`, or the callee a partial application keeps): a call of one
    /// is checked as a top-level fn's is (FE65).
    pub(super) sigs: HashMap<String, (String, super::call::Sig)>,
}

impl Scope {
    pub(super) fn new(file: bool, base: usize) -> Scope {
        Scope {
            names: HashMap::new(),
            file,
            made: Made::Block,
            base,
            pos: HashMap::new(),
            used: HashSet::new(),
            param: HashSet::new(),
            fns: HashSet::new(),
            sigs: HashMap::new(),
        }
    }
}

/// One function being resolved: its block scopes, slot counter, the
/// values it captures from enclosing functions, and its own name when it
/// is a named local fn (reachable as `(self)`).
#[derive(Default)]
pub(super) struct FnScope {
    pub(super) blocks: Vec<Scope>,
    pub(super) next_slot: usize,
    pub(super) max_slot: usize,
    pub(super) captures: Vec<Capture>,
    pub(super) cap_index: HashMap<String, usize>,
    pub(super) self_name: String,
    /// The parameters of a named local fn, for its calls of itself.
    pub(super) self_sig: Option<super::call::Sig>,
    /// The synthetic lambda of a `for`: `return` and `?` inside are
    /// E0237 (D42).
    pub(super) for_body: bool,
    /// A lambda `=>`: `return` and `?` inside are E0237 (D42).
    pub(super) lambda: bool,
    /// A parameter default is being resolved: `return` there is E0211
    /// (FE38).
    pub(super) in_default: bool,
    /// A hoisted top-level fn: it and the functions inside it see the
    /// file's constants when called (03, L9).
    pub(super) top: bool,
    /// The blocks that bind each name, outermost first: a lookup takes
    /// the last without walking every block (FE16).
    pub(super) bound_in: HashMap<String, Vec<usize>>,
    /// Names defined in blocks of this fn that have ended (E0206), with
    /// what made the block.
    pub(super) closed: HashMap<String, Made>,
    /// The names bound in the outermost block or closed, each once, in the
    /// order they came: they stay visible as long as the function is
    /// resolved, so the "similar name" index takes them as they come
    /// (FE36).
    pub(super) lasting: Vec<String>,
    /// Per block: local fn names declared later in that block (E0208).
    pub(super) later: Vec<HashSet<String>>,
}

impl FnScope {
    pub(super) fn new(self_name: &str) -> FnScope {
        FnScope {
            self_name: self_name.to_string(),
            ..FnScope::default()
        }
    }

    /// Binds name in block `block`, or rebinds it there; true when the
    /// block did not bind it before.
    pub(super) fn bind(&mut self, block: usize, name: &str, b: Binding) -> bool {
        let new = self.blocks[block]
            .names
            .insert(name.to_string(), b)
            .is_none();
        if new {
            if block == 0 && !self.closed.contains_key(name) {
                self.lasting.push(name.to_string());
            }
            let at = self.bound_in.entry(name.to_string()).or_default();
            let i = at.partition_point(|&x| x < block);
            at.insert(i, block);
        }
        new
    }

    /// The innermost block that binds name.
    pub(super) fn innermost(&self, name: &str) -> Option<usize> {
        self.bound_in.get(name)?.last().copied()
    }

    /// Closes the innermost block, forgetting where its names were bound.
    pub(super) fn pop_scope(&mut self) -> Scope {
        let sc = self.blocks.pop().expect("block stack");
        for name in sc.names.keys() {
            if let Some(at) = self.bound_in.get_mut(name) {
                at.pop();
                if at.is_empty() {
                    self.bound_in.remove(name);
                }
            }
        }
        sc
    }

    /// Records a name of a block that ended (E0206).
    pub(super) fn close(&mut self, name: String, made: Made) {
        let lasting = !self.closed.contains_key(&name)
            && !self
                .blocks
                .first()
                .is_some_and(|b| b.names.contains_key(&name));
        if lasting {
            self.lasting.push(name.clone());
        }
        self.closed.insert(name, made);
    }

    /// Takes the next free slot of this function.
    pub(super) fn alloc_slot(&mut self) -> usize {
        let slot = self.next_slot;
        self.next_slot += 1;
        self.max_slot = self.max_slot.max(self.next_slot);
        slot
    }
}
