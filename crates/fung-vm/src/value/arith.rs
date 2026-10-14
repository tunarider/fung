use std::rc::Rc;

use fung_front::diag::{Arg, Diag, error};
use fung_front::source::{FileId, Pos};

use super::dur::{add_dur, div_dur, scale_dur, sub_dur};
use super::{Value, type_name};

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

pub fn add(a: &Value, b: &Value) -> Result<Value, Diag> {
    match (a, b) {
        (Value::Str(x), Value::Str(y)) => {
            let mut s = String::with_capacity(x.len() + y.len());
            s.push_str(x);
            s.push_str(y);
            return Ok(Value::Str(s.into()));
        }
        (Value::Dur(x), Value::Dur(y)) => return add_dur(*x, *y),
        (Value::List(x), Value::List(y)) => {
            // An empty side returns the other operand itself, as Go does.
            if y.is_empty() {
                return Ok(a.clone());
            }
            if x.is_empty() {
                return Ok(b.clone());
            }
            return Ok(Value::List(Rc::new(x.concat(y))));
        }
        (Value::Str(_) | Value::Dur(_) | Value::List(_), _) => {
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
/// duration.
pub fn div(a: &Value, b: &Value) -> Result<Value, Diag> {
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

/// `//`: floor division; ints stay ints.
pub fn floor_div(a: &Value, b: &Value) -> Result<Value, Diag> {
    match numeric("divide", a, b)? {
        Num::Floats(x, y) => {
            if y == 0.0 {
                return Err(divzero());
            }
            finite((x / y).floor())
        }
        Num::Ints(x, y) => {
            if y == 0 {
                return Err(divzero());
            }
            if x == i64::MIN && y == -1 {
                return Err(overflow());
            }
            let mut q = x / y;
            if x % y != 0 && (x < 0) != (y < 0) {
                q -= 1;
            }
            Ok(Value::Int(q))
        }
    }
}

/// `%`, paired with `//`: the result has the sign of the divisor.
pub fn rem(a: &Value, b: &Value) -> Result<Value, Diag> {
    match numeric("take the remainder of", a, b)? {
        Num::Floats(x, y) => {
            if y == 0.0 {
                return Err(divzero());
            }
            finite(x - y * (x / y).floor())
        }
        Num::Ints(x, y) => {
            if y == 0 {
                return Err(divzero());
            }
            if x == i64::MIN && y == -1 {
                return Ok(Value::Int(0));
            }
            let mut r = x % y;
            if r != 0 && (r < 0) != (y < 0) {
                r += y;
            }
            Ok(Value::Int(r))
        }
    }
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
