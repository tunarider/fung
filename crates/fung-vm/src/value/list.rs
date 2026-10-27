//! Lists (02). A list is immutable to fung code; `push` takes `&mut self`
//! so the VM can append in place to a list it owns alone. Small lists are
//! plain vectors; big ones are views `[lo, hi)` on a persistent vector, so
//! appending and taking the tail are cheap and old versions stay valid.

use std::fmt;

use super::pvec::{MASK, Pvec};
use super::release::{drop_children, keep};
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
                let leaf = vec.leaf_for(at);
                let start = at & MASK;
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

    /// `self` followed by `other`. Its cost follows `other`'s length when
    /// `self` is big, both lengths otherwise.
    pub fn concat(&self, other: &List) -> List {
        if other.is_empty() {
            return self.clone();
        }
        if self.is_empty() {
            return other.clone();
        }
        let total = self.len() + other.len();
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
        drop_children(|stack| self.spill(stack));
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
        let leaf = vec.leaf_for(self.next);
        let start = self.next & MASK;
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
