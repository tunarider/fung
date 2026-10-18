//! `_native.json` tests: the natives called directly, plus deep nesting
//! on a small stack. Expected texts are the whole messages.

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

/// `parse`, good inputs.
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
        ("1.5e-7", "Ok(1.5e-7)"),
        ("1e-400", "Ok(0.0)"),
        (r#"{"a":1,"b":{"a":2}}"#, "Ok({a: 1, b: {a: 2}})"),
        (r#""\uFFFD\u0041""#, "Ok(\"\u{fffd}A\")"),
        (r#""\ud83d\ude00\u00e9\/""#, "Ok(\"😀é/\")"),
        (" \t\r\n null \n", "Ok(nil)"),
    ] {
        assert_eq!(parse(input), want, "parse({input:?})");
    }
}

/// `parse`, bad inputs: the whole message, with the column of the error.
#[test]
fn parse_errors_name_the_place() {
    let ch = |c: &str| format!("unexpected character '{c}'");
    let end = "unexpected end of input".to_string();
    let after = "unexpected text after the value".to_string();
    let key = "expected a string for an object key".to_string();
    let num = "invalid number".to_string();
    let range = "number out of range".to_string();
    for (input, line, col, msg) in [
        ("{\"a\": 1,}", 1, 9, key.clone()),
        ("[1, 2", 1, 6, end.clone()),
        ("[1, 2,]", 1, 7, ch("]")),
        ("{\"a\":1} x", 1, 9, after.clone()),
        ("{\n  \"a\": tru\n}", 2, 11, ch("\\n")),
        ("9223372036854775808", 1, 1, range.clone()),
        ("-9223372036854775809", 1, 1, range.clone()),
        ("1e400", 1, 1, range.clone()),
        ("1 2", 1, 3, after.clone()),
        ("", 1, 1, end.clone()),
        ("  ", 1, 3, end.clone()),
        // A byte order mark is not whitespace; it is one character.
        ("\u{feff}1", 1, 1, ch("\\u{FEFF}")),
        (r#"{"a": [1, {"b": }]}"#, 1, 17, ch("}")),
        (
            "{\"a\":\n  1\n  \"b\": 2}",
            3,
            3,
            "expected `,` or `}`".into(),
        ),
        ("[1 2]", 1, 4, "expected `,` or `]`".into()),
        ("[1,", 1, 4, end.clone()),
        ("[1", 1, 3, end.clone()),
        ("{\"a\"", 1, 5, end.clone()),
        ("{\"a\":", 1, 6, end.clone()),
        ("{\"a\":1,", 1, 8, end.clone()),
        ("tru", 1, 4, end.clone()),
        ("trux", 1, 4, ch("x")),
        ("\"abc", 1, 5, end.clone()),
        ("{1:2}", 1, 2, key.clone()),
        ("]", 1, 1, ch("]")),
        ("[}", 1, 2, ch("}")),
        ("{]", 1, 2, key.clone()),
        ("{\"a\" 1}", 1, 6, "expected `:` after an object key".into()),
        ("{\"a\",1}", 1, 5, "expected `:` after an object key".into()),
        ("1 ]", 1, 3, after.clone()),
        ("\"\\x\"", 1, 2, "invalid escape".into()),
        ("\"\\u12\"", 1, 2, "invalid \\u escape".into()),
        ("\"\\ud800\"", 1, 2, "unpaired surrogate".into()),
        ("\"\\udc00\"", 1, 2, "unpaired surrogate".into()),
        ("\"a\tb\"", 1, 3, "control character in a string".into()),
        ("01", 1, 2, num.clone()),
        ("-", 1, 2, end.clone()),
        ("1.", 1, 3, end.clone()),
        ("1e+", 1, 4, end.clone()),
        ("1.x", 1, 3, ch("x")),
        (".5", 1, 1, ch(".")),
        ("[,1]", 1, 2, ch(",")),
        ("[1,,2]", 1, 4, ch(",")),
        ("{\"a\"::1}", 1, 6, ch(":")),
        ("[1]x", 1, 4, after.clone()),
        ("1,", 1, 2, after.clone()),
        ("[\n1\n,\n2\nx", 5, 1, "expected `,` or `]`".into()),
        ("é", 1, 1, ch("é")),
        ("[1]]", 1, 4, after.clone()),
        ("null1", 1, 5, after.clone()),
        ("{\"a\":1,\"a\":2}", 1, 8, "duplicate key \"a\"".into()),
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

/// `stringify` of each kind of value.
#[test]
fn stringify_follows_the_table() {
    let when = Value::Time(Rc::new(
        Time::from_unix(1_791_072_000, 0, Loc::Utc).unwrap(),
    ));
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
fn stringify_escapes_what_json_needs() {
    // Only `"`, `\` and characters below U+0020; U+2028 and U+2029 stay.
    let s = "\u{2028}<>&\u{7f}\u{1}\u{8}\u{c}\n\r\t\"\\\u{fffd}\u{2029}é\u{1f}";
    assert_eq!(
        stringify(Value::str(s), Value::Nil),
        "\"\u{2028}<>&\u{7f}\\u0001\\b\\f\\n\\r\\t\\\"\\\\\u{fffd}\u{2029}é\\u001f\""
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
        "[1e21,100000000000000000000.0,1e-7,0.0001,0.1,-0.0,1.0,123456789.0,NaN.0]"
    );
}

/// A line break writes `indent * depth` spaces; output past the string
/// limit is an error, raised only when a line is written.
#[test]
fn stringify_output_has_the_string_limit() {
    let one = || list(vec![Value::Int(1)]);
    // Nothing is indented: no line breaks, however large the indent.
    assert_eq!(stringify(list(vec![]), Value::Int(1 << 62)), "[]");
    assert_eq!(stringify(Value::Int(1), Value::Int(i64::MAX)), "1");
    for indent in [1 << 62, 100_000_000] {
        match call("stringify", vec![list(vec![one()]), Value::Int(indent)]) {
            Err(Stop::Error(d)) => assert_eq!(
                fung_front::diag::message(&d.code, &d.args),
                "too many characters (limit 100000000)"
            ),
            _ => panic!("not an error"),
        }
    }
    assert_eq!(stringify(one(), Value::Int(2)), "[\n  1\n]");
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

/// `parse` then `stringify` keeps the order of the keys.
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

/// The parser stops at 10,000 levels; a million-level input fails the
/// same way without the parser using the stack.
#[test]
fn deep_input_stops_at_the_depth_limit_without_recursion() {
    on_small_stack(|| {
        let deep = "[".repeat(1_000_000);
        assert_eq!(
            parse(&deep),
            "Err({kind: 'parse, line: 1, col: 10001, message: \"invalid JSON at line 1, column 10001: nesting too deep (limit 10000)\"})"
        );
        let objects = "{\"a\":".repeat(1_000_000);
        assert!(parse(&objects).contains("column 50001: nesting too deep (limit 10000)"));
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

/// The parser sees the whole input: an error names the whole character
/// where it is, however far into the input.
#[test]
fn errors_name_whole_characters() {
    let cut = format!("[{}é]", "1,".repeat(31));
    assert!(parse(&cut).contains("column 64: unexpected character 'é'"));
    let whole = format!("[{}1é]", "1,".repeat(31));
    assert!(parse(&whole).contains("column 65: expected `,` or `]`"));
}

/// An agent is rejected like a channel or a task: E0513 with its type
/// name, wherever it sits.
#[test]
fn stringify_rejects_agents() {
    let o = crate::testutil::exec(
        "import json\na = agent(1)\nprint(json.stringify([1, {x: a}]))",
        crate::testutil::Opts::default(),
    );
    assert_eq!((o.out.as_str(), o.code.as_str()), ("", "E0513"));
    assert!(
        o.err
            .starts_with("error[E0513]: cannot convert agent to JSON\n"),
        "{}",
        o.err
    );
    let o = crate::testutil::exec(
        "import json\nprint(json.stringify(agent(1)))",
        crate::testutil::Opts::default(),
    );
    assert!(
        o.err
            .starts_with("error[E0513]: cannot convert agent to JSON\n"),
        "{}",
        o.err
    );
    let caught = Value::Caught(Rc::new(crate::vm::Caught::Exit(1)));
    for v in [caught.clone(), list(vec![map(vec![("e", caught)])])] {
        match call("stringify", vec![v, Value::Nil]) {
            Err(Stop::Error(d)) => assert_eq!(
                fung_front::diag::message(&d.code, &d.args),
                "cannot convert error to JSON"
            ),
            _ => panic!("not an error"),
        }
    }
}

/// The string limit counts the whole output as it grows: a line break
/// that would take it past the limit fails before it is written.
#[test]
fn indentation_counts_toward_the_limit_as_it_grows() {
    let mut out = "x".repeat(99_999_995);
    assert!(super::newline(&mut out, 10, 1).is_err());
    assert_eq!(out.len(), 99_999_995);
    let mut small = String::new();
    assert!(super::newline(&mut small, 10, 1).is_ok());
}
