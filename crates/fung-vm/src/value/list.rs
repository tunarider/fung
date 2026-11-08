//! Lists (02). A list is immutable to fung code; `push` takes `&mut self`
//! so the VM can append in place to a list it owns alone. Small lists are
//! plain vectors; big ones are views `[lo, hi)` on a persistent vector, so
//! appending and taking the tail are cheap and old versions stay valid.

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
    Big {
        vec: Pvec<Value>,
        lo: usize,
        hi: usize,
    },
}

impl List {
    /// A list that owns `items`.
    pub fn new(items: Vec<Value>) -> List {
        if items.len() <= SMALL_MAX {
            return List {
                repr: Repr::Flat(items),
            };
        }
        let vec = Pvec::of(items);
        let hi = vec.len();
        List {
            repr: Repr::Big { vec, lo: 0, hi },
        }
    }

    /// Whether an item holds a mutable value: a slice of a big list
    /// looks at its own items only.
    /// The items of a small list (`Value::holds_mutable` walks them);
    /// `None` for a big one, which answers from its memo.
    pub(crate) fn flat(&self) -> Option<&[Value]> {
        match &self.repr {
            Repr::Flat(v) => Some(v),
            Repr::Big { .. } => None,
        }
    }

    pub(crate) fn holds_mutable(&self) -> bool {
        self.holds(Probe::Mutable)
    }

    /// Whether an item is an `Err` (a discarded list, 02): a big list
    /// answers from its memo, so discarding versions of one long list
    /// again and again does not scan it each time.
    pub(crate) fn holds_err(&self) -> bool {
        self.holds(Probe::Err)
    }

    fn holds(&self, p: Probe) -> bool {
        match &self.repr {
            Repr::Flat(v) => v.iter().any(|x| x.has(p)),
            Repr::Big { vec, lo, hi } if *lo == 0 && *hi == vec.len() => vec.holds(p),
            Repr::Big { vec, lo, hi } => vec.holds_in(*lo, *hi, p),
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
            Repr::Big { vec, lo, hi } => vec.each_holding(*lo, *hi, Probe::Mutable, seen, f),
        }
    }

    pub fn len(&self) -> usize {
        match &self.repr {
            Repr::Flat(items) => items.len(),
            Repr::Big { lo, hi, .. } => hi - lo,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn at(&self, i: usize) -> Option<&Value> {
        match &self.repr {
            Repr::Flat(items) => items.get(i),
            Repr::Big { vec, lo, hi } => (i < hi - lo).then(|| vec.at(lo + i)),
        }
    }

    /// The items from `i` on that sit together in memory: the rest of a
    /// small list, or of the leaf holding item `i` (up to the list's end).
    /// Empty from the end on.
    pub(crate) fn run_from(&self, i: usize) -> &[Value] {
        match &self.repr {
            Repr::Flat(items) => items.get(i..).unwrap_or(&[]),
            Repr::Big { vec, lo, hi } => {
                let at = lo + i;
                if at >= *hi {
                    return &[];
                }
                let (leaf, start) = vec.leaf_for(at);
                let end = leaf.len().min(start + (hi - at));
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
            Repr::Big { vec, lo: base, .. } => Iter {
                cur: [].iter(),
                big: Some(vec),
                next: base + lo,
                hi: base + hi,
            },
        }
    }

    /// Items `[lo, hi)`; callers check the bounds. A big result is a view
    /// on the same vector; a small one, or one much smaller than the
    /// vector it would keep alive, is copied out.
    /// Whether this list is a view into a shared vector (tests).
    #[cfg(test)]
    pub(crate) fn is_view(&self) -> bool {
        matches!(&self.repr, Repr::Big { lo, hi, vec } if *lo != 0 || *hi != vec.len())
    }

    pub fn slice(&self, lo: usize, hi: usize) -> List {
        assert!(lo <= hi && hi <= self.len(), "list slice out of range");
        let n = hi - lo;
        match &self.repr {
            Repr::Big { vec, lo: base, .. } if n > SMALL_MAX && n * 4 >= vec.len() => List {
                repr: Repr::Big {
                    vec: vec.clone(),
                    lo: base + lo,
                    hi: base + hi,
                },
            },
            _ => List::new(self.range(lo, hi).cloned().collect()),
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
            // the view reaches the vector's end: grow it
            Repr::Big { vec, hi, .. } if *hi == vec.len() => {
                vec.push(x);
                *hi += 1;
            }
            // a front slice that is most of its vector: cut the vector at
            // its end (O(log n), the leaves before stay shared) and grow
            // it, so a list used as a stack (`s[..-1] + [x]`) stays cheap
            Repr::Big { vec, lo, hi } if (*hi - *lo) * 4 >= *hi => {
                vec.truncate(*hi);
                vec.push(x);
                *hi += 1;
            }
            // a small front slice gets its own vector
            Repr::Big { .. } => {
                let mut items = self.to_vec();
                items.push(x);
                *self = List::new(items);
            }
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
            && let Some(mut out) = other.extendable_front()
        {
            let items = self.to_vec();
            if let Repr::Big { vec, hi, .. } = &mut out.repr {
                for v in items.into_iter().rev() {
                    vec.push_front(v);
                    *hi += 1;
                }
            }
            return out;
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

    /// A big list equal to `self` whose view starts at its vector's
    /// front, so items can be put before it: a view is cut down to its
    /// own items in O(log n) (the nodes stay shared). `None` for a small
    /// list, or a view much smaller than its vector (copying it is cheap,
    /// and keeps the vector from being held for a few items).
    fn extendable_front(&self) -> Option<List> {
        let Repr::Big { vec, lo, hi } = &self.repr else {
            return None;
        };
        if *lo == 0 {
            return Some(self.clone());
        }
        if (hi - lo) * 4 < vec.len() {
            return None;
        }
        let mut vec = vec.clone();
        vec.truncate(*hi);
        vec.drop_front(*lo);
        let hi = vec.len();
        Some(List {
            repr: Repr::Big { vec, lo: 0, hi },
        })
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
    /// (L6): a big list that owns its vector. A view of a vector others
    /// share only lets go of nodes, which is quick.
    pub(super) fn long_to_free(&self) -> bool {
        match &self.repr {
            Repr::Big { vec, .. } => self.len() > RELEASE_PIECE && vec.owns_nodes(),
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
            Repr::Big { vec, hi, .. } => {
                *hi -= n;
                vec.truncate(*hi);
            }
        }
        cut.into_iter().for_each(|v| keep(stack, v));
    }

    pub(super) fn spill(&mut self, stack: &mut Vec<Value>) {
        match self.take_repr() {
            Repr::Flat(items) => items.into_iter().for_each(|v| keep(stack, v)),
            Repr::Big { vec, .. } => vec.drain(|v| keep(stack, v)),
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
