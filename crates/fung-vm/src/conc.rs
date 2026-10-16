//! The concurrency natives (05; Go `vm.Concurrency()`), all real since
//! stage 3a: `spawn`, `await`, `cancel`, `sleep`,
//! `_native.conc.spawn_awaited`, `channel`, `send`, `recv` and `close`, on
//! `crate::vm`'s tasks and blocking point, and the Rust-only members of
//! `_native.conc` agents are built from (`crate::vm::agent`, stage 3b).
//! Go's `atom`, `swap` and `deref` are not here: stage 3b replaced atoms
//! with agents, whose `agent`, `swap` and `deref` are fung
//! (`extras/rust/agent.fung`). The parameter tables are Go's
//! (`spec/builtins.txt`), so arity and named-argument errors, which the
//! VM reports before the native runs, are the same as Go's.

use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Duration;

use fung_front::diag::Arg;

use crate::native::nat::{self, native, opt, param};
use crate::value::{Lit, Module, Native, NativeFn, Value, format};
use crate::vm::agent;
use crate::vm::channel::{self, Channel, Received};
use crate::vm::{Stop, Task, block, err_kind};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The prelude's concurrency natives (Go `vm.Concurrency()`): `spawn`
/// `await` `cancel` `channel` `send` `recv` `close` `sleep`, then the
/// members only `_native.conc` holds.
fn all() -> Vec<(&'static str, Value)> {
    NATIVES
        .get_or_init(|| {
            let tables: Vec<(&'static str, Vec<_>)> = vec![
                ("sleep", vec![param("d")]),
                ("spawn", vec![param("f")]),
                ("await", vec![param("t"), opt("timeout", Lit::Nil)]),
                ("cancel", vec![param("t")]),
                ("channel", vec![opt("n", Lit::Int(0))]),
                ("send", vec![param("ch"), param("v")]),
                ("recv", vec![param("ch"), opt("timeout", Lit::Nil)]),
                ("close", vec![param("ch")]),
                ("spawn_awaited", vec![param("f")]),
                // Rust only (stage 3b, `crate::vm::agent`): the members
                // of `_native.conc` that `extras/rust/agent.fung` uses.
                ("spawn_background", vec![param("f")]),
                ("agent_handle", vec![param("ch")]),
                ("check_swap", vec![param("a"), param("f")]),
                (
                    "agent_ask",
                    vec![param("a"), param("request"), param("reply")],
                ),
                (
                    "try_call",
                    vec![param("f"), param("x"), opt("reply", Lit::Nil)],
                ),
                ("raise", vec![param("e")]),
                ("send_unchecked", vec![param("ch"), param("v")]),
                (
                    "agent_busy",
                    vec![param("ch"), param("on"), opt("state", Lit::Nil)],
                ),
                ("agent_peek", vec![param("a")]),
                // Rust only (stage 4d Task 5): what `extras/rust/http.fung`
                // runs its handlers with.
                ("set_asker", vec![param("ch")]),
                ("describe", vec![param("e")]),
            ];
            tables
                .into_iter()
                .map(|(name, params)| {
                    let f: NativeFn = match name {
                        "sleep" => sleep,
                        "spawn" => spawn,
                        "await" => await_fn,
                        "cancel" => cancel,
                        "spawn_awaited" => spawn_awaited,
                        "channel" => channel_fn,
                        "send" => send,
                        "recv" => recv,
                        "close" => close,
                        "spawn_background" => agent::spawn_background,
                        "agent_handle" => agent::agent_handle,
                        "check_swap" => agent::check_swap,
                        "agent_ask" => agent::agent_ask,
                        "try_call" => agent::try_call,
                        "raise" => agent::raise,
                        "send_unchecked" => send_unchecked,
                        "agent_busy" => agent::agent_busy,
                        "agent_peek" => agent::agent_peek,
                        "set_asker" => agent::set_asker,
                        "describe" => agent::describe,
                        _ => unreachable!("no native {name}"),
                    };
                    (name, native(name, f, params))
                })
                .collect()
        })
        .iter()
        .map(|(n, f)| (*n, Value::Native(f)))
        .collect()
}

/// The members of `_native.conc`: Go's `spawn_awaited`, then the Rust
/// only ones. The front end learns the Rust-only ones from
/// `RUST_MODULE_MEMBERS` in `crates/fung-front/src/builtins.rs` (the
/// differences from `spec/builtins.txt`); the boot test
/// `native_modules_match_the_spec` keeps the two lists, and the
/// parameter tables, in sync.
const MODULE_ONLY: &[&str] = &[
    "spawn_awaited",
    "spawn_background",
    "agent_handle",
    "check_swap",
    "agent_ask",
    "try_call",
    "raise",
    "send_unchecked",
    "agent_busy",
    "agent_peek",
    "set_asker",
    "describe",
];

/// The prelude's concurrency natives, without the members only
/// `_native.conc` holds.
pub fn natives() -> Vec<(&'static str, Value)> {
    all()
        .into_iter()
        .filter(|(n, _)| !MODULE_ONLY.contains(n))
        .collect()
}

/// `_native.conc` (Go `vm.NativeModules()`): `spawn_awaited`, which
/// `extras/prelude.fung` uses for `fan_in` and `pmap`, and the helpers
/// `extras/rust/agent.fung` is built from (stage 3b, `crate::vm::agent`).
pub fn module() -> Rc<Module> {
    let mut m = Module::new("conc", "_native.conc");
    for (name, v) in all() {
        if MODULE_ONLY.contains(&name) {
            m.set(name, v);
        }
    }
    Rc::new(m)
}

/// A duration argument as a wait: zero for zero and negative ones.
fn wait_of(d: i64) -> Duration {
    Duration::from_nanos(d.max(0) as u64)
}

/// `sleep(d)` (Go `sleepFn`): waits for `d`; zero and negative durations
/// return at once. A cancelled task wakes and unwinds with E0701.
fn sleep(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let d = match &args[0] {
        Value::Dur(d) => *d,
        v => return Err(nat::wrong_arg("d", "dur", v)),
    };
    if d > 0 {
        block(t, |_, _| None::<()>, Some(wait_of(d)))?;
    }
    Ok(Value::Nil)
}

/// `spawn(f)` (Go `spawnFn`): runs `f` on a new task below this one.
fn spawn(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    spawn_with(t, args, false)
}

/// `_native.conc.spawn_awaited(f)`: a spawn whose task counts as awaited
/// from the start (`pmap`, `fan_in`).
fn spawn_awaited(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    spawn_with(t, args, true)
}

fn spawn_with(t: &mut Task<'_>, args: Vec<Value>, awaited: bool) -> Result<Value, Stop> {
    let f = args.into_iter().next().expect("one argument");
    nat::need_fn("f", &f)?;
    t.spawn(f, awaited)
}

/// `await(t, timeout: nil)` (Go `awaitFn`): the task's result, kept for
/// later calls; `Err({kind: 'timeout})` when the timeout passes first (the
/// task keeps running).
fn await_fn(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let Value::Task(h) = &args[0] else {
        return Err(nat::wrong_arg("t", "task", &args[0]));
    };
    let timeout = timeout_arg(&args[1])?;
    let h = h.clone();
    // Getting the result retrieves a failure in it; a timeout does not.
    let ready = |_: &mut _, me: &_| {
        let r = h.retrieve();
        if r.is_none() {
            h.state.waiters.borrow_mut().add(me);
        }
        r
    };
    match block(t, ready, timeout)? {
        Some(v) => Ok(v),
        None => Ok(err_kind("timeout")),
    }
}

/// `cancel(t)` (Go `cancelFn`): cancels the task and every task below it;
/// a finished task is left as it is.
fn cancel(_: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let Value::Task(h) = &args[0] else {
        return Err(nat::wrong_arg("t", "task", &args[0]));
    };
    h.cancel.cancel();
    Ok(Value::Nil)
}

/// A `timeout: dur | nil` argument (Go `timeoutArg`).
fn timeout_arg(v: &Value) -> Result<Option<Duration>, Stop> {
    match v {
        Value::Nil => Ok(None),
        Value::Dur(d) => Ok(Some(wait_of(*d))),
        v => Err(nat::wrong_arg("timeout", "dur | nil", v)),
    }
}

/// The largest buffer Go's `make(chan Value, n)` accepts: past it Go
/// panics with `makechan: size out of range` (16-byte elements against
/// its 2^48-byte allocation limit), which its VM reports as E0801. Below
/// it Rust allocates nothing up front.
const MAX_CHANNEL_BUFFER: i64 = 17_592_186_044_409;

/// `channel(n = 0)` (Go `channelFn`): a rendezvous channel, or one that
/// buffers `n` values.
fn channel_fn(_: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    match &args[0] {
        Value::Int(n) if *n > MAX_CHANNEL_BUFFER => Err(nat::fail(
            "E0801",
            vec![nat::s("makechan: size out of range")],
        )),
        Value::Int(n) if *n >= 0 => Ok(Value::Channel(Rc::new(Channel::new(*n as usize)))),
        v => Err(nat::fail(
            "E0403",
            vec![
                nat::s("n"),
                nat::s("a non-negative int"),
                Arg::Str(format(v)),
            ],
        )),
    }
}

fn need_channel(v: &Value) -> Result<&Rc<Channel>, Stop> {
    match v {
        Value::Channel(c) => Ok(c),
        _ => Err(nat::wrong_arg("ch", "channel", v)),
    }
}

/// `send(ch, v)` (Go `sendFn`): `Ok(nil)` once delivered,
/// `Err({kind: 'closed})` on a closed channel. Rust only: E0705 when `v`
/// contains `ch` (`channel::contains`), checked before anything else
/// happens, so the error is the same whether or not the channel is
/// closed or full.
fn send(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    if channel::contains(need_channel(&args[0])?, &args[1]) {
        return Err(nat::fail("E0705", vec![]));
    }
    deliver(t, args)
}

/// `_native.conc.send_unchecked(ch, v)` (stage 3b, Rust only): `send`
/// without that check, for an agent's own requests, which are taken out
/// of the channel at once and so leave no cycle behind.
fn send_unchecked(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    deliver(t, args)
}

/// What `send` and `send_unchecked` share: the delivery itself.
fn deliver(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let v = args.pop().expect("v");
    let ch = need_channel(&args[0])?;
    if channel::send(t, ch, v)? {
        Ok(nat::ok(Value::Nil))
    } else {
        Ok(err_kind("closed"))
    }
}

/// `recv(ch, timeout: nil)` (Go `recvFn`): `Ok(v)`, or
/// `Err({kind: 'closed})` once the channel is closed and empty, or
/// `Err({kind: 'timeout})`.
fn recv(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let ch = need_channel(&args[0])?;
    let timeout = timeout_arg(&args[1])?;
    Ok(match channel::recv(t, ch, timeout)? {
        Received::Value(v) => nat::ok(v),
        Received::Closed => err_kind("closed"),
        Received::Timeout => err_kind("timeout"),
    })
}

/// `close(ch)` (Go `closeFn`): `Ok(nil)`, also on a closed channel.
fn close(_: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    need_channel(&args[0])?.close();
    Ok(nat::ok(Value::Nil))
}
