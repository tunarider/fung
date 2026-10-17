//! File handles (Go `value.File` and the handle half of
//! `internal/native/fs.go`): `fs.open` makes one, `fs.write_line`,
//! `fs.close`, `read_line` and `read_lines` use it.
//!
//! A handle is shared by every task that holds a copy, and its input and
//! output run without the run token (`outside`), so the token does not
//! protect it: each handle has its own lock, held for one whole operation
//! (one `write_line`, one line read), as Go's handle mutex. Lines written
//! by different tasks therefore never mix.
//!
//! The handle holds no fung value (only its path and the open file), so
//! it may be used and dropped without the token: a closure passed to
//! `outside` borrows `&File`, never the `Rc` around it. When the last
//! copy of the handle goes, the file is closed (reference counting makes
//! that exact; Go closes it whenever its garbage collector finds it). As
//! in Go, data still in the write buffer is not written then: only
//! `close` flushes.

use std::io::{self, BufRead, BufReader, Write};
use std::sync::{Mutex, MutexGuard};

/// Go `bufio`'s default buffer size: what decides when written lines
/// reach the file before `close`.
const BUF_SIZE: usize = 4096;

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
    Write(GoWriter<std::fs::File>),
}

/// Why a handle operation failed: the `Err` value is made with the token
/// back (`fs::fault_value`).
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

    /// The handle's state. A panic inside an operation leaves it usable
    /// (Go's mutex has no poisoning either).
    fn lock(&self) -> MutexGuard<'_, Option<Io>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Go `fsWriteLineFn` under the lock: `line` (with its `\n`) goes
    /// into the buffer as one piece.
    pub(crate) fn write_line(&self, line: &[u8]) -> Result<(), Fault> {
        match &mut *self.lock() {
            None => Err(Fault::Closed),
            Some(Io::Read(_)) => Err(Fault::NotWriting),
            Some(Io::Write(w)) => w.write_str(line).map_err(Fault::Io),
        }
    }

    /// Go `nextLine` on a handle: the next line with its `\n` (the caller
    /// trims it), `None` at the end. A read error ends the input, after
    /// the part of a line read before it (Go `ReadString`).
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

    /// Go `fsCloseFn`: marks the handle closed, flushes what was written
    /// and closes the file. A second close is `Closed`.
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
                drop(w);
                r.map_err(Fault::Io)
            }
        }
    }
}

/// Opens `path` the way Go's `fsOpenFn` does: `os.Open` (read only),
/// `os.Create` (read and write, created with 0666 before the umask,
/// truncated), or append (write only, created with 0644).
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
            .mode(0o666),
        Mode::Append => o.append(true).create(true).mode(0o644),
    };
    let f = o.open(path)?;
    Ok(match mode {
        Mode::Read => Io::Read(BufReader::with_capacity(BUF_SIZE, f)),
        Mode::Write | Mode::Append => Io::Write(GoWriter::new(f)),
    })
}

/// `fs.open`'s modes.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Mode {
    Read,
    Write,
    Append,
}

/// Go's `bufio.Writer` over a file, step by step, so written lines reach
/// the file at the same moments as in Go (a program that reads the file
/// before `close` sees the same bytes): a 4096-byte buffer; a write that
/// does not fit fills the buffer and flushes it, or, when the buffer is
/// empty, goes to the file directly; the first write error sticks.
pub(crate) struct GoWriter<W> {
    w: W,
    buf: Vec<u8>,
    err: Option<io::Error>,
}

impl<W: Write> GoWriter<W> {
    pub(crate) fn new(w: W) -> GoWriter<W> {
        GoWriter {
            w,
            buf: Vec::with_capacity(BUF_SIZE),
            err: None,
        }
    }

    fn available(&self) -> usize {
        BUF_SIZE - self.buf.len()
    }

    /// Go `Writer.WriteString`.
    pub(crate) fn write_str(&mut self, mut s: &[u8]) -> io::Result<()> {
        while s.len() > self.available() && self.err.is_none() {
            let n = if self.buf.is_empty() {
                let (n, e) = write_counted(&mut self.w, s);
                self.err = e;
                n
            } else {
                let n = self.available();
                self.buf.extend_from_slice(&s[..n]);
                let _ = self.flush();
                n
            };
            s = &s[n..];
        }
        if let Some(e) = &self.err {
            return Err(copy_err(e));
        }
        self.buf.extend_from_slice(s);
        Ok(())
    }

    /// Go `Writer.Flush`: what was not written stays in the buffer.
    pub(crate) fn flush(&mut self) -> io::Result<()> {
        if let Some(e) = &self.err {
            return Err(copy_err(e));
        }
        if self.buf.is_empty() {
            return Ok(());
        }
        let (n, e) = write_counted(&mut self.w, &self.buf);
        self.buf.drain(..n);
        match e {
            Some(e) => {
                let r = copy_err(&e);
                self.err = Some(e);
                Err(r)
            }
            None => Ok(()),
        }
    }

    #[cfg(test)]
    pub(crate) fn inner(&self) -> &W {
        &self.w
    }
}

/// Go `os.File.Write`: writes until all of `b` is written or an error
/// (an interrupted call is repeated); the count is what was written.
fn write_counted(w: &mut impl Write, b: &[u8]) -> (usize, Option<io::Error>) {
    let mut n = 0;
    while n < b.len() {
        match w.write(&b[n..]) {
            Ok(0) => return (n, Some(io::Error::from(io::ErrorKind::WriteZero))),
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return (n, Some(e)),
        }
    }
    (n, None)
}

/// A copy of a stuck error (`io::Error` is not `Clone`).
fn copy_err(e: &io::Error) -> io::Error {
    match e.raw_os_error() {
        Some(code) => io::Error::from_raw_os_error(code),
        None => io::Error::new(e.kind(), e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A writer that records each write and fails once `fail_after`
    /// bytes have gone through.
    #[derive(Default)]
    struct Rec {
        writes: Vec<usize>,
        total: usize,
        fail_after: Option<usize>,
    }

    impl Write for Rec {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if let Some(limit) = self.fail_after {
                if self.total >= limit {
                    return Err(io::Error::from_raw_os_error(28));
                }
                let k = b.len().min(limit - self.total);
                self.writes.push(k);
                self.total += k;
                return Ok(k);
            }
            self.writes.push(b.len());
            self.total += b.len();
            Ok(b.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// The flush points of Go's `bufio.Writer.WriteString` (the same
    /// sequence `native/fs_tests.rs` compares with the Go binary through
    /// file sizes).
    #[test]
    fn writes_reach_the_writer_when_go_s_do() {
        let mut w = GoWriter::new(Rec::default());
        w.write_str(&[b'a'; 4095]).unwrap();
        assert_eq!(w.inner().writes, Vec::<usize>::new());
        w.write_str(b"b\n").unwrap();
        assert_eq!(w.inner().writes, vec![4096]);
        w.write_str(&[b'c'; 5001]).unwrap();
        assert_eq!(w.inner().writes, vec![4096, 4096]);
        w.write_str(&[b'd'; 9001]).unwrap();
        assert_eq!(w.inner().writes, vec![4096, 4096, 4096, 5811]);
        w.write_str(&[b'e'; 10001]).unwrap();
        assert_eq!(w.inner().writes, vec![4096, 4096, 4096, 5811, 10001]);
        w.write_str(b"\n").unwrap();
        w.flush().unwrap();
        assert_eq!(w.inner().total, 4095 + 2 + 5001 + 9001 + 10001 + 1);
        w.flush().unwrap();
        assert_eq!(w.inner().writes.len(), 6);
    }

    /// The first write error sticks: every later write and flush returns
    /// it, and the unwritten part stays buffered.
    #[test]
    fn a_write_error_sticks() {
        let mut w = GoWriter::new(Rec {
            fail_after: Some(2000),
            ..Rec::default()
        });
        w.write_str(&[b'a'; 4000]).unwrap();
        let e = w.write_str(&[b'b'; 4000]).unwrap_err();
        assert_eq!(e.raw_os_error(), Some(28));
        assert_eq!((w.inner().total, w.buf.len()), (2000, 2096));
        let e = w.write_str(b"x").unwrap_err();
        assert_eq!(e.raw_os_error(), Some(28));
        let e = w.flush().unwrap_err();
        assert_eq!(e.raw_os_error(), Some(28));
        assert_eq!(w.inner().total, 2000);
    }
}
