//! Test helpers: compile a program with the front end, link it against
//! the core prelude, and run it the way the fung command will (Go
//! `cmd/fung/pipeline.go` `execute`): the entry, then the exit handlers,
//! then the exit code. Everything runs on a 4 GiB thread, because value
//! formatting and comparison recurse on deep nesting like Go.
//!
//! The prelude is `crate::prelude::core()` plus test-only natives (Go's
//! VM tests pass their own natives the same way).

use std::cell::RefCell;
use std::io::{Cursor, Write};
use std::rc::Rc;

use fung_front::bytecode::Proto;
use fung_front::diag::{self, CallSite, Diag};
use fung_front::loader::{Loader, Stage};

use crate::native::nat;
use crate::value::{Native, Value, format};
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
    pub notes: Vec<String>,
    pub path: Vec<CallSite>,
}

impl ErrInfo {
    fn of(d: &Diag) -> ErrInfo {
        ErrInfo {
            code: d.code.clone(),
            line: d.pos.line,
            col: d.pos.col,
            headline: diag::headline(d),
            notes: d.notes.clone(),
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
    /// The entry's file name (Go tests compile `t.fung`).
    pub file: &'static str,
    /// Keep test blocks (`fung test`).
    pub tests: bool,
    /// Reads imported files (the loader's default reads the disk).
    pub read: fn(&str) -> std::io::Result<String>,
}

impl Default for Opts {
    fn default() -> Opts {
        Opts {
            stdin: "",
            extra: Vec::new,
            patch: |_| {},
            file: "-e",
            tests: false,
            read: Loader::default().read_file,
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
pub struct Buf(Rc<RefCell<Vec<u8>>>);

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
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.borrow()).into_owned()
    }
}

/// A compiled, linked entry with a runtime writing into two buffers.
pub struct Setup {
    pub rt: Runtime,
    pub main: Rc<crate::vm::RtProto>,
    pub out: Buf,
    pub err: Buf,
}

/// Compiles `src` and makes a runtime for it, or returns the compile
/// errors formatted as the fung command prints them.
pub fn setup(src: &str, p: &Prelude, opts: Opts) -> Result<Setup, (String, String)> {
    let loader = Loader {
        extra_prelude: p.0.keys().cloned().collect(),
        tests: opts.tests,
        read_file: opts.read,
        ..Loader::default()
    };
    let (b, errs) = loader.compile(opts.file, src, Stage::All);
    if !errs.is_empty() {
        let text = errs.iter().map(|e| diag::format(e, &b.sources)).collect();
        return Err((text, errs[0].code.clone()));
    }
    let mut entry = b.proto.expect("a compiled entry");
    (opts.patch)(&mut entry);
    let main = link(&entry, p);
    let (out, err) = (Buf::default(), Buf::default());
    let mut rt = Runtime::new(
        Box::new(out.clone()),
        Box::new(err.clone()),
        Box::new(Cursor::new(opts.stdin.as_bytes().to_vec())),
        b.sources,
    );
    // Go `register` (cmd/fung/pipeline.go). Native `_native.*` modules
    // have no proto; the caller registers them (`register_native`).
    for m in &b.modules {
        if let Some(proto) = &m.proto {
            rt.register_module(&m.key, link(proto, p));
        }
    }
    Ok(Setup { rt, main, out, err })
}

thread_local! {
    /// The in-memory files `run_files` serves to the loader.
    static FILES: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

fn mem_read(path: &str) -> std::io::Result<String> {
    FILES.with(|f| {
        f.borrow()
            .iter()
            .find(|(k, _)| k == path)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| std::io::Error::other(format!("no file {path}")))
    })
}

/// Runs the in-memory file `entry` with its imports read from `files`
/// (absolute paths), like Go `module_test.go` `runProgram`.
pub fn run_files(files: &[(&str, &str)], entry: &str) -> Outcome {
    let files: Vec<(String, String)> = files
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let entry = entry.to_string();
    on_big_stack(move || {
        let src = files
            .iter()
            .find(|(k, _)| *k == entry)
            .map(|(_, v)| v.clone())
            .expect("the entry is one of the files");
        FILES.with(|f| *f.borrow_mut() = files);
        let opts = Opts {
            file: leak(entry),
            read: mem_read,
            ..Opts::default()
        };
        exec_on(&src, prelude(), opts)
    })
}

/// Test-only: `Opts.file` is `&'static str` so `Opts` stays `Copy`.
fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// A fresh directory under the system temp dir, removed when dropped.
pub struct TempDir(std::path::PathBuf);

impl std::ops::Deref for TempDir {
    type Target = std::path::Path;

    fn deref(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Makes a uniquely named temp dir (process id, a counter and the time).
pub fn temp_dir() -> TempDir {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "fung-vm-test-{}-{}-{nanos}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    TempDir(dir)
}

/// Runs a file from disk as `fung run <path>` would: (stdout, stderr,
/// exit code). The path is passed to the loader as given, so it is the
/// entry's display name, like Go's `fung run` with that argument; tests
/// pass the absolute temp path, which is also what Go prints for it when
/// run from the repository root (imports outside the working directory
/// show as absolute paths).
pub fn run_file(path: &std::path::Path) -> (String, String, i32) {
    let path = path.to_string_lossy().into_owned();
    let o = on_big_stack(move || {
        let src = std::fs::read(&path)
            .map(|b| fung_front::source::decode_go(&b))
            .expect("read the entry file");
        let opts = Opts {
            file: leak(path),
            ..Opts::default()
        };
        exec_on(&src, prelude(), opts)
    });
    (o.out, o.err, o.exit)
}

/// The process exit status a shell sees for exit(n): the operating system
/// keeps the low 8 bits (Go `os.Exit(300)` gives 44).
pub fn status(n: i64) -> i32 {
    (n & 0xff) as i32
}

fn exec_on(src: &str, p: Prelude, opts: Opts) -> Outcome {
    let mut o = Outcome::default();
    let Setup {
        mut rt,
        main,
        out,
        err,
    } = match setup(src, &p, opts) {
        Ok(s) => s,
        Err((text, code)) => {
            o.err = text;
            o.code = code;
            o.exit = diag::EXIT_COMPILE;
            return o;
        }
    };
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
        o.exit = status(n);
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
        o.exit = status(n); // an exit requested inside a handler
    }
    drop(rt);
    o.out = out.text();
    o.err = err.text();
    o
}

/// The prelude the tests run with: the real core natives, plus stand-ins
/// for the `extras/prelude.fung` functions the VM tests use (Go's tests
/// boot the fung prelude; Task 9 does that here).
pub fn prelude() -> Prelude {
    let mut p = Prelude::default();
    for (name, v) in crate::prelude::core() {
        p.0.insert(name.to_string(), v);
    }
    // extras/prelude.fung `is_empty(x) = len(x) == 0`.
    p.0.insert(
        "is_empty".to_string(),
        nat::def(
            "is_empty",
            |_, a| Ok(Value::Bool(crate::value::index::len(&a[0])? == 0)),
            vec![nat::param("x")],
        ),
    );
    p
}

/// Go `native_test.go` `padNative`: a default and a rest parameter.
pub fn pad() -> Vec<Value> {
    vec![Value::Native(Native::leak(
        "pad",
        vec![
            nat::param("s"),
            nat::opt("n", Value::Int(2)),
            nat::rest("tail"),
        ],
        |_, a| {
            let (Value::Str(s), Value::Int(n)) = (&a[0], &a[1]) else {
                return Err(nat::fail("E0801", vec![nat::s("pad: bad arguments")]));
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
