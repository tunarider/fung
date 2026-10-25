//! Maps (02): string keys in insertion order. A map is immutable to fung
//! code; `put` and `remove` take `&mut self` so the VM can update in place
//! a map it owns alone. Small maps are a plain vector of entries (linear
//! lookup); big ones are an entry vector plus a HAMT index (`Omap`) whose
//! nodes are shared between versions.

use std::fmt;
use std::rc::Rc;

use super::Value;
use super::omap::{self, Omap};
use super::release::{drop_children, keep};

/// The largest map kept as a plain vector.
pub const SMALL_MAP_MAX: usize = 16;

#[derive(Clone)]
pub struct Map {
    repr: Repr,
}

#[derive(Clone)]
enum Repr {
    Flat(Vec<(Rc<String>, Value)>),
    Big(Omap),
}

impl Default for Map {
    fn default() -> Map {
        Map::new()
    }
}

impl Map {
    pub fn new() -> Map {
        Map {
            repr: Repr::Flat(Vec::new()),
        }
    }

    /// Whether a value in the map holds a mutable one.
    /// The entries of a small map (`Value::holds_mutable` walks them);
    /// `None` for a big one, which answers from its memo.
    pub(crate) fn flat(&self) -> Option<&[(Rc<String>, Value)]> {
        match &self.repr {
            Repr::Flat(v) => Some(v),
            Repr::Big(_) => None,
        }
    }

    pub(crate) fn holds_mutable(&self) -> bool {
        match &self.repr {
            Repr::Flat(v) => v.iter().any(|(_, x)| x.holds_mutable()),
            Repr::Big(o) => o.holds_mutable(),
        }
    }

    pub fn len(&self) -> usize {
        match &self.repr {
            Repr::Flat(es) => es.len(),
            Repr::Big(o) => o.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, k: &str) -> Option<&Value> {
        match &self.repr {
            Repr::Flat(es) => es.iter().find(|(key, _)| **key == k).map(|(_, v)| v),
            Repr::Big(o) => o.get(k),
        }
    }

    /// Sets `k`; an existing key keeps its place. Nodes shared with other
    /// maps are copied, never written through.
    pub fn put(&mut self, k: Rc<String>, v: Value) {
        match &mut self.repr {
            Repr::Flat(es) => {
                if let Some(e) = es.iter_mut().find(|(key, _)| *key == k) {
                    e.1 = v;
                    return;
                }
                es.push((k, v));
                if es.len() > SMALL_MAP_MAX {
                    let es = std::mem::take(es);
                    self.repr = Repr::Big(Omap::of(es));
                }
            }
            Repr::Big(o) => o.put(k, v),
        }
    }

    /// Removes `k`; returns whether it was there (the map is unchanged
    /// otherwise). A removed key put again comes back at the end.
    pub fn remove(&mut self, k: &str) -> bool {
        match &mut self.repr {
            Repr::Flat(es) => match es.iter().position(|(key, _)| **key == k) {
                Some(i) => {
                    es.remove(i);
                    true
                }
                None => false,
            },
            Repr::Big(o) => {
                if !o.remove(k) {
                    return false;
                }
                if o.len() <= SMALL_MAP_MAX {
                    self.repr = Repr::Flat(o.entries());
                }
                true
            }
        }
    }

    /// `self` with `other`'s entries put in `other`'s order; `other` wins
    /// on a clash.
    pub fn merge(&self, other: &Map) -> Map {
        let mut out = self.clone();
        for (k, v) in other.entries() {
            out.put(k.clone(), v.clone());
        }
        out
    }

    /// Keys and values in insertion order.
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            inner: match &self.repr {
                Repr::Flat(es) => IterRepr::Flat(es.iter()),
                Repr::Big(o) => IterRepr::Big(o.iter()),
            },
        }
    }

    /// Like `iter`, with the shared key strings.
    pub fn entries(&self) -> impl Iterator<Item = (&Rc<String>, &Value)> {
        let (flat, big) = match &self.repr {
            Repr::Flat(es) => (Some(es.iter().map(|(k, v)| (k, v))), None),
            Repr::Big(o) => (None, Some(o.iter().map(|e| (&e.key, &e.val)))),
        };
        flat.into_iter().flatten().chain(big.into_iter().flatten())
    }

    /// Moves the values out for `release`, leaving the map empty.
    pub(super) fn spill(&mut self, stack: &mut Vec<Value>) {
        match std::mem::replace(&mut self.repr, Repr::Flat(Vec::new())) {
            Repr::Flat(es) => es.into_iter().for_each(|(_, v)| keep(stack, v)),
            Repr::Big(o) => o.drain(|v| keep(stack, v)),
        }
    }
}

/// Frees nested values with an explicit stack (`release`), so maps nested
/// in maps (or lists, tuples, tags) of any depth drop without recursion.
impl Drop for Map {
    fn drop(&mut self) {
        drop_children(|stack| self.spill(stack));
    }
}

impl fmt::Debug for Map {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

/// The keys and values of a map in insertion order.
pub struct Iter<'a> {
    inner: IterRepr<'a>,
}

enum IterRepr<'a> {
    Flat(std::slice::Iter<'a, (Rc<String>, Value)>),
    Big(omap::Iter<'a>),
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a str, &'a Value);

    fn next(&mut self) -> Option<(&'a str, &'a Value)> {
        match &mut self.inner {
            IterRepr::Flat(it) => it.next().map(|(k, v)| (k.as_str(), v)),
            IterRepr::Big(it) => it.next().map(|e| (e.key.as_str(), &e.val)),
        }
    }
}

/// Collects entries in place before the map is shared.
#[derive(Default)]
pub struct MapBuilder {
    m: Map,
}

impl MapBuilder {
    /// A builder with room for `n` entries of a small map.
    pub fn with_capacity(n: usize) -> MapBuilder {
        MapBuilder {
            m: Map {
                repr: Repr::Flat(Vec::with_capacity(n.min(SMALL_MAP_MAX))),
            },
        }
    }

    /// Sets `k`; a repeated key keeps its first place.
    pub fn set(&mut self, k: Rc<String>, v: Value) {
        self.m.put(k, v);
    }

    pub fn len(&self) -> usize {
        self.m.len()
    }

    pub fn is_empty(&self) -> bool {
        self.m.is_empty()
    }

    pub fn finish(self) -> Map {
        self.m
    }
}
