use std::cmp::Ordering;
use std::rc::Rc;

use fung_front::diag::Diag;

use super::Value;
use super::arith::compare_error;

/// `==`: numbers compare by value across int/float (exactly), same types by
/// value, reference types by identity, different types are never equal.
pub fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Int(x), Value::Float(y)) | (Value::Float(y), Value::Int(x)) => {
            cmp_int_float(*x, *y) == Ordering::Equal
        }
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Nil, Value::Nil) => true,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Sym(x), Value::Sym(y)) => x == y,
        (Value::Dur(x), Value::Dur(y)) => x == y,
        (Value::Tuple(x), Value::Tuple(y)) => equal_items(x.iter(), x.len(), y.iter(), y.len()),
        (Value::List(x), Value::List(y)) => equal_items(x.iter(), x.len(), y.iter(), y.len()),
        (Value::Tag(x), Value::Tag(y)) => {
            x.name == y.name
                && equal_items(x.args.iter(), x.args.len(), y.args.iter(), y.args.len())
        }
        (Value::Range(x), Value::Range(y)) => x == y,
        // Same keys with equal values; order does not matter.
        (Value::Map(x), Value::Map(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, xv)| y.get(k).is_some_and(|yv| equal(xv, yv)))
        }
        (Value::Closure(x), Value::Closure(y)) => Rc::ptr_eq(x, y),
        (Value::Native(x), Value::Native(y)) => std::ptr::eq(*x, *y),
        (Value::Module(x), Value::Module(y)) => Rc::ptr_eq(x, y),
        (Value::Task(x), Value::Task(y)) => Rc::ptr_eq(x, y),
        (Value::Channel(x), Value::Channel(y)) => Rc::ptr_eq(x, y),
        (Value::Atom(x), Value::Atom(y)) => Rc::ptr_eq(x, y),
        (Value::Agent(x), Value::Agent(y)) => Rc::ptr_eq(x, y),
        (Value::Caught(x), Value::Caught(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

fn equal_items<'a>(
    a: impl Iterator<Item = &'a Value>,
    an: usize,
    b: impl Iterator<Item = &'a Value>,
    bn: usize,
) -> bool {
    an == bn && a.zip(b).all(|(x, y)| equal(x, y))
}

/// `< > <= >=` for numbers, strings (code point order), durations, lists
/// and tuples. Anything else is E0501.
pub fn compare(a: &Value, b: &Value) -> Result<Ordering, Diag> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Ok(x.cmp(y)),
        (Value::Int(x), Value::Float(y)) => Ok(cmp_int_float(*x, *y)),
        (Value::Float(x), Value::Int(y)) => Ok(cmp_int_float(*y, *x).reverse()),
        (Value::Float(x), Value::Float(y)) => Ok(cmp_floats(*x, *y)),
        // Rust compares UTF-8 bytes, which orders the same as code points.
        (Value::Str(x), Value::Str(y)) => Ok(x.as_str().cmp(y.as_str())),
        (Value::Dur(x), Value::Dur(y)) => Ok(x.cmp(y)),
        (Value::Tuple(x), Value::Tuple(y)) => {
            compare_items(x.iter(), x.len(), y.iter(), y.len()).ok_or_else(|| compare_error(a, b))
        }
        (Value::List(x), Value::List(y)) => {
            compare_items(x.iter(), x.len(), y.iter(), y.len()).ok_or_else(|| compare_error(a, b))
        }
        _ => Err(compare_error(a, b)),
    }
}

fn cmp_floats(x: f64, y: f64) -> Ordering {
    if x < y {
        Ordering::Less
    } else if x > y {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// Lexicographic; a shorter prefix is smaller. `None` if two elements are
/// not comparable.
fn compare_items<'a>(
    a: impl Iterator<Item = &'a Value>,
    an: usize,
    b: impl Iterator<Item = &'a Value>,
    bn: usize,
) -> Option<Ordering> {
    for (x, y) in a.zip(b) {
        match compare(x, y).ok()? {
            Ordering::Equal => {}
            o => return Some(o),
        }
    }
    Some(an.cmp(&bn))
}

/// Compares an int and a float exactly, without converting the int to f64.
/// Floats are never NaN in fung.
fn cmp_int_float(i: i64, f: f64) -> Ordering {
    if f >= 9223372036854775808.0 {
        return Ordering::Less;
    }
    if f < -9223372036854775808.0 {
        return Ordering::Greater;
    }
    let t = f as i64; // exact: |f| < 2^63 and truncation is exact
    match i.cmp(&t) {
        Ordering::Equal => {}
        o => return o,
    }
    let frac = f - t as f64;
    if frac > 0.0 {
        Ordering::Less
    } else if frac < 0.0 {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}
