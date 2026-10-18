//! The concurrency natives (05): `spawn(s, f)`, `await`, `cancel`, `sleep`,
//! `scope`, `supervise`, `channel`, `send`, `recv` and `close`, on
//! `crate::vm`'s tasks and blocking point, `atom`, `deref`, `swap` and
//! `reset` (`crate::vm::atom`), and `_native.conc.try_spawn`, which the
//! `http` server spawns its connections with. The parameter tables here
//! are the source of `spec/builtins.txt`; the VM checks arity and named
//! arguments against them before the native runs.

use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Duration;

use fung_front::diag::Arg;

use crate::native::nat::{self, native, opt, param};
use crate::value::{Lit, Module, Native, NativeFn, Value, format};
use crate::vm::channel::{self, Channel, Received};
use crate::vm::{Stop, Task, block, err_kind};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The prelude's concurrency natives: `spawn`
/// `await` `cancel` `channel` `send` `recv` `close` `sleep` and the rest,
/// then the member only `_native.conc` holds.
fn all() -> Vec<(&'static str, Value)> {
    NATIVES
        .get_or_init(|| {
            let tables: Vec<(&'static str, Vec<_>)> = vec![
                ("sleep", vec![param("d")]),
                ("scope", vec![param("f")]),
                ("timeout", vec![param("d"), param("f")]),
                ("atom", vec![param("v")]),
                ("deref", vec![param("a")]),
                ("swap", vec![param("a"), param("f")]),
                ("reset", vec![param("a"), param("v")]),
                (
                    "with",
                    vec![param("acquire"), param("use"), param("release")],
                ),
                ("supervise", vec![param("f"), param("on_fail")]),
                ("spawn", vec![param("s"), param("f")]),
                ("await", vec![param("t")]),
                ("cancel", vec![param("t")]),
                ("channel", vec![opt("n", Lit::Int(0))]),
                ("send", vec![param("tx"), param("v")]),
                ("recv", vec![param("rx")]),
                ("close", vec![param("tx")]),
                ("stop_signal", vec![]),
                ("request_stop", vec![]),
                // Only in `_native.conc` (`extras/http.fung`).
                ("try_spawn", vec![param("s"), param("f")]),
            ];
            tables
                .into_iter()
                .map(|(name, params)| {
                    let f: NativeFn = match name {
                        "sleep" => sleep,
                        "scope" => scope,
                        "timeout" => timeout,
                        "atom" => atom,
                        "deref" => deref,
                        "swap" => swap,
                        "reset" => reset,
                        "with" => with,
                        "supervise" => supervise,
                        "spawn" => spawn,
                        "await" => await_fn,
                        "cancel" => cancel,
                        "channel" => channel_fn,
                        "send" => send,
                        "recv" => recv,
                        "close" => close,
                        "stop_signal" => crate::vm::stop::stop_signal,
                        "request_stop" => crate::vm::stop::request_stop,
                        "try_spawn" => try_spawn,
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

/// The members of `_native.conc`. The front end learns them from
/// `spec/builtins.txt`, which the `spec_file` test generates from this
/// module; the boot test `native_modules_match_the_spec` checks the two
/// agree, parameter tables included.
const MODULE_ONLY: &[&str] = &["try_spawn"];

/// The prelude's concurrency natives, without the members only
/// `_native.conc` holds.
pub fn natives() -> Vec<(&'static str, Value)> {
    all()
        .into_iter()
        .filter(|(n, _)| !MODULE_ONLY.contains(n))
        .collect()
}

/// `_native.conc`: `try_spawn`, which `extras/http.fung` spawns its
/// connections with.
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

/// `sleep(d)`: waits for `d`; zero and negative durations
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

/// `scope(f)`: opens a scope and runs `f(s)` in it (`vm/scope.rs`).
fn scope(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let f = args.pop().expect("f");
    nat::need_fn("f", &f)?;
    crate::vm::run_scope(t, f, None)
}

/// `timeout(d, f)`: runs `f()` with a deadline `d` from now
/// (`vm/deadline.rs`).
fn timeout(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let f = args.pop().expect("f");
    let d = match args.pop().expect("d") {
        Value::Dur(d) => d,
        v => return Err(nat::wrong_arg("d", "dur", &v)),
    };
    nat::need_fn("f", &f)?;
    crate::vm::run_timeout(t, d, f)
}

/// `atom(v)`: a new atom holding `v` (`vm/atom.rs`).
fn atom(_: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    crate::vm::atom_make(args.pop().expect("v"))
}

fn need_atom(v: &Value) -> Result<&Rc<crate::vm::Atom>, Stop> {
    match v {
        Value::Atom(a) => Ok(a),
        v => Err(nat::wrong_arg("a", "atom", v)),
    }
}

/// `deref(a)`: the atom's value, without waiting.
fn deref(_: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    Ok(crate::vm::atom_read(need_atom(&args[0])?))
}

/// `swap(a, f)`: compare-and-set with `f` of the value; the new value.
fn swap(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let f = args.pop().expect("f");
    let a = need_atom(&args[0])?.clone();
    nat::need_fn("f", &f)?;
    crate::vm::atom_swap(t, &a, f)
}

/// `reset(a, v)`: sets the value; the new value.
fn reset(_: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let v = args.pop().expect("v");
    crate::vm::atom_reset(need_atom(&args[0])?, v)
}

/// `with(acquire, use, release)`: `use(acquire())`, then `release` of
/// what `acquire` gave, whatever `use` did (`vm/cleanup.rs`).
fn with(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let release = args.pop().expect("release");
    let use_ = args.pop().expect("use");
    let acquire = args.pop().expect("acquire");
    nat::need_fn("acquire", &acquire)?;
    nat::need_fn("use", &use_)?;
    nat::need_fn("release", &release)?;
    crate::vm::run_with(t, acquire, use_, release)
}

/// `supervise(f, on_fail)`: a scope whose children's failures go to
/// `on_fail` instead of failing it (`vm/scope.rs`).
fn supervise(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let on_fail = args.pop().expect("on_fail");
    let f = args.pop().expect("f");
    nat::need_fn("f", &f)?;
    nat::need_fn("on_fail", &on_fail)?;
    crate::vm::run_scope(t, f, Some(on_fail))
}

/// `spawn(s, f)`: runs `f` on a new task in scope `s`.
fn spawn(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let f = args.pop().expect("f");
    let s = args.pop().expect("s");
    let Value::Scope(h) = &s else {
        return Err(nat::wrong_arg("s", "scope", &s));
    };
    nat::need_fn("f", &f)?;
    t.spawn_in(&h.state, f)
}

/// `try_spawn(s, f)` (`_native.conc`, the `http` server): `Ok(<task>)`,
/// or `Err({kind: 'error, message})` when the task cannot start (no
/// thread left: E0801), so one connection that cannot get a task does not
/// end the server. Goes when spawning cannot fail (R7).
fn try_spawn(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    match spawn(t, args) {
        Ok(h) => Ok(nat::ok(h)),
        Err(Stop::Error(d)) if d.code == "E0801" => Ok(nat::err_value(
            "error",
            &fung_front::diag::headline(&d),
            Vec::new(),
        )),
        Err(e) => Err(e),
    }
}

/// `await(t)`: the task's result, kept for later calls. A deadline
/// around the wait (`timeout`) ends the wait, not the task.
fn await_fn(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let Value::Task(h) = &args[0] else {
        return Err(nat::wrong_arg("t", "task", &args[0]));
    };
    let h = h.clone();
    let ready = |_: &mut _, me: &_| {
        let r = h.state.result();
        if r.is_none() {
            h.state.waiters.borrow_mut().add(me);
        }
        r
    };
    Ok(block(t, ready, None)?.expect("a wait without a timeout ends with a value"))
}

/// `cancel(t)`: cancels the task and every task below it, or the scope
/// (`vm/scope.rs`); a finished task is left as it is.
fn cancel(_: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    match &args[0] {
        Value::Task(h) => h.cancel.cancel(),
        Value::Scope(h) => h.state.cancel(),
        v => return Err(nat::wrong_arg("t", "task | scope", v)),
    }
    Ok(Value::Nil)
}

/// `channel(n = 0)`: the ends `(tx, rx)` of a rendezvous channel, or of
/// one that buffers `n` values. Nothing is allocated up front, so `n` has
/// no upper limit.
fn channel_fn(_: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    match &args[0] {
        Value::Int(n) if *n >= 0 => {
            let (tx, rx) = channel::pair(*n as usize);
            Ok(Value::tuple(vec![
                Value::Sender(Rc::new(tx)),
                Value::Receiver(Rc::new(rx)),
            ]))
        }
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

/// The channel behind a sending end, or E0403 for `tx`.
fn need_sender(v: &Value) -> Result<&Rc<Channel>, Stop> {
    match v {
        Value::Sender(e) => Ok(&e.0),
        _ => Err(nat::wrong_arg("tx", "sender", v)),
    }
}

/// The channel behind a receiving end, or E0403 for `rx`.
fn need_receiver(v: &Value) -> Result<&Rc<Channel>, Stop> {
    match v {
        Value::Receiver(e) => Ok(&e.0),
        _ => Err(nat::wrong_arg("rx", "receiver", v)),
    }
}

/// `send(tx, v)`: `Ok(nil)` once delivered,
/// `Err({kind: 'closed})` on a closed channel (closed, or no receiving
/// end left). E0705 when `v` contains an end of the same channel
/// (`channel::contains`), checked before anything else
/// happens, so the error is the same whether or not the channel is
/// closed or full.
fn send(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    if channel::contains(need_sender(&args[0])?, &args[1]) {
        return Err(nat::fail("E0705", vec![]));
    }
    deliver(t, args)
}

/// The delivery itself.
fn deliver(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let v = args.pop().expect("v");
    let ch = need_sender(&args[0])?;
    if channel::send(t, ch, v)? {
        Ok(nat::ok(Value::Nil))
    } else {
        Ok(err_kind("closed"))
    }
}

/// `recv(rx)`: `Ok(v)`, or `Err({kind: 'closed})` once the channel is
/// closed and empty.
fn recv(t: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let ch = need_receiver(&args[0])?;
    Ok(match channel::recv(t, ch)? {
        Received::Value(v) => nat::ok(v),
        Received::Closed => err_kind("closed"),
    })
}

/// `close(tx)`: `Ok(nil)`, also on a closed channel. It closes for every
/// copy of the end.
fn close(_: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    need_sender(&args[0])?.close();
    Ok(nat::ok(Value::Nil))
}
