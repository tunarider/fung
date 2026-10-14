//! Collection operations shared by the VM and the builtins: `len`,
//! `x[i]`, `x.name`, `x[a..b]` and `in`. Every bound is checked here, so
//! no user program can reach a panicking slice or index.

use std::rc::Rc;

use fung_front::diag::{Arg, Diag, error};

use super::arith::no_pos;
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

fn out_of_range(i: i64, n: usize) -> Diag {
    err("E0507", vec![Arg::Int(i), Arg::Int(n as i64)])
}

/// Runes of a string or elements of a collection (02).
pub fn len(x: &Value) -> Result<i64, Diag> {
    Ok(match x {
        Value::Str(s) => s.chars().count() as i64,
        Value::List(l) => l.len() as i64,
        Value::Tuple(t) => t.len() as i64,
        Value::Map(m) => m.len() as i64,
        Value::Range(r) => r.len(),
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
        return Err(out_of_range(i, n));
    }
    Ok(idx as usize)
}

/// `x[i]` (02): lists and strings by position (negative from the end),
/// maps by string key. Out of range is an error.
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
            let rs: Vec<char> = s.chars().collect();
            let idx = norm_index(*n, rs.len())?;
            Ok(Value::Str(Rc::new(rs[idx].to_string())))
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
        // Module members (E0303, E0304) arrive with the module task.
        _ => Err(err("E0509", vec![Arg::Str(type_name(x))])),
    }
}

/// `x.name`: an index by the name as a string key.
pub fn field(x: &Value, name: &str) -> Result<Value, Diag> {
    index(x, &Value::str(name))
}

/// One end of a slice: `None` or nil is open, otherwise an int.
fn bound(v: Option<&Value>) -> Result<Option<i64>, Diag> {
    match v {
        None | Some(Value::Nil) => Ok(None),
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
/// may not exceed end. An inclusive end is normalized first, then extended
/// by one. The arithmetic wraps like Go's `int`.
fn slice_bounds(
    start: Option<&Value>,
    end: Option<&Value>,
    n: usize,
    inclusive: bool,
) -> Result<(usize, usize), Diag> {
    let n64 = n as i64;
    let lo = match bound(start)? {
        Some(s) if s < 0 => s.wrapping_add(n64),
        Some(s) => s,
        None => 0,
    };
    let hi = match bound(end)? {
        Some(e) => {
            let e = if e < 0 { e.wrapping_add(n64) } else { e };
            if inclusive { e.wrapping_add(1) } else { e }
        }
        None => n64,
    };
    if lo < 0 || lo > n64 {
        return Err(out_of_range(lo, n));
    }
    if hi < 0 || hi > n64 {
        return Err(out_of_range(hi, n));
    }
    if lo > hi {
        return Err(out_of_range(lo, n));
    }
    Ok((lo as usize, hi as usize))
}

/// `x[a..b]` and `x[a..=b]` on lists and strings (02). A missing end (or
/// nil) is open.
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
            let rs: Vec<char> = s.chars().collect();
            let (lo, hi) = slice_bounds(start, end, rs.len(), inclusive)?;
            Ok(Value::Str(Rc::new(rs[lo..hi].iter().collect())))
        }
        _ => Err(err("E0511", vec![Arg::Str(type_name(x))])),
    }
}

/// `x in c` (07): list elements, substrings, map keys, integers of a range.
pub fn contains(c: &Value, x: &Value) -> Result<bool, Diag> {
    match c {
        Value::List(l) => Ok(l.iter().any(|item| super::equal(item, x))),
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
            _ => Ok(false),
        },
        _ => Err(err("E0512", vec![Arg::Str(type_name(c))])),
    }
}
