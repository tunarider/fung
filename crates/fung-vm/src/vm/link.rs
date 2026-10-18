//! Linking: a compiled proto from the front end becomes a runtime proto
//! whose constant pool holds values. The front end knows nothing of values,
//! so it leaves `Const::Builtin(name)` for prelude names and the VM fills
//! them in here.

use std::collections::BTreeMap;
use std::rc::Rc;

use fung_front::bytecode::{Assert, Instr, Param, Proto};
use fung_front::consts::Const;
use fung_front::diag::Arg;
use fung_front::source::Pos;
use fung_front::tree::Capture;

use super::matcher::{RtPattern, link_pattern};
use super::{Stop, Task, internal};
use crate::value::{List, Native, NativeParam, Value, intern};

/// The prelude table: every name a program can use without importing
/// (06), mapped to its value.
#[derive(Clone, Debug, Default)]
pub struct Prelude(pub BTreeMap<String, Value>);

/// A compiled function ready to run: the front end's proto with its
/// constants turned into values and its children linked too.
#[derive(Debug)]
pub struct RtProto {
    /// "" for lambdas and anonymous functions, `<main>` for a file body.
    pub name: String,
    pub code: Vec<Instr>,
    pub pos: Vec<Pos>,
    pub consts: Vec<Value>,
    pub num_locals: usize,
    pub global_names: Vec<String>,
    pub params: Vec<Param>,
    pub protos: Vec<Rc<RtProto>>,
    pub captures: Vec<Capture>,
    pub types: Vec<Vec<String>>,
    pub return_type: i32,
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
        code: p.code.clone(),
        pos: p.pos.clone(),
        consts: p.consts.iter().map(|c| constant(c, prelude)).collect(),
        num_locals: p.num_locals,
        global_names: p.global_names.clone(),
        params: p.params.clone(),
        protos: p.protos.iter().map(|c| link(c, prelude)).collect(),
        captures: p.captures.clone(),
        types: p.types.clone(),
        return_type: p.return_type,
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
        Const::Sym(s) => Value::Sym(intern(s)),
        Const::Dur(d) => Value::Dur(*d),
        Const::Builtin(name) => match prelude.0.get(name) {
            Some(v) => v.clone(),
            None => missing(name),
        },
        // The compiler makes a list constant only for the prelude's `args`
        // (empty at compile time); the run's value is the prelude's.
        Const::List(items) => match prelude.0.get("args") {
            Some(v @ Value::List(_)) if items.is_empty() => v.clone(),
            _ => Value::List(Rc::new(List::new(
                items.iter().map(|c| constant(c, prelude)).collect(),
            ))),
        },
    }
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

fn missing_fn(t: &mut Task<'_>, _: Vec<Value>) -> Result<Value, Stop> {
    let name = t.calling_native().unwrap_or("?");
    Err(internal(vec![Arg::Str(format!(
        "builtin `{name}` is not available"
    ))]))
}
