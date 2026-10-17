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
//!   channel `ch`.
//! - `check_swap(a, f)`: checks a `swap`'s arguments as a native would
//!   (E0403) and that the current task is not inside `a` (E0704, `swap`
//!   inside its own `f`).
//! - `agent_ask(a, request, reply)`: sends `request` to `a` and waits
//!   for the answer on `reply`, holding the handle `a` until then (the
//!   caller's may be the last copy, and dropping it would close the
//!   channel the answer is coming through). It records the asking
//!   task's cancellation node on `reply`.
//! - `try_call(f, x, reply = nil)`: `Ok(f(x))`, or `Err(e)` where `e` is
//!   what stopped `f`, as an opaque value (`Value::Caught`): an error, or
//!   an `exit`. With a `reply` from `agent_ask`, `f` runs following the
//!   task that asked (below).
//! - `set_asker(ch)`, `describe(e)` (stage 4d, the `http` server): see
//!   their functions.
//! - `raise(e)`: stops again as `e` did: an `exit` exits, an error is
//!   raised with its code and message at the position of the call (the
//!   original position and call path are gone, except for E0704, which
//!   keeps the position of the inner `swap`).
//! - `send_unchecked(ch, v)`: `send` without the check that a channel is
//!   not sent into itself (stage 3b Task 5), for the agent's own
//!   requests (`conc.rs`).
//! - `agent_busy(ch, on, state = nil)`: the agent's task marks itself as
//!   serving a request on `ch` with `state` (on) or not (off). The record
//!   is kept on the channel, not the handle (see the rule below).
//! - `agent_peek(a)`: `Ok(state)` when the current task is inside `a`
//!   right now (the task serving `a`'s request, or a task it spawned, at
//!   any depth, while serving it), with the state it serves with;
//!   `Err(nil)` otherwise. Checking and reading are one call on purpose:
//!   the turn can pass between two calls, and `f` can end there.
//!
//! **Inside an agent.** `agent_busy` makes a fresh `Mark` per request,
//! records it on the channel and adds it to the serving task's marks
//! (`Task::serving`); `spawn` copies a task's marks into the task it
//! starts (`spawn_background` does not: an agent made inside `f` is its
//! own). A task is inside `a` when the mark on `a`'s channel is among its
//! marks. A task `f` spawned that outlives that request holds an old mark
//! and so is outside again, waiting its turn like any other.
//!
//! **The agent's loop holds only the request channel, never a handle.**
//! The handle is the closer: while any copy is alive the channel stays
//! open, so a loop holding one would keep itself alive forever and an
//! agent nobody can reach would never end. Everything the loop needs
//! (`agent_busy`, `send_unchecked`, `recv`) therefore takes the channel.
//!
//! **`f` follows the task that asked.** Go's atom ran `f` on the
//! caller's task, so cancelling the caller (an `exit`, a deadlock report,
//! `cancel(t)`) stopped `f`. An agent runs `f` on its own task, so
//! `try_call` runs it under a node of its own that is below both the
//! agent's node and the asking task's node (`CancelNode::also_under`):
//! cancelling the asker cancels `f` (and what `f` spawned), while the
//! agent's task itself is not cancelled. `try_call` then answers
//! `Err(<error>)` with the cancellation, nobody reads it, the state stays
//! and the loop serves the next request. A request whose asker was
//! cancelled before the agent took it up is skipped (`f` is not
//! called). After `f` the node lives as long as the tasks `f` spawned
//! (`end_when_empty`), still below the asker: they end when the asker
//! does, as under Go's atom. Cancelling the agent's own node (after the
//! exit handlers) still unwinds the agent.
//!
//! A task the agent's `f` spawns is a background task too (`task.rs`):
//! it does not keep the program alive, and it ends with the asker or
//! with the agent's root, whichever comes first.

use std::rc::Rc;
use std::sync::Arc;

use fung_front::diag::Diag;

use super::block::cancelled;
use super::channel::{self, Channel, Received};
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

/// What `try_call` caught: an error (its code, arguments, help and
/// notes; no underline or call path, and a position only for E0704) or
/// an `exit`.
pub enum Caught {
    Error(Diag),
    Exit(i64),
}

impl std::fmt::Debug for Caught {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Caught::Error(d) => write!(f, "<error {}>", d.code),
            Caught::Exit(n) => write!(f, "<exit {n}>"),
        }
    }
}

/// One request being served: the identity tasks inside it share
/// (module doc, "Inside an agent").
pub(crate) struct Mark;

/// The request an agent is serving and the state it serves with
/// (`agent_busy`). Kept on the request channel; a fung value, so only
/// the token's holder touches it.
pub(crate) struct Serving {
    pub(crate) mark: Arc<Mark>,
    pub(crate) state: Value,
}

fn need_channel(v: &Value) -> Result<&Rc<Channel>, Stop> {
    match v {
        Value::Channel(c) => Ok(c),
        _ => Err(nat::wrong_arg("ch", "channel", v)),
    }
}

fn need_channel_named<'v>(name: &str, v: &'v Value) -> Result<&'v Rc<Channel>, Stop> {
    match v {
        Value::Channel(c) => Ok(c),
        _ => Err(nat::wrong_arg(name, "channel", v)),
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

/// `check_swap(a, f)`: E0403 when `a` is not an agent or `f` not a
/// function (the order of Go's atom `swap`); E0704 when the current task
/// is inside `a` (its own `f`, or a task that `f` spawned): it would
/// wait for itself.
pub(crate) fn check_swap(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let a = need_agent(&args[0])?;
    nat::need_fn("f", &args[1])?;
    if serving_state(t, a).is_some() {
        // At the user's inner `swap(`, which `try_call` keeps (Go's atom
        // reported its E0703 there). An inner `swap` in tail position
        // replaced the user's frame, so no frame of user code is left on
        // this task: the error then gets the position of this call, in
        // the embedded file, which `try_call` drops, and it lands on the
        // caller's `swap(` like any error of `f`.
        let pos = t.user_pos().unwrap_or_else(no_pos);
        return Err(Stop::Error(Box::new(fung_front::diag::error(
            "E0704",
            pos,
            Vec::new(),
        ))));
    }
    Ok(Value::Nil)
}

/// `agent_ask(a, request, reply)`: sends `request` on `a`'s channel and
/// returns what arrives on `reply`. The arguments own a copy of the
/// handle until the answer is in, so the channel stays open even when
/// the caller passed its last copy (`deref(agent(5))`). A closed channel
/// either way is an internal error: the handle keeps the request channel
/// open, and the agent always answers. `reply` carries this task's
/// cancellation node to the agent, whose `f` runs below it (`try_call`).
pub(crate) fn agent_ask(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let a = need_agent(&args[0])?.clone();
    let reply = need_channel(&args[2])?.clone();
    reply.set_asker(t.cancel.clone());
    if !channel::send(t, a.channel(), args[1].clone())? {
        return Err(internal(vec![fung_front::diag::Arg::Str(
            "an agent's request channel is closed".into(),
        )]));
    }
    match channel::recv(t, &reply, None)? {
        Received::Value(v) => Ok(v),
        _ => Err(internal(vec![fung_front::diag::Arg::Str(
            "an agent's reply channel is closed".into(),
        )])),
    }
}

/// `try_call(f, x, reply = nil)`: `Ok(f(x))`, or `Err(<error>)` for what
/// stopped `f`: a fung error or an `exit` (handed back to the caller's
/// `swap`, so the agent's loop keeps serving).
///
/// With a `reply` that `agent_ask` sent, `f` runs under a node below
/// this task's and the asking task's (module doc, "`f` follows the task
/// that asked"). The asker's cancellation stopping `f` (E0701, or E0702
/// after a deadlock report) is caught too, as "not applied"; an asker
/// already cancelled means `f` is not called at all. Cancellation of
/// this task itself (E0701, E0702) unwinds on through.
pub(crate) fn try_call(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let reply = args.pop().expect("reply");
    let x = args.pop().expect("x");
    let f = args.pop().expect("f");
    nat::need_fn("f", &f)?;
    let asker = match &reply {
        Value::Nil => None,
        v => need_channel_named("reply", v)?.asker(),
    };
    let asked = asker.is_some();
    let own = t.cancel.clone();
    let r = match asker {
        None => t.call(&f, vec![x]),
        Some(asker) => {
            let node = own.child();
            node.also_under(&asker);
            let r = if node.is_cancelled() {
                // Asked by a task cancelled before the agent took the
                // request up: skipped.
                Err(cancelled(no_pos()))
            } else {
                t.cancel = node.clone();
                let r = t.call(&f, vec![x]);
                t.cancel = own.clone();
                r
            };
            node.end_when_empty();
            r
        }
    };
    let caught = match r {
        Ok(v) => return Ok(nat::ok(v)),
        Err(Stop::Error(d)) if asked && is_cancel(&d) && !own.is_cancelled() => {
            // The asker was cancelled: the update is not applied. The
            // answer goes to a task that no longer waits for it.
            let mut d = *d;
            d.pos = no_pos();
            d.len = 0;
            d.call_path.clear();
            Caught::Error(d)
        }
        Err(Stop::Error(d)) if !is_cancel(&d) => {
            let mut d = *d;
            // E0704 keeps where the inner `swap` was when that is user
            // code (`check_swap`); every other error lands on the
            // caller's `swap(`.
            if d.code != "E0704" || t.world.is_embedded(d.pos.file) {
                d.pos = no_pos();
                d.len = 0;
            }
            d.call_path.clear();
            Caught::Error(d)
        }
        Err(Stop::Exit(n)) => Caught::Exit(n),
        Err(e) => return Err(e),
    };
    Ok(nat::tag("Err", vec![Value::Caught(Rc::new(caught))]))
}

/// `set_asker(ch)` (stage 4d Task 5, the `http` server): records this
/// task's cancellation node on `ch` as `agent_ask` does on a reply
/// channel. The record stays (`try_call` copies it), so every `try_call`
/// with `ch` runs its `f` below this task: the server's task marks one
/// channel and each request's handler follows it, as Go runs handlers
/// under the server's context.
pub(crate) fn set_asker(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    need_channel_named("ch", &args[0])?.set_asker(t.cancel.clone());
    Ok(Value::Nil)
}

/// `describe(e)` (stage 4d Task 5): what a value `try_call` caught was,
/// as `{code, text}`: an error's code and headline (Go's `Error()`), or
/// `{code: "exit", text: "exit(<n>)"}`. The `http` server reports a
/// failing handler with it and tells a shutdown (an exit, E0701, E0702)
/// from a bug.
pub(crate) fn describe(_: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let Value::Caught(c) = &args[0] else {
        return Err(nat::wrong_arg("e", "error", &args[0]));
    };
    let (code, text) = match &**c {
        Caught::Exit(n) => ("exit".to_string(), format!("exit({n})")),
        Caught::Error(d) => (d.code.clone(), fung_front::diag::headline(d)),
    };
    let mut m = crate::value::MapBuilder::default();
    m.set(Rc::new("code".to_string()), Value::str(&code));
    m.set(Rc::new("text".to_string()), Value::str(&text));
    Ok(Value::Map(Rc::new(m.finish())))
}

/// `raise(e)`: stops as `e` did. An error without a position gets the
/// position of this call (and, when the call is inside an embedded
/// extras file, of the nearest user frame) and the call path of this
/// task; one with a position (E0704) keeps it, with this task's call
/// path.
pub(crate) fn raise(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    match &args[0] {
        Value::Caught(c) => match &**c {
            Caught::Exit(n) => Err(Stop::Exit(*n)),
            Caught::Error(d) => {
                let mut d = d.clone();
                if !d.pos.is_zero() {
                    // A recorded call path stops the VM from moving the
                    // position to the nearest user frame (`attach`).
                    d.call_path = t.call_path();
                }
                Err(Stop::Error(Box::new(d)))
            }
        },
        v => Err(nat::wrong_arg("e", "error", v)),
    }
}

/// `agent_busy(ch, on, state = nil)`: marks the current task as serving a
/// request on `ch` with `state` (a fresh `Mark`, also added to the task's
/// marks so the tasks it spawns are inside too), or clears the mark.
///
/// A mark that is never cleared (the agent was cancelled in the middle
/// of `f`) stays on the channel and keeps its state alive until the
/// channel drops. Accepted: no live task holds that mark any more, so
/// `agent_peek` finds nothing, and the state goes with the channel (with the
/// agent, at the latest at the program's end).
pub(crate) fn agent_busy(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let state = args.pop().expect("state");
    let on = match &args[1] {
        Value::Bool(b) => *b,
        v => return Err(nat::wrong_arg("on", "bool", v)),
    };
    let ch = need_channel(&args[0])?;
    let serving = on.then(|| {
        let mark = Arc::new(Mark);
        t.serving.push(mark.clone());
        Serving { mark, state }
    });
    let old = ch.set_serving(serving);
    if !on && let Some(s) = &old {
        t.serving.retain(|m| !Arc::ptr_eq(m, &s.mark));
    }
    // The old record (and its state) drops here, after the channel's
    // borrow ended.
    drop(old);
    Ok(Value::Nil)
}

/// The state `a`'s request is served with, if the current task is
/// inside it.
fn serving_state(t: &Task<'_>, a: &Agent) -> Option<Value> {
    a.channel().serving_state(&t.serving)
}

/// `agent_peek(a)`: `Ok(state)` when the current task is inside `a` (the
/// state its current request is served with), `Err(nil)` otherwise (a
/// tag, since the state may be nil). One call checks and reads: between
/// two calls the turn can pass, and the agent can end its `f` there.
pub(crate) fn agent_peek(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let a = need_agent(&args[0])?;
    Ok(match serving_state(t, a) {
        Some(state) => nat::ok(state),
        None => nat::tag("Err", vec![Value::Nil]),
    })
}
