//! `_native.arg`: the argument checks of the modules written in fung
//! (`cli`, `conc`, `http`) that a type annotation cannot say and fung
//! code cannot raise itself: a count, a positive count, a duration that
//! is not negative, a list of one type, a map of strings (SP18), a token
//! and a map of header lines (LB34). Each
//! gives `nil` (`count` its count), or E0403 naming the argument it is
//! told, as a native's own check would (06: the error names the user's
//! argument). The prelude's counts use `count` too. `header_problem` and
//! `is_token` give the same rules to `http`'s server, which answers a bad
//! handler's headers with 500 instead.

use std::rc::Rc;
use std::sync::OnceLock;

use fung_front::diag::Arg;

use super::nat::{self, native, param};
use crate::value::{Args, Module, Native, Value, type_name};
use crate::vm::{Stop, Task};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The `_native.arg` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.get_or_init(|| {
        vec![
            (
                "count",
                native("count", count_fn, vec![param("name"), param("n")]),
            ),
            (
                "positive",
                native("positive", positive_fn, vec![param("name"), param("n")]),
            ),
            (
                "dur",
                native("dur", dur_fn, vec![param("name"), param("d")]),
            ),
            (
                "list_of",
                native(
                    "list_of",
                    list_of_fn,
                    vec![param("name"), param("xs"), param("kind")],
                ),
            ),
            (
                "str_map",
                native("str_map", str_map_fn, vec![param("name"), param("m")]),
            ),
            (
                "token",
                native("token", token_fn, vec![param("name"), param("s")]),
            ),
            (
                "headers",
                native("headers", headers_fn, vec![param("name"), param("m")]),
            ),
            (
                "header_problem",
                native("header_problem", header_problem_fn, vec![param("m")]),
            ),
            (
                "is_token",
                native("is_token", is_token_fn, vec![param("s")]),
            ),
        ]
    });
    let mut m = Module::new("arg", "_native.arg");
    for (name, n) in natives {
        m.set(name, Value::Native(n));
    }
    Rc::new(m)
}

/// `count(name, n)`: `n` when it is an int from 0 (`http`'s `max_body`;
/// the prelude's `take`, `drop`, `pad_left`, `pad_right` and `iterate`,
/// SP15, D11).
fn count_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let name = nat::need_str("name", &a[0])?;
    nat::need_count(name, &a[1])?;
    Ok(a[1].clone())
}

/// `positive(name, n)`: an int from 1 (`conc.pmap`'s `limit`,
/// `conc.retry`'s `times`; D11).
fn positive_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let name = nat::need_str("name", &a[0])?;
    match &a[1] {
        Value::Int(n) if *n >= 1 => Ok(Value::Nil),
        Value::Int(_) => Err(nat::fail(
            "E0403",
            vec![
                nat::s(name),
                nat::s("a positive int"),
                Arg::Str(crate::value::format(&a[1])),
            ],
        )),
        v => Err(nat::wrong_arg(name, "int", v)),
    }
}

/// `dur(name, d)`: a dur from `0s` (`http`'s `timeout` and `drain`,
/// `conc.retry`'s `delay`; SP15).
fn dur_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let name = nat::need_str("name", &a[0])?;
    nat::need_dur(name, &a[1]).map(|_| Value::Nil)
}

/// `list_of(name, xs, kind)`: a list whose elements are all of the type
/// named `kind` (`cli.parse`'s `argv` and `bools` hold strs,
/// `conc.fan_in`'s `rxs` receiving ends); the error names the first
/// that is not.
fn list_of_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let name = nat::need_str("name", &a[0])?;
    let kind = nat::need_str("kind", &a[2])?;
    let Value::List(xs) = &a[1] else {
        return Err(nat::wrong_arg(name, "list", &a[1]));
    };
    match xs.iter().find(|v| type_name(v) != kind) {
        Some(v) => Err(holding(name, &format!("a list of {kind}"), "list", v)),
        None => Ok(Value::Nil),
    }
}

/// `str_map(name, m)`: a map whose values are all strs (`http`'s
/// `headers`); the error names the first that is not, in the map's order.
fn str_map_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let name = nat::need_str("name", &a[0])?;
    let Value::Map(m) = &a[1] else {
        return Err(nat::wrong_arg(name, "map", &a[1]));
    };
    match m.iter().find(|(_, v)| !matches!(v, Value::Str(_))) {
        Some((_, v)) => Err(holding(name, "a map of strings", "map", v)),
        None => Ok(Value::Nil),
    }
}

/// A token: one or more token characters (RFC 9110 5.6.2).
fn is_token(s: &[u8]) -> bool {
    !s.is_empty()
        && s.iter()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(b))
}

/// `token(name, s)`: a str of token characters, at least one (RFC 9110
/// 5.6.2; `http.request`'s `method`, LB34).
fn token_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let name = nat::need_str("name", &a[0])?;
    let s = nat::need_bytes(name, &a[1])?;
    if is_token(s) {
        return Ok(Value::Nil);
    }
    Err(nat::fail(
        "E0403",
        vec![
            nat::s(name),
            nat::s("a token (RFC 9110 5.6.2)"),
            Arg::Str(crate::value::format(&a[1])),
        ],
    ))
}

/// `headers(name, m)`: a map of strs that can be header lines of a
/// request (`http`'s `headers`, LB34; the rule is [`header_problem`]),
/// none of them a framing header (`Content-Length`, `Transfer-Encoding`,
/// `Trailer`, in any case: the request writes its own). The error names
/// the first header that is not.
fn headers_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let name = nat::need_str("name", &a[0])?;
    let Value::Map(m) = &a[1] else {
        return Err(nat::wrong_arg(name, "map", &a[1]));
    };
    let bad = |want: &str, got: String| {
        nat::fail("E0403", vec![nat::s(name), nat::s(want), Arg::Str(got)])
    };
    Err(match header_problem(m) {
        // the request frames its body itself (LB34, D27): a framing
        // header of the user's would be dropped or contradict it
        None => match m.iter().find(|(k, _)| is_framing(k)) {
            None => return Ok(Value::Nil),
            Some((k, _)) => bad(
                "a map without Content-Length, Transfer-Encoding or Trailer (the request frames its body itself)",
                format!("`{}`", show(k)),
            ),
        },
        Some(Header::NotStr(_, v)) => holding(name, "a map of strings", "map", v),
        Some(Header::NotToken(k)) => bad(
            "a map whose names are tokens (RFC 9110 5.6.2)",
            format!("the name {}", fung_front::consts::quote_bytes(k)),
        ),
        Some(Header::Control(k, v)) => bad(
            "a map whose values have no control characters but tab",
            format!("{} for `{}`", crate::value::format(v), show(k)),
        ),
        Some(Header::Twice(first, k)) => bad(
            "a map without names that differ only in case",
            format!("`{}` and `{}`", show(first), show(k)),
        ),
    })
}

/// `header_problem(m)`: nil when the map of headers can be written, else
/// the reason (the `http` server's for a handler's answer, which it
/// answers with 500). The rule is the client's `headers` check
/// ([`header_problem`]), so the two cannot drift.
fn header_problem_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let Value::Map(m) = &a[0] else {
        return Err(nat::wrong_arg("m", "map", &a[0]));
    };
    Ok(match header_problem(m) {
        None => Value::Nil,
        Some(Header::NotStr(k, v)) => Value::str(&format!(
            "header `{}` must be str, got {}",
            show(k),
            type_name(v)
        )),
        Some(Header::NotToken(k)) => {
            Value::str(&format!("header name `{}` is not a token", show(k)))
        }
        Some(Header::Control(k, _)) => Value::str(&format!(
            "header `{}` has a control character in its value",
            show(k)
        )),
        Some(Header::Twice(_, k)) => {
            Value::str(&format!("header `{}` is given twice", canonical(k)))
        }
    })
}

/// What keeps a map of headers from being written as header lines.
enum Header<'a> {
    /// A value that is not a str.
    NotStr(&'a [u8], &'a Value),
    /// A name that is not a token (RFC 9110 5.6.2).
    NotToken(&'a [u8]),
    /// A value with a control character but tab (RFC 9110 5.5: no line
    /// can be smuggled in, as with a CR LF).
    Control(&'a [u8], &'a Value),
    /// Two names that differ only in ASCII case (a header name is
    /// case-insensitive, so the two would be one header): the first and
    /// the second.
    Twice(&'a [u8], &'a [u8]),
}

/// The first problem of a map of headers, in the order: a value that is
/// not a str, a name that is not a token or a value with a control
/// character, a name given twice. `http`'s client (E0403) and server
/// (500) both judge headers by it (LB3, LB34).
fn header_problem(m: &crate::value::Map) -> Option<Header<'_>> {
    if let Some((k, v)) = m.iter().find(|(_, v)| !matches!(v, Value::Str(_))) {
        return Some(Header::NotStr(k, v));
    }
    for (k, v) in m.iter() {
        if !is_token(k) {
            return Some(Header::NotToken(k));
        }
        if let Value::Str(t) = v
            && t.as_bytes()
                .iter()
                .any(|&b| (b < 32 && b != b'\t') || b == 127)
        {
            return Some(Header::Control(k, v));
        }
    }
    let mut seen: std::collections::HashMap<Vec<u8>, &[u8]> = std::collections::HashMap::new();
    for (k, _) in m.iter() {
        if let Some(first) = seen.insert(k.to_ascii_lowercase(), k) {
            return Some(Header::Twice(first, k));
        }
    }
    None
}

/// A header name in its usual case: `content-type` → `Content-Type`.
fn canonical(k: &[u8]) -> String {
    let mut up = true;
    let out: Vec<u8> = k
        .iter()
        .map(|&b| {
            let c = if up {
                b.to_ascii_uppercase()
            } else {
                b.to_ascii_lowercase()
            };
            up = b == b'-';
            c
        })
        .collect();
    show(&out)
}

/// A header that says where a message's body ends (RFC 9112 6).
fn is_framing(k: &[u8]) -> bool {
    [&b"content-length"[..], b"transfer-encoding", b"trailer"]
        .iter()
        .any(|f| k.eq_ignore_ascii_case(f))
}

fn show(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// `is_token(s)`: whether `s` is a token (RFC 9110 5.6.2): `http`'s
/// methods and header names, client and server.
fn is_token_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    Ok(Value::Bool(is_token(nat::need_bytes("s", &a[0])?)))
}

/// E0403: argument `name` must be `want`, got a `kind` holding `v`'s type.
fn holding(name: &str, want: &str, kind: &str, v: &Value) -> Stop {
    nat::fail(
        "E0403",
        vec![
            nat::s(name),
            nat::s(want),
            Arg::Str(format!("a {kind} holding {}", type_name(v))),
        ],
    )
}

#[cfg(test)]
#[path = "arg_tests.rs"]
mod tests;
