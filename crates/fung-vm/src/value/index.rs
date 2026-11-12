//! Collection operations shared by the VM and the builtins: `len`,
//! `x[i]`, `x.name`, `x[a..b]` and `in`. Every bound is checked here, so
//! no user program can reach a panicking slice or index.

use std::rc::Rc;

use fung_front::diag::{Arg, Diag, error};

use super::arith::{no_pos, overflow};
use fung_front::text;

use super::{List, Text, Value, type_name};

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

/// E0509 for `x[i]` that has no meaning: one wording for every index,
/// whichever of the two is wrong (VA39).
fn index_error(x: &Value, i: &Value) -> Diag {
    err(
        "E0509",
        vec![Arg::Str(type_name(x)), Arg::Str(type_name(i))],
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
        Value::Str(s) => s.char_len() as i64,
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

/// E0514 for a map key that is not a string, wherever a map is looked
/// up or changed by key (SP21, D13). A symbol, the usual slip (JSON and
/// files give strings), is told how to write it.
pub fn not_a_key(k: &Value) -> Diag {
    let mut d = err("E0514", vec![Arg::Str(type_name(k))]);
    if let Value::Sym(s) = k {
        let name = s.name();
        d.help = Some(format!(
            "map keys are strings: write `\"{name}\"` for `'{name}`, or convert the key with `sym_name(k)`"
        ));
    }
    d
}

/// An index as a position, or E0507 naming `i`: positions count from
/// the start, so a negative one is out of range (D37).
fn norm_index(i: i64, n: usize) -> Result<usize, Diag> {
    match usize::try_from(i) {
        Ok(idx) if idx < n => Ok(idx),
        _ => Err(out_of_range("index", i, n)),
    }
}

/// `x[i]` (02): lists and strings by position (from the start, D37)
/// or by a range (a slice), maps by string key, modules by member name (04: a private name is
/// E0304, a missing one E0303). Out of range is an error.
pub fn index(x: &Value, i: &Value) -> Result<Value, Diag> {
    // A range value indexes a list or a string as the slice of the same
    // ends does, E0516 included (D7): `xs[r]` is `xs[a..b]`.
    if let (Value::List(_) | Value::Str(_), Value::Range(r)) = (x, i) {
        return slice(
            x,
            Some(&Value::Int(r.start)),
            Some(&Value::Int(r.end)),
            r.inclusive,
        );
    }
    match x {
        Value::List(l) => {
            let Value::Int(n) = i else {
                return Err(index_error(x, i));
            };
            let idx = norm_index(*n, l.len())?;
            Ok(l.at(idx).expect("index checked against the length").clone())
        }
        Value::Str(s) => {
            let Value::Int(n) = i else {
                return Err(index_error(x, i));
            };
            // Only the characters up to `n` are read; the length is
            // counted for the message alone (VA12).
            let at = usize::try_from(*n).ok().and_then(|i| {
                let at = s.offset_of(i);
                (at < s.len()).then_some(at)
            });
            let Some(at) = at else {
                return Err(out_of_range("index", *n, s.char_len()));
            };
            let ch = text::first(&s[at..]).expect("index checked against the length");
            Ok(Value::char_str(&s[at..at + ch.byte_len()]))
        }
        Value::Map(m) => {
            let Value::Str(k) = i else {
                return Err(not_a_key(i));
            };
            match m.get(&***k) {
                Some(v) => Ok(v.clone()),
                None => Err(err("E0508", vec![Arg::Str(key_text(k))])),
            }
        }
        Value::Module(m) => {
            let Value::Str(k) = i else {
                return Err(index_error(x, i));
            };
            // A member is a name: found in NFC however it was written (01).
            let Some(k) = k.as_str() else {
                return Err(err(
                    "E0303",
                    vec![Arg::Str(m.name.clone()), Arg::Str(key_text(k))],
                ));
            };
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
        _ => Err(index_error(x, i)),
    }
}

/// `x.name`: an index by the name as a string key.
pub fn field(x: &Value, name: &str) -> Result<Value, Diag> {
    let k = Value::str(name);
    index(x, &k).map_err(|d| field_help(d, x, &k))
}

/// The help of `x.name`'s E0509: only a map and a module have names, so
/// `xs.len` meant a call of the prelude's `len` (VA39).
pub fn field_help(d: Diag, x: &Value, name: &Value) -> Diag {
    let (true, Value::Str(k)) = (d.code == "E0509", name) else {
        return d;
    };
    let Some(name) = k.as_str() else {
        return d;
    };
    if fung_front::builtins::builtins()
        .prelude_signature(name)
        .is_none()
    {
        return d;
    }
    let t = type_name(x);
    d.with_help(&format!(
        "a {t} has no fields; to call `{name}` on it, write `{name}(…)`"
    ))
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
/// may not exceed end (E0516). Ends count from the start, so a negative
/// one is out of range (D37), and an inclusive end takes one more. An end
/// out of range is reported as the program wrote it (counted in i128, so
/// nothing wraps).
fn slice_bounds(
    start: Option<&Value>,
    end: Option<&Value>,
    n: usize,
    inclusive: bool,
) -> Result<(usize, usize), Diag> {
    let len = n as i128;
    let resolve = i128::from;
    let start = bound(start)?;
    let end = bound(end)?;
    let lo = start.map_or(0, resolve);
    let written = |b: Option<i64>, dflt: i128| b.unwrap_or(dflt as i64);
    if !(0..=len).contains(&lo) {
        return Err(out_of_range("slice start", written(start, 0), n));
    }
    // An inclusive end is an element's index (`0..len`), checked before
    // the one is added: `xs[..=3]` of three, or `[][..=0]`, is out of
    // range rather than empty (VA2).
    let hi = match end {
        Some(e) if inclusive => {
            let i = resolve(e);
            if !(0..len).contains(&i) {
                return Err(out_of_range("slice end", e, n));
            }
            i + 1
        }
        _ => end.map_or(len, resolve),
    };
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
            let (lo, hi) = slice_bounds(start, end, s.char_len(), inclusive)?;
            let from = s.offset_of(lo);
            let to = s.byte_at(from, hi - lo);
            if from == 0 && to == s.len() {
                return Ok(x.clone());
            }
            // A slice of valid UTF-8 at character boundaries is valid
            // UTF-8, ASCII if the whole is: not looked at again (VA32).
            Ok(match s.as_str() {
                Some(t) if s.is_ascii() => {
                    Value::Str(Rc::new(Text::ascii(t[from..to].to_string(), true)))
                }
                Some(t) => Value::Str(Rc::new(Text::utf8(t[from..to].to_string()))),
                None => Value::bytes(s[from..to].to_vec()),
            })
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
            Value::Str(sub) => Ok(find(s, sub).is_some()),
            _ => Err(type_error("search", c, x)),
        },
        Value::Map(m) => match x {
            Value::Str(k) => Ok(m.get(&***k).is_some()),
            _ => Err(not_a_key(x)),
        },
        Value::Range(r) => match x {
            Value::Int(i) => Ok(r.contains(*i)),
            Value::Float(f) => Ok(whole_int(*f).is_some_and(|i| r.contains(i))),
            Value::Nil => Ok(false),
            // A range holds ints: another type is `==`'s error (D40).
            _ => {
                super::compare::check_element(x, &Value::Int(r.start))?;
                Ok(false)
            }
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

/// A map key in a message: its literal form when it is not text.
fn key_text(k: &super::Text) -> String {
    match k.as_str() {
        Some(k) => k.to_string(),
        None => fung_front::consts::quote_bytes(k),
    }
}

/// Where `needle` first occurs in `hay`, by bytes; the empty needle is at 0.
pub fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    memchr::memmem::find(hay, needle)
}
