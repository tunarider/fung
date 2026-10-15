//! The calling convention (09. 호출 규약): binding arguments to slots for
//! fung functions and natives alike (06: 같은 호출 규약), entering a
//! callee, tail calls, returns, and calls from natives back into fung.

use std::rc::Rc;

use fung_front::bytecode::{Param, decode_call};
use fung_front::diag::{Arg, Diag, error};
use fung_front::source::Pos;

use super::frame::Frame;
use super::{RtProto, Stop, Task, at, err, no_pos};
use crate::value::{Closure, List, NativeParam, Sym, Value, type_name};

/// Bounds non-tail recursion (09: 깊이 한도 100,000).
pub const MAX_FRAMES: usize = 100_000;

/// A function's parameters as binding sees them: fung parameters take
/// their defaults from the prologue, natives' are constants filled here.
#[derive(Clone, Copy)]
enum Params<'a> {
    Fung(&'a [Param]),
    Native(&'a [NativeParam]),
}

impl Params<'_> {
    fn len(self) -> usize {
        match self {
            Params::Fung(p) => p.len(),
            Params::Native(p) => p.len(),
        }
    }

    fn name(&self, i: usize) -> &str {
        match self {
            Params::Fung(p) => &p[i].name,
            Params::Native(p) => p[i].name,
        }
    }

    fn rest(self, i: usize) -> bool {
        match self {
            Params::Fung(p) => p[i].rest,
            Params::Native(p) => p[i].rest,
        }
    }

    fn has_default(self, i: usize) -> bool {
        match self {
            Params::Fung(p) => p[i].has_default,
            Params::Native(p) => p[i].default.is_some(),
        }
    }

    /// A native's default value; fung defaults are computed by the prologue.
    fn default(self, i: usize) -> Option<Value> {
        match self {
            Params::Fung(_) => None,
            Params::Native(p) => p[i].default.map(|l| l.to_value()),
        }
    }
}

fn arity(name: &str, fixed: usize, got: usize) -> Diag {
    error(
        "E0402",
        no_pos(),
        vec![
            Arg::Str(name.to_string()),
            Arg::Int(fixed as i64),
            Arg::Int(got as i64),
        ],
    )
}

/// Lays out a call's arguments in a fresh slot array: positionals in
/// order, named by lookup, a trailing rest parameter collecting the
/// surplus as a list (03). A slot left `None` means "use the default" (a
/// fung prologue fills it; a native default is filled here); a required
/// slot left empty is E0407.
fn bind_args(
    name: &str,
    specs: Params<'_>,
    num_slots: usize,
    positional: Vec<Value>,
    named: Vec<(Sym, Value)>,
) -> Result<Vec<Option<Value>>, Diag> {
    let mut fixed = specs.len();
    let mut rest = None;
    if fixed > 0 && specs.rest(fixed - 1) {
        fixed -= 1;
        rest = Some(fixed);
    }
    if positional.len() > fixed && rest.is_none() {
        return Err(arity(name, fixed, positional.len()));
    }
    let mut slots = vec![None; num_slots.max(specs.len())];
    let mut it = positional.into_iter();
    for (i, v) in it.by_ref().take(fixed).enumerate() {
        slots[i] = Some(v);
    }
    if let Some(r) = rest {
        slots[r] = Some(Value::List(Rc::new(List::new(it.collect()))));
    }
    for (sym, v) in named {
        let n = sym.name();
        let Some(idx) = (0..fixed).find(|&i| specs.name(i) == n) else {
            return Err(error(
                "E0406",
                no_pos(),
                vec![Arg::Str(name.to_string()), Arg::Str(n.to_string())],
            ));
        };
        if slots[idx].is_some() {
            return Err(error("E0405", no_pos(), vec![Arg::Str(n.to_string())]));
        }
        slots[idx] = Some(v);
    }
    for (i, slot) in slots.iter_mut().enumerate().take(fixed) {
        if slot.is_some() {
            continue;
        }
        if !specs.has_default(i) {
            return Err(error(
                "E0407",
                no_pos(),
                vec![Arg::Str(specs.name(i).to_string())],
            ));
        }
        *slot = specs.default(i);
    }
    Ok(slots)
}

/// Whether a call fills every parameter of `p` by position and nothing
/// else: the arguments are then the first slots of the frame as they are,
/// with no binding work (09: 호출 규약, fast path).
fn exact_positional(p: &RtProto, npos: usize, nnamed: usize) -> bool {
    let n = p.params.len();
    nnamed == 0 && npos == n && (n == 0 || !p.params[n - 1].rest)
}

impl Task<'_> {
    /// CALL / TAILCALL with the callee and its operands on the stack.
    /// Returns the result when the callee was a native (it ran to the
    /// end), `None` when a fung frame was entered.
    pub(super) fn call_op(
        &mut self,
        arg: usize,
        pos: Pos,
        tail: bool,
    ) -> Result<Option<Value>, Stop> {
        let (npos, nnamed) = decode_call(arg);
        let top = self.stack.len();
        let callee_at = top - npos - 2 * nnamed - 1;
        // Fast path: a fung function called with exactly its positional
        // parameters. The arguments already sit on the stack in order, so
        // they become the new frame's first slots where they are.
        if let Some(Value::Closure(c)) = &self.stack[callee_at]
            && exact_positional(&c.proto, npos, nnamed)
        {
            let Some(Value::Closure(c)) = self.stack[callee_at].take() else {
                unreachable!()
            };
            if tail {
                let base = self.frames.last().expect("a frame").base;
                // Drop the frame's slots, temporaries and the callee slot;
                // the arguments slide down to the frame base.
                self.stack.drain(base..=callee_at);
                self.reuse_frame(c);
                return Ok(None);
            }
            if self.frames.len() >= MAX_FRAMES {
                return Err(err("E0804", pos, vec![Arg::Int(MAX_FRAMES as i64)]));
            }
            self.stack.remove(callee_at);
            self.push_frame(c, callee_at);
            return Ok(None);
        }
        let named_start = top - 2 * nnamed;
        let mut named = Vec::with_capacity(nnamed);
        let mut ops = self.stack.drain(named_start..);
        while let (Some(k), Some(v)) = (ops.next(), ops.next()) {
            let Some(Value::Sym(k)) = k else {
                panic!("named argument without a symbol name")
            };
            named.push((k, v.expect("named argument from an unassigned slot")));
        }
        drop(ops);
        let positional: Vec<Value> = self
            .stack
            .drain(callee_at + 1..)
            .map(|v| v.expect("argument from an unassigned slot"))
            .collect();
        let callee = self.pop();
        self.enter(callee, positional, named, pos, tail)
    }

    /// Starts running `callee`. A native runs at once and its result comes
    /// back; a closure gets a new frame (or reuses the current one when
    /// `tail` is set) and the result is `None`.
    pub(super) fn enter(
        &mut self,
        callee: Value,
        positional: Vec<Value>,
        named: Vec<(Sym, Value)>,
        pos: Pos,
        tail: bool,
    ) -> Result<Option<Value>, Stop> {
        match callee {
            Value::Native(n) => {
                let slots = bind_args(
                    n.name,
                    Params::Native(&n.params),
                    n.params.len(),
                    positional,
                    named,
                )
                .map_err(|d| at(d, pos))?;
                let args = slots
                    .into_iter()
                    .map(|s| s.expect("bound native argument"))
                    .collect();
                let outer = self.native.replace(n);
                let r = (n.func)(self, args);
                self.native = outer;
                match r {
                    Ok(v) => Ok(Some(v)),
                    Err(Stop::Exit(code)) => {
                        // exit(n) ends the whole program (05).
                        self.world.request_exit(code);
                        Err(Stop::Exit(code))
                    }
                    Err(Stop::Error(d)) => Err(at(*d, pos)),
                }
            }
            Value::Closure(c) => {
                let base = if tail {
                    self.frames.last().expect("a frame").base
                } else {
                    self.stack.len()
                };
                if exact_positional(&c.proto, positional.len(), named.len()) {
                    if tail {
                        self.stack.truncate(base);
                    }
                    self.stack.extend(positional.into_iter().map(Some));
                } else {
                    let name = if c.proto.name.is_empty() {
                        "the function"
                    } else {
                        &c.proto.name
                    };
                    // Defaults, a rest parameter, named arguments or an
                    // arity error: lay the slots out the general way.
                    let slots = bind_args(
                        name,
                        Params::Fung(&c.proto.params),
                        c.proto.num_locals,
                        positional,
                        named,
                    )
                    .map_err(|d| at(d, pos))?;
                    if tail {
                        self.stack.truncate(base);
                    }
                    self.stack.extend(slots);
                }
                if tail {
                    self.reuse_frame(c);
                    return Ok(None);
                }
                if self.frames.len() >= MAX_FRAMES {
                    self.stack.truncate(base);
                    return Err(err("E0804", pos, vec![Arg::Int(MAX_FRAMES as i64)]));
                }
                self.push_frame(c, base);
                Ok(None)
            }
            other => Err(err("E0401", pos, vec![Arg::Str(type_name(&other))])),
        }
    }

    /// Pads the slots from `base` up to the callee's frame size.
    fn pad_slots(&mut self, base: usize, num_locals: usize) {
        if self.stack.len() < base + num_locals {
            self.stack.resize(base + num_locals, None);
        }
    }

    /// Pushes a frame for `c` whose arguments start at `base`.
    fn push_frame(&mut self, c: Rc<Closure>, base: usize) {
        self.pad_slots(base, c.proto.num_locals);
        self.frames.push(Frame {
            proto: c.proto.clone(),
            ip: 0,
            base,
            globals: c.globals.clone(),
            closure: Some(c),
            tail_calls: 0,
        });
    }

    /// Makes the current frame run `c` (a tail call); the arguments are
    /// already at the frame base.
    fn reuse_frame(&mut self, c: Rc<Closure>) {
        let base = self.frames.last().expect("a frame").base;
        self.pad_slots(base, c.proto.num_locals);
        let f = self.frames.last_mut().expect("a frame");
        f.proto = c.proto.clone();
        f.globals = c.globals.clone();
        f.closure = Some(c);
        f.ip = 0;
        f.tail_calls += 1;
    }

    /// Pops the current frame, leaving its result on the caller's stack.
    /// Returns the result instead when the frame count drops to `stop`.
    pub(super) fn ret(&mut self, stop: usize) -> Option<Value> {
        let v = self.pop();
        let f = self.frames.pop().expect("a frame");
        self.stack.truncate(f.base);
        if self.frames.len() == stop {
            return Some(v);
        }
        self.push(v);
        None
    }

    /// Calls a fung function or native from Rust (natives such as `fold`
    /// and `map`, exit handlers) and returns when that call returns. On an
    /// error the task is put back as it was, with the call path recorded.
    pub fn call(&mut self, f: &Value, args: Vec<Value>) -> Result<Value, Stop> {
        let depth = self.frames.len();
        let height = self.stack.len();
        let r = self.guarded(
            |t| match t.enter(f.clone(), args, Vec::new(), no_pos(), false)? {
                Some(v) => Ok(v),
                None => t.run(depth),
            },
        );
        if r.is_err() {
            self.frames.truncate(depth);
            self.stack.truncate(height);
        }
        r
    }

    /// `call` with the arguments taken from `args`, which is left empty
    /// with its capacity kept: natives that call a function per element
    /// (`map`, `fold`, …) reuse one buffer. Only a fung function called
    /// with exactly its parameters is allocation-free: the arguments move
    /// straight onto the stack. A native gets a fresh argument vector
    /// (but skips binding); anything else goes through `call`.
    // `drain(..).collect()` keeps the buffer's capacity for the next call
    // (`mem::take` would hand the allocation to the callee).
    #[allow(clippy::drain_collect)]
    pub fn call_buf(&mut self, f: &Value, args: &mut Vec<Value>) -> Result<Value, Stop> {
        if let Value::Native(n) = f
            && n.params.len() == args.len()
            && n.params.last().is_none_or(|p| !p.rest)
        {
            // Every parameter given by position: nothing to bind. A panic
            // inside reaches the fence of the run that called the native.
            let outer = self.native.replace(n);
            let r = (n.func)(self, args.drain(..).collect());
            self.native = outer;
            if let Err(Stop::Exit(code)) = r {
                self.world.request_exit(code);
            }
            return r;
        }
        let Value::Closure(c) = f else {
            return self.call(f, args.drain(..).collect());
        };
        if !exact_positional(&c.proto, args.len(), 0) || self.frames.len() >= MAX_FRAMES {
            return self.call(f, args.drain(..).collect());
        }
        let depth = self.frames.len();
        let height = self.stack.len();
        self.stack.extend(args.drain(..).map(Some));
        self.push_frame(c.clone(), height);
        let r = self.guarded(|t| t.run(depth));
        if r.is_err() {
            self.frames.truncate(depth);
            self.stack.truncate(height);
        }
        r
    }
}
