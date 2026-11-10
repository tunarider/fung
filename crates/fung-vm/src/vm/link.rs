//! Linking: a compiled proto from the front end becomes a runtime proto
//! whose constant pool holds values. The front end knows nothing of values,
//! so it leaves `Const::Builtin(name)` for prelude names and the VM fills
//! them in here.

use std::collections::BTreeMap;
use std::rc::Rc;

use fung_front::bytecode::{Assert, Op, Param, Proto};
use fung_front::consts::Const;
use fung_front::diag::Arg;
use fung_front::source::Pos;
use fung_front::tree::Capture;

use super::matcher::{RtPattern, link_pattern};
use super::{Stop, Task, internal};
use crate::value::{
    Args, List, MapBuilder, Native, NativeParam, Range, Sym, Tag, TypeSet, Value, intern,
};

/// The prelude table: every name a program can use without importing
/// (06), mapped to its value.
#[derive(Clone, Debug, Default)]
pub struct Prelude(pub BTreeMap<String, Value>);

/// An instruction decoded once, at link time: the run loop reads its
/// opcode and operand as fields (VM16).
#[derive(Clone, Copy, Debug)]
pub struct Code {
    pub op: Op,
    pub arg: u32,
}

/// A compiled function ready to run: the front end's proto with its
/// constants turned into values and its children linked too.
#[derive(Debug)]
pub struct RtProto {
    /// "" for lambdas and anonymous functions, `<main>` for a file body.
    pub name: String,
    pub code: Vec<Code>,
    pub pos: Vec<Pos>,
    pub consts: Vec<Value>,
    pub num_locals: usize,
    pub global_names: Vec<String>,
    pub params: Vec<Param>,
    /// The parameters' names as symbols: a named argument finds its
    /// parameter by number (VM6).
    pub param_syms: Vec<Sym>,
    pub protos: Vec<Rc<RtProto>>,
    pub captures: Vec<Capture>,
    pub types: Vec<TypeSet>,
    pub patterns: Vec<RtPattern>,
    pub exports: Vec<String>,
    pub asserts: Vec<Assert>,
    /// Compiled from an embedded extras file: errors point at the caller (06).
    pub builtin: bool,
}

/// Links a proto and its children against the prelude.
pub fn link(p: &Proto, prelude: &Prelude) -> Rc<RtProto> {
    Rc::new(RtProto {
        name: p.name.clone(),
        code: p
            .code
            .iter()
            .map(|i| Code {
                op: i.op(),
                arg: i.arg() as u32,
            })
            .collect(),
        pos: p.pos.clone(),
        consts: p.consts.iter().map(|c| constant(c, prelude)).collect(),
        num_locals: p.num_locals,
        global_names: p.global_names.clone(),
        params: p.params.clone(),
        param_syms: p.params.iter().map(|q| intern(&q.name)).collect(),
        protos: p.protos.iter().map(|c| link(c, prelude)).collect(),
        captures: p.captures.clone(),
        types: p.types.iter().map(|a| TypeSet::new(a)).collect(),
        patterns: p
            .patterns
            .iter()
            .map(|q| link_pattern(q, prelude))
            .collect(),
        exports: p.exports.clone(),
        asserts: p.asserts.clone(),
        builtin: p.builtin,
    })
}

pub(super) fn constant(c: &Const, prelude: &Prelude) -> Value {
    match c {
        Const::Int(n) => Value::Int(*n),
        Const::Float(f) => Value::Float(*f),
        Const::Bool(b) => Value::Bool(*b),
        Const::Nil => Value::Nil,
        Const::Str(s) => Value::str(s),
        Const::Bytes(b) => Value::bytes(b.clone()),
        Const::Sym(s) => Value::Sym(intern(s)),
        Const::Dur(d) => Value::Dur(*d),
        Const::Builtin(name) => match prelude.0.get(name) {
            Some(v) => v.clone(),
            None => missing(name),
        },
        // the dbg a direct `dbg(e)` calls, with its text and place (SP11)
        Const::Dbg => Value::Native(crate::prelude::dbg_at()),
        // the run's arguments, which the prelude holds
        Const::Args => match prelude.0.get("args") {
            Some(v) => v.clone(),
            None => Value::List(Rc::new(List::new(Vec::new()))),
        },
        Const::List(items) => Value::List(Rc::new(List::new(all(items, prelude)))),
        Const::Tuple(items) => Value::tuple(all(items, prelude)),
        Const::Tag(name, args) => Value::Tag(Rc::new(Tag {
            name: intern(name),
            args: args.iter().map(|c| constant(c, prelude)).collect(),
        })),
        Const::Range(start, end, inclusive) => Value::Range(Rc::new(Range {
            start: *start,
            end: *end,
            inclusive: *inclusive,
        })),
        Const::Map(entries) => {
            let mut m = MapBuilder::default();
            for (k, v) in entries {
                let Value::Str(k) = constant(k, prelude) else {
                    panic!("a map constant with a key that is not a string");
                };
                m.set(k, constant(v, prelude));
            }
            Value::Map(Rc::new(m.finish()))
        }
    }
}

fn all(items: &[Const], prelude: &Prelude) -> Vec<Value> {
    items.iter().map(|c| constant(c, prelude)).collect()
}

/// A prelude name the runtime has no value for (a declared native with no
/// implementation): calling it is an internal error rather than a crash.
fn missing(name: &str) -> Value {
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let any = NativeParam {
        name: "args",
        default: None,
        rest: true,
    };
    Value::Native(Native::leak(name, vec![any], missing_fn))
}

fn missing_fn(t: &mut Task<'_>, _: Args) -> Result<Value, Stop> {
    let name = t.calling_native().unwrap_or("?");
    Err(internal(vec![Arg::Str(format!(
        "builtin `{name}` is not available"
    ))]))
}

#[cfg(test)]
mod tests {
    use fung_front::bytecode::{Instr, Op};

    /// A decoded instruction is as small as the word it came from, plus
    /// the opcode's byte rounded up: 8 bytes (VM16).
    #[test]
    fn a_decoded_instruction_is_8_bytes() {
        assert_eq!(std::mem::size_of::<super::Code>(), 8);
        let i = Instr::new(Op::Call, 4097);
        let c = super::Code {
            op: i.op(),
            arg: i.arg() as u32,
        };
        assert_eq!((c.op, c.arg), (Op::Call, 4097));
    }
}
