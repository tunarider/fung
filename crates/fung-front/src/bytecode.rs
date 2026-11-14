//! The instruction set of the fung VM and the compiled-function model.
//! Every instruction has a source position in the parallel `Proto::pos`
//! (08: 모든 명령은 소스 위치를 가진다).

mod disasm;
mod op;
mod pattern;
#[cfg(test)]
mod tests;
mod verify;

pub use disasm::disassemble;
pub use op::{Effect, Flow, Info, Instr, MAX_OPERAND, Op, Operand};
pub use pattern::{PatKind, Pattern};
pub use verify::{Violation, verify};

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
/// top-level comparison, each side's text ("" for a literal side), the
/// operator and where it is written (its errors point there, as the
/// operator's own do, VM45).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Assert {
    pub text: String,
    pub left: String,
    pub right: String,
    pub cmp: Option<(Cmp, Pos)>,
}

/// The comparison an assert splits to show both sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    In,
    NotIn,
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
    /// The first instruction after the prologue that fills defaults: a
    /// call that passes every parameter by position starts there (VM21).
    /// 0 when no parameter has a default.
    pub body: usize,
    /// Where each constant with a hashable key sits in `consts`, so that
    /// `add_const` finds an equal one without comparing with all of them.
    pub(crate) const_index: HashMap<ConstKey, usize>,
    /// The first instruction whose operand did not fit in 24 bits
    /// (`check_operands`).
    pub(crate) overflow: Option<(Op, Operand, Pos)>,
    /// The furthest place a jump was pointed at so far (`patch`): a jump
    /// to the end of the code means the next instruction is reached.
    pub(crate) last_target: usize,
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
            body: 0,
            const_index: HashMap::new(),
            overflow: None,
            last_target: 0,
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
        self.emit_with(op, arg, 0, pos)
    }

    /// `emit` with a second operand (`Op::second`, VM32).
    pub fn emit_with(&mut self, op: Op, arg: usize, b: usize, pos: Pos) -> usize {
        if arg > MAX_OPERAND {
            self.overflow.get_or_insert((op, op.info().operand, pos));
        }
        if b > MAX_OPERAND {
            self.overflow.get_or_insert((op, op.second(), pos));
        }
        self.code.push(Instr::with(op, arg, b));
        self.pos.push(pos);
        self.code.len() - 1
    }

    /// Rewrites the operand of an earlier instruction (jump targets).
    pub fn patch(&mut self, at: usize, arg: usize) {
        let (op, b) = (self.code[at].op(), self.code[at].b());
        if arg > MAX_OPERAND {
            self.overflow
                .get_or_insert((op, op.info().operand, self.pos[at]));
        }
        self.code[at] = Instr::with(op, arg, b);
        if op.jumps() {
            self.last_target = self.last_target.max(arg);
        }
    }

    /// Whether the code ends in an instruction that ends the frame and no
    /// jump goes past it: what would come next cannot be reached (VM28).
    pub fn ends_closed(&self) -> bool {
        self.code.last().is_some_and(|i| {
            matches!(
                i.op(),
                Op::Return | Op::TailCall | Op::TailCallSpread | Op::NoMatch
            )
        }) && self.last_target < self.code.len()
    }

    /// Points each jump that lands on an unconditional JUMP at that
    /// JUMP's target, and turns a JUMP to a RETURN into the RETURN
    /// (VM28). Targets only move forward, so jumps stay forward.
    ///
    /// Where each chain of forward JUMPs ends is found once, from the
    /// back (VM51: following the chain from every jump was quadratic in
    /// a long `else if`).
    pub fn thread_jumps(&mut self) {
        let n = self.code.len();
        let mut end: Vec<usize> = (0..n).collect();
        for i in (0..n).rev() {
            let ins = self.code[i];
            if ins.op() == Op::Jump && ins.arg() > i {
                end[i] = end.get(ins.arg()).copied().unwrap_or(ins.arg());
            }
        }
        for at in 0..n {
            let op = self.code[at].op();
            if !op.jumps() {
                continue;
            }
            // Only instructions after `at` are read, which this pass has
            // not changed yet.
            let mut to = self.code[at].arg();
            if to > at && to < n {
                to = end[to];
            } else {
                while let Some(next) = self.code.get(to)
                    && next.op() == Op::Jump
                    && next.arg() > to
                {
                    to = next.arg();
                }
            }
            if op == Op::Jump && self.code.get(to).is_some_and(|i| i.op() == Op::Return) {
                self.code[at] = Instr::new(Op::Return, 0);
            } else if to != self.code[at].arg() {
                self.code[at] = Instr::with(op, to, self.code[at].b());
            }
        }
    }

    /// E0230 when an instruction's operand did not fit in its 24 bits
    /// (VM11: a constant, jump or slot number cut short would run the
    /// wrong code), at the first one, naming what there were too many of.
    pub fn check_operands(&self) -> Result<(), crate::diag::Diag> {
        let Some((op, operand, pos)) = self.overflow else {
            return Ok(());
        };
        let what = match operand {
            _ if op == Op::Concat => "parts in one string",
            _ if op.info().effect == Effect::Called => "arguments",
            Operand::Local if op == Op::CheckType || op == Op::JumpIfSet => "parameters",
            Operand::Const => "constants in a function",
            Operand::Count => "items in one literal",
            Operand::Local => "local variables in a function",
            Operand::Global => "names in a file",
            Operand::Proto => "functions in a function",
            Operand::Captured => "captured values in a function",
            Operand::Pattern => "patterns in a function",
            Operand::Type => "type annotations in a function",
            Operand::Assert => "asserts in a function",
            Operand::None | Operand::Target | Operand::Flags => "instructions in a function",
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
