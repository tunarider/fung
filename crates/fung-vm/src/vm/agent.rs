//! What agents are built from (stage 3b, Rust only): the members of
//! `_native.conc` that only `extras/agent.fung` uses. An agent is a
//! background task (`task.rs`) that holds a state and serves requests
//! from a channel; its handle is a value that closes that channel when
//! its last copy drops, so an agent nobody can reach ends.
//!
//! - `spawn_background(f)`: `f` on a background task. It does not keep
//!   the program alive, is left out of the deadlock count while it
//!   cannot help (`deadlock.rs`), is cancelled after the exit handlers,
//!   and its failure is never reported as unretrieved.
//! - `agent_handle(ch)`: the handle (`Value::Agent`) over the request
//!   channel `ch`; `agent_channel(a)` gives `ch` back.
//! - `swap_channel(a, f)`: `ch` for a `swap` request, after checking the
//!   arguments as a native would (E0403) and that the current task is
//!   not serving a request of `a` (E0704, `swap` inside its own `f`).
//! - `try_call(f, x)`: `Ok(f(x))`, or `Err(e)` where `e` is the caught
//!   error as an opaque value (`Value::Caught`). Cancellation (E0701,
//!   E0702) and `exit` are not caught.
//! - `raise(e)`: raises the caught error again, with its code and
//!   message, at the position of the call (the original position and
//!   call path are gone).
//! - `send_unchecked(ch, v)`: `send` without the check that a channel is
//!   not sent into itself (stage 3b Task 5), for the agent's own
//!   requests (`conc.rs`).
//! - `agent_busy(ch, on, state = nil)`: the agent's task marks itself as
//!   serving a request on `ch` with `state` (on) or not (off). The mark
//!   is kept on the channel, not the handle (see the rule below).
//! - `in_agent(a)`: whether the current task is the one serving a request
//!   of `a` right now; `agent_state(a)` is then the state it serves with.
//!
//! **The agent's loop holds only the request channel, never a handle.**
//! The handle is the closer: while any copy is alive the channel stays
//! open, so a loop holding one would keep itself alive forever and an
//! agent nobody can reach would never end. Everything the loop needs
//! (`agent_busy`, `send_unchecked`, `recv`) therefore takes the channel.
//!
//! A task the agent's `f` spawns is a background task too (`task.rs`):
//! it ends with the agent's root, not the program's.

use std::rc::Rc;
use std::sync::Arc;

use fung_front::diag::Diag;

use super::block::Waiter;
use super::channel::Channel;
use super::task::is_cancel;
use super::{Stop, Task, internal, no_pos, token};
use crate::native::nat;
use crate::value::{Value, drop_children, keep};

/// An agent's handle: the closer of its request channel. Two handles are
/// equal only when they are the same handle; it prints as `<agent>`
/// (the state is the agent's, and reading it means asking the agent).
///
/// The agent's own loop must hold only the channel, never a handle: a
/// handle it held would keep the channel open, and the agent alive,
/// forever (module doc).
pub struct Agent {
    /// The request channel; taken out (and closed) when the handle drops.
    channel: Option<Rc<Channel>>,
}

impl Agent {
    /// The request channel.
    pub(crate) fn channel(&self) -> &Rc<Channel> {
        self.channel
            .as_ref()
            .expect("an agent handle without its channel")
    }

    /// Closes the request channel and moves it out for `release`
    /// (`value/release.rs`): closing wakes the agent's loop, which sees
    /// `Err({kind: 'closed})` and ends.
    pub(crate) fn spill(&mut self, stack: &mut Vec<Value>) {
        if let Some(ch) = self.channel.take() {
            ch.close();
            keep(stack, Value::Channel(ch));
        }
    }
}

impl std::fmt::Debug for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<agent>")
    }
}

/// A handle is a value: its last copy drops on the token's holder, which
/// closes the channel as `close(ch)` does (`Channel::close` wakes the
/// waiting tasks, a change only the token's holder may make).
impl Drop for Agent {
    fn drop(&mut self) {
        token::assert_holds_token();
        drop_children(|stack| self.spill(stack));
    }
}

/// An error `try_call` caught: its code, arguments, help and notes,
/// without the position, underline or call path it had.
pub struct Caught(Diag);

impl std::fmt::Debug for Caught {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<error {}>", self.0.code)
    }
}

/// The task serving a request of an agent and the state it serves with
/// (`agent_busy`). Kept on the request channel; a fung value, so only
/// the token's holder touches it.
pub(crate) struct Serving {
    /// The serving task, by its waiter (a task keeps one for its life).
    pub(crate) waiter: Arc<Waiter>,
    pub(crate) state: Value,
}

fn need_channel(v: &Value) -> Result<&Rc<Channel>, Stop> {
    match v {
        Value::Channel(c) => Ok(c),
        _ => Err(nat::wrong_arg("ch", "channel", v)),
    }
}

fn need_agent(v: &Value) -> Result<&Rc<Agent>, Stop> {
    match v {
        Value::Agent(a) => Ok(a),
        _ => Err(nat::wrong_arg("a", "agent", v)),
    }
}

/// `spawn_background(f)`: the background task's handle.
pub(crate) fn spawn_background(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let f = args.into_iter().next().expect("one argument");
    nat::need_fn("f", &f)?;
    t.spawn_background(f)
}

/// `agent_handle(ch)`: a new handle over `ch`.
pub(crate) fn agent_handle(_: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let ch = need_channel(&args[0])?.clone();
    Ok(Value::Agent(Rc::new(Agent { channel: Some(ch) })))
}

/// `agent_channel(a)`: the handle's request channel.
pub(crate) fn agent_channel(_: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    Ok(Value::Channel(need_agent(&args[0])?.channel().clone()))
}

/// `swap_channel(a, f)`: the request channel for `swap(a, f)`. E0403 when
/// `a` is not an agent or `f` not a function (the order of Go's atom
/// `swap`); E0704 when the current task is serving a request of `a`,
/// that is, inside `a`'s own `f` (it would wait for itself).
pub(crate) fn swap_channel(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let a = need_agent(&args[0])?;
    nat::need_fn("f", &args[1])?;
    if serving_state(t, a).is_some() {
        return Err(nat::fail("E0704", Vec::new()));
    }
    Ok(Value::Channel(a.channel().clone()))
}

/// `try_call(f, x)`: `Ok(f(x))` or `Err(<error>)`. Only fung errors are
/// caught: `exit` (`Stop::Exit`) and cancellation (E0701, or E0702 for a
/// task woken by the deadlock report) unwind on through.
pub(crate) fn try_call(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let x = args.pop().expect("x");
    let f = args.pop().expect("f");
    nat::need_fn("f", &f)?;
    match t.call(&f, vec![x]) {
        Ok(v) => Ok(nat::ok(v)),
        Err(Stop::Error(d)) if !is_cancel(&d) => {
            let mut d = *d;
            d.pos = no_pos();
            d.len = 0;
            d.call_path.clear();
            Ok(nat::tag("Err", vec![Value::Caught(Rc::new(Caught(d)))]))
        }
        Err(e) => Err(e),
    }
}

/// `raise(e)`: the caught error again. It has no position, so the VM
/// gives it the position of this call (and, when the call is inside an
/// embedded extras file, of the nearest user frame) and the call path of
/// this task.
pub(crate) fn raise(_: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    match &args[0] {
        Value::Caught(c) => Err(Stop::Error(Box::new(c.0.clone()))),
        v => Err(nat::wrong_arg("e", "error", v)),
    }
}

/// `agent_busy(ch, on, state = nil)`: marks the current task as serving a
/// request on `ch` with `state`, or clears the mark.
///
/// A mark that is never cleared (the loop's `f` exited, or the agent was
/// cancelled in the middle of it) stays on the channel and keeps its
/// state alive until the channel drops. Accepted: the dead task's
/// identity never matches again, so `in_agent` stays false, and the
/// state goes with the channel (with the agent, at the latest at the
/// program's end).
pub(crate) fn agent_busy(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let state = args.pop().expect("state");
    let on = match &args[1] {
        Value::Bool(b) => *b,
        v => return Err(nat::wrong_arg("on", "bool", v)),
    };
    let ch = need_channel(&args[0])?;
    let serving = on.then(|| Serving {
        waiter: t.waiter.clone(),
        state,
    });
    // The old mark (and its state) drops here, after the channel's
    // borrow ended.
    drop(ch.set_serving(serving));
    Ok(Value::Nil)
}

/// The state `a`'s request is served with, if the current task serves it.
fn serving_state(t: &Task<'_>, a: &Agent) -> Option<Value> {
    a.channel().serving_state(&t.waiter)
}

/// `in_agent(a)`: whether the current task is serving a request of `a`.
pub(crate) fn in_agent(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let a = need_agent(&args[0])?;
    Ok(Value::Bool(serving_state(t, a).is_some()))
}

/// `agent_state(a)`: the state the current task serves `a`'s request
/// with; an internal error anywhere else (`extras/agent.fung` asks
/// `in_agent` first).
pub(crate) fn agent_state(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let a = need_agent(&args[0])?;
    serving_state(t, a).ok_or_else(|| {
        internal(vec![fung_front::diag::Arg::Str(
            "agent_state outside the agent".into(),
        )])
    })
}
