//! The core collection functions: the ones that walk elements natively.
//! They iterate lists, ranges and channels (02: 순회 대상); strings and
//! maps become lists with `chars` and `pairs` first.

use std::collections::HashMap;
use std::rc::Rc;

use fung_front::diag::Arg;

use crate::native::nat::{self, def, def_step, fail, opt, param};
use crate::native::walk::{Collect, Cursor, PerItem, drain_with, walk};
use crate::value::{
    List, ListBuilder, Lit, MapBuilder, Text, Value, compare, eq, hash_value, type_name,
};
use crate::vm::pieces::{self, STEP_COMPARES, STEP_COPIES, STEP_ITEMS, Work};
use crate::vm::{Flow, Stop, Task, discarded_err};

pub(super) fn add(add: &mut impl FnMut(Value)) {
    add(def_step("filter", filter_fn, vec![param("xs"), param("f")]));
    add(def_step(
        "fold",
        fold_fn,
        vec![param("xs"), param("init"), param("f")],
    ));
    add(def_step("each", each_fn, vec![param("xs"), param("f")]));
    add(def_step("sort", sort_fn, vec![param("xs")]));
    add(def_step("unique", unique_fn, vec![param("xs")]));
    add(def_step(
        "sort_by",
        sort_by_fn,
        vec![param("xs"), param("f")],
    ));
    add(def_step("find", find_fn, vec![param("xs"), param("f")]));
    add(def_step("any", any_fn, vec![param("xs"), param("f")]));
    add(def_step("all", all_fn, vec![param("xs"), param("f")]));
    add(def_step("flatten", flatten_fn, vec![param("xs")]));
    add(def_step(
        "group_by",
        group_by_fn,
        vec![param("xs"), param("f")],
    ));
    add(def_step("pairs", pairs_fn, vec![param("m")]));
    add(def(
        "get",
        get_fn,
        vec![param("m"), param("k"), opt("default", Lit::Nil)],
    ));
    add(def("put", put_fn, vec![param("m"), param("k"), param("v")]));
    add(def_step("merge", merge_fn, vec![param("a"), param("b")]));
    add(def("remove", remove_fn, vec![param("m"), param("k")]));
    add(def_step("chars", chars_fn, vec![param("s")]));
}

/// The collection `a[0]`, then the function `a[f_at]`, in argument order.
fn need_fn_after(a: &[Value], f_at: usize, verb: &str) -> Result<(), Stop> {
    nat::check_items(&a[0], verb)?;
    nat::need_fn("f", &a[f_at])
}

fn filter_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    need_fn_after(&a, 1, "filter")?;
    struct Filter {
        x: Option<Value>,
        out: ListBuilder,
    }
    impl PerItem for Filter {
        fn args(&mut self, t: &mut Task<'_>, x: Value) -> usize {
            t.push_arg(x.clone());
            self.x = Some(x);
            1
        }
        fn got(&mut self, r: Value) -> Result<bool, Stop> {
            if nat::want_bool(r)? {
                self.out.push(self.x.take().expect("the element"));
            }
            Ok(false)
        }
        fn result(&mut self, _: bool) -> Result<Value, Stop> {
            Ok(Value::List(Rc::new(std::mem::take(&mut self.out).finish())))
        }
    }
    let p = Filter {
        x: None,
        out: ListBuilder::default(),
    };
    walk(t, Cursor::new(&a[0]), a[1].clone(), p)
}

fn fold_fn(t: &mut Task<'_>, mut a: Vec<Value>) -> Result<Flow, Stop> {
    need_fn_after(&a, 2, "fold")?;
    /// The accumulator moves into each call and its result moves back:
    /// nothing is copied per element.
    struct Fold(Option<Value>);
    impl PerItem for Fold {
        fn args(&mut self, t: &mut Task<'_>, x: Value) -> usize {
            t.push_arg(self.0.take().expect("the accumulator"));
            t.push_arg(x);
            2
        }
        fn got(&mut self, r: Value) -> Result<bool, Stop> {
            self.0 = Some(r);
            Ok(false)
        }
        fn result(&mut self, _: bool) -> Result<Value, Stop> {
            Ok(self.0.take().expect("the accumulator"))
        }
    }
    let f = a.pop().expect("f");
    let init = a.pop().expect("init");
    walk(t, Cursor::new(&a[0]), f, Fold(Some(init)))
}

fn each_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    need_fn_after(&a, 1, "iterate over")?;
    struct Each;
    impl PerItem for Each {
        fn args(&mut self, t: &mut Task<'_>, x: Value) -> usize {
            t.push_arg(x);
            1
        }
        /// What `f` returns is discarded: an Err is E0606 (02).
        fn got(&mut self, r: Value) -> Result<bool, Stop> {
            match discarded_err(&r) {
                Some(d) => Err(Stop::Error(Box::new(d))),
                None => Ok(false),
            }
        }
        fn result(&mut self, _: bool) -> Result<Value, Stop> {
            Ok(Value::Nil)
        }
    }
    walk(t, Cursor::new(&a[0]), a[1].clone(), Each)
}

/// `xs` ordered by `keys` (02: stable, by `<`): a merge sort on indices.
/// Comparisons can fail (`1 < "a"`), and the first failure ends the sort
/// with its error. (The standard library's sort is not used: it may panic
/// when the order it is given is not total, and it cannot stop midway.)
///
/// It is bottom-up so that it can stop after any comparison and go on
/// later (L6, `vm/pieces.rs`): runs of `width` are merged pairwise from
/// `src` into `dst`, then the two swap and `width` doubles. The left run
/// wins ties, so equal keys keep their order.
struct Sort {
    xs: Vec<Value>,
    /// The keys, or none when the items are their own keys (`sort`).
    keys: Option<Vec<Value>>,
    src: Vec<usize>,
    dst: Vec<usize>,
    width: usize,
    /// The pair being merged starts at `lo`; its runs are read at `i` and
    /// `j` and written at `k`.
    lo: usize,
    i: usize,
    j: usize,
    k: usize,
    /// Once sorted: the result so far, grown in place (a builder would
    /// make its vector in one go at the end).
    out: Option<List>,
}

impl Sort {
    fn new(xs: Vec<Value>, keys: Option<Vec<Value>>) -> Sort {
        let n = xs.len();
        Sort {
            xs,
            keys,
            src: (0..n).collect(),
            dst: vec![0; n],
            width: 1,
            lo: 0,
            i: 0,
            j: 1.min(n),
            k: 0,
            out: None,
        }
    }
}

impl Work for Sort {
    fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
        let n = self.src.len();
        let mut budget = STEP_COMPARES;
        let keys = self.keys.as_deref().unwrap_or(&self.xs);
        while self.width < n {
            let mid = (self.lo + self.width).min(n);
            let hi = (self.lo + 2 * self.width).min(n);
            // the merge on locals: the state goes back to `self` at a pause
            let (src, dst) = (&self.src[..], &mut self.dst[..]);
            let (mut i, mut j, mut k) = (self.i, self.j, self.k);
            while i < mid && j < hi {
                if budget == 0 {
                    (self.i, self.j, self.k) = (i, j, k);
                    return Ok(None);
                }
                budget -= 1;
                if compare(&keys[src[j]], &keys[src[i]])?.is_lt() {
                    dst[k] = src[j];
                    j += 1;
                } else {
                    dst[k] = src[i];
                    i += 1;
                }
                k += 1;
            }
            // one run is used up: the rest of the other follows as it is
            let rest = if i < mid { &src[i..mid] } else { &src[j..hi] };
            dst[k..hi].copy_from_slice(rest);
            self.lo = hi;
            if self.lo >= n {
                std::mem::swap(&mut self.src, &mut self.dst);
                self.width *= 2;
                self.lo = 0;
            }
            self.i = self.lo;
            self.j = (self.lo + self.width).min(n);
            self.k = self.lo;
        }
        // The result, a piece at a time too, on what is left of this
        // piece's budget: an item costs about two comparisons.
        let out = self.out.get_or_insert_with(|| List::new(Vec::new()));
        let from = out.len();
        for &i in self.src.iter().skip(from).take(budget / 2) {
            out.push(self.xs[i].clone());
        }
        if out.len() < n {
            return Ok(None);
        }
        let out = self.out.take().expect("built");
        Ok(Some(Value::List(Rc::new(out))))
    }
}

/// The elements of `v` handed to `then` all at once: at once for a range,
/// a piece at a time for a long list (`pieces::with_items`), as they
/// arrive for a receiving end (the walk parks).
fn all_items(
    t: &mut Task<'_>,
    v: &Value,
    then: impl FnMut(&mut Task<'_>, Vec<Value>) -> Result<Flow, Stop> + 'static,
) -> Result<Flow, Stop> {
    struct Gather<F> {
        xs: Vec<Value>,
        then: F,
    }
    impl<F: FnMut(&mut Task<'_>, Vec<Value>) -> Result<Flow, Stop>> Collect for Gather<F> {
        fn item(&mut self, x: Value) -> Result<bool, Stop> {
            self.xs.push(x);
            Ok(false)
        }
        fn result(&mut self, t: &mut Task<'_>) -> Result<Flow, Stop> {
            (self.then)(t, std::mem::take(&mut self.xs))
        }
    }
    let mut then = then;
    match v {
        Value::Receiver(_) => drain_with(
            t,
            Cursor::new(v),
            Gather {
                xs: Vec::new(),
                then,
            },
        ),
        Value::List(l) => pieces::with_items(t, l, then),
        _ => then(t, nat::items(v, "sort")?),
    }
}

fn sort_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "sort")?;
    all_items(t, &a[0], |t, xs| pieces::run(t, Sort::new(xs, None)))
}

/// `unique(xs)`: the elements without the later ones `==` an earlier one,
/// in order (02). Hashing by `hash_value` makes it linear (P1); elements
/// in one bucket are compared with `eq`, so a symbol meeting a string is
/// E0517 as with `in`.
fn unique_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "unique")?;
    /// The kept elements, and for each hash the first kept element with
    /// it, the others chained through `next`: one table and two vectors,
    /// not a vector per hash, so freeing them is quick too (L6).
    struct Unique {
        xs: std::vec::IntoIter<Value>,
        first: HashMap<u64, usize>,
        next: Vec<usize>,
        /// The kept elements, and the same grown as the result.
        out: Vec<Value>,
        list: List,
    }
    impl Work for Unique {
        fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
            for x in self.xs.by_ref().take(STEP_ITEMS) {
                let mut h = std::hash::DefaultHasher::new();
                hash_value(&x, &mut h);
                let h = std::hash::Hasher::finish(&h);
                let mut at = self.first.get(&h).copied().unwrap_or(usize::MAX);
                let mut seen = false;
                while at != usize::MAX {
                    if eq(&self.out[at], &x)? {
                        seen = true;
                        break;
                    }
                    at = self.next[at];
                }
                if !seen {
                    let i = self.out.len();
                    self.next
                        .push(self.first.insert(h, i).unwrap_or(usize::MAX));
                    self.list.push(x.clone());
                    self.out.push(x);
                }
            }
            if self.xs.len() > 0 {
                return Ok(None);
            }
            let list = std::mem::replace(&mut self.list, List::new(Vec::new()));
            Ok(Some(Value::List(Rc::new(list))))
        }
    }
    all_items(t, &a[0], |t, xs| {
        pieces::run(
            t,
            // Sized up front: a table that grows rehashes all it holds at
            // once, which would undo the pieces (L6).
            Unique {
                first: HashMap::with_capacity(xs.len()),
                next: Vec::with_capacity(xs.len()),
                out: Vec::with_capacity(xs.len()),
                list: List::new(Vec::new()),
                xs: xs.into_iter(),
            },
        )
    })
}

fn sort_by_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "sort")?;
    nat::need_fn("f", &a[1])?;
    struct Keys {
        xs: Vec<Value>,
        keys: Vec<Value>,
    }
    impl PerItem for Keys {
        fn args(&mut self, t: &mut Task<'_>, x: Value) -> usize {
            t.push_arg(x);
            1
        }
        fn got(&mut self, r: Value) -> Result<bool, Stop> {
            self.keys.push(r);
            Ok(false)
        }
        fn result(&mut self, _: bool) -> Result<Value, Stop> {
            unreachable!("sort_by ends with `finish`")
        }
        fn finish(&mut self, t: &mut Task<'_>, _: bool) -> Result<Flow, Stop> {
            let xs = std::mem::take(&mut self.xs);
            let keys = std::mem::take(&mut self.keys);
            pieces::run(t, Sort::new(xs, Some(keys)))
        }
    }
    let f = a[1].clone();
    all_items(t, &a[0], move |t, xs| {
        let p = Keys {
            keys: Vec::with_capacity(xs.len()),
            xs: xs.clone(),
        };
        walk(t, Cursor::Items(xs, 0), f.clone(), p)
    })
}

fn find_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "search")?;
    nat::need_fn("f", &a[1])?;
    struct Find(Option<Value>);
    impl PerItem for Find {
        fn args(&mut self, t: &mut Task<'_>, x: Value) -> usize {
            t.push_arg(x.clone());
            self.0 = Some(x);
            1
        }
        fn got(&mut self, r: Value) -> Result<bool, Stop> {
            nat::want_bool(r)
        }
        fn result(&mut self, stopped: bool) -> Result<Value, Stop> {
            Ok(match stopped {
                true => self.0.take().expect("the element"),
                false => Value::Nil,
            })
        }
    }
    walk(t, Cursor::new(&a[0]), a[1].clone(), Find(None))
}

/// `any` (stop = true) and `all` (stop = false): stops at the first
/// element whose predicate result equals `stop`.
fn quantify(t: &mut Task<'_>, a: &mut [Value], stop: bool) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "test")?;
    nat::need_fn("f", &a[1])?;
    struct Quantify(bool);
    impl PerItem for Quantify {
        fn args(&mut self, t: &mut Task<'_>, x: Value) -> usize {
            t.push_arg(x);
            1
        }
        fn got(&mut self, r: Value) -> Result<bool, Stop> {
            Ok(nat::want_bool(r)? == self.0)
        }
        fn result(&mut self, stopped: bool) -> Result<Value, Stop> {
            Ok(Value::Bool(stopped == self.0))
        }
    }
    walk(t, Cursor::new(&a[0]), a[1].clone(), Quantify(stop))
}

fn any_fn(t: &mut Task<'_>, mut a: Vec<Value>) -> Result<Flow, Stop> {
    quantify(t, &mut a, true)
}

fn all_fn(t: &mut Task<'_>, mut a: Vec<Value>) -> Result<Flow, Stop> {
    quantify(t, &mut a, false)
}

/// `flatten(xs)` splices nested lists one level deep. It stays in Rust:
/// a fung fold with `acc + x` would copy the accumulator per element.
fn flatten_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "flatten")?;
    nat::limit_elements(&a[0])?;
    struct Flatten(ListBuilder);
    impl Collect for Flatten {
        fn item(&mut self, x: Value) -> Result<bool, Stop> {
            match &x {
                Value::List(inner) => inner.iter().for_each(|v| self.0.push(v.clone())),
                _ => self.0.push(x),
            }
            Ok(false)
        }
        fn result(&mut self, _: &mut Task<'_>) -> Result<Flow, Stop> {
            let out = std::mem::take(&mut self.0).finish();
            Ok(Flow::Done(Value::List(Rc::new(out))))
        }
    }
    drain_with(t, Cursor::new(&a[0]), Flatten(ListBuilder::default()))
}

fn group_by_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "group")?;
    nat::need_fn("f", &a[1])?;
    nat::limit_elements(&a[0])?;
    /// Keys in first-seen order, each group in input order. The hash map
    /// only finds a key's group; the order comes from `groups`.
    #[derive(Default)]
    struct Group {
        x: Option<Value>,
        groups: Vec<(Rc<Text>, ListBuilder)>,
        at: HashMap<Rc<Text>, usize>,
    }
    impl PerItem for Group {
        fn args(&mut self, t: &mut Task<'_>, x: Value) -> usize {
            t.push_arg(x.clone());
            self.x = Some(x);
            1
        }
        fn got(&mut self, k: Value) -> Result<bool, Stop> {
            let Value::Str(key) = k else {
                return Err(fail("E0514", vec![Arg::Str(type_name(&k))]));
            };
            let groups = &mut self.groups;
            let i = *self.at.entry(key.clone()).or_insert_with(|| {
                groups.push((key, ListBuilder::default()));
                groups.len() - 1
            });
            groups[i].1.push(self.x.take().expect("the element"));
            Ok(false)
        }
        fn result(&mut self, _: bool) -> Result<Value, Stop> {
            unreachable!("group_by's result is built by `finish`")
        }
        /// The result map is built a piece at a time too (VA10): each
        /// group's list is made of its items, and the map of the groups.
        fn finish(&mut self, t: &mut Task<'_>, _: bool) -> Result<Flow, Stop> {
            struct Build {
                groups: std::vec::IntoIter<(Rc<Text>, ListBuilder)>,
                out: MapBuilder,
            }
            impl Work for Build {
                fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
                    let mut done = 0;
                    while done < STEP_COPIES {
                        let Some((k, items)) = self.groups.next() else {
                            let out = std::mem::take(&mut self.out).finish();
                            return Ok(Some(Value::Map(Rc::new(out))));
                        };
                        done += 1 + items.len();
                        self.out.set(k, Value::List(Rc::new(items.finish())));
                    }
                    Ok(None)
                }
            }
            let groups = std::mem::take(&mut self.groups).into_iter();
            pieces::run(
                t,
                Build {
                    groups,
                    out: MapBuilder::default(),
                },
            )
        }
    }
    walk(t, Cursor::new(&a[0]), a[1].clone(), Group::default())
}

/// `pairs(m)`, a piece at a time (VA10): the list grows in place, and
/// the map is read by a cursor that holds no borrow.
fn pairs_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    nat::need_map("list the pairs of", &a[0])?;
    struct Pairs {
        m: Value,
        at: usize,
        out: List,
    }
    impl Work for Pairs {
        fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
            let Value::Map(m) = &self.m else {
                unreachable!("checked by need_map")
            };
            for _ in 0..STEP_ITEMS {
                let Some((i, k, v)) = m.entry_from(self.at) else {
                    let out = std::mem::replace(&mut self.out, List::new(Vec::new()));
                    return Ok(Some(Value::List(Rc::new(out))));
                };
                self.out
                    .push(Value::tuple(vec![Value::Str(k.clone()), v.clone()]));
                self.at = i + 1;
            }
            Ok(None)
        }
    }
    pieces::run(
        t,
        Pairs {
            m: a[0].clone(),
            at: 0,
            out: List::new(Vec::new()),
        },
    )
}

/// The map argument by move: changed in place when nothing else holds it.
fn take_map(a: &mut [Value], i: usize) -> Rc<crate::value::Map> {
    match std::mem::replace(&mut a[i], Value::Nil) {
        Value::Map(m) => m,
        _ => unreachable!("checked by need_map"),
    }
}

/// `get(m, k, default = nil)`: the value at `k`, or `default` when the
/// key is missing. A key that is not a string fails like `k in m`.
fn get_fn(_: &mut Task<'_>, mut a: Vec<Value>) -> Result<Value, Stop> {
    let m = nat::need_map("get from", &a[0])?;
    let Value::Str(k) = &a[1] else {
        return Err(fail(
            "E0501",
            vec![
                Arg::Str("search".into()),
                Arg::Str("map".into()),
                Arg::Str(type_name(&a[1])),
            ],
        ));
    };
    match m.get(&***k) {
        Some(v) => Ok(v.clone()),
        None => Ok(a.pop().expect("default")),
    }
}

fn put_fn(_: &mut Task<'_>, mut a: Vec<Value>) -> Result<Value, Stop> {
    nat::need_map("put into", &a[0])?;
    let k = nat::need_key("index", &a[1])?.clone();
    let v = a.pop().expect("v");
    let mut m = take_map(&mut a, 0);
    // Copies the map first when it is shared (`make_mut`): the original
    // never changes.
    Rc::make_mut(&mut m).put(k, v);
    Ok(Value::Map(m))
}

/// `merge(a, b)`: `b`'s entries put into `a` in `b`'s order, a piece at
/// a time (VA10). `a` is changed in place when nothing else holds it.
fn merge_fn(t: &mut Task<'_>, mut a: Vec<Value>) -> Result<Flow, Stop> {
    nat::need_map("merge", &a[0])?;
    nat::need_map("merge", &a[1])?;
    struct Merge {
        out: crate::value::Map,
        b: Value,
        at: usize,
    }
    impl Work for Merge {
        fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
            let Value::Map(b) = &self.b else {
                unreachable!("checked by need_map")
            };
            for _ in 0..STEP_ITEMS {
                let Some((i, k, v)) = b.entry_from(self.at) else {
                    let out = std::mem::take(&mut self.out);
                    return Ok(Some(Value::Map(Rc::new(out))));
                };
                self.out.put(k.clone(), v.clone());
                self.at = i + 1;
            }
            Ok(None)
        }
    }
    let b = a.pop().expect("b");
    let out = Rc::try_unwrap(take_map(&mut a, 0)).unwrap_or_else(|m| (*m).clone());
    pieces::run(t, Merge { out, b, at: 0 })
}

fn remove_fn(_: &mut Task<'_>, mut a: Vec<Value>) -> Result<Value, Stop> {
    nat::need_map("remove from", &a[0])?;
    let k = nat::need_key("index", &a[1])?.clone();
    let mut m = take_map(&mut a, 0);
    if m.get(&*k).is_some() {
        // A version something else holds is copied, and its copy keeps
        // its removed entries longer (VA1).
        if Rc::strong_count(&m) == 1 {
            Rc::make_mut(&mut m).remove(&*k);
        } else {
            Rc::make_mut(&mut m).remove_shared(&*k);
        }
    }
    Ok(Value::Map(m))
}

fn chars_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    if !matches!(a[0], Value::Str(_)) {
        return Err(fail(
            "E0506",
            vec![nat::s("list the characters of"), Arg::Str(type_name(&a[0]))],
        ));
    }
    super::strings::chars_of(t, a[0].clone())
}
