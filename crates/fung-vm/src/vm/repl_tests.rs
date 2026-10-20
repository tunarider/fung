//! The runtime side of the REPL (08): every input is compiled on its own
//! against the names defined so far and runs on one runtime, whose entry
//! globals outlive each run.

use std::io::Cursor;

use fung_front::loader::{Loader, Stage};
use fung_front::source::{FileId, SourceMap};

use crate::testutil::{Buf, on_big_stack};
use crate::vm::{Runtime, Stop};

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
            keep_value: true,
            ..Loader::default()
        };
        let from = self.known.len();
        let prior = std::mem::take(&mut self.known);
        let (b, errs) = loader.compile_after(prior, "<repl>", src, Stage::All);
        self.known = b.sources;
        assert!(errs.is_empty(), "{src:?}: {errs:?}");
        self.rt.add_session_files(&self.known, from);
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

fn value(s: &str) -> Outcome {
    Outcome::Value(s.to_string())
}

/// A closure from an earlier input sees a later redefinition (the table is
/// shared, so growing it keeps the closures' view).
#[test]
fn globals_survive_and_closures_see_redefinitions() {
    on_big_stack(|| {
        let mut s = Session::new();
        assert_eq!(s.input("fn g() = 1\nfn f() = g()"), value("nil"));
        assert_eq!(s.input("fn g() = 2\nh = 3\nf()"), value("2"));
        assert_eq!(s.input("x = 1"), value("nil"));
        assert_eq!(s.input("fn k(n) = n * x\nk(5) + h"), value("8"));
        // A REPL fn reads a global when called (03, L9).
        assert_eq!(s.input("x = 10\nk(5)"), value("50"));
        assert_eq!(s.input("import fs\nfs.exists(\"/\")"), value("true"));
        assert_eq!(s.input("import fs\nfs = 1\nfs + 1"), value("2"));
        assert_eq!(s.out.text(), "");
    });
}

/// A global whose assignment failed reads as E0808, not
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
        // The runtime keeps each input's text, so a task failure report
        // excerpts the input its code came from.
        assert_eq!(
            s.rt.with_sources(|m| m.text(third).to_string()),
            "fn f(x) = x / 0"
        );
        // An error does not end the session.
        assert_eq!(s.input("print(\"on\")"), value("nil"));
        assert_eq!(s.out.text(), "on\n");
    });
}

/// A destructuring that fails part way changes no global: the names it
/// binds are written only once the whole pattern has matched, so the
/// session goes on with the values it had (a new name stays unset).
#[test]
fn failed_destructuring_changes_no_global() {
    on_big_stack(|| {
        let mut s = Session::new();
        assert_eq!(s.input("a = 5\nb = 6"), value("nil"));
        let at = FileId(s.known.len() as u32);
        assert_eq!(
            s.input("{x: a, y: [b]} = {x: 1, y: 2}"),
            Outcome::Error("E0602".into(), at)
        );
        assert_eq!(s.input("(a, b)"), value("(5, 6)"));
        let at = FileId(s.known.len() as u32);
        assert_eq!(
            s.input("[c, 3] = [1, 2]"),
            Outcome::Error("E0602".into(), at)
        );
        let at = FileId(s.known.len() as u32);
        assert_eq!(s.input("c"), Outcome::Error("E0808".into(), at));
        // A failed alternative leaves nothing behind for the one that
        // matches.
        assert_eq!(s.input("(a, 1) | (a, 2) = (7, 2)\na"), value("7"));
    });
}

/// In the REPL, where names can be assigned again, a top-level fn reads a
/// global when it is called (03, L9).
#[test]
fn a_repl_fn_reads_globals_when_called() {
    on_big_stack(|| {
        let mut s = Session::new();
        assert_eq!(s.input("x = 1"), value("nil"));
        assert_eq!(s.input("fn f() = x"), value("nil"));
        assert_eq!(s.input("f()"), value("1"));
        assert_eq!(s.input("x = 2"), value("nil"));
        assert_eq!(s.input("f()"), value("2"));
        assert_eq!(s.input("y = 1\ny = 3\nfn g() = y\ng()"), value("3"));
    });
}

/// An input's last value is printed, an Err too; one discarded before it
/// is E0606 (02: 버려지는 Err), and a for at the end still shows its list.
#[test]
fn an_input_prints_its_last_err_and_rejects_a_discarded_one() {
    on_big_stack(|| {
        let mut s = Session::new();
        assert_eq!(s.input("Err(1)"), value("Err(1)"));
        assert_eq!(s.input("for x in [1]: Err(x) ;;"), value("[Err(1)]"));
        let at = FileId(s.known.len() as u32);
        assert_eq!(s.input("Err(1)\n2"), Outcome::Error("E0606".into(), at));
        assert_eq!(s.input("_ = Err(1)\n2"), value("2"));
    });
}

/// Tasks inside an input's scope run while the REPL holds the runtime,
/// over a channel an earlier input made; a deadlock inside one input is
/// still reported.
#[test]
fn tasks_run_inside_an_input_and_a_deadlock_is_reported() {
    on_big_stack(|| {
        let mut s = Session::new();
        assert_eq!(s.input("(c_tx, c) = channel()"), value("nil"));
        assert_eq!(
            s.input(
                "scope(fn(s):\n  t = spawn(s, () => recv(c))\n  send(c_tx, 5)\n  await(t)\n;;)"
            ),
            value("Ok(5)")
        );
        assert_eq!(s.err.text(), "");
        assert!(!s.rt.deadlocked());
        assert_eq!(
            s.input("(_tx, never) = channel()\nrecv(never)"),
            Outcome::Error("E0702".into(), FileId(s.known.len() as u32 - 1))
        );
        assert!(s.rt.deadlocked());
        s.rt.cancel_all();
        s.rt.wait();
    });
}

#[test]
fn exit_comes_back() {
    on_big_stack(|| {
        let mut s = Session::new();
        assert_eq!(s.input("exit(3)"), Outcome::Exit(3));
    });
}

/// The session's globals have no fixed limit: the table grows.
#[test]
fn many_globals() {
    on_big_stack(|| {
        let mut s = Session::new();
        let src: String = (0..=65_536).map(|i| format!("v{i} = {i}\n")).collect();
        assert_eq!(s.input(&src), value("nil"));
        assert_eq!(s.input("v65536 + 1"), value("65537"));
    });
}

/// Atoms in the REPL: one a global holds keeps its value across inputs.
#[test]
fn atoms_keep_their_value_across_inputs() {
    on_big_stack(|| {
        let mut s = Session::new();
        assert_eq!(s.input("a = atom(1)"), value("nil"));
        assert_eq!(s.input("deref(atom(5))"), value("5"));
        assert_eq!(s.input("swap(a, (n) => n + 1)"), value("2"));
        assert_eq!(s.input("deref(a)"), value("2"));
    });
}

/// After a deadlock inside an input (decided 2026-10-17):
/// `recover_deadlock` cancels and waits for the program's tasks (here the
/// stuck child of the input's scope), clears the deadlock and its exit
/// request, and gives the program a fresh run root. Globals (an atom
/// among them) stay.
#[test]
fn a_deadlock_is_recovered() {
    on_big_stack(|| {
        let mut s = Session::new();
        assert_eq!(s.input("a = atom(1)\nk = 7"), value("nil"));
        assert!(!s.rt.recover_deadlock(), "nothing to recover");
        let file = FileId(s.known.len() as u32);
        assert_eq!(
            s.input(
                "(_c_tx, c) = channel()\n(_tx, never) = channel()\nscope(fn(s):\n  spawn(s, () => recv(c))\n  recv(never)\n;;)"
            ),
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
        assert_eq!(s.input("print(\"after\")\nk"), value("7"));
        assert_eq!(
            s.input("scope(s => await(spawn(s, () => k + 1)))"),
            value("8")
        );

        assert_eq!(s.input("swap(a, (n) => n + 1)"), value("2"));
        assert_eq!(s.out.text(), "after\n");
        // A second deadlock is reported and recovered too.
        let file = FileId(s.known.len() as u32);
        assert_eq!(
            s.input("(_tx, never) = channel()\nrecv(never)"),
            Outcome::Error("E0702".into(), file)
        );
        assert!(s.rt.recover_deadlock());
        assert_eq!(s.input("1 + 1"), value("2"));
        // Each report shows where the input and its tasks wait.
        assert_eq!(
            s.err.text(),
            // The REPL's globals hold the sending ends (L7).
            "error[E0702]: deadlock: all tasks are waiting\n  --> <repl>:5:7\n  |\n5 |   recv(never)\n  |       ^\n  |\n call path (innermost first):\n   <repl>:5   <lambda>\n   <repl>:3   <entry>\n help: this task waits to receive, but it still holds that channel's sending end itself (the global `_tx`), so the channel cannot close: close it with `close(_tx)` first, or leave the sending end to the producers only\n 1 other task is waiting:\n   recv at <repl>:4:22 (<lambda>, holds its own sending end)\n\
             error[E0702]: deadlock: all tasks are waiting\n  --> <repl>:2:5\n  |\n2 | recv(never)\n  |     ^\n  |\n call path (innermost first):\n   <repl>:2   <entry>\n help: this task waits to receive, but it still holds that channel's sending end itself (the global `_tx`), so the channel cannot close: close it with `close(_tx)` first, or leave the sending end to the producers only\n"
        );
    });
}

/// An exit a task asks for after a deadlock was reported is not lost by
/// the recovery: only the deadlock's own exit(1) is cleared, the run root
/// stays cancelled, and the session ends with the task's code.
///
/// The request is made from Rust, as a task's `exit` call makes it: no
/// fung code can make it in that window today. Every task of the program
/// is cancelled by the report, and cancellation is checked at every
/// call. The rule guards against a later way in (a task that nothing
/// cancels).
#[test]
fn a_recovery_keeps_a_later_exit() {
    on_big_stack(|| {
        let mut s = Session::new();
        let file = FileId(s.known.len() as u32);
        assert_eq!(
            s.input("(_tx, never) = channel()\nrecv(never)"),
            Outcome::Error("E0702".into(), file)
        );
        s.rt.task().request_exit(5);
        assert_eq!(s.rt.exit_code(), Some(1), "the first request stays");
        assert_eq!(s.rt.exit_requested(), Some(5));
        assert!(s.rt.recover_deadlock());
        assert!(!s.rt.deadlocked());
        assert_eq!(s.rt.exit_code(), Some(5), "the later exit is kept");
        assert!(
            s.rt.shared().root().is_cancelled(),
            "no fresh root while an exit is pending"
        );
    });
}

/// Without a later exit, the deadlock's own request is not a task's, and
/// the recovery clears it.
#[test]
fn a_deadlocks_exit_is_not_a_tasks() {
    on_big_stack(|| {
        let mut s = Session::new();
        let file = FileId(s.known.len() as u32);
        assert_eq!(
            s.input("(_tx, never) = channel()\nrecv(never)"),
            Outcome::Error("E0702".into(), file)
        );
        assert_eq!(s.rt.exit_code(), Some(1));
        assert_eq!(s.rt.exit_requested(), None);
        assert!(s.rt.recover_deadlock());
        assert_eq!(s.rt.exit_code(), None);
        assert!(!s.rt.shared().root().is_cancelled());
    });
}
