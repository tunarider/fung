//! Conversions (Go `prelude/convert.go`). `int`, `float`, `str` and `sym`
//! assert ("this must be a number": a runtime error when it is not);
//! `parse_int`, `parse_float` and `parse_dur` check and return Ok/Err.

use fung_front::diag::Arg;
use fung_front::lexer::{Kind, lex};
use fung_front::source::FileId;

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

/// An optional sign, digits and `_` separators (01), like Go's
/// `strconv.ParseInt(s, 10, 64)` after the separators are removed.
fn parse_int(s: &str) -> Option<i64> {
    if !is_space_free(s) || !well_placed_underscores(s) {
        return None;
    }
    s.replace('_', "").parse().ok()
}

/// Go's `strconv.ParseFloat(s, 64)` for finite results: decimal forms and
/// hexadecimal ones with a `p` exponent; infinities, NaN and overflow are
/// rejected.
fn parse_float(s: &str) -> Option<f64> {
    if !is_space_free(s) || !well_placed_underscores(s) {
        return None;
    }
    let s = s.replace('_', "");
    let f = parse_hex_float(&s).or_else(|| {
        // Rust also reads "inf", "infinity" and "nan"; they are not finite.
        let body = s.trim_start_matches(['+', '-']);
        if body.starts_with(|c: char| c.is_ascii_digit() || c == '.') {
            s.parse::<f64>().ok()
        } else {
            None
        }
    })?;
    f.is_finite().then_some(f)
}

/// `[+-]0x<hex>[.<hex>]p[+-]<dec>` (Go's hexadecimal floats; the exponent
/// is required). Mantissas beyond 64 bits keep their leading bits.
fn parse_hex_float(s: &str) -> Option<f64> {
    let (neg, body) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let body = body
        .strip_prefix("0x")
        .or_else(|| body.strip_prefix("0X"))?;
    let (mant, exp) = body.split_once(['p', 'P'])?;
    let (int_part, frac_part) = mant.split_once('.').unwrap_or((mant, ""));
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    let mut m: u64 = 0;
    let mut shift: i64 = 0; // binary exponent adjustment
    let mut sticky = false;
    for (i, c) in int_part.chars().chain(frac_part.chars()).enumerate() {
        let d = c.to_digit(16)? as u64;
        if m >> 60 == 0 {
            m = m << 4 | d;
            if i >= int_part.len() {
                shift -= 4;
            }
        } else {
            sticky |= d != 0;
            if i < int_part.len() {
                shift += 4;
            }
        }
    }
    let exp_digits = exp.trim_start_matches(['+', '-']);
    if exp_digits.is_empty() || !exp_digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let e: i64 = exp_digits.parse().unwrap_or(i64::MAX / 4).min(100_000);
    let e = if exp.starts_with('-') { -e } else { e };
    if sticky {
        m |= 1; // keeps rounding correct past the kept bits
    }
    let mut f = m as f64;
    let mut k = e + shift;
    while k > 0 {
        let step = k.min(1000);
        f *= 2f64.powi(step as i32);
        k -= step;
    }
    while k < 0 {
        let step = (-k).min(1000);
        f /= 2f64.powi(step as i32);
        k += step;
    }
    Some(if neg { -f } else { f })
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

/// Go `value.IsName`: a lowercase letter or `_`, then letters, digits and
/// `_`. Unicode classes are approximated as in the lexer.
fn is_name(s: &str) -> bool {
    let mut cs = s.chars();
    let Some(first) = cs.next() else {
        return false;
    };
    (first == '_' || (first.is_alphabetic() && !first.is_uppercase()))
        && cs.all(|c| c == '_' || c.is_alphabetic() || c.is_numeric())
}

fn sym_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    match &a[0] {
        Value::Str(s) if is_name(s) => Ok(Value::Sym(intern(s))),
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
