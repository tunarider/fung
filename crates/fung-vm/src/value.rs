//! Fung values and the primitive operations on them. Values are immutable by
//! convention: every operation returns a new value. Errors carry no position;
//! the VM fills in the position of the failing instruction.

mod args;
mod arith;
mod compare;
mod dur;
mod format;
mod hamt;
pub mod index;
mod items;
mod list;
mod map;
mod omap;
mod pvec;
mod release;
mod sym;
mod text;
mod time;
mod types;

use std::cell::RefCell;
use std::rc::Rc;

pub use crate::native::file::File;
pub use crate::native::net::{Conn, Listener};
use crate::vm::{Atom, Receiver, RtProto, ScopeHandle, Sender, Stop, Task, TaskHandle};

pub use args::Args;
pub use arith::{add, div, floor_div, mul, neg, not, pow, rem, sub};
pub(crate) use arith::{add_text, check_chars, check_returned, floor_div_i64, rem_i64};
pub(crate) use compare::check_element;
pub use compare::{compare, eq, equal, hash_value};
#[cfg(test)]
pub use format::format_for_print;
pub use format::{
    MAX_TEXT_BYTES, format, format_start, format_text, format_within, type_name, write_for_print,
};
pub use items::Items;
pub use list::{Iter as ListIter, List, ListBuilder, SMALL_MAX};
pub use map::{Iter as MapIter, Map, MapBuilder, SMALL_MAP_MAX};
pub(crate) use release::{drop_children, keep, pending, release_piece, set_deferring};
pub use sym::{Sym, intern};
pub use text::Text;

/// A map key of text.
pub fn key(s: &str) -> Rc<Text> {
    Rc::new(Text::from(s))
}
pub use time::Time;
pub(crate) use time::{Loc, MAX_UNIX, MIN_UNIX, civil_from_days, days_from_civil, offset_text};
pub use types::{TypeSet, matches_type};

/// What a persistent collection can be asked of its items. Its nodes
/// remember each answer (`pvec.rs`), so a look after a change costs the
/// changed path only.
#[derive(Clone, Copy)]
pub(crate) enum Probe {
    /// An item holds a mutable value (concurrency v2: what an atom may
    /// not hold, 05/스펙 7.2).
    Mutable,
    /// An item is, or holds at any depth in its lists, maps, tuples and
    /// tags, an `Err` (a discarded value, 02 버려지는 Err, D54).
    Err,
}

/// An item that answers a `Probe`.
pub(crate) trait Probed {
    fn has(&self, p: Probe) -> bool;
}

impl Probed for Value {
    fn has(&self, p: Probe) -> bool {
        match p {
            Probe::Mutable => self.holds_mutable(),
            Probe::Err => self.holds_err(),
        }
    }
}

/// A fung value. Every pointer is a thin `Rc`, so a value is 16 bytes.
#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Nil,
    Str(Rc<Text>),
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
    /// An atom (concurrency v2, `atom`; `vm/atom.rs`).
    Atom(Rc<Atom>),
    /// A scope (concurrency v2, `scope`; `vm/scope.rs`).
    Scope(Rc<ScopeHandle>),
    /// A channel (05, `channel`).
    Sender(Rc<Sender>),
    Receiver(Rc<Receiver>),
    /// An open file handle (`fs.open`); it closes the file when its last
    /// copy drops (`native/file.rs`). An `Arc`: a wait on the operating
    /// system takes a copy of the handle with it (`nat::offload`).
    File(std::sync::Arc<File>),
    /// A TCP connection (stage 4d; only `_native.net` makes one): it
    /// closes when its last copy drops (`native/net.rs`). An `Arc`, as
    /// for `File`: a socket wait takes a copy with it (`nat::offload`).
    Conn(std::sync::Arc<Conn>),
    /// A listening TCP socket (stage 4d, like `Conn`).
    Listener(std::sync::Arc<Listener>),
    /// An instant with its zone (02, `time`).
    Time(Rc<Time>),
}

/// How many containers, and how deep, `holds_deep` looks at by recursion
/// before it takes its stack and set (VA31).
const SMALL_WALK: u32 = 64;
const SMALL_DEPTH: u32 = 16;

impl Value {
    /// The channel behind either end (the test probes); `None` for
    /// anything else.
    #[cfg(test)]
    pub(crate) fn channel_end(&self) -> Option<&Rc<crate::vm::Channel>> {
        match self {
            Value::Sender(e) => Some(&e.0),
            Value::Receiver(e) => Some(&e.0),
            _ => None,
        }
    }

    /// Whether this value is, or holds, one that changes: a channel end, a
    /// task or scope handle, a file or a socket, or a list, map, tuple,
    /// tag or closure (its copied values) that holds one. A module does
    /// not count (it lives for the whole program, 스펙 7.2).
    ///
    /// The big lists and maps answer in O(1) from their nodes' memo
    /// (`pvec.rs`); the small containers are walked with an explicit
    /// stack (a million nested tuples must not overflow), and a part
    /// reached through a shared `Rc` is walked once (`[x, x]` nested
    /// forty deep costs its 41 nodes, not its 2^40 paths).
    pub(crate) fn holds_mutable(&self) -> bool {
        self.holds_deep(Probe::Mutable)
    }

    /// Whether this value is, or holds in its lists, maps, tuples and
    /// tags at any depth, an `Err` (D54: what a discarded place must not
    /// drop), as `holds_mutable` looks (memos, a shared part once).
    pub(crate) fn holds_err(&self) -> bool {
        self.holds_deep(Probe::Err)
    }

    /// Whether `v` itself answers `p`.
    fn hit(&self, p: Probe) -> bool {
        match p {
            Probe::Mutable => self.is_mutable(),
            Probe::Err => matches!(self, Value::Tag(t) if t.name == Sym::ERR),
        }
    }

    /// Whether `p` looks inside `v`: a closure's copied values hold what
    /// it may change, not data it gives back.
    fn opens(&self, p: Probe) -> bool {
        match p {
            Probe::Mutable => self.is_container(),
            Probe::Err => {
                matches!(
                    self,
                    Value::List(_) | Value::Map(_) | Value::Tuple(_) | Value::Tag(_)
                )
            }
        }
    }

    /// `holds_mutable` and `holds_err`: the value or a part of it answers `p`.
    fn holds_deep(&self, p: Probe) -> bool {
        if self.hit(p) {
            return true;
        }
        if !self.opens(p) {
            return false;
        }
        // A small value (a record, a pair) is answered without the stack
        // and the set below: their allocations were most of the cost of a
        // swap or a discarded value (VA31).
        let mut budget = SMALL_WALK;
        if let Some(held) = self.holds_small(p, &mut budget, 0) {
            return held;
        }
        let mut stack: Vec<&Value> = vec![self];
        let mut seen: std::collections::HashSet<*const ()> = std::collections::HashSet::new();
        while let Some(v) = stack.pop() {
            if v.hit(p) {
                return true;
            }
            if !v.opens(p) {
                continue;
            }
            let (ptr, shared): (*const (), bool) = match v {
                Value::List(rc) => (Rc::as_ptr(rc).cast(), Rc::strong_count(rc) > 1),
                Value::Map(rc) => (Rc::as_ptr(rc).cast(), Rc::strong_count(rc) > 1),
                Value::Tuple(rc) => (Rc::as_ptr(rc).cast(), Rc::strong_count(rc) > 1),
                Value::Tag(rc) => (Rc::as_ptr(rc).cast(), Rc::strong_count(rc) > 1),
                Value::Closure(rc) => (Rc::as_ptr(rc).cast(), Rc::strong_count(rc) > 1),
                _ => continue,
            };
            if shared && !seen.insert(ptr) {
                continue;
            }
            match v {
                Value::List(l) => match l.flat() {
                    Some(items) => stack.extend(items),
                    None if l.holds(p) => return true,
                    None => {}
                },
                Value::Map(m) => match m.flat() {
                    Some(entries) => stack.extend(entries.iter().map(|(_, x)| x)),
                    None if m.holds(p) => return true,
                    None => {}
                },
                Value::Tuple(t) => stack.extend(t.iter()),
                Value::Tag(t) => stack.extend(t.args.iter()),
                Value::Closure(c) => stack.extend(c.captured.iter()),
                _ => {}
            }
        }
        false
    }

    /// `holds_deep` on a value of at most `SMALL_WALK` containers and
    /// `SMALL_DEPTH` levels, by recursion with no allocation; `None` when
    /// the value is bigger than that (the caller walks it with its stack
    /// and its set of shared parts). `self` answers no `hit` and opens.
    /// A shared part may be looked at twice here; the budget bounds it.
    fn holds_small(&self, p: Probe, budget: &mut u32, depth: u32) -> Option<bool> {
        let mut look = |x: &Value| -> Option<bool> {
            if x.hit(p) {
                return Some(true);
            }
            if !x.opens(p) {
                return Some(false);
            }
            if *budget == 0 || depth == SMALL_DEPTH {
                return None;
            }
            *budget -= 1;
            x.holds_small(p, budget, depth + 1)
        };
        let mut any = |it: &mut dyn Iterator<Item = &Value>| -> Option<bool> {
            for x in it {
                if look(x)? {
                    return Some(true);
                }
            }
            Some(false)
        };
        match self {
            Value::List(l) => match l.flat() {
                Some(items) => any(&mut items.iter()),
                None => Some(l.holds(p)),
            },
            Value::Map(m) => match m.flat() {
                Some(entries) => any(&mut entries.iter().map(|(_, x)| x)),
                None => Some(m.holds(p)),
            },
            Value::Tuple(t) => any(&mut t.iter()),
            Value::Tag(t) => any(&mut t.args.iter()),
            Value::Closure(c) => any(&mut c.captured.iter()),
            _ => Some(false),
        }
    }

    /// The containers `holds_mutable` looks inside.
    fn is_container(&self) -> bool {
        matches!(
            self,
            Value::List(_) | Value::Map(_) | Value::Tuple(_) | Value::Tag(_) | Value::Closure(_)
        )
    }

    /// The values that change themselves.
    fn is_mutable(&self) -> bool {
        matches!(
            self,
            Value::Atom(_)
                | Value::Sender(_)
                | Value::Receiver(_)
                | Value::Task(_)
                | Value::Scope(_)
                | Value::File(_)
                | Value::Conn(_)
                | Value::Listener(_)
        )
    }

    /// A string value.
    pub fn str(s: &str) -> Value {
        Value::Str(Rc::new(Text::from(s)))
    }

    /// A string value of text already built.
    pub fn string(s: String) -> Value {
        Value::Str(Rc::new(Text::from(s)))
    }

    /// A string value of bytes, UTF-8 or not (L4).
    pub fn bytes(b: Vec<u8>) -> Value {
        Value::Str(Rc::new(Text::new(b)))
    }

    /// A string of one character's bytes (`chars`, `s[i]`). A one-byte
    /// one, an ASCII character or a byte outside UTF-8, is shared from a
    /// table made once per thread: a list of characters holds no string
    /// of its own for each (VA11).
    pub fn char_str(b: &[u8]) -> Value {
        thread_local! {
            static ONE_BYTE: [Rc<Text>; 256] =
                std::array::from_fn(|i| Rc::new(Text::new(vec![i as u8])));
        }
        match b {
            [byte] => ONE_BYTE.with(|t| Value::Str(t[usize::from(*byte)].clone())),
            _ => Value::bytes(b.to_vec()),
        }
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
    /// `proto.code`, one step nearer the run loop (VM35).
    pub code: Rc<[crate::vm::Code]>,
    pub captured: Items,
    /// The module's globals; `None` is a slot not assigned yet.
    ///
    /// A top-level `fn` is a closure in this table that holds the table
    /// again: an `Rc` cycle that is never freed (VM36). With one runtime per
    /// process it goes away with the process; an embedding or a language server that
    /// makes runtimes again and again would leak each module's globals
    /// (open-questions, 알려진 한계).
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
pub struct Tuple(Items);

impl Tuple {
    pub fn new(items: Vec<Value>) -> Tuple {
        Tuple(items.into())
    }

    /// A tuple of values already gathered (VM8).
    pub fn of(items: Items) -> Tuple {
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
    /// Reads as a slice; made from a `Vec` with `into()` (VM8).
    pub args: Items,
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

/// `start..end` or `start..=end`. A range value never has its start
/// after its end (D7, E0522): the empty ranges are `n..n` and `n..=n-1`.
#[derive(Debug)]
pub struct Range {
    pub start: i64,
    pub end: i64,
    pub inclusive: bool,
}

/// Two ranges are equal when they hold the same numbers (D19): `0..5`
/// equals `0..=4`, and every empty range equals every other.
impl PartialEq for Range {
    fn eq(&self, other: &Range) -> bool {
        self.ends() == other.ends()
    }
}

impl Eq for Range {}

impl Range {
    /// Whether `start..end` (`inclusive` for `..=`) is a range value:
    /// its start is not after its end, so the empty ranges are `n..n`
    /// and `n..=n-1` (D7).
    pub fn in_order(start: i64, end: i64, inclusive: bool) -> bool {
        i128::from(start) <= i128::from(end) + i128::from(inclusive)
    }

    /// The first and the last number, `None` for an empty range: what
    /// `==` and the hash compare (D19).
    pub fn ends(&self) -> Option<(i64, i64)> {
        let last = i128::from(self.end) - i128::from(!self.inclusive);
        // Not empty: `last` lies within `start..=end`, so it fits an int.
        (last >= i128::from(self.start)).then_some((self.start, last as i64))
    }

    /// The number of integers in the range, 0 if it is empty. Counted
    /// exactly: a range wider than the int range (up to 2^64 numbers) is
    /// not empty.
    pub fn len(&self) -> i128 {
        let n = i128::from(self.end) - i128::from(self.start) + i128::from(self.inclusive);
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
    /// A symbol by its name (`'read`).
    Sym(&'static str),
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
            Lit::Sym(s) => Value::Sym(intern(s)),
        }
    }
}

/// The Rust function behind a native: it gets one argument per parameter
/// (defaults filled in, a rest parameter as a list), laid out by the same
/// calling convention as fung functions (06).
pub type NativeFn = fn(&mut Task<'_>, Args) -> Result<Value, Stop>;

/// A native that calls back into fung (`map`, `with`, …): instead of
/// calling the function itself it says what to call and how to go on
/// (`vm/cont.rs`), so the task's one run loop makes the call.
pub type StepFn = fn(&mut Task<'_>, Args) -> Result<crate::vm::Flow, Stop>;

/// How a native runs.
#[derive(Clone, Copy, Debug)]
pub enum NativeBody {
    /// Runs to its result.
    Plain(NativeFn),
    /// May call fung functions back through continuations.
    Step(StepFn),
}

/// A function implemented in Rust.
#[derive(Debug)]
pub struct Native {
    pub name: &'static str,
    pub params: Vec<NativeParam>,
    /// Each parameter's name as a symbol: a named argument finds its
    /// parameter by number (VM6).
    pub param_syms: Vec<Sym>,
    pub body: NativeBody,
}

impl Native {
    /// Leaks the native so values can hold a plain `&'static` reference.
    /// Natives live for the whole process, so nothing is lost.
    pub fn leak(name: &'static str, params: Vec<NativeParam>, func: NativeFn) -> &'static Native {
        Native::leak_body(name, params, NativeBody::Plain(func))
    }

    /// [`Native::leak`] for a step native.
    pub fn leak_step(
        name: &'static str,
        params: Vec<NativeParam>,
        step: StepFn,
    ) -> &'static Native {
        Native::leak_body(name, params, NativeBody::Step(step))
    }

    fn leak_body(
        name: &'static str,
        params: Vec<NativeParam>,
        body: NativeBody,
    ) -> &'static Native {
        let param_syms = params.iter().map(|p| sym::intern(p.name)).collect();
        Box::leak(Box::new(Native {
            name,
            params,
            param_syms,
            body,
        }))
    }

    /// The Rust function of a plain native, for tests that call one
    /// directly. A step native has none: it needs the run loop.
    pub fn plain(&self) -> NativeFn {
        match self.body {
            NativeBody::Plain(f) => f,
            NativeBody::Step(_) => panic!("`{}` is a step native", self.name),
        }
    }
}

#[cfg(test)]
mod map_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod testutil;
#[cfg(test)]
mod time_tests;
