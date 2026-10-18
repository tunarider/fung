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
    "spawn", "await", "cancel", "channel", "send", "recv", "close", "sleep",
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
        .filter(|n| !CONCURRENCY.contains(n))
        .collect();
    let got: BTreeSet<&str> = core().iter().map(|(n, _)| *n).collect();
    assert_eq!(got, want);
    let args = core().into_iter().find(|(n, _)| *n == "args").unwrap().1;
    assert_eq!(format(&args), "[]");
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
const RUN_CASE_COUNT: usize = 358;

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
    let r = (n.func)(&mut rt.task(), args);
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
    b.set(std::rc::Rc::new("a".into()), Value::Int(1));
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
            "Err({kind: 'parse, message: \"not an integer: x\"})",
        ),
        ("parse_float", st("1.5"), "Ok(1.5)"),
        (
            "parse_float",
            st("x"),
            "Err({kind: 'parse, message: \"not a number: x\"})",
        ),
        ("parse_dur", st("1h30m"), "Ok(1h30m)"),
        ("parse_dur", st(" 500ms "), "Ok(500ms)"),
        ("parse_dur", st("-5s"), "Ok(-5s)"),
        (
            "parse_dur",
            st("30"),
            "Err({kind: 'parse, message: \"not a duration: 30\"})",
        ),
        (
            "parse_dur",
            st("1h x"),
            "Err({kind: 'parse, message: \"not a duration: 1h x\"})",
        ),
        (
            "parse_dur",
            st("30m1h"),
            "Err({kind: 'parse, message: \"not a duration: 30m1h\"})",
        ),
        (
            "parse_dur",
            st(""),
            "Err({kind: 'parse, message: \"not a duration: \"})",
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

/// `sym` takes a name (01, UAX #31): letter numerals and combining marks
/// may be part of one; superscripts and circled letters may not.
#[test]
fn sym_requires_a_name() {
    for bad in ["", "a b", "Ab", "1a", "a-b", "x²", "ⓐ"] {
        assert_eq!(code(&call("sym", vec![st(bad)])), "E0513", "sym({bad:?})");
    }
    for good in [
        "ok",
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
fn at_exit_registers_functions_only() {
    assert_eq!(code(&call("at_exit", vec![Value::Int(1)])), "E0403");
    let print = core().into_iter().find(|(k, _)| *k == "print").unwrap().1;
    assert!(matches!(call("at_exit", vec![print]), Ok(Value::Nil)));
}

#[test]
fn string_functions() {
    let cases: Vec<(&str, Vec<Value>, &str)> = vec![
        (
            "split",
            vec![st("a,b,,c"), st(",")],
            "[\"a\", \"b\", \"\", \"c\"]",
        ),
        ("split", vec![st("참치"), st("")], "[\"참\", \"치\"]"),
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
        ("repeat", vec![st("ab"), Value::Int(-1)], "\"\""),
    ];
    for (name, args, want) in cases {
        let shown = format(&list(args.clone()));
        match call(name, args) {
            Ok(v) => assert_eq!(format(&v), want, "{name}{shown}"),
            Err(e) => panic!("{name}{shown}: {e:?}"),
        }
    }
    assert_eq!(code(&call("upper", vec![Value::Int(1)])), "E0403");
    assert_eq!(code(&call("join", vec![Value::Int(1), st("")])), "E0506");
}

#[test]
fn string_size_limits() {
    assert_eq!(
        code(&call("repeat", vec![st("a"), Value::Int(200_000_000)])),
        "E0806"
    );
    assert!(call("repeat", vec![st("ab"), Value::Int(3)]).is_ok());
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
        ("chars(1)", "E0506"),
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
        (
            "xs = [1, 2]\nprint(reverse(xs), xs, range(3), range(1, 3))",
            "[2, 1] [1, 2] 0..3 1..3\n",
        ),
        (
            "print(zip([1, 2, 3], [\"a\", \"b\"]), first([]), first([1]), last([1, 2]))",
            "[(1, \"a\"), (2, \"b\")] nil 1 2\n",
        ),
        (
            "print(take([1, 2, 3], 2), take([1], 5), drop([1, 2, 3], 1), drop([1], 5), take([1], -1))",
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
        ("get({}, 1)", "E0501"),
        ("get([1], 0)", "E0803"),
        ("get([10, 20], 20)", "E0803"),
        ("min([nil, 1])", "E0501"),
        ("max([nil, 1])", "E0501"),
        ("sum([1, \"a\"])", "E0501"),
    ];
    for (src, want) in errs {
        assert_eq!(exec(src, Opts::default()).code, *want, "{src}");
    }
}

// --- more behaviour ---

/// exit keeps fung's whole int; the process status is its low 8 bits.
#[test]
fn exit_keeps_the_whole_code() {
    for (n, status) in [
        (300i64, 44),
        (-1, 255),
        (1 << 40, 0),
        ((1 << 40) + 3, 3),
        (-256, 0),
        (1 << 31, 0),
    ] {
        let r = call("exit", vec![Value::Int(n)]);
        assert!(
            matches!(r, Err(Stop::Exit(m)) if m == n),
            "exit({n}): {r:?}"
        );
        let o = exec(&format!("exit({n})"), Opts::default());
        assert_eq!((o.code.as_str(), o.exit), ("exit", status), "exit({n})");
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
        "c = channel(3)\nsend(c, 1)\nsend(c, 2)\nsend(c, 3)\nclose(c)\n\
         t = spawn(fn(): each(c, x => x == 2 then fail(\"x\") else nil) ;;)\n\
         print(type_of(await(t)), recv(c))",
    );
    assert_eq!(out, "'Err Ok(3)\n", "{err}");
}

/// The function may send into the channel it walks: each value is seen
/// as it arrives, so the walk goes on until the function closes it.
#[test]
fn resend_into_iterated_channel() {
    let (out, err, _) = run(
        "fn step(c, x):\n  print(x)\n  x < 3 then send(c, x + 1) else close(c)\n;;\n\
         c = channel(1)\nsend(c, 1)\neach(c, x => step(c, x))\nprint(\"end\")",
    );
    assert_eq!(out, "1\n2\n3\nend\n", "{err}");
}

/// `first`, `last`, `take` and `drop` on a range build only what they
/// return, so a huge range is no problem for a small result.
#[test]
fn positional_functions_on_huge_ranges() {
    let (out, err, _) = run("r = 5..1000000000005\n\
         print(first(r), last(r), take(r, 3), take(r, -1), first(0..0), last(0..0), take(0..0, 2))\n\
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
