//! The calling convention (09. 호출 규약): binding arguments to slots for
//! fung functions and natives alike (06: 같은 호출 규약), entering a
//! callee, tail calls, returns, and calls from natives back into fung.

use std::rc::Rc;

use fung_front::bytecode::decode_call;
use fung_front::diag::{Arg, Diag, error};
use fung_front::source::Pos;

use super::cont::{Cont, ContFrame, Flow, Next, Wake};
use super::frame::{Check, Frame, TailFrom};
use super::{RtProto, Stop, Task, at, err, no_pos};
use crate::value::{Args, Closure, List, Native, NativeBody, Sym, Value, type_name};

/// Bounds non-tail recursion (09: 깊이 한도 100,000).
pub const MAX_FRAMES: usize = 100_000;

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

/// What a call binds its arguments for: a fung function, whose defaults
/// the prologue computes, or a native, whose defaults are constants
/// filled here (06: 같은 호출 규약).
#[derive(Clone, Copy)]
enum Callee<'a> {
    Fung(&'a RtProto),
    Native(&'static Native),
}

impl<'a> Callee<'a> {
    /// The name errors give the callee.
    fn name(self) -> &'a str {
        match self {
            Callee::Fung(p) if p.name.is_empty() => "the function",
            Callee::Fung(p) => &p.name,
            Callee::Native(n) => n.name,
        }
    }

    /// How many parameters it has.
    fn len(self) -> usize {
        match self {
            Callee::Fung(p) => p.params.len(),
            Callee::Native(n) => n.params.len(),
        }
    }

    /// The name of parameter `i`.
    fn param_name(self, i: usize) -> &'a str {
        match self {
            Callee::Fung(p) => &p.params[i].name,
            Callee::Native(n) => n.params[i].name,
        }
    }

    /// Whether parameter `i` is a rest parameter.
    fn rest(self, i: usize) -> bool {
        match self {
            Callee::Fung(p) => p.params[i].rest,
            Callee::Native(n) => n.params[i].rest,
        }
    }

    fn has_default(self, i: usize) -> bool {
        match self {
            Callee::Fung(p) => p.params[i].has_default,
            Callee::Native(n) => n.params[i].default.is_some(),
        }
    }

    /// A native's default value; fung defaults are computed by the prologue.
    fn default(self, i: usize) -> Option<Value> {
        match self {
            Callee::Fung(_) => None,
            Callee::Native(n) => n.params[i].default.map(|l| l.to_value()),
        }
    }

    /// Whether a call fills every parameter by position and nothing else:
    /// the arguments are then the first slots of the frame, or the
    /// native's arguments, as they lie, with no binding work (09: 호출
    /// 규약, VM5).
    #[inline(always)]
    fn exact(self, npos: usize, nnamed: usize) -> bool {
        let n = self.len();
        nnamed == 0 && npos == n && (n == 0 || !self.rest(n - 1))
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
///
/// Invariant (VM36): if `resume` panics, the real box is dropped with the
/// panic and this stand-in stays in its place. The panic (E0801) then
/// unwinds through it with the default `unwind`, which only passes the
/// stop on: that native's own unwinding (a `with` release, say) does not
/// run. A panic is a runtime bug, so this is accepted, not handled.
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
    /// CALL / TAILCALL with the callee and its operands on the stack:
    /// `npos` positional arguments and `nnamed` named ones. Frame `fi`
    /// (the top one) makes the call; its position is read only for an
    /// error or a native's call (VM16).
    pub(super) fn call_op(
        &mut self,
        npos: usize,
        nnamed: usize,
        fi: usize,
        tail: bool,
    ) -> Result<Entered, Stop> {
        let top = self.fiber.stack.len();
        let callee_at = top - npos - 2 * nnamed - 1;
        // A fung function: called with exactly its positional parameters,
        // the arguments already sit on the stack in order and become the
        // new frame's first slots where they are; otherwise (defaults,
        // named arguments, a rest parameter) they are bound where they
        // lie first (VM6).
        if let Some(Value::Closure(_)) = &self.fiber.stack[callee_at] {
            let Some(Value::Closure(c)) = self.fiber.stack[callee_at].take() else {
                unreachable!()
            };
            let callee = Callee::Fung(&c.proto);
            // Every parameter given by position: the defaults' prologue
            // has nothing to fill, and the frame starts past it (VM21).
            let start = if callee.exact(npos, nnamed) {
                c.proto.body
            } else {
                self.bind_in_place(callee, callee_at + 1, npos, nnamed)
                    .map_err(|d| at(d, self.here(fi)))?;
                0
            };
            if tail {
                let base = self.fiber.frames.last().expect("a frame").base();
                // Drop the frame's slots, temporaries and the callee slot;
                // the arguments slide down to the frame base.
                self.fiber.stack.drain(base..=callee_at);
                self.reuse_frame(c, start);
                return Ok(Entered::Frame);
            }
            if self.fiber.frames.len() >= MAX_FRAMES {
                return Err(err(
                    "E0804",
                    self.here(fi),
                    vec![Arg::Int(MAX_FRAMES as i64)],
                ));
            }
            // The callee slot stays, empty, below the arguments: they
            // become the new frame's first slots where they lie (VM16).
            self.push_frame(c, callee_at + 1, callee_at, start);
            return Ok(Entered::Frame);
        }
        // A native: it runs on its arguments, bound where they lie when
        // the call is not exactly its positional parameters (VM5, VM6).
        if let Some(Value::Native(nat)) = &self.fiber.stack[callee_at] {
            let nat = *nat;
            let pos = self.here(fi);
            if !Callee::Native(nat).exact(npos, nnamed) {
                self.bind_in_place(Callee::Native(nat), callee_at + 1, npos, nnamed)
                    .map_err(|d| at(d, pos))?;
            }
            let args = self.take_args(callee_at + 1);
            self.fiber.stack.pop();
            return self.run_native(nat, args, pos, tail);
        }
        let callee = self.fiber.stack[callee_at].take().expect("callee");
        Err(err(
            "E0401",
            self.here(fi),
            vec![Arg::Str(type_name(&callee))],
        ))
    }

    /// CALLSPREAD / TAILCALLSPREAD: the last of the `npos` positional
    /// arguments is a list or a range, and its items take its place on
    /// the stack, before the named arguments (D45). Anything else is
    /// E0506. Returns the positional count after. Out of the run loop,
    /// like every rare path there: inlined, it grows the loop's frame.
    #[inline(never)]
    pub(super) fn spread_args(
        &mut self,
        npos: usize,
        nnamed: usize,
        fi: usize,
    ) -> Result<usize, Stop> {
        let slot = self.fiber.stack.len() - 2 * nnamed - 1;
        let v = self.fiber.stack[slot].take().expect("a spread value");
        let items = match &v {
            Value::List(_) | Value::Range(_) => crate::native::nat::items(&v, "spread"),
            _ => Err(crate::native::nat::not_iterable("spread", &v)),
        };
        let items = items.map_err(|e| match e {
            Stop::Error(d) => at(*d, self.here(fi)),
            other => other,
        })?;
        let n = items.len();
        self.fiber
            .stack
            .splice(slot..=slot, items.into_iter().map(Some));
        Ok(npos - 1 + n)
    }

    /// TAILCHECK at `ip` of frame `fi`: the frame takes the return check
    /// `types[ty]` of the function it runs along into the tail call that
    /// follows (D44). A call to the very closure the frame runs needs
    /// none: its own check runs where it returns.
    #[inline(never)]
    pub(super) fn tail_check(&mut self, fi: usize, ip: usize, ty: usize) {
        let f = &self.fiber.frames[fi];
        let call = f.proto().code[ip + 1];
        let (npos, nnamed) = decode_call(call.arg as usize);
        let callee_at = self.fiber.stack.len() - npos - 2 * nnamed - 1;
        if let Some(Value::Closure(c)) = &self.fiber.stack[callee_at]
            && Rc::ptr_eq(c, &f.closure)
        {
            return;
        }
        let check = Check {
            proto: f.proto().clone(),
            ty,
            pos: self.here(fi),
        };
        self.fiber.frames[fi].add_check(check);
    }

    /// The return checks the top frame took along with its tail calls
    /// (D44), on its value `v`, the latest first: E0404 where the function
    /// that wrote the return type returns its value, with the call path
    /// as it is.
    #[cold]
    #[inline(never)]
    fn run_checks(&self, v: &Value) -> Result<(), Stop> {
        let f = self.fiber.frames.last().expect("a frame");
        let checks = f.extra.as_ref().map_or(&[][..], |x| &x.checks[..]);
        for c in checks.iter().rev() {
            let t = &c.proto.types[c.ty];
            if !t.matches(v) {
                let mut d = error(
                    "E0404",
                    c.pos,
                    vec![Arg::Str(t.text().to_string()), Arg::Str(type_name(v))],
                );
                d.call_path = self.call_path();
                return Err(Stop::Error(Box::new(d)));
            }
        }
        Ok(())
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
        let n = callee.len();
        let rest = n > 0 && callee.rest(n - 1);
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
        let slot_of = |k: Sym| callee.slot_of(k, fixed);
        // One pass over the named arguments finds each one's slot and
        // marks it (VM34: the checks were a scan per pair before, cubic
        // in a call with thousands of them). The slots are kept with the
        // values in the task's buffer, which keeps its room from call to
        // call (VM6).
        let mut named = std::mem::take(&mut self.fiber.named);
        named.clear();
        let mut filled = std::mem::take(&mut self.fiber.filled);
        filled.clear();
        filled.resize(fixed, false);
        let checked = (|| {
            let stack = &self.fiber.stack;
            for j in 0..nnamed {
                let k = sym_at(stack, j);
                let Some(i) = slot_of(k) else {
                    return Err(error(
                        "E0406",
                        no_pos(),
                        vec![Arg::Str(name.to_string()), Arg::Str(k.name().to_string())],
                    ));
                };
                if i < npos || filled[i] {
                    return Err(error(
                        "E0405",
                        no_pos(),
                        vec![Arg::Str(k.name().to_string())],
                    ));
                }
                filled[i] = true;
                named.push((i, Value::Nil));
            }
            for (i, &named) in filled.iter().enumerate().skip(npos.min(fixed)) {
                if !named && !callee.has_default(i) {
                    // a parameter with no name in the source (`$`) by its
                    // place (FE55)
                    let p = callee.param_name(i);
                    let which = if p.starts_with('$') {
                        (i + 1).to_string()
                    } else {
                        format!("`{p}`")
                    };
                    return Err(error("E0407", no_pos(), vec![Arg::Str(which)]));
                }
            }
            Ok(())
        })();
        self.fiber.filled = filled;
        if let Err(d) = checked {
            self.fiber.named = named;
            return Err(d);
        }
        // The named values come off the top, next to their slots.
        let mut ops = self.fiber.stack.drain(named_at..).skip(1).step_by(2);
        for (_, v) in named.iter_mut() {
            *v = ops
                .next()
                .flatten()
                .expect("named argument from an unassigned slot");
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
        for (i, v) in named.drain(..) {
            self.fiber.stack[at + i] = Some(v);
        }
        self.fiber.named = named;
        if let Callee::Native(_) = callee {
            for i in npos.min(fixed)..fixed {
                let slot = &mut self.fiber.stack[at + i];
                if slot.is_none() {
                    *slot = callee.default(i);
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
        args: Args,
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

    /// Pushes a frame for `c` whose arguments start at `base` and that
    /// cuts the stack back to `floor` when it returns.
    /// Enters `c` in a new frame at instruction `start` (0, or past the
    /// defaults' prologue, VM21).
    fn push_frame(&mut self, c: Rc<Closure>, base: usize, floor: usize, start: usize) {
        self.pad_slots(base, c.proto.num_locals);
        let entry = self.entry_for(&c.proto, self.fiber.frames.last());
        let mut f = Frame::new(c, base, floor, entry);
        f.ip = start;
        self.fiber.frames.push(f);
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
    fn reuse_frame(&mut self, c: Rc<Closure>, start: usize) {
        let base = self.fiber.frames.last().expect("a frame").base();
        self.pad_slots(base, c.proto.num_locals);
        // A function calling itself (a loop): the frame already runs this
        // closure, with its proto, globals and entry (VM9).
        let f = self.fiber.frames.last_mut().expect("a frame");
        if Rc::ptr_eq(&f.closure, &c) {
            f.tail_calls += 1;
            f.ip = start;
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
                let t = TailFrom {
                    proto: f.proto().clone(),
                    pos: f.current_pos(),
                    tail_calls: f.tail_calls,
                };
                f.extra.get_or_insert_with(Default::default).tail_from = Some(t);
            }
            f.tail_calls = 0;
        }
        f.closure = c;
        f.ip = start;
    }

    /// Pops the current frame (a RETURN, or a TAILCALL whose native
    /// finished) and gives `v` to whoever waits for it: a continuation at
    /// that depth or the caller's stack. The value of the run loop when
    /// the last frame returns.
    #[inline(always)]
    pub(super) fn ret_with(&mut self, v: Value) -> Result<Ran, Stop> {
        if self.fiber.frames.last().is_some_and(Frame::has_checks) {
            self.run_checks(&v)?;
        }
        let f = self.fiber.frames.pop().expect("a frame");
        self.fiber.stack.truncate(f.floor());
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
                            if self.fiber.frames.last().is_some_and(Frame::has_checks) {
                                self.run_checks(&v)?;
                            }
                            let f = self.fiber.frames.pop().expect("a frame");
                            self.fiber.stack.truncate(f.floor());
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
                    if self.fiber.frames.last().is_some_and(Frame::has_checks) {
                        self.run_checks(&w)?;
                    }
                    let f = self.fiber.frames.pop().expect("a frame");
                    self.fiber.stack.truncate(f.floor());
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
                let start = if Callee::Fung(&c.proto).exact(n, 0) {
                    c.proto.body
                } else {
                    self.bind_in_place(Callee::Fung(&c.proto), at, n, 0)
                        .map_err(|d| super::at(d, no_pos()))?;
                    0
                };
                self.push_frame(c.clone(), at, at, start);
                Ok(Entered::Frame)
            }
            Value::Closure(_) => {
                self.fiber.stack.truncate(at);
                Err(err("E0804", no_pos(), vec![Arg::Int(MAX_FRAMES as i64)]))
            }
            Value::Native(nat) => {
                let nat = *nat;
                if !Callee::Native(nat).exact(n, 0) {
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
        if !Callee::Native(nat).exact(n, 0) {
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

    /// The arguments from `at` to the top of the stack, taken off it: a
    /// few are held inline, so a call allocates nothing for them (VM15).
    #[inline(always)]
    fn take_args(&mut self, at: usize) -> Args {
        let n = self.fiber.stack.len() - at;
        let mut args = Args::with_capacity(n);
        for v in self.fiber.stack.drain(at..) {
            args.push(v.expect("argument from an unassigned slot"));
        }
        args
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
