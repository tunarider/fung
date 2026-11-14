use fung_front::diag::Diag;

use super::arith::{divzero, overflow, type_error};
use super::{Value, type_name};

pub(super) fn add_dur(a: i64, b: i64) -> Result<Value, Diag> {
    a.checked_add(b).map(Value::Dur).ok_or_else(overflow)
}

pub(super) fn sub_dur(a: i64, b: i64) -> Result<Value, Diag> {
    a.checked_sub(b).map(Value::Dur).ok_or_else(overflow)
}

/// A finite float as a sign and `m * 2^e` exactly.
fn float_parts(f: f64) -> (bool, u64, i32) {
    let bits = f.to_bits();
    let neg = bits >> 63 == 1;
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1u64 << 52) - 1);
    if exp == 0 {
        (neg, frac, -1074)
    } else {
        (neg, frac | (1u64 << 52), exp - 1075)
    }
}

/// `num * 2^shift / den` for `den > 0`, rounded to the nearest int,
/// halves up (the caller works on magnitudes, so away from zero): `None`
/// when it is 2^64 or more. Long division a bit at a time: the
/// remainder stays below `den`, so nothing wraps however long the
/// dividend is.
fn div_round(num: u128, shift: u32, den: u128) -> Option<u64> {
    let (mut q, mut r) = (0u128, 0u128);
    let width = 128 - num.leading_zeros();
    for i in (0..width + shift).rev() {
        let bit = if i >= shift {
            (num >> (i - shift)) & 1
        } else {
            0
        };
        r = (r << 1) | bit;
        q <<= 1;
        if r >= den {
            r -= den;
            q |= 1;
        }
        if q >> 64 != 0 {
            return None;
        }
    }
    if r >= den - r {
        q += 1;
    }
    u64::try_from(q).ok()
}

/// A magnitude with its sign as a duration: E0503 past the range.
fn signed(neg: bool, mag: Option<u64>) -> Result<Value, Diag> {
    let mag = i128::from(mag.ok_or_else(overflow)?);
    let v = if neg { -mag } else { mag };
    i64::try_from(v).map(Value::Dur).map_err(|_| overflow())
}

/// `d * f` exactly on the float's exact value, rounded once to the
/// nanosecond, halves away from zero (D135): `|d| * m` fits 117 bits.
fn mul_float(d: i64, f: f64) -> Result<Value, Diag> {
    let (neg, m, e) = float_parts(f);
    let num = u128::from(d.unsigned_abs()) * u128::from(m);
    let mag = if num == 0 {
        Some(0)
    } else if e >= 0 {
        // a product that is not zero and is shifted past 2^64 overflows
        if e > 64 {
            None
        } else {
            div_round(num, e as u32, 1)
        }
    } else if -e >= 127 {
        // below half a nanosecond: `num` < 2^117
        Some(0)
    } else {
        div_round(num, 0, 1u128 << -e)
    };
    signed(neg != (d < 0), mag)
}

/// `d / f` exactly on the float's exact value, rounded once (D135). A
/// zero `f` is E0502.
fn div_float(d: i64, f: f64) -> Result<Value, Diag> {
    if f == 0.0 {
        return Err(divzero());
    }
    let (neg, m, e) = float_parts(f);
    let num = u128::from(d.unsigned_abs());
    let mag = if num == 0 {
        Some(0)
    } else if e <= 0 {
        // `|d| * 2^-e / m`; past 2^64 the division stops early
        div_round(num, (-e) as u32, u128::from(m))
    } else if e >= 74 {
        // `m * 2^e` >= 2^126 > 2 * |d|: below half a nanosecond
        Some(0)
    } else {
        div_round(num, 0, u128::from(m) << e)
    };
    signed(neg != (d < 0), mag)
}

/// Multiplies a duration by a number: an int exactly, a float exactly on
/// its exact value and rounded once (D135). Overflow is E0503.
pub(super) fn scale_dur(d: i64, n: &Value) -> Result<Value, Diag> {
    match n {
        Value::Int(x) => d.checked_mul(*x).map(Value::Dur).ok_or_else(overflow),
        Value::Float(x) => mul_float(d, *x),
        _ => Err(type_error("multiply", "dur", &type_name(n))),
    }
}

/// Divides a duration by a number. The result is exact and rounded once
/// to the nearest nanosecond, halves away from zero, as a duration
/// literal does, whether the divisor is an int or a float (D135).
pub(super) fn div_dur(d: i64, n: &Value) -> Result<Value, Diag> {
    match n {
        Value::Int(0) => Err(divzero()),
        Value::Int(x) => div_dur_int(d, *x),
        Value::Float(x) => div_float(d, *x),
        _ => Err(type_error("divide", "dur", &type_name(n))),
    }
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
