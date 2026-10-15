use fung_front::consts::{format_dur, format_float, quote_str};
use fung_front::unicode::{is_digit, is_letter, is_print};

use super::Value;

/// Renders a value in fung literal form.
pub fn format(v: &Value) -> String {
    match v {
        Value::Int(n) => n.to_string(),
        Value::Float(f) => format_float(*f),
        Value::Bool(b) => b.to_string(),
        Value::Nil => "nil".into(),
        Value::Str(s) => quote_str(s),
        Value::Sym(s) => format!("'{}", s.name()),
        Value::Dur(d) => format_dur(*d),
        Value::Tuple(items) => {
            if items.len() == 1 {
                format!("({},)", format(&items[0]))
            } else {
                format!("({})", format_items(items))
            }
        }
        Value::Tag(t) => format!("{}({})", t.name.name(), format_items(&t.args)),
        Value::Range(r) => {
            let op = if r.inclusive { "..=" } else { ".." };
            format!("{}{}{}", r.start, op, r.end)
        }
        Value::Native(n) if n.name.is_empty() => "<fn>".into(),
        Value::Native(n) => format!("<fn {}>", n.name),
        Value::List(l) => {
            let items: Vec<String> = l.iter().map(format).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Map(m) => {
            let parts: Vec<String> = m
                .iter()
                .map(|(k, v)| {
                    let key = if is_plain_key(k) {
                        k.into()
                    } else {
                        quote_str(k)
                    };
                    format!("{key}: {}", format(v))
                })
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        Value::Closure(c) if c.proto.name.is_empty() => "<fn>".into(),
        Value::Closure(c) => format!("<fn {}>", c.proto.name),
        // Placeholder until modules are ported.
        Value::Module(m) => format!("<module {}>", m.name),
        Value::Task(_) => "<task>".into(),
        Value::Channel(_) => "<channel>".into(),
        Value::Agent(_) => "<agent>".into(),
        Value::File(f) => format!("<file {}>", go_quote(&f.path)),
        Value::Caught(_) => "<error>".into(),
    }
}

/// Go `strconv.Quote`, which Go uses for a file handle's path (not fung's
/// own string form): `\x01`, `\u00a0`, `\U000e0001` for what does not
/// print. A path is valid UTF-8 here; Go would show a bad byte as `\xff`
/// where the Rust string holds U+FFFD.
fn go_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if is_print(c) => out.push(c),
            '\u{7}' => out.push_str("\\a"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{b}' => out.push_str("\\v"),
            c if c < ' ' || c == '\u{7f}' => out.push_str(&format!("\\x{:02x}", c as u32)),
            c if (c as u32) < 0x10000 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push_str(&format!("\\U{:08x}", c as u32)),
        }
    }
    out.push('"');
    out
}

fn format_items(items: &[Value]) -> String {
    items.iter().map(format).collect::<Vec<_>>().join(", ")
}

/// Whether a map key can be written without quotes: a letter or `_`, then
/// letters, digits and `_`, by Go's `unicode.IsLetter` and `IsDigit`.
fn is_plain_key(k: &str) -> bool {
    let mut cs = k.chars();
    let Some(first) = cs.next() else {
        return false;
    };
    (first == '_' || is_letter(first)) && cs.all(|c| c == '_' || is_letter(c) || is_digit(c))
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
        Value::List(_) => "list".into(),
        Value::Map(_) => "map".into(),
        Value::Tuple(_) => "tuple".into(),
        Value::Tag(t) => t.name.name().into(),
        Value::Range(_) => "range".into(),
        Value::Closure(_) | Value::Native(_) => "fn".into(),
        Value::Module(_) => "module".into(),
        Value::Task(_) => "task".into(),
        Value::Channel(_) => "channel".into(),
        Value::Agent(_) => "agent".into(),
        Value::File(_) => "file".into(),
        Value::Caught(_) => "error".into(),
    }
}
