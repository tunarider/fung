//! The instruction set of the fung VM and the compiled-function model.
//! Every instruction has a source position in the parallel `Proto::pos`
//! (08: 모든 명령은 소스 위치를 가진다).

mod disasm;
mod op;
mod pattern;
#[cfg(test)]
mod tests;

pub use disasm::disassemble;
pub use op::{Instr, MAX_OPERAND, Op, decode_call, encode_call};
pub use pattern::{PatKind, Pattern};

use std::collections::HashMap;

use crate::consts::Const;
use crate::source::Pos;
use crate::tree::Capture;

/// One parameter of a compiled function.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Param {
    pub name: String,
    pub has_default: bool,
    /// `...name`: collects surplus positional arguments.
    pub rest: bool,
    /// Index into `Proto::types`, -1 for unchecked.
    pub ty: i32,
}

/// The source of one assert statement: the whole expression and, for a
/// top-level comparison, each side's text ("" for a literal side) and the
/// operator.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Assert {
    pub text: String,
    pub left: String,
    pub right: String,
    pub op: String,
}

/// A compiled function body ("prototype"): code, its constant pool, one
/// source position per instruction, the frame size, (for the module body)
/// the globals in index order, and the function's parameters, child
/// functions, captures and type specs.
#[derive(Clone, Debug)]
pub struct Proto {
    pub name: String,
    pub code: Vec<Instr>,
    pub consts: Vec<Const>,
    pub pos: Vec<Pos>,
    pub num_locals: usize,
    pub global_names: Vec<String>,
    pub params: Vec<Param>,
    /// Child functions, referenced by CLOSURE.
    pub protos: Vec<Proto>,
    /// What CLOSURE copies from the creating frame.
    pub captures: Vec<Capture>,
    /// Type specs referenced by CHECKTYPE/CHECKRETURN.
    pub types: Vec<Vec<String>>,
    /// Index into `types`, -1 for unchecked.
    pub return_type: i32,
    /// Pattern trees referenced by MATCH/MATCHORFAIL.
    pub patterns: Vec<Pattern>,
    /// (module body) the public globals, in order (04).
    pub exports: Vec<String>,
    /// What ASSERT reports (08).
    pub asserts: Vec<Assert>,
    /// Compiled from an embedded extras file: errors point at the caller (06).
    pub builtin: bool,
    /// Where each constant with a hashable key sits in `consts`, so that
    /// `add_const` finds an equal one without comparing with all of them.
    pub(crate) const_index: HashMap<ConstKey, usize>,
    /// The first instruction whose operand did not fit in 24 bits
    /// (`check_operands`).
    pub(crate) overflow: Option<(Op, Pos)>,
}

/// A constant as a hash key, under the pool's equality (`Const`'s
/// `PartialEq`): the type and the value, floats by their bits with -0.0
/// as 0.0, folded collections item by item. A NaN equals nothing, so a
/// constant holding one has no key and is compared one by one.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ConstKey {
    Int(i64),
    Float(u64),
    Bool(bool),
    Nil,
    Str(String),
    Bytes(Vec<u8>),
    Sym(String),
    Dur(i64),
    Builtin(String),
    Args,
    Dbg,
    List(Vec<ConstKey>),
    Map(Vec<(ConstKey, ConstKey)>),
    Tuple(Vec<ConstKey>),
    Tag(String, Vec<ConstKey>),
    Range(i64, i64, bool),
}

impl ConstKey {
    fn of(c: &Const) -> Option<ConstKey> {
        Some(match c {
            Const::Int(n) => ConstKey::Int(*n),
            Const::Float(f) if f.is_nan() => return None,
            Const::Float(f) => ConstKey::Float(if *f == 0.0 {
                0f64.to_bits()
            } else {
                f.to_bits()
            }),
            Const::Bool(b) => ConstKey::Bool(*b),
            Const::Nil => ConstKey::Nil,
            Const::Str(t) => ConstKey::Str(t.clone()),
            Const::Bytes(b) => ConstKey::Bytes(b.clone()),
            Const::Sym(t) => ConstKey::Sym(t.clone()),
            Const::Dur(d) => ConstKey::Dur(*d),
            Const::Builtin(t) => ConstKey::Builtin(t.clone()),
            Const::Args => ConstKey::Args,
            Const::Dbg => ConstKey::Dbg,
            Const::List(xs) => ConstKey::List(ConstKey::all(xs)?),
            Const::Tuple(xs) => ConstKey::Tuple(ConstKey::all(xs)?),
            Const::Tag(n, xs) => ConstKey::Tag(n.clone(), ConstKey::all(xs)?),
            Const::Range(a, b, i) => ConstKey::Range(*a, *b, *i),
            Const::Map(es) => ConstKey::Map(
                es.iter()
                    .map(|(k, v)| Some((ConstKey::of(k)?, ConstKey::of(v)?)))
                    .collect::<Option<_>>()?,
            ),
        })
    }

    fn all(xs: &[Const]) -> Option<Vec<ConstKey>> {
        xs.iter().map(ConstKey::of).collect()
    }
}

impl Default for Proto {
    fn default() -> Proto {
        Proto {
            name: String::new(),
            code: Vec::new(),
            consts: Vec::new(),
            pos: Vec::new(),
            num_locals: 0,
            global_names: Vec::new(),
            params: Vec::new(),
            protos: Vec::new(),
            captures: Vec::new(),
            types: Vec::new(),
            return_type: -1,
            patterns: Vec::new(),
            exports: Vec::new(),
            asserts: Vec::new(),
            builtin: false,
            const_index: HashMap::new(),
            overflow: None,
        }
    }
}

impl Proto {
    /// Registers a type spec and returns its index.
    pub fn add_type(&mut self, alts: Vec<String>) -> usize {
        self.types.push(alts);
        self.types.len() - 1
    }

    /// Appends an instruction and returns its index (for `patch`). An
    /// operand past 24 bits is remembered for `check_operands`.
    pub fn emit(&mut self, op: Op, arg: usize, pos: Pos) -> usize {
        if arg > MAX_OPERAND {
            self.overflow.get_or_insert((op, pos));
        }
        self.code.push(Instr::new(op, arg));
        self.pos.push(pos);
        self.code.len() - 1
    }

    /// Rewrites the operand of an earlier instruction (jump targets).
    pub fn patch(&mut self, at: usize, arg: usize) {
        let op = self.code[at].op();
        if arg > MAX_OPERAND {
            self.overflow.get_or_insert((op, self.pos[at]));
        }
        self.code[at] = Instr::new(op, arg);
    }

    /// E0230 when an instruction's operand did not fit in its 24 bits
    /// (VM11: a constant, jump or slot number cut short would run the
    /// wrong code), at the first one, naming what there were too many of.
    pub fn check_operands(&self) -> Result<(), crate::diag::Diag> {
        let Some((op, pos)) = self.overflow else {
            return Ok(());
        };
        let what = match op {
            Op::Const | Op::Field | Op::Import | Op::Test => "constants in a function",
            Op::List | Op::Tuple | Op::Map | Op::Tag => "items in one literal",
            Op::Concat => "parts in one string",
            Op::LoadLocal | Op::StoreLocal | Op::ClearLocal | Op::MoveLocal => {
                "local variables in a function"
            }
            Op::LoadGlobal | Op::StoreGlobal => "names in a file",
            Op::Closure => "functions in a function",
            Op::LoadCaptured => "captured values in a function",
            Op::Match | Op::MatchOrFail => "patterns in a function",
            Op::CheckReturn | Op::CheckType => "type annotations in a function",
            Op::Assert => "asserts in a function",
            _ => "instructions in a function",
        };
        Err(crate::diag::error(
            "E0230",
            pos,
            vec![
                crate::diag::Arg::Str(what.into()),
                crate::diag::Arg::Int(MAX_OPERAND as i64),
            ],
        ))
    }

    /// Adds a constant, reusing an existing equal one of the same type.
    pub fn add_const(&mut self, c: Const) -> usize {
        let key = ConstKey::of(&c);
        let found = match &key {
            Some(k) => self.const_index.get(k).copied(),
            None => self.consts.iter().position(|x| *x == c),
        };
        if let Some(i) = found {
            return i;
        }
        self.consts.push(c);
        let i = self.consts.len() - 1;
        if let Some(k) = key {
            self.const_index.insert(k, i);
        }
        i
    }
}
