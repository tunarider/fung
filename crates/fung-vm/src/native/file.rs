//! File handles: `fs.open` makes one, `fs.write_line`, `fs.close`,
//! `read_line` and `read_lines` use it.
//!
//! A handle is shared by every task that holds a copy, and its input and
//! output run on a pool thread (`nat::offload`), but for a line already
//! in the read buffer or one that fits in the write buffer, which the run
//! thread takes or puts there itself (no system call, so it never
//! blocks). The run thread's exclusive access does not protect a handle:
//! each has its own lock, held for one whole operation (one `write_line`,
//! one line read). Lines written by different tasks therefore never mix.
//!
//! The handle holds no fung value (only its path and the open file), so
//! it may be used and dropped on any thread: an
//! offloaded job owns a clone of the handle's `Arc`. When the last
//! copy of the handle goes open, the runtime's closer flushes the buffer
//! and closes the file on a thread of its own (`vm/closer.rs`, L11): when
//! that happens is not said, and a failure there is reported on standard
//! error. `close` flushes and closes at once and gives the error.

use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::sync::{Mutex, MutexGuard, TryLockError};

use super::pipe::{Pipe, Step};
use crate::vm::Closer;

/// The read and write buffer of a handle: what decides when written lines
/// reach the file before `close`.
const BUF_SIZE: usize = 64 * 1024;

/// An open (or closed) file handle.
pub struct File {
    /// The path as the program gave it (shown in messages and the text).
    pub path: String,
    /// `None` once closed.
    state: Mutex<Option<Io>>,
    /// Where an open handle goes when its last copy drops (L11 B).
    closer: Closer,
}

/// A handle dropped open: its runtime's closer flushes and closes it off
/// the run thread (`vm/closer.rs`).
impl Drop for File {
    fn drop(&mut self) {
        let state = self.state.get_mut().unwrap_or_else(|e| e.into_inner());
        if let Some(io) = state.take() {
            self.closer.submit(std::mem::take(&mut self.path), io);
        }
    }
}

/// What a handle was opened for.
pub(crate) enum Io {
    Read(BufReader<std::fs::File>),
    Write(BufWriter<std::fs::File>),
    /// A FIFO, read and written on the run thread through the event loop
    /// (`pipe.rs`, L11).
    Pipe(Pipe),
}

/// Why a handle operation failed: the `Err` value is made on the run thread
/// (`fs::fault_value`).
#[derive(Debug)]
pub(crate) enum Fault {
    Closed,
    NotReading,
    NotWriting,
    Io(io::Error),
}

impl std::fmt::Debug for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<file {:?}>", self.path)
    }
}

impl File {
    pub(crate) fn new(path: String, io: Io, closer: Closer) -> File {
        File {
            path,
            state: Mutex::new(Some(io)),
            closer,
        }
    }

    /// The handle's state. A panic inside an operation leaves it usable.
    fn lock(&self) -> MutexGuard<'_, Option<Io>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `write_line` under the lock: `line` (with its `\n`) goes into the
    /// buffer as one piece.
    pub(crate) fn write_line(&self, line: &[u8]) -> Result<(), Fault> {
        match &mut *self.lock() {
            None => Err(Fault::Closed),
            Some(Io::Read(_)) => Err(Fault::NotWriting),
            Some(Io::Write(w)) => w.write_all(line).map_err(Fault::Io),
            Some(Io::Pipe(_)) => unreachable!("a FIFO is written on the run thread"),
        }
    }

    /// The next line with its `\n` (the caller trims it), `None` at the
    /// end. A read error ends the input, after the part of a line read
    /// before it.
    pub(crate) fn read_line(&self) -> Result<Option<Vec<u8>>, Fault> {
        match &mut *self.lock() {
            None => Err(Fault::Closed),
            Some(Io::Write(_)) => Err(Fault::NotReading),
            Some(Io::Pipe(_)) => unreachable!("a FIFO is read on the run thread"),
            Some(Io::Read(r)) => {
                let mut line = Vec::new();
                match r.read_until(b'\n', &mut line) {
                    Ok(0) => Ok(None),
                    Ok(_) => Ok(Some(line)),
                    Err(_) if line.is_empty() => Ok(None),
                    Err(_) => Ok(Some(line)),
                }
            }
        }
    }

    /// The handle's state if no other thread holds it now.
    fn try_lock(&self) -> Option<MutexGuard<'_, Option<Io>>> {
        match self.state.try_lock() {
            Ok(g) => Some(g),
            Err(TryLockError::Poisoned(e)) => Some(e.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    }

    /// [`read_line`](Self::read_line) without a system call, for the run
    /// thread: the next line when the read buffer already holds all of
    /// it (and the answer for a closed or write-only handle). `None` when
    /// the read needs the file, or a pool job holds the handle: then the
    /// caller hands the read off.
    pub(crate) fn read_buffered_line(&self) -> Option<Result<Option<Vec<u8>>, Fault>> {
        match &mut *self.try_lock()? {
            None => Some(Err(Fault::Closed)),
            Some(Io::Write(_)) => Some(Err(Fault::NotReading)),
            Some(Io::Pipe(_)) => None,
            Some(Io::Read(r)) => {
                let buf = r.buffer();
                let n = buf.iter().position(|&b| b == b'\n')? + 1;
                let line = buf[..n].to_vec();
                r.consume(n);
                Some(Ok(Some(line)))
            }
        }
    }

    /// [`write_line`](Self::write_line) without a system call, for the
    /// run thread: when `line` fits in the write buffer's free space (so
    /// nothing is flushed), and for a closed or read-only handle. `None`
    /// otherwise: the caller hands the write off.
    pub(crate) fn write_buffered_line(&self, line: &[u8]) -> Option<Result<(), Fault>> {
        match &mut *self.try_lock()? {
            None => Some(Err(Fault::Closed)),
            Some(Io::Read(_)) => Some(Err(Fault::NotWriting)),
            Some(Io::Write(w)) if line.len() < w.capacity() - w.buffer().len() => {
                Some(w.write_all(line).map_err(Fault::Io))
            }
            Some(Io::Write(_) | Io::Pipe(_)) => None,
        }
    }

    /// Whether the handle is a FIFO (read and written on the run thread).
    pub(crate) fn is_pipe(&self) -> bool {
        matches!(&*self.lock(), Some(Io::Pipe(_)))
    }

    /// A FIFO's next line (`pipe.rs`), on the run thread.
    pub(crate) fn pipe_read_line(&self) -> Result<Step<Option<Vec<u8>>>, Fault> {
        match &mut *self.lock() {
            None => Err(Fault::Closed),
            Some(Io::Pipe(p)) if !p.writes => Ok(p.read_line()),
            Some(_) => Err(Fault::NotReading),
        }
    }

    /// Queues a line on a FIFO's write end (`pipe_flush` sends it).
    pub(crate) fn pipe_queue(&self, line: &[u8]) -> Result<(), Fault> {
        match &mut *self.lock() {
            None => Err(Fault::Closed),
            Some(Io::Pipe(p)) if p.writes => {
                p.queue(line);
                Ok(())
            }
            Some(_) => Err(Fault::NotWriting),
        }
    }

    /// Sends what is queued on a FIFO's write end, on the run thread.
    pub(crate) fn pipe_flush(&self) -> Result<Step<()>, Fault> {
        match &mut *self.lock() {
            None => Err(Fault::Closed),
            Some(Io::Pipe(p)) => Ok(p.flush()),
            Some(_) => Err(Fault::NotWriting),
        }
    }

    /// The FIFO's descriptor, if the handle is an open FIFO: the waits on
    /// it are dropped before it closes (`vm::drop_io_waits`).
    pub(crate) fn pipe_fd(&self) -> Option<std::os::fd::RawFd> {
        match &*self.lock() {
            Some(Io::Pipe(p)) => Some(p.fd()),
            _ => None,
        }
    }

    /// Marks the handle closed, flushes what was written and closes the
    /// file, reporting the first failure of the two. A second close is
    /// `Closed`.
    pub(crate) fn close(&self) -> Result<(), Fault> {
        let io = self.lock().take().ok_or(Fault::Closed)?;
        finish(io, &self.path).map_err(Fault::Io)
    }
}

/// Flushes what `io` holds and closes its file: the first failure of the
/// two (L11).
pub(crate) fn finish(io: Io, path: &str) -> io::Result<()> {
    match io {
        Io::Read(r) => close_checked(r.into_inner(), path),
        Io::Write(mut w) => {
            let flushed = w.flush();
            // Closes the file without flushing again.
            let closed = close_checked(w.into_parts().0, path);
            flushed.and(closed)
        }
        Io::Pipe(p) => p.finish(path),
    }
}

/// Closes `f` and says what `close(2)` said (L11): Rust's `File` drops
/// that, and a file system that writes on close (NFS) reports a failed
/// write only there. EINTR counts as closed: on Linux the descriptor is
/// gone already, and closing it again could close another file's.
pub(crate) fn close_checked(f: std::fs::File, path: &str) -> io::Result<()> {
    use std::os::fd::IntoRawFd;
    #[cfg(test)]
    let failing = seam::fails(path);
    #[cfg(not(test))]
    let _ = path;
    #[cfg(test)]
    seam::wait_if_held(path);
    #[cfg(test)]
    if seam::slow(path) {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let fd = f.into_raw_fd();
    // SAFETY: `fd` came out of the `File`, which owned it; nothing else
    // closes it.
    let r = unsafe { libc::close(fd) };
    #[cfg(test)]
    if failing {
        return Err(io::Error::from_raw_os_error(libc::EIO));
    }
    if r == 0 {
        return Ok(());
    }
    match io::Error::last_os_error() {
        e if e.raw_os_error() == Some(libc::EINTR) => Ok(()),
        e => Err(e),
    }
}

/// Makes the close of `path` fail with EIO, as NFS may (tests).
#[cfg(test)]
pub(crate) fn fail_close_of(path: &str) {
    seam::FAILING.lock().unwrap().push(path.to_string());
}

/// Makes every close of `path` take 2 ms (tests: a closer that falls
/// behind the program dropping handles).
#[cfg(test)]
pub(crate) fn slow_close_of(path: &str) {
    seam::SLOW.lock().unwrap().push(path.to_string());
}

/// Makes the close of `path` wait until `release_close_of` (tests: a
/// close that hangs, as on NFS).
#[cfg(test)]
pub(crate) fn hold_close_of(path: &str) {
    seam::HELD.lock().unwrap().push((path.to_string(), false));
}

/// Whether a close of `path` waits in `hold_close_of` now (tests).
#[cfg(test)]
pub(crate) fn close_is_held(path: &str) -> bool {
    seam::HELD
        .lock()
        .unwrap()
        .iter()
        .any(|(p, waiting)| p == path && *waiting)
}

/// Lets the held close of `path` go on (tests).
#[cfg(test)]
pub(crate) fn release_close_of(path: &str) {
    seam::HELD.lock().unwrap().retain(|(p, _)| p != path);
    seam::LET_GO.notify_all();
}

#[cfg(test)]
mod seam {
    use std::sync::{Condvar, Mutex};

    /// The paths whose close fails (`fail_close_of`). Paths, not a flag:
    /// tests run side by side, each in its own directory.
    pub(super) static FAILING: Mutex<Vec<String>> = Mutex::new(Vec::new());

    /// The paths whose close is held, and whether one waits now.
    pub(super) static HELD: Mutex<Vec<(String, bool)>> = Mutex::new(Vec::new());
    pub(super) static LET_GO: Condvar = Condvar::new();

    /// The paths whose close is slow (`slow_close_of`).
    pub(super) static SLOW: Mutex<Vec<String>> = Mutex::new(Vec::new());

    pub(super) fn slow(path: &str) -> bool {
        SLOW.lock().unwrap().iter().any(|p| p == path)
    }

    pub(super) fn fails(path: &str) -> bool {
        FAILING.lock().unwrap().iter().any(|p| p == path)
    }

    pub(super) fn wait_if_held(path: &str) {
        let mut held = HELD.lock().unwrap();
        while let Some(h) = held.iter_mut().find(|(p, _)| p == path) {
            h.1 = true;
            held = LET_GO.wait(held).unwrap();
        }
    }
}

/// The mode a file `fs` creates gets before the umask: 0666, as the
/// shell's `>` and `>>` make one (06 `fs`).
pub(crate) const NEW_FILE_MODE: u32 = 0o666;

/// Opens `path` for `fs.open`: read only; write (read and write,
/// truncated); or append (write only). The file is written with `perm`
/// as [`open_writing`] does.
pub(crate) fn open(path: &str, mode: Mode, perm: Option<u32>) -> io::Result<Io> {
    let mut o = std::fs::OpenOptions::new();
    match mode {
        Mode::Read => {
            let f = o.read(true).open(path)?;
            return Ok(Io::Read(BufReader::with_capacity(BUF_SIZE, f)));
        }
        Mode::Write => o.read(true).write(true).create(true).truncate(true),
        Mode::Append => o.append(true).create(true),
    };
    let f = open_writing(&mut o, path, perm)?;
    Ok(Io::Write(BufWriter::with_capacity(BUF_SIZE, f)))
}

/// Opens `path` with `o` (which may create it) for writing. A new file
/// is made with [`NEW_FILE_MODE`] less the umask, or with `perm` (06,
/// F-14): then, before anything is written, the open file gets exactly
/// `perm`, umask or not, and an existing file too (a leftover temporary
/// file must not keep wider bits). Made with `perm` before the umask, a
/// new file is never wider than `perm` in between.
pub(crate) fn open_writing(
    o: &mut std::fs::OpenOptions,
    path: &str,
    perm: Option<u32>,
) -> io::Result<std::fs::File> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let f = o.mode(perm.unwrap_or(NEW_FILE_MODE)).open(path)?;
    if let Some(p) = perm {
        f.set_permissions(std::fs::Permissions::from_mode(p))?;
    }
    Ok(f)
}

/// `fs.open`'s modes.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Mode {
    Read,
    Write,
    Append,
}
