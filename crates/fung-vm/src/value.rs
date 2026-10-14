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

use std::rc::Rc;

pub use arith::{add, div, floor_div, mul, neg, not, pow, rem, sub};
pub use compare::{compare, equal};
pub use format::{format, format_for_print, type_name};
pub use list::{Iter as ListIter, List, ListBuilder, SMALL_MAX};
pub use map::{Iter as MapIter, Map, MapBuilder, SMALL_MAP_MAX};
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

/// Placeholder; filled in when the VM's function values are ported.
#[derive(Debug)]
pub struct Closure {}

/// Placeholder; filled in when modules are ported.
#[derive(Debug)]
pub struct Module {}

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
mod map_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod testutil;
