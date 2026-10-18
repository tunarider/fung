//! The core collection functions: the ones that walk elements natively.
//! They iterate lists, ranges and channels (02: 순회 대상); strings and
//! maps become lists with `chars` and `pairs` first.

use std::collections::HashMap;
use std::rc::Rc;

use fung_front::diag::{Arg, Diag};

use crate::native::nat::{self, def, fail, param};
use crate::value::{ListBuilder, MapBuilder, Value, compare, type_name};
use crate::vm::{Stop, Task};

pub(super) fn add(add: &mut impl FnMut(Value)) {
    add(def("filter", filter_fn, vec![param("xs"), param("f")]));
    add(def(
        "fold",
        fold_fn,
        vec![param("xs"), param("init"), param("f")],
    ));
    add(def("each", each_fn, vec![param("xs"), param("f")]));
    add(def("sort", sort_fn, vec![param("xs")]));
    add(def("sort_by", sort_by_fn, vec![param("xs"), param("f")]));
    add(def("find", find_fn, vec![param("xs"), param("f")]));
    add(def("any", any_fn, vec![param("xs"), param("f")]));
    add(def("all", all_fn, vec![param("xs"), param("f")]));
    add(def("flatten", flatten_fn, vec![param("xs")]));
    add(def("group_by", group_by_fn, vec![param("xs"), param("f")]));
    add(def("pairs", pairs_fn, vec![param("m")]));
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

fn filter_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    need_fn_after(&a, 1, "filter")?;
    let mut out = ListBuilder::default();
    let mut buf = Vec::with_capacity(1);
    let f = &a[1];
    nat::each(t, &a[0], "filter", |t, x| {
        if nat::call_bool(t, f, x.clone(), &mut buf)? {
            out.push(x);
        }
        Ok(())
    })?;
    Ok(Value::List(Rc::new(out.finish())))
}

fn fold_fn(t: &mut Task<'_>, mut a: Vec<Value>) -> Result<Value, Stop> {
    need_fn_after(&a, 2, "fold")?;
    let f = a.pop().expect("f");
    let mut acc = Some(a.pop().expect("init"));
    // One argument buffer for every call: the accumulator moves in and the
    // result moves back out, so nothing is copied per element.
    let mut buf = Vec::with_capacity(2);
    nat::each(t, &a[0], "fold", |t, x| {
        buf.push(acc.take().expect("the accumulator"));
        buf.push(x);
        acc = Some(t.call_buf(&f, &mut buf)?);
        Ok(())
    })?;
    Ok(acc.expect("the accumulator"))
}

fn each_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    need_fn_after(&a, 1, "iterate over")?;
    let mut buf = Vec::with_capacity(1);
    let f = &a[1];
    nat::each(t, &a[0], "iterate over", |t, x| {
        buf.push(x);
        t.call_buf(f, &mut buf)?;
        Ok(())
    })?;
    Ok(Value::Nil)
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

fn sort_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let xs = nat::items(t, &a[0], "sort")?;
    let keys = xs.clone();
    sorted_by(xs, &keys)
}

fn sort_by_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    nat::check_items(&a[0], "sort")?;
    nat::need_fn("f", &a[1])?;
    let xs = nat::items(t, &a[0], "sort")?;
    let mut keys = Vec::with_capacity(xs.len());
    let mut buf = Vec::with_capacity(1);
    for x in &xs {
        buf.push(x.clone());
        keys.push(t.call_buf(&a[1], &mut buf)?);
    }
    sorted_by(xs, &keys)
}

fn find_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    nat::check_items(&a[0], "search")?;
    nat::need_fn("f", &a[1])?;
    let mut buf = Vec::with_capacity(1);
    let mut hit = Value::Nil;
    let f = &a[1];
    nat::each_until(t, &a[0], "search", |t, x| {
        let found = nat::call_bool(t, f, x.clone(), &mut buf)?;
        if found {
            hit = x;
        }
        Ok(found)
    })?;
    Ok(hit)
}

/// `any` (stop = true) and `all` (stop = false): stops at the first
/// element whose predicate result equals `stop`.
fn quantify(t: &mut Task<'_>, a: &mut [Value], stop: bool) -> Result<Value, Stop> {
    nat::check_items(&a[0], "test")?;
    nat::need_fn("f", &a[1])?;
    let mut buf = Vec::with_capacity(1);
    let f = &a[1];
    let stopped = nat::each_until(t, &a[0], "test", |t, x| {
        Ok(nat::call_bool(t, f, x, &mut buf)? == stop)
    })?;
    Ok(Value::Bool(stopped == stop))
}

fn any_fn(t: &mut Task<'_>, mut a: Vec<Value>) -> Result<Value, Stop> {
    quantify(t, &mut a, true)
}

fn all_fn(t: &mut Task<'_>, mut a: Vec<Value>) -> Result<Value, Stop> {
    quantify(t, &mut a, false)
}

/// `flatten(xs)` splices nested lists one level deep. It stays in Rust:
/// a fung fold with `acc + x` would copy the accumulator per element.
fn flatten_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    nat::check_items(&a[0], "flatten")?;
    nat::limit_elements(&a[0])?;
    let mut out = ListBuilder::default();
    nat::each(t, &a[0], "flatten", |_, x| {
        match &x {
            Value::List(inner) => inner.iter().for_each(|v| out.push(v.clone())),
            _ => out.push(x),
        }
        Ok(())
    })?;
    Ok(Value::List(Rc::new(out.finish())))
}

fn group_by_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    nat::check_items(&a[0], "group")?;
    nat::need_fn("f", &a[1])?;
    nat::limit_elements(&a[0])?;
    // Keys in first-seen order, each group in input order. The hash map
    // only finds a key's group; the order comes from `groups`.
    let mut groups: Vec<(Rc<String>, ListBuilder)> = Vec::new();
    let mut at: HashMap<Rc<String>, usize> = HashMap::new();
    let mut buf = Vec::with_capacity(1);
    let f = &a[1];
    nat::each(t, &a[0], "group", |t, x| {
        buf.push(x.clone());
        let k = t.call_buf(f, &mut buf)?;
        let Value::Str(key) = k else {
            return Err(fail("E0514", vec![Arg::Str(type_name(&k))]));
        };
        let i = *at.entry(key.clone()).or_insert_with(|| {
            groups.push((key, ListBuilder::default()));
            groups.len() - 1
        });
        groups[i].1.push(x);
        Ok(())
    })?;
    let mut out = MapBuilder::default();
    for (k, items) in groups {
        out.set(k, Value::List(Rc::new(items.finish())));
    }
    Ok(Value::Map(Rc::new(out.finish())))
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
