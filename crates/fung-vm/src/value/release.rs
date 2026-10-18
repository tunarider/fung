//! Iterative teardown of nested values. Rust drops a value by dropping
//! its fields, so the last reference to a list of lists of lists ... would
//! free it with one stack frame per level and overflow on deep nesting.
//! Every container that holds values (list, map, tuple, tag, closure,
//! module, channel end, task handle, atom) therefore moves its
//! children out in its `Drop` and hands the ones that could go deep to
//! `release`, which frees them with an explicit stack. A shell whose
//! children were moved out drops in constant stack.
//!
//! A file handle (`Value::File`) holds no values, only its path and the
//! open file: dropping it closes the file and never goes deep, so it is
//! not handed to `release`. Nor are connections and listeners
//! (`Value::Conn`, `Value::Listener`), for the same reason.

use std::rc::Rc;

use super::Value;

/// Whether dropping `v` could go deep: a container (or closure) this is the last
/// reference to. Anything else only decrements a count or holds no values.
fn nests(v: &Value) -> bool {
    match v {
        Value::List(rc) => Rc::strong_count(rc) == 1,
        Value::Map(rc) => Rc::strong_count(rc) == 1,
        Value::Tuple(rc) => Rc::strong_count(rc) == 1,
        Value::Tag(rc) => Rc::strong_count(rc) == 1,
        Value::Closure(rc) => Rc::strong_count(rc) == 1,
        Value::Module(rc) => Rc::strong_count(rc) == 1,
        Value::Sender(rc) => Rc::strong_count(rc) == 1,
        Value::Receiver(rc) => Rc::strong_count(rc) == 1,
        Value::Task(rc) => Rc::strong_count(rc) == 1,
        Value::Atom(rc) => Rc::strong_count(rc) == 1,
        _ => false,
    }
}

/// Keeps `v` on the stack if it could go deep, drops it otherwise.
pub(crate) fn keep(stack: &mut Vec<Value>, v: Value) {
    if nests(&v) {
        stack.push(v);
    }
}

/// Frees the values on the stack and everything they alone hold, without
/// recursion: each container is unwrapped, its children moved onto the
/// stack, and the emptied shell dropped.
pub(super) fn release(mut stack: Vec<Value>) {
    while let Some(v) = stack.pop() {
        match v {
            Value::List(rc) => {
                if let Ok(mut l) = Rc::try_unwrap(rc) {
                    l.spill(&mut stack);
                }
            }
            Value::Map(rc) => {
                if let Ok(mut m) = Rc::try_unwrap(rc) {
                    m.spill(&mut stack);
                }
            }
            Value::Tuple(rc) => {
                if let Ok(mut t) = Rc::try_unwrap(rc) {
                    t.spill(&mut stack);
                }
            }
            Value::Tag(rc) => {
                if let Ok(mut t) = Rc::try_unwrap(rc) {
                    for v in std::mem::take(&mut t.args) {
                        keep(&mut stack, v);
                    }
                }
            }
            Value::Closure(rc) => {
                if let Ok(mut c) = Rc::try_unwrap(rc) {
                    c.spill(&mut stack);
                }
            }
            Value::Module(rc) => {
                if let Ok(mut m) = Rc::try_unwrap(rc) {
                    m.spill(&mut stack);
                }
            }
            // The last copy of an end: it does its own work (a sender
            // closes; a receiver closes and its buffer comes onto the
            // stack), then the channel spills if that end was its last
            // owner.
            Value::Sender(rc) => {
                if let Ok(e) = Rc::try_unwrap(rc) {
                    let ch = e.0.clone();
                    drop(e);
                    spill_channel(ch, &mut stack);
                }
            }
            Value::Receiver(rc) => {
                if let Ok(e) = Rc::try_unwrap(rc) {
                    // The buffer onto this stack; the end's own drop then
                    // abandons again, which finds it empty (idempotent).
                    e.0.abandon(&mut stack);
                    let ch = e.0.clone();
                    drop(e);
                    spill_channel(ch, &mut stack);
                }
            }
            Value::Atom(rc) => {
                // The held value goes on the stack: an atom holding a
                // long chain of values frees without recursion.
                if let Ok(a) = Rc::try_unwrap(rc) {
                    keep(&mut stack, a.into_value());
                }
            }
            Value::Task(rc) => {
                // The handle drops first (it may report a failure); the
                // result goes on the stack if this was the last reference
                // to the task's state (a running task holds one too).
                if let Ok(h) = Rc::try_unwrap(rc) {
                    let state = h.state.clone();
                    drop(h);
                    if let Ok(state) = Rc::try_unwrap(state)
                        && let Some(v) = state.take_result()
                    {
                        keep(&mut stack, v);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Moves a channel's values onto the stack if `ch` was its last owner.
fn spill_channel(ch: Rc<crate::vm::Channel>, stack: &mut Vec<Value>) {
    if let Ok(mut c) = Rc::try_unwrap(ch) {
        c.spill(stack);
    }
}

/// Drops a container's children: the common body of the containers' `Drop`.
/// Allocates nothing unless a child nests.
pub(crate) fn drop_children(children: impl FnOnce(&mut Vec<Value>)) {
    let mut stack = Vec::new();
    children(&mut stack);
    if !stack.is_empty() {
        release(stack);
    }
}
