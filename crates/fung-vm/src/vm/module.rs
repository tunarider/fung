//! Modules at run time (04, Go `internal/vm/module.go`): the runtime's
//! table of importable modules and IMPORT. A module's body runs the first
//! time it is imported, as a nested frame on the importing task with its
//! own globals; afterwards its public globals become the immutable module
//! value every later import gets.

use std::rc::Rc;

use fung_front::diag::Arg;
use fung_front::source::Pos;

#[cfg(test)]
use super::Runtime;
use super::frame::Frame;
use super::world::World;
use super::{RtProto, Stop, Task, err, no_pos};
use crate::value::{Module, Value};

/// One registered module: its compiled body and, once the body has run
/// (or for a native module), its value.
pub(super) struct ModuleState {
    proto: Option<Rc<RtProto>>,
    value: Option<Rc<Module>>,
    /// The body is running: importing it again is a run-time cycle.
    loading: bool,
    /// A standard module stage 2 does not run: importing it is E0805.
    unsupported: bool,
}

impl World {
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
                unsupported: false,
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
                unsupported: false,
            },
        );
    }

    /// Makes importing `key` report E0805 `not supported yet: <key>` at
    /// the import: a standard module whose `_native.*` bridge stage 2 does
    /// not have (see `boot::unsupported_module`).
    pub(crate) fn register_unsupported(&mut self, key: &str) {
        self.modules.insert(
            key.to_string(),
            ModuleState {
                proto: None,
                value: None,
                loading: false,
                unsupported: true,
            },
        );
    }
}

/// Test access to the module table. These hand modules and protos across
/// the API, so they exist only for the crate's single-threaded tests.
#[cfg(test)]
impl Runtime {
    pub(crate) fn register_native(&self, key: &str, module: Rc<Module>) {
        self.lock().register_native(key, module);
    }

    pub(crate) fn register_unsupported(&self, key: &str) {
        self.lock().register_unsupported(key);
    }

    /// Registers the loaded entry as the module `key`.
    pub(crate) fn register_main_as(&self, key: &str) {
        let mut w = self.lock();
        let main = w.main.clone().expect("a loaded entry");
        w.register_module(key, main);
    }

    pub(crate) fn import_module(&self, key: &str) -> Result<Rc<Module>, Stop> {
        self.task().import_module(key)
    }
}

impl Task<'_> {
    /// A module's value, running its body first if needed, behind the
    /// panic fence (boot uses it for prelude.fung, Go `ImportModule`).
    pub(crate) fn import_module(&mut self, key: &str) -> Result<Rc<Module>, Stop> {
        let mut out = None;
        self.guarded(|t| {
            out = Some(t.import(key, no_pos())?);
            Ok(Value::Nil)
        })?;
        Ok(out.expect("import returned a module"))
    }

    /// IMPORT: pushes the module registered under the constant `key`.
    pub(super) fn exec_import(&mut self, fi: usize, arg: usize, pos: Pos) -> Result<(), Stop> {
        let key = match &self.frames[fi].proto.consts[arg] {
            Value::Str(s) => s.clone(),
            v => panic!("IMPORT of a non-string constant {v:?}"),
        };
        let m = self.import(&key, pos)?;
        self.push(Value::Module(m));
        Ok(())
    }

    /// A module's value, running its body first when this is the first
    /// import. The body runs as a nested frame on this task, like a call,
    /// with its own globals; afterwards the exported globals that were
    /// assigned become the module's members, in export order.
    fn import(&mut self, key: &str, pos: Pos) -> Result<Rc<Module>, Stop> {
        let not_registered = || {
            err(
                "E0801",
                pos,
                vec![Arg::Str(format!("module `{key}` was not registered"))],
            )
        };
        let st = self.world.modules.get_mut(key).ok_or_else(not_registered)?;
        if let Some(m) = &st.value {
            return Ok(m.clone());
        }
        if st.unsupported {
            return Err(err("E0805", pos, vec![Arg::Str(key.to_string())]));
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
        let depth = self.frames.len();
        let base = self.stack.len();
        self.stack.resize(base + proto.num_locals, None);
        self.frames.push(Frame {
            proto: proto.clone(),
            ip: 0,
            base,
            closure: None,
            globals: globals.clone(),
            tail_calls: 0,
        });
        // The body's value is not used: a module is its globals.
        self.run(depth)?;
        let mut m = Module::new(module_name(&proto.name), key);
        {
            let g = globals.borrow();
            for name in &proto.exports {
                let slot = proto.global_names.iter().position(|n| n == name);
                if let Some(Some(v)) = slot.map(|i| &g[i]) {
                    m.set(name, v.clone());
                }
            }
        }
        let m = Rc::new(m);
        let st = self.world.modules.get_mut(key).expect("registered above");
        st.value = Some(m.clone());
        st.loading = false;
        Ok(m)
    }
}

/// Strips the `<module …>` frame label the loader gives a module body.
fn module_name(label: &str) -> &str {
    let s = label.strip_prefix("<module ").unwrap_or(label);
    s.strip_suffix('>').unwrap_or(s)
}
