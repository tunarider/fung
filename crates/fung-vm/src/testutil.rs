//! Test helpers: compile a program with the front end, link it against a
//! small test prelude, and run it the way the fung command will (Go
//! `cmd/fung/pipeline.go` `execute`): the entry, then the exit handlers,
//! then the exit code. Everything runs on a 4 GiB thread, because value
//! formatting and comparison recurse on deep nesting like Go.
//!
//! The prelude here is a stand-in until the real natives are ported: just
//! what the VM tests need, with Go's parameter tables and errors.

use std::cell::RefCell;
use std::io::{Cursor, Write};
use std::rc::Rc;

use fung_front::bytecode::Proto;
use fung_front::diag::{self, Arg, CallSite, Diag, error};
use fung_front::loader::{Loader, Stage};

use crate::value::{
    List, Native, NativeParam, Tag, Value, format, format_for_print, intern, type_name,
};
use crate::vm::{Prelude, Runtime, Stop, Task, link};

/// Runs `f` on a thread with a 4 GiB stack and returns its result.
pub fn on_big_stack<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    std::thread::Builder::new()
        .stack_size(4 << 30)
        .spawn(f)
        .expect("spawn test thread")
        .join()
        .unwrap_or_else(|e| std::panic::resume_unwind(e))
}

/// An error as a test sees it.
#[derive(Debug, Clone)]
pub struct ErrInfo {
    pub code: String,
    pub line: u32,
    pub col: u32,
    /// `error[E0803]: boom`.
    pub headline: String,
    pub path: Vec<CallSite>,
}

impl ErrInfo {
    fn of(d: &Diag) -> ErrInfo {
        ErrInfo {
            code: d.code.clone(),
            line: d.pos.line,
            col: d.pos.col,
            headline: diag::headline(d),
            path: d.call_path.clone(),
        }
    }
}

/// What a run produced.
#[derive(Debug, Default)]
pub struct Outcome {
    /// The entry's result in literal form, when it returned one.
    pub value: Option<String>,
    pub out: String,
    pub err: String,
    /// The entry's error code: "" on success, "exit" for exit(n).
    pub code: String,
    /// The process exit code the fung command would use.
    pub exit: i32,
    /// The entry's runtime error.
    pub error: Option<ErrInfo>,
    /// Errors the exit handlers reported.
    pub handler_errors: Vec<ErrInfo>,
}

/// How to run: standard input and extra natives (made on the run thread,
/// since values cannot cross threads).
#[derive(Clone, Copy)]
pub struct Opts {
    pub stdin: &'static str,
    pub extra: fn() -> Vec<Value>,
    /// Changes the compiled entry before it is linked.
    pub patch: fn(&mut Proto),
}

impl Default for Opts {
    fn default() -> Opts {
        Opts {
            stdin: "",
            extra: Vec::new,
            patch: |_| {},
        }
    }
}

/// Runs a program as `fung -e` would: (stdout, stderr, exit code).
pub fn run(src: &str) -> (String, String, i32) {
    let o = exec(src, Opts::default());
    (o.out, o.err, o.exit)
}

/// Runs a program with the test prelude plus `opts.extra`.
pub fn exec(src: &str, opts: Opts) -> Outcome {
    let src = src.to_string();
    on_big_stack(move || {
        let mut p = prelude();
        for v in (opts.extra)() {
            if let Value::Native(n) = &v {
                p.0.insert(n.name.to_string(), v.clone());
            }
        }
        exec_on(&src, p, opts)
    })
}

/// Runs a program with a given prelude on the current thread.
pub fn exec_with_prelude(src: &str, p: Prelude) -> Outcome {
    exec_on(src, p, Opts::default())
}

/// A shared byte buffer the runtime writes into.
#[derive(Clone, Default)]
struct Buf(Rc<RefCell<Vec<u8>>>);

impl Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Buf {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.borrow()).into_owned()
    }
}

fn exec_on(src: &str, p: Prelude, opts: Opts) -> Outcome {
    let mut o = Outcome::default();
    let loader = Loader {
        extra_prelude: p.0.keys().cloned().collect(),
        ..Loader::default()
    };
    let (b, errs) = loader.compile("-e", src, Stage::All);
    if !errs.is_empty() {
        o.err = errs.iter().map(|e| diag::format(e, &b.sources)).collect();
        o.code = errs[0].code.clone();
        o.exit = diag::EXIT_COMPILE;
        return o;
    }
    let mut entry = b.proto.expect("a compiled entry");
    (opts.patch)(&mut entry);
    let main = link(&entry, &p);
    let (out, err) = (Buf::default(), Buf::default());
    let mut rt = Runtime::new(
        Box::new(out.clone()),
        Box::new(err.clone()),
        Box::new(Cursor::new(opts.stdin.as_bytes().to_vec())),
        b.sources,
    );
    match rt.run(main) {
        Ok(v) => o.value = Some(format(&v)),
        Err(Stop::Exit(_)) => o.code = "exit".into(),
        Err(Stop::Error(d)) => {
            o.code = d.code.clone();
            o.error = Some(ErrInfo::of(&d));
            if rt.exit_code().is_none() {
                let text = diag::format(&d, rt.sources());
                err.clone().write_all(text.as_bytes()).unwrap();
                o.exit = diag::EXIT_RUNTIME;
            }
        }
    }
    let before = rt.exit_code();
    if let Some(n) = before {
        o.exit = n;
    }
    for d in rt.run_exit_handlers() {
        let text = diag::format(&d, rt.sources());
        err.clone().write_all(text.as_bytes()).unwrap();
        if o.exit == diag::EXIT_OK {
            o.exit = diag::EXIT_RUNTIME;
        }
        o.handler_errors.push(ErrInfo::of(&d));
    }
    if let (None, Some(n)) = (before, rt.exit_code()) {
        o.exit = n; // an exit requested inside a handler
    }
    drop(rt);
    o.out = out.text();
    o.err = err.text();
    o
}

fn param(name: &'static str) -> NativeParam {
    NativeParam {
        name,
        default: None,
        rest: false,
    }
}

fn opt(name: &'static str, default: Value) -> NativeParam {
    NativeParam {
        name,
        default: Some(default),
        rest: false,
    }
}

fn rest(name: &'static str) -> NativeParam {
    NativeParam {
        name,
        default: None,
        rest: true,
    }
}

fn fail(code: &str, args: Vec<Arg>) -> Stop {
    Stop::Error(Box::new(error(code, crate::vm::no_pos(), args)))
}

fn s(x: &str) -> Arg {
    Arg::Str(x.to_string())
}

fn items(v: &Value) -> Vec<Value> {
    match v {
        Value::List(l) => l.to_vec(),
        _ => Vec::new(),
    }
}

fn join_for_print(v: &Value) -> String {
    items(v)
        .iter()
        .map(format_for_print)
        .collect::<Vec<_>>()
        .join(" ")
}

fn ok(v: Value) -> Value {
    Value::Tag(Rc::new(Tag {
        name: intern("Ok"),
        args: vec![v],
    }))
}

fn is_fn(v: &Value) -> bool {
    matches!(v, Value::Closure(_) | Value::Native(_))
}

fn write_err(e: std::io::Error) -> Stop {
    fail("E0801", vec![s(&e.to_string())])
}

/// The test prelude: Go's `print`, `eprint`, `fail`, `exit`, `at_exit`,
/// `len`, `type_of`, `read_line`, `read_lines` and a list/range `fold`.
pub fn prelude() -> Prelude {
    let mut p = Prelude::default();
    let mut add = |name: &'static str, params: Vec<NativeParam>, f: crate::value::NativeFn| {
        p.0.insert(
            name.to_string(),
            Value::Native(Native::leak(name, params, f)),
        );
    };
    add("print", vec![rest("args")], |t, a| {
        let line = join_for_print(&a[0]) + "\n";
        t.stdout().write_all(line.as_bytes()).map_err(write_err)?;
        Ok(Value::Nil)
    });
    add("eprint", vec![rest("args")], |t, a| {
        let line = join_for_print(&a[0]) + "\n";
        t.stderr().write_all(line.as_bytes()).map_err(write_err)?;
        Ok(Value::Nil)
    });
    add("fail", vec![rest("args")], |_, a| {
        let msg = if items(&a[0]).is_empty() {
            "fail".to_string()
        } else {
            join_for_print(&a[0])
        };
        Err(fail("E0803", vec![Arg::Str(msg)]))
    });
    add("exit", vec![opt("code", Value::Int(0))], |_, a| {
        match a[0] {
            Value::Int(n) => Err(Stop::Exit(n as i32)),
            ref v => Err(fail("E0403", vec![s("code"), s("int"), s(&type_name(v))])),
        }
    });
    add("at_exit", vec![param("f")], |t, a| {
        if !is_fn(&a[0]) {
            return Err(fail("E0403", vec![s("f"), s("fn"), s(&type_name(&a[0]))]));
        }
        t.at_exit(a[0].clone());
        Ok(Value::Nil)
    });
    add("len", vec![param("x")], |_, a| {
        crate::value::index::len(&a[0])
            .map(Value::Int)
            .map_err(|d| Stop::Error(Box::new(d)))
    });
    add("type_of", vec![param("x")], |_, a| {
        Ok(Value::Sym(intern(&type_name(&a[0]))))
    });
    add("read_line", vec![opt("f", Value::Nil)], |t, _| {
        Ok(ok(match t.read_line() {
            Some(l) => Value::str(trim_line(&l)),
            None => Value::Nil,
        }))
    });
    add("read_lines", vec![opt("f", Value::Nil)], |t, _| {
        let mut lines = Vec::new();
        while let Some(l) = t.read_line() {
            lines.push(Value::str(trim_line(&l)));
        }
        Ok(ok(Value::List(Rc::new(List::new(lines)))))
    });
    add(
        "fold",
        vec![param("xs"), param("init"), param("f")],
        |t, a| {
            let xs: Vec<Value> = match &a[0] {
                Value::Range(r) => {
                    let end = if r.inclusive { r.end + 1 } else { r.end };
                    (r.start..end).map(Value::Int).collect()
                }
                v => items(v),
            };
            let mut acc = a[1].clone();
            for x in xs {
                acc = t.call(&a[2], vec![acc, x])?;
            }
            Ok(acc)
        },
    );
    p
}

fn trim_line(l: &str) -> &str {
    let l = l.strip_suffix('\n').unwrap_or(l);
    l.strip_suffix('\r').unwrap_or(l)
}

/// Go `native_test.go` `padNative`: a default and a rest parameter.
pub fn pad() -> Vec<Value> {
    vec![Value::Native(Native::leak(
        "pad",
        vec![param("s"), opt("n", Value::Int(2)), rest("tail")],
        |_, a| {
            let (Value::Str(s), Value::Int(n)) = (&a[0], &a[1]) else {
                return Err(fail("E0801", vec![self::s("pad: bad arguments")]));
            };
            Ok(Value::str(&format!(
                "{s}{}{}",
                "*".repeat(*n as usize),
                format(&a[2])
            )))
        },
    ))]
}

/// A native that panics, for the panic fence.
pub fn boom() -> Vec<Value> {
    vec![Value::Native(Native::leak(
        "boom",
        vec![],
        |_: &mut Task, _| panic!("x"),
    ))]
}
