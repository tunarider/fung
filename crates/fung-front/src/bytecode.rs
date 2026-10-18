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
        if let Some(i) = self.consts.iter().position(|x| *x == c) {
            return i;
        }
        self.consts.push(c);
        self.consts.len() - 1
    }
}
