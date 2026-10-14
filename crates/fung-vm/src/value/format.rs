use fung_front::consts::{format_dur, format_float, quote_str};

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
        Value::Module(_) => "<module>".into(),
    }
}

fn format_items(items: &[Value]) -> String {
    items.iter().map(format).collect::<Vec<_>>().join(", ")
}

/// Whether a map key can be written without quotes: a letter or `_`, then
/// letters, digits and `_`. Like the lexer, Unicode letters and digits are
/// approximated with `is_alphabetic` and `is_numeric`.
fn is_plain_key(k: &str) -> bool {
    let mut cs = k.chars();
    let Some(first) = cs.next() else {
        return false;
    };
    (first == '_' || first.is_alphabetic())
        && cs.all(|c| c == '_' || c.is_alphabetic() || c.is_numeric())
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
    }
}
