//! The runtime side of the REPL (08, Go `PrepareREPL` and
//! `TestReplGlobalsSurviveGrowth`): every input is compiled on its own
//! against the names defined so far and runs on one runtime, whose entry
//! globals outlive each run.

use std::io::Cursor;

use fung_front::loader::{Loader, Stage};
use fung_front::source::{FileId, SourceMap};

use crate::testutil::{Buf, on_big_stack};
use crate::vm::task::CancelNode;
use crate::vm::{Runtime, Stop, Task};

/// A REPL session as the command drives it, minus the line reading.
struct Session {
    rt: Runtime,
    out: Buf,
    err: Buf,
    known: SourceMap,
    globals: Vec<String>,
}

/// What one input did: the value's text, or the error's code and file.
#[derive(Debug, PartialEq)]
enum Outcome {
    Value(String),
    Error(String, FileId),
    Exit(i64),
}

impl Session {
    fn new() -> Session {
        let (out, err) = (Buf::default(), Buf::default());
        let rt = Runtime::new(
            Box::new(out.clone()),
            Box::new(err.clone()),
            Box::new(Cursor::new(Vec::new())),
            SourceMap::new(),
        );
        rt.boot(Vec::new()).expect("boot");
        rt.prepare_repl();
        rt.hold();
        let known = rt.with_sources(SourceMap::clone);
        Session {
            rt,
            out,
            err,
            known,
            globals: Vec::new(),
        }
    }

    fn input(&mut self, src: &str) -> Outcome {
        let loader = Loader {
            globals: self.globals.clone(),
            redefine: true,
            ..Loader::default()
        };
        let from = self.known.len();
        let prior = std::mem::take(&mut self.known);
        let (b, errs) = loader.compile_after(prior, "<repl>", src, Stage::All);
        self.known = b.sources;
        assert!(errs.is_empty(), "{src:?}: {errs:?}");
        self.rt.add_names(&self.known, from);
        let entry = b.proto.expect("an entry");
        self.globals = entry.global_names.clone();
        self.rt.load(&entry, &b.modules);
        match self.rt.run_repl() {
            Ok(v) => Outcome::Value(v),
            Err(Stop::Error(d)) => Outcome::Error(d.code.clone(), d.pos.file),
            Err(Stop::Exit(n)) => Outcome::Exit(n),
        }
    }
}

impl Session {
    /// How many background tasks (agents' loops) are running, read
    /// through a task that is not live (no effect on deadlock judging).
    fn background(&self) -> usize {
        let shared = self.rt.shared();
        Task::unlisted(&shared, CancelNode::root()).background_tasks()
    }

    /// Waits until `background()` is `n` (an agent's loop ends on its own
    /// thread once its last handle drops).
    fn background_until(&self, n: usize) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while self.background() != n {
            assert!(
                std::time::Instant::now() < end,
                "background stays {}",
                self.background()
            );
            std::thread::yield_now();
        }
    }
}

fn value(s: &str) -> Outcome {
    Outcome::Value(s.to_string())
}

/// Go `TestReplGlobalsSurviveGrowth`: a closure from an earlier input
/// sees a later redefinition (Go keeps one table of fixed capacity; the
/// Rust table is shared, so growing it keeps the closures' view).
#[test]
fn globals_survive_and_closures_see_redefinitions() {
    on_big_stack(|| {
        let mut s = Session::new();
        assert_eq!(s.input("fn g() = 1\nfn f() = g()"), value("nil"));
        assert_eq!(s.input("fn g() = 2\nh = 3\nf()"), value("2"));
        assert_eq!(s.input("x = 1"), value("nil"));
        assert_eq!(s.input("fn k(n) = n * x\nk(5) + h"), value("8"));
        assert_eq!(s.input("x = 10\nk(5)"), value("5"), "k copied x (model C)");
        assert_eq!(s.input("import fs\nfs.exists(\"/\")"), value("true"));
        assert_eq!(s.input("import fs\nfs = 1\nfs + 1"), value("2"));
        assert_eq!(s.out.text(), "");
    });
}

/// A global whose assignment failed reads as E0808 (Go `replMode`), not
/// as an internal error; positions name the input that holds the code.
#[test]
fn failed_assignment_is_e0808_and_positions_name_their_input() {
    on_big_stack(|| {
        let mut s = Session::new();
        let first = FileId(s.known.len() as u32);
        assert_eq!(s.input("y = 1 / 0"), Outcome::Error("E0502".into(), first));
        let second = FileId(s.known.len() as u32);
        assert_eq!(s.input("y"), Outcome::Error("E0808".into(), second));
        let third = FileId(s.known.len() as u32);
        assert_eq!(s.input("fn f(x) = x / 0"), value("nil"));
        assert_eq!(s.input("f(1)"), Outcome::Error("E0502".into(), third));
        assert_eq!(s.rt.with_sources(|m| m.name(third).to_string()), "<repl>");
        // The runtime keeps the names only (Go's REPL runtime has no
        // sources: a task failure report shows no excerpt).
        assert_eq!(s.rt.with_sources(|m| m.text(third).to_string()), "");
        // An error does not end the session.
        assert_eq!(s.input("print(\"on\")"), value("nil"));
        assert_eq!(s.out.text(), "on\n");
    });
}

/// A task an input leaves waiting for a later input is not a deadlock
/// while the REPL holds the runtime (Go's 50 ms grace covers a pipe); a
/// deadlock inside one input is still reported.
#[test]
fn a_task_may_wait_for_a_later_input() {
    on_big_stack(|| {
        let mut s = Session::new();
        assert_eq!(
            s.input("c = channel()\nt = spawn(() => recv(c))"),
            value("nil")
        );
        assert_eq!(s.input("send(c, 5)\nawait(t)"), value("Ok(5)"));
        assert_eq!(s.err.text(), "");
        assert!(!s.rt.deadlocked());
        assert_eq!(
            s.input("recv(channel())"),
            Outcome::Error("E0702".into(), FileId(s.known.len() as u32 - 1))
        );
        assert!(s.rt.deadlocked());
        s.rt.cancel_all();
        s.rt.wait();
        s.rt.release();
    });
}

#[test]
fn exit_comes_back() {
    on_big_stack(|| {
        let mut s = Session::new();
        assert_eq!(s.input("exit(3)"), Outcome::Exit(3));
        s.rt.release();
    });
}

/// Go's table has a fixed capacity: more entry globals than that is
/// E0806 (Go-parity scaffolding: the Rust table could grow).
#[test]
fn too_many_globals() {
    on_big_stack(|| {
        let mut s = Session::new();
        let src: String = (0..=65_536).map(|i| format!("v{i} = 0\n")).collect();
        assert_eq!(s.input(&src), Outcome::Error("E0806".into(), FileId(0)));
        // The names stay declared (Go records them before it runs), so
        // every later input hits the limit too.
        assert_eq!(s.input("1 + 1"), Outcome::Error("E0806".into(), FileId(0)));
        s.rt.release();
    });
}

/// Agents in the REPL (Rust only, stage 3b): one a global holds keeps
/// its loop across inputs; a temporary one ends once its input's values
/// drop (its handle closes the request channel); every one ends when the
/// session ends (`end_background`, as the command's `finish` does).
#[test]
fn agents_live_while_referenced() {
    on_big_stack(|| {
        let mut s = Session::new();
        assert_eq!(s.input("a = agent(1)"), value("nil"));
        assert_eq!(s.background(), 1);
        assert_eq!(s.input("deref(agent(5))"), value("5"));
        s.background_until(1);
        assert_eq!(s.input("swap(a, (n) => n + 1)"), value("2"));
        assert_eq!(s.background(), 1, "the stored agent still serves");
        assert_eq!(s.input("a = nil"), value("nil"));
        s.background_until(0);
        assert_eq!(s.input("b = agent(0)"), value("nil"));
        s.rt.cancel_all();
        s.rt.wait();
        s.rt.release();
        s.rt.end_background();
        assert_eq!(s.background(), 0);
    });
}

/// After a deadlock inside an input (decided 2026-10-17, a deliberate
/// divergence from Go): `recover_deadlock` cancels and waits for the
/// program's tasks, including those earlier inputs left blocked, clears
/// the deadlock and its exit request, and gives the program a fresh run
/// root. Globals and agents (which hang from the background root) stay.
#[test]
fn a_deadlock_is_recovered() {
    on_big_stack(|| {
        let mut s = Session::new();
        assert_eq!(s.input("a = agent(1)\nk = 7"), value("nil"));
        assert_eq!(
            s.input("c = channel()\nt = spawn(() => recv(c))"),
            value("nil")
        );
        assert!(!s.rt.recover_deadlock(), "nothing to recover");
        let file = FileId(s.known.len() as u32);
        assert_eq!(
            s.input("recv(channel())"),
            Outcome::Error("E0702".into(), file)
        );
        assert_eq!(s.rt.exit_code(), Some(1));
        assert!(s.rt.recover_deadlock());
        assert!(!s.rt.deadlocked());
        assert_eq!(
            s.rt.exit_code(),
            None,
            "the deadlock's exit request is cleared"
        );
        assert_eq!(s.input("await(t)"), value("Err({kind: 'cancelled})"));
        assert_eq!(s.input("print(\"after\")\nk"), value("7"));
        assert_eq!(s.input("u = spawn(() => k + 1)\nawait(u)"), value("8"));
        assert_eq!(s.input("swap(a, (n) => n + 1)"), value("2"));
        assert_eq!(s.out.text(), "after\n");
        // A second deadlock is reported and recovered too.
        let file = FileId(s.known.len() as u32);
        assert_eq!(
            s.input("recv(channel())"),
            Outcome::Error("E0702".into(), file)
        );
        assert!(s.rt.recover_deadlock());
        assert_eq!(s.input("1 + 1"), value("2"));
        assert_eq!(
            s.err.text(),
            "error[E0702]: deadlock: all tasks are waiting\n".repeat(2)
        );
        s.rt.release();
    });
}
