//! The concurrency natives (05): `spawn(s, f)`, `await`, `cancel`, `sleep`,
//! `scope`, `supervise`, `channel`, `send`, `recv` and `close`, on
//! `crate::vm`'s tasks and blocking point, `atom`, `deref`, `swap` and
//! `reset` (`crate::vm::atom`). The parameter tables here
//! are the source of `spec/builtins.txt`; the VM checks arity and named
//! arguments against them before the native runs.

use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::native::nat::{self, native, native_step, opt, param};
use crate::value::{Args, Lit, Native, NativeFn, StepFn, Value};
use crate::vm::channel::{self, Channel, Received, RecvWait, SendWait, Try};
use crate::vm::{Cont, Flow, Next, Stop, Task, Wake, err_kind, park_until, park_until_then};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The prelude's concurrency natives: `spawn`
/// `await` `cancel` `channel` `send` `recv` `close` `sleep` and the rest.
fn all() -> Vec<(&'static str, Value)> {
    NATIVES
        .get_or_init(|| {
            let tables: Vec<(&'static str, Vec<_>)> = vec![
                ("sleep", vec![param("d")]),
                ("scope", vec![param("f")]),
                ("timeout", vec![param("f"), param("d")]),
                ("atom", vec![param("x")]),
                ("deref", vec![param("a")]),
                ("swap", vec![param("a"), param("f")]),
                ("reset", vec![param("a"), param("x")]),
                (
                    "with",
                    vec![param("acquire"), param("use"), param("release")],
                ),
                ("supervise", vec![param("f"), param("on_fail")]),
                ("spawn", vec![param("s"), param("f")]),
                ("await", vec![param("t")]),
                ("cancel", vec![param("t")]),
                ("channel", vec![opt("n", Lit::Int(0))]),
                ("send", vec![param("tx"), param("x")]),
                ("recv", vec![param("rx")]),
                ("send_now", vec![param("tx"), param("x")]),
                ("recv_now", vec![param("rx")]),
                ("wait_closed", vec![param("rx")]),
                ("close", vec![param("x")]),
                ("stop_signal", vec![]),
                ("request_stop", vec![]),
            ];
            tables
                .into_iter()
                .map(|(name, params)| {
                    let step: Option<StepFn> = match name {
                        "sleep" => Some(sleep),
                        "send" => Some(send),
                        "recv" => Some(recv),
                        "wait_closed" => Some(wait_closed),
                        "await" => Some(await_fn),
                        "scope" => Some(scope),
                        "supervise" => Some(supervise),
                        "timeout" => Some(timeout),
                        "swap" => Some(swap),
                        "with" => Some(with),
                        "close" => Some(close),
                        _ => None,
                    };
                    if let Some(step) = step {
                        return (name, native_step(name, step, params));
                    }
                    let f: NativeFn = match name {
                        "atom" => atom,
                        "deref" => deref,
                        "reset" => reset,
                        "spawn" => spawn,
                        "cancel" => cancel,
                        "channel" => channel_fn,
                        "send_now" => send_now,
                        "recv_now" => recv_now,
                        "stop_signal" => crate::vm::stop::stop_signal,
                        "request_stop" => crate::vm::stop::request_stop,
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

/// The prelude's concurrency natives.
pub fn natives() -> Vec<(&'static str, Value)> {
    all()
}

/// A duration argument (`nat::need_dur`: never negative) as a wait.
fn wait_of(d: i64) -> Duration {
    Duration::from_nanos(d as u64)
}

/// `sleep(d)`: waits for `d`; `0s` returns at once and a negative
/// duration is E0403 (SP15, D11). A cancelled task wakes and unwinds
/// with E0701.
fn sleep(t: &mut Task<'_>, args: Args) -> Result<Flow, Stop> {
    let d = nat::need_dur("d", &args[0])?;
    if d == 0 {
        return Ok(Flow::Done(Value::Nil));
    }
    // A deadline past what `Instant` can hold is no deadline, but the
    // wait still counts as one a timer ends.
    let until = Instant::now().checked_add(wait_of(d));
    let done = move |_: &mut Task<'_>, _: &_| match until {
        Some(at) if Instant::now() >= at => Some(Ok(Value::Nil)),
        _ => None,
    };
    park_until(t, done, Wake { until, timed: true })
}

/// `scope(f)`: opens a scope and runs `f(s)` in it (`vm/scope.rs`).
fn scope(t: &mut Task<'_>, mut args: Args) -> Result<Flow, Stop> {
    let f = args.pop().expect("f");
    nat::need_fn("f", &f)?;
    Ok(crate::vm::run_scope(t, f, None))
}

/// `timeout(f, d)`: runs `f()` with a deadline `d` from now
/// (`vm/deadline.rs`).
fn timeout(t: &mut Task<'_>, mut args: Args) -> Result<Flow, Stop> {
    // The function comes first, the duration second, as every native puts
    // what it works on first (SP10).
    let d = args.pop().expect("d");
    let f = args.pop().expect("f");
    nat::need_fn("f", &f)?;
    // A negative duration is E0403 (SP15, D11), not "at once".
    let d = nat::need_dur("d", &d)?;
    Ok(crate::vm::run_timeout(t, d, f))
}

/// `atom(v)`: a new atom holding `v` (`vm/atom.rs`).
fn atom(_: &mut Task<'_>, mut args: Args) -> Result<Value, Stop> {
    crate::vm::atom_make(args.pop().expect("v"))
}

fn need_atom(v: &Value) -> Result<&Rc<crate::vm::Atom>, Stop> {
    match v {
        Value::Atom(a) => Ok(a),
        v => Err(nat::wrong_arg("a", "atom", v)),
    }
}

/// `deref(a)`: the atom's value, without waiting.
fn deref(_: &mut Task<'_>, args: Args) -> Result<Value, Stop> {
    Ok(crate::vm::atom_read(need_atom(&args[0])?))
}

/// `swap(a, f)`: compare-and-set with `f` of the value; the new value.
fn swap(t: &mut Task<'_>, mut args: Args) -> Result<Flow, Stop> {
    let f = args.pop().expect("f");
    let a = need_atom(&args[0])?.clone();
    nat::need_fn("f", &f)?;
    Ok(crate::vm::atom_swap(t, a, f))
}

/// `reset(a, v)`: sets the value; the new value.
fn reset(_: &mut Task<'_>, mut args: Args) -> Result<Value, Stop> {
    let v = args.pop().expect("v");
    crate::vm::atom_reset(need_atom(&args[0])?, v)
}

/// `with(acquire, use, release)`: `use(acquire())`, then `release` of
/// what `acquire` gave, whatever `use` did (`vm/cleanup.rs`).
fn with(_: &mut Task<'_>, mut args: Args) -> Result<Flow, Stop> {
    let release = args.pop().expect("release");
    let use_ = args.pop().expect("use");
    let acquire = args.pop().expect("acquire");
    nat::need_fn("acquire", &acquire)?;
    nat::need_fn("use", &use_)?;
    nat::need_fn("release", &release)?;
    Ok(crate::vm::run_with(acquire, use_, release))
}

/// `supervise(f, on_fail)`: a scope whose children's failures go to
/// `on_fail` instead of failing it (`vm/scope.rs`).
fn supervise(t: &mut Task<'_>, mut args: Args) -> Result<Flow, Stop> {
    let on_fail = args.pop().expect("on_fail");
    let f = args.pop().expect("f");
    nat::need_fn("f", &f)?;
    nat::need_fn("on_fail", &on_fail)?;
    Ok(crate::vm::run_scope(t, f, Some(on_fail)))
}

/// `spawn(s, f)`: runs `f` on a new task in scope `s`.
fn spawn(t: &mut Task<'_>, mut args: Args) -> Result<Value, Stop> {
    let f = args.pop().expect("f");
    let s = args.pop().expect("s");
    let Value::Scope(h) = &s else {
        return Err(nat::wrong_arg("s", "scope", &s));
    };
    nat::need_fn("f", &f)?;
    t.spawn_in(&h.state, f)
}

/// `await(t)`: the task's result, kept for later calls. A deadline
/// around the wait (`timeout`) ends the wait, not the task; the waiter
/// then leaves the task's list.
fn await_fn(t: &mut Task<'_>, args: Args) -> Result<Flow, Stop> {
    let Value::Task(h) = &args[0] else {
        return Err(nat::wrong_arg("t", "task", &args[0]));
    };
    let h = h.clone();
    let state = h.state.clone();
    let ready = move |_: &mut Task<'_>, me: &_| {
        let r = h.state.read();
        if r.is_none() {
            h.state.waiters.borrow_mut().add(me);
        }
        r.map(Ok)
    };
    let leave = move |t: &mut Task<'_>| state.waiters.borrow_mut().remove(&t.waiter_handle());
    park_until_then(t, ready, Wake::default(), leave)
}

/// `cancel(t)`: cancels the task and every task below it, or the scope
/// (`vm/scope.rs`); a finished task is left as it is. A scope that has
/// closed is E0706, as `spawn` into it (D66).
fn cancel(_: &mut Task<'_>, args: Args) -> Result<Value, Stop> {
    match &args[0] {
        Value::Task(h) => h.cancel.cancel(),
        Value::Scope(h) if !h.state.is_open() => {
            return Err(nat::fail("E0706", vec![nat::s("cancel")]));
        }
        Value::Scope(h) => h.state.cancel(),
        v => return Err(nat::wrong_arg("t", "task | scope", v)),
    }
    Ok(Value::Nil)
}

/// `channel(n = 0)`: the ends `(tx, rx)` of a rendezvous channel, or of
/// one that buffers `n` values. Nothing is allocated up front, so `n` has
/// no upper limit.
fn channel_fn(_: &mut Task<'_>, args: Args) -> Result<Value, Stop> {
    // a count like any other: "must be int" for another type (FE82)
    let n = nat::need_count("n", &args[0])?;
    let (tx, rx) = channel::pair(n);
    Ok(Value::tuple(vec![
        Value::Sender(Rc::new(tx)),
        Value::Receiver(Rc::new(rx)),
    ]))
}

/// The channel behind a sending end, or E0403 for `tx`.
fn need_sender(v: &Value) -> Result<&Rc<Channel>, Stop> {
    match v {
        Value::Sender(e) => Ok(&e.0),
        _ => Err(nat::wrong_arg("tx", "sender", v)),
    }
}

/// `send(tx, v)`: `Ok(nil)` once delivered,
/// `Err({kind: 'closed, …})` on a closed channel (closed, or no receiving
/// end left). E0709 when `v` contains an end of the same channel or
/// leads back to one through other channels or tasks (`refuse_cycle`),
/// checked before anything else happens, so the error is the same
/// whether or not the channel is closed or full.
fn send(t: &mut Task<'_>, args: Args) -> Result<Flow, Stop> {
    refuse_cycle(need_sender(&args[0])?, &args[1])?;
    deliver(t, args)
}

/// E0709 when keeping `v` in `ch`'s buffer would close a cycle
/// counting could never free (`channel::cycle_to`, L7).
fn refuse_cycle(ch: &Rc<Channel>, v: &Value) -> Result<(), Stop> {
    match channel::cycle_to(channel::Place::Channel(ch), v) {
        channel::Cycle::No => Ok(()),
        channel::Cycle::Closes => Err(nat::fail("E0709", vec![])),
    }
}

/// The delivery itself.
fn deliver(t: &mut Task<'_>, mut args: Args) -> Result<Flow, Stop> {
    let v = args.pop().expect("v");
    let Value::Sender(end) = &args[0] else {
        return Err(nat::wrong_arg("tx", "sender", &args[0]));
    };
    Ok(match channel::begin_send(t, end, v) {
        Try::Ready(sent) => Flow::Done(sent_value(sent)),
        Try::Wait(w) => {
            let wake = w.wake();
            Flow::Park(Box::new(SendK(Some(w))), wake)
        }
    })
}

fn sent_value(sent: bool) -> Value {
    if sent {
        nat::ok(Value::Nil)
    } else {
        err_kind("closed", "channel closed")
    }
}

/// A `send` parked until a receiver takes the value or the channel closes.
struct SendK(Option<SendWait>);

impl Cont for SendK {
    fn resume(&mut self, _: &mut Task<'_>, _: Value) -> Result<Next, Stop> {
        let w = self.0.as_ref().expect("a waiting send");
        Ok(match w.poll() {
            Some(sent) => Next::Done(sent_value(sent)),
            None => Next::Park(w.wake()),
        })
    }

    fn unwind(&mut self, _: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        let w = self.0.take().expect("a waiting send");
        w.cancel(stop).map(|sent| Next::Done(sent_value(sent)))
    }
}

/// `recv(rx)`: `Ok(v)`, or `Err({kind: 'closed, …})` once the channel is
/// closed and empty.
fn recv(t: &mut Task<'_>, args: Args) -> Result<Flow, Stop> {
    let Value::Receiver(end) = &args[0] else {
        return Err(nat::wrong_arg("rx", "receiver", &args[0]));
    };
    Ok(match channel::begin_recv(t, end)? {
        Try::Ready(r) => Flow::Done(received_value(r)),
        Try::Wait(w) => {
            let wake = w.wake();
            Flow::Park(Box::new(RecvK(Some(w))), wake)
        }
    })
}

fn received_value(r: Received) -> Value {
    match r {
        Received::Value(v) => nat::ok(v),
        Received::Closed => err_kind("closed", "channel closed"),
    }
}

/// A `recv` parked until a value comes or the channel closes.
struct RecvK(Option<RecvWait>);

impl Cont for RecvK {
    fn receives_on(&self) -> Option<&Rc<crate::vm::Receiver>> {
        self.0.as_ref().map(RecvWait::end)
    }

    fn resume(&mut self, _: &mut Task<'_>, _: Value) -> Result<Next, Stop> {
        let w = self.0.as_ref().expect("a waiting recv");
        Ok(match w.poll() {
            Some(r) => Next::Done(received_value(r)),
            None => Next::Park(w.wake()),
        })
    }

    fn unwind(&mut self, _: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        let w = self.0.take().expect("a waiting recv");
        w.cancel(stop).map(|v| Next::Done(nat::ok(v)))
    }
}

/// `recv_now(rx)` (L3, SP10): what `recv` would take without waiting: `Ok(v)`,
/// `Err({kind: 'empty, …})` while the channel is open with nothing to
/// take, or `Err({kind: 'closed, …})` once it is closed and empty.
fn recv_now(_: &mut Task<'_>, args: Args) -> Result<Value, Stop> {
    let Value::Receiver(end) = &args[0] else {
        return Err(nat::wrong_arg("rx", "receiver", &args[0]));
    };
    Ok(end
        .recv_now()
        .map_or_else(|| err_kind("empty", "channel empty"), received_value))
}

/// `send_now(tx, v)` (L3, SP10): what `send` would do without waiting:
/// `Ok(nil)` once handed to a waiting receiver or the buffer,
/// `Err({kind: 'full, …})` when it would wait, `Err({kind: 'closed, …})`
/// on a closed channel. E0709 as `send`.
fn send_now(_: &mut Task<'_>, mut args: Args) -> Result<Value, Stop> {
    let v = args.pop().expect("v");
    refuse_cycle(need_sender(&args[0])?, &v)?;
    let Value::Sender(end) = &args[0] else {
        unreachable!("need_sender checked it")
    };
    Ok(match end.send_now(v) {
        Ok(()) => nat::ok(Value::Nil),
        Err(true) => err_kind("closed", "channel closed"),
        Err(false) => err_kind("full", "channel full"),
    })
}

/// `wait_closed(rx)` (L3): waits until the channel closes and gives nil,
/// taking nothing out (what is buffered stays). The wait holds `rx`, so
/// waiting on the stop signal watches it (`stop.rs`), and such a wait is
/// woken from outside: not a deadlock. A deadlock report looks for the
/// channel's sending end in the waiting task, as for `recv` (CC12).
fn wait_closed(t: &mut Task<'_>, args: Args) -> Result<Flow, Stop> {
    let Value::Receiver(end) = &args[0] else {
        return Err(nat::wrong_arg("rx", "receiver", &args[0]));
    };
    let me = t.waiter_handle();
    me.take_woken();
    if end.closed_or_watch(&me) {
        return Ok(Flow::Done(Value::Nil));
    }
    let wake = Wake {
        until: None,
        timed: end.external(),
    };
    Ok(Flow::Park(Box::new(ClosedWait(end.clone(), wake)), wake))
}

/// A `wait_closed` parked until its channel closes.
struct ClosedWait(Rc<crate::vm::Receiver>, Wake);

impl Cont for ClosedWait {
    fn receives_on(&self) -> Option<&Rc<crate::vm::Receiver>> {
        Some(&self.0)
    }

    fn resume(&mut self, t: &mut Task<'_>, _: Value) -> Result<Next, Stop> {
        Ok(match self.0.closed_or_watch(&t.waiter_handle()) {
            true => Next::Done(Value::Nil),
            false => Next::Park(self.1),
        })
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        self.0.unwatch(&t.waiter_handle());
        Err(stop)
    }
}

/// `close(tx)`: `Ok(nil)`, also on a closed channel. It closes for every
/// copy of the end.
fn close(t: &mut Task<'_>, args: Args) -> Result<Flow, Stop> {
    // One `close` for what can be closed (SP10): a file handle closes on
    // the pool, as `fs` did; a channel's sending end closes here. A
    // closed one of either is `Err('closed)` (D72).
    match &args[0] {
        Value::File(_) => crate::native::fs::close_fn(t, args),
        // A closed channel, by any end or because its receiving ends
        // went, is a closed handle: closing it again is an Err (D72).
        Value::Sender(e) if e.0.closed() => Ok(Flow::Done(err_kind("closed", "channel closed"))),
        Value::Sender(e) => {
            e.0.close();
            Ok(Flow::Done(nat::ok(Value::Nil)))
        }
        v => Err(nat::wrong_arg("x", "sender | file", v)),
    }
}
