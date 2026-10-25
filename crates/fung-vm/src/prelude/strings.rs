//! The prelude's string functions. Strings are sequences of characters
//! (02); `upper` and `lower` use the full Unicode case mapping.

use std::rc::Rc;

use fung_front::diag::Arg;

use crate::native::nat::{self, def, def_step, fail, opt, param};
use crate::native::walk::{Collect, Cursor, drain_with};
use crate::value::{Lit, Value, format_for_print};
use crate::vm::{Flow, Stop, Task};

pub(super) fn add(add: &mut impl FnMut(Value)) {
    add(def("split", split_fn, vec![param("s"), param("sep")]));
    add(def("lines", lines_fn, vec![param("s")]));
    add(def_step(
        "join",
        join_fn,
        vec![param("xs"), opt("sep", Lit::Str(""))],
    ));
    add(def("trim", trim_fn, vec![param("s"), opt("cut", Lit::Nil)]));
    add(def("upper", upper_fn, vec![param("s")]));
    add(def("lower", lower_fn, vec![param("s")]));
    add(def(
        "replace",
        replace_fn,
        vec![param("s"), param("old"), param("new")],
    ));
    add(def(
        "starts_with",
        starts_with_fn,
        vec![param("s"), param("prefix")],
    ));
    add(def(
        "ends_with",
        ends_with_fn,
        vec![param("s"), param("suffix")],
    ));
    add(def("repeat", repeat_fn, vec![param("s"), param("n")]));
}

fn string(s: String) -> Value {
    Value::Str(Rc::new(s))
}

/// `split(s, sep)`: an empty separator splits into characters, and splitting
/// the empty string by it gives no parts.
fn split_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let [s, sep] = nat::str_args(&a, ["s", "sep"])?;
    let parts: Vec<Value> = if sep.is_empty() {
        let mut b = [0; 4];
        s.chars()
            .map(|c| Value::str(c.encode_utf8(&mut b)))
            .collect()
    } else {
        s.split(sep).map(Value::str).collect()
    };
    Ok(nat::list(parts))
}

/// `lines(s)`: `s` split on `\n`, one `\r` that ends a line dropped, and
/// no empty line after a final newline; the empty string has no lines (06,
/// the rule of `read_lines`).
fn lines_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    if s.is_empty() {
        return Ok(nat::list(Vec::new()));
    }
    let body = s.strip_suffix('\n').unwrap_or(s);
    let parts = body
        .split('\n')
        .map(|l| Value::str(l.strip_suffix('\r').unwrap_or(l)))
        .collect();
    Ok(nat::list(parts))
}

/// `join(xs, sep = "")` renders elements as print would (06): strings
/// bare, the rest in literal form.
fn join_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "join")?;
    let sep = nat::need_str("sep", &a[1])?.to_string();
    // Every element shows as one character at least.
    if let Value::Range(r) = &a[0]
        && r.len() > i128::from(MAX_CHARS)
    {
        return Err(too_many_chars());
    }
    struct Join {
        sep: String,
        out: String,
        chars: i64,
        first: bool,
    }
    impl Collect for Join {
        fn item(&mut self, x: Value) -> Result<bool, Stop> {
            let before = self.out.len();
            if !self.first {
                self.out.push_str(&self.sep);
            }
            self.first = false;
            match &x {
                Value::Str(s) => self.out.push_str(s),
                _ => self.out.push_str(&format_for_print(&x)),
            }
            self.chars += self.out[before..].chars().count() as i64;
            if self.chars > MAX_CHARS {
                return Err(too_many_chars());
            }
            Ok(false)
        }
        fn result(&mut self, _: &mut Task<'_>) -> Result<Flow, Stop> {
            Ok(Flow::Done(string(std::mem::take(&mut self.out))))
        }
    }
    let p = Join {
        sep,
        out: String::new(),
        chars: 0,
        first: true,
    };
    drain_with(t, Cursor::new(&a[0]), p)
}

/// Whitespace for `trim`: the Unicode White_Space property.
fn is_space(c: char) -> bool {
    c.is_whitespace()
}

/// `trim(s, cut = nil)`: whitespace, or the runes of `cut`, off both ends.
fn trim_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    if let Value::Nil = a[1] {
        return Ok(Value::str(s.trim_matches(is_space)));
    }
    let cut = nat::need_str("cut", &a[1])?;
    Ok(Value::str(s.trim_matches(|c| cut.contains(c))))
}

/// `upper(s)` and `lower(s)`: the full Unicode case mapping (06), so the
/// length may change (`ß` → `SS`) and a final sigma lowercases to `ς`.
fn upper_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    Ok(string(s.to_uppercase()))
}

fn lower_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    Ok(string(s.to_lowercase()))
}

/// `replace(s, old, new)`: every occurrence; an empty `old` matches before
/// every character and at the end. A result past `MAX_CHARS` characters is
/// E0806 before anything is built (02).
fn replace_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let [s, old, new] = nat::str_args(&a, ["s", "old", "new"])?;
    if !replace_fits(s, old, new) {
        return Err(too_many_chars());
    }
    Ok(string(s.replace(old, new)))
}

/// Whether `replace(s, old, new)` stays within `MAX_CHARS`. The size of
/// the result grows with the number of matches, so this counts them only
/// when the most there could be might pass the limit, and counts
/// characters only when the bytes do.
fn replace_fits(s: &str, old: &str, new: &str) -> bool {
    let limit = MAX_CHARS as i128;
    let (len, old_len, new_len) = (s.len() as i128, old.len() as i128, new.len() as i128);
    let most = if old.is_empty() {
        len + 1
    } else {
        len / old_len
    };
    if len + most * (new_len - old_len).max(0) <= limit {
        return true;
    }
    let n = s.matches(old).count() as i128;
    if len + n * (new_len - old_len) <= limit {
        return true;
    }
    let chars = |x: &str| x.chars().count() as i128;
    chars(s) + n * (chars(new) - chars(old)) <= limit
}

fn starts_with_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let [s, prefix] = nat::str_args(&a, ["s", "prefix"])?;
    Ok(Value::Bool(s.starts_with(prefix)))
}

fn ends_with_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let [s, suffix] = nat::str_args(&a, ["s", "suffix"])?;
    Ok(Value::Bool(s.ends_with(suffix)))
}

/// Bounds the strings `repeat` (and fung's `pad_*`) may build (E0806).
pub(crate) const MAX_CHARS: i64 = 100_000_000;

/// E0806 for a string past [`MAX_CHARS`].
pub(crate) fn too_many_chars() -> Stop {
    fail("E0806", vec![nat::s("characters"), Arg::Int(MAX_CHARS)])
}

/// `repeat(s, n)`; a negative `n` is 0. A result past `MAX_CHARS`
/// characters is E0806 (02).
fn repeat_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    let n = nat::need_int("n", &a[1])?.max(0);
    if (s.chars().count() as i64)
        .checked_mul(n)
        .is_none_or(|c| c > MAX_CHARS)
    {
        return Err(too_many_chars());
    }
    Ok(string(s.repeat(n as usize)))
}
