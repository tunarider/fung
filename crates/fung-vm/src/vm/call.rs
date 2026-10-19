//! The calling convention (09. 호출 규약): binding arguments to slots for
//! fung functions and natives alike (06: 같은 호출 규약), entering a
//! callee, tail calls, returns, and calls from natives back into fung.

use std::rc::Rc;

use fung_front::bytecode::{Param, decode_call};
use fung_front::diag::{Arg, Diag, error};
use fung_front::source::Pos;

use super::cont::{Cont, ContFrame, Flow, Next, Wake};
use super::frame::Frame;
use super::{RtProto, Stop, Task, at, err, no_pos};
use crate::value::{Closure, List, NativeBody, NativeParam, Sym, Value, type_name};

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
                return Ok(Entered::Frame);
            }
            if self.frames.len() >= MAX_FRAMES {
                return Err(err("E0804", pos, vec![Arg::Int(MAX_FRAMES as i64)]));
            }
            self.stack.remove(callee_at);
            self.push_frame(c, callee_at);
            return Ok(Entered::Frame);
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

    /// Starts running `callee`. A plain native runs at once and its result
    /// comes back; a closure gets a new frame (or reuses the current one
    /// when `tail` is set); a step native either finishes at once or puts
    /// its continuation on the list and makes its first call.
    pub(super) fn enter(
        &mut self,
        callee: Value,
        positional: Vec<Value>,
        named: Vec<(Sym, Value)>,
        pos: Pos,
        tail: bool,
    ) -> Result<Entered, Stop> {
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
                self.check_effect(n).map_err(|e| match e {
                    Stop::Error(d) => at(*d, pos),
                    other => other,
                })?;
                let args = slots
                    .into_iter()
                    .map(|s| s.expect("bound native argument"))
                    .collect();
                let outer = self.native.replace(n);
                match n.body {
                    NativeBody::Plain(func) => {
                        let r = func(self, args);
                        self.native = outer;
                        match r {
                            Ok(v) => Ok(Entered::Value(v)),
                            Err(e) => Err(self.raised(e, pos)),
                        }
                    }
                    NativeBody::Step(step) => {
                        let r = step(self, args);
                        self.native = outer;
                        match r {
                            Ok(Flow::Done(v)) => Ok(Entered::Value(v)),
                            Ok(Flow::Call(k, f, nargs)) => {
                                self.conts.push(ContFrame {
                                    depth: self.frames.len(),
                                    height: self.stack.len() - nargs,
                                    pos,
                                    native: Some(n),
                                    tail,
                                    k,
                                });
                                Ok(Entered::Call(f, nargs))
                            }
                            Ok(Flow::Park(k, w)) => {
                                self.conts.push(ContFrame {
                                    depth: self.frames.len(),
                                    height: self.stack.len(),
                                    pos,
                                    native: Some(n),
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
                    return Ok(Entered::Frame);
                }
                if self.frames.len() >= MAX_FRAMES {
                    self.stack.truncate(base);
                    return Err(err("E0804", pos, vec![Arg::Int(MAX_FRAMES as i64)]));
                }
                self.push_frame(c, base);
                Ok(Entered::Frame)
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
        let entry = self.entry_for(&c.proto, self.frames.last());
        self.frames.push(Frame {
            proto: c.proto.clone(),
            ip: 0,
            base,
            globals: c.globals.clone(),
            closure: Some(c),
            tail_calls: 0,
            entry,
        });
    }

    /// The built-in function a frame for `proto` runs inside of, called
    /// from `caller`: none in user code; `proto` itself when user code
    /// (or nothing) calls it; else the caller's.
    fn entry_for(&self, proto: &Rc<RtProto>, caller: Option<&Frame>) -> Option<Rc<RtProto>> {
        if !proto.builtin {
            return None;
        }
        match caller {
            Some(f) if f.proto.builtin => f.entry.clone(),
            _ => Some(proto.clone()),
        }
    }

    /// Makes the current frame run `c` (a tail call); the arguments are
    /// already at the frame base.
    fn reuse_frame(&mut self, c: Rc<Closure>) {
        let base = self.frames.last().expect("a frame").base;
        self.pad_slots(base, c.proto.num_locals);
        let n = self.frames.len();
        // A tail call replaces the frame, not its caller: the entry is
        // the reused frame's own if it runs inside a built-in function,
        // the new one's if user code tail-calls into one.
        let entry = if self.frames[n - 1].proto.builtin && c.proto.builtin {
            self.frames[n - 1].entry.clone()
        } else {
            let caller = n.checked_sub(2).map(|i| &self.frames[i]);
            self.entry_for(&c.proto, caller)
        };
        let f = self.frames.last_mut().expect("a frame");
        f.entry = entry;
        f.proto = c.proto.clone();
        f.globals = c.globals.clone();
        f.closure = Some(c);
        f.ip = 0;
        f.tail_calls += 1;
    }

    /// Pops the current frame (a RETURN, or a TAILCALL whose native
    /// finished) and gives `v` to whoever waits for it: a continuation at
    /// that depth or the caller's stack. The value of the run loop when
    /// the frame count drops to its stop.
    #[inline(always)]
    pub(super) fn ret_with(&mut self, v: Value) -> Result<Ran, Stop> {
        let f = self.frames.pop().expect("a frame");
        self.stack.truncate(f.base);
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
    /// value at its stop.
    #[inline(always)]
    fn deliver(&mut self, mut v: Value) -> Result<Delivered, Stop> {
        loop {
            if self.conts.len() > self.floor
                && self.conts.last().expect("a continuation").depth == self.frames.len()
            {
                // The box comes out for the call (a zero-sized stand-in
                // takes its place, no allocation) and goes back if the
                // native asks for another call: the frame stays put.
                let i = self.conts.len() - 1;
                let mut k = std::mem::replace(&mut self.conts[i].k, Box::new(Hole));
                let outer = std::mem::replace(&mut self.native, self.conts[i].native);
                let r = k.resume(self, v);
                self.native = outer;
                match r {
                    Ok(Next::Call(f, n)) => {
                        self.conts[i].k = k;
                        return Ok(Delivered::Call(f, n));
                    }
                    Ok(Next::Park(w)) => {
                        self.conts[i].k = k;
                        return Ok(Delivered::Park(w));
                    }
                    Ok(Next::Done(w)) => {
                        let c = self.conts.pop().expect("a continuation");
                        v = self.done(&c, w);
                        if c.tail {
                            let f = self.frames.pop().expect("a frame");
                            self.stack.truncate(f.base);
                        }
                    }
                    Err(e) => {
                        let c = self.conts.pop().expect("a continuation");
                        return Err(self.raised(e, c.pos));
                    }
                }
                continue;
            }
            if self.frames.len() == self.stop {
                return Ok(Delivered::Finished(v));
            }
            self.push(v);
            return Ok(Delivered::Pushed);
        }
    }

    /// A continuation's result: the stack back to where its arguments
    /// started.
    fn done(&mut self, c: &ContFrame, v: Value) -> Value {
        self.stack.truncate(c.height);
        v
    }

    /// `stop` unwinds into the innermost continuation of this run loop
    /// (`drive` checked there is one): the frames and stack go back to
    /// where its native was called, and the native decides.
    pub(super) fn unwind(&mut self, stop: Stop) -> Result<Ran, Stop> {
        let mut c = self.conts.pop().expect("a continuation");
        self.frames.truncate(c.depth);
        self.stack.truncate(c.height);
        let outer = std::mem::replace(&mut self.native, c.native);
        let r = c.k.unwind(self, stop);
        self.native = outer;
        match r {
            Ok(Next::Call(f, n)) => {
                self.conts.push(c);
                self.issue(f, n)
            }
            Ok(Next::Park(w)) => {
                self.conts.push(c);
                Ok(Ran::Parked(w))
            }
            Ok(Next::Done(w)) => {
                let w = self.done(&c, w);
                if c.tail {
                    let f = self.frames.pop().expect("a frame");
                    self.stack.truncate(f.base);
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
            if self.turn_due {
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
        let at = self.stack.len() - n;
        match &f {
            Value::Closure(c)
                if exact_positional(&c.proto, n, 0) && self.frames.len() < MAX_FRAMES =>
            {
                self.push_frame(c.clone(), at);
                Ok(Entered::Frame)
            }
            _ => {
                let args = self.take_args(at);
                self.enter(f, args, Vec::new(), no_pos(), false)
            }
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
        if self.turn_due {
            return None;
        }
        let Value::Native(nat) = f else {
            return None;
        };
        let NativeBody::Plain(func) = nat.body else {
            return None;
        };
        if nat.params.len() != n || nat.params.last().is_some_and(|p| p.rest) {
            return None;
        }
        let at = self.stack.len() - n;
        if let Err(e) = self.tick(no_pos()).and_then(|()| self.check_effect(nat)) {
            self.stack.truncate(at);
            return Some(Err(e));
        }
        let args = self.take_args(at);
        let outer = self.native.replace(nat);
        let r = func(self, args);
        self.native = outer;
        if let Err(Stop::Exit(code)) = r {
            self.request_exit(code);
        }
        Some(r)
    }

    fn take_args(&mut self, at: usize) -> Vec<Value> {
        self.stack
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
            self.frames.is_empty() && self.conts.is_empty(),
            "call_root inside a running task: a nested run loop"
        );
        let height = self.stack.len();
        let n = args.len();
        self.stack.extend(args.into_iter().map(Some));
        let r = self.run_root(super::Todo::Start(f.clone(), n));
        if r.is_err() {
            self.frames.clear();
            self.conts.clear();
            self.stack.truncate(height);
        }
        r
    }
}
