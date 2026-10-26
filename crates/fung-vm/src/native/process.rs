//! `_native.process`: `exec`, `sh` and `env` (06: 명령 실행).
//! `extras/prelude.fung` re-exports them under their plain names.
//!
//! A command runs with its standard output and error captured and its
//! standard input `/dev/null` unless `stdin` is given. Before it starts,
//! `prepare` checks what would keep it from starting and gives the reason
//! of the `not_found` error.

use std::io::{Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use fung_front::diag::Arg;
use fung_front::loader::abs_path;
use fung_front::source::decode_lossy;

use super::nat::{self, native, opt, param};
use super::oserr::reason;
use crate::value::{Lit, MapBuilder, Module, Native, Value, type_name};
use crate::vm::poller::{ExitWatch, Interest};
use crate::vm::{CancelNode, Cont, Flow, IoRegs, Next, Pool, Stop, Task, Wake};

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
            ("exec", nat::native_step("exec", exec_fn, run())),
            ("sh", nat::native_step("sh", sh_fn, run())),
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
fn exec_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
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
fn sh_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
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
/// The task parks while the command runs (`CommandWait`, R4b): its end
/// and its pipes are waited for on the event loop, so other tasks run and
/// no thread waits for it. It runs in a process group of its own; when
/// the task is cancelled (or its deadline passes, or the program exits)
/// the group gets SIGTERM, and SIGKILL if it is still there 2 s later
/// (L6), and the call unwinds with the cancellation instead of giving the
/// output.
/// What `env` must be (E0403).
const ENV_WANT: &str = "a map of variable names (not empty, no `=`) to strings";

fn run_command(t: &mut Task<'_>, argv: Vec<String>, a: &[Value]) -> Result<Flow, Stop> {
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
                            nat::s(ENV_WANT),
                            Arg::Str(format!("a map holding {}", type_name(v))),
                        ],
                    ));
                };
                // The system reads a name up to its first `=`: such a name
                // would set another one. (NUL is the system's refusal, an
                // `Err`, as in a value or `cwd`.)
                if k.is_empty() || k.contains('=') {
                    return Err(nat::fail(
                        "E0403",
                        vec![
                            nat::s("env"),
                            nat::s(ENV_WANT),
                            Arg::Str(format!("the name {}", fung_front::diag::quote(k))),
                        ],
                    ));
                }
                vars.push((k.to_string(), s.to_string()));
            }
            Some(vars)
        }
        v => return Err(nat::wrong_arg("env", "map | nil", v)),
    };
    let stdin = nat::opt_str("stdin", &a[3])?.map(|s| s.as_bytes().to_vec());
    let name = argv[0].clone();
    let own = own_group();
    let spawned = prepare(&argv, cwd.as_deref(), env, own).and_then(|mut cmd| {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd.stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        // Tests start commands from many threads at once (`testutil::child`).
        #[cfg(test)]
        let _held = crate::testutil::child::starting();
        cmd.spawn().map_err(|e| reason(&e))
    });
    let child = match spawned {
        Ok(c) => c,
        Err(reason) => {
            return Ok(Flow::Done(nat::err_value(
                "not_found",
                &format!("cannot run `{name}`: {reason}"),
                Vec::new(),
            )));
        }
    };
    CommandWait::start(t, child, own, name, stdin)
}

/// How long a command has to end after SIGTERM before SIGKILL.
const TERM_GRACE: Duration = Duration::from_secs(2);

/// How often a command is looked at when nothing can tell its end (no
/// pidfd, and the pool could not take the wait).
const POLL_EXIT: Duration = Duration::from_millis(10);

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

/// Where a command's wait stands.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Running, or ended with its output still open.
    Running,
    /// A stop came while it ran: SIGTERM was sent, SIGKILL at this time.
    Terminating(Instant),
    /// SIGKILL was sent: waiting for its end.
    Killed,
    /// A stop came after it ended while what it left holds the output:
    /// the group got SIGTERM, SIGKILL at this time; the output is dropped.
    Cutting(Instant),
}

/// The input still to give a command, and how much of it went.
struct Input {
    pipe: std::process::ChildStdin,
    text: Vec<u8>,
    off: usize,
}

/// A task waiting for a command (R4b): its end (`ExitWatch`: kqueue's
/// `EVFILT_PROC`, a pidfd, or a pool job's `waitid` without pidfd) and its
/// three pipes (non-blocking: output and error read, input written as the
/// command takes it) are registered on the event loop together, so a
/// command that fills both outputs while it reads its input never stops.
/// Each look reads and writes what it can, reaps the command once it
/// ended (`try_wait`), and registers again what is still to come.
///
/// After its end the output is read until it closes: what the command
/// left in the background (`sleep 30 &`) may hold it open. A stop while it
/// runs sends SIGTERM (to its group when it has one), SIGKILL after
/// `TERM_GRACE`; a stop while only what it left holds the output ends that
/// group the same way and drops the output. Meanwhile the task waits
/// under a fresh root node, as for a job that is waited for (`offload.rs`):
/// once the command has ended its node comes back and the stop unwinds.
///
/// A command whose wait is dropped (the REPL abandoned its task) is
/// killed, and a pool job reaps it: no orphan, no zombie (spec 15).
struct CommandWait {
    child: Option<std::process::Child>,
    own: bool,
    name: String,
    exit: ExitWatch,
    /// Without pidfd: set by the pool job that waits for the end.
    exited: Option<Arc<AtomicBool>>,
    status: Option<std::process::ExitStatus>,
    out: Option<std::process::ChildStdout>,
    err: Option<std::process::ChildStderr>,
    input: Option<Input>,
    out_buf: Vec<u8>,
    err_buf: Vec<u8>,
    phase: Phase,
    cut: bool,
    /// The task's own node while it waits under a fresh root.
    node: Option<Arc<CancelNode>>,
    regs: IoRegs,
    pool: Pool,
}

/// Makes `fd` non-blocking.
fn non_blocking(fd: RawFd) {
    // SAFETY: `fd` is an open pipe end this process owns; F_GETFL and
    // F_SETFL only change its status flags.
    unsafe {
        let fl = libc::fcntl(fd, libc::F_GETFL);
        libc::fcntl(fd, libc::F_SETFL, fl | libc::O_NONBLOCK);
    }
}

/// The most a command's pipes move in one look (`CommandWait::pump`):
/// 256 KiB, a fraction of a millisecond.
/// A command that writes as fast as it is read keeps its pipe from ever
/// running empty: moving everything at once held the run thread for the
/// whole command, and other tasks got no turn (Ubuntu 26.04's `head`
/// gave a ticking task 2 turns in 1.5 s). Past the budget the wait parks
/// again; the loop's one-shot registration fires at once on what is left.
const PUMP_BUDGET: usize = 1 << 18;

/// Reads what `pipe` has into `buf`, at most about `PUMP_BUDGET` bytes;
/// false once it reached its end (or failed, which ends it too).
fn drain(pipe: &mut impl Read, buf: &mut Vec<u8>) -> bool {
    let mut chunk = [0u8; 16 * 1024];
    let mut moved = 0;
    loop {
        if moved >= PUMP_BUDGET {
            return true;
        }
        match pipe.read(&mut chunk) {
            Ok(0) => return false,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                moved += n;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return true,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return false,
        }
    }
}

impl CommandWait {
    fn start(
        t: &mut Task<'_>,
        mut child: std::process::Child,
        own: bool,
        name: String,
        stdin: Option<Vec<u8>>,
    ) -> Result<Flow, Stop> {
        let pid = child.id() as libc::pid_t;
        let exit = t
            .shared_poller()
            .exit_watch(pid)
            .unwrap_or(ExitWatch::Unsupported);
        let out = child.stdout.take();
        let err = child.stderr.take();
        let input = match (stdin, child.stdin.take()) {
            (Some(text), Some(pipe)) => Some(Input { pipe, text, off: 0 }),
            _ => None,
        };
        for fd in [
            out.as_ref().map(|p| p.as_raw_fd()),
            err.as_ref().map(|p| p.as_raw_fd()),
            input.as_ref().map(|i| i.pipe.as_raw_fd()),
        ]
        .into_iter()
        .flatten()
        {
            non_blocking(fd);
        }
        let mut w = CommandWait {
            child: Some(child),
            own,
            name,
            exit,
            exited: None,
            status: None,
            out,
            err,
            input,
            out_buf: Vec::new(),
            err_buf: Vec::new(),
            phase: Phase::Running,
            cut: false,
            node: None,
            regs: IoRegs::default(),
            pool: t.pool(),
        };
        if matches!(w.exit, ExitWatch::Unsupported) {
            w.wait_on_the_pool(t);
        }
        t.waiter_handle().take_woken();
        match w.look_or_end(t)? {
            Next::Done(v) => Ok(Flow::Done(v)),
            Next::Park(wake) => Ok(Flow::Park(Box::new(w), wake)),
            Next::Call(..) => unreachable!("a command's wait calls nothing"),
        }
    }

    /// Without pidfd: a pool job waits for the end (`waitid` with
    /// `WNOWAIT`, so that `try_wait` still reaps it) and wakes the task.
    /// When the pool cannot take it, each look ends by `POLL_EXIT` and
    /// `try_wait` looks again.
    fn wait_on_the_pool(&mut self, t: &mut Task<'_>) {
        let flag = Arc::new(AtomicBool::new(false));
        let (f, me) = (flag.clone(), t.waiter_handle());
        let pid = self.child.as_ref().map_or(0, |c| c.id()) as libc::id_t;
        #[cfg(all(test, target_os = "linux"))]
        let reap_first = t.shared_poller().reaps_first();
        let submitted = self.pool.submit_with(
            crate::vm::Ticket::new(),
            Box::new(move || {
                // SAFETY: an all-zero `siginfo_t` is a valid value to fill
                // in; WNOWAIT leaves the child for `try_wait`.
                unsafe {
                    #[cfg(all(test, target_os = "linux"))]
                    if reap_first {
                        libc::waitpid(pid as libc::pid_t, std::ptr::null_mut(), 0);
                    }
                    let mut info: libc::siginfo_t = std::mem::zeroed();
                    // Again on EINTR; any other failure (ECHILD: it was
                    // reaped already) ends the wait too, and `try_wait`
                    // tells what happened.
                    while libc::waitid(libc::P_PID, pid, &mut info, libc::WEXITED | libc::WNOWAIT)
                        == -1
                        && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
                    {
                    }
                }
                f.store(true, Ordering::SeqCst);
                me.wake();
            }),
        );
        if submitted.is_ok() {
            self.exited = Some(flag);
        }
    }

    /// Sends `sig` to the command (its group when it has one).
    fn signal(&self, sig: libc::c_int) {
        let Some(c) = &self.child else { return };
        let id = c.id() as libc::pid_t;
        // SAFETY: `killpg`/`kill` only send a signal. The command is not
        // reaped yet, so the id names no other process or group.
        unsafe {
            if self.own {
                libc::killpg(id, sig);
            } else {
                libc::kill(id, sig);
            }
        }
    }

    /// Sends `sig` to the group of an ended command whose members may
    /// remain (only when it had a group of its own).
    fn signal_group(&self, sig: libc::c_int) {
        if !self.own {
            return;
        }
        let Some(c) = &self.child else { return };
        // SAFETY: `killpg` only sends a signal. While members of the
        // group remain, its id is not given to a new process (POSIX), so
        // this reaches only them. With none left it is ESRCH, unless the
        // id went to a new process that leads a group of its own since:
        // that needs the process ids to wrap around in between, and this
        // is sent soon after the leader was reaped (a grace time at most).
        unsafe {
            libc::killpg(c.id() as libc::pid_t, sig);
        }
    }

    /// Moves what the pipes can move now.
    fn pump(&mut self) {
        if let Some(p) = &mut self.out
            && !drain(p, &mut self.out_buf)
        {
            self.out = None;
        }
        if let Some(p) = &mut self.err
            && !drain(p, &mut self.err_buf)
        {
            self.err = None;
        }
        if let Some(i) = &mut self.input {
            let start = i.off;
            loop {
                if i.off - start >= PUMP_BUDGET {
                    break;
                }
                if i.off >= i.text.len() {
                    // all given: closing the pipe is the end of input
                    self.input = None;
                    break;
                }
                match i.pipe.write(&i.text[i.off..]) {
                    Ok(n) => i.off += n,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    // EPIPE: the command stopped reading early, no error
                    Err(_) => {
                        self.input = None;
                        break;
                    }
                }
            }
        }
    }

    /// `look`, and when it fails (the loop refused a registration) the
    /// wait ends: what it registered leaves, and the task's node comes
    /// back before the error unwinds.
    fn look_or_end(&mut self, t: &mut Task<'_>) -> Result<Next, Stop> {
        let r = self.look(t);
        if r.is_err() {
            self.regs.leave_all(t);
            if let Some(node) = self.node.take() {
                t.restore_cancel_node(node);
            }
        }
        r
    }

    /// One look: moves what it can, reaps an ended command, and either
    /// gives the value or registers what is still to come.
    fn look(&mut self, t: &mut Task<'_>) -> Result<Next, Stop> {
        loop {
            self.pump();
            if self.status.is_none()
                && let Some(c) = &mut self.child
            {
                match c.try_wait() {
                    Ok(Some(st)) => self.status = Some(st),
                    Ok(None) => {}
                    Err(e) => return self.finish(t, Err(reason(&e))),
                }
            }
            // A stop came while it ran and it has ended since, but what it
            // left still holds the output: end that too (its group, when it
            // has one, at the same time), or drop the output at once.
            if self.status.is_some() {
                match self.phase {
                    Phase::Terminating(at) if self.own => self.phase = Phase::Cutting(at),
                    Phase::Terminating(_) | Phase::Killed => self.cut = true,
                    Phase::Running | Phase::Cutting(_) => {}
                }
            }
            let now = Instant::now();
            match self.phase {
                Phase::Terminating(at) if self.status.is_none() && now >= at => {
                    self.signal(libc::SIGKILL);
                    self.phase = Phase::Killed;
                    continue;
                }
                Phase::Cutting(at) if now >= at => {
                    self.signal_group(libc::SIGKILL);
                    self.cut = true;
                }
                _ => {}
            }
            if self.status.is_some() && (self.cut || (self.out.is_none() && self.err.is_none())) {
                return self.finish(t, Ok(()));
            }
            if self.status.is_none() {
                let ended = match &self.exit {
                    ExitWatch::Unsupported => self
                        .exited
                        .as_ref()
                        .is_some_and(|f| f.load(Ordering::SeqCst)),
                    w => !self.regs.exit(t, w)?,
                };
                if ended {
                    self.regs.leave_all(t);
                    continue;
                }
            }
            if let Some(p) = &self.out {
                self.regs.fd(t, p.as_raw_fd(), Interest::Read)?;
            }
            if let Some(p) = &self.err {
                self.regs.fd(t, p.as_raw_fd(), Interest::Read)?;
            }
            if let Some(i) = &self.input {
                self.regs.fd(t, i.pipe.as_raw_fd(), Interest::Write)?;
            }
            let mut until = match self.phase {
                Phase::Terminating(at) | Phase::Cutting(at) => Some(at),
                Phase::Running | Phase::Killed => None,
            };
            if self.status.is_none()
                && matches!(self.exit, ExitWatch::Unsupported)
                && self.exited.is_none()
            {
                let poll = now + POLL_EXIT;
                until = Some(until.map_or(poll, |u| u.min(poll)));
            }
            return Ok(Next::Park(Wake { until, timed: true }));
        }
    }

    /// The command has ended: the task's node comes back (a deadline that
    /// passed meanwhile is enforced), and a stop that came unwinds;
    /// otherwise its value.
    fn finish(&mut self, t: &mut Task<'_>, r: Result<(), String>) -> Result<Next, Stop> {
        self.regs.leave_all(t);
        if let Some(node) = self.node.take() {
            t.restore_cancel_node(node);
        }
        // A cancellation (or a deadline) that came meanwhile wins over
        // the output, whether or not it cut the command short.
        t.check_cancelled()?;
        let name = &self.name;
        if let Err(reason) = r {
            return Ok(Next::Done(nat::err_value(
                "not_found",
                &format!("cannot run `{name}`: {reason}"),
                Vec::new(),
            )));
        }
        let status = self.status.expect("an ended command");
        let (o, e) = if self.cut {
            (Vec::new(), Vec::new())
        } else {
            (
                std::mem::take(&mut self.out_buf),
                std::mem::take(&mut self.err_buf),
            )
        };
        let out = Value::str(&decode_lossy(&o));
        let err = Value::str(&decode_lossy(&e));
        if status.success() {
            let mut m = MapBuilder::default();
            m.set(Rc::new("out".into()), out);
            m.set(Rc::new("err".into()), err);
            m.set(Rc::new("code".into()), Value::Int(0));
            return Ok(Next::Done(nat::ok(Value::Map(Rc::new(m.finish())))));
        }
        let code = status.code().map_or(-1, i64::from);
        Ok(Next::Done(nat::err_value(
            "exit",
            &format!("`{name}` exited with code {code}"),
            vec![("code", Value::Int(code)), ("out", out), ("err", err)],
        )))
    }
}

impl Cont for CommandWait {
    fn resume(&mut self, t: &mut Task<'_>, _: Value) -> Result<Next, Stop> {
        self.regs.leave_all(t);
        self.look_or_end(t)
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        self.regs.leave_all(t);
        if let Some(node) = self.node.take() {
            // Nothing cancels the fresh root the task waits under; should
            // a stop come anyway, the task's node comes back with it.
            t.restore_cancel_node(node);
            return Err(stop);
        }
        let grace = Instant::now() + TERM_GRACE;
        if self.status.is_none() {
            self.signal(libc::SIGTERM);
            self.phase = Phase::Terminating(grace);
        } else if self.own {
            self.signal_group(libc::SIGTERM);
            self.phase = Phase::Cutting(grace);
        } else {
            self.cut = true;
        }
        self.node = Some(t.swap_in_fresh_root());
        self.look_or_end(t)
    }
}

impl Drop for CommandWait {
    fn drop(&mut self) {
        // Abandoned after it ended while what it left holds the output:
        // end that group (spec 15).
        if self.status.is_some() && (self.out.is_some() || self.err.is_some()) {
            self.signal_group(libc::SIGKILL);
        }
        // Abandoned while the command runs: kill it, and reap it on the
        // pool (a blocking `wait`).
        if self.status.is_none()
            && let Some(mut c) = self.child.take()
        {
            let id = c.id() as libc::pid_t;
            // SAFETY: `killpg`/`kill` only send a signal; the command is
            // not reaped yet, so the id names no other process or group.
            unsafe {
                if self.own {
                    libc::killpg(id, libc::SIGKILL);
                } else {
                    libc::kill(id, libc::SIGKILL);
                }
            }
            let reap = Box::new(move || {
                let _ = c.wait();
            });
            // No pool thread at all: the zombie stays until fung ends.
            let _ = self.pool.submit_with(crate::vm::Ticket::new(), reap);
        }
    }
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
    // the command starts with none blocked, and with the open file limit
    // fung started with (`rlimit.rs`).
    // SAFETY: the hook runs in the forked child before `exec` and only
    // makes async-signal-safe calls (`clear_mask_in_child`,
    // `restore_in_child`); it touches no memory of the parent's other
    // threads.
    unsafe {
        cmd.pre_exec(|| {
            crate::signals::clear_mask_in_child()?;
            crate::rlimit::restore_in_child()
        });
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
