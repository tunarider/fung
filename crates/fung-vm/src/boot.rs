//! Booting the prelude (06: prelude만 시작 시): the core natives, the
//! concurrency natives, the `_native.conc` and `_native.process` bridges,
//! and the public names of the prelude files written in fung
//! (`extras::PRELUDE_FILES`: `extras/prelude.fung`, then
//! `extras/agent.fung`), compiled and run on the runtime at startup.
//!
//! File ids: a `FileId` indexes the `SourceMap` of one loader compile, and
//! the program and the prelude are compiled separately, so both start at
//! 0. `boot` appends the prelude's files to the runtime's source map and
//! moves every position of the prelude's protos (and of its compile
//! errors) past the files already there. Create the runtime with the
//! program's sources first, then boot: every proto's `FileId` then names
//! its own file in call paths and excerpts.

use std::rc::Rc;

use fung_front::bytecode::Proto;
use fung_front::diag::{Arg, Diag, error};
use fung_front::extras;
use fung_front::loader::{Loaded, Loader, Stage};

use crate::native::{fs, json, nat, net, path, process, regex, time};
use crate::value::{Module, Value};
use crate::vm::{Prelude, Stop, Task, World, link, no_pos};
use crate::{conc, prelude};

#[cfg(test)]
mod spec_file;
#[cfg(test)]
mod tests;

/// The module key a prelude file runs as: `prelude` for prelude.fung,
/// `<name>` for the others (a key no `import` can name: the loader does
/// not know those files).
fn prelude_key(name: &str) -> String {
    if name == "prelude" {
        name.to_string()
    } else {
        format!("<{name}>")
    }
}

/// The prelude's native values: the core natives, the concurrency natives
/// and `args` (the script's arguments).
pub(crate) fn native_prelude(args: &[String]) -> Prelude {
    let mut p = Prelude::default();
    for (name, v) in prelude::core() {
        p.0.insert(name.to_string(), v);
    }
    p.0.insert(
        "args".to_string(),
        nat::list(args.iter().map(|a| Value::str(a)).collect()),
    );
    for (name, v) in conc::natives() {
        p.0.insert(name.to_string(), v);
    }
    p
}

/// Builds the prelude on the task's runtime and returns it; the program is
/// linked against it (`Runtime::boot` keeps it in the world). `args`
/// become the prelude's `args` and the runtime's. Everything here runs
/// with the task's run token held.
///
/// The errors are the files' compile errors or the error of a body,
/// with positions in the runtime's sources. Booting the embedded file does
/// not fail in practice; a runtime whose boot failed should not be reused
/// (a module whose body failed stays "loading").
pub(crate) fn boot(t: &mut Task<'_>, args: Vec<String>) -> Result<Prelude, Vec<Diag>> {
    let mut p = native_prelude(&args);
    t.world().args = args;
    for (name, src) in extras::PRELUDE_FILES {
        boot_file(t, &mut p, &extras::path(name), &prelude_key(name), src)?;
    }
    Ok(p)
}

/// Compiles an embedded prelude file (`internal`, so it may import
/// `_native.*`), runs it as the module `key` and adds its public names
/// to `p`; its code links against the names `p` has so far.
fn boot_file(
    t: &mut Task<'_>,
    p: &mut Prelude,
    file: &str,
    key: &str,
    src: &str,
) -> Result<(), Vec<Diag>> {
    let loader = Loader {
        internal: true,
        ..Loader::default()
    };
    let (b, errs) = loader.compile(file, src, Stage::All);
    let base = t.world().add_sources(&b.sources);
    if !errs.is_empty() {
        return Err(errs
            .into_iter()
            .map(|mut d| {
                d.pos.file.0 += base;
                d
            })
            .collect());
    }
    // Registers the bridges the file imports, then its modules.
    for m in b.modules {
        match m.proto {
            Some(mut proto) => {
                shift_files(&mut proto, base);
                t.world().register_module(&m.key, link(&proto, p));
            }
            None => match native_module(&m.key) {
                Some(module) => t.world().register_native(&m.key, module),
                None => {
                    return Err(vec![internal(format!(
                        "no native module `{}` for {file}",
                        m.key
                    ))]);
                }
            },
        }
    }
    let mut entry = b.proto.expect("a compiled prelude file");
    shift_files(&mut entry, base);
    t.world().register_module(key, link(&entry, p));
    let m = match t.import_module(key) {
        Ok(m) => m,
        Err(Stop::Error(d)) => return Err(vec![*d]),
        Err(Stop::Exit(n)) => {
            return Err(vec![internal(format!("exit({n}) while booting {file}"))]);
        }
    };
    for name in m.names() {
        if let Some(v) = m.get(name) {
            p.0.insert(name.to_string(), v.clone());
        }
    }
    Ok(())
}

/// Builds a `_native.*` bridge.
pub type NativeModule = fn() -> Rc<Module>;

/// The `_native.*` bridges by key, sorted: the one list of them. The
/// command registers the ones a program imports (`register`), and
/// `spec/builtins.txt` lists their members (generated from this table).
pub const NATIVE_MODULES: &[(&str, NativeModule)] = &[
    ("_native.conc", conc::module),
    ("_native.fs", fs::module),
    ("_native.json", json::module),
    ("_native.net", net::module),
    ("_native.path", path::module),
    ("_native.process", process::module),
    ("_native.regex", regex::module),
    ("_native.time", time::module),
];

/// The `_native.*` bridge `key`, if there is one ([`NATIVE_MODULES`]).
pub fn native_module(key: &str) -> Option<Rc<Module>> {
    NATIVE_MODULES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, module)| module())
}

/// Makes a program's modules importable: compiled modules linked against
/// `p`, and the `_native.*` bridges. Every standard module runs since
/// stage 4d (the E0805 placeholders for those that did not are gone); a
/// bridge that does not exist stays unregistered, so a lookup of it is an
/// internal error.
pub(crate) fn register(w: &mut World, modules: &[Loaded], p: &Prelude) {
    for m in modules {
        match &m.proto {
            Some(proto) => w.register_module(&m.key, link(proto, p)),
            None => {
                if let Some(module) = native_module(&m.key) {
                    w.register_native(&m.key, module);
                }
            }
        }
    }
}

/// Moves every position of a proto tree `base` files on.
fn shift_files(p: &mut Proto, base: u32) {
    for pos in &mut p.pos {
        pos.file.0 += base;
    }
    for child in &mut p.protos {
        shift_files(child, base);
    }
}

fn internal(msg: String) -> Diag {
    error("E0801", no_pos(), vec![Arg::Str(msg)])
}
