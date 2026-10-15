//! `_native.process` (Go `internal/native/process.go`): `exec`, `sh` and
//! `env` (06: 명령 실행). `extras/prelude.fung` re-exports them under
//! their plain names.
//!
//! A command runs with its standard output and error captured and its
//! standard input empty unless `stdin` is given (Go `exec.Cmd` with nil
//! `Stdin` reads `/dev/null`). The checks Go's `exec.Cmd.Start` makes
//! before starting a process (the `$PATH` lookup, `exec: no command`, NUL
//! bytes, the working directory) are made here in the same order, so the
//! `not_found` messages read the same.

use std::cell::OnceCell;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::rc::Rc;

use fung_front::diag::Arg;
use fung_front::loader::abs_path;
use fung_front::source::decode_go;

use super::nat::{self, def, opt, param};
use crate::value::{MapBuilder, Module, Value, type_name};
use crate::vm::{Stop, Task};

thread_local! {
    /// Made once per thread: natives are leaked (`Native::leak`).
    static NATIVES: OnceCell<Vec<(&'static str, Value)>> = const { OnceCell::new() };
}

/// The `_native.process` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.with(|c| {
        c.get_or_init(|| {
            let run = || {
                vec![
                    param("cmd"),
                    opt("cwd", Value::Nil),
                    opt("env", Value::Nil),
                    opt("stdin", Value::Nil),
                ]
            };
            vec![
                ("exec", def("exec", exec_fn, run())),
                ("sh", def("sh", sh_fn, run())),
                ("env", def("env", env_fn, vec![opt("name", Value::Nil)])),
            ]
        })
        .clone()
    });
    let mut m = Module::new("process", "_native.process");
    for (name, v) in natives {
        m.set(name, v);
    }
    Rc::new(m)
}

/// `exec(cmd, cwd, env, stdin)`: runs a program with arguments, no shell.
fn exec_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
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
    run_command(argv, &a)
}

/// `sh(cmd, cwd, env, stdin)`: `/bin/sh -c cmd`.
fn sh_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let cmd = nat::need_str("cmd", &a[0])?;
    run_command(vec!["/bin/sh".into(), "-c".into(), cmd.to_string()], &a)
}

/// `env(name = nil)`: one variable (nil when unset), or all of them as a
/// map in the environment's order.
fn env_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    if let Some(name) = nat::opt_str("name", &a[0])? {
        // Go's LookupEnv finds nothing for a name no entry can have.
        if name.is_empty() || name.contains(['=', '\0']) {
            return Ok(Value::Nil);
        }
        return Ok(match std::env::var_os(name) {
            Some(v) => Value::str(&decode_go(v.as_bytes())),
            None => Value::Nil,
        });
    }
    let mut m = MapBuilder::default();
    for (k, v) in std::env::vars_os() {
        m.set(
            Rc::new(decode_go(k.as_bytes())),
            Value::str(&decode_go(v.as_bytes())),
        );
    }
    Ok(Value::Map(Rc::new(m.finish())))
}

/// Go `runCommand`: checks `cwd`, `env` and `stdin` (in that order), runs
/// the command, and returns `Ok({out, err, code})`, `Err({kind: 'exit,
/// code, out, err, message})` for a non-zero exit (code -1 when a signal
/// ended it), or `Err({kind: 'not_found, message})` when it cannot start.
fn run_command(argv: Vec<String>, a: &[Value]) -> Result<Value, Stop> {
    let cwd = nat::opt_str("cwd", &a[1])?.filter(|d| !d.is_empty());
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
    let stdin = nat::opt_str("stdin", &a[3])?;
    let not_found = |reason: &str| {
        Ok(nat::err_value(
            "not_found",
            &format!("cannot run `{}`: {reason}", argv[0]),
            Vec::new(),
        ))
    };
    let mut cmd = match prepare(&argv, cwd, env) {
        Ok(cmd) => cmd,
        Err(reason) => return not_found(&reason),
    };
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return not_found(&reason(&e)),
    };
    // Go copies stdin from a goroutine and ignores a reader that stops
    // early (EPIPE); a thread does the same here.
    let feeder = match (stdin, child.stdin.take()) {
        (Some(text), Some(mut pipe)) => {
            let text = text.as_bytes().to_vec();
            Some(std::thread::spawn(move || {
                let _ = pipe.write_all(&text);
            }))
        }
        _ => None,
    };
    let output = child.wait_with_output();
    if let Some(f) = feeder {
        let _ = f.join();
    }
    let output = match output {
        Ok(o) => o,
        Err(e) => return not_found(&reason(&e)),
    };
    let out = Value::str(&decode_go(&output.stdout));
    let err = Value::str(&decode_go(&output.stderr));
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
        &format!("`{}` exited with code {code}", argv[0]),
        vec![("code", Value::Int(code)), ("out", out), ("err", err)],
    ))
}

/// The command Go's `exec.Command` + `Start` would run, or the reason it
/// would give for not starting it, checked in Go's order: the `$PATH`
/// lookup, an empty name, the environment (with `PWD` set to the absolute
/// `cwd` when no `env` is given), the working directory, NUL bytes.
fn prepare(
    argv: &[String],
    cwd: Option<&str>,
    env: Option<Vec<(String, String)>>,
) -> Result<Command, String> {
    let name = &argv[0];
    let path = if !name.is_empty() && !name.contains('/') {
        look_path(name)?
    } else {
        name.clone()
    };
    if path.is_empty() {
        return Err("exec: no command".into());
    }
    let vars = match (env, cwd) {
        (Some(vars), _) => vars,
        (None, Some(dir)) => vec![("PWD".to_string(), abs_path(dir))],
        (None, None) => Vec::new(),
    };
    if vars
        .iter()
        .any(|(k, v)| k.contains('\0') || v.contains('\0'))
    {
        return Err("exec: environment variable contains NUL".into());
    }
    if let Some(dir) = cwd {
        if dir.contains('\0') {
            return Err("invalid argument".into());
        }
        if let Err(e) = std::fs::metadata(dir) {
            return Err(reason(&e));
        }
    }
    if path.contains('\0') || argv.iter().any(|x| x.contains('\0')) {
        return Err("invalid argument".into());
    }
    // The standard library starts programs through `execvp`, which runs a
    // file the kernel refuses (ENOEXEC) with `/bin/sh`; Go's `execve`
    // reports it instead.
    let target = match cwd {
        Some(dir) if !path.starts_with('/') => format!("{dir}/{path}"),
        _ => path.clone(),
    };
    if !kernel_can_run(&target) {
        return Err("exec format error".into());
    }
    let mut cmd = Command::new(&path);
    cmd.arg0(name).args(&argv[1..]);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.envs(vars);
    Ok(cmd)
}

/// Go `exec.LookPath` for a name without a slash: the first `$PATH` entry
/// (an empty one is the working directory) holding an executable file of
/// that name. A relative result is refused like Go's `ErrDot`.
fn look_path(name: &str) -> Result<String, String> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    if !path.is_empty() {
        for dir in path.as_bytes().split(|b| *b == b':') {
            let dir = match decode_go(dir) {
                d if d.is_empty() => ".".to_string(),
                d => d,
            };
            let candidate = format!("{dir}/{name}");
            if is_executable(&candidate) {
                if !candidate.starts_with('/') {
                    return Err("cannot run executable found relative to current directory".into());
                }
                return Ok(candidate);
            }
        }
    }
    Err("executable file not found in $PATH".into())
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

/// Go `nat.Reason`: an OS error's text without the operation, which for
/// an errno is the C library's message in lower case
/// ("no such file or directory").
fn reason(e: &std::io::Error) -> String {
    let text = e.to_string();
    let text = match text.rfind(" (os error ") {
        Some(i) if e.raw_os_error().is_some() => &text[..i],
        _ => &text,
    };
    let mut cs = text.chars();
    match cs.next() {
        Some(c) => c.to_lowercase().chain(cs).collect(),
        None => String::new(),
    }
}
