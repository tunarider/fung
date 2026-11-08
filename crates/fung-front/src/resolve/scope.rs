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

/// One lexical block: the file (globals) or a block (slots).
pub(super) struct Scope {
    pub(super) names: HashMap<String, Binding>,
    pub(super) file: bool,
    /// First slot this block may use; restored on exit.
    pub(super) base: usize,
    /// For warnings (08): where each name was bound, which were read,
    /// and which are parameters (never reported as unused).
    pub(super) pos: HashMap<String, Pos>,
    pub(super) used: HashSet<String>,
    pub(super) param: HashSet<String>,
}

impl Scope {
    pub(super) fn new(file: bool, base: usize) -> Scope {
        Scope {
            names: HashMap::new(),
            file,
            base,
            pos: HashMap::new(),
            used: HashSet::new(),
            param: HashSet::new(),
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
    /// The synthetic lambda of a `for`: return inside is E0213.
    pub(super) for_body: bool,
    /// A lambda written as the `f` of the prelude's map or of `conc.pmap`
    /// (the name the error shows): `?` inside is E0226, the trap of a for
    /// body (03, SP1).
    pub(super) map_body: Option<&'static str>,
    /// A hoisted top-level fn: it and the functions inside it see the
    /// file's constants when called (03, L9).
    pub(super) top: bool,
    /// The names a top-level fn binds in its own code: its locals, which
    /// hide the file's constants of those names throughout it (03, L9).
    pub(super) own: HashSet<String>,
    /// The blocks that bind each name, outermost first: a lookup takes
    /// the last without walking every block (FE16).
    pub(super) bound_in: HashMap<String, Vec<usize>>,
    /// Names defined in blocks of this fn that have ended (E0206).
    pub(super) closed: HashSet<String>,
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

    /// Binds name in block `block`, or rebinds it there.
    pub(super) fn bind(&mut self, block: usize, name: &str, b: Binding) {
        if self.blocks[block]
            .names
            .insert(name.to_string(), b)
            .is_none()
        {
            let at = self.bound_in.entry(name.to_string()).or_default();
            let i = at.partition_point(|&x| x < block);
            at.insert(i, block);
        }
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

    /// Takes the next free slot of this function.
    pub(super) fn alloc_slot(&mut self) -> usize {
        let slot = self.next_slot;
        self.next_slot += 1;
        self.max_slot = self.max_slot.max(self.next_slot);
        slot
    }
}
