//! A big map: entries in insertion order in a persistent vector, and a
//! HAMT from key to entry position. Removing a key leaves a dead entry
//! behind; when dead entries outnumber live ones the map is rebuilt, so
//! removal stays cheap on average.

use std::rc::Rc;

use super::hamt::{Hamt, hash_key};
use super::pvec::{MASK, Pvec};
use super::{HoldsMutable, Value};

#[derive(Clone)]
pub(super) struct Entry {
    pub(super) key: Rc<String>,
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

impl HoldsMutable for Option<Entry> {
    fn holds_mutable(&self) -> bool {
        self.as_ref().is_some_and(|e| e.val.holds_mutable())
    }
}

impl Omap {
    /// Whether a live entry's value holds a mutable one.
    pub(super) fn holds_mutable(&self) -> bool {
        self.ents.holds_mutable()
    }

    /// A map of distinct keys in the given order.
    pub(super) fn of(entries: Vec<(Rc<String>, Value)>) -> Omap {
        let mut idx = Hamt::default();
        for (i, (k, _)) in entries.iter().enumerate() {
            idx.set(hash_key(k), k.clone(), i);
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

    pub(super) fn get(&self, k: &str) -> Option<&Value> {
        let pos = self.idx.get(hash_key(k), k)?;
        self.ents.at(pos).as_ref().map(|e| &e.val)
    }

    /// Sets `k`; an existing key keeps its position.
    pub(super) fn put(&mut self, key: Rc<String>, val: Value) {
        let h = hash_key(&key);
        match self.idx.get(h, &key) {
            Some(pos) => self.ents.set(pos, Some(Entry { key, val })),
            None => {
                let pos = self.ents.len();
                self.idx.set(h, key.clone(), pos);
                self.ents.push(Some(Entry { key, val }));
                self.live += 1;
            }
        }
    }

    /// Removes `k`; returns whether it was there.
    pub(super) fn remove(&mut self, k: &str) -> bool {
        let h = hash_key(k);
        let Some(pos) = self.idx.get(h, k) else {
            return false;
        };
        self.idx.del(h, k);
        self.ents.set(pos, None);
        self.live -= 1;
        self.dead += 1;
        if self.dead > self.live {
            *self = Omap::of(self.entries());
        }
        true
    }

    /// The live entries in order, copied out.
    pub(super) fn entries(&self) -> Vec<(Rc<String>, Value)> {
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
            let leaf = self.vec.leaf_for(self.next);
            let start = self.next & MASK;
            let end = leaf.len().min(start + (self.vec.len() - self.next));
            self.cur = leaf[start..end].iter();
            self.next += end - start;
        }
    }
}
