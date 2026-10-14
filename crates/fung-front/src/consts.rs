//! Constant values and fung literal formatting. Port of the scalar parts of
//! Go `internal/value/format.go` and `dur.go`; output must match byte for byte.

/// A compile-time constant (the scalar kinds the front end can produce).
#[derive(Clone, Debug)]
pub enum Const {
    Int(i64),
    Float(f64),
    Bool(bool),
    Nil,
    Str(String),
    Sym(String),
    /// Nanoseconds.
    Dur(i64),
    Builtin(String),
}

/// Constant-pool equality, following Go `Proto.Const`: same type and equal
/// value. So `Int(1)` and `Float(1.0)` differ, `Float` compares with `==`
/// (NaN never equals, `0.0 == -0.0`), and builtins compare by name.
impl PartialEq for Const {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Const::Int(a), Const::Int(b)) => a == b,
            (Const::Float(a), Const::Float(b)) => a == b,
            (Const::Bool(a), Const::Bool(b)) => a == b,
            (Const::Nil, Const::Nil) => true,
            (Const::Str(a), Const::Str(b)) => a == b,
            (Const::Sym(a), Const::Sym(b)) => a == b,
            (Const::Dur(a), Const::Dur(b)) => a == b,
            (Const::Builtin(a), Const::Builtin(b)) => a == b,
            _ => false,
        }
    }
}

/// Renders a constant in fung literal form.
pub fn format(c: &Const) -> String {
    match c {
        Const::Int(n) => n.to_string(),
        Const::Float(f) => format_float(*f),
        Const::Bool(true) => "true".into(),
        Const::Bool(false) => "false".into(),
        Const::Nil => "nil".into(),
        Const::Str(s) => quote_str(s),
        Const::Sym(s) => format!("'{s}"),
        Const::Dur(ns) => format_dur(*ns),
        Const::Builtin(n) => format!("<fn {n}>"),
    }
}

/// What `print()` and `${}` use: strings bare, the rest in literal form.
pub fn format_for_print(c: &Const) -> String {
    match c {
        Const::Str(s) => s.clone(),
        _ => format(c),
    }
}

/// Plain notation for decimal exponents in [-4, 21), exponent notation
/// outside (`1e21`, `1e-07`), always showing a `.` or exponent.
pub fn format_float(f: f64) -> String {
    // Go parses the exponent out of its 'e' form; NaN/Inf have none, so they
    // fall through to plain notation plus ".0" ("NaN.0", "+Inf.0").
    if f.is_nan() {
        return "NaN.0".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "+Inf.0" } else { "-Inf.0" }.into();
    }
    let e = format!("{f:e}"); // shortest round trip: d.ddde<exp>
    let (mantissa, exp) = e.split_once('e').expect("exponent form has an e");
    let exp: i32 = exp.parse().expect("valid exponent");
    if !(-4..21).contains(&exp) {
        let sign = if exp < 0 { "-" } else { "" };
        return format!("{mantissa}e{sign}{:02}", exp.abs());
    }
    let mut s = format!("{f}");
    if !s.contains('.') {
        s.push_str(".0");
    }
    s
}

/// Duration literal: descending h, m, s, ms components, the last possibly
/// fractional (`1h30m`, `1s500ms`, `0.5ms`, `0s`).
pub fn format_dur(ns: i64) -> String {
    if ns == 0 {
        return "0s".into();
    }
    const SEC: u64 = 1_000_000_000;
    let mut out = String::new();
    if ns < 0 {
        out.push('-');
    }
    let ns = ns.unsigned_abs();
    let (h, ns) = (ns / (3600 * SEC), ns % (3600 * SEC));
    let (m, ns) = (ns / (60 * SEC), ns % (60 * SEC));
    let (s, ns) = (ns / SEC, ns % SEC);
    if h > 0 {
        out.push_str(&format!("{h}h"));
    }
    if m > 0 {
        out.push_str(&format!("{m}m"));
    }
    if s > 0 {
        out.push_str(&format!("{s}s"));
    }
    if ns > 0 {
        out.push_str(&format!("{}ms", ns as f64 / 1_000_000.0));
    }
    out
}

/// A string as a fung literal using only the escapes the lexer allows.
/// `$` is escaped only where it would start an interpolation.
pub fn quote_str(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::from("\"");
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '$' if chars.get(i + 1) == Some(&'{') => out.push_str("\\$"),
            c if c != ' ' && !crate::diag::is_print(c) => {
                out.push_str(&format!("\\u{{{:X}}}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Renders a union type for messages: `int | nil`.
pub fn type_text(alts: &[String]) -> String {
    alts.join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_print_like_go() {
        assert_eq!(format_float(15000000000.0), "15000000000.0");
        assert_eq!(format_float(0.1), "0.1");
        assert_eq!(format_float(1e21), "1e21");
        assert_eq!(format_float(1e-7), "1e-07");
        assert_eq!(format_float(5.0), "5.0");
        assert_eq!(format_float(0.30000000000000004), "0.30000000000000004");
    }

    #[test]
    fn durations_and_strings() {
        assert_eq!(format_dur(90 * 60 * 1_000_000_000), "1h30m");
        assert_eq!(format(&Const::Str("a\tb\"c ".into())), "\"a\\tb\\\"c \"");
        assert_eq!(format(&Const::Sym("sym".into())), "'sym");
        assert_eq!(format(&Const::Builtin("map".into())), "<fn map>");
        assert_eq!(format(&Const::Nil), "nil");
        assert_eq!(format_for_print(&Const::Str("fs".into())), "fs");
    }

    #[test]
    fn format_literals_table() {
        let cases: Vec<(Const, &str)> = vec![
            (Const::Int(0), "0"),
            (Const::Int(-42), "-42"),
            (Const::Int(i64::MAX), "9223372036854775807"),
            (Const::Float(5.0), "5.0"),
            (Const::Float(2.5), "2.5"),
            (Const::Float(-0.25), "-0.25"),
            (Const::Float(1e21), "1e21"),
            (Const::Float(1e-7), "1e-07"),
            (Const::Float(123456789.0), "123456789.0"),
            (Const::Float(1e6), "1000000.0"),
            (Const::Bool(true), "true"),
            (Const::Bool(false), "false"),
            (Const::Nil, "nil"),
            (Const::Builtin("print".into()), "<fn print>"),
        ];
        for (c, want) in cases {
            assert_eq!(format(&c), want, "{c:?}");
        }
    }

    #[test]
    fn shortest_round_trip() {
        let (a, b) = (std::hint::black_box(0.1), 0.2);
        assert_eq!(format(&Const::Float(a + b)), "0.30000000000000004");
    }

    #[test]
    fn float_edges() {
        assert_eq!(format_float(1e20), "100000000000000000000.0");
        assert_eq!(format_float(0.0001), "0.0001");
        assert_eq!(format_float(0.00001), "1e-05");
        assert_eq!(format_float(1.5e300), "1.5e300");
        assert_eq!(format_float(-1.5e-10), "-1.5e-10");
        assert_eq!(format_float(-0.0), "-0.0");
    }

    #[test]
    fn dur_format_table() {
        const MS: i64 = 1_000_000;
        let cases: [(i64, &str); 7] = [
            (90 * 60 * 1_000_000_000, "1h30m"),
            (1500 * MS, "1s500ms"),
            (500_000, "0.5ms"),
            (0, "0s"),
            (-5_000_000_000, "-5s"),
            (2 * 3600 * 1_000_000_000 + 3_000_000_000, "2h3s"),
            (MS + 500_000, "1.5ms"),
        ];
        for (d, want) in cases {
            assert_eq!(format(&Const::Dur(d)), want);
        }
    }

    #[test]
    fn string_quoting() {
        assert_eq!(quote_str("a\nb\r\\"), "\"a\\nb\\r\\\\\"");
        assert_eq!(quote_str("${x} $y $"), "\"\\${x} $y $\"");
        assert_eq!(quote_str("\u{1}é한"), "\"\\u{1}é한\"");
    }

    #[test]
    fn const_equality_keeps_types_apart() {
        assert_ne!(Const::Int(1), Const::Float(1.0));
        assert_ne!(Const::Str("a".into()), Const::Sym("a".into()));
        assert_eq!(Const::Float(0.0), Const::Float(-0.0));
        assert_ne!(Const::Float(f64::NAN), Const::Float(f64::NAN));
        assert_eq!(Const::Builtin("x".into()), Const::Builtin("x".into()));
    }

    #[test]
    fn nbsp_is_escaped() {
        assert_eq!(quote_str("a\u{A0}b"), "\"a\\u{A0}b\"");
    }

    #[test]
    fn type_text_joins() {
        assert_eq!(type_text(&["int".into(), "nil".into()]), "int | nil");
    }
}
