//! `poller.rs` over kqueue: a read and a write wait are two filters of
//! one descriptor (`EVFILT_READ`, `EVFILT_WRITE`) with `EV_ONESHOT`, the
//! token in `udata`; the waker is an `EVFILT_USER` event with `EV_CLEAR`.

use std::io;
use std::os::fd::RawFd;
use std::time::Duration;

use super::Interest;

/// The token the waker's event would carry (never given out).
pub(super) const WAKER: u64 = u64::MAX;

/// The waker's `EVFILT_USER` identifier.
const WAKER_IDENT: usize = 0;

pub(super) struct Sys {
    kq: RawFd,
}

fn event(ident: usize, filter: i16, flags: u16, fflags: u32, udata: u64) -> libc::kevent {
    libc::kevent {
        ident,
        filter,
        flags,
        fflags,
        data: 0,
        udata: udata as usize as *mut libc::c_void,
    }
}

fn filter(i: Interest) -> i16 {
    match i {
        Interest::Read => libc::EVFILT_READ,
        Interest::Write => libc::EVFILT_WRITE,
    }
}

impl Sys {
    pub(super) fn new() -> io::Result<Sys> {
        // SAFETY: `kqueue` takes no arguments; the descriptor it returns
        // is owned by `Sys` and closed in its drop.
        let kq = unsafe { libc::kqueue() };
        if kq < 0 {
            return Err(io::Error::last_os_error());
        }
        let s = Sys { kq };
        // SAFETY: `kq` is a valid descriptor; FD_CLOEXEC keeps it out of
        // the commands fung runs.
        unsafe {
            libc::fcntl(kq, libc::F_SETFD, libc::FD_CLOEXEC);
        }
        s.change(event(
            WAKER_IDENT,
            libc::EVFILT_USER,
            libc::EV_ADD | libc::EV_CLEAR,
            0,
            WAKER,
        ))?;
        Ok(s)
    }

    /// Applies one change.
    fn change(&self, e: libc::kevent) -> io::Result<()> {
        // SAFETY: the change list is the one valid event; no event list
        // is asked for, so no output buffer is written.
        let n = unsafe { libc::kevent(self.kq, &e, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn register(&self, fd: RawFd, i: Interest, token: u64) -> io::Result<()> {
        self.change(event(
            fd as usize,
            filter(i),
            libc::EV_ADD | libc::EV_ONESHOT,
            0,
            token,
        ))
    }

    pub(super) fn deregister(&self, fd: RawFd, i: Interest) {
        // Not registered (ENOENT), or the descriptor is closed: nothing
        // to drop.
        let _ = self.change(event(fd as usize, filter(i), libc::EV_DELETE, 0, 0));
    }

    pub(super) fn exit_watch(&self, pid: libc::pid_t) -> io::Result<super::ExitWatch> {
        Ok(super::ExitWatch::Pid(pid))
    }

    /// `EVFILT_PROC` with `NOTE_EXIT`, one-shot; `ESRCH`: it has ended.
    pub(super) fn register_exit(&self, pid: libc::pid_t, token: u64) -> io::Result<bool> {
        match self.change(event(
            pid as usize,
            libc::EVFILT_PROC,
            libc::EV_ADD | libc::EV_ONESHOT,
            libc::NOTE_EXIT,
            token,
        )) {
            Ok(()) => Ok(true),
            Err(e) if e.raw_os_error() == Some(libc::ESRCH) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub(super) fn deregister_exit(&self, pid: libc::pid_t) {
        let _ = self.change(event(
            pid as usize,
            libc::EVFILT_PROC,
            libc::EV_DELETE,
            0,
            0,
        ));
    }

    pub(super) fn wait(&self, out: &mut Vec<u64>, timeout: Option<Duration>) -> io::Result<()> {
        let ts = timeout.map(|d| libc::timespec {
            tv_sec: d.as_secs().min(i64::MAX as u64) as libc::time_t,
            tv_nsec: libc::c_long::from(d.subsec_nanos() as i32),
        });
        let tsp = ts
            .as_ref()
            .map_or(std::ptr::null(), |t| t as *const libc::timespec);
        // SAFETY: an all-zero `kevent` is a valid value of the plain C
        // struct; the kernel fills at most `events.len()` of them.
        let mut events: [libc::kevent; 64] = unsafe { std::mem::zeroed() };
        // SAFETY: no change list; `events` is a valid buffer of the
        // length given; `tsp` is null or points at `ts`, alive here.
        let n = unsafe {
            libc::kevent(
                self.kq,
                std::ptr::null(),
                0,
                events.as_mut_ptr(),
                events.len() as libc::c_int,
                tsp,
            )
        };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                return Ok(());
            }
            return Err(e);
        }
        for e in &events[..n as usize] {
            if e.filter == libc::EVFILT_USER {
                continue;
            }
            out.push(e.udata as usize as u64);
        }
        Ok(())
    }

    pub(super) fn wake(&self) {
        // A failure here would mean the queue is gone; the wait it would
        // end is gone with it.
        let _ = self.change(event(
            WAKER_IDENT,
            libc::EVFILT_USER,
            0,
            libc::NOTE_TRIGGER,
            WAKER,
        ));
    }
}

impl Drop for Sys {
    fn drop(&mut self) {
        // SAFETY: `kq` is owned by this `Sys` and closed once, here.
        unsafe {
            libc::close(self.kq);
        }
    }
}
