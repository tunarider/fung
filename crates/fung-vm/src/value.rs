//! Fung values and the primitive operations on them. Values are immutable by
//! convention: every operation returns a new value. Errors carry no position;
//! the VM fills in the position of the failing instruction.

mod arith;
mod compare;
mod dur;
mod format;
mod sym;
mod types;

use std::rc::Rc;

pub use arith::{add, div, floor_div, mul, neg, not, pow, rem, sub};
pub use compare::{compare, equal};
pub use format::{format, format_for_print, type_name};
pub use sym::{Sym, intern};
pub use types::matches_type;

/// A fung value. Every pointer is a thin `Rc`, so a value is 16 bytes.
#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Nil,
    Str(Rc<String>),
    Sym(Sym),
    /// Nanoseconds.
    Dur(i64),
    List(Rc<List>),
    Map(Rc<Map>),
    Tuple(Rc<Vec<Value>>),
    Tag(Rc<Tag>),
    Range(Rc<Range>),
    Closure(Rc<Closure>),
    Native(&'static Native),
    Module(Rc<Module>),
}

impl Value {
    /// A string value.
    pub fn str(s: &str) -> Value {
        Value::Str(Rc::new(s.to_string()))
    }
}

/// Placeholder; the real list (persistent vector) arrives with a later task.
#[derive(Debug)]
pub struct List {}

/// Placeholder; the real map (ordered HAMT) arrives with a later task.
#[derive(Debug)]
pub struct Map {}

/// Placeholder; filled in when the VM's function values are ported.
#[derive(Debug)]
pub struct Closure {}

/// Placeholder; filled in when modules are ported.
#[derive(Debug)]
pub struct Module {}

/// A tagged value such as `Ok(2)`; the type name is the tag name.
#[derive(Debug)]
pub struct Tag {
    pub name: Sym,
    pub args: Vec<Value>,
}

/// `start..end` or `start..=end`.
#[derive(Debug, PartialEq, Eq)]
pub struct Range {
    pub start: i64,
    pub end: i64,
    pub inclusive: bool,
}

/// One parameter of a native function. A missing default means required.
#[derive(Clone, Debug)]
pub struct NativeParam {
    pub name: &'static str,
    pub default: Option<Value>,
    pub rest: bool,
}

/// A function implemented in Rust. The callable part is added by a later
/// task; for now a native is its name and parameter table.
#[derive(Debug)]
pub struct Native {
    pub name: &'static str,
    pub params: Vec<NativeParam>,
}

impl Native {
    /// Leaks the native so values can hold a plain `&'static` reference.
    /// Natives live for the whole process, so nothing is lost.
    pub fn leak(name: &'static str, params: Vec<NativeParam>) -> &'static Native {
        Box::leak(Box::new(Native { name, params }))
    }
}

#[cfg(test)]
mod tests;
