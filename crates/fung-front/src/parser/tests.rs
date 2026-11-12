//! Parser tests: case tables by construct, plus whole trees of larger
//! programs.

use super::*;
use crate::lexer::lex;
use crate::source::SourceMap;
use crate::tree::Lit;

fn toks_of(src: &str) -> Vec<Token> {
    let mut sm = SourceMap::new();
    let f = sm.add("t.fung", src);
    let (toks, _) = lex(f, src).unwrap_or_else(|e| panic!("lex {src:?}: {}", e.code));
    toks
}

/// The tree or `CODE line:col`.
fn ast(src: &str) -> String {
    let mut sm = SourceMap::new();
    let f = sm.add("-e", src);
    let toks = match lex(f, src) {
        Ok((toks, _)) => toks,
        Err(d) => return format!("{} {}:{}", d.code, d.pos.line, d.pos.col),
    };
    parse(&toks)
        .map(|n| n.to_string())
        .unwrap_or_else(|d| format!("{} {}:{}", d.code, d.pos.line, d.pos.col))
}

/// The tree, failing the test on an error.
#[track_caller]
fn p(src: &str) -> String {
    match parse(&toks_of(src)) {
        Ok(n) => n.to_string(),
        Err(d) => panic!("parse {src:?}: {} {}:{}", d.code, d.pos.line, d.pos.col),
    }
}

/// The error code, failing the test when parsing succeeds.
#[track_caller]
fn perr(src: &str) -> String {
    match parse(&toks_of(src)) {
        Ok(n) => panic!("parse {src:?}: want error, got {n}"),
        Err(d) => d.code,
    }
}

/// The error code or "" without echoing the input.
fn perr_quiet(src: &str) -> String {
    match parse(&toks_of(src)) {
        Ok(_) => String::new(),
        Err(d) => d.code,
    }
}

#[track_caller]
fn check(cases: &[(&str, &str)]) {
    let mut bad = Vec::new();
    for (src, want) in cases {
        let got = p(src);
        if got != *want {
            bad.push(format!("{src:?}:\n got {got}\nwant {want}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[track_caller]
fn check_errs(cases: &[(&str, &str)]) {
    let mut bad = Vec::new();
    for (src, want) in cases {
        let got = perr(src);
        if got != *want {
            bad.push(format!("{src:?}: {got}, want {want}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Runs f on a thread with a stack large enough for MAX_DEPTH nesting.
fn big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

// --- Whole trees ---

#[test]
fn whole_trees() {
    assert_eq!(
        ast("x = 1 + 2 * 3"),
        "(program (assign x (binary + 1 (binary * 2 3))))"
    );
    assert_eq!(
        ast("f = (a, b) => a - b\nprint(f(1, b: 2))"),
        "(program (assign f (fn (a, b) (block (binary - a b)))) (call print (call f 1 (named b 2))))"
    );
    assert_eq!(
        ast("s = \"hi ${1 + 1}\""),
        "(program (assign s (interp \"hi \" (binary + 1 1))))"
    );
    assert_eq!(
        ast("y = Ok(1)? ?? 0"),
        "(program (assign y (coalesce (unwrap ? (tag Ok 1)) 0)))"
    );
    assert_eq!(
        ast(
            "import fs\nfn add(a, b = 1) = a + b\nxs = [1, 2] |> map(x => add(x))\nmatch xs: [a, ...rest] => print(a, len(rest)) ;;"
        ),
        "(program (import fs) (fn add(a, b=?) 1 (block (binary + a b))) (assign xs (pipe (list 1 2) (call map (fn (x) (block (call add x)))))) (match xs (arm (plist (pbind a) (prest rest)) (nil) (block (call print a (call len rest))))))"
    );
}

#[test]
fn syntax_errors_point_at_the_token() {
    assert_eq!(ast("x = )"), "E0110 1:5");
    assert_eq!(ast("d = [1, ...[2]]"), "E0133 1:9");
    assert_eq!(ast("y = 1 < 2 < 3"), "E0111 1:11");
}

#[test]
fn deep_nesting_is_an_error_not_a_crash() {
    let src = format!("x = {}1{}", "(".repeat(20_000), ")".repeat(20_000));
    let handle = std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || ast(&src))
        .unwrap();
    assert_eq!(handle.join().unwrap(), "E0115 1:10005");
}

// --- expressions and statements ---

#[test]
fn precedence_and_associativity() {
    check(&[
        ("1 + 2 * 3", "(program (binary + 1 (binary * 2 3)))"),
        ("(1 + 2) * 3", "(program (binary * (binary + 1 2) 3))"),
        ("1 - 2 - 3", "(program (binary - (binary - 1 2) 3))"),
        ("2 ** 3 ** 2", "(program (binary ** 2 (binary ** 3 2)))"),
        ("-2 ** 2", "(program (unary - (binary ** 2 2)))"),
        ("2 ** -1", "(program (binary ** 2 (unary - 1)))"),
        ("-7 // 2", "(program (binary // (unary - 7) 2))"),
        ("7 % 3 / 2", "(program (binary / (binary % 7 3) 2))"),
        (
            "1 < 2 and 2 < 3",
            "(program (binary and (binary < 1 2) (binary < 2 3)))",
        ),
        ("not a or b", "(program (binary or (unary not a) b))"),
        ("not a == b", "(program (unary not (binary == a b)))"),
        ("a or b and c", "(program (binary or a (binary and b c)))"),
        ("1 == 1.0", "(program (binary == 1 1.0))"),
        ("true != false", "(program (binary != true false))"),
    ]);
}

#[test]
fn calls() {
    check(&[
        (
            "print(1 + 2 * 3)",
            "(program (call print (binary + 1 (binary * 2 3))))",
        ),
        ("print()", "(program (call print))"),
        ("print(1, 2,)", "(program (call print 1 2))"),
        ("f(1)(2)", "(program (call (call f 1) 2))"),
        ("print(\n  1,\n  2\n)", "(program (call print 1 2))"),
    ]);
}

#[test]
fn statements_and_newlines() {
    check(&[
        ("", "(program)"),
        ("\n\n", "(program)"),
        ("1\n2", "(program 1 2)"),
        ("1; 2;", "(program 1 2)"),
        ("1 +\n\n  2", "(program (binary + 1 2))"),
        ("true\n  and false", "(program (binary and true false))"),
        ("1\n-2", "(program 1 (unary - 2))"),
        ("f\n(1)", "(program f 1)"),
    ]);
}

#[test]
fn syntax_errors() {
    check_errs(&[
        ("1 < 2 < 3", "E0111"),
        ("a == b == c", "E0111"),
        ("1 +", "E0110"),
        ("(1", "E0112"),
        ("1 2", "E0110"),
        (")", "E0110"),
        ("print(1,,2)", "E0110"),
        ("1 ;; 2", "E0110"),
    ]);
}

#[test]
fn error_position_points_at_offending_token() {
    let d = parse(&toks_of("print(1 2)")).unwrap_err();
    assert_eq!((d.pos.line, d.pos.col), (1, 9), "err = {}", d.code);
}

#[test]
fn deep_nesting_is_a_fung_error_not_a_crash() {
    big_stack(|| {
        let deep = format!("{}1{}", "(".repeat(20_000), ")".repeat(20_000));
        assert_eq!(perr(&deep), "E0115");
        let fine = format!("{}1{}", "(".repeat(1000), ")".repeat(1000));
        assert_eq!(p(&fine), "(program 1)", "1000-deep nesting");
    });
}

#[test]
fn assignment_statements() {
    check(&[
        ("x = 1", "(program (assign x 1))"),
        (
            "x = 1\ny = x + 1",
            "(program (assign x 1) (assign y (binary + x 1)))",
        ),
        ("x =\n  1 + 2", "(program (assign x (binary + 1 2)))"),
    ]);
    assert_eq!(perr("x = y = 1"), "E0110", "chained assignment");
    assert_eq!(perr("x == 1 = 2"), "E0110", "assignment to expression");
}

#[test]
fn literals() {
    check(&[
        ("nil", "(program (nil))"),
        (r#""a b""#, r#"(program "a b")"#),
        ("`raw ${x}`", r#"(program "raw \${x}")"#),
        ("'ok", "(program 'ok)"),
        ("1h30m", "(program 1h30m)"),
        (
            r#""x=${x + 1}!""#,
            r#"(program (interp "x=" (binary + x 1) "!"))"#,
        ),
        (r#""${a}${b}""#, "(program (interp a b))"),
        (r#""${m}""#, "(program (interp m))"),
        (
            r#""${"in" + s}""#,
            r#"(program (interp (binary + "in" s)))"#,
        ),
    ]);
}

#[test]
fn interpolation_errors_keep_outer_positions() {
    let d = parse(&toks_of("x = 1\nprint(\"a ${1 +} b\")")).unwrap_err();
    assert_eq!(
        (d.code.as_str(), d.pos.line, d.pos.col),
        ("E0110", 2, 15),
        "want E0110 at 2:15 (the closing brace)"
    );
}

#[test]
fn then_else() {
    check(&[
        ("c then 1 else 2", "(program (then c 1 2))"),
        (
            "a then 1 else b then 2 else 3",
            "(program (then a 1 (then b 2 3)))",
        ),
        (
            "x = n > 0 then \"pos\" else \"neg\"",
            r#"(program (assign x (then (binary > n 0) "pos" "neg")))"#,
        ),
        ("c then 1\n  else 2", "(program (then c 1 2))"),
        (
            "c then a or b else d",
            "(program (then c (binary or a b) d))",
        ),
    ]);
    assert_eq!(perr("c then 1"), "E0112", "then without else");
}

#[test]
fn if_blocks() {
    check(&[
        ("if c: 1 ;;", "(program (if c (block 1) (nil)))"),
        ("if c: 1 else: 2 ;;", "(program (if c (block 1) (block 2)))"),
        (
            "if a: 1 else if b: 2 ;;",
            "(program (if a (block 1) (if b (block 2) (nil))))",
        ),
        (
            "if a: 1 else if b: 2 else: 3 ;;",
            "(program (if a (block 1) (if b (block 2) (block 3))))",
        ),
        (
            "if c:\n  x = 1\n  x\nelse:\n  2\n;;",
            "(program (if c (block (assign x 1) x) (block 2)))",
        ),
        ("if c: ;;", "(program (if c (block) (nil)))"),
        (
            "x = if c: 1 else: 2 ;;",
            "(program (assign x (if c (block 1) (block 2))))",
        ),
        (
            "print(if c:\n  1\nelse:\n  2\n;;)",
            "(program (call print (if c (block 1) (block 2))))",
        ),
        (
            "if c: if d: 1 ;; else: 2 ;;",
            "(program (if c (block (if d (block 1) (nil))) (block 2)))",
        ),
    ]);
    check_errs(&[
        ("if c 1 ;;", "E0112"),
        ("if c: 1", "E0112"),
        ("if c: 1 else 2 ;;", "E0112"),
        ("else: 1 ;;", "E0110"),
        (";;", "E0110"),
        ("if c: 1 ;; 2", "E0110"),
    ]);
}

#[test]
fn deeply_nested_blocks_hit_the_depth_limit() {
    big_stack(|| {
        let src = format!("{}1{}", "if true: ".repeat(20_000), " ;;".repeat(20_000));
        assert_eq!(perr(&src), "E0115");
    });
}

#[test]
fn min_int_literal() {
    assert_eq!(p("-9223372036854775808"), "(program -9223372036854775808)");
    assert_eq!(
        p("- 9_223_372_036_854_775_808"),
        "(program -9223372036854775808)",
        "with separators"
    );
    for src in [
        "9223372036854775808",
        "1 + 9223372036854775808",
        "-(9223372036854775808)",
    ] {
        assert_eq!(perr(src), "E0102", "{src:?}");
    }
    assert_eq!(
        p("-9223372036854775807"),
        "(program (unary - 9223372036854775807))",
        "ordinary negative"
    );
}

// --- functions ---

#[test]
fn fn_definitions() {
    check(&[
        (
            "fn add(a, b) = a + b",
            "(program (fn add(a, b) (block (binary + a b))))",
        ),
        ("fn f():\n  1\n  2\n;;", "(program (fn f() (block 1 2)))"),
        (
            "fn g(a, b = 1, c = a + b) = c",
            "(program (fn g(a, b=?, c=?) 1 (binary + a b) (block c)))",
        ),
        (
            "fn h(n: int, s: str | nil = nil) -> bool = true",
            "(program (fn h(n:int, s:str|nil=?)->bool (nil) (block true)))",
        ),
        (
            "fn k(xs: list) = xs",
            "(program (fn k(xs:list) (block xs)))",
        ),
        (
            "fn v(...rest) = rest",
            "(program (fn v(...rest) (block rest)))",
        ),
        (
            "fn t(s: Circle) = s",
            "(program (fn t(s:Circle) (block s)))",
        ),
        ("fn f(a,) = a", "(program (fn f(a) (block a)))"),
    ]);
}

#[test]
fn lambdas_and_anonymous_fns() {
    check(&[
        (
            "f = x => x * 2",
            "(program (assign f (fn (x) (block (binary * x 2)))))",
        ),
        (
            "f = (a, b) => a + b",
            "(program (assign f (fn (a, b) (block (binary + a b)))))",
        ),
        ("f = () => 1", "(program (assign f (fn () (block 1))))"),
        (
            "x => y => x + y",
            "(program (fn (x) (block (fn (y) (block (binary + x y))))))",
        ),
        (
            "x => c then 1 else 2",
            "(program (fn (x) (block (then c 1 2))))",
        ),
        (
            "g(xs, x => x + 1, 3)",
            "(program (call g xs (fn (x) (block (binary + x 1))) 3))",
        ),
        (
            "f = fn(a):\n  a\n;;",
            "(program (assign f (fn (a) (block a))))",
        ),
        (
            "f = fn(a) -> int:\n  a\n;;",
            "(program (assign f (fn (a)->int (block a))))",
        ),
        ("(1 + 2) * 3", "(program (binary * (binary + 1 2) 3))"),
        ("(x)", "(program x)"),
    ]);
}

#[test]
fn return_and_named_args() {
    check(&[
        ("fn f() = return 1", "(program (fn f() (block (return 1))))"),
        (
            "fn f():\n  if c: return ;;\n  2\n;;",
            "(program (fn f() (block (if c (block (return (nil))) (nil)) 2)))",
        ),
        (
            "serve(8080, dir: \"./pub\")",
            r#"(program (call serve 8080 (named dir "./pub")))"#,
        ),
        (
            "f(a: 1, b: 2)",
            "(program (call f (named a 1) (named b 2)))",
        ),
    ]);
    check_errs(&[
        ("f(a: 1, 2)", "E0126"),
        ("fn (a) = a", "E0124"),
        ("fn f(a = 1, b) = a", "E0120"),
        ("fn f(a: ) = a", "E0110"),
        ("fn f(a, a) = a", "E0119"),
        ("x =>", "E0110"),
        ("fn f() -> = 1", "E0110"),
        ("fn f(...a, b) = a", "E0112"),
    ]);
}

#[test]
fn lambda_params_require_commas() {
    assert_eq!(perr("(a b) => a + b"), "E0112");
}

#[test]
fn least_int_magnitude_needs_a_minus_in_patterns_too() {
    // 01: 9223372036854775808 is only valid right after `-`.
    assert_eq!(perr("match x: 9223372036854775808 => 1 ;;"), "E0102");
    assert_eq!(perr("[9223372036854775808] = xs"), "E0102");
    p("match x: -9223372036854775808 => 1 ;;");
}

#[test]
fn not_is_not_an_operand_of_tighter_operators() {
    // 01 EBNF: not_expr sits between `and` and comparison, so `not` cannot
    // be an operand of `==`, `+`, `??`, `..` or unary `-` without
    // parentheses (`1 == not 2 == 3` hid a comparison chain).
    for src in [
        "print(1 == not 2 == 3)",
        "x = 1 + not true",
        "x = a ?? not b",
        "x = a < not b",
        "x = -not b",
        "x = a |> not",
    ] {
        assert_eq!(perr(src), "E0110", "{src}");
    }
    for src in [
        "x = a and not b",
        "x = not not a",
        "f(not a)",
        "x = a == (not b)",
        "x = [not a]",
        "g = a => not a",
        "x = c then not a else b",
        "x = not a == b",
        "x = a or not b and c",
    ] {
        p(src);
    }
}

#[test]
fn duplicate_names_across_parameters() {
    // A name bound by two parameters, plain or inside a pattern, is the
    // same error as `fn f(a, a)`.
    check_errs(&[
        ("(a, a) => a", "E0119"),
        ("fn g(a, (a, b)) = a", "E0119"),
        ("fn g((a, b), (a, c)) = a", "E0119"),
        ("fn g([x, ...r], r) = x", "E0119"),
        ("((a, b), [b]) => a", "E0119"),
    ]);
    // One pattern binding a name twice is the resolver's E0221.
    p("((a, a)) => a");
    p("fn g((a, _), (_, b)) = a");
}

// --- collections and patterns (M4) ---

#[test]
fn collection_literals() {
    check(&[
        ("[1, 2, 3]", "(program (list 1 2 3))"),
        ("[]", "(program (list))"),
        ("[\n  1,\n  2,\n]", "(program (list 1 2))"),
        (
            "{name: \"a\", tags: [1]}",
            r#"(program (map (entry name "a") (entry tags (list 1))))"#,
        ),
        ("{\"a b\": 1}", "(program (map (entry a b 1)))"),
        ("{}", "(program (map))"),
        ("()", "(program (tuple))"),
        ("(1,)", "(program (tuple 1))"),
        ("(1, 2)", "(program (tuple 1 2))"),
        ("(1)", "(program 1)"),
        ("Circle(3)", "(program (tag Circle 3))"),
        ("Rect(2, 5)", "(program (tag Rect 2 5))"),
        ("Red", "(program Red)"),
        ("Ok(x) + 1", "(program (binary + (tag Ok x) 1))"),
    ]);
}

#[test]
fn newlines_inside_brackets_are_whitespace_everywhere() {
    // 01: inside ( ) [ ] { } a newline is whitespace — in patterns and
    // lambda parameter lists too, and a tag's `(` may follow on the next
    // line there.
    check(&[
        (
            "match r:\n  Ok(\n    y\n  ) => y\n;;",
            "(program (match r (arm (ptag Ok (pbind y)) (nil) (block y))))",
        ),
        (
            "[a,\n b] = xs",
            "(program (assign (plist (pbind a) (pbind b)) xs))",
        ),
        (
            "{name,\n port} = cfg",
            "(program (assign (pmap (pentry name (pbind name)) (pentry port (pbind port))) cfg))",
        ),
        (
            "fold(xs, 0, (acc,\n  x) => acc + x)",
            "(program (call fold xs 0 (fn (acc, x) (block (binary + acc x)))))",
        ),
        ("f = (\n) => 1", "(program (assign f (fn () (block 1))))"),
        (
            "g = ([a,\n b]) => a",
            "(program (assign g (fn ((plist (pbind a) (pbind b))) (block a))))",
        ),
        (
            "map(xs, x\n  => x)",
            "(program (call map xs (fn (x) (block x))))",
        ),
        ("x = [Ok\n  (1)]", "(program (assign x (list (tag Ok 1))))"),
        (
            "match r: [Ok\n (v)] => v ;;",
            "(program (match r (arm (plist (ptag Ok (pbind v))) (nil) (block v))))",
        ),
    ]);
    assert_eq!(
        p("import fs (\n  read,\n  write\n)"),
        p("import fs (read, write)")
    );
    // At the top level a newline still ends the tag: `Ok` alone is a value.
    check(&[("x = Ok\n(1)", "(program (assign x Ok) 1)")]);
}

#[test]
fn a_line_ending_in_dot_arrow_or_alias_equals_continues() {
    // 01: a line ending in `.`, `->` or `=` continues on the next.
    check(&[
        ("x = m.\n  a", "(program (assign x (field a m)))"),
        ("fn f() ->\n  int = 1", "(program (fn f()->int (block 1)))"),
        ("type t =\n  int | str", "(program (typealias t))"),
    ]);
}

#[test]
fn nested_indexing_parses_in_linear_time() {
    // Each `[` used to parse its contents twice (once looking for `a..]`),
    // so the work doubled with every level: 26 levels took seconds.
    let depth = 40;
    let src = format!("y = {}0{}", "x[".repeat(depth), "]".repeat(depth));
    let t = std::time::Instant::now();
    p(&src);
    assert!(
        t.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        t.elapsed()
    );
    // The forms the second parse used to cover still read the same.
    check(&[
        (
            "xs[a + 1..]",
            "(program (slice xs (binary + a 1) (missing)))",
        ),
        ("xs[a..b + 1]", "(program (slice xs a (binary + b 1)))"),
        // an index is a whole expression; a slice's end is not (D36)
        ("xs[a ?? 0]", "(program (index xs (coalesce a 0)))"),
        (
            "xs[f(x)[0]..]",
            "(program (slice xs (index (call f x) 0) (missing)))",
        ),
    ]);
}

#[test]
fn a_slice_is_a_range_directly_inside_the_brackets() {
    // FE27, D7: only a `..` at the top level of `[ ]` makes a slice, and
    // both of its ends are full expressions; a range in parentheses or
    // deeper is a range value used as an index.
    check(&[
        ("xs[(0..2)]", "(program (index xs (range .. 0 2)))"),
        (
            "xs[f(0..2)]",
            "(program (index xs (call f (range .. 0 2))))",
        ),
        (
            "xs[[0..2][0]]",
            "(program (index xs (index (list (range .. 0 2)) 0)))",
        ),
        ("xs[(a ?? 1)..2]", "(program (slice xs (coalesce a 1) 2))"),
        (
            "xs[(a ?? 1)..=(b ?? 2)]",
            "(program (slice ..= xs (coalesce a 1) (coalesce b 2)))",
        ),
        (
            "xs[..(b |> f)]",
            "(program (slice xs (missing) (pipe b f)))",
        ),
        (
            "xs[ys[0..1]..]",
            "(program (slice xs (slice ys 0 1) (missing)))",
        ),
        (
            "xs[match y: _ => f(0..2) ;;]",
            "(program (index xs (match y (arm (pwild) (nil) (block (call f (range .. 0 2)))))))",
        ),
        (
            "xs[match y: _ => 0..2 ;;]",
            "(program (index xs (match y (arm (pwild) (nil) (block (range .. 0 2))))))",
        ),
    ]);
    check_errs(&[
        ("xs[0..1..2]", "E0118"),
        ("xs[0..1..]", "E0118"),
        ("xs[..1..2]", "E0118"),
        ("xs[0..=1..2]", "E0118"),
    ]);
}

#[test]
fn only_a_dot_dot_directly_inside_the_brackets_is_the_slice() {
    // An `if` condition, a `match` subject, a `for` iterable and the
    // branch between `then` and `else` are nested expressions: their
    // ranges stay ranges.
    for src in [
        "xs[if i in 0..3: 0 else: 1 ;;]",
        "xs[match 0..2: _ => 1 ;;]",
        "xs[for x in 0..2: x ;; |> len]",
        "xs[c then 0..1 else 2]",
    ] {
        let n = parse(&toks_of(src)).unwrap_or_else(|d| panic!("{src}: {}", d.code));
        assert_eq!(n.kids[0].head, Head::Index, "{src}: {n}");
    }
}

#[test]
fn a_slice_end_cannot_hide_a_looser_operator() {
    // A comparison, `in`, `not`, `and`/`or` or `then … else` at the top of
    // a slice's end binds looser than `..` would elsewhere; the meaning
    // would change silently, so it needs parentheses (E0125).
    check_errs(&[
        ("xs[c then 0 else 1..2]", "E0125"),
        ("xs[1 in 0..3 then 0 else 1]", "E0125"),
        ("xs[a < b..]", "E0125"),
        ("xs[..a or b]", "E0125"),
        ("xs[0..=a and b]", "E0125"),
        ("xs[not a..]", "E0125"),
        ("xs[a not in b..]", "E0125"),
        // `??` and `|>` bind looser than `..` too (D36)
        ("xs[a ?? 0..3]", "E0125"),
        ("xs[a ?? 1..=b ?? 2]", "E0125"),
        ("xs[..b |> f]", "E0125"),
        ("xs[..=b ?? 2]", "E0125"),
        ("xs[n |> half..]", "E0125"),
    ]);
    check(&[
        (
            "xs[(c then 0 else 1)..2]",
            "(program (slice xs (then c 0 1) 2))",
        ),
        (
            "xs[(a ?? 0)..(b |> f)]",
            "(program (slice xs (coalesce a 0) (pipe b f)))",
        ),
    ]);
}

#[test]
fn parentheses_stay_marked_in_the_tree() {
    // FE27, D7: the pipe needs to tell `x |> f(1)` from `x |> (f(1))`.
    for (src, paren) in [("(f(1))", true), ("f(1)", false), ("((x))", true)] {
        let n = parse(&toks_of(src)).unwrap();
        assert_eq!(n.kids[0].paren, paren, "{src}");
    }
    let n = parse(&toks_of("x |> (f(1))")).unwrap();
    assert!(n.kids[0].kids[1].paren);
    assert!(!n.kids[0].kids[0].paren);
}

#[test]
fn assignment_for_and_parameters_take_one_destructuring_pattern() {
    // FE32, D10: a name, `_`, tuple, list, map or tag with values, or
    // alternatives of them; literals only inside.
    check(&[
        (
            "Ok(x) | Err(x) = r",
            "(program (assign (palt (ptag Ok (pbind x)) (ptag Err (pbind x))) r))",
        ),
        (
            "(1, y) = p",
            "(program (assign (ptuple (plit 1) (pbind y)) p))",
        ),
        (
            "for (k, 1) in xs: k ;;",
            "(program (for (ptuple (pbind k) (plit 1)) xs (block k)))",
        ),
    ]);
    check_errs(&[
        ("1 = 1", "E0112"),
        ("'a = 'a", "E0112"),
        ("nil = nil", "E0112"),
        ("\"a\" | \"b\" = \"a\"", "E0112"),
        ("Ok(x) | 1 = r", "E0112"),
        ("-1 = x", "E0112"),
        ("for 1 in xs: 1 ;;", "E0112"),
        ("for Red in xs: 1 ;;", "E0112"),
        ("for x | 1 in xs: 1 ;;", "E0112"),
        ("fn f(Red) = 1", "E0112"),
        ("fn f(Ok(x) | 2) = 1", "E0112"),
        ("f = (1) => 1", "E0112"),
        ("f = ('a) => 1", "E0112"),
    ]);
}

#[test]
fn match_takes_negative_durations() {
    check(&[(
        "match d: -1s => a; -1m30s => b ;;",
        "(program (match d (arm (plit -1s) (nil) (block a)) (arm (plit -1m30s) (nil) (block b))))",
    )]);
}

#[test]
fn nested_guards_parse_in_linear_time() {
    // An arm head was parsed once to decide it is an arm and again for
    // real, so a match in a guard doubled the work per level.
    let mut e = "1".to_string();
    for _ in 0..30 {
        e = format!("(match 1: v when ({e}) > 0 => v ;;)");
    }
    let t = std::time::Instant::now();
    p(&format!("y = {e}"));
    assert!(
        t.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        t.elapsed()
    );
}

#[test]
fn arm_head_errors_point_at_the_mistake() {
    // A guard (`when` only appears in arm heads) is reported where it
    // breaks, in the first arm or a later one.
    assert_eq!(
        ast("x = match 3:\n  n when n >> 2 => 1\n  _ => 2\n;;"),
        "E0110 2:13"
    );
    assert_eq!(
        ast("x = match 3:\n  1 => 1\n  n when n >> 2 => 1\n;;"),
        "E0110 3:13"
    );
    // A broken first pattern is reported inside the pattern.
    assert_eq!(ast("x = match 3:\n  Ok(y => 1\n;;"), "E0112 2:8");
}

#[test]
fn a_block_inside_a_guard_allows_bare_lambdas() {
    // 01: in a guard a lambda goes inside parentheses; a block opened
    // there is ordinary code again.
    p(
        "r = match 1:\n  x when any([1], fn(z):\n    f = y => y == 1\n    f(z)\n  ;;) => \"ok\"\n  _ => \"no\"\n;;",
    );
    p("r = match 1:\n  x when (match x: y => (z => z)(y) ;;) == 1 => 1\n  _ => 2\n;;");
}

#[test]
fn postfix_and_new_infix() {
    check(&[
        ("xs[0]", "(program (index xs 0))"),
        ("xs[-1]", "(program (index xs (unary - 1)))"),
        ("xs[1..3]", "(program (slice xs 1 3))"),
        ("xs[..3]", "(program (slice xs (missing) 3))"),
        ("xs[-4..]", "(program (slice xs (unary - 4) (missing)))"),
        ("xs[1..=3]", "(program (slice ..= xs 1 3))"),
        ("xs[i ?? 0]", "(program (index xs (coalesce i 0)))"),
        ("xs[c then 0 else 1]", "(program (index xs (then c 0 1)))"),
        (
            "xs[(i ?? 0)..]",
            "(program (slice xs (coalesce i 0) (missing)))",
        ),
        ("m.name.first", "(program (field first (field name m)))"),
        ("f(1)!", "(program (unwrap ! (call f 1)))"),
        ("f(1)?", "(program (unwrap ? (call f 1)))"),
        ("a ?? b ?? c", "(program (coalesce (coalesce a b) c))"),
        (
            "xs |> map(f) |> sum",
            "(program (pipe (pipe xs (call map f)) sum))",
        ),
        ("0..10", "(program (range .. 0 10))"),
        ("0..=10", "(program (range ..= 0 10))"),
        ("x in xs", "(program (binary in x xs))"),
        ("x not in xs", "(program (binary not_in x xs))"),
        (
            "files |> len > 3",
            "(program (binary > (pipe files len) 3))",
        ),
        ("a + b |> f", "(program (pipe (binary + a b) f))"),
        (
            "(fs.read(p) ?? \"{}\") |> json.parse",
            r#"(program (pipe (coalesce (call (field read fs) p) "{}") (field parse json)))"#,
        ),
        ("xs\n  |> map(f)", "(program (pipe xs (call map f)))"),
        ("m\n  .name", "(program (field name m))"),
        ("f(_)", "(program (call f (hole)))"),
        (
            "path.join(dir, _)",
            "(program (call (field join path) dir (hole)))",
        ),
    ]);
    let n = parse(&toks_of("0..=10")).unwrap();
    assert!(
        matches!(&n.kids[0].lit, Lit::Str(s) if s == "..="),
        "..= should record the inclusive operator"
    );
    let n = parse(&toks_of("f()?")).unwrap();
    assert!(
        matches!(&n.kids[0].lit, Lit::Str(s) if s == "?"),
        "unwrap lit"
    );
    for src in ["x in xs in ys", "a < b in c"] {
        let got = perr(src);
        assert!(got == "E0111" || got == "E0110", "{src:?}: {got}");
    }
}

#[test]
fn patterns_and_match() {
    check(&[
        (
            "match v:\n  0 => a\n  _ => b\n;;",
            "(program (match v (arm (plit 0) (nil) (block a)) (arm (pwild) (nil) (block b))))",
        ),
        (
            "match v: 'ok => 1; Ok(x) => x; Err(e) => 0 ;;",
            "(program (match v (arm (psym ok) (nil) (block 1)) (arm (ptag Ok (pbind x)) (nil) (block x)) (arm (ptag Err (pbind e)) (nil) (block 0))))",
        ),
        (
            "match p: (0, 0) => a; (x, 0) => b ;;",
            "(program (match p (arm (ptuple (plit 0) (plit 0)) (nil) (block a)) (arm (ptuple (pbind x) (plit 0)) (nil) (block b))))",
        ),
        (
            "match xs: [] => 0; [first, ...rest] => 1; [a, ...] => 2 ;;",
            "(program (match xs (arm (plist) (nil) (block 0)) (arm (plist (pbind first) (prest rest)) (nil) (block 1)) (arm (plist (pbind a) (prest)) (nil) (block 2))))",
        ),
        (
            "match m: {name: n, port: 80} => n ;;",
            "(program (match m (arm (pmap (pentry name (pbind n)) (pentry port (plit 80))) (nil) (block n))))",
        ),
        (
            "match x: n when n > 100 => big; 'a | 'b => ab ;;",
            "(program (match x (arm (pbind n) (binary > n 100) (block big)) (arm (palt (psym a) (psym b)) (nil) (block ab))))",
        ),
        (
            "match v: -1 => neg; \"s\" => s; 1.5 => f; nil => z; true => t; 1h => d ;;",
            r#"(program (match v (arm (plit -1) (nil) (block neg)) (arm (plit "s") (nil) (block s)) (arm (plit 1.5) (nil) (block f)) (arm (pnil) (nil) (block z)) (arm (plit true) (nil) (block t)) (arm (plit 1h) (nil) (block d))))"#,
        ),
        (
            "match r:\n  Ok(text) =>\n    lines = f(text)\n    len(lines)\n  Err(e) => print(e); 0\n;;",
            "(program (match r (arm (ptag Ok (pbind text)) (nil) (block (assign lines (call f text)) (call len lines))) (arm (ptag Err (pbind e)) (nil) (block (call print e) 0))))",
        ),
        (
            "match v: Red => 1 ;;",
            "(program (match v (arm (psym Red) (nil) (block 1))))",
        ),
        (
            "match v: y when y == x => 1; _ => 2 ;;",
            "(program (match v (arm (pbind y) (binary == y x) (block 1)) (arm (pwild) (nil) (block 2))))",
        ),
        (
            "match v: y when any(xs, x => x) => 1 ;;",
            "(program (match v (arm (pbind y) (call any xs (fn (x) (block x))) (block 1))))",
        ),
    ]);
    check_errs(&[
        ("match v: (x) => 1 ;;", "E0112"),
        ("match v: [...a, b] => 1 ;;", "E0110"),
        ("match v: 1 + 2 => 1 ;;", "E0110"),
        ("match v: ;;", "E0112"),
        ("match v: x => ;;", "E0112"),
    ]);
}

#[test]
fn for_type_and_destructuring() {
    check(&[
        (
            "for x in xs: print(x) ;;",
            "(program (for (pbind x) xs (block (call print x))))",
        ),
        (
            "for (k, v) in pairs(m): k ;;",
            "(program (for (ptuple (pbind k) (pbind v)) (call pairs m) (block k)))",
        ),
        (
            "for i in 0..10: i ;;",
            "(program (for (pbind i) (range .. 0 10) (block i)))",
        ),
        ("type shape = Circle | Rect", "(program (typealias shape))"),
        (
            "(q, r) = divmod(7, 2)",
            "(program (assign (ptuple (pbind q) (pbind r)) (call divmod 7 2)))",
        ),
        (
            "[first, ...rest] = files",
            "(program (assign (plist (pbind first) (prest rest)) files))",
        ),
        (
            "{name: n, port: p} = config",
            "(program (assign (pmap (pentry name (pbind n)) (pentry port (pbind p))) config))",
        ),
        (
            "Ok(text) = fs.read(\"a\")",
            r#"(program (assign (ptag Ok (pbind text)) (call (field read fs) "a")))"#,
        ),
        ("x = 1", "(program (assign x 1))"),
        ("(a, b)", "(program (tuple a b))"),
    ]);
    assert_eq!(perr("for x xs: 1 ;;"), "E0112", "for without in");
    assert_eq!(
        perr("type Shape = Circle"),
        "E0112",
        "alias must be lowercase"
    );
}

#[test]
fn tag_rejects_named_args() {
    assert_eq!(perr("Ok(a: 1)"), "E0121");
}

#[test]
fn deep_pattern_is_bounded() {
    big_stack(|| {
        let src = format!("{}x{} = 1", "[".repeat(20_000), "]".repeat(20_000));
        assert_eq!(perr_quiet(&src), "E0115", "deep pattern");
        let src = format!(
            "match v: {}x{} => 1 ;;",
            "Ok(".repeat(20_000),
            ")".repeat(20_000)
        );
        assert_eq!(perr_quiet(&src), "E0115", "deep pattern in match");
    });
}

// --- modules (M5) ---

#[test]
fn import_statement() {
    check(&[
        ("import fs", "(program (import fs))"),
        (
            "import \"./lib/rename\"",
            r#"(program (import "./lib/rename"))"#,
        ),
        (
            "import \"./lib/rename\" as rn",
            r#"(program (import "./lib/rename" as rn))"#,
        ),
        (
            "import \"./lib/rename\" (slugify, kebab)",
            r#"(program (import "./lib/rename" (slugify, kebab)))"#,
        ),
        (
            "import \"./x\" as y (a)",
            r#"(program (import "./x" as y (a)))"#,
        ),
        (
            "import fs\nprint(fs)",
            "(program (import fs) (call print fs))",
        ),
        (
            "import _native.fs as _n",
            "(program (import _native.fs as _n))",
        ),
    ]);
    check_errs(&[
        ("import", "E0112"),
        ("import 1", "E0112"),
        ("import fs as", "E0112"),
        ("import fs ()", "E0112"),
        ("import \"x\" as Fs", "E0112"),
        ("import \"${x}\"", "E0112"),
    ]);
}

/// A tag without arguments is its symbol (02), so the pattern `Red()`
/// would match a second value that looks like `Red`: it is a syntax error
/// (E0117; the expression `Red()` is refused by `desugar`, after pipes).
#[test]
fn a_tag_pattern_with_empty_parentheses_is_an_error() {
    assert_eq!(perr("match x: Red() => 1 ;;"), "E0117");
    assert_eq!(perr("match x: Ok(Red()) => 1 ;;"), "E0117");
    assert_eq!(p("x = Red"), "(program (assign x Red))");
}

/// A result holds exactly one value (02, SP12): `Ok`/`Err` patterns with
/// another count could never match, so they are E0236.
#[test]
fn a_result_pattern_takes_exactly_one_value() {
    assert_eq!(perr("match x: Ok(a, b) => 1 ;;"), "E0236");
    assert_eq!(perr("match x: Err(a, b, c) => 1 ;;"), "E0236");
    assert_eq!(perr("match x: Pair(Ok(a, _)) => 1 ;;"), "E0236");
    assert_eq!(perr("Ok(a, b) = x"), "E0236");
    assert!(p("match x: Ok(a) => 1; Pair(a, b) => 2 ;;").starts_with("(program"));
}

/// A name given twice in one call is caught before running, as a map
/// literal's key is (E0116; a positional and a named argument for the same
/// parameter is still E0405 at the call, which knows the parameters).
#[test]
fn call_rejects_a_named_argument_given_twice() {
    assert_eq!(perr("f(a: 1, a: 2)"), "E0116");
    assert_eq!(perr("f(1, a: 1, b: 2, a: 3)"), "E0116");
    assert_eq!(
        p("f(a: 1, b: 2)"),
        "(program (call f (named a 1) (named b 2)))"
    );
}

#[test]
fn map_literal_rejects_duplicate_keys() {
    assert_eq!(perr("{a: 1, a: 2}"), "E0113", "dup key");
    assert_eq!(perr("{a: 1, \"a\": 2}"), "E0113", "dup quoted key");
    assert_eq!(
        p("{a: 1, b: 1}"),
        "(program (map (entry a 1) (entry b 1)))",
        "distinct keys"
    );
}

#[test]
fn underscore_assignment_discards() {
    assert_eq!(p("_ = f()"), "(program (assign (pwild) (call f)))");
}

// --- tests and asserts (M6) ---

#[test]
fn test_and_assert_statements() {
    check(&[
        (
            "test \"spaces\":\n  assert f(1) == 2\n;;",
            r#"(program (test "spaces" (block (assert (binary == (call f 1) 2)))))"#,
        ),
        (
            "test \"x\": assert true ;;",
            r#"(program (test "x" (block (assert true))))"#,
        ),
        ("assert x", "(program (assert x))"),
        (
            "fn f():\n  assert x > 0\n  x\n;;",
            "(program (fn f() (block (assert (binary > x 0)) x)))",
        ),
    ]);
    check_errs(&[
        ("test:", "E0112"),
        ("test \"a\" 1", "E0112"),
        ("test \"${x}\": 1 ;;", "E0112"),
        ("assert", "E0110"),
        ("fn f(): test \"a\": 1 ;; ;;", "E0114"),
        ("if true: test \"a\": 1 ;; ;;", "E0114"),
    ]);
}

#[test]
fn expression_spans() {
    let src = "y = 참치(1, 2) |> f\nassert 참치(1) == 2\nz = (1 + 2) * xs[0]";
    let prog = parse(&toks_of(src)).unwrap();
    let assign = &prog.kids[0];
    assert_eq!(assign.kids[1].text(src), "참치(1, 2) |> f", "rhs span");
    assert_eq!(assign.kids[1].kids[0].text(src), "참치(1, 2)", "call span");
    assert_eq!(assign.text(src), "y = 참치(1, 2) |> f", "assignment span");
    let cmp = &prog.kids[1].kids[0];
    assert_eq!(cmp.text(src), "참치(1) == 2");
    assert_eq!(cmp.kids[0].text(src), "참치(1)");
    assert_eq!(cmp.kids[1].text(src), "2");
    assert_eq!(
        prog.kids[1].text(src),
        "assert 참치(1) == 2",
        "statement span"
    );
    let mul = &prog.kids[2].kids[1];
    assert_eq!(mul.text(src), "(1 + 2) * xs[0]");
    assert_eq!(mul.kids[0].text(src), "(1 + 2)");
    assert_eq!(mul.kids[1].text(src), "xs[0]");
}

#[test]
fn identifier_shaped_keys_ignore_case() {
    check(&[
        (
            "{DB_HOST: 1, name: 2}",
            "(program (map (entry DB_HOST 1) (entry name 2)))",
        ),
        ("m.DB_HOST", "(program (field DB_HOST m))"),
        (
            "match m: {DB_HOST: v} => v ;;",
            "(program (match m (arm (pmap (pentry DB_HOST (pbind v))) (nil) (block v))))",
        ),
        ("{Ok: 1}", "(program (map (entry Ok 1)))"),
    ]);
}

#[test]
fn parameter_patterns() {
    check(&[
        (
            "((a, b)) => a",
            "(program (fn ((ptuple (pbind a) (pbind b))) (block a)))",
        ),
        ("(a, b) => a", "(program (fn (a, b) (block a)))"),
        ("_ => 0", "(program (fn ((pwild)) (block 0)))"),
        (
            "(Ok(x), {name: n}) => x",
            "(program (fn ((ptag Ok (pbind x)), (pmap (pentry name (pbind n)))) (block x)))",
        ),
        (
            "fn f((a, b), c) = a",
            "(program (fn f((ptuple (pbind a) (pbind b)), c) (block a)))",
        ),
        (
            "fn f([x, ...rest], _) = x",
            "(program (fn f((plist (pbind x) (prest rest)), (pwild)) (block x)))",
        ),
        (
            "fn f(_, y = 1) = y",
            "(program (fn f((pwild), y=?) 1 (block y)))",
        ),
    ]);
    check_errs(&[
        ("fn f((a, b) = 1) = a", "E0112"),
        ("fn f((a, b): int) = a", "E0112"),
        ("(1 + 2) => 3", "E0112"),
    ]);
}

// --- keywords as names ---

#[test]
fn keywords_as_field_and_key_names() {
    check(&[
        ("m.type", "(program (field type m))"),
        ("m.match.if", "(program (field if (field match m)))"),
        (
            "{type: 1, in: 2}",
            "(program (map (entry type 1) (entry in 2)))",
        ),
        (
            "{type: t, nil: n} = m",
            "(program (assign (pmap (pentry type (pbind t)) (pentry nil (pbind n))) m))",
        ),
        (
            "match m: {test: x} => x ;;",
            "(program (match m (arm (pmap (pentry test (pbind x))) (nil) (block x))))",
        ),
    ]);
    for src in ["type = 1", "fn type() = 1", "{type} = m"] {
        assert_ne!(perr_quiet(src), "", "{src} must not parse");
    }
}

// --- map shorthand ---

#[test]
fn map_shorthand() {
    check(&[
        (
            "{a, b: 2, c}",
            "(program (map (entry a a) (entry b 2) (entry c c)))",
        ),
        ("{DB_HOST: 1}", "(program (map (entry DB_HOST 1)))"),
        (
            "{HOME: h} = m",
            "(program (assign (pmap (pentry HOME (pbind h))) m))",
        ),
        ("Ok(x) = r", "(program (assign (ptag Ok (pbind x)) r))"),
        (
            "{line, col} = e",
            "(program (assign (pmap (pentry line (pbind line)) (pentry col (pbind col))) e))",
        ),
        (
            "match e: {kind, n: 1} => kind ;;",
            "(program (match e (arm (pmap (pentry kind (pbind kind)) (pentry n (plit 1))) (nil) (block kind))))",
        ),
    ]);
    for src in ["{type}", "{\"a\"}", "{type} = m", "{a, a}"] {
        assert_ne!(perr_quiet(src), "", "{src} must not parse");
    }
    // An uppercase name is a tag: it names no variable (01, L4).
    for src in [
        "{DB_HOST}",
        "{a, Red}",
        "{HOME} = m",
        "{a, PORT} = m",
        "MAX = 3",
        "Red = Red",
    ] {
        assert_eq!(perr(src), "E0225", "{src}");
    }
}

// --- the 2026-11-08 review (FE6–FE11, FE15) ---

/// Inside brackets a newline is whitespace before `]` after an open
/// slice's `..`, between `not` and `in`, and between a named argument's
/// name and its `:` (01, FE6).
#[test]
fn newlines_inside_brackets_around_two_token_forms() {
    check(&[
        ("xs[1..\n]", "(program (slice xs 1 (missing)))"),
        (
            "print(1 not\n in xs)",
            "(program (call print (binary not_in 1 xs)))",
        ),
        ("f(a\n : 1)", "(program (call f (named a 1)))"),
    ]);
}

/// A line ending in `in` (a for head) or `|` (pattern alternatives, a
/// type union) continues (01 rule 1, FE7).
#[test]
fn a_line_ending_in_in_or_bar_continues() {
    check(&[
        (
            "for x in\n  xs:\n  x\n;;",
            "(program (for (pbind x) xs (block x)))",
        ),
        ("type t = int |\n  str", "(program (typealias t))"),
        (
            "match r:\n  Ok(x) |\n  Err(x) => x\n;;",
            "(program (match r (arm (palt (ptag Ok (pbind x)) (ptag Err (pbind x))) (nil) (block x))))",
        ),
        (
            "fn f(x: int |\n str) = x",
            "(program (fn f(x:int|str) (block x)))",
        ),
    ]);
}

/// `..` is not associative (01 EBNF, FE8).
#[test]
fn a_range_is_not_an_operand_of_a_range() {
    check_errs(&[
        ("x = 1..2..3", "E0118"),
        ("x = 1..2..=3", "E0118"),
        ("x = xs[0..1..]", "E0118"),
        ("x = xs[0..1..2]", "E0118"),
    ]);
    p("x = (1..2)");
    p("x = xs[0..1]");
}

/// The least int folds only when the literal is the whole operand of the
/// minus: `-a ** b` is `-(a ** b)` (01, FE9).
#[test]
fn least_int_folds_only_as_a_whole_operand() {
    for src in [
        "-9223372036854775808 ** 1",
        "-9223372036854775808[0]",
        "-9223372036854775808(1)",
        "-9223372036854775808!",
        "-9223372036854775808?",
    ] {
        assert_eq!(perr(src), "E0135", "{src:?}");
    }
    assert_eq!(
        p("-9223372036854775808 + 1"),
        "(program (binary + -9223372036854775808 1))"
    );
}

/// A test name and an import path are text: bytes that are not UTF-8
/// are refused, not dropped (FE10).
#[test]
fn test_names_and_import_paths_are_text() {
    assert_eq!(ast("test \"a\\xffb\": assert true ;;"), "E0112 1:6");
    assert_eq!(ast("import \"./x\\xff\""), "E0112 1:8");
    assert_eq!(
        p("test \"caf\\xc3\\xa9\": assert true ;;"),
        "(program (test \"café\" (block (assert true))))"
    );
}

/// `..._` is the same as `...`: it binds no name (FE11).
#[test]
fn rest_wildcard_binds_nothing() {
    check(&[
        (
            "[a, ..._] = xs",
            "(program (assign (plist (pbind a) (prest)) xs))",
        ),
        (
            "fn f([a, ..._], [b, ..._]) = a",
            "(program (fn f((plist (pbind a) (prest)), (plist (pbind b) (prest))) (block a)))",
        ),
    ]);
}

/// Forms the grammar reads and then refuses carry their own codes, not
/// "unexpected <sentence>" (FE15).
#[test]
fn refused_forms_have_their_own_codes() {
    check_errs(&[
        ("x = {\"${1}\": 2}", "E0122"),
        ("match s: \"${x}\" => 1 ;;", "E0122"),
        ("{\"${k}\": v} = m", "E0122"),
        ("f(a: _)", "E0123"),
        ("f = fn(x) = x", "E0124"),
        ("fn f(a = 1, (b, c)) = a", "E0120"),
    ]);
}

/// Deep nesting reaches E0115 in time linear in the input (FE16): the
/// lambda look-ahead uses a table of bracket pairs, and the code of a
/// `${…}` is lexed once, with the string, not again per level.
#[test]
fn deep_parentheses_and_interpolations_are_linear() {
    let parens = format!("x = {}1{}", "(".repeat(100_000), ")".repeat(100_000));
    let mut interp = "1".to_string();
    for _ in 0..100_000 {
        interp = format!("\"${{{interp}}}\"");
    }
    let interp = format!("x = {interp}");
    for src in [parens, interp] {
        let t = std::time::Instant::now();
        let got = big_stack(move || ast(&src));
        assert!(got.starts_with("E0115 "), "{got}");
        assert!(
            t.elapsed() < std::time::Duration::from_secs(3),
            "{:?}",
            t.elapsed()
        );
    }
    // Below the limit the interpolations still nest.
    assert_eq!(
        p("x = \"a${\"b${1}\"}\""),
        "(program (assign x (interp \"a\" (interp \"b\" 1))))"
    );
}

/// The parser continues a line after every token of the one list the
/// formatter also reads (`Kind::ends_open`, 01 rule 1, FE22).
#[test]
fn the_parser_continues_after_every_open_line_end() {
    let cases: &[(Kind, &str)] = &[
        (Kind::Plus, "x = 1 +\n2"),
        (Kind::Minus, "x = 1 -\n2"),
        (Kind::Star, "x = 1 *\n2"),
        (Kind::Slash, "x = 1 /\n2"),
        (Kind::SlashSlash, "x = 1 //\n2"),
        (Kind::Percent, "x = 1 %\n2"),
        (Kind::StarStar, "x = 1 **\n2"),
        (Kind::Eq, "x = 1 ==\n2"),
        (Kind::Ne, "x = 1 !=\n2"),
        (Kind::Lt, "x = 1 <\n2"),
        (Kind::Gt, "x = 1 >\n2"),
        (Kind::Le, "x = 1 <=\n2"),
        (Kind::Ge, "x = 1 >=\n2"),
        (Kind::KwIn, "for x in\nxs: x ;;"),
        (Kind::Coalesce, "x = a ??\n2"),
        (Kind::Pipe, "x = a |>\nf"),
        (Kind::DotDot, "x = 1..\n2"),
        (Kind::DotDotEq, "x = 1..=\n2"),
        (Kind::KwAnd, "x = a and\nb"),
        (Kind::KwOr, "x = a or\nb"),
        (Kind::KwNot, "x = not\nb"),
        (Kind::Assign, "x =\n2"),
        (Kind::FatArrow, "f = x =>\nx"),
        (Kind::Arrow, "fn f() ->\nint = 1"),
        (Kind::Dot, "x = m.\na"),
        (Kind::KwThen, "x = c then\n1 else 2"),
        (Kind::KwElse, "x = c then 1 else\n2"),
        (Kind::Bar, "type t = int |\nstr"),
    ];
    for (k, src) in cases {
        assert!(k.ends_open(), "{k:?}");
        let toks = toks_of(src);
        let got = parse(&toks).map(|n| n.kids.len());
        assert_eq!(got.ok(), Some(1), "{src:?}");
    }
    for k in [
        Kind::Name,
        Kind::Int,
        Kind::RParen,
        Kind::DoubleSemi,
        Kind::Comma,
    ] {
        assert!(!k.ends_open(), "{k:?}");
    }
}

#[test]
fn lambda_and_fn_parameters_take_the_same_patterns() {
    // SP8: a parameter is a name or a destructuring pattern (`_`, tuple,
    // list, map, tag) in both `fn` and a lambda; a literal is not.
    for src in [
        "f = (1) => 0",
        "f = ('a) => 0",
        "f = (\"a\") => 0",
        "f = (-1) => 0",
    ] {
        assert_eq!(perr(src), "E0112", "{src}");
    }
    for src in ["fn f(1) = 0", "fn f('a) = 0"] {
        assert_eq!(perr(src), "E0112", "{src}");
    }
    p("f = (_, [a], {b}, (c, d), Ok(e), x) => x");
    p("fn f(_, [a], {b}, (c, d), Ok(e), x) = x");
}

#[test]
fn an_import_list_takes_a_trailing_comma() {
    // SP8: every comma-separated list takes a trailing comma.
    p("import path (base,)");
    p("import path (\n  base,\n  ext,\n)");
    assert_eq!(perr("import path ()"), "E0112");
}

/// A lex or parse error as `CODE line:col message`, its notes and its
/// help, each on its own line.
fn full_err(src: &str) -> String {
    let mut sm = SourceMap::new();
    let f = sm.add("t.fung", src);
    let d = match lex(f, src) {
        Ok((toks, _)) => match parse(&toks) {
            Ok(n) => panic!("parse {src:?}: want error, got {n}"),
            Err(d) => d,
        },
        Err(d) => d,
    };
    let mut out = format!(
        "{} {}:{} {}",
        d.code,
        d.pos.line,
        d.pos.col,
        crate::diag::message(&d.code, &d.args)
    );
    for n in &d.notes {
        out.push('\n');
        out.push_str(n);
    }
    if let Some(h) = &d.help {
        out.push_str("\nhelp: ");
        out.push_str(h);
    }
    out
}

#[test]
fn common_syntax_errors_say_what_was_expected() {
    // FE26: a list stopped by something else than `,` or its closer says
    // so, and where the bracket opened.
    let cases = [
        (
            "x = [1, 2\nprint(x)",
            "E0112 2:1 expected `,` or `]`, found name `print`\nnote: the `[` opened at 1:5",
        ),
        (
            "x = [1, 2",
            "E0112 1:10 expected `,` or `]`, found end of file\nnote: the `[` opened at 1:5",
        ),
        (
            "x = {a: 1 b: 2}",
            "E0112 1:11 expected `,` or `}`, found name `b`\nnote: the `{` opened at 1:5",
        ),
        (
            "print(\"a\" \"b\")",
            "E0112 1:11 expected `,` or `)`, found string `\"b\"`\nnote: the `(` opened at 1:6",
        ),
        (
            "f(1, 2",
            "E0112 1:7 expected `,` or `)`, found end of file\nnote: the `(` opened at 1:2",
        ),
        (
            "x = (1, 2 3)",
            "E0112 1:11 expected `,` or `)`, found integer `3`\nnote: the `(` opened at 1:5",
        ),
        (
            "x = (1 2)",
            "E0112 1:8 expected `,` or `)`, found integer `2`\nnote: the `(` opened at 1:5",
        ),
        (
            "[a, b c] = xs",
            "E0112 1:7 expected `,` or `]`, found name `c`\nnote: the `[` opened at 1:1",
        ),
        (
            "f = (a b) => a",
            "E0112 1:8 expected `,` or `)`, found name `b`\nnote: the `(` opened at 1:5",
        ),
        (
            "fn f(a b) = a",
            "E0112 1:8 expected `,` or `)`, found name `b`\nnote: the `(` opened at 1:5",
        ),
        // a positional argument after a named one has its own error
        (
            "f(b: 2, 1)",
            "E0126 1:9 a positional argument cannot follow a named one",
        ),
        (
            "f(b: 2, _)",
            "E0126 1:9 a positional argument cannot follow a named one",
        ),
        (
            "fn f(a, ...r, b = 1) = a",
            "E0112 1:15 expected `)` after the rest parameter, found name `b`\nhelp: the rest parameter `...r` takes the surplus arguments, so it comes last",
        ),
        (
            "fn f((a, b) = (1, 2)) = a",
            "E0112 1:13 expected `,` or `)`, found `=`\nnote: the `(` opened at 1:5\nhelp: a destructuring parameter takes no type and no default",
        ),
        (
            "fn f(...r = 1) = r",
            "E0112 1:11 expected `,` or `)`, found `=`\nnote: the `(` opened at 1:5\nhelp: a rest parameter takes no default: it is `[]` when no arguments are left",
        ),
        // a string in single quotes
        (
            "x = 'hello'",
            "E0101 1:11 unexpected character \"'\"\nhelp: strings use double quotes: `\"text\"`; a symbol is `'name`, with no closing quote",
        ),
        // the code of an interpolation ends at its `}`
        (
            "x = \"${}\"",
            "E0112 1:8 expected an expression inside `${…}`, found `}`",
        ),
        ("x = \"${1 +}\"", "E0110 1:11 unexpected `}`"),
        ("x = \"${1 2}\"", "E0110 1:10 unexpected integer `2`"),
        // escapes: the escape as written, and a code point that is no
        // character has its own error
        ("x = \"\\q\"", "E0103 1:6 unknown escape `\\q` in string"),
        (
            "x = \"\\\t\"",
            "E0103 1:6 unknown escape `\\` followed by U+0009 in string",
        ),
        (
            "x = \"\\u{110000}\"",
            "E0106 1:6 `\\u{110000}` is not a Unicode scalar value",
        ),
        (
            "x = \"\\u{d800}\"",
            "E0106 1:6 `\\u{d800}` is not a Unicode scalar value",
        ),
    ];
    let mut bad = Vec::new();
    for (src, want) in cases {
        let got = full_err(src);
        if got != want {
            bad.push(format!("{src:?}:\n got {got}\nwant {want}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// FE42: a token in a headline is its first line, cut when long; its
/// control characters are escaped by `diag::message`.
#[test]
fn a_token_in_a_headline_is_one_short_line() {
    let msg = |src: &str| {
        let d = parse(&toks_of(src)).expect_err(src);
        crate::diag::message(&d.code, &d.args)
    };
    assert_eq!(
        msg("x = 1 `raw\nline\u{1b}[2J`"),
        "unexpected raw string `raw…`"
    );
    let long = format!("x = 1 \"{}\"", "a".repeat(100));
    let m = msg(&long);
    assert!(m.len() < 80 && m.ends_with("…`"), "{m}");
    assert_eq!(
        msg("x = 1 \"\u{1b}[31mred\""),
        "unexpected string `\"\\u{1B}[31mred\"`"
    );
}

/// FE52: a statement whose brackets close before `=` is an assignment,
/// so the pattern's own error is reported, not the expression's; an arm
/// head whose brackets close before `=>` likewise.
#[test]
fn destructuring_keeps_the_patterns_error() {
    let msg = |src: &str| {
        let d = parse(&toks_of(src)).expect_err(src);
        (
            d.code.clone(),
            crate::diag::message(&d.code, &d.args),
            d.help.unwrap_or_default(),
        )
    };
    let (code, m, _) = msg("[a, ...b, c] = [1, 2, 3]");
    assert!(code != "E0110" || !m.contains("`...`"), "{code} {m}");
    let (_, m, help) = msg("(a) = 1");
    assert!(format!("{m} {help}").contains("(p,)"), "{m} / {help}");
    let (_, m, help) = msg("Ok((a)) = Ok(1)");
    assert!(format!("{m} {help}").contains("(p,)"), "{m} / {help}");
    let (_, m, help) = msg("x = match 1:\n  1 => 1\n  (a) => 2\n;;");
    assert!(format!("{m} {help}").contains("(p,)"), "{m} / {help}");
    // still expressions and assignments as before
    p("[1, 2] == [1, 2]");
    p("(a, b) = (1, 2)");
    p("[a, b] | (a, b) = [1, 2]");
    p("x = match 1:\n  1 => 1\n  (a,) => 2\n;;");
}

/// FE57: the code inside `${…}` is one expression; a separator after it
/// is not taken.
#[test]
fn interpolation_takes_no_trailing_separator() {
    assert_eq!(perr("\"${1;}\""), "E0110");
    assert_eq!(perr("\"${1;2}\""), "E0110");
}

/// FE60: nested `fn` definitions count toward the nesting limit like
/// any other block.
#[test]
fn nested_fn_definitions_are_bounded() {
    big_stack(|| {
        let nest = |n: usize| format!("{}1\n{}", "fn f():\n".repeat(n), ";;\n".repeat(n));
        assert_eq!(perr(&nest(20_000)), "E0115");
        p(&nest(100));
    });
}

#[test]
fn coalesce_and_pipe_do_not_mix_without_parentheses() {
    // D35: either precedence would silently flip one natural reading.
    check_errs(&[
        ("y = x |> f ?? d", "E0127"),
        ("y = a ?? b |> f", "E0127"),
        ("y = a ?? b ?? c |> f", "E0127"),
        ("y = x |> f |> g ?? d", "E0127"),
        ("print(x |> f ?? d)", "E0127"),
    ]);
    // the help shows the two readings of what was written
    let help = |src: &str| parse(&toks_of(src)).unwrap_err().help.unwrap_or_default();
    assert!(help("y = x |> f ?? d").contains("`(x |> f) ?? d`"));
    assert!(help("y = a ?? b |> f").contains("`(a ?? b) |> f`"));
    check(&[
        (
            "y = (x |> f) ?? d",
            "(program (assign y (coalesce (pipe x f) d)))",
        ),
        (
            "y = x |> (f ?? d)",
            "(program (assign y (pipe x (coalesce f d))))",
        ),
        (
            "y = (a ?? b) |> f",
            "(program (assign y (pipe (coalesce a b) f)))",
        ),
        (
            "y = a ?? (b |> f)",
            "(program (assign y (coalesce a (pipe b f))))",
        ),
        (
            "y = a ?? b ?? c",
            "(program (assign y (coalesce (coalesce a b) c)))",
        ),
    ]);
}

#[test]
fn a_lambda_is_no_operand_without_parentheses() {
    // FE41: a lambda is an `expr`, not an operand of any operator.
    check_errs(&[
        ("f = 1 + x => x * 2", "E0110"),
        ("y = 1 |> x => x + 1 |> str", "E0110"),
        ("f = a < x => x < 3", "E0110"),
        ("f = 2 ** x => x", "E0110"),
        ("f = not x => x", "E0110"),
        ("f = -(a, b) => a", "E0110"),
        ("f = c ?? x => x", "E0110"),
    ]);
    let d = parse(&toks_of("f = 1 + x => x * 2")).unwrap_err();
    assert!(
        d.help.as_deref().unwrap_or("").contains("parentheses"),
        "{:?}",
        d.help
    );
    check(&[
        ("f = x => x", "(program (assign f (fn (x) (block x))))"),
        (
            "y = 1 |> (x => x + 1)",
            "(program (assign y (pipe 1 (fn (x) (block (binary + x 1))))))",
        ),
        (
            "y = c then x => x else (y => y)",
            "(program (assign y (then c (fn (x) (block x)) (fn (y) (block y)))))",
        ),
        ("f(x => x)", "(program (call f (fn (x) (block x))))"),
        // the arm's arrow after a guard is no lambda
        (
            "m = match 1: n when n > x => 1 ;;",
            "(program (assign m (match 1 (arm (pbind n) (binary > n x) (block 1)))))",
        ),
    ]);
}

#[test]
fn return_stands_as_a_statement_or_ends_a_branch() {
    // D50, FE51: anywhere else it is E0132.
    check_errs(&[
        ("fn f(a):\n  x = 1 + return a\n  x\n;;", "E0132"),
        ("fn f(a) = [a, return 2]", "E0132"),
        ("fn f(a) = g(return a)", "E0132"),
        ("fn f(): [return] ;;", "E0132"),
        ("fn f(): {a: return} ;;", "E0132"),
        ("fn f(): (return) ;;", "E0132"),
        ("fn f(): f(return, 1) ;;", "E0132"),
        ("fn f(a):\n  x = return a\n  x\n;;", "E0132"),
        ("fn f(a) = a ?? return 1", "E0132"),
        ("fn f(a) = return return a", "E0132"),
        ("fn f(a) = not return a", "E0132"),
    ]);
    check(&[
        (
            "fn f() = return",
            "(program (fn f() (block (return (nil)))))",
        ),
        (
            "fn f(a) = a then return 1 else 2",
            "(program (fn f(a) (block (then a (return 1) 2))))",
        ),
        (
            "fn f(a):\n  x = a then 1 else return 2\n  x\n;;",
            "(program (fn f(a) (block (assign x (then a 1 (return 2))) x)))",
        ),
        (
            "fn f(a) = g(a then 1 else return)",
            "(program (fn f(a) (block (call g (then a 1 (return (nil)))))))",
        ),
        (
            "fn f(a) = [a then 1 else return]",
            "(program (fn f(a) (block (list (then a 1 (return (nil)))))))",
        ),
        (
            "fn f(a) = match a:\n  1 => return 2\n  _ => 3\n;;",
            "(program (fn f(a) (block (match a (arm (plit 1) (nil) (block (return 2))) (arm (pwild) (nil) (block 3))))))",
        ),
    ]);
}

#[test]
fn nothing_follows_a_return_statement_in_its_block() {
    // D50: a statement after `return` in the same block never runs (E0131).
    check_errs(&[
        ("fn f():\n  return 1\n  print(1)\n;;", "E0131"),
        ("fn f(): return 1; 2 ;;", "E0131"),
        (
            "fn f():\n  if c:\n    return\n    5\n  ;;\n  1\n;;",
            "E0131",
        ),
        (
            "fn f(x) = match x:\n  1 =>\n    return 2\n    3\n  _ => 4\n;;",
            "E0131",
        ),
    ]);
    for src in [
        "fn f():\n  if c: return 1 ;;\n  2\n;;",
        "fn f(x) = match x:\n  1 => return 2\n  _ => 4\n;;",
        "fn f():\n  return 1\n;;",
    ] {
        p(src);
    }
}

#[test]
fn a_type_has_no_element_type() {
    // D52: element types are not checked, so they are not written (E0130).
    check_errs(&[
        ("fn f(x: list[int]) = x", "E0130"),
        ("fn f(x) -> map[str] = x", "E0130"),
        ("type t = list[t] | int", "E0130"),
        ("fn f(x: int[str]) = x", "E0130"),
    ]);
    p("fn f(x: list | map) -> list = x");
}

#[test]
fn imports_come_before_the_other_statements() {
    // D55: E0313 for an import after any other statement of the file.
    check_errs(&[
        ("print(1)\nimport fs", "E0313"),
        ("import fs\nx = 1\nimport json", "E0313"),
        ("fn f() = 1\nimport fs", "E0313"),
        ("type t = int\nimport fs", "E0313"),
    ]);
    p("# a comment\n\nimport fs\nimport json (parse)\nprint(1)");
}

/// `...xs` is a call's last positional argument (D45): before the named
/// ones, after the others, once; nowhere else.
#[test]
fn a_spread_is_the_last_positional_argument() {
    for (src, want) in [
        ("f(...xs)", "(program (call f (spread xs)))"),
        (
            "f(a, ...g(b), k: 1)",
            "(program (call f a (spread (call g b)) (named k 1)))",
        ),
        ("f(_, ...xs)", "(program (call f (hole) (spread xs)))"),
        ("f(...[1, 2],)", "(program (call f (spread (list 1 2))))"),
        ("f(...xs, ys)", "E0133 1:10"),
        ("f(...xs, ...ys)", "E0133 1:10"),
        ("f(...xs, _)", "E0133 1:10"),
        ("f(k: 1, ...xs)", "E0126 1:9"),
        ("x = [...xs]", "E0133 1:6"),
        ("x = Pair(1, ...xs)", "E0133 1:13"),
        ("x = ...xs", "E0110 1:5"),
    ] {
        assert_eq!(ast(src), want, "{src:?}");
    }
}

/// FE43: a habit from another language is named at the word or the
/// token that shows it, with how fung writes it (`habits.rs`).
#[test]
fn habits_from_other_languages_get_their_help() {
    let words = |w: &str| crate::habits::word(w).unwrap();
    let cases: Vec<(&str, String)> = vec![
        (
            "if true:\n  print(1)\nelif false:\n  print(2)\n;;",
            format!("E0134 3:1 `elif` is not a keyword in fung\nhelp: {}", words("elif")),
        ),
        (
            "while true:\n  print(1)\n;;",
            format!("E0134 1:1 `while` is not a keyword in fung\nhelp: {}", words("while")),
        ),
        (
            "while(x < 3):\n  print(1)\n;;",
            format!("E0134 1:1 `while` is not a keyword in fung\nhelp: {}", words("while")),
        ),
        (
            "x = 1\nlet y = 1",
            format!("E0134 2:1 `let` is not a keyword in fung\nhelp: {}", words("let")),
        ),
        (
            "def f(x):\n  x\n;;",
            format!("E0134 1:1 `def` is not a keyword in fung\nhelp: {}", words("def")),
        ),
        (
            "function f(x):\n  x\n;;",
            format!("E0134 1:1 `function` is not a keyword in fung\nhelp: {}", words("function")),
        ),
        (
            "x = true && false",
            format!("E0101 1:10 unexpected character \"&\"\nhelp: {}", crate::habits::operator("&&").unwrap()),
        ),
        (
            "x = true || false",
            format!("E0110 1:10 unexpected `||`\nhelp: {}", crate::habits::operator("||").unwrap()),
        ),
        (
            "x = !true",
            format!("E0110 1:5 unexpected `!`\nhelp: {}", crate::habits::operator("!").unwrap()),
        ),
        (
            "fn f(x):\n  x += 1\n  x\n;;",
            format!("E0110 2:5 unexpected `+=`\nhelp: {}", crate::habits::compound("+")),
        ),
        (
            "fn f(x):\n  x *= 2\n  x\n;;",
            format!("E0110 2:5 unexpected `*=`\nhelp: {}", crate::habits::compound("*")),
        ),
        (
            "fn f(x):\n  x++\n  x\n;;",
            format!("E0110 2:4 unexpected `++`\nhelp: {}", crate::habits::operator("++").unwrap()),
        ),
        (
            "f = (a, b) -> a + b",
            format!("E0110 1:12 unexpected `->`\nhelp: {}", crate::habits::operator("->").unwrap()),
        ),
        (
            "fn f(x) { x }",
            format!("E0110 1:9 unexpected `{{`\nhelp: {}", crate::habits::operator("{").unwrap()),
        ),
        (
            "if true {\n  print(1)\n}",
            format!(
                "E0112 1:9 expected `:` after the if condition, found `{{`\nhelp: {}",
                crate::habits::operator("{").unwrap()
            ),
        ),
        (
            "if true:\n  print(1)\nelse {\n  print(2)\n}",
            format!(
                "E0112 3:6 expected `:` or `if` after else, found `{{`\nhelp: {}",
                crate::habits::operator("{").unwrap()
            ),
        ),
        (
            "match = 1",
            format!("E0110 1:1 unexpected reserved word `match`\nhelp: {}", crate::habits::reserved("match")),
        ),
        (
            "type = 1",
            format!("E0110 1:1 unexpected reserved word `type`\nhelp: {}", crate::habits::reserved("type")),
        ),
        (
            "x = 1\nif x = 1:\n  print(1)\n;;",
            format!(
                "E0112 2:6 expected `:` after the if condition, found `=`\nhelp: {}",
                crate::habits::COMPARE
            ),
        ),
        (
            "print(x = 1)",
            format!(
                "E0112 1:9 expected `,` or `)`, found `=`\nnote: the `(` opened at 1:6\nhelp: {}",
                crate::habits::NAMED_ARG
            ),
        ),
        (
            "fn f():\n  print(1)\nend",
            "E0112 3:4 expected `;;` to close the function, found end of file\nnote: the `fn` opened at 1:1\nhelp: a block closes with `;;`; the `end` on line 3 is read as a name".to_string(),
        ),
    ];
    let mut bad = Vec::new();
    for (src, want) in &cases {
        let got = full_err(src);
        if got != *want {
            bad.push(format!("{src:?}:\n got {got}\nwant {want}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    // A word of the table is still a name where a name fits.
    p("let = 1\nprint(let)");
    p("while(1)");
    p("x = [end, null]");
}

/// FE44: a block left open says where it was opened.
#[test]
fn an_unclosed_block_says_where_it_opened() {
    for (src, want) in [
        (
            "fn f(x):\n  if x:\n    print(1)\n\nprint(2)",
            "E0112 5:9 expected `;;` to close the if, found end of file\nnote: the `if` opened at 2:3\nnote: line 5 is not indented under the `if`: was the block meant to close before it?",
        ),
        (
            "x = 1\nif x == 1:\n  print(1)\nelse if x == 2:\n  print(2)",
            "E0112 5:11 expected `;;` to close the if, found end of file\nnote: the `if` opened at 2:1",
        ),
        (
            "x = match 1:\n  1 => 2\n",
            "E0112 3:1 expected `;;` to close the match, found end of file\nnote: the `match` opened at 1:5",
        ),
        (
            "for x in [1]:\n  print(x)",
            "E0112 2:11 expected `;;` to close the for, found end of file\nnote: the `for` opened at 1:1",
        ),
        (
            "test \"a\":\n  assert true",
            "E0112 2:14 expected `;;` to close the test, found end of file\nnote: the `test` opened at 1:1",
        ),
        (
            "f = fn(x):\n  x",
            "E0112 2:4 expected `;;` to close the function, found end of file\nnote: the `fn` opened at 1:5",
        ),
    ] {
        assert_eq!(full_err(src), want, "{src:?}");
    }
}

/// FE61: an error underlines the whole token it names.
#[test]
fn an_error_underlines_the_whole_token() {
    for (src, len) in [
        ("x = 1 foo", 3),
        ("print(1,, 2)", 1),
        ("x = [1, 2 three]", 5),
        ("x = 1 >= ", 1),
        ("let x = 1", 3),
        ("x = true || false", 2),
        ("x = 1\nmatch = 2", 5),
    ] {
        let mut sm = SourceMap::new();
        let f = sm.add("t.fung", src);
        let (toks, _) = lex(f, src).unwrap();
        let d = parse(&toks).unwrap_err();
        assert_eq!(d.len.max(1), len, "{src:?}: {}", d.code);
    }
}

/// FE61: the least int folds across a line break after the minus, as a
/// line ending in an operator continues; `as _` binds nothing.
#[test]
fn small_parse_diagnostics_of_the_third_review() {
    assert_eq!(
        p("y = (-\n9223372036854775808)"),
        "(program (assign y -9223372036854775808))"
    );
    assert_eq!(
        full_err("import \"./a\" as _"),
        "E0112 1:17 expected a name after `as`, found name `_`\nhelp: `as _` binds nothing, so the module could not be used; give it a name"
    );
    // a duplicate key says where the first one is
    assert_eq!(
        full_err("print({a: 1, a: 2})"),
        "E0113 1:14 duplicate key `a` in a map literal or pattern\nnote: the key `a` is first written at 1:8"
    );
    assert_eq!(
        full_err("{a: x, \"a\": y} = {a: 1}"),
        "E0113 1:8 duplicate key `a` in a map literal or pattern\nnote: the key `a` is first written at 1:2"
    );
}
