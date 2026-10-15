//! The bytecode VM (09): a value stack and call frames per task, the run
//! loop, calls and runtime errors with call paths. Port of the
//! non-concurrent parts of Go `internal/vm` (`vm.go`, `call.go`, `task.go`,
//! the exit and `at_exit` parts of `runtime_conc.go`).
//!
//! Unassigned slots and globals are `None`: Go's Go-nil, distinct from the
//! fung `nil` value. Only JUMPIFSET (default arguments) looks at them; any
//! other read of an empty slot is a VM bug and panics, which the panic
//! fence turns into E0801 like Go's recovered nil dereference.

mod block;
mod call;
mod frame;
mod link;
mod m4;
#[cfg(test)]
mod m4_tests;
mod m6;
#[cfg(test)]
mod m6_tests;
#[cfg(test)]
mod match_tests;
mod matcher;
mod module;
#[cfg(test)]
mod module_tests;
mod runtime;
mod task;
#[cfg(test)]
mod task_tests;
#[cfg(test)]
mod tests;
pub(crate) mod token;
mod world;

use std::cmp::Ordering;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::Arc;

use fung_front::bytecode::Op;
use fung_front::diag::{Arg, CallSite, Diag, error};
use fung_front::source::{FileId, Pos};
use fung_front::tree::CaptureKind;

pub use crate::value::{Closure, Globals, Native, NativeFn, NativeParam};
pub(crate) use block::{YIELD_EVERY, block};
pub use call::MAX_FRAMES;
pub use link::RtProto;
pub(crate) use link::{Prelude, link};
pub(crate) use m6::TestCase;
pub use matcher::RtPattern;
pub use runtime::Runtime;
pub use task::TaskHandle;
pub(crate) use task::err_kind;

use crate::value::{self, Value};
use frame::Frame;
use task::CancelNode;
pub(crate) use world::World;
use world::{Shared, Token};

/// Why fung code stopped early: a runtime error (with its position and
/// call path) or `exit(n)`, which unwinds like an error but is not one.
/// The exit code is fung's int, as Go keeps it: `fung test` prints it in
/// full, and only the process exit status is cut to its low 8 bits (by
/// the operating system).
#[derive(Debug)]
pub enum Stop {
    Error(Box<Diag>),
    Exit(i64),
}

impl From<Diag> for Stop {
    fn from(d: Diag) -> Stop {
        Stop::Error(Box::new(d))
    }
}

/// The zero position: "the VM fills it in".
pub(crate) fn no_pos() -> Pos {
    Pos {
        file: FileId(0),
        line: 0,
        col: 0,
    }
}

/// An error at `pos`, unless it already has a position.
fn at(d: Diag, pos: Pos) -> Stop {
    Stop::Error(Box::new(d.with_pos(pos)))
}

fn err(code: &str, pos: Pos, args: Vec<Arg>) -> Stop {
    Stop::Error(Box::new(error(code, pos, args)))
}

/// An E0801 internal error without a position.
pub(crate) fn internal(args: Vec<Arg>) -> Stop {
    err("E0801", no_pos(), args)
}

/// One thread of fung execution: an operand stack (whose first slots are
/// the entry frame's) and the call frames, the per-task part of the
/// runtime. Natives get the task so they can write output, read input,
/// register exit handlers and call fung functions back; those reach the
/// shared part (`World`) through the run token the task holds.
///
/// A task takes the token when it is made and holds it until it drops,
/// except while it waits (`block`), yields its turn (`tick`) or calls the
/// operating system (`outside`); the run loop never locks per
/// instruction. Drop order matters: the stack and frames hold fung values, and
/// dropping a value changes `Rc` counts, which only the token's holder may
/// do. So `world` is the last field (fields drop in declaration order)
/// and `Drop` empties the stack and frames explicitly before it, after
/// taking the token back if a blocking point had given it up.
pub struct Task<'rt> {
    stack: Vec<Option<Value>>,
    frames: Vec<Frame>,
    /// The native being called, for natives that need their own name.
    native: Option<&'static Native>,
    /// This task's node in the cancellation tree (`task.rs`).
    cancel: Arc<CancelNode>,
    /// Ticks left before the next turn-handoff check (`block.rs`).
    budget: u32,
    /// What wakes this task when it blocks (`block.rs`).
    waiter: Arc<block::Waiter>,
    /// The shared part's owner, for spawning tasks on other threads.
    shared: &'rt Arc<Shared>,
    /// The run token, and through it the shared part. Keep it last.
    world: Token<'rt>,
}

impl<'rt> Task<'rt> {
    /// A task on this thread under the cancellation node `cancel`; waits
    /// for the run token.
    pub(crate) fn new(shared: &'rt Arc<Shared>, cancel: Arc<CancelNode>) -> Task<'rt> {
        let world = shared.lock();
        token::assert_holds_token();
        Task {
            stack: Vec::new(),
            frames: Vec::new(),
            native: None,
            cancel,
            budget: YIELD_EVERY,
            waiter: block::Waiter::new(),
            shared,
            world,
        }
    }

    /// Standard output (print).
    pub fn stdout(&mut self) -> &mut dyn std::io::Write {
        self.world.stdout()
    }

    /// Standard error (eprint, dbg).
    pub fn stderr(&mut self) -> &mut dyn std::io::Write {
        self.world.stderr()
    }

    /// One line of standard input with its line ending, `None` at the end.
    pub fn read_line(&mut self) -> Option<String> {
        self.world.read_line()
    }

    /// Registers an exit handler (03).
    pub fn at_exit(&mut self, f: Value) {
        self.world.at_exit(f);
    }

    /// The script arguments.
    pub fn args(&self) -> &[String] {
        &self.world.args
    }

    /// The shared part, for the crate's boot steps (`crate::boot`).
    pub(crate) fn world(&mut self) -> &mut World {
        &mut self.world
    }

    /// The name of the native being called.
    pub fn calling_native(&self) -> Option<&'static str> {
        self.native.map(|n| n.name)
    }

    /// Runs `main` (a file body) as the entry frame.
    fn run_main(&mut self, main: Rc<RtProto>, globals: Globals) -> Result<Value, Stop> {
        self.stack = vec![None; main.num_locals];
        self.frames.push(Frame {
            proto: main,
            ip: 0,
            base: 0,
            closure: None,
            globals,
            tail_calls: 0,
        });
        self.guarded(|t| t.run(0))
    }

    /// Runs `f` behind the panic fence: a Rust panic (a bug in a native or
    /// the VM) must never reach the user as a Rust backtrace (00, 09). It
    /// wraps every entry from Rust into fung code: `run_main` and `call`.
    fn guarded(&mut self, f: impl FnOnce(&mut Self) -> Result<Value, Stop>) -> Result<Value, Stop> {
        match catch_unwind(AssertUnwindSafe(|| f(self))) {
            Ok(r) => r,
            Err(payload) => {
                // A panic never leaves the token given up: a blocking
                // point that panicked mid-wait would; take it back.
                self.world.reacquire();
                let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "panic".to_string()
                };
                let pos = self
                    .frames
                    .last()
                    .and_then(Frame::current_pos)
                    .unwrap_or_else(no_pos);
                let d = error("E0801", pos, vec![Arg::Str(msg)]).with_call_path(self.call_path());
                Err(Stop::Error(Box::new(d)))
            }
        }
    }

    /// Runs until the frame count drops to `stop` and attaches the call
    /// path to any error (innermost run loop first; outer loops keep it).
    fn run(&mut self, stop: usize) -> Result<Value, Stop> {
        match self.exec(stop) {
            Err(Stop::Error(d)) => Err(Stop::Error(self.attach(d))),
            r => r,
        }
    }

    /// Records the call path once and, when the error arose inside an
    /// embedded extras file, moves its position to the nearest frame of
    /// user code: the user cannot open extras/*.fung, and the extras frames
    /// stay visible in the call path (06).
    fn attach(&self, mut d: Box<Diag>) -> Box<Diag> {
        if !d.call_path.is_empty() {
            return d;
        }
        d.call_path = self.call_path();
        if self.frames.last().is_some_and(|f| f.proto.builtin) {
            for f in self.frames.iter().rev().skip(1) {
                if !f.proto.builtin
                    && let Some(p) = f.current_pos()
                {
                    d.pos = p;
                    break;
                }
            }
        }
        d
    }

    /// The frames innermost first, for error reports (08).
    fn call_path(&self) -> Vec<CallSite> {
        self.frames
            .iter()
            .rev()
            .map(|f| {
                let (file, line) = match f.current_pos() {
                    Some(p) => (self.world.file_name(p.file).to_string(), p.line),
                    None => (String::new(), 0),
                };
                let func = if f.proto.name.is_empty() {
                    "<lambda>".to_string()
                } else {
                    f.proto.name.clone()
                };
                CallSite {
                    file,
                    line,
                    func,
                    elided: f.tail_calls,
                }
            })
            .collect()
    }

    fn push(&mut self, v: Value) {
        self.stack.push(Some(v));
    }

    /// Pops an operand. An empty slot here is a VM bug (see the module doc).
    fn pop(&mut self) -> Value {
        match self.stack.pop() {
            Some(Some(v)) => v,
            Some(None) => panic!("VM read an unassigned slot"),
            None => panic!("VM stack underflow"),
        }
    }

    fn top(&self) -> &Value {
        match self.stack.last() {
            Some(Some(v)) => v,
            _ => panic!("VM read an unassigned slot"),
        }
    }

    /// A binary operator. Two ints take the inline path (`int`, `None` on
    /// overflow); anything else, and overflow, goes to the value layer.
    #[inline(always)]
    fn binary(
        &mut self,
        pos: Pos,
        int: impl Fn(i64, i64) -> Option<i64>,
        f: impl Fn(&Value, &Value) -> Result<Value, Diag>,
    ) -> Result<(), Stop> {
        let b = self.pop();
        let a = self.pop();
        if let (Value::Int(x), Value::Int(y)) = (&a, &b)
            && let Some(r) = int(*x, *y)
        {
            self.push(Value::Int(r));
            return Ok(());
        }
        let r = f(&a, &b).map_err(|d| at(d, pos))?;
        self.push(r);
        Ok(())
    }

    fn unary(&mut self, f: fn(&Value) -> Result<Value, Diag>, pos: Pos) -> Result<(), Stop> {
        let a = self.pop();
        let r = f(&a).map_err(|d| at(d, pos))?;
        self.push(r);
        Ok(())
    }

    #[inline(always)]
    fn compare(&mut self, pos: Pos, test: impl Fn(Ordering) -> bool) -> Result<(), Stop> {
        let b = self.pop();
        let a = self.pop();
        let c = match (&a, &b) {
            (Value::Int(x), Value::Int(y)) => x.cmp(y),
            _ => value::compare(&a, &b).map_err(|d| at(d, pos))?,
        };
        self.push(Value::Bool(test(c)));
        Ok(())
    }

    /// The run loop. Each iteration re-reads the current frame: calls and
    /// returns change it.
    fn exec(&mut self, stop: usize) -> Result<Value, Stop> {
        loop {
            let fi = self.frames.len() - 1;
            let f = &mut self.frames[fi];
            let ip = f.ip;
            let instr = f.proto.code[ip];
            let pos = f.proto.pos[ip];
            f.ip = ip + 1;
            let base = f.base;
            let arg = instr.arg();
            match instr.op() {
                Op::Const => {
                    let v = self.frames[fi].proto.consts[arg].clone();
                    self.push(v);
                }
                Op::Pop => {
                    self.stack.pop();
                }
                Op::Add => self.binary(pos, i64::checked_add, value::add)?,
                Op::Sub => self.binary(pos, i64::checked_sub, value::sub)?,
                Op::Mul => self.binary(pos, i64::checked_mul, value::mul)?,
                Op::Div => self.binary(pos, |_, _| None, value::div)?,
                Op::FloorDiv => self.binary(pos, |_, _| None, value::floor_div)?,
                Op::Mod => self.binary(pos, |_, _| None, value::rem)?,
                Op::Pow => self.binary(pos, |_, _| None, value::pow)?,
                Op::Neg => self.unary(value::neg, pos)?,
                Op::Not => self.unary(value::not, pos)?,
                Op::Eq | Op::Ne => {
                    let b = self.pop();
                    let a = self.pop();
                    let eq = value::equal(&a, &b);
                    self.push(Value::Bool(eq == (instr.op() == Op::Eq)));
                }
                Op::Lt => self.compare(pos, Ordering::is_lt)?,
                Op::Gt => self.compare(pos, Ordering::is_gt)?,
                Op::Le => self.compare(pos, Ordering::is_le)?,
                Op::Ge => self.compare(pos, Ordering::is_ge)?,
                Op::CheckBool => {
                    let top = self.top();
                    if !matches!(top, Value::Bool(_)) {
                        let tn = value::type_name(top);
                        return Err(err("E0505", pos, vec![Arg::Str(tn)]));
                    }
                }
                Op::Jump => {
                    // The compiler emits forward jumps only today; a
                    // backward one would be a loop, which must stay
                    // cancellable and share the token like a call.
                    if arg <= ip {
                        self.tick(pos)?;
                    }
                    self.frames[fi].ip = arg;
                }
                Op::JumpIfFalse => match self.pop() {
                    Value::Bool(true) => {}
                    Value::Bool(false) => self.frames[fi].ip = arg,
                    v => {
                        let tn = value::type_name(&v);
                        return Err(err("E0505", pos, vec![Arg::Str(tn)]));
                    }
                },
                Op::Call | Op::TailCall => {
                    self.tick(pos)?;
                    let tail = instr.op() == Op::TailCall;
                    if let Some(r) = self.call_op(arg, pos, tail)? {
                        // A native ran to completion.
                        self.push(r);
                        if tail && let Some(v) = self.ret(stop) {
                            return Ok(v);
                        }
                    }
                }
                Op::Closure => {
                    let c = self.make_closure(fi, arg);
                    self.push(c);
                }
                Op::LoadCaptured => {
                    let v = self.frames[fi]
                        .closure
                        .as_ref()
                        .expect("LOADCAPTURED outside a closure")
                        .captured[arg]
                        .clone();
                    self.push(v);
                }
                Op::LoadSelf => {
                    let c = self.frames[fi]
                        .closure
                        .clone()
                        .expect("LOADSELF outside a closure");
                    self.push(Value::Closure(c));
                }
                Op::JumpIfSet => {
                    if self.stack.pop().flatten().is_some() {
                        self.frames[fi].ip = arg;
                    }
                }
                Op::CheckType => {
                    let (slot, ti) = (arg & 0xFFF, arg >> 12);
                    let v = self.stack[base + slot]
                        .as_ref()
                        .expect("CHECKTYPE on an unassigned slot");
                    let p = &self.frames[fi].proto;
                    if !value::matches_type(v, Some(&p.types[ti])) {
                        return Err(err(
                            "E0403",
                            pos,
                            vec![
                                Arg::Str(p.params[slot].name.clone()),
                                Arg::Str(p.types[ti].join(" | ")),
                                Arg::Str(value::type_name(v)),
                            ],
                        ));
                    }
                }
                Op::CheckReturn => {
                    let v = self.top();
                    let p = &self.frames[fi].proto;
                    if !value::matches_type(v, Some(&p.types[arg])) {
                        return Err(err(
                            "E0404",
                            pos,
                            vec![
                                Arg::Str(p.types[arg].join(" | ")),
                                Arg::Str(value::type_name(v)),
                            ],
                        ));
                    }
                }
                Op::LoadLocal => {
                    // Copies an empty slot too: JUMPIFSET reads it.
                    let v = self.stack[base + arg].clone();
                    self.stack.push(v);
                }
                Op::StoreLocal => {
                    let v = self.stack.pop().expect("VM stack underflow");
                    self.stack[base + arg] = v;
                }
                Op::LoadGlobal => {
                    let v = self.frames[fi].globals.borrow()[arg].clone();
                    match v {
                        Some(v) => self.push(v),
                        None => {
                            // Unreachable since model C: resolve binds
                            // top-down and functions copy variables.
                            let name = &self.frames[fi].proto.global_names[arg];
                            return Err(err(
                                "E0801",
                                pos,
                                vec![Arg::Str(format!("global `{name}` read before assignment"))],
                            ));
                        }
                    }
                }
                Op::StoreGlobal => {
                    let v = self.pop();
                    // The old value drops after the table's borrow ends.
                    let old = self.frames[fi].globals.borrow_mut()[arg].replace(v);
                    drop(old);
                }
                Op::Concat => {
                    let start = self.stack.len() - arg;
                    let mut s = String::new();
                    for part in self.stack.drain(start..) {
                        s.push_str(&value::format_for_print(
                            part.as_ref().expect("CONCAT of an unassigned slot"),
                        ));
                    }
                    self.push(Value::Str(Rc::new(s)));
                }
                Op::List
                | Op::Map
                | Op::Tuple
                | Op::Tag
                | Op::Range
                | Op::Index
                | Op::Slice
                | Op::Field
                | Op::In
                | Op::UnwrapAbort
                | Op::UnwrapTest
                | Op::Coalesce
                | Op::Match
                | Op::MatchOrFail
                | Op::NoMatch => self.exec_m4(fi, instr.op(), arg, pos)?,
                Op::Import => self.exec_import(fi, arg, pos)?,
                Op::Assert | Op::Test => self.exec_m6(fi, instr.op(), arg, pos)?,
                Op::Return => {
                    if let Some(v) = self.ret(stop) {
                        return Ok(v);
                    }
                } // Every opcode is handled: the match is exhaustive, so Go's
                  // "unknown opcode" default cannot arise.
            }
        }
    }

    /// CLOSURE: copies what the child captures from the creating frame
    /// (03, model C).
    fn make_closure(&self, fi: usize, child: usize) -> Value {
        let f = &self.frames[fi];
        let proto = f.proto.protos[child].clone();
        let captured = proto
            .captures
            .iter()
            .map(|c| match c.kind {
                CaptureKind::Local => self.stack[f.base + c.index]
                    .clone()
                    .expect("captured an unassigned slot"),
                CaptureKind::Captured => f
                    .closure
                    .as_ref()
                    .expect("captured from outside a closure")
                    .captured[c.index]
                    .clone(),
                CaptureKind::SelfFn => {
                    Value::Closure(f.closure.clone().expect("captured self outside a closure"))
                }
                CaptureKind::Global => f.globals.borrow()[c.index]
                    .clone()
                    .expect("captured an unassigned global"),
            })
            .collect();
        Value::Closure(Rc::new(Closure {
            proto,
            captured,
            globals: f.globals.clone(),
        }))
    }
}

impl Drop for Task<'_> {
    /// The end of a task is a value boundary (token.rs): the stack and
    /// frames drop here, with the token held, before `world` releases it.
    fn drop(&mut self) {
        self.world.reacquire();
        token::assert_holds_token();
        self.native = None;
        self.frames.clear();
        self.stack.clear();
    }
}
