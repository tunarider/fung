//! Compiler tests by milestone (core forms, functions, collections,
//! modules, tests and asserts) and whole `fung dump bytecode` outputs.

use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::builtins::builtins;
use crate::bytecode::{Op, PatKind, disassemble, encode_call};
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
    let core = desugar(&parse(&toks).unwrap()).unwrap();
    let names: Vec<String> = prelude.iter().map(|s| s.to_string()).collect();
    resolve(&core, &names, ropts).unwrap()
}

fn opts(src: &str, tests: bool) -> Options {
    Options {
        source: src.into(),
        file: "t.fung".into(),
        tests,
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

fn bare(core: Node) -> Resolved {
    Resolved {
        core,
        num_locals: 0,
        globals: vec![],
        imports: vec![],
        exports: vec![],
        warnings: vec![],
    }
}

// ---- core forms ----

#[test]
fn compile_arithmetic() {
    let p = comp("1 + 2 * 3");
    assert_eq!(ops(&p), "CONST(0) CONST(1) CONST(2) MUL ADD RETURN");
    assert_eq!(p.consts.len(), 3);
    assert_eq!(p.consts[2], Const::Int(3));
}

#[test]
fn compile_statements_pop_all_but_last() {
    assert_eq!(ops(&comp("1\n2")), "CONST(0) POP CONST(1) RETURN");
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
    // (if true (op bool false) false)
    assert_eq!(
        ops(&comp("true and false")),
        "CONST(0) JUMPIFFALSE(5) CONST(1) CHECKBOOL JUMP(6) CONST(1) RETURN"
    );
}

#[test]
fn compile_unary_and_comparison() {
    assert_eq!(ops(&comp("not (1 < 2)")), "CONST(0) CONST(1) LT NOT RETURN");
    assert_eq!(ops(&comp("-1")), "CONST(0) NEG RETURN");
}

#[test]
fn compile_records_positions() {
    let p = comp("1 + 2");
    // CONST 1 at col 1, CONST 2 at col 5, ADD at col 3 (the operator)
    let cols: Vec<u32> = p.pos.iter().take(3).map(|p| p.col).collect();
    assert_eq!(cols, [1, 5, 3]);
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
        "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) STOREGLOBAL(1) CONST(1) JUMPIFFALSE(10) LOADGLOBAL(1) STORELOCAL(0) LOADLOCAL(0) JUMP(11) CONST(2) RETURN"
    );
    assert_eq!(p.num_locals, 1);
    assert_eq!(p.global_names, ["x", "y"]);
    // the nil const for the statement-valued program
    assert_eq!(p.consts[2], Const::Nil);
}

#[test]
fn compile_block_ending_in_statement_yields_nil() {
    assert_eq!(
        ops(&comp("x = 1")),
        "CONST(0) STOREGLOBAL(0) CONST(1) RETURN"
    );
}

#[test]
fn compile_literals_and_concat() {
    let p = comp(r#""a${1}b" + 'ok"#);
    assert_eq!(
        ops(&p),
        "CONST(0) CONST(1) CONST(2) CONCAT(3) CONST(3) ADD RETURN"
    );
    assert_eq!(p.consts[0], Const::Str("a".into()));
    assert_eq!(p.consts[3], Const::Sym("ok".into()));
    let p = comp("nil\n1h");
    assert_eq!(ops(&p), "CONST(0) POP CONST(1) RETURN");
    assert_eq!(p.consts[1], Const::Dur(3_600_000_000_000));
}

#[test]
fn jump_if_false_carries_condition_position() {
    let p = comp("1 and true");
    // JUMPIFFALSE points at the condition (col 1), not the operator (col 3)
    assert_eq!(p.pos[1].col, 1);
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
    assert_eq!(ops(add), "LOADLOCAL(0) LOADLOCAL(1) ADD RETURN");
}

#[test]
fn compile_defaults_and_type_checks() {
    let p = comp("fn f(a: int, b = a * 2) -> num = a + b");
    let f = &p.protos[0];
    // JUMPIFSET skips the default expression and its store (index 6).
    assert_eq!(
        ops(f),
        "LOADLOCAL(1) JUMPIFSET(6) LOADLOCAL(0) CONST(0) MUL STORELOCAL(1) CHECKTYPE(0) LOADLOCAL(0) LOADLOCAL(1) ADD CHECKRETURN(1) RETURN"
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
    // Not tail: operand of +, with a return type.
    for src in ["fn f(n) = 1 + f(n)", "fn f(n) -> int = f(n)"] {
        let got = ops(&comp(src).protos[0]);
        assert!(
            !got.contains("TAILCALL"),
            "{src:?} compiled a tail call: {got}"
        );
    }
    // An argument is not in tail position, but the outer call is.
    let got = ops(&comp("fn f(n) = print(f(n))").protos[0]);
    assert!(
        got.contains("LOADSELF LOADLOCAL(0) CALL(1) TAILCALL(1)"),
        "{got}"
    );
    // return of a call is a tail call; the branches of if are tail positions.
    let got = ops(&comp(
        "fn f(n):\n  if n > 0: return f(n - 1) ;;\n  if n < 0: f(0) else: 0 ;;\n;;",
    )
    .protos[0]);
    assert_eq!(got.matches("TAILCALL(1)").count(), 2, "{got}");
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
    let p = comp("fn f(a, b = 2) = a\nf(1, b: 3)");
    // Stack at the call: f, 1, 'b, 3 — the name symbol is constant 1.
    let got = ops(&p);
    let want = format!("CONST(1) CONST(2) CALL({})", encode_call(1, 1));
    assert!(got.contains(&want), "named call: {got}");
    assert_eq!(p.consts[1], Const::Sym("b".into()));
}

#[test]
fn too_many_arguments_or_parameters_is_a_compile_error() {
    let args = vec!["1"; 4096].join(", ");
    let err = compile_err(&format!("print({args})")).unwrap_err();
    assert_eq!(err.code, "E0806");
    assert_eq!(err.args, [Arg::Str("arguments".into()), Arg::Int(4095)]);
    assert_eq!((err.pos.line, err.pos.col), (1, 6));
    let names: Vec<String> = (0..4096).map(|i| format!("p{i}")).collect();
    let err = compile_err(&format!("fn f({}) = 1", names.join(", "))).unwrap_err();
    assert_eq!(err.code, "E0806");
    assert_eq!(err.args, [Arg::Str("parameters".into()), Arg::Int(4095)]);
    let ok = vec!["1"; 4095].join(", ");
    assert!(compile_err(&format!("print({ok})")).is_ok());
}

#[test]
fn check_type_carries_the_parameter_position() {
    let p = comp("fn f(a,\n  b: int) = b");
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
        ("[1, 2]", "CONST(0) CONST(1) LIST(2) RETURN"),
        ("{a: 1}", "CONST(0) CONST(1) MAP(1) RETURN"),
        ("(1,)", "CONST(0) TUPLE(1) RETURN"),
        ("Ok(1)", "CONST(0) CONST(1) TAG(1) RETURN"),
        ("0..=3", "CONST(0) CONST(1) RANGE(1) RETURN"),
        (
            "xs = []\nxs[0]",
            "LIST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(0) INDEX RETURN",
        ),
        (
            "xs = []\nxs[1..]",
            "LIST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(0) SLICE(1) RETURN",
        ),
        (
            "xs = []\nxs[..2]",
            "LIST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(0) SLICE(2) RETURN",
        ),
        (
            "m = {}\nm.k",
            "MAP(0) STOREGLOBAL(0) LOADGLOBAL(0) FIELD(0) RETURN",
        ),
        ("1 in [1]", "CONST(0) CONST(0) LIST(1) IN RETURN"),
        ("Ok(1)!", "CONST(0) CONST(1) TAG(1) UNWRAPABORT RETURN"),
        ("nil ?? 1", "CONST(0) COALESCE(3) JUMP(4) CONST(1) RETURN"),
    ];
    for (src, want) in cases {
        assert_eq!(ops(&comp(src)), want, "{src:?}");
    }
}

#[test]
fn compile_question_inside_function() {
    let p = comp("fn f(r) = r? + 1");
    // r; UNWRAPTEST L1; JUMP L2; L1: RETURN; L2: CONST 1; ADD; RETURN
    assert_eq!(
        ops(&p.protos[0]),
        "LOADLOCAL(0) UNWRAPTEST(3) JUMP(4) RETURN CONST(0) ADD RETURN"
    );
    let got = ops(&comp("fn f(r) -> int = r?").protos[0]);
    assert!(
        got.contains("UNWRAPTEST(3) JUMP(5) CHECKRETURN(0) RETURN"),
        "annotated: {got}"
    );
}

#[test]
fn compile_match_and_destructuring() {
    let p = comp("match 1:\n  0 => \"zero\"\n  n when n > 0 => \"pos\"\n  _ => \"other\"\n;;");
    let got = ops(&p);
    // Shape: subject stored; each arm loads it, matches, jumps on failure.
    for frag in [
        "CONST(0) STORELOCAL(0) LOADLOCAL(0) MATCH(0) JUMPIFFALSE(",
        "LOADLOCAL(0) MATCH(1) JUMPIFFALSE(",
        "LOADLOCAL(1) CONST(2) GT JUMPIFFALSE(",
        "LOADLOCAL(0) NOMATCH",
    ] {
        assert!(got.contains(frag), "missing {frag:?} in {got}");
    }
    assert_eq!(got.matches("JUMP(").count(), 3, "{got}");
    assert_eq!(p.patterns.len(), 3);
    assert_eq!(p.patterns[1].to_string(), "$1");
    assert_eq!(p.patterns[0].to_string(), "0");
    let p = comp("(a, b) = (1, 2)");
    assert_eq!(
        ops(&p),
        "CONST(0) CONST(1) TUPLE(2) MATCHORFAIL(0) CONST(2) RETURN"
    );
    assert_eq!(p.patterns[0].to_string(), "(@0, @1)");
}

#[test]
fn compile_rest_param() {
    let p = comp("fn f(a, ...xs) = xs");
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
    let got = compile_with_imports("import fs");
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
        (&*a.text, &*a.left, &*a.right, &*a.op),
        ("f(1) == 2", "f(1)", "", "==")
    );
    let a = &compile_opts("assert ok(1)", false).asserts[0];
    assert_eq!((&*a.text, &*a.op), ("ok(1)", ""));
    let p = compile_opts("fn g(x):\n  assert x > 0\n  x\n;;", false);
    // an assert inside a fn uses that proto's table
    let a = &p.protos[0].asserts[0];
    assert_eq!((&*a.text, &*a.left, &*a.op), ("x > 0", "x", ">"));
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
    assert!(
        !ops(&compile_opts("dbg(1, text: \"t\")", false)).contains("CALL(3)"),
        "named arguments disable the rewrite"
    );
}

// ---- whole outputs (`fung dump bytecode -e`) ----

fn bytecode(src: &str) -> String {
    let mut sm = SourceMap::new();
    let f = sm.add("-e", src);
    let (toks, _) = lex(f, src).unwrap();
    let names = builtins().prelude_names();
    let r = resolve(
        &desugar(&parse(&toks).unwrap()).unwrap(),
        &names,
        &ROpts::default(),
    )
    .unwrap();
    let set = names.into_iter().collect();
    let o = Options {
        source: src.into(),
        file: "-e".into(),
        tests: false,
    };
    disassemble(&compile(&r, &set, &o).unwrap())
}

#[test]
fn dump_outputs() {
    assert_eq!(
        bytecode("x = 1 + 2 * 3"),
        "== <main> ==\n\
         0000  1:5   CONST        0    ; 1\n\
         0001  1:9   CONST        1    ; 2\n\
         0002  1:13  CONST        2    ; 3\n\
         0003  1:11  MUL\n\
         0004  1:7   ADD\n\
         0005  1:1   STOREGLOBAL  0    ; x\n\
         0006  1:1   CONST        3    ; nil\n\
         0007  1:1   RETURN\n"
    );
}

#[test]
fn dump_outputs_functions_and_operators() {
    let src = "fn outer(y):
  fn loop(n) = n == 0 then y else loop(n - 1)
  g = x => x + y
  for i in [1]: print(i) ;;
  (a, [b, ...]) = (1, [2])
  return loop(2)
;;
fn h(r) -> int = r? + 1
assert h(Ok(1)) == 2
assert true
test \"t\": assert 1 < 2 ;;
not (1 < 2) and -1 > 0 or false
\"x\" == \"x\"; 2.0 / 1; 7 // 2; 7 % 2; 2 ** 3; 1 != 2; 1 <= 2; 1 >= 2";
    assert_eq!(bytecode(src), include_str!("testdata/functions.txt"));
}

#[test]
fn dump_outputs_data_and_patterns() {
    let src = "fn f(a: int, b = 2) -> int = a + b
xs = [1, 2]
match xs:
  [h, ...t] => h
  {k: v} | {j: v} => v
  Ok(1.5) => \"s${f(1, b: 2)}\"
  (x,) => x
  _ when true => nil
;;
m = {a: 1}
m.a; xs[1..]; xs[..=2]; xs[0]; 0..=3; 1 in xs; Ok(1)!; nil ?? 1; 'sym; 1h; print(1)
dbg(1)";
    assert_eq!(bytecode(src), include_str!("testdata/data.txt"));
}

/// `args` is the prelude's one plain value: the program's argument list,
/// empty when compiling for a dump: it prints as `[]`.
#[test]
fn prelude_args_is_an_empty_list() {
    assert_eq!(
        bytecode("args; take; args"),
        "== <main> ==\n\
         0000  1:1   CONST        0    ; []\n\
         0001  1:1   POP\n\
         0002  1:7   CONST        1    ; <fn take>\n\
         0003  1:7   POP\n\
         0004  1:13  CONST        0    ; []\n\
         0005  1:1   RETURN\n"
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
            let src = format!("x = [1]{}", "[0]".repeat(9_000));
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
