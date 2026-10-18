//! Pattern matching: `match`, guards, failed arms, and destructuring
//! assignment.

use super::m4_tests::check_out;
use crate::testutil::run;

#[test]
fn match_and_destructuring_run() {
    let (out, err, code) =
        run("match [1, 2, 3]: [a, ...rest] => print(a, rest) ;;\n(x, y) = (1, 2)\nprint(x + y)");
    assert_eq!((out.as_str(), err.as_str(), code), ("1 [2, 3]\n3\n", "", 0));
}

#[test]
fn match_runs() {
    check_out(&[
        (
            "fn f(v) = match v:\n  0 => \"zero\"\n  n when n < 0 => \"neg\"\n  n => \"pos ${n}\"\n;;\nprint(f(0), f(-1), f(5))",
            "zero neg pos 5\n",
        ),
        (
            "fn f(r) = match r: Ok(x) => x; Err(e) => \"err:${e}\" ;;\nprint(f(Ok(1)), f(Err('bad)))",
            "1 err:'bad\n",
        ),
        (
            "fn f(p) = match p: (0, 0) => \"origin\"; (x, 0) => \"x${x}\"; (0, y) => \"y${y}\"; _ => \"other\" ;;\nprint(f((0, 0)), f((2, 0)), f((0, 3)), f((1, 1)))",
            "origin x2 y3 other\n",
        ),
        (
            "fn len2(xs) = match xs: [] => 0; [_, ...rest] => 1 + len2(rest) ;;\nprint(len2([]), len2([1, 2, 3]))",
            "0 3\n",
        ),
        (
            "fn f(xs) = match xs: [a, b] => a + b; [a, ...] => a; _ => -1 ;;\nprint(f([1, 2]), f([5, 6, 7]), f([]))",
            "3 5 -1\n",
        ),
        (
            "fn f(m) = match m: {name: n, port: 80} => \"${n}:80\"; {name: n} => n; _ => \"?\" ;;\nprint(f({name: \"a\", port: 80}), f({name: \"b\", x: 1}), f({}))",
            "a:80 b ?\n",
        ),
        (
            "fn f(c) = match c: 'r | 'g | 'b => \"rgb\"; Red => \"red\"; _ => \"no\" ;;\nprint(f('g), f(Red), f('x))",
            "rgb red no\n",
        ),
        (
            "fn f(v) = match v: 1 | 1.0 => \"one\"; \"s\" => \"str\"; nil => \"nil\"; true => \"t\"; 1h => \"h\"; _ => \"?\" ;;\nprint(f(1.0), f(\"s\"), f(nil), f(true), f(1h), f(2))",
            "one str nil t h ?\n",
        ),
        (
            "fn f(v) = match v: Ok(Ok(x)) => x; Ok(_) => 0; _ => -1 ;;\nprint(f(Ok(Ok(7))), f(Ok(3)), f(Err(1)))",
            "7 0 -1\n",
        ),
        (
            "fn f(v, x) = match v: y when y == x => \"same\"; _ => \"diff\" ;;\nprint(f(10, 10), f(3, 10))",
            "same diff\n",
        ),
        (
            "r = match 3: n when n > 2 => \"big\"; _ => \"small\" ;;\nprint(r)",
            "big\n",
        ),
        (
            "fn f(v) = match v: Ok => 1; Ok(_) => 2 ;;\nprint(f('Ok), f(Ok(0)))",
            "1 2\n",
        ),
    ]);
}

#[test]
fn failed_arm_does_not_leak_bindings() {
    // Arm 0 binds a and b then fails on the guard; arm 1 binds c into the
    // slot a used. c must see its own value.
    let (out, _, code) =
        run("fn f(v) = match v: (a, b) when a > 100 => a; (c, _) => c ;;\nprint(f((1, 2)))");
    assert_eq!((out.as_str(), code), ("1\n", 0));
    let (out, _, _) = run(
        "fn f(xs) = match xs: [first, ...rest] => rest; [...rest] => rest ;;\nprint(f([]), f([1]))",
    );
    assert_eq!(out, "[] []\n", "rest on empty");
    let (out, _, _) = run(
        "fn f(xs) = match xs: [a] => match a: (p, q) => p + q; _ => a ;;; _ => 0 ;;\nprint(f([(1, 2)]), f([5]))",
    );
    assert_eq!(out, "3 5\n", "nested match");
}

#[test]
fn destructuring_assignment() {
    check_out(&[
        ("(q, r) = (7, 2)\nprint(q, r)", "7 2\n"),
        (
            "[first, ...rest] = [1, 2, 3]\nprint(first, rest)",
            "1 [2, 3]\n",
        ),
        (
            "{name: n, port: p} = {name: \"a\", port: 80, x: 1}\nprint(n, p)",
            "a 80\n",
        ),
        ("Ok(text) = Ok(\"hi\")\nprint(text)", "hi\n"),
        (
            "fn f():\n  (a, b) = (1, 2)\n  (a, b) = (b, a)\n  a - b\n;;\nprint(f())",
            "1\n",
        ),
    ]);
}

/// The rest of a big list is a view on the same vector, and a long list
/// walked by `[_, ...rest]` recursion stays linear.
#[test]
fn rest_of_a_big_list_matches() {
    let (out, err, code) = run(
        "fn walk(xs, n) = match xs: [] => n; [_, ...rest] => walk(rest, n + 1) ;;\nprint(walk(for i in 0..20000: i ;;, 0))",
    );
    assert_eq!((out.as_str(), err.as_str(), code), ("20000\n", "", 0));
}

#[test]
fn no_match_report() {
    let (_, err, code) = run("fn f(x) = match x: 1 => 2 ;;\nf(5)");
    assert_eq!(
        err,
        "error[E0601]: no pattern matched 5\n  --> -e:1:11\n  |\n1 | fn f(x) = match x: 1 => 2 ;;\n  |           ^\n  |\n call path (innermost first):\n   -e:1       f\n   -e:2       <entry>\n"
    );
    assert_eq!(code, 1);
}
