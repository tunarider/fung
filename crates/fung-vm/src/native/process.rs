//! `_native.process`: `exec`, `sh` and `env` (06: 명령 실행).
//! `extras/prelude.fung` re-exports them under their plain names.
//!
//! A command runs with its standard output and error captured and its
//! standard input `/dev/null` unless `stdin` is given. Before it starts,
//! `prepare` checks what would keep it from starting and gives the reason
//! of the `not_found` error.

use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use fung_front::diag::Arg;
use fung_front::loader::abs_path;
use fung_front::source::decode_lossy;

use super::nat::{self, native, opt, param};
use super::oserr::reason;
use crate::value::{Lit, MapBuilder, Module, Native, Value, type_name};
use crate::vm::{Stop, Task};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The `_native.process` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.get_or_init(|| {
        let run = || {
            vec![
                param("cmd"),
                opt("cwd", Lit::Nil),
                opt("env", Lit::Nil),
                opt("stdin", Lit::Nil),
            ]
        };
        vec![
            ("exec", native("exec", exec_fn, run())),
            ("sh", native("sh", sh_fn, run())),
            ("env", native("env", env_fn, vec![opt("name", Lit::Nil)])),
        ]
    });
    let mut m = Module::new("process", "_native.process");
    for (name, n) in natives {
        m.set(name, Value::Native(n));
    }
    Rc::new(m)
}

/// `exec(cmd, cwd, env, stdin)`: runs a program with arguments, no shell.
fn exec_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let argv = nat::str_list("cmd", &a[0])?;
    if argv.is_empty() {
        return Err(nat::fail(
            "E0403",
            vec![
                nat::s("cmd"),
                nat::s("a non-empty list of strings"),
                nat::s("an empty list"),
            ],
        ));
    }
    run_command(t, argv, &a)
}

/// `sh(cmd, cwd, env, stdin)`: `/bin/sh -c cmd`.
fn sh_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let cmd = nat::need_str("cmd", &a[0])?;
    run_command(t, vec!["/bin/sh".into(), "-c".into(), cmd.to_string()], &a)
}

/// `env(name = nil)`: one variable (nil when unset), or all of them as a
/// map in the environment's order.
fn env_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    if let Some(name) = nat::opt_str("name", &a[0])? {
        // No entry can have such a name.
        if name.is_empty() || name.contains(['=', '\0']) {
            return Ok(Value::Nil);
        }
        return Ok(match std::env::var_os(name) {
            Some(v) => Value::str(&decode_lossy(v.as_bytes())),
            None => Value::Nil,
        });
    }
    let mut m = MapBuilder::default();
    for (k, v) in std::env::vars_os() {
        m.set(
            Rc::new(decode_lossy(k.as_bytes())),
            Value::str(&decode_lossy(v.as_bytes())),
        );
    }
    Ok(Value::Map(Rc::new(m.finish())))
}

/// Checks `cwd`, `env` and `stdin` (in that order), runs
/// the command, and returns `Ok({out, err, code})`, `Err({kind: 'exit,
/// code, out, err, message})` for a non-zero exit (code -1 when a signal
/// ended it), or `Err({kind: 'not_found, message})` when it cannot start.
///
/// The command runs outside the run token (stage 3a): other tasks run
/// while it does. It runs in a process group of its own; when the task is
/// cancelled (or its deadline passes, or the program exits) the group
/// gets SIGTERM, and SIGKILL if it is still there 2 s later (L6), and the
/// call unwinds with the cancellation instead of giving the output.
fn run_command(t: &mut Task<'_>, argv: Vec<String>, a: &[Value]) -> Result<Value, Stop> {
    let cwd = nat::opt_str("cwd", &a[1])?
        .filter(|d| !d.is_empty())
        .map(str::to_string);
    let env = match &a[2] {
        Value::Nil => None,
        Value::Map(m) => {
            let mut vars = Vec::new();
            for (k, v) in m.iter() {
                let Value::Str(s) = v else {
                    return Err(nat::fail(
                        "E0403",
                        vec![
                            nat::s("env"),
                            nat::s("a map of strings"),
                            Arg::Str(format!("a map holding {}", type_name(v))),
                        ],
                    ));
                };
                vars.push((k.to_string(), s.to_string()));
            }
            Some(vars)
        }
        v => return Err(nat::wrong_arg("env", "map | nil", v)),
    };
    let stdin = nat::opt_str("stdin", &a[3])?.map(|s| s.as_bytes().to_vec());
    let name = argv[0].clone();
    let not_found = |reason: &str| {
        Ok(nat::err_value(
            "not_found",
            &format!("cannot run `{name}`: {reason}"),
            Vec::new(),
        ))
    };
    let node = t.cancel_node();
    let stopped =
        move || node.is_cancelled() || node.deadline_at().is_some_and(|at| Instant::now() >= at);
    let output = crate::vm::outside(t, move || {
        run_process(&argv, cwd.as_deref(), env, stdin, stopped)
    });
    // A cancellation (or a deadline) that came meanwhile wins over the
    // output, whether or not it cut the command short.
    t.check_cancelled()?;
    let output = match output {
        Ok(o) => o,
        Err(reason) => return not_found(&reason),
    };
    let out = Value::str(&decode_lossy(&output.stdout));
    let err = Value::str(&decode_lossy(&output.stderr));
    if output.status.success() {
        let mut m = MapBuilder::default();
        m.set(Rc::new("out".into()), out);
        m.set(Rc::new("err".into()), err);
        m.set(Rc::new("code".into()), Value::Int(0));
        return Ok(nat::ok(Value::Map(Rc::new(m.finish()))));
    }
    let code = output.status.code().map_or(-1, i64::from);
    Ok(nat::err_value(
        "exit",
        &format!("`{name}` exited with code {code}"),
        vec![("code", Value::Int(code)), ("out", out), ("err", err)],
    ))
}

/// How long a command has to end after SIGTERM before SIGKILL.
const TERM_GRACE: Duration = Duration::from_secs(2);

/// Set by tests: every command gets a group of its own, whether or not
/// `cargo test` runs in the foreground of a terminal.
#[cfg(test)]
static OWN_GROUP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
fn force_own_group() {
    OWN_GROUP.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Whether a command gets a process group of its own (L6, user decision
/// 2026-10-18). Not when fung runs in the foreground of its terminal: a
/// command in another group would be stopped (SIGTTIN, SIGTTOU) the
/// moment it reads or sets up the terminal (a `sudo` or `ssh` password,
/// a credential prompt), and the terminal's Ctrl-C reaches it as in a
/// shell. Without a terminal (a service, a container, CI, cron) it does,
/// so a cancel ends what it started too.
fn own_group() -> bool {
    #[cfg(test)]
    if OWN_GROUP.load(std::sync::atomic::Ordering::SeqCst) {
        return true;
    }
    !in_terminal_foreground()
}

/// Whether this process has a controlling terminal and is in its
/// foreground process group.
fn in_terminal_foreground() -> bool {
    // SAFETY: `open` reads the NUL-terminated path; the descriptor it
    // returns is ours and closed below; `tcgetpgrp` and `getpgrp` only
    // read process state. O_NOCTTY keeps a terminal from becoming ours.
    unsafe {
        let fd = libc::open(
            c"/dev/tty".as_ptr(),
            libc::O_RDONLY | libc::O_NOCTTY | libc::O_CLOEXEC,
        );
        if fd < 0 {
            return false;
        }
        let fg = libc::tcgetpgrp(fd);
        libc::close(fd);
        fg >= 0 && fg == libc::getpgrp()
    }
}

/// Starts the command and waits for it, with its output captured, or
/// gives the reason it cannot start. Handles no fung value: it runs
/// without the run token. While it waits it asks `stopped` (the task's
/// cancellation) every 10 ms at most, and terminates the command's group
/// once it says so (`terminate`).
fn run_process(
    argv: &[String],
    cwd: Option<&str>,
    env: Option<Vec<(String, String)>>,
    stdin: Option<Vec<u8>>,
    stopped: impl Fn() -> bool,
) -> Result<std::process::Output, String> {
    let own = own_group();
    let mut cmd = prepare(argv, cwd, env, own)?;
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = cmd.spawn().map_err(|e| reason(&e))?;
    // A thread feeds stdin, and a command that stops reading early
    // (EPIPE) is not an error.
    let feeder = match (stdin, child.stdin.take()) {
        (Some(text), Some(mut pipe)) => Some(std::thread::spawn(move || {
            let _ = pipe.write_all(&text);
        })),
        _ => None,
    };
    let out = read_all(child.stdout.take());
    let err = read_all(child.stderr.take());
    let group = child.id() as libc::pid_t;
    let mut pause = Duration::from_millis(1);
    let mut stop = || {
        let s = stopped();
        if !s {
            std::thread::sleep(pause);
            pause = (pause * 2).min(Duration::from_millis(10));
        }
        s
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Ok(st),
            Ok(None) if stop() => break terminate(&mut child, own),
            Ok(None) => {}
            Err(e) => break Err(e),
        }
    };
    // The command has ended, but what it left in the background may
    // still hold its output open (`sleep 30 &`): wait for the pipes to
    // close, and on a cancel end the group too and leave the readers.
    let mut cut = false;
    while !(out.is_finished() && err.is_finished()) {
        if stop() {
            if own {
                end_group(group, || out.is_finished() && err.is_finished());
            }
            cut = true;
            break;
        }
    }
    if let Some(f) = feeder {
        let _ = f.join();
    }
    let output = status.map(|status| std::process::Output {
        status,
        stdout: if cut {
            Vec::new()
        } else {
            out.join().unwrap_or_default()
        },
        stderr: if cut {
            Vec::new()
        } else {
            err.join().unwrap_or_default()
        },
    });
    output.map_err(|e| reason(&e))
}

/// SIGTERM to a group whose leader is gone, then SIGKILL after
/// `TERM_GRACE` unless `done` says its members have let go.
fn end_group(group: libc::pid_t, done: impl Fn() -> bool) {
    // SAFETY: `killpg` only sends a signal. The group id outlives its
    // leader while members remain; with none left it is ESRCH.
    unsafe {
        libc::killpg(group, libc::SIGTERM);
    }
    let start = Instant::now();
    while start.elapsed() < TERM_GRACE {
        if done() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // SAFETY: as above.
    unsafe {
        libc::killpg(group, libc::SIGKILL);
    }
}

/// Reads a child's pipe to its end on a thread of its own.
fn read_all(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut b = Vec::new();
        if let Some(mut p) = pipe {
            let _ = p.read_to_end(&mut b);
        }
        b
    })
}

/// SIGTERM to the child's process group (`own`) or to the child alone,
/// then SIGKILL if the child is still there after `TERM_GRACE`; its
/// status.
fn terminate(
    child: &mut std::process::Child,
    own: bool,
) -> std::io::Result<std::process::ExitStatus> {
    let id = child.id() as libc::pid_t;
    let send = |sig| {
        // SAFETY: `killpg`/`kill` only send a signal; the child (and the
        // group it leads, when `own`) is not reaped yet, so the id names
        // no other process or group.
        unsafe {
            if own {
                libc::killpg(id, sig);
            } else {
                libc::kill(id, sig);
            }
        }
    };
    send(libc::SIGTERM);
    let start = Instant::now();
    while start.elapsed() < TERM_GRACE {
        if let Some(st) = child.try_wait()? {
            return Ok(st);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    send(libc::SIGKILL);
    child.wait()
}

/// The command to run, or the reason it cannot start, checked in
/// argument order (`exec(cmd, cwd, env, stdin)`): the command (an empty
/// name, a NUL in any argument, the `$PATH` lookup), the working
/// directory, the environment (with `PWD` set to the absolute `cwd` when
/// no `env` is given), and last whether the kernel can execute the file.
fn prepare(
    argv: &[String],
    cwd: Option<&str>,
    env: Option<Vec<(String, String)>>,
    own: bool,
) -> Result<Command, String> {
    let name = &argv[0];
    if name.is_empty() {
        return Err("empty command name".into());
    }
    if argv.iter().any(|x| x.contains('\0')) {
        return Err("argument contains a NUL byte".into());
    }
    let path = if name.contains('/') {
        name.clone()
    } else {
        look_path(name)?
    };
    if let Some(dir) = cwd {
        if dir.contains('\0') {
            return Err("working directory contains a NUL byte".into());
        }
        if let Err(e) = std::fs::metadata(dir) {
            return Err(reason(&e));
        }
    }
    if let Some(vars) = &env
        && vars
            .iter()
            .any(|(k, v)| k.contains('\0') || v.contains('\0'))
    {
        return Err("environment variable contains a NUL byte".into());
    }
    let vars = match (env, cwd) {
        (Some(vars), _) => vars,
        (None, Some(dir)) => vec![("PWD".to_string(), abs_path(dir))],
        (None, None) => Vec::new(),
    };
    // A file the kernel cannot execute (ENOEXEC) is refused rather than
    // handed to `/bin/sh`, which the standard library's `execvp` would do:
    // shell scripts go through `sh()`.
    let target = match cwd {
        Some(dir) if !path.starts_with('/') => format!("{dir}/{path}"),
        _ => path.clone(),
    };
    if !kernel_can_run(&target) {
        return Err("not an executable file (run scripts with sh)".into());
    }
    let mut cmd = Command::new(&path);
    cmd.arg0(name).args(&argv[1..]);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.envs(vars);
    // A group of its own (`own_group`): a cancellation terminates what
    // the command started too (`terminate`).
    if own {
        cmd.process_group(0);
    }
    // fung blocks SIGINT and SIGTERM in all its threads (`signals.rs`);
    // the command starts with none blocked.
    // SAFETY: the hook runs in the forked child before `exec` and only
    // makes async-signal-safe calls (`clear_mask_in_child`); it touches
    // no memory of the parent's other threads.
    unsafe {
        cmd.pre_exec(crate::signals::clear_mask_in_child);
    }
    Ok(cmd)
}

/// The first `$PATH` entry (an empty one is the working directory)
/// holding an executable file named `name`. A command found only through
/// a relative entry is refused: which program runs would then depend on
/// the working directory.
fn look_path(name: &str) -> Result<String, String> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    if !path.is_empty() {
        for dir in path.as_bytes().split(|b| *b == b':') {
            let dir = match decode_lossy(dir) {
                d if d.is_empty() => ".".to_string(),
                d => d,
            };
            let candidate = format!("{dir}/{name}");
            if is_executable(&candidate) {
                if !candidate.starts_with('/') {
                    return Err("command found only in a relative PATH entry".into());
                }
                return Ok(candidate);
            }
        }
    }
    Err("command not found in PATH".into())
}

/// Whether `execve` would accept an executable file rather than fail with
/// ENOEXEC: it starts with `#!` or a binary format's magic number (ELF,
/// Mach-O, universal Mach-O). Anything that is not an executable regular
/// file we can read is left to the start itself, which gives the OS error
/// (EACCES for a directory or a file without an execute bit, ...).
fn kernel_can_run(path: &str) -> bool {
    use std::io::Read;
    if !is_executable(path) {
        return true;
    }
    let Ok(f) = std::fs::File::open(path) else {
        return true;
    };
    let mut head = Vec::with_capacity(4);
    if f.take(4).read_to_end(&mut head).is_err() {
        return true;
    }
    const MAGIC: &[&[u8]] = &[
        b"#!",
        b"\x7fELF",
        b"\xfe\xed\xfa\xce",
        b"\xfe\xed\xfa\xcf",
        b"\xce\xfa\xed\xfe",
        b"\xcf\xfa\xed\xfe",
        b"\xca\xfe\xba\xbe",
        b"\xca\xfe\xba\xbf",
    ];
    MAGIC.iter().any(|m| head.starts_with(m))
}

/// A file (not a directory) with an execute bit set.
fn is_executable(path: &str) -> bool {
    std::fs::metadata(path).is_ok_and(|m| !m.is_dir() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
