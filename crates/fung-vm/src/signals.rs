//! SIGINT and SIGTERM (concurrency v2, L6; 08 "신호").
//!
//! The command blocks both signals in its first thread, before any other
//! thread exists ([`init`]), so every thread inherits the block, and one
//! thread of its own receives them with `sigwait`. So no signal handler
//! runs at an arbitrary point of an arbitrary thread: what a signal does
//! is ordinary code on that thread, which may take locks and post an event
//! to the run thread (a handler could only set a flag).
//!
//! What a signal does is the command's [`Policy`]: none set, the process
//! ends at once with 128 + the signal's number, as the default action
//! would (no command that runs no fung code needs more). `fung run`,
//! `-e` and `test` set [`stop_requests`]: the first signal is a stop
//! request (`vm/stop.rs`), the second ends the process at once. The REPL
//! cancels the evaluation in progress on SIGINT.
//!
//! A child inherits the mask across `exec`, so a command fung runs
//! would start with SIGINT and SIGTERM blocked: `process.rs` clears the
//! child's mask between `fork` and `exec` ([`clear_mask_in_child`]).

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crate::vm::StopHandle;

/// What a signal does, called on the signal thread with its number.
pub type Policy = Arc<dyn Fn(i32) + Send + Sync>;

static POLICY: Mutex<Option<Policy>> = Mutex::new(None);

/// The set of SIGINT, SIGTERM and SIGHUP (which ends the process at once,
/// restoring a raw terminal first).
fn stop_set() -> libc::sigset_t {
    // SAFETY: `sigemptyset` initialises the zeroed local set before
    // `sigaddset` uses it; both only touch that set.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        libc::sigaddset(&mut set, libc::SIGHUP);
        set
    }
}

/// Blocks SIGINT and SIGTERM in this thread and starts the thread that
/// receives them. Call it first in `main`, before any other thread is
/// made: threads made later inherit the block.
pub fn init() {
    let set = stop_set();
    // SAFETY: `pthread_sigmask` reads the initialised local `set` and
    // accepts a null old-mask pointer; it changes only this thread's mask.
    unsafe {
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
    }
    let started = std::thread::Builder::new()
        .name("fung-signals".into())
        .spawn(move || {
            loop {
                let mut sig: libc::c_int = 0;
                // SAFETY: `sigwait` reads the initialised `set` and writes
                // the signal's number into the local `sig`.
                let r = unsafe { libc::sigwait(&set, &mut sig) };
                if r != 0 {
                    continue;
                }
                // A hang-up ends the process as its default action would,
                // whatever the command: the terminal is gone (08).
                if sig == libc::SIGHUP {
                    exit_now(sig);
                }
                let policy = POLICY.lock().unwrap_or_else(|e| e.into_inner()).clone();
                match policy {
                    Some(p) => p(sig),
                    None => exit_now(sig),
                }
            }
        });
    // Without the thread the signals would stay blocked: unblock them so
    // the default action still ends the process.
    if started.is_err() {
        // SAFETY: as above.
        unsafe {
            libc::pthread_sigmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut());
        }
    }
}

/// Clears the signal mask of a forked child before it `exec`s (a
/// `CommandExt::pre_exec` hook): only `sigemptyset` and `sigprocmask`,
/// both async-signal-safe, so it is sound between `fork` and `exec`.
pub fn clear_mask_in_child() -> std::io::Result<()> {
    // SAFETY: `sigemptyset` initialises the zeroed local set, which
    // `sigprocmask` reads; the child has a single thread here, where
    // `sigprocmask` sets that thread's mask.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        if libc::sigprocmask(libc::SIG_SETMASK, &set, std::ptr::null_mut()) != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Sets what a signal does from now on; `None` ends the process.
pub fn set(policy: Option<Policy>) {
    *POLICY.lock().unwrap_or_else(|e| e.into_inner()) = policy;
}

/// Ends the process at once with 128 + `sig`, without unwinding or
/// cleanup (a second signal; a command that runs no fung code). A
/// terminal in the REPL's raw mode is put back first (`term::restore`).
pub fn exit_now(sig: i32) -> ! {
    crate::term::restore();
    std::process::exit(128 + sig)
}

/// The policy of a run: the first signal is a stop request to `h`'s
/// runtime, any later one ends the process at once. `first` keeps the
/// first signal's number (0 until one came): `fung test` stops running
/// files after it, and a later runtime's policy shares it, so a signal
/// after the first still ends the process.
///
/// The request only posts an event for the run thread (`StopHandle`), so
/// the signal thread goes back to `sigwait` at once, and a second signal
/// ends the process even while a long native runs.
pub fn stop_requests(h: StopHandle, first: Arc<AtomicI32>) -> Policy {
    Arc::new(move |sig| {
        if first
            .compare_exchange(0, sig, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            h.deliver(sig);
        } else {
            exit_now(sig);
        }
    })
}

/// The REPL's policy: SIGINT cancels the evaluation in progress (the
/// session goes on), SIGTERM ends the process at once.
pub fn repl(h: StopHandle) -> Policy {
    Arc::new(move |sig| {
        if sig == libc::SIGINT {
            h.interrupt();
        } else {
            exit_now(sig);
        }
    })
}
