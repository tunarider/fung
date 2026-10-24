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
//! copy of the handle goes, the buffer is flushed and the file closed
//! (reference counting makes that exact); only `close` reports an error
//! from that flush.

use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::sync::{Mutex, MutexGuard, TryLockError};

/// The read and write buffer of a handle: what decides when written lines
/// reach the file before `close`.
const BUF_SIZE: usize = 64 * 1024;

/// An open (or closed) file handle.
pub struct File {
    /// The path as the program gave it (shown in messages and the text).
    pub path: String,
    /// `None` once closed.
    state: Mutex<Option<Io>>,
}

/// What a handle was opened for.
pub(crate) enum Io {
    Read(BufReader<std::fs::File>),
    Write(BufWriter<std::fs::File>),
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
    pub(crate) fn new(path: String, io: Io) -> File {
        File {
            path,
            state: Mutex::new(Some(io)),
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
        }
    }

    /// The next line with its `\n` (the caller trims it), `None` at the
    /// end. A read error ends the input, after the part of a line read
    /// before it.
    pub(crate) fn read_line(&self) -> Result<Option<Vec<u8>>, Fault> {
        match &mut *self.lock() {
            None => Err(Fault::Closed),
            Some(Io::Write(_)) => Err(Fault::NotReading),
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
            Some(Io::Write(_)) => None,
        }
    }

    /// Marks the handle closed, flushes what was written and closes the
    /// file. A second close is `Closed`.
    pub(crate) fn close(&self) -> Result<(), Fault> {
        let mut st = self.lock();
        match st.take() {
            None => Err(Fault::Closed),
            Some(Io::Read(r)) => {
                drop(r);
                Ok(())
            }
            Some(Io::Write(mut w)) => {
                let r = w.flush();
                // Closes the file without flushing again.
                drop(w.into_parts());
                r.map_err(Fault::Io)
            }
        }
    }
}

/// The mode a file `fs` creates gets before the umask: 0666, as the
/// shell's `>` and `>>` make one (06 `fs`).
pub(crate) const NEW_FILE_MODE: u32 = 0o666;

/// Opens `path` for `fs.open`: read only; write (read and write,
/// truncated); or append (write only). A new file is made with
/// [`NEW_FILE_MODE`].
pub(crate) fn open(path: &str, mode: Mode) -> io::Result<Io> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut o = std::fs::OpenOptions::new();
    match mode {
        Mode::Read => o.read(true),
        Mode::Write => o
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .mode(NEW_FILE_MODE),
        Mode::Append => o.append(true).create(true).mode(NEW_FILE_MODE),
    };
    let f = o.open(path)?;
    Ok(match mode {
        Mode::Read => Io::Read(BufReader::with_capacity(BUF_SIZE, f)),
        Mode::Write | Mode::Append => Io::Write(BufWriter::with_capacity(BUF_SIZE, f)),
    })
}

/// `fs.open`'s modes.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Mode {
    Read,
    Write,
    Append,
}
