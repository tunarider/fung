//! A persistent vector (Clojure's PersistentVector) that grows at both
//! ends (L5): a 32-way trie of full leaves between a head and a tail of up
//! to 32 items each. Nodes are shared through `Rc`; an update copies only
//! the nodes it touches that are shared (`Rc::make_mut`), so every other
//! version stays valid and a uniquely owned vector is updated in place.
//!
//! The trie's items sit at positions `org..end` of its index space (both
//! multiples of 32). Positions before `org` are room for leaves moved in
//! from the head: a slot there holds a placeholder (an empty branch) until
//! a leaf lands in it, and when no room is left the trie grows a level at
//! the front, the old root becoming the new root's second child. So adding
//! at either end costs O(1) amortized, and indexing stays O(log32 n).

use std::cell::Cell;
use std::rc::Rc;

use super::{Probe, Probed};

const BITS: u32 = 5;
const WIDTH: usize = 1 << BITS;
pub(crate) const MASK: usize = WIDTH - 1;

/// A trie node. A leaf always holds `WIDTH` items, except the tail.
#[derive(Clone)]
enum Node<T> {
    Branch(Vec<Rc<Node<T>>>, Memo),
    Leaf(Vec<T>, Memo),
}

/// What a node's subtree answers to each `Probe` (`holds`), found on the
/// first look and kept, two bits a probe: `UNKNOWN` until then. A node
/// changes only through `kids_mut`/`vals_mut` (on a node this vector owns
/// alone, `Rc::make_mut`), which forget all of it.
type Memo = Cell<u8>;
const UNKNOWN: u8 = 0;
const NO: u8 = 1;
const YES: u8 = 2;

/// Where a probe's two bits sit in a memo.
fn shift(p: Probe) -> u8 {
    match p {
        Probe::Mutable => 0,
        Probe::Err => 2,
        Probe::Fn => 4,
    }
}

fn leaf<T>(items: Vec<T>) -> Rc<Node<T>> {
    Rc::new(Node::Leaf(items, Cell::new(UNKNOWN)))
}

fn branch<T>(kids: Vec<Rc<Node<T>>>) -> Rc<Node<T>> {
    Rc::new(Node::Branch(kids, Cell::new(UNKNOWN)))
}

/// A slot before the trie's first position: it holds no items, so every
/// walk over the trie's nodes passes it by.
fn placeholder<T>() -> Rc<Node<T>> {
    branch(Vec::new())
}

fn is_placeholder<T>(n: &Node<T>) -> bool {
    matches!(n, Node::Branch(k, _) if k.is_empty())
}

impl<T> Node<T> {
    fn kids(&self) -> &[Rc<Node<T>>] {
        match self {
            Node::Branch(k, _) => k,
            Node::Leaf(..) => unreachable!("pvec: leaf where a branch belongs"),
        }
    }

    fn kids_mut(&mut self) -> &mut Vec<Rc<Node<T>>> {
        match self {
            Node::Branch(k, m) => {
                m.set(UNKNOWN);
                k
            }
            Node::Leaf(..) => unreachable!("pvec: leaf where a branch belongs"),
        }
    }

    fn vals(&self) -> &[T] {
        match self {
            Node::Leaf(v, _) => v,
            Node::Branch(..) => unreachable!("pvec: branch where a leaf belongs"),
        }
    }

    fn vals_mut(&mut self) -> &mut Vec<T> {
        match self {
            Node::Leaf(v, m) => {
                m.set(UNKNOWN);
                v
            }
            Node::Branch(..) => unreachable!("pvec: branch where a leaf belongs"),
        }
    }
}

/// Frees a subtree with an explicit stack instead of recursion: a child
/// whose last reference this is gets its own children moved onto the
/// stack before it is dropped, so no drop ever goes deeper than one node.
impl<T> Drop for Node<T> {
    fn drop(&mut self) {
        let Node::Branch(kids, _) = self else { return };
        if kids.is_empty() {
            return;
        }
        let mut stack = std::mem::take(kids);
        while let Some(rc) = stack.pop() {
            if let Ok(mut node) = Rc::try_unwrap(rc)
                && let Node::Branch(k, _) = &mut node
            {
                stack.append(k);
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct Pvec<T> {
    cnt: usize,
    /// Bits below the root's children; leaves are at level 0.
    shift: u32,
    root: Rc<Node<T>>,
    /// The trie's first and past-the-end positions; equal (0) when it
    /// holds no leaf.
    org: usize,
    end: usize,
    /// A `Node::Leaf` of 0..=WIDTH items before the trie.
    head: Rc<Node<T>>,
    /// A `Node::Leaf` of 0..=WIDTH items after the trie.
    tail: Rc<Node<T>>,
}

fn leaf_with<T>(x: T) -> Rc<Node<T>> {
    let mut vals = Vec::with_capacity(WIDTH);
    vals.push(x);
    leaf(vals)
}

fn new_path<T>(level: u32, node: Rc<Node<T>>) -> Rc<Node<T>> {
    let mut n = node;
    let mut l = level;
    while l > 0 {
        n = branch(vec![n]);
        l -= BITS;
    }
    n
}

/// The first `count` positions of a subtree at `level` (`count` a
/// positive multiple of `WIDTH`): the children before the cut are shared,
/// the one it falls in is cut in turn.
fn prefix<T>(node: &Rc<Node<T>>, level: u32, count: usize) -> Rc<Node<T>> {
    let per = 1 << level;
    let (full, rem) = (count / per, count % per);
    let kids = node.kids();
    let mut out: Vec<Rc<Node<T>>> = kids[..full].to_vec();
    if rem > 0 {
        out.push(prefix(&kids[full], level - BITS, rem));
    }
    branch(out)
}

/// A subtree at `level` without its positions before `from` (a multiple
/// of `WIDTH`, inside the subtree): the children before the cut become
/// placeholders, the one it falls in is cut in turn, the rest are shared.
fn suffix<T>(node: &Rc<Node<T>>, level: u32, from: usize) -> Rc<Node<T>> {
    let per = 1 << level;
    let (full, rem) = (from / per, from % per);
    let kids = node.kids();
    let mut out: Vec<Rc<Node<T>>> = Vec::with_capacity(kids.len());
    out.extend((0..full).map(|_| placeholder()));
    if rem > 0 {
        out.push(suffix(&kids[full], level - BITS, rem));
        out.extend(kids[full + 1..].iter().cloned());
    } else {
        out.extend(kids[full..].iter().cloned());
    }
    branch(out)
}

impl<T: Clone> Pvec<T> {
    /// Builds a vector in O(n): full 32-chunks become leaves, the last
    /// chunk the tail, then levels are grouped bottom-up.
    pub(crate) fn of(mut items: Vec<T>) -> Pvec<T> {
        let n = items.len();
        if n == 0 {
            return Pvec {
                cnt: 0,
                shift: BITS,
                root: branch(Vec::new()),
                org: 0,
                end: 0,
                head: leaf(Vec::new()),
                tail: leaf(Vec::with_capacity(WIDTH)),
            };
        }
        let tail_len = if n.is_multiple_of(WIDTH) {
            WIDTH
        } else {
            n % WIDTH
        };
        let off = n - tail_len;
        let mut tail = Vec::with_capacity(WIDTH);
        tail.extend(items.drain(off..));
        let mut it = items.into_iter();
        let mut nodes: Vec<Rc<Node<T>>> = (0..off / WIDTH)
            .map(|_| leaf(it.by_ref().take(WIDTH).collect()))
            .collect();
        let mut shift = BITS;
        while nodes.len() > WIDTH {
            let mut it = nodes.into_iter();
            let mut up = Vec::new();
            loop {
                let chunk: Vec<_> = it.by_ref().take(WIDTH).collect();
                if chunk.is_empty() {
                    break;
                }
                up.push(branch(chunk));
            }
            nodes = up;
            shift += BITS;
        }
        Pvec {
            cnt: n,
            shift,
            root: branch(nodes),
            org: 0,
            end: off,
            head: leaf(Vec::new()),
            tail: leaf(tail),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.cnt
    }

    /// Whether no other vector shares its root, head or tail: dropping it
    /// frees its items (otherwise it only lets go of shared nodes).
    pub(crate) fn owns_nodes(&self) -> bool {
        Rc::strong_count(&self.root) == 1
            && Rc::strong_count(&self.head) == 1
            && Rc::strong_count(&self.tail) == 1
    }

    fn head_len(&self) -> usize {
        self.head.vals().len()
    }

    fn tail_off(&self) -> usize {
        self.cnt - self.tail.vals().len()
    }

    /// The trie's position of item `i` (one of its items).
    fn pos(&self, i: usize) -> usize {
        self.org + (i - self.head_len())
    }

    /// The leaf at trie position `p`.
    fn leaf_at(&self, p: usize) -> &[T] {
        let mut n = &*self.root;
        let mut level = self.shift;
        while level > 0 {
            n = &n.kids()[(p >> level) & MASK];
            level -= BITS;
        }
        n.vals()
    }

    /// The array holding item `i` (the head, a leaf or the tail) and the
    /// index of item `i` in it.
    pub(crate) fn leaf_for(&self, i: usize) -> (&[T], usize) {
        let h = self.head_len();
        if i < h {
            return (self.head.vals(), i);
        }
        let off = self.tail_off();
        if i >= off {
            return (self.tail.vals(), i - off);
        }
        let p = self.pos(i);
        (self.leaf_at(p), p & MASK)
    }

    /// Item `i`; panics when out of range (callers check bounds).
    pub(crate) fn at(&self, i: usize) -> &T {
        let (leaf, j) = self.leaf_for(i);
        &leaf[j]
    }

    /// The trie without leaves.
    fn clear_trie(&mut self) {
        self.root = branch(Vec::new());
        self.shift = BITS;
        self.org = 0;
        self.end = 0;
    }

    pub(crate) fn push(&mut self, x: T) {
        if self.tail.vals().len() < WIDTH {
            match Rc::get_mut(&mut self.tail) {
                Some(t) => t.vals_mut().push(x),
                None => {
                    // shared: copy the tail, never write through it
                    let mut vals = Vec::with_capacity(WIDTH);
                    vals.extend_from_slice(self.tail.vals());
                    vals.push(x);
                    self.tail = leaf(vals);
                }
            }
            self.cnt += 1;
            return;
        }
        // The tail is full: it moves into the trie as a leaf (shared, not
        // copied) and x starts a new tail.
        let leaf = std::mem::replace(&mut self.tail, leaf_with(x));
        self.push_leaf(leaf);
        self.cnt += 1;
    }

    /// Puts a full leaf at the trie's end.
    fn push_leaf(&mut self, leaf: Rc<Node<T>>) {
        if self.end + WIDTH > 1 << (self.shift + BITS) {
            // the root is full: grow a level
            let path = new_path(self.shift, leaf);
            self.root = branch(vec![self.root.clone(), path]);
            self.shift += BITS;
        } else {
            self.push_tail(leaf);
        }
        self.end += WIDTH;
    }

    /// Inserts a full leaf at position `end`, copying the shared nodes on
    /// its path; the root has room for it.
    fn push_tail(&mut self, leaf: Rc<Node<T>>) {
        let p = self.end;
        let mut level = self.shift;
        let mut n = Rc::make_mut(&mut self.root);
        loop {
            let sub = (p >> level) & MASK;
            let kids = n.kids_mut();
            if level == BITS {
                if sub < kids.len() {
                    kids[sub] = leaf;
                } else {
                    kids.push(leaf);
                }
                return;
            }
            if sub < kids.len() {
                n = Rc::make_mut(&mut kids[sub]);
                level -= BITS;
            } else {
                kids.push(new_path(level - BITS, leaf));
                return;
            }
        }
    }

    /// Puts `x` before the first item: into the head while it has room,
    /// else the full head moves into the trie as a leaf (shared, not
    /// copied) and `x` starts a new head.
    pub(crate) fn push_front(&mut self, x: T) {
        self.cnt += 1;
        if self.head_len() < WIDTH {
            match Rc::get_mut(&mut self.head) {
                Some(h) => h.vals_mut().insert(0, x),
                None => {
                    let mut vals = Vec::with_capacity(WIDTH);
                    vals.push(x);
                    vals.extend_from_slice(self.head.vals());
                    self.head = leaf(vals);
                }
            }
            return;
        }
        let full = std::mem::replace(&mut self.head, leaf_with(x));
        if self.org == self.end {
            // no leaf yet: the head's leaf is the first and the last
            self.clear_trie();
            self.push_leaf(full);
            return;
        }
        if self.org == 0 {
            // no room before the trie: grow a level at the front
            let cap = 1 << (self.shift + BITS);
            self.root = branch(vec![placeholder(), self.root.clone()]);
            self.shift += BITS;
            self.org += cap;
            self.end += cap;
        }
        self.org -= WIDTH;
        let p = self.org;
        let mut level = self.shift;
        let mut n = Rc::make_mut(&mut self.root);
        loop {
            let kid = &mut n.kids_mut()[(p >> level) & MASK];
            if level == BITS {
                *kid = full;
                return;
            }
            if is_placeholder(kid) {
                *kid = branch((0..WIDTH).map(|_| placeholder()).collect());
            }
            n = Rc::make_mut(kid);
            level -= BITS;
        }
    }

    /// Keeps the first `n` items (`n <= len`) in O(log n): the leaf that
    /// holds item `n - 1` becomes the tail, and only the path to it is
    /// copied; the leaves before it stay shared (P3).
    pub(crate) fn truncate(&mut self, n: usize) {
        assert!(n <= self.cnt, "pvec: truncate past the end");
        if n == self.cnt {
            return;
        }
        let h = self.head_len();
        if n <= h {
            *self = Pvec::of(self.head.vals()[..n].to_vec());
            return;
        }
        let off = self.tail_off();
        if n > off {
            let keep = n - off;
            Rc::make_mut(&mut self.tail).vals_mut().truncate(keep);
            self.cnt = n;
            return;
        }
        let last = self.pos(n - 1);
        let start = last & !MASK;
        let mut vals = Vec::with_capacity(WIDTH);
        vals.extend_from_slice(&self.leaf_at(last)[..=last - start]);
        self.tail = leaf(vals);
        self.cnt = n;
        if start == self.org {
            self.clear_trie();
            return;
        }
        let mut root = prefix(&self.root, self.shift, start);
        let mut shift = self.shift;
        while shift > BITS && root.kids().len() == 1 {
            root = root.kids()[0].clone();
            shift -= BITS;
        }
        self.root = root;
        self.shift = shift;
        self.end = start;
    }

    /// Drops the first `k` items (`k <= len`) in O(log n), the mirror of
    /// `truncate`: the rest of the leaf that holds item `k` becomes the
    /// head, the slots before it in the trie become placeholders, and
    /// only the path to it is copied.
    pub(crate) fn drop_front(&mut self, k: usize) {
        assert!(k <= self.cnt, "pvec: drop past the end");
        if k == 0 {
            return;
        }
        let h = self.head_len();
        if k <= h {
            let rest = self.head.vals()[k..].to_vec();
            self.head = leaf(rest);
            self.cnt -= k;
            return;
        }
        let off = self.tail_off();
        if k >= off {
            *self = Pvec::of(self.tail.vals()[k - off..].to_vec());
            return;
        }
        let first = self.pos(k);
        let start = first & !MASK;
        self.head = leaf(self.leaf_at(first)[first - start..].to_vec());
        self.cnt -= k;
        let from = start + WIDTH;
        if from == self.end {
            self.clear_trie();
            return;
        }
        let mut root = suffix(&self.root, self.shift, from);
        // Children of the root that are all placeholders are let go, the
        // positions moving down with them, so a vector used as a queue
        // does not grow deeper forever.
        let per = 1usize << self.shift;
        let gone = from / per;
        let (mut org, mut end) = (from, self.end);
        if gone > 0 {
            let kids: Vec<_> = root.kids()[gone..].to_vec();
            root = branch(kids);
            org -= gone * per;
            end -= gone * per;
        }
        let mut shift = self.shift;
        while shift > BITS && root.kids().len() == 1 {
            root = root.kids()[0].clone();
            shift -= BITS;
        }
        self.root = root;
        self.shift = shift;
        self.org = org;
        self.end = end;
    }

    pub(crate) fn set(&mut self, i: usize, x: T) {
        let h = self.head_len();
        if i < h {
            Rc::make_mut(&mut self.head).vals_mut()[i] = x;
            return;
        }
        let off = self.tail_off();
        if i >= off {
            Rc::make_mut(&mut self.tail).vals_mut()[i - off] = x;
            return;
        }
        let p = self.pos(i);
        let mut level = self.shift;
        let mut n = Rc::make_mut(&mut self.root);
        while level > 0 {
            n = Rc::make_mut(&mut n.kids_mut()[(p >> level) & MASK]);
            level -= BITS;
        }
        n.vals_mut()[p & MASK] = x;
    }

    /// Consumes the vector, handing every item of the nodes it owns alone
    /// to `f`; shared nodes are only released. Iterative, like `Drop`.
    pub(crate) fn drain(self, mut f: impl FnMut(T)) {
        let mut stack = vec![self.root, self.tail, self.head];
        while let Some(rc) = stack.pop() {
            let Ok(mut node) = Rc::try_unwrap(rc) else {
                continue;
            };
            match &mut node {
                Node::Branch(k, _) => stack.append(k),
                Node::Leaf(v, _) => v.drain(..).for_each(&mut f),
            }
        }
    }
}

impl<T: Probed> Pvec<T> {
    /// Whether an item answers `p` (`Probed::has`): the trie's nodes
    /// remember the answer, so after a change only the copied path is
    /// looked at again; the tail (at most 32) is scanned.
    pub(crate) fn holds(&self, p: Probe) -> bool {
        self.head.vals().iter().any(|x| x.has(p))
            || node_holds(&self.root, p)
            || self.tail.vals().iter().any(|x| x.has(p))
    }
}

impl<T: Probed + Clone> Pvec<T> {
    /// `holds` for the items `lo..hi` only (a list's slice): the subtrees
    /// inside the range answer from their memo, and only the leaves the
    /// range cuts are scanned, so a slice costs about what the whole
    /// vector does.
    pub(crate) fn holds_in(&self, lo: usize, hi: usize, p: Probe) -> bool {
        let h = self.head_len();
        if lo < h && self.head.vals()[lo..hi.min(h)].iter().any(|x| x.has(p)) {
            return true;
        }
        let off = self.tail_off();
        if hi > off {
            let from = lo.max(off) - off;
            if self.tail.vals()[from..hi - off].iter().any(|x| x.has(p)) {
                return true;
            }
        }
        let (a, b) = (lo.max(h), hi.min(off));
        a < b
            && range_holds(
                &self.root,
                self.shift,
                0,
                self.pos(a),
                self.pos(b - 1) + 1,
                p,
            )
    }
}

impl<T: Probed> Pvec<T> {
    /// Hands `f` each item of `lo..hi` that answers `p`, a node at a time:
    /// a subtree whose memo says no is passed by, and a shared node that
    /// `seen` has met already (it returns false) is not entered again, so
    /// many views on one vector cost about their cut leaves each (L5, the
    /// `send` walk).
    pub(crate) fn each_holding(
        &self,
        lo: usize,
        hi: usize,
        p: Probe,
        seen: &mut dyn FnMut(*const ()) -> bool,
        f: &mut dyn FnMut(&T),
    ) {
        let h = self.head.vals().len();
        if lo < h {
            self.head.vals()[lo..hi.min(h)]
                .iter()
                .filter(|x| x.has(p))
                .for_each(&mut *f);
        }
        let off = self.cnt - self.tail.vals().len();
        let (a, b) = (lo.max(h), hi.min(off));
        if a < b {
            let (pa, pb) = (self.org + (a - h), self.org + (b - 1 - h) + 1);
            range_each(&self.root, self.shift, 0, pa, pb, p, seen, f);
        }
        if hi > off {
            let from = lo.max(off) - off;
            self.tail.vals()[from..hi - off]
                .iter()
                .filter(|x| x.has(p))
                .for_each(f);
        }
    }
}

/// `each_holding` over positions `lo..hi` of the subtree `n` (at `level`,
/// whose first position is `start`).
#[allow(clippy::too_many_arguments)]
fn range_each<T: Probed>(
    n: &Node<T>,
    level: u32,
    start: usize,
    lo: usize,
    hi: usize,
    p: Probe,
    seen: &mut dyn FnMut(*const ()) -> bool,
    f: &mut dyn FnMut(&T),
) {
    match n {
        Node::Leaf(v, _) => v[lo - start..hi - start]
            .iter()
            .filter(|x| x.has(p))
            .for_each(f),
        Node::Branch(k, _) => {
            let width = 1usize << level;
            for (j, kid) in k.iter().enumerate() {
                let ks = start + j * width;
                let ke = ks + width;
                if ke <= lo || ks >= hi || !node_holds(kid, p) {
                    continue;
                }
                let whole = lo <= ks && ke <= hi;
                if whole && Rc::strong_count(kid) > 1 && !seen(Rc::as_ptr(kid).cast()) {
                    continue;
                }
                range_each(kid, level - BITS, ks, lo.max(ks), hi.min(ke), p, seen, f);
            }
        }
    }
}

/// Whether an item `lo..hi` of the subtree `n` (at `level`, whose first
/// item is `start`) answers `p`.
fn range_holds<T: Probed>(
    n: &Node<T>,
    level: u32,
    start: usize,
    lo: usize,
    hi: usize,
    p: Probe,
) -> bool {
    match n {
        Node::Leaf(v, _) => v[lo - start..hi - start].iter().any(|x| x.has(p)),
        Node::Branch(k, _) => {
            let width = 1usize << level;
            k.iter().enumerate().any(|(j, kid)| {
                let ks = start + j * width;
                let ke = ks + width;
                if ke <= lo || ks >= hi {
                    false
                } else if lo <= ks && ke <= hi {
                    node_holds(kid, p)
                } else {
                    range_holds(kid, level - BITS, ks, lo.max(ks), hi.min(ke), p)
                }
            })
        }
    }
}

fn node_holds<T: Probed>(n: &Node<T>, p: Probe) -> bool {
    let memo = match n {
        Node::Branch(_, m) | Node::Leaf(_, m) => m,
    };
    let s = shift(p);
    match (memo.get() >> s) & 3 {
        YES => return true,
        NO => return false,
        _ => {}
    }
    let held = match n {
        Node::Branch(k, _) => k.iter().any(|c| node_holds(c, p)),
        Node::Leaf(v, _) => v.iter().any(|x| x.has(p)),
    };
    let answer = if held { YES } else { NO };
    memo.set(memo.get() & !(3 << s) | answer << s);
    held
}
