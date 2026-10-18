// ---- HAMT, maps, the map API, collection operations and indexing ----

use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

use super::hamt::{Hamt, hash_key, set_test_hash};
use super::index;
use super::testutil::Rng;
use super::*;

fn key(s: &str) -> Rc<String> {
    Rc::new(s.to_string())
}

fn s(x: &str) -> Value {
    Value::str(x)
}

fn list(items: Vec<Value>) -> Value {
    Value::List(Rc::new(List::new(items)))
}

fn ints(xs: &[i64]) -> Vec<Value> {
    xs.iter().map(|&x| Value::Int(x)).collect()
}

fn map_of(pairs: &[(&str, Value)]) -> Map {
    let mut b = MapBuilder::default();
    for (k, v) in pairs {
        b.set(key(k), v.clone());
    }
    b.finish()
}

fn mapv(m: Map) -> Value {
    Value::Map(Rc::new(m))
}

fn range(start: i64, end: i64, inclusive: bool) -> Value {
    Value::Range(Rc::new(Range {
        start,
        end,
        inclusive,
    }))
}

fn tag(name: &str, args: Vec<Value>) -> Value {
    Value::Tag(Rc::new(Tag {
        name: intern(name),
        args,
    }))
}

fn int_of(v: &Value) -> i64 {
    match v {
        Value::Int(n) => *n,
        other => panic!("not an int: {other:?}"),
    }
}

/// Restores the real hash when a collision test ends, even on failure.
struct HashGuard;

impl Drop for HashGuard {
    fn drop(&mut self) {
        set_test_hash(None);
    }
}

// checkHamt: random set/del against a HashMap; saved versions are re-read
// to check that updates never change an older trie.
fn check_hamt(ops: usize, keyspace: usize, seed: u64) {
    let mut r = Rng::new(seed);
    let mut root = Hamt::default();
    let mut model: HashMap<String, usize> = HashMap::new();
    let mut history: Vec<(Hamt, HashMap<String, usize>)> = vec![];
    for step in 0..ops {
        let k = format!("k{}", r.int_n(keyspace));
        let h = hash_key(&k);
        if r.int_n(4) == 0 {
            let ok = root.del(h, &k);
            let had = model.remove(&k).is_some();
            assert_eq!(ok, had, "del {k}");
        } else {
            root.set(h, key(&k), step);
            model.insert(k, step);
        }
        if step % (ops / 20 + 1) == 0 {
            history.push((root.clone(), model.clone()));
        }
    }
    history.push((root, model));
    for (n, model) in &history {
        for i in 0..keyspace {
            let k = format!("k{i}");
            assert_eq!(n.get(hash_key(&k), &k), model.get(&k).copied(), "get {k}");
        }
    }
}

// TestHamt
#[test]
fn hamt_matches_reference() {
    check_hamt(200_000, 50_000, 3);
}

// TestHamtCollisions
#[test]
fn hamt_collisions() {
    let _g = HashGuard;
    set_test_hash(Some(|_| 7)); // every key collides
    check_hamt(5000, 1000, 4);
    set_test_hash(Some(|s| (s.len() as u32).wrapping_mul(0x9e37_79b9))); // few distinct hashes
    check_hamt(20_000, 5000, 5);
}

// TestHamtEmpty
#[test]
fn hamt_empty() {
    let mut n = Hamt::default();
    assert_eq!(n.get(hash_key("a"), "a"), None, "empty get");
    assert!(!n.del(hash_key("a"), "a") && n.is_empty(), "empty del");
    n.set(hash_key("a"), key("a"), 1);
    assert!(n.del(hash_key("a"), "a"), "del the last key");
    assert!(n.is_empty(), "deleting the last key leaves an empty trie");
}

/// The reference model of 02: insertion order, an existing key keeps its
/// place on put, a removed key comes back at the end.
#[derive(Clone, Default)]
struct RefMap {
    keys: Vec<String>,
    vals: HashMap<String, i64>,
}

impl RefMap {
    fn put(&mut self, k: &str, v: i64) {
        if self.vals.insert(k.to_string(), v).is_none() {
            self.keys.push(k.to_string());
        }
    }

    fn remove(&mut self, k: &str) {
        if self.vals.remove(k).is_some() {
            self.keys.retain(|a| a != k);
        }
    }
}

fn same_map(what: &str, m: &Map, r: &RefMap) {
    assert_eq!(m.len(), r.keys.len(), "{what}: len");
    for k in &r.keys {
        assert_eq!(m.get(k).map(int_of), Some(r.vals[k]), "{what}: get({k})");
    }
    let got: Vec<(&str, i64)> = m.iter().map(|(k, v)| (k, int_of(v))).collect();
    let want: Vec<(&str, i64)> = r.keys.iter().map(|k| (k.as_str(), r.vals[k])).collect();
    assert_eq!(got, want, "{what}: iter");
    assert!(m.get("absent-key").is_none(), "{what}: absent key found");
}

fn check_map_model(seed: u64) {
    let mut r = Rng::new(seed);
    let mut cur = (Map::new(), RefMap::default());
    let mut history: Vec<(Map, RefMap)> = vec![];
    let mut keyspace = 40; // starts near the small/big boundary, then grows
    for step in 0..20_000i64 {
        if step % 2500 == 0 {
            keyspace *= 2;
        }
        let k = format!("k{}", r.int_n(keyspace));
        match r.int_n(10) {
            0..6 => {
                cur.0.put(key(&k), Value::Int(step));
                cur.1.put(&k, step);
            }
            6..9 => {
                let had = cur.1.vals.contains_key(&k);
                assert_eq!(cur.0.remove(&k), had, "remove {k}");
                cur.1.remove(&k);
            }
            _ => {
                // merge a few keys from a fresh builder
                let mut b = MapBuilder::default();
                for j in 0..3 {
                    let kk = format!("k{}", r.int_n(keyspace));
                    b.set(key(&kk), Value::Int(-step - j));
                }
                let o = b.finish();
                for (kk, v) in o.iter() {
                    cur.1.put(kk, int_of(v));
                }
                cur.0 = cur.0.merge(&o);
            }
        }
        if step % 397 == 0 {
            history.push(cur.clone());
        }
        if step % 1999 == 0 {
            for (m, model) in &history {
                same_map("history", m, model);
            }
        }
    }
    same_map("final", &cur.0, &cur.1);
    let mut other = cur.0.clone();
    other.put(key("zz"), Value::Int(1));
    other.remove("zz");
    assert!(
        equal(&mapv(cur.0.clone()), &mapv(other)),
        "== must ignore internal state"
    );
}

// TestMapMatchesReference
#[test]
fn map_matches_reference() {
    check_map_model(0x7777_8888_9999_aaaa);
}

// The same model with forced hash collisions, so the omap runs over
// collision nodes and deep tries.
#[test]
fn map_matches_reference_with_collisions() {
    let _g = HashGuard;
    set_test_hash(Some(|s| (s.len() as u32).wrapping_mul(0x9e37_79b9)));
    check_map_model(11);
    set_test_hash(Some(|s| s.bytes().last().unwrap_or(0) as u32 & 3));
    check_map_model(12);
}

// TestMapPutExistingKeepsPosition
#[test]
fn map_put_existing_keeps_position() {
    let mut m = Map::new();
    for i in 0..100 {
        m.put(key(&format!("k{i}")), Value::Int(i));
    }
    m.put(key("k0"), Value::Int(-1));
    let first = m.iter().next().unwrap();
    assert_eq!(
        (first.0, int_of(first.1)),
        ("k0", -1),
        "put on an existing key moved it"
    );
    m.remove("k0");
    m.put(key("k0"), Value::Int(1));
    assert_eq!(m.len(), 100);
    assert_eq!(
        m.iter().last().unwrap().0,
        "k0",
        "a removed key comes back at the end"
    );
}

// TestMapBuilderBig
#[test]
fn map_builder_big() {
    let mut b = MapBuilder::default();
    let mut model = RefMap::default();
    for i in 0..500 {
        let k = format!("k{}", i % 300);
        b.set(key(&k), Value::Int(i));
        model.put(&k, i);
    }
    same_map("builder", &b.finish(), &model);
}

// TestMapPutFoldIsCheap: 100,000 new keys then 100,000 updates must be
// linear overall. Timed only in release builds.
#[cfg(not(debug_assertions))]
#[test]
fn map_put_fold_is_cheap() {
    let start = std::time::Instant::now();
    let mut m = Map::new();
    for i in 0..100_000 {
        m.put(key(&format!("k{i}")), Value::Int(i));
    }
    for i in 0..100_000 {
        m.put(key(&format!("k{i}")), Value::Int(-i));
    }
    let took = start.elapsed();
    assert!(took.as_secs_f64() < 2.0, "200,000 puts took {took:?}");
    assert_eq!(m.get("k99999").map(int_of), Some(-99999));
    assert_eq!(m.len(), 100_000);
}

// Shared values, single-threaded: Rc values never cross threads, so the
// point (derivations from one shared base never see each other) is
// checked by interleaving eight derivations.
#[test]
fn derivations_from_a_shared_base_stay_apart() {
    let mut base = Map::new();
    let mut lst = List::new(vec![]);
    for i in 0..200 {
        base.put(key(&format!("k{i}")), Value::Int(i));
        lst.push(Value::Int(i));
    }
    let mut ms: Vec<Map> = (0..8).map(|_| base.clone()).collect();
    let mut ls: Vec<List> = (0..8).map(|_| lst.clone()).collect();
    for i in 0..500 {
        for g in 0..8 {
            ms[g].put(key(&format!("g{g}-{i}")), Value::Int(i));
            ms[g].put(key("k0"), Value::Int(g as i64));
            ls[g].push(Value::Int(g as i64));
        }
    }
    for g in 0..8 {
        assert_eq!(
            ms[g].get("k0").map(int_of),
            Some(g as i64),
            "{g} saw another's update"
        );
        assert_eq!(ms[g].len(), 700);
        assert_eq!(ls[g].len(), 700);
        assert_eq!(ls[g].at(699).map(int_of), Some(g as i64));
    }
    assert_eq!(
        base.get("k0").map(int_of),
        Some(0),
        "the shared base changed"
    );
    assert_eq!((base.len(), lst.len()), (200, 200));
}

// brief: put on a shared map keeps the original
#[test]
fn put_on_a_shared_map_keeps_the_original() {
    let mut a = Map::new();
    for i in 0..100 {
        a.put(Rc::new(format!("k{i}")), Value::Int(i));
    }
    let b = a.clone();
    a.put(Rc::new("k0".to_string()), Value::Int(-1));
    assert!(matches!(b.get("k0"), Some(Value::Int(0))));
    assert_eq!(a.iter().next().unwrap().0, "k0"); // existing key keeps its place
}

// Small maps, remove and new keys on shared big maps, and the flat/big
// switch in both directions leave every other copy alone.
#[test]
fn shared_maps_are_never_changed() {
    let mut small = map_of(&[("a", Value::Int(1)), ("b", Value::Int(2))]);
    let keep = small.clone();
    small.put(key("a"), Value::Int(9));
    small.put(key("c"), Value::Int(3));
    small.remove("b");
    assert_eq!(format(&mapv(keep)), "{a: 1, b: 2}");
    assert_eq!(format(&mapv(small)), "{a: 9, c: 3}");

    let mut big = Map::new();
    for i in 0..SMALL_MAP_MAX as i64 {
        big.put(key(&format!("k{i}")), Value::Int(i));
    }
    let flat = big.clone();
    big.put(key("x"), Value::Int(-1)); // flat -> big
    let at17 = big.clone();
    big.put(key("y"), Value::Int(-2));
    big.remove("k3");
    let removed = big.clone();
    big.remove("x");
    big.remove("y"); // big -> flat
    assert_eq!(flat.len(), SMALL_MAP_MAX);
    assert!(flat.get("x").is_none());
    assert_eq!(at17.len(), SMALL_MAP_MAX + 1);
    assert!(at17.get("y").is_none() && at17.get("k3").is_some());
    assert_eq!(removed.len(), SMALL_MAP_MAX + 1);
    assert!(removed.get("k3").is_none() && removed.get("y").is_some());
    assert_eq!(big.len(), SMALL_MAP_MAX - 1);
    let order: Vec<&str> = big.iter().map(|(k, _)| k).collect();
    assert_eq!(order[..4], ["k0", "k1", "k2", "k4"]);
}

#[test]
fn dropping_a_huge_map_does_not_overflow() {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(|| {
            let mut m = Map::new();
            for i in 0..200_000 {
                m.put(Rc::new(format!("k{i}")), Value::Int(i));
            }
            drop(m);
        })
        .unwrap()
        .join()
        .unwrap();
}

/// Builds a chain `depth` levels deep with `wrap` and drops it on a 1 MB
/// stack; any recursive drop overflows it.
fn drop_deep_chain(depth: usize, wrap: fn(Value) -> Value) {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || {
            let mut v = Value::Nil;
            for _ in 0..depth {
                v = wrap(v);
            }
            drop(v);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn dropping_a_deep_tuple_chain_does_not_overflow() {
    drop_deep_chain(1_000_000, |v| Value::tuple(vec![v]));
}

#[test]
fn dropping_a_deep_tag_chain_does_not_overflow() {
    drop_deep_chain(1_000_000, |v| tag("Some", vec![v]));
}

#[test]
fn dropping_a_deep_map_chain_does_not_overflow() {
    // small (flat) maps
    drop_deep_chain(1_000_000, |v| mapv(map_of(&[("next", v)])));
    // big (omap) maps holding the next level among other keys
    drop_deep_chain(20_000, |v| {
        let mut m = Map::new();
        for i in 0..20 {
            m.put(key(&format!("k{i}")), Value::Int(i));
        }
        m.put(key("next"), v);
        mapv(m)
    });
}

#[test]
fn dropping_mixed_nesting_does_not_overflow() {
    // map -> list -> tuple -> tag -> map ...
    drop_deep_chain(300_000, |v| {
        mapv(map_of(&[(
            "xs",
            list(vec![Value::tuple(vec![tag("T", vec![v])])]),
        )]))
    });
}

// A value replaced inside a map it alone owned is freed iteratively too.
#[test]
fn replacing_a_deep_value_does_not_overflow() {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(|| {
            let mut v = Value::Nil;
            for _ in 0..1_000_000 {
                v = Value::tuple(vec![v]);
            }
            let mut m = Map::new();
            m.put(key("deep"), v);
            m.put(key("deep"), Value::Nil);
            assert_eq!(m.len(), 1);
        })
        .unwrap()
        .join()
        .unwrap();
}

// TestCollectionFormat (maps, tuples, tags, ranges)
#[test]
fn collection_format() {
    let m = map_of(&[("name", s("x")), ("a b", Value::Int(1))]);
    let cases = [
        (Value::tuple(ints(&[1, 2])), "(1, 2)"),
        (Value::tuple(ints(&[1])), "(1,)"),
        (Value::tuple(vec![]), "()"),
        (mapv(m), r#"{name: "x", "a b": 1}"#),
        (mapv(Map::new()), "{}"),
        (tag("Circle", ints(&[3])), "Circle(3)"),
        (tag("Rect", ints(&[2, 5])), "Rect(2, 5)"),
        (range(0, 10, false), "0..10"),
        (range(0, 10, true), "0..=10"),
        // keys that need quoting
        (
            mapv(map_of(&[
                ("a b", Value::Int(1)),
                ("name", s("x")),
                ("1x", Value::Int(2)),
                ("", Value::Int(3)),
                ("한글", Value::Int(4)),
                ("_a1", Value::Int(5)),
            ])),
            r#"{"a b": 1, name: "x", "1x": 2, "": 3, 한글: 4, _a1: 5}"#,
        ),
        (
            mapv(map_of(&[(
                "a",
                list(vec![Value::Int(1), mapv(map_of(&[("b", s("s"))]))]),
            )])),
            r#"{a: [1, {b: "s"}]}"#,
        ),
    ];
    for (v, want) in cases {
        assert_eq!(format(&v), want);
    }
    assert_eq!(
        format_for_print(&mapv(map_of(&[("k", s("v"))]))),
        r#"{k: "v"}"#
    );
    assert_eq!(type_name(&tag("Circle", vec![])), "Circle");
    assert_eq!(type_name(&mapv(Map::new())), "map");
    assert_eq!(type_name(&Value::tuple(vec![])), "tuple");
    assert_eq!(type_name(&range(0, 0, false)), "range");
    // a big map formats in insertion order
    let mut big = Map::new();
    for i in (0..40).rev() {
        big.put(key(&format!("k{i}")), Value::Int(i));
    }
    let want: Vec<String> = (0..40).rev().map(|i| format!("k{i}: {i}")).collect();
    assert_eq!(format(&mapv(big)), format!("{{{}}}", want.join(", ")));
}

// TestCollectionEquality (maps, tags, ranges)
#[test]
fn collection_equality() {
    let m1 = map_of(&[("a", Value::Int(1)), ("b", Value::Int(2))]);
    let m2 = map_of(&[("b", Value::Int(2)), ("a", Value::Int(1))]);
    let cases = [
        (mapv(m1.clone()), mapv(m2), true),
        (tag("Ok", ints(&[1])), tag("Ok", ints(&[1])), true),
        (tag("Ok", ints(&[1])), tag("Err", ints(&[1])), false),
        (range(0, 3, false), range(0, 3, false), true),
        (range(0, 3, false), range(0, 3, true), false),
        // values compare like ==: 1 == 1.0
        (
            mapv(map_of(&[("a", Value::Int(1))])),
            mapv(map_of(&[("a", Value::Float(1.0))])),
            true,
        ),
        (
            mapv(map_of(&[("a", Value::Int(1))])),
            mapv(map_of(&[("a", Value::Int(2))])),
            false,
        ),
        (
            mapv(map_of(&[("a", Value::Int(1))])),
            mapv(map_of(&[("b", Value::Int(1))])),
            false,
        ),
        (
            mapv(m1.clone()),
            mapv(map_of(&[("a", Value::Int(1))])),
            false,
        ),
        (mapv(Map::new()), Value::Nil, false),
    ];
    for (a, b, want) in cases {
        assert_eq!(equal(&a, &b), want, "{} == {}", format(&a), format(&b));
    }
    // a big map equals a small one with the same entries
    let mut big = Map::new();
    let mut small = Map::new();
    for i in 0..20 {
        big.put(key(&format!("k{i}")), Value::Int(i));
    }
    for i in (0..20).rev() {
        small.put(key(&format!("k{i}")), Value::Int(i));
    }
    big.remove("k5");
    small.remove("k5");
    assert!(equal(&mapv(big), &mapv(small)));
}

// TestCollectionOrdering (tuples, maps, tags)
#[test]
fn collection_ordering() {
    let c = compare(&Value::tuple(vec![s("b")]), &Value::tuple(vec![s("a")]));
    assert_eq!(c.unwrap(), Ordering::Greater, "tuples");
    let pairs = [
        (mapv(Map::new()), mapv(Map::new())),
        (tag("A", vec![]), tag("A", vec![])),
        (range(0, 1, false), range(0, 1, false)),
    ];
    for (a, b) in pairs {
        let e = compare(&a, &b).unwrap_err();
        assert_eq!(e.code, "E0501", "{} <=> {}", format(&a), format(&b));
    }
    let e = compare(&mapv(Map::new()), &mapv(Map::new())).unwrap_err();
    assert_eq!(msg(&e), "cannot compare map and map");
}

// `put` on a map copies it; maps have no `+`.
#[test]
fn map_put_copies() {
    let m = map_of(&[("a", Value::Int(1))]);
    let mut m2 = m.clone();
    m2.put(key("b"), Value::Int(2));
    assert_eq!(m.len(), 1);
    assert_eq!(format(&mapv(m2.clone())), "{a: 1, b: 2}");
    let mut m3 = m2.clone();
    m3.put(key("a"), Value::Int(9));
    assert_eq!(format(&mapv(m3)), "{a: 9, b: 2}", "put keeps order");
    let e = add(&mapv(m.clone()), &mapv(m2)).unwrap_err();
    assert_eq!(msg(&e), "cannot add map and map");
}

// TestMapAPI
#[test]
fn map_api() {
    let mut b = MapBuilder::default();
    b.set(key("b"), Value::Int(1));
    b.set(key("a"), Value::Int(2));
    b.set(key("b"), Value::Int(3)); // replace keeps position
    let m = b.finish();
    assert_eq!(format(&mapv(m.clone())), "{b: 3, a: 2}");
    assert_eq!(m.len(), 2);
    let mut m2 = m.clone();
    m2.put(key("c"), Value::Int(4));
    m2.put(key("b"), Value::Int(5));
    assert_eq!(format(&mapv(m2.clone())), "{b: 5, a: 2, c: 4}");
    assert_eq!(format(&mapv(m.clone())), "{b: 3, a: 2}");
    let mut r = m2.clone();
    assert!(r.remove("a"));
    assert_eq!(format(&mapv(r)), "{b: 5, c: 4}");
    let mut r = m2.clone();
    assert!(!r.remove("zz"));
    assert_eq!(format(&mapv(r)), format(&mapv(m2.clone())));
    let o = map_of(&[("a", Value::Int(0)), ("d", Value::Int(1))]);
    assert_eq!(format(&mapv(m.merge(&o))), "{b: 3, a: 0, d: 1}");
    let mut n = 0;
    for (k, v) in m2.iter() {
        assert!(equal(m2.get(k).unwrap(), v), "iter");
        n += 1;
    }
    assert_eq!(n, 3);
    assert_eq!(Map::new().len(), 0);
    assert!(Map::new().is_empty());
}

fn msg(e: &fung_front::diag::Diag) -> String {
    fung_front::diag::message(&e.code, &e.args)
}

fn code_of(r: Result<impl std::fmt::Debug, fung_front::diag::Diag>) -> String {
    match r {
        Err(e) => e.code,
        Ok(v) => panic!("want an error, got {v:?}"),
    }
}

// len of each sized value
#[test]
fn len_cases() {
    let cases = [
        (s("참치"), 2),
        (list(ints(&[1, 2])), 2),
        (Value::tuple(vec![]), 0),
        (range(0, 10, false), 10),
        (range(0, 10, true), 11),
        (range(5, 2, false), 0),
        (mapv(map_of(&[("k", Value::Nil)])), 1),
        (range(i64::MIN, -2, true), i64::MAX),
    ];
    for (v, want) in cases {
        assert_eq!(index::len(&v).unwrap(), want, "len({})", format(&v));
    }
    // A range wider than the int range is counted exactly; its length is
    // more than an int holds.
    let e = index::len(&range(i64::MIN, i64::MAX, false)).unwrap_err();
    assert_eq!(e.code, "E0503");
    let e = index::len(&Value::Int(1)).unwrap_err();
    assert_eq!(e.code, "E0506");
    assert_eq!(msg(&e), "cannot take the length of int");
}

// TestIndex
#[test]
fn index_cases() {
    let xs = list(ints(&[10, 20, 30]));
    let m = mapv(map_of(&[("name", s("x"))]));
    let ok = [
        (xs.clone(), Value::Int(0), "10"),
        (xs.clone(), Value::Int(-1), "30"),
        (s("참치"), Value::Int(1), r#""치""#),
        (s("참치"), Value::Int(-2), r#""참""#),
        (m.clone(), s("name"), r#""x""#),
    ];
    for (v, i, want) in ok {
        let got = index::index(&v, &i).unwrap();
        assert_eq!(format(&got), want, "{}[{}]", format(&v), format(&i));
    }
    let bad = [
        (
            xs.clone(),
            Value::Int(3),
            "E0507",
            "index 3 is out of range (length 3)",
        ),
        (
            xs.clone(),
            Value::Int(-4),
            "E0507",
            "index -4 is out of range (length 3)",
        ),
        (
            xs.clone(),
            Value::Int(i64::MIN),
            "E0507",
            "index -9223372036854775808 is out of range (length 3)",
        ),
        (
            s("ab"),
            Value::Int(2),
            "E0507",
            "index 2 is out of range (length 2)",
        ),
        (xs.clone(), s("a"), "E0501", "cannot index list and str"),
        (
            Value::tuple(ints(&[10, 20])),
            Value::Int(0),
            "E0509",
            "cannot index a value of type tuple",
        ),
        (
            Value::Int(1),
            Value::Int(0),
            "E0509",
            "cannot index a value of type int",
        ),
        (m.clone(), s("age"), "E0508", "key `age` not found"),
        (
            m.clone(),
            Value::Int(0),
            "E0501",
            "cannot index map and int",
        ),
        (s("a"), s("b"), "E0501", "cannot index str and str"),
    ];
    for (v, i, code, want) in bad {
        let e = index::index(&v, &i).unwrap_err();
        assert_eq!((e.code.as_str(), msg(&e)), (code, want.to_string()));
    }
    // a big list and a view on it
    let big: Vec<i64> = (0..100).collect();
    let l = List::new(ints(&big)).slice(10, 90);
    let lv = Value::List(Rc::new(l));
    assert_eq!(int_of(&index::index(&lv, &Value::Int(0)).unwrap()), 10);
    assert_eq!(int_of(&index::index(&lv, &Value::Int(-1)).unwrap()), 89);
    assert_eq!(code_of(index::index(&lv, &Value::Int(80))), "E0507");
}

// `x.name` is an index by a string key.
#[test]
fn field_cases() {
    let m = mapv(map_of(&[("name", s("x"))]));
    assert_eq!(format(&index::field(&m, "name").unwrap()), r#""x""#);
    let e = index::field(&m, "age").unwrap_err();
    assert_eq!(e.code, "E0508");
    let e = index::field(&s("a"), "b").unwrap_err();
    assert_eq!(msg(&e), "cannot index str and str");
}

fn slice_str(x: &Value, start: Value, end: Value, inclusive: bool) -> Result<String, String> {
    let open = |v: &Value| !matches!(v, Value::Nil);
    let (a, b) = (open(&start).then_some(&start), open(&end).then_some(&end));
    match index::slice(x, a, b, inclusive) {
        Ok(v) => Ok(format(&v)),
        Err(e) => Err(format!("{} {}", e.code, msg(&e))),
    }
}

// TestSlice
#[test]
fn slice_cases() {
    let xs = list(ints(&[1, 2, 3]));
    let n = Value::Nil;
    let i = Value::Int;
    let cases: [(Value, Value, Result<&str, &str>); 11] = [
        (i(1), i(3), Ok("[2, 3]")),
        (n.clone(), i(2), Ok("[1, 2]")),
        (i(-2), n.clone(), Ok("[2, 3]")),
        (i(0), i(0), Ok("[]")),
        (i(3), n.clone(), Ok("[]")),
        (
            i(4),
            n.clone(),
            Err("E0507 index 4 is out of range (length 3)"),
        ),
        (i(0), i(4), Err("E0507 index 4 is out of range (length 3)")),
        (i(2), i(1), Err("E0507 index 2 is out of range (length 3)")),
        (n.clone(), n.clone(), Ok("[1, 2, 3]")),
        // the index is reported as written
        (
            i(-5),
            n.clone(),
            Err("E0507 index -5 is out of range (length 3)"),
        ),
        (
            s("a"),
            n.clone(),
            Err("E0501 cannot slice with int and str"),
        ),
    ];
    for (a, b, want) in cases {
        let got = slice_str(&xs, a.clone(), b.clone(), false);
        assert_eq!(got.as_deref().map_err(|e| e.as_str()), want, "{a:?}..{b:?}");
    }
    assert_eq!(
        slice_str(&xs, i(0), s("b"), false).unwrap_err(),
        "E0501 cannot slice with int and str"
    );
    // an explicit nil is open

    let nil = Value::Nil;
    assert_eq!(
        format(&index::slice(&xs, Some(&nil), Some(&nil), false).unwrap()),
        "[1, 2, 3]"
    );
    assert_eq!(
        slice_str(&s("참치회"), i(1), n.clone(), false).unwrap(),
        r#""치회""#
    );
    assert_eq!(
        slice_str(&Value::tuple(vec![]), n.clone(), n.clone(), false).unwrap_err(),
        "E0511 cannot slice a value of type tuple"
    );
    // a slice of a big list is a list with those items
    let big: Vec<i64> = (0..100).collect();
    let bl = list(ints(&big));
    let got = index::slice(&bl, Some(&i(-60)), Some(&i(-10)), false).unwrap();
    let Value::List(l) = &got else { unreachable!() };
    let want: Vec<i64> = (40..90).collect();
    assert_eq!(l.iter().map(int_of).collect::<Vec<_>>(), want);
}

// TestSliceInclusive
#[test]
fn slice_inclusive_cases() {
    let xs = list(ints(&[1, 2, 3]));
    let n = Value::Nil;
    let i = Value::Int;
    let cases: [(Value, Value, Result<&str, &str>); 6] = [
        (i(1), i(2), Ok("[2, 3]")),
        (i(0), i(-1), Ok("[1, 2, 3]")),
        (n.clone(), i(0), Ok("[1]")),
        (i(0), i(3), Err("E0507 index 3 is out of range (length 3)")),
        (i(2), i(0), Err("E0507 index 2 is out of range (length 3)")),
        // the end as written, not plus one, and nothing wraps
        (
            i(0),
            i(i64::MAX),
            Err("E0507 index 9223372036854775807 is out of range (length 3)"),
        ),
    ];
    for (a, b, want) in cases {
        let got = slice_str(&xs, a.clone(), b.clone(), true);
        assert_eq!(
            got.as_deref().map_err(|e| e.as_str()),
            want,
            "{a:?}..={b:?}"
        );
    }
}

// TestContains
#[test]
fn contains_cases() {
    let m = mapv(map_of(&[("k", Value::Int(1))]));
    let cases = [
        (list(ints(&[1, 2])), Value::Int(2), true),
        (list(ints(&[1])), Value::Float(1.0), true),
        (list(vec![]), Value::Int(1), false),
        (s("hello"), s("ell"), true),
        (s("hello"), s("z"), false),
        (m.clone(), s("k"), true),
        (m.clone(), s("z"), false),
        (range(0, 10, false), Value::Int(9), true),
        (range(0, 10, false), Value::Int(10), false),
        (range(0, 10, true), Value::Int(10), true),
        (range(0, 10, false), Value::Float(1.0), false),
    ];
    for (c, x, want) in cases {
        let got = index::contains(&c, &x).unwrap();
        assert_eq!(got, want, "{} in {}", format(&x), format(&c));
    }
    let e = index::contains(&s("a"), &Value::Int(1)).unwrap_err();
    assert_eq!(
        (e.code.as_str(), msg(&e)),
        ("E0501", "cannot search str and int".into())
    );
    let e = index::contains(&m, &Value::Int(1)).unwrap_err();
    assert_eq!(e.code, "E0501");
    let e = index::contains(&Value::Int(1), &Value::Int(1)).unwrap_err();
    assert_eq!(
        (e.code.as_str(), msg(&e)),
        (
            "E0512",
            "cannot test membership in a value of type int".into()
        )
    );
}
