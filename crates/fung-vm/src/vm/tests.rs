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
        ("1 != true", "true"),
        ("2 >= 3", "false"),
        ("not true", "false"),
        ("true and false", "false"),
        ("false or true", "true"),
        ("1\n2", "2"),
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
        ("x = 1\ny = x + 1\nx = x * 10\nprint(x, y)", "10 2\n"),
        ("if true: z = 5; print(z) ;;", "5\n"),
        (
            "w = if 7 > 5: \"big\" else: \"small\" ;;\nprint(w)",
            "big\n",
        ),
        ("print(if false: 1 ;;)", "nil\n"),
        ("print(1 > 0 then \"a\" else \"b\")", "a\n"),
        (
            "x = 1\nprint(\"x is ${x}, next ${x + 1}\")",
            "x is 1, next 2\n",
        ),
        (
            "print(\"a\" + \"b\", \"a\" == \"a\", \"a\" < \"b\")",
            "ab true true\n",
        ),
        ("print('ok, 'ok == 'ok, 'ok == \"ok\")", "'ok true false\n"),
        (
            "print(1h30m, 500ms + 1.5s, 2 * 30s, 1m / 4, 30s < 1m)",
            "1h30m 2s 1m 15s true\n",
        ),
        ("print(nil, nil == nil, nil == 0)", "nil true false\n"),
        ("print(-9223372036854775808)", "-9223372036854775808\n"),
        ("print(9007199254740993 == 9007199254740992.0)", "false\n"),
        ("print(\"${\"q\"}\", \"${'s}\", \"${1.0}\")", "q 's 1.0\n"),
        ("x = 1\nx = 2\nprint(x)", "2\n"),
    ];
    for (src, want) in cases {
        let (out, err, code) = run_src(src);
        assert_eq!((out.as_str(), err.as_str(), code), (want, "", 0), "{src:?}");
    }
}

#[test]
fn block_value_and_statement_value() {
    let o = exec("x = 1", Opts::default());
    assert_eq!((o.code.as_str(), o.value.as_deref()), ("", Some("nil")));
    let o = exec("if true: a = 1; a + 1 ;;", Opts::default());
    assert_eq!(o.value.as_deref(), Some("2"));
}

#[test]
fn fail_raises_e0803_with_position() {
    let o = exec("print(1)\nfail(\"boom\", 2)", Opts::default());
    assert_eq!((o.code.as_str(), o.out.as_str()), ("E0803", "1\n"));
    let o = exec("x = 1\nfail(\"boom\")", Opts::default());
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
        ("fn f(): x = 1 ;;\nprint(f())", "nil\n"),
        (
            "fn add(a, b) = a + b\nprint(add, x => x)",
            "<fn add> <fn>\n",
        ),
    ]);
}

#[test]
fn closures_capture_by_value() {
    outputs(&[
        // copied when f was made, like a local
        ("x = 1\nf = () => x\nx = 2\nprint(f())", "1\n"),
        // a local: copied when f was made
        (
            "fn mk():\n  x = 1\n  f = () => x\n  x = 2\n  f()\n;;\nprint(mk())",
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
        // A return type annotation disables TCO (03), so this one overflows too.
        (
            "fn count(n, acc) -> int = n == 0 then acc else count(n - 1, acc + 1)\nprint(count(200000, 0))",
            "E0804",
        ),
    ]);
}

#[test]
fn type_annotations_are_checked() {
    outputs(&[
        (
            "fn f(n: int, s: str = \"x\") -> str = s\nprint(f(1), f(2, \"y\"))",
            "x y\n",
        ),
        ("fn f(n: num) = n\nprint(f(1), f(1.5))", "1 1.5\n"),
        ("fn f(v: str | nil) = v\nprint(f(nil), f(\"a\"))", "nil a\n"),
        ("fn f(v: any) = v\nprint(f('s))", "'s\n"),
    ]);
    codes(&[
        ("fn f(n: int) = n\nf(\"a\")", "E0403"),
        // default is checked too
        ("fn f(n: int, s: str = 1) = n\nf(1)", "E0403"),
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
            "fn f(n: int) = n\nf(\"a\")",
            "error[E0403]: argument `n` must be int, got str",
        ),
        (
            "fn f(v: str | nil) = v\nf(1)",
            "error[E0403]: argument `v` must be str | nil, got int",
        ),
        (
            "fn f() -> str = 1\nf()",
            "error[E0404]: return value must be str, got int",
        ),
        (
            "fn f(a, b) = a\nf(1, 2, 3)",
            "error[E0402]: f takes 2 argument(s), got 3",
        ),
        (
            "(x => x)(1, 2)",
            "error[E0402]: the function takes 1 argument(s), got 2",
        ),
        (
            "fn f(a) = a\nf(1, a: 2)",
            "error[E0405]: argument `a` given twice",
        ),
        (
            "fn f(a) = a\nf(b: 1)",
            "error[E0406]: f has no argument named `b`",
        ),
        ("fn f(a, b) = a\nf(1)", "error[E0407]: missing argument `b`"),
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
        run_src("print(fold(0..3, 0, (a, x) => a + fold(0..3, 0, (b, y) => b + 1 / (y - 2))))");
    assert_eq!(
        err,
        "error[E0502]: division by zero\n  --> -e:1:65\n  |\n1 | print(fold(0..3, 0, (a, x) => a + fold(0..3, 0, (b, y) => b + 1 / (y - 2))))\n  |                                                                 ^\n  |\n call path (innermost first):\n   -e:1       <lambda>\n   -e:1       <lambda>\n   -e:1       <entry>\n"
    );
    assert_eq!(code, 1);
    let (_, err, _) =
        run_src("fn f(x) = fold([1], 0, (a, b) => g(b))\nfn g(n) = n / 0\nprint(f(1))");
    assert!(
        err.ends_with(" call path (innermost first):\n   -e:2       g (1 tail calls elided)\n   -e:1       f\n   -e:3       <entry>\n"),
        "{err}"
    );
    // A calling-convention error of the lambda points at the native's call.
    let (_, err, _) = run_src("fold([1], 0, (a) => a)");
    assert_eq!(
        err,
        "error[E0402]: the function takes 1 argument(s), got 2\n  --> -e:1:5\n  |\n1 | fold([1], 0, (a) => a)\n  |     ^\n  |\n call path (innermost first):\n   -e:1       <entry>\n"
    );
    // exit inside a lambda ends the program with its code.
    let (out, err, code) = run_src("fold([1], 0, (a, b) => exit(3))\nprint(1)");
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
        (r#"pad("a", n: 1, n: 2)"#, "E0405"),
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
        with_pad("len(1, 2)").error.unwrap().headline,
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
        let src = format!("print(fold(0..{n}, 0, (a, x) => a))");
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
        fn twice(t: &mut Task, args: Vec<Value>) -> Result<crate::vm::Flow, Stop> {
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
/// nearest user frame, and the extras frame stays in the call path.
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
    assert_eq!(path, [("helper", 1), ("<main>", 2)]);
}

#[test]
fn closures_print_and_compare() {
    let (out, _, _) =
        run_src("fn add(a) = a\nf = add\nprint(f, x => x, f == add, add == (x => x))");
    assert_eq!(out, "<fn add> <fn> true false\n");
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
                    proto: proto.clone(),
                    captured: vec![v],
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
    let (b, errs) = Loader::default().compile("-e", "1 / 0", Stage::All);
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
    t.frames.push(super::frame::Frame {
        proto: main,
        ip: 1,
        base: 0,
        closure: None,
        globals,
        tail_calls: u32::MAX as u64,
        entry: None,
    });
    t.frames[0].tail_calls += 1;
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
        assert_eq!(rt.args(), ["a"]);
        assert_eq!(out.text(), "[\"a\"]\nbye\n");
    });
}
