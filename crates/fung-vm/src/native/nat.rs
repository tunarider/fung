//! Helpers shared by the core prelude (`crate::prelude`) and the native
//! bridges: parameter declarations, Ok/Err values,
//! argument checks, and walking what a collection function iterates.

use std::rc::Rc;

use fung_front::diag::{Arg, error};

use crate::value::{
    ListBuilder, Lit, MapBuilder, Native, NativeFn, NativeParam, StepFn, Tag, Text, Value, intern,
    key, type_name, write_for_print,
};
use crate::vm::{Flow, Kind, Stop, Task, no_pos};

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

/// [`def`] for a step native (`vm/cont.rs`).
pub fn def_step(name: &'static str, step: StepFn, params: Vec<NativeParam>) -> Value {
    Value::Native(Native::leak_step(name, params, step))
}

/// [`native`] for a step native.
pub fn native_step(name: &'static str, step: StepFn, params: Vec<NativeParam>) -> &'static Native {
    Native::leak_step(name, params, step)
}

/// Hands a wait on the operating system off: `job` gets plain data (no
/// fung value, so it is `Send`) and does the system calls on a pool
/// thread while the task parks; `finish` turns its result into the
/// native's value, back on the run thread (`vm/offload.rs`). An error
/// `finish` gives without a position gets the native's call site, as any
/// native error does. `job` must own what it uses (`'static`): copy
/// strings out of fung values and clone the `Arc` of a handle.
///
/// A job that has started is waited for, even past a cancellation or a
/// deadline: use this for jobs that change something.
pub(crate) fn offload<R: Send + 'static>(
    t: &mut Task<'_>,
    job: impl FnOnce() -> R + Send + 'static,
    finish: impl FnOnce(&mut Task<'_>, R) -> Result<Value, Stop> + 'static,
) -> Result<Flow, Stop> {
    crate::vm::offload_kind(t, Kind::Wait, job, |t, r| finish(t, r).map(Flow::Done))
}

/// [`offload`] for a job that only reads: a cancellation or a deadline
/// unwinds the task at once, and the job's result is dropped when it
/// ends.
pub(crate) fn offload_abandoning<R: Send + 'static>(
    t: &mut Task<'_>,
    job: impl FnOnce() -> R + Send + 'static,
    finish: impl FnOnce(&mut Task<'_>, R) -> Result<Value, Stop> + 'static,
) -> Result<Flow, Stop> {
    crate::vm::offload_kind(t, Kind::Abandon, job, |t, r| finish(t, r).map(Flow::Done))
}

/// [`offload_abandoning`] for a job that only reads and is short on a
/// local file system: while the task is alone (`vm::alone`) it runs here,
/// on the run thread, where blocking holds no other task back; otherwise
/// it is handed off. A slow file system then delays only the events that
/// come meanwhile (a stop request), as a computing turn does.
pub(crate) fn offload_short<R: Send + 'static>(
    t: &mut Task<'_>,
    job: impl FnOnce() -> R + Send + 'static,
    finish: impl FnOnce(&mut Task<'_>, R) -> Result<Value, Stop> + 'static,
) -> Result<Flow, Stop> {
    if crate::vm::alone(t) {
        let r = job();
        return finish(t, r).map(Flow::Done);
    }
    offload_abandoning(t, job, finish)
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
        args: args.into(),
    }))
}

/// `Ok(v)`.
pub fn ok(v: Value) -> Value {
    Value::Tag(Rc::new(Tag {
        name: crate::value::Sym::OK,
        args: crate::value::Items::One([v]),
    }))
}

/// `Err({kind, extra…, message})` (06: 표준 라이브러리 Err의 내용은 맵):
/// the situation's fields first, the message last.
pub fn err_value(kind: &str, message: &str, extra: Vec<(&str, Value)>) -> Value {
    let mut m = MapBuilder::default();
    m.set(key("kind"), Value::Sym(intern(kind)));
    for (k, v) in extra {
        m.set(key(k), v);
    }
    m.set(key("message"), Value::str(message));
    tag("Err", vec![Value::Map(Rc::new(m.finish()))])
}

/// Whether text read from outside keeps to the string limit (LB29, D22):
/// bytes bound the characters from above, so they are counted only past
/// it.
pub fn fits_limit(b: &[u8]) -> bool {
    let max = crate::prelude::MAX_CHARS as usize;
    b.len() <= max || fung_front::text::char_count(b) <= max
}

/// The reason a read gives for text past the string limit (D22):
/// ``<what> is longer than the string limit (100000000 characters)``.
pub fn too_long(what: &str) -> String {
    format!(
        "{what} is longer than the string limit ({} characters)",
        crate::prelude::MAX_CHARS
    )
}

/// The read error of text from outside past the string limit, for the
/// readers that stop as soon as it passes (a line, a FIFO): its reason
/// is ``<what> is longer than the string limit (…)`` (`oserr::reason`),
/// its kind `'io`.
pub fn too_long_error(what: &str) -> std::io::Error {
    std::io::Error::other(too_long(what))
}

/// The characters of text that grows a piece at a time (a command's
/// output), counted once each: a read can stop as soon as the text passes
/// the string limit, before it holds all that comes (LB29).
#[derive(Default)]
pub struct CharCount {
    /// The bytes counted so far: the buffer's start up to a byte no
    /// character runs across.
    counted: usize,
    chars: usize,
}

impl CharCount {
    /// Counts what `buf` gained since the last call; false once its
    /// characters pass the string limit.
    pub fn within(&mut self, buf: &[u8]) -> bool {
        let max = crate::prelude::MAX_CHARS as usize;
        buf.len() <= max || self.count(buf) <= max
    }

    /// The characters of `buf`, of which the part counted before is not
    /// counted again.
    pub fn count(&mut self, buf: &[u8]) -> usize {
        // A character still coming may have begun in the last three
        // bytes: the count stops before its first byte. Counted pieces
        // then add up to the count of the whole (each starts at a byte
        // no valid character runs across).
        let mut end = buf.len();
        for back in 1..=3 {
            if end < back || end - back < self.counted {
                break;
            }
            if buf[end - back] >= 0xC0 {
                end -= back;
                break;
            }
        }
        if end > self.counted {
            self.chars += fung_front::text::char_count(&buf[self.counted..end]);
            self.counted = end;
        }
        // The uncounted rest is a few bytes at most.
        self.chars + fung_front::text::char_count(&buf[self.counted..])
    }
}

/// A list value from collected items.
pub fn list(items: Vec<Value>) -> Value {
    let mut b = ListBuilder::with_capacity(items.len());
    for v in items {
        b.push(v);
    }
    Value::List(Rc::new(b.finish()))
}

/// Renders values the way print shows them, space-separated: bytes,
/// since a string's are written as they are (L4). A value whose text
/// passes the string limit is E0806 (VA28).
pub fn join_for_print<'a>(items: impl Iterator<Item = &'a Value>) -> Result<Vec<u8>, Stop> {
    let mut out = Vec::new();
    for (i, v) in items.enumerate() {
        if i > 0 {
            out.push(b' ');
        }
        write_for_print(v, &mut out)?;
    }
    Ok(out)
}

/// The E0506 a collection function reports for what it cannot iterate.
/// A sending end gets a help line, the same whatever the verb: the fix
/// is the other end. `map` and `for` both "iterate over" (VA39).
pub(crate) fn not_iterable(verb: &str, v: &Value) -> Stop {
    let d = error("E0506", no_pos(), vec![s(verb), Arg::Str(type_name(v))]);
    let d = match v {
        Value::Sender(_) => {
            d.with_help("a sending end (`tx`) cannot be read; go over the receiving end (`rx`)")
        }
        _ => d,
    };
    Stop::Error(Box::new(d))
}

/// The most elements a function builds a result of from one range (02):
/// a hundred million values are 1.6 GB.
pub const MAX_ELEMENTS: u64 = 100_000_000;

/// E0806 for a result past [`MAX_ELEMENTS`].
pub fn too_many_elements() -> Stop {
    fail("E0806", vec![s("elements"), Arg::Int(MAX_ELEMENTS as i64)])
}

/// E0806 when a list of `n` elements would pass [`MAX_ELEMENTS`] (D63):
/// every path that builds a list checks the length it is about to have.
pub fn check_len(n: u64) -> Result<(), Stop> {
    if n > MAX_ELEMENTS {
        Err(too_many_elements())
    } else {
        Ok(())
    }
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

/// Whether `v` can be iterated (02): a list, a range or a receiving end, or
/// E0506 with `verb`. Collection natives check this first, in argument
/// order, before they receive or build anything.
pub fn check_items(v: &Value, verb: &str) -> Result<(), Stop> {
    match v {
        Value::List(_) | Value::Range(_) | Value::Receiver(_) => Ok(()),
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

/// A list's or range's elements as a vector, for the functions that need
/// every element at once (`sort`). A receiving end is walked by
/// `native/walk.rs` instead, which parks while the channel is empty.
pub fn items(v: &Value, verb: &str) -> Result<Vec<Value>, Stop> {
    match v {
        Value::List(l) => Ok(l.to_vec()),
        Value::Range(r) if r.len() > i128::from(MAX_ELEMENTS) => Err(too_many_elements()),
        Value::Receiver(_) => unreachable!("a receiving end is walked by native/walk.rs"),
        Value::Range(r) => Ok((0..r.len())
            .map(|i| Value::Int(r.start.wrapping_add(i as i64)))
            .collect()),
        _ => Err(not_iterable(verb, v)),
    }
}

/// A predicate's result, which must be a bool (E0505).
pub fn want_bool(r: Value) -> Result<bool, Stop> {
    match r {
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

/// E0403: argument `name` must be a non-empty str, showing the value
/// given when it is the empty string (`got ""`) and the type otherwise,
/// as [`not_count`] shows a negative int.
pub fn not_nonempty(name: &str, v: &Value) -> Stop {
    crate::value::index::not_nonempty(name, v).into()
}

/// E0403: argument `name` must be a non-negative int, showing the value
/// given (a negative int, or a value of another kind).
pub fn not_count(name: &str, v: &Value) -> Stop {
    fail(
        "E0403",
        vec![
            s(name),
            s("a non-negative int"),
            Arg::Str(crate::value::format(v)),
        ],
    )
}

/// The map argument, or E0506 with `verb`.
pub fn need_map<'a>(verb: &str, v: &'a Value) -> Result<&'a Rc<crate::value::Map>, Stop> {
    match v {
        Value::Map(m) => Ok(m),
        _ => Err(not_iterable(verb, v)),
    }
}

/// A map key argument, or E0514 (SP21, D13): the same error as `m[k]`
/// and `k in m` give.
pub fn need_key(v: &Value) -> Result<&Rc<Text>, Stop> {
    match v {
        Value::Str(k) => Ok(k),
        _ => Err(Stop::Error(Box::new(crate::value::index::not_a_key(v)))),
    }
}

pub fn need_int(name: &str, v: &Value) -> Result<i64, Stop> {
    match v {
        Value::Int(n) => Ok(*n),
        _ => Err(wrong_arg(name, "int", v)),
    }
}

/// A count or a limit: a non-negative int, else E0403 (`must be a
/// non-negative int, got -1` for a negative one; SP15, D11).
pub fn need_count(name: &str, v: &Value) -> Result<usize, Stop> {
    match v {
        Value::Int(n) if *n >= 0 => Ok(*n as usize),
        Value::Int(_) => Err(not_count(name, v)),
        _ => Err(wrong_arg(name, "int", v)),
    }
}

/// A duration to wait or to allow: a non-negative dur, else E0403
/// (`must be a non-negative dur, got -1s` for a negative one; SP15, D11).
/// A wait that may come out negative says so with `max(d, 0s)`.
pub fn need_dur(name: &str, v: &Value) -> Result<i64, Stop> {
    match v {
        Value::Dur(d) if *d >= 0 => Ok(*d),
        Value::Dur(_) => Err(fail(
            "E0403",
            vec![
                s(name),
                s("a non-negative dur"),
                Arg::Str(crate::value::format(v)),
            ],
        )),
        _ => Err(wrong_arg(name, "dur", v)),
    }
}

/// A string argument as text: valid UTF-8, or E0518. For an argument
/// that only makes sense as text (a pattern, a format, a URL); the rest
/// take [`need_bytes`] (L4).
pub fn need_str<'a>(name: &str, v: &'a Value) -> Result<&'a str, Stop> {
    let t = need_text(name, v)?;
    t.as_str()
        .ok_or_else(|| not_utf8(&format!("argument `{name}`"), t))
}

/// E0518: `what` needs text, and `t` is not valid UTF-8.
pub fn not_utf8(what: &str, t: &Text) -> Stop {
    fail(
        "E0518",
        vec![
            s(what),
            Arg::Str(crate::value::format(&Value::Str(Rc::new(t.clone())))),
        ],
    )
}

/// A string argument, whatever its bytes.
pub fn need_text<'a>(name: &str, v: &'a Value) -> Result<&'a Rc<Text>, Stop> {
    match v {
        Value::Str(x) => Ok(x),
        _ => Err(wrong_arg(name, "str", v)),
    }
}

/// A string argument's bytes.
pub fn need_bytes<'a>(name: &str, v: &'a Value) -> Result<&'a [u8], Stop> {
    Ok(need_text(name, v)?.as_bytes())
}

/// Checks that every named argument is a string, in order: its bytes.
pub fn bytes_args<'a, const N: usize>(
    args: &'a [Value],
    names: [&str; N],
) -> Result<[&'a [u8]; N], Stop> {
    let mut out: [&[u8]; N] = [&[]; N];
    for (i, n) in names.iter().enumerate() {
        out[i] = need_bytes(n, &args[i])?;
    }
    Ok(out)
}

/// A `dur | nil` argument: `None` for nil; a negative one is E0403 as
/// for [`need_dur`].
pub fn opt_dur(name: &str, v: &Value) -> Result<Option<i64>, Stop> {
    match v {
        Value::Nil => Ok(None),
        Value::Dur(_) => need_dur(name, v).map(Some),
        _ => Err(wrong_arg(name, "dur | nil", v)),
    }
}

/// A `str | nil` argument of text.
pub fn opt_str<'a>(name: &str, v: &'a Value) -> Result<Option<&'a str>, Stop> {
    match v {
        Value::Nil => Ok(None),
        Value::Str(_) => need_str(name, v).map(Some),
        _ => Err(wrong_arg(name, "str | nil", v)),
    }
}

/// A `str | nil` argument's bytes.
pub fn opt_bytes<'a>(name: &str, v: &'a Value) -> Result<Option<&'a [u8]>, Stop> {
    match v {
        Value::Nil => Ok(None),
        Value::Str(x) => Ok(Some(x.as_bytes())),
        _ => Err(wrong_arg(name, "str | nil", v)),
    }
}

/// A list of strings argument: each one's bytes.
pub fn str_list(name: &str, v: &Value) -> Result<Vec<Vec<u8>>, Stop> {
    let Value::List(l) = v else {
        return Err(wrong_arg(name, "a list of strings", v));
    };
    l.iter()
        .map(|item| match item {
            Value::Str(x) => Ok(x.to_vec()),
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

    /// LB29: text counted a piece at a time, cut anywhere (inside a
    /// character, between invalid bytes), has the characters of the
    /// whole.
    #[test]
    fn characters_counted_in_pieces_add_up() {
        let text: Vec<u8> = [
            "a\u{e9}\u{d55c}\u{1f600}".as_bytes(),
            b"\xff\xe3\x82x\x80\x80\x80\x80\xf0\x9f",
            "\u{1f600}\u{d55c}b".as_bytes(),
            b"\xc3",
        ]
        .concat();
        let whole = fung_front::text::char_count(&text);
        for step in 1..=7 {
            let mut c = CharCount::default();
            let mut n = 0;
            let mut end = 0;
            while end < text.len() {
                end = (end + step).min(text.len());
                n = c.count(&text[..end]);
            }
            assert_eq!(n, whole, "pieces of {step}");
        }
    }

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
        // D63: the one length check every path that builds a list makes.
        assert!(check_len(MAX_ELEMENTS).is_ok());
        assert!(check_len(MAX_ELEMENTS + 1).is_err());
    }
}
