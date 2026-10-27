//! Modules at run time (04): the runtime's table of importable modules and
//! IMPORT. A module's body runs the first time it is imported, as a nested
//! frame on the importing task with its own globals; afterwards its public
//! globals become the immutable module value every later import gets.

use std::rc::Rc;

use fung_front::diag::Arg;
use fung_front::source::Pos;

#[cfg(test)]
use super::Runtime;
use super::cont::{Cont, ContFrame, Next};
use super::frame::Frame;
use super::world::World;
use super::{RtProto, Stop, Task, err, no_pos};
use crate::value::{Globals, Module, Value};

/// One registered module: its compiled body and, once the body has run
/// (or for a native module), its value.
pub(super) struct ModuleState {
    proto: Option<Rc<RtProto>>,
    value: Option<Rc<Module>>,
    /// The body is running: importing it again is a run-time cycle.
    loading: bool,
}

impl World {
    /// The name a user knows a primitive by, when they call it directly
    /// (E0707): a prelude name as it is, else `module.name` of the first
    /// standard module that holds it (`fs.read` is `_native.fs.read`).
    pub(crate) fn public_name(&self, n: &'static crate::value::Native) -> Option<String> {
        let same = |v: &Value| matches!(v, Value::Native(m) if std::ptr::eq(*m, n));
        if let Some((name, _)) = self.prelude.0.iter().find(|(_, v)| same(v)) {
            return Some(name.clone());
        }
        self.modules
            .iter()
            // Only the standard modules: a user's module may bind the
            // primitive too, under a name of its own and a path as key.
            .filter(|(key, _)| fung_front::extras::source(key).is_some())
            .find_map(|(key, m)| {
                let m = m.value.as_ref()?;
                m.names()
                    .into_iter()
                    .find(|name| m.get(name).is_some_and(same))
                    .map(|name| format!("{key}.{name}"))
            })
    }

    /// Makes a compiled module importable under `key`. A module that
    /// already ran keeps its value: a module's top level runs once (04).
    pub(crate) fn register_module(&mut self, key: &str, proto: Rc<RtProto>) {
        if self.modules.get(key).is_some_and(|st| st.value.is_some()) {
            return;
        }
        self.modules.insert(
            key.to_string(),
            ModuleState {
                proto: Some(proto),
                value: None,
                loading: false,
            },
        );
    }

    /// Makes a Rust-built module (a `_native.*` bridge) importable under `key`.
    pub(crate) fn register_native(&mut self, key: &str, module: Rc<Module>) {
        self.modules.insert(
            key.to_string(),
            ModuleState {
                proto: None,
                value: Some(module),
                loading: false,
            },
        );
    }
}

/// Test access to the module table. These hand modules and protos across
/// the API, so they exist only for the crate's single-threaded tests.
#[cfg(test)]
impl Runtime {
    pub(crate) fn register_native(&self, key: &str, module: Rc<Module>) {
        self.world().register_native(key, module);
    }

    /// Registers the loaded entry as the module `key`.
    pub(crate) fn register_main_as(&self, key: &str) {
        let mut w = self.world();
        let main = w.main.clone().expect("a loaded entry");
        w.register_module(key, main);
    }

    pub(crate) fn import_module(&self, key: &str) -> Result<Rc<Module>, Stop> {
        self.task().import_module(key)
    }
}

impl Task<'_> {
    /// A module's value, running its body first if needed as the root of
    /// a run (boot uses it for prelude.fung). No fung code runs on the
    /// task yet, so the body gets the run loop to itself (`run_root`
    /// refuses a run inside a run).
    pub(crate) fn import_module(&mut self, key: &str) -> Result<Rc<Module>, Stop> {
        match self.begin_import(key, no_pos())? {
            Imported::Ready(m) => Ok(m),
            Imported::Body(b) => {
                let r = self.run_root(super::Todo::Loop);
                if r.is_err() {
                    self.frames.clear();
                    self.conts.clear();
                }
                r?;
                Ok(self.end_import(&b))
            }
        }
    }

    /// IMPORT: pushes the module registered under the constant `key`. On
    /// the first import the module's body runs first, in this run loop:
    /// a continuation waits for it and pushes the module (`cont.rs`).
    pub(super) fn exec_import(&mut self, fi: usize, arg: usize, pos: Pos) -> Result<(), Stop> {
        let key = match &self.frames[fi].proto.consts[arg] {
            Value::Str(s) => s.clone(),
            v => panic!("IMPORT of a non-string constant {v:?}"),
        };
        let depth = self.frames.len();
        let height = self.stack.len();
        match self.begin_import(&key, pos)? {
            Imported::Ready(m) => self.push(Value::Module(m)),
            Imported::Body(b) => self.conts.push(ContFrame {
                depth,
                height,
                pos,
                native: None,
                tail: false,
                k: Box::new(ImportBody(Some(b))),
            }),
        }
        Ok(())
    }

    /// The module `key` if it is loaded; otherwise its body is pushed as a
    /// frame, with its own globals, to run like a call.
    fn begin_import(&mut self, key: &str, pos: Pos) -> Result<Imported, Stop> {
        let not_registered = || {
            err(
                "E0801",
                pos,
                vec![Arg::Str(format!("module `{key}` was not registered"))],
            )
        };
        let st = self.world.modules.get_mut(key).ok_or_else(not_registered)?;
        if let Some(m) = &st.value {
            return Ok(Imported::Ready(m.clone()));
        }
        if st.loading {
            return Err(err(
                "E0801",
                pos,
                vec![Arg::Str(format!(
                    "module `{key}` imports itself at run time"
                ))],
            ));
        }
        st.loading = true;
        let proto = st.proto.clone().ok_or_else(not_registered)?;
        let globals = self.world.new_globals(proto.global_names.len());
        let base = self.stack.len();
        self.stack.resize(base + proto.num_locals, None);
        self.frames.push(Frame {
            proto: proto.clone(),
            ip: 0,
            base,
            closure: None,
            globals: globals.clone(),
            tail_calls: 0,
            entry: None,
            tail_from: None,
        });
        Ok(Imported::Body(Body {
            key: key.to_string(),
            proto,
            globals,
        }))
    }

    /// The body of module `b` has run: the exported globals that were
    /// assigned become the module's members, in export order.
    fn end_import(&mut self, b: &Body) -> Rc<Module> {
        let mut m = Module::new(module_name(&b.proto.name), &b.key);
        {
            let g = b.globals.borrow();
            for name in &b.proto.exports {
                let slot = b.proto.global_names.iter().position(|n| n == name);
                if let Some(Some(v)) = slot.map(|i| &g[i]) {
                    m.set(name, v.clone());
                }
            }
        }
        let m = Rc::new(m);
        let st = self.world.modules.get_mut(&b.key).expect("registered");
        st.value = Some(m.clone());
        st.loading = false;
        m
    }
}

/// What `begin_import` found.
enum Imported {
    Ready(Rc<Module>),
    /// The body's frame is pushed.
    Body(Body),
}

/// A module whose body runs.
struct Body {
    key: String,
    proto: Rc<RtProto>,
    globals: Globals,
}

/// The continuation of a first IMPORT: the body's value is not used, a
/// module is its globals.
struct ImportBody(Option<Body>);

impl Cont for ImportBody {
    fn resume(&mut self, t: &mut Task<'_>, _: Value) -> Result<Next, Stop> {
        let b = self.0.take().expect("the module body");
        Ok(Next::Done(Value::Module(t.end_import(&b))))
    }
}

/// Strips the `<module …>` frame label the loader gives a module body.
fn module_name(label: &str) -> &str {
    let s = label.strip_prefix("<module ").unwrap_or(label);
    s.strip_suffix('>').unwrap_or(s)
}
