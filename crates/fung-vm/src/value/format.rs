use fung_front::consts::{format_dur, format_float, is_plain_key, quote_bytes, write_quoted};

use super::{Sym, Value};

/// Renders a value in fung literal form.
pub fn format(v: &Value) -> String {
    if !is_collection(v) {
        return leaf_text(v);
    }
    let mut out = String::new();
    write(v, &mut out);
    out
}

/// A list, map, tuple or tag: a value that holds others, walked with a
/// stack. Everything else is shown, compared and hashed at once (the
/// walks' fast path: most values are not collections).
pub(super) fn is_collection(v: &Value) -> bool {
    matches!(
        v,
        Value::List(_) | Value::Map(_) | Value::Tuple(_) | Value::Tag(_)
    )
}

/// The items of a collection being written, one at a time.
pub(super) enum Items<'a> {
    List(super::ListIter<'a>),
    Slice(std::slice::Iter<'a, Value>),
}

impl<'a> Iterator for Items<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<&'a Value> {
        match self {
            Items::List(it) => it.next(),
            Items::Slice(it) => it.next(),
        }
    }
}

/// A collection whose items are being written: what is left of it and
/// what closes it.
enum Open<'a> {
    Seq {
        items: Items<'a>,
        first: bool,
        close: &'static str,
    },
    Map {
        entries: super::MapIter<'a>,
        first: bool,
    },
}

/// Writes `v` in fung literal form at the end of `out`. A collection's
/// items are written one after another into the one buffer, with the
/// collections still open on an explicit stack (L9): the Rust stack does
/// not grow with the value's depth, and each character is written once,
/// so a value a million levels deep takes linear time.
pub(crate) fn write(v: &Value, out: &mut String) {
    let mut open: Vec<Open<'_>> = Vec::new();
    let mut next = Some(v);
    loop {
        if let Some(v) = next.take()
            && let Some(o) = write_head(v, out)
        {
            open.push(o);
        }
        let Some(top) = open.last_mut() else {
            return;
        };
        match top {
            Open::Seq {
                items,
                first,
                close,
            } => match items.next() {
                Some(x) => {
                    if !std::mem::take(first) {
                        out.push_str(", ");
                    }
                    next = Some(x);
                }
                None => {
                    out.push_str(close);
                    open.pop();
                }
            },
            Open::Map { entries, first } => match entries.next() {
                Some((k, x)) => {
                    if !std::mem::take(first) {
                        out.push_str(", ");
                    }
                    match k.as_str().filter(|k| is_plain_key(k)) {
                        Some(k) => out.push_str(k),
                        None => write_quoted(k, out),
                    }
                    out.push_str(": ");
                    next = Some(x);
                }
                None => {
                    out.push('}');
                    open.pop();
                }
            },
        }
    }
}

/// Writes what `v` shows before its items, or all of it if it has none;
/// a collection comes back to have its items written.
fn write_head<'a>(v: &'a Value, out: &mut String) -> Option<Open<'a>> {
    let seq = |items, close| {
        Some(Open::Seq {
            items,
            first: true,
            close,
        })
    };
    match v {
        Value::Tuple(items) => {
            out.push('(');
            let close = if items.len() == 1 { ",)" } else { ")" };
            seq(Items::Slice(items.iter()), close)
        }
        Value::Tag(t) => {
            out.push_str(t.name.name());
            out.push('(');
            seq(Items::Slice(t.args.iter()), ")")
        }
        Value::List(l) => {
            out.push('[');
            seq(Items::List(l.iter()), "]")
        }
        Value::Map(m) => {
            out.push('{');
            Some(Open::Map {
                entries: m.iter(),
                first: true,
            })
        }
        Value::Int(n) => {
            use std::fmt::Write;
            let _ = write!(out, "{n}");
            None
        }
        // Into the one buffer, not through a string of its own (VA13).
        Value::Str(s) => {
            write_quoted(s, out);
            None
        }
        v => {
            out.push_str(&leaf_text(v));
            None
        }
    }
}

/// The text of a value that holds no other.
fn leaf_text(v: &Value) -> String {
    match v {
        Value::Int(n) => n.to_string(),
        Value::Float(f) => format_float(*f),
        Value::Bool(b) => b.to_string(),
        Value::Nil => "nil".into(),
        Value::Str(s) => quote_bytes(s),
        Value::Sym(s) => format!("'{}", s.name()),
        Value::Dur(d) => format_dur(*d),
        Value::Time(t) => t.iso(),
        Value::Range(r) => {
            let op = if r.inclusive { "..=" } else { ".." };
            format!("{}{}{}", r.start, op, r.end)
        }
        Value::Native(n) if n.name.is_empty() => "<fn>".into(),
        Value::Native(n) => format!("<fn {}>", n.name),
        Value::Closure(c) if c.proto.name.is_empty() => "<fn>".into(),
        Value::Closure(c) => format!("<fn {}>", c.proto.name),
        Value::Module(m) => format!("<module {}>", m.name),
        Value::Task(_) => "<task>".into(),
        Value::Scope(_) => "<scope>".into(),
        Value::Atom(_) => "<atom>".into(),
        Value::Sender(_) => "<sender>".into(),
        Value::Receiver(_) => "<receiver>".into(),
        Value::File(f) => format!("<file {}>", quote_bytes(&f.path)),
        Value::Conn(_) => "<conn>".into(),
        Value::Listener(_) => "<listener>".into(),
        Value::List(_) | Value::Map(_) | Value::Tuple(_) | Value::Tag(_) => {
            unreachable!("collections are written by `write`")
        }
    }
}

/// `Ok` or `Err` when `v` is a result: interpolation and `join` refuse to
/// put one into text (SP2), so a forgotten unwrap does not become the
/// text `Ok(...)` in a path or a URL. `str` and `print` still show it.
pub fn result_tag(v: &Value) -> Option<&'static str> {
    match v {
        Value::Tag(t) => match t.name {
            Sym::OK => Some("Ok"),
            Sym::ERR => Some("Err"),
            _ => None,
        },
        _ => None,
    }
}

/// What `print` and `${}` use, at the end of `out`: strings bare (their
/// bytes as they are, L4), the rest in literal form.
pub fn write_for_print(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Str(s) => out.extend_from_slice(s),
        _ => out.extend_from_slice(format(v).as_bytes()),
    }
}

/// `write_for_print` as text, bytes outside UTF-8 as U+FFFD (tests).
#[cfg(test)]
pub fn format_for_print(v: &Value) -> String {
    let mut out = Vec::new();
    write_for_print(v, &mut out);
    String::from_utf8_lossy(&out).into_owned()
}

/// The name `type_of` reports; a tagged value is its tag name.
pub fn type_name(v: &Value) -> String {
    match v {
        Value::Int(_) => "int".into(),
        Value::Float(_) => "float".into(),
        Value::Bool(_) => "bool".into(),
        Value::Nil => "nil".into(),
        Value::Str(_) => "str".into(),
        Value::Sym(_) => "sym".into(),
        Value::Dur(_) => "dur".into(),
        Value::Time(_) => "time".into(),
        Value::List(_) => "list".into(),
        Value::Map(_) => "map".into(),
        Value::Tuple(_) => "tuple".into(),
        Value::Tag(t) => t.name.name().into(),
        Value::Range(_) => "range".into(),
        Value::Closure(_) | Value::Native(_) => "fn".into(),
        Value::Module(_) => "module".into(),
        Value::Task(_) => "task".into(),
        Value::Scope(_) => "scope".into(),
        Value::Atom(_) => "atom".into(),
        Value::Sender(_) => "sender".into(),
        Value::Receiver(_) => "receiver".into(),
        Value::File(_) => "file".into(),
        Value::Conn(_) => "conn".into(),
        Value::Listener(_) => "listener".into(),
    }
}
