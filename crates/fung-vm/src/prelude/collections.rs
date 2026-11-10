//! The core collection functions: the ones that walk elements natively.
//! They iterate lists, ranges and channels (02: 순회 대상); strings and
//! maps become lists with `chars` and `pairs` first. The ones that go
//! over every element without a callback (`reverse`, `sum`, `min`, `max`,
//! `keys`, `values`) live here too, not in `extras/prelude.fung`: a fung
//! loop pays a call per element (VA25), and an error then names the
//! function the user called (VA19).

use std::collections::HashMap;
use std::rc::Rc;

use crate::native::nat::{self, def, def_step, fail, opt, param};
use crate::native::walk::{Collect, Cursor, PerItem, drain_with, walk};
use crate::value::{
    Args, List, ListBuilder, Lit, MapBuilder, Range, Text, Value, compare, eq, hash_value,
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
    add(def_step("reverse", reverse_fn, vec![param("xs")]));
    add(def_step("sum", sum_fn, vec![param("xs")]));
    add(def_step("min", min_fn, vec![param("xs")]));
    add(def_step("max", max_fn, vec![param("xs")]));
    add(def_step("pairs", pairs_fn, vec![param("m")]));
    add(def_step("keys", keys_fn, vec![param("m")]));
    add(def_step("values", values_fn, vec![param("m")]));
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

fn filter_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
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

fn fold_fn(t: &mut Task<'_>, mut a: Args) -> Result<Flow, Stop> {
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

fn each_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
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

/// `xs` ordered by `keys` (02: stable, by `<`): a natural merge sort on
/// indices. Comparisons can fail (`1 < "a"`), and the first failure ends
/// the sort with its error. (The standard library's sort is not used: it
/// may panic when the order it is given is not total, and it cannot stop
/// midway.)
///
/// It can stop after any comparison and go on later (L6, `vm/pieces.rs`).
/// First the input is cut into runs (VA23): an ascending run goes on while
/// each item is not smaller than the one before, a strictly descending run
/// is reversed (strict, so equal keys keep their order), and a run shorter
/// than `MIN_RUN` is grown to that length by binary insertion (after its
/// equal keys). Sorted input is then one run and costs n - 1 comparisons.
/// Then runs are merged pairwise from `src` into `dst`, the two swap, and
/// this repeats until one run is left. The left run wins ties, so equal
/// keys keep their order.
pub(super) struct Sort {
    /// Shared with `sort_by`'s walk, which reads the items from it too.
    xs: Rc<Vec<Value>>,
    /// The keys, or none when the items are their own keys (`sort`).
    keys: Option<Vec<Value>>,
    src: Vec<usize>,
    dst: Vec<usize>,
    /// Finding runs: the run being found starts at `pos`, `scan` is the
    /// next index to compare with its predecessor, `desc` whether the
    /// run turned out descending (none until the first comparison), and
    /// `grow` the next index to insert once the run is being grown.
    pos: usize,
    scan: usize,
    desc: Option<bool>,
    grow: Option<usize>,
    /// The end of each run found (this pass's, then the pass being built).
    runs: Vec<usize>,
    next_runs: Vec<usize>,
    /// Merging: the pair being merged is the runs `r` and `r + 1`; its
    /// runs are read at `i` and `j` and written at `k`; `merging` is
    /// whether the pair has been started.
    r: usize,
    merging: bool,
    i: usize,
    j: usize,
    k: usize,
    /// Once sorted: the result so far, grown in place (a builder would
    /// make its vector in one go at the end).
    out: Option<List>,
}

/// The shortest run the merging starts from.
const MIN_RUN: usize = 32;

impl Sort {
    pub(super) fn new(xs: Rc<Vec<Value>>, keys: Option<Vec<Value>>) -> Sort {
        let n = xs.len();
        Sort {
            xs,
            keys,
            src: (0..n).collect(),
            dst: vec![0; n],
            pos: 0,
            scan: 1,
            desc: None,
            grow: None,
            runs: Vec::new(),
            next_runs: Vec::new(),
            r: 0,
            merging: false,
            i: 0,
            j: 0,
            k: 0,
            out: None,
        }
    }
}

impl Work for Sort {
    fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
        self.run()
    }
}

impl Sort {
    /// One piece of the sort: `None` when the budget ran out first.
    pub(super) fn run(&mut self) -> Result<Option<Value>, Stop> {
        let n = self.src.len();
        let mut budget = STEP_COMPARES;
        let keys = self.keys.as_deref().unwrap_or(&self.xs[..]);
        // A lone key is never compared with another: it must still have
        // an order, as `x < x` asks (VA18, D16).
        if let [k] = keys {
            compare(k, k)?;
        }
        // finding the runs
        while self.pos < n {
            if self.grow.is_none() {
                while self.scan < n {
                    if budget == 0 {
                        return Ok(None);
                    }
                    budget -= 1;
                    let lt = compare(&keys[self.src[self.scan]], &keys[self.src[self.scan - 1]])?
                        .is_lt();
                    match self.desc {
                        None => self.desc = Some(lt),
                        Some(d) if d == lt => {}
                        Some(_) => break,
                    }
                    self.scan += 1;
                }
                if self.desc == Some(true) {
                    self.src[self.pos..self.scan].reverse();
                }
                self.grow = Some(self.scan);
            }
            // grow a short run, one binary insertion at a time
            let end = (self.pos + MIN_RUN).min(n);
            let mut at = self.grow.expect("set above");
            while at < end {
                // at most about log2(MIN_RUN) comparisons
                if budget < 8 {
                    self.grow = Some(at);
                    return Ok(None);
                }
                let x = self.src[at];
                let (mut lo, mut hi) = (self.pos, at);
                while lo < hi {
                    let mid = lo + (hi - lo) / 2;
                    budget -= 1;
                    if compare(&keys[x], &keys[self.src[mid]])?.is_lt() {
                        hi = mid;
                    } else {
                        lo = mid + 1;
                    }
                }
                self.src.copy_within(lo..at, lo + 1);
                self.src[lo] = x;
                at += 1;
            }
            let stop = at.max(self.scan);
            self.runs.push(stop);
            self.pos = stop;
            self.scan = stop + 1;
            self.desc = None;
            self.grow = None;
        }
        // merging passes
        while self.runs.len() > 1 {
            let last = self.runs.len();
            while self.r < last {
                let lo = if self.r == 0 {
                    0
                } else {
                    self.runs[self.r - 1]
                };
                let mid = self.runs[self.r];
                // an odd run out goes through as it is
                let hi = self.runs.get(self.r + 1).copied().unwrap_or(mid);
                if !self.merging {
                    self.i = lo;
                    self.j = mid;
                    self.k = lo;
                    self.merging = true;
                }
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
                self.next_runs.push(hi);
                self.merging = false;
                self.r += 2;
            }
            std::mem::swap(&mut self.src, &mut self.dst);
            self.runs = std::mem::take(&mut self.next_runs);
            self.r = 0;
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

fn sort_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "sort")?;
    all_items(t, &a[0], |t, xs| {
        pieces::run(t, Sort::new(Rc::new(xs), None))
    })
}

/// `unique(xs)`: the elements without the later ones `==` an earlier one,
/// in order (02). Hashing by `hash_value` makes it linear (P1); elements
/// in one bucket are compared with `eq`, so a symbol meeting a string is
/// E0517 as with `in`.
fn unique_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "take the unique elements of")?;
    /// The kept elements, and for each hash the first kept element with
    /// it, the others chained through `next`: one table and two vectors,
    /// not a vector per hash, so freeing them is quick too (L6).
    struct Unique {
        xs: std::vec::IntoIter<Value>,
        first: HashMap<u64, usize>,
        next: Vec<usize>,
        /// The kept elements, grown as the result (and read back for the
        /// comparisons: one copy of them, VA24).
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
                    if eq(self.list.at(at).expect("kept"), &x)? {
                        seen = true;
                        break;
                    }
                    at = self.next[at];
                }
                if !seen {
                    let i = self.list.len();
                    self.next
                        .push(self.first.insert(h, i).unwrap_or(usize::MAX));
                    self.list.push(x);
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
                list: List::new(Vec::new()),
                xs: xs.into_iter(),
            },
        )
    })
}

fn sort_by_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "sort")?;
    nat::need_fn("f", &a[1])?;
    struct Keys {
        xs: Rc<Vec<Value>>,
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
            let xs = Rc::clone(&self.xs);
            let keys = std::mem::take(&mut self.keys);
            pieces::run(t, Sort::new(xs, Some(keys)))
        }
    }
    let f = a[1].clone();
    all_items(t, &a[0], move |t, xs| {
        let xs = Rc::new(xs);
        let p = Keys {
            keys: Vec::with_capacity(xs.len()),
            xs: Rc::clone(&xs),
        };
        walk(t, Cursor::Items(xs, 0), f.clone(), p)
    })
}

fn find_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
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

fn any_fn(t: &mut Task<'_>, mut a: Args) -> Result<Flow, Stop> {
    quantify(t, &mut a, true)
}

fn all_fn(t: &mut Task<'_>, mut a: Args) -> Result<Flow, Stop> {
    quantify(t, &mut a, false)
}

/// `flatten(xs)` splices nested lists one level deep. It stays in Rust:
/// a fung fold with `acc + x` would copy the accumulator per element.
fn flatten_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
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

fn group_by_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
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
                return Err(Stop::Error(Box::new(crate::value::index::not_a_key(&k))));
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

/// What `pairs`, `keys` and `values` make of a map's entry.
#[derive(Clone, Copy)]
enum Entries {
    Pairs,
    Keys,
    Values,
}

/// `pairs(m)`, `keys(m)`, `values(m)`, a piece at a time (VA10): the
/// list grows in place, and the map is read by a cursor that holds no
/// borrow.
fn entries(t: &mut Task<'_>, a: &[Value], verb: &str, what: Entries) -> Result<Flow, Stop> {
    nat::need_map(verb, &a[0])?;
    struct Walk {
        m: Value,
        what: Entries,
        at: usize,
        out: List,
    }
    impl Work for Walk {
        fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
            let Value::Map(m) = &self.m else {
                unreachable!("checked by need_map")
            };
            for _ in 0..STEP_ITEMS {
                let Some((i, k, v)) = m.entry_from(self.at) else {
                    let out = std::mem::replace(&mut self.out, List::new(Vec::new()));
                    return Ok(Some(Value::List(Rc::new(out))));
                };
                self.out.push(match self.what {
                    Entries::Pairs => Value::tuple(vec![Value::Str(k.clone()), v.clone()]),
                    Entries::Keys => Value::Str(k.clone()),
                    Entries::Values => v.clone(),
                });
                self.at = i + 1;
            }
            Ok(None)
        }
    }
    pieces::run(
        t,
        Walk {
            m: a[0].clone(),
            what,
            at: 0,
            out: List::new(Vec::new()),
        },
    )
}

fn pairs_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    entries(t, &a, "list the pairs of", Entries::Pairs)
}

fn keys_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    entries(t, &a, "list the keys of", Entries::Keys)
}

fn values_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    entries(t, &a, "list the values of", Entries::Values)
}

/// `reverse(xs)`: the elements last first, built a piece at a time.
fn reverse_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "reverse")?;
    nat::limit_elements(&a[0])?;
    /// What is left to take, from the end: the items, or a range's
    /// numbers (made as they are taken, never all at once).
    enum Left {
        Items(Vec<Value>),
        Ints { start: i64, left: usize },
    }
    struct Reverse {
        left: Left,
        out: List,
    }
    impl Work for Reverse {
        fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
            for _ in 0..STEP_COPIES {
                let x = match &mut self.left {
                    Left::Items(xs) => xs.pop(),
                    Left::Ints { left: 0, .. } => None,
                    Left::Ints { start, left } => {
                        *left -= 1;
                        Some(Value::Int(start.wrapping_add(*left as i64)))
                    }
                };
                let Some(x) = x else {
                    let out = std::mem::replace(&mut self.out, List::new(Vec::new()));
                    return Ok(Some(Value::List(Rc::new(out))));
                };
                self.out.push(x);
            }
            Ok(None)
        }
    }
    let reverse = |left| Reverse {
        left,
        out: List::new(Vec::new()),
    };
    if let Value::Range(r) = &a[0] {
        // within MAX_ELEMENTS (checked above), so it fits a usize
        let left = Left::Ints {
            start: r.start,
            left: r.len() as usize,
        };
        return pieces::run(t, reverse(left));
    }
    all_items(t, &a[0], move |t, xs| {
        pieces::run(t, reverse(Left::Items(xs)))
    })
}

/// `sum(xs)`: 0 when empty, else the elements joined by `+` from the
/// first, so durations, strings and lists sum too. A lone element is
/// checked as `+` would check it (VA18, D16): `sum([nil])` is not nil.
fn sum_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "sum")?;
    if let Value::Range(r) = &a[0] {
        return Ok(Flow::Done(range_sum(r)?));
    }
    struct Sum {
        acc: Option<Value>,
        lone: bool,
    }
    impl Collect for Sum {
        fn item(&mut self, x: Value) -> Result<bool, Stop> {
            self.acc = Some(match self.acc.take() {
                None => x,
                Some(acc) => {
                    self.lone = false;
                    crate::value::add(&acc, &x)?
                }
            });
            Ok(false)
        }
        fn result(&mut self, _: &mut Task<'_>) -> Result<Flow, Stop> {
            let Some(x) = self.acc.take() else {
                return Ok(Flow::Done(Value::Int(0)));
            };
            if self.lone
                && !matches!(
                    x,
                    Value::Int(_)
                        | Value::Float(_)
                        | Value::Str(_)
                        | Value::List(_)
                        | Value::Dur(_)
                )
            {
                // the error `x + x` gives (no other value adds to itself)
                crate::value::add(&x, &x)?;
            }
            Ok(Flow::Done(x))
        }
    }
    let p = Sum {
        acc: None,
        lone: true,
    };
    drain_with(t, Cursor::new(&a[0]), p)
}

/// A range's sum without walking it, the same as `+` folded from its
/// first number, overflow included (`sum` of a range is the sum of the
/// list of its numbers). The partial sums after k numbers are
/// `P(k) = k*a + k*(k-1)/2`: convex in k, so the largest is at an end
/// (`P(1) = a` fits; `P(n)` is the total) and the smallest is at the
/// vertex `k = 1/2 - a`, checked at its integer neighbours within 1..=n.
/// Any of them outside an int is the fold's E0503. In i128, which holds
/// every partial sum near the vertex; past it the value is far past an
/// int anyway.
fn range_sum(r: &Range) -> Result<Value, Stop> {
    let n = r.len();
    if n <= 0 {
        return Ok(Value::Int(0));
    }
    let a = i128::from(r.start);
    let partial = |k: i128| -> Option<i128> {
        // k*(k-1) is even, so the halving is exact
        k.checked_mul(a)?.checked_add(k.checked_mul(k - 1)? / 2)
    };
    let fits = |k: i128| partial(k).is_some_and(|p| i64::try_from(p).is_ok());
    // the vertex 1/2 - a lies between -a and 1 - a
    let low = [-a, 1 - a].map(|k| k.clamp(1, n));
    if !fits(n) || low.into_iter().any(|k| !fits(k)) {
        return Err(fail("E0503", vec![]));
    }
    Ok(Value::Int(partial(n).expect("checked") as i64))
}

/// `min(xs)` and `max(xs)`: nil when empty. The first element seeds the
/// search, so an element without an order fails as `<` does, and a lone
/// one is asked `x < x` (VA18). A range is answered from its ends.
fn best(t: &mut Task<'_>, a: &[Value], verb: &str, want: std::cmp::Ordering) -> Result<Flow, Stop> {
    nat::check_items(&a[0], verb)?;
    if let Value::Range(r) = &a[0] {
        let last = i128::from(r.end) - i128::from(!r.inclusive);
        if last < i128::from(r.start) {
            return Ok(Flow::Done(Value::Nil));
        }
        let at = if want.is_lt() { r.start } else { last as i64 };
        return Ok(Flow::Done(Value::Int(at)));
    }
    struct Best {
        best: Option<Value>,
        lone: bool,
        want: std::cmp::Ordering,
    }
    impl Collect for Best {
        fn item(&mut self, x: Value) -> Result<bool, Stop> {
            match &self.best {
                None => self.best = Some(x),
                Some(b) => {
                    self.lone = false;
                    // ties keep the earlier element
                    if compare(&x, b)? == self.want {
                        self.best = Some(x);
                    }
                }
            }
            Ok(false)
        }
        fn result(&mut self, _: &mut Task<'_>) -> Result<Flow, Stop> {
            let Some(best) = self.best.take() else {
                return Ok(Flow::Done(Value::Nil));
            };
            if self.lone {
                compare(&best, &best)?;
            }
            Ok(Flow::Done(best))
        }
    }
    let p = Best {
        best: None,
        lone: true,
        want,
    };
    drain_with(t, Cursor::new(&a[0]), p)
}

fn min_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    best(t, &a, "take the min of", std::cmp::Ordering::Less)
}

fn max_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    best(t, &a, "take the max of", std::cmp::Ordering::Greater)
}

/// The map argument by move: changed in place when nothing else holds it.
fn take_map(a: &mut [Value], i: usize) -> Rc<crate::value::Map> {
    match std::mem::replace(&mut a[i], Value::Nil) {
        Value::Map(m) => m,
        _ => unreachable!("checked by need_map"),
    }
}

/// `get(m, k, default = nil)`: the value at `k`, or `default` when the
/// key is missing. A key that is not a string is E0514, as for `k in m`.
fn get_fn(_: &mut Task<'_>, mut a: Args) -> Result<Value, Stop> {
    let m = nat::need_map("get a key from", &a[0])?;
    let k = nat::need_key(&a[1])?;
    match m.get(&**k) {
        Some(v) => Ok(v.clone()),
        None => Ok(a.pop().expect("default")),
    }
}

fn put_fn(_: &mut Task<'_>, mut a: Args) -> Result<Value, Stop> {
    nat::need_map("put into", &a[0])?;
    let k = nat::need_key(&a[1])?.clone();
    let v = a.pop().expect("v");
    let mut m = take_map(&mut a, 0);
    // Copies the map first when it is shared (`make_mut`): the original
    // never changes.
    Rc::make_mut(&mut m).put(k, v);
    Ok(Value::Map(m))
}

/// `merge(a, b)`: `b`'s entries put into `a` in `b`'s order, a piece at
/// a time (VA10). `a` is changed in place when nothing else holds it.
fn merge_fn(t: &mut Task<'_>, mut a: Args) -> Result<Flow, Stop> {
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

fn remove_fn(_: &mut Task<'_>, mut a: Args) -> Result<Value, Stop> {
    nat::need_map("remove from", &a[0])?;
    let k = nat::need_key(&a[1])?.clone();
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

/// `chars(s)`: a string function, so a value of another type is E0403
/// as for the others (`upper`, `split`), not a collection's E0506 (VA19).
fn chars_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    nat::need_text("s", &a[0])?;
    super::strings::chars_of(t, a[0].clone())
}
