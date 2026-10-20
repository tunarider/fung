use std::rc::Rc;

use fung_front::diag::{Arg, Diag, error};
use fung_front::source::{FileId, Pos};

use super::dur::{add_dur, div_dur, scale_dur, sub_dur};
use super::time::{add_time_dur, sub_time_dur, sub_times};
use super::{Value, type_name};
use crate::prelude::MAX_CHARS;

/// A position-less location; the VM fills the real one in with `with_pos`.
pub(super) fn no_pos() -> Pos {
    Pos {
        file: FileId(0),
        line: 0,
        col: 0,
    }
}

pub(super) fn type_error(verb: &str, a: &str, b: &str) -> Diag {
    error(
        "E0501",
        no_pos(),
        vec![
            Arg::Str(verb.into()),
            Arg::Str(a.into()),
            Arg::Str(b.into()),
        ],
    )
}

pub(super) fn overflow() -> Diag {
    error("E0503", no_pos(), vec![])
}

pub(super) fn divzero() -> Diag {
    error("E0502", no_pos(), vec![])
}

pub(super) fn compare_error(a: &Value, b: &Value) -> Diag {
    type_error("compare", &type_name(a), &type_name(b))
}

/// Both ints stay ints, otherwise both become floats.
enum Num {
    Ints(i64, i64),
    Floats(f64, f64),
}

fn numeric(verb: &str, a: &Value, b: &Value) -> Result<Num, Diag> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Ok(Num::Ints(*x, *y)),
        (Value::Int(x), Value::Float(y)) => Ok(Num::Floats(*x as f64, *y)),
        (Value::Float(x), Value::Int(y)) => Ok(Num::Floats(*x, *y as f64)),
        (Value::Float(x), Value::Float(y)) => Ok(Num::Floats(*x, *y)),
        _ => Err(type_error(verb, &type_name(a), &type_name(b))),
    }
}

/// Rejects Inf and NaN: fung values have neither.
fn finite(f: f64) -> Result<Value, Diag> {
    if f.is_finite() {
        Ok(Value::Float(f))
    } else {
        Err(error("E0504", no_pos(), vec![]))
    }
}

/// E0806 for a string past the limit of `MAX_CHARS` characters (02).
fn too_many_chars() -> Diag {
    error(
        "E0806",
        no_pos(),
        vec![Arg::Str("characters".into()), Arg::Int(MAX_CHARS)],
    )
}

/// Whether strings of these parts together stay within the string limit.
/// Bytes bound the characters from above, so the characters are counted
/// only past it.
fn chars_fit(parts: &[&str]) -> bool {
    let fits = |n: usize| n as u64 <= MAX_CHARS as u64;
    fits(parts.iter().map(|p| p.len()).sum()) || fits(parts.iter().map(|p| p.chars().count()).sum())
}

/// E0806 for a string the VM built past the string limit (interpolation).
pub(crate) fn check_chars(s: &str) -> Result<(), Diag> {
    if chars_fit(&[s]) {
        Ok(())
    } else {
        Err(too_many_chars())
    }
}

pub fn add(a: &Value, b: &Value) -> Result<Value, Diag> {
    match (a, b) {
        (Value::Str(x), Value::Str(y)) => {
            if !chars_fit(&[x, y]) {
                return Err(too_many_chars());
            }
            let mut s = String::with_capacity(x.len() + y.len());
            s.push_str(x);
            s.push_str(y);
            return Ok(Value::Str(s.into()));
        }
        (Value::Dur(x), Value::Dur(y)) => return add_dur(*x, *y),
        (Value::Time(t), Value::Dur(d)) | (Value::Dur(d), Value::Time(t)) => {
            return add_time_dur(t, *d);
        }
        (Value::List(x), Value::List(y)) => {
            // An empty side returns the other operand itself.
            if y.is_empty() {
                return Ok(a.clone());
            }
            if x.is_empty() {
                return Ok(b.clone());
            }
            return Ok(Value::List(Rc::new(x.concat(y))));
        }
        (Value::Str(_) | Value::Dur(_) | Value::List(_) | Value::Time(_), _) => {
            return Err(type_error("add", &type_name(a), &type_name(b)));
        }
        _ => {}
    }
    match numeric("add", a, b)? {
        Num::Floats(x, y) => finite(x + y),
        Num::Ints(x, y) => x.checked_add(y).map(Value::Int).ok_or_else(overflow),
    }
}

pub fn sub(a: &Value, b: &Value) -> Result<Value, Diag> {
    if let Value::Time(x) = a {
        return match b {
            Value::Dur(y) => sub_time_dur(x, *y),
            Value::Time(y) => sub_times(x, y),
            _ => Err(type_error("subtract", &type_name(a), &type_name(b))),
        };
    }
    if let Value::Dur(x) = a {
        return match b {
            Value::Dur(y) => sub_dur(*x, *y),
            _ => Err(type_error("subtract", &type_name(a), &type_name(b))),
        };
    }
    match numeric("subtract", a, b)? {
        Num::Floats(x, y) => finite(x - y),
        Num::Ints(x, y) => x.checked_sub(y).map(Value::Int).ok_or_else(overflow),
    }
}

pub fn mul(a: &Value, b: &Value) -> Result<Value, Diag> {
    if let Value::Dur(d) = a {
        return scale_dur(*d, b);
    }
    if let Value::Dur(d) = b {
        return scale_dur(*d, a);
    }
    match numeric("multiply", a, b)? {
        Num::Floats(x, y) => finite(x * y),
        Num::Ints(x, y) => x.checked_mul(y).map(Value::Int).ok_or_else(overflow),
    }
}

/// `/`: always a float for numbers; a duration divided by a number stays a
/// duration, and by a duration is a float: the number in that unit (07).
pub fn div(a: &Value, b: &Value) -> Result<Value, Diag> {
    if let (Value::Dur(x), Value::Dur(y)) = (a, b) {
        return dur_ratio(*x, *y);
    }
    if let Value::Dur(d) = a {
        return div_dur(*d, b);
    }
    let (x, y) = match numeric("divide", a, b)? {
        Num::Ints(x, y) => (x as f64, y as f64),
        Num::Floats(x, y) => (x, y),
    };
    if y == 0.0 {
        return Err(divzero());
    }
    finite(x / y)
}

/// `dur / dur` (07): the whole quotient and the remainder's share apart,
/// so a quotient past 2^53 loses nothing to an early rounding of `x`.
fn dur_ratio(x: i64, y: i64) -> Result<Value, Diag> {
    if y == 0 {
        return Err(divzero());
    }
    let (x, y) = (i128::from(x), i128::from(y));
    finite((x / y) as f64 + (x % y) as f64 / y as f64)
}

/// `//` on ints, by the floor rule (07); `y` is not zero.
fn floor_div_i64(x: i64, y: i64) -> Result<i64, Diag> {
    if x == i64::MIN && y == -1 {
        return Err(overflow());
    }
    let mut q = x / y;
    if x % y != 0 && (x < 0) != (y < 0) {
        q -= 1;
    }
    Ok(q)
}

/// `%` on ints, paired with `floor_div_i64`; `y` is not zero.
fn rem_i64(x: i64, y: i64) -> i64 {
    if x == i64::MIN && y == -1 {
        return 0;
    }
    let mut r = x % y;
    if r != 0 && (r < 0) != (y < 0) {
        r += y;
    }
    r
}

/// `//`: floor division; ints stay ints, and a duration by a duration is
/// an int (07).
pub fn floor_div(a: &Value, b: &Value) -> Result<Value, Diag> {
    if let (Value::Dur(x), Value::Dur(y)) = (a, b) {
        if *y == 0 {
            return Err(divzero());
        }
        return floor_div_i64(*x, *y).map(Value::Int);
    }
    match numeric("divide", a, b)? {
        Num::Floats(x, y) => {
            if y == 0.0 {
                return Err(divzero());
            }
            finite(float_divmod(x, y).0)
        }
        Num::Ints(x, y) => {
            if y == 0 {
                return Err(divzero());
            }
            floor_div_i64(x, y).map(Value::Int)
        }
    }
}

/// `%`, paired with `//`: the result has the sign of the divisor. A
/// duration by a duration is a duration (07).
pub fn rem(a: &Value, b: &Value) -> Result<Value, Diag> {
    if let (Value::Dur(x), Value::Dur(y)) = (a, b) {
        if *y == 0 {
            return Err(divzero());
        }
        return Ok(Value::Dur(rem_i64(*x, *y)));
    }
    match numeric("take the remainder of", a, b)? {
        Num::Floats(x, y) => {
            if y == 0.0 {
                return Err(divzero());
            }
            finite(float_divmod(x, y).1)
        }
        Num::Ints(x, y) => {
            if y == 0 {
                return Err(divzero());
            }
            Ok(Value::Int(rem_i64(x, y)))
        }
    }
}

/// Float `//` and `%` together, as CPython's `float_divmod` does: the
/// remainder comes from `fmod`, which is exact, and takes the divisor's
/// sign; the quotient is `(x - m) / y` snapped to the nearest integer.
/// Flooring the rounded `x / y` instead can leave a remainder larger than
/// the divisor and of the other sign. `y` is not zero.
fn float_divmod(x: f64, y: f64) -> (f64, f64) {
    let mut m = x % y;
    let mut div = (x - m) / y;
    if m != 0.0 {
        if (y < 0.0) != (m < 0.0) {
            m += y;
            div -= 1.0;
        }
    } else {
        // A zero remainder takes the divisor's sign too.
        m = 0.0_f64.copysign(y);
    }
    let q = if div != 0.0 {
        let q = div.floor();
        if div - q > 0.5 { q + 1.0 } else { q }
    } else {
        // A zero quotient takes the sign of the true quotient.
        0.0_f64.copysign(x / y)
    };
    (q, m)
}

/// `**`: int ** non-negative int stays int (checked), otherwise float.
pub fn pow(a: &Value, b: &Value) -> Result<Value, Diag> {
    match numeric("raise", a, b)? {
        Num::Floats(x, y) => finite(x.powf(y)),
        Num::Ints(x, y) if y < 0 => finite((x as f64).powf(y as f64)),
        Num::Ints(base, exp) => {
            let (mut result, mut base, mut exp) = (1i64, base, exp);
            while exp > 0 {
                if exp & 1 == 1 {
                    result = result.checked_mul(base).ok_or_else(overflow)?;
                }
                exp >>= 1;
                if exp > 0 {
                    base = base.checked_mul(base).ok_or_else(overflow)?;
                }
            }
            Ok(Value::Int(result))
        }
    }
}

pub fn neg(a: &Value) -> Result<Value, Diag> {
    match a {
        Value::Int(x) => x.checked_neg().map(Value::Int).ok_or_else(overflow),
        Value::Float(x) => Ok(Value::Float(-x)),
        Value::Dur(x) => x.checked_neg().map(Value::Dur).ok_or_else(overflow),
        _ => Err(error(
            "E0506",
            no_pos(),
            vec![Arg::Str("negate".into()), Arg::Str(type_name(a))],
        )),
    }
}

/// `!` requires a bool: there is no truthiness.
pub fn not(a: &Value) -> Result<Value, Diag> {
    match a {
        Value::Bool(b) => Ok(Value::Bool(!b)),
        _ => Err(error("E0505", no_pos(), vec![Arg::Str(type_name(a))])),
    }
}
