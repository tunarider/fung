//! Compiler tests by milestone (core forms, functions, collections,
//! modules, tests and asserts) and whole `fung dump bytecode` outputs.

use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::builtins::builtins;
use crate::bytecode::{Cmp, Op, PatKind, disassemble};
use crate::consts::Const;
use crate::desugar::desugar;
use crate::diag::Arg;
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::{ModuleInfo, Options as ROpts, Resolved, resolve};
use crate::source::{FileId, Pos, SourceMap};
use crate::tree::{Head, Lit, Node};

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| s.to_string()).collect()
}

fn resolved(src: &str, prelude: &[&str], ropts: &ROpts) -> Resolved {
    let mut sm = SourceMap::new();
    let f = sm.add("t.fung", src);
    let (toks, _) = lex(f, src).unwrap();
    let core = desugar(&parse(&toks).unwrap(), true).unwrap();
    let names: Vec<String> = prelude.iter().map(|s| s.to_string()).collect();
    // The last value is kept, as desugared.
    let ropts = ROpts {
        keep_value: true,
        ..ropts.clone()
    };
    resolve(&core, &names, &ropts).unwrap()
}

fn opts(src: &str, tests: bool) -> Options<'_> {
    Options {
        source: src,
        file: "t.fung",
        tests,
        keep_value: true,
    }
}

/// Compiles with the prelude's `print` and no source text: the error.
fn compile_err(src: &str) -> Result<Proto, Diag> {
    let r = resolved(src, &["print"], &ROpts::default());
    compile(&r, &set(&["print"]), &opts("", false))
}

fn comp(src: &str) -> Proto {
    compile_err(src).unwrap_or_else(|e| panic!("{src:?}: {e:?}"))
}

/// Strips positions for shape comparisons: "CONST(0) ADD RETURN".
fn ops(p: &Proto) -> String {
    p.code
        .iter()
        .map(|i| {
            if i.op().has_arg() {
                format!("{}({})", i.op().name(), i.arg())
            } else {
                i.op().name().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn zero() -> Pos {
    Pos {
        file: FileId(0),
        line: 0,
        col: 0,
    }
}

fn bare(mut core: Node) -> Resolved {
    core.number();
    Resolved {
        core,
        num_locals: 0,
        globals: vec![],
        imports: vec![],
        exports: vec![],
        sigs: Default::default(),
    }
}

// ---- core forms ----

#[test]
fn compile_arithmetic() {
    // Arithmetic on number literals is one constant (VM10 g).
    let p = comp("1 + 2 * 3");
    assert_eq!(ops(&p), "CONST(0) RETURN");
    assert_eq!(p.consts, [Const::Int(7)]);
    let p = comp("7 // -2 - 1.5 / 3");
    assert_eq!(p.consts, [Const::Float(-4.5)]);
    // What would fail is left to the VM, which fails at the same place.
    for src in [
        "9223372036854775807 + 1",
        "1 // 0",
        "1 % 0",
        "1.0 / 0",
        "-0.0",
    ] {
        assert_ne!(ops(&comp(src)), "CONST(0) RETURN", "{src}");
    }
}

#[test]
fn compile_statements_pop_all_but_last() {
    assert_eq!(
        ops(&comp("print(1)\n2")),
        "CONST(0) CONST(1) CALL(1) POP CONST(2) RETURN"
    );
}

#[test]
fn compile_empty_program_returns_nil() {
    let p = comp("");
    assert_eq!(ops(&p), "CONST(0) RETURN");
    assert_eq!(p.consts[0], Const::Nil);
}

#[test]
fn compile_call_and_prelude() {
    let p = comp("print(1, 2)");
    assert_eq!(ops(&p), "CONST(0) CONST(1) CONST(2) CALL(2) RETURN");
    assert_eq!(p.consts[0], Const::Builtin("print".into()));
}

#[test]
fn compile_if_with_jumps() {
    // (if true (op bool false) false): a literal needs no CHECKBOOL, and
    // a literal condition no test (VM31); the jump past the dead else
    // goes to a RETURN, so it is that RETURN (VM28).
    assert_eq!(
        ops(&comp("true and false")),
        "CONST(0) RETURN CONST(0) RETURN"
    );
    // As a condition, `and` and `or` jump to the branch (VM10 d).
    assert_eq!(
        ops(&comp("a = true\nif a and a: 1 else: 2 ;;")),
        "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) JUMPIFFALSE(8) LOADGLOBAL(0) \
         JUMPIFFALSE(8) CONST(1) RETURN CONST(2) RETURN"
    );
    // `or`'s left side jumps to the branch when true (VM27).
    assert_eq!(
        ops(&comp("a = true\nif a or a: 1 else: 2 ;;")),
        "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) JUMPIFTRUE(6) LOADGLOBAL(0) \
         JUMPIFFALSE(8) CONST(1) RETURN CONST(2) RETURN"
    );
    // `not` of a comparison is the comparison with the targets swapped.
    assert_eq!(
        ops(&comp("a = 1\nif not (a < 2): 1 else: 2 ;;")),
        "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(1) LT JUMPIFTRUE(8) CONST(0) \
         RETURN CONST(1) RETURN"
    );
}

#[test]
fn compile_unary_and_comparison() {
    assert_eq!(ops(&comp("not (1 < 2)")), "CONST(0) CONST(1) LT NOT RETURN");
    assert_eq!(ops(&comp("-1")), "CONST(0) RETURN");
    assert_eq!(
        ops(&comp("a = 1; -a")),
        "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) NEG RETURN"
    );
}

#[test]
fn compile_records_positions() {
    let p = comp("1 == 2");
    // CONST 1 at col 1, CONST 2 at col 6, EQ at col 3 (the operator)
    let cols: Vec<u32> = p.pos.iter().take(3).map(|p| p.col).collect();
    assert_eq!(cols, [1, 6, 3]);
}

#[test]
fn compile_rejects_unknown_core_operator() {
    let int = |i| Node::leaf(Head::Int, zero(), Lit::Int(i));
    let bad = Node::with_lit(
        Head::Op,
        zero(),
        Lit::Str("@@".into()),
        vec![int(1), int(2)],
    );
    let err = compile(
        &bare(Node::new(Head::Do, zero(), vec![bad])),
        &set(&["print"]),
        &Options::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, "E0801");
    assert_eq!(err.args, [Arg::Str("compiler: unknown operator @@".into())]);
    let bad_unary = Node::with_lit(Head::Op, zero(), Lit::Str("@@".into()), vec![int(1)]);
    let err = compile(
        &bare(Node::new(Head::Do, zero(), vec![bad_unary])),
        &set(&["print"]),
        &Options::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, "E0801");
}

#[test]
fn compile_rejects_unknown_prelude_name() {
    let core = Node::new(
        Head::Do,
        zero(),
        vec![Node::leaf(Head::Prelude, zero(), Lit::Str("nope".into()))],
    );
    let err = compile(&bare(core), &set(&["print"]), &Options::default()).unwrap_err();
    assert_eq!(err.code, "E0801");
    assert_eq!(
        err.args,
        [Arg::Str("compiler: unknown prelude name nope".into())]
    );
}

#[test]
fn compile_let_and_loads() {
    let p = comp("x = 1\ny = x\nif true: a = y; a ;;");
    assert_eq!(
        ops(&p),
        "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) STOREGLOBAL(1) LOADGLOBAL(1) STORELOCAL(0) MOVELOCAL(0) RETURN CONST(1) RETURN"
    );
    assert_eq!(p.num_locals, 1);
    assert_eq!(p.global_names, ["x", "y"]);
    // the nil const of the else branch that is never taken (VM31)
    assert_eq!(p.consts[1], Const::Nil);
}

#[test]
fn compile_block_ending_in_statement_yields_nil() {
    assert_eq!(
        ops(&comp("_x = 1")),
        "CONST(0) STOREGLOBAL(0) CONST(1) RETURN"
    );
}

#[test]
fn compile_literals_and_concat() {
    // An interpolation of literals is one constant (VM31), inside a list
    // literal too. (Its `+` with a symbol is a compile error, E0501, FE81.)
    let p = comp(r#"["a${1}b", 'ok]"#);
    assert_eq!(
        p.consts[0],
        Const::List(vec![Const::Str("a1b".into()), Const::Sym("ok".into())])
    );
    let p = comp(r#""x" + "y${-2}" + "\xff""#);
    assert_eq!(ops(&p), "CONST(0) RETURN");
    assert_eq!(p.consts[0], Const::Bytes(b"xy-2\xff".to_vec()));
    let p = comp(
        r#"a = 1.5
"x${a}" + "y""#,
    );
    assert!(ops(&p).contains("CONCAT(2) CONST(2) ADD"), "{}", ops(&p));
    let p = comp("1h");
    assert_eq!(ops(&p), "CONST(0) RETURN");
    assert_eq!(p.consts[0], Const::Dur(3_600_000_000_000));
}

#[test]
fn jump_if_false_carries_condition_position() {
    let p = comp("x = 1\nx and true");
    // JUMPIFFALSE points at the condition (col 1), not the operator (col 3)
    let at = p
        .code
        .iter()
        .position(|i| i.op() == Op::JumpIfFalse)
        .expect("a jump");
    assert_eq!((p.pos[at].line, p.pos[at].col), (2, 1));
}

// ---- functions ----

#[test]
fn compile_function_proto_and_closure() {
    let p = comp("fn add(a, b) = a + b\nprint(add(1, 2))");
    assert_eq!(
        ops(&p),
        "CLOSURE(0) STOREGLOBAL(0) CONST(0) LOADGLOBAL(0) CONST(1) CONST(2) CALL(2) CALL(1) RETURN"
    );
    let add = &p.protos[0];
    assert_eq!(add.name, "add");
    assert_eq!(add.num_locals, 2);
    assert_eq!(add.params.len(), 2);
    assert_eq!(add.return_type, -1);
    assert_eq!(ops(add), "MOVELOCAL(0) MOVELOCAL(1) ADD RETURN");
}

#[test]
fn compile_defaults_and_type_checks() {
    let p = comp("fn f(a: int, b = a * 2) -> num = a + b");
    let f = &p.protos[0];
    // JUMPIFSET skips the default expression and its store (index 5).
    assert_eq!(
        ops(f),
        "JUMPIFSET(5) LOADLOCAL(0) CONST(0) MUL STORELOCAL(1) CHECKTYPE(0) MOVELOCAL(0) MOVELOCAL(1) ADD CHECKRETURN(1) RETURN"
    );
    assert_eq!(f.types, [vec!["int".to_string()], vec!["num".to_string()]]);
    assert_eq!(f.return_type, 1);
    assert_eq!((f.params[0].ty, f.params[1].ty), (0, -1));
    assert!(f.params[1].has_default && !f.params[0].has_default);
}

#[test]
fn compile_tail_calls() {
    let p = comp("fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)");
    let got = ops(&p.protos[0]);
    assert!(
        got.contains("TAILCALL(2)") && !got.contains(" CALL("),
        "expected a tail call: {got}"
    );
    // Not tail: operand of +.
    let got = ops(&comp("fn f(n) = 1 + f(n)").protos[0]);
    assert!(!got.contains("TAILCALL"), "compiled a tail call: {got}");
    // With a return type, a call to another fn is a tail call that takes
    // the check along (D44); a call to itself needs none (03).
    let got = ops(&comp("fn f(n) -> int = g(n)\nfn g(n) = n").protos[0]);
    assert!(got.ends_with("TAILCHECK(0) TAILCALL(1)"), "{got}");
    let got = ops(&comp("fn f(n) -> int = f(n)").protos[0]);
    assert!(
        got.contains("TAILCALL") && !got.contains("TAILCHECK"),
        "{got}"
    );
    // A spread call is a tail call too (D45).
    let got = ops(&comp("fn f(xs) = print(...xs)").protos[0]);
    assert!(got.ends_with("TAILCALLSPREAD(1)"), "{got}");
    let got = ops(&comp("fn f(xs) = [print(0, ...xs)]").protos[0]);
    assert!(got.contains(" CALLSPREAD("), "{got}");
    assert!(!got.contains("TAILCALLSPREAD"), "{got}");
    // An argument is not in tail position, but the outer call is.
    let got = ops(&comp("fn f(n) = print(f(n))").protos[0]);
    assert!(
        got.contains("LOADSELF MOVELOCAL(0) CALL(1) TAILCALL(1)"),
        "{got}"
    );
    // return of a call is a tail call; the branches of if are tail positions.
    let got = ops(&comp(
        "fn f(n):\n  if n > 0: return f(n - 1) ;;\n  if n < 0: f(0) else: 0 ;;\n;;",
    )
    .protos[0]);
    assert_eq!(got.matches("TAILCALL(1)").count(), 2, "{got}");
}

/// Nothing follows a TAILCALL or RETURN that ends a tail position: the
/// call ends the frame by itself, so a RETURN (or a CHECKRETURN) after it
/// could never run (VM17).
#[test]
fn no_return_after_a_tail_call() {
    for src in [
        "fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)",
        "fn f(n) = print(n)",
        "fn f(n) -> int = n == 0 then 0 else f(n - 1)",
        "fn f(n):\n  if n > 0: return f(n - 1) ;;\n  if n < 0: f(0) else: print(0) ;;\n;;",
        "fn f(n) = match n:\n  0 => print(0)\n  _ => f(n - 1)\n;;",
        // the RETURN after the TAILCALL is the left side's (VM26)
        "fn f(n) = (n ?? 0) + 0",
        "fn f(n):\n  return n\n;;",
    ] {
        let p = comp(src);
        let got = ops(&p.protos[0]);
        for dead in [
            "TAILCALL(1) RETURN",
            "TAILCALL(2) RETURN",
            "TAILCALL(1) CHECKRETURN",
        ] {
            assert!(!got.contains(dead), "{src:?}: {got}");
        }
        assert!(!got.contains("RETURN RETURN"), "{src:?}: {got}");
    }
    // A function that does not end in a tail call still returns.
    assert!(ops(&comp("fn f(n) = n + 1").protos[0]).ends_with("ADD RETURN"));
}

/// A guarded arm whose pattern is one name or `_` always matches its
/// pattern: only the guard is tested, with no MATCH (VM17).
#[test]
fn a_guarded_name_arm_tests_only_its_guard() {
    let got = ops(&comp(
        "fn f(m) = match m:\n  k when k > 1 => 1\n  _ when m < 0 => 2\n  _ => 3\n;;",
    )
    .protos[0]);
    assert!(!got.contains("MATCH"), "{got}");
    assert!(
        got.starts_with("LOADLOCAL(0) STORELOCAL(1) LOADLOCAL(1)"),
        "{got}"
    );
}

#[test]
fn compile_captures_self_and_named_args() {
    let p = comp(
        "fn outer(y):\n  fn loop(n) = n == 0 then y else loop(n - 1)\n  loop(2)\n;;\nouter(1)",
    );
    let lp = &p.protos[0].protos[0];
    assert_eq!(lp.captures.len(), 1);
    assert_eq!(lp.captures[0].index, 0);
    let got = ops(lp);
    assert!(
        got.contains("LOADCAPTURED(0)") && got.contains("LOADSELF"),
        "loop = {got}"
    );
    let p = comp("fn f(a, b = 2) = [a, b]\nf(1, b: 3)");
    // Stack at the call: f, 1, 'b, 3 — the name symbol is constant 1.
    let got = ops(&p);
    let want = "CONST(1) CONST(2) CALL(1)";
    assert!(got.contains(want), "named call: {got}");
    assert_eq!(p.consts[1], Const::Sym("b".into()));
}

/// Arguments and parameters have the limit of every other count, an
/// operand's 24 bits (VM43): the 4,095 of a packed operand is gone.
#[test]
fn many_arguments_and_parameters_compile() {
    let args = vec!["1"; 5000].join(", ");
    let p = comp(&format!("print({args})"));
    let call = p.code.iter().find(|i| i.op() == Op::Call).unwrap();
    assert_eq!((call.arg(), call.b()), (5000, 0));
    let names: Vec<String> = (0..5000).map(|i| format!("_p{i}")).collect();
    assert!(compile_err(&format!("fn f({}) = 1", names.join(", "))).is_ok());
    let named: Vec<String> = (0..5000).map(|i| format!("_p{i}: 1")).collect();
    assert!(
        compile_err(&format!(
            "fn f({}) = 1\nprint(f({}))",
            names.join(", "),
            named.join(", ")
        ))
        .is_ok()
    );
}

#[test]
fn check_type_carries_the_parameter_position() {
    let p = comp("fn f(_a,\n  b: int) = b");
    let f = &p.protos[0];
    let i = f
        .code
        .iter()
        .position(|i| i.op() == Op::CheckType)
        .expect("no CHECKTYPE emitted");
    assert_eq!((f.pos[i].line, f.pos[i].col), (2, 3));
}

// ---- collections and patterns (M4) ----

#[test]
fn compile_collections_and_ops() {
    let cases = [
        // A list, map, tuple or tag value of constants is one constant
        // (P7, L5), empty ones and nested ones too; one with a variable
        // is built.
        ("[1, 2]", "CONST(0) RETURN"),
        (
            "x = 1\n[x, 2]",
            "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(1) LIST(2) RETURN",
        ),
        ("[[]]", "CONST(0) RETURN"),
        ("[]", "CONST(0) RETURN"),
        ("{a: 1}", "CONST(0) RETURN"),
        ("{}", "CONST(0) RETURN"),
        ("(1,)", "CONST(0) RETURN"),
        ("Ok(1)", "CONST(0) RETURN"),
        ("{k: [1, (2, 'a)], \"j k\": Some({})}", "CONST(0) RETURN"),
        (
            "x = 1
{a: x}",
            // the keys are the MAP's second operand (VM30)
            "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) MAP(1) RETURN",
        ),
        (
            "x = 1
(1, x)",
            "CONST(0) STOREGLOBAL(0) CONST(0) LOADGLOBAL(0) TUPLE(2) RETURN",
        ),
        (
            "x = 1
Ok([x])",
            // the name is the TAG's second operand (VM30)
            "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) LIST(1) TAG(1) RETURN",
        ),
        ("0..=3", "CONST(0) RETURN"),
        (
            "a = 3\n0..=a",
            "CONST(0) STOREGLOBAL(0) CONST(1) LOADGLOBAL(0) RANGE(1) RETURN",
        ),
        (
            "xs = []\nxs[0]",
            "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(1) INDEX RETURN",
        ),
        (
            "xs = []\nxs[1..]",
            "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(1) SLICE(1) RETURN",
        ),
        (
            "xs = []\nxs[..2]",
            "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(1) SLICE(2) RETURN",
        ),
        (
            "m = {}\nm.k",
            "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) FIELD(1) RETURN",
        ),
        ("1 in [1]", "CONST(0) CONST(1) IN RETURN"),
        ("Ok(1)!", "CONST(0) UNWRAPABORT RETURN"),
        // the value goes past the right side, with no JUMP (VM26)
        ("nil ?? 1", "CONST(0) COALESCE(3) CONST(1) RETURN"),
    ];
    for (src, want) in cases {
        assert_eq!(ops(&comp(src)), want, "{src:?}");
    }
}

#[test]
fn compile_question_inside_function() {
    let p = comp("fn f(r) = r? + 1");
    // r; UNWRAPTEST L1; CONST 1; ADD; RETURN; L1: RETURN -- the Err's return
    // is after the function's end, so success goes straight on (VM10 f).
    assert_eq!(
        ops(&p.protos[0]),
        "MOVELOCAL(0) UNWRAPTEST(5) CONST(0) ADD RETURN RETURN"
    );
    let got = ops(&comp("fn f(r) -> int = r?").protos[0]);
    assert_eq!(
        got, "MOVELOCAL(0) UNWRAPTEST(4) CHECKRETURN(0) RETURN CHECKRETURN(0) RETURN",
        "annotated"
    );
}

#[test]
fn compile_match_and_destructuring() {
    let p = comp("match 1:\n  0 => \"zero\"\n  n when n > 0 => \"pos\"\n  _ => \"other\"\n;;");
    let got = ops(&p);
    // Shape: subject stored; each arm loads it, matches, jumps on failure.
    // A guarded name stores the subject and tests only the guard (VM17).
    // The last arm `_` always matches: no test, and no NOMATCH after it
    // (VM10 c). Each body lets go of the subject as it starts, and of
    // `n`, read last by the guard; the last also of `n` the failed arm
    // bound (VM19).
    for frag in [
        // MATCH goes to the next arm itself when it fails (VM29)
        "CONST(0) STORELOCAL(0) LOADLOCAL(0) MATCH(7) CLEARLOCAL(0) CONST(1) RETURN",
        "LOADLOCAL(0) STORELOCAL(1) LOADLOCAL(1) CONST(2) GT JUMPIFFALSE(17) CLEARLOCAL(0) \
         CLEARLOCAL(1) CONST(3) RETURN",
        "RETURN CLEARLOCAL(0) CLEARLOCAL(1) CONST(4) RETURN",
    ] {
        assert!(got.contains(frag), "missing {frag:?} in {got}");
    }
    assert!(!got.contains("NOMATCH"), "{got}");
    // the jumps past the other arms went to the RETURN: they are it (VM28)
    assert_eq!(got.matches("JUMP(").count(), 0, "{got}");
    assert_eq!(p.patterns.len(), 1);
    assert_eq!(p.patterns[0].to_string(), "0");
    let p = comp("(_a, _b) = (1, 2)");
    assert_eq!(ops(&p), "CONST(0) MATCHORFAIL(0) CONST(1) RETURN");
    assert_eq!(p.patterns[0].to_string(), "(@0, @1)");
}

#[test]
fn compile_rest_param() {
    let p = comp("fn f(_a, ...xs) = xs");
    assert!(p.protos[0].params[1].rest, "Rest flag not copied");
}

// ---- modules (M5) ----

fn compile_with_imports(src: &str) -> String {
    let mut imports = BTreeMap::new();
    imports.insert(
        "fs".to_string(),
        ModuleInfo {
            key: "fs".into(),
            name: "fs".into(),
            exports: vec!["read".into()],
            ..Default::default()
        },
    );
    let ropts = ROpts {
        imports,
        ..ROpts::default()
    };
    let r = resolved(src, &["print"], &ropts);
    ops(&compile(&r, &set(&["print"]), &opts("", false)).unwrap())
}

#[test]
fn compile_import() {
    assert_eq!(
        compile_with_imports("import fs\nprint(fs.read)"),
        "IMPORT(0) STOREGLOBAL(0) CONST(1) LOADGLOBAL(0) FIELD(2) CALL(1) RETURN"
    );
    assert_eq!(
        compile_with_imports("import fs (read)\nread"),
        "IMPORT(0) FIELD(1) STOREGLOBAL(0) LOADGLOBAL(0) RETURN"
    );
    let got = compile_with_imports("import fs as _fs");
    assert!(
        got.ends_with("STOREGLOBAL(0) CONST(1) RETURN"),
        "import as last statement leaves nil: {got}"
    );
}

// ---- tests and asserts (M6) ----

const M6_PRELUDE: [&str; 5] = ["print", "dbg", "f", "ok", "map"];

fn compile_opts(src: &str, tests: bool) -> Proto {
    let r = resolved(src, &M6_PRELUDE, &ROpts::default());
    compile(&r, &set(&M6_PRELUDE), &opts(src, tests)).unwrap()
}

fn str_consts(p: &Proto) -> Vec<&str> {
    p.consts
        .iter()
        .filter_map(|c| match c {
            Const::Str(s) => Some(s.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn compile_assert() {
    let p = compile_opts("assert f(1) == 2", false);
    assert_eq!(
        ops(&p),
        "CONST(0) CONST(1) CALL(1) CONST(2) ASSERT(0) CONST(3) RETURN"
    );
    // literal sides are blank
    let a = &p.asserts[0];
    assert_eq!(
        (&*a.text, &*a.left, &*a.right, a.cmp.map(|c| (c.0, c.1.col))),
        ("f(1) == 2", "f(1)", "", Some((Cmp::Eq, 13)))
    );
    let a = &compile_opts("assert ok(1)", false).asserts[0];
    assert_eq!((&*a.text, a.cmp), ("ok(1)", None));
    let p = compile_opts("fn g(x):\n  assert x > 0\n  x\n;;", false);
    // an assert inside a fn uses that proto's table
    let a = &p.protos[0].asserts[0];
    assert_eq!(
        (&*a.text, &*a.left, a.cmp.map(|c| c.0)),
        ("x > 0", "x", Some(Cmp::Gt))
    );
}

#[test]
fn compile_test_blocks() {
    let src = "test \"a\": assert true ;;\nprint(1)";
    let p = compile_opts(src, true);
    let got = ops(&p);
    assert!(got.starts_with("CLOSURE(0) TEST(0) "), "with tests: {got}");
    assert_eq!(p.protos[0].name, "test: a");
    let got = ops(&compile_opts(src, false));
    assert!(
        !got.contains("TEST") && !got.contains("CLOSURE"),
        "without tests the block compiles to nothing: {got}"
    );
    let got = ops(&compile_opts("test \"only\": assert true ;;", false));
    assert_eq!(got, "CONST(0) RETURN", "a file of only tests is empty");
}

#[test]
fn compile_dbg_appends_text_and_place() {
    let p = compile_opts("xs = [1]\nxs |> map(x => x) |> dbg", false);
    let got = ops(&p);
    assert!(
        got.contains("CALL(3) "),
        "dbg gets three positional args: {got}"
    );
    let texts = str_consts(&p);
    assert!(
        texts.contains(&"xs |> map(x => x)") && texts.contains(&"t.fung:2"),
        "consts = {texts:?}"
    );
    assert!(
        !ops(&compile_opts("d = dbg\nd(1)", false)).contains("CALL(3)"),
        "an aliased dbg is a plain call"
    );
}

// ---- whole outputs (`fung dump bytecode -e`) ----

fn bytecode(src: &str) -> String {
    let mut sm = SourceMap::new();
    let f = sm.add("-e", src);
    let (toks, _) = lex(f, src).unwrap();
    let names = builtins().prelude_names();
    let r = resolve(
        &desugar(&parse(&toks).unwrap(), true).unwrap(),
        &names,
        &ROpts {
            keep_value: true,
            ..ROpts::default()
        },
    )
    .unwrap();
    let set = names.into_iter().collect();
    let o = Options {
        source: src,
        file: "-e",
        tests: false,
        keep_value: true,
    };
    disassemble(&compile(&r, &set, &o).unwrap())
}

#[test]
fn dump_outputs() {
    assert_eq!(
        bytecode("_x = 1 + 2 * 3"),
        "== <main> ==\n\
         0000  1:8   CONST        0    ; 7\n\
         0001  1:1   STOREGLOBAL  0    ; _x\n\
         0002  1:1   CONST        1    ; nil\n\
         0003  1:1   RETURN\n"
    );
}

/// A local is let go after the statement that reads it last (03, L7):
/// CLEARLOCAL follows that statement, unless the read moved the value out
/// already. One never read is kept.
#[test]
fn locals_are_cleared_after_their_last_read() {
    let ops = |src: &str| -> Vec<String> {
        let b = bytecode(src);
        let f = b.split("== f ==\n").nth(1).expect("f's code").to_string();
        f.lines()
            .take_while(|l| !l.is_empty())
            .map(|l| {
                let w: Vec<&str> = l.split_whitespace().skip(2).take(2).collect();
                w.iter()
                    .take_while(|x| **x != ";")
                    .copied()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    };
    assert_eq!(
        ops("fn f(a, b):\n  print(a)\n  x = b\n  print(x)\n  0\n;;"),
        [
            "CONST 0",
            "MOVELOCAL 0",
            "CALL 1",
            "POP",
            "MOVELOCAL 1",
            "STORELOCAL 2",
            "CONST 0",
            "MOVELOCAL 2",
            "CALL 1",
            "POP",
            "CONST 1",
            "RETURN"
        ]
    );
    // Read in a branch: let go after the if; never read: kept; captured
    // by a lambda: that is a read, let go right after the CLOSURE (VM19).
    assert_eq!(
        ops("fn f(a, b, c):\n  _k = c\n  if b: print(a) ;;\n  g = () => b\n  print(1)\n  g\n;;"),
        [
            "MOVELOCAL 2",
            "STORELOCAL 3",
            "LOADLOCAL 1",
            "JUMPIFFALSE 8",
            "CONST 0",
            "MOVELOCAL 0",
            "CALL 1",
            "POP",
            "CLEARLOCAL 0",
            "CLOSURE 0",
            "CLEARLOCAL 1",
            "STORELOCAL 4",
            "CONST 0",
            "CONST 1",
            "CALL 1",
            "POP",
            "MOVELOCAL 4",
            "RETURN"
        ]
    );
}

/// The loads of locals in `f`'s code, in order (`LOADLOCAL i` or
/// `MOVELOCAL i`).
fn local_loads(src: &str) -> Vec<String> {
    let b = bytecode(src);
    let f = b.split("== f ==\n").nth(1).expect("f's code").to_string();
    f.lines()
        .take_while(|l| !l.is_empty())
        .filter_map(|l| {
            let w: Vec<&str> = l.split_whitespace().skip(2).take(2).collect();
            matches!(w[0], "LOADLOCAL" | "MOVELOCAL").then(|| w.join(" "))
        })
        .collect()
}

/// The last read of a local moves its value out of the slot (03, P9):
/// reads before it in evaluation order, in later statements, in a match's
/// subject or guard, and captures copy. Each branch moves its own last
/// read.
#[test]
fn the_last_read_of_a_local_moves_it() {
    let cases: &[(&str, &[&str])] = &[
        (
            "fn f(m, i) = put(m, \"k${i}\", i)",
            &["MOVELOCAL 0", "LOADLOCAL 1", "MOVELOCAL 1"],
        ),
        (
            "fn f(m):\n  a = put(m, \"x\", 1)\n  len(m) + len(a)\n;;",
            &["LOADLOCAL 0", "MOVELOCAL 0", "MOVELOCAL 1"],
        ),
        (
            "fn f(x, c) = c then len(x) else str(x)",
            &["MOVELOCAL 1", "MOVELOCAL 0", "MOVELOCAL 0"],
        ),
        (
            "fn f(x, c):\n  if c: print(x) ;;\n  len(x)\n;;",
            &["MOVELOCAL 1", "LOADLOCAL 0", "MOVELOCAL 0"],
        ),
        (
            "fn f(x) = match x:\n  [a] when a > 0 => a\n  _ => len(x)\n;;",
            // The subject, a local, is read where it is (VM10 b); `a` is
            // slot 1. The last arm always matches: no test (VM10 c).
            &["LOADLOCAL 0", "LOADLOCAL 1", "MOVELOCAL 1", "MOVELOCAL 0"],
        ),
        (
            "fn f(x):\n  g = () => x\n  [g, x]\n;;",
            &["MOVELOCAL 1", "MOVELOCAL 0"],
        ),
        ("fn f(x):\n  print(x)\n  () => x\n;;", &["LOADLOCAL 0"]),
        ("fn f(x, y) = x ?? y", &["MOVELOCAL 0", "MOVELOCAL 1"]),
        ("fn f(x) = x ?? len(x)", &["LOADLOCAL 0", "MOVELOCAL 0"]),
        (
            "fn f(x):\n  if x > 0: return x ;;\n  x\n;;",
            &["LOADLOCAL 0", "MOVELOCAL 0", "MOVELOCAL 0"],
        ),
    ];
    for (src, want) in cases {
        assert_eq!(local_loads(src), *want, "{src}");
    }
}

#[test]
fn dump_outputs_functions_and_operators() {
    let src = "fn outer(y):
  fn loop(k) = k == 0 then y else loop(k - 1)
  _g = x => x + y
  for i in [1]: print(i) ;;
  (_a, [_b, ...]) = (1, [2])
  return loop(2)
;;
fn h(r) -> int = r? + 1
assert h(Ok(1)) == 2
assert true
test \"t\": assert 1 < 2 ;;
_c = not (1 < 2) and -1 > 0 or false
n = 2
print(\"x\" == \"x\", 2.0 / n, 7 // n, 7 % n, n ** 3, 1 != n, 1 <= n, 1 >= n); 1 + 2 * 3";
    same_as_file(&bytecode(src), "functions.txt");
}

/// Compares a dump with `testdata/<name>`; `FUNG_UPDATE_TESTDATA=1`
/// rewrites the file after a deliberate change.
fn same_as_file(got: &str, name: &str) {
    let path = format!(
        "{}/src/compiler/testdata/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    if std::env::var_os("FUNG_UPDATE_TESTDATA").is_some() {
        std::fs::write(&path, got).unwrap();
    }
    assert_eq!(got, std::fs::read_to_string(&path).unwrap(), "{name}");
}

#[test]
fn dump_outputs_data_and_patterns() {
    let src = "fn f(a: int, b = 2) -> int = a + b
xs = [1, 2]
_r = match xs:
  [h, ..._t] => h
  {k: v} | {j: v} => v
  Ok(1.5) => \"s${f(1, b: 2)}\"
  (x,) => x
  _ when true => nil
;;
m = {a: 1}
print(m.a, xs[1..], xs[..=2], xs[0], 0..=3, 1 in xs, Ok(1)!, nil ?? 1, 'sym, 1h); print(1)
dbg(1)";
    same_as_file(&bytecode(src), "data.txt");
}

/// `args` is the prelude's one plain value: the program's argument list,
/// which the VM fills in, so it has a constant of its own (L5).
#[test]
fn prelude_args_is_a_constant_of_its_own() {
    assert_eq!(
        bytecode("print(args, take); args"),
        "== <main> ==\n\
         0000  1:1   CONST        0    ; <fn print>\n\
         0001  1:7   CONST        1    ; args\n\
         0002  1:13  CONST        2    ; <fn take>\n\
         0003  1:6   CALL         2 0    ; 2 positional, 0 named\n\
         0004  1:1   POP\n\
         0005  1:20  CONST        1    ; args\n\
         0006  1:1   RETURN\n"
    );
}

#[test]
fn pattern_constants() {
    let p = comp("match 1:\n  'a | nil | 2s | true | \"s\" | 1.5 => 1\n  [x, ...] => x\n;;");
    assert_eq!(
        p.patterns[0].to_string(),
        "'a | nil | 2s | true | \"s\" | 1.5"
    );
    assert_eq!(p.patterns[0].kind, PatKind::Alt);
    assert_eq!(p.patterns[1].to_string(), "[$1, ...]");
}

#[test]
fn deep_trees_do_not_overflow() {
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(|| {
            let src = format!("_x = [1]{}", "[0]".repeat(9_000));
            let p = comp(&src);
            assert_eq!(p.code.iter().filter(|i| i.op() == Op::Index).count(), 9_000);
            let src = format!("fn f(a) = {}a{}", "[".repeat(9_000), "]".repeat(9_000));
            let p = comp(&src);
            assert_eq!(
                p.protos[0]
                    .code
                    .iter()
                    .filter(|i| i.op() == Op::List)
                    .count(),
                9_000
            );
            let src = format!(
                "match 1:\n  {}x{} => x\n;;",
                "[".repeat(9_000),
                "]".repeat(9_000)
            );
            let p = comp(&src);
            let want = format!("{}$1{}", "[".repeat(9_000), "]".repeat(9_000));
            assert_eq!(p.patterns[0].to_string(), want);
            assert!(disassemble(&p).contains("MATCH"));
        })
        .unwrap()
        .join()
        .unwrap();
}

/// The code of function `name` in `src`: one `OP arg` per instruction.
fn fn_ops(src: &str, name: &str) -> Vec<String> {
    let b = bytecode(src);
    let f = b
        .split(&format!("== {name} ==\n"))
        .nth(1)
        .expect("the function's code")
        .to_string();
    f.lines()
        .take_while(|l| !l.is_empty())
        .map(|l| {
            let w: Vec<&str> = l.split_whitespace().skip(2).take(2).collect();
            w.iter()
                .take_while(|x| **x != ";")
                .copied()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// The right side of a tail `and`/`or` is in tail position as a whole:
/// the branch that returns `true` at once lets go of nothing (VM53).
#[test]
fn a_tail_and_or_nested_lets_go_of_nothing_on_the_quick_path() {
    let got = fn_ops("fn a(x) = x > 0 and (x > 1 or b(x))\nfn b(x) = x == 3", "a");
    assert!(!got.iter().any(|o| o.starts_with("CLEARLOCAL")), "{got:?}");
}

/// A branch lets go, as it starts, of what only the other paths read,
/// and a capture that is a slot's last read lets go right after the
/// CLOSURE (VM19, D61). A branch that calls nothing and ends the frame
/// lets go of nothing: the return does.
#[test]
fn branches_and_captures_let_go_where_they_start() {
    // `tx` is slot 1, `rx` slot 2: each branch starts by letting go of
    // what only the other reads.
    let got = fn_ops(
        "fn f(flag):\n  (tx, rx) = channel(1)\n  if flag:\n    send(tx, 1)\n  else:\n    \
         for x in rx: x ;;\n  ;;\n;;",
        "f",
    );
    assert_eq!(got[4..7], ["MOVELOCAL 0", "JUMPIFFALSE 11", "CLEARLOCAL 2"]);
    assert_eq!(got[11], "CLEARLOCAL 1");
    // The capture is the last read of `tx`.
    let got = fn_ops(
        "fn f(s):\n  (tx, rx) = channel(1)\n  [spawn(s, () => send(tx, 1)), recv(rx)]\n;;",
        "f",
    );
    let at = got.iter().position(|o| o == "CLOSURE 0").unwrap();
    assert_eq!(got[at + 1], "CLEARLOCAL 1");
    // The arm's body lets go of the subject and of what its guard read.
    let got = fn_ops(
        "fn f(x, t):\n  y = match x:\n    n when ok(t) => g(n)\n    _ => 0\n  ;;\n  y\n;;\nfn ok(t) = t\nfn g(n) = n",
        "f",
    );
    let at = got
        .iter()
        .position(|o| o.starts_with("JUMPIFFALSE"))
        .unwrap();
    assert_eq!(got[at + 1..at + 3], ["CLEARLOCAL 0", "CLEARLOCAL 1"]);
    // Quiet: the base case returns at once.
    let got = fn_ops("fn f(n, y) = n == 0 then y else f(n - 1, y)", "f");
    assert!(!got.iter().any(|o| o.starts_with("CLEARLOCAL")), "{got:?}");
}

/// A call that passes every parameter by position starts past the
/// prologue that fills defaults (VM21): `body` marks where it ends.
#[test]
fn body_starts_after_the_defaults_prologue() {
    let p = comp("fn f(a, b = 1, c = 2) = a + b + c\nprint(f(1))");
    let f = &p.protos[0];
    assert_eq!(
        ops(f),
        "JUMPIFSET(3) CONST(0) STORELOCAL(1) JUMPIFSET(6) CONST(1) STORELOCAL(2) MOVELOCAL(0) MOVELOCAL(1) ADD MOVELOCAL(2) ADD RETURN"
    );
    assert_eq!(f.body, 6);
    let g = &comp("fn g(a) = a\nprint(g(1))").protos[0];
    assert_eq!(g.body, 0);
}

/// An `if` statement whose body returns leaves no POP no path reaches
/// (VM46).
#[test]
fn a_returning_if_statement_drops_nothing() {
    let p = comp("fn f(c, x):\n  if c: return x ;;\n  0\n;;\nprint(f)");
    let f = &p.protos[0];
    assert!(!ops(f).contains("POP"), "{}", ops(f));
    let p = comp("fn f(c, x):\n  if c: print(x) ;;\n  0\n;;\nprint(f)");
    assert!(ops(&p.protos[0]).contains("POP"));
}
