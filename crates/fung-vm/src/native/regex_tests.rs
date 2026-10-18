//! The `_native.regex` natives called directly: matches with character
//! positions, empty matches, `split`, `$` references, errors and the
//! pattern cache.

use std::io::Cursor;

use fung_front::source::SourceMap;

use super::*;
use std::sync::Condvar;
use std::time::Duration;

use crate::testutil::{Buf, Opts, UNTIL, exec, probes};
use crate::value::{Native, Value, format};
use crate::vm::{Runtime, Stop, Task};

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
    // \w is Unicode: Hangul is a word character.
    assert_eq!(
        ok("find", &["가a1 é٣_", r"\w+"]),
        r#"{text: "가a1", start: 0, end: 3, groups: [], named: {}}"#
    );
    // a group that did not take part is nil, by number and by name
    assert_eq!(
        ok("find", &["a", "(?P<x>a)(?P<y>b)?"]),
        r#"{text: "a", start: 0, end: 1, groups: ["a", nil], named: {x: "a", y: nil}}"#
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
    let Err(Stop::Error(d)) = call("find", &["x", "a{2,1}"]) else {
        panic!("a{{2,1}} must fail");
    };
    assert_eq!(
        d.args,
        vec![Arg::Str(
            "`a{2,1}`: invalid repetition count range, the start must be <= the end".into()
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

/// Empty matches (06: 정규식): `find_all` and `replace` skip an empty
/// match right after the previous match; `split` does not cut at an empty
/// match at the very start or end of the subject.
#[test]
fn empty_matches() {
    let spans = |s: &str, pat: &str| {
        let r = shown(call("find_all", &[s, pat]));
        r.matches("start: ").count()
    };
    assert_eq!(spans("baaa", "a*"), 2);
    assert_eq!(spans("abc", "x*"), 4);
    assert_eq!(ok("split", &["abc", ""]), r#"["a", "b", "c"]"#);
    assert_eq!(ok("split", &["", "x*"]), r#"[""]"#);
    assert_eq!(ok("split", &["", ""]), "[]");
    assert_eq!(ok("split", &["a,b,", ","]), r#"["a", "b", ""]"#);
    assert_eq!(ok("split", &["a,,b", ","]), r#"["a", "", "b"]"#);
    assert_eq!(ok("split", &["baaa", "a*"]), r#"["b", ""]"#);
    assert_eq!(ok("split", &["ab", "$"]), r#"["ab"]"#);
    assert_eq!(ok("split", &["가나 다", r"\b"]), r#"["가나", " ", "다"]"#);
    assert_eq!(ok("replace", &["baaa", "a*", "<$0>"]), r#""<>b<aaa>""#);
    assert_eq!(ok("replace", &["가나", "x*", "-"]), r#""-가-나-""#);
    assert_eq!(ok("replace", &["abc", r"\b", "|"]), r#""|abc|""#);
}

/// `$` in a replacement (06): `$$`, numbers, names, braces; `$1a` is the
/// name "1a" (write `${1}a`), a reference that is not one stays text, and
/// a group that is not there gives nothing.
#[test]
fn replacement_templates() {
    assert_eq!(
        ok(
            "replace",
            &[
                "k=v",
                r"(?P<key>\w+)=(?P<val>\w+)",
                "$$|$1|${1}|$key|${val}|$9|$1a|${1}a|$|${1|$0|$01|$_|$한",
            ]
        ),
        r#""$|k|k|k|v|||ka|$|\${1|k=v|k||$한""#
    );
}

#[test]
fn the_same_pattern_comes_from_the_cache() {
    let rt = Runtime::new(
        Box::new(Buf::default()),
        Box::new(Buf::default()),
        Box::new(Cursor::new(Vec::new())),
        SourceMap::default(),
    );
    let mut t = rt.task();
    let a = compile_pattern(&mut t, r"\d+").unwrap_or_else(|_| panic!("compile"));
    let b = compile_pattern(&mut t, r"\d+").unwrap_or_else(|_| panic!("compile"));
    drop(t);
    drop(rt);
    assert!(
        Arc::ptr_eq(&a, &b),
        "the same pattern must come from the cache"
    );
}

/// A full cache is emptied whole, instead of evicting one entry. Checked on a map of the test's own: the process-wide cache is
/// shared with every other test running at the same time.
#[test]
fn a_full_cache_is_emptied() {
    let mut m = HashMap::new();
    let one = || {
        Arc::new(Compiled {
            re: Regex::new("x").unwrap(),
            names: vec![String::new()],
        })
    };
    for i in 0..MAX_CACHED_PATTERNS {
        remember(&mut m, &format!("p{i}"), one());
    }
    assert_eq!(m.len(), MAX_CACHED_PATTERNS);
    remember(&mut m, "p0", one());
    assert_eq!(
        m.len(),
        MAX_CACHED_PATTERNS,
        "a known pattern replaces itself"
    );
    remember(&mut m, "one more", one());
    assert_eq!(
        m.len(),
        1,
        "the cache is cleared, then the new pattern added"
    );
    assert!(m.contains_key("one more"));
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

// --- compiling outside the run token ---

/// The pattern whose compilation `compile_hook` holds at a gate.
const GATED: &str = "gated-compile-probe";

/// `(entered, opened, timed_out)` of the gate, and its wake-up.
static GATE: Mutex<(bool, bool, bool)> = Mutex::new((false, false, false));
static GATE_CHANGED: Condvar = Condvar::new();

fn gate() -> MutexGuard<'static, (bool, bool, bool)> {
    GATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Called by `compile` for every pattern it compiles. The `GATED`
/// pattern waits there until a fung task opens the gate, standing in for
/// a pattern that takes seconds to compile. The wait gives up after 10 s
/// (and says so) so a compile that holds the run token, where no task can
/// open the gate, fails the test instead of hanging it.
pub(super) fn compile_hook(pat: &str) {
    if pat != GATED {
        return;
    }
    let mut g = gate();
    g.0 = true;
    GATE_CHANGED.notify_all();
    let (mut g, r) = GATE_CHANGED
        .wait_timeout_while(g, Duration::from_secs(10), |g| !g.1)
        .unwrap_or_else(|e| e.into_inner());
    g.2 = r.timed_out();
}

/// The probes plus `compiling()` (a compile waits at the gate) and
/// `open_gate()`.
fn gate_natives() -> Vec<Value> {
    let mut v = probes();
    v.push(Value::Native(Native::leak(
        "compiling",
        vec![],
        |_: &mut Task, _| Ok(Value::Bool(gate().0)),
    )));
    v.push(Value::Native(Native::leak(
        "open_gate",
        vec![],
        |_: &mut Task, _| {
            gate().1 = true;
            GATE_CHANGED.notify_all();
            Ok(Value::Nil)
        },
    )));
    v
}

/// The compile runs without the run token: while
/// one task is still compiling, another runs fung code (it sees the
/// compile in progress and opens the gate that ends it).
#[test]
fn other_tasks_run_while_a_pattern_compiles() {
    let o = exec(
        &format!(
            "{UNTIL}import regex\nh = spawn(() => regex.is_match(\"x\", `{GATED}`))\nuntil(compiling)\nopen_gate()\nprint(await(h))"
        ),
        Opts {
            extra: gate_natives,
            ..Opts::default()
        },
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("false\n", ""), "{o:?}");
    let g = gate();
    assert!(g.0 && g.1, "the gate was not used: {:?}", *g);
    assert!(
        !g.2,
        "the compile held the run token: no task could open the gate"
    );
}

// --- the regex crate's syntax (06: 정규식) ---

fn err_msg(name: &str, args: &[&str]) -> String {
    match call(name, args) {
        Err(Stop::Error(d)) => fung_front::diag::message(&d.code, &d.args),
        r => panic!("{name}{args:?}: {}", shown(r)),
    }
}

#[test]
fn unicode_classes() {
    assert_eq!(ok("is_match", &["٣", r"\d"]), "true");
    assert_eq!(ok("is_match", &["é", r"\w"]), "true");
    assert_eq!(ok("is_match", &["가", r"^\w$"]), "true");
    assert_eq!(ok("is_match", &["٣", r"(?-u:\d)"]), "false");
    assert!(ok("find", &["가a1 x", r"\w+"]).contains("start: 0, end: 3"));
}

#[test]
fn crate_syntax() {
    assert_eq!(ok("is_match", &["ab", "(?x) a b"]), "true");
    assert_eq!(ok("is_match", &["a", "^[a&&[a-c]]$"]), "true");
}

#[test]
fn pattern_errors() {
    assert_eq!(
        err_msg("is_match", &["a", "a("]),
        "invalid pattern `a(`: unclosed group"
    );
    assert!(
        err_msg("is_match", &["a", "(?P<n>a)(?P<n>b)"]).contains("duplicate capture group name")
    );
    assert!(err_msg("is_match", &["a", r"\pL{1000}{1000}"]).ends_with("pattern too large"));
}

/// Patterns of an ordinary size compile: Unicode classes repeated a few
/// hundred times fit the compile limit.
#[test]
fn ordinary_large_patterns_compile() {
    assert_eq!(ok("is_match", &["x", r"\w{400}"]), "false");
    assert_eq!(ok("is_match", &["x", r"\pL{300}"]), "false");
}

/// `split` of the empty string by the empty pattern is `[]`, like the
/// string `split("", "")`.
#[test]
fn split_of_nothing_by_nothing() {
    assert_eq!(ok("split", &["", ""]), "[]");
    assert_eq!(ok("split", &["", "x*"]), r#"[""]"#);
}
