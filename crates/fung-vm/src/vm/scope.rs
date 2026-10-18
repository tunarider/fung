//! Scopes (concurrency v2, L1; 05 "범위"): tasks start only inside one.
//!
//! `scope(f)` opens a scope, calls `f(s)` on the task that opened it and
//! returns only once that body and every task spawned into the scope
//! (`spawn(s, g)`) have ended.
//!
//! **The cancellation node.** A scope has a node of its own, below the
//! node of the task that opened it, and its children's nodes hang below
//! it. The body runs under it too (the task's node is swapped for the
//! call, as in `run_task`), so cancelling the scope reaches the body and
//! every child, and a cancellation from outside reaches the scope.
//!
//! **The wait for the children** cannot be cut short: once the body is
//! done the opener waits under a fresh root node, so a cancellation from
//! outside does not end the wait. It still reaches the children (they hang
//! below the opener's node), which unwind and end, and that ends the wait.
//! The waiting task stays live, so a wait nothing can end is a deadlock
//! (`deadlock.rs`).
//!
//! **What a scope holds.** It counts its live children instead of holding
//! their handles, so a child that holds the scope's handle (to spawn
//! more) makes no `Rc` cycle: a child holds the scope only while it runs.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use fung_front::diag::{self, Diag, headline};

use super::block::WaitList;
use super::task::{CancelNode, err_kind, is_cancel};
use super::{Stop, Task, block};
use crate::value::{MapBuilder, Value, intern};

/// The value `scope` hands its body: two handles are equal only when they
/// are the same scope; it prints as `<scope>`.
pub struct ScopeHandle {
    pub(crate) state: Rc<ScopeState>,
}

impl std::fmt::Debug for ScopeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<scope>")
    }
}

/// What a scope's handle, its opener and its children share. Only the
/// run token's holder touches it.
pub(crate) struct ScopeState {
    /// The node the body and the children run below.
    pub(crate) node: Arc<CancelNode>,
    /// False once `scope(` has returned: spawning into it is E0706.
    open: Cell<bool>,
    /// The children that have not ended.
    live: Cell<usize>,
    /// The opener, waiting for `live` to reach zero.
    waiters: RefCell<WaitList>,
    /// The first child failure, raised at `scope(`.
    failure: RefCell<Option<Box<Diag>>>,
    /// The headlines of the failures after the first.
    also: RefCell<Vec<String>>,
    /// Cancelled by `cancel(s)`: `scope(` then gives `Err({kind:
    /// 'cancelled})` instead of unwinding further.
    self_cancelled: Cell<bool>,
    /// What a child's failure does.
    policy: Policy,
}

/// What a child's failure does to its scope.
pub(crate) enum Policy {
    /// `scope`: the first failure cancels the scope and is raised at
    /// `scope(`.
    FailFast,
    /// `supervise`: failures go to `on_fail` one at a time, in the
    /// order they happen, and the rest of the scope runs on.
    Supervise(Supervisor),
}

/// A `supervise` scope's handler and the failures waiting for it.
///
/// No handler task is spawned: the first child to fail while nobody
/// handles becomes the handler. It stays a live child of the scope and
/// runs `on_fail` for its own failure and every one queued meanwhile, so
/// failures are handled in order and never two at a time.
pub(crate) struct Supervisor {
    /// Released when the scope closes.
    on_fail: RefCell<Option<Value>>,
    /// Each failure's value for `on_fail` (`{kind: 'error, message}`) and
    /// the report written if it is never handled.
    queue: RefCell<VecDeque<(Value, String)>>,
    handling: Cell<bool>,
}

impl ScopeState {
    /// A scope whose first failing child fails it (`scope`).
    pub(crate) fn fail_fast(node: Arc<CancelNode>) -> Rc<ScopeState> {
        ScopeState::new(node, Policy::FailFast)
    }

    /// A scope that hands its children's failures to `on_fail`
    /// (`supervise`).
    pub(crate) fn supervise(node: Arc<CancelNode>, on_fail: Value) -> Rc<ScopeState> {
        ScopeState::new(
            node,
            Policy::Supervise(Supervisor {
                on_fail: RefCell::new(Some(on_fail)),
                queue: RefCell::new(VecDeque::new()),
                handling: Cell::new(false),
            }),
        )
    }

    fn new(node: Arc<CancelNode>, policy: Policy) -> Rc<ScopeState> {
        Rc::new(ScopeState {
            node,
            open: Cell::new(true),
            live: Cell::new(0),
            waiters: RefCell::new(WaitList::default()),
            failure: RefCell::new(None),
            also: RefCell::new(Vec::new()),
            self_cancelled: Cell::new(false),
            policy,
        })
    }

    /// Records the scope's failure (the first fails it, cancelling the
    /// body and the children; later ones become notes).
    fn fail(&self, d: &Diag) {
        let mut first = self.failure.borrow_mut();
        if first.is_none() {
            *first = Some(Box::new(d.clone()));
            drop(first);
            self.node.cancel();
        } else {
            self.also.borrow_mut().push(headline(d));
        }
    }

    /// The failure to raise at `scope(`, if a child failed: the first
    /// one with its own position and call path, and notes naming where
    /// the scope was opened (`opened_at`, `file:line`) and the failures
    /// after it.
    pub(crate) fn take_failure(&self, opened_at: &str) -> Option<Box<Diag>> {
        let mut d = self.failure.borrow_mut().take()?;
        d.notes.push(format!("note: scope opened at {opened_at}"));
        for h in self.also.borrow_mut().drain(..) {
            d.notes.push(format!("note: also failed: {h}"));
        }
        Some(d)
    }

    /// `cancel(s)`: cancels the body and every child; `scope(` gives
    /// `Err({kind: 'cancelled})`.
    pub(crate) fn cancel(&self) {
        self.self_cancelled.set(true);
        self.node.cancel();
    }

    /// Whether tasks may still be spawned into it.
    pub(crate) fn is_open(&self) -> bool {
        self.open.get()
    }

    /// A child is about to start.
    pub(crate) fn child_started(&self) {
        self.live.set(self.live.get() + 1);
    }

    /// A child ended, or failed to start. Called with the run token held,
    /// by the child's thread once its result is stored.
    ///
    /// A child that died of a runtime error (not a cancellation, a
    /// deadlock report or an exit) fails the scope: the first failure is
    /// kept and cancels the body and the other children, later ones are
    /// kept as notes.
    pub(crate) fn child_done(&self, r: &Result<Value, Stop>) {
        if let Err(Stop::Error(d)) = r
            && !is_cancel(d)
            && matches!(self.policy, Policy::FailFast)
        {
            self.fail(d);
        }
        let n = self.live.get() - 1;
        self.live.set(n);
        if n == 0 {
            self.waiters.borrow_mut().wake_all();
        }
    }
}

impl Task<'_> {
    /// A child of `s` ended with `r`, on its own thread with the run token
    /// held. In a `supervise` scope a failure is queued, and if nobody is
    /// handling, this task handles the queue before it counts as ended.
    pub(crate) fn end_in_scope(&mut self, s: &ScopeState, r: &Result<Value, Stop>) {
        if let Policy::Supervise(sv) = &s.policy
            && let Err(Stop::Error(d)) = r
            && !is_cancel(d)
        {
            let text = format!("task failed: {}", diag::format(d, &self.world.sources));
            sv.queue.borrow_mut().push_back((failure_value(d), text));
            if !sv.handling.get() {
                sv.handling.set(true);
                self.handle(s, sv);
                sv.handling.set(false);
            }
        }
        s.child_done(r);
    }

    /// Runs `on_fail` for each queued failure until the queue is empty,
    /// the scope is cancelled, or `on_fail` fails (which fails the scope).
    ///
    /// The handler runs under a node of its own below the scope's, not
    /// under the failed task's: that task has finished, and cancelling a
    /// finished task's handle must not stop the handler. A failure whose
    /// `on_fail` was cancelled goes back to the queue, so the scope
    /// reports it when it closes.
    fn handle(&mut self, s: &ScopeState, sv: &Supervisor) {
        let node = s.node.child();
        let own = std::mem::replace(&mut self.cancel, node.clone());
        while !self.cancel.is_cancelled() {
            let Some((e, text)) = sv.queue.borrow_mut().pop_front() else {
                break;
            };
            let Some(f) = sv.on_fail.borrow().clone() else {
                break;
            };
            match self.call(&f, vec![e.clone()]) {
                Err(Stop::Error(d)) if !is_cancel(&d) => {
                    s.fail(&d);
                    break;
                }
                Err(_) => {
                    sv.queue.borrow_mut().push_front((e, text));
                    break;
                }
                Ok(_) => {}
            }
        }
        self.cancel = own;
        node.cancel();
    }
}

/// The value `on_fail` gets for a failure: the map `await` gives inside
/// its `Err`, `{kind: 'error, message}`.
fn failure_value(d: &Diag) -> Value {
    let mut m = MapBuilder::default();
    m.set(Rc::new("kind".to_string()), Value::Sym(intern("error")));
    m.set(Rc::new("message".to_string()), Value::str(&headline(d)));
    Value::Map(Rc::new(m.finish()))
}

/// `scope(f)`: opens a scope, runs `f(s)`, waits for the scope's children
/// and gives the result (05 "범위"), in this order:
/// - a child's failure, raised here;
/// - the body's own runtime error or exit;
/// - a cancellation from outside, unwinding further (E0701, or E0702
///   after a deadlock report), even if the body had returned;
/// - `cancel(s)`: `Err({kind: 'cancelled})`;
/// - the body's value.
pub(crate) fn run_scope(t: &mut Task<'_>, f: Value, on_fail: Option<Value>) -> Result<Value, Stop> {
    // The innermost line of user code: a scope a stdlib function opens
    // (`conc.pmap`) is named by the user's call of it.
    let opened_at = t
        .user_pos()
        .map(|p| format!("{}:{}", t.world.file_name(p.file), p.line))
        .unwrap_or_default();
    let node = t.cancel.child();
    let state = match on_fail {
        None => ScopeState::fail_fast(node.clone()),
        Some(h) => ScopeState::supervise(node.clone(), h),
    };
    let handle = Value::Scope(Rc::new(ScopeHandle {
        state: state.clone(),
    }));
    let outer = std::mem::replace(&mut t.cancel, node.clone());
    let r = t.call(&f, vec![handle]);
    t.cancel = outer;
    if r.is_err() {
        node.cancel();
    }
    drain(t, &state);
    state.open.set(false);
    if let Policy::Supervise(sv) = &state.policy {
        sv.on_fail.borrow_mut().take();
        // Failures the scope was cancelled before handling: reported as
        // a failure nobody saw (the exit code becomes 1).
        for (_, text) in sv.queue.borrow_mut().drain(..) {
            t.shared.failures.report_now(&text);
        }
    }
    // Out of the opener's node's list: nothing runs below it any more.
    node.cancel();
    let outer = t.cancel.is_cancelled().then(|| t.unwound());
    settle(&state, r, &opened_at, outer)
}

/// What `scope(` gives once the scope has closed, from the body's result
/// `r` and `outer`, the stop of a cancellation from outside (if any).
pub(crate) fn settle(
    state: &ScopeState,
    r: Result<Value, Stop>,
    opened_at: &str,
    outer: Option<Stop>,
) -> Result<Value, Stop> {
    if let Some(mut d) = state.take_failure(opened_at) {
        // The body's own failure is kept too, not dropped.
        if let Err(Stop::Error(b)) = &r
            && !is_cancel(b)
        {
            d.notes.push(format!("note: also failed: {}", headline(b)));
        }
        return Err(Stop::Error(d));
    }
    match r {
        // The body's own error or exit stands.
        Err(Stop::Exit(_)) => r,
        Err(Stop::Error(ref d)) if !is_cancel(d) => r,
        // A cancellation from outside unwinds on; if the body unwound on
        // it, its stop keeps the place the body stopped at.
        Err(_) if outer.is_some() => r,
        _ if outer.is_some() => Err(outer.expect("checked")),
        _ if state.self_cancelled.get() => Ok(err_kind("cancelled")),
        _ => r,
    }
}

/// Waits, without being cancellable, until the scope has no live child.
fn drain(t: &mut Task<'_>, state: &Rc<ScopeState>) {
    let outer = std::mem::replace(&mut t.cancel, CancelNode::root());
    let r = block(
        t,
        |_, me| {
            if state.live.get() == 0 {
                return Some(());
            }
            state.waiters.borrow_mut().add(me);
            None
        },
        None,
    );
    t.cancel = outer;
    debug_assert!(matches!(r, Ok(Some(()))), "a scope's wait ended early");
}
