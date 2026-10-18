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
    desugar(&parse(&toks).unwrap()).unwrap()
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

fn res(src: &str) -> (Option<Resolved>, Vec<Diag>) {
    resolve_with(src, &["print", "fail"], &Options::default())
}

fn ok(src: &str) -> Resolved {
    let (r, errs) = res(src);
    assert!(errs.is_empty(), "{src:?}: {errs:?}");
    r.unwrap()
}

fn codes(errs: &[Diag]) -> Vec<String> {
    errs.iter().map(|e| e.code.clone()).collect()
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
    let d = desugar(&parse(&toks).unwrap()).unwrap();
    resolve(&d, &prelude(), &Options::default())
        .map(|r| r.core.to_string())
        .map_err(|es| es.iter().map(|e| e.code.clone()).collect())
}

// ---- whole outputs ----

#[test]
fn whole_outputs() {
    assert_eq!(
        core("x = 1 + 2 * 3").unwrap(),
        "(do (let (global 0) (op + 1 (op * 2 3))))"
    );
    assert_eq!(
        core("f = (a, b) => a - b\nprint(f(1, b: 2))").unwrap(),
        "(do (let (global 0) (fn (a, b) (do (op - (local 0) (local 1))))) (call (prelude print) (call (global 0) 1 (named b 2))))"
    );
    assert_eq!(
        core("x = 1\nx = 2").unwrap(),
        "(do (let (global 0) 1) (let (global 0) 2))"
    );
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
    let (_, errs) = res("x = 1\nfn f() = x");
    assert_eq!(
        help(&errs[0]),
        "`x` is a file-level variable (assigned on line 1) that this function cannot see: functions copy only the variables that exist when they are created, and top-level fns are created first; pass it as an argument"
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
            let src = format!("x = [1]{}", "[0]".repeat(9_000));
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
    let r = ok("x = 1\ny = x + 1\nx = x * 2\nprint(x, y)");
    assert_eq!(
        r.core.to_string(),
        "(do (let (global 0) 1) (let (global 1) (op + (global 0) 1)) (let (global 0) (op * (global 0) 2)) (call (prelude print) (global 0) (global 1)))"
    );
    assert_eq!(r.num_locals, 0);
    assert_eq!(r.globals, names(&["x", "y"]));
}

#[test]
fn block_names_become_slots_and_are_reused() {
    let r = ok("if true: a = 1; b = a ;;\nif true: c = 2 ;;");
    assert_eq!(
        r.core.to_string(),
        "(do (if true (do (let (local 0) 1) (let (local 1) (local 0))) (nil)) (if true (do (let (local 0) 2)) (nil)))"
    );
    assert_eq!(r.num_locals, 2);
}

#[test]
fn nested_blocks_stack_slots() {
    let r = ok("if true: a = 1; if true: b = a ;; ;;");
    assert_eq!(r.num_locals, 2);
    assert!(r.core.to_string().contains("(let (local 1) (local 0))"));
}

#[test]
fn reassigning_outer_name_in_block_is_e0205() {
    let (_, errs) = res("total = 0\nif true: total = total + 1 ;;");
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].code, "E0205");
    assert_eq!(arg0(&errs[0]), "total");
    assert_eq!(errs[0].len, 5);
    assert_eq!((errs[0].pos.line, errs[0].pos.col), (2, 10));
    // Block-local names may be reassigned inside their own block.
    ok("if true: n = 1; n = n + 1 ;;");
    // And a local may not be reassigned from a nested block either.
    let (_, errs) = res("if true: n = 1; if true: n = 2 ;; ;;");
    assert_eq!(codes(&errs), vec!["E0205"]);
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
    let r = ok("fn f(a, b = a + 1):\n  c = a * b\n  if c > 0: d = c ;;\n  c\n;;");
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
    let r = ok("fn h() = 1\nx = 1\ng = () => x + h()");
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
    // A top-level fn is created before any variable exists, so it cannot
    // see x, whether x is assigned before or after it (03, model C).
    for src in ["x = 1\nfn f() = x", "fn f() = x\nx = 1"] {
        let (_, errs) = res(src);
        assert_eq!(codes(&errs), vec!["E0204"], "{src:?}");
        assert!(help(&errs[0]).contains("pass it as an argument"), "{src:?}");
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
}

#[test]
fn fn_scope_errors() {
    let cases = [
        ("fn f() = 1\nfn f() = 2", "E0210"),
        ("fn f() = 1\nf = 2", "E0209"),
        ("x = 1\nfn f(): x = 2 ;;", ""), // f cannot see x: this is a new local
        ("x = 1\nf = fn(): x = 2 ;;", "E0205"), // an anonymous fn created after x sees it
        ("fn f(a): g = () => a; a = 2 ;;", ""), // same scope rebinding is fine
        ("return 1", "E0211"),
        ("if true: return 1 ;;", "E0211"),
        ("fn f(a: integer) = a", "E0212"),
        ("fn f(a) -> strng = a", "E0212"),
        ("fn f(a: Circle | int) = a", ""),
        ("fn f() = y\ny = 1", "E0204"), // created before y exists
        ("f = () => f()", "E0204"),     // f is not bound yet when the lambda is created
        ("fn outer(): f = () => f(); f() ;;", "E0204"), // a block local: only named fns see themselves
        ("fn f():\n  if true: v = 1 ;;\n  v\n;;", "E0206"),
        ("fn f(): fn g() = 1 ;;\nfn h() = g()", "E0204"), // local fn of f is not visible in h
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
fn per_function_closed_sets() {
    // A name closed in f's block must not turn an unrelated typo in g into E0206.
    let (_, errs) = res("fn f(): if true: tmp = 1 ;; ;;\nfn g() = tmp");
    assert_eq!(codes(&errs), vec!["E0204"]);
}

// ---- collections and patterns (M4) ----

const M4_PRELUDE: &[&str] = &["print", "f", "g", "xs"];

#[test]
fn destructuring_binds_like_assignment() {
    let (r, errs) = resolve_with(
        "(q, r) = f()\nprint(q, r)\n[a, ...rest] = xs\n(q, r) = g()",
        M4_PRELUDE,
        &Options::default(),
    );
    assert!(errs.is_empty(), "{errs:?}");
    let r = r.unwrap();
    let got = r.core.to_string();
    assert!(
        got.contains("(let (ptuple (pbind (global 0)) (pbind (global 1)))"),
        "{got}"
    );
    assert!(got.contains("(prest (global 3))"), "{got}");
    assert_eq!(r.globals[0], "q");
    assert_eq!(r.globals[3], "rest");
    let (_, errs) = res("total = 0\nif true: (total, n) = (1, 2) ;;");
    assert_eq!(codes(&errs), vec!["E0205"]);
}

#[test]
fn match_arms_shadow_and_scope() {
    let r = ok("x = 1\nmatch 2:\n  x when x > 1 => x\n  _ => x\n;;\nprint(x)");
    let got = r.core.to_string();
    // The arm's x is a fresh slot; outside the match, x is still the global.
    assert!(
        got.contains("(arm (pbind (local 1)) (op > (local 1) 1) (do (local 1)))"),
        "{got}"
    );
    assert!(got.contains("(call (prelude print) (global 0))"), "{got}");
    assert_eq!(r.num_locals, 2); // $m0 and the arm's x
    let (_, errs) = res("match 1: 'a | 'b => 1 ;;\nmatch 1: Ok(x) | Err(y) => 1 ;;");
    assert_eq!(codes(&errs), vec!["E0216"]);
    let (_, errs) = res("match 1: x => 1 ;;\nprint(x)");
    assert_eq!(codes(&errs), vec!["E0206"]);
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
    let cases = [
        ("for x in xs: return x ;;", "E0213"),
        ("for x in xs: if x: return 1 ;; ;;", "E0213"),
        ("for x in xs: (() => return x)() ;;", ""), // a nested fn may return
        ("fn f(xs) = for x in xs: return x ;;", "E0213"),
        ("fn f(xs) = for x in xs: x? ;;", "E0220"),
        ("fn f(xs) = for x in xs: g(x?) ;;", "E0220"),
        ("fn f(xs) = for x in xs: (() => x?)() ;;", ""), // a nested fn may return
        ("a = 1\nfor (a, b) in xs: a ;;", ""),           // for variables shadow (03)
        ("a = 1\nfor a in xs: a ;;", ""),
        ("f()?", "E0214"),
        ("fn g() = f()?", ""),
        ("print(_)", ""), // hole: fine
        ("_ + 1", "E0215"),
        ("x = _", "E0215"),
    ];
    for (src, want) in cases {
        let (_, errs) = resolve_with(src, M4_PRELUDE, &Options::default());
        assert_eq!(first_code(&errs), want, "{src:?}: {errs:?}");
    }
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
    ];
    for (src, want) in cases {
        let (_, errs) = res(src);
        assert_eq!(first_code(&errs), want, "{src:?}: {errs:?}");
    }
    let r = ok("type shape = Circle | Rect\nfn area(s: shape | nil) = s");
    let info = fn_info(&r.core, &[0, 1]);
    assert_eq!(
        info.params[0].types.as_ref().unwrap().join("|"),
        "Circle|Rect|nil"
    );
}

#[test]
fn rest_param_is_allowed_now() {
    let r = ok("fn f(a, ...xs) = xs");
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
        },
    );
    imports.insert(
        "./lib/x".to_string(),
        ModuleInfo {
            key: "/abs/lib/x.fung".into(),
            name: "x".into(),
            exports: names(&["a", "b"]),
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
        "import fs\nimport \"./lib/x\" (a)\n_p = 1\nfn f() = 1\nq = 2",
        false,
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
        ("import fs\nfs = 1", "E0308"),
        ("import fs\nfn fs() = 1", "E0308"),
        ("import fs\nfn f(fs) = fs", "E0308"),
        ("import fs\nfor fs in [1]: fs ;;", "E0308"),
        ("import fs\nmatch 1: fs => fs ;;", "E0308"),
        ("fn f():\n  import fs\n  1\n;;", "E0311"),
        ("import \"./lib/x\" (a)\na = 2", "E0209"), // listed names are constants (plan decision 4)
        ("import \"./lib/x\" (a)\nfn g() = a", ""), // and hoisted fns see them
        ("import \"./lib/x\" (a)\nprint(a, x)", "E0204"), // the module name is not bound when only names are listed
        ("fn map(xs) = xs\nprint(map(1))", ""),           // prelude names may be shadowed (04)
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

#[test]
fn library_top_level_restriction() {
    let cases = [
        (
            "import fs\nfn f() = 1\ntype t = int\nx = print(1)\n(a, b) = (1, 2)",
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
        ("match 1: Ok(a) | Err(a) => a ;;", ""), // the same name once per alternative is fine
        ("match 1: [a, b] when a == b => a ;;", ""),
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

/// Each warning as
/// `CODE:name`, in the order reported.
fn warnings_of(src: &str, opts: &Options) -> Vec<String> {
    let (r, errs) = resolve_with(src, &["print", "map", "len"], opts);
    assert!(errs.is_empty(), "{src:?}: {errs:?}");
    r.unwrap()
        .warnings
        .iter()
        .map(|w| format!("{}:{}", w.code, arg0(w)))
        .collect()
}

#[test]
fn warnings() {
    let mut imports = BTreeMap::new();
    imports.insert(
        "fs".to_string(),
        ModuleInfo {
            key: "fs".into(),
            name: "fs".into(),
            exports: names(&["read"]),
        },
    );
    let opts = Options {
        imports,
        ..Options::default()
    };
    let cases: &[(&str, &[&str])] = &[
        ("import fs", &["W0101:fs"]),
        ("import fs as f\nprint(f.read)", &[]),
        ("import fs (read)", &["W0101:read"]),
        ("import fs (read)\nprint(read)", &[]),
        ("fn f():\n  x = 1\n  2\n;;", &["W0102:x"]),
        ("fn f():\n  _x = 1\n  2\n;;", &[]),
        ("fn f(a) = 1", &[]), // parameters never warn
        ("x = 1", &[]),       // file globals never warn (they are the module's API)
        ("fn f():\n  (a, b) = (1, 2)\n  a\n;;", &["W0102:b"]),
        ("fn f(v) = match v: (a, _) => 1 ;;", &["W0102:a"]),
        ("fn f(v) = match v: x when x > 0 => 1; _ => 0 ;;", &[]),
        ("fn map(xs) = xs", &["W0103:map"]),
        ("len = 1", &["W0103:len"]),
        ("fn f(len) = len", &["W0103:len"]),
        ("fn f(v) = match v: map => map ;;", &["W0103:map"]),
        ("fn f():\n  x = 1\n  fn g() = x\n  g()\n;;", &[]), // read by a nested fn counts
        ("fn f():\n  x = 1\n  x = 2\n  x\n;;", &[]),        // reassigned then read
        // A block used as a value ends with an assignment.
        ("fn f():\n  x = if true: _y = 1 ;;\n  x\n;;", &["W0104:_y"]),
        ("fn f():\n  x = if true: 1 ;;\n  x\n;;", &[]),
        ("print(if true: _y = 1 ;;)", &["W0104:_y"]),
        // Any branch.
        (
            "fn f():\n  x = if true: 1 else: _z = 2 ;;\n  x\n;;",
            &["W0104:_z"],
        ),
        ("fn f() = if true: _z = 2 ;;", &[]), // fn bodies are not checked
        (
            "fn f():\n  return match 1: 1 => _y = 1 ;;\n;;",
            &["W0104:_y"],
        ),
        // more warning cases (Review Focus 3)
        ("fn f():\n  x = 1\n  h = () => x\n  h()\n;;", &[]), // a lambda's read counts
        ("x = 1\ntest \"t\":\n  assert x == 1\n;;", &[]),
        ("fn f(xs) = for (a, b) in xs: a ;;", &["W0102:b"]),
        ("fn f(v):\n  {a, b} = v\n  a\n;;", &["W0102:b"]),
        ("fn f(v) = match v: Ok(x) | Err(x) => 1 ;;", &["W0102:x"]),
        ("fn f(v) = match v: Ok(x) | Err(x) => x ;;", &[]),
        // Only its own body calls it: unused.
        (
            "fn f():\n  fn g(n) = if n > 0: g(n - 1) else: 0 ;;\n  1\n;;",
            &["W0102:g"],
        ),
        ("import fs\nfn f() = fs.read", &[]), // read by a top-level fn
        // Same position: the shadow warning comes first.
        (
            "fn f():\n  x = 1\n  len = 2\n  x\n;;",
            &["W0103:len", "W0102:len"],
        ),
        (
            "fn f():\n  b = 1\n  a = 2\n  0\n;;",
            &["W0102:b", "W0102:a"],
        ), // sorted by position
        (
            "fn f():\n  x = if true: (a, b) = (1, 2) ;;\n  x\n;;",
            &["W0104:this pattern", "W0102:a", "W0102:b"],
        ),
    ];
    let lib = Options {
        library: true,
        ..Options::default()
    };
    assert_eq!(
        warnings_of("fn map(xs) = xs\nlen = 1", &lib),
        ["W0103:map", "W0103:len"],
        "the rule does not depend on how a file is loaded (08)"
    );
    for (src, want) in cases {
        assert_eq!(warnings_of(src, &opts), *want, "{src:?}");
    }
}

#[test]
fn warnings_carry_positions_and_spans() {
    let src = "fn f():\n  len = 2\n  0\n;;";
    let (r, _) = resolve_with(src, &["len"], &Options::default());
    let w = &r.unwrap().warnings;
    assert_eq!((w[0].pos.line, w[0].pos.col, w[0].len), (2, 3, 3));
    assert_eq!(w[0].code, "W0103");
    assert_eq!(w[1].code, "W0102");
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
        ["E0210"],
        "without redefine a known fn is a duplicate"
    );
    let (r, errs) = resolve_with("y = 1", &["print"], &repl(&["x"], false));
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(r.unwrap().globals, names(&["x", "y"]), "new globals append");
    let (r, errs) = resolve_with("x = 5", &["print"], &repl(&["x"], true));
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(r.unwrap().globals.len(), 1, "reassigning a known global");
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
    };
    let mut imports = BTreeMap::new();
    imports.insert("fs".to_string(), info);
    let with = |redefine| Options {
        imports: imports.clone(),
        redefine,
        ..Options::default()
    };
    for (src, plain) in [
        ("fn f() = 1\nf = 2\nf", "E0209"),
        ("import fs\nfs = 1\nfs", "E0308"),
        ("import fs\nimport fs\nfs.exists", "E0309"),
        ("fn f() = 1\nfn f() = 2\nf()", "E0210"),
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
