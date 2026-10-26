//! Walking a collection while calling a fung function per element, as a
//! continuation (`vm/cont.rs`): `map`, `filter`, `fold`, `each`, `find`,
//! `any`, `all`, `group_by` and `sort_by`'s keys.
//!
//! A [`Cursor`] is where the walk is; it owns what it walks, so it can
//! wait in a continuation between two calls. A [`PerItem`] is what one
//! native does with each element and each result. [`walk`] ties them to
//! the run loop: it asks for the call for the first element and, on every
//! result, either stops or asks for the next one.

use std::rc::Rc;

use crate::value::{List, Range, Value};
use crate::vm::channel::{self, Received, RecvWait, Try};
use crate::vm::{Cont, Flow, Next, Receiver, Stop, Task, Wake};

/// Where a walk over a list, a range or a receiving end is.
pub(crate) enum Cursor {
    /// A list: the items not yet taken from the leaf being walked wait in
    /// `run`, last first, so each step is O(1) amortized.
    List {
        list: Rc<List>,
        next: usize,
        run: Vec<Value>,
    },
    /// A range's numbers, made one at a time.
    Range { start: i64, len: i128, next: i128 },
    /// Values received as they arrive; what is left after a stop or an
    /// error stays in the channel (05). `wait` is a receive that waits.
    Receiver {
        rx: Rc<Receiver>,
        wait: Option<RecvWait>,
    },
    /// Items already collected (`sort_by`).
    Items(Vec<Value>, usize),
}

impl Cursor {
    /// The receiving end it walks, if it walks one.
    pub(crate) fn receiver(&self) -> Option<&Rc<Receiver>> {
        match self {
            Cursor::Receiver { rx, .. } => Some(rx),
            _ => None,
        }
    }

    /// A cursor over `v`, which the caller checked is a list, a range or
    /// a receiving end (`nat::check_items`).
    pub(crate) fn new(v: &Value) -> Cursor {
        match v {
            Value::List(l) => Cursor::List {
                list: l.clone(),
                next: 0,
                run: Vec::new(),
            },
            Value::Range(r) => Cursor::range(r),
            Value::Receiver(rx) => Cursor::Receiver {
                rx: rx.clone(),
                wait: None,
            },
            _ => unreachable!("a cursor over a checked collection"),
        }
    }

    fn range(r: &Range) -> Cursor {
        Cursor::Range {
            start: r.start,
            len: r.len(),
            next: 0,
        }
    }

    /// The next element, the end, or a wait for a channel's next value
    /// (the walk parks; `next` looks again after the wake).
    pub(crate) fn next(&mut self, t: &mut Task<'_>) -> Result<Step, Stop> {
        Ok(match self {
            Cursor::List { list, next, run } => {
                if let Some(v) = run.pop() {
                    return Ok(Step::Item(v));
                }
                let items = list.run_from(*next);
                if items.is_empty() {
                    return Ok(Step::End);
                }
                *next += items.len();
                run.extend(items.iter().rev().cloned());
                Step::Item(run.pop().expect("a run is not empty"))
            }
            Cursor::Range { start, len, next } => {
                if *next >= *len {
                    return Ok(Step::End);
                }
                // The number itself fits an int; only the offset may not.
                let v = Value::Int(start.wrapping_add(*next as i64));
                *next += 1;
                Step::Item(v)
            }
            Cursor::Receiver { rx, wait } => {
                let got = match wait {
                    Some(w) => match w.poll() {
                        Some(r) => r,
                        None => return Ok(Step::Wait(w.wake())),
                    },
                    None => {
                        // A function that cancelled its task returns here
                        // before any call checks: look first, so a value
                        // is never taken only to be dropped (05).
                        t.check_cancelled()?;
                        match channel::begin_recv(t, rx)? {
                            Try::Ready(r) => r,
                            Try::Wait(w) => {
                                let wake = w.wake();
                                *wait = Some(w);
                                return Ok(Step::Wait(wake));
                            }
                        }
                    }
                };
                *wait = None;
                match got {
                    Received::Value(x) => Step::Item(x),
                    Received::Closed => Step::End,
                }
            }
            Cursor::Items(items, next) => {
                let v = items.get(*next).cloned();
                *next += 1;
                v.map_or(Step::End, Step::Item)
            }
        })
    }

    /// A stop came while the cursor waited for a value: a value already
    /// delivered is still the next element; otherwise the stop goes on.
    pub(crate) fn cancel_wait(&mut self, e: Stop) -> Result<Value, Stop> {
        match self {
            Cursor::Receiver { wait, .. } => match wait.take() {
                Some(w) => w.cancel(e),
                None => Err(e),
            },
            _ => Err(e),
        }
    }
}

/// What a cursor gave.
pub(crate) enum Step {
    Item(Value),
    End,
    Wait(Wake),
}

/// What one iterating native does per element.
pub(crate) trait PerItem {
    /// Pushes the arguments of the call for element `x` (`Task::push_arg`)
    /// and says how many.
    fn args(&mut self, t: &mut Task<'_>, x: Value) -> usize;
    /// The call for the last element returned `r`; true stops the walk.
    fn got(&mut self, r: Value) -> Result<bool, Stop>;
    /// The native's result once the walk ended (`stopped`: by `got`).
    fn result(&mut self, stopped: bool) -> Result<Value, Stop>;
}

/// Calls `f` for every element `cur` gives, as `p` says.
pub(crate) fn walk<P: PerItem + 'static>(
    t: &mut Task<'_>,
    cur: Cursor,
    f: Value,
    p: P,
) -> Result<Flow, Stop> {
    let mut w = Walker {
        cur,
        f,
        p,
        waiting: false,
        pending: None,
    };
    Ok(match w.advance(t)? {
        Next::Done(v) => Flow::Done(v),
        Next::Call(f, n) => Flow::Call(Box::new(w), f, n),
        Next::Park(wake) => Flow::Park(Box::new(w), wake),
    })
}

struct Walker<P> {
    cur: Cursor,
    f: Value,
    p: P,
    /// Parked on the cursor (a channel's next value).
    waiting: bool,
    /// An element received before a stop that came while waiting: it is
    /// called back first (the next call then sees the stop).
    pending: Option<Value>,
}

impl<P: PerItem> Walker<P> {
    fn advance(&mut self, t: &mut Task<'_>) -> Result<Next, Stop> {
        loop {
            let x = match self.pending.take() {
                Some(x) => x,
                None => match self.cur.next(t)? {
                    Step::Item(x) => x,
                    Step::End => return Ok(Next::Done(self.p.result(false)?)),
                    Step::Wait(wake) => {
                        self.waiting = true;
                        return Ok(Next::Park(wake));
                    }
                },
            };
            let n = self.p.args(t, x);
            // A plain native never calls back, so it runs right here
            // (`map(xs, str)`): no trip through the run loop per element.
            if let Some(r) = t.call_plain(&self.f, n) {
                if self.p.got(r?)? {
                    return Ok(Next::Done(self.p.result(true)?));
                }
                continue;
            }
            return Ok(Next::Call(self.f.clone(), n));
        }
    }
}

impl<P: PerItem> Cont for Walker<P> {
    fn resume(&mut self, t: &mut Task<'_>, v: Value) -> Result<Next, Stop> {
        if std::mem::take(&mut self.waiting) {
            return self.advance(t);
        }
        if self.p.got(v)? {
            return Ok(Next::Done(self.p.result(true)?));
        }
        self.advance(t)
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        if !std::mem::take(&mut self.waiting) {
            return Err(stop);
        }
        self.pending = Some(self.cur.cancel_wait(stop)?);
        self.advance(t)
    }

    fn waits_for(&self) -> Option<&'static str> {
        self.waiting.then_some("recv")
    }

    fn receives_on(&self) -> Option<&Rc<Receiver>> {
        self.cur.receiver().filter(|_| self.waiting)
    }
}

/// What a walk without a callback (`join`, `flatten`, collecting for
/// `sort`) does with each element and at the end.
pub(crate) trait Collect {
    /// Takes element `x`; true stops the walk.
    fn item(&mut self, x: Value) -> Result<bool, Stop>;
    /// The native's next step once the walk ended: its result, or a
    /// call or a park it goes on with (`sort_by` walks its keys next).
    fn result(&mut self, t: &mut Task<'_>) -> Result<Flow, Stop>;
}

/// Walks `cur` with `p`, parking while a channel has no value yet.
pub(crate) fn drain_with<P: Collect + 'static>(
    t: &mut Task<'_>,
    cur: Cursor,
    p: P,
) -> Result<Flow, Stop> {
    let mut d = Drainer { cur, p, then: None };
    match d.advance(t)? {
        Drained::Wait(wake) => Ok(Flow::Park(Box::new(d), wake)),
        Drained::Ended => d.p.result(t),
    }
}

enum Drained {
    Wait(Wake),
    Ended,
}

struct Drainer<P> {
    cur: Cursor,
    p: P,
    /// What the native went on with after the walk, when that was a call
    /// or a park: this box stands in for it from then on.
    then: Option<Box<dyn Cont>>,
}

impl<P: Collect> Drainer<P> {
    fn advance(&mut self, t: &mut Task<'_>) -> Result<Drained, Stop> {
        loop {
            match self.cur.next(t)? {
                Step::Item(x) => {
                    if self.p.item(x)? {
                        return Ok(Drained::Ended);
                    }
                }
                Step::End => return Ok(Drained::Ended),
                Step::Wait(wake) => return Ok(Drained::Wait(wake)),
            }
        }
    }

    /// The walk ended: the native's result, or the box it goes on with.
    fn ended(&mut self, t: &mut Task<'_>) -> Result<Next, Stop> {
        Ok(match self.p.result(t)? {
            Flow::Done(v) => Next::Done(v),
            Flow::Call(k, f, n) => {
                self.then = Some(k);
                Next::Call(f, n)
            }
            Flow::Park(k, wake) => {
                self.then = Some(k);
                Next::Park(wake)
            }
        })
    }
}

impl<P: Collect> Cont for Drainer<P> {
    fn resume(&mut self, t: &mut Task<'_>, v: Value) -> Result<Next, Stop> {
        if let Some(k) = &mut self.then {
            return k.resume(t, v);
        }
        match self.advance(t)? {
            Drained::Wait(wake) => Ok(Next::Park(wake)),
            Drained::Ended => self.ended(t),
        }
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        if let Some(k) = &mut self.then {
            return k.unwind(t, stop);
        }
        // A value delivered before the stop is taken, as a receive does;
        // the walk then goes on until it would wait again.
        let x = self.cur.cancel_wait(stop)?;
        if self.p.item(x)? {
            return self.ended(t);
        }
        match self.advance(t)? {
            Drained::Wait(wake) => Ok(Next::Park(wake)),
            Drained::Ended => self.ended(t),
        }
    }

    fn waits_for(&self) -> Option<&'static str> {
        match &self.then {
            Some(k) => k.waits_for(),
            None => Some("recv"),
        }
    }

    fn receives_on(&self) -> Option<&Rc<Receiver>> {
        match &self.then {
            Some(k) => k.receives_on(),
            None => self.cur.receiver(),
        }
    }

    fn releasing(&self) -> bool {
        self.then.as_ref().is_some_and(|k| k.releasing())
    }
}
