//! Booting the prelude (Go `cmd/fung/pipeline.go` `bootPrelude`, 06:
//! prelude만 시작 시): the core natives, the concurrency natives,
//! the `_native.conc` and `_native.process` bridges, and the public names
//! of `extras/prelude.fung` and (Rust only, stage 3b) `extras/rust/agent.fung`,
//! compiled and run on the runtime at startup.
//!
//! File ids: a `FileId` indexes the `SourceMap` of one loader compile, and
//! the program and the prelude are compiled separately, so both start at
//! 0. `boot` appends the prelude's files to the runtime's source map and
//! moves every position of the prelude's protos (and of its compile
//! errors) past the files already there. Create the runtime with the
//! program's sources first, then boot: every proto's `FileId` then names
//! its own file in call paths and excerpts. (Go positions carry the file
//! name, so it has no such step.)

use std::rc::Rc;

use fung_front::bytecode::Proto;
use fung_front::diag::{Arg, Diag, error};
use fung_front::extras;
use fung_front::loader::{Loaded, Loader, Stage};

use crate::native::{fs, nat, path, process, time};
use crate::value::{Module, Value};
use crate::vm::{Prelude, Stop, Task, World, link, no_pos};
use crate::{conc, prelude};

#[cfg(test)]
mod tests;

/// The name prelude.fung positions show (Go `bootPrelude`).
const PRELUDE_FILE: &str = "extras/prelude.fung";

/// The name agent.fung positions show, and the module key it runs as (a
/// key no `import` can name: the loader does not know the file).
const AGENT_FILE: &str = "extras/rust/agent.fung";
const AGENT_KEY: &str = "<agent>";

/// Builds the prelude on the task's runtime and returns it; the program is
/// linked against it (`Runtime::boot` keeps it in the world). `args`
/// become the prelude's `args` and the runtime's. Everything here runs
/// with the task's run token held.
///
/// The errors are the files' compile errors or the error of a body,
/// with positions in the runtime's sources. Booting the embedded file does
/// not fail in practice; a runtime whose boot failed should not be reused
/// (a module whose body failed stays "loading", like Go).
pub(crate) fn boot(t: &mut Task<'_>, args: Vec<String>) -> Result<Prelude, Vec<Diag>> {
    let mut p = Prelude::default();
    for (name, v) in prelude::core() {
        p.0.insert(name.to_string(), v);
    }
    p.0.insert(
        "args".to_string(),
        nat::list(args.iter().map(|a| Value::str(a)).collect()),
    );
    t.world().args = args;
    for (name, v) in conc::natives() {
        p.0.insert(name.to_string(), v);
    }
    let Some(src) = extras::source("prelude") else {
        return Ok(p);
    };
    boot_file(t, &mut p, PRELUDE_FILE, "prelude", src)?;
    // Stage 3b, Rust only: `agent`, `swap` and `deref`, written in fung
    // over the prelude (`fung_front::builtins` lists the difference).
    boot_file(t, &mut p, AGENT_FILE, AGENT_KEY, extras::AGENT)?;
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
    // Go `register`: the bridges the file imports, then its modules.
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

/// The Rust-built `_native.*` modules (Go `vm.NativeModules()` and
/// `native.Bridges()`, the part stage 2 has). The command registers the
/// ones a program imports with it, as Go `register` does.
pub fn native_module(key: &str) -> Option<Rc<Module>> {
    match key {
        "_native.conc" => Some(conc::module()),
        "_native.fs" => Some(fs::module()),
        "_native.path" => Some(path::module()),
        "_native.process" => Some(process::module()),
        "_native.time" => Some(time::module()),
        _ => None,
    }
}

/// The standard modules stage 2 does not run: each is a thin fung file
/// over a `_native.*` bridge the Rust port does not have yet. `cli` is
/// pure fung and is not among them.
pub fn unsupported_module(key: &str) -> bool {
    matches!(key, "json" | "regex" | "http")
}

/// Makes a program's modules importable (Go `register`): compiled modules
/// linked against `p`, the `_native.*` bridges stage 2 has, and the
/// standard modules it does not run as E0805 placeholders. A bridge stage
/// 2 does not have stays unregistered (only those placeholders import
/// it), so a lookup of it is still an internal error.
pub(crate) fn register(w: &mut World, modules: &[Loaded], p: &Prelude) {
    for m in modules {
        match &m.proto {
            Some(_) if unsupported_module(&m.key) => w.register_unsupported(&m.key),
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
