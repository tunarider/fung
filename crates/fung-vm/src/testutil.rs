//! Test helpers: compile a program with the front end, boot the prelude
//! and run it the way the fung command will (Go `cmd/fung/pipeline.go`
//! `execute`): the entry, waiting for the tasks it left (or cancelling
//! them after an error), the exit handlers, the end of the background
//! tasks (stage 3b), the report of task failures nobody retrieved, then
//! the exit code.
//! Everything runs on a 4 GiB thread, because value formatting and
//! comparison recurse on deep nesting like Go.
//!
//! The prelude is the booted one (`crate::boot::boot`: the core natives,
//! the concurrency natives and `extras/prelude.fung`), like Go's VM
//! tests (`bootTestPrelude`), plus test-only natives passed in `Opts`.

use std::cell::RefCell;
use std::io::{BufRead, Cursor, Write};

use fung_front::bytecode::Proto;
use fung_front::diag::{self, CallSite, Diag};
use fung_front::loader::{Loader, Stage};

use crate::native::nat;
use crate::value::{Lit, Native, Value, format};
use crate::vm::{Prelude, Runtime, Stop, Task};

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
    /// The script arguments (the prelude's `args`).
    pub args: &'static [&'static str],
    /// Compiles the entry as an embedded extras file would be, so it may
    /// import `_native.*` (test-only: user code gets E0310). Its protos
    /// are then unmarked builtin, so errors report where they arise, as
    /// in user code; `patch` may mark some again.
    pub internal: bool,
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
            args: &[],
            internal: false,
        }
    }
}

/// Runs a program as `fung -e` would, with the booted prelude: (stdout,
/// stderr, exit code).
pub fn run(src: &str) -> (String, String, i32) {
    let o = exec(src, Opts::default());
    (o.out, o.err, o.exit)
}

/// `run` under the name the boot tests use: the prelude is always booted.
pub fn run_booted(src: &str) -> (String, String, i32) {
    run(src)
}

/// Runs a program with the booted prelude plus `opts.extra`.
pub fn exec(src: &str, opts: Opts) -> Outcome {
    let src = src.to_string();
    on_big_stack(move || exec_on(&src, None, opts))
}

/// `exec` with `stdin` as standard input instead of `opts.stdin`.
pub fn exec_with_stdin(src: &str, opts: Opts, stdin: Box<dyn BufRead + Send>) -> Outcome {
    let src = src.to_string();
    on_big_stack(move || {
        let s = setup_with(&src, None, opts, Some(stdin));
        finish(s)
    })
}

/// `exec`, with `prepare` called on the loaded runtime before the entry
/// runs (a test registers a module of its own over a standard one).
pub fn exec_prepared(src: &str, opts: Opts, prepare: fn(&Runtime)) -> Outcome {
    let src = src.to_string();
    on_big_stack(move || {
        let s = setup_with(&src, None, opts, None);
        if let Ok(s) = &s {
            prepare(&s.rt);
        }
        finish(s)
    })
}

/// Runs a program with a given prelude (no boot) on the current thread.
pub fn exec_with_prelude(src: &str, p: Prelude) -> Outcome {
    exec_on(src, Some(p), Opts::default())
}

/// A shared byte buffer the runtime writes into.
#[derive(Clone, Default)]
pub struct Buf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Buf {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

/// A compiled, linked entry with a runtime writing into two buffers.
pub struct Setup {
    /// The runtime with the entry loaded: `rt.run()` runs it.
    pub rt: Runtime,
    pub out: Buf,
    pub err: Buf,
}

/// Compiles `src`, makes a runtime for it and boots the prelude on it
/// (plus `opts.extra`), or returns the compile errors formatted as the
/// fung command prints them.
pub fn setup(src: &str, opts: Opts) -> Result<Setup, (String, String)> {
    setup_with(src, None, opts, None)
}

fn setup_with(
    src: &str,
    given: Option<Prelude>,
    opts: Opts,
    stdin: Option<Box<dyn BufRead + Send>>,
) -> Result<Setup, (String, String)> {
    let extra = (opts.extra)();
    let mut names: Vec<String> = extra
        .iter()
        .filter_map(|v| match v {
            Value::Native(n) => Some(n.name.to_string()),
            _ => None,
        })
        .collect();
    if let Some(p) = &given {
        names.extend(p.0.keys().cloned());
    }
    let loader = Loader {
        extra_prelude: names,
        tests: opts.tests,
        read_file: opts.read,
        internal: opts.internal,
        ..Loader::default()
    };
    let (b, errs) = loader.compile(opts.file, src, Stage::All);
    if !errs.is_empty() {
        let text = errs.iter().map(|e| diag::format(e, &b.sources)).collect();
        return Err((text, errs[0].code.clone()));
    }
    let mut entry = b.proto.expect("a compiled entry");
    if opts.internal {
        unmark_builtin(&mut entry);
    }
    (opts.patch)(&mut entry);
    let (out, err) = (Buf::default(), Buf::default());
    let rt = Runtime::new(
        Box::new(out.clone()),
        Box::new(err.clone()),
        stdin.unwrap_or_else(|| Box::new(Cursor::new(opts.stdin.as_bytes().to_vec()))),
        b.sources,
    );
    // Booted after the program is compiled: the runtime holds the
    // program's sources, and boot appends the prelude's after them.
    match given {
        Some(p) => rt.set_prelude(p),
        None => {
            let args = opts.args.iter().map(|a| a.to_string()).collect();
            rt.boot(args).unwrap_or_else(|errs| {
                let text: String =
                    rt.with_sources(|s| errs.iter().map(|e| diag::format(e, s)).collect());
                panic!("boot failed:\n{text}")
            })
        }
    }
    // The extra natives are `&'static` (no `Rc`), made on this thread.
    rt.with_prelude(|p| {
        for v in extra {
            if let Value::Native(n) = &v {
                p.0.insert(n.name.to_string(), v.clone());
            }
        }
    });
    // Links the entry and registers the modules (Go `register`), as the
    // command does. A caller may register other natives over them.
    rt.load(&entry, &b.modules);
    Ok(Setup { rt, out, err })
}

/// Clears the builtin mark an internal compile put on every proto.
fn unmark_builtin(p: &mut Proto) {
    p.builtin = false;
    for c in &mut p.protos {
        unmark_builtin(c);
    }
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
        exec_on(&src, None, opts)
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
        exec_on(&src, None, opts)
    });
    (o.out, o.err, o.exit)
}

/// The process exit status a shell sees for exit(n): the operating system
/// keeps the low 8 bits (Go `os.Exit(300)` gives 44).
pub fn status(n: i64) -> i32 {
    (n & 0xff) as i32
}

fn exec_on(src: &str, given: Option<Prelude>, opts: Opts) -> Outcome {
    finish(setup_with(src, given, opts, None))
}

/// Runs a set-up program the way the command does (Go `execute`).
fn finish(setup: Result<Setup, (String, String)>) -> Outcome {
    let mut o = Outcome::default();
    let Setup { rt, out, err } = match setup {
        Ok(s) => s,
        Err((text, code)) => {
            o.err = text;
            o.code = code;
            o.exit = diag::EXIT_COMPILE;
            return o;
        }
    };
    let r = rt.run_with(format);
    // Go `execute`: the entry is done, the tasks it left keep the program
    // alive; an entry error (or an exit) cancels them first.
    if r.is_ok() {
        rt.wait();
    } else {
        rt.cancel_all();
        rt.wait();
    }
    match r {
        Ok(v) => o.value = Some(v),
        Err(Stop::Exit(_)) => o.code = "exit".into(),
        Err(Stop::Error(d)) => {
            o.code = d.code.clone();
            o.error = Some(ErrInfo::of(&d));
            if rt.exit_code().is_none() {
                let text = rt.with_sources(|s| diag::format(&d, s));
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
        if d.code == "E0701" || d.code == "E0702" {
            // A handler cut short by the deadlock report (or an exit),
            // already told (Go `execute`).
            continue;
        }
        let text = rt.with_sources(|s| diag::format(&d, s));
        err.clone().write_all(text.as_bytes()).unwrap();
        if o.exit == diag::EXIT_OK {
            o.exit = diag::EXIT_RUNTIME;
        }
        o.handler_errors.push(ErrInfo::of(&d));
    }
    // The handlers are done: end the background tasks (stage 3b), then
    // the program is over: report every task failure nobody retrieved.
    rt.end_background();
    rt.wait();
    rt.report_unretrieved();
    match (before, rt.exit_code()) {
        (None, Some(n)) => o.exit = status(n), // an exit requested inside a handler
        (None, None) if o.exit == diag::EXIT_OK && rt.failed() => o.exit = diag::EXIT_RUNTIME,
        _ => {}
    }
    drop(rt);
    o.out = out.text();
    o.err = err.text();
    o
}

/// Go `native_test.go` `padNative`: a default and a rest parameter.
pub fn pad() -> Vec<Value> {
    vec![Value::Native(Native::leak(
        "pad",
        vec![
            nat::param("s"),
            nat::opt("n", Lit::Int(2)),
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

/// Probes for ordering concurrent tests by state instead of by sleeping
/// (a fixed sleep is not enough under load, where a spawned thread can
/// take longer than the sleep to start):
/// - `queued(ch)`: how many blocked sends and receives wait in `ch`;
/// - `hold(ms)`: sleeps the thread without giving the run token up, so no
///   other task runs meanwhile;
/// - `background()`: how many background tasks have not finished.
pub fn probes() -> Vec<Value> {
    fn int_arg(a: &[Value]) -> i64 {
        let Value::Int(n) = a[0] else {
            panic!("an int argument")
        };
        n
    }
    vec![
        Value::Native(Native::leak(
            "queued",
            vec![nat::param("ch")],
            |_: &mut Task, a| {
                let Value::Channel(c) = &a[0] else {
                    panic!("a channel argument")
                };
                Ok(Value::Int(c.queued() as i64))
            },
        )),
        Value::Native(Native::leak("background", vec![], |t: &mut Task, _| {
            Ok(Value::Int(t.background_tasks() as i64))
        })),
        Value::Native(Native::leak(
            "hold",
            vec![nat::param("ms")],
            |_: &mut Task, a| {
                std::thread::sleep(std::time::Duration::from_millis(int_arg(&a) as u64));
                Ok(Value::Nil)
            },
        )),
    ]
}

/// fung helpers over `probes`, put before a program by `exec_waiting`:
/// `until(f)` sleeps 1 ms at a time (letting the other tasks run) until
/// `f()` is true; `blocked(ch, n)` waits until `n` tasks wait in `ch`.
pub const UNTIL: &str = "fn until(f) = f() then nil else fn(): sleep(1ms); until(f) ;;()\nfn blocked(ch, n) = until(() => queued(ch) >= n)\n";

/// Runs `src` after `UNTIL` with the probes.
pub fn exec_waiting(src: &str) -> Outcome {
    exec(
        &format!("{UNTIL}{src}"),
        Opts {
            extra: probes,
            ..Opts::default()
        },
    )
}

/// The expected-output file format of the CLI tests, shared: files under
/// `crates/fung-vm/tests/expected/`, `FUNG_EXPECTED=bless` rewrites them.
#[allow(dead_code)]
#[path = "../../fung-cli/tests/common/expected.rs"]
pub mod expected;

/// Standard output, standard error and exit status, as `expected` takes
/// them.
#[allow(dead_code)]
pub type Run = (Vec<u8>, Vec<u8>, String);

/// An outcome as an expected-output entry: standard output, standard
/// error and the exit code, with each `(from, to)` replacement applied to
/// the two outputs.
pub fn outcome_entry(label: &str, o: &Outcome, subs: &[(&str, &str)]) -> expected::Entry {
    expected::Entry::new(label)
        .field("stdout", expected::replace(o.out.as_bytes(), subs))
        .field("stderr", expected::replace(o.err.as_bytes(), subs))
        .field("exit", o.exit.to_string())
}
