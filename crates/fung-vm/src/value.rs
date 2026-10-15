//! Fung values and the primitive operations on them. Values are immutable by
//! convention: every operation returns a new value. Errors carry no position;
//! the VM fills in the position of the failing instruction.

mod arith;
mod compare;
mod dur;
mod format;
mod hamt;
pub mod index;
mod list;
mod map;
mod omap;
mod pvec;
mod release;
mod sym;
mod types;

use std::cell::RefCell;
use std::rc::Rc;

use crate::vm::{Agent, Atom, Caught, Channel, RtProto, Stop, Task, TaskHandle};

pub use arith::{add, div, floor_div, mul, neg, not, pow, rem, sub};
pub use compare::{compare, equal};
pub use format::{format, format_for_print, type_name};
pub use list::{Iter as ListIter, List, ListBuilder, SMALL_MAX};
pub use map::{Iter as MapIter, Map, MapBuilder, SMALL_MAP_MAX};
pub(crate) use release::{drop_children, keep};
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
    Tuple(Rc<Tuple>),
    Tag(Rc<Tag>),
    Range(Rc<Range>),
    Closure(Rc<Closure>),
    Native(&'static Native),
    Module(Rc<Module>),
    /// A task handle (05, `spawn`).
    Task(Rc<TaskHandle>),
    /// A channel (05, `channel`).
    Channel(Rc<Channel>),
    /// An atom (05, `atom`; removed in stage 3b).
    Atom(Rc<Atom>),
    /// An agent's handle (stage 3b, Rust only): it closes the agent's
    /// request channel when its last copy drops (`vm/agent.rs`).
    Agent(Rc<Agent>),
    /// An error `_native.conc.try_call` caught, for `raise` (stage 3b,
    /// Rust only; only `extras/agent.fung` handles one).
    Caught(Rc<Caught>),
}

impl Value {
    /// A string value.
    pub fn str(s: &str) -> Value {
        Value::Str(Rc::new(s.to_string()))
    }

    /// A tuple value.
    pub fn tuple(items: Vec<Value>) -> Value {
        Value::Tuple(Rc::new(Tuple::new(items)))
    }
}

/// A fung function value: a linked proto plus the values it copied when it
/// was made (03, model C) and the globals of the module it was made in (04).
pub struct Closure {
    pub proto: Rc<RtProto>,
    pub captured: Vec<Value>,
    /// The module's globals; `None` is a slot not assigned yet.
    pub globals: Globals,
}

/// A module's global table, shared by its frames and closures.
pub type Globals = Rc<RefCell<Vec<Option<Value>>>>;

impl Closure {
    /// Moves the captured values out for `release`.
    fn spill(&mut self, stack: &mut Vec<Value>) {
        for v in std::mem::take(&mut self.captured) {
            release::keep(stack, v);
        }
    }
}

/// Frees captured values without recursion (`release`), so a long chain of
/// closures that each captured the previous one drops in constant stack.
impl Drop for Closure {
    fn drop(&mut self) {
        release::drop_children(|stack| self.spill(stack));
    }
}

/// Not derived: the globals can hold the closure itself.
impl std::fmt::Debug for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<fn {}>", self.proto.name)
    }
}

/// A module's public names (04: 모듈은 값): an immutable record once the
/// module body has run. Members keep export order. Two modules are equal
/// only when they are the same module.
#[derive(Debug)]
pub struct Module {
    /// What the user wrote or the file stem: `fs`, `rename`.
    pub name: String,
    /// The runtime key: a stdlib name, `_native.x`, or the absolute path.
    pub key: String,
    entries: Vec<(String, Value)>,
}

impl Module {
    pub fn new(name: &str, key: &str) -> Module {
        Module {
            name: name.to_string(),
            key: key.to_string(),
            entries: Vec::new(),
        }
    }

    /// Adds or replaces a member, keeping first-insertion order.
    pub fn set(&mut self, name: &str, v: Value) {
        match self.entries.iter_mut().find(|(k, _)| k == name) {
            Some(e) => e.1 = v,
            None => self.entries.push((name.to_string(), v)),
        }
    }

    /// A member by name. A linear scan: modules have tens of members.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    /// The member names in order.
    pub fn names(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Moves the members out for `release`.
    fn spill(&mut self, stack: &mut Vec<Value>) {
        for (_, v) in std::mem::take(&mut self.entries) {
            release::keep(stack, v);
        }
    }
}

/// Frees the members without recursion (`release`).
impl Drop for Module {
    fn drop(&mut self) {
        release::drop_children(|stack| self.spill(stack));
    }
}

/// A fixed-size group of values (02). It reads as a slice of its items.
#[derive(Debug)]
pub struct Tuple(Vec<Value>);

impl Tuple {
    pub fn new(items: Vec<Value>) -> Tuple {
        Tuple(items)
    }

    /// Moves the items out for `release`, leaving the tuple empty.
    fn spill(&mut self, stack: &mut Vec<Value>) {
        for v in std::mem::take(&mut self.0) {
            release::keep(stack, v);
        }
    }
}

impl std::ops::Deref for Tuple {
    type Target = [Value];

    fn deref(&self) -> &[Value] {
        &self.0
    }
}

/// Frees nested values without recursion (`release`), so a long chain of
/// tuples drops in constant stack.
impl Drop for Tuple {
    fn drop(&mut self) {
        release::drop_children(|stack| self.spill(stack));
    }
}

/// A tagged value such as `Ok(2)`; the type name is the tag name.
#[derive(Debug)]
pub struct Tag {
    pub name: Sym,
    pub args: Vec<Value>,
}

/// Frees nested values without recursion (`release`).
impl Drop for Tag {
    fn drop(&mut self) {
        release::drop_children(|stack| {
            for v in std::mem::take(&mut self.args) {
                release::keep(stack, v);
            }
        });
    }
}

/// `start..end` or `start..=end`.
#[derive(Debug, PartialEq, Eq)]
pub struct Range {
    pub start: i64,
    pub end: i64,
    pub inclusive: bool,
}

impl Range {
    /// The number of integers in the range, 0 if it is empty. Wraps like
    /// Go's int64, so a range wider than i64 counts as empty.
    pub fn len(&self) -> i64 {
        let mut n = self.end.wrapping_sub(self.start);
        if self.inclusive {
            n = n.wrapping_add(1);
        }
        n.max(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contains(&self, i: i64) -> bool {
        if self.inclusive {
            i >= self.start && i <= self.end
        } else {
            i >= self.start && i < self.end
        }
    }
}

/// One parameter of a native function. A missing default means required.
#[derive(Clone, Debug)]
pub struct NativeParam {
    pub name: &'static str,
    pub default: Option<Lit>,
    pub rest: bool,
}

/// A literal default of a native parameter. Natives are shared by every
/// thread, so their defaults cannot hold reference-counted values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lit {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'static str),
    Dur(i64),
}

impl Lit {
    /// The value this literal stands for, made fresh at each call.
    pub fn to_value(&self) -> Value {
        match *self {
            Lit::Nil => Value::Nil,
            Lit::Bool(b) => Value::Bool(b),
            Lit::Int(i) => Value::Int(i),
            Lit::Float(f) => Value::Float(f),
            Lit::Str(s) => Value::str(s),
            Lit::Dur(d) => Value::Dur(d),
        }
    }
}

/// The Rust function behind a native: it gets one argument per parameter
/// (defaults filled in, a rest parameter as a list), laid out by the same
/// calling convention as fung functions (06).
pub type NativeFn = fn(&mut Task<'_>, Vec<Value>) -> Result<Value, Stop>;

/// A function implemented in Rust.
#[derive(Debug)]
pub struct Native {
    pub name: &'static str,
    pub params: Vec<NativeParam>,
    pub func: NativeFn,
}

impl Native {
    /// Leaks the native so values can hold a plain `&'static` reference.
    /// Natives live for the whole process, so nothing is lost.
    pub fn leak(name: &'static str, params: Vec<NativeParam>, func: NativeFn) -> &'static Native {
        Box::leak(Box::new(Native { name, params, func }))
    }
}

#[cfg(test)]
mod map_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod testutil;
