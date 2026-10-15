//! Ports of Go `internal/vm/m6_test.go` (except the REPL's
//! TestReplGlobalsSurviveGrowth): assert, test blocks, dbg and parameter
//! patterns.

use super::m4_tests::{check_code, check_out};
use crate::testutil::{Opts, Setup, exec, on_big_stack, run, setup};
use crate::vm::Stop;

#[test]
fn assert_reports() {
    let cases: [(&str, &[&str]); 4] = [
        (
            "fn f(x) = x + 1\nassert f(1) == 3",
            &["assertion failed: f(1) == 3", "f(1) = 2"],
        ),
        (
            "a = 1\nb = 2\nassert a > b",
            &["assertion failed: a > b", "a = 1", "b = 2"],
        ),
        ("assert false", &["assertion failed: false"]),
        (
            "xs = []\nassert is_empty(xs) == false",
            &["is_empty(xs) = true"],
        ),
    ];
    for (src, wants) in cases {
        let o = exec(src, Opts::default());
        let e = o.error.unwrap_or_else(|| panic!("{src:?}: no error"));
        assert_eq!(e.code, "E0807", "{src:?}");
        let text = e.headline.clone() + "\n" + &e.notes.join("\n");
        for w in wants {
            assert!(text.contains(w), "{src:?}: {text:?} lacks {w:?}");
        }
    }
    check_code(&[("assert 1", "E0505"), ("assert 1 < \"a\"", "E0501")]);
    check_out(&[(
        "fn f(): print(\"f\"); 1 ;;\nfn g(): print(\"g\"); 1 ;;\nassert f() == g()\nprint(\"ok\")",
        "f\ng\nok\n",
    )]);
    check_code(&[("assert 1 != 1.0", "E0807")]);
}

/// The whole report, as the Go binary prints it.
#[test]
fn assert_report_matches_go() {
    let (_, err, code) = run("a = 1\nb = 2\nassert a > b");
    assert_eq!(
        err,
        "error[E0807]: assertion failed: a > b\n  --> -e:3:1\n  |\n3 | assert a > b\n  | ^\n  |\n  a = 1\n  b = 2\n call path (innermost first):\n   -e:3       <entry>\n"
    );
    assert_eq!(code, 1);
}

fn with_tests(src: &'static str) -> Setup {
    let opts = Opts {
        file: "t.fung",
        tests: true,
        ..Opts::default()
    };
    match setup(src, opts) {
        Ok(s) => s,
        Err((text, _)) => panic!("{text}"),
    }
}

#[test]
fn test_blocks_register_only_when_requested() {
    on_big_stack(|| {
        let src = "x = 1\ntest \"one\": assert x == 1 ;;\ntest \"two\": assert x == 2 ;;\nprint(\"body\")";
        let (out, _, code) = run(src);
        assert_eq!((out.as_str(), code), ("body\n", 0), "run ignores tests");

        let Setup { rt, main, out, .. } = with_tests(src);
        rt.run(main).expect("the body runs");
        let tests: Vec<_> = rt.tests().to_vec();
        let names: Vec<&str> = tests.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["one", "two"]);
        assert_eq!(out.text(), "body\n");
        assert!(rt.call_test(&tests[0].func).is_ok(), "test one");
        match rt.call_test(&tests[1].func) {
            Err(Stop::Error(d)) => assert_eq!(d.code, "E0807"),
            r => panic!("test two: {r:?}"),
        }

        let Setup { rt, main, .. } = with_tests("x = 1\ntest \"t\": assert x == 1 ;;\nx = 2");
        rt.run(main).expect("the body runs");
        let f = rt.tests()[0].func.clone();
        assert!(
            rt.call_test(&f).is_ok(),
            "a test sees the variables that existed when it was defined (model C)"
        );

        let Setup { rt, main, .. } = with_tests("test \"e\": exit(3) ;;");
        rt.run(main).expect("the body runs");
        let f = rt.tests()[0].func.clone();
        assert!(
            matches!(rt.call_test(&f), Err(Stop::Exit(3))),
            "exit signal"
        );
    });
}

#[test]
fn dbg_prints_to_stderr_and_returns_value() {
    let o = exec(
        "xs = [1, 2]\nprint(xs |> map(x => x * 2) |> dbg |> len)\nd = dbg\nprint(d(3))",
        Opts {
            file: "t.fung",
            ..Opts::default()
        },
    );
    assert_eq!(o.code, "");
    assert_eq!(o.out, "2\n3\n");
    assert_eq!(
        o.err,
        "[t.fung:2] xs |> map(x => x * 2) = [2, 4]\ndbg = 3\n"
    );
}

#[test]
fn dbg_inside_interpolation_shows_the_right_text() {
    let o = exec(
        "x = 5\nprint(\"v=${dbg(x + 1)} tail\")",
        Opts {
            file: "t.fung",
            ..Opts::default()
        },
    );
    assert_eq!(o.code, "");
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        ("v=6 tail\n", "[t.fung:2] x + 1 = 6\n")
    );
}

#[test]
fn parameter_patterns_run() {
    check_out(&[
        (
            "print(map([(1, 2), (3, 4)], ((a, b)) => a + b))",
            "[3, 7]\n",
        ),
        ("fn f((a, b), c) = a + b + c\nprint(f((1, 2), 3))", "6\n"),
        ("fn g({name: n}) = n\nprint(g({name: \"x\", y: 1}))", "x\n"),
        (
            "print(map([Ok(1), Ok(2)], (Ok(x)) => x * 10))",
            "[10, 20]\n",
        ),
        (
            "print(map([1, 2], _ => 0), sort_by([(1, \"b\"), (2, \"a\")], ((_, s)) => s))",
            "[0, 0] [(2, \"a\"), (1, \"b\")]\n",
        ),
        ("each(pairs({a: 1}), ((k, v)) => print(k, v))", "a 1\n"),
        (
            "fn h(x, ...rest) = rest\nfn p([a], y = 2) = a + y\nprint(h(1, 2), p([1]))",
            "[2] 3\n",
        ),
    ]);
    check_code(&[
        ("fn f((a, b)) = a\nf((1, 2, 3))", "E0602"),
        ("f = ((a, b)) => a\nf(1)", "E0602"),
        ("fn f((a, b)) = a\nf(x: 1)", "E0406"),
    ]);
}
