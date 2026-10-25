//! The bytecode VM (09): a value stack and call frames per task, the run
//! loop, calls and runtime errors with call paths, and `exit`. Tasks,
//! scopes, channels and atoms live in the submodules.
//!
//! Unassigned slots and globals are `None`, distinct from the fung `nil`
//! value. Only JUMPIFSET (default arguments) looks at them; any other
//! read of an empty slot is a VM bug and panics, which the panic fence
//! turns into E0801 (an internal error, not a crash of the process).

mod atom;
#[cfg(test)]
mod atom_tests;
mod block;
mod call;
pub(crate) mod channel;
#[cfg(test)]
mod channel_tests;
mod cleanup;
#[cfg(test)]
mod cleanup_tests;
mod cont;
#[cfg(test)]
mod cont_tests;
mod deadline;
#[cfg(test)]
mod deadline_tests;
mod deadlock;
#[cfg(test)]
mod exit_tests;
mod failure;
#[cfg(test)]
mod failure_tests;
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
#[cfg(test)]
mod mutable_tests;
mod offload;
#[cfg(test)]
mod park_tests;
pub(crate) mod poller;
#[cfg(test)]
mod poller_tests;
mod pool;
#[cfg(test)]
mod pool_tests;
#[cfg(test)]
mod repl_tests;
mod runtime;
mod sched;
#[cfg(test)]
mod sched_tests;
mod scope;
#[cfg(test)]
mod scope_tests;
mod stdin;
pub(crate) mod stop;
#[cfg(test)]
mod stop_tests;
mod task;
#[cfg(test)]
mod task_tests;
#[cfg(test)]
mod tests;
mod world;

use std::cmp::Ordering;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::Arc;

use fung_front::bytecode::Op;
use fung_front::diag::{Arg, CallSite, Diag, error};
use fung_front::source::{FileId, Pos, decode_lossy};
use fung_front::tree::CaptureKind;

pub use crate::value::{Closure, Globals, Native, NativeBody, NativeFn, NativeParam, StepFn};
pub use atom::Atom;
#[cfg(test)]
pub(crate) use atom::effect_names;
#[cfg(test)]
pub(crate) use atom::{EFFECTS, PURE, is_listed};
pub(crate) use atom::{
    make as atom_make, read as atom_read, reset as atom_reset, run_swap as atom_swap,
};
pub(crate) use block::{
    IoRegs, IoStep, WaitList, Waiter, YIELD_EVERY, drop_io_waits, park_io, park_until,
    park_until_then,
};
pub use call::MAX_FRAMES;
pub use channel::{Channel, Receiver, Sender};
pub(crate) use cleanup::run_with;
pub use cont::{Cont, Flow, Next, Wake};
pub(crate) use deadline::run_timeout;
pub use link::RtProto;
pub(crate) use link::{Prelude, link};
pub(crate) use m4::{discarded, discarded_err, is_err};
pub(crate) use m6::TestCase;
pub use matcher::RtPattern;
pub(crate) use offload::{Kind, alone, offload_kind};
pub(crate) use poller::Interest;
pub(crate) use pool::{Pool, Ticket};
pub use runtime::Runtime;
pub use scope::ScopeHandle;
pub(crate) use scope::run_scope;
pub use stdin::BUFFER as STDIN_BUFFER;
pub use stop::StopHandle;
#[cfg(test)]
pub(crate) use task::CancelNode as CancelNodeForTests;
pub use task::TaskHandle;
pub(crate) use task::err_kind;

use crate::value::{self, Value};
use call::{Entered, Ran};
use cont::ContFrame;
use frame::Frame;
pub(crate) use task::CancelNode;
use world::Shared;
pub(crate) use world::World;

/// Why fung code stopped early: a runtime error (with its position and
/// call path) or `exit(n)`, which unwinds like an error but is not one.
/// The exit code is a whole fung int: `fung test` prints it in full,
/// and only the process exit status is cut to its low 8 bits (by the
/// operating system).
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

/// E0606 for an `Err` that a statement discarded (02). Out of the run
/// loop, like every rare error path there: inlined, such a path changes
/// how the compiler lays out the whole loop and slows the instructions
/// that never take it (P12: `fib` lost 11% to two of them).
#[cold]
#[inline(never)]
fn discarded_stop(pos: Pos, v: &Value) -> Stop {
    err("E0606", pos, vec![discarded_err(v).expect("an Err")])
}

/// An E0801 internal error without a position.
pub(crate) fn internal(args: Vec<Arg>) -> Stop {
    err("E0801", no_pos(), args)
}

fn stdin_line(line: Option<Vec<u8>>) -> Option<String> {
    line.map(|b| decode_lossy(&b))
}

/// A task between its turns (`sched.rs`): what `Task` holds of it while
/// it runs. Fields as in `Task`.
pub(crate) struct Fiber {
    stack: Vec<Option<Value>>,
    frames: Vec<Frame>,
    conts: Vec<ContFrame>,
    stop: usize,
    floor: usize,
    #[cfg(test)]
    loops: u32,
    #[cfg(test)]
    parks: u32,
    #[cfg(test)]
    offloads: u32,
    #[cfg(test)]
    yields: u32,
    turn_due: bool,
    native: Option<&'static Native>,
    cancel: Arc<CancelNode>,
    budget: u32,
    pure: u32,
    waiter: Arc<block::Waiter>,
    todo: Option<Todo>,
}

impl Fiber {
    /// A task that has not run yet, under `cancel`, with a number of its
    /// own.
    pub(super) fn new(shared: &Shared, cancel: Arc<CancelNode>) -> Fiber {
        Fiber {
            stack: Vec::new(),
            frames: Vec::new(),
            conts: Vec::new(),
            stop: 0,
            floor: 0,
            #[cfg(test)]
            loops: 0,
            #[cfg(test)]
            parks: 0,
            #[cfg(test)]
            offloads: 0,
            #[cfg(test)]
            yields: 0,
            turn_due: false,
            native: None,
            cancel,
            budget: YIELD_EVERY,
            pure: 0,
            waiter: block::Waiter::new(shared.next_fiber(), shared.ready.clone()),
            todo: None,
        }
    }

    /// A placeholder for the run thread's own state while the root runs.
    pub(super) fn empty(shared: &Shared) -> Fiber {
        Fiber::new(shared, CancelNode::root())
    }

    pub(super) fn id(&self) -> sched::FiberId {
        self.waiter.id()
    }
}

/// Where the run loop goes on at a task's next turn.
pub(crate) enum Todo {
    /// Call `f` with the `n` arguments on top of the stack.
    Start(Value, usize),
    /// Run the current frame.
    Loop,
    /// The parked continuation looks again.
    Resume,
    /// Unwind with this stop.
    Unwind(Stop),
}

/// How a turn ended (`sched.rs`).
pub(super) enum Turn {
    Done(Result<Value, Stop>),
    Parked(Wake),
    Yielded,
}

/// One thread of fung execution: an operand stack (whose first slots are
/// the entry frame's) and the call frames, the per-task part of the
/// runtime. Natives get the task so they can write output, read input
/// and ask for calls back into fung (`cont.rs`); those reach the
/// shared part (`World`) through the borrow the task holds.
///
/// A task borrows the world when it is made and keeps it until it drops;
/// the tasks of a run take turns on the run thread (`sched.rs`), so the
/// run loop never locks per instruction. Drop order matters: the stack
/// and frames hold fung values, and dropping a value changes `Rc` counts,
/// which only the run thread may do. So `world` is the last field (fields
/// drop in declaration order) and `Drop` empties the stack and frames
/// explicitly before it.
pub struct Task<'rt> {
    stack: Vec<Option<Value>>,
    frames: Vec<Frame>,
    /// Step natives waiting for a call they asked for (`cont.rs`).
    conts: Vec<ContFrame>,
    /// The frame count at which the innermost run loop returns.
    stop: usize,
    /// The continuations the innermost run loop serves: those from this
    /// index on (`cont.rs`).
    floor: usize,
    /// How many run loops are running on this task (a test probe).
    #[cfg(test)]
    pub(crate) loops: u32,
    /// How many times this task parked (a test probe).
    #[cfg(test)]
    pub(crate) parks: u32,
    /// How many waits on the operating system it handed off (a test probe).
    #[cfg(test)]
    pub(crate) offloads: u32,
    /// How many times it gave its turn at a yield point (a test probe).
    #[cfg(test)]
    pub(crate) yields: u32,
    /// The turn is over (`Task::tick`): the next yield point leaves the
    /// run loop so the others get theirs.
    turn_due: bool,
    /// The native being called, for natives that need their own name.
    native: Option<&'static Native>,
    /// This task's node in the cancellation tree (`task.rs`).
    cancel: Arc<CancelNode>,
    /// Ticks left before the next turn-handoff check (`block.rs`).
    budget: u32,
    /// How deep this task is in swaps' functions (`atom.rs`): while it is
    /// above zero, calling an effect is E0707.
    pub(crate) pure: u32,
    /// What wakes this task when it parks (`block.rs`): it carries the
    /// task's number in the scheduler (`sched.rs`).
    waiter: Arc<block::Waiter>,
    /// Where the run loop goes on at the task's next turn.
    todo: Option<Todo>,
    /// The shared part's owner.
    shared: &'rt Arc<Shared>,
    /// The borrow of the shared part. Keep it last.
    world: std::cell::RefMut<'rt, World>,
}

impl<'rt> Task<'rt> {
    /// The task the command runs on this thread (the entry, a test block,
    /// a module body): it borrows the world and keeps it while it lives.
    /// Its runs (`run_root`) give every task of the program its turns.
    pub(crate) fn new(
        world: &'rt std::cell::RefCell<World>,
        shared: &'rt Arc<Shared>,
        cancel: Arc<CancelNode>,
    ) -> Task<'rt> {
        let world = world.borrow_mut();
        let waiter = block::Waiter::new(shared.next_fiber(), shared.ready.clone());
        Task {
            stack: Vec::new(),
            frames: Vec::new(),
            conts: Vec::new(),
            stop: 0,
            floor: 0,
            #[cfg(test)]
            loops: 0,
            #[cfg(test)]
            parks: 0,
            #[cfg(test)]
            offloads: 0,
            #[cfg(test)]
            yields: 0,
            turn_due: false,
            native: None,
            cancel,
            budget: YIELD_EVERY,
            pure: 0,
            waiter,
            todo: None,
            shared,
            world,
        }
    }

    /// Swaps this task's state with `f`'s: the scheduler gives a task its
    /// turn by swapping its fiber in, and takes it out again after.
    pub(super) fn exchange(&mut self, f: &mut Fiber) {
        use std::mem::swap;
        swap(&mut self.stack, &mut f.stack);
        swap(&mut self.frames, &mut f.frames);
        swap(&mut self.conts, &mut f.conts);
        swap(&mut self.stop, &mut f.stop);
        swap(&mut self.floor, &mut f.floor);
        #[cfg(test)]
        {
            swap(&mut self.loops, &mut f.loops);
            swap(&mut self.parks, &mut f.parks);
            swap(&mut self.offloads, &mut f.offloads);
            swap(&mut self.yields, &mut f.yields);
        }
        swap(&mut self.turn_due, &mut f.turn_due);
        swap(&mut self.native, &mut f.native);
        swap(&mut self.cancel, &mut f.cancel);
        swap(&mut self.budget, &mut f.budget);
        swap(&mut self.pure, &mut f.pure);
        swap(&mut self.waiter, &mut f.waiter);
        swap(&mut self.todo, &mut f.todo);
    }

    /// Standard output (print).
    pub fn stdout(&mut self) -> &mut dyn std::io::Write {
        self.world.stdout()
    }

    /// Standard error (eprint, dbg).
    pub fn stderr(&mut self) -> &mut dyn std::io::Write {
        self.world.stderr()
    }

    /// One line of standard input with its line ending if one is queued
    /// (`Some(None)` at the end of the input); otherwise this task is put
    /// on the reader's waiting list and `None`: park until woken
    /// (`stdin.rs`). Bytes that are not UTF-8 become U+FFFD one by one,
    /// like source text.
    pub(crate) fn stdin_poll(&mut self) -> Option<Option<String>> {
        let me = self.waiter.clone();
        self.world.stdin.poll(&me).map(stdin_line)
    }

    /// A wait for a line that ended without one leaves the waiting list.
    pub(crate) fn stdin_leave(&mut self) {
        let me = self.waiter.clone();
        self.world.stdin.leave(&me);
    }

    /// How many tasks wait for a line of standard input (a test probe).
    #[cfg(test)]
    pub(crate) fn stdin_waiting(&self) -> usize {
        self.world.stdin.waiting()
    }

    /// The ticks left before this task's next turn handoff (a test probe).
    #[cfg(test)]
    pub(crate) fn budget_left(&self) -> u32 {
        self.budget
    }

    /// exit(n) from any task (05 "종료"): records the
    /// first code requested and cancels every task of the program, so
    /// the requesting task unwinds on its exit and the others on E0701,
    /// wherever they wait, unwinding through every `with`'s release
    /// (`cleanup.rs`; the releases run under shield nodes the exit does
    /// not reach).
    pub(super) fn request_exit(&mut self, code: i64) {
        self.world.request_exit(code);
        self.shared.root().cancel();
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

    /// Runs `main` (a file body) as the entry frame, the root of a run.
    fn run_main(&mut self, main: Rc<RtProto>, globals: Globals) -> Result<Value, Stop> {
        self.stack = vec![None; main.num_locals];
        self.frames.push(Frame {
            proto: main,
            ip: 0,
            base: 0,
            closure: None,
            globals,
            tail_calls: 0,
            entry: None,
        });
        self.run_root(Todo::Loop)
    }

    /// A global read before it holds a value: E0808 in the REPL (an input
    /// declared the name, then failed before storing it, 08), else E0809
    /// (a top-level fn read a constant of the file before its assignment
    /// ran, 03, L9). Cold, out of the run loop (`discarded_stop`).
    #[cold]
    #[inline(never)]
    fn unset_global(&self, fi: usize, arg: usize, pos: Pos) -> Stop {
        let name = self.global_name(fi, arg);
        let code = if self.world.repl.is_some() {
            "E0808"
        } else {
            "E0809"
        };
        let len = name.chars().count();
        Stop::Error(Box::new(error(code, pos, vec![Arg::Str(name)]).span(len)))
    }

    /// A caught panic as E0801 at the innermost frame, with the call path.
    #[cold]
    fn panicked(&mut self, payload: Box<dyn std::any::Any + Send>) -> Stop {
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
        Stop::Error(Box::new(d))
    }

    /// One turn of this task (`sched.rs`): after a park, the wait's end
    /// is checked first (a cancellation or a deadline unwinds the parked
    /// continuation; otherwise it looks again), then the run loop goes
    /// until the task is done, parks or gives its turn.
    pub(super) fn turn(&mut self) -> Turn {
        if matches!(self.todo, Some(Todo::Resume)) {
            self.todo = Some(match self.after_park() {
                Ok(()) => Todo::Resume,
                Err(e) => Todo::Unwind(self.mark(e)),
            });
        }
        #[cfg(test)]
        {
            self.loops += 1;
        }
        let turn = self.drive();
        #[cfg(test)]
        {
            self.loops -= 1;
        }
        turn
    }

    /// The runtime's shared part (test probes).
    #[cfg(test)]
    pub(crate) fn shared_for_tests(&self) -> &Arc<Shared> {
        self.shared
    }

    /// The task's waiter (tests that try a socket operation by hand).
    #[cfg(test)]
    pub(crate) fn waiter_for_tests(&self) -> Arc<block::Waiter> {
        self.waiter.clone()
    }

    /// How many waits on descriptors are registered (test probes).
    #[cfg(test)]
    pub(crate) fn io_waiting_for_tests(&self) -> usize {
        self.world.io.len()
    }

    /// A parked task resumed: its woken flag is taken first, so a wake
    /// that comes after this queues the task again whatever the
    /// continuation does next (a stop's unwind may park again: a job that
    /// is waited for, a scope draining its children). Then a cancelled
    /// task (or one past its node's deadline, which expires the node)
    /// unwinds; otherwise its waiter watches its node again, before the
    /// continuation looks.
    fn after_park(&mut self) -> Result<(), Stop> {
        self.waiter.take_woken();
        if self.cancel.is_cancelled() {
            return Err(self.unwound());
        }
        let now = std::time::Instant::now();
        if self.cancel.deadline_at().is_some_and(|at| now >= at) && self.cancel.expire_due(now) {
            return Err(self.unwound());
        }
        self.cancel.watch(&self.waiter);
        Ok(())
    }

    /// The run loop with its unwinding: an error, a cancellation or an
    /// exit goes to the innermost continuation (which may call `release`,
    /// or turn it into a value), and leaves the loop only when none is
    /// left. A panic is caught here and unwinds as E0801 (00, 09: a Rust
    /// panic never reaches the user as a backtrace). A park request or the
    /// end of the turn (`cont.rs`, `Task::tick`) returns to the scheduler;
    /// where to go on is kept in `todo`.
    fn drive(&mut self) -> Turn {
        let mut todo = self.todo.take().unwrap_or(Todo::Loop);
        loop {
            let r = catch_unwind(AssertUnwindSafe(|| match todo {
                Todo::Start(f, n) => self.issue(f, n),
                Todo::Loop => self.step_loop(),
                Todo::Resume => self.finish(Value::Nil),
                Todo::Unwind(e) => self.unwind(e),
            }));
            todo = match r {
                Ok(Ok(Ran::Done(v))) => return Turn::Done(Ok(v)),
                Ok(Ok(Ran::Going)) => Todo::Loop,
                Ok(Ok(Ran::Yielded(call))) => {
                    #[cfg(test)]
                    {
                        self.yields += 1;
                    }
                    self.turn_due = false;
                    self.todo = Some(match call {
                        Some((f, n)) => Todo::Start(f, n),
                        None => Todo::Loop,
                    });
                    return Turn::Yielded;
                }
                Ok(Ok(Ran::Parked(w))) => {
                    #[cfg(test)]
                    {
                        self.parks += 1;
                    }
                    self.todo = Some(Todo::Resume);
                    return Turn::Parked(w);
                }
                Ok(Err(e)) => {
                    let e = self.mark(e);
                    if self.conts.len() <= self.floor {
                        return Turn::Done(Err(e));
                    }
                    Todo::Unwind(e)
                }
                Err(payload) => {
                    let e = self.panicked(payload);
                    if self.conts.len() <= self.floor {
                        return Turn::Done(Err(e));
                    }
                    Todo::Unwind(e)
                }
            };
        }
    }

    /// An error as it leaves the place it arose: positioned at the call
    /// of the innermost waiting native when it has no place of its own (a
    /// native called back), with the call path of the frames as they are.
    fn mark(&self, e: Stop) -> Stop {
        match e {
            Stop::Error(mut d) => {
                if let Some(c) = self.conts.last()
                    && self.conts.len() > self.floor
                    && d.pos.is_zero()
                {
                    d.pos = c.pos;
                }
                Stop::Error(self.attach(d))
            }
            other => other,
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
        if self.frames.last().is_some_and(|f| f.proto.builtin)
            && let Some(p) = self.user_pos()
        {
            d.pos = p;
        }
        d
    }

    /// Where the innermost frame of user code is (its current call), for
    /// an error raised inside an embedded extras file (`attach`).
    pub(crate) fn user_pos(&self) -> Option<Pos> {
        self.frames
            .iter()
            .rev()
            .filter(|f| !f.proto.builtin)
            .find_map(|f| f.current_pos())
    }

    /// The function of a built-in module that user code called, which is
    /// running now (`fs.with_open`, a prelude one as `take`): what an
    /// error raised deep inside it is about (E0707). The frames remember
    /// it across the helpers it calls and tail-calls (`Frame::entry`).
    /// `None` when user code called the native itself.
    pub(crate) fn user_callee(&self) -> Option<String> {
        let i = self.frames.iter().rposition(|f| !f.proto.builtin)?;
        let p = self.frames.get(i + 1)?.entry.clone()?;
        let file = p
            .pos
            .first()
            .map(|at| self.world.file_name(at.file).to_string())?;
        let module = std::path::Path::new(&file)
            .file_stem()?
            .to_str()?
            .to_string();
        Some(if module == "prelude" {
            p.name.clone()
        } else {
            format!("{module}.{}", p.name)
        })
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

    /// The run loop proper. Each iteration re-reads the current frame:
    /// calls and returns change it. It returns when the frame count drops
    /// to `self.stop`.
    fn step_loop(&mut self) -> Result<Ran, Stop> {
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
                // A statement's value is discarded: an Err is E0606 (02).
                Op::Pop => {
                    if let Some(Some(v)) = self.stack.pop()
                        && is_err(&v)
                    {
                        return Err(discarded_stop(pos, &v));
                    }
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
                    let eq = value::eq(&a, &b).map_err(|d| at(d, pos))?;
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
                    // cancellable and give its turn like a call.
                    if arg <= ip {
                        self.tick(pos)?;
                        if self.turn_due {
                            // Back at this jump after the others' turns.
                            self.frames[fi].ip = ip;
                            return Ok(Ran::Yielded(None));
                        }
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
                    if self.turn_due {
                        // Back at this call after the others' turns.
                        self.frames[fi].ip = ip;
                        return Ok(Ran::Yielded(None));
                    }
                    let tail = instr.op() == Op::TailCall;
                    match self.call_op(arg, pos, tail)? {
                        Entered::Frame => {}
                        // A native ran to completion.
                        Entered::Value(r) if tail => match self.ret_with(r)? {
                            Ran::Going => {}
                            ran => return Ok(ran),
                        },
                        Entered::Value(r) => self.push(r),
                        Entered::Call(f, n) => match self.issue(f, n)? {
                            Ran::Going => {}
                            ran => return Ok(ran),
                        },
                        Entered::Park(w) => return Ok(Ran::Parked(w)),
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
                    if !p.types[ti].matches(v) {
                        return Err(err(
                            "E0403",
                            pos,
                            vec![
                                Arg::Str(p.params[slot].name.clone()),
                                Arg::Str(p.types[ti].text().to_string()),
                                Arg::Str(value::type_name(v)),
                            ],
                        ));
                    }
                }
                Op::CheckReturn => {
                    let v = self.top();
                    let p = &self.frames[fi].proto;
                    if !p.types[arg].matches(v) {
                        return Err(err(
                            "E0404",
                            pos,
                            vec![
                                Arg::Str(p.types[arg].text().to_string()),
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
                // The slot is not read again: its value goes now (03).
                Op::ClearLocal => self.stack[base + arg] = None,
                Op::MoveLocal => {
                    let v = self.stack[base + arg].take();
                    self.stack.push(v);
                }
                Op::LoadGlobal => {
                    let v = self.frames[fi].globals.borrow()[arg].clone();
                    match v {
                        Some(v) => self.push(v),
                        None => return Err(self.unset_global(fi, arg, pos)),
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
                    value::check_chars(&s).map_err(|d| at(d, pos))?;
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
                    let v = self.pop();
                    match self.ret_with(v)? {
                        Ran::Going => {}
                        ran => return Ok(ran),
                    }
                } // Every opcode is handled: the match is exhaustive, so an
                  // "unknown opcode" case cannot arise.
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

impl Task<'_> {
    /// The name of global `i` of frame `fi`'s module, for an error. A
    /// function's proto has none of its module's names; the module's
    /// `<main>` frame does, and it is running when its constant is read
    /// too early (E0809).
    fn global_name(&self, fi: usize, i: usize) -> String {
        let g = &self.frames[fi].globals;
        self.frames[..=fi]
            .iter()
            .rev()
            .filter(|f| Rc::ptr_eq(&f.globals, g))
            .find_map(|f| f.proto.global_names.get(i))
            .cloned()
            .unwrap_or_else(|| "a global".to_string())
    }
}

impl Drop for Task<'_> {
    /// The end of a task is a value boundary (`Runtime` is `!Send`): the stack
    /// and frames drop here, on the run thread, before `world` releases it.
    fn drop(&mut self) {
        self.native = None;
        self.conts.clear();
        self.frames.clear();
        self.stack.clear();
    }
}
