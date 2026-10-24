//! The core collection functions: the ones that walk elements natively.
//! They iterate lists, ranges and channels (02: 순회 대상); strings and
//! maps become lists with `chars` and `pairs` first.

use std::collections::HashMap;
use std::rc::Rc;

use fung_front::diag::{Arg, Diag};

use crate::native::nat::{self, def, def_step, fail, opt, param};
use crate::native::walk::{Collect, Cursor, PerItem, drain_with, walk};
use crate::value::{ListBuilder, Lit, MapBuilder, Value, compare, eq, hash_value, type_name};
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
    add(def("pairs", pairs_fn, vec![param("m")]));
    add(def(
        "get",
        get_fn,
        vec![param("m"), param("k"), opt("default", Lit::Nil)],
    ));
    add(def("put", put_fn, vec![param("m"), param("k"), param("v")]));
    add(def("merge", merge_fn, vec![param("a"), param("b")]));
    add(def("remove", remove_fn, vec![param("m"), param("k")]));
    add(def("chars", chars_fn, vec![param("s")]));
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
                Some(a) => Err(fail("E0606", vec![a])),
                None => Ok(false),
            }
        }
        fn result(&mut self, _: bool) -> Result<Value, Stop> {
            Ok(Value::Nil)
        }
    }
    walk(t, Cursor::new(&a[0]), a[1].clone(), Each)
}

/// `xs` ordered by `keys` (02: stable, by `<`). A top-down merge sort on
/// indices: comparisons can fail (`1 < "a"`), and the first failure ends
/// the sort with its error. (The standard library's sort is not used: it
/// may panic when the order it is given is not total.)
fn sorted_by(xs: Vec<Value>, keys: &[Value]) -> Result<Value, Stop> {
    fn merge_sort(idx: &mut [usize], tmp: &mut [usize], keys: &[Value]) -> Result<(), Diag> {
        let n = idx.len();
        if n < 2 {
            return Ok(());
        }
        let mid = n / 2;
        merge_sort(&mut idx[..mid], &mut tmp[..mid], keys)?;
        merge_sort(&mut idx[mid..], &mut tmp[mid..], keys)?;
        tmp[..n].copy_from_slice(idx);
        let (mut i, mut j) = (0, mid);
        for slot in idx.iter_mut() {
            // The right element goes first only when it is smaller: equal
            // keys keep their order.
            let right = j < n && (i == mid || compare(&keys[tmp[j]], &keys[tmp[i]])?.is_lt());
            if right {
                *slot = tmp[j];
                j += 1;
            } else {
                *slot = tmp[i];
                i += 1;
            }
        }
        Ok(())
    }
    let mut idx: Vec<usize> = (0..xs.len()).collect();
    let mut tmp = vec![0; xs.len()];
    merge_sort(&mut idx, &mut tmp, keys)?;
    let mut slots: Vec<Option<Value>> = xs.into_iter().map(Some).collect();
    let mut out = ListBuilder::with_capacity(slots.len());
    for i in idx {
        out.push(slots[i].take().expect("each index once"));
    }
    Ok(Value::List(Rc::new(out.finish())))
}

/// The elements of `v` handed to `then` all at once: at once for a list
/// or range, as they arrive for a receiving end (the walk parks).
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
        _ => then(t, nat::items(v, "sort")?),
    }
}

fn sort_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "sort")?;
    all_items(t, &a[0], |_, xs| {
        let keys = xs.clone();
        sorted_by(xs, &keys).map(Flow::Done)
    })
}

/// `unique(xs)`: the elements without the later ones `==` an earlier one,
/// in order (02). Hashing by `hash_value` makes it linear (P1); elements
/// in one bucket are compared with `eq`, so a symbol meeting a string is
/// E0517 as with `in`.
fn unique_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "unique")?;
    all_items(t, &a[0], |_, xs| {
        let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
        let mut out: Vec<Value> = Vec::new();
        for x in xs {
            let mut h = std::hash::DefaultHasher::new();
            hash_value(&x, &mut h);
            let b = buckets.entry(std::hash::Hasher::finish(&h)).or_default();
            let mut seen = false;
            for &i in b.iter() {
                if eq(&out[i], &x)? {
                    seen = true;
                    break;
                }
            }
            if !seen {
                b.push(out.len());
                out.push(x);
            }
        }
        Ok(Flow::Done(nat::list(out)))
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
            sorted_by(std::mem::take(&mut self.xs), &self.keys)
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
        groups: Vec<(Rc<String>, ListBuilder)>,
        at: HashMap<Rc<String>, usize>,
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
            let mut out = MapBuilder::default();
            for (k, items) in self.groups.drain(..) {
                out.set(k, Value::List(Rc::new(items.finish())));
            }
            Ok(Value::Map(Rc::new(out.finish())))
        }
    }
    walk(t, Cursor::new(&a[0]), a[1].clone(), Group::default())
}

fn pairs_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let m = nat::need_map("list the pairs of", &a[0])?;
    let mut out = ListBuilder::with_capacity(m.len());
    for (k, v) in m.entries() {
        out.push(Value::tuple(vec![Value::Str(k.clone()), v.clone()]));
    }
    Ok(Value::List(Rc::new(out.finish())))
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
    match m.get(k) {
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

fn merge_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let x = nat::need_map("merge", &a[0])?;
    let y = nat::need_map("merge", &a[1])?;
    Ok(Value::Map(Rc::new(x.merge(y))))
}

fn remove_fn(_: &mut Task<'_>, mut a: Vec<Value>) -> Result<Value, Stop> {
    nat::need_map("remove from", &a[0])?;
    let k = nat::need_key("index", &a[1])?.clone();
    let mut m = take_map(&mut a, 0);
    if m.get(&k).is_some() {
        Rc::make_mut(&mut m).remove(&k);
    }
    Ok(Value::Map(m))
}

fn chars_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let Value::Str(s) = &a[0] else {
        return Err(fail(
            "E0506",
            vec![nat::s("list the characters of"), Arg::Str(type_name(&a[0]))],
        ));
    };
    let mut out = ListBuilder::default();
    let mut b = [0; 4];
    for c in s.chars() {
        out.push(Value::str(c.encode_utf8(&mut b)));
    }
    Ok(Value::List(Rc::new(out.finish())))
}
