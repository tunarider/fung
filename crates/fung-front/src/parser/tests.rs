//! Ports of the Go parser tests (`internal/parser/*_test.go`), case tables
//! included, plus the parity cases checked against the Go binary.

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
    let (toks, _) = lex(f, src).unwrap();
    parse(&toks)
        .map(|n| n.to_string())
        .unwrap_or_else(|d| format!("{} {}:{}", d.code, d.pos.line, d.pos.col))
}

/// Go `parse`: the tree, failing the test on an error.
#[track_caller]
fn p(src: &str) -> String {
    match parse(&toks_of(src)) {
        Ok(n) => n.to_string(),
        Err(d) => panic!("parse {src:?}: {} {}:{}", d.code, d.pos.line, d.pos.col),
    }
}

/// Go `parseErr`: the error code, failing the test when parsing succeeds.
#[track_caller]
fn perr(src: &str) -> String {
    match parse(&toks_of(src)) {
        Ok(n) => panic!("parse {src:?}: want error, got {n}"),
        Err(d) => d.code,
    }
}

/// Go `parseErrQuiet`: the error code or "" without echoing the input.
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

// --- Parity cases from the brief (outputs of the Go binary) ---

#[test]
fn real_go_outputs() {
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
    assert_eq!(ast("d = [1, ...[2]]"), "E0110 1:9");
    assert_eq!(ast("y = 1 < 2 < 3"), "E0111 1:11");
}

#[test]
fn deep_nesting_is_an_error_not_a_crash() {
    let src = format!("x = {}1{}", "(".repeat(20_000), ")".repeat(20_000));
    let handle = std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || ast(&src))
        .unwrap();
    assert_eq!(handle.join().unwrap(), "E0802 1:10005");
}

// --- parser_test.go ---

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
        ("(1", "E0110"),
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
        assert_eq!(perr(&deep), "E0802");
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
        assert_eq!(perr(&src), "E0802");
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

// --- fn_test.go ---

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
            "fn k(xs: list[int]) = xs",
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
        ("f(a: 1, 2)", "E0110"),
        ("fn (a) = a", "E0110"),
        ("fn f(a = 1, b) = a", "E0110"),
        ("fn f(a: ) = a", "E0110"),
        ("fn f(a, a) = a", "E0110"),
        ("x =>", "E0110"),
        ("fn f() -> = 1", "E0110"),
        ("fn f(...a, b) = a", "E0110"),
    ]);
}

#[test]
fn lambda_params_require_commas() {
    assert_eq!(perr("(a b) => a + b"), "E0110");
}

// --- m4_test.go ---

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
        ("Red", "(program 'Red)"),
        ("Ok(x) + 1", "(program (binary + (tag Ok x) 1))"),
    ]);
}

#[test]
fn postfix_and_new_infix() {
    check(&[
        ("xs[0]", "(program (index xs 0))"),
        ("xs[-1]", "(program (index xs (unary - 1)))"),
        ("xs[1..3]", "(program (slice xs 1 3))"),
        ("xs[..3]", "(program (slice xs (nil) 3))"),
        ("xs[-4..]", "(program (slice xs (unary - 4) (nil)))"),
        ("xs[1..=3]", "(program (slice ..= xs 1 3))"),
        ("xs[i ?? 0]", "(program (index xs (coalesce i 0)))"),
        ("xs[c then 0 else 1]", "(program (index xs (then c 0 1)))"),
        (
            "xs[(i ?? 0)..]",
            "(program (slice xs (coalesce i 0) (nil)))",
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
            "fs.read(p) ?? \"{}\" |> json.parse",
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
    for src in ["x in xs in ys", "a < b in c", "f(a: _)"] {
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
        ("match v: (x) => 1 ;;", "E0110"),
        ("match v: [...a, b] => 1 ;;", "E0110"),
        ("match v: 1 + 2 => 1 ;;", "E0110"),
        ("match v: ;;", "E0110"),
        ("match v: x => ;;", "E0110"),
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
    assert_eq!(perr("Ok(a: 1)"), "E0110");
}

#[test]
fn deep_pattern_is_bounded() {
    big_stack(|| {
        let src = format!("{}x{} = 1", "[".repeat(20_000), "]".repeat(20_000));
        assert_eq!(perr_quiet(&src), "E0802", "deep pattern");
        let src = format!(
            "match v: {}x{} => 1 ;;",
            "Ok(".repeat(20_000),
            ")".repeat(20_000)
        );
        assert_eq!(perr_quiet(&src), "E0802", "deep pattern in match");
    });
}

// --- m5_test.go ---

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
        ("import fs (a,)", "E0112"),
        ("import \"x\" as Fs", "E0112"),
        ("import \"${x}\"", "E0112"),
    ]);
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

// --- m6_test.go ---

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
        ("fn f((a, b) = 1) = a", "E0110"),
        ("fn f((a, b): int) = a", "E0110"),
        ("(1 + 2) => 3", "E0110"),
    ]);
}

// --- keyword_names_test.go ---

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

// --- map_shorthand_test.go ---

#[test]
fn map_shorthand() {
    check(&[
        (
            "{a, b: 2, c}",
            "(program (map (entry a a) (entry b 2) (entry c c)))",
        ),
        ("{DB_HOST}", "(program (map (entry DB_HOST DB_HOST)))"),
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
}
