//! Whether a value holds a mutable one (concurrency v2, L5): what an
//! atom may not hold, known in O(1) after the first look.

use std::rc::Rc;

use crate::testutil::on_big_stack;
use crate::value::{List, Map, Tag, Tuple, Value, intern};

/// Runs `f` on a big stack.
fn with_token(f: fn()) {
    on_big_stack(f);
}

/// A channel's sending end (its receiving end is dropped at once).
fn ch() -> Value {
    Value::Sender(Rc::new(crate::vm::channel::pair(0).0))
}

fn ints(n: i64) -> Vec<Value> {
    (0..n).map(Value::Int).collect()
}

fn key(i: usize) -> Rc<String> {
    Rc::new(format!("k{i}"))
}

fn big_map(n: usize, with: Option<(usize, Value)>) -> Map {
    let mut m = Map::new();
    for i in 0..n {
        m.put(key(i), Value::Int(i as i64));
    }
    if let Some((i, v)) = with {
        m.put(key(i), v);
    }
    m
}

#[test]
fn ints_hold_nothing_mutable() {
    with_token(|| {
        assert!(!Value::List(Rc::new(List::new(ints(1000)))).holds_mutable());
        assert!(!Value::Map(Rc::new(big_map(1000, None))).holds_mutable());
    });
}

#[test]
fn a_channel_anywhere_is_found() {
    with_token(|| {
        let mut items = ints(1000);
        items[500] = ch();
        assert!(Value::List(Rc::new(List::new(items))).holds_mutable());
        assert!(Value::Map(Rc::new(big_map(1000, Some((7, ch()))))).holds_mutable());
        assert!(Value::Tuple(Rc::new(Tuple::new(vec![Value::Int(1), ch()]))).holds_mutable());
        let tag = Tag {
            name: intern("Ok"),
            args: vec![ch()],
        };
        assert!(Value::Tag(Rc::new(tag)).holds_mutable());
        let inner = Value::Tuple(Rc::new(Tuple::new(vec![ch()])));
        let mut m = Map::new();
        m.put(key(0), inner);
        let nested = Value::List(Rc::new(List::new(vec![Value::Map(Rc::new(m))])));
        assert!(nested.holds_mutable());
    });
}

#[test]
fn removing_the_channel_clears_it() {
    with_token(|| {
        let mut m = big_map(1000, Some((7, ch())));
        assert!(Value::Map(Rc::new(m.clone())).holds_mutable());
        m.put(key(7), Value::Int(7));
        assert!(!Value::Map(Rc::new(m.clone())).holds_mutable());
        let mut m2 = big_map(1000, Some((900, ch())));
        assert!(m2.remove("k900"));
        assert!(!Value::Map(Rc::new(m2)).holds_mutable());
    });
}

#[test]
fn a_slice_without_the_channel_holds_none() {
    with_token(|| {
        let mut items = ints(200);
        items[0] = ch();
        let l = List::new(items);
        assert!(Value::List(Rc::new(l.clone())).holds_mutable());
        assert!(!Value::List(Rc::new(l.slice(100, 200))).holds_mutable());
    });
}

/// A slice of a big list answers for its own range only, wherever the
/// channel sits: at a leaf's edge, inside a subtree, in the tail.
#[test]
fn a_slice_of_a_big_list_sees_exactly_its_range() {
    with_token(|| {
        for at in [0, 31, 32, 1023, 1024, 3000, 5000, 5070] {
            let mut items = ints(5080);
            items[at] = ch();
            let l = List::new(items);
            // all at least a quarter of the list, so each stays a view
            let n = 5080;
            let w = 1500;
            let ranges = [
                (0, n),
                (at.saturating_sub(w), (at + 1).max(w)),
                (at.min(n - w), (at.min(n - w) + w)),
                (at + 1, (at + 1 + w).min(n)),
                (at.saturating_sub(w), at),
            ];
            for (lo, hi) in ranges.into_iter().filter(|(lo, hi)| hi - lo >= w) {
                let want = lo <= at && at < hi;
                let s = l.slice(lo, hi);
                assert!(s.is_view() || hi - lo == n, "{lo}..{hi} is a view");
                let got = Value::List(Rc::new(s)).holds_mutable();
                assert_eq!(got, want, "channel at {at}, slice {lo}..{hi}");
            }
        }
    });
}

// --- L4 final review: no recursion, shared parts once ---

/// Runs `f` on a 1 MB thread: a recursive walk or drop of a deep value
/// overflows it.
fn on_small_stack(f: fn()) {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || {
            f();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn a_deep_tuple_is_checked_without_recursion() {
    on_small_stack(|| {
        for leaf in [Value::Int(1), ch()] {
            let want = leaf.holds_mutable();
            let mut v = leaf;
            for _ in 0..1_000_000 {
                v = Value::tuple(vec![v]);
            }
            assert_eq!(v.holds_mutable(), want);
        }
    });
}

/// `[x, x]` sixty levels deep has 2^60 paths and 61 distinct nodes: each
/// shared node is looked at once.
#[test]
fn shared_parts_are_checked_once() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut got = Vec::new();
        for leaf in [Value::Int(1), Value::str("s")] {
            let mut v = leaf;
            for i in 0..60 {
                v = if i % 2 == 0 {
                    Value::List(Rc::new(List::new(vec![v.clone(), v])))
                } else {
                    Value::tuple(vec![v.clone(), v])
                };
            }
            got.push(v.holds_mutable());
        }
        let _ = tx.send(got);
    });
    let got = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("did not end within 10s");
    assert_eq!(got, [false, false]);
}

/// A million channels, each buffering the receiving end (or both ends)
/// of the one before, freed on a small stack.
#[test]
fn a_deep_chain_of_channel_ends_frees_on_a_small_stack() {
    on_small_stack(|| {
        for both in [false, true] {
            let mut prev = Value::Nil;
            for _ in 0..1_000_000 {
                let (tx, rx) = crate::vm::channel::pair(1);
                let tx = Value::Sender(Rc::new(tx));
                if both {
                    rx.0.put(Value::tuple(vec![tx, prev]));
                } else {
                    rx.0.put(prev);
                    drop(tx);
                }
                prev = Value::Receiver(Rc::new(rx));
            }
            drop(prev);
        }
    });
}
