//! FIFOs read and written through the event loop (L11 C; 06 `fs`).
//!
//! A regular file is always "ready", so its input and output run on a
//! pool thread. A FIFO is not: a read waits for a writer, a write for a
//! reader and for room. Done on a pool thread, such a wait could not be
//! cut short (a deadline or a cancellation left the thread blocked, and
//! the pool filled up). So a FIFO is opened with `O_NONBLOCK` and every
//! read and write is tried on the run thread; one that would block parks
//! the task on the descriptor (`vm::park_io`). Bytes are taken only when
//! they are there, so a cancelled read loses nothing: what came stays in
//! the handle's buffer for the next read.
//!
//! The two ends meet differently from a blocking `open`:
//! - a read end opens at once, and a read that finds no writer yet waits
//!   for one (where a blocking `open` would have waited);
//! - a write end needs a reader: `open` says ENXIO until one comes, so it
//!   is tried again every `RELOOK` until then (no event says it).
//!
//! A read gives 0 bytes both before any writer came and after the last
//! one left. It is the end only once a writer was seen: data or EAGAIN
//! came, or (Linux) the loop woke the task, which epoll does on data and
//! on the hang-up. macOS reports neither a writer's open nor its close on
//! a FIFO (neither kqueue nor poll), so there the read looks again every
//! `RELOOK`; a writer that opens and closes without writing between two
//! looks is not seen there, and the read waits for the next writer.
//!
//! On macOS `/dev/fd/N` (and `/dev/stdin` and the like) opens the same
//! open file as descriptor N: making it non-blocking would change fung's
//! own standard input, so those stay on the pool (`is_shared_alias`).

use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
use std::time::{Duration, Instant};

use crate::vm::Interest;

/// How often a wait no event ends looks again: a write end waiting for a
/// reader, and (macOS) a read that may have reached the end.
pub(crate) const RELOOK: Duration = Duration::from_millis(50);

/// The size of one read.
const CHUNK: usize = 64 * 1024;

/// One end of a FIFO, non-blocking, with what was read and not yet
/// taken and what was written and not yet sent.
pub(crate) struct Pipe {
    file: std::fs::File,
    /// The write end (else the read end).
    pub(crate) writes: bool,
    rbuf: Vec<u8>,
    wbuf: Vec<u8>,
    /// The read end has reached the end.
    eof: bool,
    /// A writer was seen (data or EAGAIN came).
    seen: bool,
    /// The read parked once: on Linux the next look follows an event.
    parked: bool,
}

/// What a try on a pipe came to.
pub(crate) enum Step<T> {
    Ready(T),
    /// It would block: wait for the descriptor to be ready for this.
    Wait(RawFd, Interest),
    Fail(io::Error),
}

/// Whether `path` names a FIFO to read and write through the event loop.
/// Follows symbolic links, as `open` does.
pub(crate) fn is_fifo(path: &str) -> bool {
    !is_shared_alias(path) && std::fs::metadata(path).is_ok_and(|m| m.file_type().is_fifo())
}

/// A path that opens an open file of this process again rather than a
/// new one (macOS): see the module doc.
fn is_shared_alias(path: &str) -> bool {
    cfg!(target_os = "macos")
        && (path.starts_with("/dev/fd/")
            || matches!(path, "/dev/stdin" | "/dev/stdout" | "/dev/stderr"))
}

impl Pipe {
    fn of(file: std::fs::File, writes: bool) -> Pipe {
        Pipe {
            file,
            writes,
            rbuf: Vec::new(),
            wbuf: Vec::new(),
            eof: false,
            seen: false,
            parked: false,
        }
    }

    /// The read end of the FIFO `path`; it opens at once.
    pub(crate) fn open_read(path: &str) -> io::Result<Pipe> {
        let f = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)?;
        Ok(Pipe::of(f, false))
    }

    /// The write end of the FIFO `path` (appending or not makes no
    /// difference to a FIFO), with `perm` as `file::open_writing` gives
    /// it; `None` while no reader has it open.
    pub(crate) fn open_write(path: &str, perm: Option<u32>) -> io::Result<Option<Pipe>> {
        use std::os::unix::fs::PermissionsExt;
        let r = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path);
        match r {
            Ok(f) => {
                if let Some(p) = perm {
                    f.set_permissions(std::fs::Permissions::from_mode(p))?;
                }
                Ok(Some(Pipe::of(f, true)))
            }
            Err(e) if e.raw_os_error() == Some(libc::ENXIO) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub(crate) fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    /// The next line with its `\n`, or what is left at the end (`None`
    /// when nothing is). A read error ends the input, as on a file.
    pub(crate) fn read_line(&mut self) -> Step<Option<Vec<u8>>> {
        loop {
            if let Some(i) = self.rbuf.iter().position(|&b| b == b'\n') {
                return Step::Ready(Some(self.rbuf.drain(..=i).collect()));
            }
            if self.eof {
                let rest = std::mem::take(&mut self.rbuf);
                return Step::Ready((!rest.is_empty()).then_some(rest));
            }
            if let Some(wait) = self.fill() {
                return wait;
            }
        }
    }

    /// Everything up to the end.
    pub(crate) fn read_to_end(&mut self) -> Step<Vec<u8>> {
        loop {
            if self.eof {
                return Step::Ready(std::mem::take(&mut self.rbuf));
            }
            if let Some(wait) = self.fill() {
                return wait;
            }
        }
    }

    /// Reads what is there into the buffer, or says to wait.
    fn fill<T>(&mut self) -> Option<Step<T>> {
        let mut chunk = vec![0; CHUNK];
        match self.file.read(&mut chunk) {
            Ok(0) if self.seen || (cfg!(target_os = "linux") && self.parked) => {
                self.eof = true;
                None
            }
            Ok(0) => Some(self.wait_read()),
            Ok(n) => {
                self.seen = true;
                self.rbuf.extend_from_slice(&chunk[..n]);
                None
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.seen = true;
                Some(self.wait_read())
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => None,
            Err(_) => {
                self.eof = true;
                None
            }
        }
    }

    fn wait_read<T>(&mut self) -> Step<T> {
        self.parked = true;
        Step::Wait(self.fd(), Interest::Read)
    }

    /// Queues `bytes` to be sent (`flush` sends them).
    pub(crate) fn queue(&mut self, bytes: &[u8]) {
        self.wbuf.extend_from_slice(bytes);
    }

    /// Sends what is queued: done, wait for room, or the failure (EPIPE
    /// once the reader is gone), which drops what was queued.
    pub(crate) fn flush(&mut self) -> Step<()> {
        while !self.wbuf.is_empty() {
            match self.file.write(&self.wbuf) {
                Ok(n) => {
                    self.wbuf.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Step::Wait(self.fd(), Interest::Write);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    self.wbuf.clear();
                    return Step::Fail(e);
                }
            }
        }
        Step::Ready(())
    }

    /// Sends what is still queued, waiting if it must, and closes the
    /// FIFO, reporting the close (`file::finish`: `fs.close` and a
    /// dropped handle, on a thread that may block).
    pub(crate) fn finish(self, path: &str) -> io::Result<()> {
        let Pipe { mut file, wbuf, .. } = self;
        let sent = if wbuf.is_empty() {
            Ok(())
        } else {
            set_blocking(&file).and_then(|()| file.write_all(&wbuf))
        };
        let closed = super::file::close_checked(file, path);
        sent.and(closed)
    }
}

/// Clears `O_NONBLOCK`, so that the last bytes are written in full.
fn set_blocking(f: &std::fs::File) -> io::Result<()> {
    let fd = f.as_raw_fd();
    // SAFETY: plain fcntl calls on a descriptor `f` owns.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// When a wait on a pipe's descriptor looks again without an event: on
/// macOS, which reports no hang-up on a FIFO (see the module doc); never
/// elsewhere.
pub(crate) fn relook() -> Option<Instant> {
    cfg!(target_os = "macos").then(|| Instant::now() + RELOOK)
}

/// How a FIFO wait looks for the event loop.
pub(crate) fn io_step(fd: RawFd, interest: Interest) -> crate::vm::IoStep {
    crate::vm::IoStep::Wait(fd, interest, relook())
}

/// The look again of a write end waiting for a reader.
pub(crate) fn until_reader() -> crate::vm::IoStep {
    crate::vm::IoStep::Until(Instant::now() + RELOOK)
}
