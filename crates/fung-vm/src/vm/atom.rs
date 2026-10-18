//! Atoms (concurrency v2, L5; 05 "공유 상태"): `atom(v)`, `deref(a)`,
//! `swap(a, f)`, `reset(a, v)`.
//!
//! An atom is a cell pointing at one immutable value, with a version
//! number that every change bumps. `deref` reads the value without
//! waiting. `swap` is compare-and-set: it reads the value and the
//! version, runs `f` on the value, and stores the result if the version
//! is still the one it read; otherwise another task changed the atom in
//! between (the run token can change hands inside `f`), and it runs `f`
//! again on the new value. So no update is lost, and the meaning holds on
//! several cores as well.
//!
//! **`f` is pure.** It may run more than once, so it must not do
//! anything that shows outside but the time it takes: while it runs
//! (`Task::pure`), calling a native marked as an effect is E0707 (the
//! table below). That rule also replaces the agents' E0704: `f` cannot
//! swap another atom.
//!
//! **What an atom holds** cannot be or hold a mutable value
//! (`Value::holds_mutable`: an atom, a channel, a task or scope handle, a
//! file or a socket): E0708. So no `Rc` cycle can run through an atom.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::OnceLock;

use crate::native::nat;
use crate::value::{Native, Value};
use crate::vm::{Stop, Task};

/// The value `atom` makes; it prints as `<atom>`.
pub struct Atom {
    cell: RefCell<Value>,
    version: Cell<u64>,
}

impl std::fmt::Debug for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<atom>")
    }
}

impl Atom {
    /// The value, moved out for `release` (`value/release.rs`).
    pub(crate) fn into_value(self) -> Value {
        self.cell.into_inner()
    }

    fn set(&self, v: Value) {
        *self.cell.borrow_mut() = v;
        self.version.set(self.version.get() + 1);
    }
}

/// E0708 unless `v` may go into an atom.
fn need_holdable(v: &Value) -> Result<(), Stop> {
    if v.holds_mutable() {
        return Err(nat::fail("E0708", Vec::new()));
    }
    Ok(())
}

/// `atom(v)`.
pub(crate) fn make(v: Value) -> Result<Value, Stop> {
    need_holdable(&v)?;
    Ok(Value::Atom(Rc::new(Atom {
        cell: RefCell::new(v),
        version: Cell::new(0),
    })))
}

/// `deref(a)`.
pub(crate) fn read(a: &Atom) -> Value {
    a.cell.borrow().clone()
}

/// `reset(a, v)`: the new value.
pub(crate) fn reset(a: &Atom, v: Value) -> Result<Value, Stop> {
    need_holdable(&v)?;
    a.set(v.clone());
    Ok(v)
}

/// `swap(a, f)`: compare-and-set, again until no other task changed the
/// atom while `f` ran; the new value. A failing `f` changes nothing.
pub(crate) fn run_swap(t: &mut Task<'_>, a: &Rc<Atom>, f: Value) -> Result<Value, Stop> {
    loop {
        let version = a.version.get();
        let old = read(a);
        t.pure += 1;
        let r = t.call(&f, vec![old]);
        t.pure -= 1;
        let new = r?;
        need_holdable(&new)?;
        if a.version.get() == version {
            a.set(new.clone());
            return Ok(new);
        }
    }
}

/// The prelude natives that are effects: what a swap's function may not
/// call. Everything of the modules in [`EFFECT_MODULES`] is one too, and
/// so are the listed members of [`EFFECT_MEMBERS`].
const EFFECT_PRELUDE: &[&str] = &[
    "print",
    "eprint",
    "dbg",
    "read_line",
    "read_lines",
    "exit",
    "spawn",
    "await",
    "cancel",
    "send",
    "recv",
    "close",
    "sleep",
    "swap",
    "reset",
    "request_stop",
];
const EFFECT_MODULES: &[&str] = &["_native.fs", "_native.net", "_native.conc"];
const EFFECT_MEMBERS: &[(&str, &[&str])] = &[("_native.process", &["exec", "sh"])];

/// The natives marked as effects, by address; made on first use.
fn effects() -> &'static HashSet<usize> {
    static SET: OnceLock<HashSet<usize>> = OnceLock::new();
    SET.get_or_init(|| {
        let mut set = HashSet::new();
        let mut add = |v: &Value| {
            if let Value::Native(n) = v {
                set.insert(*n as *const Native as usize);
            }
        };
        for (name, v) in crate::prelude::core()
            .into_iter()
            .chain(crate::conc::natives())
        {
            if EFFECT_PRELUDE.contains(&name) {
                add(&v);
            }
        }
        for (key, make) in crate::boot::NATIVE_MODULES {
            let m = make();
            let members: Option<&[&str]> = EFFECT_MEMBERS
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, ms)| *ms);
            for name in m.names() {
                if EFFECT_MODULES.contains(key) || members.is_some_and(|ms| ms.contains(&name)) {
                    add(m.get(name).expect("a member"));
                }
            }
        }
        set
    })
}

/// Whether calling `n` is an effect (E0707 inside a swap's function).
pub(crate) fn is_effect(n: &'static Native) -> bool {
    effects().contains(&(n as *const Native as usize))
}

/// The effects as `where.name`, a whole module as `where.*` (the table
/// test).
#[cfg(test)]
pub(crate) fn effect_names() -> Vec<String> {
    let mut out = Vec::new();
    for (name, v) in crate::prelude::core()
        .into_iter()
        .chain(crate::conc::natives())
    {
        if let Value::Native(n) = v
            && is_effect(n)
        {
            out.push(format!("prelude.{name}"));
        }
    }
    for (key, make) in crate::boot::NATIVE_MODULES {
        let m = make();
        let names = m.names();
        let marked: Vec<&str> = names
            .iter()
            .copied()
            .filter(|n| matches!(m.get(n), Some(Value::Native(x)) if is_effect(x)))
            .collect();
        if !marked.is_empty() && marked.len() == names.len() {
            out.push(format!("{key}.*"));
        } else {
            out.extend(marked.iter().map(|n| format!("{key}.{n}")));
        }
    }
    out
}

impl Task<'_> {
    /// E0707 if `n` is an effect and this task runs a swap's function.
    #[inline]
    pub(crate) fn check_effect(&self, n: &'static Native) -> Result<(), Stop> {
        if self.pure > 0 && is_effect(n) {
            return Err(nat::fail(
                "E0707",
                vec![fung_front::diag::Arg::Str(n.name.to_string())],
            ));
        }
        Ok(())
    }
}
