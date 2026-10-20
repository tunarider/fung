use std::cmp::Ordering;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

use fung_front::diag::Diag;

use super::Value;
use super::arith::compare_error;

/// `==`: numbers compare by value across int/float (exactly), same types by
/// value, reference types by identity, different types are never equal.
pub fn equal(a: &Value, b: &Value) -> bool {
    if same_object(a, b) {
        return true;
    }
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
        (Value::Time(x), Value::Time(y)) => x.equal(y),
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
        (Value::Scope(x), Value::Scope(y)) => Rc::ptr_eq(x, y),
        (Value::Atom(x), Value::Atom(y)) => Rc::ptr_eq(x, y),
        (Value::Sender(x), Value::Sender(y)) => Rc::ptr_eq(x, y),
        (Value::Receiver(x), Value::Receiver(y)) => Rc::ptr_eq(x, y),
        (Value::File(x), Value::File(y)) => std::sync::Arc::ptr_eq(x, y),
        (Value::Conn(x), Value::Conn(y)) => std::sync::Arc::ptr_eq(x, y),
        (Value::Listener(x), Value::Listener(y)) => std::sync::Arc::ptr_eq(x, y),
        _ => false,
    }
}

/// `==` and `!=` as programs see them (02): `equal`, except that a symbol
/// meeting a string anywhere the comparison looks (inside lists, tuples,
/// tags and maps too) is E0517: never equal, and almost always a value
/// that came back from JSON or a file (L10). Patterns keep `equal`.
pub fn eq(a: &Value, b: &Value) -> Result<bool, Diag> {
    sym_meets_str(a, b)?;
    Ok(equal(a, b))
}

/// Whether `a` and `b` are the same shared collection (P5): such a value
/// equals itself, and fung has no NaN to say otherwise.
fn same_object(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::List(x), Value::List(y)) => Rc::ptr_eq(x, y),
        (Value::Map(x), Value::Map(y)) => Rc::ptr_eq(x, y),
        (Value::Tuple(x), Value::Tuple(y)) => Rc::ptr_eq(x, y),
        (Value::Tag(x), Value::Tag(y)) => Rc::ptr_eq(x, y),
        (Value::Str(x), Value::Str(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

/// E0517 where `equal` would compare a symbol with a string.
fn sym_meets_str(a: &Value, b: &Value) -> Result<(), Diag> {
    if same_object(a, b) {
        return Ok(());
    }
    let both = |x: &[Value], y: &[Value]| -> Result<(), Diag> {
        if x.len() == y.len() {
            for (p, q) in x.iter().zip(y) {
                sym_meets_str(p, q)?;
            }
        }
        Ok(())
    };
    match (a, b) {
        (Value::Sym(_), Value::Str(_)) | (Value::Str(_), Value::Sym(_)) => {
            Err(fung_front::diag::error(
                "E0517",
                super::arith::no_pos(),
                vec![
                    fung_front::diag::Arg::Str(super::format(a)),
                    fung_front::diag::Arg::Str(super::format(b)),
                ],
            ))
        }
        (Value::List(x), Value::List(y)) => {
            if x.len() == y.len() {
                for (p, q) in x.iter().zip(y.iter()) {
                    sym_meets_str(p, q)?;
                }
            }
            Ok(())
        }
        (Value::Tuple(x), Value::Tuple(y)) => both(x, y),
        (Value::Tag(x), Value::Tag(y)) if x.name == y.name => both(&x.args, &y.args),
        (Value::Map(x), Value::Map(y)) if x.len() == y.len() => {
            for (k, xv) in x.iter() {
                if let Some(yv) = y.get(k) {
                    sym_meets_str(xv, yv)?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
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

/// `< > <= >=` for numbers, strings (code point order), durations, times, lists
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
        (Value::Time(x), Value::Time(y)) => Ok(x.cmp_instant(y)),
        (Value::Tuple(x), Value::Tuple(y)) => compare_items(x.iter(), x.len(), y.iter(), y.len()),
        (Value::List(x), Value::List(y)) => compare_items(x.iter(), x.len(), y.iter(), y.len()),
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

/// Lexicographic; a shorter prefix is smaller. Two elements that do not
/// compare give their own error, naming their types.
fn compare_items<'a>(
    a: impl Iterator<Item = &'a Value>,
    an: usize,
    b: impl Iterator<Item = &'a Value>,
    bn: usize,
) -> Result<Ordering, Diag> {
    for (x, y) in a.zip(b) {
        match compare(x, y)? {
            Ordering::Equal => {}
            o => return Ok(o),
        }
    }
    Ok(an.cmp(&bn))
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

/// A hash that agrees with `equal` (P1, `unique`): equal values hash
/// alike. An int and the float of the same value hash as the int; a map
/// hashes its entries in any order; a shared object hashes its address.
/// A symbol hashes as the string of its name, so the two meet in one
/// bucket and `eq` reports them (E0517) as `in` does.
pub fn hash_value(v: &Value, h: &mut DefaultHasher) {
    match v {
        Value::Int(i) => (0u8, *i).hash(h),
        Value::Float(f) => {
            if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f < i64::MAX as f64 {
                (0u8, *f as i64).hash(h);
            } else {
                (1u8, f.to_bits()).hash(h);
            }
        }
        Value::Bool(b) => (2u8, *b).hash(h),
        Value::Nil => 3u8.hash(h),
        Value::Str(s) => (4u8, s.as_str()).hash(h),
        Value::Sym(s) => (4u8, s.name()).hash(h),
        Value::Dur(d) => (5u8, *d).hash(h),
        Value::Time(t) => (6u8, t.instant()).hash(h),
        Value::List(l) => {
            (7u8, l.len()).hash(h);
            for x in l.iter() {
                hash_value(x, h);
            }
        }
        Value::Tuple(t) => {
            (8u8, t.len()).hash(h);
            for x in t.iter() {
                hash_value(x, h);
            }
        }
        Value::Tag(t) => {
            (9u8, t.name.name(), t.args.len()).hash(h);
            for x in &t.args {
                hash_value(x, h);
            }
        }
        Value::Range(r) => (10u8, r.start, r.end, r.inclusive).hash(h),
        Value::Map(m) => {
            // The entries' hashes summed: the order does not count.
            let mut sum = 0u64;
            for (k, x) in m.iter() {
                let mut e = DefaultHasher::new();
                k.hash(&mut e);
                hash_value(x, &mut e);
                sum = sum.wrapping_add(e.finish());
            }
            (11u8, m.len(), sum).hash(h);
        }
        Value::Closure(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::Native(x) => (12u8, std::ptr::from_ref(*x) as usize).hash(h),
        Value::Module(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::Task(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::Scope(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::Atom(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::Sender(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::Receiver(x) => (12u8, Rc::as_ptr(x) as usize).hash(h),
        Value::File(x) => (12u8, std::sync::Arc::as_ptr(x) as *const u8 as usize).hash(h),
        Value::Conn(x) => (12u8, std::sync::Arc::as_ptr(x) as *const u8 as usize).hash(h),
        Value::Listener(x) => (12u8, std::sync::Arc::as_ptr(x) as *const u8 as usize).hash(h),
    }
}
