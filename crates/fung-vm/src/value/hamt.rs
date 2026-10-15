//! A HAMT (hash array mapped trie) from string keys to positions, the
//! index of a big map. Each node covers 5 bits of the key's 32-bit hash:
//! a bitmap says which of the 32 slots are present and a compact array
//! holds only those. Nodes are shared through `Rc`; an update copies only
//! the shared nodes on its path (`Rc::make_mut`), so older tries stay
//! valid and a trie owned alone is updated in place. When every hash bit
//! is used up, keys with the same hash share a collision node.
//!
//! The trie is at most 8 nodes deep (shifts 0, 5, ..., 30, then a
//! collision node) and holds no values, so the plain recursive drop and
//! the recursive `set`/`del` are bounded.

use std::hash::{BuildHasher, RandomState};
use std::rc::Rc;
use std::sync::OnceLock;

const BITS: u32 = 5;
const MASK: u32 = (1 << BITS) - 1;
/// Levels use shifts 0, 5, ..., 30; beyond that, collision nodes.
const MAX_SHIFT: u32 = 30;

fn hasher() -> &'static RandomState {
    static H: OnceLock<RandomState> = OnceLock::new();
    H.get_or_init(RandomState::new)
}

/// A replacement hash for tests.
#[cfg(test)]
type TestHash = Option<fn(&str) -> u32>;

#[cfg(test)]
thread_local! {
    static TEST_HASH: std::cell::Cell<TestHash> = const { std::cell::Cell::new(None) };
}

/// Replaces the hash on this thread so tests can force collisions; `None`
/// restores the real one. Only on this thread: a test that sets it must
/// not rely on it in tasks it spawns, which run on their own threads with
/// the real hash.
#[cfg(test)]
pub(super) fn set_test_hash(f: TestHash) {
    TEST_HASH.with(|h| h.set(f));
}

/// The 32-bit hash of a key. Iteration order never depends on it (the
/// map iterates its entry vector), so a per-process random seed is fine.
pub(super) fn hash_key(s: &str) -> u32 {
    #[cfg(test)]
    if let Some(f) = TEST_HASH.with(|h| h.get()) {
        return f(s);
    }
    let h = hasher().hash_one(s);
    (h as u32) ^ ((h >> 32) as u32)
}

#[derive(Clone)]
struct Leaf {
    key: Rc<String>,
    hash: u32,
    pos: usize,
}

#[derive(Clone)]
enum Slot {
    Leaf(Leaf),
    Sub(Rc<Node>),
}

#[derive(Clone)]
enum Node {
    Bits {
        bitmap: u32,
        slots: Vec<Slot>,
    },
    /// Entries whose hashes are identical.
    Coll(Vec<Leaf>),
}

fn bit_at(shift: u32, h: u32) -> u32 {
    1 << ((h >> shift) & MASK)
}

/// The slot index of `bit` in a compact array.
fn slot_index(bitmap: u32, bit: u32) -> usize {
    (bitmap & (bit - 1)).count_ones() as usize
}

/// A persistent map from keys to positions; `Clone` shares the whole trie.
#[derive(Clone, Default)]
pub(super) struct Hamt {
    root: Option<Rc<Node>>,
}

impl Hamt {
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub(super) fn get(&self, h: u32, key: &str) -> Option<usize> {
        let mut n = self.root.as_deref()?;
        let mut shift = 0;
        loop {
            match n {
                Node::Coll(c) => return c.iter().find(|l| *l.key == key).map(|l| l.pos),
                Node::Bits { bitmap, slots } => {
                    let bit = bit_at(shift, h);
                    if bitmap & bit == 0 {
                        return None;
                    }
                    match &slots[slot_index(*bitmap, bit)] {
                        Slot::Leaf(l) => return (*l.key == key).then_some(l.pos),
                        Slot::Sub(sub) => n = sub,
                    }
                }
            }
            shift += BITS;
        }
    }

    /// Maps `key` to `pos`, replacing an existing position.
    pub(super) fn set(&mut self, h: u32, key: Rc<String>, pos: usize) {
        let leaf = Leaf { key, hash: h, pos };
        match &mut self.root {
            None => {
                self.root = Some(Rc::new(Node::Bits {
                    bitmap: bit_at(0, h),
                    slots: vec![Slot::Leaf(leaf)],
                }))
            }
            Some(root) => set_in(Rc::make_mut(root), 0, leaf),
        }
    }

    /// Removes `key`; returns whether it was there. Nothing is copied when
    /// the key is absent.
    pub(super) fn del(&mut self, h: u32, key: &str) -> bool {
        if self.get(h, key).is_none() {
            return false;
        }
        let root = self.root.as_mut().expect("found a key in an empty trie");
        if del_in(Rc::make_mut(root), 0, h, key) {
            self.root = None;
        }
        true
    }
}

fn set_in(n: &mut Node, shift: u32, leaf: Leaf) {
    match n {
        Node::Coll(c) => match c.iter_mut().find(|l| l.key == leaf.key) {
            Some(l) => l.pos = leaf.pos,
            None => c.push(leaf),
        },
        Node::Bits { bitmap, slots } => {
            let bit = bit_at(shift, leaf.hash);
            let i = slot_index(*bitmap, bit);
            if *bitmap & bit == 0 {
                slots.insert(i, Slot::Leaf(leaf));
                *bitmap |= bit;
                return;
            }
            match &mut slots[i] {
                Slot::Sub(sub) => set_in(Rc::make_mut(sub), shift + BITS, leaf),
                Slot::Leaf(l) if l.key == leaf.key => l.pos = leaf.pos,
                Slot::Leaf(l) => {
                    let old = l.clone();
                    slots[i] = Slot::Sub(Rc::new(pair(shift + BITS, old, leaf)));
                }
            }
        }
    }
}

/// The smallest subtree holding two entries whose hashes agree on the
/// bits below `shift`.
fn pair(shift: u32, a: Leaf, b: Leaf) -> Node {
    if shift > MAX_SHIFT {
        // every hash bit is used: a true collision
        return Node::Coll(vec![a, b]);
    }
    let (ia, ib) = ((a.hash >> shift) & MASK, (b.hash >> shift) & MASK);
    if ia == ib {
        return Node::Bits {
            bitmap: 1 << ia,
            slots: vec![Slot::Sub(Rc::new(pair(shift + BITS, a, b)))],
        };
    }
    let slots = if ia < ib {
        vec![Slot::Leaf(a), Slot::Leaf(b)]
    } else {
        vec![Slot::Leaf(b), Slot::Leaf(a)]
    };
    Node::Bits {
        bitmap: 1 << ia | 1 << ib,
        slots,
    }
}

/// Removes a key known to be in `n`; returns whether `n` became empty.
fn del_in(n: &mut Node, shift: u32, h: u32, key: &str) -> bool {
    match n {
        Node::Coll(c) => {
            c.retain(|l| *l.key != key);
            c.is_empty()
        }
        Node::Bits { bitmap, slots } => {
            let bit = bit_at(shift, h);
            let i = slot_index(*bitmap, bit);
            let emptied = match &mut slots[i] {
                Slot::Sub(sub) => del_in(Rc::make_mut(sub), shift + BITS, h, key),
                Slot::Leaf(_) => true,
            };
            if emptied {
                slots.remove(i);
                *bitmap &= !bit;
            }
            slots.is_empty()
        }
    }
}
