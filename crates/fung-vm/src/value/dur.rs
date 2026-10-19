use fung_front::diag::Diag;

use super::arith::{divzero, overflow, type_error};
use super::{Value, type_name};

pub(super) fn add_dur(a: i64, b: i64) -> Result<Value, Diag> {
    a.checked_add(b).map(Value::Dur).ok_or_else(overflow)
}

pub(super) fn sub_dur(a: i64, b: i64) -> Result<Value, Diag> {
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

/// Divides a duration by a number. An int divisor divides exactly and
/// rounds to the nearest nanosecond, halves away from zero, as a duration
/// literal does; only a float divisor goes through f64.
pub(super) fn div_dur(d: i64, n: &Value) -> Result<Value, Diag> {
    let f = match n {
        Value::Int(0) => return Err(divzero()),
        Value::Int(x) => return div_dur_int(d, *x),
        Value::Float(x) => *x,
        _ => return Err(type_error("divide", "dur", &type_name(n))),
    };
    if f == 0.0 {
        return Err(divzero());
    }
    round_to_dur((d as f64 / f).round())
}

/// `d / n` for `n != 0`, in i128 so that nothing wraps.
fn div_dur_int(d: i64, n: i64) -> Result<Value, Diag> {
    let (d, n) = (i128::from(d), i128::from(n));
    let (mut q, r) = (d / n, d % n);
    if 2 * r.abs() >= n.abs() {
        q += if (d < 0) == (n < 0) { 1 } else { -1 };
    }
    i64::try_from(q).map(Value::Dur).map_err(|_| overflow())
}
