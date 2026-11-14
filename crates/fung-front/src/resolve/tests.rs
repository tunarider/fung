//! Resolver tests by milestone, plus whole outputs of larger programs.

use std::collections::BTreeMap;

use super::*;
use crate::builtins::builtins;
use crate::desugar::desugar;
use crate::diag::{Arg, Diag};
use crate::lexer::lex;
use crate::parser::parse;
use crate::source::SourceMap;
use crate::tree::{Capture, CaptureKind, FnInfo, Lit, Node};

fn core_of(src: &str) -> Node {
    let mut sm = SourceMap::new();
    let f = sm.add("t.fung", src);
    let (toks, _) = lex(f, src).unwrap();
    desugar(&parse(&toks).unwrap(), true).unwrap()
}

fn names(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

fn resolve_with(src: &str, prelude: &[&str], opts: &Options) -> (Option<Resolved>, Vec<Diag>) {
    match resolve(&core_of(src), &names(prelude), opts) {
        Ok(r) => (Some(r), Vec::new()),
        Err(es) => (None, es),
    }
}

/// Resolves like the REPL's input: the file's last item is kept
/// (`core_of` desugars with `keep_value`).
fn res(src: &str) -> (Option<Resolved>, Vec<Diag>) {
    let opts = Options {
        keep_value: true,
        ..Options::default()
    };
    resolve_with(src, &["print", "fail"], &opts)
}

/// Resolves like a file run with `fung run`: the last item is discarded.
fn res_discard(src: &str) -> (Option<Resolved>, Vec<Diag>) {
    let mut sm = SourceMap::new();
    let f = sm.add("t.fung", src);
    let (toks, _) = lex(f, src).unwrap();
    let core = desugar(&parse(&toks).unwrap(), false).unwrap();
    match resolve(&core, &names(&["print", "fail"]), &Options::default()) {
        Ok(r) => (Some(r), Vec::new()),
        Err(es) => (None, es),
    }
}

fn ok(src: &str) -> Resolved {
    let (r, errs) = res(src);
    assert!(errs.is_empty(), "{src:?}: {errs:?}");
    r.unwrap()
}

fn codes(errs: &[Diag]) -> Vec<String> {
    errs.iter().map(|e| e.code.clone()).collect()
}

/// The errors without the E0238s for binding a prelude name again:
/// M4_PRELUDE's `f`, `g` and `xs` stand for outer names that the cases
/// bind again (D38).
fn not_hiding(errs: &[Diag]) -> Vec<Diag> {
    errs.iter()
        .filter(|e| {
            !(e.code == "E0238" && e.notes.iter().any(|n| n.ends_with("is a prelude function")))
        })
        .cloned()
        .collect()
}

fn first_code(errs: &[Diag]) -> String {
    errs.first().map(|e| e.code.clone()).unwrap_or_default()
}

fn arg0(e: &Diag) -> String {
    match &e.args[0] {
        Arg::Str(s) => s.clone(),
        Arg::Int(n) => n.to_string(),
    }
}

fn help(e: &Diag) -> &str {
    e.help.as_deref().unwrap_or("")
}

fn fn_info<'a>(core: &'a Node, path: &[usize]) -> &'a FnInfo {
    let mut n = core;
    for &i in path {
        n = &n.kids[i];
    }
    match &n.lit {
        Lit::Fn(f) => f,
        _ => panic!("node {n} is not a fn"),
    }
}

fn prelude() -> Vec<String> {
    builtins().prelude_names()
}

fn core(src: &str) -> Result<String, Vec<String>> {
    let mut sm = SourceMap::new();
    let f = sm.add("-e", src);
    let (toks, _) = lex(f, src).unwrap();
    let d = desugar(&parse(&toks).unwrap(), true).unwrap();
    resolve(&d, &prelude(), &Options::default())
        .map(|r| r.core.to_string())
        .map_err(|es| es.iter().map(|e| e.code.clone()).collect())
}

// ---- whole outputs ----

#[test]
fn whole_outputs() {
    assert_eq!(
        core("_x = 1 + 2 * 3").unwrap(),
        "(do (let (global 0) (op + 1 (op * 2 3))))"
    );
    assert_eq!(
        core("f = (a, b) => a - b\nprint(f(1, b: 2))").unwrap(),
        "(do (let (global 0) (fn (a, b) (do (op - (local 0) (local 1))))) (call (prelude print) (call (global 0) 1 (named b 2))))"
    );
    assert_eq!(core("x = 1\nx = 2").unwrap_err(), vec!["E0238"]);
}

#[test]
fn errors_are_all_collected() {
    assert_eq!(core("print(nope)").unwrap_err(), vec!["E0204"]);
    assert_eq!(core("y = Ok(1)? ?? 0").unwrap_err(), vec!["E0214"]);
    assert_eq!(core("print(a, b)").unwrap_err(), vec!["E0204", "E0204"]);
}

#[test]
fn spec_lists_prelude_fung_names_that_the_prelude_file_exports() {
    // the Rust front end computes prelude.fung's exports itself; they must match the spec
    let b = builtins();
    assert!(b.prelude_fung.iter().any(|n| n == "take"));
    assert!(!b.prelude_fung.iter().any(|n| n.starts_with('_')));
}

#[test]
fn builtins_spec_is_parsed() {
    let b = builtins();
    assert!(b.prelude_native.iter().any(|n| n == "print"));
    assert!(b.prelude_native.iter().any(|n| n == "args"));
    assert!(b.modules["_native.fs"].iter().any(|n| n == "read"));
    assert!(b.modules["_native.fs"].iter().any(|n| n == "temp_dir"));
    assert!(b.prelude_native.windows(2).all(|w| w[0] < w[1]));
    assert!(b.prelude_fung.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn help_texts_match_go() {
    let (_, errs) = res("if true: y = 1 ;;\nprint(y)");
    assert_eq!(
        help(&errs[0]),
        "assign the block's value instead: `y = if … ;;`"
    );
    assert_eq!(errs[0].len, 1);
    let (_, errs) = res("f = () => x\nx = 1");
    assert_eq!(
        help(&errs[0]),
        "`x` is a file-level variable (assigned on line 2) that this function cannot see: a lambda or a function made in a block copies only the variables that exist when it is made; assign `x` first, or pass it as an argument"
    );
    let opts = m5_opts(false);
    let (_, errs) = resolve_with("import fs\nfs = 1", &["print", "map"], &opts);
    assert_eq!(
        help(&errs[0]),
        "import it under another name: `import fs as m`"
    );
    assert_eq!(errs[0].len, 2);
    assert_eq!((errs[0].pos.line, errs[0].pos.col), (2, 1));
}

#[test]
fn deep_trees_do_not_overflow() {
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(|| {
            let src = format!("_x = [1]{}", "[0]".repeat(9_000));
            let r = resolve(&core_of(&src), &[], &Options::default()).unwrap();
            assert!(r.core.to_string().starts_with("(do (let (global 0) (index"));
            let src = format!("fn f(a) = {}a{}", "[".repeat(9_000), "]".repeat(9_000));
            let r = resolve(&core_of(&src), &[], &Options::default()).unwrap();
            assert!(r.core.to_string().contains("(local 0)"));
        })
        .unwrap()
        .join()
        .unwrap();
}

// ---- names and scopes ----

#[test]
fn resolve_marks_prelude_names() {
    assert_eq!(
        ok("print(1)").core.to_string(),
        "(do (call (prelude print) 1))"
    );
}

#[test]
fn resolve_collects_all_undefined_names() {
    let (_, errs) = res("print(fiels)\nprint(x + y)");
    assert_eq!(errs.len(), 3, "{errs:?}");
    for (i, want) in ["fiels", "x", "y"].iter().enumerate() {
        assert_eq!(errs[i].code, "E0204");
        assert_eq!(arg0(&errs[i]), *want);
    }
    assert_eq!((errs[1].pos.line, errs[1].pos.col), (2, 7));
}

#[test]
fn resolve_does_not_mutate_input() {
    let input = core_of("print(1)");
    let before = input.to_string();
    let _ = resolve(&input, &names(&["print"]), &Options::default());
    assert_eq!(input.to_string(), before);
}

#[test]
fn top_level_names_become_globals() {
    let r = ok("x = 1\ny = x + 1\nz = x * 2\nprint(z, y)");
    assert_eq!(
        r.core.to_string(),
        "(do (let (global 0) 1) (let (global 1) (op + (global 0) 1)) (let (global 2) (op * (global 0) 2)) (call (prelude print) (global 2) (global 1)))"
    );
    assert_eq!(r.num_locals, 0);
    assert_eq!(r.globals, names(&["x", "y", "z"]));
}

#[test]
fn block_names_become_slots_and_are_reused() {
    let r = ok("if true: a = 1; _b = a ;;\nif true: _c = 2 ;;");
    assert_eq!(
        r.core.to_string(),
        "(do (if true (do (let (local 0) 1) (let (local 1) (local 0))) (nil)) (if true (do (let (local 0) 2)) (nil)))"
    );
    assert_eq!(r.num_locals, 2);
}

#[test]
fn nested_blocks_stack_slots() {
    let r = ok("if true: a = 1; if true: _b = a ;; ;;");
    assert_eq!(r.num_locals, 2);
    assert!(r.core.to_string().contains("(let (local 1) (local 0))"));
}

#[test]
fn reassigning_outer_name_in_block_is_e0238() {
    let (_, errs) = res("total = 0\nif true: total = total + 1 ;;");
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].code, "E0238");
    assert_eq!(arg0(&errs[0]), "total");
    assert_eq!(errs[0].len, 5);
    assert_eq!((errs[0].pos.line, errs[0].pos.col), (2, 10));
    // A block's own name is bound once too (D88).
    let (_, errs) = res("if true: n = 1; n = n + 1; print(n) ;;");
    assert_eq!(codes(&errs), vec!["E0238"]);
    // And a local may not be bound again from a nested block either.
    let (_, errs) = res("if true: n = 1; if true: n = 2 ;; ;;");
    assert_eq!(codes(&errs), vec!["E0238"]);
}

#[test]
fn using_block_name_outside_is_e0206() {
    let (_, errs) = res("if true: y = 1 ;;\nprint(y)");
    assert_eq!(codes(&errs), vec!["E0206"]);
    assert_eq!(arg0(&errs[0]), "y");
}

#[test]
fn use_before_assignment_is_undefined() {
    let (_, errs) = res("print(x)\nx = 1");
    assert_eq!(codes(&errs), vec!["E0204"]);
    // The right-hand side is resolved before the name is bound.
    let (_, errs) = res("x = x + 1");
    assert_eq!(codes(&errs), vec!["E0204"]);
}

#[test]
fn undefined_name_suggests_similar_name() {
    let (_, errs) = res("files = 1\nprint(fiels)");
    assert_eq!(errs.len(), 1);
    assert_eq!(help(&errs[0]), "a similar name exists: `files`");
    assert_eq!(errs[0].len, 5);
    let (_, errs) = res("print(prnt(1))");
    assert_eq!(errs.len(), 1);
    assert_eq!(help(&errs[0]), "a similar name exists: `print`");
    let (_, errs) = res("print(zzzzzz)");
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].help, None);
}

/// A name the prelude moved to the module `conc` (stage 6a) is still
/// undefined, but the help says where it went, ahead of any similar
/// name. The module does have each moved name.
#[test]
fn a_name_moved_to_conc_points_at_the_module() {
    let moved: Vec<&str> = super::MOVED_TO_MODULE.iter().map(|(n, _)| *n).collect();
    assert_eq!(moved, ["fan_in", "pmap", "retry"]);
    for (name, module) in super::MOVED_TO_MODULE {
        assert!(
            builtins().modules[*module].iter().any(|n| n == name),
            "{module}.{name}"
        );
        assert!(
            !builtins().prelude_names().iter().any(|n| n == name),
            "{name}"
        );
    }
    for name in moved {
        let (_, errs) = resolve_with(
            &format!("{name}(1)"),
            &["print", "map"],
            &Options::default(),
        );
        assert_eq!(codes(&errs), vec!["E0204"]);
        assert_eq!(
            help(&errs[0]),
            format!("moved to module conc: import conc, then conc.{name}")
        );
    }
    // A local of that name is not undefined at all.
    ok("pmap = 1\nprint(pmap)");
}

#[test]
fn similar_name_threshold_scales_with_length() {
    let cases = [
        ("x = 1\nprint(zz)", ""),  // short names: no guessing
        ("ab = 1\nprint(ac)", ""), // 2 letters: still none
        ("abc = 1\nprint(abd)", "a similar name exists: `abc`"), // 3-4 letters: one edit
        ("abcd = 1\nprint(abxy)", ""), // two edits is too far at 4
        ("print(fiels)", ""),      // nothing similar in prelude
        ("files = 1\nprint(fiels)", "a similar name exists: `files`"), // transposition counts as one edit
        (
            "counter = 1\nprint(countr)",
            "a similar name exists: `counter`",
        ),
    ];
    for (src, want) in cases {
        let (_, errs) = res(src);
        assert_eq!(errs.len(), 1, "{src:?}");
        assert_eq!(help(&errs[0]), want, "{src:?}");
    }
}

// ---- functions ----

#[test]
fn top_level_fns_are_hoisted_globals() {
    let r = ok("print(f(1))\nfn f(n) = g(n)\nfn g(n) = n + 1");
    assert_eq!(
        r.core.to_string(),
        "(do (let (global 0) (fn f(n) (do (call (global 1) (local 0))))) (let (global 1) (fn g(n) (do (op + (local 0) 1)))) (call (prelude print) (call (global 0) 1)))"
    );
    assert_eq!(r.globals, names(&["f", "g"]));
    assert_eq!(r.num_locals, 0);
    let info = fn_info(&r.core, &[0, 1]);
    assert_eq!(info.num_locals, 1);
    assert!(info.captures.is_empty());
}

#[test]
fn params_locals_and_defaults() {
    let r = ok("fn f(a, b = a + 1):\n  c = a * b\n  if c > 0: _d = c ;;\n  c\n;;");
    assert_eq!(
        r.core.to_string(),
        "(do (let (global 0) (fn f(a, b=?) (op + (local 0) 1) (do (let (local 2) (op * (local 0) (local 1))) (if (op > (local 2) 0) (do (let (local 3) (local 2))) (nil)) (local 2)))))"
    );
    assert_eq!(fn_info(&r.core, &[0, 1]).num_locals, 4);
}

#[test]
fn closures_capture_by_chain() {
    // y is a local of the outer fn; the middle lambda captures it from a
    // slot, the inner lambda captures it from the middle closure.
    let r = ok("fn outer(y) = x => z => x + y + z");
    assert_eq!(
        r.core.to_string(),
        "(do (let (global 0) (fn outer(y) (do (fn (x) (do (fn (z) (do (op + (op + (captured 0) (captured 1)) (local 0))))))))))"
    );
    let local = |index| Capture {
        kind: CaptureKind::Local,
        index,
    };
    let middle = fn_info(&r.core, &[0, 1, 0, 0]);
    assert_eq!(middle.captures, vec![local(0)]);
    let inner = fn_info(&r.core, &[0, 1, 0, 0, 0, 0]);
    assert_eq!(
        inner.captures,
        vec![
            local(0),
            Capture {
                kind: CaptureKind::Captured,
                index: 0
            }
        ]
    );
}

#[test]
fn top_level_variables_are_copied_by_lambdas_and_invisible_to_hoisted_fns() {
    // A lambda created after x exists copies x's value (Global capture);
    // other top-level fns are constants and stay direct (global n) refs.
    let r = ok("fn h() = 1\nx = 1\n_g = () => x + h()");
    assert!(
        r.core
            .to_string()
            .contains("(fn () (do (op + (captured 0) (call (global 0)))))")
    );
    let g = fn_info(&r.core, &[2, 1]);
    assert_eq!(
        g.captures,
        vec![Capture {
            kind: CaptureKind::Global,
            index: 1
        }]
    );
    // Every top-level name is a constant (03 L9, D53): a top-level fn
    // sees x before or after it.
    for src in ["x = 1\nfn f() = x", "fn f() = x\nx = 1"] {
        let (_, errs) = res(src);
        assert!(errs.is_empty(), "{src:?}: {errs:?}");
    }
}

#[test]
fn local_named_fn_uses_self_and_rejects_later_siblings() {
    let r = ok("fn outer():\n  fn loop(n) = n == 0 then 0 else loop(n - 1)\n  loop(3)\n;;");
    let got = r.core.to_string();
    assert!(got.contains("(call (self) (op - (local 0) 1))"), "{got}");
    assert!(got.contains("(call (local 0) 3)"), "{got}");
    // A lambda inside the named fn reaches self through a capture.
    let r = ok("fn outer():\n  fn f(n) = (() => f(n))()\n  f(1)\n;;");
    let got = r.core.to_string();
    assert!(
        got.contains("(fn () (do (call (captured 0) (captured 1))))"),
        "{got}"
    );
    let (_, errs) = res("fn outer():\n  fn a() = b()\n  fn b() = 1\n  a()\n;;");
    assert_eq!(codes(&errs), vec!["E0208"]);
    assert_eq!(arg0(&errs[0]), "b");
    // A local fn may call those defined before it (FE82).
    assert!(
        crate::diag::catalog::entry("E0208")
            .unwrap()
            .help
            .contains("defined before it"),
        "{}",
        crate::diag::catalog::entry("E0208").unwrap().help
    );
}

#[test]
fn fn_scope_errors() {
    let cases = [
        ("fn f() = 1\nfn f() = 2", "E0238"),
        ("fn f() = 1\nf = 2", "E0238"),
        // A top-level fn cannot bind a constant's name, not even in its
        // outermost block (D38 reverses 11-10 D4).
        ("x = 1\nfn f(): x = 2; x ;;", "E0238"),
        ("x = 1\nfn f():\n  y = x\n  x = 2\n  y\n;;", "E0238"),
        ("x = 1\n_f = fn():\n  x = 2\n  x\n;;", "E0238"), // an anonymous fn created after x sees it
        ("fn f(a): g = () => a; a = 2; g ;;", "E0238"),   // a name is bound once (D88)
        ("return 1", "E0211"),
        ("if true: return 1 ;;", "E0211"),
        ("fn f(a: integer) = a", "E0212"),
        ("fn f(a) -> strng = a", "E0212"),
        ("fn f(a: Circle | int) = a", ""),
        ("fn f() = y\ny = 1", ""), // every top-level name is a constant (03, L9, D53)
        ("fn f() = y\ny = 1\ny = 2", "E0238"), // bound once (D53)
        ("f = () => f()", "E0204"), // f is not bound yet when the lambda is created
        ("fn outer(): f = () => f(); f() ;;", "E0204"), // a block local: only named fns see themselves
        ("fn f():\n  if true: v = 1 ;;\n  v\n;;", "E0206"),
        ("fn f(): fn g() = 1; g ;;\nfn h() = g()", "E0204"), // local fn of f is not visible in h
        // No block of a top-level fn can assign the constant (FE23, D38).
        (
            "c = 1\nfn g():\n  print(c)\n  if true:\n    c = 2\n  ;;\n  0\n;;",
            "E0238",
        ),
        ("c = 1\nfn g():\n  if true: c = 2 ;;\n  c\n;;", "E0238"),
        (
            "c = 1\nfn g(x):\n  match x:\n    _ =>\n      c = 2\n      c\n  ;;\n;;",
            "E0238",
        ),
        (
            "c = 1\nfn g():\n  f = () => c\n  if true: c = 2 ;;\n  f\n;;",
            "E0238",
        ),
        (
            "c = 1\nfn g():\n  f = fn():\n    c = 2\n    c\n  ;;\n  f\n;;",
            "E0238",
        ),
        (
            "c = 1\nfn g():\n  y = if true:\n    c = 2\n    c\n  else:\n    0\n  ;;\n  y\n;;",
            "E0238",
        ),
        // A `for` or an arm cannot hide the constant either (D38).
        (
            "c = 1\nfn g():\n  for c in [5]: print(c) ;;\n  c\n;;",
            "E0238",
        ),
        (
            "c = 1\nfn g(x):\n  y = match x: c => c ;;\n  (y, c)\n;;",
            "E0238",
        ),
        // A block's own name is bound once (D88).
        (
            "fn g():\n  if true:\n    v = 1\n    v = v + 1\n    print(v)\n  ;;\n  0\n;;",
            "E0238",
        ),
        // `return` in a parameter default would return from the call
        // before the body runs (FE38); a test body is a function body.
        ("fn f(a = return 5) = a", "E0211"),
        ("fn f(a = fn(): return 1 ;;) = a", ""),
        // `?` returns early just as `return` does (FE38, D8 extended).
        ("fn f(r, a = r?) = a", "E0211"),
        ("fn f(r, a = (fn(): r? ;;)()) = a", ""),
        // An inner block's assignment is the inner block's own name, and
        // the constant is not the fn's to assign anywhere (FE23, D38).
        (
            "c = 1\nfn g(t):\n  if t: c = 2 ;;\n  c = 3\n  c\n;;",
            "E0238",
        ),
        ("fn g(t):\n  if t: c = 2; print(c) ;;\n  c = 3\n  c\n;;", ""),
        (
            "c = 1\nfn g(t):\n  c = 3\n  if t: c = 2 ;;\n  c\n;;",
            "E0238",
        ),
        ("test \"a\":\n  return nil\n;;", ""),
    ];
    for (src, want) in cases {
        let (_, errs) = res(src);
        if want.is_empty() {
            assert!(errs.is_empty(), "{src:?}: unexpected {errs:?}");
        } else {
            assert_eq!(first_code(&errs), want, "{src:?}: {errs:?}");
        }
    }
}

#[test]
fn defaults_see_names_of_earlier_pattern_params() {
    // 03: a default may refer to the parameters before it, names bound by a
    // pattern parameter included.
    let opts = Options::default();
    for src in [
        "fn f((a, b), c = a + b) = c",
        "fn f((a, b), c = a, d = c) = d + b",
        "_f = fn([x, ...rest], n = len(rest)): x + n ;;",
    ] {
        let (_, errs) = resolve_with(src, &["len"], &opts);
        assert!(errs.is_empty(), "{src:?}: no unused copies: {errs:?}");
    }
    // A default still cannot see a later parameter (a pattern parameter
    // cannot follow a default at all).
    let (_, errs) = resolve_with("fn f((a, b), c = d, d = 1) = c", &[], &opts);
    assert_eq!(codes(&errs), vec!["E0204"]);
    // A name no one reads is still unused, once.
    let (_, errs) = resolve_with("fn f((a, b), c = a) = c", &[], &opts);
    let unused: Vec<String> = errs.iter().map(name_of).collect();
    assert_eq!(
        (codes(&errs), unused),
        (vec!["E0239".into()], vec!["b".into()])
    );
}

#[test]
fn a_closed_name_is_not_read_as_the_prelude() {
    // A prelude name cannot be bound at all (D38), so no name that left
    // its block is a prelude name: binding one is E0238 (03, rule 4).
    let opts = Options::default();
    let (_, errs) = resolve_with(
        "if true: lines = [1] ;;\nprint(lines)",
        &["print", "lines"],
        &opts,
    );
    assert_eq!(first_code(&errs), "E0238");
    let (_, errs) = resolve_with(
        "fn f():\n  max(1)\n  fn max(x) = x\n  nil\n;;",
        &["max"],
        &opts,
    );
    assert_eq!(codes(&errs), ["E0208", "E0238"]);
    // The prelude stays visible where nothing hides it.
    let (_, errs) = resolve_with("fn f(s) = lines(s)", &["lines"], &opts);
    assert!(errs.is_empty(), "{errs:?}");
    // A lambda or an inner fn reading a name closed in an enclosing
    // function is E0206 too (FE3).
    for src in [
        "if true: found = [1] ;;\n_g = () => found",
        "if true: found = [1] ;;\nprint(map([1], _ => found))",
        "fn f():\n  if true: found = [1] ;;\n  fn g() = found\n  g\n;;",
        "if true: x = 1 ;;\n_g = () => x",
    ] {
        let (_, errs) = resolve_with(src, &["print", "lines", "map"], &opts);
        assert_eq!(codes(&errs), vec!["E0206"], "{src:?}");
    }
}

#[test]
fn per_function_closed_sets() {
    // A name closed in f's block must not turn an unrelated typo in g into E0206.
    let (_, errs) = res("fn f():\n  if true: tmp = 1 ;;\n  nil\n;;\nfn g() = tmp");
    assert_eq!(codes(&errs), vec!["E0204"]);
}

/// FE33: every way of binding a name inside a block says so when the
/// name is read after it, with a help for that way; a `for` binds like
/// the others, so its names are not the prelude's after it either.
#[test]
fn names_read_after_their_block_say_how_they_were_bound() {
    let opts = Options::default();
    let pre = &["print", "len"];
    let help_of = |src: &str| {
        let (_, errs) = resolve_with(src, pre, &opts);
        assert_eq!(errs.len(), 1, "{src:?}: {errs:?}");
        (
            errs[0].code.clone(),
            errs[0].help.clone().unwrap_or_default(),
        )
    };
    for (src, want) in [
        (
            "r = if true: q = 1; q else: 0 ;;\nprint(q)",
            "assign the block's value instead: `q = if … ;;`",
        ),
        (
            "r = match 1: q => q ;;\nprint(q)",
            "`q` is bound in a `match` arm, only there; use the match's value instead: `q = match … ;;`",
        ),
        (
            "for i in [1]: print(i) ;;\nprint(i)",
            "`i` is bound in a `for` body, only there; use the for's value instead: `xs = for … ;;`",
        ),
        (
            "for (a, b) in [(1, 2)]: print(a, b) ;;\nprint(a)",
            "`a` is bound in a `for` body, only there; use the for's value instead: `xs = for … ;;`",
        ),
        (
            "for x in [1]: y = x; print(y) ;;\nprint(y)",
            "`y` is bound in a `for` body, only there; use the for's value instead: `xs = for … ;;`",
        ),
        (
            "fn f():\n  for i in [1]: print(i) ;;\n  i\n;;",
            "`i` is bound in a `for` body, only there; use the for's value instead: `xs = for … ;;`",
        ),
    ] {
        assert_eq!(
            help_of(src),
            ("E0206".to_string(), want.to_string()),
            "{src:?}"
        );
    }
    // A lambda that calls itself by the name it is assigned to.
    let lambda = "a lambda copies the names that exist when it is made, so it cannot call itself: define `f` with `fn f(n) = …`";
    for src in [
        "f = n => if n == 0: 0 else: f(n - 1) ;;\nprint(f(3))",
        "fn g():\n  f = n => if n == 0: 0 else: f(n - 1) ;;\n  f(3)\n;;",
    ] {
        assert_eq!(
            help_of(src),
            ("E0204".to_string(), lambda.to_string()),
            "{src:?}"
        );
    }
}

/// FE33: an import binds its names before the file's other statements,
/// so an assignment above it is the one refused; a note says why.
#[test]
fn reassigning_an_imported_name_notes_where_it_is_imported() {
    let mut opts = m5_opts(false);
    opts.imports.insert(
        "./lib/m".to_string(),
        ModuleInfo {
            key: "/p/lib/m.fung".to_string(),
            name: "m".to_string(),
            exports: vec!["x".to_string()],
            ..Default::default()
        },
    );
    let (_, errs) = resolve_with("import \"./lib/m\" (x)\nx = 0\nprint(x)", &["print"], &opts);
    assert_eq!(codes(&errs), vec!["E0238"]);
    assert_eq!(errs[0].notes, ["note: `x` is imported on line 1"]);
}

// ---- collections and patterns (M4) ----

const M4_PRELUDE: &[&str] = &["print", "f", "g", "xs", "map", "filter"];

#[test]
fn destructuring_binds_like_assignment() {
    let (r, errs) = resolve_with(
        "(q, r) = f()\nprint(q, r)\n[a, ...rest] = xs\nprint(a, rest)\nfn h():\n  (u, v) = g()\n  u + v\n;;",
        M4_PRELUDE,
        &Options::default(),
    );
    assert!(errs.is_empty(), "{errs:?}");
    let r = r.unwrap();
    let got = r.core.to_string();
    assert!(
        got.contains("(let (ptuple (pbind (global 1)) (pbind (global 2)))"),
        "{got}"
    );
    assert!(got.contains("(prest (global 4))"), "{got}");
    assert!(
        got.contains("(ptuple (pbind (local 0)) (pbind (local 1)))"),
        "{got}"
    );
    assert_eq!(r.globals[1], "q");
    assert_eq!(r.globals[4], "rest");
    let (_, errs) = res("total = 0\nif true: (total, n) = (1, 2) ;;");
    assert_eq!(codes(&errs), vec!["E0238"]);
}

#[test]
fn match_arms_shadow_and_scope() {
    let r = ok("x = 1\ny = match 2:\n  v when v > 1 => v\n  _ => x\n;;\nprint(x, y)");
    let got = r.core.to_string();
    // The arm's v is a fresh slot; outside the match, x is the global.
    assert!(
        got.contains("(arm (pbind (local 1)) (op > (local 1) 1) (do (local 1)))"),
        "{got}"
    );
    assert!(
        got.contains("(call (prelude print) (global 0) (global 1))"),
        "{got}"
    );
    assert_eq!(r.num_locals, 2); // $m0 and the arm's v
    let (_, errs) = res("match 1: 'a | 'b => print(1) ;;\nmatch 1: Ok(x) | Err(y) => print(1) ;;");
    assert_eq!(codes(&errs), vec!["E0216"]);
    let (_, errs) = res("match 1: x => print(1) ;;\nprint(x)");
    assert_eq!(codes(&errs), vec!["E0206"]);
}

#[test]
fn an_arm_binds_its_names_once() {
    // 03: an arm's names are new names in the arm's block, like a `for`
    // variable; like every name they are bound once (D88).
    let (_, errs) = res("_x = match 1: v => v = v + 1; v ;;");
    assert_eq!(codes(&errs), vec!["E0238"]);
    let (_, errs) = res("for v in [1]: v = v * 10; v ;;");
    assert_eq!(codes(&errs), vec!["E0238"]);
    // An outer name is still E0238, and arm names do not leak.
    let (_, errs) = res("y = 1\nmatch 1: v => y = 2; print(v) ;;");
    assert_eq!(codes(&errs), vec!["E0238"]);
    let (_, errs) = res("match 1: v => w = v; print(w) ;;\nprint(w)");
    assert_eq!(codes(&errs), vec!["E0206"]);
    // A local fn later in an arm is still E0208 before its definition.
    let (_, errs) = res("match 1: v => g(); fn g() = v ;;");
    assert_eq!(codes(&errs), vec!["E0208"]);
}

#[test]
fn nested_match_uses_separate_hidden_slots() {
    let r = ok("match 1:\n  a => match 2:\n    b => a + b\n  ;;\n;;");
    let got = r.core.to_string();
    assert!(got.contains("(let (local 0) 1)"), "{got}");
    assert!(got.contains("(let (local 2) 2)"), "{got}");
}

#[test]
fn for_body_return_and_question_outside_fn() {
    // D42: a `for` body and a lambda `=>` cannot return early (E0237); an
    // anonymous `fn(x): … ;;` and a named fn can.
    let cases = [
        ("for x in xs: return x ;;", "E0237"),
        ("for x in xs: if x: return 1 ;; ;;", "E0237"),
        ("for x in xs: (fn(): return x ;;)() ;;", ""), // a nested fn may return
        ("for _x in xs: (() => 1)() ;;", ""),
        ("fn f(xs) = for x in xs: return x ;;", "E0237"),
        ("fn f(xs) = for x in xs: x? ;;", "E0237"),
        ("fn f(xs) = for x in xs: g(x?) ;;", "E0237"),
        ("fn f(xs) = for x in xs: (fn(): x? ;;)() ;;", ""), // a nested fn may return
        ("fn f(xs) = map(xs, x => x?)", "E0237"),
        ("fn f(xs) = xs |> map(x => g(x?))", "E0237"),
        ("fn f(xs) = map(xs, fn(x): x? ;;)", ""), // anonymous fn: may return
        ("fn f(xs) = map(xs, f: x => x?)", "E0237"),
        ("fn f(xs) = map(xs, f: x => x?)", "E0237"), // whatever the callee
        ("fn f(xs) = filter(xs, x => x?)", "E0237"),
        ("fn f(map, xs) = map(xs, x => x?)", "E0237"),
        (
            "fn f(xs) = filter(xs, x => x > 1 then return true else false)",
            "E0237",
        ),
        ("fn f(xs) = filter(xs, x => return x)", "E0237"),
        ("fn f(xs) = map(xs, x => (fn(): x? ;;)())", ""), // a nested fn may return
        ("fn f(xs) = map(xs, x => (y => y?)(x))", "E0237"),
        ("fn h(x) = x?\nfn f(xs) = map(xs, h)", ""), // a named fn may
        ("fn f(xs) = map(xs, g(_, xs?))", ""),       // a partial's arguments are outside
        ("f()?", "E0214"),
        ("fn g() = f()?", ""),
        ("_ = print(_)", ""), // hole: fine
        ("_ + 1", "E0215"),
        ("x = _", "E0215"),
    ];
    for (src, want) in cases {
        // `core_of` keeps the last value, as the REPL does: a `for` there
        // is a `map`, not a discarded one.
        let opts = Options {
            keep_value: true,
            ..Options::default()
        };
        let (_, errs) = resolve_with(src, M4_PRELUDE, &opts);
        assert_eq!(first_code(&not_hiding(&errs)), want, "{src:?}: {errs:?}");
    }
}

/// Every top-level name is bound once (D53), so each is a constant a
/// top-level fn sees (03, L9).
#[test]
fn top_level_fns_see_names_assigned_once() {
    for src in [
        "x = 1\nfn f() = x",
        "fn f() = x\nx = 1",
        "(a, b) = (1, 2)\nfn f() = a + b",
        "x = 1\nfn f() = () => x",
        "x = 1\nfn f():\n  fn g() = x\n  g()\n;;",
    ] {
        let (_, errs) = res(src);
        assert!(errs.is_empty(), "{src:?}: {errs:?}");
    }
    // A second binding at the top level is E0238, by assignment or by a
    // pattern, wherever the first one was.
    for src in [
        "x = 1\nx = 2\nfn f() = x",
        "x = 1\nx = x + 1",
        "(a, b) = (1, 2)\na = 3",
        "a = 3\n[a, b] = [1, 2]",
        "x = 1\nprint(x)\nx = 2",
    ] {
        let (_, errs) = res(src);
        assert_eq!(codes(&errs), vec!["E0238"], "{src:?}");
    }
    let (_, errs) = res("n = 0\nn = n + 1");
    assert_eq!(arg0(&errs[0]), "n");
    assert_eq!((errs[0].pos.line, errs[0].pos.col), (2, 1));
    assert!(
        errs[0].notes.iter().any(|n| n.contains("line 1")),
        "{:?}",
        errs[0].notes
    );
    // A block's own names are bound once as well (D88).
    let (_, errs) = res("fn f():\n  x = 1\n  x = x + 1\n  x\n;;\nprint(f())");
    assert_eq!(codes(&errs), vec!["E0238"]);
    // A lambda is still made where it stands: top-down.
    let (_, errs) = res("f = () => x\nx = 1");
    assert_eq!(first_code(&errs), "E0204");
    // A top-level fn cannot hide the constant with a local (D38).
    let (_, errs) = res("x = 1\nfn f():\n  x = 2\n  x\n;;");
    assert_eq!(codes(&errs), ["E0238"]);
}

#[test]
fn type_aliases() {
    let cases = [
        ("type shape = Circle | Rect\nfn area(s: shape) = s", ""),
        ("fn area(s: shape) = s\ntype shape = Circle", ""), // hoisted
        ("type a = b\ntype b = int\nfn f(x: a) = x", ""),
        ("type a = a\nfn f(x: a) = x", "E0218"),
        ("type a = b\ntype b = a\nfn f(x: a) = x", "E0218"),
        ("type a = int\ntype a = str", "E0217"),
        ("fn f(): type a = int ;;", "E0219"),
        ("type a = zzz", "E0212"),
        // a built-in type name cannot be an alias: `int` would mean
        // something else in this file only (07)
        ("type int = str\nfn f(x: int) = x", "E0222"),
        ("type any = int", "E0222"),
        ("type num = int | float", "E0222"),
        ("type int = int", "E0222"),
    ];
    for (src, want) in cases {
        let (_, errs) = res(src);
        assert_eq!(first_code(&errs), want, "{src:?}: {errs:?}");
    }
    let r = ok("type shape = Circle | Rect\nfn area(s: shape | nil) = s");
    let info = fn_info(&r.core, &[0, 1]);
    let names: Vec<&str> = info.params[0]
        .types
        .as_ref()
        .unwrap()
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(names.join("|"), "Circle|Rect|nil");
}

/// FE40: each alias is expanded once and a union keeps a name once, so
/// doubling aliases and long chains are linear, not exponential.
#[test]
fn alias_expansion_is_linear() {
    let mut src = String::from("type t0 = int | Red\n");
    for i in 1..=60 {
        src.push_str(&format!("type t{i} = t{} | t{}\n", i - 1, i - 1));
    }
    src.push_str("fn f(x: t60 | int) = x");
    let r = ok(&src);
    let info = fn_info(&r.core, &[0, 1]);
    let names: Vec<&str> = info.params[0]
        .types
        .as_ref()
        .unwrap()
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(names.join("|"), "int|Red");
    let mut src = String::from("type t0 = int\n");
    for i in 1..20_000 {
        src.push_str(&format!("type t{i} = t{}\n", i - 1));
    }
    ok(&src);
}

#[test]
fn rest_param_is_allowed_now() {
    let r = ok("fn f(_a, ...xs) = xs");
    assert!(fn_info(&r.core, &[0, 1]).params[1].rest);
}

// ---- modules (M5) ----

fn m5_opts(library: bool) -> Options {
    let mut imports = BTreeMap::new();
    imports.insert(
        "fs".to_string(),
        ModuleInfo {
            key: "fs".into(),
            name: "fs".into(),
            exports: names(&["read", "write"]),
            ..Default::default()
        },
    );
    imports.insert(
        "./lib/x".to_string(),
        ModuleInfo {
            key: "/abs/lib/x.fung".into(),
            name: "x".into(),
            exports: names(&["a", "b"]),
            ..Default::default()
        },
    );
    Options {
        imports,
        library,
        ..Options::default()
    }
}

fn resolve_m5(src: &str, lib: bool) -> (Option<Resolved>, Vec<String>) {
    let (r, errs) = resolve_with(src, &["print", "map"], &m5_opts(lib));
    (r, codes(&errs))
}

#[test]
fn import_binds_constant_globals() {
    let (r, errs) = resolve_m5(
        "import fs\nimport \"./lib/x\" as y (a)\nprint(fs.read, y.b, a)",
        false,
    );
    assert!(errs.is_empty(), "{errs:?}");
    let r = r.unwrap();
    let got = r.core.to_string();
    for want in [
        "(import \"fs\" (global 0))",
        "(import \"/abs/lib/x.fung\" (global 1) (member a (global 2)))",
        "(field read (global 0))",
    ] {
        assert!(got.contains(want), "missing {want} in {got}");
    }
    assert_eq!(r.imports, names(&["fs", "/abs/lib/x.fung"]));
    assert_eq!(r.globals, names(&["fs", "y", "a"]));
    let (r, _) = resolve_m5(
        "import fs\nimport \"./lib/x\" (a)\n_p = 1\nfn f() = fs.read(a)\nq = 2",
        true,
    );
    // imports and private names must be excluded
    assert_eq!(r.unwrap().exports, names(&["f", "q"]));
}

#[test]
fn import_errors() {
    let cases = [
        ("import nope", "E0301"),
        ("import fs\nprint(fs.nope)", "E0303"),
        ("import \"./lib/x\" (nope)", "E0303"),
        ("import fs\nprint(fs._secret)", "E0304"),
        ("import fs\nimport fs", "E0309"),
        ("import fs\nimport \"./lib/x\" as fs", "E0309"),
        ("import fs\nfs = 1", "E0238"),
        ("import fs\nfn fs() = 1", "E0238"),
        ("import fs\nfn f(fs) = fs", "E0238"),
        ("import fs\nxs = for fs in [1]: fs ;;\nprint(xs)", "E0238"),
        ("import fs\nmatch 1: fs => fs ;;", "E0238"),
        ("fn f():\n  import fs\n  1\n;;", "E0311"),
        ("import \"./lib/x\" (a)\na = 2", "E0238"), // listed names are constants (plan decision 4)
        ("import \"./lib/x\" (a)\nfn g() = a", ""), // and hoisted fns see them
        ("import \"./lib/x\" (a)\nprint(a, x)", "E0204"), // the module name is not bound when only names are listed
        ("fn f():\n  map = 1\n  map\n;;", "E0238"), // a local cannot hide a prelude name (D38)
        ("fn map(xs) = xs\nprint(map(1))", ""), // a public top-level name can: the file's interface (D86)
    ];
    for (src, want) in cases {
        let (_, errs) = resolve_m5(src, false);
        assert_eq!(
            errs.first().cloned().unwrap_or_default(),
            want,
            "{src:?}: {errs:?}"
        );
    }
}

/// A diagnostic about the name an import binds underlines that name as
/// written (the module, its alias or the path string), not the start of
/// the `import` keyword.
#[test]
fn import_diagnostics_point_at_the_bound_name() {
    let at = |d: &Diag| (d.code.clone(), d.pos.line, d.pos.col, d.len);
    let unused = |src: &str| {
        let (_, errs) = resolve_with(src, &["print"], &m5_opts(false));
        errs.iter().map(at).collect::<Vec<_>>()
    };
    assert_eq!(unused("import fs"), [("E0239".into(), 1, 8, 2)]);
    assert_eq!(unused("import fs as f"), [("E0239".into(), 1, 14, 1)]);
    assert_eq!(unused("import \"./lib/x\""), [("E0239".into(), 1, 8, 9)]);
    let failed = |src: &str| {
        let (_, errs) = resolve_with(src, &["print"], &m5_opts(false));
        errs.iter().map(at).collect::<Vec<_>>()
    };
    assert_eq!(
        failed("import fs\nimport fs\nprint(fs)"),
        [("E0309".into(), 2, 8, 2)]
    );
    assert_eq!(
        failed("import fs\nimport \"./lib/x\" as fs\nprint(fs)"),
        [("E0309".into(), 2, 21, 2)]
    );
}

#[test]
fn shadowing_help_names_the_import_as_written() {
    // E0238's help for a module name shows how to import the module under another name,
    // with the module it really is (not the alias it is bound to).
    let help_of = |src: &str| {
        let (_, errs) = resolve_with(src, &["print"], &m5_opts(false));
        assert_eq!(codes(&errs), vec!["E0238"], "{src:?}");
        help(&errs[0]).to_string()
    };
    assert_eq!(
        help_of("import fs\nfs = 1"),
        "import it under another name: `import fs as m`"
    );
    assert_eq!(
        help_of("import fs as f\nf = 1"),
        "import it under another name: `import fs as m`"
    );
    assert_eq!(
        help_of("import \"./lib/x\" as y\nfn f(y) = y"),
        "import it under another name: `import \"./lib/x\" as m`"
    );
}

#[test]
fn alternatives_binding_different_names_do_not_cascade() {
    // 08: one cause, one error — the names used in the arm are not also
    // reported as undefined.
    let (_, errs) = res("match [1]: [a] | [a, ...rest] => print(a, rest) ;;");
    assert_eq!(codes(&errs), vec!["E0216"]);
}

#[test]
fn library_top_level_restriction() {
    let cases = [
        (
            "import fs\nfn f() = fs.read\ntype t = int\nx = print(1)\n(a, b) = (1, 2)",
            "",
        ),
        ("print(1)", "E0306"),
        ("fn f() = 1\nf()", "E0306"),
        ("1 + 1", "E0306"),
        ("if true: 1 ;;", "E0306"),
    ];
    for (src, want) in cases {
        let (_, errs) = resolve_m5(src, true);
        assert_eq!(
            errs.first().cloned().unwrap_or_default(),
            want,
            "{src:?}: {errs:?}"
        );
    }
    let (_, errs) = resolve_m5("print(1)", false);
    assert!(errs.is_empty(), "the entry file may have statements");
}

#[test]
fn duplicate_pattern_names() {
    let cases = [
        ("[a, a] = [1, 2]", "E0221"),
        ("match 1: (a, a) => a ;;", "E0221"),
        ("match 1: {x: a, y: a} => a ;;", "E0221"),
        ("match 1: [a, ...a] => a ;;", "E0221"),
        ("match 1: Ok(a) | Err(a) => print(a) ;;", ""), // the same name once per alternative is fine
        ("match 1: [a, b] when a == b => print(a) ;;", ""),
        ("_ = print(1)", ""),
    ];
    for (src, want) in cases {
        let (_, errs) = resolve_m5(src, false);
        assert_eq!(
            errs.first().cloned().unwrap_or_default(),
            want,
            "{src:?}: {errs:?}"
        );
    }
}

/// Each unused or hiding name (D38) as `CODE:name`, in the order
/// reported.
fn name_errors_of(src: &str, opts: &Options) -> Vec<String> {
    let (_, errs) = resolve_with(src, &["print", "map", "len"], opts);
    errs.iter()
        .map(|e| format!("{}:{}", e.code, name_of(e)))
        .collect()
}

/// The name a name error is about: E0239's is its second argument.
fn name_of(e: &Diag) -> String {
    match (e.code.as_str(), e.args.get(1)) {
        ("E0239", Some(Arg::Str(s))) => s.clone(),
        _ => arg0(e),
    }
}

#[test]
fn unused_and_hiding_names() {
    let mut imports = BTreeMap::new();
    imports.insert(
        "fs".to_string(),
        ModuleInfo {
            key: "fs".into(),
            name: "fs".into(),
            exports: names(&["read"]),
            ..Default::default()
        },
    );
    let opts = Options {
        imports,
        ..Options::default()
    };
    let cases: &[(&str, &[&str])] = &[
        ("import fs", &["E0239:fs"]),
        ("import fs as f\nprint(f.read)", &[]),
        ("import fs (read)", &["E0239:read"]),
        ("import fs (read)\nprint(read)", &[]),
        ("fn f():\n  x = 1\n  2\n;;", &["E0239:x"]),
        ("fn f():\n  _x = 1\n  2\n;;", &[]),
        ("fn f(a) = 1", &["E0239:a"]),
        ("fn f(_a) = 1", &[]),
        ("fn f(a, b = a) = b", &[]), // read by a later default
        ("f = x => 1\nprint(f)", &["E0239:x"]),
        ("print(for x in [1]: 2 ;;)", &["E0239:x"]),
        ("x = 1", &[]), // a public top-level name is the file's interface (D86)
        ("if true:\n  y = 1\n;;", &["E0239:y"]), // a block's name at the top level is a local
        ("_x = 1", &[]),
        ("fn f() = 1", &[]), // a top-level fn is not a variable
        ("fn f():\n  (a, b) = (1, 2)\n  a\n;;", &["E0239:b"]),
        ("fn f():\n  (a, _b) = (1, 2)\n  a\n;;", &[]),
        ("fn f(v) = match v: (a, _) => 1 ;;", &["E0239:a"]),
        ("fn f(v) = match v: x when x > 0 => 1; _ => 0 ;;", &[]),
        // A private top-level name and a private fn's parameter are no
        // interface: they cannot take a prelude name (D86).
        ("fn _f():\n  map = 1\n  map\n;;", &["E0238:map"]),
        ("fn _f(len) = len", &["E0238:len"]),
        ("fn f(v) = match v: map => map ;;", &["E0238:map"]),
        ("fn f(v) = match v: v => v ;;", &["E0238:v"]),
        ("fn f(v) = map([1], v => v)", &["E0238:v"]),
        ("fn f(v) = for v in [1]: v ;;", &["E0238:v"]),
        ("fn g() = 1\nfn f(g) = g", &["E0238:g"]),
        ("fn g() = 1\nfn f():\n  g = 2\n  g\n;;", &["E0238:g"]),
        ("c = 1\nfn f():\n  c = 2\n  c\n;;\nprint(c)", &["E0238:c"]),
        (
            "c = 1\nfn f():\n  fn c() = 2\n  c()\n;;\nprint(c)",
            &["E0238:c"],
        ),
        ("c = 1\nfn f(c) = c\nprint(c)", &["E0238:c"]),
        ("import fs (read)\nfn f(read) = read", &["E0238:read"]),
        ("fn f():\n  x = 1\n  fn g() = x\n  g()\n;;", &[]), // read by a nested fn counts
        ("fn f():\n  x = 1\n  x = 2\n  x\n;;", &["E0238:x"]), // bound once (D88)
        ("fn f():\n  x = 1\n  h = () => x\n  h()\n;;", &[]), // a lambda's read counts
        ("x = 1\ntest \"t\":\n  assert x == 1\n;;", &[]),
        ("x = 1\nfn f() = x", &[]), // read by a top-level fn
        ("fn f(xs) = for (a, b) in xs: a ;;", &["E0239:b"]),
        ("fn f(v):\n  {a, b} = v\n  a\n;;", &["E0239:b"]),
        ("fn f(v) = match v: Ok(x) | Err(x) => 1 ;;", &["E0239:x"]),
        ("fn f(v) = match v: Ok(x) | Err(x) => x ;;", &[]),
        // Only its own body calls it: unused.
        (
            "fn f():\n  fn g(n) = if n > 0: g(n - 1) else: 0 ;;\n  1\n;;",
            &["E0239:g"],
        ),
        ("import fs\nfn f() = fs.read", &[]), // read by a top-level fn
        (
            "fn f():\n  b = 1\n  a = 2\n  0\n;;",
            &["E0239:b", "E0239:a"],
        ), // sorted by position
    ];
    for (src, want) in cases {
        assert_eq!(name_errors_of(src, &opts), *want, "{src:?}");
    }
    // An imported file's public names are its interface: unread, and
    // prelude names, they are not errors; its private ones are (D38).
    let lib = Options {
        library: true,
        ..Options::default()
    };
    assert_eq!(
        name_errors_of("fn map(xs) = xs\nlen = 1", &lib),
        [] as [&str; 0]
    );
    assert_eq!(
        name_errors_of("fn get(m, len = 1) = [m, len]", &lib),
        [] as [&str; 0]
    );
    assert_eq!(
        name_errors_of("fn _get(m, len = 1) = [m, len]\nx = 1\n_y = 2", &lib),
        ["E0238:len"]
    );
    assert_eq!(
        name_errors_of("fn get(m):\n  len = 1\n  [m, len]\n;;", &lib),
        ["E0238:len"],
        "a local of a public fn is no interface"
    );
    // A rest parameter is never given by name (E0406): no interface, as
    // a pattern parameter is not (FE82).
    assert_eq!(
        name_errors_of("fn get(...len) = len", &lib),
        ["E0238:len"],
        "a rest parameter is no interface"
    );
    assert_eq!(
        name_errors_of("fn get((len, b)) = [len, b]", &lib),
        ["E0238:len"]
    );
    // The rule is the file's own, however the file is loaded (D86): the
    // same holds when the file is the entry, so a library can be checked
    // and tested by itself.
    for src in [
        "fn map(xs) = xs\nlen = 1",
        "fn get(m, len = 1) = [m, len]",
        "max_size = 10\nfn double(n) = n * 2",
    ] {
        assert_eq!(
            name_errors_of(src, &Options::default()),
            [] as [&str; 0],
            "{src:?}"
        );
    }
    assert_eq!(
        name_errors_of(
            "fn _get(m, len = 1) = [m, len]\nx = 1\n_y = 2",
            &Options::default()
        ),
        ["E0238:len"]
    );
    // The REPL keeps every top-level name and import for later inputs.
    let mut repl_opts = repl(&[], true);
    repl_opts.imports = opts.imports.clone();
    assert_eq!(
        name_errors_of("import fs\ny = 1", &repl_opts),
        [] as [&str; 0]
    );
    assert_eq!(
        name_errors_of("fn f():\n  x = 1\n  2\n;;", &repl_opts),
        ["E0239:x"],
        "a fn's locals still count"
    );
}

/// A value that is used must not be a silent nil (01, L3): an if without
/// else is E0223, a block that is empty or ends in a statement E0224
/// (was W0104; E0231 merged into it, SP24).
#[test]
fn values_used_must_not_be_nil() {
    let cases: &[(&str, &[&str])] = &[
        ("fn f():\n  x = if true: 1 ;;\n  x\n;;", &["E0223"]),
        (
            "fn f():\n  x = if true: _y = 1 else: 2 ;;\n  x\n;;",
            &["E0224"],
        ),
        ("print(if true: _y = 1 else: 1 ;;)", &["E0224"]),
        ("x = if true: (_a, _b) = (1, 2) else: 0 ;;", &["E0224"]),
        ("fn f(v):\n  return match v: 1 => _y = 1 ;;\n;;", &["E0224"]),
        ("x = if false: 1 else if true: 2 ;;", &["E0223"]),
        ("x = if true: if false: 1 ;; else: 2 ;;", &["E0223"]),
        ("x = 1 + (if true: 1 ;;)", &["E0223"]),
        ("x = [if true: 1 ;;]", &["E0223"]),
        ("x = \"${if true: 1 ;;}\"", &["E0223"]),
        ("assert if true: true ;;", &["E0223"]),
        ("fn f(a = if true: 1 ;;) = a", &["E0223"]),
        ("fn f() = if true: 1 ;;", &["E0223"]),
        ("f = x => if x: 1 ;;", &["E0223"]),
        ("ys = for x in [1]: if x: 1 ;; ;;", &["E0223"]),
        ("print(for x in [1]: if x: 1 ;; ;;)", &["E0223"]),
        // A block-bodied function's last item is its value.
        ("fn f():\n  if true: 1 ;;\n;;", &["E0223"]),
        ("f = fn(x): if x: 1 ;; ;;", &["E0223"]),
        ("fn f():\n  _ = 1\n;;", &["E0224"]),
        ("fn f(x):\n  y = x\n;;", &["E0224"]),
        ("fn f(x):\n  if x: 1 else: nil ;;\n;;", &[]),
        ("fn f(x):\n  if x: print(1) ;;\n  nil\n;;", &[]),
        // Nobody uses the value.
        ("if true: print(1) ;;\n1", &[]),
        ("for x in [1]: if x: print(1) ;; ;;\n1", &[]),
        ("for x in [1]: if x: 1 ;; ;;", &[]), // the REPL's last value
        (
            "x = if true:\n  if false: print(1) ;;\n  2\nelse:\n  3\n;;",
            &[],
        ),
        ("test \"t\":\n  if true: print(1) ;;\n;;", &[]),
        ("x = if false: 1 else: nil ;;", &[]),
        ("x = true then 1 else nil", &[]),
        // `then … else` always has its else, whatever its branches turn
        // into (a match on a call, a partial application): FE63.
        ("x = true then match print(1): _ => 1 ;; else nil", &[]),
        ("x = true then print(_, 1) else nil", &[]),
        ("x = false then nil else match print(1): _ => 1 ;;", &[]),
        // As a statement its branches are discarded, as a statement
        // `if`'s are (D121), the same whether the match is on a name or a
        // call: an `if` without `else` is fine there, a bare value is not.
        (
            "true then match print(1): 1 => if true: print(1) ;; ;; else print(2)\n1",
            &[],
        ),
        (
            "y = 1\ntrue then match y: 1 => if true: print(1) ;; ;; else print(2)\n1",
            &[],
        ),
        (
            "true then match print(1): 1 => 5 ;; else print(2)\n1",
            &["E0234"],
        ),
        (
            "y = 1\ntrue then match y: 1 => 5 ;; else print(2)\n1",
            &["E0234"],
        ),
        // used, its branches are used
        (
            "x = true then match print(1): 1 => if true: print(1) ;; ;; else nil\nx",
            &["E0223"],
        ),
        (
            "fn f():\n  x = if true:\n    y = 1\n    y\n  else: 0 ;;\n  x\n;;",
            &[],
        ),
    ];
    for (src, want) in cases {
        // The cases' names go unread: E0239 is not what they test.
        let (_, errs) = res(src);
        let errs: Vec<Diag> = errs.into_iter().filter(|e| e.code != "E0239").collect();
        assert_eq!(codes(&errs), *want, "{src:?}");
    }
    // One number, one message: the argument says what the block is or
    // ends with, and the caret is at that statement (FE28, SP24).
    for (src, what, col, len) in [
        (
            "x = if true: _y = 1 else: 2 ;;",
            "ends with an assignment to `_y`",
            14,
            2,
        ),
        (
            "x = if true: (_a, _b) = (1, 2) else: 0 ;;",
            "ends with an assignment to a pattern",
            14,
            1,
        ),
        (
            "x = [for i in [1]: _ = i ;;]",
            "ends with an assignment to `_`",
            20,
            1,
        ),
        ("x = if false: 1 else: ;;", "is empty", 17, 1),
        // an empty arm is an empty block (D121)
        ("x = match 1: 1 => 2; _ => ;;", "is empty", 24, 1),
        (
            "x = if true: assert true else: 0 ;;",
            "ends with `assert`",
            14,
            6,
        ),
        (
            "x = if true:\n  fn g() = 1\nelse: 0 ;;",
            "ends with the definition of `g`",
            3,
            2,
        ),
    ] {
        let (_, errs) = res(src);
        assert_eq!(errs.len(), 1, "{src:?}: {errs:?}");
        let e = &errs[0];
        assert_eq!(
            (e.code.as_str(), arg0(e).as_str(), e.pos.col, e.len.max(1)),
            ("E0224", what, col, len),
            "{src:?}"
        );
    }
}

/// A discarded `nil` says how to write a branch that does nothing (D121).
#[test]
fn a_discarded_nil_points_at_an_empty_branch() {
    let (_, errs) = res_discard("match 1:\n  1 => print(1)\n  _ => nil\n;;\nprint(1)");
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert_eq!(arg0(&errs[0]), "`nil`");
    assert_eq!(
        errs[0].help.as_deref(),
        Some("a branch with nothing to do is left empty: `_ =>` in a match, no `else` after an if")
    );
}

/// `1 => y => y + 1` is an empty arm and an arm `y`: used as a value the
/// empty arm is E0224, whose help says how to write a lambda as the
/// arm's value (FE25, D121).
#[test]
fn an_empty_arm_before_a_name_arm_hints_at_a_lambda() {
    let (_, errs) = res("f = match 1: 1 => y => y + 1 ;;\nprint(f)");
    let e = errs.iter().find(|e| e.code == "E0224").expect("E0224");
    assert_eq!(
        e.help.as_deref(),
        Some("a lambda as an arm's value goes in parentheses: `1 => (y => y + 1)`")
    );
    let (_, errs) = res("f = match 1: 1 => ; _ => 2 ;;\nprint(f)");
    let e = errs.iter().find(|e| e.code == "E0224").expect("E0224");
    assert_eq!(e.help, None);
}

/// Binding a visible name again is one error, E0238, however the name is
/// bound and wherever it comes from (D122: E0205, E0210, E0240 and E0308
/// retired). The note tells where the name is from, and the help what to
/// write instead.
#[test]
fn binding_a_visible_name_again_is_one_error() {
    const FRESH: &str = "a visible name means one thing: give the new one a name of its own";
    const LOOP: &str = "a name is bound once, and a block cannot bind an outer one again; build the value with an expression instead, e.g. `x = if … ;;` or `fold`";
    const MODULE: &str = "import it under another name: `import fs as m`";
    const FN: &str = "fung has no overloading: give each function a name of its own";
    let cases: &[(&str, &str, Option<&str>)] = &[
        ("x = 1\nx = 2", "note: `x` is first bound on line 1", None),
        (
            "fn f() = 1\nfn f() = 2",
            "note: `f` is a function defined on line 1",
            Some(FN),
        ),
        (
            "fn g():\n  fn f() = 1\n  fn f() = 2\n  f\n;;",
            "note: `f` is a function defined on line 2",
            Some(FN),
        ),
        (
            "fn f() = 1\nf = 2",
            "note: `f` is a function defined on line 1",
            None,
        ),
        (
            "import \"./lib/x\" (a)\na = 1",
            "note: `a` is imported on line 1",
            None,
        ),
        (
            "import \"./lib/x\" (a)\nfn a() = 1",
            "note: `a` is imported on line 1",
            Some(FN),
        ),
        (
            "total = 0\nfor x in [1]: total = total + x ;;",
            "note: `total` is a top-level name bound on line 1",
            Some(LOOP),
        ),
        (
            "fn g():\n  t = 0\n  for x in [1]: t = t + x ;;\n  t\n;;",
            "note: `t` is bound outside this block, on line 2",
            Some(LOOP),
        ),
        (
            "limit = 1\nfn f(n):\n  limit = n\n  limit\n;;",
            "note: `limit` is a top-level name bound on line 1",
            Some(LOOP),
        ),
        (
            "fn g():\n  y = 1\n  f = y => y\n  f(y)\n;;",
            "note: `y` is bound outside this block, on line 2",
            Some(FRESH),
        ),
        (
            "fn g() = 1\nfn f(g) = g",
            "note: `g` is a function defined on line 1",
            Some(FRESH),
        ),
        (
            "import \"./lib/x\" (a)\nfn f(a) = a",
            "note: `a` is imported on line 1",
            Some(FRESH),
        ),
        (
            "fn f(v) = for print in [v]: print ;;",
            "note: `print` is a prelude function",
            Some(FRESH),
        ),
        (
            "import fs\nfs = 1",
            "note: `fs` is the module imported on line 1",
            Some(MODULE),
        ),
        (
            "import fs\nfn fs() = 1",
            "note: `fs` is the module imported on line 1",
            Some(MODULE),
        ),
        (
            "import fs\nfn f(fs) = fs",
            "note: `fs` is the module imported on line 1",
            Some(MODULE),
        ),
    ];
    for (src, note, help) in cases {
        let (_, errs) = resolve_with(src, &["print"], &m5_opts(false));
        // (a statement `for` that ends a library file is E0234 here)
        let errs: Vec<Diag> = errs.into_iter().filter(|e| e.code != "E0234").collect();
        assert_eq!(codes(&errs), ["E0238"], "{src:?}: {errs:?}");
        let e = &errs[0];
        assert!(e.notes.iter().any(|n| n == note), "{src:?}: {:?}", e.notes);
        assert_eq!(e.help.as_deref(), *help, "{src:?}");
    }
}

#[test]
fn name_errors_carry_positions_and_spans() {
    let src = "fn f():\n  len = 2\n  0\n;;";
    let (_, errs) = resolve_with(src, &["len"], &Options::default());
    let at: Vec<_> = errs
        .iter()
        .map(|e| (e.code.as_str(), e.pos.line, e.pos.col, e.len))
        .collect();
    assert_eq!(at, [("E0238", 2, 3, 3)]);
    let src = "fn f():\n  fn len(_x) = 1\n  len(0)\n;;";
    let (_, errs) = resolve_with(src, &["len"], &Options::default());
    assert_eq!(
        (errs[0].code.as_str(), errs[0].pos.col, errs[0].len),
        ("E0238", 6, 3),
        "at the fn's name, not at `fn`"
    );
    let (_, errs) = resolve_with(
        "c = 1\nfn f(c) = c\nprint(c)",
        &["print"],
        &Options::default(),
    );
    assert_eq!(
        errs[0].notes,
        ["note: `c` is a top-level name bound on line 1"]
    );
}

fn repl(globals: &[&str], redefine: bool) -> Options {
    Options {
        globals: names(globals),
        redefine,
        ..Options::default()
    }
}

/// The REPL's earlier globals keep
/// their indexes, and with `redefine` a known fn may be defined again.
#[test]
fn repl_options() {
    let (r, errs) = resolve_with(
        "fn g() = 2\nprint(x, g())",
        &["print"],
        &repl(&["x", "g"], true),
    );
    assert!(errs.is_empty(), "{errs:?}");
    let r = r.unwrap();
    assert_eq!(
        r.globals,
        names(&["x", "g"]),
        "existing names keep their indexes"
    );
    assert_eq!(
        r.core.to_string(),
        "(do (let (global 1) (fn g() (do 2))) (call (prelude print) (global 0) (call (global 1))))"
    );
    let (_, errs) = resolve_with("fn g() = 2", &["print"], &repl(&["g"], false));
    assert_eq!(
        codes(&errs),
        ["E0238"],
        "without redefine a known fn is a duplicate"
    );
    let (r, errs) = resolve_with("y = 1\nprint(y)", &["print"], &repl(&["x"], false));
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(r.unwrap().globals, names(&["x", "y"]), "new globals append");
    let (r, errs) = resolve_with("x = 5", &["print"], &repl(&["x"], true));
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(r.unwrap().globals.len(), 1, "reassigning a known global");
    // FE23 in the REPL: an inner block of a top-level fn cannot assign an
    // earlier input's global or a name of the same input either.
    let src = "fn g():\n  if true:\n    c = 2\n  ;;\n  c\n;;";
    let (_, errs) = resolve_with(src, &["print"], &repl(&["c"], true));
    assert_eq!(codes(&errs), ["E0238"], "{errs:?}");
    let src = "c = 1\nfn g():\n  if true:\n    c = 2\n  ;;\n  c\n;;";
    let (_, errs) = resolve_with(src, &["print"], &repl(&[], true));
    assert_eq!(codes(&errs), ["E0238"], "{errs:?}");
}

/// Redefinition inside one input (`redefine`): a plain assignment replaces a fn
/// or a module, an import replaces a module, a fn replaces a fn. Without
/// `redefine` each is an error.
#[test]
fn repl_redefinition_in_one_input() {
    let info = ModuleInfo {
        key: "fs".into(),
        name: "fs".into(),
        exports: names(&["exists"]),
        ..Default::default()
    };
    let mut imports = BTreeMap::new();
    imports.insert("fs".to_string(), info);
    let with = |redefine| Options {
        imports: imports.clone(),
        redefine,
        keep_value: redefine,
        ..Options::default()
    };
    for (src, plain) in [
        ("fn f() = 1\nf = 2\nf", "E0238"),
        ("import fs\nfs = 1\nfs", "E0238"),
        ("import fs\nimport fs\nfs.exists", "E0309"),
        ("fn f() = 1\nfn f() = 2\nf()", "E0238"),
        ("x = 1\nx = 2\nx", "E0238"),
    ] {
        let (_, errs) = resolve_with(src, &["print"], &with(true));
        assert!(errs.is_empty(), "{src:?}: {errs:?}");
        let (_, errs) = resolve_with(src, &["print"], &with(false));
        assert_eq!(first_code(&errs), plain, "{src:?}");
    }
    // A name a plain assignment took back from a module is a plain name.
    let (_, errs) = resolve_with("import fs\nfs = 1\ny = fs", &["print"], &with(true));
    assert!(errs.is_empty(), "{errs:?}");
    // Earlier globals are plain names: an import may take one over.
    let opts = Options {
        globals: names(&["fs"]),
        ..with(true)
    };
    let (r, errs) = resolve_with("import fs\nfs.exists", &["print"], &opts);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(r.unwrap().globals, names(&["fs"]));
}

/// The catalog's help is the default help (08, D-2): an error that always
/// writes its own help line (to name the variable or the module) has a
/// default in the catalog too, and says the same up to the first code
/// span, so the catalog and the manual show what users see.
#[test]
fn errors_that_always_have_a_help_line_have_its_default_in_the_catalog() {
    let opts = m5_opts(false);
    let (src, code) = ("if true: y = 1 ;;\nprint(y)", "E0206");
    let (_, errs) = resolve_with(src, &["print"], &opts);
    assert_eq!(codes(&errs), vec![code], "{src:?}");
    let default = &crate::diag::catalog::entry(code).unwrap().help;
    assert!(!default.is_empty(), "{code} has no default help");
    let lead = |h: &str| h.split('`').next().unwrap_or("").to_string();
    assert_eq!(lead(help(&errs[0])), lead(default), "{code}");
    // the import as written, with the module and not the name bound to it
    let (_, errs) = resolve_with("import fs as f\nf = 1", &["print"], &opts);
    assert_eq!(
        help(&errs[0]),
        "import it under another name: `import fs as m`"
    );
}

/// A lambda given to `conc.pmap` is a lambda like any other: `?` in it
/// is E0237, through the module or imported by name (D42).
#[test]
fn question_in_a_lambda_given_to_pmap() {
    let mut imports = BTreeMap::new();
    imports.insert(
        "conc".to_string(),
        ModuleInfo {
            key: "conc".into(),
            name: "conc".into(),
            exports: names(&["pmap", "race"]),
            ..Default::default()
        },
    );
    let opts = Options {
        imports,
        ..Options::default()
    };
    for (src, want) in [
        ("import conc\nfn f(xs) = conc.pmap(xs, x => x?)", "E0237"),
        ("import conc (pmap)\nfn f(xs) = pmap(xs, x => x?)", "E0237"),
        ("import conc\nfn f(xs) = conc.race([() => g()?])", "E0237"),
        ("import conc\nfn f(_xs) = conc.race([fn(): g()? ;;])", ""),
    ] {
        let (_, errs) = resolve_with(src, M4_PRELUDE, &opts);
        assert_eq!(first_code(&not_hiding(&errs)), want, "{src:?}: {errs:?}");
    }
    let (_, errs) = resolve_with("fn h(ys) = map(ys, x => x?)", M4_PRELUDE, &opts);
    assert_eq!(
        crate::diag::message(&errs[0].code, &errs[0].args),
        "`?` inside a lambda `=>`"
    );
    let (_, errs) = resolve_with("fn h(ys) = for x in ys: return x ;;", M4_PRELUDE, &opts);
    assert_eq!(
        crate::diag::message(&errs[0].code, &errs[0].args),
        "`return` inside a `for` body"
    );
}

/// Finding a name does not walk every enclosing block: deeply nested
/// blocks that each bind a name resolve in linear time (FE16).
#[test]
fn deep_blocks_resolve_in_linear_time() {
    let n = 9_000;
    let mut src = String::from("fn f(x):\n  match x:\n");
    for i in 0..n {
        src.push_str(&format!("  _v{i} => match x:\n"));
    }
    src.push_str("  _ => x\n");
    src.push_str(&";;\n".repeat(n + 2));
    let elapsed = std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || {
            let core = core_of(&src);
            let t = std::time::Instant::now();
            let r = resolve(&core, &names(&["print"]), &Options::default());
            assert!(r.is_ok());
            t.elapsed()
        })
        .unwrap()
        .join()
        .unwrap();
    assert!(elapsed < std::time::Duration::from_secs(1), "{elapsed:?}");
}

/// A discarded value that can do nothing is E0234 (FE24): names,
/// literals, lambdas, partial applications, and operators, fields,
/// indexes, tuples, lists and maps of only those. `nil` alone, calls,
/// assignments, `assert`, tags and `_ = …` are not.
#[test]
fn effect_free_values_must_not_be_discarded() {
    let cases: &[(&str, &[&str])] = &[
        ("x = 1\nx\nprint(x)", &["E0234"]),
        ("x = 1; -x\nprint(x)", &["E0234"]),
        ("print(1, _)\nprint(2)", &["E0234"]),
        ("x = 1\nx == 1\nprint(x)", &["E0234"]),
        ("x => x\nprint(1)", &["E0234"]),
        ("fn(x): x ;;\nprint(1)", &["E0234"]),
        ("1 + 2\nprint(1)", &["E0234"]),
        ("\"s\"\nprint(1)", &["E0234"]),
        ("'sym\nprint(1)", &["E0234"]),
        ("x = [1]\nx[0]\nprint(1)", &["E0234"]),
        ("x = [1]\nx[0..]\nprint(1)", &["E0234"]),
        ("x = {a: 1}\nx.a\nprint(1)", &["E0234"]),
        ("x = 1\n(x, 1)\nprint(1)", &["E0234"]),
        ("x = 1\n{a: x}\nprint(1)", &["E0234"]),
        ("x = 1\nx ?? 2\nprint(1)", &["E0234"]),
        ("x = 1\n1..x\nprint(1)", &["E0234"]),
        ("x = true\nx and false\nprint(1)", &["E0234"]),
        ("x = true\nx then 1 else 2\nprint(1)", &["E0234"]),
        ("x = 1\n\"v${x}\"\nprint(1)", &["E0234"]),
        ("[print(_)]\nprint(1)", &["E0234"]),
        ("x = 1\nprint(x, _)\nprint(1)", &["E0234"]),
        ("fn g() = 1\nprint(g(), _)\nprint(1)", &[]),
        // The file's last item is discarded in a file.
        ("x = 1\nx", &["E0234"]),
        // A discarded block's last item, and a discarded branch's.
        ("if true: 1 ;;\nprint(1)", &["E0234"]),
        ("match 1: _ => 2 ;;\nprint(1)", &["E0234"]),
        ("for x in [1]: x ;;\nprint(1)", &["E0234"]),
        ("test \"t\":\n  x = 1\n  x == 1\n;;", &["E0234"]),
        // Not effect-free, or not discarded.
        ("print(1)\nprint(2)", &[]),
        ("x = 1\ny = x + 1\nassert y == 2\nprint(y)", &[]),
        ("_ = 1 == 2\n_ = print(_)\nprint(1)", &[]),
        ("fn f(x):\n  y = x * 2\n  y - 1\n;;\nprint(f(1))", &[]),
        // A bare `nil` statement is effect-free like any other (D121).
        (
            "match 1:\n  1 => print(1)\n  _ => nil\n;;\nprint(1)",
            &["E0234"],
        ),
        ("match 1:\n  1 => print(1)\n  _ =>\n;;\nprint(1)", &[]),
        ("match 1: 1 => print(1); _ => ;;\nprint(1)", &[]),
        ("if true: print(1) else: nil ;;\nprint(1)", &["E0234"]),
        ("if true: print(1) ;;\nprint(1)", &[]),
        ("for _ in [1]: nil ;;\nprint(1)", &["E0234"]),
        ("fn f():\n  nil\n  print(1)\n;;\nf()", &["E0234"]),
        ("fn f():\n  print(1)\n  nil\n;;\nf()", &[]),
        ("x = true\nx or fail(\"no\")\nprint(1)", &[]),
        ("x = true\nx and fail(\"no\")\nprint(1)", &[]),
        // `then … else` discards its branches as a statement `if` does
        ("x = true\nx then print(1) else nil\nprint(1)", &["E0234"]),
        ("x = true\nx then 1 else print(2)\nprint(1)", &["E0234"]),
        (
            "x = [1]\nx == [] then print(1) else [x]\nprint(1)",
            &["E0234"],
        ),
        (
            "fn f(c):\n  c then print(1) else print(2)\n  1\n;;\nprint(f)",
            &[],
        ),
        ("fn f(c) = c then nil else 1\nprint(f)", &[]),
        ("Err(1)\nprint(1)", &["E0234"]),
        ("Ok(1)\nprint(1)", &["E0234"]),
        ("Red\nprint(1)", &["E0234"]),
        ("x = 1\nPoint(x, [x])\nprint(1)", &["E0234"]),
        ("Err(print(1))\nprint(1)", &[]),
        ("x = [1]\nOk(fail(x))\nprint(1)", &[]),
        ("nil\nprint(1)", &["E0234"]),
        ("fn f() = 1\nprint(f())", &[]),
    ];
    for (src, want) in cases {
        let (_, errs) = res_discard(src);
        assert_eq!(codes(&errs), *want, "{src:?}");
    }
    // The REPL prints its last value: not discarded.
    let (_, errs) = res("x = 1\nx");
    assert_eq!(codes(&errs), Vec::<String>::new());
    let helps = [
        ("fn g() = 1\ng\nprint(1)", "to call it, add `()`: `g()`"),
        ("x = 1\nx == 2\nprint(1)", "did you mean `=`?"),
        ("print(1, _)\nprint(1)", "a call with `_` makes a function"),
        (
            "test \"t\":\n  x = 1\n  x == 1\n;;",
            "did you mean `assert` before it?",
        ),
    ];
    for (src, want) in helps {
        let (_, errs) = res_discard(src);
        assert!(help(&errs[0]).contains(want), "{src:?}: {errs:?}");
    }
}

/// A call of a builtin known to have no effect, on effect-free arguments,
/// is effect-free too (FE77): a discarded expression made of such calls
/// is E0234, as the call alone is.
#[test]
fn pure_calls_inside_discarded_values() {
    let run = |src: &str| {
        let mut sm = SourceMap::new();
        let f = sm.add("t.fung", src);
        let (toks, _) = lex(f, src).unwrap();
        let core = desugar(&parse(&toks).unwrap(), false).unwrap();
        match resolve(&core, &prelude(), &Options::default()) {
            Ok(_) => Vec::new(),
            Err(es) => es,
        }
    };
    let cases: &[(&str, &[&str])] = &[
        ("xs = [1]\nlen(xs) == 2\nprint(1)", &["E0234"]),
        ("xs = [1]\nupper(\"a\") + \"b\"\nprint(xs)", &["E0234"]),
        ("xs = [1]\nlen(sort(xs)) + 1\nprint(1)", &["E0234"]),
        ("xs = [1]\n[sort(xs)]\nprint(1)", &["E0234"]),
        ("xs = [1]\nsort(xs)[0]\nprint(1)", &["E0234"]),
        ("xs = [1]\nOk(sort(xs))\nprint(1)", &["E0234"]),
        ("xs = [1]\n\"${sort(xs)}\"\nprint(1)", &["E0234"]),
        ("xs = [1]\nnil ?? sort(xs)\nprint(1)", &["E0234"]),
        ("xs = [1]\nsort(xs)\nprint(1)", &["E0234"]),
        ("test \"t\":\n  len([1]) == 2\n;;", &["E0234"]),
        // An argument that may do something keeps the call.
        ("len([print(1)]) == 1\nprint(1)", &[]),
        ("fn g() = [1]\nlen(g()) == 1\nprint(1)", &[]),
    ];
    for (src, want) in cases {
        assert_eq!(codes(&run(src)), *want, "{src:?}");
    }
    let helps = [
        (
            "test \"t\":\n  len([1]) == 2\n;;",
            "did you mean `assert` before it?",
        ),
        ("xs = [1]\nsort(xs)\nprint(1)", "`sort` has no effect"),
        ("xs = [1]\nsort(xs) + [2]\nprint(1)", ""),
    ];
    for (src, want) in helps {
        let errs = run(src);
        assert_eq!(errs.len(), 1, "{src:?}: {errs:?}");
        assert!(help(&errs[0]).contains(want), "{src:?}: {errs:?}");
    }
}

/// An arm after one that matches everything without a guard never runs:
/// E0235 (FE25).
#[test]
fn arms_after_a_catch_all_are_errors() {
    let cases: &[(&str, &[&str])] = &[
        ("x = match 1: _ => 1; 2 => 3 ;;", &["E0235"]),
        ("x = match 1: n => n; 2 => 3 ;;", &["E0235"]),
        ("x = match 1: n | n => n; 2 => 3 ;;", &["E0235", "E0241"]),
        ("x = match 1: _ | _ => 1; _ => 3 ;;", &["E0235", "E0241"]),
        ("x = match 1: 1 => 1; _ => 2; _ => 3 ;;", &["E0235"]),
        ("x = match 1: 1 | _ => 1; 2 => 3 ;;", &["E0235"]),
        (
            "x = match 1: (a, b) | n => 1; 2 => 3 ;;",
            &["E0235", "E0216"],
        ),
        // Last, guarded, or not matching everything.
        ("x = match 1: 1 => 1; _ => 2 ;;", &[]),
        ("x = match 1: n when n > 0 => 1; _ => 2 ;;", &[]),
        ("x = match 1: (a, b) => a + b; _ => 2 ;;", &[]),
        ("x = match 1: 1 | _ => 1 ;;", &[]),
        // the same literal twice, the first without a guard (D51)
        ("x = match 1: 1 => 1; 1 => 2; _ => 3 ;;", &["E0235"]),
        ("x = match 1: \"a\" => 1; \"a\" => 2; _ => 3 ;;", &["E0235"]),
        ("x = match 1: 'a => 1; 'a => 2; _ => 3 ;;", &["E0235"]),
        ("x = match 1: Red => 1; Red => 2; _ => 3 ;;", &["E0235"]),
        ("x = match 1: nil => 1; nil => 2; _ => 3 ;;", &["E0235"]),
        ("x = match 1: 1 | 2 => 1; 2 => 2; _ => 3 ;;", &["E0235"]),
        (
            "x = match 1: 1 => 1; 1 when true => 2; _ => 3 ;;",
            &["E0235"],
        ),
        ("x = match 1: 1.5 => 1; 1.5 => 2; _ => 3 ;;", &["E0235"]),
        // one number however it is spelled (D127)
        ("x = match 1: 0xFF => 1; 0xff => 2; _ => 3 ;;", &["E0235"]),
        ("x = match 1: 0xff => 1; 255 => 2; _ => 3 ;;", &["E0235"]),
        ("x = match 1: 1s => 1; 1s => 2; _ => 3 ;;", &["E0235"]),
        ("x = match 1: true => 1; true => 2; _ => 3 ;;", &["E0235"]),
        // a guard on the first, or a different literal, or a wider arm
        ("x = match 1: 1 when false => 1; 1 => 2; _ => 3 ;;", &[]),
        ("x = match 1: 1 => 1; 2 => 2; _ => 3 ;;", &[]),
        // an alternative the arm before covers (D112)
        ("x = match 1: 1 => 1; 1 | 2 => 2; _ => 3 ;;", &["E0241"]),
        ("x = match 1: 1 => 1; \"1\" => 2; _ => 3 ;;", &[]),
    ];
    for (src, want) in cases {
        // The cases' names go unread: E0239 is not what they test.
        let (_, errs) = res(src);
        let errs: Vec<Diag> = errs.into_iter().filter(|e| e.code != "E0239").collect();
        assert_eq!(codes(&errs), *want, "{src:?}");
    }
    let (_, errs) = res("limit = 5\nx = match 1:\n  limit => 1\n  _ => 2\n;;");
    assert!(
        help(&errs[0]).contains("`v when v == limit =>`"),
        "{errs:?}"
    );
    let (_, errs) = res("x = match 1:\n  _ => 1\n  _ => 2\n;;");
    assert_eq!(help(&errs[0]), "", "the catalog help: {errs:?}");
    let src = "x = match 1:\n  1 =>\n    print(1)\n    y => y + 1\n  _ => 2\n;;";
    let (_, errs) = res(src);
    assert!(help(&errs[0]).contains("goes in parentheses"), "{errs:?}");
}

/// The second arm of a repeated literal never runs: E0235 there, naming
/// the line of the first (D51).
#[test]
fn a_repeated_literal_arm_names_the_first() {
    let (_, errs) = res("x = match 1:\n  1 => 1\n  1 => 2\n  _ => 3\n;;");
    assert_eq!((errs[0].pos.line, errs[0].pos.col), (3, 3), "{errs:?}");
    let msg = crate::diag::message(&errs[0].code, &errs[0].args);
    assert!(msg.contains("line 2"), "{msg}");
}

/// A call whose callee is known at compile time — a top-level fn of the
/// file, a prelude function, a member of an imported module — is checked
/// for its argument count and names then, with the codes and messages the
/// call gets at run time (D43).
#[test]
fn calls_to_known_functions_are_checked_at_compile_time() {
    let pre = prelude();
    let pre: Vec<&str> = pre.iter().map(String::as_str).collect();
    let check = |src: &str| {
        let (_, errs) = resolve_with(src, &pre, &m5_sig_opts());
        errs.iter()
            .map(|e| format!("{} {}", e.code, crate::diag::message(&e.code, &e.args)))
            .collect::<Vec<_>>()
    };
    let cases: &[(&str, &[&str])] = &[
        (
            "fn add(a, b) = a + b\nprint(add(1))",
            &["E0407 missing argument `b`"],
        ),
        (
            "fn add(a, b) = a + b\nprint(add(1, 2, 3))",
            &["E0402 add takes 2 argument(s), got 3"],
        ),
        (
            "fn add(a, b) = a + b\nprint(add(1, zz: 2))",
            &["E0406 add has no argument named `zz`"],
        ),
        (
            "fn add(a, b) = a + b\nprint(add(1, a: 2))",
            &["E0405 argument `a` given twice"],
        ),
        (
            "print(len(1, 2, 3))",
            &["E0402 len takes 1 argument(s), got 3"],
        ),
        ("print(first())", &["E0407 missing argument `xs`"]),
        (
            "print(join([1], sepp: \",\"))",
            &["E0406 join has no argument named `sepp`"],
        ),
        (
            "import fs\nfs.read(\"a\", \"b\")",
            &["E0402 read takes 1 argument(s), got 2"],
        ),
        (
            "import fs (read)\nread()",
            &["E0407 missing argument `path`"],
        ),
        (
            "fn f((a, b)) = a\nprint(f())",
            &["E0407 missing argument 1"],
        ),
        // Defaults, a rest parameter, named arguments, partial application
        // and a pipe are counted as the call binds them.
        (
            "fn greet(name, greeting = \"hi\", ..._extra) = name + greeting\nprint(greet(\"a\"), greet(\"a\", \"b\", \"c\", \"d\"), greet(name: \"a\", greeting: \"b\"))",
            &[],
        ),
        (
            "fn add(a, b) = a + b\ng = add(1, _)\nprint(g(2), 1 |> add(2))",
            &[],
        ),
        ("print(join([1], sep: \",\"), print())", &[]),
        (
            "fn add(a, b) = a + b\nprint(1 |> add(2, 3))",
            &["E0402 add takes 2 argument(s), got 3"],
        ),
        // A spread's count is known only when the call runs; its names
        // are checked.
        (
            "fn add(a, b) = a + b\nxs = [1]\nprint(add(...xs), add(1, ...xs))",
            &[],
        ),
        (
            "fn add(a, b) = a + b\nxs = [1]\nprint(add(...xs, c: 1))",
            &["E0406 add has no argument named `c`"],
        ),
        // A name bound to something else is not the known function.
        (
            "fn add(a, b) = a + b\nfn f(g) = g(1)\nprint(f(x => x), add)",
            &[],
        ),
        ("fn f(g) = g(1, 2)\nprint(f((a, _b) => a))", &[]),
        // A name bound to a lambda is that lambda (D110).
        (
            "f = (a) => a\nprint(f(1, 2))",
            &["E0402 the function takes 1 argument(s), got 2"],
        ),
        // Errors in every call are reported, in the order found.
        (
            "fn one(a) = a\nprint(one(), one(1, 2))",
            &[
                "E0407 missing argument `a`",
                "E0402 one takes 1 argument(s), got 2",
            ],
        ),
    ];
    for (src, want) in cases {
        assert_eq!(check(src), *want, "{src:?}");
    }
}

/// The REPL may define a top-level fn again in a later input, so a call
/// to one is checked only when it runs there; prelude functions are
/// checked as anywhere (D43).
#[test]
fn the_repl_checks_only_calls_that_cannot_change() {
    let pre = prelude();
    let pre: Vec<&str> = pre.iter().map(String::as_str).collect();
    let opts = Options {
        redefine: true,
        keep_value: true,
        ..Options::default()
    };
    let (_, errs) = resolve_with("fn g() = f(1, 2)\nfn f(a) = a\ng", &pre, &opts);
    assert!(errs.is_empty(), "{errs:?}");
    let (_, errs) = resolve_with("len(1, 2)", &pre, &opts);
    assert_eq!(codes(&errs), ["E0402"]);
}

/// `m5_opts` with the members' parameters, as the loader gives them.
fn m5_sig_opts() -> Options {
    let mut opts = m5_opts(false);
    let fs = opts.imports.get_mut("fs").expect("fs");
    fs.sigs.insert("read".into(), Sig::parse("path"));
    fs.sigs
        .insert("write".into(), Sig::parse("path, text, perm=?"));
    opts
}

/// FE61: the small name diagnostics of the third review.
#[test]
fn small_name_diagnostics_of_the_third_review() {
    // E0238 (was E0210) says where the first definition is
    let (_, errs) = res("fn a() = 1\nfn a() = 2\nprint(a())");
    assert_eq!(codes(&errs), ["E0238"]);
    assert_eq!(errs[0].notes, ["note: `a` is a function defined on line 1"]);
    // a local fn defined twice in one block too
    let (_, errs) = res("fn g():\n  fn a() = 1\n  fn a() = 2\n  a()\n;;\nprint(g())");
    assert_eq!(codes(&errs), ["E0238"]);
    assert_eq!((errs[0].pos.line, errs[0].pos.col), (3, 6));
    assert_eq!(errs[0].notes, ["note: `a` is a function defined on line 2"]);
    // in two blocks it is two functions
    ok(
        "fn g(c):\n  if c:\n    fn a() = 1\n    a()\n  else:\n    fn a() = 2\n    a()\n  ;;\n;;\nprint(g(true))",
    );
    // a value word of another language
    let (_, errs) = res("print(null)");
    assert_eq!(codes(&errs), ["E0204"]);
    assert_eq!(errs[0].help.as_deref(), Some("fung's empty value is `nil`"));
    // an alternative after one that takes its values never matches
    for (src, line, col) in [
        ("x | x = 1\nprint(x)", 1, 5),
        ("match 1:\n  1 | 1 => print(1)\n  _ => print(2)\n;;", 2, 7),
        ("match 1:\n  _ | 1 => print(1)\n;;", 2, 7),
        (
            "match [1]:\n  [a] | [a] => print(a)\n  _ => print(2)\n;;",
            2,
            9,
        ),
    ] {
        let (_, errs) = res(src);
        assert_eq!(codes(&errs), ["E0241"], "{src:?}");
        assert_eq!((errs[0].pos.line, errs[0].pos.col), (line, col), "{src:?}");
    }
    ok("match 1:\n  1 | 2 => print(1)\n  _ => print(2)\n;;");
}

/// FE61: a module member gets the similar-name help a variable gets, and
/// a private name in an import list is reported once.
#[test]
fn module_member_diagnostics() {
    let (_, errs) = resolve_with(
        "import fs\nprint(fs.raed(\"a\"))",
        &["print"],
        &m5_opts(false),
    );
    assert_eq!(codes(&errs), ["E0303"]);
    assert_eq!(
        errs[0].help.as_deref(),
        Some("a similar name exists: `read`")
    );
    let (_, errs) = resolve_m5("import \"./lib/x\" (_p)\nprint(_p)", false);
    assert_eq!(errs, ["E0304"]);
}

/// A name is bound once, wherever it is bound (D88): a second binding in
/// the same block is E0238 for a variable and a local fn alike, in any
/// order (E0209 is retired into it). A visible outer name is not bound
/// again either: E0238 (D122), whose help for an assignment to a
/// variable, the usual counter in a loop, shows what to write instead.
#[test]
fn a_name_is_bound_once() {
    let cases: &[(&str, &[&str])] = &[
        ("fn f(x):\n  y = x\n  y = y + 1\n  y\n;;", &["E0238:y"]),
        (
            "fn f():\n  (a, b) = (1, 2)\n  (a, b) = (b, a)\n  a + b\n;;",
            &["E0238:a", "E0238:b"],
        ),
        ("fn f():\n  fn g() = 1\n  g = 2\n  g\n;;", &["E0238:g"]),
        ("fn f():\n  g = 2\n  fn g() = g\n  g()\n;;", &["E0238:g"]),
        ("fn f(x):\n  x = x + 1\n  x\n;;", &["E0238:x"]),
        (
            "fn f(c):\n  if c:\n    v = 1\n    v = 2\n    print(v)\n  ;;\n  nil\n;;",
            &["E0238:v"],
        ),
        // A visible outer variable assigned in a block.
        (
            "fn f():\n  t = 0\n  if true:\n    t = t + 1\n    print(t)\n  ;;\n  t\n;;",
            &["E0238:t"],
        ),
        ("c = 1\nfn f():\n  c = 2\n  c\n;;\nprint(c)", &["E0238:c"]),
        ("fn g() = 1\ng = 2", &["E0238:g"]),
        // Each block binds its own names; a pattern's alternatives bind
        // one name once.
        (
            "fn f(c):\n  if c:\n    v = 1\n    print(v)\n  else:\n    v = 2\n    print(v)\n  ;;\n  nil\n;;",
            &[],
        ),
        ("fn f(r):\n  Ok(x) | Err(x) = r\n  x\n;;", &[]),
        ("fn f(x):\n  y = x + 1\n  z = y * 2\n  z\n;;", &[]),
    ];
    for (src, want) in cases {
        assert_eq!(name_errors_of(src, &Options::default()), *want, "{src:?}");
    }
    // The REPL binds a top-level name again; a block there does not.
    let repl_opts = repl(&[], true);
    assert_eq!(name_errors_of("x = 1\nx = 2", &repl_opts), [] as [&str; 0]);
    assert_eq!(
        name_errors_of("fn f():\n  y = 1\n  y = 2\n  y\n;;", &repl_opts),
        ["E0238:y"]
    );
}

/// Checks of the fourth review: a module is imported once in a file
/// (D96) and a test block has something in it (D100); an uppercase
/// annotation is a tag name (D134, where D101 had made `Int` an error).
#[test]
fn one_import_a_test_with_a_body_and_uppercase_tags() {
    let mut imports = BTreeMap::new();
    imports.insert(
        "fs".to_string(),
        ModuleInfo {
            key: "fs".into(),
            name: "fs".into(),
            exports: names(&["read"]),
            ..Default::default()
        },
    );
    let opts = Options {
        imports,
        ..Options::default()
    };
    let code = |src: &str| {
        let (_, errs) = resolve_with(src, &["print"], &opts);
        codes(&errs)
    };
    assert_eq!(
        code("import fs\nimport fs as f2\nprint(fs.read, f2.read)"),
        ["E0309"]
    );
    assert_eq!(
        code("import fs (read)\nimport fs\nprint(fs.read, read)"),
        ["E0309"]
    );
    assert_eq!(code("import fs\nprint(fs.read)"), [] as [&str; 0]);
    assert_eq!(code("test \"nothing\": ;;"), ["E0242"]);
    assert_eq!(code("test \"something\": assert true ;;"), [] as [&str; 0]);
    // An uppercase name in an annotation is a tag name, whatever it
    // spells (D134, undoing D101).
    for src in [
        "fn f(x: Int) = x",
        "fn f(x) -> String = x",
        "fn f(x: List | Nil) = x",
        "type t = Bool",
        "fn f(c: Red | Circle) = c",
    ] {
        assert_eq!(code(src), [] as [&str; 0], "{src}");
    }
}

/// What the types of literals settle is an error before the program
/// runs, with the number the run would give (D99): a literal called, a
/// literal that is no bool where a bool goes, an operator over two
/// literals it has no meaning for, and `??` after a literal that is
/// never nil or a result.
#[test]
fn literal_type_errors_are_caught_at_compile_time() {
    for (src, want) in [
        ("x = 1(2)", "E0401"),
        ("x = \"a\"(2)", "E0401"),
        ("x = nil()", "E0401"),
        ("x = not 1", "E0505"),
        ("x = 1 then 2 else 3", "E0505"),
        ("x = if \"a\": 1 else: 2 ;;", "E0505"),
        ("x = 1 and true", "E0505"),
        ("x = true or nil", "E0505"),
        ("x = 1 + \"a\"", "E0501"),
        ("x = \"a\" - \"b\"", "E0501"),
        ("x = true * 2", "E0501"),
        ("x = nil < 1", "E0501"),
        ("x = \"a\" < 1", "E0501"),
        ("x = 1 ?? 2", "E0243"),
        ("x = \"a\" ?? \"b\"", "E0243"),
        ("x = [1] ?? []", "E0243"),
        // Every kind the syntax shows (FE81): durations, interpolated
        // strings, collections, tag values, functions, ranges, and `-`.
        ("x = not 1s", "E0505"),
        ("x = if 1s: 1 else: 2 ;;", "E0505"),
        ("x = 1s(2)", "E0401"),
        ("x = 1s + \"a\"", "E0501"),
        ("x = 1s ?? 2", "E0243"),
        ("x = \"a${1}\"(2)", "E0401"),
        ("x = not \"a${1}\"", "E0505"),
        ("x = [1](2)", "E0401"),
        ("x = {a: 1}(2)", "E0401"),
        ("x = if [1]: 1 else: 2 ;;", "E0505"),
        ("x = not [1]", "E0505"),
        ("x = [1] - [1]", "E0501"),
        ("x = Red(1)(2)", "E0401"),
        ("x = Red(1) ?? 2", "E0243"),
        ("x = -\"a\"", "E0506"),
        ("x = -true", "E0506"),
        ("x = -nil", "E0506"),
        ("x = (x => x) + 1", "E0501"),
        ("x = print(1, _) + 1", "E0501"),
        ("x = 1..2 ?? 3", "E0243"),
        ("x = (1..2)(3)", "E0401"),
        ("x = \"a\" * 1s", "E0501"),
        ("x = 1s // 2", "E0501"),
        ("x = 1 < [1]", "E0501"),
        // Values decide these: left to the run.
        ("x = [1] < [\"a\"]", ""),
        ("x = (1,) < (1, 2)", ""),
        ("x = (x => x)(1)", ""),
        ("x = Ok(1) ?? 2", ""),
        ("x = [1] + [2]", ""),
        ("x = -1s", ""),
        ("x = 1s / 2", ""),
        ("x = 2 * 1s", ""),
        ("x = 1 // 0", ""),
        ("x = 9223372036854775807 + 1", ""),
        ("x = 1 + 2.5", ""),
        ("x = \"a\" + \"b\"", ""),
        ("x = \"a\" < \"b\"", ""),
        ("x = 1 == \"a\"", ""),
        ("x = 1s * 2", ""),
        ("x = nil ?? 2", ""),
        ("x = print(1) ?? 2", ""),
    ] {
        let (_, errs) = res(src);
        assert_eq!(first_code(&errs), want, "{src:?}: {errs:?}");
    }
}

/// Small diagnostics of the fifth review (FE82): a discarded undefined
/// name is one error, its E0204; an unused name that a whole arm binds
/// says that it matches every value.
#[test]
fn small_diagnostics_of_the_fifth_review() {
    let (_, errs) = res_discard("x = 1\nprint(x)\nend");
    assert_eq!(codes(&errs), ["E0204"], "{errs:?}");
    let (_, errs) = res("x = match 1:\n  1 => 1\n  default => 2\n;;");
    assert_eq!(codes(&errs), ["E0239"], "{errs:?}");
    assert!(help(&errs[0]).contains("`_ =>`"), "{errs:?}");
    let (_, errs) = res("x = match (1, 2):\n  (a, b) => 1\n  _ => 2\n;;");
    assert!(!help(&errs[0]).contains("`_ =>`"), "{errs:?}");
}

/// Small resolve fixes of the fourth review: a module's top level takes
/// no `_ = e` (FE69), a number literal an earlier arm matches is found by
/// its value (FE70), and the errors come in source order (FE73).
#[test]
fn small_resolve_fixes() {
    let (_, errs) = resolve_with("_ = print(1)\nx = 2", &["print"], &m5_opts(true));
    assert_eq!(codes(&errs), ["E0306"]);
    let (_, errs) = resolve_with("(a, b) = (1, 2)", &["print"], &m5_opts(true));
    assert_eq!(codes(&errs), [] as [&str; 0]);
    // A destructuring that binds no name is a bare statement too (FE80).
    for src in [
        "(_, _) = (print(1), 2)\nx = 2",
        "[_, ...] = [print(1)]\nx = 2",
        "{a: _} = {a: print(1)}\nx = 2",
    ] {
        let (_, errs) = resolve_with(src, &["print"], &m5_opts(true));
        assert_eq!(codes(&errs), ["E0306"], "{src}");
    }
    let (_, errs) = resolve_with("(_, b) = (1, 2)", &["print"], &m5_opts(true));
    assert_eq!(codes(&errs), [] as [&str; 0]);
    for src in [
        "fn f(x) = match x: 1 => 1; 1.0 => 2; _ => 0 ;;",
        "fn f(x) = match x: 0.0 => 1; -0.0 => 2; _ => 0 ;;",
        "fn f(x) = match x: 2.0 | 3 => 1; 2 => 2; _ => 0 ;;",
    ] {
        let (_, errs) = res(src);
        assert_eq!(codes(&errs), ["E0235"], "{src}");
    }
    let (_, errs) = res("fn f(x) = match x: 1 => 1; 1.5 => 2; _ => 0 ;;");
    assert_eq!(codes(&errs), [] as [&str; 0]);
    let (_, errs) = res(
        "print(aa)\nfn f() = bb\nx = if true: 1 ;;\nprint(cc, x)\nfn g() = dd\n1 + 2\nprint(1)",
    );
    let lines: Vec<u32> = errs.iter().map(|e| e.pos.line).collect();
    assert_eq!(lines, [1, 2, 3, 4, 5, 6], "{errs:?}");
}

/// A partial application and a call of a local fn are checked when the
/// file is compiled, as any call of a known function is (FE65).
#[test]
fn partial_applications_and_local_fns_are_checked() {
    let cases = [
        (
            "fn add(a, b) = a + b\nloc = add(1, 2, _)\nprint(loc)",
            "E0402",
        ),
        (
            "fn add(a, b) = a + b\nloc = add(_, zz: 1)\nprint(loc)",
            "E0406",
        ),
        (
            "fn f():\n  fn loc(a) = a\n  loc(1, 2)\n;;\nprint(f)",
            "E0402",
        ),
        ("fn f():\n  fn loc(a) = a\n  loc()\n;;\nprint(f)", "E0407"),
        (
            "fn f():\n  fn loc(a) = a\n  w = () => loc(1, 2)\n  w\n;;\nprint(f)",
            "E0402",
        ),
        (
            "fn f():\n  fn loc(a) = a == 0 then 0 else loc(a - 1, 1)\n  loc(1)\n;;\nprint(f)",
            "E0402",
        ),
        (
            "fn f():\n  fn loc(a) = a\n  loc(_, 1)\n;;\nprint(f)",
            "E0402",
        ),
        // what is right stays right
        (
            "fn add(a, b) = a + b\nloc = add(1, _)\nprint(loc(2), add(_, b: 1))",
            "",
        ),
        (
            "fn f():\n  fn loc(a, b = 1) = a + b\n  loc(1) + loc(1, 2) + loc(a: 1)\n;;\nprint(f)",
            "",
        ),
        // a parameter of the same name is not the fn around it
        (
            "fn f():\n  fn loc(a) = a\n  w = loc => loc(1, 2)\n  w(loc)\n;;\nprint(f)",
            "E0238",
        ),
        ("fn f(k):\n  loc = k(1, _)\n  loc\n;;\nprint(f)", ""),
    ];
    for (src, want) in cases {
        let (_, errs) = resolve_with(src, M4_PRELUDE, &Options::default());
        let got = codes(&errs);
        if want.is_empty() {
            assert!(got.is_empty(), "{src}: {errs:?}");
        } else {
            assert_eq!(got, [want], "{src}: {errs:?}");
        }
    }
}

/// D110: a name whose one binding is visibly a function (a lambda, an
/// anonymous `fn`, a known function's name, a partial application of one)
/// is that function in every block: calls through it are checked as a
/// call of a `fn` is, and so is a function literal called where it stands.
#[test]
fn names_bound_to_a_function_are_checked() {
    let cases = [
        ("f = (a, b) => a + b\nprint(f(1))", "E0407"),
        ("f = fn(a): a ;;\nprint(f(1, 2))", "E0402"),
        ("fn g(a) = a\nf = g\nprint(f(1, 2))", "E0402"),
        ("fn g(a) = a\nf = g\nh = f\nprint(h(b: 1))", "E0406"),
        ("x = len\nprint(x(1, 2))", "E0402"),
        ("fn g(a, b) = a\nf = g(1, _)\nprint(f(1, 2))", "E0402"),
        ("print((x => x)(1, 2))", "E0402"),
        ("print((fn(x): x ;;)(1, x: 2))", "E0405"),
        ("fn g(a) = a\nprint(g(1, 2, ...[3]))", "E0402"),
        ("fn g(a) = a\nf = g\nprint(f(1, 2, ...[3]))", "E0402"),
        // in a function, and from a function the name is captured by
        ("fn h():\n  f = x => x\n  f(1, 2)\n;;\nprint(h)", "E0402"),
        (
            "fn h():\n  f = len\n  w = () => f()\n  w\n;;\nprint(h)",
            "E0407",
        ),
        (
            "fn h():\n  if true:\n    f = x => x\n    f()\n  ;;\n  1\n;;\nprint(h)",
            "E0407",
        ),
        // a top-level fn calls a constant bound to a function
        ("fn h() = f(1, 2)\nf = x => x\nprint(h)", "E0402"),
        ("f = x => x\ng = () => f(1, 2)\nprint(g)", "E0402"),
        // destructured from a literal
        ("(f, g) = (x => x, len)\nprint(f(1, 2), g)", "E0402"),
        // the purity of what it names (E0234)
        ("s = len\ns([1])\nprint(1)", "E0234"),
        ("fn h():\n  s = len\n  s([1])\n  1\n;;\nprint(h)", "E0234"),
        // what is right stays right
        (
            "f = fn(a, b = 1): a + b ;;\nprint(f(1), f(1, 2), f(a: 1, b: 2))",
            "",
        ),
        ("fn g(a, ...r) = [a, r]\nf = g\nprint(f(1, 2, 3))", ""),
        ("fn g(a, b) = a + b\nprint(g(1, ...[2]))", ""),
        // not known: a choice, a call's result, a parameter
        (
            "fn g(a) = a\nfn k(a) = a\nf = true then g else k\nprint(f(1, 2))",
            "",
        ),
        ("fn pick() = x => x\nf = pick()\nprint(f(1, 2))", ""),
        ("fn h(f) = f(1, 2)\nprint(h)", ""),
    ];
    for (src, want) in cases {
        let (_, errs) = resolve_with(src, &["print", "len"], &Options::default());
        let got = codes(&errs);
        if want.is_empty() {
            assert!(got.is_empty(), "{src}: {errs:?}");
        } else {
            assert_eq!(got, [want], "{src}: {errs:?}");
        }
    }
    // The errors name what the function's own errors name.
    let (_, errs) = resolve_with(
        "fn g(a) = a\nf = g\nh = x => x\nprint(f(1, 2), h(1, 2))",
        &["print"],
        &Options::default(),
    );
    let msgs: Vec<String> = errs
        .iter()
        .map(|e| crate::diag::message(&e.code, &e.args))
        .collect();
    assert_eq!(
        msgs,
        [
            "g takes 1 argument(s), got 2",
            "the function takes 1 argument(s), got 2"
        ]
    );
    // Too many fixed arguments before a spread.
    let (_, errs) = resolve_with(
        "fn g(a) = a\nprint(g(1, 2, ...[3]))",
        &["print"],
        &Options::default(),
    );
    let msgs: Vec<String> = errs
        .iter()
        .map(|e| crate::diag::message(&e.code, &e.args))
        .collect();
    assert_eq!(msgs, ["g takes 1 argument(s), got at least 2"]);
    // The REPL may bind a top-level name again: no signature there.
    let opts = Options {
        redefine: true,
        ..Options::default()
    };
    let (_, errs) = resolve_with("f = x => x\nprint(f(1, 2))", &["print"], &opts);
    assert!(errs.is_empty(), "{errs:?}");
}

/// D113: a literal where a type annotation stands (a parameter default, a
/// value the function returns, an argument of a call whose function is
/// known) that the annotation does not take is the run's error, before
/// anything runs.
#[test]
fn literals_against_annotations_are_checked() {
    let cases = [
        ("fn f() -> int = \"x\"\nprint(f)", "E0404"),
        ("fn f(a: int = \"x\") = a\nprint(f)", "E0403"),
        ("fn g(a: int) = a\nfn h() = g(\"x\")\nprint(h)", "E0403"),
        ("fn f() -> int: return nil ;;\nprint(f)", "E0404"),
        ("fn f() -> int:\n  return\n;;\nprint(f)", "E0404"),
        (
            "fn f(c) -> int:\n  if c: 1 else: \"x\" ;;\n;;\nprint(f)",
            "E0404",
        ),
        (
            "fn f(c) -> int:\n  if c: return \"x\" ;;\n  1\n;;\nprint(f)",
            "E0404",
        ),
        ("fn f(c) -> int = c then 1 else 2.5\nprint(f)", "E0404"),
        (
            "fn f(c) -> int = match c: 1 => 2; _ => nil ;;\nprint(f)",
            "E0404",
        ),
        (
            "fn f(c) -> int = return c then 1 else 's\nprint(f)",
            "E0404",
        ),
        ("fn g(a: int, b: str) = [a, b]\nprint(g(1, b: 2))", "E0403"),
        ("fn g(a: num) = a\nprint(g(\"x\"))", "E0403"),
        (
            "type id = int | str\nfn g(a: id) = a\nprint(g(1.5))",
            "E0403",
        ),
        ("fn g(a: Red) = a\nprint(g(Blue(1)))", "E0403"),
        ("fn g(a: Red) = a\nprint(g(Blue))", "E0403"),
        ("fn g(a: fn) = a\nprint(g([1]))", "E0403"),
        ("fn g(a: int) = a\nh = g\nprint(h(\"x\"))", "E0403"),
        ("f = fn(a: int): a ;;\nprint(f(\"x\"))", "E0403"),
        (
            "fn g(a: int) = a\nprint(map([1], g(_)), g(\"${1}\"))",
            "E0403",
        ),
        // what the annotation takes
        (
            "fn g(a: num, b: any, c: Red, d: Red, e: sym, f: fn, h: int | nil) = [a, b, c, d, e, f, h]\nprint(g(1.5, \"x\", Red(1), Red, 'a, x => x, nil))",
            "",
        ),
        ("fn f(c) -> int | nil = c then 1 else nil\nprint(f)", ""),
        ("fn f(a: Ok | Err = Ok(1)) = a\nprint(f)", ""),
        // a name is no literal
        ("fn g(a: int) = a\ns = \"x\"\nprint(g(s))", ""),
        // a rest parameter is always a list
        ("fn g(...xs: list) = xs\nprint(g(\"x\"))", ""),
    ];
    for (src, want) in cases {
        let (_, errs) = resolve_with(src, &["print", "map"], &Options::default());
        let got = codes(&errs);
        if want.is_empty() {
            assert!(got.is_empty(), "{src}: {errs:?}");
        } else {
            assert_eq!(got, [want], "{src}: {errs:?}");
        }
    }
    // The run's wording, with the union as written once expanded.
    let (_, errs) = resolve_with(
        "type id = int | str\nfn g(a: id, b: Red = 1) -> id | nil = a == 1 then 2.5 else nil\nprint(g(true))",
        &["print"],
        &Options::default(),
    );
    let msgs: Vec<String> = errs
        .iter()
        .map(|e| crate::diag::message(&e.code, &e.args))
        .collect();
    assert_eq!(
        msgs,
        [
            "argument `b` must be Red, got int",
            "return value must be int | str | nil, got float",
            "argument `a` must be int | str, got bool",
        ]
    );
}

/// D112: an arm (or an alternative) that an earlier arm without a guard
/// covers never runs: E0235 for the arm, E0241 for one alternative.
/// A pattern covers another when every value the other matches it
/// matches too, seen from their shapes alone.
#[test]
fn an_arm_covered_by_an_earlier_one_never_runs() {
    let cases: &[(&str, &[&str])] = &[
        ("[] => 1; [] => 2", &["E0235"]),
        ("Ok(_) => 1; Ok(1) => 2", &["E0235"]),
        ("(_, _) => 1; (1, 2) => 2", &["E0235"]),
        ("(a, _) => a; (1, b) => b", &["E0235"]),
        ("[...] => 1; [] => 2", &["E0235"]),
        ("[x, ...] => x; [1, 2] => 2", &["E0235"]),
        ("[x, ...] => x; [1, ...r] => r", &["E0235"]),
        ("[_, _] => 1; [1, 2] => 2", &["E0235"]),
        ("Pt(1, x) => x; Pt(1, 2) => 2", &["E0235"]),
        ("{a: 1} => 1; {a: 1} => 2", &["E0235"]),
        ("{a: _, ...} => 1; {a: 1, b: 2} => 2", &["E0235"]),
        ("{a: _, ...} => 1; {a: 1, b: 2, ...} => 2", &["E0235"]),
        ("{...} => 1; {} => 2", &["E0235"]),
        ("{...} => 1; {a: 1, ...} => 2", &["E0235"]),
        ("1 | 2 => 1; 2 | 1 => 2", &["E0235"]),
        ("Ok(1 | 2) => 1; Ok(2) => 2", &["E0235"]),
        ("1 | 2 => 1; 2 | 3 => 2", &["E0241"]),
        ("Ok(_) | Err(_) => 1; Ok(2) => 2", &["E0235"]),
        // a guard on the earlier arm, or a shape that differs
        ("[x] when x > 0 => 1; [1] => 2", &[]),
        ("[x, ...] => x; [] => 2", &[]),
        ("[1, ...] => 1; [x, ...] => x", &[]),
        ("[a] => a; [a, b] => a + b", &[]),
        ("(a, b) => a; (a, b, c) => c", &[]),
        ("Pt(a) => a; Pt(a, b) => b", &[]),
        ("Pt(a) => a; Qt(a) => a", &[]),
        ("Ok(1) => 1; Ok(_) => 2", &[]),
        ("{a: 1} => 1; {a: 1, b: 2} => 2", &[]),
        ("{a: 1} => 1; {a: 1, ...} => 2", &[]),
        ("{a: 1, ...} => 1; {b: 1} => 2", &[]),
        ("{} => 1; {...} => 2", &[]),
        ("1 => 1; Red => 2", &[]),
        // several arms together are not looked at (out of scope)
        ("true => 1; false => 2", &[]),
    ];
    for (arms, want) in cases {
        let src = format!(
            "fn f(v) = match v:\n  {}\n  _ => 0\n;;\nprint(f)",
            arms.replace("; ", "\n  ")
        );
        let (_, errs) = res(&src);
        let errs: Vec<Diag> = errs.into_iter().filter(|e| e.code != "E0239").collect();
        assert_eq!(codes(&errs), *want, "{src}: {errs:?}");
    }
    // The second arm, naming the first; the dead alternative.
    let (_, errs) = res("fn f(v) = match v:\n  Ok(_) => 1\n  Ok(1) => 2\n  _ => 0\n;;\nprint(f)");
    assert_eq!((errs[0].pos.line, errs[0].pos.col), (3, 3), "{errs:?}");
    let msg = crate::diag::message(&errs[0].code, &errs[0].args);
    assert!(msg.contains("line 2"), "{msg}");
    let (_, errs) = res("fn f(v) = match v:\n  1 | 2 => 1\n  2 | 3 => 2\n  _ => 0\n;;\nprint(f)");
    assert_eq!((errs[0].pos.line, errs[0].pos.col), (3, 3), "{errs:?}");
    // inside one `|`, by structure too
    let (_, errs) = res("fn f(v) = match v:\n  [_] | [1] => 1\n  _ => 0\n;;\nprint(f)");
    assert_eq!(codes(&errs), ["E0241"], "{errs:?}");
    // `1 => y => y + 1` in a statement: an empty arm `1`, then `y`, which
    // matches everything before the last arm.
    let (_, errs) = res(
        "fn f(v):\n  match v:\n    1 => y => print(y)\n    _ => print(2)\n  ;;\n  1\n;;\nprint(f)",
    );
    assert_eq!(codes(&errs), ["E0235"], "{errs:?}");
    assert!(help(&errs[0]).contains("parentheses"), "{errs:?}");
}
