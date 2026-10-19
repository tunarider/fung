//! `poller.rs` over epoll. epoll keeps one registration per descriptor,
//! so a read and a write wait on one descriptor share it: the tokens of
//! both are kept here (`waits`), the registration asks for both with
//! `EPOLLONESHOT`, and after an event the wait still pending is armed
//! again. The registration's data is the descriptor. The waker is an
//! `eventfd`, level-triggered, read empty when it fires.

use std::collections::HashMap;
use std::io;
use std::os::fd::RawFd;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::Interest;

/// The data of the waker's registration (never a descriptor's).
pub(super) const WAKER: u64 = u64::MAX;

/// The tokens waiting on one descriptor.
#[derive(Default, Clone, Copy)]
struct Waits {
    read: Option<u64>,
    write: Option<u64>,
}

impl Waits {
    fn events(self) -> u32 {
        let mut e = libc::EPOLLONESHOT as u32;
        // The peer's end of stream is for a read wait only: asked for
        // with a write wait alone it would fire again and again while
        // the write waits for room (a half-closed peer).
        if self.read.is_some() {
            e |= libc::EPOLLIN as u32 | libc::EPOLLRDHUP as u32;
        }
        if self.write.is_some() {
            e |= libc::EPOLLOUT as u32;
        }
        e
    }

    fn is_empty(self) -> bool {
        self.read.is_none() && self.write.is_none()
    }
}

pub(super) struct Sys {
    ep: RawFd,
    wake_fd: RawFd,
    waits: Mutex<HashMap<RawFd, Waits>>,
}

impl Sys {
    pub(super) fn new() -> io::Result<Sys> {
        // SAFETY: plain flags; the descriptor returned is owned by `Sys`.
        let ep = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        if ep < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: plain flags; the descriptor returned is owned by `Sys`.
        let wake_fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if wake_fd < 0 {
            let e = io::Error::last_os_error();
            // SAFETY: `ep` was just made and is closed once.
            unsafe { libc::close(ep) };
            return Err(e);
        }
        let s = Sys {
            ep,
            wake_fd,
            waits: Mutex::default(),
        };
        let mut ev = libc::epoll_event {
            events: libc::EPOLLIN as u32,
            u64: WAKER,
        };
        // SAFETY: both descriptors are valid; `ev` is a valid event.
        if unsafe { libc::epoll_ctl(ep, libc::EPOLL_CTL_ADD, wake_fd, &mut ev) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(s)
    }

    fn waits(&self) -> MutexGuard<'_, HashMap<RawFd, Waits>> {
        // A map of plain values that no panic leaves half-changed.
        self.waits.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Arms `fd` for `w` (adding it if the kernel does not have it).
    fn arm(&self, fd: RawFd, w: Waits) -> io::Result<()> {
        let mut ev = libc::epoll_event {
            events: w.events(),
            u64: fd as u64,
        };
        // SAFETY: `ep` is valid; `ev` is a valid event.
        if unsafe { libc::epoll_ctl(self.ep, libc::EPOLL_CTL_MOD, fd, &mut ev) } == 0 {
            return Ok(());
        }
        let e = io::Error::last_os_error();
        if e.raw_os_error() != Some(libc::ENOENT) {
            return Err(e);
        }
        // SAFETY: as above.
        if unsafe { libc::epoll_ctl(self.ep, libc::EPOLL_CTL_ADD, fd, &mut ev) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn drop_fd(&self, fd: RawFd) {
        // SAFETY: `ep` is valid; a null event is allowed for DEL. ENOENT
        // or EBADF (closed already) mean there is nothing to drop.
        unsafe {
            libc::epoll_ctl(self.ep, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut());
        }
    }

    pub(super) fn register(&self, fd: RawFd, i: Interest, token: u64) -> io::Result<()> {
        let mut waits = self.waits();
        let mut w = waits.get(&fd).copied().unwrap_or_default();
        match i {
            Interest::Read => w.read = Some(token),
            Interest::Write => w.write = Some(token),
        }
        self.arm(fd, w)?;
        waits.insert(fd, w);
        Ok(())
    }

    pub(super) fn deregister(&self, fd: RawFd, i: Interest) {
        let mut waits = self.waits();
        let Some(mut w) = waits.get(&fd).copied() else {
            return;
        };
        match i {
            Interest::Read => w.read = None,
            Interest::Write => w.write = None,
        }
        if w.is_empty() {
            waits.remove(&fd);
            self.drop_fd(fd);
        } else {
            waits.insert(fd, w);
            let _ = self.arm(fd, w);
        }
    }

    /// A pidfd for `pid` (readable once it ended); `Unsupported` when
    /// the kernel has none (`ENOSYS`, before Linux 5.3).
    pub(super) fn exit_watch(&self, pid: libc::pid_t) -> io::Result<super::ExitWatch> {
        use std::os::fd::FromRawFd;
        // SAFETY: `pidfd_open` takes a pid and flags and returns a new
        // descriptor (close-on-exec by the kernel) or -1.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        if fd < 0 {
            let e = io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::ENOSYS) {
                return Ok(super::ExitWatch::Unsupported);
            }
            return Err(e);
        }
        // SAFETY: `fd` was just made and is owned by nothing else.
        Ok(super::ExitWatch::Fd(unsafe {
            std::os::fd::OwnedFd::from_raw_fd(fd as RawFd)
        }))
    }

    pub(super) fn register_exit(&self, _: libc::pid_t, _: u64) -> io::Result<bool> {
        unreachable!("a pidfd is registered as a descriptor")
    }

    pub(super) fn deregister_exit(&self, _: libc::pid_t) {}

    pub(super) fn wait(&self, out: &mut Vec<u64>, timeout: Option<Duration>) -> io::Result<()> {
        // Milliseconds, rounded up: the wait is never shorter than asked.
        let ms = match timeout {
            None => -1,
            Some(d) => {
                let ms = d.as_nanos().div_ceil(1_000_000);
                ms.min(i32::MAX as u128) as libc::c_int
            }
        };
        let mut events = [libc::epoll_event { events: 0, u64: 0 }; 64];
        // SAFETY: `events` is a valid buffer of the length given.
        let n = unsafe {
            libc::epoll_wait(
                self.ep,
                events.as_mut_ptr(),
                events.len() as libc::c_int,
                ms,
            )
        };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                return Ok(());
            }
            return Err(e);
        }
        let mut waits = self.waits();
        for e in &events[..n as usize] {
            if e.u64 == WAKER {
                let mut buf = [0u8; 8];
                // SAFETY: `wake_fd` is valid; `buf` holds the 8 bytes an
                // eventfd read gives. EAGAIN (read already) is fine.
                unsafe {
                    libc::read(self.wake_fd, buf.as_mut_ptr().cast(), buf.len());
                }
                continue;
            }
            let fd = e.u64 as RawFd;
            let Some(mut w) = waits.get(&fd).copied() else {
                continue;
            };
            let bad = libc::EPOLLERR as u32 | libc::EPOLLHUP as u32 | libc::EPOLLRDHUP as u32;
            if e.events & (libc::EPOLLIN as u32 | bad) != 0
                && let Some(t) = w.read.take()
            {
                out.push(t);
            }
            let writable = libc::EPOLLOUT as u32 | libc::EPOLLERR as u32 | libc::EPOLLHUP as u32;
            if e.events & writable != 0
                && let Some(t) = w.write.take()
            {
                out.push(t);
            }
            if w.is_empty() {
                // One-shot left it disarmed; keep the registration for
                // the next wait (a MOD), it goes when the fd closes.
                waits.remove(&fd);
            } else {
                waits.insert(fd, w);
                let _ = self.arm(fd, w);
            }
        }
        Ok(())
    }

    pub(super) fn wake(&self) {
        let one: u64 = 1;
        // SAFETY: `wake_fd` is valid; an eventfd write takes 8 bytes. A
        // full counter (EAGAIN) still leaves it readable.
        unsafe {
            libc::write(self.wake_fd, (&one as *const u64).cast(), 8);
        }
    }
}

impl Drop for Sys {
    fn drop(&mut self) {
        // SAFETY: both descriptors are owned by this `Sys` and closed
        // once, here.
        unsafe {
            libc::close(self.wake_fd);
            libc::close(self.ep);
        }
    }
}
