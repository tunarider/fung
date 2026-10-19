//! Continuation frames (concurrency v2 R2; Lua 5.2's `lua_callk`): how a
//! native calls back into fung without nesting a run loop on the Rust
//! stack.
//!
//! A *step native* (`NativeBody::Step`) does not call a function itself.
//! It returns [`Flow::Call`]: the function to call, its arguments (pushed
//! on the task stack with `Task::push_arg`) and a box with the state to
//! continue from. The VM puts the box on the task's continuation list
//! (`Task::conts`, beside the frames) and calls the function in the same
//! run loop. When the call returns, the VM gives its value to
//! [`Cont::resume`]; when an error, a cancellation or an exit unwinds
//! through the native, it gives the stop to [`Cont::unwind`]. Either says
//! what is next: another call or the native's result.
//!
//! Everything a waiting task holds is then on the heap (frames, values,
//! boxes), so a scheduler can put the task aside and go on with it later
//! (R1). A side benefit: deep recursion through `map` or `fold` no longer
//! grows the Rust stack.
//!
//! **Where a box sits.** `depth` is the frame count when the native was
//! called: the box is due when the frames are back to that count. Two
//! boxes can sit at one depth (a step native that is itself the callback
//! of another, `map(fs, scope)`); the later one is due first. A run loop
//! only serves the boxes put on the list after it started (`floor`).
//! Today a task has one run loop at a time and entry points start on an
//! empty list (`Task::call_root` asserts it), so `floor` is always 0: it
//! is a guard, so that a second loop could never resume a box of another.
//!
//! The first import of a module waits the same way for the module's body
//! (`module.rs`), with no native.

use std::time::Instant;

use fung_front::source::Pos;

use super::{Stop, Task};
use crate::value::{Native, Value};

/// What a native's continuation does next.
pub enum Next {
    /// The native's result.
    Done(Value),
    /// Call `f` with the `n` arguments the continuation pushed with
    /// `Task::push_arg`, then resume with its value.
    Call(Value, usize),
    /// Park the task until it is woken: the continuation registered the
    /// task's waiter on what it waits for (R1a). Then `resume` gets
    /// `nil` and looks again.
    Park(Wake),
}

/// What a step native does when it is called: give its result at once,
/// call `f` with the `n` arguments it pushed and continue with the box,
/// or park with the box (`Next::Park`).
pub enum Flow {
    Done(Value),
    Call(Box<dyn Cont>, Value, usize),
    Park(Box<dyn Cont>, Wake),
}

/// What else, besides a waker, ends a park.
#[derive(Clone, Copy, Debug, Default)]
pub struct Wake {
    /// The continuation's own deadline (`sleep`): the run loop resumes it
    /// then, and it sees the time. A deadline over the task's node is
    /// added by the run loop.
    pub until: Option<Instant>,
    /// A timer or the world outside may wake it (`sleep`, the stop
    /// channel): not a deadlock while it waits.
    pub timed: bool,
}

/// The state of a step native between the calls it makes.
pub trait Cont {
    /// The call the native asked for returned `v`, or, after a park, the
    /// task was woken (`v` is `nil`): look again and say what is next.
    ///
    /// While `resume` runs, the box is out of the continuation list. A
    /// panic inside it unwinds past this native without calling its
    /// `unwind`, so `resume` must leave nothing to undo before a call that
    /// can panic (restore swapped nodes and counters first).
    fn resume(&mut self, t: &mut Task<'_>, v: Value) -> Result<Next, Stop>;

    /// `stop` (a runtime error, a cancellation, an exit) is unwinding
    /// through the native. By default it goes on unwinding.
    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        let _ = t;
        Err(stop)
    }
}

/// A step native waiting for the call it asked for.
pub(super) struct ContFrame {
    /// The frame count when the native was called: the box is due when
    /// the frames are back to it.
    pub depth: usize,
    /// The stack height below the native's pushed arguments.
    pub height: usize,
    /// The native's call site, for errors it raises.
    pub pos: Pos,
    /// The native, or none for a module body (`module.rs`).
    pub native: Option<&'static Native>,
    /// Called by TAILCALL: its result returns from the calling frame.
    pub tail: bool,
    pub k: Box<dyn Cont>,
}
