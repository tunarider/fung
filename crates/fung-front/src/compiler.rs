//! Turns a resolved core tree into bytecode. Port of Go
//! `internal/compiler`. It is a single recursive walk: each core form knows
//! how to emit itself, every emitted instruction carries the position of
//! the node it came from, and a `tail` flag follows the walk so calls in
//! tail position become TAILCALL (03. 꼬리 위치).
//!
//! Recursion follows tree depth, so callers with very deep trees need a
//! large stack (see `parser`).

mod fn_;
mod m4;
mod m6;
mod pattern;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use crate::bytecode::{Op, Proto};
use crate::consts::Const;
use crate::diag::{Arg, Diag, error};
use crate::resolve::Resolved;
use crate::source::Pos;
use crate::tree::{Head, Lit, Node};

/// Configures compilation of one file.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// The file's text, for assert and dbg source spans (08).
    pub source: String,
    /// The file's name as positions print it (`-e`, `t.fung`): the place
    /// `dbg` reports. Go reads it from the position itself.
    pub file: String,
    /// Register test blocks (`fung test` / `check`); false drops them.
    pub tests: bool,
}

struct Compiler<'a> {
    proto: Proto,
    prelude: &'a BTreeSet<String>,
    opts: &'a Options,
    /// A return type annotation disables tail calls (03).
    has_ret: bool,
}

/// Produces the `<main>` proto for a resolved program. Prelude names
/// become `Const::Builtin(name)` (`args` an empty `Const::List`). The main
/// body never uses tail calls: it is the entry frame.
pub fn compile(res: &Resolved, prelude: &BTreeSet<String>, opts: &Options) -> Result<Proto, Diag> {
    let mut c = Compiler {
        proto: Proto {
            name: "<main>".into(),
            num_locals: res.num_locals,
            global_names: res.globals.clone(),
            exports: res.exports.clone(),
            ..Proto::default()
        },
        prelude,
        opts,
        has_ret: true,
    };
    c.block(&res.core, false)?;
    c.proto.emit(Op::Return, 0, res.core.pos);
    Ok(c.proto)
}

/// An E0801 internal error.
fn internal(pos: Pos, msg: String) -> Diag {
    error("E0801", pos, vec![Arg::Str(msg)])
}

fn lit_str(n: &Node) -> &str {
    match &n.lit {
        Lit::Str(s) => s,
        _ => "",
    }
}

fn lit_index(n: &Node) -> usize {
    match n.lit {
        Lit::Int(i) => i as usize,
        _ => 0,
    }
}

/// The constant a prelude name compiles to. Go puts the prelude's values
/// in the pool: functions print as `<fn name>`, and `args` is the program's
/// argument list, which is empty when compiling (`fung dump` passes none).
fn prelude_const(name: &str) -> Const {
    if name == "args" {
        return Const::List(Vec::new());
    }
    Const::Builtin(name.to_string())
}

fn binary_op(op: &str) -> Option<Op> {
    Some(match op {
        "+" => Op::Add,
        "-" => Op::Sub,
        "*" => Op::Mul,
        "/" => Op::Div,
        "//" => Op::FloorDiv,
        "%" => Op::Mod,
        "**" => Op::Pow,
        "==" => Op::Eq,
        "!=" => Op::Ne,
        "<" => Op::Lt,
        ">" => Op::Gt,
        "<=" => Op::Le,
        ">=" => Op::Ge,
        _ => return None,
    })
}

fn unary_op(op: &str) -> Option<Op> {
    Some(match op {
        "-" => Op::Neg,
        "not" => Op::Not,
        "bool" => Op::CheckBool,
        _ => return None,
    })
}

impl Compiler<'_> {
    fn emit_const(&mut self, c: Const, pos: Pos) {
        let i = self.proto.add_const(c);
        self.proto.emit(Op::Const, i, pos);
    }

    /// Compiles `(do e…)` leaving the block's value on the stack: the last
    /// expression's value, or nil when the block ends in a statement (01:
    /// 블록이 문장으로 끝나면 nil). Only the last expression is in tail
    /// position.
    fn block(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        if n.kids.is_empty() {
            self.emit_const(Const::Nil, n.pos);
            return Ok(());
        }
        let last_i = n.kids.len() - 1;
        for (i, k) in n.kids.iter().enumerate() {
            let last = i == last_i;
            let stmt = match k.head {
                Head::Let => Some(self.let_stmt(k)),
                Head::Import => Some(self.import_stmt(k)),
                Head::Assert => Some(self.assert_stmt(k)),
                Head::Test => Some(self.test_stmt(k)),
                _ => None,
            };
            if let Some(r) = stmt {
                r?;
                if last {
                    self.emit_const(Const::Nil, k.pos);
                }
                continue;
            }
            self.expr(k, tail && last)?;
            if !last {
                self.proto.emit(Op::Pop, 0, k.pos);
            }
        }
        Ok(())
    }

    /// Compiles an assignment: the value, then a store into the slot or
    /// global that resolve chose — or a MATCHORFAIL for a destructuring
    /// pattern.
    fn let_stmt(&mut self, n: &Node) -> Result<(), Diag> {
        self.expr(&n.kids[1], false)?;
        let target = &n.kids[0];
        match target.head {
            Head::Ptuple
            | Head::Plist
            | Head::Pmap
            | Head::Ptag
            | Head::Palt
            | Head::Plit
            | Head::Pnil
            | Head::Psym
            | Head::Pwild
            | Head::Pbind => {
                let idx = self.add_pattern(target)?;
                self.proto.emit(Op::MatchOrFail, idx, target.pos);
                Ok(())
            }
            _ => self.store_ref(target),
        }
    }

    /// Emits the store into a resolved `(local n)` or `(global n)`.
    fn store_ref(&mut self, target: &Node) -> Result<(), Diag> {
        let op = match target.head {
            Head::Local => Op::StoreLocal,
            Head::Global => Op::StoreGlobal,
            h => {
                return Err(internal(
                    target.pos,
                    format!("compiler: unresolved assignment target {}", h.as_str()),
                ));
            }
        };
        self.proto.emit(op, lit_index(target), target.pos);
        Ok(())
    }

    /// Compiles `(import key binding (member name binding)…)`: the module
    /// value comes from IMPORT (cached after the first run, 04) and is
    /// stored into the module name and each listed member.
    fn import_stmt(&mut self, n: &Node) -> Result<(), Diag> {
        let key = self.proto.add_const(Const::Str(lit_str(n).to_string()));
        if n.kids[0].head != Head::Nil {
            self.proto.emit(Op::Import, key, n.pos);
            self.store_ref(&n.kids[0])?;
        }
        for m in &n.kids[1..] {
            self.proto.emit(Op::Import, key, m.pos);
            let name = self.proto.add_const(Const::Str(lit_str(m).to_string()));
            self.proto.emit(Op::Field, name, m.pos);
            self.store_ref(&m.kids[0])?;
        }
        Ok(())
    }

    /// Compiles an expression; `tail` says whether its value is what the
    /// enclosing function returns.
    fn expr(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        let pos = n.pos;
        match n.head {
            Head::Do => return self.block(n, tail),
            Head::Int | Head::Float | Head::Bool | Head::Nil => {
                let c = match n.lit {
                    Lit::Int(i) => Const::Int(i),
                    Lit::Float(f) => Const::Float(f),
                    Lit::Bool(b) => Const::Bool(b),
                    _ => Const::Nil,
                };
                self.emit_const(c, pos);
            }
            Head::String => self.emit_const(Const::Str(lit_str(n).to_string()), pos),
            Head::Symbol => self.emit_const(Const::Sym(lit_str(n).to_string()), pos),
            Head::Duration => {
                let ns = if let Lit::Dur(d) = n.lit { d } else { 0 };
                self.emit_const(Const::Dur(ns), pos);
            }
            Head::Local => _ = self.proto.emit(Op::LoadLocal, lit_index(n), pos),
            Head::Global => _ = self.proto.emit(Op::LoadGlobal, lit_index(n), pos),
            Head::Captured => _ = self.proto.emit(Op::LoadCaptured, lit_index(n), pos),
            Head::SelfFn => _ = self.proto.emit(Op::LoadSelf, 0, pos),
            Head::Prelude => {
                let name = lit_str(n);
                if !self.prelude.contains(name) {
                    return Err(internal(
                        pos,
                        format!("compiler: unknown prelude name {name}"),
                    ));
                }
                self.emit_const(prelude_const(name), pos);
            }
            Head::Op => return self.op(n),
            Head::If => return self.if_expr(n, tail),
            Head::Concat => {
                self.kids(n)?;
                self.proto.emit(Op::Concat, n.kids.len(), pos);
            }
            Head::List
            | Head::Map
            | Head::Tuple
            | Head::Tag
            | Head::Range
            | Head::Index
            | Head::Slice
            | Head::Field => return self.collection(n),
            Head::Unwrap => return self.unwrap(n),
            Head::Coalesce => return self.coalesce(n, tail),
            Head::Match => return self.match_expr(n, tail),
            Head::Call => return self.call(n, tail),
            Head::Fn => {
                let idx = self.function(n)?;
                self.proto.emit(Op::Closure, idx, pos);
            }
            Head::Return => return self.return_expr(n),
            // Assignment is a statement (01); it cannot appear in
            // expression position.
            Head::Let => return Err(internal(pos, "compiler: let outside a block".into())),
            h => {
                return Err(internal(
                    pos,
                    format!("compiler: unknown core form {}", h.as_str()),
                ));
            }
        }
        Ok(())
    }

    /// `(op name a [b])`: the operands, then the operator. A missing table
    /// entry must be an error, not a wrong opcode: later milestones add
    /// operators to the parser first.
    fn op(&mut self, n: &Node) -> Result<(), Diag> {
        self.kids(n)?;
        let name = lit_str(n);
        if name == "in" {
            self.proto.emit(Op::In, 0, n.pos);
            return Ok(());
        }
        let op = if n.kids.len() == 2 {
            binary_op(name)
        } else {
            unary_op(name)
        };
        let Some(op) = op else {
            return Err(internal(
                n.pos,
                format!("compiler: unknown operator {name}"),
            ));
        };
        self.proto.emit(op, 0, n.pos);
        Ok(())
    }

    fn if_expr(&mut self, n: &Node, tail: bool) -> Result<(), Diag> {
        self.expr(&n.kids[0], false)?;
        // The jump carries the condition's position so a non-bool
        // condition is reported where the condition is.
        let to_else = self.proto.emit(Op::JumpIfFalse, 0, n.kids[0].pos);
        self.expr(&n.kids[1], tail)?;
        let to_end = self.proto.emit(Op::Jump, 0, n.pos);
        self.proto.patch(to_else, self.proto.code.len());
        self.expr(&n.kids[2], tail)?;
        self.proto.patch(to_end, self.proto.code.len());
        Ok(())
    }

    fn return_expr(&mut self, n: &Node) -> Result<(), Diag> {
        self.expr(&n.kids[0], !self.has_ret)?;
        if self.has_ret && self.proto.return_type >= 0 {
            let rt = self.proto.return_type as usize;
            self.proto.emit(Op::CheckReturn, rt, n.pos);
        }
        self.proto.emit(Op::Return, 0, n.pos);
        Ok(())
    }
}
