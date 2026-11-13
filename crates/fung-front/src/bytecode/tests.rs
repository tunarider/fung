//! Bytecode tests: the instruction word and the opcodes, then by
//! milestone (collections, modules, tests).

use super::*;
use crate::consts::Const;
use crate::source::{FileId, Pos};

fn pos(line: u32, col: u32) -> Pos {
    Pos {
        file: FileId(0),
        line,
        col,
    }
}

fn main_proto() -> Proto {
    Proto {
        name: "<main>".into(),
        ..Proto::default()
    }
}

#[test]
fn encode_round_trip() {
    for (op, arg) in [
        (Op::Const, 0),
        (Op::Jump, (1 << 24) - 1),
        (Op::Call, 3),
        (Op::Add, 0),
    ] {
        let i = Instr::new(op, arg);
        assert_eq!((i.op(), i.arg()), (op, arg), "{op:?} {arg}");
    }
}

/// An operand wider than 24 bits is E0230, not cut short (VM11): at the
/// first such instruction, naming what there were too many of.
#[test]
fn an_operand_past_24_bits_is_e0230() {
    let mut p = main_proto();
    p.emit(Op::Const, MAX_OPERAND, pos(1, 1));
    p.emit(Op::Jump, 0, pos(2, 1));
    assert!(p.check_operands().is_ok());
    p.emit(Op::Const, MAX_OPERAND + 1, pos(3, 4));
    p.emit(Op::LoadLocal, MAX_OPERAND + 1, pos(4, 1));
    let e = p.check_operands().unwrap_err();
    assert_eq!(e.code, "E0230");
    assert_eq!((e.pos.line, e.pos.col), (3, 4));
    assert_eq!(
        e.args,
        [
            crate::diag::Arg::Str("constants in a function".into()),
            crate::diag::Arg::Int(MAX_OPERAND as i64)
        ]
    );
    let mut p = main_proto();
    let j = p.emit(Op::Jump, 0, pos(5, 2));
    p.patch(j, MAX_OPERAND + 1);
    let e = p.check_operands().unwrap_err();
    assert_eq!((e.pos.line, e.pos.col), (5, 2));
    assert_eq!(
        e.args[0],
        crate::diag::Arg::Str("instructions in a function".into())
    );
}

#[test]
fn proto_emit_patch_and_const_dedup() {
    let mut p = main_proto();
    let at_pos = pos(1, 1);
    let i0 = p.add_const(Const::Int(1));
    let i1 = p.add_const(Const::Int(1));
    let i2 = p.add_const(Const::Float(1.0));
    assert_eq!((i0, i1, i2), (0, 0, 1));
    // Floats compare with `==`: 0.0 and -0.0 share a slot, a NaN never does.
    let z = p.add_const(Const::Float(0.0));
    assert_eq!(p.add_const(Const::Float(-0.0)), z);
    let n = p.add_const(Const::Float(f64::NAN));
    assert_ne!(p.add_const(Const::Float(f64::NAN)), n);
    // Same text, different types: different slots.
    let s = p.add_const(Const::Str("a".into()));
    let y = p.add_const(Const::Sym("a".into()));
    assert_ne!(s, y);
    assert_eq!(p.add_const(Const::Sym("a".into())), y);
    assert_eq!(
        p.add_const(Const::List(Vec::new())),
        p.add_const(Const::List(Vec::new()))
    );
    assert_eq!(p.add_const(Const::Int(1)), i0);
    let at = p.emit(Op::JumpIfFalse, 0, at_pos);
    p.emit(Op::Const, i0, at_pos);
    p.patch(at, 2);
    assert_eq!(p.code[0].arg(), 2);
    assert_eq!(p.code[0].op(), Op::JumpIfFalse);
    assert_eq!(p.pos.len(), 2);
    assert_eq!(p.pos[1], at_pos);
}

#[test]
fn disassemble_basic() {
    let mut p = main_proto();
    let c = p.add_const(Const::Int(1));
    p.emit(Op::Const, c, pos(1, 7));
    let c = p.add_const(Const::Int(2));
    p.emit(Op::Const, c, pos(1, 11));
    p.emit(Op::Add, 0, pos(1, 9));
    p.emit(Op::Return, 0, pos(1, 7));
    assert_eq!(
        disassemble(&p),
        "== <main> ==\n\
         0000  1:7   CONST        0    ; 1\n\
         0001  1:11  CONST        1    ; 2\n\
         0002  1:9   ADD\n\
         0003  1:7   RETURN\n"
    );
}

#[test]
fn new_ops_have_args_and_names() {
    for op in [
        Op::LoadLocal,
        Op::StoreLocal,
        Op::LoadGlobal,
        Op::StoreGlobal,
        Op::Concat,
    ] {
        assert!(op.has_arg(), "{op:?}");
        assert!(!op.name().is_empty() && !op.name().starts_with("Op("));
    }
}

#[test]
fn disassemble_shows_global_names() {
    let mut p = Proto {
        global_names: vec!["x".into()],
        ..main_proto()
    };
    let at = pos(1, 1);
    let c = p.add_const(Const::Int(1));
    p.emit(Op::Const, c, at);
    p.emit(Op::StoreGlobal, 0, at);
    p.emit(Op::LoadGlobal, 0, at);
    p.emit(Op::LoadLocal, 2, at);
    assert_eq!(
        disassemble(&p),
        "== <main> ==\n\
         0000  1:1   CONST        0    ; 1\n\
         0001  1:1   STOREGLOBAL  0    ; x\n\
         0002  1:1   LOADGLOBAL   0    ; x\n\
         0003  1:1   LOADLOCAL    2\n"
    );
}

/// A closure is a runtime value (`<fn add>`, `<fn>` when anonymous); the
/// front end has no closures. What it shows of a function's name is the CLOSURE comment and the child
/// header, `<lambda>` when anonymous.
#[test]
fn closure_names_in_disassembly() {
    let at = pos(1, 1);
    let mut named = Proto {
        name: "add".into(),
        ..Proto::default()
    };
    named.emit(Op::Return, 0, at);
    let mut anon = Proto::default();
    anon.emit(Op::Return, 0, at);
    let mut p = Proto {
        protos: vec![named, anon],
        ..main_proto()
    };
    p.emit(Op::Closure, 0, at);
    p.emit(Op::Closure, 1, at);
    assert_eq!(
        disassemble(&p),
        "== <main> ==\n\
         0000  1:1   CLOSURE      0    ; add\n\
         0001  1:1   CLOSURE      1    ; <lambda>\n\
         \n== add ==\n\
         0000  1:1   RETURN\n\
         \n== <lambda> ==\n\
         0000  1:1   RETURN\n"
    );
}

#[test]
fn disassemble_functions() {
    let at = pos(1, 1);
    let mut child = Proto {
        name: "add".into(),
        num_locals: 2,
        params: vec![
            Param {
                name: "a".into(),
                has_default: false,
                rest: false,
                ty: 0,
            },
            Param {
                name: "b".into(),
                has_default: false,
                rest: false,
                ty: -1,
            },
        ],
        types: vec![vec!["int".into(), "nil".into()]],
        ..Proto::default()
    };
    child.emit(Op::CheckType, 0, at);
    child.emit(Op::LoadLocal, 0, at);
    child.emit(Op::Return, 0, at);
    let mut p = Proto {
        protos: vec![child],
        ..main_proto()
    };
    p.emit(Op::Closure, 0, at);
    let c = p.add_const(Const::Int(1));
    p.emit(Op::Const, c, at);
    p.emit(Op::Call, 1, at);
    p.emit_with(Op::TailCall, 0, 2, at);
    p.emit(Op::LoadSelf, 0, at);
    let want = "== <main> ==\n\
         0000  1:1   CLOSURE      0    ; add\n\
         0001  1:1   CONST        0    ; 1\n\
         0002  1:1   CALL         1 0    ; 1 positional, 0 named\n\
         0003  1:1   TAILCALL     0 2    ; 0 positional, 2 named\n\
         0004  1:1   LOADSELF\n\
         \n== add ==\n\
         0000  1:1   CHECKTYPE    0 0    ; a: int | nil\n\
         0001  1:1   LOADLOCAL    0\n\
         0002  1:1   RETURN\n";
    assert_eq!(disassemble(&p), want);
}

// ---- collections and patterns (M4) ----

fn pat(kind: PatKind) -> Pattern {
    Pattern {
        kind,
        ..Pattern::default()
    }
}

#[test]
fn pattern_string() {
    let p = Pattern {
        kids: vec![
            Pattern {
                name: "Ok".into(),
                kids: vec![Pattern {
                    index: 1,
                    ..pat(PatKind::Bind)
                }],
                ..pat(PatKind::Tag)
            },
            Pattern {
                kids: vec![Pattern {
                    lit: Some(Const::Int(1)),
                    ..pat(PatKind::Lit)
                }],
                rest: Some(Box::new(pat(PatKind::Wild))),
                ..pat(PatKind::List)
            },
            Pattern {
                keys: vec!["k".into()],
                kids: vec![Pattern {
                    global: true,
                    index: 2,
                    ..pat(PatKind::Bind)
                }],
                ..pat(PatKind::Map)
            },
            Pattern {
                kids: vec![
                    pat(PatKind::Wild),
                    Pattern {
                        lit: Some(Const::Str("s".into())),
                        ..pat(PatKind::Lit)
                    },
                ],
                ..pat(PatKind::Tuple)
            },
        ],
        ..pat(PatKind::Alt)
    };
    assert_eq!(p.to_string(), r#"Ok($1) | [1, ...] | {k: @2} | (_, "s")"#);
}

#[test]
fn pattern_string_rest_bind_and_single_tuple() {
    let p = Pattern {
        kids: vec![Pattern {
            index: 1,
            ..pat(PatKind::Bind)
        }],
        rest: Some(Box::new(Pattern {
            index: 2,
            ..pat(PatKind::Bind)
        })),
        ..pat(PatKind::List)
    };
    assert_eq!(p.to_string(), "[$1, ...$2]");
    let t = Pattern {
        kids: vec![pat(PatKind::Wild)],
        ..pat(PatKind::Tuple)
    };
    assert_eq!(t.to_string(), "(_,)");
}

#[test]
fn m4_ops_disassemble() {
    let at = pos(1, 1);
    let mut p = Proto {
        patterns: vec![pat(PatKind::Bind)],
        ..main_proto()
    };
    p.emit(Op::List, 2, at);
    let ok = p.add_const(Const::Sym("Ok".into()));
    p.emit_with(Op::Tag, 1, ok, at);
    p.emit(Op::Slice, 3, at);
    let c = p.add_const(Const::Str("name".into()));
    p.emit(Op::Field, c, at);
    p.emit_with(Op::Match, 6, 0, at);
    p.emit(Op::Range, 1, at);
    p.emit(Op::In, 0, at);
    assert_eq!(
        disassemble(&p),
        "== <main> ==\n\
         0000  1:1   LIST         2\n\
         0001  1:1   TAG          1 0    ; Ok, 1 args\n\
         0002  1:1   SLICE        3    ; start, end\n\
         0003  1:1   FIELD        1    ; name\n\
         0004  1:1   MATCH        6 0    ; $0, else 6\n\
         0005  1:1   RANGE        1    ; inclusive\n\
         0006  1:1   IN\n"
    );
}

// ---- modules (M5) ----

#[test]
fn import_opcode() {
    assert!(Op::Import.has_arg());
    assert_eq!(Op::Import.name(), "IMPORT");
    let i = Instr::new(Op::Import, 7);
    assert_eq!((i.op(), i.arg()), (Op::Import, 7));
}

// ---- tests and asserts (M6) ----

#[test]
fn assert_and_test_opcodes() {
    for op in [Op::Assert, Op::Test] {
        assert!(op.has_arg(), "{op:?} needs an argument");
    }
    assert_eq!((Op::Assert.name(), Op::Test.name()), ("ASSERT", "TEST"));
    let mut p = Proto::default();
    p.asserts.push(Assert {
        text: "a == b".into(),
        left: "a".into(),
        right: "b".into(),
        cmp: Some((Cmp::Eq, pos(1, 3))),
    });
    p.emit(Op::Assert, 0, pos(1, 1));
    let c = p.add_const(Const::Str("name".into()));
    p.emit(Op::Test, c, pos(1, 1));
    let s = disassemble(&p);
    assert!(s.contains("ASSERT       0    ; a == b"), "{s}");
    assert!(s.contains("TEST         0    ; \"name\""), "{s}");
}

// ---- the opcode table ----

#[test]
fn every_opcode_has_its_name_and_number() {
    let names = [
        "CONST",
        "POP",
        "ADD",
        "SUB",
        "MUL",
        "DIV",
        "FLOORDIV",
        "MOD",
        "POW",
        "NEG",
        "NOT",
        "EQ",
        "NE",
        "LT",
        "GT",
        "LE",
        "GE",
        "CHECKBOOL",
        "JUMP",
        "JUMPIFFALSE",
        "CALL",
        "RETURN",
        "LOADLOCAL",
        "STORELOCAL",
        "LOADGLOBAL",
        "STOREGLOBAL",
        "CONCAT",
        "CLOSURE",
        "LOADCAPTURED",
        "LOADSELF",
        "TAILCALL",
        "JUMPIFSET",
        "CHECKTYPE",
        "CHECKRETURN",
        "LIST",
        "MAP",
        "TUPLE",
        "TAG",
        "RANGE",
        "INDEX",
        "SLICE",
        "FIELD",
        "IN",
        "UNWRAPABORT",
        "UNWRAPTEST",
        "COALESCE",
        "MATCH",
        "MATCHORFAIL",
        "NOMATCH",
        "IMPORT",
        "ASSERT",
        "TEST",
        "CLEARLOCAL",
        "MOVELOCAL",
        "CALLSPREAD",
        "TAILCALLSPREAD",
        "TAILCHECK",
        "JUMPIFTRUE",
        "TAILCHECKBOOL",
    ];
    assert_eq!(Op::ALL.len(), names.len());
    for (i, (op, name)) in Op::ALL.iter().zip(names).enumerate() {
        assert_eq!(*op as usize, i);
        assert_eq!(op.name(), name);
        assert_eq!(Op::from_u8(i as u8), Some(*op));
    }
    assert_eq!(Op::from_u8(names.len() as u8), None);
    let no_arg = [
        Op::Pop,
        Op::Add,
        Op::Neg,
        Op::CheckBool,
        Op::Return,
        Op::LoadSelf,
        Op::Index,
        Op::In,
        Op::UnwrapAbort,
        Op::NoMatch,
    ];
    for op in no_arg {
        assert!(!op.has_arg(), "{op:?}");
    }
}

#[test]
fn encode_masks_the_operand_to_24_bits() {
    let i = Instr::new(Op::Const, (1 << 24) + 5);
    assert_eq!((i.op(), i.arg()), (Op::Const, 5));
}

// ---- the verifier (VM22) ----

/// A proto with `code` (one position each), `locals` slots and one
/// constant.
fn code(locals: usize, code: &[(Op, usize)]) -> Proto {
    let mut p = main_proto();
    p.num_locals = locals;
    p.add_const(Const::Int(1));
    for &(op, arg) in code {
        p.emit(op, arg, pos(1, 1));
    }
    p
}

/// What the verifier says is wrong with `p`.
fn violation(p: &Proto) -> String {
    match verify(p) {
        Ok(v) => panic!("verified: {v:?}"),
        Err(v) => v.to_string(),
    }
}

#[test]
fn the_verifier_passes_good_code_and_reports_the_deepest_stack() {
    let p = code(
        1,
        &[
            (Op::Const, 0),
            (Op::StoreLocal, 0),
            (Op::LoadLocal, 0),
            (Op::Const, 0),
            (Op::Lt, 0),
            (Op::JumpIfFalse, 9),
            (Op::Const, 0),
            (Op::MoveLocal, 0),
            (Op::Return, 0),
            (Op::Const, 0),
            (Op::Return, 0),
        ],
    );
    assert_eq!(verify(&p), Ok(Verified { max_stack: 2 }));
}

#[test]
fn the_verifier_catches_each_kind_of_broken_promise() {
    let cases: &[(&str, Proto)] = &[
        (
            "CONST 5: consts has 1",
            code(0, &[(Op::Const, 5), (Op::Return, 0)]),
        ),
        (
            "LOADLOCAL 3: local slots has 1",
            code(1, &[(Op::LoadLocal, 3), (Op::Return, 0)]),
        ),
        (
            "LOADGLOBAL 0: globals has 0",
            code(0, &[(Op::LoadGlobal, 0), (Op::Return, 0)]),
        ),
        (
            "CLOSURE 0: protos has 0",
            code(0, &[(Op::Closure, 0), (Op::Return, 0)]),
        ),
        (
            "JUMP 7: past the end",
            code(0, &[(Op::Jump, 7), (Op::Const, 0), (Op::Return, 0)]),
        ),
        (
            "JUMP 0: not forward",
            code(0, &[(Op::Jump, 0), (Op::Const, 0), (Op::Return, 0)]),
        ),
        (
            "falls off its end",
            code(0, &[(Op::Const, 0), (Op::Pop, 0)]),
        ),
        (
            "ADD takes 2 values from a stack of 1",
            code(0, &[(Op::Const, 0), (Op::Add, 0), (Op::Return, 0)]),
        ),
        (
            "paths meet with stack heights 2 and 0",
            code(
                0,
                &[
                    (Op::Const, 0),
                    (Op::JumpIfFalse, 4),
                    (Op::Const, 0),
                    (Op::Const, 0),
                    (Op::Const, 0),
                    (Op::Return, 0),
                ],
            ),
        ),
        (
            "LOADLOCAL reads slot 0, which may be unassigned",
            code(1, &[(Op::LoadLocal, 0), (Op::Return, 0)]),
        ),
        (
            // Assigned on one branch only.
            "MOVELOCAL reads slot 0, which may be unassigned",
            code(
                1,
                &[
                    (Op::Const, 0),
                    (Op::JumpIfFalse, 4),
                    (Op::Const, 0),
                    (Op::StoreLocal, 0),
                    (Op::MoveLocal, 0),
                    (Op::Return, 0),
                ],
            ),
        ),
        (
            // Read after its value was let go.
            "LOADLOCAL reads slot 0, which may be unassigned",
            code(
                1,
                &[
                    (Op::Const, 0),
                    (Op::StoreLocal, 0),
                    (Op::ClearLocal, 0),
                    (Op::LoadLocal, 0),
                    (Op::Return, 0),
                ],
            ),
        ),
        (
            // The VM reads the tail call a TAILCHECK hands its check to.
            "TAILCHECK is not followed by a tail call",
            {
                let mut p = code(0, &[(Op::Const, 0)]);
                p.add_type(vec!["int".into()]);
                p.emit(Op::TailCheck, 0, pos(1, 1));
                p.emit(Op::Return, 0, pos(1, 1));
                p
            },
        ),
    ];
    for (want, p) in cases {
        let got = violation(p);
        assert!(got.contains(want), "want {want:?}, got {got:?}");
    }
}

/// The slot facts the verifier knows from the code around a read: a
/// parameter with a default is tested where it may be empty, a MATCH
/// binds on the path it goes on with, not on its jump (VM29), and a
/// capture is a read.
#[test]
fn the_verifier_follows_defaults_matches_and_captures() {
    let mut p = code(
        2,
        &[
            (Op::JumpIfSet, 3),
            (Op::Const, 0),
            (Op::StoreLocal, 0),
            (Op::CheckType, 0),
            (Op::Const, 0),
            (Op::Match, 8),
            (Op::MoveLocal, 1),
            (Op::Return, 0),
            (Op::Const, 0),
            (Op::Return, 0),
        ],
    );
    p.params.push(Param {
        name: "a".into(),
        has_default: true,
        ty: 0,
        ..Param::default()
    });
    p.add_type(vec!["int".into()]);
    p.patterns.push(Pattern {
        index: 1,
        ..pat(PatKind::Bind)
    });
    p.body = 3;
    assert!(verify(&p).is_ok(), "{:?}", verify(&p));
    // The failing path of the MATCH has not bound slot 1.
    p.code[7] = Instr::new(Op::Pop, 0);
    p.code[8] = Instr::new(Op::MoveLocal, 1);
    assert!(violation(&p).contains("MOVELOCAL reads slot 1"));

    let mut main = code(1, &[(Op::Closure, 0), (Op::Return, 0)]);
    main.protos.push(Proto {
        captures: vec![crate::tree::Capture {
            kind: crate::tree::CaptureKind::Local,
            index: 0,
        }],
        ..code(0, &[(Op::Const, 0), (Op::Return, 0)])
    });
    assert!(violation(&main).contains("CLOSURE reads slot 0"));
    main.protos[0].captures[0].index = 4;
    assert!(violation(&main).contains("out of range"));
}

/// What the VM takes on trust beyond ranges and heights (VM40): the kind
/// of constant an operand names, the slots and shape of a pattern, where
/// a call that gives every argument starts, a spread's list, flag bits,
/// and the one place a slot may be read empty.
#[test]
fn the_verifier_checks_what_the_operands_mean() {
    let at = pos(1, 1);
    // `ops` after a CONST, with `consts` after the first constant.
    let with = |consts: Vec<Const>, ops: &[(Op, usize, usize)]| {
        let mut p = code(0, &[(Op::Const, 0)]);
        for c in consts {
            p.add_const(c);
        }
        for &(op, arg, b) in ops {
            p.emit_with(op, arg, b, at);
        }
        p.emit(Op::Return, 0, at);
        p
    };
    let matching = |locals: usize, pattern: Pattern| {
        let mut p = code(
            locals,
            &[(Op::Const, 0), (Op::MatchOrFail, 0), (Op::Const, 0)],
        );
        p.patterns.push(pattern);
        p.emit(Op::Return, 0, at);
        p
    };
    let keys = |n: usize| Const::List((0..n).map(|i| Const::Str(format!("k{i}"))).collect());
    let cases: Vec<(&str, Proto)> = vec![
        ("MAP 1: its 2 keys", with(vec![keys(2)], &[(Op::Map, 1, 1)])),
        (
            "MAP 1: its keys are not a list of strings",
            with(vec![Const::List(vec![Const::Int(1)])], &[(Op::Map, 1, 1)]),
        ),
        (
            "MAP 1: its keys are not a list of strings",
            with(vec![], &[(Op::Map, 1, 0)]),
        ),
        (
            "TAG 1: its name is not a symbol",
            with(vec![], &[(Op::Tag, 1, 0)]),
        ),
        ("FIELD 0: not a string", with(vec![], &[(Op::Field, 0, 0)])),
        (
            "IMPORT 0: not a string",
            with(vec![], &[(Op::Pop, 0, 0), (Op::Import, 0, 0)]),
        ),
        (
            "TEST 0: not a string",
            with(vec![], &[(Op::Test, 0, 0), (Op::Const, 0, 0)]),
        ),
        (
            "pattern 0 binds slot 5: local slots has 1",
            matching(
                1,
                Pattern {
                    index: 5,
                    ..pat(PatKind::Bind)
                },
            ),
        ),
        (
            // Far past the slots: the verifier itself must not fail.
            "pattern 0 binds slot 900: local slots has 1",
            matching(
                1,
                Pattern {
                    kids: vec![Pattern {
                        index: 900,
                        ..pat(PatKind::Bind)
                    }],
                    ..pat(PatKind::Tuple)
                },
            ),
        ),
        (
            "pattern 0 binds global 3: globals has 0",
            matching(
                1,
                Pattern {
                    index: 3,
                    global: true,
                    ..pat(PatKind::Bind)
                },
            ),
        ),
        (
            "pattern 0: a map pattern with 1 keys for 0 patterns",
            matching(
                0,
                Pattern {
                    keys: vec!["a".into()],
                    ..pat(PatKind::Map)
                },
            ),
        ),
        (
            "pattern 0: a literal pattern without its literal",
            matching(0, pat(PatKind::Lit)),
        ),
        (
            "the body starts at 999: past the end",
            Proto {
                body: 999,
                ..code(0, &[(Op::Const, 0), (Op::Return, 0)])
            },
        ),
        (
            // A call that gives every argument starts in the middle of an
            // expression.
            "paths meet with stack heights 1 and 0",
            Proto {
                body: 1,
                ..code(0, &[(Op::Const, 0), (Op::Return, 0)])
            },
        ),
        (
            "CALLSPREAD 0: no list to spread",
            with(vec![], &[(Op::CallSpread, 0, 0)]),
        ),
        (
            "SLICE 248: flags past 7",
            with(vec![], &[(Op::Slice, 0xF8, 0)]),
        ),
        (
            "RANGE 7: flags past 1",
            with(vec![], &[(Op::Const, 0, 0), (Op::Range, 7, 0)]),
        ),
        (
            // Only a parameter with a default is tested while empty.
            "JUMPIFSET 0: not a parameter with a default",
            code(
                1,
                &[
                    (Op::JumpIfSet, 2),
                    (Op::Const, 0),
                    (Op::Const, 0),
                    (Op::Return, 0),
                ],
            ),
        ),
        (
            "LOADLOCAL reads slot 0, which may be unassigned",
            code(1, &[(Op::LoadLocal, 0), (Op::Return, 0)]),
        ),
    ];
    for (want, p) in &cases {
        let got = violation(p);
        assert!(got.contains(want), "want {want:?}, got {got:?}");
    }
    // What only the prologue assigned is not there for a call that
    // starts at the body.
    let mut skipped = code(
        2,
        &[
            (Op::JumpIfSet, 5),
            (Op::Const, 0),
            (Op::StoreLocal, 1),
            (Op::Const, 0),
            (Op::StoreLocal, 0),
            (Op::LoadLocal, 1),
            (Op::Return, 0),
        ],
    );
    skipped.params.push(Param {
        name: "a".into(),
        has_default: true,
        ty: -1,
        ..Param::default()
    });
    skipped.body = 5;
    assert!(violation(&skipped).contains("LOADLOCAL reads slot 1"));
    // The same shapes, right.
    let good = with(
        vec![
            keys(1),
            Const::Sym("Some".into()),
            Const::Str("name".into()),
        ],
        &[(Op::Map, 1, 1), (Op::Tag, 1, 2), (Op::Field, 3, 0)],
    );
    assert!(verify(&good).is_ok(), "{:?}", verify(&good));
}
