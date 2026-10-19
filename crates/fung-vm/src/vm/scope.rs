//! Scopes (concurrency v2, L1; 05 "범위"): tasks start only inside one.
//!
//! `scope(f)` opens a scope, calls `f(s)` on the task that opened it and
//! returns only once that body and every task spawned into the scope
//! (`spawn(s, g)`) have ended.
//!
//! **The cancellation node.** A scope has a node of its own, below the
//! node of the task that opened it, and its children's nodes hang below
//! it. The body runs under it too (the task's node is swapped while the
//! body runs, as `timeout` does), so cancelling the scope reaches the body and
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
use super::cont::{Cont, Flow, Next};
use super::task::{CancelNode, err_kind, is_cancel};
use super::{Stop, Task, Wake};
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
/// run thread touches it.
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

    /// A child ended, or failed to start. Called on the run thread, by the
    /// child's task once its result is stored.
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

/// A spawned task's end, waiting below its function (`call_root_with`):
/// what the task does once its function has returned or unwound, in this
/// order:
/// 1. the result is stored and the tasks awaiting it are woken;
/// 2. in a `supervise` scope a real failure is queued, and if nobody is
///    handling, this task becomes the handler: it runs `on_fail` for each
///    queued failure, one at a time and in order, until the queue is
///    empty, the scope is cancelled, or `on_fail` fails (which fails the
///    scope). The handler runs under a node of its own below the scope's,
///    not under the failed task's: that task has finished, and cancelling
///    a finished task's handle must not stop the handler. A failure
///    `on_fail` was called for has been handed over, even if `on_fail` is
///    then cancelled or exits: only the ones still queued are reported
///    when the scope closes (05 "범위");
/// 3. the scope learns of the end and the task's node is cancelled (what
///    it spawned and left running ends with it).
///
/// The handler's calls are this box's calls, so a handler that waits
/// parks like any other wait.
pub(super) struct TaskEnd {
    state: Rc<super::task::TaskState>,
    scope: Option<Rc<ScopeState>>,
    node: Arc<CancelNode>,
    /// While handling: the body's result and the task's own node
    /// (swapped for the handler's).
    handling: Option<Handling>,
}

struct Handling {
    r: Result<Value, Stop>,
    own: Arc<CancelNode>,
    handler: Arc<CancelNode>,
}

impl TaskEnd {
    pub(super) fn new(
        state: Rc<super::task::TaskState>,
        scope: Option<Rc<ScopeState>>,
        node: Arc<CancelNode>,
    ) -> TaskEnd {
        TaskEnd {
            state,
            scope,
            node,
            handling: None,
        }
    }

    /// The function came to `r`.
    fn body_ended(&mut self, t: &mut Task<'_>, r: Result<Value, Stop>) -> Result<Next, Stop> {
        *self.state.result.borrow_mut() = Some(super::task::outcome(&r));
        self.state.waiters.borrow_mut().wake_all();
        let Some(s) = self.scope.clone() else {
            return Ok(self.ended(t, r));
        };
        if let Policy::Supervise(sv) = &s.policy
            && let Err(Stop::Error(d)) = &r
            && !is_cancel(d)
        {
            let text = format!("task failed: {}", diag::format(d, &t.world.sources));
            sv.queue.borrow_mut().push_back((failure_value(d), text));
            if !sv.handling.get() {
                sv.handling.set(true);
                let handler = s.node.child();
                let own = std::mem::replace(&mut t.cancel, handler.clone());
                self.handling = Some(Handling { r, own, handler });
                return Ok(self.next_failure(t, sv));
            }
        }
        Ok(self.ended(t, r))
    }

    /// Calls `on_fail` for the next queued failure, or stops handling.
    fn next_failure(&mut self, t: &mut Task<'_>, sv: &Supervisor) -> Next {
        if !t.cancel.is_cancelled()
            && let Some(f) = sv.on_fail.borrow().clone()
            && let Some((e, _)) = sv.queue.borrow_mut().pop_front()
        {
            t.push_arg(e);
            return Next::Call(f, 1);
        }
        self.stop_handling(t, sv)
    }

    /// Done handling: back under the task's own node, then the end.
    fn stop_handling(&mut self, t: &mut Task<'_>, sv: &Supervisor) -> Next {
        let h = self.handling.take().expect("handling");
        t.cancel = h.own;
        h.handler.cancel();
        sv.handling.set(false);
        self.ended(t, h.r)
    }

    /// `on_fail` came to `r` for the failure being handled. That failure
    /// was handed over whatever `r` is; a cancellation or an exit only
    /// stops the handling, leaving the rest of the queue to be reported.
    fn handled(&mut self, t: &mut Task<'_>, r: Result<Value, Stop>) -> Next {
        let s = self.scope.clone().expect("a supervised task");
        let Policy::Supervise(sv) = &s.policy else {
            unreachable!("handling outside a supervise scope")
        };
        match r {
            Ok(_) => self.next_failure(t, sv),
            Err(Stop::Error(d)) if !is_cancel(&d) => {
                s.fail(&d);
                self.stop_handling(t, sv)
            }
            Err(_) => self.stop_handling(t, sv),
        }
    }

    /// The scope learns of the end; the task's node is cancelled.
    fn ended(&mut self, _: &mut Task<'_>, r: Result<Value, Stop>) -> Next {
        if let Some(s) = self.scope.take() {
            s.child_done(&r);
        }
        drop(r);
        self.node.cancel();
        Next::Done(Value::Nil)
    }
}

/// A task dropped before its end (the REPL abandoned it,
/// `Sched::abandon`) ends as cancelled: a handle kept elsewhere awaits a
/// result, not a task that will never end.
impl Drop for TaskEnd {
    fn drop(&mut self) {
        if self.state.result.borrow().is_some() {
            return;
        }
        let r = Err(super::block::cancelled(super::no_pos()));
        *self.state.result.borrow_mut() = Some(super::task::outcome(&r));
        self.state.waiters.borrow_mut().wake_all();
    }
}

impl Cont for TaskEnd {
    fn resume(&mut self, t: &mut Task<'_>, v: Value) -> Result<Next, Stop> {
        if self.handling.is_some() {
            return Ok(self.handled(t, Ok(v)));
        }
        self.body_ended(t, Ok(v))
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        if self.handling.is_some() {
            return Ok(self.handled(t, Err(stop)));
        }
        self.body_ended(t, Err(stop))
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
///
/// The run loop calls the body (`cont.rs`); the rest happens when it
/// returns or unwinds (`Scope::close`).
pub(crate) fn run_scope(t: &mut Task<'_>, f: Value, on_fail: Option<Value>) -> Flow {
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
    t.push_arg(handle);
    Flow::Call(
        Box::new(Scope {
            node,
            state,
            outer: Some(outer),
            opened_at,
            draining: None,
        }),
        f,
        1,
    )
}

/// A scope whose body runs; the opener's node waits in `outer`.
struct Scope {
    node: Arc<CancelNode>,
    state: Rc<ScopeState>,
    outer: Option<Arc<CancelNode>>,
    opened_at: String,
    /// The body has ended and the scope waits for its children.
    draining: Option<Draining>,
}

/// A scope waiting for its children: the body's result, and the task's
/// own node, swapped for a fresh root while it waits.
struct Draining {
    r: Result<Value, Stop>,
    own: Arc<CancelNode>,
}

impl Scope {
    /// The body came to `r`: back under the opener's node, and the
    /// children are waited for.
    fn body_ended(&mut self, t: &mut Task<'_>, r: Result<Value, Stop>) -> Result<Next, Stop> {
        t.cancel = self.outer.take().expect("the opener's node");
        if r.is_err() {
            self.node.cancel();
        }
        self.wait(t, r)
    }

    /// Waits, without being cancellable, until the scope has no live
    /// child: the task waits under a fresh root, which nothing cancels
    /// (not even a deadlock report: it is no shield). A cancellation
    /// from outside still reaches the children, which hang below the
    /// scope's node; when they have unwound, the scope goes on with it.
    fn wait(&mut self, t: &mut Task<'_>, r: Result<Value, Stop>) -> Result<Next, Stop> {
        if self.state.live.get() == 0 {
            return self.closed(t, r);
        }
        let own = std::mem::replace(&mut t.cancel, CancelNode::root());
        self.watch(t);
        self.draining = Some(Draining { r, own });
        Ok(Next::Park(Wake::default()))
    }

    /// Registers the task to be woken when a child ends.
    fn watch(&self, t: &mut Task<'_>) {
        let me = t.waiter.clone();
        me.take_woken();
        self.state.waiters.borrow_mut().add(&me);
    }

    /// Every child has ended: the scope closes and settles.
    fn closed(&mut self, t: &mut Task<'_>, r: Result<Value, Stop>) -> Result<Next, Stop> {
        let (node, state) = (&self.node, &self.state);
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
        settle(state, r, &self.opened_at, outer).map(Next::Done)
    }
}

impl Cont for Scope {
    fn resume(&mut self, t: &mut Task<'_>, v: Value) -> Result<Next, Stop> {
        let Some(d) = self.draining.take() else {
            return self.body_ended(t, Ok(v));
        };
        // Woken while waiting for the children.
        if self.state.live.get() > 0 {
            self.watch(t);
            self.draining = Some(d);
            return Ok(Next::Park(Wake::default()));
        }
        t.cancel = d.own;
        self.closed(t, d.r)
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        let Some(d) = self.draining.take() else {
            return self.body_ended(t, Err(stop));
        };
        debug_assert!(false, "a scope's wait ended early");
        t.cancel = d.own;
        Err(stop)
    }
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
