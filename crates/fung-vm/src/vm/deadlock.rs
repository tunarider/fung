//! Deadlock detection (05 "교착").
//!
//! The program is deadlocked when every task is parked and none of them
//! is one a timer or the world outside ends: nothing can ever run again.
//! The scheduler knows that exactly (`sched.rs`): every task is running,
//! in the ready queue or parked with it, and the counts change only on
//! the run thread. So it calls `judge` when the ready queue is empty and
//! every parked task is stuck, and the report is at once, with no grace
//! period (spec 2절). A parked task woken but not run yet is in the ready
//! queue, so it is never counted stuck.
//!
//! After the report: E0702 on standard error, exit code 1 (the first exit
//! request), and every task of the program cancelled (the root). Each
//! parked task wakes and unwinds with E0702 (its handle turns that into
//! `Err({kind: 'cancelled})`); the command does not report the entry's
//! E0702 again (an exit was requested).
//!
//! While a `with`'s release runs (`cleanup.rs`), the check is judged too
//! even after an exit or a deadlock: the release runs under a shield node
//! nothing else cancels, so a deadlock there cancels the shield nodes of
//! the releases running then (and the root), and they are cut short.
//!
//! **The report** (review C7, decided 2026-10-20) says where the tasks
//! wait. The root (the entry, a test block) is shown as a runtime error
//! is: the call it waits in, its line and its call path. Then one line per
//! kind of wait and place for the other tasks, in the order they were
//! spawned, with how many wait there (`×N`); at most `REPORT_LINES`
//! lines, the tasks past them counted. A wait inside a built-in module's
//! function is shown at the user's call of it and named with it (`recv
//! in take`); one inside a `with`'s release says so.

use std::collections::HashMap;

use fung_front::diag::{self, Diag, error, headline};
use fung_front::source::Pos;

use super::block::cancelled;
use super::frame::Frame;
use super::{Stop, Task, no_pos};
use crate::value::Value;

/// The most lines the report gives the other tasks.
const REPORT_LINES: usize = 10;

/// What the parked tasks wait in, gathered by the scheduler for the
/// report (`Sched::stuck`).
#[derive(Default)]
pub(super) struct Stuck {
    /// The root's wait as an error: its place and call path.
    pub root: Option<Diag>,
    /// The other tasks' waits, in the order they were spawned.
    pub others: Vec<Place>,
}

/// Where a parked task waits.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct Place {
    /// The wait (`recv`, `await`, `scope` …), with the built-in module
    /// function it is inside of (`recv in take`).
    kind: String,
    file: String,
    line: u32,
    col: u32,
    /// The function the place is in.
    func: String,
    /// Inside a `with`'s release.
    release: bool,
    /// It waits to receive while it holds that channel's sending end
    /// itself (`Task::own_sender`).
    holds_sender: bool,
}

impl Place {
    fn line_of(&self, n: usize) -> String {
        let mut l = self.kind.clone();
        if self.line > 0 {
            l += &format!(" at {}:{}:{}", self.file, self.line, self.col);
        }
        l += &format!(" ({}", self.func);
        if self.release {
            l += ", in release";
        }
        if self.holds_sender {
            l += ", holds its own sending end";
        }
        l.push(')');
        if n > 1 {
            l += &format!(" \u{d7}{n}");
        }
        l
    }
}

/// A frame's function as call paths name it.
fn func_name(f: &Frame) -> String {
    match f.proto.name.as_str() {
        "" => "<lambda>".to_string(),
        "<main>" => "<entry>".to_string(),
        name => name.to_string(),
    }
}

impl Task<'_> {
    /// Reports a deadlock: every task is parked and stuck (the scheduler
    /// checked); true if it reported. After an exit or an earlier report
    /// the program is unwinding, so it is judged only while a release
    /// runs. `stuck` gathers where the parked tasks wait, for the report.
    pub(super) fn judge(&mut self, stuck: impl FnOnce(&mut Task<'_>) -> Stuck) -> bool {
        let shields = self.world.live_shields();
        if shields.is_empty() && (self.world.deadlocked || self.world.exit.is_some()) {
            return false;
        }
        let s = stuck(self);
        let report = self.deadlock_report(s);
        let w = &mut *self.world;
        w.deadlocked = true;
        let _ = w.stderr().write_all(report.as_bytes());
        let _ = w.stderr().flush();
        w.request_deadlock_exit();
        for s in &shields {
            s.cancel();
        }
        self.shared.root().cancel();
        true
    }

    /// The report's text (see the module doc).
    fn deadlock_report(&self, s: Stuck) -> String {
        let mut b = match &s.root {
            Some(d) => diag::format(d, &self.world.sources),
            None => headline(&error("E0702", no_pos(), Vec::new())) + "\n",
        };
        let n = s.others.len();
        if n == 0 {
            return b;
        }
        b += &if n == 1 {
            " 1 other task is waiting:\n".to_string()
        } else {
            format!(" {n} other tasks are waiting:\n")
        };
        let mut groups: Vec<(Place, usize)> = Vec::new();
        let mut at: HashMap<Place, usize> = HashMap::new();
        for p in s.others {
            match at.get(&p) {
                Some(&i) => groups[i].1 += 1,
                None => {
                    at.insert(p.clone(), groups.len());
                    groups.push((p, 1));
                }
            }
        }
        for (p, n) in groups.iter().take(REPORT_LINES) {
            b += &format!("   {}\n", p.line_of(*n));
        }
        let more: usize = groups.iter().skip(REPORT_LINES).map(|g| g.1).sum();
        if more > 0 {
            b += &format!("   \u{2026} {more} more\n");
        }
        b
    }

    /// Where this task, parked, waits: the call of the waiting native
    /// (its last box), or, inside a built-in module's function, the user's
    /// call of that function; the wait's name; the function the place is
    /// in; and whether it is inside a release.
    fn wait_site(&self) -> (Pos, String, String, bool) {
        let c = self.conts.last();
        let wait = c
            .and_then(|c| c.k.waits_for().or(c.native.map(|n| n.name)))
            .unwrap_or("wait");
        let mut pos = c.map_or(no_pos(), |c| c.pos);
        let mut kind = wait.to_string();
        if self.frames.last().is_some_and(|f| f.proto.builtin) {
            if let Some(p) = self.user_pos() {
                pos = p;
            }
            if let Some(callee) = self.user_callee().or_else(|| self.outer_callee()) {
                kind = format!("{wait} in {callee}");
            }
        }
        let func = self
            .frames
            .iter()
            .rev()
            .find(|f| !f.proto.builtin)
            .or(self.frames.last())
            .map_or_else(|| "<entry>".to_string(), func_name);
        let release = self.conts.iter().any(|c| c.k.releasing());
        (pos, kind, func, release)
    }

    /// The built-in module function a task runs inside of when no user
    /// frame is left: a lambda that tail-called it (`() => take(rx, 2)`)
    /// gave its frame to it. Named as `user_callee` names one.
    fn outer_callee(&self) -> Option<String> {
        let p = self.frames.first()?.entry.clone()?;
        let at = p.pos.first()?;
        let file = self.world.file_name(at.file);
        let module = std::path::Path::new(file).file_stem()?.to_str()?;
        Some(if module == "prelude" {
            p.name.clone()
        } else {
            format!("{module}.{}", p.name)
        })
    }

    /// Where this task, parked receiving, holds that channel's sending end
    /// itself, so that the channel can never close (review L7): its stack
    /// (a function's variables), its frames' captured values, or a
    /// global, which every task holds. `None` if it does not.
    fn own_sender(&self) -> Option<(String, Option<String>)> {
        let rx = self.conts.last()?.k.receives_on()?;
        let held = |v: &Value| matches!(v, Value::Sender(tx) if rx.pairs_with(tx));
        for (i, v) in self.stack.iter().enumerate() {
            if v.as_ref().is_some_and(held) {
                let f = self.frames.iter().rev().find(|f| f.base <= i)?;
                return Some((format!("a variable of `{}`", func_name(f)), None));
            }
        }
        for f in self.frames.iter().rev() {
            if f.closure
                .as_ref()
                .is_some_and(|c| c.captured.iter().any(held))
            {
                return Some((format!("a value `{}` captured", func_name(f)), None));
            }
        }
        for f in &self.frames {
            let g = f.globals.borrow();
            if let Some(i) = g.iter().position(|v| v.as_ref().is_some_and(held)) {
                // A function's proto has no names of its module's globals;
                // the entry's do, when these are the entry's (a test block).
                let entry = self.world.main.as_ref().map(|m| &m.global_names);
                let names = Some(&f.proto.global_names)
                    .filter(|n| !n.is_empty())
                    .or(entry.filter(|n| n.len() == g.len()));
                return Some(match names.and_then(|n| n.get(i)) {
                    Some(name) => (format!("the global `{name}`"), Some(name.clone())),
                    None => ("a global".to_string(), None),
                });
            }
        }
        None
    }

    /// The root's wait as an error, as a runtime error there would show
    /// (`Sched::stuck`), with what holds the channel open when the root
    /// holds its sending end itself.
    pub(super) fn stuck_root(&self) -> Diag {
        let (pos, ..) = self.wait_site();
        let mut d = error("E0702", pos, Vec::new()).with_call_path(self.call_path());
        if let Some((at, name)) = self.own_sender() {
            let close = match name {
                Some(n) => format!("`close({n})`"),
                None => "`close`".to_string(),
            };
            d.help = Some(format!(
                "this task waits to receive, but it still holds that channel's sending end itself ({at}), so the channel cannot close: close it with {close} first, or leave the sending end to the producers only"
            ));
        }
        d
    }

    /// Another task's wait (`Sched::stuck`).
    pub(super) fn stuck_place(&self) -> Place {
        let (pos, kind, func, release) = self.wait_site();
        Place {
            kind,
            file: self.world.file_name(pos.file).to_string(),
            line: pos.line,
            col: pos.col,
            func,
            release,
            holds_sender: self.own_sender().is_some(),
        }
    }

    /// The error a cancelled blocked task unwinds with: E0702 once a
    /// deadlock was reported, E0701 otherwise.
    pub(super) fn unwound(&self) -> Stop {
        if self.world.deadlocked {
            Stop::Error(Box::new(error("E0702", no_pos(), Vec::new())))
        } else {
            cancelled(no_pos())
        }
    }
}
