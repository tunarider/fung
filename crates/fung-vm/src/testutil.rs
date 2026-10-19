//! Test helpers: compile a program with the front end, boot the prelude
//! and run it the way the fung command will: the entry, waiting for the
//! tasks it left (or cancelling them after an error, unwinding through the releases),
//! then the exit code.
//! Everything runs on a 4 GiB thread, because value formatting and
//! comparison recurse on deep nesting.
//!
//! The prelude is the booted one (`crate::boot::boot`: the core natives,
//! the concurrency natives and `extras/prelude.fung`), plus test-only
//! natives passed in `Opts`.

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
}

/// How to run: standard input and extra natives (made on the run thread,
/// since values cannot cross threads).
#[derive(Clone, Copy)]
pub struct Opts {
    pub stdin: &'static str,
    pub extra: fn() -> Vec<Value>,
    /// Changes the compiled entry before it is linked.
    pub patch: fn(&mut Proto),
    /// The entry's file name (`-e` by default, as for `fung -e`).
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
    // Links the entry and registers the modules, as the
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
/// (absolute paths).
pub fn run_files(files: &[(&str, &str)], entry: &str) -> Outcome {
    run_files_with(files, entry, Vec::new)
}

/// `run_files` with extra natives (`Opts::extra`).
pub fn run_files_with(files: &[(&str, &str)], entry: &str, extra: fn() -> Vec<Value>) -> Outcome {
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
            extra,
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
/// entry's display name, as `fung run` shows that argument; tests pass
/// the absolute temp path, which is also what `fung run` prints for it
/// when run from the repository root (imports outside the working
/// directory show as absolute paths).
pub fn run_file(path: &std::path::Path) -> (String, String, i32) {
    let path = path.to_string_lossy().into_owned();
    let o = on_big_stack(move || {
        let src = std::fs::read(&path)
            .map(|b| fung_front::source::decode_lossy(&b))
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
/// keeps the low 8 bits (`exit(300)` gives 44).
pub fn status(n: i64) -> i32 {
    (n & 0xff) as i32
}

fn exec_on(src: &str, given: Option<Prelude>, opts: Opts) -> Outcome {
    finish(setup_with(src, given, opts, None))
}

/// Runs a set-up program the way the command does.
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
    // The entry is done, the tasks it left keep the program
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
            // reported even after an exit, as the command does
            if d.code != "E0701" && d.code != "E0702" {
                let text = rt.with_sources(|s| diag::format(&d, s));
                err.clone().write_all(text.as_bytes()).unwrap();
                o.exit = diag::EXIT_RUNTIME;
            }
        }
    }
    let reported = o.exit == diag::EXIT_RUNTIME;
    let before = rt.exit_code();
    if let Some(n) = before {
        o.exit = status(n);
    }
    if reported && o.exit == diag::EXIT_OK {
        o.exit = diag::EXIT_RUNTIME;
    }
    match (before, rt.exit_code()) {
        (None, Some(n)) => o.exit = status(n), // an exit requested while unwinding
        (None, None) if o.exit == diag::EXIT_OK && rt.failed() => o.exit = diag::EXIT_RUNTIME,
        _ => {}
    }
    drop(rt);
    o.out = out.text();
    o.err = err.text();
    o
}

/// A test native `pad`: a default and a rest parameter.
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
/// - `hold(ms)`: sleeps the run thread without parking, so no
///   other task runs meanwhile;
/// - `buffered(ch)`: how many values wait in `ch`'s buffer;
/// - `finished(t)`: whether task `t` has finished;
/// - `loop_depth()`: how many run loops run on this task (`cont.rs`);
/// - `parks()`: how many times this task parked (`cont.rs`);
/// - `offloads()`: how many waits on the operating system it handed off (`nat::offload`);
/// - `yields()`: how many times it gave its turn at a yield point.
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
                let c = a[0].channel_end().expect("a channel end");
                Ok(Value::Int(c.queued() as i64))
            },
        )),
        Value::Native(Native::leak(
            "buffered",
            vec![nat::param("ch")],
            |_: &mut Task, a| {
                let c = a[0].channel_end().expect("a channel end");
                Ok(Value::Int(c.buffered() as i64))
            },
        )),
        Value::Native(Native::leak(
            "finished",
            vec![nat::param("t")],
            |_: &mut Task, a| {
                let Value::Task(h) = &a[0] else {
                    panic!("a task argument")
                };
                Ok(Value::Bool(h.state.result().is_some()))
            },
        )),
        Value::Native(Native::leak("yields", vec![], |t: &mut Task, _| {
            Ok(Value::Int(i64::from(t.yields)))
        })),
        Value::Native(Native::leak("offloads", vec![], |t: &mut Task, _| {
            Ok(Value::Int(i64::from(t.offloads)))
        })),
        Value::Native(Native::leak("parks", vec![], |t: &mut Task, _| {
            Ok(Value::Int(i64::from(t.parks)))
        })),
        Value::Native(Native::leak("loop_depth", vec![], |t: &mut Task, _| {
            Ok(Value::Int(i64::from(t.loops)))
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

/// Latches and counters by name, shared with offload jobs on other
/// threads (`job_natives`). Each test uses names of its own.
pub mod jobs {
    use std::collections::HashMap;
    use std::sync::{Condvar, Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use crate::native::nat;
    use crate::value::{Native, Value};
    use crate::vm::{Flow, Stop, Task};

    #[derive(Default)]
    struct State {
        open: HashMap<String, bool>,
        counts: HashMap<String, usize>,
    }

    fn state() -> &'static (Mutex<State>, Condvar) {
        static S: OnceLock<(Mutex<State>, Condvar)> = OnceLock::new();
        S.get_or_init(Default::default)
    }

    /// Lets the jobs waiting on latch `name` go.
    pub fn open(name: &str) {
        let (m, c) = state();
        m.lock().unwrap().open.insert(name.to_string(), true);
        c.notify_all();
    }

    /// Waits until latch `name` is open.
    pub fn wait(name: &str) {
        let (m, c) = state();
        let mut s = m.lock().unwrap();
        while !s.open.get(name).copied().unwrap_or(false) {
            s = c.wait(s).unwrap();
        }
    }

    /// How many times `count_job(name)` ran.
    pub fn count(name: &str) -> usize {
        state()
            .0
            .lock()
            .unwrap()
            .counts
            .get(name)
            .copied()
            .unwrap_or(0)
    }

    /// The write end of the pipe `pipe_pair(name)` made.
    pub fn pipe_write_fd(name: &str) -> i32 {
        state()
            .0
            .lock()
            .unwrap()
            .counts
            .get(&format!("pipe:{name}"))
            .copied()
            .unwrap_or(0) as i32
    }

    /// Writes `s` to the pipe `pipe_pair(name)` made.
    pub fn pipe_write(name: &str, s: &str) {
        let fd = pipe_write_fd(name);
        // SAFETY: `fd` is a pipe's write end `pipe_pair` made and never
        // closes; `s` is a valid buffer of its length.
        let n = unsafe { libc::write(fd, s.as_ptr().cast(), s.len()) };
        assert_eq!(n, s.len() as isize);
    }

    /// Bumps counter `name` (a test thread marks what it saw).
    pub fn mark(name: &str) {
        bump(name);
    }

    fn bump(name: &str) {
        *state()
            .0
            .lock()
            .unwrap()
            .counts
            .entry(name.to_string())
            .or_default() += 1;
    }

    /// Waits (at most 5 s) until `cond` holds; whether it did.
    pub fn until(cond: impl Fn() -> bool) -> bool {
        let end = Instant::now() + Duration::from_secs(5);
        while !cond() {
            if Instant::now() >= end {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        true
    }

    fn name(a: &[Value]) -> String {
        let Value::Str(s) = &a[0] else {
            panic!("a name")
        };
        s.to_string()
    }

    fn thread_name() -> String {
        std::thread::current().name().unwrap_or("").to_string()
    }

    /// - `block_job(name)`: a job (one that is waited for) that waits on
    ///   latch `name`;
    /// - `abandoning_block_job(name)`: the same as a job a stop abandons;
    /// - `count_job(name)`: a job that bumps counter `name`;
    /// - `pool_thread()`: the name of the thread a pool job runs on;
    /// - `panic_job()`: a job that panics;
    /// - `open_latch(name)`: `open(name)` from fung;
    /// - `settle_mark(name)`: `mark(name)`, and the scheduler's parked
    ///   count is reset so a test thread sees the next one;
    /// - `pool_state()`: the pool's `[threads, idle, queued]`.
    pub fn job_natives() -> Vec<Value> {
        fn block_job(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
            let n = name(&a);
            nat::offload(t, move || wait(&n), |_, ()| Ok(Value::str("done")))
        }
        fn abandoning_block_job(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
            let n = name(&a);
            nat::offload_abandoning(t, move || wait(&n), |_, ()| Ok(Value::str("done")))
        }
        fn count_job(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
            let n = name(&a);
            nat::offload(t, move || bump(&n), |_, ()| Ok(Value::Nil))
        }
        fn pool_thread(t: &mut Task<'_>, _: Vec<Value>) -> Result<Flow, Stop> {
            nat::offload(t, thread_name, |_, s| Ok(Value::str(&s)))
        }
        fn panic_job(t: &mut Task<'_>, _: Vec<Value>) -> Result<Flow, Stop> {
            nat::offload(t, || panic!("in a job"), |_, ()| Ok(Value::Nil))
        }
        let p = || vec![nat::param("name")];
        let mut v = super::probes();
        for (n, params, f) in [
            ("block_job", p(), block_job as crate::vm::StepFn),
            ("abandoning_block_job", p(), abandoning_block_job),
            ("count_job", p(), count_job),
            ("pool_thread", vec![], pool_thread),
            ("panic_job", vec![], panic_job),
        ] {
            v.push(Value::Native(Native::leak_step(n, params, f)));
        }
        v.push(Value::Native(Native::leak(
            "open_latch",
            vec![nat::param("name")],
            |_: &mut Task, a| {
                open(&name(&a));
                Ok(Value::Nil)
            },
        )));
        v.push(Value::Native(Native::leak(
            "settle_mark",
            vec![nat::param("name")],
            |t: &mut Task, a| {
                // The next parked count the scheduler stores is one from
                // after this call.
                t.shared_for_tests()
                    .parked_probe
                    .store(0, std::sync::atomic::Ordering::SeqCst);
                mark(&name(&a));
                Ok(Value::Nil)
            },
        )));
        fn pipe_read(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
            let Value::Int(fd) = a[0] else {
                panic!("a descriptor")
            };
            let fd = fd as i32;
            crate::vm::park_io(
                t,
                move |_, _| {
                    let mut buf = [0u8; 64];
                    // SAFETY: `fd` is a non-blocking pipe's read end; `buf` is
                    // a valid buffer of its length.
                    let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
                    if n < 0 {
                        assert_eq!(
                            std::io::Error::last_os_error().kind(),
                            std::io::ErrorKind::WouldBlock
                        );
                        return crate::vm::IoStep::Wait(
                            fd,
                            crate::vm::poller::Interest::Read,
                            None,
                        );
                    }
                    crate::vm::IoStep::Done(Ok(Value::str(&String::from_utf8_lossy(
                        &buf[..n as usize],
                    ))))
                },
                |_| {},
            )
        }
        v.push(Value::Native(Native::leak_step(
            "pipe_read",
            vec![nat::param("fd")],
            pipe_read,
        )));
        v.push(Value::Native(Native::leak(
            "pipe_pair",
            vec![nat::param("name")],
            |_: &mut Task, a| {
                let mut fds = [0; 2];
                // SAFETY: `fds` holds the two descriptors `pipe` makes;
                // the read end is made non-blocking. The test never
                // closes them.
                unsafe {
                    assert_eq!(libc::pipe(fds.as_mut_ptr()), 0);
                    libc::fcntl(fds[0], libc::F_SETFL, libc::O_NONBLOCK);
                }
                state()
                    .0
                    .lock()
                    .unwrap()
                    .counts
                    .insert(format!("pipe:{}", name(&a)), fds[1] as usize);
                Ok(Value::Int(i64::from(fds[0])))
            },
        )));
        v.push(Value::Native(Native::leak(
            "pipe_put",
            vec![nat::param("name"), nat::param("s")],
            |_: &mut Task, a| {
                let Value::Str(s) = &a[1] else {
                    panic!("a string")
                };
                pipe_write(&name(&a), s);
                Ok(Value::Nil)
            },
        )));
        v.push(Value::Native(Native::leak(
            "fail_register_at",
            vec![nat::param("n")],
            |t: &mut Task, a| {
                let Value::Int(n) = a[0] else {
                    panic!("an int")
                };
                t.shared_for_tests().poller.fail_register_at(n as usize);
                Ok(Value::Nil)
            },
        )));
        v.push(Value::Native(Native::leak(
            "waiting_io",
            vec![],
            |t: &mut Task, _| Ok(Value::Int(t.io_waiting_for_tests() as i64)),
        )));
        v.push(Value::Native(Native::leak(
            "pool_state",
            vec![],
            |t: &mut Task, _| {
                let p = &t.shared_for_tests().pool;
                let n = |x: usize| Value::Int(x as i64);
                Ok(nat::list(vec![n(p.threads()), n(p.idle()), n(p.queued())]))
            },
        )));
        v
    }
}
