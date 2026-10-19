//! Death by SIGPIPE on a closed standard output or error.
//!
//! fung's rule (06): a write to standard output or error whose reader has
//! gone ends the process by SIGPIPE, so `fung run x | head -1` stops at
//! once, from whichever task wrote, with no report and no further cleanup,
//! and the parent sees signal 13, as for any Unix filter. A write to any
//! other descriptor that finds the reader gone (a child's closed standard
//! input, a fifo whose reader left, a socket) is an error value.
//!
//! The Rust runtime sets SIGPIPE to ignored at startup and fung keeps it
//! so, which makes those other writes fail with EPIPE. Only the standard
//! streams call [`die`], which restores the default action just before
//! raising. (`std::process::Command` already resets SIGPIPE to the
//! default in a child.)

/// The status a shell shows for a process killed by SIGPIPE (128 + 13):
/// the exit code when the signal cannot kill the process.
pub const EXIT_BROKEN_PIPE: i32 = 141;

/// Ends the process by SIGPIPE, for a write to standard output or error
/// that found the reading end closed (EPIPE). Callable from any thread,
/// whatever locks other threads hold or the run thread is doing: no fung value is
/// touched and nothing is unwound or flushed.
///
/// It sets the default action even when the process was started with
/// SIGPIPE ignored: the rule is the same however fung was started, and a
/// script in a pipeline must stop when the pipeline does. It unblocks
/// SIGPIPE in the calling thread first. If the signal still does not
/// kill the process, it exits with [`EXIT_BROKEN_PIPE`], what a shell
/// would show for the signal.
pub fn die() -> ! {
    #[cfg(unix)]
    // SAFETY: `signal`, `sigemptyset`, `sigaddset`, `pthread_sigmask` and
    // `raise` are thread-safe libc calls; the only memory they touch is
    // `set`, a local `sigset_t` that `sigemptyset` initialises before any
    // other use, and the null old-mask pointer `pthread_sigmask` accepts.
    // Setting the default action (SIG_DFL, no handler code of ours)
    // cannot break an invariant of the Rust runtime: std only relies on
    // SIGPIPE being ignored so writes return EPIPE, and this process is
    // about to end. `raise` sends SIGPIPE to the calling thread, now
    // unblocked there, so it is delivered before `raise` returns; the
    // default action terminates the whole process in the kernel, so no
    // other thread runs on and no lock it holds matters.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGPIPE);
        libc::pthread_sigmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut());
        libc::raise(libc::SIGPIPE);
    }
    std::process::exit(EXIT_BROKEN_PIPE)
}
