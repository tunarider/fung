//! The core prelude (06): functions implemented in Rust that every fung
//! program can call without importing anything (Go `internal/prelude`).
//! The combinators built from these live in `extras/prelude.fung`; the
//! concurrency natives and the process bridges are added by the runtime.

mod collections;
mod convert;
mod io;
mod strings;

#[cfg(test)]
mod go_cases;
#[cfg(test)]
mod tests;

use std::cell::OnceCell;
use std::rc::Rc;

use fung_front::diag::Arg;

use crate::native::nat::{self, def, fail, opt, param, rest};
use crate::value::{ListBuilder, Value, intern, type_name};
use crate::vm::{Stop, Task};

pub use io::dbg;

thread_local! {
    /// The natives are made once per thread: they are leaked (`Native::leak`).
    static CORE: OnceCell<Vec<(&'static str, Value)>> = const { OnceCell::new() };
}

/// The core prelude: the value operations the language guarantees (06),
/// plus `args`, an empty list here (the runtime puts the script arguments
/// in its place).
pub fn core() -> Vec<(&'static str, Value)> {
    CORE.with(|c| c.get_or_init(build).clone())
}

fn build() -> Vec<(&'static str, Value)> {
    let mut p: Vec<(&'static str, Value)> = Vec::new();
    p.push(("args", nat::list(Vec::new())));
    let mut add = |v: Value| {
        let Value::Native(n) = &v else {
            unreachable!("def makes natives")
        };
        p.push((n.name, v));
    };
    add(def("print", print_fn, vec![rest("args")]));
    add(def("eprint", io::eprint_fn, vec![rest("args")]));
    add(def("fail", fail_fn, vec![rest("args")]));
    add(def("at_exit", at_exit_fn, vec![param("f")]));
    add(def("len", len_fn, vec![param("x")]));
    add(def("map", map_fn, vec![param("xs"), param("f")]));
    add(def("type_of", type_of_fn, vec![param("x")]));
    add(def("exit", io::exit_fn, vec![opt("code", Value::Int(0))]));
    add(def(
        "read_line",
        io::read_line_fn,
        vec![opt("f", Value::Nil)],
    ));
    add(def(
        "read_lines",
        io::read_lines_fn,
        vec![opt("f", Value::Nil)],
    ));
    add(Value::Native(dbg()));
    collections::add(&mut add);
    strings::add(&mut add);
    convert::add(&mut add);
    p
}

/// The rest-parameter list a native gets.
fn rest_items(v: &Value) -> &crate::value::List {
    match v {
        Value::List(l) => l,
        _ => unreachable!("a rest parameter is a list"),
    }
}

/// `at_exit(f)` registers a function to run when the program ends (03).
fn at_exit_fn(t: &mut Task<'_>, mut a: Vec<Value>) -> Result<Value, Stop> {
    nat::need_fn("f", &a[0])?;
    t.at_exit(a.swap_remove(0));
    Ok(Value::Nil)
}

/// `fail(...)` raises E0803 with the joined arguments as the message
/// (06: `x ?? fail("…")`). The VM fills in the position.
fn fail_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let items = rest_items(&a[0]);
    let msg = if items.is_empty() {
        "fail".to_string()
    } else {
        nat::join_for_print(items.iter())
    };
    Err(fail("E0803", vec![Arg::Str(msg)]))
}

/// `print(...)` joins its arguments with spaces and writes one line (06).
fn print_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let line = nat::join_for_print(rest_items(&a[0]).iter()) + "\n";
    t.stdout()
        .write_all(line.as_bytes())
        .map_err(nat::write_err)?;
    Ok(Value::Nil)
}

/// `len(x)` counts runes or elements (02).
fn len_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Ok(Value::Int(crate::value::index::len(&a[0])?))
}

/// `type_of(x)` is the type name as a symbol (07).
fn type_of_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Ok(Value::Sym(intern(&type_name(&a[0]))))
}

/// Go cannot make a slice of more than 2^48 bytes (16-byte interfaces):
/// `make` panics, which the VM reports as E0801.
const MAX_SLICE: usize = 1 << 44;

/// `map(xs, f)` applies `f` to every element of a list or range and
/// collects the results (06). `for` lowers to this (03).
fn map_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    if !nat::is_fn(&a[1]) {
        nat::check_items(&a[0], "map over")?;
        return Err(nat::wrong_arg("f", "fn", &a[1]));
    }
    let hint = nat::size_hint(&a[0]);
    if hint > MAX_SLICE {
        return Err(fail(
            "E0801",
            vec![Arg::Str(
                "runtime error: makeslice: cap out of range".into(),
            )],
        ));
    }
    // Room for a modest hint only: memory comes as results arrive.
    let mut out = ListBuilder::with_capacity(hint.min(1 << 16));
    let mut buf = Vec::with_capacity(1);
    let f = &a[1];
    nat::each(t, &a[0], "map over", |t, x| {
        buf.push(x);
        out.push(t.call_buf(f, &mut buf)?);
        Ok(())
    })?;
    Ok(Value::List(Rc::new(out.finish())))
}
