//! The core prelude (06): functions implemented in Rust that every fung
//! program can call without importing anything.
//! The combinators built from these live in `extras/prelude.fung`; the
//! concurrency natives and the process bridges are added by the runtime.

mod collections;
mod convert;
mod io;
mod strings;

#[cfg(test)]
mod run_cases;
#[cfg(test)]
mod tests;

use std::rc::Rc;
use std::sync::OnceLock;

use fung_front::diag::Arg;

use crate::native::nat::{self, def, def_step, fail, opt, param, rest};
use crate::native::walk::{Cursor, PerItem, walk};
use crate::value::{ListBuilder, Lit, Native, Tag, Value, intern, type_name};
use crate::vm::{Flow, Stop, Task};

pub use io::dbg;
pub(crate) use strings::{MAX_CHARS, too_many_chars};

/// The natives are made once per process: they are leaked (`Native::leak`).
static CORE: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The core prelude: the value operations the language guarantees (06),
/// plus `args`, an empty list here (the runtime puts the script arguments
/// in its place).
pub fn core() -> Vec<(&'static str, Value)> {
    let mut p = vec![("args", nat::list(Vec::new()))];
    p.extend(
        CORE.get_or_init(build)
            .iter()
            .map(|(n, f)| (*n, Value::Native(f))),
    );
    p
}

fn build() -> Vec<(&'static str, &'static Native)> {
    let mut p: Vec<(&'static str, &'static Native)> = Vec::new();
    let mut add = |v: Value| {
        let Value::Native(n) = &v else {
            unreachable!("def makes natives")
        };
        p.push((n.name, *n));
    };
    add(def("print", print_fn, vec![rest("args")]));
    add(def("eprint", io::eprint_fn, vec![rest("args")]));
    add(def("fail", fail_fn, vec![rest("args")]));
    add(def("len", len_fn, vec![param("x")]));
    add(def_step("map", map_fn, vec![param("xs"), param("f")]));
    add(def_step(
        "try_map",
        try_map_fn,
        vec![param("xs"), param("f")],
    ));
    add(def("type_of", type_of_fn, vec![param("x")]));
    add(def("exit", io::exit_fn, vec![opt("code", Lit::Int(0))]));
    add(def_step(
        "read_line",
        io::read_line_fn,
        vec![opt("f", Lit::Nil)],
    ));
    add(def_step(
        "read_lines",
        io::read_lines_fn,
        vec![opt("f", Lit::Nil)],
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

/// `map(xs, f)` applies `f` to every element of a list or range and
/// collects the results (06). `for` lowers to this (03).
fn map_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "map over")?;
    nat::need_fn("f", &a[1])?;
    nat::limit_elements(&a[0])?;
    struct Map(ListBuilder);
    impl PerItem for Map {
        fn args(&mut self, t: &mut Task<'_>, x: Value) -> usize {
            t.push_arg(x);
            1
        }
        fn got(&mut self, r: Value) -> Result<bool, Stop> {
            self.0.push(r);
            Ok(false)
        }
        fn result(&mut self, _: bool) -> Result<Value, Stop> {
            Ok(Value::List(Rc::new(std::mem::take(&mut self.0).finish())))
        }
    }
    let hint = nat::size_hint(&a[0]);
    // Room for a modest hint only: memory comes as results arrive.
    let p = Map(ListBuilder::with_capacity(hint.min(1 << 16)));
    walk(t, Cursor::new(&a[0]), a[1].clone(), p)
}

/// `try_map(xs, f)` (03): `Ok` of the values `f`'s `Ok`s hold, or the
/// first `Err` it returns, after which no element is visited. A result of
/// another kind is E0605, as for `!` and `?`.
fn try_map_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "map over")?;
    nat::need_fn("f", &a[1])?;
    nat::limit_elements(&a[0])?;
    struct TryMap {
        out: ListBuilder,
        err: Option<Value>,
    }
    impl PerItem for TryMap {
        fn args(&mut self, t: &mut Task<'_>, x: Value) -> usize {
            t.push_arg(x);
            1
        }
        fn got(&mut self, r: Value) -> Result<bool, Stop> {
            match &r {
                Value::Tag(tag) if tag.name.name() == "Ok" => {
                    // `Ok(a, b)` holds the tuple, as `?` unwraps it.
                    self.out.push(match tag.args.as_slice() {
                        [v] => v.clone(),
                        vs => Value::tuple(vs.to_vec()),
                    });
                    Ok(false)
                }
                Value::Tag(tag) if tag.name.name() == "Err" => {
                    self.err = Some(r);
                    Ok(true)
                }
                _ => Err(fail(
                    "E0605",
                    vec![nat::s("try_map"), Arg::Str(type_name(&r))],
                )),
            }
        }
        fn result(&mut self, _: bool) -> Result<Value, Stop> {
            Ok(match self.err.take() {
                Some(e) => e,
                None => Value::Tag(Rc::new(Tag {
                    name: intern("Ok"),
                    args: vec![Value::List(Rc::new(std::mem::take(&mut self.out).finish()))],
                })),
            })
        }
    }
    let p = TryMap {
        out: ListBuilder::with_capacity(nat::size_hint(&a[0]).min(1 << 16)),
        err: None,
    };
    walk(t, Cursor::new(&a[0]), a[1].clone(), p)
}
