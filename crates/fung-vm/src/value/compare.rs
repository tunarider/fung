use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

use fung_front::diag::Diag;

use super::Value;
use super::arith::compare_error;
use super::format::{Items, is_collection};

/// `==` without its errors, for literal patterns (D40: a pattern tests
/// the type, so a value of another type simply does not match): numbers
/// compare by value across int/float (exactly), same types by value,
/// handles by identity, different types are never equal. Collections are
/// walked pair by pair with an explicit stack (L9): the Rust stack does
/// not grow with the values' depth. The walk stops at the first
/// difference: whether `==` is an error does not depend on the walk
/// (`check_fns`, D116).
pub fn equal(a: &Value, b: &Value) -> bool {
    walk(a, b)
}

/// The one walk behind `equal` and `eq`. Values of different types are
/// not equal (D91).
fn walk(a: &Value, b: &Value) -> bool {
    if !(is_collection(a) && is_collection(b)) {
        return equal_leaf(a, b);
    }
    let mut open: Vec<Pairs<'_>> = Vec::new();
    // The pairs of shared collections already walked (VA44): the walk
    // stops at the first difference, so a pair met again is equal, and
    // two values that share their parts many times over are walked once
    // per part, not once per path to it.
    let mut walked = Walked::default();
    let mut next = Some((a, b));
    loop {
        // A shared collection equals itself without a look inside.
        if let Some((a, b)) = next.take()
            && !same_object(a, b)
            && walked.first_time(a, b)
        {
            match kids(a, b) {
                Kids::Pairs(p) => open.push(p),
                Kids::Mismatch => return false,
                Kids::None => {
                    if !equal_leaf(a, b) {
                        return false;
                    }
                }
            }
        }
        let Some(top) = open.last_mut() else {
            return true;
        };
        match top.next() {
            Some((x, Some(y))) => next = Some((x, y)),
            // a key of the left map that the right one lacks
            Some((_, None)) => return false,
            None => {
                open.pop();
            }
        }
    }
}

/// Two tuples of different sizes met in an order comparison (D87): a
/// tuple's size is fixed, so there is no order between them.
fn tuple_sizes(a: &Value, b: &Value) -> Diag {
    compare_error(a, b)
        .with_help("tuples of different sizes are different types: a tuple's size is fixed")
}

/// Functions have no equality (D62), and neither has a value that holds
/// one: two values that both hold a function, at any depth of their
/// lists, maps, tuples and tags, are E0501 for `==` whatever else they
/// hold and wherever the functions stand (D116). The answer depends on
/// the two values as wholes, not on the order of their items, their sizes
/// or what they share (VA43). A function on one side only is a
/// difference like any other: `cb == nil` is `false` (D91).
fn check_fns(a: &Value, b: &Value) -> Result<(), Diag> {
    if a.holds_fn() && b.holds_fn() {
        let help = if is_fn(a) && is_fn(b) {
            "functions have no equality; compare a name or a value that stands for each"
        } else {
            "both values hold a function, and functions have no equality; compare the parts that are not functions"
        };
        return Err(compare_error(a, b).with_help(help));
    }
    Ok(())
}

fn is_fn(v: &Value) -> bool {
    matches!(v, Value::Closure(_) | Value::Native(_))
}

/// `==` on two values that are not two collections of one kind.
fn equal_leaf(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Int(x), Value::Float(y)) | (Value::Float(y), Value::Int(x)) => {
            cmp_int_float(*x, *y) == Ordering::Equal
        }
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Nil, Value::Nil) => true,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Sym(x), Value::Sym(y)) => x == y,
        (Value::Dur(x), Value::Dur(y)) => x == y,
        (Value::Time(x), Value::Time(y)) => x.equal(y),
        (Value::Range(x), Value::Range(y)) => x == y,
        (Value::Closure(x), Value::Closure(y)) => Rc::ptr_eq(x, y),
        (Value::Native(x), Value::Native(y)) => std::ptr::eq(*x, *y),
        (Value::Module(x), Value::Module(y)) => Rc::ptr_eq(x, y),
        (Value::Task(x), Value::Task(y)) => Rc::ptr_eq(x, y),
        (Value::Scope(x), Value::Scope(y)) => Rc::ptr_eq(x, y),
        (Value::Atom(x), Value::Atom(y)) => Rc::ptr_eq(x, y),
        (Value::Sender(x), Value::Sender(y)) => Rc::ptr_eq(x, y),
        (Value::Receiver(x), Value::Receiver(y)) => Rc::ptr_eq(x, y),
        (Value::File(x), Value::File(y)) => std::sync::Arc::ptr_eq(x, y),
        (Value::Conn(x), Value::Conn(y)) => std::sync::Arc::ptr_eq(x, y),
        (Value::Listener(x), Value::Listener(y)) => std::sync::Arc::ptr_eq(x, y),
        _ => false,
    }
}

/// The items of two collections walked side by side (`equal`,
/// `walk`): two lists, tuples or tags item by item; two maps by
/// the left one's keys, with the right one's value under each (`None`
/// where it has no such key; order does not matter).
enum Pairs<'a> {
    Seq(Items<'a>, Items<'a>),
    Map(super::MapIter<'a>, &'a super::Map),
}

impl<'a> Pairs<'a> {
    fn next(&mut self) -> Option<(&'a Value, Option<&'a Value>)> {
        match self {
            Pairs::Seq(x, y) => Some((x.next()?, y.next())),
            Pairs::Map(x, y) => {
                let (k, xv) = x.next()?;
                Some((xv, y.get(k)))
            }
        }
    }
}

/// What two values hold to compare item by item.
enum Kids<'a> {
    /// Two collections of one kind and shape: their items, paired.
    Pairs(Pairs<'a>),
    /// Two collections of one kind whose shapes differ (lengths, tag
    /// names): never equal.
    Mismatch,
    /// Not two collections of one kind.
    None,
}

fn kids<'a>(a: &'a Value, b: &'a Value) -> Kids<'a> {
    let seq = |x: Items<'a>, xn: usize, y: Items<'a>, yn: usize| {
        if xn == yn {
            Kids::Pairs(Pairs::Seq(x, y))
        } else {
            Kids::Mismatch
        }
    };
    match (a, b) {
        (Value::List(x), Value::List(y)) => seq(
            Items::List(x.iter()),
            x.len(),
            Items::List(y.iter()),
            y.len(),
        ),
        (Value::Tuple(x), Value::Tuple(y)) => seq(
            Items::Slice(x.iter()),
            x.len(),
            Items::Slice(y.iter()),
            y.len(),
        ),
        (Value::Tag(x), Value::Tag(y)) if x.name == y.name => seq(
            Items::Slice(x.args.iter()),
            x.args.len(),
            Items::Slice(y.args.iter()),
            y.args.len(),
        ),
        (Value::Tag(_), Value::Tag(_)) => Kids::Mismatch,
        (Value::Map(x), Value::Map(y)) if x.len() == y.len() => {
            Kids::Pairs(Pairs::Map(x.iter(), y))
        }
        (Value::Map(_), Value::Map(_)) => Kids::Mismatch,
        _ => Kids::None,
    }
}

/// `==` and `!=` as programs see them (02): `equal`, except that two
/// values that both hold a function are E0501 (D62, D116). Values of
/// different types are not equal, like any two different values (D91;
/// E0517, which D40 made of them, is retired). Patterns keep `equal`.
pub fn eq(a: &Value, b: &Value) -> Result<bool, Diag> {
    check_fns(a, b)?;
    Ok(walk(a, b))
}

/// Whether `a` and `b` are the same shared collection (P5): such a value
/// equals itself, and fung has no NaN to say otherwise.
fn same_object(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::List(x), Value::List(y)) => Rc::ptr_eq(x, y),
        (Value::Map(x), Value::Map(y)) => Rc::ptr_eq(x, y),
        (Value::Tuple(x), Value::Tuple(y)) => Rc::ptr_eq(x, y),
        (Value::Tag(x), Value::Tag(y)) => Rc::ptr_eq(x, y),
        (Value::Str(x), Value::Str(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

/// `< > <= >=` for numbers, strings (code point order), durations, times, lists
/// and tuples, lexicographic (a shorter prefix is smaller). Anything else
/// is E0501, from the first pair of items that do not compare, naming
/// their types; two tuples of different sizes do not compare. Lists and
/// tuples are walked with an explicit stack (L9).
pub fn compare(a: &Value, b: &Value) -> Result<Ordering, Diag> {
    if !matches!(
        (a, b),
        (Value::List(_), Value::List(_)) | (Value::Tuple(_), Value::Tuple(_))
    ) {
        return compare_leaf(a, b);
    }
    // Each open pair: what is left of both, and their lengths.
    let mut open: Vec<(Items<'_>, Items<'_>, usize, usize)> = Vec::new();
    // The pairs of shared collections already compared (VA36, VA44): a
    // pair that did not compare stopped the walk the first time, and what
    // it added to the answer the first time is all it can add; a value
    // shared many times is walked once.
    let mut walked = Walked::default();
    let mut next = Some((a, b));
    // The answer is the first difference, but the walk goes on to the end
    // of what the two sides line up (D87): a pair that does not compare
    // is an error wherever it stands, not only before the first
    // difference.
    let mut found = Ordering::Equal;
    loop {
        if let Some((a, b)) = next.take()
            && walked.first_time(a, b)
        {
            match (a, b) {
                (Value::Tuple(x), Value::Tuple(y)) => {
                    if x.len() != y.len() {
                        return Err(tuple_sizes(a, b));
                    }
                    open.push((
                        Items::Slice(x.iter()),
                        Items::Slice(y.iter()),
                        x.len(),
                        y.len(),
                    ));
                }
                (Value::List(x), Value::List(y)) => open.push((
                    Items::List(x.iter()),
                    Items::List(y.iter()),
                    x.len(),
                    y.len(),
                )),
                _ => found = found.then(compare_leaf(a, b)?),
            }
        }
        let Some((x, y, xn, yn)) = open.last_mut() else {
            return Ok(found);
        };
        match (x.next(), y.next()) {
            (Some(p), Some(q)) => next = Some((p, q)),
            _ => {
                found = found.then((*xn).cmp(yn));
                open.pop();
            }
        }
    }
}

/// The one shape of the values an ordering function sorts or searches
/// (VA60): `sort`, `sort_by`, `min` and `max` fold every element (key)
/// into it before they order any, and the first that does not fit is
/// E0501. Two values order exactly when their shapes unify (`compare`
/// looks at every pair the two sides line up, D87), so the elements
/// order pairwise exactly when all of them unify into one shape, which
/// does not depend on their order nor on their values. A number, a
/// string, a duration and a time are each a shape; a tuple is its size
/// and a shape per position; a list a shape per position, as many as
/// its longest element has (a shorter list says nothing of the positions
/// it lacks). A value of any other type has no order: it fails as soon
/// as anything meets it, or at the end (`finish`) as `x < x` would.
///
/// The nodes never change: folding a value in makes new nodes where the
/// shape grows and shares the rest, so a node can stand at many places.
/// Within one element, folding a collection with more than one holder
/// into a node is remembered (VA44): a value that shares its parts many
/// times over is folded once per (node, part), as `compare` walks such a
/// pair once. So the work is linear in the size of the elements,
/// counting a shared part once per node it meets.
pub struct Shapes {
    /// The nodes; the first is the fresh node no value has made.
    nodes: Vec<Shape>,
    /// The shape so far.
    root: usize,
    /// The first node made by a value with no order (`Kind::None`).
    unordered: Option<usize>,
    /// What folding a shared collection into a node gave.
    folded: HashMap<(usize, *const ()), usize>,
}

/// A node of the shape and the first value that made it, which the
/// errors name as `compare` would.
struct Shape {
    kind: Kind,
    first: Value,
}

enum Kind {
    /// No value has made the node yet (the first node only).
    Fresh,
    Num,
    Str,
    Dur,
    Time,
    Tuple(Rc<[usize]>),
    List(Rc<[usize]>),
    /// A value with no order.
    None,
}

/// The fresh node.
const FRESH: usize = 0;

/// A collection being folded into a node: its items still to fold, the
/// kids they go into, and what the kids became so far.
struct Fold<'v> {
    node: usize,
    value: &'v Value,
    items: Items<'v>,
    kids: Vec<usize>,
    done: usize,
}

impl Default for Shapes {
    fn default() -> Shapes {
        Shapes {
            nodes: vec![Shape {
                kind: Kind::Fresh,
                first: Value::Nil,
            }],
            root: FRESH,
            unordered: None,
            folded: HashMap::new(),
        }
    }
}

impl Shapes {
    /// Folds `v` into the shape: E0501 where it does not fit.
    pub fn add(&mut self, v: &Value) -> Result<(), Diag> {
        // An element itself is folded in whoever else holds it (the
        // caller's cursor does); what it shares inside is remembered.
        let mut open: Vec<Fold<'_>> = Vec::new();
        // An address only stands for one value while an element holds it.
        self.folded.clear();
        let mut got = match self.open(self.root, v)? {
            Ok(f) => {
                open.push(f);
                None
            }
            Err(n) => Some(n),
        };
        while let Some(top) = open.last_mut() {
            if let Some(n) = got.take() {
                top.kids[top.done] = n;
                top.done += 1;
            }
            let Some(x) = top.items.next() else {
                let f = open.pop().expect("the top");
                let n = self.close(f);
                got = Some(n);
                continue;
            };
            let k = top.kids[top.done];
            let key = shared(x).map(|p| (k, p));
            if let Some(n) = key.and_then(|key| self.folded.get(&key)) {
                got = Some(*n);
                continue;
            }
            match self.open(k, x)? {
                Ok(f) => open.push(f),
                Err(n) => got = Some(n),
            }
        }
        self.root = got.expect("the root");
        Ok(())
    }

    /// E0501 when some position holds a value with no order: `x < x`
    /// fails for it, alone as among others.
    pub fn finish(&self) -> Result<(), Diag> {
        match self.unordered {
            Some(at) => {
                let first = &self.nodes[at].first;
                compare_leaf(first, first).map(|_| ())
            }
            None => Ok(()),
        }
    }

    /// Starts folding `v` into node `at`: a collection to walk (`Ok`), or
    /// the node a plain value leaves (`Err`).
    #[allow(clippy::type_complexity)]
    fn open<'v>(&mut self, at: usize, v: &'v Value) -> Result<Result<Fold<'v>, usize>, Diag> {
        if self.flat_fit(at, v)? {
            return Ok(Err(at));
        }
        let fresh = matches!(self.nodes[at].kind, Kind::Fresh);
        let (items, kids) = match (v, &self.nodes[at].kind) {
            (Value::Tuple(x), Kind::Fresh) => (Items::Slice(x.iter()), vec![FRESH; x.len()]),
            (Value::Tuple(x), Kind::Tuple(k)) if k.len() == x.len() => {
                (Items::Slice(x.iter()), k.to_vec())
            }
            (Value::List(x), Kind::Fresh) => (Items::List(x.iter()), vec![FRESH; x.len()]),
            (Value::List(x), Kind::List(k)) => {
                // a longer list adds the positions it has beyond
                let mut kids = k.to_vec();
                kids.resize(kids.len().max(x.len()), FRESH);
                (Items::List(x.iter()), kids)
            }
            _ if fresh => return Ok(Err(self.leaf(v))),
            _ => {
                self.fit(at, v)?;
                return Ok(Err(at));
            }
        };
        Ok(Ok(Fold {
            node: at,
            value: v,
            items,
            kids,
            done: 0,
        }))
    }

    /// Whether `v` is a collection of plain values that fits the made
    /// node `at` as it is (the usual row of a sort): checked without
    /// walking it as a collection. `false` leaves the walk to `open`.
    fn flat_fit(&self, at: usize, v: &Value) -> Result<bool, Diag> {
        let (kids, items) = match (&self.nodes[at].kind, v) {
            (Kind::Tuple(k), Value::Tuple(x)) if k.len() == x.len() => (k, Items::Slice(x.iter())),
            (Kind::List(k), Value::List(x)) if k.len() >= x.len() => (k, Items::List(x.iter())),
            _ => return Ok(false),
        };
        for (&k, x) in kids.iter().zip(items) {
            if is_collection(x) || matches!(self.nodes[k].kind, Kind::Fresh) {
                return Ok(false);
            }
        }
        let items = match v {
            Value::Tuple(x) => Items::Slice(x.iter()),
            Value::List(x) => Items::List(x.iter()),
            _ => unreachable!("a collection"),
        };
        for (&k, x) in kids.iter().zip(items) {
            self.fit(k, x)?;
        }
        Ok(true)
    }

    /// The node a folded collection leaves: the old one when no kid
    /// changed.
    fn close(&mut self, f: Fold<'_>) -> usize {
        let node = &self.nodes[f.node];
        let n = match &node.kind {
            Kind::Tuple(k) | Kind::List(k) if **k == f.kids[..] => f.node,
            kind => {
                let first = match kind {
                    Kind::Fresh => f.value.clone(),
                    _ => node.first.clone(),
                };
                let kind = match f.value {
                    Value::Tuple(_) => Kind::Tuple(f.kids.into()),
                    _ => Kind::List(f.kids.into()),
                };
                self.push(kind, first)
            }
        };
        if let Some(p) = shared(f.value) {
            self.folded.insert((f.node, p), n);
        }
        n
    }

    /// The node a plain value makes of the fresh node.
    fn leaf(&mut self, v: &Value) -> usize {
        let kind = match v {
            Value::Int(_) | Value::Float(_) => Kind::Num,
            Value::Str(_) => Kind::Str,
            Value::Dur(_) => Kind::Dur,
            Value::Time(_) => Kind::Time,
            _ => Kind::None,
        };
        let none = matches!(kind, Kind::None);
        let n = self.push(kind, v.clone());
        if none {
            self.unordered.get_or_insert(n);
        }
        n
    }

    fn push(&mut self, kind: Kind, first: Value) -> usize {
        self.nodes.push(Shape { kind, first });
        self.nodes.len() - 1
    }

    /// E0501, as `compare` gives it, when the plain value `v` (or a
    /// collection of another kind) does not fit the made node `at`.
    fn fit(&self, at: usize, v: &Value) -> Result<(), Diag> {
        let node = &self.nodes[at];
        let fits = matches!(
            (&node.kind, v),
            (Kind::Num, Value::Int(_) | Value::Float(_))
                | (Kind::Str, Value::Str(_))
                | (Kind::Dur, Value::Dur(_))
                | (Kind::Time, Value::Time(_))
        );
        if fits {
            return Ok(());
        }
        match (&node.first, v) {
            (a @ Value::Tuple(_), b @ Value::Tuple(_)) => Err(tuple_sizes(a, b)),
            (a, b) => Err(compare_error(a, b)),
        }
    }
}

/// How many pairs a walk looks at before it starts to remember them: a
/// small comparison (a `sort`'s, of two short tuples) pays nothing for
/// the memory, and what a long walk met before it began to remember is
/// walked at most once more.
const REMEMBER_AFTER: u32 = 64;

/// The pairs of shared collections a walk has been through.
#[derive(Default)]
struct Walked {
    steps: u32,
    pairs: HashSet<(*const (), *const ())>,
}

impl Walked {
    /// Whether the walk has not been through the pair `a`, `b` since it
    /// began to remember. Only a pair of collections something else holds
    /// too can be met twice (a collection with one holder is reached by
    /// one path).
    #[inline]
    fn first_time(&mut self, a: &Value, b: &Value) -> bool {
        if self.steps < REMEMBER_AFTER {
            self.steps += 1;
            return true;
        }
        match (shared(a), shared(b)) {
            (Some(x), Some(y)) => self.pairs.insert((x, y)),
            _ => true,
        }
    }
}

/// The address of a collection with more than one holder.
fn shared(v: &Value) -> Option<*const ()> {
    match v {
        Value::List(x) if Rc::strong_count(x) > 1 => Some(Rc::as_ptr(x).cast()),
        Value::Tuple(x) if Rc::strong_count(x) > 1 => Some(Rc::as_ptr(x).cast()),
        Value::Tag(x) if Rc::strong_count(x) > 1 => Some(Rc::as_ptr(x).cast()),
        Value::Map(x) if Rc::strong_count(x) > 1 => Some(Rc::as_ptr(x).cast()),
        _ => None,
    }
}

/// `compare` on two values that are not two lists or two tuples.
fn compare_leaf(a: &Value, b: &Value) -> Result<Ordering, Diag> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Ok(x.cmp(y)),
        (Value::Int(x), Value::Float(y)) => Ok(cmp_int_float(*x, *y)),
        (Value::Float(x), Value::Int(y)) => Ok(cmp_int_float(*y, *x).reverse()),
        (Value::Float(x), Value::Float(y)) => Ok(cmp_floats(*x, *y)),
        // Bytes: for valid UTF-8 the same order as code points (L4).
        (Value::Str(x), Value::Str(y)) => Ok(x.as_bytes().cmp(y.as_bytes())),
        (Value::Dur(x), Value::Dur(y)) => Ok(x.cmp(y)),
        (Value::Time(x), Value::Time(y)) => Ok(x.cmp_instant(y)),
        _ => Err(compare_error(a, b)),
    }
}

fn cmp_floats(x: f64, y: f64) -> Ordering {
    if x < y {
        Ordering::Less
    } else if x > y {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// Compares an int and a float exactly, without converting the int to f64.
/// Floats are never NaN in fung.
fn cmp_int_float(i: i64, f: f64) -> Ordering {
    if f >= 9223372036854775808.0 {
        return Ordering::Less;
    }
    if f < -9223372036854775808.0 {
        return Ordering::Greater;
    }
    let t = f as i64; // exact: |f| < 2^63 and truncation is exact
    match i.cmp(&t) {
        Ordering::Equal => {}
        o => return o,
    }
    let frac = f - t as f64;
    if frac > 0.0 {
        Ordering::Less
    } else if frac < 0.0 {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// A hash that agrees with `equal` (P1, `unique`): equal values hash
/// alike. An int and the float of the same value hash as the int; a map
/// hashes its entries in any order; a shared object hashes its address.
/// A symbol hashes as the string of its name does; the two are not
/// equal, which a shared bucket does not change.
///
/// Walked with an explicit stack (L9). A map's entries are each hashed
/// on a hasher of their own and summed. A collection three levels deep
/// or more (`deep`) is hashed on a hasher of its own and adds that hash
/// to the one it is in, which waits in its frame; a shallower one (a
/// record of plain values and flat lists, the common case) goes straight
/// into the hasher it is in. The hash of a deep collection with more
/// than one holder is kept by its address, so a value that shares its
/// parts many times over is hashed once per part (VA44): what is not
/// kept is at most two levels of items below something that is. How deep
/// a value is does not depend on who else holds it, so equal values
/// still hash alike. A function has no equality, so no hash either:
/// E0501, as `==` gives (D62).
pub fn hash_value(v: &Value, h: &mut DefaultHasher) -> Result<(), Diag> {
    if !is_collection(v) {
        return hash_leaf(v, h);
    }
    // The common case, a record of plain values and flat lists, needs
    // none of what follows.
    if !deep(v) {
        return hash_shallow(v, h);
    }
    enum Items2<'a> {
        Seq(Items<'a>),
        Map {
            entries: super::MapIter<'a>,
            len: usize,
            sum: u64,
            /// An entry is being hashed on the current hasher.
            in_entry: bool,
            /// The map's own hasher, while an entry is hashed.
            own: DefaultHasher,
        },
    }
    struct Open<'a> {
        items: Items2<'a>,
        /// The hasher the collection's hash goes to; `None` for one
        /// hashed straight into the hasher it is in.
        outer: Option<DefaultHasher>,
        /// Where to keep the hash, for a shared collection.
        keep: Option<*const ()>,
    }
    let mut known: std::collections::HashMap<*const (), u64> = std::collections::HashMap::new();
    // As `Walked`: a small value is hashed without the memory.
    let mut steps = 0u32;
    let mut cur = std::mem::take(h);
    let mut open: Vec<Open<'_>> = Vec::new();
    let mut next = Some(v);
    loop {
        if let Some(v) = next.take() {
            let nested = deep(v);
            let keep = if steps < REMEMBER_AFTER {
                steps += 1;
                None
            } else if nested {
                shared(v)
            } else {
                None
            };
            if let Some(done) = keep.and_then(|p| known.get(&p)) {
                (13u8, *done).hash(&mut cur);
            } else if is_collection(v) {
                let outer = nested.then(|| std::mem::replace(&mut cur, DefaultHasher::new()));
                let items = match v {
                    Value::List(l) => {
                        (7u8, l.len()).hash(&mut cur);
                        Items2::Seq(Items::List(l.iter()))
                    }
                    Value::Tuple(t) => {
                        (8u8, t.len()).hash(&mut cur);
                        Items2::Seq(Items::Slice(t.iter()))
                    }
                    Value::Tag(t) => {
                        (9u8, t.name.name(), t.args.len()).hash(&mut cur);
                        Items2::Seq(Items::Slice(t.args.iter()))
                    }
                    Value::Map(m) => Items2::Map {
                        entries: m.iter(),
                        len: m.len(),
                        sum: 0,
                        in_entry: false,
                        own: std::mem::take(&mut cur),
                    },
                    _ => unreachable!("a collection"),
                };
                open.push(Open { items, outer, keep });
            } else {
                hash_leaf(v, &mut cur)?;
            }
        }
        let Some(top) = open.last_mut() else {
            *h = cur;
            return Ok(());
        };
        let done = match &mut top.items {
            Items2::Seq(items) => match items.next() {
                Some(x) => {
                    next = Some(x);
                    false
                }
                None => true,
            },
            Items2::Map {
                entries,
                len,
                sum,
                in_entry,
                own,
            } => {
                if std::mem::take(in_entry) {
                    *sum = sum.wrapping_add(std::mem::take(&mut cur).finish());
                }
                match entries.next() {
                    Some((k, x)) => {
                        cur = DefaultHasher::new();
                        k.hash(&mut cur);
                        *in_entry = true;
                        next = Some(x);
                        false
                    }
                    None => {
                        cur = std::mem::take(own);
                        (11u8, *len, *sum).hash(&mut cur);
                        true
                    }
                }
            }
        };
        if done {
            let top = open.pop().expect("an open collection");
            if let Some(outer) = top.outer {
                let hash = std::mem::replace(&mut cur, outer).finish();
                if let Some(p) = top.keep {
                    known.insert(p, hash);
                }
                (13u8, hash).hash(&mut cur);
            }
        }
    }
}

/// `hash_value` of a collection less than three levels deep: straight
/// into `h`, a map's entries each on a hasher of their own and summed
/// while the hasher the map belongs to waits in its frame.
fn hash_shallow(v: &Value, h: &mut DefaultHasher) -> Result<(), Diag> {
    enum Open<'a> {
        Seq(Items<'a>),
        Map {
            entries: super::MapIter<'a>,
            len: usize,
            sum: u64,
            /// The hasher the map's own hash goes to.
            outer: DefaultHasher,
            /// An entry is being hashed on the current hasher.
            in_entry: bool,
        },
    }
    let mut cur = std::mem::take(h);
    let mut open: Vec<Open<'_>> = Vec::new();
    let mut next = Some(v);
    loop {
        if let Some(v) = next.take() {
            match v {
                Value::List(l) => {
                    (7u8, l.len()).hash(&mut cur);
                    open.push(Open::Seq(Items::List(l.iter())));
                }
                Value::Tuple(t) => {
                    (8u8, t.len()).hash(&mut cur);
                    open.push(Open::Seq(Items::Slice(t.iter())));
                }
                Value::Tag(t) => {
                    (9u8, t.name.name(), t.args.len()).hash(&mut cur);
                    open.push(Open::Seq(Items::Slice(t.args.iter())));
                }
                Value::Map(m) => open.push(Open::Map {
                    entries: m.iter(),
                    len: m.len(),
                    sum: 0,
                    outer: std::mem::take(&mut cur),
                    in_entry: false,
                }),
                v => hash_leaf(v, &mut cur)?,
            }
        }
        let Some(top) = open.last_mut() else {
            *h = cur;
            return Ok(());
        };
        match top {
            Open::Seq(items) => match items.next() {
                Some(x) => next = Some(x),
                None => {
                    open.pop();
                }
            },
            Open::Map {
                entries,
                len,
                sum,
                outer,
                in_entry,
            } => {
                if std::mem::take(in_entry) {
                    *sum = sum.wrapping_add(std::mem::take(&mut cur).finish());
                }
                match entries.next() {
                    Some((k, x)) => {
                        cur = DefaultHasher::new();
                        k.hash(&mut cur);
                        *in_entry = true;
                        next = Some(x);
                    }
                    None => {
                        cur = std::mem::take(outer);
                        (11u8, *len, *sum).hash(&mut cur);
                        open.pop();
                    }
                }
            }
        }
    }
}

/// Whether `v` holds a collection that holds a collection.
fn deep(v: &Value) -> bool {
    fn any(v: &Value, test: fn(&Value) -> bool) -> bool {
        match v {
            Value::List(l) => l.iter().any(test),
            Value::Tuple(t) => t.iter().any(test),
            Value::Tag(t) => t.args.iter().any(test),
            Value::Map(m) => m.iter().any(|(_, x)| test(x)),
            _ => false,
        }
    }
    any(v, |x| any(x, is_collection))
}

/// `hash_value` of a value that holds no other.
fn hash_leaf(v: &Value, h: &mut DefaultHasher) -> Result<(), Diag> {
    if is_fn(v) {
        return check_fns(v, v);
    }
    match v {
        Value::Int(i) => (0u8, *i).hash(h),
        Value::Float(f) => {
            if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f < i64::MAX as f64 {
                (0u8, *f as i64).hash(h);
            } else {
                (1u8, f.to_bits()).hash(h);
            }
        }
        Value::Bool(b) => (2u8, *b).hash(h),
        Value::Nil => 3u8.hash(h),
        Value::Str(s) => (4u8, s.as_bytes()).hash(h),
        Value::Sym(s) => (4u8, s.name().as_bytes()).hash(h),
        Value::Dur(d) => (5u8, *d).hash(h),
        Value::Time(t) => (6u8, t.instant()).hash(h),
        // By the numbers it holds, as `==` compares (D19).
        Value::Range(r) => (10u8, r.ends()).hash(h),
        Value::Closure(_) | Value::Native(_) => unreachable!("a function has no hash"),
        Value::Module(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::Task(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::Scope(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::Atom(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::Sender(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::Receiver(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::File(x) => (12u8, std::sync::Arc::as_ptr(x) as *const u8 as usize).hash(h),
        Value::Conn(x) => (12u8, std::sync::Arc::as_ptr(x) as *const u8 as usize).hash(h),
        Value::Listener(x) => (12u8, std::sync::Arc::as_ptr(x) as *const u8 as usize).hash(h),
        Value::List(_) | Value::Tuple(_) | Value::Tag(_) | Value::Map(_) => {
            unreachable!("collections are walked by `hash_value`")
        }
    }
    Ok(())
}
