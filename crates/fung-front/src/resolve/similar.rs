//! "A similar name exists" suggestions for undefined names.
//!
//! The candidates that only grow while a file is resolved (the prelude,
//! the file's own names, the names its blocks closed) are kept in an
//! `Index` by their deletion neighbourhood: every text reached by
//! deleting up to `MAX_BUDGET` characters. Two names within `k` edits
//! (optimal string alignment) share a text reached by at most `k`
//! deletions from each, so a lookup checks only the names that can be
//! within its budget instead of every name (FE36). The few names of the
//! blocks and functions open at the error are checked one by one.

use std::collections::{HashMap, HashSet};

/// The largest edit budget (`edit_budget`).
const MAX_BUDGET: usize = 2;

/// Candidate names indexed by their deletion neighbourhood.
#[derive(Default)]
pub(super) struct Index {
    names: Vec<String>,
    known: HashSet<String>,
    by_deletion: HashMap<String, Vec<u32>>,
}

impl Index {
    /// Adds a name; a name added before is skipped.
    pub(super) fn add(&mut self, name: &str) {
        if self.known.contains(name) {
            return;
        }
        self.known.insert(name.to_string());
        let id = self.names.len() as u32;
        self.names.push(name.to_string());
        for d in deletions(name, MAX_BUDGET) {
            self.by_deletion.entry(d).or_default().push(id);
        }
    }

    /// The names that may be within `budget` edits of `name`: a superset,
    /// each to be measured.
    fn candidates(&self, name: &str, budget: usize) -> impl Iterator<Item = &str> {
        let mut ids: Vec<u32> = deletions(name, budget)
            .iter()
            .filter_map(|d| self.by_deletion.get(d))
            .flatten()
            .copied()
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter().map(|i| self.names[i as usize].as_str())
    }
}

/// Returns the name of `index` or `others` closest to `name` within an
/// edit budget that grows with the name's length (short names would
/// otherwise match almost anything), preferring the alphabetically first
/// on ties, or `None`.
pub(super) fn similar<'a>(
    name: &str,
    index: &'a Index,
    others: impl Iterator<Item = &'a str>,
) -> Option<String> {
    let budget = edit_budget(name.chars().count());
    if budget == 0 {
        return None;
    }
    let mut best: Option<(usize, &str)> = None;
    let mut rows = Rows::default();
    for c in index.candidates(name, budget).chain(others) {
        if c == name || c.chars().count().abs_diff(name.chars().count()) > budget {
            continue;
        }
        let d = rows.distance(name, c);
        if d <= budget && best.is_none_or(|b| (d, c) < b) {
            best = Some((d, c));
        }
    }
    best.map(|(_, c)| c.to_string())
}

/// How many edits may separate a typo from its intended name.
pub(super) fn edit_budget(n: usize) -> usize {
    match n {
        0..=2 => 0,
        3..=4 => 1,
        _ => MAX_BUDGET,
    }
}

/// Every text reached from `s` by deleting up to `k` characters, `s`
/// included.
fn deletions(s: &str, k: usize) -> HashSet<String> {
    let mut out = HashSet::new();
    out.insert(s.to_string());
    let mut level = vec![s.chars().collect::<Vec<char>>()];
    for _ in 0..k {
        let mut next = Vec::new();
        for cs in &level {
            for i in 0..cs.len() {
                let mut d = cs.clone();
                d.remove(i);
                if out.insert(d.iter().collect()) {
                    next.push(d);
                }
            }
        }
        level = next;
    }
    out
}

/// The rows of the distance table, kept between measurements.
#[derive(Default)]
struct Rows {
    a: Vec<char>,
    b: Vec<char>,
    d: Vec<usize>,
}

impl Rows {
    /// The optimal string alignment distance in characters: like
    /// Levenshtein, but a transposition of adjacent characters (fiels →
    /// files) counts as one edit.
    fn distance(&mut self, a: &str, b: &str) -> usize {
        self.a.clear();
        self.a.extend(a.chars());
        self.b.clear();
        self.b.extend(b.chars());
        let (ra, rb) = (&self.a, &self.b);
        let w = rb.len() + 1;
        self.d.clear();
        self.d.resize((ra.len() + 1) * w, 0);
        let d = &mut self.d;
        for i in 0..=ra.len() {
            d[i * w] = i;
        }
        for (j, cell) in d[..w].iter_mut().enumerate() {
            *cell = j;
        }
        for i in 1..=ra.len() {
            for j in 1..=rb.len() {
                let cost = usize::from(ra[i - 1] != rb[j - 1]);
                let mut v = (d[(i - 1) * w + j] + 1)
                    .min(d[i * w + j - 1] + 1)
                    .min(d[(i - 1) * w + j - 1] + cost);
                if i > 1 && j > 1 && ra[i - 1] == rb[j - 2] && ra[i - 2] == rb[j - 1] {
                    v = v.min(d[(i - 2) * w + j - 2] + 1);
                }
                d[i * w + j] = v;
            }
        }
        d[ra.len() * w + rb.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit_distance(a: &str, b: &str) -> usize {
        Rows::default().distance(a, b)
    }

    #[test]
    fn distances() {
        assert_eq!(edit_distance("fiels", "files"), 1);
        assert_eq!(edit_distance("abcd", "abxy"), 2);
        assert_eq!(edit_distance("없는", "없다"), 1);
        assert_eq!(edit_distance("", "abc"), 3);
    }

    /// The index finds what measuring every name finds.
    #[test]
    fn the_index_finds_what_a_scan_finds() {
        let words = [
            "files",
            "fiels",
            "print",
            "prnit",
            "count",
            "counter",
            "conter",
            "ab",
            "abc",
            "abd",
            "abcd",
            "bacd",
            "acbd",
            "xyzzy",
            "map",
            "mpa",
            "len",
            "lens",
            "listen",
            "silent",
            "없는",
            "없다",
            "없",
            "parse_int",
            "parse_float",
            "pares_int",
            "x1",
            "x12",
            "x123",
        ];
        let mut index = Index::default();
        for w in &words {
            index.add(w);
        }
        for q in words
            .iter()
            .chain(&["fils", "prit", "cuont", "aabc", "parseint", "zz"])
        {
            let budget = edit_budget(q.chars().count());
            let scan = words
                .iter()
                .filter(|w| *w != q && budget > 0)
                .map(|w| (edit_distance(q, w), *w))
                .filter(|(d, _)| *d <= budget)
                .min()
                .map(|(_, w)| w.to_string());
            assert_eq!(similar(q, &index, std::iter::empty()), scan, "{q}");
        }
    }
}
