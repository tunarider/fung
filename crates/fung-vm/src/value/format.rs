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
        // Placeholders until lists, maps, closures and modules are ported.
        Value::List(_) => "<list>".into(),
        Value::Map(_) => "<map>".into(),
        Value::Closure(_) => "<fn>".into(),
        Value::Module(_) => "<module>".into(),
    }
}

fn format_items(items: &[Value]) -> String {
    items.iter().map(format).collect::<Vec<_>>().join(", ")
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
