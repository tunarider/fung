//! The prelude's string functions (Go `prelude/strings.go`). Strings are
//! sequences of runes (02); case mapping is rune by rune like Go's.

use std::rc::Rc;

use fung_front::diag::Arg;

use crate::native::nat::{self, def, fail, opt, param};
use crate::value::{Lit, Value, format_for_print};
use crate::vm::{Stop, Task};

pub(super) fn add(add: &mut impl FnMut(Value)) {
    add(def("split", split_fn, vec![param("s"), param("sep")]));
    add(def(
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

/// Go `strings.Split`: an empty separator splits into runes, and splitting
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

/// `join(xs, sep = "")` renders elements as print would (06): strings
/// bare, the rest in literal form.
fn join_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    nat::check_items(&a[0], "join")?;
    let sep = nat::need_str("sep", &a[1])?;
    let mut out = String::new();
    let mut first = true;
    nat::each(t, &a[0], "join", |_, x| {
        if !first {
            out.push_str(sep);
        }
        first = false;
        match &x {
            Value::Str(s) => out.push_str(s),
            _ => out.push_str(&format_for_print(&x)),
        }
        Ok(())
    })?;
    Ok(string(out))
}

/// Go `unicode.IsSpace`: the Unicode White_Space property, which Rust's
/// `char::is_whitespace` follows too.
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

/// Go `unicode.ToUpper` of one rune: the simple (one-rune) mapping. Rust
/// gives the full mapping; where that has several runes the simple one is
/// the rune itself, except for the Greek letters with ypogegrammeni,
/// whose simple uppercase is the titlecase form.
fn upper_rune(c: char) -> char {
    let mut up = c.to_uppercase();
    match (up.next(), up.next()) {
        (Some(u), None) => u,
        _ => match c as u32 {
            0x1F80..=0x1F87 | 0x1F90..=0x1F97 | 0x1FA0..=0x1FA7 => {
                char::from_u32(c as u32 + 8).unwrap_or(c)
            }
            0x1FB3 => '\u{1FBC}',
            0x1FC3 => '\u{1FCC}',
            0x1FF3 => '\u{1FFC}',
            _ => c,
        },
    }
}

/// Go `unicode.ToLower` of one rune. Only U+0130 has a multi-rune full
/// lowercase; its simple mapping is `i`.
fn lower_rune(c: char) -> char {
    let mut low = c.to_lowercase();
    match (low.next(), low.next()) {
        (Some(l), None) => l,
        _ if c == '\u{130}' => 'i',
        _ => c,
    }
}

fn upper_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    Ok(string(s.chars().map(upper_rune).collect()))
}

fn lower_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    Ok(string(s.chars().map(lower_rune).collect()))
}

/// Go `strings.ReplaceAll`; an empty `old` matches before every rune and
/// at the end, as in Rust.
fn replace_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let [s, old, new] = nat::str_args(&a, ["s", "old", "new"])?;
    Ok(string(s.replace(old, new)))
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

/// `repeat(s, n)`; a negative `n` is 0. Like Go, the rune count times `n`
/// wraps on overflow, and a byte length past the int range is the
/// internal error Go's `strings.Repeat` panics with.
fn repeat_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    let n = nat::need_int("n", &a[1])?.max(0);
    if n > 0 && (s.chars().count() as i64).wrapping_mul(n) > MAX_CHARS {
        return Err(fail(
            "E0806",
            vec![nat::s("characters"), Arg::Int(MAX_CHARS)],
        ));
    }
    if (s.len() as i64).checked_mul(n).is_none() {
        return Err(fail(
            "E0801",
            vec![nat::s("strings: Repeat output length overflow")],
        ));
    }
    Ok(string(s.repeat(n as usize)))
}
