//! Collection operations shared by the VM and the builtins: `len`,
//! `x[i]`, `x.name`, `x[a..b]` and `in`. Every bound is checked here, so
//! no user program can reach a panicking slice or index.

use std::rc::Rc;

use fung_front::diag::{Arg, Diag, error};

use super::arith::{no_pos, overflow};
use super::{List, Value, type_name};

fn err(code: &str, args: Vec<Arg>) -> Diag {
    error(code, no_pos(), args)
}

fn str_arg(s: &str) -> Arg {
    Arg::Str(s.to_string())
}

/// E0501 for an operation on two types.
fn type_error(verb: &str, a: &Value, b: &Value) -> Diag {
    err(
        "E0501",
        vec![
            str_arg(verb),
            Arg::Str(type_name(a)),
            Arg::Str(type_name(b)),
        ],
    )
}

/// E0507 for `what` (an index, or a slice's start or end) at `i`.
fn out_of_range(what: &str, i: i64, n: usize) -> Diag {
    err(
        "E0507",
        vec![str_arg(what), Arg::Int(i), Arg::Int(n as i64)],
    )
}

/// Runes of a string or elements of a collection (02).
pub fn len(x: &Value) -> Result<i64, Diag> {
    Ok(match x {
        Value::Str(s) => s.chars().count() as i64,
        Value::List(l) => l.len() as i64,
        Value::Tuple(t) => t.len() as i64,
        Value::Map(m) => m.len() as i64,
        Value::Range(r) => i64::try_from(r.len()).map_err(|_| overflow())?,
        _ => {
            return Err(err(
                "E0506",
                vec![str_arg("take the length of"), Arg::Str(type_name(x))],
            ));
        }
    })
}

/// A possibly negative index as a position, or E0507 naming `i`.
fn norm_index(i: i64, n: usize) -> Result<usize, Diag> {
    let idx = if i < 0 { i + n as i64 } else { i };
    if idx < 0 || idx >= n as i64 {
        return Err(out_of_range("index", i, n));
    }
    Ok(idx as usize)
}

/// `x[i]` (02): lists and strings by position (negative from the end),
/// maps by string key, modules by member name (04: a private name is
/// E0304, a missing one E0303). Out of range is an error.
pub fn index(x: &Value, i: &Value) -> Result<Value, Diag> {
    match x {
        Value::List(l) => {
            let Value::Int(n) = i else {
                return Err(type_error("index", x, i));
            };
            let idx = norm_index(*n, l.len())?;
            Ok(l.at(idx).expect("index checked against the length").clone())
        }
        Value::Str(s) => {
            let Value::Int(n) = i else {
                return Err(type_error("index", x, i));
            };
            let idx = norm_index(*n, char_len(s))?;
            let at = byte_at(s, idx);
            let ch = s[at..]
                .chars()
                .next()
                .expect("index checked against the length");
            Ok(Value::Str(Rc::new(ch.to_string())))
        }
        Value::Map(m) => {
            let Value::Str(k) = i else {
                return Err(type_error("index", x, i));
            };
            match m.get(k) {
                Some(v) => Ok(v.clone()),
                None => Err(err("E0508", vec![Arg::Str(k.to_string())])),
            }
        }
        Value::Module(m) => {
            let Value::Str(k) = i else {
                return Err(type_error("index", x, i));
            };
            // A member is a name: found in NFC however it was written (01).
            let k = &*fung_front::unicode::nfc(k);
            if k.starts_with('_') {
                return Err(err(
                    "E0304",
                    vec![Arg::Str(k.to_string()), Arg::Str(m.name.clone())],
                ));
            }
            match m.get(k) {
                Some(v) => Ok(v.clone()),
                None => Err(err(
                    "E0303",
                    vec![Arg::Str(m.name.clone()), Arg::Str(k.to_string())],
                )),
            }
        }
        _ => Err(err("E0509", vec![Arg::Str(type_name(x))])),
    }
}

/// `x.name`: an index by the name as a string key.
pub fn field(x: &Value, name: &str) -> Result<Value, Diag> {
    index(x, &Value::str(name))
}

/// One end of a slice: `None` is open (`SLICE` marks a missing end),
/// otherwise an int. nil is not an open end: it is an error like any
/// other use of nil as an index (02).
fn bound(v: Option<&Value>) -> Result<Option<i64>, Diag> {
    match v {
        None => Ok(None),
        Some(Value::Int(n)) => Ok(Some(*n)),
        Some(other) => Err(err(
            "E0501",
            vec![
                str_arg("slice with"),
                str_arg("int"),
                Arg::Str(type_name(other)),
            ],
        )),
    }
}

/// Resolves the ends against length `n`. Both ends may equal `n`; start
/// may not exceed end (E0516). A negative end counts from the end, and an
/// inclusive end takes one more. An end out of range is reported as the
/// program wrote it (counted in i128, so nothing wraps).
fn slice_bounds(
    start: Option<&Value>,
    end: Option<&Value>,
    n: usize,
    inclusive: bool,
) -> Result<(usize, usize), Diag> {
    let len = n as i128;
    let resolve = |i: i64| -> i128 {
        let i = i128::from(i);
        if i < 0 { i + len } else { i }
    };
    let start = bound(start)?;
    let end = bound(end)?;
    let lo = start.map_or(0, resolve);
    let hi = end.map_or(len, |e| resolve(e) + i128::from(inclusive));
    let written = |b: Option<i64>, dflt: i128| b.unwrap_or(dflt as i64);
    if !(0..=len).contains(&lo) {
        return Err(out_of_range("slice start", written(start, 0), n));
    }
    if !(0..=len).contains(&hi) {
        return Err(out_of_range("slice end", written(end, len), n));
    }
    if lo > hi {
        // Both ends were written: an open one cannot cross the other.
        return Err(err(
            "E0516",
            vec![
                Arg::Int(written(start, 0)),
                Arg::Int(written(end, len)),
                Arg::Int(n as i64),
            ],
        ));
    }
    Ok((lo as usize, hi as usize))
}

/// `x[a..b]` and `x[a..=b]` on lists and strings (02). A missing end is
/// open.
pub fn slice(
    x: &Value,
    start: Option<&Value>,
    end: Option<&Value>,
    inclusive: bool,
) -> Result<Value, Diag> {
    match x {
        Value::List(l) => {
            let (lo, hi) = slice_bounds(start, end, l.len(), inclusive)?;
            if lo == 0 && hi == l.len() {
                return Ok(x.clone());
            }
            Ok(Value::List(Rc::new(List::slice(l, lo, hi))))
        }
        Value::Str(s) => {
            let (lo, hi) = slice_bounds(start, end, char_len(s), inclusive)?;
            let from = byte_at(s, lo);
            let to = from + byte_at(&s[from..], hi - lo);
            Ok(Value::Str(Rc::new(s[from..to].to_string())))
        }
        _ => Err(err("E0511", vec![Arg::Str(type_name(x))])),
    }
}

/// `x in c` (07): list elements, substrings, map keys, integers of a range.
/// A range holds a float that `==` one of its ints: a whole float within
/// the int range (02).
pub fn contains(c: &Value, x: &Value) -> Result<bool, Diag> {
    match c {
        Value::List(l) => {
            for item in l.iter() {
                if super::eq(item, x)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Value::Str(s) => match x {
            Value::Str(sub) => Ok(s.contains(sub.as_str())),
            _ => Err(type_error("search", c, x)),
        },
        Value::Map(m) => match x {
            Value::Str(k) => Ok(m.get(k).is_some()),
            _ => Err(type_error("search", c, x)),
        },
        Value::Range(r) => match x {
            Value::Int(i) => Ok(r.contains(*i)),
            Value::Float(f) => Ok(whole_int(*f).is_some_and(|i| r.contains(i))),
            _ => Ok(false),
        },
        _ => Err(err("E0512", vec![Arg::Str(type_name(c))])),
    }
}

/// The int a float equals exactly, if any: no fraction and within
/// `[-2^63, 2^63)`.
fn whole_int(f: f64) -> Option<i64> {
    let in_range = (-9223372036854775808.0..9223372036854775808.0).contains(&f);
    (in_range && f.fract() == 0.0).then_some(f as i64)
}

/// A string's length in characters (02), without a copy (P4). An ASCII
/// string is its byte count.
fn char_len(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.chars().count()
    }
}

/// The byte offset of character `i` of `s` (`s.len()` past the end).
fn byte_at(s: &str, i: usize) -> usize {
    if s.is_ascii() {
        return i.min(s.len());
    }
    s.char_indices().nth(i).map_or(s.len(), |(b, _)| b)
}
