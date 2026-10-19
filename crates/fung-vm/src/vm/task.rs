//! Tasks (05, stage 3a): `spawn(s, f)` runs a function on a new
//! operating-system thread inside scope `s` (`scope.rs`) and hands back a
//! handle; the handle keeps the task's result for `await` and cancels it.
//!
//! **Fibers.** A spawned task is a `Fiber` the scheduler runs on the
//! run thread (`sched.rs`): its function is called with its end waiting
//! below it (`scope::TaskEnd`), which stores the result, tells the scope,
//! runs a supervisor's handler and cancels the task's node.
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
//! the program's root (`Shared::root`) and ends before the program does,
//! since a scope does not return until its children have ended.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::Instant;

use fung_front::diag::{Diag, headline};

use super::block::{WaitList, Waiter};
use super::deadline::Deadline;
use super::scope::ScopeState;
use super::{Stop, Task};
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
/// blocks are cancelled when they return.
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
}

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
    pub(super) result: RefCell<Option<Value>>,
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
pub(super) fn outcome(r: &Result<Value, Stop>) -> Value {
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
        // The new task is a fiber the scheduler takes in after this turn
        // (`sched.rs`): no thread, so starting one cannot fail. Its end
        // waits below its function (`scope::TaskEnd`).
        let mut fiber = super::Fiber::new(self.shared, node.clone());
        fiber.conts.push(super::cont::ContFrame {
            depth: 0,
            height: 0,
            pos: super::no_pos(),
            native: None,
            tail: false,
            k: Box::new(super::scope::TaskEnd::new(state, scope, node)),
        });
        fiber.todo = Some(super::Todo::Start(f, 0));
        self.world.new_fibers.push(fiber);
        Ok(Value::Task(handle))
    }

    /// Whether this task was cancelled, for natives that wait outside fung.
    pub fn cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}
