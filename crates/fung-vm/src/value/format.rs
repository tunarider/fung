use fung_front::consts::{format_dur, format_float, quote_str};
use fung_front::diag::quote;
use fung_front::unicode::{is_name_continue, is_name_start};

use super::Value;

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
                    if is_plain_key(k) {
                        out.push_str(k);
                    } else {
                        out.push_str(&quote_str(k));
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
        Value::Str(s) => quote_str(s),
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
        Value::File(f) => format!("<file {}>", quote(&f.path)),
        Value::Conn(_) => "<conn>".into(),
        Value::Listener(_) => "<listener>".into(),
        Value::List(_) | Value::Map(_) | Value::Tuple(_) | Value::Tag(_) => {
            unreachable!("collections are written by `write`")
        }
    }
}

/// Whether a map key can be written without quotes: it reads as a name
/// or a tag (01).
fn is_plain_key(k: &str) -> bool {
    let mut cs = k.chars();
    let Some(first) = cs.next() else {
        return false;
    };
    (first == '_' || is_name_start(first)) && cs.all(is_name_continue)
}

/// What `print` and `${}` use: strings bare, the rest in literal form.
pub fn format_for_print(v: &Value) -> String {
    match v {
        Value::Str(s) => s.to_string(),
        _ => format(v),
    }
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
