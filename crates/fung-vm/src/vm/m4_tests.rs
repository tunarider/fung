//! The M4 instructions: collections, indexing, slices, `in`, `!` `?` `??`,
//! partial application, and their runtime errors.

use crate::testutil::{Opts, exec, run};

/// Runs every case and reports all mismatches at once.
pub(super) fn check_out(cases: &[(&str, &str)]) {
    let mut bad = Vec::new();
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        if !o.code.is_empty() || o.out != *want {
            bad.push(format!(
                "{src:?}: out={:?} code={} err={:?}, want {want:?}",
                o.out, o.code, o.err
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Runs every case and checks the error code it stops with.
pub(super) fn check_code(cases: &[(&str, &str)]) {
    let mut bad = Vec::new();
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        if o.code != *want {
            bad.push(format!(
                "{src:?}: code={} err={:?}, want {want}",
                o.code, o.err
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The headline of the error a program stops with.
pub(super) fn headline(src: &str) -> String {
    exec(src, Opts::default())
        .error
        .map(|e| e.headline)
        .unwrap_or_default()
}

#[test]
fn collections_run() {
    check_out(&[
        (
            "print([1, \"a\", [2]], (1,), (), {name: \"x\", \"a b\": 1}, Circle(3), 0..3, 0..=3)",
            "[1, \"a\", [2]] (1,) () {name: \"x\", \"a b\": 1} Circle(3) 0..3 0..=3\n",
        ),
        (
            "xs = [10, 20, 30]\nprint(xs[0], xs[-1], xs[1..3], xs[..1], xs[-2..], xs[0..0], xs[3..])",
            "10 30 [20, 30] [10] [20, 30] [] []\n",
        ),
        (
            "print(\"참치회\"[1], \"참치회\"[1..], len(\"참치\"), len([1, 2]), len(0..10))",
            "치 치회 2 2 10\n",
        ),
        (
            "m = {name: \"x\", port: 80}\nprint(m.name, m[\"port\"], len(m))",
            "x 80 2\n",
        ),
        (
            "print(1 in [1, 2], 3 in [1, 2], \"ell\" in \"hello\", \"k\" in {k: 1}, 5 in 0..10, 10 in 0..10, 10 in 0..=10, 2 not in [1])",
            "true false true true true false true true\n",
        ),
        (
            "print([1] + [2], [1, 2] == [1, 2], [1] == (1,), {a: 1, b: 2} == {b: 2, a: 1}, [1, 2] < [1, 3], Ok(1) == Ok(1))",
            "[1, 2] true false true true true\n",
        ),
        (
            "print(type_of(Circle(3)), type_of(Red), type_of([]), type_of(0..1))",
            "'Circle 'sym 'list 'range\n",
        ),
        (
            "fn area(s: Circle | Rect) = 1\nprint(area(Circle(3)))",
            "1\n",
        ),
        (
            "xs = [1, 2, 3]\nprint(xs |> len, xs |> map(x => x * 2), xs |> map(x => x + 1) |> len)",
            "3 [2, 4, 6] 3\n",
        ),
        (
            "print(for x in [1, 2]: x * 10 ;;, for i in 0..3: i ;;)",
            "[10, 20] [0, 1, 2]\n",
        ),
        (
            "print(for (k, v) in [(1, 2), (3, 4)]: k + v ;;)",
            "[3, 7]\n",
        ),
        (
            "fn sum(...xs) = len(xs)\nprint(sum(), sum(1, 2, 3))",
            "0 3\n",
        ),
        (
            "fn f(a, ...rest) = rest\nprint(f(1), f(1, 2, 3))",
            "[] [2, 3]\n",
        ),
        (
            "add = (a, b) => a + b\ninc = add(1, _)\nprint(inc(5), inc(10))",
            "6 11\n",
        ),
        (
            "fn join3(a, b, c) = a + b + c\nmid = join3(\"<\", _, \">\")\nprint(mid(\"x\"))",
            "<x>\n",
        ),
    ]);
}

#[test]
fn review_fixes() {
    check_out(&[
        (
            "xs = [1, 2, 3]\nprint(xs[1..=2], xs[0..=-1], xs[..=0], \"참치회\"[1..=1])",
            "[2, 3] [1, 2, 3] [1] 치\n",
        ),
        (
            "xs = [1, 2, 3]\ni = nil\nprint(xs[i ?? 0], xs[true then 1 else 2])",
            "1 2\n",
        ),
        (
            "a = 1\nprint(for (a, b) in [(2, 3)]: a + b ;;, a)",
            "[5] 1\n",
        ),
    ]);
    check_code(&[
        ("[1, 2, 3][1..=3]", "E0507"),
        ("Err()!", "E0603"),
        ("Err(1, 2)!", "E0603"),
    ]);
    assert_eq!(
        headline("Err(1, 2)!"),
        "error[E0603]: unhandled Err: Err(1, 2)"
    );
}

#[test]
fn partial_evaluates_fixed_args_once() {
    let (out, _, code) =
        run("fn f(a, b) = a + b\nfn side(): print(\"once\"); 0 ;;\ng = f(side(), _)\ng(1)\ng(2)");
    assert_eq!((out.as_str(), code), ("once\n", 0));
}

#[test]
fn result_operators() {
    check_out(&[
        (
            "print(Ok(1)!, Ok(2) ?? 0, nil ?? 3, Err(\"e\") ?? 4, 5 ?? 6)",
            "1 2 3 4 5\n",
        ),
        (
            "fn f(r) = r? + 1\nprint(f(Ok(1)), f(Err(\"bad\")))",
            "2 Err(\"bad\")\n",
        ),
        (
            "fn f(r) -> Ok | Err = Ok(r? * 2)\nprint(f(Ok(2)), f(Err('x)))",
            "Ok(4) Err('x)\n",
        ),
        ("print(nil ?? Err(1) ?? 2)", "2\n"),
        ("fn f(r) = r?\nprint(f(nil))", "nil\n"),
        // Ok with zero or several payloads unwraps to a tuple.
        ("print(Ok() ?? 1, Ok(1, 2)!)", "() (1, 2)\n"),
    ]);
    check_code(&[
        ("Err({kind: 'x, message: \"boom\"})!", "E0603"),
        ("nil!", "E0604"),
        ("5!", "E0605"),
        ("fn f(r) = r?\nf(1)", "E0605"),
    ]);
    assert_eq!(
        headline("Err({kind: 'x, message: \"boom\"})!"),
        "error[E0603]: unhandled Err: boom"
    );
}

#[test]
fn runtime_errors_m4() {
    check_code(&[
        ("[1][1]", "E0507"),
        ("[1][-2]", "E0507"),
        ("[1, 2][0..3]", "E0507"),
        ("{a: 1}.b", "E0508"),
        ("{a: 1}[\"b\"]", "E0508"),
        ("(1, 2)[0]", "E0509"),
        ("5[0]", "E0509"),
        ("(1,)[0..1]", "E0511"),
        ("1 in 1", "E0512"),
        ("1 in \"a\"", "E0501"),
        ("[1][\"a\"]", "E0501"),
        ("\"a\"..\"b\"", "E0501"),
        ("match 1: 0 => 1 ;;", "E0601"),
        ("(a, b) = (1, 2, 3)", "E0602"),
        ("Ok(x) = Err(1)", "E0602"),
        ("map(1, x => x)", "E0506"),
    ]);
}

/// Whole reports and headlines, as `fung` prints them.
#[test]
fn m4_error_reports() {
    let cases = [
        ("1 in \"a\"", "error[E0501]: cannot search str and int"),
        (
            "Ok(x) = Err(1)",
            "error[E0602]: value Err(1) does not match the pattern",
        ),
        (
            "(1,)[0..1]",
            "error[E0511]: cannot slice a value of type tuple",
        ),
        ("nil!", "error[E0604]: `!` on nil"),
        (
            "5!",
            "error[E0605]: `!` on a value of type int (not Ok or Err)",
        ),
        (
            "fn f(r) = r?\nf(1)",
            "error[E0605]: `?` on a value of type int (not Ok or Err)",
        ),
        (
            "[1, 2, 3][1..=3]",
            "error[E0507]: index 3 is out of range (length 3)",
        ),
        (
            "1 in (1, 2)",
            "error[E0512]: cannot test membership in a value of type tuple",
        ),
    ];
    for (src, want) in cases {
        assert_eq!(headline(src), want, "{src:?}");
    }
    let (_, err, code) = run("Err({kind: 1, message: \"boom\"})!");
    assert_eq!(
        err,
        "error[E0603]: unhandled Err: boom\n  --> -e:1:32\n  |\n1 | Err({kind: 1, message: \"boom\"})!\n  |                                ^\n  |\n call path (innermost first):\n   -e:1       <entry>\n"
    );
    assert_eq!(code, 1);
    let (_, err, _) = run("print((1,2).x)");
    assert_eq!(
        err,
        "error[E0509]: cannot index a value of type tuple\n  --> -e:1:12\n  |\n1 | print((1,2).x)\n  |            ^\n  |\n call path (innermost first):\n   -e:1       <entry>\n"
    );
}
