//! Prelude tests: the core natives called directly, the collection
//! functions, the parameter tables against `spec/builtins.txt`, and whole
//! runs against their expected output (`run_cases.rs`).

use std::collections::BTreeSet;
use std::io::Cursor;

use fung_front::diag;
use fung_front::source::SourceMap;

use super::core;
use crate::testutil::{self, Buf, Opts, exec, run};
use crate::value::{List, MapBuilder, NativeParam, Value, format, intern};
use crate::vm::{Runtime, Stop};

/// The concurrency natives (`crate::conc`, not in `core()`).
const CONCURRENCY: &[&str] = &[
    "spawn",
    "await",
    "cancel",
    "channel",
    "send",
    "recv",
    "send_now",
    "recv_now",
    "wait_closed",
    "close",
    "stop_signal",
    "request_stop",
    "sleep",
    "scope",
    "supervise",
    "timeout",
    "with",
    "atom",
    "deref",
    "swap",
    "reset",
];

fn render_params(ps: &[NativeParam]) -> String {
    ps.iter()
        .map(|p| {
            if p.rest {
                format!("...{}", p.name)
            } else if p.default.is_some() {
                format!("{}=?", p.name)
            } else {
                p.name.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[test]
fn prelude_params_match_the_spec() {
    // every Rust prelude native has exactly the parameter list spec/builtins.txt records
    let spec = fung_front::builtins::builtins();
    for (name, v) in core() {
        let Value::Native(n) = v else { continue };
        assert_eq!(n.name, name);
        let line = spec.native_signature(name).expect(name);
        assert_eq!(render_params(&n.params), line, "{name}");
    }
}

#[test]
fn core_has_every_spec_native_but_concurrency() {
    let spec = fung_front::builtins::builtins();
    let want: BTreeSet<&str> = spec
        .prelude_native
        .iter()
        .map(String::as_str)
        .filter(|n| !CONCURRENCY.contains(n) && *n != "args")
        .collect();
    let got: BTreeSet<&str> = core().iter().map(|(n, _)| *n).collect();
    assert_eq!(got, want);
    // `args` has one definition, in the native prelude (VA27)
    let p = crate::boot::native_prelude(&[b"x".to_vec()]);
    assert_eq!(format(&p.0["args"]), "[\"x\"]");
    assert_eq!(
        crate::boot::native_prelude(&[]).0.len(),
        core().len() + 1 + crate::conc::natives().len()
    );
}

#[test]
fn fold_over_a_range_and_strings() {
    let (out, _, _) = crate::testutil::run(
        "print(fold(0..5, 0, (a, x) => a + x), join(map([1, 2], str), \"-\"), str(1.0))",
    );
    assert_eq!(out, "10 1-2 1.0\n");
}

/// How many whole-run cases there are (a case must not go missing
/// unnoticed).
const RUN_CASE_COUNT: usize = 425;

/// Every program in `run_cases.rs`, and the huge-range programs, give
/// the standard output, standard error and exit code of
/// `tests/expected/prelude/runs.txt`.
#[test]
fn runs_match_the_expected_output() {
    use testutil::expected;
    let mut cases: Vec<(String, &'static str)> = super::run_cases::RUN_CASES
        .iter()
        .map(|(src, stdin)| (src.to_string(), *stdin))
        .collect();
    cases.extend(huge_range_cases().into_iter().map(|(src, _)| (src, "")));
    assert_eq!(cases.len(), RUN_CASE_COUNT, "cases");
    // The working directory (the crate's), and the real path of `/tmp`
    // where it is a link (macOS `/private/tmp`), as on systems where it
    // is not.
    let cwd = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let tmp = std::fs::canonicalize("/tmp")
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let mut subs = vec![(cwd.as_str(), "$CRATE")];
    if tmp != "/tmp" {
        subs.push((tmp.as_str(), "/tmp"));
    }
    let got: Vec<expected::Entry> = cases
        .iter()
        .map(|(src, stdin)| {
            let o = exec(
                src,
                Opts {
                    stdin,
                    ..Opts::default()
                },
            );
            let label = if stdin.is_empty() {
                format!("{src:?}")
            } else {
                format!("{src:?} stdin {stdin:?}")
            };
            testutil::outcome_entry(&expected::short_label(&label), &o, &subs)
        })
        .collect();
    expected::verify(
        "prelude/runs.txt",
        "Whole runs of the prelude programs (crates/fung-vm/src/prelude/run_cases.rs and\n\
         `huge_range_cases`). $CRATE stands for crates/fung-vm, /tmp for its real path. Update: FUNG_EXPECTED=bless cargo test -p fung-vm runs_match_the_expected_output",
        64 * 1024,
        &got,
    );
}

// --- direct calls (a native called with a test runtime) ---

/// Calls a core native with already bound arguments: (result, stdout).
fn call_out(name: &str, args: Vec<Value>) -> (Result<Value, Stop>, String) {
    let Some((_, Value::Native(n))) = core().into_iter().find(|(k, _)| *k == name) else {
        panic!("no native {name}");
    };
    let (out, err) = (Buf::default(), Buf::default());
    let rt = Runtime::new(
        Box::new(out.clone()),
        Box::new(err),
        Box::new(Cursor::new(Vec::new())),
        SourceMap::default(),
    );
    let r = match n.body {
        crate::value::NativeBody::Plain(f) => f(&mut rt.task(), args.into()),
        // A step native needs the run loop (`vm/cont.rs`).
        crate::value::NativeBody::Step(_) => rt.task().call_root(&Value::Native(n), args),
    };
    drop(rt);
    (r, out.text())
}

fn call(name: &str, args: Vec<Value>) -> Result<Value, Stop> {
    call_out(name, args).0
}

fn code(r: &Result<Value, Stop>) -> String {
    match r {
        Ok(_) => String::new(),
        Err(Stop::Error(d)) => d.code.clone(),
        Err(Stop::Exit(n)) => format!("exit {n}"),
    }
}

fn first_arg(r: &Result<Value, Stop>) -> String {
    match r {
        Err(Stop::Error(d)) => diag::headline(d),
        _ => String::new(),
    }
}

fn list(items: Vec<Value>) -> Value {
    Value::List(std::rc::Rc::new(List::new(items)))
}

fn st(s: &str) -> Value {
    Value::str(s)
}

#[test]
fn map_functions_copy() {
    let mut b = MapBuilder::default();
    b.set(crate::value::key("a"), Value::Int(1));
    let m = Value::Map(std::rc::Rc::new(b.finish()));
    let put = call("put", vec![m.clone(), st("b"), Value::Int(2)]).unwrap();
    let removed = call("remove", vec![m.clone(), st("a")]).unwrap();
    let merged = call("merge", vec![m.clone(), put.clone()]).unwrap();
    assert_eq!(format(&put), "{a: 1, b: 2}");
    assert_eq!(format(&removed), "{}");
    assert_eq!(format(&merged), "{a: 1, b: 2}");
    assert_eq!(format(&m), "{a: 1}", "the input map changed");
}

#[test]
fn flatten_splices_one_level() {
    let f = call(
        "flatten",
        vec![list(vec![
            list(vec![Value::Int(1)]),
            Value::Int(2),
            list(vec![]),
            list(vec![list(vec![Value::Int(3)])]),
        ])],
    )
    .unwrap();
    assert_eq!(format(&f), "[1, 2, [3]]");
}

#[test]
fn conversions() {
    let sym = |s: &str| Value::Sym(intern(s));
    let cases: Vec<(&str, Value, &str)> = vec![
        ("int", Value::Int(3), "3"),
        ("int", Value::Float(3.9), "3"),
        ("int", Value::Float(-3.9), "-3"),
        ("int", st("42"), "42"),
        ("int", st("-1_000"), "-1000"),
        ("float", Value::Int(2), "2.0"),
        ("float", st("2.5"), "2.5"),
        ("float", st("1e3"), "1000.0"),
        ("str", st("a"), "\"a\""),
        ("str", Value::Int(1), "\"1\""),
        ("str", Value::Float(1.0), "\"1.0\""),
        ("str", sym("ok"), "\"'ok\""),
        ("str", list(vec![Value::Int(1)]), "\"[1]\""),
        ("sym", st("ok"), "'ok"),
        ("sym", sym("x"), "'x"),
        ("parse_int", st("12"), "Ok(12)"),
        (
            "parse_int",
            st("x"),
            "Err({kind: 'parse, message: \"not an integer: \\\"x\\\"\"})",
        ),
        ("parse_float", st("1.5"), "Ok(1.5)"),
        (
            "parse_float",
            st("x"),
            "Err({kind: 'parse, message: \"not a number: \\\"x\\\"\"})",
        ),
        ("parse_dur", st("1h30m"), "Ok(1h30m)"),
        ("parse_dur", st("+500ms"), "Ok(500ms)"),
        ("parse_dur", st("-5s"), "Ok(-5s)"),
        (
            "parse_dur",
            st("30"),
            "Err({kind: 'parse, message: \"not a duration: \\\"30\\\"\"})",
        ),
        (
            "parse_dur",
            st("1h x"),
            "Err({kind: 'parse, message: \"not a duration: \\\"1h x\\\"\"})",
        ),
        (
            "parse_dur",
            st("30m1h"),
            "Err({kind: 'parse, message: \"not a duration: \\\"30m1h\\\"\"})",
        ),
        (
            "parse_dur",
            st(""),
            "Err({kind: 'parse, message: \"not a duration: \\\"\\\"\"})",
        ),
    ];
    for (name, arg, want) in cases {
        let shown = format(&arg);
        match call(name, vec![arg]) {
            Ok(v) => assert_eq!(format(&v), want, "{name}({shown})"),
            Err(e) => panic!("{name}({shown}): {e:?}"),
        }
    }
    let errs: Vec<(&str, Value)> = vec![
        ("int", st("x")),
        ("int", st(" 1")),
        ("int", Value::Bool(true)),
        ("int", Value::Float(f64::INFINITY)),
        ("float", st("abc")),
        ("sym", Value::Int(1)),
    ];
    for (name, arg) in errs {
        let shown = format(&arg);
        assert_eq!(code(&call(name, vec![arg])), "E0513", "{name}({shown})");
    }
    assert_eq!(code(&call("parse_int", vec![Value::Int(1)])), "E0403");
}

#[test]
fn numeric_separators_are_strict() {
    for bad in ["1_", "_1", "1__2", "1_.5", "1._5"] {
        if call("int", vec![st(bad)]).is_ok() && call("float", vec![st(bad)]).is_ok() {
            panic!("{bad:?} accepted by both int and float");
        }
        assert!(
            call("float", vec![st(bad)]).is_err(),
            "float({bad:?}) accepted"
        );
    }
    assert!(matches!(
        call("int", vec![st("1_000")]),
        Ok(Value::Int(1000))
    ));
    assert!(matches!(call("float", vec![st("1_0.5")]), Ok(Value::Float(f)) if f == 10.5));
}

/// `sym` takes what a symbol literal takes (01, UAX #31, SP4): a tag-shaped
/// name too; letter numerals and combining marks may be part of one;
/// superscripts and circled letters may not.
#[test]
fn sym_requires_a_name() {
    for bad in ["", "a b", "1a", "a-b", "x²", "ⓐ"] {
        assert_eq!(code(&call("sym", vec![st(bad)])), "E0513", "sym({bad:?})");
    }
    for good in [
        "ok",
        "Ab",
        "Red",
        "snake_case",
        "Ⅻ",
        "ⅻ",
        "हिंदी",
        "참치",
        "_x",
        "a1",
        "ß1",
        "x٣",
        "한글",
        "ǅx",
    ] {
        assert!(call("sym", vec![st(good)]).is_ok(), "sym({good:?})");
    }
}

#[test]
fn print_joins_args_with_spaces() {
    let (r, out) = call_out(
        "print",
        vec![list(vec![
            Value::Int(1),
            Value::Float(2.0),
            Value::Bool(true),
        ])],
    );
    assert!(matches!(r, Ok(Value::Nil)));
    assert_eq!(out, "1 2.0 true\n");
    let (_, out) = call_out("print", vec![list(vec![])]);
    assert_eq!(out, "\n");
}

#[test]
fn fail_returns_e0803() {
    let r = call("fail", vec![list(vec![st("port"), Value::Int(1)])]);
    assert_eq!(code(&r), "E0803");
    assert_eq!(first_arg(&r), "error[E0803]: port 1");
    let r = call("fail", vec![list(vec![])]);
    assert_eq!(first_arg(&r), "error[E0803]: fail");
}

#[test]
fn string_functions() {
    let cases: Vec<(&str, Vec<Value>, &str)> = vec![
        (
            "split",
            vec![st("a,b,,c"), st(",")],
            "[\"a\", \"b\", \"\", \"c\"]",
        ),
        ("chars", vec![st("참치")], "[\"참\", \"치\"]"),
        (
            "join",
            vec![list(vec![st("a"), Value::Int(1)]), st("-")],
            "\"a-1\"",
        ),
        ("trim", vec![st("  a \n"), Value::Nil], "\"a\""),
        ("trim", vec![st("xxaxx"), st("x")], "\"a\""),
        ("upper", vec![st("aé")], "\"AÉ\""),
        ("lower", vec![st("AÉ")], "\"aé\""),
        ("replace", vec![st("a-b-c"), st("-"), st("+")], "\"a+b+c\""),
        ("starts_with", vec![st("abc"), st("ab")], "true"),
        ("ends_with", vec![st("abc"), st("ab")], "false"),
        ("repeat", vec![st("ab"), Value::Int(2)], "\"abab\""),
        ("repeat", vec![st("ab"), Value::Int(0)], "\"\""),
        ("byte_len", vec![st("한a")], "4"),
        ("byte_len", vec![Value::bytes(b"a\xff".to_vec())], "2"),
        ("bytes", vec![st("한a")], "[237, 149, 156, 97]"),
        ("bytes", vec![st("")], "[]"),
        (
            "from_bytes",
            vec![list(vec![
                Value::Int(237),
                Value::Int(149),
                Value::Int(156),
            ])],
            "\"한\"",
        ),
        (
            "from_bytes",
            vec![list(vec![Value::Int(97), Value::Int(255)])],
            "\"a\\xff\"",
        ),
        ("is_utf8", vec![st("é")], "true"),
        ("is_utf8", vec![Value::bytes(b"\xc3".to_vec())], "false"),
    ];
    for (name, args, want) in cases {
        let shown = format(&list(args.clone()));
        match call(name, args) {
            Ok(v) => assert_eq!(format(&v), want, "{name}{shown}"),
            Err(e) => panic!("{name}{shown}: {e:?}"),
        }
    }
    assert_eq!(code(&call("upper", vec![Value::Int(1)])), "E0403");
    assert_eq!(code(&call("byte_len", vec![Value::Int(1)])), "E0403");
    assert_eq!(code(&call("from_bytes", vec![Value::Int(1)])), "E0403");
    let bad = list(vec![Value::Int(1), Value::Int(256)]);
    assert_eq!(code(&call("from_bytes", vec![bad])), "E0403");
    assert_eq!(code(&call("join", vec![Value::Int(1), st("")])), "E0506");
}

#[test]
fn string_size_limits() {
    assert_eq!(
        code(&call("repeat", vec![st("a"), Value::Int(200_000_000)])),
        "E0806"
    );
    assert!(call("repeat", vec![st("ab"), Value::Int(3)]).is_ok());
    // `replace` checks the size before it builds: 200 million characters
    // here, from 20,000 occurrences of a 10,000-character replacement.
    let (a, b) = (st(&"a".repeat(20_000)), st(&"b".repeat(10_000)));
    assert_eq!(code(&call("replace", vec![a, st("a"), b])), "E0806");
    let a = st(&"a".repeat(60_000_000));
    assert_eq!(code(&call("replace", vec![a, st(""), st("x")])), "E0806");
    // 120 million bytes, but 60 million characters.
    let (a, b) = (st(&"a".repeat(30_000)), st(&"é".repeat(2_000)));
    let v = call("replace", vec![a, st("a"), b]).unwrap();
    assert!(matches!(&v, Value::Str(r) if r.len() == 120_000_000));
}

/// The collection functions, the cases that need only the core
/// natives. The rest use functions of `extras/prelude.fung`
/// (`collection_functions_with_the_fung_prelude`).
#[test]
fn collection_functions() {
    let cases: &[(&str, &str)] = &[
        (
            "print(filter([1, 2, 3, 4], x => x % 2 == 0), filter(0..5, x => x > 2))",
            "[2, 4] [3, 4]\n",
        ),
        (
            "print(fold([1, 2, 3], 0, (acc, x) => acc + x), fold([], 1, (a, x) => a))",
            "6 1\n",
        ),
        ("print(each([1, 2], x => print(x)))", "1\n2\nnil\n"),
        (
            "print(sort([3, 1, 2]), sort([\"b\", \"a\"]), sort_by([\"bb\", \"a\"], len))",
            "[1, 2, 3] [\"a\", \"b\"] [\"a\", \"bb\"]\n",
        ),
        (
            "print(find([1, 2, 3], x => x > 1), find([], x => true), any([1, 2], x => x > 1), all([], x => false), all([1], x => x > 1))",
            "2 nil true true false\n",
        ),
        (
            "print(group_by([\"ab\", \"c\", \"de\"], s => s[..1]))",
            "{a: [\"ab\"], c: [\"c\"], d: [\"de\"]}\n",
        ),
        (
            "m = {b: 1, a: 2}\nprint(pairs(m))",
            "[(\"b\", 1), (\"a\", 2)]\n",
        ),
        (
            "m = {a: 1}\nprint(put(m, \"b\", 2), m)",
            "{a: 1, b: 2} {a: 1}\n",
        ),
        (
            "print(merge({a: 1, b: 1}, {b: 2, c: 3}), remove({a: 1, b: 2}, \"a\"), remove({}, \"x\"))",
            "{a: 1, b: 2, c: 3} {b: 2} {}\n",
        ),
        (
            "print(chars(\"참치\"), map(0..2, x => x * x))",
            "[\"참\", \"치\"] [0, 1]\n",
        ),
        ("print(flatten([[1], [2, [3]], 4]))", "[1, 2, [3], 4]\n"),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", *want), "{src}");
    }
    let errs: &[(&str, &str)] = &[
        ("filter(1, x => x)", "E0506"),
        ("filter([1], x => 1)", "E0505"),
        ("sort([1, \"a\"])", "E0501"),
        ("fold(\"ab\", 0, (a, x) => a)", "E0506"),
        ("group_by([1], x => x)", "E0514"),
        ("put(1, \"a\", 2)", "E0506"),
        ("get([1], 0)", "E0506"),
        ("get({a: 1}, 1)", "E0514"),
        ("chars(1)", "E0403"),
        ("each([1], 1)", "E0403"),
    ];
    for (src, want) in errs {
        assert_eq!(exec(src, Opts::default()).code, *want, "{src}");
    }
}

/// The rest of the collection functions: these cases call functions of
/// `extras/prelude.fung` (`is_empty`, `sum`, `min`, `reverse`, `zip`, …),
/// which Task 9 compiles and boots.
#[test]
fn collection_functions_with_the_fung_prelude() {
    let cases: &[(&str, &str)] = &[
        (
            "print(is_empty([]), is_empty(\"a\"), is_empty({}), is_empty(0..0))",
            "true false true true\n",
        ),
        (
            "print(sum([1, 2.5]), sum([]), min([3, 1, 2]), max([\"b\", \"a\"]), min([]))",
            "3.5 0 1 b nil\n",
        ),
        ("xs = [1, 2]\nprint(reverse(xs), xs)", "[2, 1] [1, 2]\n"),
        (
            "print(zip([1, 2, 3], [\"a\", \"b\"]), first([]), first([1]), last([1, 2]))",
            "[(1, \"a\"), (2, \"b\")] nil 1 2\n",
        ),
        (
            "print(take([1, 2, 3], 2), take([1], 5), drop([1, 2, 3], 1), drop([1], 5), take([1], 0))",
            "[1, 2] [1] [2, 3] [] []\n",
        ),
        (
            "print(contains([1, 2], 2), contains(\"abc\", \"b\"), contains({a: 1}, \"a\"), contains(0..3, 3))",
            "true true true false\n",
        ),
        (
            "print(unique([1, 1.0, 2, 1]), flatten([[1], [2, [3]], 4]))",
            "[1, 2] [1, 2, [3], 4]\n",
        ),
        (
            "m = {b: 1, a: 2}\nprint(keys(m), values(m), pairs(m))",
            "[\"b\", \"a\"] [1, 2] [(\"b\", 1), (\"a\", 2)]\n",
        ),
        (
            "m = {a: 1}\nprint(get(m, \"a\"), get(m, \"z\"), get(m, \"z\", 0), put(m, \"b\", 2), m)",
            "1 nil 0 {a: 1, b: 2} {a: 1}\n",
        ),
        ("print([3, 1, 2] |> sort |> reverse |> first)", "3\n"),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", *want), "{src}");
    }
    let errs: &[(&str, &str)] = &[
        ("min([1, \"a\"])", "E0501"),
        ("keys([])", "E0506"),
        ("get({}, 1)", "E0514"),
        ("get([1], 0)", "E0506"),
        ("get([10, 20], 20)", "E0506"),
        ("min([nil, 1])", "E0501"),
        ("max([nil, 1])", "E0501"),
        ("sum([1, \"a\"])", "E0501"),
    ];
    for (src, want) in errs {
        assert_eq!(exec(src, Opts::default()).code, *want, "{src}");
    }
}

/// `trim` from both ends cuts what cutting the string into characters
/// would: invalid bytes are characters of their own, cut only when `cut`
/// holds them (VA5 finds the end from the back).
#[test]
fn trim_cuts_whole_characters_from_both_ends() {
    use fung_front::text::{Unit, units};
    let reference = |s: &[u8], cut: Option<&[u8]>| -> Vec<u8> {
        let cut: Vec<Unit> = cut.map_or(Vec::new(), |c| units(c).map(|(_, u)| u).collect());
        let gone = |u: Unit| match u {
            Unit::Char(c) if cut.is_empty() => c.is_whitespace(),
            u => cut.contains(&u),
        };
        let us: Vec<(usize, Unit)> = units(s).collect();
        let Some(lo) = us.iter().position(|&(_, u)| !gone(u)) else {
            return Vec::new();
        };
        let hi = us.iter().rposition(|&(_, u)| !gone(u)).unwrap();
        s[us[lo].0..us[hi].0 + us[hi].1.byte_len()].to_vec()
    };
    let strings: [&[u8]; 9] = [
        b"",
        b"  a b  ",
        b"\xe2\x82 a \xe2\x82",
        b"\x82\xe2\x82\xac x \xe2\x82\xac\x82",
        "\u{3000}가\u{2003}".as_bytes(),
        b"\xff\xff",
        b"xx\xffxx",
        "€€a€".as_bytes(),
        b"\xf0\x9f a \xf0\x9f",
    ];
    let cuts: [Option<&[u8]>; 5] = [
        None,
        Some(b"\xe2\x82"),
        Some(b"\x82"),
        Some("€".as_bytes()),
        Some(b"x\xff"),
    ];
    for s in strings {
        for cut in cuts {
            let arg = cut.map_or(Value::Nil, |c| Value::bytes(c.to_vec()));
            let got = call("trim", vec![Value::bytes(s.to_vec()), arg]).unwrap();
            let want = Value::bytes(reference(s, cut));
            assert_eq!(format(&got), format(&want), "trim({s:?}, {cut:?})");
        }
    }
}

/// The prelude written in fung answers a range without spreading it out
/// (VA4), checks its arguments as a native does with E0403 naming the
/// argument (VA7), sums any values `+` adds (VA7), and a lone element of
/// `min`/`max` must have an order too (VA15).
#[test]
fn prelude_in_fung_checks_like_a_native() {
    let cases: &[(&str, &str)] = &[
        (
            "print(min(0..200_000_000), max(0..200_000_000), min(5..5), max(5..=4), max(3..=7))",
            "0 199999999 nil nil 7\n",
        ),
        (
            "print(zip(0..200_000_000, [\"a\", \"b\"]), zip([1], 5..9), zip(0..3, 10..12))",
            "[(0, \"a\"), (1, \"b\")] [(1, 5)] [(0, 10), (1, 11)]\n",
        ),
        (
            "print(sum([1s, 2s]), sum([]), sum([\"a\", \"b\"]), sum(1..=4), sum([[1], [2]]))",
            "3s 0 ab 10 [1, 2]\n",
        ),
        (
            "print(max([1]), min([\"a\"]), max([[1]]), min([2s]), pad_left(\"ab\", 4), pad_right(\"ab\", 4, \"xy\"))",
            "1 a [1] 2s   ab abxy\n",
        ),
        (
            "print(iterate(1, x => x * 2, 3), until(1, x => x + 1, x => x > 3))",
            "8 4\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", *want), "{src}");
    }
    let errs: &[(&str, &str, &str)] = &[
        ("pad_right(\"ab\", 5, [\"x\"])", "E0403", "`pad`"),
        ("pad_left(\"ab\", 5, 1)", "E0403", "`pad`"),
        ("pad_left(\"a\", 3.5)", "E0403", "`n`"),
        ("pad_left(\"a\", \"3\")", "E0403", "`n`"),
        ("pad_left(1, 3)", "E0403", "`s`"),
        ("pad_left(\"a\", 3, \"\")", "E0403", "`pad`"),
        ("iterate(1, x => x * 2, 3.5)", "E0403", "`n`"),
        ("iterate(1, x => x, \"n\")", "E0403", "`n`"),
        ("iterate(1, 2, 3)", "E0403", "`f`"),
        ("until(1, x => x + 1, 3)", "E0403", "`pred`"),
        ("sum(5)", "E0506", "sum"),
        ("sum([1, 2s])", "E0501", "add"),
        ("max([nil])", "E0501", "nil"),
        ("min([{}])", "E0501", "map"),
        ("max([true])", "E0501", "bool"),
        ("max([[nil]])", "E0501", "nil"),
        ("sum([nil])", "E0501", "nil"),
        // Each E0506 reads as a sentence (SP24).
        ("zip(1, [1])", "E0506", "cannot zip the elements of int"),
        ("zip([1], 1)", "E0506", "cannot zip the elements of int"),
        ("get([1], 0)", "E0506", "cannot get a key from list"),
        (
            "unique(1)",
            "E0506",
            "cannot take the unique elements of int",
        ),
    ];
    for (src, want, says) in errs {
        let o = exec(src, Opts::default());
        let said = o
            .error
            .as_ref()
            .map_or(String::new(), |e| e.headline.clone());
        assert_eq!(o.code, *want, "{src}: {said}");
        assert!(said.contains(says), "{src}: {said}");
    }
}

// --- more behaviour ---

/// exit keeps its code, and takes only 0..=255 (TL1): the system keeps
/// the low 8 bits, so a wider code would end as another one.
#[test]
fn exit_takes_a_code_from_0_to_255() {
    for n in [0i64, 4, 255] {
        let r = call("exit", vec![Value::Int(n)]);
        assert!(
            matches!(r, Err(Stop::Exit(m)) if m == n),
            "exit({n}): {r:?}"
        );
        let o = exec(&format!("exit({n})"), Opts::default());
        assert_eq!((o.code.as_str(), o.exit), ("exit", n as i32), "exit({n})");
    }
    for n in [256i64, 300, -1, -256, 1 << 40] {
        let o = exec(&format!("exit({n})"), Opts::default());
        assert_eq!(o.code, "E0403", "exit({n})");
    }
}

/// A write error on stdout or stderr is E0801 at the call, with the
/// writer's message (the native returns the writer's error).
#[test]
fn write_errors_are_internal_errors_at_the_call() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("broken pipe"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    for (src, col) in [("print(1)", 6), ("eprint(1)", 7), ("dbg(1)", 4)] {
        let o = testutil::on_big_stack(move || {
            use fung_front::loader::{Loader, Stage};
            let (b, errs) = Loader::default().compile("-e", src, Stage::All);
            assert!(errs.is_empty());
            let rt = Runtime::new(
                Box::new(Broken),
                Box::new(Broken),
                Box::new(Cursor::new(Vec::new())),
                b.sources,
            );
            rt.boot(Vec::new()).expect("boot");
            rt.load(b.proto.as_ref().expect("an entry"), &b.modules);
            match rt.run() {
                Err(Stop::Error(d)) => (d.code.clone(), d.pos.col, diag::headline(&d)),
                r => panic!("{r:?}"),
            }
        });
        assert_eq!(
            o,
            (
                "E0801".to_string(),
                col,
                "error[E0801]: internal error: broken pipe".to_string()
            ),
            "{src}"
        );
    }
}

/// Natives called with a range never build its list: a huge range is
/// walked in constant memory, and `map` refuses a result past the element
/// limit (02).
#[test]
fn ranges_are_walked_in_place() {
    let (out, _, _) = run(
        "print(fold(0..3000000, 0, (a, x) => a + 1), find(0..1000000000000, x => x > 5), any(0..1000000000000, x => x == 7))",
    );
    assert_eq!(out, "3000000 6 true\n");
    let (_, err, code) = run("print(map(0..4611686018427387904, x => x))");
    assert!(
        err.starts_with("error[E0806]: too many elements (limit 100000000)\n  --> -e:1:10\n"),
        "{err}"
    );
    assert_eq!(code, 1);
}

/// The programs `huge_range_cases` builds: the collection natives on a
/// range of 10^17 and 10^18 elements, with the column of the call.
pub(super) fn huge_range_cases() -> Vec<(String, u32)> {
    let calls: &[(&str, u32)] = &[
        ("sort(R)", 11),
        ("sort_by(R, x => x)", 14),
        ("sort_by(R, 1)", 14),
        ("join(R, \"\")", 11),
        ("join(R, 1)", 11),
        ("flatten(R)", 14),
        ("find(R, x => true)", 11),
        ("find(R, 1)", 11),
        ("any(R, x => true)", 10),
        ("all(R, x => false)", 10),
        ("group_by(R, x => 1)", 15),
        ("map(R, 1)", 10),
        ("map(R, x => x)", 10),
        ("filter(R, 1)", 13),
        ("fold(R, 0, 1)", 11),
        ("each(R, 1)", 11),
    ];
    let mut out = Vec::new();
    for n in ["100000000000000000", "1000000000000000000"] {
        for (call, col) in calls {
            out.push((
                format!("print({})", call.replace('R', &format!("0..{n}"))),
                *col,
            ));
        }
    }
    out
}

/// What each collection function gives for a range far past the element
/// limit: argument errors first, then E0806 where the result would hold
/// every element; the functions that may stop early stop.
#[test]
fn huge_ranges_hit_the_limits_after_argument_checks() {
    const ELEMENTS: &str = "error[E0806]: too many elements (limit 100000000)";
    const CHARS: &str = "error[E0806]: too many characters (limit 100000000)";
    const ARG: &str = "error[E0403]";
    let want: &[(&str, &str)] = &[
        ("sort(R)", ELEMENTS),
        ("sort_by(R, x => x)", ELEMENTS),
        ("sort_by(R, 1)", ARG),
        ("join(R, \"\")", CHARS),
        ("join(R, 1)", ARG),
        ("flatten(R)", ELEMENTS),
        ("find(R, x => true)", "0"),
        ("find(R, 1)", ARG),
        ("any(R, x => true)", "true"),
        ("all(R, x => false)", "false"),
        ("group_by(R, x => 1)", ELEMENTS),
        ("map(R, 1)", ARG),
        ("map(R, x => x)", ELEMENTS),
        ("filter(R, 1)", ARG),
        ("fold(R, 0, 1)", ARG),
        ("each(R, 1)", ARG),
    ];
    let cases = huge_range_cases();
    assert_eq!(cases.len(), 2 * want.len());
    for ((src, _), (_, w)) in cases.iter().zip(want.iter().chain(want.iter())) {
        let (out, err, _) = run(src);
        let got = if err.is_empty() {
            out.trim_end()
        } else {
            err.lines().next().unwrap()
        };
        assert!(got.starts_with(w), "{src}: {got}");
    }
    // Walking a range in place starts walking.
    let (_, err, _) = run("print(fold(0..100000000000000000, 0, (a, x) => x / 0))");
    assert!(err.starts_with("error[E0502]"), "{err}");
}

/// Sorting is stable at every size the merge sort splits at, and a
/// comparison error ends it.
#[test]
fn sort_is_stable_and_stops_at_first_error() {
    let (out, err, _) = run(
        "print(sort_by([(2, \"a\"), (1, \"b\"), (2, \"c\"), (1, \"d\")], ((k, _)) => k))\n\
         print(sort([]), sort([1]), sort([2, 1]))\n\
         for n in [21, 1000]: print(sort(map(0..n, x => n - x)) == map(0..n, x => x + 1)) ;;\n\
         xs = map(0..1000, x => (x * 7919) % 1000)\n\
         print(sort(xs) == map(0..1000, x => x), sort_by(0..100, x => x % 2)[..3])",
    );
    assert_eq!(
        out,
        "[(1, \"b\"), (1, \"d\"), (2, \"a\"), (2, \"c\")]\n[] [1] [1, 2]\ntrue\ntrue\ntrue [0, 2, 4]\n",
        "{err}"
    );
    let (_, err, _) = run("print(sort([1, \"a\"]))");
    assert!(err.starts_with("error[E0"), "{err}");
}

#[test]
fn natives_are_the_same_on_every_thread() {
    fn print_addr() -> usize {
        match core().into_iter().find(|(n, _)| *n == "print").unwrap().1 {
            Value::Native(n) => n as *const _ as usize,
            _ => 0,
        }
    }
    let here = print_addr();
    let there = std::thread::spawn(print_addr).join().unwrap();
    assert_ne!(here, 0);
    assert_eq!(here, there);
}

#[test]
fn natives_can_cross_threads() {
    fn assert_sync<T: Send + Sync>() {}
    assert_sync::<crate::value::Native>();
}

/// A failing function stops a channel walk where it fails: the values not
/// yet received stay in the channel.
#[test]
fn each_failure_leaves_rest() {
    let (out, err, _) = run(
        "(c_tx, c) = channel(3)\nsend(c_tx, 1)\nsend(c_tx, 2)\nsend(c_tx, 3)\nclose(c_tx)\n\
         supervise(fn(s):\n  t = spawn(s, fn(): each(c, x => x == 2 then fail(\"x\") else nil) ;;)\n  \
         print(type_of(await(t)), recv(c))\n;;, e => nil)",
    );
    assert_eq!(out, "'Err Ok(3)\n", "{err}");
}

/// A walk whose function cancels its own scope stops before it takes the
/// next value: that value and the rest stay in the channel (05). Before,
/// the walk received the next value and only then saw the cancellation,
/// so that value was lost.
#[test]
fn a_cancelled_walk_takes_no_more_values() {
    for walk in ["each", "map", "filter", "any"] {
        let src = format!(
            "(tx, rx) = channel(4)\neach([1, 2, 3, 4], x => send(tx, x)!)\nclose(tx)\n\
             fn stop_at(s, x):\n  if x == 2: cancel(s) ;;\n  false\n;;\n\
             print(scope(s => {walk}(rx, x => stop_at(s, x))) |> is_err)\n\
             print(recv(rx), recv(rx))"
        );
        let (out, err, _) = run(&src);
        assert_eq!(out, "true\nOk(3) Ok(4)\n", "{walk}: {err}");
    }
}

/// The function may send into the channel it walks: each value is seen
/// as it arrives, so the walk goes on until the function closes it.
#[test]
fn resend_into_iterated_channel() {
    let (out, err, _) = run(
        "fn step(tx, x):\n  print(x)\n  x < 3 then send(tx, x + 1) else close(tx)\n;;\n\
         (tx, c) = channel(1)\nsend(tx, 1)\neach(c, x => step(tx, x))\nprint(\"end\")",
    );
    assert_eq!(out, "1\n2\n3\nend\n", "{err}");
}

/// `first`, `last`, `take` and `drop` on a range build only what they
/// return, so a huge range is no problem for a small result.
#[test]
fn positional_functions_on_huge_ranges() {
    let (out, err, _) = run("r = 5..1000000000005\n\
         print(first(r), last(r), take(r, 3), take(r, 0), first(0..0), last(0..0), take(0..0, 2))\n\
         print(drop(5..=9, 3), drop(5..9, 10), take(5..=9, 99), last(1..=9223372036854775807), drop(9223372036854775805..=9223372036854775807, 1))");
    assert_eq!(
        out,
        "5 1000000000004 [5, 6, 7] [] nil nil []\n[8, 9] [] [5, 6, 7, 8, 9] 9223372036854775807 [9223372036854775806, 9223372036854775807]\n",
        "{err}"
    );
}

/// A range wider than the int range is not empty: its length is counted
/// exactly, walking it works, and `len` reports what an int cannot hold.
#[test]
fn wide_ranges_count_every_number() {
    let (out, err, _) = run("r = -9000000000000000000..9000000000000000000\n\
         print(find(r, x => x > -8999999999999999999), any(r, x => x < 0), 5 in r, first(r))\n\
         print(len(-9223372036854775808..=9223372036854775806))");
    assert_eq!(
        out, "-8999999999999999998 true true -9000000000000000000\n",
        "{err}"
    );
    assert!(err.starts_with("error[E0503]: integer overflow"), "{err}");
    let (_, err, _) = run("print(map(-9000000000000000000..9000000000000000000, x => x))");
    assert!(err.starts_with("error[E0806]: too many elements"), "{err}");
}

/// `get` is a core map function like `put` (06): a value that is not a
/// map is E0506 at the caller, with no prelude frame in the call path,
/// and a missing key gives the default.
#[test]
fn get_reads_a_key_or_the_default() {
    let o = exec(
        "m = {a: 1}\nprint(get(m, \"a\"), get(m, \"b\"), get(m, \"b\", 0))",
        Opts::default(),
    );
    assert_eq!((o.code.as_str(), o.out.as_str()), ("", "1 nil 0\n"));
    let o = exec("fn f(x) = get(x, 0)\nprint(f([1]))", Opts::default());
    assert_eq!(o.code, "E0506");
    assert!(o.err.contains("cannot get a key from list"), "{}", o.err);
    assert!(!o.err.contains("extras/prelude.fung"), "{}", o.err);
}

/// `lines` splits on `\n`, drops one `\r` that ends a line, and a final
/// newline makes no empty line (06, as `read_lines`). It is a native: a
/// value that is not a string is E0403 at the caller, with no prelude frame
/// in the call path.
#[test]
fn lines_splits_text_into_lines() {
    let o = exec(
        "print(lines(\"\"), lines(\"a\\nb\"), lines(\"a\\n\"), lines(\"a\\r\\nb\\r\\n\"), lines(\"a\\n\\n\"), lines(\"\\n\"), lines(\"a\\r\"), lines(\"\\r\\n\"), lines(\"x\\r\\r\\n\"))",
        Opts::default(),
    );
    assert_eq!(
        (o.code.as_str(), o.out.as_str()),
        (
            "",
            "[] [\"a\", \"b\"] [\"a\"] [\"a\", \"b\"] [\"a\", \"\"] [\"\"] [\"a\"] [\"\"] [\"x\\r\"]\n"
        )
    );
    let o = exec("fn f(x) = lines(x)\nprint(f(1))", Opts::default());
    assert_eq!(o.code, "E0403");
    assert!(
        o.err.contains("argument `s` must be str, got int"),
        "{}",
        o.err
    );
    assert!(!o.err.contains("extras/prelude.fung"), "{}", o.err);
}

/// Natural merge sort (VA23): the order and the stability hold on every
/// shape of input, and an already sorted input costs about n comparisons
/// (a run of the whole input, no merging).
#[test]
fn sort_finds_runs_and_stays_stable() {
    let (out, err, _) = run("fn ok(n, key):\n\
           xs = map(0..n, i => [key(i, n), i])\n\
           r = sort_by(xs, p => p[0])\n\
           good = fold(0..(n - 1), true, (a, i) =>\n\
             a and (r[i][0] < r[i + 1][0] or (r[i][0] == r[i + 1][0] and r[i][1] < r[i + 1][1])))\n\
           good and len(r) == n and sum(map(r, p => p[1])) == sum(0..n)\n\
         ;;\n\
         shapes = [(i, n) => i, (i, n) => n - i, (i, n) => i % 3, (i, n) => (i * 7919) % (n + 1),\n\
                   (i, n) => i % 17, (i, n) => (i / 5) % 2, (i, n) => 0, (i, n) => (i < n / 2) then i else n - i,\n\
                   (i, n) => (i / 40) % 2 == 0 then i else -i, (i, n) => (i / 3) * ((i / 3) % 2 == 0 then 1 else -1)]\n\
         sizes = [2, 3, 31, 32, 33, 63, 64, 65, 100, 1000, 2500]\n\
         print(all(sizes, n => all(shapes, f => ok(n, f))))\n\
         print(sort([3, 2, 2, 1]), sort([1, 2, 2, 3]), sort_by([[3, 'a], [2, 'b], [2, 'c], [1, 'd]], p => p[0]))");
    assert_eq!(
        out, "true\n[1, 2, 2, 3] [1, 2, 2, 3] [[1, 'd], [2, 'b], [2, 'c], [3, 'a]]\n",
        "{err}"
    );
}

#[test]
fn sort_of_sorted_input_is_linear() {
    use super::collections::Sort;
    use crate::vm::pieces::STEP_COMPARES;
    let n = 200_000i64;
    for xs in [(0..n).collect::<Vec<_>>(), (0..n).rev().collect::<Vec<_>>()] {
        let mut sort = Sort::new(
            std::rc::Rc::new(xs.into_iter().map(Value::Int).collect()),
            None,
        );
        let mut pieces = 0;
        let out = loop {
            pieces += 1;
            match sort.run() {
                Ok(Some(v)) => break v,
                Ok(None) => {}
                Err(_) => panic!("no error expected"),
            }
        };
        let Value::List(l) = out else { panic!() };
        assert_eq!(l.len(), n as usize);
        // n comparisons, then the result at STEP_COMPARES / 2 items a piece
        let limit = 7 * (n as usize) / (2 * STEP_COMPARES);
        assert!(pieces <= limit, "{pieces} pieces, limit {limit}");
    }
}
