//! Tasks (05, stage 3a): `spawn(s, f)` runs a function on a new
//! operating-system thread inside scope `s` (`scope.rs`) and hands back a
//! handle; the handle keeps the task's result for `await` and cancels it.
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
//! **Cancellation** is a tree: each task has a `CancelNode`, a child of its
//! scope's node, which is a child of the node of the task that opened it. Cancelling a node cancels everything
//! below it. A task's node is also cancelled when the task finishes, so the
//! tasks it spawned and left running end with it. The VM checks the flag at
//! every call and backward jump (`Task::tick`) and `block` checks it while
//! waiting; a cancelled task unwinds with E0701, which its handle turns
//! into `Err({kind: 'cancelled})`.
//!
//! Every task belongs to the program: it hangs, through its scopes, from
//! the program's root (`Shared::root`), is counted in `World::running`
//! and `World::live`, and ends before the program does, since a scope
//! does not return until its children have ended.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::Instant;

use fung_front::diag::{Arg, Diag, headline};

use super::block::{WaitList, Waiter};
use super::deadline::Deadline;
use super::scope::ScopeState;
use super::world::Shared;
use super::{Stop, Task, internal, token};
use crate::native::nat;
use crate::value::{MapBuilder, Value, drop_children, intern, keep};

/// A node of the cancellation tree. Only the flag is read on the hot path
/// (one relaxed load per call); the rest is touched when a task is
/// spawned, blocks, finishes or is cancelled.
///
/// A parent holds its children strongly and a child leaves its parent's
/// list when it is cancelled. The other way to keep the tree whole,
/// cancelling a node's children when the node drops, would end a task just
/// because the task or handle above it went away; with strong links nothing
/// below a live node can become unreachable from the roots, whichever
/// tasks, handles or intermediate nodes are gone, so cancelling a root
/// reaches every task under it. Nodes do not pile up: every spawned task's
/// node is cancelled when the task ends, and the nodes made for test
/// blocks and `run_task` are cancelled when they return.
pub(crate) struct CancelNode {
    cancelled: AtomicBool,
    parent: Weak<CancelNode>,
    inner: Mutex<NodeInner>,
    /// The earliest deadline over this node (`deadline.rs`): its own (a
    /// `timeout`'s node) or the one it inherited from its parent.
    deadline: Option<Deadline>,
    /// Cancelled because its own deadline passed (`expire`).
    expired: AtomicBool,
}

#[derive(Default)]
struct NodeInner {
    /// The live children by address.
    children: HashMap<usize, Arc<CancelNode>>,
    /// The tasks blocked under this node, woken when it is cancelled.
    watchers: WaitList,
    /// Run once when this node is cancelled (`on_cancel`, stage 4d): what
    /// wakes a task that waits on the operating system (`watch.rs`).
    hooks: Vec<(u64, Hook)>,
}

/// What `CancelNode::on_cancel` registers. It runs on the cancelling
/// thread, under the node's lock, so it must be short and take no lock.
pub(crate) type Hook = Box<dyn FnOnce() + Send>;

impl CancelNode {
    pub(crate) fn root() -> Arc<CancelNode> {
        CancelNode::under(Weak::new(), None)
    }

    fn under(parent: Weak<CancelNode>, deadline: Option<Deadline>) -> Arc<CancelNode> {
        Arc::new(CancelNode {
            cancelled: AtomicBool::new(false),
            parent,
            inner: Mutex::new(NodeInner::default()),
            deadline,
            expired: AtomicBool::new(false),
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
        let c = CancelNode::under(Arc::downgrade(self), self.deadline.clone());
        self.adopt(c)
    }

    /// A child of this node with a deadline of its own at `at` (a
    /// `timeout`): its deadline is the earlier of `at` and this node's,
    /// and it is the owner of `at`. Already expired if `at` has passed.
    pub(crate) fn with_deadline(self: &Arc<Self>, at: Option<Instant>) -> Arc<CancelNode> {
        let inherited = self.deadline.clone();
        let c = Arc::new_cyclic(|me| CancelNode {
            cancelled: AtomicBool::new(false),
            parent: Arc::downgrade(self),
            inner: Mutex::new(NodeInner::default()),
            deadline: match (inherited, at) {
                (Some(d), Some(a)) if d.at <= a => Some(d),
                (_, Some(a)) => Some(Deadline::new(a, me.clone())),
                (d, None) => d,
            },
            expired: AtomicBool::new(false),
        });
        let c = self.adopt(c);
        // Already past: expire the owner of the deadline in force, which
        // is an outer one if that passed first.
        if at.is_some_and(|a| Instant::now() >= a) {
            c.expire_due(Instant::now());
        }
        c
    }

    /// When this node's deadline passes, if it has one.
    #[inline]
    pub(crate) fn deadline_at(&self) -> Option<Instant> {
        self.deadline.as_ref().map(|d| d.at)
    }

    /// If this node's deadline has passed by `now`, expires the node that
    /// owns it (and so everything below it) and says so.
    pub(crate) fn expire_due(&self, now: Instant) -> bool {
        match &self.deadline {
            Some(d) if now >= d.at => d.expire(),
            _ => false,
        }
    }

    /// Cancels this node because its own deadline passed.
    pub(crate) fn expire(&self) {
        self.expired.store(true, Ordering::SeqCst);
        self.cancel();
    }

    /// Whether this node was cancelled by its own deadline.
    pub(crate) fn expired(&self) -> bool {
        self.expired.load(Ordering::SeqCst)
    }

    /// Puts a new child `c` in this node's list (or cancels it at once if
    /// this node is cancelled).
    fn adopt(self: &Arc<Self>, c: Arc<CancelNode>) -> Arc<CancelNode> {
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

    /// The number of hooks registered (tests).
    #[cfg(test)]
    pub(crate) fn hook_count(&self) -> usize {
        self.inner().hooks.len()
    }

    /// The number of nodes below this one, through the primary lists
    /// (tests). Never holds two locks: each level is copied out first.
    #[cfg(test)]
    pub(crate) fn descendant_count(&self) -> usize {
        let mut todo: Vec<Arc<CancelNode>> = self.inner().children.values().cloned().collect();
        let mut n = 0;
        while let Some(c) = todo.pop() {
            n += 1;
            todo.extend(c.inner().children.values().cloned());
        }
        n
    }

    /// Wakes `w` when this node is cancelled (`block`). Already cancelled
    /// nodes are checked by the caller after registering.
    pub(crate) fn watch(&self, w: &Arc<Waiter>) {
        self.inner().watchers.add(w);
    }

    /// Registers `hook` (under `id`) to run when this node is cancelled,
    /// unless it already is: then it returns false and drops the hook.
    ///
    /// No cancellation is missed: `take` sets the flag before it takes
    /// the lock to run the hooks, and the flag is checked here under that
    /// lock. Either this registration holds the lock first, and `take`
    /// finds the hook and runs it, or the flag is already visible here.
    pub(crate) fn on_cancel(&self, id: u64, hook: Hook) -> bool {
        let mut inner = self.inner();
        if self.cancelled.load(Ordering::SeqCst) {
            return false;
        }
        inner.hooks.push((id, hook));
        true
    }

    /// Unregisters the hook `id`, dropping it unrun; nothing if it has
    /// run already. Once this returns, the hook is not running and never
    /// will (both happen under the node's lock).
    pub(crate) fn remove_hook(&self, id: u64) {
        let hook = {
            let mut inner = self.inner();
            let i = inner.hooks.iter().position(|(h, _)| *h == id);
            i.map(|i| inner.hooks.swap_remove(i))
        };
        // dropped outside the lock (a hook owns a pipe end)
        drop(hook);
    }

    /// Cancels this node and every node below it (without recursion: a
    /// chain of nested spawns can be long), wakes the tasks blocked under
    /// them, and takes this node out of its parent's list (the nodes below
    /// it go with it).
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

    /// Sets the flag, wakes this node's watchers, runs its hooks and takes
    /// its children; `None` if it was already cancelled.
    fn take(&self) -> Option<Vec<Arc<CancelNode>>> {
        if self.cancelled.swap(true, Ordering::SeqCst) {
            return None;
        }
        let mut inner = self.inner();
        inner.watchers.wake_all();
        for (_, hook) in inner.hooks.drain(..) {
            hook();
        }
        Some(std::mem::take(&mut inner.children).into_values().collect())
    }
}

/// The value `spawn` returns: the task's result once it has finished, and
/// its cancellation node. Two handles are equal only when they are the same
/// handle; it prints as `<task>`.
pub struct TaskHandle {
    pub(crate) state: Rc<TaskState>,
    pub(crate) cancel: Arc<CancelNode>,
}

impl std::fmt::Debug for TaskHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<task>")
    }
}

impl TaskHandle {
    /// A handle of a finished task with result `v`, without a thread
    /// (tests of value lifetimes).
    #[cfg(test)]
    pub(crate) fn finished(v: Value, cancel: Arc<CancelNode>) -> Value {
        Value::Task(Rc::new(TaskHandle {
            state: Rc::new(TaskState {
                result: RefCell::new(Some(v)),
                waiters: RefCell::new(WaitList::default()),
            }),
            cancel,
        }))
    }
}

/// What the handle and the task's thread share: the result cell.
pub(crate) struct TaskState {
    /// Set once, when the task finishes. Results are kept: `await` can
    /// be called any number of times.
    result: RefCell<Option<Value>>,
    /// The tasks waiting for the result (`await`), woken when it is set.
    pub(crate) waiters: RefCell<WaitList>,
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
struct Handoff(Option<Cargo>);

/// The function, the result cell and the scope the task belongs to.
type Cargo = (Value, Rc<TaskState>, Option<Rc<ScopeState>>);

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
// Between attempts at stack sizes (`stack::spawn_sized`) the `Handoff`
// waits in a mutex slot the parent made; a started thread only moves it
// out (the mutex orders that move after the parent's), and when no
// attempt starts the parent takes it back and drops it, token held.
unsafe impl Send for Handoff {}

impl Handoff {
    /// The contents, for the spawned task's thread once it holds the
    /// token (`_t` is the proof: a `Task` holds it).
    fn open(mut self, _t: &Task<'_>) -> Cargo {
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

/// What a task's run becomes as its result: the value, or
/// `Err({kind: 'cancelled})` for a cancelled task (E0701), one woken by the
/// deadlock report (E0702) or one that called `exit`, or
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
pub(super) fn is_cancel(d: &Diag) -> bool {
    d.code == "E0701" || d.code == "E0702"
}

impl<'rt> Task<'rt> {
    /// Starts `f` on a new task in scope `s` (`spawn(s, g)`) and returns
    /// its handle; E0706 once the scope has closed. The task hangs below
    /// the scope's node, and the scope waits for it.
    pub(crate) fn spawn_in(&mut self, s: &Rc<ScopeState>, f: Value) -> Result<Value, Stop> {
        if !s.is_open() {
            return Err(nat::fail("E0706", Vec::new()));
        }
        let node = s.node.child();
        self.start(f, node, Some(s.clone()))
    }

    fn start(
        &mut self,
        f: Value,
        node: Arc<CancelNode>,
        scope: Option<Rc<ScopeState>>,
    ) -> Result<Value, Stop> {
        token::assert_holds_token();
        let state = Rc::new(TaskState {
            result: RefCell::new(None),
            waiters: RefCell::new(WaitList::default()),
        });
        let handle = Rc::new(TaskHandle {
            state: state.clone(),
            cancel: node.clone(),
        });
        if let Some(s) = &scope {
            s.child_started();
        }
        let in_scope = scope.clone();
        let cargo = Handoff(Some((f, state, scope)));
        let shared = Arc::clone(self.shared);
        // Counted from now, before the thread starts: a task whose thread
        // has not taken the token yet will still run.
        self.world.count(true);
        // The stack each spawned task's thread reserves: 1 GiB first, so
        // deep programs keep running in a task. fung calls do not use it
        // (frames live on the VM's heap); only natives that call back into
        // fung (`map`, `fold`, …) nest Rust frames, and formatting or
        // comparing deeply nested values recurses. It is address space, not
        // memory: the pages are touched only when used (with 256 MiB, `map`
        // calling back 99,000 deep overflowed and aborted the process where
        // the entry thread succeeds). A host that refuses the reservation
        // gets a smaller stack (`stack::TASK_STACKS`, R3a 2절); only when
        // every size is refused does `spawn` fail, with an error rather
        // than an abort.
        let started = crate::stack::spawn_sized(
            || std::thread::Builder::new().name("fung-task".into()),
            shared.task_stacks(),
            (shared, node, cargo),
            |(shared, node, cargo)| run_spawned(shared, node, cargo),
        );
        if let Err((e, payload)) = started {
            // The values are dropped here, on this thread, with the token
            // held.
            drop(payload);
            self.world.count(false);
            if let Some(s) = in_scope {
                s.child_done(&Ok(Value::Nil));
            }
            return Err(internal(vec![Arg::Str(format!(
                "cannot start a task: {e}"
            ))]));
        }
        Ok(Value::Task(handle))
    }

    /// Runs `f` on a fresh child task on this thread and returns its
    /// result (for natives that run fung code per event, such as an HTTP
    /// handler): the call sees this task's cancellation and its own, which
    /// ends when it returns.
    pub fn run_task(&mut self, f: &Value, args: Vec<Value>) -> Result<Value, Stop> {
        let child = self.cancel.child();
        let outer = std::mem::replace(&mut self.cancel, child.clone());
        let r = self.call(f, args);
        self.cancel = outer;
        child.cancel();
        r
    }

    /// Whether this task was cancelled, for natives that wait outside fung.
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
        let (f, state, scope) = cargo.open(&t);
        let r = t.call(&f, Vec::new());
        drop(f);
        *state.result.borrow_mut() = Some(outcome(&r));
        state.waiters.borrow_mut().wake_all();
        // The scope learns of the end once the result is stored.
        if let Some(s) = scope {
            t.end_in_scope(&s, &r);
        }
        drop(r);
        drop(state);
        // What this task spawned and left running ends with it.
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
        use std::io::Write as _;
        // `writeln!` with the result dropped, not `eprintln!`: with standard
        // error closed `eprintln!` panics, and `abort` must still run.
        let _ = writeln!(
            std::io::stderr(),
            "fung: internal error: a task thread panicked outside the panic fence"
        );
        std::process::abort();
    }
}
