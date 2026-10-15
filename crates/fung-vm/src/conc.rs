//! The concurrency natives (05; Go `vm.Concurrency()`). Tasks and
//! channels are real (stage 3a: `spawn`, `await`, `cancel`, `sleep`,
//! `_native.conc.spawn_awaited`, `channel`, `send`, `recv` and `close`, on
//! `crate::vm`'s tasks and blocking point). Atoms are not ported yet:
//! calling them raises
//! E0805 `not supported yet: <name>`. The parameter tables are Go's
//! (`spec/builtins.txt`), so arity and named-argument errors, which the
//! VM reports before the native runs, are the same as Go's.

use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Duration;

use fung_front::diag::Arg;

use crate::native::nat::{self, native, opt, param};
use crate::value::{Lit, Module, Native, NativeFn, Value, format};
use crate::vm::channel::{self, Channel, Received};
use crate::vm::{Stop, Task, block, err_kind};

/// Made once per process: natives are leaked (`Native::leak`).
static PLACEHOLDERS: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The prelude's concurrency natives (Go `vm.Concurrency()`): `spawn`
/// `await` `cancel` `channel` `send` `recv` `close` `sleep` `atom` `swap`
/// `deref`, and `_native.conc.spawn_awaited` last.
fn all() -> Vec<(&'static str, Value)> {
    PLACEHOLDERS
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
                ("atom", vec![param("v")]),
                ("swap", vec![param("a"), param("f")]),
                ("deref", vec![param("a")]),
                ("spawn_awaited", vec![param("f")]),
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
                        _ => unsupported,
                    };
                    (name, native(name, f, params))
                })
                .collect()
        })
        .iter()
        .map(|(n, f)| (*n, Value::Native(f)))
        .collect()
}

/// The prelude's concurrency natives, without `spawn_awaited` (which only
/// `_native.conc` holds).
pub fn placeholders() -> Vec<(&'static str, Value)> {
    all()
        .into_iter()
        .filter(|(n, _)| *n != "spawn_awaited")
        .collect()
}

/// `_native.conc` (Go `vm.NativeModules()`): `spawn_awaited`, which
/// `extras/prelude.fung` uses for `fan_in` and `pmap`.
pub fn module() -> Rc<Module> {
    let mut m = Module::new("conc", "_native.conc");
    for (name, v) in all() {
        if name == "spawn_awaited" {
            m.set(name, v);
        }
    }
    Rc::new(m)
}

/// Every placeholder: E0805 with the native's own name. The VM gives the
/// error the call's position (or, inside `extras/prelude.fung`, the user
/// call's).
fn unsupported(t: &mut Task<'_>, _: Vec<Value>) -> Result<Value, Stop> {
    let name = t.calling_native().unwrap_or("?");
    Err(nat::fail("E0805", vec![nat::s(name)]))
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
    let state = h.state.clone();
    let ready = |_: &mut _, me: &_| {
        let r = state.result();
        if r.is_none() {
            state.waiters.borrow_mut().add(me);
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
/// `Err({kind: 'closed})` on a closed channel.
fn send(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
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
