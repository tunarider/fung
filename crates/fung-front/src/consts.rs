//! Constant values and fung literal formatting (08 "출력 형식"): the
//! scalar parts the VM's value printing shares.

use crate::unicode::{is_name_continue, is_name_start};

/// A compile-time constant: a scalar, or a literal list, map, tuple or
/// tag value whose items are all constants (folded by the compiler, L5).
#[derive(Clone, Debug)]
pub enum Const {
    Int(i64),
    Float(f64),
    Bool(bool),
    Nil,
    Str(String),
    /// A string that is not valid UTF-8 (`"\xff"`, L4).
    Bytes(Vec<u8>),
    Sym(String),
    /// Nanoseconds.
    Dur(i64),
    Builtin(String),
    /// The prelude's `args`: the run's argument list, which the VM fills
    /// in (empty when compiling).
    Args,
    /// The `dbg` a direct `dbg(e)` calls: it also takes the source text
    /// and place the compiler passes (08), which a program cannot pass
    /// itself (SP11). Shown as the prelude `dbg`.
    Dbg,
    List(Vec<Const>),
    /// Keys are `Str` or `Bytes`, distinct, in the literal's order.
    Map(Vec<(Const, Const)>),
    Tuple(Vec<Const>),
    /// A tag value: its name and arguments (`Ok(1)`).
    Tag(String, Vec<Const>),
    /// A range of constant ends: start, end, inclusive (VM10).
    Range(i64, i64, bool),
}

/// Constant-pool equality: same type and equal
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
            (Const::Bytes(a), Const::Bytes(b)) => a == b,
            (Const::Sym(a), Const::Sym(b)) => a == b,
            (Const::Dur(a), Const::Dur(b)) => a == b,
            (Const::Builtin(a), Const::Builtin(b)) => a == b,
            (Const::Args, Const::Args) => true,
            (Const::Dbg, Const::Dbg) => true,
            (Const::List(a), Const::List(b)) | (Const::Tuple(a), Const::Tuple(b)) => a == b,
            (Const::Map(a), Const::Map(b)) => a == b,
            (Const::Tag(n, a), Const::Tag(m, b)) => n == m && a == b,
            (Const::Range(a, b, i), Const::Range(c, d, j)) => (a, b, i) == (c, d, j),
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
        Const::Bytes(b) => quote_bytes(b),
        Const::Sym(s) => format!("'{s}"),
        Const::Dur(ns) => format_dur(*ns),
        Const::Builtin(n) => format!("<fn {n}>"),
        Const::Dbg => "<fn dbg>".into(),
        Const::Args => "args".into(),
        Const::List(items) => format!("[{}]", format_items(items)),
        Const::Tuple(items) if items.len() == 1 => format!("({},)", format(&items[0])),
        Const::Tuple(items) => format!("({})", format_items(items)),
        Const::Tag(name, args) => format!("{name}({})", format_items(args)),
        Const::Range(a, b, true) => format!("{a}..={b}"),
        Const::Range(a, b, false) => format!("{a}..{b}"),
        Const::Map(entries) => {
            let entries: Vec<String> = entries
                .iter()
                .map(|(k, v)| {
                    let k = match k {
                        Const::Str(s) if is_plain_key(s) => s.clone(),
                        k => format(k),
                    };
                    format!("{k}: {}", format(v))
                })
                .collect();
            format!("{{{}}}", entries.join(", "))
        }
    }
}

fn format_items(items: &[Const]) -> String {
    items.iter().map(format).collect::<Vec<_>>().join(", ")
}

/// Whether a map key can be written without quotes: it reads as a name
/// or a tag (01).
pub fn is_plain_key(k: &str) -> bool {
    let mut cs = k.chars();
    let Some(first) = cs.next() else {
        return false;
    };
    (first == '_' || is_name_start(first)) && cs.all(is_name_continue)
}

/// What `print()` and `${}` use: strings bare, the rest in literal form.
pub fn format_for_print(c: &Const) -> String {
    match c {
        Const::Str(s) => s.clone(),
        _ => format(c),
    }
}

/// Plain notation for decimal exponents in [-4, 21), exponent notation
/// outside (`1e21`, `1e-7`: the exponent as a literal writes it, without
/// padding or `+`), always showing a `.` or exponent.
pub fn format_float(f: f64) -> String {
    // the exponent is read from the 'e' form; NaN/Inf have none, so they
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
        return format!("{mantissa}e{exp}");
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
    quote_bytes(s.as_bytes())
}

/// A string in fung literal form: a byte outside valid UTF-8 shows as
/// `\xff`, so the text reads back as the same bytes (L4).
pub fn quote_bytes(b: &[u8]) -> String {
    let mut out = String::with_capacity(b.len() + 2);
    write_quoted(b, &mut out);
    out
}

/// [`quote_bytes`] at the end of `out`. A run of ASCII that needs no
/// escape goes in at once; only the rest is read a character at a time
/// (VA13).
pub fn write_quoted(b: &[u8], out: &mut String) {
    use crate::text::{Unit, first};
    use std::fmt::Write;
    // Printable ASCII but the three that may need a backslash.
    let plain = |c: &u8| (b' '..=b'~').contains(c) && !matches!(c, b'"' | b'\\' | b'$');
    out.reserve(b.len() + 2);
    out.push('"');
    let mut at = 0;
    while at < b.len() {
        let run = b[at..].iter().take_while(|c| plain(c)).count();
        if run > 0 {
            out.push_str(std::str::from_utf8(&b[at..at + run]).expect("ASCII"));
            at += run;
            continue;
        }
        let u = first(&b[at..]).expect("not at the end");
        at += u.byte_len();
        let c = match u {
            Unit::Byte(x) => {
                let _ = write!(out, "\\x{x:02x}");
                continue;
            }
            Unit::Char(c) => c,
        };
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '$' if b.get(at) == Some(&b'{') => out.push_str("\\$"),
            c if c != ' ' && !crate::unicode::is_print(c) => {
                let _ = write!(out, "\\u{{{:X}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Renders a union type for messages: `int | nil`.
pub fn type_text(alts: &[String]) -> String {
    alts.join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The quoting as first written, a character at a time: the fast
    /// writer must give the same text (VA13).
    fn quote_slowly(b: &[u8]) -> String {
        use crate::text::{Unit, units};
        let us: Vec<Unit> = units(b).map(|(_, u)| u).collect();
        let mut out = String::from("\"");
        for (i, &u) in us.iter().enumerate() {
            let c = match u {
                Unit::Byte(x) => {
                    out.push_str(&format!("\\x{x:02x}"));
                    continue;
                }
                Unit::Char(c) => c,
            };
            match c {
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                '\r' => out.push_str("\\r"),
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '$' if us.get(i + 1) == Some(&Unit::Char('{')) => out.push_str("\\$"),
                c if c != ' ' && !crate::unicode::is_print(c) => {
                    out.push_str(&format!("\\u{{{:X}}}", c as u32))
                }
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }

    #[test]
    fn quoting_writes_what_it_did_a_character_at_a_time() {
        let pieces: [&[u8]; 16] = [
            b"abc",
            b"$",
            b"{",
            b"${",
            b"\"",
            b"\\",
            b"\n\t\r",
            b"\x00\x1f\x7f",
            b" ",
            b"\xff",
            b"\xe2\x82",
            "€가😀".as_bytes(),
            "\u{200b}\u{0301}".as_bytes(),
            b"~}",
            "\u{a0}".as_bytes(),
            b"$$",
        ];
        for x in pieces {
            for y in pieces {
                for z in pieces {
                    let b = [x, y, z].concat();
                    assert_eq!(quote_bytes(&b), quote_slowly(&b), "{b:?}");
                    let mut out = String::from("[");
                    write_quoted(&b, &mut out);
                    assert_eq!(out, format!("[{}", quote_slowly(&b)));
                }
            }
        }
    }

    #[test]
    fn floats_print() {
        assert_eq!(format_float(15000000000.0), "15000000000.0");
        assert_eq!(format_float(0.1), "0.1");
        assert_eq!(format_float(1e21), "1e21");
        // the exponent as written in a literal: no padding, no `+`
        assert_eq!(format_float(1e-7), "1e-7");
        assert_eq!(format_float(0.00001), "1e-5");
        assert_eq!(format_float(1.5e-300), "1.5e-300");
        assert_eq!(format_float(2e100), "2e100");
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
            (Const::Float(1e-7), "1e-7"),
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
        assert_eq!(format_float(0.00001), "1e-5");
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
    fn lists_format_and_compare_by_items() {
        let l = Const::List(vec![Const::Int(1), Const::Str("a".into())]);
        assert_eq!(format(&l), "[1, \"a\"]");
        assert_eq!(format(&Const::List(vec![])), "[]");
        assert_eq!(l, l.clone());
        assert_ne!(Const::List(vec![]), l);
    }

    #[test]
    fn type_text_joins() {
        assert_eq!(type_text(&["int".into(), "nil".into()]), "int | nil");
    }
}
