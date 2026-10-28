//! The event loop core (concurrency v2 R4a, spec 15): which file
//! descriptors are ready, over kqueue (macOS) or epoll (Linux).
//!
//! **Readiness, one-shot.** A registration says "tell me once when this
//! descriptor can be read (or written)", with a token. When it is ready
//! the token comes out of `wait` once; the registration then sleeps until
//! it is made again. The one who registered (a parked task) tries its
//! system call, and registers again only if that would still block. So a
//! descriptor nobody waits on never wakes the loop, and two waits never
//! get one event.
//!
//! The token, not the descriptor, names the wait: a descriptor closed and
//! its number reused by a new socket must not wake the old waiter, so the
//! scheduler gives each park a fresh token and drops events of tokens it
//! no longer knows (`sched.rs`).
//!
//! A read and a write wait on one descriptor are apart (a task reading a
//! connection while another writes it). A hang-up or an error fires the
//! waits there: the next system call reports it.
//!
//! **Waking from another thread.** `Waker::wake` makes the current or the
//! next `wait` return (`EVFILT_USER`, `eventfd`); wakes before a wait are
//! kept, and many count as one.

use std::io;
use std::os::fd::RawFd;
use std::sync::Arc;
use std::time::Duration;

#[cfg(target_os = "linux")]
#[path = "poller/epoll.rs"]
mod sys;
#[cfg(any(target_os = "macos", target_os = "freebsd"))]
#[path = "poller/kqueue.rs"]
mod sys;

/// What a wait on a descriptor waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Interest {
    Read,
    Write,
}

/// How the loop learns that a child process ended (`exit_watch`).
pub(crate) enum ExitWatch {
    /// kqueue's `EVFILT_PROC` on the pid.
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    Pid(libc::pid_t),
    /// Linux: a pidfd, readable once the process ended.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Fd(std::os::fd::OwnedFd),
    /// Neither (an old Linux kernel): someone else must wait for it.
    Unsupported,
}

/// The loop: registrations and the wait.
pub(crate) struct Poller {
    sys: Arc<sys::Sys>,
    /// The registration that many from now fails, 0 for none (tests).
    #[cfg(test)]
    fail_at: std::sync::atomic::AtomicUsize,
    /// pidfd is as if missing (tests).
    #[cfg(test)]
    no_pidfd: std::sync::atomic::AtomicBool,
}

/// Wakes a `Poller`'s wait from any thread.
#[derive(Clone)]
pub(crate) struct Waker {
    sys: Arc<sys::Sys>,
}

impl Poller {
    pub(crate) fn new() -> io::Result<Poller> {
        Ok(Poller {
            sys: Arc::new(sys::Sys::new()?),
            #[cfg(test)]
            fail_at: std::sync::atomic::AtomicUsize::new(0),
            #[cfg(test)]
            no_pidfd: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Waits once for `interest` on `fd`, naming the wait `token`; a new
    /// registration for the same `fd` and `interest` replaces it.
    pub(crate) fn register(&self, fd: RawFd, interest: Interest, token: u64) -> io::Result<()> {
        debug_assert!(token != sys::WAKER, "a reserved token");
        #[cfg(test)]
        self.injected_failure()?;
        self.sys.register(fd, interest, token)
    }

    /// Drops the wait for `interest` on `fd`, if there is one.
    pub(crate) fn deregister(&self, fd: RawFd, interest: Interest) {
        self.sys.deregister(fd, interest);
    }

    /// Waits until a registration is ready, a waker wakes it, or
    /// `timeout` passes (`None`: no timeout), and adds the tokens of the
    /// ready ones to `out`. A signal ends the wait early with none.
    pub(crate) fn wait(&self, out: &mut Vec<u64>, timeout: Option<Duration>) -> io::Result<()> {
        self.sys.wait(out, timeout)
    }

    /// What waits for child `pid`'s end: made once per child (a pidfd on
    /// Linux), registered once per wait (`register_exit`).
    pub(crate) fn exit_watch(&self, pid: libc::pid_t) -> io::Result<ExitWatch> {
        #[cfg(test)]
        if self.no_pidfd.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(ExitWatch::Unsupported);
        }
        self.sys.exit_watch(pid)
    }

    /// Waits once for the end of `w`'s child under `token`; false if it
    /// has ended already (look at once). `Unsupported` cannot be waited
    /// for here.
    pub(crate) fn register_exit(&self, w: &ExitWatch, token: u64) -> io::Result<bool> {
        #[cfg(test)]
        self.injected_failure()?;
        match w {
            ExitWatch::Pid(pid) => self.sys.register_exit(*pid, token),
            ExitWatch::Fd(fd) => {
                use std::os::fd::AsRawFd;
                self.sys.register(fd.as_raw_fd(), Interest::Read, token)?;
                Ok(true)
            }
            ExitWatch::Unsupported => Err(io::ErrorKind::Unsupported.into()),
        }
    }

    pub(crate) fn deregister_exit(&self, w: &ExitWatch) {
        match w {
            ExitWatch::Pid(pid) => self.sys.deregister_exit(*pid),
            ExitWatch::Fd(fd) => {
                use std::os::fd::AsRawFd;
                self.sys.deregister(fd.as_raw_fd(), Interest::Read);
            }
            ExitWatch::Unsupported => {}
        }
    }

    /// Makes pidfd look missing (tests: a command's end without it).
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn disable_pidfd(&self) {
        self.no_pidfd
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Makes the next registration fail with `ENOSPC` (tests).
    #[cfg(test)]
    pub(crate) fn fail_next_register(&self) {
        self.fail_register_at(1);
    }

    /// Makes the `n`-th registration from now fail with `ENOSPC` (tests).
    #[cfg(test)]
    pub(crate) fn fail_register_at(&self, n: usize) {
        self.fail_at.store(n, std::sync::atomic::Ordering::SeqCst);
    }

    #[cfg(test)]
    fn injected_failure(&self) -> io::Result<()> {
        use std::sync::atomic::Ordering::SeqCst;
        let n = self.fail_at.load(SeqCst);
        if n == 0 {
            return Ok(());
        }
        self.fail_at.store(n - 1, SeqCst);
        if n == 1 {
            return Err(io::Error::from_raw_os_error(libc::ENOSPC));
        }
        Ok(())
    }

    pub(crate) fn waker(&self) -> Waker {
        Waker {
            sys: self.sys.clone(),
        }
    }
}

impl Waker {
    pub(crate) fn wake(&self) {
        self.sys.wake();
    }
}
