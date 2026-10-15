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

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use fung_front::diag::{Arg, headline};

use super::world::Shared;
use super::{Stop, Task, internal, token};
use crate::native::nat;
use crate::value::{MapBuilder, Value, intern};

/// The stack each spawned task's thread reserves. fung calls do not use
/// it (frames live on the VM's heap); only natives that call back into
/// fung (`map`, `fold`, …) nest Rust frames, and formatting or comparing
/// deeply nested values recurses. It is address space, not memory: the
/// pages are touched only when used. The entry keeps its 4 GiB stack.
pub(crate) const TASK_STACK: usize = 256 << 20;

/// A node of the cancellation tree. Only the flag is read on the hot path
/// (one relaxed load per call); the child list is touched when a task is
/// spawned or cancelled.
pub(crate) struct CancelNode {
    cancelled: AtomicBool,
    /// The nodes below this one. Weak: a node lives as long as its task
    /// runs or its handle is held, not as long as its parent.
    children: Mutex<Vec<Weak<CancelNode>>>,
}

impl CancelNode {
    pub(crate) fn root() -> Arc<CancelNode> {
        Arc::new(CancelNode {
            cancelled: AtomicBool::new(false),
            children: Mutex::new(Vec::new()),
        })
    }

    /// Whether this node or one above it was cancelled.
    #[inline(always)]
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// A new node below this one; already cancelled if this one is.
    pub(crate) fn child(&self) -> Arc<CancelNode> {
        let c = CancelNode::root();
        let mut kids = self.children.lock().unwrap_or_else(PoisonError::into_inner);
        // Checked under the list's lock: `cancel` sets the flag before it
        // takes the list, so a child is either in the list it takes or
        // sees the flag here.
        if self.is_cancelled() {
            c.cancelled.store(true, Ordering::Relaxed);
            return c;
        }
        // Drop the entries of nodes that are gone once the list doubles,
        // so a long-lived parent spawning many short tasks stays small.
        if kids.len() == kids.capacity() {
            kids.retain(|w| w.strong_count() > 0);
        }
        kids.push(Arc::downgrade(&c));
        c
    }

    /// Cancels this node and every node below it (without recursion: a
    /// chain of nested spawns can be long). Wake blocked tasks afterwards
    /// (`Shared::notify_all`).
    pub(crate) fn cancel(&self) {
        let mut todo = self.take_children();
        while let Some(w) = todo.pop() {
            if let Some(n) = w.upgrade() {
                todo.extend(n.take_children());
            }
        }
    }

    /// Sets the flag and takes the child list (empty if it was set).
    fn take_children(&self) -> Vec<Weak<CancelNode>> {
        if self.cancelled.swap(true, Ordering::SeqCst) {
            return Vec::new();
        }
        std::mem::take(&mut *self.children.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

/// The value `spawn` returns (Go `TaskHandle`): the task's result once
/// it has finished, and its cancellation node. Two handles are equal
/// only when they are the same handle; it prints as `<task>`.
pub struct TaskHandle {
    pub(crate) state: Rc<TaskState>,
    pub(crate) cancel: Arc<CancelNode>,
}

impl std::fmt::Debug for TaskHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<task>")
    }
}

/// A handle is a value: it drops only on the token's holder.
impl Drop for TaskHandle {
    fn drop(&mut self) {
        token::assert_holds_token();
    }
}

/// What the handle and the task's thread share: the result cell.
pub(crate) struct TaskState {
    /// Set once, when the task finishes. Results are kept: `await` can
    /// be called any number of times.
    result: RefCell<Option<Value>>,
    /// Spawned by a helper that awaits it (`_native.conc.spawn_awaited`):
    /// its failure is never reported as unretrieved (Task 6).
    #[allow(dead_code)]
    pub(crate) awaited: bool,
}

impl TaskState {
    /// The result, if the task has finished.
    pub(crate) fn result(&self) -> Option<Value> {
        self.result.borrow().clone()
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
fn outcome(r: Result<Value, Stop>) -> Value {
    match r {
        Ok(v) => v,
        Err(Stop::Exit(_)) => err_kind("cancelled"),
        Err(Stop::Error(d)) if d.code == "E0701" || d.code == "E0702" => err_kind("cancelled"),
        Err(Stop::Error(d)) => nat::err_value("error", &headline(&d), Vec::new()),
    }
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
            awaited,
        });
        let handle = Rc::new(TaskHandle {
            state: state.clone(),
            cancel: node.clone(),
        });
        let cargo = Handoff(Some((f, state)));
        let shared = Arc::clone(self.shared);
        self.world.running += 1;
        let started = std::thread::Builder::new()
            .name("fung-task".into())
            .stack_size(TASK_STACK)
            .spawn(move || run_spawned(shared, node, cargo));
        if let Err(e) = started {
            // The closure, and the values in it, were dropped here, on
            // this thread, with the token held.
            self.world.running -= 1;
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
        self.shared.notify_all();
        r
    }

    /// Wakes the blocked tasks to check their conditions again, after a
    /// change they may wait for (a cancellation, later a channel send).
    pub(crate) fn notify_all(&self) {
        self.shared.notify_all();
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
        let mut t = Task::new(&shared, node.clone());
        token::assert_holds_token();
        let (f, state) = cargo.open(&t);
        let r = t.call(&f, Vec::new());
        drop(f);
        *state.result.borrow_mut() = Some(outcome(r));
        drop(state);
        // Go `defer child.cancel()`: what this task spawned and left
        // running ends with it.
        node.cancel();
        t.world.running -= 1;
        shared.notify_all();
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
