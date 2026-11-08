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
mod closer;
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
pub(crate) mod pieces;
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
use fung_front::source::{FileId, Pos};
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
    IoRegs, IoStep, Waiter, YIELD_EVERY, drop_io_waits, park_io, park_until, park_until_then,
};
pub use call::MAX_FRAMES;
pub use channel::{Channel, Receiver, Sender};
pub(crate) use cleanup::run_with;
pub(crate) use closer::Closer;
pub use cont::{Cont, Flow, Next, Wake};
pub(crate) use deadline::run_timeout;
pub use link::RtProto;
pub(crate) use link::{Prelude, link};
pub(crate) use m4::{discarded, discarded_err, may_hold_err, not_result};
pub(crate) use m6::TestCase;
pub use matcher::RtPattern;
pub(crate) use offload::{Kind, alone, offload_kind};
pub(crate) use poller::Interest;
pub(crate) use pool::{Pool, Ticket};
pub use runtime::Runtime;
pub(crate) use sched::FiberId;
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

/// E0606 for an `Err` that a statement discarded, alone or in a list
/// (02). Out of the run loop, like every rare error path there: inlined,
/// such a path changes how the compiler lays out the whole loop and slows
/// the instructions that never take it (P12: `fib` lost 11% to two of
/// them).
#[cold]
#[inline(never)]
fn discarded_stop(pos: Pos, v: &Value) -> Option<Stop> {
    discarded_err(v).map(|d| at(d, pos))
}

/// An E0801 internal error without a position.
pub(crate) fn internal(args: Vec<Arg>) -> Stop {
    err("E0801", no_pos(), args)
}

/// One task's own state: an operand stack (whose first slots are the
/// entry frame's) and the call frames, with what the scheduler and the
/// natives need of it. A `Task` holds the fiber that has the turn; the
/// others wait in the scheduler (`sched.rs`).
pub(crate) struct Fiber {
    stack: Vec<Option<Value>>,
    frames: Vec<Frame>,
    /// Step natives waiting for a call they asked for (`cont.rs`). The
    /// run loop serves all of them: it returns when the frames are gone.
    conts: Vec<ContFrame>,
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
    /// Room for a call's named arguments while they are bound
    /// (`Task::bind_in_place`), kept from call to call (VM6).
    named: Vec<(crate::value::Sym, Value)>,
}

impl Fiber {
    /// A task that has not run yet, under `cancel`, with a number of its
    /// own.
    pub(super) fn new(shared: &Shared, cancel: Arc<CancelNode>) -> Fiber {
        Fiber {
            stack: Vec::new(),
            frames: Vec::new(),
            conts: Vec::new(),
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
            named: Vec::new(),
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
    /// The task that has the turn (`Fiber`).
    pub(crate) fiber: Fiber,
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
        Task {
            fiber: Fiber::new(shared, cancel),
            shared,
            world,
        }
    }

    /// Swaps this task's state with `f`'s: the scheduler gives a task its
    /// turn by swapping its fiber in, and takes it out again after.
    pub(super) fn exchange(&mut self, f: &mut Fiber) {
        std::mem::swap(&mut self.fiber, f);
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
    /// (`stdin.rs`). The line is bytes, as read (L4). The error: the
    /// reader thread cannot start.
    pub(crate) fn stdin_poll(&mut self) -> std::io::Result<Option<Option<Vec<u8>>>> {
        let me = self.fiber.waiter.clone();
        self.world.stdin.poll(&me)
    }

    /// A wait for a line that ended without one leaves the waiting list.
    pub(crate) fn stdin_leave(&mut self) {
        let me = self.fiber.waiter.clone();
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
        self.fiber.budget
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
    pub fn args(&self) -> &[Vec<u8>] {
        &self.world.args
    }

    /// The shared part, for the crate's boot steps (`crate::boot`).
    pub(crate) fn world(&mut self) -> &mut World {
        &mut self.world
    }

    /// The name of the native being called.
    pub fn calling_native(&self) -> Option<&'static str> {
        self.fiber.native.map(|n| n.name)
    }

    /// Whether the task runs a swap's function (`atom.rs`): it must not
    /// park there.
    pub(crate) fn in_pure(&self) -> bool {
        self.fiber.pure > 0
    }

    /// Runs `main` (a file body) as the entry frame, the root of a run.
    fn run_main(&mut self, main: Rc<RtProto>, globals: Globals) -> Result<Value, Stop> {
        self.fiber.stack = vec![None; main.num_locals];
        self.fiber.frames.push(Frame {
            proto: main,
            ip: 0,
            base: 0,
            closure: None,
            globals,
            tail_calls: 0,
            entry: None,
            tail_from: None,
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
            .fiber
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
        if matches!(self.fiber.todo, Some(Todo::Resume)) {
            self.fiber.todo = Some(match self.after_park() {
                Ok(()) => Todo::Resume,
                Err(e) => Todo::Unwind(self.mark(e)),
            });
        }
        #[cfg(test)]
        {
            self.fiber.loops += 1;
        }
        let turn = self.drive();
        #[cfg(test)]
        {
            self.fiber.loops -= 1;
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
        self.fiber.waiter.clone()
    }

    /// How many waits on descriptors are registered (test probes).
    #[cfg(test)]
    pub(crate) fn io_waiting_for_tests(&self) -> usize {
        self.world.io.len()
    }

    /// Whether the ready queue held a task twice when this turn began
    /// (test probes).
    #[cfg(test)]
    pub(crate) fn queue_dupes_for_tests(&self) -> bool {
        self.world.queue_dupes
    }

    /// A parked task resumed: its woken flag is taken first, so a wake
    /// that comes after this queues the task again whatever the
    /// continuation does next (a stop's unwind may park again: a job that
    /// is waited for, a scope draining its children). Then a cancelled
    /// task (or one past its node's deadline, which expires the node)
    /// unwinds; otherwise its waiter watches its node again, before the
    /// continuation looks.
    fn after_park(&mut self) -> Result<(), Stop> {
        self.fiber.waiter.take_woken();
        if self.fiber.cancel.is_cancelled() {
            return Err(self.unwound());
        }
        let now = std::time::Instant::now();
        if self.fiber.cancel.deadline_at().is_some_and(|at| now >= at)
            && self.fiber.cancel.expire_due(now)
        {
            return Err(self.unwound());
        }
        self.fiber.cancel.watch(&self.fiber.waiter);
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
        let mut todo = self.fiber.todo.take().unwrap_or(Todo::Loop);
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
                        self.fiber.yields += 1;
                    }
                    self.fiber.turn_due = false;
                    self.fiber.todo = Some(match call {
                        Some((f, n)) => Todo::Start(f, n),
                        None => Todo::Loop,
                    });
                    return Turn::Yielded;
                }
                Ok(Ok(Ran::Parked(w))) => {
                    #[cfg(test)]
                    {
                        self.fiber.parks += 1;
                    }
                    // A park gives the turn: one that ended on the call
                    // that parked is not owed again after the wake.
                    self.fiber.turn_due = false;
                    self.fiber.todo = Some(Todo::Resume);
                    return Turn::Parked(w);
                }
                Ok(Err(e)) => {
                    let e = self.mark(e);
                    if self.fiber.conts.is_empty() {
                        return Turn::Done(Err(e));
                    }
                    Todo::Unwind(e)
                }
                Err(payload) => {
                    let e = self.panicked(payload);
                    if self.fiber.conts.is_empty() {
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
                if let Some(c) = self.fiber.conts.last()
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
        if self.fiber.frames.last().is_some_and(|f| f.proto.builtin)
            && let Some(p) = self.user_pos()
        {
            d.pos = p;
        }
        d
    }

    /// Where the innermost frame of user code is (its current call), for
    /// an error raised inside an embedded extras file (`attach`).
    pub(crate) fn user_pos(&self) -> Option<Pos> {
        self.fiber.frames.iter().rev().find_map(|f| {
            if f.proto.builtin {
                f.tail_from.as_ref().and_then(|t| t.pos)
            } else {
                f.current_pos()
            }
        })
    }

    /// The function of a built-in module that user code called, which is
    /// running now (`fs.with_open`, a prelude one as `take`): what an
    /// error raised deep inside it is about (E0707). The frames remember
    /// it across the helpers it calls and tail-calls (`Frame::entry`).
    /// `None` when user code called the native itself.
    pub(crate) fn user_callee(&self) -> Option<String> {
        let i = self.fiber.frames.iter().rposition(|f| !f.proto.builtin)?;
        let p = self.fiber.frames.get(i + 1)?.entry.clone()?;
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
    /// One line per frame, innermost first, and after a frame the frame
    /// of user code its tail call into a built-in function replaced (L8).
    fn call_path(&self) -> Vec<CallSite> {
        let mut path = Vec::with_capacity(self.fiber.frames.len());
        for f in self.fiber.frames.iter().rev() {
            path.push(self.site(&f.proto, f.current_pos(), f.tail_calls));
            if let Some(t) = &f.tail_from {
                path.push(self.site(&t.proto, t.pos, t.tail_calls));
            }
        }
        path
    }

    /// A line of the call path: `proto` at `pos`, after `elided` tail calls.
    fn site(&self, proto: &RtProto, pos: Option<Pos>, elided: u64) -> CallSite {
        let (file, line) = match pos {
            Some(p) => (self.world.file_name(p.file).to_string(), p.line),
            None => (String::new(), 0),
        };
        let func = if proto.name.is_empty() {
            "<lambda>".to_string()
        } else {
            proto.name.clone()
        };
        CallSite {
            file,
            line,
            func,
            elided,
        }
    }

    fn push(&mut self, v: Value) {
        self.fiber.stack.push(Some(v));
    }

    /// Pops an operand. An empty slot here is a VM bug (see the module doc).
    fn pop(&mut self) -> Value {
        match self.fiber.stack.pop() {
            Some(Some(v)) => v,
            Some(None) => panic!("VM read an unassigned slot"),
            None => panic!("VM stack underflow"),
        }
    }

    fn top(&self) -> &Value {
        match self.fiber.stack.last() {
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
        if let Some((x, y)) = self.top_ints()
            && let Some(r) = int(x, y)
        {
            self.put_over_ints(Value::Int(r));
            return Ok(());
        }
        let b = self.pop();
        let a = self.pop();
        let r = f(&a, &b).map_err(|d| at(d, pos))?;
        self.push(r);
        Ok(())
    }

    /// The two operands on top of the stack when both are ints: the
    /// operators look at them where they lie (VM9).
    #[inline(always)]
    fn top_ints(&self) -> Option<(i64, i64)> {
        match self.fiber.stack.as_slice() {
            [.., Some(Value::Int(x)), Some(Value::Int(y))] => Some((*x, *y)),
            _ => None,
        }
    }

    /// Replaces the two ints on top of the stack (`top_ints`) with `v`.
    /// An int needs no drop, so they are forgotten rather than dropped:
    /// the run loop does not call the drop of a whole `Value` per
    /// operator (VM9).
    #[inline(always)]
    fn put_over_ints(&mut self, v: Value) {
        let s = &mut self.fiber.stack;
        std::mem::forget(s.pop());
        let n = s.len();
        std::mem::forget(s[n - 1].replace(v));
    }

    fn unary(&mut self, f: fn(&Value) -> Result<Value, Diag>, pos: Pos) -> Result<(), Stop> {
        let a = self.pop();
        let r = f(&a).map_err(|d| at(d, pos))?;
        self.push(r);
        Ok(())
    }

    #[inline(always)]
    fn compare(&mut self, pos: Pos, test: impl Fn(Ordering) -> bool) -> Result<(), Stop> {
        if let Some((x, y)) = self.top_ints() {
            self.put_over_ints(Value::Bool(test(x.cmp(&y))));
            return Ok(());
        }
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
    /// to zero.
    fn step_loop(&mut self) -> Result<Ran, Stop> {
        loop {
            let fi = self.fiber.frames.len() - 1;
            let f = &mut self.fiber.frames[fi];
            let ip = f.ip;
            let instr = f.proto.code[ip];
            let pos = f.proto.pos[ip];
            f.ip = ip + 1;
            let base = f.base;
            let arg = instr.arg();
            match instr.op() {
                Op::Const => {
                    let v = self.fiber.frames[fi].proto.consts[arg].clone();
                    self.push(v);
                }
                // A statement's value is discarded: an Err is E0606 (02).
                Op::Pop => {
                    if let Some(Some(v)) = self.fiber.stack.pop()
                        && may_hold_err(&v)
                        && let Some(stop) = discarded_stop(pos, &v)
                    {
                        return Err(stop);
                    }
                }
                Op::Add => self.binary(pos, i64::checked_add, value::add)?,
                Op::Sub => self.binary(pos, i64::checked_sub, value::sub)?,
                Op::Mul => self.binary(pos, i64::checked_mul, value::mul)?,
                Op::Div => self.binary(pos, |_, _| None, value::div)?,
                // Zero and overflow take the value layer's way to the error.
                Op::FloorDiv => self.binary(
                    pos,
                    |x, y| {
                        if y == 0 {
                            None
                        } else {
                            value::floor_div_i64(x, y).ok()
                        }
                    },
                    value::floor_div,
                )?,
                Op::Mod => self.binary(
                    pos,
                    |x, y| (y != 0).then(|| value::rem_i64(x, y)),
                    value::rem,
                )?,
                Op::Pow => self.binary(pos, |_, _| None, value::pow)?,
                Op::Neg => self.unary(value::neg, pos)?,
                Op::Not => self.unary(value::not, pos)?,
                Op::Eq | Op::Ne => {
                    if let Some((x, y)) = self.top_ints() {
                        self.put_over_ints(Value::Bool((x == y) == (instr.op() == Op::Eq)));
                        continue;
                    }
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
                    // cancellable and give its turn like a call. Kept
                    // for bytecode that might one day loop (VM12).
                    if arg <= ip {
                        debug_assert!(false, "a backward JUMP");
                        self.tick(pos)?;
                        if self.fiber.turn_due {
                            // Back at this jump after the others' turns.
                            self.fiber.frames[fi].ip = ip;
                            return Ok(Ran::Yielded(None));
                        }
                    }
                    self.fiber.frames[fi].ip = arg;
                }
                Op::JumpIfFalse => match self.pop() {
                    Value::Bool(true) => {}
                    Value::Bool(false) => self.fiber.frames[fi].ip = arg,
                    v => {
                        let tn = value::type_name(&v);
                        return Err(err("E0505", pos, vec![Arg::Str(tn)]));
                    }
                },
                Op::Call | Op::TailCall => {
                    self.tick(pos)?;
                    if self.fiber.turn_due {
                        // Back at this call after the others' turns.
                        self.fiber.frames[fi].ip = ip;
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
                    let c = self.make_closure(fi, arg, pos)?;
                    self.push(c);
                }
                Op::LoadCaptured => {
                    let v = self.fiber.frames[fi]
                        .closure
                        .as_ref()
                        .expect("LOADCAPTURED outside a closure")
                        .captured[arg]
                        .clone();
                    self.push(v);
                }
                Op::LoadSelf => {
                    let c = self.fiber.frames[fi]
                        .closure
                        .clone()
                        .expect("LOADSELF outside a closure");
                    self.push(Value::Closure(c));
                }
                Op::JumpIfSet => {
                    if self.fiber.stack.pop().flatten().is_some() {
                        self.fiber.frames[fi].ip = arg;
                    }
                }
                Op::CheckType => {
                    let (slot, ti) = (arg & 0xFFF, arg >> 12);
                    let v = self.fiber.stack[base + slot]
                        .as_ref()
                        .expect("CHECKTYPE on an unassigned slot");
                    let p = &self.fiber.frames[fi].proto;
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
                    let p = &self.fiber.frames[fi].proto;
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
                    let v = self.fiber.stack[base + arg].clone();
                    self.fiber.stack.push(v);
                }
                Op::StoreLocal => {
                    let v = self.fiber.stack.pop().expect("VM stack underflow");
                    self.fiber.stack[base + arg] = v;
                }
                // The slot is not read again: its value goes now (03).
                Op::ClearLocal => self.fiber.stack[base + arg] = None,
                Op::MoveLocal => {
                    let v = self.fiber.stack[base + arg].take();
                    self.fiber.stack.push(v);
                }
                Op::LoadGlobal => {
                    let v = self.fiber.frames[fi].globals.borrow()[arg].clone();
                    match v {
                        Some(v) => self.push(v),
                        None => return Err(self.unset_global(fi, arg, pos)),
                    }
                }
                Op::StoreGlobal => {
                    let v = self.pop();
                    // The old value drops after the table's borrow ends.
                    let old = self.fiber.frames[fi].globals.borrow_mut()[arg].replace(v);
                    drop(old);
                }
                Op::Concat => {
                    let start = self.fiber.stack.len() - arg;
                    let mut s = Vec::new();
                    for part in self.fiber.stack.drain(start..) {
                        let part = part.expect("CONCAT of an unassigned slot");
                        if let Some(tag) = value::result_tag(&part) {
                            let d = fung_front::diag::error(
                                "E0519",
                                pos,
                                vec![
                                    Arg::Str(tag.into()),
                                    Arg::Str("string interpolation".into()),
                                ],
                            );
                            return Err(Stop::Error(Box::new(d)));
                        }
                        value::write_for_print(&part, &mut s);
                    }
                    value::check_chars(&s).map_err(|d| at(d, pos))?;
                    self.push(Value::bytes(s));
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
    /// (03, model C). A global with no value is E0808 in the REPL (VM3:
    /// an input declared it, then failed); a script's resolve refuses it.
    fn make_closure(&self, fi: usize, child: usize, pos: Pos) -> Result<Value, Stop> {
        let f = &self.fiber.frames[fi];
        let proto = f.proto.protos[child].clone();
        let captured = proto
            .captures
            .iter()
            .map(|c| {
                Ok(match c.kind {
                    CaptureKind::Local => self.fiber.stack[f.base + c.index]
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
                    CaptureKind::Global => match f.globals.borrow()[c.index].clone() {
                        Some(v) => v,
                        None => return Err(self.unset_global(fi, c.index, pos)),
                    },
                })
            })
            .collect::<Result<_, Stop>>()?;
        Ok(Value::Closure(Rc::new(Closure {
            proto,
            captured,
            globals: f.globals.clone(),
        })))
    }
}

impl Task<'_> {
    /// The name of global `i` of frame `fi`'s module, for an error. A
    /// function's proto has none of its module's names; the module's
    /// `<main>` frame does, and it is running when its constant is read
    /// too early (E0809).
    fn global_name(&self, fi: usize, i: usize) -> String {
        let g = &self.fiber.frames[fi].globals;
        self.fiber.frames[..=fi]
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
        self.fiber.native = None;
        self.fiber.conts.clear();
        self.fiber.frames.clear();
        self.fiber.stack.clear();
    }
}
