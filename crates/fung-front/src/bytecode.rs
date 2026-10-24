//! The instruction set of the fung VM and the compiled-function model.
//! Every instruction has a source position in the parallel `Proto::pos`
//! (08: 모든 명령은 소스 위치를 가진다).

mod disasm;
mod op;
mod pattern;
#[cfg(test)]
mod tests;

pub use disasm::disassemble;
pub use op::{Instr, Op, decode_call, encode_call};
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
}

/// A constant as a hash key, under the pool's equality (`Const`'s
/// `PartialEq`): the type and the value, floats by their bits with -0.0
/// as 0.0. A NaN equals nothing and a list is rare (only `args`), so
/// neither has a key; they are compared one by one.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ConstKey {
    Int(i64),
    Float(u64),
    Bool(bool),
    Nil,
    Str(String),
    Sym(String),
    Dur(i64),
    Builtin(String),
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
            Const::Sym(t) => ConstKey::Sym(t.clone()),
            Const::Dur(d) => ConstKey::Dur(*d),
            Const::Builtin(t) => ConstKey::Builtin(t.clone()),
            Const::List(_) => return None,
        })
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
        }
    }
}

impl Proto {
    /// Registers a type spec and returns its index.
    pub fn add_type(&mut self, alts: Vec<String>) -> usize {
        self.types.push(alts);
        self.types.len() - 1
    }

    /// Appends an instruction and returns its index (for `patch`).
    pub fn emit(&mut self, op: Op, arg: usize, pos: Pos) -> usize {
        self.code.push(Instr::new(op, arg));
        self.pos.push(pos);
        self.code.len() - 1
    }

    /// Rewrites the operand of an earlier instruction (jump targets).
    pub fn patch(&mut self, at: usize, arg: usize) {
        self.code[at] = Instr::new(self.code[at].op(), arg);
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
