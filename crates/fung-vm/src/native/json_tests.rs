//! Go `internal/native/json_test.go`: the `_native.json` natives called
//! directly, plus deep nesting on a small stack. Expected texts come from
//! the Go 1.27 implementation (`encoding/json` over `jsontext`).

use std::io::Cursor;
use std::rc::Rc;

use fung_front::source::SourceMap;

use super::module;
use crate::testutil::Buf;
use crate::value::{ListBuilder, Loc, MapBuilder, Time, Value, format, intern};
use crate::vm::{Runtime, Stop};

fn call(name: &str, args: Vec<Value>) -> Result<Value, Stop> {
    let m = module();
    let Some(Value::Native(n)) = m.get(name) else {
        panic!("no _native.json.{name}");
    };
    let rt = Runtime::new(
        Box::new(Buf::default()),
        Box::new(Buf::default()),
        Box::new(Cursor::new(Vec::new())),
        SourceMap::default(),
    );
    let r = (n.func)(&mut rt.task(), args);
    drop(rt);
    r
}

/// The result in fung form, or the error code.
fn shown(r: Result<Value, Stop>) -> String {
    match r {
        Ok(v) => format(&v),
        Err(Stop::Error(d)) => d.code.clone(),
        Err(Stop::Exit(n)) => format!("exit {n}"),
    }
}

fn parse(s: &str) -> String {
    shown(call("parse", vec![Value::str(s)]))
}

fn stringify(v: Value, indent: Value) -> String {
    match call("stringify", vec![v, indent]) {
        Ok(Value::Str(s)) => s.to_string(),
        r => shown(r),
    }
}

fn list(items: Vec<Value>) -> Value {
    let mut b = ListBuilder::with_capacity(items.len());
    for v in items {
        b.push(v);
    }
    Value::List(Rc::new(b.finish()))
}

fn map(entries: Vec<(&str, Value)>) -> Value {
    let mut m = MapBuilder::default();
    for (k, v) in entries {
        m.set(Rc::new(k.to_string()), v);
    }
    Value::Map(Rc::new(m.finish()))
}

/// Go `TestJSONParse`, good inputs.
#[test]
fn parse_keeps_order_and_numbers() {
    for (input, want) in [
        (
            r#"{"b": 1, "a": [true, null, "x"], "n": 1.5, "e": 1e2, "one": 1.0, "big": 12345678901}"#,
            r#"Ok({b: 1, a: [true, nil, "x"], n: 1.5, e: 100.0, one: 1.0, big: 12345678901})"#,
        ),
        ("\"가\\n\"", "Ok(\"가\\n\")"),
        ("[]", "Ok([])"),
        ("{}", "Ok({})"),
        ("-0.0", "Ok(-0.0)"),
        ("-0", "Ok(0)"),
        (r#"{"a": {"b": {"c": []}}}"#, "Ok({a: {b: {c: []}}})"),
        ("9223372036854775807", "Ok(9223372036854775807)"),
        ("-9223372036854775808", "Ok(-9223372036854775808)"),
        ("1.5e-7", "Ok(1.5e-07)"),
        ("1e-400", "Ok(0.0)"),
        (r#"{"a":1,"a":2}"#, "Ok({a: 2})"),
        (r#""\ud800""#, "Ok(\"\u{fffd}\")"),
        (r#""\ud83d\ude00\u00e9\/""#, "Ok(\"😀é/\")"),
        (" \t\r\n null \n", "Ok(nil)"),
    ] {
        assert_eq!(parse(input), want, "parse({input:?})");
    }
}

/// Go `TestJSONParse` and `TestJSONErrorColumns`, bad inputs: the whole
/// message, as Go 1.27 words it.
#[test]
fn parse_errors_name_the_place() {
    for (input, line, col, msg) in [
        (
            "{\"a\": 1,}",
            1,
            8,
            "invalid character ',' looking for beginning of value",
        ),
        ("[1, 2", 1, 6, "unexpected end of JSON input"),
        (
            "{\n  \"a\": tru\n}",
            2,
            11,
            "invalid character '\\n' in literal true (expecting 'e')",
        ),
        (
            "9223372036854775808",
            1,
            1,
            "integer 9223372036854775808 is out of range",
        ),
        (
            "-9223372036854775809",
            1,
            1,
            "integer -9223372036854775809 is out of range",
        ),
        ("1e400", 1, 1, "number 1e400 is out of range"),
        ("1 2", 1, 3, "unexpected text after the value"),
        ("", 1, 1, "unexpected end of input"),
        ("  ", 1, 3, "unexpected end of input"),
        // a byte order mark is not skipped; the column counts its bytes
        (
            "\u{feff}1",
            1,
            3,
            "invalid character '\\ufeff' looking for beginning of value",
        ),
        (
            r#"{"a": [1, {"b": }]}"#,
            1,
            17,
            "missing value after object key",
        ),
        (
            "{\"a\":\n  1\n  \"b\": 2}",
            3,
            3,
            "invalid character '\"' after object key:value pair",
        ),
        ("[1 2]", 1, 4, "invalid character '2' after array element"),
        ("[1,", 1, 4, "unexpected end of JSON input"),
        ("{\"a\"", 1, 5, "unexpected end of input"),
        ("{\"a\":", 1, 5, "unexpected end of input"),
        ("{\"a\":1,", 1, 8, "unexpected end of JSON input"),
        ("tru", 1, 1, "unexpected EOF"),
        (
            "trux",
            1,
            4,
            "invalid character 'x' in literal true (expecting 'e')",
        ),
        ("\"abc", 1, 1, "unexpected EOF"),
        ("{1:2}", 1, 2, "object member name must be a string"),
        (
            "]",
            1,
            1,
            "invalid character ']' looking for beginning of value",
        ),
        (
            "[}",
            1,
            2,
            "invalid character '}' looking for beginning of value",
        ),
        (
            "{]",
            1,
            2,
            "invalid character ']' looking for beginning of value",
        ),
        ("{\"a\" 1}", 1, 6, "invalid character '1' after object key"),
        (
            "1 ]",
            1,
            3,
            "invalid character ']' looking for beginning of value",
        ),
        ("\"\\x\"", 1, 3, "invalid escape sequence `\\x` in string"),
        ("\"a\tb\"", 1, 3, "invalid character '\\t' in string"),
        ("01", 1, 2, "unexpected text after the value"),
        ("-", 1, 1, "unexpected EOF"),
        ("1.", 1, 1, "unexpected EOF"),
        ("1e+", 1, 1, "unexpected EOF"),
        ("1.x", 1, 3, "invalid character 'x' in numeric literal"),
        (
            ".5",
            1,
            1,
            "invalid character '.' looking for beginning of value",
        ),
        (
            "[,1]",
            1,
            2,
            "invalid character ',' looking for beginning of value",
        ),
        (
            "[1,,2]",
            1,
            4,
            "invalid character ',' looking for beginning of value",
        ),
        (
            "{\"a\"::1}",
            1,
            6,
            "invalid character ':' looking for beginning of value",
        ),
        ("{\"a\",1}", 1, 5, "invalid character ',' after object key"),
        (
            "[1]x",
            1,
            4,
            "invalid character 'x' looking for beginning of value",
        ),
        (
            "1,",
            1,
            2,
            "invalid character ',' looking for beginning of value",
        ),
        (
            "[\n1\n,\n2\nx",
            5,
            1,
            "invalid character 'x' after array element",
        ),
        // Go counts the column over the bytes up to the character's end - 1.
        (
            "é",
            1,
            2,
            "invalid character 'é' looking for beginning of value",
        ),
        (
            "[1]]",
            1,
            4,
            "invalid character ']' looking for beginning of value",
        ),
        ("null1", 1, 5, "unexpected text after the value"),
        (
            "\"\\u12\"",
            1,
            6,
            "invalid escape sequence `\\u12\"` in string",
        ),
    ] {
        let want = format!(
            "Err({{kind: 'parse, line: {line}, col: {col}, message: {}}})",
            fung_front::consts::quote_str(&format!(
                "invalid JSON at line {line}, column {col}: {msg}"
            ))
        );
        assert_eq!(parse(input), want, "parse({input:?})");
    }
}

#[test]
fn parse_needs_a_string() {
    assert_eq!(shown(call("parse", vec![Value::Int(1)])), "E0403");
}

/// Go `TestJSONStringify`.
#[test]
fn stringify_follows_the_table() {
    let when = Value::Time(Rc::new(Time::from_unix(1_791_072_000, 0, Loc::Utc)));
    let m = map(vec![
        ("b", Value::Int(1)),
        (
            "a",
            list(vec![
                Value::Bool(true),
                Value::Nil,
                Value::str("x<y"),
                Value::Float(2.0),
            ]),
        ),
        ("t", Value::tuple(vec![Value::Int(1), Value::Int(2)])),
        ("s", Value::Sym(intern("ok"))),
        ("when", when),
    ]);
    assert_eq!(
        stringify(m, Value::Nil),
        r#"{"b":1,"a":[true,null,"x<y",2.0],"t":[1,2],"s":"ok","when":"2026-10-04T00:00:00Z"}"#
    );
    assert_eq!(
        stringify(list(vec![Value::Int(1), map(vec![])]), Value::Int(2)),
        "[\n  1,\n  {}\n]"
    );
    assert_eq!(stringify(Value::Dur(1), Value::Nil), "E0513");
    assert_eq!(stringify(Value::Int(1), Value::str("x")), "E0403");
    assert_eq!(stringify(Value::Int(1), Value::Int(-1)), "E0403");
}

#[test]
fn stringify_escapes_like_go() {
    let s = "\u{2028}<>&\u{7f}\u{1}\u{8}\u{c}\n\r\t\"\\\u{fffd}\u{2029}é\u{1f}";
    assert_eq!(
        stringify(Value::str(s), Value::Nil),
        "\"\\u2028<>&\u{7f}\\u0001\\b\\f\\n\\r\\t\\\"\\\\\u{fffd}\\u2029é\\u001f\""
    );
    assert_eq!(
        stringify(map(vec![("a\"b", Value::Int(1))]), Value::Nil),
        r#"{"a\"b":1}"#
    );
}

#[test]
fn stringify_floats_like_fung() {
    let fs = [
        1e21,
        1e20,
        1e-7,
        1e-4,
        0.1,
        -0.0,
        1.0,
        123456789.0,
        f64::NAN,
    ];
    let v = list(fs.iter().map(|f| Value::Float(*f)).collect());
    assert_eq!(
        stringify(v, Value::Nil),
        "[1e21,100000000000000000000.0,1e-07,0.0001,0.1,-0.0,1.0,123456789.0,NaN.0]"
    );
}

/// Go writes the indent with `strings.Repeat(" ", indent*depth)`; past
/// what Go can allocate that panics (E0801), only when a line is written.
#[test]
fn stringify_with_huge_indents_fails_like_go() {
    let one = || list(vec![Value::Int(1)]);
    assert_eq!(stringify(list(vec![]), Value::Int(1 << 62)), "[]");
    assert_eq!(stringify(Value::Int(1), Value::Int(i64::MAX)), "1");
    let e = call("stringify", vec![one(), Value::Int(1 << 62)]);
    match e {
        Err(Stop::Error(d)) => assert_eq!(
            fung_front::diag::message(&d.code, &d.args),
            "internal error: runtime error: makeslice: len out of range"
        ),
        _ => panic!("not an error"),
    }
    let nested = list(vec![one()]);
    match call("stringify", vec![nested, Value::Int(1 << 62)]) {
        Err(Stop::Error(d)) => assert_eq!(d.code, "E0801"),
        _ => panic!("not an error"),
    }
    // indent * depth wraps to a negative count in Go
    let deeper = list(vec![list(vec![one()])]);
    match call("stringify", vec![deeper, Value::Int(1 << 62)]) {
        Err(Stop::Error(d)) => assert_eq!(
            fung_front::diag::message(&d.code, &d.args),
            "internal error: runtime error: makeslice: len out of range"
        ),
        _ => panic!("not an error"),
    }
}

#[test]
fn stringify_rejects_what_json_cannot_hold() {
    for v in [
        Value::Dur(5),
        nat_tag(),
        Value::Range(Rc::new(crate::value::Range {
            start: 1,
            end: 2,
            inclusive: false,
        })),
        list(vec![Value::Int(1), Value::Dur(1)]),
        map(vec![("a", Value::Dur(1))]),
    ] {
        assert_eq!(stringify(v, Value::Nil), "E0513");
    }
}

fn nat_tag() -> Value {
    crate::native::nat::ok(Value::Int(1))
}

/// Go `TestJSONRoundTripKeepsOrder`.
#[test]
fn round_trip_keeps_order() {
    let input = r#"{"z":1,"y":{"b":2,"a":3},"x":[{"k":"v"}]}"#;
    let Ok(Value::Tag(t)) = call("parse", vec![Value::str(input)]) else {
        panic!("not a tag")
    };
    assert_eq!(stringify(t.args[0].clone(), Value::Nil), input);
}

/// The size of the threads the deep tests run on: far below what a
/// recursive walk of a million levels needs.
const SMALL_STACK: usize = 1 << 20;

fn on_small_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(SMALL_STACK)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

/// Go's decoder stops at 10,000 levels; a million-level input fails the
/// same way without the parser using the stack.
#[test]
fn deep_input_stops_at_go_depth_without_recursion() {
    on_small_stack(|| {
        let deep = "[".repeat(1_000_000);
        assert_eq!(
            parse(&deep),
            "Err({kind: 'parse, line: 1, col: 10001, message: \"invalid JSON at line 1, column 10001: exceeded max depth\"})"
        );
        let objects = "{\"a\":".repeat(1_000_000);
        assert!(parse(&objects).contains("column 50001: exceeded max depth"));
        let ok = format!("{}{}", "[".repeat(10_000), "]".repeat(10_000));
        let r = call("parse", vec![Value::str(&ok)]);
        assert!(matches!(&r, Ok(Value::Tag(t)) if t.name.name() == "Ok"));
        drop(r); // the nested lists are freed without recursion too
    });
}

/// `stringify` walks values of any depth without recursion.
#[test]
fn deep_values_stringify_without_recursion() {
    on_small_stack(|| {
        let n = 1_000_000;
        let mut v = list(vec![]);
        for i in 0..n {
            v = if i % 2 == 0 {
                list(vec![v])
            } else {
                map(vec![("k", v)])
            };
        }
        let out = stringify(v.clone(), Value::Nil);
        assert_eq!(out.len(), 2 + n / 2 * 2 + n / 2 * 6);
        assert!(out.starts_with("{\"k\":[{\"k\":[") && out.ends_with("]}]}"));
        let mut t = Value::Dur(1);
        for _ in 0..n {
            t = Value::tuple(vec![Value::Int(1), t]);
        }
        assert_eq!(stringify(t, Value::Nil), "E0513");
        let mut small = list(vec![]);
        for _ in 0..3 {
            small = list(vec![small, Value::Int(2)]);
        }
        assert_eq!(
            stringify(small, Value::Int(1)),
            "[\n [\n  [\n   [],\n   2\n  ],\n  2\n ],\n 2\n]"
        );
        drop(v);
    });
}

/// Go reads its input 64 bytes at a time at first, and an error shows the
/// character as far as it has read it: here only the first byte of `é`.
#[test]
fn errors_show_what_go_has_read() {
    let cut = format!("[{}é]", "1,".repeat(31));
    assert_eq!(
        parse(&cut),
        "Err({kind: 'parse, line: 1, col: 64, message: \"invalid JSON at line 1, column 64: invalid character '\\\\xc3' looking for beginning of value\"})"
    );
    let whole = format!("[{}1é]", "1,".repeat(31));
    assert!(parse(&whole).contains("column 66: invalid character 'é' after array element"));
}
