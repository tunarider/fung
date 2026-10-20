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
    /// A lambda written as an argument of the prelude's map: `?` inside
    /// is E0226, the trap of a for body (03).
    pub(super) map_body: bool,
    /// A hoisted top-level fn: it and the functions inside it see the
    /// file's constants when called (03, L9).
    pub(super) top: bool,
    /// The names a top-level fn binds in its own code: its locals, which
    /// hide the file's constants of those names throughout it (03, L9).
    pub(super) own: HashSet<String>,
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

    /// Takes the next free slot of this function.
    pub(super) fn alloc_slot(&mut self) -> usize {
        let slot = self.next_slot;
        self.next_slot += 1;
        self.max_slot = self.max_slot.max(self.next_slot);
        slot
    }
}
