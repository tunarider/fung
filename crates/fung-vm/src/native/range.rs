//! `_native.range`: what the prelude written in fung needs of a range
//! beyond walking it. A range may hold more numbers than an int counts
//! (`-9223372036854775808..=9223372036854775807`, 02), so `len` fails on
//! it; these count in i128 and never do. `extras/prelude.fung` builds
//! `last`, `take` and `drop` of a range from them.

use std::rc::Rc;
use std::sync::OnceLock;

use super::nat::{self, native, opt, param};
use crate::value::{Lit, Module, Native, Range, Value};
use crate::vm::{Stop, Task};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The `_native.range` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.get_or_init(|| {
        vec![
            ("last", native("last", last_fn, vec![param("r")])),
            (
                "slice",
                native(
                    "slice",
                    slice_fn,
                    vec![param("r"), param("from"), opt("to", Lit::Nil)],
                ),
            ),
        ]
    });
    let mut m = Module::new("range", "_native.range");
    for (name, n) in natives {
        m.set(name, Value::Native(n));
    }
    Rc::new(m)
}

fn need_range(v: &Value) -> Result<&Range, Stop> {
    match v {
        Value::Range(r) => Ok(r),
        _ => Err(nat::wrong_arg("r", "range", v)),
    }
}

/// `last(r)`: the last number, nil for an empty range.
fn last_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let r = need_range(&a[0])?;
    let last = i128::from(r.end) - i128::from(!r.inclusive);
    Ok(if last >= i128::from(r.start) {
        Value::Int(last as i64)
    } else {
        Value::Nil
    })
}

/// `slice(r, from, to = nil)`: the numbers at positions `from` up to, not
/// including, `to` (nil is the end), as a range. Positions count from 0
/// and are clamped into `0..=len(r)`, so the result is never more than
/// `r` has.
fn slice_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let r = need_range(&a[0])?;
    let len = r.len();
    let at = |name: &str, v: &Value| -> Result<i128, Stop> {
        Ok(i128::from(nat::need_int(name, v)?).clamp(0, len))
    };
    let from = at("from", &a[1])?;
    let to = match &a[2] {
        Value::Nil => len,
        v => at("to", v)?,
    };
    let start = i128::from(r.start);
    // Both ends lie within r, so they fit an int.
    let part = if from < to {
        Range {
            start: (start + from) as i64,
            end: (start + to - 1) as i64,
            inclusive: true,
        }
    } else {
        Range {
            start: r.start,
            end: r.start,
            inclusive: false,
        }
    };
    Ok(Value::Range(Rc::new(part)))
}
