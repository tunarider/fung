//! The calling convention (09. 호출 규약): binding arguments to slots for
//! fung functions and natives alike (06: 같은 호출 규약), entering a
//! callee, tail calls, returns, and calls from natives back into fung.

use std::rc::Rc;

use fung_front::bytecode::{Param, decode_call};
use fung_front::diag::{Arg, Diag, error};
use fung_front::source::Pos;

use super::cont::{Cont, ContFrame, Flow, Next, Wake};
use super::frame::{Frame, TailFrom};
use super::{RtProto, Stop, Task, at, err, no_pos};
use crate::value::{Closure, List, Native, NativeBody, NativeParam, Sym, Value, type_name};

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

/// Whether a call fills every parameter of `p` by position and nothing
/// else: the arguments are then the first slots of the frame as they are,
/// with no binding work (09: 호출 규약, fast path).
fn exact_positional(p: &RtProto, npos: usize, nnamed: usize) -> bool {
    let n = p.params.len();
    nnamed == 0 && npos == n && (n == 0 || !p.params[n - 1].rest)
}

/// Whether a call fills every parameter of native `nat` by position and
/// nothing else: its arguments are then its arguments as they lie (VM5).
fn exact_native(nat: &Native, npos: usize, nnamed: usize) -> bool {
    nnamed == 0 && nat.params.len() == npos && !nat.params.last().is_some_and(|p| p.rest)
}

/// What a call binds its arguments for.
#[derive(Clone, Copy)]
enum Callee<'a> {
    Fung(&'a RtProto),
    Native(&'static Native),
}

impl<'a> Callee<'a> {
    fn params(self) -> Params<'a> {
        match self {
            Callee::Fung(p) => Params::Fung(&p.params),
            Callee::Native(n) => Params::Native(&n.params),
        }
    }

    /// The name errors give the callee.
    fn name(self) -> &'a str {
        match self {
            Callee::Fung(p) if p.name.is_empty() => "the function",
            Callee::Fung(p) => &p.name,
            Callee::Native(n) => n.name,
        }
    }

    fn num_locals(self) -> usize {
        match self {
            Callee::Fung(p) => p.num_locals,
            Callee::Native(n) => n.params.len(),
        }
    }

    /// The slot of the fixed parameter named `k`, if any.
    fn slot_of(self, k: Sym, fixed: usize) -> Option<usize> {
        match self {
            Callee::Fung(p) => p.param_syms[..fixed].iter().position(|&s| s == k),
            Callee::Native(n) => n.param_syms[..fixed].iter().position(|&s| s == k),
        }
    }
}

/// The stand-in for a continuation's box while it runs (`deliver`).
struct Hole;

impl Cont for Hole {
    fn resume(&mut self, _: &mut Task<'_>, _: Value) -> Result<Next, Stop> {
        unreachable!("a continuation resumed while it runs")
    }
}

/// What giving a value to the waiting continuations led to.
enum Delivered {
    /// A continuation asks for this call.
    Call(Value, usize),
    /// The value went on the caller's stack.
    Pushed,
    /// The run loop's value.
    Finished(Value),
    /// A continuation waits to be woken (`Next::Park`).
    Park(Wake),
}

/// Where the run loop is after a step: going on, done with this value,
/// parked until the task is woken (`cont.rs`), or giving its turn: then
/// the call a continuation asked for, not made yet, waits here with its
/// arguments on the stack.
pub(super) enum Ran {
    Going,
    Done(Value),
    Parked(Wake),
    Yielded(Option<(Value, usize)>),
}

/// What starting a call did.
pub(super) enum Entered {
    /// A fung frame was entered, or a step native is waiting for a call
    /// it made: the run loop goes on.
    Frame,
    /// A native ran to the end with this result.
    Value(Value),
    /// A step native put its continuation on the list and asks for this
    /// call (`f`, its `n` arguments on the stack). The caller makes it
    /// with `issue`, so a step native called back per element never
    /// nests one `issue` in another.
    Call(Value, usize),
    /// A step native put its continuation on the list and waits to be
    /// woken (`Flow::Park`).
    Park(Wake),
}

impl Task<'_> {
    /// CALL / TAILCALL with the callee and its operands on the stack.
    pub(super) fn call_op(&mut self, arg: usize, pos: Pos, tail: bool) -> Result<Entered, Stop> {
        let (npos, nnamed) = decode_call(arg);
        let top = self.fiber.stack.len();
        let callee_at = top - npos - 2 * nnamed - 1;
        // Fast path: a fung function called with exactly its positional
        // parameters. The arguments already sit on the stack in order, so
        // they become the new frame's first slots where they are.
        if let Some(Value::Closure(c)) = &self.fiber.stack[callee_at]
            && exact_positional(&c.proto, npos, nnamed)
        {
            let Some(Value::Closure(c)) = self.fiber.stack[callee_at].take() else {
                unreachable!()
            };
            if tail {
                let base = self.fiber.frames.last().expect("a frame").base;
                // Drop the frame's slots, temporaries and the callee slot;
                // the arguments slide down to the frame base.
                self.fiber.stack.drain(base..=callee_at);
                self.reuse_frame(c);
                return Ok(Entered::Frame);
            }
            if self.fiber.frames.len() >= MAX_FRAMES {
                return Err(err("E0804", pos, vec![Arg::Int(MAX_FRAMES as i64)]));
            }
            self.fiber.stack.remove(callee_at);
            self.push_frame(c, callee_at);
            return Ok(Entered::Frame);
        }
        // A fung function with defaults, named arguments or a rest
        // parameter: the arguments are bound where they lie (VM6).
        if let Some(Value::Closure(_)) = &self.fiber.stack[callee_at] {
            let Some(Value::Closure(c)) = self.fiber.stack[callee_at].take() else {
                unreachable!()
            };
            self.bind_in_place(Callee::Fung(&c.proto), callee_at + 1, npos, nnamed)
                .map_err(|d| at(d, pos))?;
            if tail {
                let base = self.fiber.frames.last().expect("a frame").base;
                self.fiber.stack.drain(base..=callee_at);
                self.reuse_frame(c);
                return Ok(Entered::Frame);
            }
            if self.fiber.frames.len() >= MAX_FRAMES {
                return Err(err("E0804", pos, vec![Arg::Int(MAX_FRAMES as i64)]));
            }
            self.fiber.stack.remove(callee_at);
            self.push_frame(c, callee_at);
            return Ok(Entered::Frame);
        }
        // A native: it runs on its arguments, bound where they lie when
        // the call is not exactly its positional parameters (VM5, VM6).
        if let Some(Value::Native(nat)) = &self.fiber.stack[callee_at] {
            let nat = *nat;
            if !exact_native(nat, npos, nnamed) {
                self.bind_in_place(Callee::Native(nat), callee_at + 1, npos, nnamed)
                    .map_err(|d| at(d, pos))?;
            }
            let args = self.take_args(callee_at + 1);
            self.fiber.stack.pop();
            return self.run_native(nat, args, pos, tail);
        }
        let callee = self.fiber.stack[callee_at].take().expect("callee");
        Err(err("E0401", pos, vec![Arg::Str(type_name(&callee))]))
    }

    /// Binds a call where its arguments lie (VM6, 09: 호출 규약), for fung
    /// functions and natives alike: from `at` the stack holds the `npos`
    /// positional arguments, then each of the `nnamed` named ones as its
    /// symbol and value. Afterwards the slots start at `at`: positionals
    /// in order, a trailing rest parameter collecting the surplus as a
    /// list, each named value in its parameter's slot, and for a default
    /// an empty slot the prologue fills (03), or a native's constant.
    /// Everything is checked before the stack changes.
    fn bind_in_place(
        &mut self,
        callee: Callee<'_>,
        at: usize,
        npos: usize,
        nnamed: usize,
    ) -> Result<(), Diag> {
        let p = callee.params();
        let n = p.len();
        let rest = n > 0 && p.rest(n - 1);
        let fixed = if rest { n - 1 } else { n };
        let name = callee.name();
        if npos > fixed && !rest {
            return Err(arity(name, fixed, npos));
        }
        let named_at = at + npos;
        let sym_at = |s: &[Option<Value>], j: usize| match &s[named_at + 2 * j] {
            Some(Value::Sym(k)) => *k,
            _ => panic!("named argument without a symbol name"),
        };
        let stack = &self.fiber.stack;
        let slot_of = |k: Sym| callee.slot_of(k, fixed);
        for j in 0..nnamed {
            let k = sym_at(stack, j);
            let Some(i) = slot_of(k) else {
                return Err(error(
                    "E0406",
                    no_pos(),
                    vec![Arg::Str(name.to_string()), Arg::Str(k.name().to_string())],
                ));
            };
            if i < npos || (0..j).any(|e| sym_at(stack, e) == k) {
                return Err(error(
                    "E0405",
                    no_pos(),
                    vec![Arg::Str(k.name().to_string())],
                ));
            }
        }
        for i in npos.min(fixed)..fixed {
            let named = (0..nnamed).any(|j| slot_of(sym_at(stack, j)) == Some(i));
            if !named && !p.has_default(i) {
                return Err(error(
                    "E0407",
                    no_pos(),
                    vec![Arg::Str(p.name(i).to_string())],
                ));
            }
        }
        // The named values come off the top into the task's buffer, which
        // keeps its room from call to call.
        let mut named = std::mem::take(&mut self.fiber.named);
        let mut ops = self.fiber.stack.drain(named_at..);
        while let (Some(k), Some(v)) = (ops.next(), ops.next()) {
            let Some(Value::Sym(k)) = k else {
                unreachable!("checked above")
            };
            named.push((k, v.expect("named argument from an unassigned slot")));
        }
        drop(ops);
        if rest {
            let surplus: Vec<Value> = if npos > fixed {
                self.fiber
                    .stack
                    .drain(at + fixed..)
                    .map(|v| v.expect("argument from an unassigned slot"))
                    .collect()
            } else {
                Vec::new()
            };
            self.fiber.stack.resize(at + fixed, None);
            self.fiber
                .stack
                .push(Some(Value::List(Rc::new(List::new(surplus)))));
        }
        self.pad_slots(at, callee.num_locals().max(n));
        for (k, v) in named.drain(..) {
            let i = slot_of(k).expect("checked above");
            self.fiber.stack[at + i] = Some(v);
        }
        self.fiber.named = named;
        if let Callee::Native(_) = callee {
            for i in npos.min(fixed)..fixed {
                let slot = &mut self.fiber.stack[at + i];
                if slot.is_none() {
                    *slot = p.default(i);
                }
            }
        }
        Ok(())
    }

    /// Runs native `nat` on its bound arguments. A plain native runs at
    /// once and its result comes back; a step native either finishes at
    /// once or puts its continuation on the list and makes its first call
    /// or waits.
    fn run_native(
        &mut self,
        nat: &'static Native,
        args: Vec<Value>,
        pos: Pos,
        tail: bool,
    ) -> Result<Entered, Stop> {
        self.check_effect(nat).map_err(|e| match e {
            Stop::Error(d) => at(*d, pos),
            other => other,
        })?;
        let outer = self.fiber.native.replace(nat);
        match nat.body {
            NativeBody::Plain(func) => {
                let r = func(self, args).and_then(returned);
                self.fiber.native = outer;
                match r {
                    Ok(v) => Ok(Entered::Value(v)),
                    Err(e) => Err(self.raised(e, pos)),
                }
            }
            NativeBody::Step(step) => {
                let r = step(self, args);
                self.fiber.native = outer;
                match r {
                    Ok(Flow::Done(v)) => match returned(v) {
                        Ok(v) => Ok(Entered::Value(v)),
                        Err(e) => Err(self.raised(e, pos)),
                    },
                    Ok(Flow::Call(k, f, nargs)) => {
                        self.fiber.conts.push(ContFrame {
                            depth: self.fiber.frames.len(),
                            height: self.fiber.stack.len() - nargs,
                            pos,
                            native: Some(nat),
                            tail,
                            k,
                        });
                        Ok(Entered::Call(f, nargs))
                    }
                    Ok(Flow::Park(k, w)) => {
                        self.fiber.conts.push(ContFrame {
                            depth: self.fiber.frames.len(),
                            height: self.fiber.stack.len(),
                            pos,
                            native: Some(nat),
                            tail,
                            k,
                        });
                        Ok(Entered::Park(w))
                    }
                    Err(e) => Err(self.raised(e, pos)),
                }
            }
        }
    }

    /// Pads the slots from `base` up to the callee's frame size.
    fn pad_slots(&mut self, base: usize, num_locals: usize) {
        if self.fiber.stack.len() < base + num_locals {
            self.fiber.stack.resize(base + num_locals, None);
        }
    }

    /// Pushes a frame for `c` whose arguments start at `base`.
    fn push_frame(&mut self, c: Rc<Closure>, base: usize) {
        self.pad_slots(base, c.proto.num_locals);
        let entry = self.entry_for(&c.proto, self.fiber.frames.last());
        self.fiber.frames.push(Frame::new(c, base, entry));
    }

    /// The built-in function a frame for `proto` runs inside of, called
    /// from `caller`: none in user code; `proto` itself when user code
    /// (or nothing) calls it; else the caller's.
    fn entry_for(&self, proto: &Rc<RtProto>, caller: Option<&Frame>) -> Option<Rc<RtProto>> {
        if !proto.builtin {
            return None;
        }
        match caller {
            Some(f) if f.proto().builtin => f.entry.clone(),
            _ => Some(proto.clone()),
        }
    }

    /// Makes the current frame run `c` (a tail call); the arguments are
    /// already at the frame base.
    fn reuse_frame(&mut self, c: Rc<Closure>) {
        let base = self.fiber.frames.last().expect("a frame").base;
        self.pad_slots(base, c.proto.num_locals);
        // A function calling itself (a loop): the frame already runs this
        // closure, with its proto, globals and entry (VM9).
        let f = self.fiber.frames.last_mut().expect("a frame");
        if Rc::ptr_eq(&f.closure, &c) {
            f.tail_calls += 1;
            f.ip = 0;
            return;
        }
        let n = self.fiber.frames.len();
        // A tail call replaces the frame, not its caller: the entry is
        // the reused frame's own if it runs inside a built-in function,
        // the new one's if user code tail-calls into one.
        let entry = if self.fiber.frames[n - 1].proto().builtin && c.proto.builtin {
            self.fiber.frames[n - 1].entry.clone()
        } else {
            let caller = n.checked_sub(2).map(|i| &self.fiber.frames[i]);
            self.entry_for(&c.proto, caller)
        };
        let f = self.fiber.frames.last_mut().expect("a frame");
        f.entry = entry;
        // Across the boundary between user code and a built-in function
        // the count starts again (L8): user code going in leaves its frame
        // behind as `tail_from`; a callback coming out does not inherit
        // the library's tail calls.
        if f.proto().builtin == c.proto.builtin {
            f.tail_calls += 1;
        } else {
            if !f.proto().builtin {
                f.tail_from = Some(Box::new(TailFrom {
                    proto: f.proto().clone(),
                    pos: f.current_pos(),
                    tail_calls: f.tail_calls,
                }));
            }
            f.tail_calls = 0;
        }
        f.closure = c;
        f.ip = 0;
    }

    /// Pops the current frame (a RETURN, or a TAILCALL whose native
    /// finished) and gives `v` to whoever waits for it: a continuation at
    /// that depth or the caller's stack. The value of the run loop when
    /// the last frame returns.
    #[inline(always)]
    pub(super) fn ret_with(&mut self, v: Value) -> Result<Ran, Stop> {
        let f = self.fiber.frames.pop().expect("a frame");
        self.fiber.stack.truncate(f.base);
        self.finish(v)
    }

    /// A call made at the current frame depth returned `v`: gives it to
    /// the continuations due here and makes the next call one asks for.
    #[inline(always)]
    pub(super) fn finish(&mut self, v: Value) -> Result<Ran, Stop> {
        match self.deliver(v)? {
            Delivered::Call(f, n) => self.issue(f, n),
            Delivered::Pushed => Ok(Ran::Going),
            Delivered::Finished(v) => Ok(Ran::Done(v)),
            Delivered::Park(w) => Ok(Ran::Parked(w)),
        }
    }

    /// Resumes the continuations due at the current frame depth,
    /// innermost first, until one asks for another call. Then, or when
    /// none is due, `v` goes on the caller's stack, or is the run loop's
    /// value once no frame is left.
    #[inline(always)]
    fn deliver(&mut self, mut v: Value) -> Result<Delivered, Stop> {
        loop {
            if !self.fiber.conts.is_empty()
                && self.fiber.conts.last().expect("a continuation").depth == self.fiber.frames.len()
            {
                // The box comes out for the call (a zero-sized stand-in
                // takes its place, no allocation) and goes back if the
                // native asks for another call: the frame stays put.
                let i = self.fiber.conts.len() - 1;
                let mut k = std::mem::replace(&mut self.fiber.conts[i].k, Box::new(Hole));
                let outer = std::mem::replace(&mut self.fiber.native, self.fiber.conts[i].native);
                let r = k.resume(self, v).and_then(|n| match n {
                    Next::Done(w) => returned(w).map(Next::Done),
                    n => Ok(n),
                });
                self.fiber.native = outer;
                match r {
                    Ok(Next::Call(f, n)) => {
                        self.fiber.conts[i].k = k;
                        return Ok(Delivered::Call(f, n));
                    }
                    Ok(Next::Park(w)) => {
                        self.fiber.conts[i].k = k;
                        return Ok(Delivered::Park(w));
                    }
                    Ok(Next::Done(w)) => {
                        let c = self.fiber.conts.pop().expect("a continuation");
                        v = self.done(&c, w);
                        if c.tail {
                            let f = self.fiber.frames.pop().expect("a frame");
                            self.fiber.stack.truncate(f.base);
                        }
                    }
                    Err(e) => {
                        let c = self.fiber.conts.pop().expect("a continuation");
                        return Err(self.raised(e, c.pos));
                    }
                }
                continue;
            }
            if self.fiber.frames.is_empty() {
                return Ok(Delivered::Finished(v));
            }
            self.push(v);
            return Ok(Delivered::Pushed);
        }
    }

    /// A continuation's result: the stack back to where its arguments
    /// started.
    fn done(&mut self, c: &ContFrame, v: Value) -> Value {
        self.fiber.stack.truncate(c.height);
        v
    }

    /// `stop` unwinds into the innermost continuation of this run loop
    /// (`drive` checked there is one): the frames and stack go back to
    /// where its native was called, and the native decides.
    pub(super) fn unwind(&mut self, stop: Stop) -> Result<Ran, Stop> {
        let mut c = self.fiber.conts.pop().expect("a continuation");
        self.fiber.frames.truncate(c.depth);
        self.fiber.stack.truncate(c.height);
        let outer = std::mem::replace(&mut self.fiber.native, c.native);
        let r = c.k.unwind(self, stop).and_then(|n| match n {
            Next::Done(w) => returned(w).map(Next::Done),
            n => Ok(n),
        });
        self.fiber.native = outer;
        match r {
            Ok(Next::Call(f, n)) => {
                self.fiber.conts.push(c);
                self.issue(f, n)
            }
            Ok(Next::Park(w)) => {
                self.fiber.conts.push(c);
                Ok(Ran::Parked(w))
            }
            Ok(Next::Done(w)) => {
                let w = self.done(&c, w);
                if c.tail {
                    let f = self.fiber.frames.pop().expect("a frame");
                    self.fiber.stack.truncate(f.base);
                }
                self.finish(w)
            }
            Err(e) => Err(self.raised(e, c.pos)),
        }
    }

    /// Calls `f` with the `n` arguments on top of the stack for the
    /// innermost continuation, which receives the result (09: 호출 규약).
    /// A fung function given exactly its parameters gets a frame on the
    /// arguments where they lie, so a native calling back per element
    /// allocates nothing; a plain native given exactly its parameters runs
    /// on them without binding.
    pub(super) fn issue(&mut self, mut f: Value, mut n: usize) -> Result<Ran, Stop> {
        // A call that ends at once (a native) is delivered here and the
        // next call made in this loop: a native called back per element
        // must not deepen the Rust stack per element.
        loop {
            if self.fiber.turn_due {
                return Ok(Ran::Yielded(Some((f, n))));
            }
            let v = match self.start_call(f, n)? {
                Entered::Frame => return Ok(Ran::Going),
                Entered::Value(v) => v,
                Entered::Call(g, m) => {
                    (f, n) = (g, m);
                    continue;
                }
                Entered::Park(w) => return Ok(Ran::Parked(w)),
            };
            match self.deliver(v)? {
                Delivered::Call(g, m) => (f, n) = (g, m),
                Delivered::Pushed => return Ok(Ran::Going),
                Delivered::Finished(v) => return Ok(Ran::Done(v)),
                Delivered::Park(w) => return Ok(Ran::Parked(w)),
            }
        }
    }

    /// Starts the call `issue` makes.
    #[inline(always)]
    fn start_call(&mut self, f: Value, n: usize) -> Result<Entered, Stop> {
        if let Some(r) = self.call_plain(&f, n) {
            return r.map(Entered::Value);
        }
        self.tick(no_pos())?;
        let at = self.fiber.stack.len() - n;
        match &f {
            Value::Closure(c) if self.fiber.frames.len() < MAX_FRAMES => {
                // Defaults or a rest parameter: bound where they lie (VM6).
                if !exact_positional(&c.proto, n, 0) {
                    self.bind_in_place(Callee::Fung(&c.proto), at, n, 0)
                        .map_err(|d| super::at(d, no_pos()))?;
                }
                self.push_frame(c.clone(), at);
                Ok(Entered::Frame)
            }
            Value::Closure(_) => {
                self.fiber.stack.truncate(at);
                Err(err("E0804", no_pos(), vec![Arg::Int(MAX_FRAMES as i64)]))
            }
            Value::Native(nat) => {
                let nat = *nat;
                if !exact_native(nat, n, 0) {
                    self.bind_in_place(Callee::Native(nat), at, n, 0)
                        .map_err(|d| super::at(d, no_pos()))?;
                }
                let args = self.take_args(at);
                self.run_native(nat, args, no_pos(), false)
            }
            other => Err(err("E0401", no_pos(), vec![Arg::Str(type_name(other))])),
        }
    }

    /// Calls `f` with the `n` arguments on top of the stack if it is a
    /// plain native given exactly its parameters: it runs to its result
    /// without binding and without the run loop, so a native walking
    /// elements may call it in place (`native/walk.rs`). `None` for
    /// anything else, with the arguments left where they are.
    #[inline(always)]
    pub(crate) fn call_plain(&mut self, f: &Value, n: usize) -> Option<Result<Value, Stop>> {
        // A turn that is over goes through `issue`, a yield point.
        if self.fiber.turn_due {
            return None;
        }
        let Value::Native(nat) = f else {
            return None;
        };
        let NativeBody::Plain(func) = nat.body else {
            return None;
        };
        if !exact_native(nat, n, 0) {
            return None;
        }
        let at = self.fiber.stack.len() - n;
        if let Err(e) = self.tick(no_pos()).and_then(|()| self.check_effect(nat)) {
            self.fiber.stack.truncate(at);
            return Some(Err(e));
        }
        let args = self.take_args(at);
        let outer = self.fiber.native.replace(nat);
        let r = func(self, args).and_then(returned);
        self.fiber.native = outer;
        if let Err(Stop::Exit(code)) = r {
            self.request_exit(code);
        }
        Some(r)
    }

    fn take_args(&mut self, at: usize) -> Vec<Value> {
        self.fiber
            .stack
            .drain(at..)
            .map(|v| v.expect("argument from an unassigned slot"))
            .collect()
    }

    /// A stop a native raised: an exit ends the whole program (05); an
    /// error without a position gets the native's call site.
    pub(super) fn raised(&mut self, e: Stop, pos: Pos) -> Stop {
        match e {
            Stop::Exit(code) => {
                self.request_exit(code);
                Stop::Exit(code)
            }
            Stop::Error(d) => at(*d, pos),
        }
    }

    /// Pushes an argument for the call a step native asks for next
    /// (`Next::Call`, `Flow::Call`).
    pub(crate) fn push_arg(&mut self, v: Value) {
        self.push(v);
    }

    /// Calls a fung function or native from Rust where no fung code runs
    /// yet on this task (a test block, a native a test calls), as the root
    /// of a run (`sched.rs`), and returns when that call and every task it
    /// spawned have ended. On an error the task is put back as it was,
    /// with the call path recorded. A native that calls back into fung
    /// does not use this: it continues (`cont.rs`).
    pub(crate) fn call_root(&mut self, f: &Value, args: Vec<Value>) -> Result<Value, Stop> {
        assert!(
            self.fiber.frames.is_empty() && self.fiber.conts.is_empty(),
            "call_root inside a running task: a nested run loop"
        );
        let height = self.fiber.stack.len();
        let n = args.len();
        self.fiber.stack.extend(args.into_iter().map(Some));
        let r = self.run_root(super::Todo::Start(f.clone(), n));
        if r.is_err() {
            self.fiber.frames.clear();
            self.fiber.conts.clear();
            self.fiber.stack.truncate(height);
        }
        r
    }
}

/// A native's value, once it is done, within the string limit (VA9).
#[inline(always)]
fn returned(v: Value) -> Result<Value, Stop> {
    match crate::value::check_returned(&v) {
        Ok(()) => Ok(v),
        Err(d) => Err(Stop::Error(Box::new(d))),
    }
}
