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

#[test]
fn call_encoding() {
    for (np, nn) in [(0, 0), (3, 0), (2, 1), (4095, 4095)] {
        assert_eq!(decode_call(encode_call(np, nn)), (np, nn));
    }
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
    p.emit(Op::Call, encode_call(1, 0), at);
    p.emit(Op::TailCall, encode_call(0, 2), at);
    p.emit(Op::LoadSelf, 0, at);
    let want = format!(
        "== <main> ==\n\
         0000  1:1   CLOSURE      0    ; add\n\
         0001  1:1   CONST        0    ; 1\n\
         0002  1:1   CALL         1    ; 1 positional, 0 named\n\
         0003  1:1   TAILCALL     {}    ; 0 positional, 2 named\n\
         0004  1:1   LOADSELF\n\
         \n== add ==\n\
         0000  1:1   CHECKTYPE    0    ; a: int | nil\n\
         0001  1:1   LOADLOCAL    0\n\
         0002  1:1   RETURN\n",
        encode_call(0, 2)
    );
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
    p.emit(Op::Tag, 1, at);
    p.emit(Op::Slice, 3, at);
    let c = p.add_const(Const::Str("name".into()));
    p.emit(Op::Field, c, at);
    p.emit(Op::Match, 0, at);
    p.emit(Op::Range, 1, at);
    p.emit(Op::In, 0, at);
    assert_eq!(
        disassemble(&p),
        "== <main> ==\n\
         0000  1:1   LIST         2\n\
         0001  1:1   TAG          1    ; 1 args\n\
         0002  1:1   SLICE        3    ; start, end\n\
         0003  1:1   FIELD        0    ; name\n\
         0004  1:1   MATCH        0    ; $0\n\
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
        op: "==".into(),
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
