//! Conversions. `int`, `float`, `str` and `sym`
//! assert ("this must be a number": a runtime error when it is not);
//! `parse_int`, `parse_float` and `parse_dur` check and return Ok/Err.

use fung_front::diag::Arg;
use fung_front::lexer::{Kind, lex};
use fung_front::source::FileId;
use fung_front::unicode::{is_name_continue, is_name_start, is_upper, nfc};

use crate::native::nat::{self, def, fail, param};
use crate::value::{Value, format, intern};
use crate::vm::{Stop, Task};

pub(super) fn add(add: &mut impl FnMut(Value)) {
    add(def("int", int_fn, vec![param("x")]));
    add(def("float", float_fn, vec![param("x")]));
    add(def("str", str_fn, vec![param("x")]));
    add(def("sym", sym_fn, vec![param("x")]));
    add(def("parse_int", parse_int_fn, vec![param("s")]));
    add(def("parse_float", parse_float_fn, vec![param("s")]));
    add(def("parse_dur", parse_dur_fn, vec![param("s")]));
}

fn cannot_convert(v: &Value, to: &str) -> Stop {
    fail("E0513", vec![Arg::Str(format(v)), nat::s(to)])
}

/// Whether `s` is non-empty with no whitespace at either end (conversions
/// are strict: `int(" 1")` is an error).
fn is_space_free(s: &str) -> bool {
    match (s.chars().next(), s.chars().next_back()) {
        (Some(a), Some(b)) => !a.is_whitespace() && !b.is_whitespace(),
        _ => false,
    }
}

/// Whether every `_` sits between two ASCII digits (01: 숫자 사이에만).
fn well_placed_underscores(s: &str) -> bool {
    let rs: Vec<char> = s.chars().collect();
    rs.iter().enumerate().all(|(i, &r)| {
        r != '_'
            || (i > 0
                && i + 1 < rs.len()
                && rs[i - 1].is_ascii_digit()
                && rs[i + 1].is_ascii_digit())
    })
}

/// An optional sign, then decimal digits with `_` separators (01).
fn parse_int(s: &str) -> Option<i64> {
    if !is_space_free(s) || !well_placed_underscores(s) {
        return None;
    }
    s.replace('_', "").parse().ok()
}

/// The float literal syntax (01) with an optional sign: digits with `_`
/// only between digits, an optional fraction with digits on both sides of
/// the point, an optional exponent. An integer (`"42"`) is accepted; hex
/// floats, `.5`, `1.`, `inf` and `nan` are not, and the result must be
/// finite.
fn parse_float(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    // Digits with single `_` separators; false when there is none.
    let digits = |i: &mut usize| {
        let start = *i;
        while *i < b.len() {
            match b[*i] {
                b'0'..=b'9' => *i += 1,
                b'_' if *i > start && b.get(*i + 1).is_some_and(u8::is_ascii_digit) => *i += 1,
                _ => break,
            }
        }
        *i > start
    };
    if !digits(&mut i) {
        return None;
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        if !digits(&mut i) {
            return None;
        }
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        if !digits(&mut i) {
            return None;
        }
    }
    if i != b.len() {
        return None;
    }
    let f: f64 = s.replace('_', "").parse().ok()?;
    f.is_finite().then_some(f)
}

fn int_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    match &a[0] {
        Value::Int(n) => return Ok(Value::Int(*n)),
        Value::Float(f) => {
            // 2^63 itself is out of range; -2^63 is in.
            if f.is_finite()
                && *f < 9_223_372_036_854_775_808.0
                && *f >= -9_223_372_036_854_775_808.0
            {
                return Ok(Value::Int(*f as i64));
            }
        }
        Value::Str(s) => {
            if let Some(n) = parse_int(s) {
                return Ok(Value::Int(n));
            }
        }
        _ => {}
    }
    Err(cannot_convert(&a[0], "int"))
}

fn float_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    match &a[0] {
        Value::Int(n) => return Ok(Value::Float(*n as f64)),
        Value::Float(f) => return Ok(Value::Float(*f)),
        Value::Str(s) => {
            if let Some(f) = parse_float(s) {
                return Ok(Value::Float(f));
            }
        }
        _ => {}
    }
    Err(cannot_convert(&a[0], "float"))
}

fn str_fn(_: &mut Task<'_>, mut a: Vec<Value>) -> Result<Value, Stop> {
    match a.swap_remove(0) {
        s @ Value::Str(_) => Ok(s),
        v => Ok(Value::str(&format(&v))),
    }
}

/// A name (01): `_` or a name-start character that is not uppercase,
/// then name-continue characters (UAX #31).
fn is_name(s: &str) -> bool {
    let mut cs = s.chars();
    let Some(first) = cs.next() else {
        return false;
    };
    (first == '_' || (is_name_start(first) && !is_upper(first))) && cs.all(is_name_continue)
}

fn sym_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    match &a[0] {
        Value::Str(s) if is_name(&nfc(s)) => Ok(Value::Sym(intern(&nfc(s)))),
        Value::Sym(x) => Ok(Value::Sym(*x)),
        v => Err(cannot_convert(v, "sym")),
    }
}

fn parse_int_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    Ok(match parse_int(s) {
        Some(n) => nat::ok(Value::Int(n)),
        None => nat::err_value("parse", &format!("not an integer: {s}"), Vec::new()),
    })
}

fn parse_float_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    Ok(match parse_float(s) {
        Some(f) => nat::ok(Value::Float(f)),
        None => nat::err_value("parse", &format!("not a number: {s}"), Vec::new()),
    })
}

/// `parse_dur(s)` reads a duration in the literal syntax (01: 1h30m,
/// 500ms, -5s), the way `parse_int` reads an int: Ok/Err for untrusted
/// text.
fn parse_dur_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    Ok(match parse_dur(s.trim_matches(char::is_whitespace)) {
        Some(d) => nat::ok(Value::Dur(d)),
        None => nat::err_value("parse", &format!("not a duration: {s}"), Vec::new()),
    })
}

/// Exactly one duration literal, optionally negated.
fn parse_dur(s: &str) -> Option<i64> {
    let (neg, s) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let (toks, _) = lex(FileId(0), s).ok()?;
    let first = toks.first().filter(|t| t.kind == Kind::Duration)?;
    if toks[1..]
        .iter()
        .any(|t| t.kind != Kind::Eof && t.kind != Kind::Newline)
    {
        return None;
    }
    Some(if neg {
        first.dur.wrapping_neg()
    } else {
        first.dur
    })
}

#[cfg(test)]
mod tests {
    use super::parse_float;

    #[test]
    fn float_literal_syntax() {
        for (s, f) in [
            ("3.25", 3.25),
            ("-2.5e-3", -2.5e-3),
            ("+1e9", 1e9),
            ("42", 42.0),
            ("1_000.5", 1000.5),
            ("1E+2", 100.0),
        ] {
            assert_eq!(parse_float(s), Some(f), "{s:?}");
        }
        for s in [
            "0x1p4", ".5", "1.", "1e", "1__0", "_1", "1_", "inf", "nan", "+-1", " 1", "1e400", "",
            "-", "1._5", "1e_5", "١",
        ] {
            assert_eq!(parse_float(s), None, "{s:?}");
        }
    }
}
