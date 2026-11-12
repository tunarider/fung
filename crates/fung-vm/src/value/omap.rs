//! A big map: entries in insertion order in a persistent vector, and a
//! HAMT from key to entry position. Removing a key leaves a dead entry
//! behind; when dead entries outnumber live ones the map is rebuilt, so
//! removal stays cheap on average. A version something else still holds
//! is rebuilt only far later (`remove`): removing again and again from
//! the same version must not rebuild it every time.

use std::rc::Rc;

use super::hamt::{Hamt, hash_key};
use super::pvec::Pvec;
use super::{Probe, Probed, Text, Value};

#[derive(Clone)]
pub(super) struct Entry {
    pub(super) key: Rc<Text>,
    pub(super) val: Value,
}

#[derive(Clone)]
pub(super) struct Omap {
    /// `None` marks a removed entry.
    ents: Pvec<Option<Entry>>,
    idx: Hamt,
    live: usize,
    dead: usize,
}

impl Probed for Option<Entry> {
    fn has(&self, p: Probe) -> bool {
        self.as_ref().is_some_and(|e| e.val.has(p))
    }
}

impl Omap {
    /// Whether a live entry's value answers `p`.
    pub(super) fn holds(&self, p: Probe) -> bool {
        self.ents.holds(p)
    }

    /// Hands `f` each live value that holds a mutable one, a node at a
    /// time (`Pvec::each_holding`).
    pub(super) fn each_mutable(
        &self,
        seen: &mut dyn FnMut(*const ()) -> bool,
        f: &mut dyn FnMut(&Value),
    ) {
        let n = self.ents.len();
        self.ents
            .each_holding(0, n, Probe::Mutable, seen, &mut |e: &Option<Entry>| {
                if let Some(e) = e {
                    f(&e.val);
                }
            });
    }

    /// The first live entry at slot `at` or after it, with its slot.
    pub(super) fn entry_from(&self, mut at: usize) -> Option<(usize, &Entry)> {
        while at < self.ents.len() {
            if let Some(e) = self.ents.at(at) {
                return Some((at, e));
            }
            at += 1;
        }
        None
    }

    /// A map of distinct keys in the given order.
    pub(super) fn of(entries: Vec<(Rc<Text>, Value)>) -> Omap {
        let mut idx = Hamt::default();
        for (i, (k, _)) in entries.iter().enumerate() {
            idx.set(hash_key(k.as_bytes()), k.clone(), i);
        }
        let live = entries.len();
        let ents = entries
            .into_iter()
            .map(|(key, val)| Some(Entry { key, val }))
            .collect();
        Omap {
            ents: Pvec::of(ents),
            idx,
            live,
            dead: 0,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.live
    }

    pub(super) fn get(&self, k: &[u8]) -> Option<&Value> {
        let pos = self.idx.get(hash_key(k), k)?;
        self.ents.at(pos).as_ref().map(|e| &e.val)
    }

    /// Sets `k`; an existing key keeps its position.
    pub(super) fn put(&mut self, key: Rc<Text>, val: Value) {
        let h = hash_key(key.as_bytes());
        match self.idx.get(h, key.as_bytes()) {
            Some(pos) => self.ents.set(pos, Some(Entry { key, val })),
            None => {
                let pos = self.ents.len();
                self.idx.set(h, key.clone(), pos);
                self.ents.push(Some(Entry { key, val }));
                self.live += 1;
            }
        }
    }

    /// Removes `k`; returns whether it was there. The map is rebuilt
    /// without its dead entries once they outnumber the live ones; when
    /// the version removed from is `shared`, only far later (VA1). The
    /// rebuild lands in the new version alone, and a shared version left
    /// at the lower bound would pay it again on each of its removals: the
    /// same version is changed again and again when values are immutable.
    pub(super) fn remove(&mut self, k: &[u8], shared: bool) -> bool {
        let h = hash_key(k);
        let Some(pos) = self.idx.get(h, k) else {
            return false;
        };
        self.idx.del(h, k);
        self.ents.set(pos, None);
        self.live -= 1;
        self.dead += 1;
        let bound = if shared {
            4 * self.live + 64
        } else {
            self.live
        };
        if self.dead > bound {
            *self = Omap::of(self.entries());
        }
        true
    }

    /// Entry slots, the removed ones included.
    #[cfg(test)]
    pub(super) fn slots(&self) -> usize {
        self.ents.len()
    }

    /// The live entries in order, copied out.
    pub(super) fn entries(&self) -> Vec<(Rc<Text>, Value)> {
        let mut out = Vec::with_capacity(self.live);
        out.extend(self.iter().map(|e| (e.key.clone(), e.val.clone())));
        out
    }

    /// Live entries in order, a leaf at a time.
    pub(super) fn iter(&self) -> Iter<'_> {
        Iter {
            vec: &self.ents,
            next: 0,
            cur: [].iter(),
        }
    }

    /// Consumes the map, handing the values of the entries it owns alone
    /// to `f` (see `Pvec::drain`).
    pub(super) fn drain(self, mut f: impl FnMut(Value)) {
        self.ents.drain(|e| {
            if let Some(e) = e {
                f(e.val)
            }
        });
    }
}

pub(super) struct Iter<'a> {
    vec: &'a Pvec<Option<Entry>>,
    /// Index of the next leaf to read.
    next: usize,
    cur: std::slice::Iter<'a, Option<Entry>>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Entry;

    fn next(&mut self) -> Option<&'a Entry> {
        loop {
            if let Some(e) = self.cur.by_ref().flatten().next() {
                return Some(e);
            }
            if self.next >= self.vec.len() {
                return None;
            }
            let (leaf, start) = self.vec.leaf_for(self.next);
            let end = leaf.len().min(start + (self.vec.len() - self.next));
            self.cur = leaf[start..end].iter();
            self.next += end - start;
        }
    }
}
