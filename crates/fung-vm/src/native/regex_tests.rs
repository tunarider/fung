//! Go `internal/native/regex_test.go`: the `_native.regex` natives called
//! directly, plus Go `regexp`'s empty-match, `Split` and `Expand` rules.
//! Expected texts come from the Go 1.27 implementation.

use std::io::Cursor;

use fung_front::source::SourceMap;

use super::*;
use crate::testutil::Buf;
use crate::value::{Value, format};
use crate::vm::{Runtime, Stop};

fn call(name: &str, args: &[&str]) -> Result<Value, Stop> {
    call_values(name, args.iter().map(|s| Value::str(s)).collect())
}

fn call_values(name: &str, args: Vec<Value>) -> Result<Value, Stop> {
    let m = module();
    let Some(Value::Native(n)) = m.get(name) else {
        panic!("no _native.regex.{name}");
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

fn ok(name: &str, args: &[&str]) -> String {
    let r = shown(call(name, args));
    assert!(!r.starts_with('E'), "{name}{args:?}: {r}");
    r
}

#[test]
fn find_uses_rune_indexes() {
    assert_eq!(
        ok("find", &["가나다 나다", "나(다)"]),
        r#"{text: "나다", start: 1, end: 3, groups: ["다"], named: {}}"#
    );
    assert_eq!(ok("find", &["abc", "z"]), "nil");
    assert_eq!(
        ok("find", &["k=v", r"(?P<key>\w+)=(?P<val>\w+)(x)?"]),
        r#"{text: "k=v", start: 0, end: 3, groups: ["k", "v", nil], named: {key: "k", val: "v"}}"#
    );
    // \w is ASCII (Go): the Hangul and é are not word characters.
    assert_eq!(
        ok("find", &["가a1 é٣_", r"\w+"]),
        r#"{text: "a1", start: 1, end: 3, groups: [], named: {}}"#
    );
    // a later group of the same name wins, also when it did not match
    assert_eq!(
        ok("find", &["aa", "(?P<x>a)(?P<x>b)?"]),
        r#"{text: "a", start: 0, end: 1, groups: ["a", nil], named: {x: nil}}"#
    );
    assert_eq!(
        ok("find", &["ab", "(?P<x>a)(?P<x>b)?"]),
        r#"{text: "ab", start: 0, end: 2, groups: ["a", "b"], named: {x: "b"}}"#
    );
}

#[test]
fn the_other_functions() {
    assert_eq!(ok("is_match", &["file.log", r"\.log$"]), "true");
    assert_eq!(ok("is_match", &["file.txt", r"\.log$"]), "false");
    assert_eq!(
        ok("find_all", &["a1 b22 c333", r"\d+"]),
        r#"[{text: "1", start: 1, end: 2, groups: [], named: {}}, {text: "22", start: 4, end: 6, groups: [], named: {}}, {text: "333", start: 8, end: 11, groups: [], named: {}}]"#
    );
    assert_eq!(ok("find_all", &["none", r"\d"]), "[]");
    assert_eq!(
        ok("replace", &["2026-10-04", r"(\d+)-(\d+)-(\d+)", "$3/$2/$1"]),
        r#""04/10/2026""#
    );
    assert_eq!(
        ok("split", &["a, b,c ,", r"\s*,\s*"]),
        r#"["a", "b", "c", ""]"#
    );
}

#[test]
fn bad_patterns_and_arguments() {
    assert_eq!(shown(call("is_match", &["x", "("])), "E0515");
    let Err(Stop::Error(d)) = call("find", &["x", "a{1001}"]) else {
        panic!("a{{1001}} must fail");
    };
    assert_eq!(
        d.args,
        vec![Arg::Str(
            "error parsing regexp: invalid repeat count: `{1001}`".into()
        )]
    );
    assert_eq!(
        shown(call_values(
            "is_match",
            vec![Value::Int(1), Value::str("x")]
        )),
        "E0403"
    );
    assert_eq!(
        shown(call_values("find", vec![Value::str("x"), Value::Nil])),
        "E0403"
    );
    // the pattern is compiled before the replacement is checked
    assert_eq!(
        shown(call_values(
            "replace",
            vec![Value::str("x"), Value::str("("), Value::Int(1)]
        )),
        "E0515"
    );
    assert_eq!(
        shown(call_values(
            "replace",
            vec![Value::str("x"), Value::str("x"), Value::Int(1)]
        )),
        "E0403"
    );
}

/// Go `allMatches`, `Split` and `replaceAll` around empty matches.
#[test]
fn empty_matches_follow_go() {
    let starts_ends = |s: &str, pat: &str| {
        let re = compile_go(pat).unwrap();
        let mut out = Vec::new();
        go_all(&re, s, |l| out.push(l.get(0).unwrap()));
        out
    };
    assert_eq!(starts_ends("baaa", "a*"), vec![(0, 0), (1, 4)]);
    // byte offsets: é takes 2 bytes, 가 3
    assert_eq!(starts_ends("é가 a", r"\b"), vec![(6, 6), (7, 7)]);
    assert_eq!(starts_ends("", "x*"), vec![(0, 0)]);
    assert_eq!(ok("split", &["baaa", "a*"]), r#"["b", ""]"#);
    assert_eq!(ok("split", &["abc", ""]), r#"["a", "b", "c"]"#);
    assert_eq!(ok("split", &["", ""]), "[]");
    assert_eq!(ok("split", &["", "x"]), r#"[""]"#);
    assert_eq!(ok("split", &["ab", "$"]), r#"["ab"]"#);
    assert_eq!(ok("split", &["가나 다", r"\b"]), r#"["가나 다"]"#);
    assert_eq!(ok("replace", &["baaa", "a*", "<$0>"]), r#""<>b<aaa>""#);
    assert_eq!(ok("replace", &["가나", "x*", "-"]), r#""-가-나-""#);
    assert_eq!(ok("replace", &["abc", r"\b", "|"]), r#""|abc|""#);
}

/// Go `Regexp.Expand`: `$$`, numbers, names, braces, `$1a` as the name
/// "1a", a malformed reference kept as text, leading zeros and huge
/// numbers as names.
#[test]
fn replacement_templates_follow_go() {
    assert_eq!(
        ok(
            "replace",
            &[
                "k=v",
                r"(?P<key>\w+)=(?P<val>\w+)",
                "$$|$1|${1}|$key|${val}|$9|$1a|${1}a|$|${1|$0|$01|$_|$한|$10000000000",
            ]
        ),
        r#""$|k|k|k|v|||ka|$|\${1|k=v||||""#
    );
}

#[test]
fn the_cache_is_bounded() {
    let a = compile_pattern(r"\d+").unwrap();
    let b = compile_pattern(r"\d+").unwrap();
    assert!(
        Arc::ptr_eq(&a, &b),
        "the same pattern must come from the cache"
    );
    for i in 0..MAX_CACHED_PATTERNS + 5 {
        let _ = compile_pattern(&format!("x{{{}}}{}", i % 10, i));
    }
    let n = cache().len();
    assert!(n <= MAX_CACHED_PATTERNS, "cache grew to {n}");
}

/// Rune positions must not be recounted from the start of the subject
/// for every match (a 300KB log would take seconds). Release builds only:
/// a debug build is too slow to say anything.
#[cfg(not(debug_assertions))]
#[test]
fn find_all_is_linear() {
    let s = "가 1 ".repeat(50_000);
    let start = std::time::Instant::now();
    let r = call("find_all", &[&s, r"\d"]).unwrap_or_else(|_| panic!("find_all failed"));
    let took = start.elapsed();
    assert!(
        took < std::time::Duration::from_secs(1),
        "find_all over 300KB took {took:?}"
    );
    let Value::List(items) = r else {
        panic!("not a list")
    };
    assert_eq!(items.len(), 50_000);
    let Some(Value::Map(last)) = items.at(49_999) else {
        panic!("no last match")
    };
    assert_eq!(
        last.get("end").map(format),
        Some((50_000 * 4 - 1).to_string())
    );
}
