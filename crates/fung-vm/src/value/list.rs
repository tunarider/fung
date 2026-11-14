//! Lists (02). A list is immutable to fung code; `push` takes `&mut self`
//! so the VM can append in place to a list it owns alone. Small lists are
//! plain vectors; big ones are persistent vectors, so appending, putting
//! before and taking a slice are cheap and old versions stay valid. A big
//! list is its whole vector: a slice is cut from a copy of the tree in
//! O(log n) and keeps only the leaves its items are in, so the items cut
//! off are let go with the list they were in (VA59).

use std::fmt;

use super::pvec::Pvec;
use super::release::{RELEASE_PIECE, drop_children, keep};
use super::{Probe, Probed, Value};

/// The largest list kept as a plain vector; 32 is one
/// trie node.
pub const SMALL_MAX: usize = 32;

#[derive(Clone)]
pub struct List {
    repr: Repr,
}

#[derive(Clone)]
enum Repr {
    Flat(Vec<Value>),
    // Boxed so that a small list's head is the size of a `Vec` (VA37).
    Big(Box<Pvec<Value>>),
}

impl List {
    /// A list that owns `items`.
    pub fn new(items: Vec<Value>) -> List {
        if items.len() <= SMALL_MAX {
            return List {
                repr: Repr::Flat(items),
            };
        }
        List {
            repr: Repr::Big(Box::new(Pvec::of(items))),
        }
    }

    /// The items of a small list (`Value::holds_mutable` walks them);
    /// `None` for a big one, which answers from its memo.
    pub(crate) fn flat(&self) -> Option<&[Value]> {
        match &self.repr {
            Repr::Flat(v) => Some(v),
            Repr::Big(_) => None,
        }
    }

    /// Whether an item answers `p` (`Probed::has`): a big list answers
    /// from its memo, so discarding versions of one long list again and
    /// again does not scan it each time.
    pub(crate) fn holds(&self, p: Probe) -> bool {
        match &self.repr {
            Repr::Flat(v) => v.iter().any(|x| x.has(p)),
            Repr::Big(vec) => vec.holds(p),
        }
    }

    /// Hands `f` each item that holds a mutable value, a node at a time
    /// for a big list (`Pvec::each_holding`; `seen` as there).
    pub(crate) fn each_mutable(
        &self,
        seen: &mut dyn FnMut(*const ()) -> bool,
        f: &mut dyn FnMut(&Value),
    ) {
        match &self.repr {
            Repr::Flat(v) => v.iter().filter(|x| x.holds_mutable()).for_each(f),
            Repr::Big(vec) => vec.each_holding(0, vec.len(), Probe::Mutable, seen, f),
        }
    }

    pub fn len(&self) -> usize {
        match &self.repr {
            Repr::Flat(items) => items.len(),
            Repr::Big(vec) => vec.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn at(&self, i: usize) -> Option<&Value> {
        match &self.repr {
            Repr::Flat(items) => items.get(i),
            Repr::Big(vec) => (i < vec.len()).then(|| vec.at(i)),
        }
    }

    /// The items from `i` on that sit together in memory: the rest of a
    /// small list, or of the leaf holding item `i` (up to the list's end).
    /// Empty from the end on.
    pub(crate) fn run_from(&self, i: usize) -> &[Value] {
        match &self.repr {
            Repr::Flat(items) => items.get(i..).unwrap_or(&[]),
            Repr::Big(vec) => {
                if i >= vec.len() {
                    return &[];
                }
                let (leaf, start) = vec.leaf_for(i);
                let end = leaf.len().min(start + (vec.len() - i));
                &leaf[start..end]
            }
        }
    }

    /// The items in order, a leaf at a time for big lists.
    pub fn iter(&self) -> Iter<'_> {
        self.range(0, self.len())
    }

    /// Items `[lo, hi)` in order; `lo <= hi <= len`.
    fn range(&self, lo: usize, hi: usize) -> Iter<'_> {
        match &self.repr {
            Repr::Flat(items) => Iter {
                cur: items[lo..hi].iter(),
                big: None,
                next: 0,
                hi: 0,
            },
            Repr::Big(vec) => Iter {
                cur: [].iter(),
                big: Some(vec),
                next: lo,
                hi,
            },
        }
    }

    /// Items `[lo, hi)`; callers check the bounds. A big result is cut
    /// from a copy of the tree in O(log n) (VA33): the nodes inside stay
    /// shared, and it holds only the leaves its items are in, so what was
    /// cut off is not kept alive by it (VA59). A small one is copied out.
    pub fn slice(&self, lo: usize, hi: usize) -> List {
        assert!(lo <= hi && hi <= self.len(), "list slice out of range");
        match &self.repr {
            Repr::Big(vec) if hi - lo > SMALL_MAX => {
                let mut vec = vec.clone();
                vec.truncate(hi);
                vec.drop_front(lo);
                List {
                    repr: Repr::Big(vec),
                }
            }
            _ => List::new(self.range(lo, hi).cloned().collect()),
        }
    }

    /// Cuts this list down to items `[lo, hi)` in place, when the result
    /// is big and what is cut off is at most `RELEASE_PIECE` items, which
    /// are let go now (a step of about a millisecond, like a piece of
    /// `release`); false otherwise, and nothing changed. For a list that
    /// nothing else holds (`index::slice_owned`): its nodes are changed
    /// where they are, without the copy of the head leaf and the path that
    /// `slice` makes. Nodes shared with other lists are still copied.
    pub fn cut(&mut self, lo: usize, hi: usize) -> bool {
        assert!(lo <= hi && hi <= self.len(), "list cut out of range");
        match &mut self.repr {
            Repr::Big(vec) if hi - lo > SMALL_MAX && vec.len() - (hi - lo) <= RELEASE_PIECE => {
                vec.truncate(hi);
                vec.drop_front(lo);
                true
            }
            _ => false,
        }
    }

    /// Appends `x`. Nodes shared with other lists are copied, never
    /// written through.
    pub fn push(&mut self, x: Value) {
        match &mut self.repr {
            Repr::Flat(items) if items.len() < SMALL_MAX => items.push(x),
            Repr::Flat(items) => {
                let mut items = std::mem::take(items);
                items.push(x);
                *self = List::new(items);
            }
            Repr::Big(vec) => vec.push(x),
        }
    }

    /// `self` followed by `other`. The shorter side is added to the longer
    /// one at its end, so the cost follows the shorter length (L5): a
    /// small list put before a long one (`[x] + acc`) is as cheap as one
    /// put after it.
    pub fn concat(&self, other: &List) -> List {
        if other.is_empty() {
            return self.clone();
        }
        if self.is_empty() {
            return other.clone();
        }
        let total = self.len() + other.len();
        if other.len() > self.len()
            && let Repr::Big(vec) = &other.repr
        {
            let mut vec = vec.clone();
            let items = self.to_vec();
            for v in items.into_iter().rev() {
                vec.push_front(v);
            }
            return List {
                repr: Repr::Big(vec),
            };
        }
        if matches!(self.repr, Repr::Flat(_)) || total <= SMALL_MAX {
            let mut items = Vec::with_capacity(total);
            items.extend(self.iter().cloned());
            items.extend(other.iter().cloned());
            return List::new(items);
        }
        let mut out = self.clone();
        for v in other.iter() {
            out.push(v.clone());
        }
        out
    }

    /// The items copied into a new vector.
    pub fn to_vec(&self) -> Vec<Value> {
        self.iter().cloned().collect()
    }

    /// Moves the items out, leaving the list empty.
    fn take_repr(&mut self) -> Repr {
        std::mem::replace(&mut self.repr, Repr::Flat(Vec::new()))
    }
}

impl List {
    /// Moves the items out for `release`, leaving the list empty.
    /// Whether freeing this list is long work, to do a piece at a time
    /// (L6): a big list that owns its vector. A vector whose nodes others
    /// share only lets go of them, which is quick.
    pub(super) fn long_to_free(&self) -> bool {
        match &self.repr {
            Repr::Big(vec) => vec.len() > RELEASE_PIECE && vec.owns_nodes(),
            Repr::Flat(_) => false,
        }
    }

    /// Takes the last `n` items off (`n < len`) for `release`: a big
    /// list is freed a piece at a time (L6). The items go on the stack if
    /// they could go deep, and drop otherwise.
    pub(super) fn shed(&mut self, n: usize, stack: &mut Vec<Value>) {
        let len = self.len();
        let cut: Vec<Value> = self.range(len - n, len).cloned().collect();
        match &mut self.repr {
            Repr::Flat(items) => items.truncate(len - n),
            Repr::Big(vec) => vec.truncate(len - n),
        }
        cut.into_iter().for_each(|v| keep(stack, v));
    }

    pub(super) fn spill(&mut self, stack: &mut Vec<Value>) {
        match self.take_repr() {
            Repr::Flat(items) => items.into_iter().for_each(|v| keep(stack, v)),
            Repr::Big(vec) => vec.drain(|v| keep(stack, v)),
        }
    }
}

/// Frees nested values with an explicit stack (`release`), so a deeply
/// nested list cannot overflow the stack when its last reference goes
/// away. The persistent vector's own nodes are freed iteratively too
/// (`Pvec::drain`, `Node`'s `Drop`).
impl Drop for List {
    fn drop(&mut self) {
        drop_children(|stack| {
            if self.long_to_free() {
                // a big list goes to `release` whole, which frees it a
                // piece at a time while a scheduler runs (L6)
                let repr = self.take_repr();
                stack.push(Value::List(std::rc::Rc::new(List { repr })));
            } else {
                self.spill(stack);
            }
        });
    }
}

impl fmt::Debug for List {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// The items of a list in order.
pub struct Iter<'a> {
    cur: std::slice::Iter<'a, Value>,
    big: Option<&'a Pvec<Value>>,
    /// Absolute index of the next leaf to read, up to `hi`.
    next: usize,
    hi: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<&'a Value> {
        if let Some(v) = self.cur.next() {
            return Some(v);
        }
        let vec = self.big?;
        if self.next >= self.hi {
            return None;
        }
        let (leaf, start) = vec.leaf_for(self.next);
        let end = leaf.len().min(start + (self.hi - self.next));
        self.cur = leaf[start..end].iter();
        self.next += end - start;
        self.cur.next()
    }
}

/// Collects items in place before the list is shared.
#[derive(Default)]
pub struct ListBuilder {
    items: Vec<Value>,
}

impl ListBuilder {
    /// A builder with room for `n` items.
    pub fn with_capacity(n: usize) -> ListBuilder {
        ListBuilder {
            items: Vec::with_capacity(n),
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn push(&mut self, v: Value) {
        self.items.push(v);
    }

    pub fn finish(self) -> List {
        List::new(self.items)
    }
}
