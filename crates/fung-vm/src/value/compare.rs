use std::cmp::Ordering;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

use fung_front::diag::Diag;

use super::Value;
use super::arith::compare_error;
use super::format::{Items, is_collection};

/// `==`: numbers compare by value across int/float (exactly), same types by
/// value, reference types by identity, different types are never equal.
/// Collections are walked pair by pair with an explicit stack (L9): the
/// Rust stack does not grow with the values' depth. The walk stops at
/// the first difference.
pub fn equal(a: &Value, b: &Value) -> bool {
    if !is_collection(a) || !is_collection(b) {
        return equal_leaf(a, b);
    }
    let mut open: Vec<Pairs<'_>> = Vec::new();
    let mut next = Some((a, b));
    loop {
        if let Some((a, b)) = next.take()
            && !same_object(a, b)
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
/// `sym_meets_str`): two lists, tuples or tags item by item; two maps by
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

/// `==` and `!=` as programs see them (02): `equal`, except that a symbol
/// meeting a string anywhere the comparison looks (inside lists, tuples,
/// tags and maps too) is E0517: never equal, and almost always a value
/// that came back from JSON or a file (L10). Patterns keep `equal`.
pub fn eq(a: &Value, b: &Value) -> Result<bool, Diag> {
    sym_meets_str(a, b)?;
    Ok(equal(a, b))
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

/// E0517 where `equal` would compare a symbol with a string: the first
/// such pair `equal` would meet, walked as `equal` walks (L9). Items of
/// collections whose shapes differ are not looked at (`equal` does not
/// compare them either).
fn sym_meets_str(a: &Value, b: &Value) -> Result<(), Diag> {
    if (!is_collection(a) || !is_collection(b))
        && !matches!(
            (a, b),
            (Value::Sym(_), Value::Str(_)) | (Value::Str(_), Value::Sym(_))
        )
    {
        return Ok(());
    }
    let mut open: Vec<Pairs<'_>> = Vec::new();
    let mut next = Some((a, b));
    loop {
        if let Some((a, b)) = next.take()
            && !same_object(a, b)
        {
            match kids(a, b) {
                Kids::Pairs(p) => open.push(p),
                Kids::Mismatch => {}
                Kids::None => {
                    if let (Value::Sym(_), Value::Str(_)) | (Value::Str(_), Value::Sym(_)) = (a, b)
                    {
                        return Err(fung_front::diag::error(
                            "E0517",
                            super::arith::no_pos(),
                            vec![
                                fung_front::diag::Arg::Str(super::format(a)),
                                fung_front::diag::Arg::Str(super::format(b)),
                            ],
                        ));
                    }
                }
            }
        }
        let Some(top) = open.last_mut() else {
            return Ok(());
        };
        match top.next() {
            Some((x, Some(y))) => next = Some((x, y)),
            Some((_, None)) => {}
            None => {
                open.pop();
            }
        }
    }
}

/// `< > <= >=` for numbers, strings (code point order), durations, times, lists
/// and tuples, lexicographic (a shorter prefix is smaller). Anything else
/// is E0501, from the first pair of items that do not compare, naming
/// their types. Lists and tuples are walked with an explicit stack (L9).
pub fn compare(a: &Value, b: &Value) -> Result<Ordering, Diag> {
    if !matches!(
        (a, b),
        (Value::List(_), Value::List(_)) | (Value::Tuple(_), Value::Tuple(_))
    ) {
        return compare_leaf(a, b);
    }
    // Each open pair: what is left of both, and their lengths.
    let mut open: Vec<(Items<'_>, Items<'_>, usize, usize)> = Vec::new();
    let mut next = Some((a, b));
    loop {
        if let Some((a, b)) = next.take() {
            match (a, b) {
                (Value::Tuple(x), Value::Tuple(y)) => open.push((
                    Items::Slice(x.iter()),
                    Items::Slice(y.iter()),
                    x.len(),
                    y.len(),
                )),
                (Value::List(x), Value::List(y)) => open.push((
                    Items::List(x.iter()),
                    Items::List(y.iter()),
                    x.len(),
                    y.len(),
                )),
                _ => match compare_leaf(a, b)? {
                    Ordering::Equal => {}
                    o => return Ok(o),
                },
            }
        }
        let Some((x, y, xn, yn)) = open.last_mut() else {
            return Ok(Ordering::Equal);
        };
        match (x.next(), y.next()) {
            (Some(p), Some(q)) => next = Some((p, q)),
            _ => {
                let o = (*xn).cmp(yn);
                open.pop();
                if o != Ordering::Equal {
                    return Ok(o);
                }
            }
        }
    }
}

/// `compare` on two values that are not two lists or two tuples.
fn compare_leaf(a: &Value, b: &Value) -> Result<Ordering, Diag> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Ok(x.cmp(y)),
        (Value::Int(x), Value::Float(y)) => Ok(cmp_int_float(*x, *y)),
        (Value::Float(x), Value::Int(y)) => Ok(cmp_int_float(*y, *x).reverse()),
        (Value::Float(x), Value::Float(y)) => Ok(cmp_floats(*x, *y)),
        // Rust compares UTF-8 bytes, which orders the same as code points.
        (Value::Str(x), Value::Str(y)) => Ok(x.as_str().cmp(y.as_str())),
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
/// A symbol hashes as the string of its name, so the two meet in one
/// bucket and `eq` reports them (E0517) as `in` does.
///
/// Walked with an explicit stack (L9). A map's entries are each hashed
/// on a hasher of their own and summed; meanwhile the hasher the map
/// belongs to waits in its frame.
pub fn hash_value(v: &Value, h: &mut DefaultHasher) {
    if !is_collection(v) {
        return hash_leaf(v, h);
    }
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
                v => hash_leaf(v, &mut cur),
            }
        }
        let Some(top) = open.last_mut() else {
            *h = cur;
            return;
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

/// `hash_value` of a value that holds no other.
fn hash_leaf(v: &Value, h: &mut DefaultHasher) {
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
        Value::Str(s) => (4u8, s.as_str()).hash(h),
        Value::Sym(s) => (4u8, s.name()).hash(h),
        Value::Dur(d) => (5u8, *d).hash(h),
        Value::Time(t) => (6u8, t.instant()).hash(h),
        Value::Range(r) => (10u8, r.start, r.end, r.inclusive).hash(h),
        Value::Closure(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::Native(x) => (12u8, std::ptr::from_ref(*x) as usize).hash(h),
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
}
