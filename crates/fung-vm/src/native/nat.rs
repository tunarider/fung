//! Helpers shared by the core prelude (`crate::prelude`) and the native
//! bridges: parameter declarations, Ok/Err values,
//! argument checks, and walking what a collection function iterates.

use std::rc::Rc;

use fung_front::diag::{Arg, error};

use crate::value::{
    ListBuilder, Lit, MapBuilder, Native, NativeFn, NativeParam, Tag, Value, format_for_print,
    intern, type_name,
};
use crate::vm::{Stop, Task, channel, no_pos};

/// A required parameter.
pub fn param(name: &'static str) -> NativeParam {
    NativeParam {
        name,
        default: None,
        rest: false,
    }
}

/// An optional parameter with its default.
pub fn opt(name: &'static str, default: Lit) -> NativeParam {
    NativeParam {
        name,
        default: Some(default),
        rest: false,
    }
}

/// A trailing rest parameter: the native gets the surplus as a list.
pub fn rest(name: &'static str) -> NativeParam {
    NativeParam {
        name,
        default: None,
        rest: true,
    }
}

/// Declares a native with its parameter list (06: 같은 호출 규약). The VM
/// checks arity and names; `func` gets one value per parameter.
pub fn def(name: &'static str, func: NativeFn, params: Vec<NativeParam>) -> Value {
    Value::Native(Native::leak(name, params, func))
}

/// Like [`def`], but the bare native, for the process-wide tables.
pub fn native(name: &'static str, func: NativeFn, params: Vec<NativeParam>) -> &'static Native {
    Native::leak(name, params, func)
}

/// An error without a position: the VM gives it the call's.
pub fn fail(code: &str, args: Vec<Arg>) -> Stop {
    Stop::Error(Box::new(error(code, no_pos(), args)))
}

/// A string argument for an error message.
pub fn s(x: &str) -> Arg {
    Arg::Str(x.to_string())
}

/// A failed write to an output stream, reported as E0801 at the call.
pub fn write_err(e: std::io::Error) -> Stop {
    fail("E0801", vec![Arg::Str(e.to_string())])
}

/// `Tag(args…)`.
pub fn tag(name: &str, args: Vec<Value>) -> Value {
    Value::Tag(Rc::new(Tag {
        name: intern(name),
        args,
    }))
}

/// `Ok(v)`.
pub fn ok(v: Value) -> Value {
    tag("Ok", vec![v])
}

/// `Err({kind, extra…, message})` (06: 표준 라이브러리 Err의 내용은 맵):
/// the situation's fields first, the message last.
pub fn err_value(kind: &str, message: &str, extra: Vec<(&str, Value)>) -> Value {
    let mut m = MapBuilder::default();
    m.set(Rc::new("kind".to_string()), Value::Sym(intern(kind)));
    for (k, v) in extra {
        m.set(Rc::new(k.to_string()), v);
    }
    m.set(Rc::new("message".to_string()), Value::str(message));
    tag("Err", vec![Value::Map(Rc::new(m.finish()))])
}

/// A list value from collected items.
pub fn list(items: Vec<Value>) -> Value {
    let mut b = ListBuilder::with_capacity(items.len());
    for v in items {
        b.push(v);
    }
    Value::List(Rc::new(b.finish()))
}

/// Renders values the way print shows them, space-separated.
pub fn join_for_print<'a>(items: impl Iterator<Item = &'a Value>) -> String {
    let mut out = String::new();
    for (i, v) in items.enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&format_for_print(v));
    }
    out
}

/// The E0506 a collection function reports for what it cannot iterate.
fn not_iterable(verb: &str, v: &Value) -> Stop {
    fail("E0506", vec![s(verb), Arg::Str(type_name(v))])
}

/// The most elements a function builds a result of from one range (02):
/// a hundred million values are 1.6 GB.
pub const MAX_ELEMENTS: u64 = 100_000_000;

/// E0806 for a result past [`MAX_ELEMENTS`].
pub fn too_many_elements() -> Stop {
    fail("E0806", vec![s("elements"), Arg::Int(MAX_ELEMENTS as i64)])
}

/// For a function whose result holds every element (`map`, `sort`,
/// `flatten`, `group_by`): a range longer than [`MAX_ELEMENTS`] is
/// refused before any work, after the arguments are checked.
pub fn limit_elements(v: &Value) -> Result<(), Stop> {
    match v {
        Value::Range(r) if r.len() > i128::from(MAX_ELEMENTS) => Err(too_many_elements()),
        _ => Ok(()),
    }
}

/// Whether `v` can be iterated (02): a list, a range or a channel, or
/// E0506 with `verb`. Collection natives check this first, in argument
/// order, before they receive or build anything.
pub fn check_items(v: &Value, verb: &str) -> Result<(), Stop> {
    match v {
        Value::List(_) | Value::Range(_) | Value::Channel(_) => Ok(()),
        _ => Err(not_iterable(verb, v)),
    }
}

/// Walks what `items` lists, one element at a time, stopping at the first
/// error `f` returns. Lists and ranges are walked in place: a range yields
/// its numbers without ever building a list of them.
pub fn each(
    t: &mut Task<'_>,
    v: &Value,
    verb: &str,
    mut f: impl FnMut(&mut Task<'_>, Value) -> Result<(), Stop>,
) -> Result<(), Stop> {
    each_until(t, v, verb, |t, x| f(t, x).map(|()| false))?;
    Ok(())
}

/// `each` that also stops when `f` returns true; the result says whether
/// it did (`find`, `any`, `all`). A channel is walked as its values
/// arrive (05): `f` sees each one when it is received, and what is left
/// after a stop or an error stays in the channel.
pub fn each_until(
    t: &mut Task<'_>,
    v: &Value,
    verb: &str,
    mut f: impl FnMut(&mut Task<'_>, Value) -> Result<bool, Stop>,
) -> Result<bool, Stop> {
    match v {
        Value::List(l) => {
            for x in l.iter() {
                if f(t, x.clone())? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Value::Range(r) => {
            for i in 0..r.len() {
                // The number itself fits an int; only the offset may not.
                if f(t, Value::Int(r.start.wrapping_add(i as i64)))? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Value::Channel(ch) => loop {
            match channel::recv(t, ch, None)? {
                channel::Received::Value(x) => {
                    if f(t, x)? {
                        return Ok(true);
                    }
                }
                channel::Received::Closed | channel::Received::Timeout => return Ok(false),
            }
        },
        _ => Err(not_iterable(verb, v)),
    }
}

/// The element count of a list or range, 0 for anything else.
pub fn size_hint(v: &Value) -> usize {
    match v {
        Value::List(l) => l.len(),
        Value::Range(r) => usize::try_from(r.len()).unwrap_or(usize::MAX),
        _ => 0,
    }
}

/// What a collection function iterates (02), as a vector, for the
/// functions that need every element at once (`sort`): lists, ranges and
/// channels (received until closed). Prefer `each` where the elements are
/// only walked.
pub fn items(t: &mut Task<'_>, v: &Value, verb: &str) -> Result<Vec<Value>, Stop> {
    match v {
        Value::List(l) => Ok(l.to_vec()),
        Value::Range(r) if r.len() > i128::from(MAX_ELEMENTS) => Err(too_many_elements()),
        Value::Channel(ch) => channel::drain(t, ch),
        Value::Range(r) => Ok((0..r.len())
            .map(|i| Value::Int(r.start.wrapping_add(i as i64)))
            .collect()),
        _ => Err(not_iterable(verb, v)),
    }
}

/// Runs a predicate with the argument buffer `buf` and insists on a bool
/// result (E0505).
pub fn call_bool(
    t: &mut Task<'_>,
    f: &Value,
    x: Value,
    buf: &mut Vec<Value>,
) -> Result<bool, Stop> {
    buf.push(x);
    match t.call_buf(f, buf)? {
        Value::Bool(b) => Ok(b),
        r => Err(fail("E0505", vec![Arg::Str(type_name(&r))])),
    }
}

pub fn is_fn(v: &Value) -> bool {
    matches!(v, Value::Closure(_) | Value::Native(_))
}

/// E0403 unless `v` is a function.
pub fn need_fn(name: &str, v: &Value) -> Result<(), Stop> {
    if is_fn(v) {
        return Ok(());
    }
    Err(wrong_arg(name, "fn", v))
}

/// E0403: argument `name` must be `want`.
pub fn wrong_arg(name: &str, want: &str, v: &Value) -> Stop {
    fail("E0403", vec![s(name), s(want), Arg::Str(type_name(v))])
}

/// The map argument, or E0506 with `verb`.
pub fn need_map<'a>(verb: &str, v: &'a Value) -> Result<&'a Rc<crate::value::Map>, Stop> {
    match v {
        Value::Map(m) => Ok(m),
        _ => Err(not_iterable(verb, v)),
    }
}

/// A map key argument, or E0501 (`cannot <verb> map and <type>`).
pub fn need_key<'a>(verb: &str, v: &'a Value) -> Result<&'a Rc<String>, Stop> {
    match v {
        Value::Str(k) => Ok(k),
        _ => Err(fail(
            "E0501",
            vec![s(verb), s("map"), Arg::Str(type_name(v))],
        )),
    }
}

pub fn need_int(name: &str, v: &Value) -> Result<i64, Stop> {
    match v {
        Value::Int(n) => Ok(*n),
        _ => Err(wrong_arg(name, "int", v)),
    }
}

pub fn need_str<'a>(name: &str, v: &'a Value) -> Result<&'a str, Stop> {
    match v {
        Value::Str(x) => Ok(x),
        _ => Err(wrong_arg(name, "str", v)),
    }
}

/// Checks that every named argument is a string, in order.
pub fn str_args<'a, const N: usize>(
    args: &'a [Value],
    names: [&str; N],
) -> Result<[&'a str; N], Stop> {
    let mut out = [""; N];
    for (i, n) in names.iter().enumerate() {
        out[i] = need_str(n, &args[i])?;
    }
    Ok(out)
}

/// A `str | nil` argument.
pub fn opt_str<'a>(name: &str, v: &'a Value) -> Result<Option<&'a str>, Stop> {
    match v {
        Value::Nil => Ok(None),
        Value::Str(x) => Ok(Some(x)),
        _ => Err(wrong_arg(name, "str | nil", v)),
    }
}

/// A list of strings argument.
pub fn str_list(name: &str, v: &Value) -> Result<Vec<String>, Stop> {
    let Value::List(l) = v else {
        return Err(wrong_arg(name, "a list of strings", v));
    };
    l.iter()
        .map(|item| match item {
            Value::Str(x) => Ok(x.to_string()),
            _ => Err(fail(
                "E0403",
                vec![
                    s(name),
                    s("a list of strings"),
                    Arg::Str(format!("a list holding {}", type_name(item))),
                ],
            )),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Range;

    fn range(n: i64) -> Value {
        Value::Range(Rc::new(Range {
            start: 0,
            end: n,
            inclusive: false,
        }))
    }

    #[test]
    fn element_limit_boundary() {
        assert!(limit_elements(&range(MAX_ELEMENTS as i64)).is_ok());
        assert!(limit_elements(&range(MAX_ELEMENTS as i64 + 1)).is_err());
        assert!(limit_elements(&list(vec![Value::Int(1)])).is_ok());
    }
}
