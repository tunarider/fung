//! Conversions. `int`, `float`, `str` and `sym`
//! assert ("this must be a number": a runtime error when it is not);
//! `parse_int`, `parse_float` and `parse_dur` check and return Ok/Err.

use fung_front::consts::quote_bytes;
use fung_front::diag::Arg;
use fung_front::lexer::{Kind, Token, lex};
use fung_front::source::FileId;
use fung_front::unicode::{is_name_continue, is_name_start, nfc};

use crate::native::nat::{self, def, fail, param};
use crate::value::{Args, Value, format, intern};
use crate::vm::{Stop, Task};

pub(super) fn add(add: &mut impl FnMut(Value)) {
    add(def("int", int_fn, vec![param("x")]));
    add(def("float", float_fn, vec![param("x")]));
    add(def("str", str_fn, vec![param("x")]));
    add(def("sym", sym_fn, vec![param("x")]));
    add(def("sym_name", sym_name_fn, vec![param("s")]));
    add(def("parse_int", parse_int_fn, vec![param("s")]));
    add(def("parse_float", parse_float_fn, vec![param("s")]));
    add(def("parse_dur", parse_dur_fn, vec![param("s")]));
}

fn cannot_convert(v: &Value, to: &str) -> Stop {
    fail("E0513", vec![Arg::Str(format(v)), nat::s(to)])
}

/// A number read from text by the literal rules (SP14, D12): at most
/// one leading sign `-` or `+`, then exactly one number literal (01) and
/// nothing else, so no spaces at either end, no leading zero (`"01"`),
/// `_` only between digits, lowercase prefixes and exponents, and a
/// literal that is not zero never reads as zero (D9). The parts are
/// the lexer's own: a number in a file and one in the text are read
/// alike. `Err` holds the lexer's help, if it had one.
fn literal(s: &str, want: Kind) -> Result<Token, Option<String>> {
    let (neg, body) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    // Every literal is ASCII and starts with a digit; anything else
    // (a second sign, a space, `.5`, a name) is no number.
    if !body.is_ascii() || !body.starts_with(|c: char| c.is_ascii_digit()) {
        return Err(None);
    }
    // The lexer's help is kept only when it is about the number itself
    // (its error starts the text): `"5 'x'"` is no number, and the
    // quote's help would mislead.
    let (toks, _) = lex(FileId(0), body).map_err(|d| {
        if d.pos.line == 1 && d.pos.col == 1 {
            d.help
        } else {
            None
        }
    })?;
    let Some(mut t) = toks
        .into_iter()
        .next()
        .filter(|t| t.kind == want && t.off == 0 && t.end == body.len())
    else {
        return Err(None);
    };
    match want {
        // The lexer reads the magnitude of the least int as `i64::MIN`
        // (decimal or with a prefix): only a minus makes it an int.
        Kind::Int if t.int == i64::MIN => {
            if !neg {
                return Err(None);
            }
        }
        Kind::Int if neg => t.int = -t.int,
        Kind::Float if neg => t.float = -t.float,
        Kind::Duration if neg => t.dur = t.dur.wrapping_neg(),
        _ => {}
    }
    Ok(t)
}

/// An int in the literal syntax (`literal`).
fn parse_int(s: &str) -> Result<i64, Option<String>> {
    literal(s, Kind::Int).map(|t| t.int)
}

/// A float in the literal syntax (`literal`); a decimal int literal
/// (`"42"`) is a float too, as `float(42)` is. A based int (`"0x10"`)
/// is not: no float is written in hex.
fn parse_float(s: &str) -> Result<f64, Option<String>> {
    match literal(s, Kind::Float) {
        Ok(t) => Ok(t.float),
        Err(None) => match literal(s, Kind::Int) {
            Ok(t) if t.text.bytes().all(|b| b.is_ascii_digit() || b == b'_') => Ok(t.int as f64),
            Ok(_) => Err(None),
            Err(help) => Err(help),
        },
        Err(help) => Err(help),
    }
}

/// A duration in the literal syntax (`literal`): 1h30m, 500ms, -5s.
fn parse_dur(s: &str) -> Result<i64, Option<String>> {
    literal(s, Kind::Duration).map(|t| t.dur)
}

/// The value `x` in a conversion message: a string in quotes, so that
/// spaces show (`" 1"`), any other value as printed.
fn shown(x: &Value) -> String {
    match x {
        Value::Str(s) => quote_bytes(s),
        v => format(v),
    }
}

/// E0513, with the lexer's help when it had one (`int("007")` is told
/// about `0o`).
fn cannot_convert_text(v: &Value, to: &str, help: Option<String>) -> Stop {
    let mut stop = fail("E0513", vec![Arg::Str(shown(v)), nat::s(to)]);
    if let Stop::Error(d) = &mut stop {
        d.help = help;
    }
    stop
}

/// The `parse_*` Err: the input in quotes, then the lexer's help.
fn parse_failed(what: &str, s: &Value, help: Option<String>) -> Value {
    let mut m = format!("not {what}: {}", shown(s));
    if let Some(h) = help {
        m = format!("{m} ({h})");
    }
    nat::err_value("parse", &m, Vec::new())
}

fn int_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    match &a[0] {
        Value::Int(n) => return Ok(Value::Int(*n)),
        Value::Float(f) => {
            // 2^63 itself is out of range; -2^63 is in.
            if f.is_finite()
                && *f < 9_223_372_036_854_775_808.0
                && *f >= -9_223_372_036_854_775_808.0
            {
                return Ok(Value::Int(*f as i64));
            }
        }
        Value::Str(s) => {
            return match s.as_str().map(parse_int) {
                Some(Ok(n)) => Ok(Value::Int(n)),
                Some(Err(help)) => Err(cannot_convert_text(&a[0], "int", help)),
                None => Err(cannot_convert_text(&a[0], "int", None)),
            };
        }
        _ => {}
    }
    Err(cannot_convert(&a[0], "int"))
}

fn float_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    match &a[0] {
        Value::Int(n) => return Ok(Value::Float(*n as f64)),
        Value::Float(f) => return Ok(Value::Float(*f)),
        Value::Str(s) => {
            return match s.as_str().map(parse_float) {
                Some(Ok(f)) => Ok(Value::Float(f)),
                Some(Err(help)) => Err(cannot_convert_text(&a[0], "float", help)),
                None => Err(cannot_convert_text(&a[0], "float", None)),
            };
        }
        _ => {}
    }
    Err(cannot_convert(&a[0], "float"))
}

fn str_fn(_: &mut Task<'_>, mut a: Args) -> Result<Value, Stop> {
    match a.swap_remove(0) {
        s @ Value::Str(_) => Ok(s),
        v => Ok(Value::str(&format(&v))),
    }
}

/// What a symbol literal takes after `'` (01, SP4): `_` or a name-start
/// character, then name-continue characters (UAX #31). A tag-shaped name
/// is one too, so `sym("Red")` is the tag `Red` an enum type names.
fn is_name(s: &str) -> bool {
    let mut cs = s.chars();
    let Some(first) = cs.next() else {
        return false;
    };
    (first == '_' || is_name_start(first)) && cs.all(is_name_continue)
}

fn sym_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    match &a[0] {
        Value::Str(s) if s.as_str().is_some_and(|s| is_name(&nfc(s))) => {
            Ok(Value::Sym(intern(&nfc(s.as_str().expect("checked")))))
        }
        Value::Sym(x) => Ok(Value::Sym(*x)),
        v => Err(cannot_convert(v, "sym")),
    }
}

/// `sym_name(s)`: a symbol's name as text, the other way from `sym`
/// (SP26, D15): `sym_name('ok)` is `"ok"`, where `str('ok)` is `"'ok"`.
fn sym_name_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    match &a[0] {
        Value::Sym(x) => Ok(Value::str(x.name())),
        v => Err(nat::wrong_arg("s", "sym", v)),
    }
}

/// The `parse_*` functions read text from outside: bytes that are not
/// UTF-8 are no number either, an `Err` like any other (L4). The text
/// follows the literal rules (`literal`); spaces are trimmed first by
/// the caller (`trim`).
fn parse_int_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    nat::need_text("s", &a[0])?;
    Ok(match text_of(&a[0]).map(parse_int) {
        Some(Ok(n)) => nat::ok(Value::Int(n)),
        Some(Err(help)) => parse_failed("an integer", &a[0], help),
        None => parse_failed("an integer", &a[0], None),
    })
}

fn parse_float_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    nat::need_text("s", &a[0])?;
    Ok(match text_of(&a[0]).map(parse_float) {
        Some(Ok(f)) => nat::ok(Value::Float(f)),
        Some(Err(help)) => parse_failed("a number", &a[0], help),
        None => parse_failed("a number", &a[0], None),
    })
}

/// `parse_dur(s)` reads a duration in the literal syntax (01: 1h30m,
/// 500ms, -5s), the way `parse_int` reads an int: Ok/Err for untrusted
/// text.
fn parse_dur_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    nat::need_text("s", &a[0])?;
    Ok(match text_of(&a[0]).map(parse_dur) {
        Some(Ok(d)) => nat::ok(Value::Dur(d)),
        Some(Err(help)) => parse_failed("a duration", &a[0], help),
        None => parse_failed("a duration", &a[0], None),
    })
}

/// The text of a string value, `None` when it is not valid UTF-8.
fn text_of(v: &Value) -> Option<&str> {
    match v {
        Value::Str(s) => s.as_str(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_dur, parse_float, parse_int};

    /// The table every reader of numbers from text follows (SP14, D12):
    /// the literal rules (01), one leading sign, nothing around it.
    #[test]
    fn int_literal_syntax() {
        for (s, n) in [
            ("42", 42),
            ("-42", -42),
            ("+42", 42),
            ("0", 0),
            ("-0", 0),
            ("1_000", 1000),
            ("0x1f", 31),
            ("-0o17", -15),
            ("0b101", 5),
            ("9223372036854775807", i64::MAX),
            ("-9223372036854775808", i64::MIN),
            ("-0x8000000000000000", i64::MIN),
        ] {
            assert_eq!(parse_int(s), Ok(n), "{s:?}");
        }
        for s in [
            "",
            "-",
            "+",
            " 1",
            "1 ",
            "1\n",
            "\u{a0}1",
            "01",
            "007",
            "+-1",
            "--1",
            "- 1",
            "1__0",
            "_1",
            "1_",
            "0X1F",
            "1.0",
            "1e3",
            "1s",
            "9223372036854775808",
            "0x8000000000000000",
            "١",
            "1 # c",
        ] {
            assert!(parse_int(s).is_err(), "{s:?}");
        }
        let help = parse_int("01").unwrap_err().unwrap();
        assert!(help.contains("0o"), "{help}");
        // Only a help about the number itself: a lex error further on
        // (a quote, an uppercase prefix) is no help for this text.
        for s in ["5 'x'", "1 0X1", "1 01"] {
            assert_eq!(parse_int(s), Err(None), "{s:?}");
        }
        assert_eq!(parse_dur("5s 01"), Err(None));
    }

    #[test]
    fn float_literal_syntax() {
        for (s, f) in [
            ("3.25", 3.25),
            ("-2.5e-3", -2.5e-3),
            ("+1e9", 1e9),
            ("42", 42.0),
            ("1_000.5", 1000.5),
            ("0.5", 0.5),
            ("1e+2", 100.0),
        ] {
            assert_eq!(parse_float(s), Ok(f), "{s:?}");
        }
        for s in [
            "0x1p4", "0x10", "0b1", ".5", "1.", "1e", "1__0", "_1", "1_", "inf", "nan", "+-1",
            " 1", "1 ", "1e400", "", "-", "1._5", "1e_5", "١", "1E+2", "01.5", "1e-400",
        ] {
            assert!(parse_float(s).is_err(), "{s:?}");
        }
        assert_eq!(parse_float("0e-999"), Ok(0.0));
    }

    /// One literal and nothing else: no comment after it, no space
    /// between the sign or at the ends (VA6, D12).
    #[test]
    fn duration_literal_syntax() {
        assert_eq!(parse_dur("5s"), Ok(5_000_000_000));
        assert_eq!(parse_dur("-5s"), Ok(-5_000_000_000));
        assert_eq!(parse_dur("+5s"), Ok(5_000_000_000));
        assert_eq!(parse_dur("1h30m"), Ok(5_400_000_000_000));
        for s in [
            "5s # hi",
            "- 5s",
            "5s\n# c",
            "5s\n",
            "--5s",
            "5s;",
            "#5s",
            " 5s",
            "5s ",
            "\t5s",
            "\u{a0}5s",
            "5s 1m",
            "01s",
            "1h05m",
            "0.0000004ms",
            "5",
            "",
        ] {
            assert!(parse_dur(s).is_err(), "{s:?}");
        }
    }
}
