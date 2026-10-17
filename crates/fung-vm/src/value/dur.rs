use fung_front::diag::Diag;

use super::arith::{divzero, overflow, type_error};
use super::{Value, type_name};

pub(super) fn add_dur(a: i64, b: i64) -> Result<Value, Diag> {
    a.checked_add(b).map(Value::Dur).ok_or_else(overflow)
}

pub(super) fn sub_dur(a: i64, b: i64) -> Result<Value, Diag> {
    // Go negates `b` first, so the minimum overflows even if `a - b` fits.
    if b == i64::MIN {
        return Err(overflow());
    }
    a.checked_sub(b).map(Value::Dur).ok_or_else(overflow)
}

fn round_to_dur(r: f64) -> Result<Value, Diag> {
    if r.is_nan() || r.abs() >= i64::MAX as f64 {
        return Err(overflow());
    }
    Ok(Value::Dur(r as i64))
}

/// Multiplies a duration by a number, rejecting overflow.
pub(super) fn scale_dur(d: i64, n: &Value) -> Result<Value, Diag> {
    let f = match n {
        Value::Int(x) => return d.checked_mul(*x).map(Value::Dur).ok_or_else(overflow),
        Value::Float(x) => *x,
        _ => return Err(type_error("multiply", "dur", &type_name(n))),
    };
    round_to_dur((d as f64 * f).round())
}

/// Divides a duration by a number.
pub(super) fn div_dur(d: i64, n: &Value) -> Result<Value, Diag> {
    let f = match n {
        Value::Int(x) => *x as f64,
        Value::Float(x) => *x,
        _ => return Err(type_error("divide", "dur", &type_name(n))),
    };
    if f == 0.0 {
        return Err(divzero());
    }
    round_to_dur((d as f64 / f).round())
}
