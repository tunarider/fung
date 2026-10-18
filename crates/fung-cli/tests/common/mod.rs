//! What the CLI tests share: running a binary, the expected-output files
//! (`expected`) and a TLS server (`tls_server`).

// Each test binary uses its own subset of these helpers.
#![allow(dead_code)]

pub mod expected;
pub mod tls_server;

use std::{
    ffi::OsString,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// The `fung` binary under test.
pub fn fung_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fung"))
}

/// Standard output, standard error (bytes, compared exactly) and the exit
/// status, compared as its debug text so two different signal deaths differ.
pub type Run = (Vec<u8>, Vec<u8>, String);

/// The local zone every fung child of a comparison or expected-output
/// suite runs in, so what it prints does not depend on the host's zone
/// (`time_expected` sets its own). `UTC` needs no zoneinfo files.
pub const TZ: &str = "UTC";

/// How long one run may take before it is killed.
pub const RUN_LIMIT: std::time::Duration = std::time::Duration::from_secs(60);

/// Runs `bin` from the repository root with no standard input.
pub fn run(bin: &Path, args: &[OsString]) -> Run {
    run_with_stdin(bin, args, None)
}

/// Runs `bin` from the repository root; `stdin` is written to its
/// standard input and closed (`None`: standard input is closed at once).
pub fn run_with_stdin(bin: &Path, args: &[OsString], stdin: Option<&[u8]>) -> Run {
    let (out, err, status) = run_status(bin, args, stdin);
    (out, err, format!("{status:?}"))
}

/// Like [`run_with_stdin`], with the exit status itself.
pub fn run_status(
    bin: &Path,
    args: &[OsString],
    stdin: Option<&[u8]>,
) -> (Vec<u8>, Vec<u8>, std::process::ExitStatus) {
    // killed and waited for if anything below panics
    let mut child = Reaped::new(
        Command::new(bin)
            .args(args)
            .env("TZ", TZ)
            .current_dir(repo())
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let writer = stdin.map(|data| {
        let mut pipe = child.stdin.take().unwrap();
        let data = data.to_vec();
        // A thread, so a child that writes much before reading cannot
        // deadlock against us. A child that exits early closes the pipe.
        std::thread::spawn(move || {
            let _ = pipe.write_all(&data);
        })
    });
    // A run that hangs is killed (and then differs) rather than hanging
    // the test.
    let (o, _) = child.output_within(RUN_LIMIT).unwrap();
    if let Some(w) = writer {
        w.join().unwrap();
    }
    (o.stdout, o.stderr, o.status)
}

/// `f` over `xs` on a few worker threads, results in order. Each worker
/// takes the next item when it is free, so a slow item holds up only its
/// own worker.
pub fn par_map<T: Sync, R: Send>(xs: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let n = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let next = AtomicUsize::new(0);
    let mut out: Vec<(usize, R)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..n)
            .map(|_| {
                s.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(x) = xs.get(i) else { break };
                        done.push((i, f(x)));
                    }
                    done
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    out.sort_by_key(|(i, _)| *i);
    out.into_iter().map(|(_, r)| r).collect()
}

/// A child process that is killed and waited for when dropped, so a test
/// that panics (a failed assertion included) leaves no process behind,
/// and no zombie either. Every child a test starts and does not finish
/// with `Command::output`/`status` on the same thread goes in one.
/// Killing a child that has already been waited for does nothing (the
/// standard library remembers its status and sends no signal, so a
/// reused process id is never hit).
///
/// Only the child itself is killed: a grandchild that outlives it is
/// not. The programs the tests start leave none behind (`sh` wrappers
/// `exec` fung; fung waits for the processes it starts). Starting each
/// child in its own process group would let the guard kill the whole
/// group, but it would also keep Ctrl-C at the terminal from reaching
/// the children, which is how an interrupted `cargo test` ends them now.
pub struct Reaped(Option<Child>);

impl Reaped {
    pub fn new(child: Child) -> Reaped {
        Reaped(Some(child))
    }

    /// The child itself, no longer killed on drop.
    pub fn into_inner(mut self) -> Child {
        self.0.take().unwrap()
    }

    /// `Child::wait_with_output` that keeps the guard: the piped streams
    /// are read on threads while the child runs.
    pub fn output(self) -> std::io::Result<Output> {
        self.output_within(Duration::MAX).map(|(o, _)| o)
    }

    /// Like [`Reaped::output`], killing the child after `limit`: the
    /// output, and whether it was killed. A killed child is waited for
    /// too. The standard streams are read to their end, so a grandchild
    /// still holding them would keep this waiting (no test starts one).
    pub fn output_within(mut self, limit: Duration) -> std::io::Result<(Output, bool)> {
        let out = drain(self.stdout.take());
        let err = drain(self.stderr.take());
        let start = Instant::now();
        // Short runs end in a few milliseconds, long ones need no fine
        // polling: the pause doubles up to 20ms.
        let mut pause = Duration::from_millis(1);
        let mut killed = false;
        let status = loop {
            if let Some(st) = self.try_wait()? {
                break st;
            }
            if start.elapsed() > limit {
                let _ = self.kill();
                killed = true;
                break self.wait()?;
            }
            std::thread::sleep(pause);
            pause = (pause * 2).min(Duration::from_millis(20));
        };
        let output = Output {
            status,
            stdout: out.join().unwrap()?,
            stderr: err.join().unwrap()?,
        };
        Ok((output, killed))
    }
}

/// Reads `pipe` (if any) to its end on a thread.
fn drain<R: Read + Send + 'static>(
    pipe: Option<R>,
) -> std::thread::JoinHandle<std::io::Result<Vec<u8>>> {
    std::thread::spawn(move || {
        let mut b = Vec::new();
        if let Some(mut p) = pipe {
            p.read_to_end(&mut b)?;
        }
        Ok(b)
    })
}

impl std::ops::Deref for Reaped {
    type Target = Child;
    fn deref(&self) -> &Child {
        self.0.as_ref().unwrap()
    }
}

impl std::ops::DerefMut for Reaped {
    fn deref_mut(&mut self) -> &mut Child {
        self.0.as_mut().unwrap()
    }
}

impl Drop for Reaped {
    fn drop(&mut self) {
        if let Some(c) = &mut self.0 {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
