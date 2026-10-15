//! Tasks (05, stage 3a; Go `internal/vm/task.go`): `spawn` runs a
//! function on a new operating-system thread and hands back a handle;
//! the handle keeps the task's result for `await` and cancels it.
//!
//! **Threads and the run token.** Every task has its own thread, and fung
//! code runs only on the thread that holds the run token (`world.rs`).
//! A spawned thread's life is:
//! 1. take the token (in line, `Gate`);
//! 2. take the function and the result cell out of the `Handoff`;
//! 3. run the function behind the panic fence (`Task::call`);
//! 4. store the result, drop every value it holds (the function, the
//!    result cell, its stack and frames), cancel its own subtree and
//!    count itself finished, all with the token still held;
//! 5. give the token up; the thread ends.
//!
//! **Cancellation** is a tree like Go's `context`: each task has a
//! `CancelNode`, a child of the node of the task that spawned it.
//! Cancelling a node cancels everything below it. A task's node is also
//! cancelled when the task finishes (Go `defer child.cancel()`), so the
//! tasks it spawned and left running end with it. The VM checks the flag
//! at every call and backward jump (`Task::tick`) and `block` checks it
//! while waiting; a cancelled task unwinds with E0701, which its handle
//! turns into `Err({kind: 'cancelled})`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use fung_front::diag::{self, Arg, Diag, headline};

use super::block::{WaitList, Waiter};
use super::failure::Failures;
use super::world::Shared;
use super::{Stop, Task, internal, token};
use crate::native::nat;
use crate::value::{MapBuilder, Value, drop_children, intern, keep};

/// The stack each spawned task's thread reserves. fung calls do not use
/// it (frames live on the VM's heap); only natives that call back into
/// fung (`map`, `fold`, …) nest Rust frames, and formatting or comparing
/// deeply nested values recurses. It is address space, not memory: the
/// pages are touched only when used. The entry keeps its 4 GiB stack.
pub(crate) const TASK_STACK: usize = 256 << 20;

/// A node of the cancellation tree. Only the flag is read on the hot path
/// (one relaxed load per call); the rest is touched when a task is
/// spawned, blocks, finishes or is cancelled.
///
/// A parent holds its children strongly and a child leaves its parent's
/// list when it is cancelled, as in Go's `context`. The other way to keep
/// the tree whole, cancelling a node's children when the node drops,
/// would end a task just because the task or handle above it went away;
/// with strong links nothing below a live node can become unreachable
/// from the roots, whichever tasks, handles or intermediate nodes are
/// gone, so cancelling a root reaches every task under it. Nodes do not
/// pile up: every spawned task's node is cancelled when the task ends,
/// and the nodes made for test blocks, exit handlers and `run_task` are
/// cancelled when they return.
pub(crate) struct CancelNode {
    cancelled: AtomicBool,
    parent: Weak<CancelNode>,
    inner: Mutex<NodeInner>,
}

#[derive(Default)]
struct NodeInner {
    /// The live children by address.
    children: HashMap<usize, Arc<CancelNode>>,
    /// The tasks blocked under this node, woken when it is cancelled.
    watchers: WaitList,
}

impl CancelNode {
    pub(crate) fn root() -> Arc<CancelNode> {
        CancelNode::under(Weak::new())
    }

    fn under(parent: Weak<CancelNode>) -> Arc<CancelNode> {
        Arc::new(CancelNode {
            cancelled: AtomicBool::new(false),
            parent,
            inner: Mutex::new(NodeInner::default()),
        })
    }

    fn inner(&self) -> std::sync::MutexGuard<'_, NodeInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether this node or one above it was cancelled.
    #[inline(always)]
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// A new node below this one; already cancelled if this one is.
    pub(crate) fn child(self: &Arc<Self>) -> Arc<CancelNode> {
        let c = CancelNode::under(Arc::downgrade(self));
        let mut inner = self.inner();
        // Checked under the lock: `cancel` sets the flag before it takes
        // the children, so a child is either among them or sees the flag.
        if self.is_cancelled() {
            c.cancelled.store(true, Ordering::Relaxed);
        } else {
            inner.children.insert(Arc::as_ptr(&c) as usize, c.clone());
        }
        c
    }

    /// The number of live children (tests).
    #[cfg(test)]
    pub(crate) fn child_count(&self) -> usize {
        self.inner().children.len()
    }

    /// Wakes `w` when this node is cancelled (`block`). Already cancelled
    /// nodes are checked by the caller after registering.
    pub(crate) fn watch(&self, w: &Arc<Waiter>) {
        self.inner().watchers.add(w);
    }

    /// Cancels this node and every node below it (without recursion: a
    /// chain of nested spawns can be long), wakes the tasks blocked under
    /// them, and takes this node out of its parent's list.
    pub(crate) fn cancel(&self) {
        let Some(mut todo) = self.take() else {
            return;
        };
        if let Some(p) = self.parent.upgrade() {
            p.inner()
                .children
                .remove(&(self as *const CancelNode as usize));
        }
        while let Some(n) = todo.pop() {
            if let Some(more) = n.take() {
                todo.extend(more);
            }
        }
    }

    /// Sets the flag, wakes this node's watchers and takes its children;
    /// `None` if it was already cancelled.
    fn take(&self) -> Option<Vec<Arc<CancelNode>>> {
        if self.cancelled.swap(true, Ordering::SeqCst) {
            return None;
        }
        let mut inner = self.inner();
        inner.watchers.wake_all();
        Some(std::mem::take(&mut inner.children).into_values().collect())
    }
}

/// The value `spawn` returns (Go `TaskHandle`): the task's result once
/// it has finished, and its cancellation node. Two handles are equal
/// only when they are the same handle; it prints as `<task>`.
pub struct TaskHandle {
    pub(crate) state: Rc<TaskState>,
    pub(crate) cancel: Arc<CancelNode>,
    /// Where a failure is reported when this handle drops (`failure.rs`).
    failures: Arc<Failures>,
}

impl std::fmt::Debug for TaskHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<task>")
    }
}

/// A handle is a value: it drops only on the token's holder. With it goes
/// the last way to retrieve the task's result (every copy of the value
/// shares this one handle), so a failure nobody retrieved is reported
/// now, or when the task fails if it is still running (`failure.rs`).
impl Drop for TaskHandle {
    fn drop(&mut self) {
        token::assert_holds_token();
        self.state.orphan.set(true);
        if let Some(n) = self.state.failure.take() {
            self.failures.report(n);
        }
    }
}

impl TaskHandle {
    /// A handle of a finished task with result `v`, without a thread
    /// (tests of value lifetimes).
    #[cfg(test)]
    pub(crate) fn finished(v: Value, cancel: Arc<CancelNode>, failures: Arc<Failures>) -> Value {
        Value::Task(Rc::new(TaskHandle {
            state: Rc::new(TaskState {
                result: RefCell::new(Some(v)),
                waiters: RefCell::new(WaitList::default()),
                awaited: false,
                failure: Cell::new(None),
                orphan: Cell::new(false),
            }),
            cancel,
            failures,
        }))
    }

    /// The task's result, if it has finished, as `await` gets it: a
    /// failure in it is now the caller's to handle (Go `retrieve`).
    pub(crate) fn retrieve(&self) -> Option<Value> {
        let r = self.state.result();
        if r.is_some()
            && let Some(n) = self.state.failure.take()
        {
            self.failures.untrack(n);
        }
        r
    }
}

/// What the handle and the task's thread share: the result cell.
pub(crate) struct TaskState {
    /// Set once, when the task finishes. Results are kept: `await` can
    /// be called any number of times.
    result: RefCell<Option<Value>>,
    /// The tasks waiting for the result (`await`), woken when it is set.
    pub(crate) waiters: RefCell<WaitList>,
    /// Spawned by a helper that awaits it (`_native.conc.spawn_awaited`):
    /// its failure counts as retrieved from the start.
    awaited: bool,
    /// The number of the task's failure in `Failures` while it is neither
    /// retrieved nor reported.
    failure: Cell<Option<u64>>,
    /// The handle is gone: a failure is reported as it happens.
    orphan: Cell<bool>,
}

impl TaskState {
    /// The result, if the task has finished.
    pub(crate) fn result(&self) -> Option<Value> {
        self.result.borrow().clone()
    }

    /// Takes the result out (`value/release.rs`).
    pub(crate) fn take_result(&self) -> Option<Value> {
        self.result.borrow_mut().take()
    }
}

/// A result holding another task's handle, whose result holds another,
/// and so on, drops in constant stack (`value/release.rs`).
impl Drop for TaskState {
    fn drop(&mut self) {
        if let Some(v) = self.result.get_mut().take() {
            drop_children(|s| keep(s, v));
        }
    }
}

/// The function and the result cell a parent hands to the thread of the
/// task it spawns. They are fung values (`Rc`s), so they are not `Send`;
/// this wrapper is the one place they cross to another thread.
struct Handoff(Option<(Value, Rc<TaskState>)>);

// SAFETY: the run-token rule (constraints: 실행권 규칙) is what makes moving
// these `Rc`s sound. The parent builds the `Handoff` while it holds the
// token. Moving it into the new thread copies the pointers without
// touching the counts. The only way to reach the contents is `open`,
// which the new thread calls only after it has taken the token (and
// which asserts it in debug builds). Until then nothing can reach these
// `Rc`s from the new thread, and the parent's clones are touched only by
// whoever holds the token, as always. If the thread never starts, the
// closure holding the `Handoff` is dropped by `Builder::spawn` on the
// parent's thread, which still holds the token; `Drop` asserts that.
unsafe impl Send for Handoff {}

impl Handoff {
    /// The contents, for the spawned task's thread once it holds the
    /// token (`_t` is the proof: a `Task` holds it).
    fn open(mut self, _t: &Task<'_>) -> (Value, Rc<TaskState>) {
        token::assert_holds_token();
        self.0.take().expect("a handoff opened twice")
    }
}

impl Drop for Handoff {
    fn drop(&mut self) {
        if self.0.is_some() {
            token::assert_holds_token();
        }
    }
}

/// `Err({kind: 'k})`, the result of a cancelled task or a timed-out wait.
pub(crate) fn err_kind(kind: &str) -> Value {
    let mut m = MapBuilder::default();
    m.set(Rc::new("kind".to_string()), Value::Sym(intern(kind)));
    nat::tag("Err", vec![Value::Map(Rc::new(m.finish()))])
}

/// What a task's run becomes as its result (Go `finish`): the value, or
/// `Err({kind: 'cancelled})` for a cancelled task (E0701), one woken by
/// the deadlock report (E0702) or one that called `exit`, or
/// `Err({kind: 'error, message: "error[E…]: …"})` for a failure.
fn outcome(r: &Result<Value, Stop>) -> Value {
    match r {
        Ok(v) => v.clone(),
        Err(Stop::Exit(_)) => err_kind("cancelled"),
        Err(Stop::Error(d)) if is_cancel(d) => err_kind("cancelled"),
        Err(Stop::Error(d)) => nat::err_value("error", &headline(d), Vec::new()),
    }
}

/// Cancelled, or woken by the deadlock report: not the task's failure.
fn is_cancel(d: &Diag) -> bool {
    d.code == "E0701" || d.code == "E0702"
}

impl<'rt> Task<'rt> {
    /// Starts `f` on a new task below this one and returns its handle
    /// (Go `spawn`). `awaited` marks a task a helper promises to await
    /// (`spawn_awaited`).
    pub(crate) fn spawn(&mut self, f: Value, awaited: bool) -> Result<Value, Stop> {
        token::assert_holds_token();
        let node = self.cancel.child();
        let state = Rc::new(TaskState {
            result: RefCell::new(None),
            waiters: RefCell::new(WaitList::default()),
            awaited,
            failure: Cell::new(None),
            orphan: Cell::new(false),
        });
        let handle = Rc::new(TaskHandle {
            state: state.clone(),
            cancel: node.clone(),
            failures: self.shared.failures.clone(),
        });
        let cargo = Handoff(Some((f, state)));
        let shared = Arc::clone(self.shared);
        self.world.running += 1;
        // Live from now (Go `addLive` before the goroutine starts): a
        // task whose thread has not taken the token yet will still run.
        self.world.live += 1;
        let started = std::thread::Builder::new()
            .name("fung-task".into())
            .stack_size(TASK_STACK)
            .spawn(move || run_spawned(shared, node, cargo));
        if let Err(e) = started {
            // The closure, and the values in it, were dropped here, on
            // this thread, with the token held.
            self.world.running -= 1;
            self.world.live -= 1;
            return Err(internal(vec![Arg::Str(format!(
                "cannot start a task: {e}"
            ))]));
        }
        Ok(Value::Task(handle))
    }

    /// Runs `f` on a fresh child task on this thread and returns its
    /// result (Go `RunTask`, for natives that run fung code per event,
    /// such as an HTTP handler): the call sees this task's cancellation
    /// and its own, which ends when it returns.
    pub fn run_task(&mut self, f: &Value, args: Vec<Value>) -> Result<Value, Stop> {
        let child = self.cancel.child();
        let outer = std::mem::replace(&mut self.cancel, child.clone());
        let r = self.call(f, args);
        self.cancel = outer;
        child.cancel();
        r
    }

    /// Whether this task was cancelled (Go `Done()` being ready), for
    /// natives that wait outside fung.
    pub fn cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

/// The body of a spawned task's thread (see the module doc for the
/// order). A panic that escapes the panic fence here is a bug in this
/// file; the values would then be dropped in an unknown state, so the
/// process stops instead.
fn run_spawned(shared: Arc<Shared>, node: Arc<CancelNode>, cargo: Handoff) {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut t = Task::spawned(&shared, node.clone());
        token::assert_holds_token();
        let (f, state) = cargo.open(&t);
        let r = t.call(&f, Vec::new());
        drop(f);
        *state.result.borrow_mut() = Some(outcome(&r));
        if let Err(Stop::Error(d)) = &r
            && !is_cancel(d)
            && !state.awaited
        {
            // Go `finish`: a failure someone must see (`failure.rs`).
            let text = format!("task failed: {}", diag::format(d, &t.world.sources));
            if state.orphan.get() {
                shared.failures.report_now(&text);
            } else {
                state.failure.set(Some(shared.failures.track(text)));
            }
        }
        drop(r);
        state.waiters.borrow_mut().wake_all();
        drop(state);
        // Go `defer child.cancel()`: what this task spawned and left
        // running ends with it.
        node.cancel();
        t.world.running -= 1;
        if t.world.running == 0 {
            t.world.idle.wake_all();
        }
        token::assert_holds_token();
        // Dropping the task empties its stack and frames, then gives the
        // token up.
        drop(t);
    }));
    if r.is_err() {
        eprintln!("fung: internal error: a task thread panicked outside the panic fence");
        std::process::abort();
    }
}
