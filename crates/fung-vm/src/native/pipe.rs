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
//! macOS also misses data: a write larger than the room left in the
//! FIFO's buffer (8 KiB) goes in pieces, and after the first piece
//! neither kqueue nor poll says the read end is ready, though a blocked
//! `read` wakes (`head -c 4000000 /dev/zero | cat > f`: 64 KiB writes).
//! Anonymous pipes, socket pairs and Linux FIFOs are not affected. A
//! read that waited for the event only looked again after `RELOOK`, so
//! such a writer went at one buffer per 50 ms (about 330 KB/s). So the
//! look again after a read that found data comes soon
//! (`FIRST_RELOOK`) and doubles with each look that finds nothing, up to
//! `RELOOK`: a streaming writer is followed closely, an idle one costs a
//! dozen looks and then one every `RELOOK`.
//!
//! On macOS `/dev/fd/N` (and `/dev/stdin` and the like) opens the same
//! open file as descriptor N: making it non-blocking would change fung's
//! own standard input, so those stay on the pool (`is_shared_alias`).

use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
use std::time::{Duration, Instant};

use super::file::os_path;
use crate::vm::Interest;

/// How often a wait no event ends looks again: a write end waiting for a
/// reader, and (macOS) a read that may have reached the end.
pub(crate) const RELOOK: Duration = Duration::from_millis(50);

/// (macOS) The first look again of a read after it found data; each look
/// that finds nothing doubles it, up to `RELOOK` (see the module doc).
const FIRST_RELOOK: Duration = Duration::from_micros(50);

/// The size of one read.
const CHUNK: usize = 64 * 1024;

/// One end of a FIFO, non-blocking, with what was read and not yet
/// taken and what was written and not yet sent.
pub(crate) struct Pipe {
    file: std::fs::File,
    /// The write end (else the read end).
    pub(crate) writes: bool,
    rbuf: Vec<u8>,
    /// How much of `rbuf` holds no `\n`: a line is looked for once.
    scanned: usize,
    /// The characters of `rbuf` while it holds no `\n` (LB29).
    count: super::nat::CharCount,
    wbuf: Vec<u8>,
    /// The read end has reached the end.
    eof: bool,
    /// A writer was seen (data or EAGAIN came).
    seen: bool,
    /// The read parked once: on Linux the next look follows an event.
    parked: bool,
    /// The looks in a row that found nothing to read since data came.
    quiet: u32,
}

/// What a try on a pipe came to.
pub(crate) enum Step<T> {
    Ready(T),
    /// It would block: wait for the descriptor to be ready for this, and
    /// (macOS) look again at the time given in any case.
    Wait(RawFd, Interest, Option<Instant>),
    Fail(io::Error),
}

/// Whether `path` names a FIFO to read and write through the event loop.
/// Follows symbolic links, as `open` does.
pub(crate) fn is_fifo(path: &[u8]) -> bool {
    !is_shared_alias(path)
        && std::fs::metadata(os_path(path)).is_ok_and(|m| m.file_type().is_fifo())
}

/// A path that opens an open file of this process again rather than a
/// new one (macOS): see the module doc.
fn is_shared_alias(path: &[u8]) -> bool {
    cfg!(target_os = "macos")
        && (path.starts_with(b"/dev/fd/")
            || matches!(path, b"/dev/stdin" | b"/dev/stdout" | b"/dev/stderr"))
}

impl Pipe {
    fn of(file: std::fs::File, writes: bool) -> Pipe {
        Pipe {
            file,
            writes,
            rbuf: Vec::new(),
            scanned: 0,
            count: Default::default(),
            wbuf: Vec::new(),
            eof: false,
            seen: false,
            parked: false,
            quiet: 0,
        }
    }

    /// The read end of the FIFO `path`; it opens at once.
    pub(crate) fn open_read(path: &[u8]) -> io::Result<Pipe> {
        let f = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(os_path(path))?;
        Ok(Pipe::of(f, false))
    }

    /// The write end of the FIFO `path` (appending or not makes no
    /// difference to a FIFO), with `perm` as `file::open_writing` gives
    /// it; `None` while no reader has it open.
    pub(crate) fn open_write(path: &[u8], perm: Option<u32>) -> io::Result<Option<Pipe>> {
        use std::os::unix::fs::PermissionsExt;
        let r = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(os_path(path));
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
    /// when nothing is). A read error ends the input, as on a file. A
    /// line past the string limit fails as soon as it passes (LB29): what
    /// was gathered goes, and the line's rest is read on as the next line.
    pub(crate) fn read_line(&mut self) -> Step<Option<Vec<u8>>> {
        loop {
            if let Some(i) = self.rbuf[self.scanned..].iter().position(|&b| b == b'\n') {
                let i = self.scanned + i;
                self.scanned = 0;
                self.count = Default::default();
                return Step::Ready(Some(self.rbuf.drain(..=i).collect()));
            }
            self.scanned = self.rbuf.len();
            if !self.count.within(&self.rbuf) {
                self.drop_read();
                return Step::Fail(super::nat::too_long_error("line"));
            }
            if self.eof {
                let rest = std::mem::take(&mut self.rbuf);
                self.scanned = 0;
                self.count = Default::default();
                return Step::Ready((!rest.is_empty()).then_some(rest));
            }
            if let Some(wait) = self.fill() {
                return wait;
            }
        }
    }

    /// Everything up to the end, or the failure once it passes the
    /// string limit (LB29: an endless writer is not held whole).
    pub(crate) fn read_to_end(&mut self) -> Step<Vec<u8>> {
        loop {
            if !self.count.within(&self.rbuf) {
                self.drop_read();
                return Step::Fail(super::nat::too_long_error("file"));
            }
            if self.eof {
                self.scanned = 0;
                self.count = Default::default();
                return Step::Ready(std::mem::take(&mut self.rbuf));
            }
            if let Some(wait) = self.fill() {
                return wait;
            }
        }
    }

    /// Drops what was read, past the string limit.
    fn drop_read(&mut self) {
        self.rbuf = Vec::new();
        self.scanned = 0;
        self.count = Default::default();
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
                self.quiet = 0;
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
        let after = backoff(self.quiet);
        self.quiet = self.quiet.saturating_add(1);
        Step::Wait(self.fd(), Interest::Read, relook(after))
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
                    return Step::Wait(self.fd(), Interest::Write, relook(RELOOK));
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
    /// FIFO, reporting the close (`file::finish`: `close` and a
    /// dropped handle, on a thread that may block).
    pub(crate) fn finish(self, path: &[u8]) -> io::Result<()> {
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

/// How long a read waits before it looks again without an event, after
/// `quiet` looks in a row that found nothing (see the module doc).
fn backoff(quiet: u32) -> Duration {
    FIRST_RELOOK
        .saturating_mul(1u32.checked_shl(quiet).unwrap_or(u32::MAX))
        .min(RELOOK)
}

/// When a wait on a pipe's descriptor looks again without an event, in
/// `after`: on macOS, which reports no hang-up on a FIFO and misses data
/// (see the module doc); never elsewhere.
fn relook(after: Duration) -> Option<Instant> {
    cfg!(target_os = "macos").then(|| Instant::now() + after)
}

/// How a FIFO wait looks for the event loop.
pub(crate) fn io_step(fd: RawFd, interest: Interest, at: Option<Instant>) -> crate::vm::IoStep {
    crate::vm::IoStep::Wait(fd, interest, at)
}

/// The look again of a write end waiting for a reader.
pub(crate) fn until_reader() -> crate::vm::IoStep {
    crate::vm::IoStep::Until(Instant::now() + RELOOK)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A read that found data looks again soon; each empty look doubles
    /// the wait, up to `RELOOK`, and stays there.
    #[test]
    fn the_look_again_backs_off_from_data() {
        assert_eq!(backoff(0), FIRST_RELOOK);
        assert_eq!(backoff(1), FIRST_RELOOK * 2);
        assert_eq!(backoff(3), FIRST_RELOOK * 8);
        assert_eq!(backoff(10), RELOOK);
        assert_eq!(backoff(40), RELOOK);
        assert_eq!(backoff(u32::MAX), RELOOK);
    }

    /// Data resets the back-off; each empty look raises it.
    #[test]
    fn data_resets_the_back_off() {
        let d = std::env::temp_dir().join(format!("fung-pipe-backoff-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir(&d).unwrap();
        let f = d.join("pipe");
        let made =
            crate::testutil::child::status(std::process::Command::new("mkfifo").arg(&f)).unwrap();
        assert!(made.success());
        let mut r = Pipe::open_read(f.as_os_str().as_encoded_bytes()).unwrap();
        let mut w = std::fs::OpenOptions::new().write(true).open(&f).unwrap();
        for _ in 0..3 {
            assert!(matches!(r.read_line(), Step::Wait(..)));
        }
        assert_eq!(r.quiet, 3);
        w.write_all(b"a\n").unwrap();
        assert!(matches!(r.read_line(), Step::Ready(Some(_))));
        assert_eq!(r.quiet, 0);
        assert!(matches!(r.read_line(), Step::Wait(..)));
        assert_eq!(r.quiet, 1);
        drop((r, w));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
