//! The concurrency natives (05). Stage 2 of the Rust port has no tasks,
//! channels or atoms, so calling any of them raises E0805
//! `not supported yet: <name>`, except `sleep`: with one task and nothing
//! to cancel it, a plain thread sleep is exactly Go's behavior, and it keeps
//! `retry` usable. The parameter tables are Go's (`spec/builtins.txt`), so
//! arity and named-argument errors, which the VM reports before the native
//! runs, are the same as Go's.

use std::cell::OnceCell;
use std::rc::Rc;

use crate::native::nat::{self, def, opt, param};
use crate::value::{Module, Value};
use crate::vm::{Stop, Task};

thread_local! {
    /// Made once per thread: natives are leaked (`Native::leak`).
    static PLACEHOLDERS: OnceCell<Vec<(&'static str, Value)>> = const { OnceCell::new() };
}

/// The prelude's concurrency natives (Go `vm.Concurrency()`): `spawn`
/// `await` `cancel` `channel` `send` `recv` `close` `sleep` `atom` `swap`
/// `deref`, and `_native.conc.spawn_awaited` last.
fn all() -> Vec<(&'static str, Value)> {
    PLACEHOLDERS.with(|c| {
        c.get_or_init(|| {
            let tables: Vec<(&'static str, Vec<_>)> = vec![
                ("sleep", vec![param("d")]),
                ("spawn", vec![param("f")]),
                ("await", vec![param("t"), opt("timeout", Value::Nil)]),
                ("cancel", vec![param("t")]),
                ("channel", vec![opt("n", Value::Int(0))]),
                ("send", vec![param("ch"), param("v")]),
                ("recv", vec![param("ch"), opt("timeout", Value::Nil)]),
                ("close", vec![param("ch")]),
                ("atom", vec![param("v")]),
                ("swap", vec![param("a"), param("f")]),
                ("deref", vec![param("a")]),
                ("spawn_awaited", vec![param("f")]),
            ];
            tables
                .into_iter()
                .map(|(name, params)| {
                    let f = if name == "sleep" { sleep } else { unsupported };
                    (name, def(name, f, params))
                })
                .collect()
        })
        .clone()
    })
}

/// The prelude's concurrency natives, without `spawn_awaited` (which only
/// `_native.conc` holds).
pub fn placeholders() -> Vec<(&'static str, Value)> {
    all()
        .into_iter()
        .filter(|(n, _)| *n != "spawn_awaited")
        .collect()
}

/// `_native.conc` (Go `vm.NativeModules()`): `spawn_awaited`, which
/// `extras/prelude.fung` uses for `fan_in` and `pmap`.
pub fn module() -> Rc<Module> {
    let mut m = Module::new("conc", "_native.conc");
    for (name, v) in all() {
        if name == "spawn_awaited" {
            m.set(name, v);
        }
    }
    Rc::new(m)
}

/// Every placeholder: E0805 with the native's own name. The VM gives the
/// error the call's position (or, inside `extras/prelude.fung`, the user
/// call's).
fn unsupported(t: &mut Task<'_>, _: Vec<Value>) -> Result<Value, Stop> {
    let name = t.calling_native().unwrap_or("?");
    Err(nat::fail("E0805", vec![nat::s(name)]))
}

/// `sleep(d)` (Go `sleepFn`): waits for `d`; zero and negative durations
/// return at once. Stage 3 makes it park the task and end early with E0701
/// when the task is cancelled.
fn sleep(_: &mut Task<'_>, args: Vec<Value>) -> Result<Value, Stop> {
    let d = match &args[0] {
        Value::Dur(d) => *d,
        v => return Err(nat::wrong_arg("d", "dur", v)),
    };
    if d > 0 {
        std::thread::sleep(std::time::Duration::from_nanos(d as u64));
    }
    Ok(Value::Nil)
}
