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

/// The headline of the first error a program gives, before it runs or
/// while it runs.
pub(super) fn first_error(src: &str) -> String {
    let o = exec(src, Opts::default());
    match o.error {
        Some(e) => e.headline,
        None => o.err.lines().next().unwrap_or("").to_string(),
    }
}

#[test]
fn collections_run() {
    check_out(&[
        (
            "print([1, \"a\", [2]], (1,), (), {name: \"x\", \"a b\": 1}, Circle(3), 0..3, 0..=3)",
            "[1, \"a\", [2]] (1,) () {name: \"x\", \"a b\": 1} Circle(3) 0..3 0..=3\n",
        ),
        (
            "xs = [10, 20, 30]\nprint(xs[0], xs[2], xs[1..3], xs[..1], xs[1..], xs[0..0], xs[3..])",
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
            "print([1] + [2], [1, 2] == [1, 2], [1] == [2], {a: 1, b: 2} == {b: 2, a: 1}, [1, 2] < [1, 3], Ok(1) == Ok(1))",
            "[1, 2] true false true true true\n",
        ),
        (
            "print(type_of(Circle(3)), type_of(Red), type_of([]), type_of(0..1))",
            "Circle 'sym 'list 'range\n",
        ),
        (
            "fn area(_s: Circle | Rect) = 1\nprint(area(Circle(3)))",
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
            "fn count(...xs) = len(xs)\nprint(count(), count(1, 2, 3))",
            "0 3\n",
        ),
        (
            "fn f(_a, ...rest) = rest\nprint(f(1), f(1, 2, 3))",
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

/// An uppercase name in a type annotation is a tag name, whatever it
/// spells (D134): `Int` is the tag `Int`, not the type `int`.
#[test]
fn uppercase_annotations_are_tag_names() {
    check_out(&[
        (
            "type token = Int | Str\nfn show(t: token) = t\nprint(show(Int(3)), show(Str(\"a\")))",
            "Int(3) Str(\"a\")\n",
        ),
        (
            "fn show(t: Int | Str) -> Int | Str = t\nprint(show(Int(3)), show(Str(\"a\")))",
            "Int(3) Str(\"a\")\n",
        ),
        // Like any tag name, the bare symbol of that name too.
        (
            "fn f(x: List | Bool) = x\nprint(f(List), f(Bool(1)))",
            "List Bool(1)\n",
        ),
    ]);
    check_code(&[
        ("fn f(x: Int) = x\nf(3)", "E0403"),
        ("fn f(x) -> String = x\nf(\"a\")", "E0404"),
        ("fn f(a: integer) = a", "E0212"),
    ]);
}

#[test]
fn review_fixes() {
    check_out(&[
        (
            "xs = [1, 2, 3]\nprint(xs[1..=2], xs[0..=2], xs[..=0], \"참치회\"[1..=1])",
            "[2, 3] [1, 2, 3] [1] 치\n",
        ),
        (
            "xs = [1, 2, 3]\ni = nil\nprint(xs[i ?? 0], xs[true then 1 else 2])",
            "1 2\n",
        ),
        (
            "a = 1\nprint(for (p, q) in [(2, 3)]: p + q ;;, a)",
            "[5] 1\n",
        ),
    ]);
    check_code(&[
        ("[1, 2, 3][1..=3]", "E0507"),
        ("Err()!", "E0117"),
        // A result holds exactly one value (SP12): Err(1, 2) is not a value.
        ("Err(1, 2)!", "E0236"),
    ]);
}

#[test]
fn partial_evaluates_fixed_args_once() {
    let (out, _, code) =
        run("fn f(a, b) = a + b\nfn side(): print(\"once\"); 0 ;;\ng = f(side(), _)\ng(1)\ng(2)");
    assert_eq!((out.as_str(), code), ("once\n", 0));
}

/// A bare `Ok` or `Err` is its symbol, like any bare tag (D123): no
/// result, and the printed form reads back.
#[test]
fn bare_result_tags_are_symbols() {
    check_out(&[
        (
            "x = Ok\nprint(x, [Err], x == sym(\"Ok\"), type_of(x), type_of(Ok(1)) == Ok)",
            "Ok [Err] true 'sym true\n",
        ),
        (
            "fn f(r) = match r:\n  Ok => \"sym\"\n  Ok(v) => \"ok ${v}\"\n  _ => \"?\"\n;;\nprint(f(Ok), f(Ok(1)), f(Err))",
            "sym ok 1 ?\n",
        ),
        (
            "fn f(x) -> Ok | Err = x\nprint(f(Ok), f(Err), f(Ok(1)))",
            "Ok Err Ok(1)\n",
        ),
        (
            "r = Ok\nprint(\"${r}\", r ?? 1, is_ok(r), is_err(Err))",
            "Ok Ok false false\n",
        ),
        ("x = 1 |> Ok\nprint(x)", "Ok(1)\n"),
    ]);
    check_code(&[
        ("fn f() = Ok\nf()!", "E0605"),
        ("fn f(r) = r?\nf(Err)", "E0605"),
        ("print(Ok(1, 2))", "E0236"),
        ("x = 1 |> Ok(2)", "E0236"),
        ("print(Ok())", "E0117"),
        ("x = 1 |> (Ok)", "E0401"),
        ("print('Ok)", "E0128"),
        ("print(Ok ?? 1)", "E0243"),
    ]);
}

#[test]
fn result_operators() {
    check_out(&[
        (
            "five = 5\nprint(Ok(1)!, Ok(2) ?? 0, nil ?? 3, Err(\"e\") ?? 4, five ?? 6)",
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
    ]);
    check_code(&[
        ("Err({kind: 'x, message: \"boom\"})!", "E0603"),
        ("nil!", "E0604"),
        // `?` takes only a result, as `!` does (SP13).
        ("fn f(r) = r?\nf(nil)", "E0604"),
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
        ("[1][-1]", "E0507"),
        ("\"ab\"[-1]", "E0507"),
        ("[1, 2][..-1]", "E0507"),
        ("\"ab\"[-1..]", "E0507"),
        ("[1, 2][0..3]", "E0507"),
        ("{a: 1}.b", "E0508"),
        ("{a: 1}[\"b\"]", "E0508"),
        ("(1, 2)[0]", "E0509"),
        ("5[0]", "E0509"),
        ("(1,)[0..1]", "E0511"),
        ("1 in 1", "E0512"),
        ("1 in \"a\"", "E0501"),
        ("[1][\"a\"]", "E0509"),
        ("\"a\"..\"b\"", "E0501"),
        ("match 1: 0 => 1 ;;", "E0601"),
        ("(_a, _b) = (1, 2, 3)", "E0602"),
        ("Ok(_x) = Err(1)", "E0602"),
        ("map(1, x => x)", "E0506"),
    ]);
}

/// Whole reports and headlines, as `fung` prints them.
#[test]
fn m4_error_reports() {
    let cases = [
        ("1 in \"a\"", "error[E0501]: cannot search str and int"),
        (
            "Ok(_x) = Err(1)",
            "error[E0602]: value Err(1) does not match the pattern",
        ),
        (
            "(1,)[0..1]",
            "error[E0511]: cannot slice a value of type tuple",
        ),
        ("nil!", "error[E0604]: `!` on nil"),
        (
            "5!",
            "error[E0605]: `!` on a value of type int, which is not a result (Ok(x) or Err(e))",
        ),
        (
            "fn f(r) = r?\nf(1)",
            "error[E0605]: `?` on a value of type int, which is not a result (Ok(x) or Err(e))",
        ),
        (
            "[1, 2, 3][1..=3]",
            "error[E0507]: slice end 3 is out of range (length 3)",
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
        "error[E0509]: cannot index tuple with str\n  --> -e:1:12\n  |\n1 | print((1,2).x)\n  |            ^\n  |\n call path (innermost first):\n   -e:1       <entry>\n help: a tuple has no fields: only maps and modules do\n"
    );
}

/// A pattern or `!` error shows at most the first 200 characters of the
/// value, then `…`: a list of a million ints would print megabytes into
/// a one-line headline.
#[test]
fn errors_cut_long_values_in_the_headline() {
    let head = |src: &str| exec(src, Opts::default()).error.expect(src).headline;
    let ints: Vec<String> = (0..1000).map(|i| i.to_string()).collect();
    let list: String = format!("[{}", ints.join(", ")).chars().take(200).collect();
    assert_eq!(
        head("[_a] = map(0..1000000, x => x)"),
        format!("error[E0602]: value {list}… does not match the pattern")
    );
    assert_eq!(
        head("match map(0..1000000, x => x): [] => 0 ;;"),
        format!("error[E0601]: no pattern matched {list}…")
    );
    let xs = "x".repeat(300);
    assert_eq!(
        head(&format!("Err({{message: \"{xs}\"}})!")),
        format!("error[E0603]: unhandled Err: {}…", &xs[..200])
    );
    // 200 characters exactly are shown whole, quotes included.
    let fits = "가".repeat(198);
    assert_eq!(
        head(&format!("match \"{fits}\": \"\" => 0 ;;")),
        format!("error[E0601]: no pattern matched \"{fits}\"")
    );
    let over = "가".repeat(199);
    assert_eq!(
        head(&format!("match \"{over}\": \"\" => 0 ;;")),
        format!("error[E0601]: no pattern matched \"{}…", &over[..199 * 3])
    );
}

/// VA28: a value whose parts are shared shows its start at once, however
/// long its whole text would be; `assert`'s notes are cut the same way.
#[test]
fn errors_show_a_shared_value_at_once() {
    let x = "x = fold(0..40, [1], (a, _i) => [a, a])\n";
    let head = |src: &str| {
        exec(&format!("{x}{src}"), Opts::default())
            .error
            .expect(src)
            .headline
    };
    let start = format!("{}1]", "[".repeat(41));
    assert!(head("[_y] = x").starts_with(&format!("error[E0602]: value {start}")));
    assert!(head("[_y] = x").ends_with("… does not match the pattern"));
    assert!(head("match x: [] => 1 ;;").ends_with('…'));
    let o = exec(&format!("{x}assert x != x"), Opts::default());
    let e = o.error.expect("assert fails");
    assert!(e.headline.starts_with("error[E0807]"), "{}", e.headline);
    let notes = e.notes.join("\n");
    assert!(notes.contains(&format!("x = {start}")), "{notes}");
    assert!(notes.len() < 1000, "{notes}");
    // A conversion's message cuts the value too.
    assert!(head("int(x)").len() < 300);
}

/// A guard on alternatives is tried with each alternative in turn (03):
/// the arm is taken when any alternative matches and passes the guard,
/// nested alternatives included. Without a guard the first match wins,
/// as before.
#[test]
fn a_failed_guard_tries_the_next_alternative() {
    check_out(&[
        (
            "fn big(p) = match p: (a, _) | (_, a) when a > 1 => a; _ => -1 ;;\n\
             print(big((0, 5)), big((7, 0)), big((0, 0)), big((3, 9)))",
            "5 7 -1 3\n",
        ),
        (
            "fn f(r) = match r: Ok((a, _) | (_, a)) when a > 1 => a; _ => -1 ;;\n\
             print(f(Ok((0, 5))), f(Ok((0, 0))), f(Err(1)))",
            "5 -1 -1\n",
        ),
        (
            "fn f(x) = match x: (1, a) | (a, 1) when a == 2 => \"two\"; _ => \"no\" ;;\n\
             print(f((1, 2)), f((2, 1)), f((1, 1)))",
            "two two no\n",
        ),
        // the guard runs once per alternative tried
        (
            "fn g(a):\n  print(\"g\", a)\n  a > 1\n;;\n\
             print(match (0, 5): (a, _) | (_, a) when g(a) => a; _ => -1 ;;)",
            "g 0\ng 5\n5\n",
        ),
    ]);
    // only the `|` that bind names are spread, so the limit counts those
    // (D131): each nested one multiplies them
    let names = |n: usize| {
        (1..=n)
            .map(|i| format!("(a{i}, _) | (_, a{i})"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let sum = |n: usize| {
        (1..=n)
            .map(|i| format!("a{i}"))
            .collect::<Vec<_>>()
            .join(" + ")
    };
    check_code(&[(
        &format!(
            "print(match 1: [{}] when {} > 0 => 1; _ => 2 ;;)",
            names(9),
            sum(9)
        ),
        "E0230",
    )]);
    let pairs = ["(1, 2)"; 8].join(", ");
    check_out(&[(
        &format!(
            "print(match [{pairs}]: [{}] when {} == 8 => 1; _ => 2 ;;)",
            names(8),
            sum(8)
        ),
        "1\n",
    )]);
    // a `|` that binds nothing is no multiplier: nine of them, or
    // two hundred, compile, and the guard still sees every way to match
    let many = ["1 | 2"; 9].join(", ");
    let plain = ["2"; 9].join(", ");
    let wide = ["1 | 2"; 200].join(", ");
    let wide_vals = vec!["2"; 200].join(", ");
    check_out(&[
        (
            &format!("print(match [{plain}]: [{many}] when true => 1; _ => 2 ;;)"),
            "1\n",
        ),
        (
            &format!("print(match [{wide_vals}]: [{wide}] when true => 1; _ => 2 ;;)"),
            "1\n",
        ),
        (
            &format!("print(match [{plain}, 7]: [{many}, a] when a == 7 => a; _ => 0 ;;)"),
            "7\n",
        ),
        // the names bind in the alternatives that do, around ones that do not
        (
            "fn f(p) = match p: (1 | 2, (a, _) | (_, a)) when a > 1 => a; _ => -1 ;;\n\
             print(f((2, (0, 5))), f((1, (7, 0))), f((3, (9, 9))), f((1, (0, 0))))",
            "5 7 -1 -1\n",
        ),
    ]);
}

/// FE55: a parameter with no name in the source (a hole, a pattern) is
/// called by its place, and a hole cannot be named.
#[test]
fn hidden_parameters_are_called_by_place() {
    assert_eq!(
        first_error("f = fn((_a, _b), _c): 1 ;;\nf()"),
        "error[E0407]: missing argument 1"
    );
    assert_eq!(
        first_error("fn s2(a, b) = a - b\ng = s2(_, 1)\ng()"),
        "error[E0407]: missing argument 1"
    );
    assert!(
        first_error("fn s2(a, b) = a - b\ng = s2(_, 1)\nprint(g(h0: 5))")
            .starts_with("error[E0406]")
    );
    assert_eq!(
        first_error("f = fn(a, b): [a, b] ;;\nf(1)"),
        "error[E0407]: missing argument `b`"
    );
}
