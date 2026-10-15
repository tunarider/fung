//! Atoms (05, stage 3a; Go `internal/vm/atom.go`): the one mutable box in
//! the language. Stage 3b removes them, so everything about atoms lives in
//! this file (the value, the lock, the natives).
//!
//! - `atom(v)` makes one; it prints as `<atom v>`, `type_of` gives
//!   `'atom`, and two atoms are equal only when they are the same atom.
//! - `deref(a)` reads the current value. It never waits: during a swap it
//!   sees the value from before the swap.
//! - `swap(a, f)` calls `f` on the current value exactly once while
//!   holding the atom, stores the result and returns it. Another task's
//!   `swap` of the same atom waits through `block` (so it is cancellable
//!   and counts as blocked); a `swap` of the same atom from inside `f`, in
//!   the same task, is E0703.
//!
//! The lock is a holder plus a FIFO queue of waiting swappers, touched
//! only by the run token's holder. Releasing hands the atom straight to
//! the first queued swapper that is still live, the way Task 4's channel
//! slots are paired: each queued entry carries its task's cancellation
//! node, and a cancelled entry is dropped from the queue and never granted
//! (its task wakes through its cancellation watch and returns E0701). The
//! holder is a `Held` guard, so the atom is released on every way out of
//! `f`: a value, a fung error, E0701, `exit`, or a panic caught by the
//! VM's fence.
//!
//! The holder is identified by its task's `Waiter`, which a task keeps for
//! its whole life (unlike its cancellation node, which exit handlers
//! replace, or its OS thread, which stage 3b takes away).
//!
//! An atom can hold a value that holds the atom itself
//! (`swap(a, _ => [a])`): an `Rc` cycle that is never freed, where Go's
//! collector would free it.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use super::block::{Waiter, block};
use super::{CancelNode, Stop, Task, token};
use crate::native::nat;
use crate::value::{Value, drop_children, format, keep};

/// The value `atom(v)` returns.
pub struct Atom {
    value: RefCell<Value>,
    lock: RefCell<Lock>,
}

#[derive(Default)]
struct Lock {
    /// The task inside `swap`, by its waiter.
    holder: Option<Arc<Waiter>>,
    /// Swappers waiting for the atom, oldest first.
    queue: VecDeque<Rc<Wanting>>,
}

/// One waiting `swap`: the task to wake, its cancellation (which claims
/// the entry before any grant), and whether the atom was handed to it.
struct Wanting {
    waiter: Arc<Waiter>,
    cancel: Arc<CancelNode>,
    granted: Cell<bool>,
}

impl Atom {
    fn new(v: Value) -> Atom {
        Atom {
            value: RefCell::new(v),
            lock: RefCell::new(Lock::default()),
        }
    }

    /// The current value (`deref`).
    pub(crate) fn load(&self) -> Value {
        self.value.borrow().clone()
    }

    /// What `print` and `format` show: `<atom v>`.
    pub(crate) fn display(&self) -> String {
        format!("<atom {}>", format(&self.value.borrow()))
    }

    /// Hands the atom to the first queued swapper that is still live,
    /// dropping the cancelled ones before it, or leaves it free.
    fn release(&self) {
        let mut l = self.lock.borrow_mut();
        l.holder = None;
        while let Some(w) = l.queue.pop_front() {
            if !w.cancel.is_cancelled() {
                w.granted.set(true);
                l.holder = Some(w.waiter.clone());
                w.waiter.wake();
                return;
            }
        }
    }

    /// Whether a `swap` holds the atom (tests order tasks by it).
    #[cfg(test)]
    pub(crate) fn held(&self) -> bool {
        self.lock.borrow().holder.is_some()
    }

    /// How many swappers wait for the atom (tests).
    #[cfg(test)]
    pub(crate) fn waiting(&self) -> usize {
        self.lock.borrow().queue.len()
    }

    /// Takes a waiter whose wait ended without a grant out of the queue.
    fn leave(&self, w: &Rc<Wanting>) {
        self.lock.borrow_mut().queue.retain(|x| !Rc::ptr_eq(x, w));
    }

    /// Moves the value out for `release` (`value/release.rs`).
    pub(crate) fn spill(&mut self, stack: &mut Vec<Value>) {
        keep(stack, std::mem::replace(self.value.get_mut(), Value::Nil));
    }
}

impl std::fmt::Debug for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<atom>")
    }
}

/// An atom is a value: it drops only on the token's holder, and frees its
/// value without recursion (`release`).
impl Drop for Atom {
    fn drop(&mut self) {
        token::assert_holds_token();
        drop_children(|stack| self.spill(stack));
    }
}

/// The atom held by a running `swap`; dropping it releases the atom.
struct Held(Rc<Atom>);

impl Drop for Held {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// Takes the atom for `t`, waiting for another task's swap to end.
fn acquire(t: &mut Task<'_>, a: &Rc<Atom>) -> Result<Held, Stop> {
    let w = {
        let mut l = a.lock.borrow_mut();
        match &l.holder {
            None => {
                l.holder = Some(t.waiter.clone());
                return Ok(Held(a.clone()));
            }
            Some(h) if Arc::ptr_eq(h, &t.waiter) => {
                return Err(nat::fail("E0703", Vec::new()));
            }
            Some(_) => {}
        }
        let w = Rc::new(Wanting {
            waiter: t.waiter.clone(),
            cancel: t.cancel.clone(),
            granted: Cell::new(false),
        });
        l.queue.push_back(w.clone());
        w
    };
    let r = block(t, |_, _| w.granted.get().then_some(()), None);
    match r {
        Ok(_) => Ok(Held(a.clone())),
        // Granted and cancelled at once: pass the atom on.
        Err(e) if w.granted.get() => {
            a.release();
            Err(e)
        }
        Err(e) => {
            a.leave(&w);
            Err(e)
        }
    }
}

fn need_atom(v: &Value) -> Result<&Rc<Atom>, Stop> {
    match v {
        Value::Atom(a) => Ok(a),
        _ => Err(nat::wrong_arg("a", "atom", v)),
    }
}

/// `atom(v)` (Go `atomFn`).
pub(crate) fn atom_fn(_: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let v = args.pop().expect("v");
    Ok(Value::Atom(Rc::new(Atom::new(v))))
}

/// `swap(a, f)` (Go `swapFn`): runs `f` on the current value while holding
/// the atom, stores the result and returns it. A failing `f` leaves the
/// value unchanged.
pub(crate) fn swap_fn(t: &mut Task<'_>, mut args: Vec<Value>) -> Result<Value, Stop> {
    let f = args.pop().expect("f");
    let a = need_atom(&args[0])?.clone();
    nat::need_fn("f", &f)?;
    let held = acquire(t, &a)?;
    let r = t.call(&f, vec![a.load()])?;
    drop(a.value.replace(r.clone()));
    drop(held);
    Ok(r)
}

/// `deref(a)` (Go `derefFn`): never waits.
pub(crate) fn deref_fn(_: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    Ok(need_atom(&args[0])?.load())
}
