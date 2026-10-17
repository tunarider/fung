//! A persistent vector (Clojure's PersistentVector): a 32-way trie of
//! leaves plus a tail of up to 32 items. Nodes are shared through `Rc`;
//! an update copies only the nodes it touches that are shared
//! (`Rc::make_mut`), so every other version stays valid and a uniquely
//! owned vector is updated in place.

use std::rc::Rc;

const BITS: u32 = 5;
const WIDTH: usize = 1 << BITS;
pub(crate) const MASK: usize = WIDTH - 1;

/// A trie node. A leaf always holds `WIDTH` items, except the tail.
#[derive(Clone)]
enum Node<T> {
    Branch(Vec<Rc<Node<T>>>),
    Leaf(Vec<T>),
}

impl<T> Node<T> {
    fn kids(&self) -> &[Rc<Node<T>>] {
        match self {
            Node::Branch(k) => k,
            Node::Leaf(_) => unreachable!("pvec: leaf where a branch belongs"),
        }
    }

    fn kids_mut(&mut self) -> &mut Vec<Rc<Node<T>>> {
        match self {
            Node::Branch(k) => k,
            Node::Leaf(_) => unreachable!("pvec: leaf where a branch belongs"),
        }
    }

    fn vals(&self) -> &[T] {
        match self {
            Node::Leaf(v) => v,
            Node::Branch(_) => unreachable!("pvec: branch where a leaf belongs"),
        }
    }

    fn vals_mut(&mut self) -> &mut Vec<T> {
        match self {
            Node::Leaf(v) => v,
            Node::Branch(_) => unreachable!("pvec: branch where a leaf belongs"),
        }
    }
}

/// Frees a subtree with an explicit stack instead of recursion: a child
/// whose last reference this is gets its own children moved onto the
/// stack before it is dropped, so no drop ever goes deeper than one node.
impl<T> Drop for Node<T> {
    fn drop(&mut self) {
        let Node::Branch(kids) = self else { return };
        if kids.is_empty() {
            return;
        }
        let mut stack = std::mem::take(kids);
        while let Some(rc) = stack.pop() {
            if let Ok(mut node) = Rc::try_unwrap(rc)
                && let Node::Branch(k) = &mut node
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
    /// Always a `Node::Leaf` of 1..=WIDTH items (0 only when empty).
    tail: Rc<Node<T>>,
}

fn leaf_with<T>(x: T) -> Rc<Node<T>> {
    let mut vals = Vec::with_capacity(WIDTH);
    vals.push(x);
    Rc::new(Node::Leaf(vals))
}

fn new_path<T>(level: u32, node: Rc<Node<T>>) -> Rc<Node<T>> {
    let mut n = node;
    let mut l = level;
    while l > 0 {
        n = Rc::new(Node::Branch(vec![n]));
        l -= BITS;
    }
    n
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
                root: Rc::new(Node::Branch(Vec::new())),
                tail: Rc::new(Node::Leaf(Vec::with_capacity(WIDTH))),
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
            .map(|_| Rc::new(Node::Leaf(it.by_ref().take(WIDTH).collect())))
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
                up.push(Rc::new(Node::Branch(chunk)));
            }
            nodes = up;
            shift += BITS;
        }
        Pvec {
            cnt: n,
            shift,
            root: Rc::new(Node::Branch(nodes)),
            tail: Rc::new(Node::Leaf(tail)),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.cnt
    }

    fn tail_off(&self) -> usize {
        self.cnt - self.tail.vals().len()
    }

    /// The 32-item array holding index `i` (the tail or a leaf).
    pub(crate) fn leaf_for(&self, i: usize) -> &[T] {
        if i >= self.tail_off() {
            return self.tail.vals();
        }
        let mut n = &*self.root;
        let mut level = self.shift;
        while level > 0 {
            n = &n.kids()[(i >> level) & MASK];
            level -= BITS;
        }
        n.vals()
    }

    /// Item `i`; panics when out of range (callers check bounds).
    pub(crate) fn at(&self, i: usize) -> &T {
        &self.leaf_for(i)[i & MASK]
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
                    self.tail = Rc::new(Node::Leaf(vals));
                }
            }
            self.cnt += 1;
            return;
        }
        // The tail is full: it moves into the trie as a leaf (shared, not
        // copied) and x starts a new tail.
        let leaf = std::mem::replace(&mut self.tail, leaf_with(x));
        if (self.cnt >> BITS) > (1 << self.shift) {
            // the root is full: grow a level
            let path = new_path(self.shift, leaf);
            self.root = Rc::new(Node::Branch(vec![self.root.clone(), path]));
            self.shift += BITS;
        } else {
            self.push_tail(leaf);
        }
        self.cnt += 1;
    }

    /// Inserts a full leaf at the end of the trie, copying the shared
    /// nodes on its path. `self.cnt` counts the items including that leaf.
    fn push_tail(&mut self, leaf: Rc<Node<T>>) {
        let cnt = self.cnt;
        let mut level = self.shift;
        let mut n = Rc::make_mut(&mut self.root);
        loop {
            let sub = ((cnt - 1) >> level) & MASK;
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

    pub(crate) fn set(&mut self, i: usize, x: T) {
        if i >= self.tail_off() {
            Rc::make_mut(&mut self.tail).vals_mut()[i & MASK] = x;
            return;
        }
        let mut level = self.shift;
        let mut n = Rc::make_mut(&mut self.root);
        while level > 0 {
            n = Rc::make_mut(&mut n.kids_mut()[(i >> level) & MASK]);
            level -= BITS;
        }
        n.vals_mut()[i & MASK] = x;
    }

    /// Consumes the vector, handing every item of the nodes it owns alone
    /// to `f`; shared nodes are only released. Iterative, like `Drop`.
    pub(crate) fn drain(self, mut f: impl FnMut(T)) {
        let mut stack = vec![self.root, self.tail];
        while let Some(rc) = stack.pop() {
            let Ok(mut node) = Rc::try_unwrap(rc) else {
                continue;
            };
            match &mut node {
                Node::Branch(k) => stack.append(k),
                Node::Leaf(v) => v.drain(..).for_each(&mut f),
            }
        }
    }
}
