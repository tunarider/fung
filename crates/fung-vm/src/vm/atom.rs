//! Atoms (concurrency v2, L5; 05 "공유 상태"): `atom(v)`, `deref(a)`,
//! `swap(a, f)`, `reset(a, v)`.
//!
//! An atom is a cell pointing at one immutable value, with a version
//! number that every change bumps. `deref` reads the value without
//! waiting. `swap` is compare-and-set: it reads the value and the
//! version, runs `f` on the value, and stores the result if the version
//! is still the one it read; otherwise another task changed the atom in
//! between, and it runs `f` again on the new value. So no update is lost,
//! and the meaning would hold on several cores as well. On one run thread
//! a retry is rare: `f` never parks (effects are E0707, and a regex
//! compiles in place while `pure` is set), and its turn does not end while
//! it runs (`Task::end_turn` waits while `pure` is set), so it gives its
//! turn at most once, at its first call when the turn was due as it
//! started.
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
use crate::vm::{Cont, Flow, Next, Stop, Task};

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
/// atom while `f` ran; the new value. A failing `f` changes nothing. The
/// run loop calls `f` (`cont.rs`).
pub(crate) fn run_swap(t: &mut Task<'_>, a: Rc<Atom>, f: Value) -> Flow {
    let version = a.version.get();
    t.pure += 1;
    t.push_arg(read(&a));
    Flow::Call(
        Box::new(Swap {
            a,
            f: f.clone(),
            version,
        }),
        f,
        1,
    )
}

/// A `swap` waiting for its function, with the version it read.
struct Swap {
    a: Rc<Atom>,
    f: Value,
    version: u64,
}

impl Cont for Swap {
    fn resume(&mut self, t: &mut Task<'_>, new: Value) -> Result<Next, Stop> {
        t.pure -= 1;
        need_holdable(&new)?;
        if self.a.version.get() == self.version {
            self.a.set(new.clone());
            return Ok(Next::Done(new));
        }
        self.version = self.a.version.get();
        t.pure += 1;
        t.push_arg(read(&self.a));
        Ok(Next::Call(self.f.clone(), 1))
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        t.pure -= 1;
        Err(stop)
    }
}

/// The builtins that are effects, by where they live (`prelude` for the
/// core and `conc` natives, else the `_native.*` module): what a swap's
/// function may not call. Every builtin is here or in [`PURE`], and in
/// one only (`every_builtin_is_classified`).
pub(crate) const EFFECTS: &[(&str, &[&str])] = &[
    (
        "prelude",
        &[
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
        ],
    ),
    (
        "_native.fs",
        &[
            "read",
            "write",
            "append",
            "list",
            "glob",
            "walk",
            "exists",
            "stat",
            "mkdir",
            "remove",
            "remove_all",
            "move",
            "copy",
            "open",
            "close",
            "write_line",
            "temp_dir",
        ],
    ),
    (
        "_native.net",
        &[
            "connect",
            "listen",
            "accept",
            "read_line",
            "read_raw_line",
            "read_bytes",
            "read_all",
            "write",
            "close",
            "set_deadline",
            "read_bytes_into",
            "take_body",
            "start_tls",
        ],
    ),
    ("_native.process", &["exec", "sh"]),
];

/// The builtins that are pure: computations on their arguments (a clock
/// read and an environment read count as such: they change nothing).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const PURE: &[(&str, &[&str])] = &[
    (
        "prelude",
        &[
            "fail",
            "len",
            "map",
            "try_map",
            "type_of",
            "filter",
            "fold",
            "each",
            "sort",
            "unique",
            "sort_by",
            "find",
            "any",
            "all",
            "flatten",
            "group_by",
            "pairs",
            "put",
            "merge",
            "remove",
            "chars",
            "split",
            "join",
            "trim",
            "upper",
            "lower",
            "replace",
            "starts_with",
            "ends_with",
            "repeat",
            "int",
            "float",
            "str",
            "sym",
            "parse_int",
            "parse_float",
            "parse_dur",
            "scope",
            "timeout",
            "atom",
            "deref",
            "with",
            "supervise",
            "channel",
            "stop_signal",
        ],
    ),
    (
        "_native.net",
        &[
            "local_addr",
            "byte_len",
            "utf8_bytes",
            "utf8_string",
            "need_str_map",
            "user_agent",
        ],
    ),
    ("_native.json", &["parse", "stringify"]),
    (
        "_native.path",
        &["join", "base", "dir", "ext", "stem", "abs"],
    ),
    ("_native.process", &["env"]),
    ("_native.range", &["last", "slice"]),
    (
        "_native.regex",
        &["is_match", "find", "find_all", "replace", "split"],
    ),
    (
        "_native.time",
        &[
            "now",
            "parse",
            "format",
            "utc",
            "local",
            "unix",
            "from_unix",
            "parts",
        ],
    ),
];

/// Whether `table` lists `name` under `module`.
pub(crate) fn is_listed(table: &[(&str, &[&str])], module: &str, name: &str) -> bool {
    table
        .iter()
        .any(|(m, names)| *m == module && names.contains(&name))
}

/// The natives marked as effects, by address; made on first use.
fn effects() -> &'static HashSet<usize> {
    static SET: OnceLock<HashSet<usize>> = OnceLock::new();
    SET.get_or_init(|| {
        let mut set = HashSet::new();
        let mut add = |module: &str, name: &str, v: &Value| {
            if let Value::Native(n) = v
                && is_listed(EFFECTS, module, name)
            {
                set.insert(*n as *const Native as usize);
            }
        };
        for (name, v) in crate::prelude::core()
            .into_iter()
            .chain(crate::conc::natives())
        {
            add("prelude", name, &v);
        }
        for (key, make) in crate::boot::NATIVE_MODULES {
            let m = make();
            for name in m.names() {
                add(key, name, m.get(name).expect("a member"));
            }
        }
        set
    })
}

/// Whether calling `n` is an effect (E0707 inside a swap's function).
pub(crate) fn is_effect(n: &'static Native) -> bool {
    effects().contains(&(n as *const Native as usize))
}

/// The effects as `where.name` (the table test).
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
        for n in m.names() {
            if matches!(m.get(n), Some(Value::Native(x)) if is_effect(x)) {
                out.push(format!("{key}.{n}"));
            }
        }
    }
    out
}

impl Task<'_> {
    /// E0707 if `n` is an effect and this task runs a swap's function.
    #[inline]
    pub(crate) fn check_effect(&self, n: &'static Native) -> Result<(), Stop> {
        if self.pure > 0 && is_effect(n) {
            let name = self
                .user_callee()
                .or_else(|| self.world.public_name(n))
                .unwrap_or_else(|| n.name.to_string());
            return Err(nat::fail("E0707", vec![fung_front::diag::Arg::Str(name)]));
        }
        Ok(())
    }
}
