//! VM tests: the run loop, functions, natives and call-heavy loops, plus
//! the call paths and exit-handler reports, checked byte for byte.

use std::rc::Rc;

use crate::testutil::{self, Opts, exec, run};
use crate::value::{Closure, Native, NativeParam, Value, format};
use crate::vm::{Prelude, Runtime, Stop, Task, link};

fn run_src(src: &str) -> (String, String, i32) {
    run(src)
}

/// The error code a program stops with: "" on success, "exit" for exit(n).
fn code_of(src: &str) -> String {
    exec(src, Opts::default()).code
}

#[test]
fn runtime_error_carries_the_call_path() {
    let (out, err, code) =
        run_src("fn f(n) = n == 0 then 1 / 0 else f(n - 1)\nfn g(x) = f(x) + 1\nprint(g(3))");
    assert_eq!(out, "");
    assert!(
        err.ends_with(
            " call path (innermost first):\n   -e:1       f (3 tail calls elided)\n   -e:2       g\n   -e:3       <entry>\n"
        ),
        "{err}"
    );
    assert_eq!(code, 1);
}

#[test]
fn tail_calls_do_not_grow_and_deep_recursion_is_e0804() {
    let (out, _, _) = run_src(
        "fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)\nprint(count(1000000, 0))",
    );
    assert_eq!(out, "1000000\n");
    let (_, err, code) = run_src("fn d(n) = n == 0 then 0 else 1 + d(n - 1)\nprint(d(200000))");
    assert!(
        err.starts_with("error[E0804]: stack overflow: call depth exceeded 100000"),
        "{err}"
    );
    assert_eq!(code, 1);
}

/// A discarded long list answers from its nodes' memo (02, L2): a slice
/// that leaves the Err out is discarded quietly, one that keeps it is
/// E0606 at the right element, also after the memo was filled.
#[test]
fn a_discarded_long_list_finds_its_err() {
    // `id(…)`: a slice alone as a statement does nothing (E0234).
    let long = "fn id(v) = v\nxs = map(0..200, x => x == 70 then Err(x) else x)\n";
    let quiet = format!("{long}id(xs[71..])\nid(xs[..70])\nid(xs[71..])\nprint(\"ok\")");
    assert_eq!(run_src(&quiet), ("ok\n".into(), "".into(), 0));
    for tail in [
        "id(xs[60..90])",
        "id(xs[..70])\nid(xs[60..90])",
        "id(xs[71..])\nid(xs[60..90])",
    ] {
        let (_, err, code) = run_src(&format!("{long}{tail}\nprint(1)"));
        assert!(
            err.starts_with("error[E0606]: discarded Err: 70\n"),
            "{tail}: {err}"
        );
        assert!(
            err.contains("note: element 11 of the discarded list (of 30)"),
            "{err}"
        );
        assert_eq!(code, 1);
    }
    let pushed = format!("{long}ys = xs[71..] + [Err(\"late\")]\nid(ys[..10])\nid(ys)\nprint(1)");
    let (_, err, _) = run_src(&pushed);
    assert!(
        err.contains("note: element 130 of the discarded list (of 130)"),
        "{err}"
    );
}

/// L8: user code that tail-calls a library function written in fung
/// keeps its place: the error points at that call, and the call path
/// shows the frame the tail call replaced, with the tail calls it had
/// made before (the library's own are counted apart and, like its
/// frames, not shown: VA19).
#[test]
fn a_tail_call_into_the_library_keeps_the_callers_place() {
    let (_, err, code) = run_src(
        "fn firsts(xs):\n  take(xs, \"two\")\n;;\nfn main():\n  r = firsts([1, 2, 3])\n  print(r)\n;;\nmain()",
    );
    assert!(
        err.starts_with("error[E0403]: argument `n` must be int, got str\n  --> -e:2:7\n"),
        "{err}"
    );
    assert!(
        err.contains(
            " call path (innermost first):\n   -e:2       firsts\n   -e:5       main\n   -e:8       <entry>\n"
        ),
        "{err}"
    );
    assert_eq!(code, 1);
    // A tail-recursive caller: its own tail calls are shown on its line.
    let (_, err, _) =
        run_src("fn go(n) = n == 0 then take([1], \"x\") else go(n - 1)\nprint(go(3))");
    assert!(err.contains("\n  --> -e:1:"), "{err}");
    assert!(
        err.contains("   -e:1       go (3 tail calls elided)\n"),
        "{err}"
    );
}

/// A user callback a library function tail-calls does not inherit the
/// library's tail calls, and the frame that called the library stays.
#[test]
fn a_callback_after_a_library_tail_call_keeps_the_path() {
    let (_, err, _) = run_src(
        "import fs\nfn load(p):\n  fs.with_open(p, fn(_h): 1 / 0 ;;)\n;;\nr = load(\"Cargo.toml\")\nprint(r)",
    );
    assert!(err.contains("\n  --> -e:3:29\n"), "{err}");
    assert!(err.contains("   -e:3       <lambda>\n"), "{err}");
    assert!(
        err.contains("   -e:3       load\n   -e:5       <entry>\n"),
        "{err}"
    );
}

/// The deadlock report points at the library call too.
#[test]
fn a_deadlock_in_a_library_tail_call_points_at_the_call() {
    let (_, err, _) = run_src(
        "fn first_two(ch):\n  take(ch, 2)\n;;\n(tx, rx) = channel()\nr = first_two(rx)\nprint(r, tx)",
    );
    assert!(
        err.starts_with("error[E0702]: deadlock: all tasks are waiting\n  --> -e:2:7\n"),
        "{err}"
    );
    assert!(
        err.contains("   -e:2       first_two\n   -e:5       <entry>\n"),
        "{err}"
    );
}

/// The whole E0804 report, as `fung` prints it.
#[test]
fn deep_recursion_report() {
    let (out, err, code) = run_src("fn d(n) = n == 0 then 0 else 1 + d(n - 1)\nprint(d(200000))");
    assert_eq!(out, "");
    let mut want = String::from(
        "error[E0804]: stack overflow: call depth exceeded 100000\n  --> -e:1:35\n  |\n1 | fn d(n) = n == 0 then 0 else 1 + d(n - 1)\n  |                                   ^\n  |\n call path (innermost first):\n",
    );
    for _ in 0..8 {
        want.push_str("   -e:1       d\n");
    }
    want.push_str("   … 99990 more\n   -e:1       d\n   -e:2       <entry>\n help: make the recursion a tail call or use `fold`\n");
    assert_eq!(err, want);
    assert_eq!(code, 1);
}

// --- the run loop ---------------------------------------------------------

#[test]
fn evaluates_expressions() {
    let cases = [
        ("1 + 2 * 3", "7"),
        ("(1 + 2) * 3", "9"),
        ("10 / 4", "2.5"),
        ("7 // 2", "3"),
        ("-7 % 3", "2"),
        ("2 ** 10", "1024"),
        ("-2 ** 2", "-4"),
        ("1 == 1.0", "true"),
        ("1 != nil", "true"),
        ("2 >= 3", "false"),
        ("not true", "false"),
        ("true and false", "false"),
        ("false or true", "true"),
        ("_x = 1\n2", "2"),
        ("", "nil"),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!(
            (o.code.as_str(), o.value.as_deref()),
            ("", Some(want)),
            "{src:?}"
        );
    }
}

/// The run loop's int paths (VM9) give what the value layer gives: the
/// floor of a quotient and a remainder with the divisor's sign, and zero
/// and overflow end in the same errors. The operands come from a call so
/// no constant is folded.
#[test]
fn int_operators_agree_with_the_value_layer() {
    let cases = [
        ("a // b", 7, 2, "3"),
        ("a // b", -7, 2, "-4"),
        ("a // b", 7, -2, "-4"),
        ("a // b", -7, -2, "3"),
        ("a % b", -7, 2, "1"),
        ("a % b", 7, -2, "-1"),
        ("a % b", -7, -2, "-1"),
        ("a % b", i64::MIN, -1, "0"),
        ("a == b", 3, 3, "true"),
        ("a != b", 3, 3, "false"),
        ("a == b", 3, 4, "false"),
        ("a < b", 3, 4, "true"),
        ("a >= b", 3, 4, "false"),
        ("a - b", 3, 4, "-1"),
    ];
    for (op, a, b, want) in cases {
        let src = format!("fn f(a, b) = {op}\nf({a}, {b})");
        let o = exec(&src, Opts::default());
        assert_eq!(
            (o.code.as_str(), o.value.as_deref()),
            ("", Some(want)),
            "{op} {a} {b}"
        );
    }
    for (op, a, b, code) in [
        ("a // b", 1, 0, "E0502"),
        ("a % b", 1, 0, "E0502"),
        ("a // b", i64::MIN, -1, "E0503"),
        ("a + b", i64::MAX, 1, "E0503"),
    ] {
        let src = format!("fn f(a, b) = {op}\nf({a}, {b})");
        assert_eq!(code_of(&src), code, "{op} {a} {b}");
    }
}

/// Defaults, named arguments and a rest parameter bound where the
/// arguments lie (VM6): in a call, a tail call and a native's callback,
/// with the errors of the general binding.
#[test]
fn arguments_bind_in_place() {
    let defs = "fn f(a, b = 10, c = a + b, ...r) = [a, b, c, r]\n\
                fn t(n, acc = 0) = n == 0 then acc else t(n - 1, acc: acc + n)\n\
                fn g(x, y = 1) = x * y\n";
    let cases = [
        ("f(1)", "[1, 10, 11, []]"),
        ("f(1, 2)", "[1, 2, 3, []]"),
        ("f(1, 2, 3, 4, 5)", "[1, 2, 3, [4, 5]]"),
        ("f(1, c: 7)", "[1, 10, 7, []]"),
        ("f(b: 2, a: 1)", "[1, 2, 3, []]"),
        ("t(100)", "5050"),
        ("t(3, acc: 1)", "7"),
        ("map([1, 2], g)", "[1, 2]"),
        ("fn h(...xs) = xs\nh(1, 2)", "[1, 2]"),
        ("fn h(...xs) = xs\nh()", "[]"),
    ];
    for (src, want) in cases {
        let o = exec(&format!("{defs}{src}"), Opts::default());
        assert_eq!(
            (o.code.as_str(), o.value.as_deref()),
            ("", Some(want)),
            "{src}"
        );
    }
    for (src, code) in [
        ("f()", "E0407"),
        ("f(1, d: 2)", "E0406"),
        ("f(1, a: 2)", "E0405"),
        ("g(1, 2, 3)", "E0402"),
        ("fn k(a, b) = a\nk(b: 1)", "E0407"),
    ] {
        assert_eq!(code_of(&format!("{defs}{src}")), code, "{src}");
    }
}

/// A native called with defaults, named arguments or a rest parameter
/// binds where its arguments lie, as a fung function does, with the
/// same errors: plain and step natives, in tail position too.
#[test]
fn native_arguments_bind_in_place() {
    let cases = [
        ("get({a: 1}, \"b\")", "nil"),
        ("get({a: 1}, \"b\", default: 2)", "2"),
        ("get(m: {a: 1}, k: \"a\")", "1"),
        ("get({a: 1}, k: \"a\")", "1"),
        ("join([\"a\", \"b\"])", "\"ab\""),
        ("join([\"a\", \"b\"], sep: \"-\")", "\"a-b\""),
        ("fn f(m) = get(m, \"a\", default: 0)\nf({})", "0"),
        ("map(f: x => x + 1, xs: [1, 2])", "[2, 3]"),
        ("fn f(xs) = map(xs, f: x => x * 2)\nf([1, 2])", "[2, 4]"),
        ("import path\npath.join(\"a\", \"b\", \"c\")", "\"a/b/c\""),
        ("import conc\nconc.retry(() => Ok(1), times: 2)", "Ok(1)"),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!(
            (o.code.as_str(), o.value.as_deref()),
            ("", Some(want)),
            "{src}"
        );
    }
    for (src, want) in [
        (
            "get({}, \"a\", x: 1)",
            "error[E0406]: get has no argument named `x`",
        ),
        (
            "get({}, \"a\", k: \"b\")",
            "error[E0405]: argument `k` given twice",
        ),
        ("get({})", "error[E0407]: missing argument `k`"),
        (
            "get({}, 1, 2, 3)",
            "error[E0402]: get takes 3 argument(s), got 4",
        ),
        (
            "trim(\" a \", cut: 1)",
            "error[E0403]: argument `cut` must be str, got int",
        ),
        (
            "print(1, sep: 2)",
            "error[E0406]: print has no argument named `sep`",
        ),
    ] {
        let (_, err, _) = run_src(src);
        assert_eq!(err.lines().next(), Some(want), "{src}");
    }
}

/// The leaner code of VM10 means what it meant: a subject read in place,
/// arms that always match, `and`/`or` as jumps (errors at the same
/// places), a statement `if` without `else`, returns in tail branches and
/// after `?`, and folded constants.
#[test]
fn leaner_code_keeps_its_meaning() {
    let cases = [
        // A pattern that fails part way after binding the subject's own
        // name: the next arm still sees the subject.
        (
            "fn f(m) = match m:\n  [k] when k > 5 => \"big\"\n  [k] => \"small ${k}\"\n  _ => \"other\"\n;;\nf([3])",
            "\"small 3\"",
        ),
        (
            "fn f(x) = match x:\n  (1, y) => y\n  (a, b) => a + b\n;;\nf((2, 5))",
            "7",
        ),
        (
            "fn f(x) = match x:\n  0 => \"z\"\n  n => n * 2\n;;\nf(4)",
            "8",
        ),
        (
            "fn f(x) = match x:\n  n when n > 0 => 1\n  _ => 2\n;;\nf(4)",
            "1",
        ),
        (
            "x = [1, 2]\nmatch x:\n  [a] => a\n  other => len(other)\n;;",
            "2",
        ),
        ("a = 1\nb = 2\na < b and b < 3", "true"),
        ("a = 1\nif a > 1 or a == 1: \"y\" else: \"n\" ;;", "\"y\""),
        ("a = 1\nif a > 1 and 1 / 0 == 1: 1 else: 2 ;;", "2"),
        ("a = 1\nif a == 1 or 1 / 0 == 1: 1 else: 2 ;;", "1"),
        (
            "fn f(r) = Ok(r? + 1)\n[f(Ok(1)), f(Err(2))]",
            "[Ok(2), Err(2)]",
        ),
        ("fn f(c) = c then 1 else 2\nf(true) + f(false)", "3"),
        ("fn f(x) = x ?? 9\n(f(nil), f(Ok(1)))", "(9, 1)"),
        (
            "[1 + 2 * 3, -4, 7 // -2, 0..=2, 1.5 * 2]",
            "[7, -4, -4, 0..=2, 3.0]",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!(
            (o.code.as_str(), o.value.as_deref()),
            ("", Some(want)),
            "{src}"
        );
    }
    for (src, code, at) in [
        ("x = 5\nif x and true: 1 else: 2 ;;", "E0505", "-e:2:4"),
        ("x = 5\nif true and x: 1 else: 2 ;;", "E0505", "-e:2:13"),
        ("x = 5\nif false or x: 1 else: 2 ;;", "E0505", "-e:2:13"),
        ("x = 5\n_y = true and x", "E0505", "-e:2:15"),
        ("fn f() = Err(1)\nif true: f() ;;\n1", "E0606", "-e:2:1"),
        (
            "fn f(c, s) -> int = c then 1 else s\nf(false, \"s\")",
            "E0404",
            "-e:1:19",
        ),
        ("fn f(r) -> int = r? + 1\nf(Err(1))", "E0404", "-e:1:19"),
        ("print(9223372036854775807 + 1)", "E0503", "-e:1:27"),
        ("print(1 // 0)", "E0502", "-e:1:9"),
    ] {
        let (_, err, _) = run_src(src);
        assert!(
            err.starts_with(&format!("error[{code}]")) && err.contains(at),
            "{src}: {err}"
        );
    }
}

#[test]
fn short_circuit_skips_right_side() {
    // 1 / 0 would be E0502 if evaluated.
    assert_eq!(code_of("false and (1 / 0 == 1)"), "");
    assert_eq!(code_of("true or (1 / 0 == 1)"), "");
}

#[test]
fn print_writes_to_runtime_stdout() {
    let (out, err, code) = run_src("print(1 + 2 * 3)\nprint(1, 2.0, true)\nprint()");
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("7\n1 2.0 true\n\n", "", 0)
    );
}

#[test]
fn runtime_errors_carry_code_and_position() {
    let cases = [
        ("1 / 0", "E0502"),
        ("9223372036854775807 + 1", "E0503"),
        ("1 + true", "E0501"),
        ("1e308 * 10", "E0504"),
        ("not 1", "E0505"),
        ("1 and true", "E0505"),
        ("true and 1", "E0505"),
        ("false or 1", "E0505"),
        ("1(2)", "E0401"),
        ("-true", "E0506"),
    ];
    for (src, want) in cases {
        assert_eq!(code_of(src), want, "{src:?}");
    }
    let o = exec("print(1 +\n 2 / 0)", Opts::default());
    let e = o.error.expect("an error");
    assert_eq!((e.line, e.col), (2, 4), "position of `/`");
}

#[test]
fn rust_panic_becomes_internal_error() {
    let o = exec(
        "boom()",
        Opts {
            extra: testutil::boom,
            ..Opts::default()
        },
    );
    assert_eq!(o.code, "E0801");
    assert_eq!(
        o.err,
        "error[E0801]: internal error: x\n  --> -e:1:5\n  |\n1 | boom()\n  |     ^\n  |\n call path (innermost first):\n   -e:1       <entry>\n"
    );
    assert_eq!(o.exit, 1);
}

#[test]
fn variables_blocks_and_literals() {
    let cases = [
        ("x = 1\ny = x + 1\nz = x * 10\nprint(z, y)", "10 2\n"),
        ("if true: z = 5; print(z) ;;", "5\n"),
        (
            "w = if 7 > 5: \"big\" else: \"small\" ;;\nprint(w)",
            "big\n",
        ),
        ("print(if false: 1 else: nil ;;)", "nil\n"),
        ("print(1 > 0 then \"a\" else \"b\")", "a\n"),
        (
            "x = 1\nprint(\"x is ${x}, next ${x + 1}\")",
            "x is 1, next 2\n",
        ),
        (
            "print(\"a\" + \"b\", \"a\" == \"a\", \"a\" < \"b\")",
            "ab true true\n",
        ),
        // A symbol is not a string: not equal (D91).
        ("print('ok, 'ok == 'ok, 'ok == nil)", "'ok true false\n"),
        (
            "print(1h30m, 500ms + 1.5s, 2 * 30s, 1m / 4, 30s < 1m)",
            "1h30m 2s 1m 15s true\n",
        ),
        ("print(nil, nil == nil, nil == 0)", "nil true false\n"),
        ("print(-9223372036854775808)", "-9223372036854775808\n"),
        ("print(9007199254740993 == 9007199254740992.0)", "false\n"),
        ("print(\"${\"q\"}\", \"${'s}\", \"${1.0}\")", "q 's 1.0\n"),
        ("fn f():\n  x = 1\n  y = 2\n  x + y\n;;\nprint(f())", "3\n"),
    ];
    for (src, want) in cases {
        let (out, err, code) = run_src(src);
        assert_eq!((out.as_str(), err.as_str(), code), (want, "", 0), "{src:?}");
    }
}

/// Interpolation keeps to the string limit like `+` (02).
#[test]
fn interpolation_past_the_string_limit_is_an_error() {
    let o = exec(
        "s = repeat(\"a\", 60000000)\nprint(len(\"${s}${s}\"))",
        Opts::default(),
    );
    assert_eq!(o.code, "E0806");
    let o = exec(
        "s = repeat(\"é\", 30000000)\nprint(len(\"${s}${s}\"))",
        Opts::default(),
    );
    assert_eq!((o.code.as_str(), o.out.as_str()), ("", "60000000\n"));
}

#[test]
fn block_value_and_statement_value() {
    let o = exec("_x = 1", Opts::default());
    assert_eq!((o.code.as_str(), o.value.as_deref()), ("", Some("nil")));
    let o = exec("if true: a = 1; a + 1 ;;", Opts::default());
    assert_eq!(o.value.as_deref(), Some("2"));
}

#[test]
fn fail_raises_e0803_with_position() {
    let o = exec("print(1)\nfail(\"boom\", 2)", Opts::default());
    assert_eq!((o.code.as_str(), o.out.as_str()), ("E0803", "1\n"));
    let o = exec("_x = 1\nfail(\"boom\")", Opts::default());
    let e = o.error.expect("an error");
    assert_eq!(
        (e.headline.as_str(), e.line, e.col),
        ("error[E0803]: boom", 2, 5)
    );
    let o = exec("fail()", Opts::default());
    assert_eq!(o.error.expect("an error").headline, "error[E0803]: fail");
}

#[test]
fn runtime_errors_for_new_types() {
    let cases = [
        ("nil + 1", "E0501"),
        ("\"a\" + 1", "E0501"),
        ("'a < 'b", "E0501"),
        ("1s * 1s", "E0501"),
        ("1s / 0", "E0502"),
        ("if 1: 2 ;;", "E0505"),
        ("1 then 2 else 3", "E0505"),
    ];
    for (src, want) in cases {
        assert_eq!(code_of(src), want, "{src:?}");
    }
}

// --- functions ------------------------------------------------------------

fn outputs(cases: &[(&str, &str)]) {
    for (src, want) in cases {
        let (out, err, code) = run_src(src);
        assert_eq!(
            (out.as_str(), err.as_str(), code),
            (*want, "", 0),
            "{src:?}"
        );
    }
}

fn codes(cases: &[(&str, &str)]) {
    for (src, want) in cases {
        assert_eq!(code_of(src), *want, "{src:?}");
    }
}

#[test]
fn functions_run() {
    outputs(&[
        ("fn add(a, b) = a + b\nprint(add(1, 2))", "3\n"),
        (
            "fn fib(n) = n < 2 then n else fib(n - 1) + fib(n - 2)\nprint(fib(20))",
            "6765\n",
        ),
        (
            "print(even(10), odd(10))\nfn even(n) = n == 0 then true else odd(n - 1)\nfn odd(n) = n == 0 then false else even(n - 1)",
            "true false\n",
        ),
        ("f = x => x * 2\nprint(f(21))", "42\n"),
        ("print((fn(a):\n  a + 1\n;;)(1))", "2\n"),
        ("fn f():\n  x = 1\n  y = 2\n  x + y\n;;\nprint(f())", "3\n"),
        ("fn f() = return 1\nprint(f())", "1\n"),
        (
            "fn f(n):\n  if n > 0: return \"pos\" ;;\n  \"other\"\n;;\nprint(f(1), f(0))",
            "pos other\n",
        ),
        ("fn f() = return\nprint(f())", "nil\n"),
        ("fn f():\n  _ = 1\n  nil\n;;\nprint(f())", "nil\n"),
        (
            "fn add(a, b) = a + b\nprint(add, x => x)",
            "<fn add> <fn>\n",
        ),
    ]);
}

#[test]
fn closures_capture_by_value() {
    outputs(&[
        // a local: copied when f was made, and kept after its frame
        // is gone (a name is bound once, D88, so nothing can show the
        // copy apart from a shared variable)
        (
            "fn g():\n  x = 1\n  f = () => x\n  f()\n;;\nprint(g())",
            "1\n",
        ),
        (
            "fn mk():\n  x = 1\n  () => x\n;;\nf = mk()\nprint(f())",
            "1\n",
        ),
        (
            "fn adder(n) = x => x + n\nadd5 = adder(5)\nprint(add5(1), adder(10)(1))",
            "6 11\n",
        ),
        (
            "fn outer(a) = b => c => a + b + c\nprint(outer(1)(2)(3))",
            "6\n",
        ),
        (
            "fn outer():\n  fn loop(n, acc) = n == 0 then acc else loop(n - 1, acc + n)\n  loop(4, 0)\n;;\nprint(outer())",
            "10\n",
        ),
        // self through a lambda
        (
            "fn outer(y):\n  fn f(n) = n == 0 then y else (() => f(n - 1))()\n  f(3)\n;;\nprint(outer(7))",
            "7\n",
        ),
    ]);
}

#[test]
fn arguments_defaults_and_names() {
    outputs(&[
        ("fn f(a, b = a * 2) = a + b\nprint(f(1), f(1, 5))", "3 6\n"),
        (
            "fn f(a, b = 1, c = 2) = a + b * 10 + c * 100\nprint(f(1, c: 3), f(c: 3, a: 2))",
            "311 312\n",
        ),
        (
            "fn f(greeting = \"hi\", name = \"you\") = greeting + \", \" + name\nprint(f(name: \"fung\"))",
            "hi, fung\n",
        ),
        ("f = (a, b) => a - b\nprint(f(b: 1, a: 3))", "2\n"),
        // default evaluated only when needed
        ("fn f(a = print(\"eval\")) = a\nf(1)\nf()", "eval\n"),
    ]);
}

#[test]
fn tail_calls_do_not_grow_the_stack() {
    outputs(&[
        (
            "fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)\nprint(count(1000000, 0))",
            "1000000\n",
        ),
        (
            "fn even(n) = n == 0 then true else odd(n - 1)\nfn odd(n) = n == 0 then false else even(n - 1)\nprint(even(300001))",
            "false\n",
        ),
    ]);
}

#[test]
fn non_tail_recursion_hits_the_frame_limit() {
    codes(&[
        (
            "fn f(n) = n == 0 then 0 else 1 + f(n - 1)\nprint(f(50000))",
            "",
        ),
        (
            "fn f(n) = n == 0 then 0 else 1 + f(n - 1)\nprint(f(200000))",
            "E0804",
        ),
        // A return type keeps the frame for calls to other functions, but a
        // self tail call reuses it (03, T4).
        (
            "fn count(n, acc) -> int = n == 0 then acc else count(n - 1, acc + 1)\nprint(count(200000, 0))",
            "",
        ),
        (
            "fn g(n) = n\nfn h(n) -> int = n == 0 then 0 else 1 + h(n - 1)\nprint(h(200000))",
            "E0804",
        ),
    ]);
}

#[test]
fn type_annotations_are_checked() {
    outputs(&[
        (
            "fn f(_n: int, s: str = \"x\") -> str = s\nprint(f(1), f(2, \"y\"))",
            "x y\n",
        ),
        ("fn f(n: num) = n\nprint(f(1), f(1.5))", "1 1.5\n"),
        ("fn f(v: str | nil) = v\nprint(f(nil), f(\"a\"))", "nil a\n"),
        ("fn f(v: any) = v\nprint(f('s))", "'s\n"),
    ]);
    codes(&[
        ("fn f(n: int) = n\nf(\"a\")", "E0403"),
        // default is checked too
        ("fn f(n: int, s: str = 1) = [n, s]\nf(1)", "E0403"),
        ("fn f() -> str = 1\nf()", "E0404"),
        ("fn f(a, b) = a\nf(1, 2, 3)", "E0402"),
        ("fn f(a) = a\nf(1, a: 2)", "E0405"),
        ("fn f(a) = a\nf(b: 1)", "E0406"),
        ("fn f(a, b) = a\nf(1)", "E0407"),
        ("print(x: 1)", "E0406"),
        ("f = 1\nf()", "E0401"),
    ]);
}

/// The messages of the calling-convention errors.
#[test]
fn calling_convention_messages() {
    let cases = [
        (
            "fn f(n: int) = n\na = \"a\"\nf(a)",
            "error[E0403]: argument `n` must be int, got str",
        ),
        (
            "fn f(v: str | nil) = v\na = 1\nf(a)",
            "error[E0403]: argument `v` must be str | nil, got int",
        ),
        (
            "fn f(a) -> str = a\nf(1)",
            "error[E0404]: return value must be str, got int",
        ),
        (
            "f = 1\nf()",
            "error[E0401]: cannot call a value of type int",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!(o.error.expect(src).headline, want, "{src:?}");
    }
}

/// A call the file shows the target of is checked before anything runs
/// (D43, D110): by the function's own name, through a name bound to it,
/// or written where it is called. The error is the one the same call
/// gives at run time, where the function comes in as a parameter.
#[test]
fn calls_of_known_functions_are_the_runs_errors() {
    // The function, the arguments, the error.
    let cases = [
        (
            "fn f(a, b) = [a, b]",
            "1, 2, 3",
            "error[E0402]: f takes 2 argument(s), got 3",
        ),
        (
            "fn f(a) = a",
            "1, a: 2",
            "error[E0405]: argument `a` given twice",
        ),
        (
            "fn f(a) = a",
            "b: 1",
            "error[E0406]: f has no argument named `b`",
        ),
        (
            "fn f(a, b) = [a, b]",
            "1",
            "error[E0407]: missing argument `b`",
        ),
        (
            "f = x => x",
            "1, 2",
            "error[E0402]: the function takes 1 argument(s), got 2",
        ),
        (
            "f = fn(a, b): [a, b] ;;",
            "1",
            "error[E0407]: missing argument `b`",
        ),
        (
            "f = len",
            "1, 2",
            "error[E0402]: len takes 1 argument(s), got 2",
        ),
        (
            "fn g(a, b) = [a, b]\nf = g(1, _)",
            "1, 2",
            "error[E0402]: the function takes 1 argument(s), got 2",
        ),
    ];
    for (def, args, want) in cases {
        let ran = format!("{def}\nfn via(h) = h({args})\nprint(\"ran\")\nprint(via(f))");
        let o = exec(&ran, Opts::default());
        assert_eq!(o.error.expect(&ran).headline, want, "{ran:?}");
        let alias = format!("{def}\nh = f\nprint(\"ran\")\nprint(h({args}))");
        let inner = format!("{def}\nfn k():\n  h = f\n  h({args})\n;;\nprint(\"ran\")\nprint(k())");
        for src in [
            format!("{def}\nprint(\"ran\")\nprint(f({args}))"),
            alias,
            inner,
        ] {
            let o = exec(&src, Opts::default());
            assert!(o.error.is_none(), "{src:?}");
            assert_eq!(o.exit, 2, "{src:?}");
            assert!(
                o.err.starts_with(&format!("{want}\n")),
                "{src:?}: {}",
                o.err
            );
        }
    }
    let ran = "fn via(h) = h(1, 2)\nprint(\"ran\")\nprint(via(x => x))";
    let o = exec(ran, Opts::default());
    let want = o.error.expect(ran).headline;
    let o = exec("print(\"ran\")\nprint((x => x)(1, 2))", Opts::default());
    assert!(o.err.starts_with(&format!("{want}\n")), "{}", o.err);
}

#[test]
fn runtime_errors_carry_the_call_path() {
    // g's call is not a tail call (+ 0), so g keeps its own frame.
    let o = exec("fn f(x) = x / 0\nfn g() = f(1) + 0\ng()", Opts::default());
    let e = o.error.expect("an error");
    assert_eq!(e.code, "E0502");
    let path: Vec<(&str, u32)> = e.path.iter().map(|c| (c.func.as_str(), c.line)).collect();
    assert_eq!(path, [("f", 1), ("g", 2), ("<main>", 3)]);
    let o = exec(
        "fn loop(n) = n == 0 then 1 / 0 else loop(n - 1)\nloop(5)",
        Opts::default(),
    );
    assert_eq!(o.error.expect("an error").path[0].elided, 5);
}

/// A runtime error inside a lambda that a native calls reports the call
/// path from the failing frame out, through the native's caller (Review
/// Focus 3).
#[test]
fn errors_inside_lambdas_called_by_natives_carry_call_paths() {
    let (_, err, code) =
        run_src("print(fold(0..3, 0, (a, _x) => a + fold(0..3, 0, (b, y) => b + 1 / (y - 2))))");
    assert_eq!(
        err,
        "error[E0502]: division by zero\n  --> -e:1:66\n  |\n1 | print(fold(0..3, 0, (a, _x) => a + fold(0..3, 0, (b, y) => b + 1 / (y - 2))))\n  |                                                                  ^\n  |\n call path (innermost first):\n   -e:1       <lambda>\n   -e:1       <lambda>\n   -e:1       <entry>\n"
    );
    assert_eq!(code, 1);
    let (_, err, _) =
        run_src("fn f(_x) = fold([1], 0, (_a, b) => g(b))\nfn g(n) = n / 0\nprint(f(1))");
    assert!(
        err.ends_with(" call path (innermost first):\n   -e:2       g (1 tail call elided)\n   -e:1       f\n   -e:3       <entry>\n"),
        "{err}"
    );
    // A calling-convention error of the lambda points at the native's call.
    let (_, err, _) = run_src("_ = fold([1], 0, (a) => a)");
    assert_eq!(
        err,
        "error[E0402]: the function takes 1 argument(s), got 2\n  --> -e:1:9\n  |\n1 | _ = fold([1], 0, (a) => a)\n  |         ^\n  |\n call path (innermost first):\n   -e:1       <entry>\n"
    );
    // exit inside a lambda ends the program with its code.
    let (out, err, code) = run_src("_ = fold([1], 0, (_a, _b) => exit(3))\nprint(1)");
    assert_eq!((out.as_str(), err.as_str(), code), ("", "", 3));
}

/// `exit` and errors with `with`'s cleanup: the exit codes and the order
/// `fung` shows (Review Focus 4; `at_exit` gave way to `with`).
#[test]
fn exit_codes_and_order_with_cleanup() {
    let cases: &[(&str, &str, &str, i32)] = &[
        // A runtime error in `use`, then the release.
        (
            "with(() => nil, _ => print(1 / 0), _ => print(\"bye\"))",
            "bye\n",
            "error[E0502]: division by zero\n  --> -e:1:30\n  |\n1 | with(() => nil, _ => print(1 / 0), _ => print(\"bye\"))\n  |                              ^\n  |\n call path (innermost first):\n   -e:1       <lambda>\n   -e:1       <entry>\n",
            1,
        ),
        // exit inside a release after a runtime error sets the code, and
        // `use`'s error still goes up and is reported (cleanup.rs).
        (
            "with(() => nil, _ => 1 / 0, _ => exit(5))",
            "",
            "error[E0502]: division by zero\n  --> -e:1:24\n  |\n1 | with(() => nil, _ => 1 / 0, _ => exit(5))\n  |                        ^\n  |\n call path (innermost first):\n   -e:1       <lambda>\n   -e:1       <entry>\n",
            5,
        ),
    ];
    for (src, out, err, code) in cases {
        let (o, e, c) = run_src(src);
        assert_eq!((o.as_str(), e.as_str(), c), (*out, *err, *code), "{src:?}");
    }
}

// --- natives ----------------------------------------------------------------

#[test]
fn natives_share_the_calling_convention() {
    let with_pad = |src: &str| {
        exec(
            src,
            Opts {
                extra: testutil::pad,
                ..Opts::default()
            },
        )
    };
    let cases = [
        (r#"print(pad("a"))"#, "a**[]\n"),
        (r#"print(pad("a", 3))"#, "a***[]\n"),
        (r#"print(pad("a", n: 1))"#, "a*[]\n"),
        (r#"print(pad("a", 1, 'x, 'y))"#, "a*['x, 'y]\n"),
        (r#"print(pad(n: 0, s: "b"))"#, "b[]\n"),
        // same as the default
        (r#"print(pad("a", n: 2))"#, "a**[]\n"),
    ];
    for (src, want) in cases {
        let o = with_pad(src);
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", want), "{src}");
    }
    let errs = [
        (r#"pad()"#, "E0407"),
        // a name twice is caught by the parser; positional and named for
        // the same parameter only at the call
        (r#"pad("a", n: 1, n: 2)"#, "E0116"),
        (r#"pad("a", s: "b")"#, "E0405"),
        (r#"pad("a", m: 1)"#, "E0406"),
        // rest params cannot be named
        (r#"print(1, sep: "")"#, "E0406"),
        (r#"len()"#, "E0407"),
        (r#"len(1, 2)"#, "E0402"),
        (r#"type_of()"#, "E0407"),
    ];
    for (src, want) in errs {
        assert_eq!(with_pad(src).code, want, "{src}");
    }
    assert_eq!(
        with_pad("fn via(l) = l(1, 2)\nvia(len)")
            .error
            .unwrap()
            .headline,
        "error[E0402]: len takes 1 argument(s), got 2"
    );
}

#[test]
fn exit_unwinds_as_a_signal() {
    let o = exec(
        "fn f(): print(1); exit(3); print(2) ;;\nf()\nprint(3)",
        Opts::default(),
    );
    assert_eq!(
        (o.code.as_str(), o.out.as_str(), o.exit),
        ("exit", "1\n", 3)
    );
    let o = exec("exit()", Opts::default());
    assert_eq!((o.code.as_str(), o.exit), ("exit", 0));
    assert_eq!(code_of("exit(\"x\")"), "E0403");
}

#[test]
fn stdin_and_stderr() {
    let o = exec(
        "eprint(\"warn\", 1)\nprint(read_line(), read_line(), read_line())",
        Opts {
            stdin: "a\nb\n",
            ..Opts::default()
        },
    );
    assert_eq!(o.code, "");
    assert_eq!(o.out, "Ok(\"a\") Ok(\"b\") Ok(nil)\n");
    assert_eq!(o.err, "warn 1\n");
    let o = exec(
        "print(read_lines())",
        Opts {
            stdin: "x\ny",
            ..Opts::default()
        },
    );
    assert_eq!(o.out, "Ok([\"x\", \"y\"])\n");
}

// --- call-heavy loops ------------------------------------------------------

/// Loops that make many calls give the right result.
#[test]
fn call_heavy_loops_run() {
    for n in [100, 200] {
        let src = format!(
            "fn add(a, b) = a + b\nfn spin(n, acc) = n == 0 then acc else spin(n - 1, add(acc, 0))\nprint(spin({n}, 1))"
        );
        assert_eq!(run_src(&src).0, "1\n");
        let src = format!(
            "fn spin(n, acc) = n == 0 then acc else spin(n - 1, acc + len(\"ab\") - 2)\nprint(spin({n}, 1))"
        );
        assert_eq!(run_src(&src).0, "1\n");
        let src = format!("print(fold(0..{n}, 0, (a, _x) => a))");
        assert_eq!(run_src(&src).0, "0\n");
    }
}

// --- more cases -------------------------------------------------------------

#[test]
fn natives_call_closures_through_the_task() {
    testutil::on_big_stack(|| {
        /// `f(f(1))`, continuing in the task's run loop (`cont.rs`).
        struct Twice(Value, bool);
        impl crate::vm::Cont for Twice {
            fn resume(&mut self, t: &mut Task, v: Value) -> Result<crate::vm::Next, Stop> {
                if self.1 {
                    return Ok(crate::vm::Next::Done(v));
                }
                self.1 = true;
                t.push_arg(v);
                Ok(crate::vm::Next::Call(self.0.clone(), 1))
            }
        }
        fn twice(t: &mut Task, args: crate::value::Args) -> Result<crate::vm::Flow, Stop> {
            t.push_arg(Value::Int(1));
            let f = args[0].clone();
            Ok(crate::vm::Flow::Call(
                Box::new(Twice(f.clone(), false)),
                f,
                1,
            ))
        }
        let n = Native::leak_step(
            "twice",
            vec![NativeParam {
                name: "f",
                default: None,
                rest: false,
            }],
            twice,
        );
        let mut p = Prelude::default();
        p.0.insert("twice".into(), Value::Native(n));
        let o = testutil::exec_with_prelude("twice(x => x + 10)", p);
        assert_eq!(o.value.as_deref(), Some("21"));
    });
}

/// An error inside a function from an embedded extras file points at the
/// nearest user frame, and the extras frame is left out of the call path
/// (VA19).
#[test]
fn errors_in_builtin_code_point_at_the_user_call() {
    let o = exec(
        "fn helper(x) = x / 0\nprint(helper(1) + 0)",
        Opts {
            patch: |p| p.protos[0].builtin = true,
            ..Opts::default()
        },
    );
    let e = o.error.expect("an error");
    assert_eq!((e.code.as_str(), e.line, e.col), ("E0502", 2, 13));
    let path: Vec<(&str, u32)> = e.path.iter().map(|c| (c.func.as_str(), c.line)).collect();
    assert_eq!(path, [("<main>", 2)]);
}

/// When every frame is a built-in function's, the call path is empty and
/// the report has no "call path" header (VA19).
#[test]
fn an_all_builtin_path_prints_no_header() {
    fn mark(p: &mut fung_front::bytecode::Proto) {
        p.builtin = true;
        p.protos.iter_mut().for_each(mark);
    }
    let o = exec(
        "fn helper(x) = x / 0\nprint(helper(1) + 0)",
        Opts {
            patch: mark,
            ..Opts::default()
        },
    );
    let e = o.error.expect("an error");
    assert_eq!(e.code, "E0502");
    assert!(e.path.is_empty(), "{:?}", e.path);
    assert!(o.err.starts_with("error[E0502]"), "{}", o.err);
    assert!(!o.err.contains("call path"), "{}", o.err);
}

#[test]
fn closures_print_and_compare() {
    let (out, _, _) = run_src("fn add(a) = a\nf = add\nprint(f, x => x, f != nil)");
    assert_eq!(out, "<fn add> <fn> true\n");
    // D62: functions have no equality.
    let (_, err, _) = run_src("fn add(a) = a\nf = add\nprint(f == add)");
    assert!(
        err.starts_with("error[E0501]: cannot compare fn and fn"),
        "{err}"
    );
    // The answer does not depend on whether the two sides share their
    // storage (VA43): a list compared with itself still meets its
    // function, in a small value and in one the nodes remember.
    for src in [
        "f = x => x\na = [f]\nb = a\nprint(a == b)",
        "f = x => x\na = [f]\nb = a\nprint(a in [b])",
        "f = x => x\na = {k: (1, Ok(f))}\nb = a\nprint([a] == [b])",
        "a = map(0..5000, i => i) + [print]\nb = a\nprint(a == b)",
        "a = map(0..5000, i => [i, len])\nb = a\nprint(a != b)",
        // D116: whatever else the two sides hold and wherever the
        // functions stand (VA43's order of items and of map keys).
        "f = x => x\nprint([1, f] == [2, f])",
        "f = x => x\nprint({a: 1, cb: f} == {a: 2, cb: f})",
        "f = x => x\nprint({cb: f, a: 1} == {cb: f, a: 2})",
        "f = x => x\nprint([f] != [1, 2, f])",
    ] {
        let (out, err, _) = run_src(src);
        assert!(
            err.starts_with("error[E0501]: cannot compare"),
            "{src}: {out}{err}"
        );
        assert!(err.contains("no equality"), "{src}: {err}");
    }
    // A function on one side only is a difference (D91).
    let (out, err, _) = run_src("f = x => x\nprint([1, f] == [1, 2], f in [1, 2], [f] == [nil])");
    assert_eq!(out, "false false false\n", "{err}");
    // A shared value that holds no function is still equal to itself.
    let (out, err, _) = run_src(
        "a = map(0..5000, i => [i, \"x\"])\nb = a\nc = {k: (1, Ok(nil))}\nd = c\nprint(a == b, c == d)",
    );
    assert_eq!(out, "true true\n", "{err}");
}

#[test]
fn unknown_builtins_link_to_an_internal_error() {
    // The booted prelude has every spec name; a prelude without `upper`
    // links it to a placeholder that reports E0801.
    let o = testutil::on_big_stack(|| {
        let mut p = Prelude::default();
        for (name, v) in crate::prelude::core() {
            if name != "upper" {
                p.0.insert(name.to_string(), v);
            }
        }
        testutil::exec_with_prelude("upper(\"a\")", p)
    });
    assert_eq!(o.code, "E0801");
    assert_eq!(
        o.error.unwrap().headline,
        "error[E0801]: internal error: builtin `upper` is not available"
    );
}

/// A long chain of closures, each capturing the previous one, frees
/// without recursion (like lists, maps, tuples and tags).
#[test]
fn dropping_a_deep_closure_chain_does_not_overflow() {
    let h = std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(|| {
            let proto = link(&fung_front::bytecode::Proto::default(), &Prelude::default());
            let globals: crate::value::Globals = Rc::new(std::cell::RefCell::new(Vec::new()));
            let mut v = Value::Nil;
            for _ in 0..1_000_000 {
                v = Value::Closure(Rc::new(Closure {
                    code: proto.code.clone(),
                    proto: proto.clone(),
                    captured: vec![v].into(),
                    globals: globals.clone(),
                }));
            }
            assert_eq!(format(&v), "<fn>");
            drop(v);
        })
        .unwrap();
    h.join().unwrap();
}

/// A deep chain built by the program itself (each closure captures the
/// last) is freed when the run ends.
#[test]
fn a_program_building_a_deep_closure_chain_ends_cleanly() {
    let (out, err, code) = run_src(
        "fn wrap(f) = x => f(x)\nfn build(n, f) = n == 0 then f else build(n - 1, wrap(f))\ng = build(100000, x => x + 1)\nprint(g(1))",
    );
    assert_eq!((out.as_str(), err.as_str(), code), ("2\n", "", 0));
}

/// An empty slot costs nothing: `Option<Value>` uses a spare tag.
#[test]
fn stack_slots_are_16_bytes() {
    assert_eq!(std::mem::size_of::<Option<Value>>(), 16);
}

/// The count of elided tail calls keeps growing past u32::MAX instead of
/// overflowing.
#[test]
fn elided_tail_calls_count_past_u32() {
    use fung_front::diag;
    use fung_front::loader::{Loader, Stage};
    let (b, errs) = Loader::default().compile("-e", "print(1 / 0)", Stage::All);
    assert!(errs.is_empty());
    let main = link(b.proto.as_ref().unwrap(), &Prelude::default());
    let rt = Runtime::new(
        Box::new(std::io::sink()),
        Box::new(std::io::sink()),
        Box::new(std::io::Cursor::new(Vec::new())),
        b.sources,
    );
    let mut t = rt.task();
    let globals = t.world.new_globals(0);
    let mut f = super::frame::Frame::body(main, globals, 0);
    f.ip = 1;
    f.tail_calls = u32::MAX as u64;
    t.fiber.frames.push(f);
    t.fiber.frames[0].tail_calls += 1;
    let path = t.call_path();
    drop(t); // releases the world borrow for `rt.sources()`
    assert_eq!(path[0].elided, 4_294_967_296);
    let d = fung_front::diag::error("E0502", crate::vm::no_pos(), vec![]).with_call_path(path);
    let text = rt.with_sources(|s| diag::format(&d, s));
    assert!(
        text.contains("   -e:1       <entry> (4294967296 tail calls elided)\n"),
        "{text}"
    );
}

// --- one thread (R5) ---

/// fung values live on the thread that runs them: the runtime cannot
/// move to another thread (`!Send`), while what other threads hold of it
/// (`Shared`, the stop handle) can be shared. Compile-time checks: if
/// the runtime were `Send`, both impls would apply and the item path
/// would be ambiguous, which does not compile.
#[test]
fn the_runtime_stays_on_its_thread() {
    trait AmbiguousIfSend<A> {
        fn some_item() {}
    }
    impl<T: ?Sized> AmbiguousIfSend<()> for T {}
    impl<T: ?Sized + Send> AmbiguousIfSend<u8> for T {}
    let _ = <crate::vm::Runtime as AmbiguousIfSend<_>>::some_item;
    fn shared<T: Send + Sync>() {}
    shared::<crate::vm::world::Shared>();
    shared::<crate::vm::StopHandle>();
}

/// Test blocks are reached by index; their functions stay in the runtime.
#[test]
fn test_blocks_are_named_and_called_by_index() {
    testutil::on_big_stack(|| {
        let s = testutil::setup(
            "test \"ok\": assert 1 == 1 ;;\ntest \"bad\": assert 1 == 2 ;;",
            Opts {
                tests: true,
                ..Opts::default()
            },
        )
        .unwrap_or_else(|(e, _)| panic!("{e}"));
        s.rt.run().expect("the body runs");
        let names: Vec<String> = s.rt.tests();
        assert_eq!(names, ["ok", "bad"]);
        assert!(s.rt.call_test(0).is_ok());
        assert!(matches!(s.rt.call_test(1), Err(Stop::Error(d)) if d.code == "E0807"));
        assert!(matches!(s.rt.call_test(2), Err(Stop::Error(d)) if d.code == "E0801"));
    });
}

/// The public flow the command uses: boot, load, run. No
/// step hands a fung value back; everything that comes out is plain data.
#[test]
fn the_public_api_runs_a_program_without_handing_out_values() {
    testutil::on_big_stack(|| {
        use fung_front::loader::{Loader, Stage};
        let (b, errs) = Loader::default().compile(
            "-e",
            "with(() => nil, fn(_): print(args); 1 / 0 ;;, _ => print(\"bye\"))",
            Stage::All,
        );
        assert!(errs.is_empty());
        let out = testutil::Buf::default();
        let rt = Runtime::new(
            Box::new(out.clone()),
            Box::new(std::io::sink()),
            Box::new(std::io::Cursor::new(Vec::new())),
            b.sources,
        );
        rt.boot(vec!["a".into()]).expect("boot");
        rt.load(b.proto.as_ref().expect("an entry"), &b.modules);
        let r: Result<(), Stop> = rt.run();
        let Err(Stop::Error(d)) = r else {
            panic!("{r:?}")
        };
        let text = rt.with_sources(|s| fung_front::diag::format(&d, s));
        assert!(text.contains("-e:1"), "{text}");
        assert_eq!(rt.exit_code(), None);
        assert_eq!(rt.args(), [b"a".to_vec()]);
        assert_eq!(out.text(), "[\"a\"]\nbye\n");
    });
}

/// A call's last read of a local moves the value (03, P9): the callee
/// holds a map or list alone, so `put` and `+` change it in place. A read
/// that is not the last copies. The values are built at run time (`one`):
/// a literal of constants is one constant the program holds (L5), so it is
/// never held alone.
#[test]
fn the_last_read_of_a_local_hands_the_value_over() {
    let o = crate::testutil::exec(
        "one = 1\n\
         fn last_read(m) = sole(m)\n\
         fn not_last(m):\n  s = sole(m)\n  [s, len(m)]\n;;\n\
         print(last_read({a: one}), last_read(map(0..2, i => i)), not_last({a: one}), sole({a: 1}))\n\
         print(fold(0..3, {z: one}, (m, i) => put(sole(m), \"k${i}\", i)))\n\
         fn go(acc, n) = n == 0 then acc else go(sole(acc) + [n], n - 1)\n\
         print(go([one], 3))",
        crate::testutil::Opts {
            extra: crate::testutil::probes,
            ..crate::testutil::Opts::default()
        },
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        (
            "{a: 1} [0, 1] [Err(\"shared\"), 1] Err(\"shared\")\n{z: 1, k0: 0, k1: 1, k2: 2}\n[1, 3, 2, 1]\n",
            ""
        )
    );
}

/// FE61: a task's error says where the task was spawned: its call path
/// starts in the task.
#[test]
fn a_task_error_names_its_spawn() {
    let (_, err, code) = run_src(
        "fn boom(n) = 1 / n\nfn main():\n  r = scope(fn(s):\n    t = spawn(s, () => boom(0))\n    await(t)\n  ;;)\n  print(r)\n;;\nmain()",
    );
    assert!(
        err.contains("\n  note: the task was spawned at -e:4\n"),
        "{err}"
    );
    assert_eq!(code, 1);
}

/// VA39: the runtime's wording of the third review: one family for an
/// index, `len(xs)` for `xs.len`, `iterate` for a `for` value, and the
/// help to unwrap a result used as a value.
#[test]
fn runtime_wording_of_the_third_review() {
    let first = |src: &str| {
        let (_, err, _) = run_src(src);
        let help = err
            .lines()
            .find_map(|l| l.strip_prefix(" help: "))
            .unwrap_or("")
            .to_string();
        (err.lines().next().unwrap_or("").to_string(), help)
    };
    for (src, head, help) in [
        ("print(5[0])", "error[E0509]: cannot index int with int", ""),
        (
            "print(nil[0])",
            "error[E0509]: cannot index nil with int",
            "",
        ),
        (
            "print([1][\"a\"])",
            "error[E0509]: cannot index list with str",
            "",
        ),
        (
            "print((1, 2).a)",
            "error[E0509]: cannot index tuple with str",
            "a tuple has no fields: only maps and modules do",
        ),
        (
            "print([1].len)",
            "error[E0509]: cannot index list with str",
            "a list has no fields; to call `len` on it, write `len(…)`",
        ),
        (
            "print(\"abc\".len)",
            "error[E0509]: cannot index str with str",
            "a str has no fields; to call `len` on it, write `len(…)`",
        ),
        (
            "print([1].nope)",
            "error[E0509]: cannot index list with str",
            "a list has no fields: only maps and modules do",
        ),
        (
            "print(for x in 5: x ;;)",
            "error[E0506]: cannot iterate over int",
            "",
        ),
        // a field of a value that has none is the index's E0509, as
        // `x["a"]` is (`m.name` is `m["name"]`), with a help that says
        // it was a field; and a slice's end (FE82)
        (
            "x = nil\nprint(x.a)",
            "error[E0509]: cannot index nil with str",
            "a nil has no fields: only maps and modules do",
        ),
        (
            "xs = [1, 2]\ny = \"a\"\nprint(xs[y..2])",
            "error[E0509]: cannot index list with str",
            "",
        ),
        (
            "xs = [1, 2]\ny = 1.5\nprint(xs[..y])",
            "error[E0509]: cannot index list with float",
            "",
        ),
        (
            "print(\"${atom(1)}\")",
            "error[E0519]: an atom handle in string interpolation: it is not put into text silently",
            "unwrap a result with `!` or `?` and give `nil` a default with `??` first; to show any value as `print` does, write `str(x)`",
        ),
        (
            "c = channel(1.0)",
            "error[E0403]: argument `n` must be int, got float",
            "",
        ),
        (
            "print(parse_int(\"12\") + 1)",
            "error[E0501]: cannot add Ok and int",
            "unwrap it with `!` or `?` first",
        ),
        (
            "print(1 < parse_int(\"12\"))",
            "error[E0501]: cannot compare int and Ok",
            "unwrap it with `!` or `?` first",
        ),
        (
            "print(-parse_int(\"1\"))",
            "error[E0506]: cannot negate Ok",
            "unwrap it with `!` or `?` first",
        ),
        (
            "print(1 - \"a\")",
            "error[E0501]: cannot subtract int and str",
            "",
        ),
    ] {
        assert_eq!(first(src), (head.to_string(), help.to_string()), "{src}");
    }
}

/// A function that captures nothing is made once per module and reused
/// (VM20, D62: a program cannot tell); one that captures is made anew.
#[test]
fn closures_that_capture_nothing_are_made_once() {
    fn same(_: &mut Task, a: crate::value::Args) -> Result<Value, Stop> {
        let (Value::Closure(f), Value::Closure(g)) = (&a[0], &a[1]) else {
            panic!("two closures")
        };
        Ok(Value::Bool(Rc::ptr_eq(f, g)))
    }
    let two = |name: &'static str| NativeParam {
        name,
        default: None,
        rest: false,
    };
    let n = Native::leak("same", vec![two("f"), two("g")], same);
    for (src, want) in [
        ("fn mk() = x => x + 1\nsame(mk(), mk())", "true"),
        ("fn mk(y) = x => x + y\nsame(mk(1), mk(1))", "false"),
        ("fn mk() = fn(x): x ;;\nsame(mk(), mk())", "true"),
    ] {
        let mut p = Prelude::default();
        p.0.insert("same".into(), Value::Native(n));
        let o = testutil::exec_with_prelude(src, p);
        assert_eq!(o.value.as_deref(), Some(want), "{src}");
    }
}

/// Values of different types are not equal: `==`, `!=` and `in` say so
/// with `false`, like any other pair of different values (D91; E0517 is
/// retired). Functions alone have no equality (D62). An order is another
/// matter: there is none between a string and an int, so `<` and the
/// functions that order their elements still fail, and they look at
/// every pair, whatever the values (D87).
#[test]
fn values_of_different_types_are_not_equal() {
    let (out, err, _) = run_src(
        "print(\"8080\" == 8080, 'ok == \"ok\", 1 != \"1\", Ok(1) == 1, [1] == (1,))\n\
         print([1, 'a] == [1, \"a\"], [1, \"a\"] == [2, 3], {a: 1} == {a: \"1\"})\n\
         print((1, 2) == (1, 2, 3), 1 == 1.0, nil == 1, 0..3 == [0, 1, 2])\n\
         print(1 in [1, \"a\"], \"a\" in [1, \"a\"], \"x\" in [1, 2], \"1\" in 0..3)\n\
         print(unique([1, \"1\", 1, 'a, \"a\"]), contains([1, 2], \"1\"))",
    );
    assert_eq!(
        out,
        "false false true false false\n\
         false false false\n\
         false true false false\n\
         true true false false\n\
         [1, \"1\", 'a, \"a\"] false\n",
        "{err}"
    );
    for (src, want) in [
        // functions have no equality
        ("f = x => x\nprint([f] == [f])", "E0501"),
        ("f = x => x\nprint(f in [f])", "E0501"),
        // no order between values of different types, wherever they stand
        ("print(\"a\" < 1)", "E0501"),
        ("print([1, \"a\"] < [2, 1])", "E0501"),
        ("print((1, 2) < (1, 2, 3))", "E0501"),
        ("print(sort([[1, \"a\"], [2, 2]]))", "E0501"),
        ("print(sort([3, 1, \"a\"]))", "E0501"),
        ("print(sort_by([1, 2], x => [x, print]))", "E0501"),
        ("print(max([[1, nil], [2, nil]]))", "E0501"),
        (
            "print([1, \"a\"] < [2, \"b\"], sort([[2, \"b\"], [1, \"a\"]]))",
            "",
        ),
    ] {
        let (out, err, _) = run_src(src);
        let got = err
            .split_once("error[")
            .and_then(|(_, rest)| rest.split(']').next())
            .unwrap_or("");
        assert_eq!(got, want, "{src}: {out}{err}");
    }
}

/// The functions that order their elements fold every one into one
/// shape before they start (VA60): lists of different lengths line up
/// by position, so whether they order cannot depend on the order of the
/// elements, nor on the values.
#[test]
fn ordering_functions_unify_the_shapes_of_all_elements() {
    let rows = ["[]", "[1, \"a\"]", "[2]", "[3, 0]"];
    let mut orders = Vec::new();
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let ix = [a, b, c, d];
                    if (0..4).all(|i| ix.contains(&i)) {
                        orders.push(ix.map(|i| rows[i]).join(", "));
                    }
                }
            }
        }
    }
    assert_eq!(orders.len(), 24);
    for xs in &orders {
        for call in [
            "sort([{xs}])",
            "max([{xs}])",
            "min([{xs}])",
            "sort_by([{xs}], x => x)",
        ] {
            let src = format!("print({})", call.replace("{xs}", xs));
            let (out, err, _) = run_src(&src);
            assert!(err.contains("error[E0501]"), "{src}: {out}{err}");
        }
    }
    for (src, want) in [
        // lists of other lengths whose positions agree order
        (
            "print(sort([[2, 3], [], [1], [1, 4, 5]]))",
            "[[], [1], [1, 4, 5], [2, 3]]\n",
        ),
        (
            "print(max([[1.5, \"b\"], [2], [], [1, \"a\", 3s]]))",
            "[2]\n",
        ),
        ("print(min([(1, [2]), (0, [])]))", "(0, [])\n"),
        // a position no other element has must still have an order
        ("print(sort([[1], [2, true]]))", ""),
        ("print(min([[1], [2, nil]]))", ""),
        ("print(sort([(1, 2), (1, 2, 3)]))", ""),
        ("print(sort([[1, 2], [3, (1, 2)]]))", ""),
    ] {
        let (out, err, _) = run_src(src);
        assert_eq!(out, want, "{src}: {err}");
        if want.is_empty() {
            assert!(err.contains("error[E0501]"), "{src}: {err}");
        }
    }
    // the error names the first conflict, in element order
    let (_, err, _) = run_src("print(sort([[], [3, 0], [1, \"a\"], [2]]))");
    assert!(err.contains("cannot compare int and str"), "{err}");
    let (_, err, _) = run_src("print(sort([true, 1]))");
    assert!(err.contains("cannot compare bool and int"), "{err}");
}

/// A call on the right of `and`/`or` in tail position is a tail call
/// (D97): the frame takes the bool check along, as it takes a return
/// type's (D44), so the two operators recurse as deep as `then … else`.
#[test]
fn the_right_side_of_and_or_is_a_tail_call() {
    let (out, err, _) = run_src(
        "fn allpos(i) = i == 0 or (i > 0 and allpos(i - 1))\n\
         fn even(n) = n == 0 or odd(n - 1)\n\
         fn odd(n) = n != 0 and even(n - 1)\n\
         print(allpos(300000), even(300000), odd(300000))",
    );
    assert_eq!(out, "true true false\n", "{err}");
    // The check still runs, where the operator stands, however deep the
    // value comes from.
    let (_, err, _) = run_src(
        "fn count(i) = i == 0 then 5 else count(i - 1)\n\
         fn bad(i) = i < 0 or count(i)\n\
         print(bad(1000))",
    );
    assert!(
        err.starts_with("error[E0505]: expected bool, got int\n  --> -e:2:27"),
        "{err}"
    );
    // A loop that takes the same check again each time round keeps it
    // once (VM52), and one that takes two by turns keeps both: either
    // way the check runs where its operator stands.
    for (src, at) in [
        (
            "fn spin(n) = n == 0 then 5 else (n < 0 or spin(n - 1))\nprint(spin(100000))",
            "-e:1:47",
        ),
        (
            "fn a(n) = n == 0 then 5 else (n < 0 or b(n - 1))\nfn b(n) = n < 0 or a(n - 1)\nprint(a(100000))",
            "-e:2:21",
        ),
    ] {
        let (_, err, _) = run_src(src);
        assert!(
            err.starts_with(&format!("error[E0505]: expected bool, got int\n  --> {at}")),
            "{src}: {err}"
        );
    }
    // With a return type too: both checks go along.
    let (out, err, _) = run_src(
        "fn down(i) -> bool = i == 0 or down2(i - 1)\n\
         fn down2(i) -> bool = i == 0 or down(i - 1)\n\
         print(down(200001))",
    );
    assert_eq!(out, "true\n", "{err}");
    // The bool check runs before the return type's, as when the value is
    // named first (VM49): a non-bool right side is E0505 at its call.
    for (src, at) in [
        (
            "fn not_bool(x) = x\nfn h(x) -> bool = x > 0 and not_bool(x)\nprint(h(2))",
            "-e:2:37",
        ),
        (
            "fn not_bool(x) = x\nfn h(x) -> bool:\n  r = x > 0 and not_bool(x)\n  r\n;;\nprint(h(2))",
            "-e:3:25",
        ),
    ] {
        let (_, err, _) = run_src(src);
        assert!(
            err.starts_with(&format!("error[E0505]: expected bool, got int\n  --> {at}")),
            "{src}: {err}"
        );
    }
}

/// `index_of` finds what `in` finds (D107): bytes, so a match that
/// starts inside a character is found too, at that character's index.
#[test]
fn index_of_finds_what_in_finds() {
    let (out, err, _) = run_src(
        "print(\"\\xa9\" in \"é\", index_of(\"é\", \"\\xa9\"), index_of(\"aé\", \"\\xa9\"))\n\
         print(index_of(\"참치김밥\", \"김\"), index_of(\"abc\", \"x\"), \"x\" in \"abc\")",
    );
    assert_eq!(out, "true 0 1\n2 nil false\n", "{err}");
}

/// `index_of` searches a piece (`STEP_SCAN` bytes) at a time; a match
/// inside a character that straddles a piece boundary, or past a stretch
/// of invalid bytes, is still found where `in` finds it (VA62).
#[test]
fn index_of_finds_across_pieces() {
    let (out, err, _) = run_src(
        "for k in [65535, 131071, 262143, 131070, 131072]:\n\
           s = repeat(\"a\", k) + \"\u{e9}\" + \"zz\"\n\
           print(\"\\xa9\" in s, index_of(s, \"\\xa9\") == k, index_of(s, \"\\xa9z\") == k, index_of(s, \"zz\") == k + 1)\n\
         ;;\n\
         t = repeat(\"\\xa9\", 131071) + \"\u{ac00}\" + repeat(\"\u{e9}\", 70000) + \"!\"\n\
         print(index_of(t, \"\\x80\") == 131071, index_of(t, \"!\") == 201072, index_of(t, \"?\"))",
    );
    assert_eq!(
        out,
        "true true true true\n".repeat(5) + "true true nil\n",
        "{err}"
    );
}

/// Small fixes of the fourth review's value layer.
#[test]
fn small_value_fixes() {
    // VA57: a pad that fills exactly to the string limit is no error.
    let (out, err, _) = run_src(
        "print(len(pad_left(\"\", 100000000)), len(pad_right(\"x\", 100000000, \"abc\")), pad_left(\"7\", 4, \"ab\"), pad_right(\"7\", 5, \"ab\"))",
    );
    assert_eq!(out, "100000000 100000000 aba7 7abab\n", "{err}");
    // VA45: a shown string reads back as source: `$` before a name or
    // `{` is escaped, and `\u{…}` is written in lowercase.
    let (out, err, _) = run_src("print([\"a\\$b\", \"\\${a}\", \"\\$_x\", \"\\$5\", \"\\u{1b}\"])");
    assert_eq!(
        out, "[\"a\\$b\", \"\\${a}\", \"\\$_x\", \"$5\", \"\\u{1b}\"]\n",
        "{err}"
    );
    // VA42, D135: a duration and a float are computed exactly and
    // rounded once to the nanosecond (halves away from zero), so a
    // duration a float cannot hold loses nothing and is no error.
    let (out, err, _) =
        run_src("x = 2600h + 0.000001ms\nprint(x * 1.0, x / 2.0, 1.0 * x, x * 0.5, -x / 2.0)");
    assert_eq!(
        out, "2600h0.000001ms 1300h0.000001ms 2600h0.000001ms 1300h0.000001ms -1300h0.000001ms\n",
        "{err}"
    );
    let (out, err, _) = run_src("print(1h * 1.5, 3s / 2.0, (2600h + 0.000001ms) * 2)");
    assert_eq!(out, "1h30m 1s500ms 5200h0.000002ms\n", "{err}");
    // VA58: an empty command name is the code's mistake, like an empty
    // command list.
    let (_, err, _) = run_src("print(exec([\"\"]))");
    assert!(err.starts_with("error[E0403]"), "{err}"); // FE74: a map pattern takes the keys a map literal takes.
    let (out, err, _) = run_src(
        "m = {\"\\xff\": 1, a: 2}\n{\"\\xff\": x, a: y} = m\nprint(x, y, match m: {\"\\xfe\": _, ...} => 0; {\"\\xff\": v, ...} => v ;;)",
    );
    assert_eq!(out, "1 2 1\n", "{err}");
}

/// FE65: a partial application of a builtin and a call of a local fn are
/// refused before anything runs.
#[test]
fn partials_and_local_fns_are_checked_before_running() {
    for (src, code) in [
        ("print(\"ran\")\ng = len(_, 1, 2)\nprint(g)", "E0402"),
        (
            "print(\"ran\")\nfn f(xs) = map(xs, str(_, 2))\nprint(f([1]))",
            "E0402",
        ),
        (
            "fn f():\n  fn g(a) = a\n  print(\"ran\")\n  g(1, 2)\n;;\nprint(f())",
            "E0402",
        ),
    ] {
        let (out, err, _) = run_src(src);
        assert_eq!(out, "", "{src}");
        assert!(err.starts_with(&format!("error[{code}]")), "{src}: {err}");
    }
    let (out, err, _) =
        run_src("fn add(a, b) = a + b\ng = add(1, _)\nprint(g(2), map([1, 2], add(_, b: 10)))");
    assert_eq!(out, "3 [11, 12]\n", "{err}");
}

/// VA44: two values that share their parts many times over are compared
/// and hashed once per part, not once per path.
#[test]
fn shared_parts_are_walked_once() {
    let (out, err, _) = run_src(
        "x = fold(0..200, [1], (acc, _i) => [acc, acc])\n\
         y = fold(0..200, [1], (acc, _i) => [acc, acc])\n\
         z = fold(0..200, [2], (acc, _i) => [acc, acc])\n\
         print(x == y, x == z, x < z, x <= y, len(unique([x, y, z])), x in [z, y])\n\
         t = fold(0..200, (1,), (acc, _i) => Pair(acc, {a: acc}))\n\
         u = fold(0..200, (1,), (acc, _i) => Pair(acc, {a: acc}))\n\
         print(t == u, len(unique([t, u])), match t: Pair(_, _) => 1 ;;)",
    );
    assert_eq!(out, "true false true true 2 true\ntrue 1 1\n", "{err}");
    // What is equal hashes alike, shared or not.
    let (out, err, _) = run_src(
        "a = [1, 2]\nprint(len(unique([[a, a], [[1, 2], [1, 2]], ([1, 2], a), ([1, 2], [1, 2])])))",
    );
    assert_eq!(out, "2\n", "{err}");
}

/// VM47: a top-level fn holds its module's global table and is held by
/// it; the runtime empties the tables as it ends, so the ring is freed.
#[test]
fn the_globals_ring_is_freed_with_the_runtime() {
    let Ok(s) = crate::testutil::setup(
        "big = repeat(\"x\", 1000)\nfn f() = big\nfn g() = f\nprint(len(g()()))",
        crate::testutil::Opts::default(),
    ) else {
        panic!("compile");
    };
    assert!(s.rt.run().is_ok());
    let tables: Vec<_> =
        s.rt.world()
            .globals
            .iter()
            .map(std::rc::Rc::downgrade)
            .collect();
    assert!(!tables.is_empty());
    drop(s);
    assert!(tables.iter().all(|t| t.upgrade().is_none()));
}

/// What the syntax says a value is stops the program before it runs,
/// with the error the run gives the same value (D99, FE81): each form
/// written with literals is a compile error whose head line and help are
/// those of the run, where the values come through a function.
#[test]
fn literal_type_errors_are_the_runs() {
    // The literals of each form, then the form with `@0`, `@1` for them.
    let cases: &[(&[&str], &str)] = &[
        (&["1s"], "@0(2)"),
        (&["\"a${1}\""], "@0(2)"),
        (&["[1]"], "@0(2)"),
        (&["{a: 1}"], "@0(2)"),
        (&["(1, 2)"], "@0(2)"),
        (&["Red(1)"], "@0(2)"),
        (&["(1..3)"], "@0(2)"),
        (&["1s"], "not @0"),
        (&["\"a${1}\""], "not @0"),
        (&["[1]"], "if @0: 1 else: 2 ;;"),
        (&["(x => x)"], "@0 then 1 else 2"),
        (&["Ok(1)"], "true and @0"),
        (&["\"a\""], "-@0"),
        (&["true"], "-@0"),
        (&["nil"], "-@0"),
        (&["[1]"], "-@0"),
        (&["Ok(1)"], "-@0"),
        (&["1s", "\"a\""], "@0 + @1"),
        (&["\"a\"", "1s"], "@0 + @1"),
        (&["[1]", "[1]"], "@0 - @1"),
        (&["(x => x)", "1"], "@0 + @1"),
        (&["\"a\"", "1s"], "@0 * @1"),
        (&["[1]", "1s"], "@0 * @1"),
        (&["1s", "1s"], "@0 * @1"),
        (&["1s", "2"], "@0 // @1"),
        (&["1s", "2"], "@0 % @1"),
        (&["1s", "\"a\""], "@0 / @1"),
        (&["\"a\"", "1s"], "@0 / @1"),
        (&["{a: 1}", "{b: 2}"], "@0 + @1"),
        (&["Ok(1)", "1"], "@0 + @1"),
        (&["\"a${1}\"", "1"], "@0 + @1"),
        (&["1", "[1]"], "@0 < @1"),
        (&["[1]", "(1,)"], "@0 < @1"),
        (&["1s", "1"], "@0 < @1"),
        (&["1s", "1"], "@0 - @1"),
        (&["[1]", "2"], "@0 ** @1"),
        (&["(1..2)", "[1]"], "@0 + @1"),
    ];
    // The head line and the help line of an error report.
    let report = |src: &str| -> (String, String) {
        let (_, err, _) = run_src(src);
        let head = err.lines().next().unwrap_or("").to_string();
        let help = err
            .lines()
            .find(|l| l.trim_start().starts_with("help:"))
            .unwrap_or("")
            .trim()
            .to_string();
        (head, help)
    };
    for (lits, form) in cases {
        let fill = |f: &dyn Fn(&str) -> String| {
            let mut s = form.to_string();
            for (i, l) in lits.iter().enumerate() {
                s = s.replace(&format!("@{i}"), &f(l));
            }
            s
        };
        let compiled = format!("x = {}\nprint(x)", fill(&|l| l.to_string()));
        let ran = format!(
            "fn id(v) = v\nx = {}\nprint(x)",
            fill(&|l| format!("id({l})"))
        );
        let want = report(&ran);
        assert!(want.0.starts_with("error[E0"), "{ran:?} runs: {want:?}");
        assert_eq!(report(&compiled), want, "{compiled:?}");
    }
}

/// D110: a module member bound to a name in any block is that member: its
/// calls are checked and a call of a pure one as a statement is E0234.
#[test]
fn module_members_bound_to_names_are_checked() {
    for (src, code) in [
        (
            "import path\nfn f():\n  j = path.join\n  j(\"a\", \"b\")\n  1\n;;\nprint(\"ran\")\nprint(f())",
            "E0234",
        ),
        (
            "import fs\nfn f():\n  r = fs.read\n  r()\n;;\nprint(\"ran\")\nprint(f())",
            "E0407",
        ),
        (
            "import fs\nr = fs.read\nfn f() = r(\"a\", \"b\")\nprint(\"ran\")\nprint(f())",
            "E0402",
        ),
        (
            "s = sort\nfn f():\n  s([2, 1])\n  1\n;;\nprint(\"ran\")\nprint(f())",
            "E0234",
        ),
    ] {
        let (out, err, _) = run_src(src);
        assert_eq!(out, "", "{src}");
        assert!(err.starts_with(&format!("error[{code}]")), "{src}: {err}");
    }
    // The error shows the name called, the help the function's own.
    let (_, err, _) = run_src("s = sort\ns([2, 1])\nprint(1)");
    assert!(
        err.starts_with("error[E0234]: the result of `s(…)` is computed and discarded")
            && err.contains("`sort` has no effect"),
        "{err}"
    );
}

/// D113: a literal that a type annotation refuses stops the program
/// before it runs, with the head line the run gives the same value
/// coming through a function.
#[test]
fn literal_annotation_errors_are_the_runs() {
    // The form with `@` for the value, the literals.
    let forms = [
        "fn f(a: int = @) = a\nprint(f())",
        "fn f() -> int = @\nprint(f())",
        "fn f() -> int:\n  return @\n;;\nprint(f())",
        "fn f(c) -> int | nil = c then 1 else @\nprint(f(false))",
        "fn g(a: int) = a\nprint(g(@))",
        "type id = num | Red\nfn g(a: id) = a\nprint(g(@))",
        "fn g(a, b: str) = [a, b]\nprint(g(1, b: @))",
        "fn g(a: sym) = a\nh = g\nprint(h(@))",
    ];
    let lits = [
        "\"x\"",
        "nil",
        "1.5",
        "Blue(1)",
        "Blue",
        "[1]",
        "(x => x)",
        "\"a${1}\"",
        "1s",
        "true",
    ];
    let report = |src: &str| -> String {
        let (_, err, _) = run_src(src);
        err.lines().next().unwrap_or("").to_string()
    };
    let mut seen = 0;
    for form in forms {
        for lit in lits {
            let ran = format!("fn id(v) = v\n{}", form.replace('@', &format!("id({lit})")));
            let want = report(&ran);
            let compiled = format!("fn id(v) = v\n_ = id\n{}", form.replace('@', lit));
            let (out, err, code) = run_src(&compiled);
            if want.is_empty() {
                assert_eq!(code, 0, "{compiled:?}: {err}");
                continue;
            }
            seen += 1;
            assert_eq!(out, "", "{compiled:?}");
            assert_eq!(code, 2, "{compiled:?}: {err}");
            assert_eq!(report(&compiled), want, "{compiled:?}");
        }
    }
    assert!(seen > 50, "{seen}");
}

/// D111: a statement after one that cannot complete is E0131, and only
/// that: the dead line hides nothing once it is gone (the unused name it
/// read is then E0239).
#[test]
fn a_dead_statement_is_the_one_error() {
    let src = "fn f(c):\n  x = c then 1 else 2\n  if c: return 1 else: return 2 ;;\n  print(\"never\", x)\n;;\nprint(f(true))";
    let (out, err, code) = run_src(src);
    assert_eq!((out.as_str(), code), ("", 2), "{err}");
    assert!(err.starts_with("error[E0131]"), "{err}");
    assert!(err.contains("-e:4:3"), "{err}");
    let (_, err, _) = run_src(&src.replace("  print(\"never\", x)\n", ""));
    assert!(
        err.starts_with("error[E0239]: unused variable `x`"),
        "{err}"
    );
}
