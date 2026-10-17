//! `_native.net` (stage 4d, Rust only): the TCP primitives the Rust
//! `http` module (`extras/rust/http.fung`) is written over. User code
//! cannot import it (E0310); only embedded extras files can.
//!
//! **Values.** A connection (`Value::Conn`, printed `<conn>`, `type_of`
//! `'conn`) and a listener (`Value::Listener`, `<listener>`,
//! `'listener`) work like file handles (`native/file.rs`): shared by
//! every task holding a copy, compared by identity, holding no fung
//! value, used without the run token, and closed when the last copy
//! goes (the peer then sees the end of the stream).
//!
//! **Locks.** A connection has a read lock (the read-ahead buffer, held
//! for one whole `read_line`, `read_bytes` or `read_all`) and a write
//! lock (held for one whole `write`), so one read and one write can run
//! at once, and lines from two readers never mix. **One reader and one
//! writer at a time per connection**: a second concurrent reader (or
//! writer) waits for the first on the lock, and cannot be cancelled
//! while it waits there (HTTP needs one of each). The socket itself sits
//! in a slot of its own, locked only to take a reference: `close` never
//! waits for a read or a write. It empties the slot and shuts the socket
//! down, which wakes a read or write blocked on it (Go's `Close` does the
//! same); the descriptor closes when the operation that held it lets go.
//! A listener's `close` wakes a blocked `accept` through a pipe of the
//! listener's own (`shutdown` does not wake `accept` on macOS).
//!
//! **Waiting.** Sockets are non-blocking. A call that would block waits
//! outside the token in a `Watch` (`vm/watch.rs`), which a cancellation
//! of the task interrupts: `accept`, the reads, `write` and `connect`
//! (its name lookup included) then end with E0701. A name lookup runs
//! on a thread of its own, since `getaddrinfo` cannot be interrupted: a
//! cancelled `connect` returns at once and the thread ends when the
//! resolver answers (it holds no fung value and no socket).
//!
//! **Errors** are `Err({kind, message})`: `'timeout` when a deadline
//! passed, `'limit` when a read went past its limit, `'network` for the
//! rest. Messages are Go's `net` texts (`dial tcp 127.0.0.1:1: connect:
//! connection refused`), errno wording from `oserr`; the `http` module
//! wraps them (`cannot request ...: <message>`). Copying Go's texts is a
//! transitional parity measure (open-questions "Go 동등을 위한 과도기
//! 잔가지").
//!
//! **Strings.** What is read is decoded as UTF-8, a byte that is not
//! UTF-8 becoming U+FFFD (as for files); `byte_len` gives the UTF-8 size
//! a string is sent with.
//!
//! **A server's end** (Task 5, `hold`): a listener counts the connections
//! accepted from it that are still open (closed, or their last copy
//! gone). `hold(l, grace)` waits until its task is cancelled, then closes
//! `l` and waits, at most `grace`, until that count is zero, and ends
//! with E0701: Go's `Server.Shutdown`, which `http.serve` runs when its
//! task is cancelled. Its task waits outside the token the whole time,
//! so the tasks answering those connections run meanwhile.

use std::io::{self, Read, Write};
use std::net::{
    IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, SocketAddrV6, TcpListener, TcpStream,
};
use std::os::fd::{AsRawFd, RawFd};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use fung_front::source::decode_go;

use super::nat::{self, native, opt, param};
use super::oserr;
use super::sock;
use crate::value::{Lit, Module, Native, Value};
use crate::vm::{Interest, Stop, Task, Waited, Watch, outside_watched};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The `_native.net` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.get_or_init(|| {
        vec![
            (
                "connect",
                native(
                    "connect",
                    connect_fn,
                    vec![
                        param("host"),
                        param("port"),
                        opt("tls", Lit::Bool(false)),
                        opt("timeout", Lit::Dur(0)),
                    ],
                ),
            ),
            ("listen", native("listen", listen_fn, vec![param("addr")])),
            (
                "local_addr",
                native("local_addr", local_addr_fn, vec![param("x")]),
            ),
            ("accept", native("accept", accept_fn, vec![param("l")])),
            (
                "read_line",
                native("read_line", read_line_fn, vec![param("c"), param("limit")]),
            ),
            (
                "read_raw_line",
                native(
                    "read_raw_line",
                    read_raw_line_fn,
                    vec![param("c"), param("limit")],
                ),
            ),
            (
                "read_bytes",
                native("read_bytes", read_bytes_fn, vec![param("c"), param("n")]),
            ),
            (
                "read_all",
                native("read_all", read_all_fn, vec![param("c"), param("limit")]),
            ),
            (
                "write",
                native("write", write_fn, vec![param("c"), param("s")]),
            ),
            ("close", native("close", close_fn, vec![param("x")])),
            (
                "set_deadline",
                native(
                    "set_deadline",
                    set_deadline_fn,
                    vec![param("c"), param("d")],
                ),
            ),
            (
                "byte_len",
                native("byte_len", byte_len_fn, vec![param("s")]),
            ),
            (
                "read_bytes_into",
                native(
                    "read_bytes_into",
                    read_bytes_into_fn,
                    vec![param("c"), param("n"), param("cap")],
                ),
            ),
            (
                "take_body",
                native("take_body", take_body_fn, vec![param("c")]),
            ),
            (
                "utf8_bytes",
                native("utf8_bytes", utf8_bytes_fn, vec![param("s")]),
            ),
            (
                "start_tls",
                native(
                    "start_tls",
                    start_tls_fn,
                    vec![param("c"), param("host"), opt("timeout", Lit::Dur(0))],
                ),
            ),
            (
                "need_str_map",
                native(
                    "need_str_map",
                    need_str_map_fn,
                    vec![param("name"), param("m")],
                ),
            ),
            (
                "hold",
                native("hold", hold_fn, vec![param("l"), param("grace")]),
            ),
            (
                "close_on_cancel",
                native("close_on_cancel", close_on_cancel_fn, vec![param("l")]),
            ),
            (
                "utf8_string",
                native("utf8_string", utf8_string_fn, vec![param("bs")]),
            ),
        ]
    });
    let mut m = Module::new("net", "_native.net");
    for (name, n) in natives {
        m.set(name, Value::Native(n));
    }
    Rc::new(m)
}

/// How much one read takes from the socket at most.
const CHUNK: usize = 16 * 1024;

/// Counters for the leak checks and the state-ordered tests.
static SOCKETS: AtomicUsize = AtomicUsize::new(0);
static LOOKUPS: AtomicUsize = AtomicUsize::new(0);
static WAITING: AtomicUsize = AtomicUsize::new(0);
static READ_WAITS: AtomicUsize = AtomicUsize::new(0);
static WRITE_WAITS: AtomicUsize = AtomicUsize::new(0);

/// Why an operation failed, made into an `Err` value with the token
/// back (`fail_value`).
#[derive(Debug)]
enum Fail {
    Network(String),
    Timeout(String),
    Limit(String),
    /// A wait ended as cancelled: `outside_watched` unwinds instead.
    Cancelled,
}

fn fail_value(f: Fail) -> Value {
    let (kind, msg) = match f {
        Fail::Network(m) => ("network", m),
        Fail::Timeout(m) => ("timeout", m),
        Fail::Limit(m) => ("limit", m),
        // Never seen: a cancelled wait makes `outside_watched` unwind.
        Fail::Cancelled => ("network", "operation was canceled".to_string()),
    };
    nat::err_value(kind, &msg, Vec::new())
}

/// `Ok(v)`, or the failure as a value.
fn result(r: Result<Value, Fail>) -> Value {
    match r {
        Ok(v) => nat::ok(v),
        Err(f) => fail_value(f),
    }
}

/// Waits in `w` for `fds` (the first one decides which count a test
/// sees), until `deadline`.
fn wait_io(
    w: &mut Watch,
    fds: &[(RawFd, Interest)],
    deadline: Option<Instant>,
) -> Result<Waited, Fail> {
    match fds[0].1 {
        Interest::Read => READ_WAITS.fetch_add(1, Ordering::SeqCst),
        Interest::Write => WRITE_WAITS.fetch_add(1, Ordering::SeqCst),
    };
    WAITING.fetch_add(1, Ordering::SeqCst);
    let r = w.wait(fds, deadline);
    WAITING.fetch_sub(1, Ordering::SeqCst);
    match r {
        Ok(Waited::Cancelled) => Err(Fail::Cancelled),
        Ok(waited) => Ok(waited),
        Err(e) => Err(Fail::Network(format!("poll: {}", oserr::reason(&e)))),
    }
}

/// A socket, counted while it is open.
struct Counted<T>(T);

impl<T> Counted<T> {
    fn new(x: T) -> Counted<T> {
        SOCKETS.fetch_add(1, Ordering::SeqCst);
        Counted(x)
    }
}

impl<T> Drop for Counted<T> {
    fn drop(&mut self) {
        SOCKETS.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The lock if no one holds it (`None` if someone does).
fn try_lock<T>(m: &Mutex<T>) -> Option<MutexGuard<'_, T>> {
    match m.try_lock() {
        Ok(g) => Some(g),
        Err(std::sync::TryLockError::Poisoned(p)) => Some(p.into_inner()),
        Err(std::sync::TryLockError::WouldBlock) => None,
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // The state is plain data that no panic leaves half-changed.
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a connection's read lock guards.
#[derive(Default)]
struct ReadState {
    /// Bytes read from the socket and not taken yet.
    buf: Vec<u8>,
    /// Raw bytes `read_bytes_into` gathered for `take_body`.
    body: Vec<u8>,
}

/// A TCP connection.
pub struct Conn {
    /// `None` once closed. Locked only to take or drop a reference.
    sock: Mutex<Option<Arc<Counted<TcpStream>>>>,
    /// The read-ahead and body buffers, locked for a whole read.
    rd: Mutex<ReadState>,
    /// Locked for a whole write.
    wr: Mutex<()>,
    /// When reads and writes time out (`set_deadline`).
    deadline: Mutex<Option<Instant>>,
    /// The rustls state of a TLS connection (`tls.rs`), locked only for
    /// steps that do not wait.
    tls: Option<Mutex<tls::Session>>,
    /// `local->peer` as Go's messages show it.
    name: String,
    /// The local address.
    local: String,
    /// For a connection a listener accepted: its place in the
    /// listener's count of open connections, given up when the
    /// connection closes or goes (`hold`).
    track: Mutex<Option<Track>>,
}

impl std::fmt::Debug for Conn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<conn {}>", self.name)
    }
}

impl Conn {
    fn new(s: TcpStream, peer: SocketAddr) -> Conn {
        let local = s
            .local_addr()
            .map(|a| sock::addr_text(&a))
            .unwrap_or_default();
        Conn {
            name: format!("{local}->{}", sock::addr_text(&peer)),
            local,
            sock: Mutex::new(Some(Arc::new(Counted::new(s)))),
            rd: Mutex::new(ReadState::default()),
            wr: Mutex::new(()),
            deadline: Mutex::new(None),
            tls: None,
            track: Mutex::new(None),
        }
    }

    fn closed(&self, op: &str) -> Fail {
        Fail::Network(format!(
            "{op} tcp {}: use of closed network connection",
            self.name
        ))
    }

    fn timed_out(&self, op: &str) -> Fail {
        Fail::Timeout(format!("{op} tcp {}: i/o timeout", self.name))
    }

    fn is_closed(&self) -> bool {
        lock(&self.sock).is_none()
    }

    /// The socket for one operation, or the closed error.
    fn sock(&self, op: &str) -> Result<Arc<Counted<TcpStream>>, Fail> {
        lock(&self.sock).clone().ok_or_else(|| self.closed(op))
    }

    fn deadline(&self) -> Option<Instant> {
        *lock(&self.deadline)
    }

    /// Reads what the socket has into `buf` (waiting for something):
    /// the count, 0 at the end of the stream.
    fn fill(&self, w: &mut Watch, s: &TcpStream, buf: &mut Vec<u8>) -> Result<usize, Fail> {
        if let Some(t) = &self.tls {
            return tls::fill(self, t, w, s, buf);
        }
        let mut chunk = [0u8; CHUNK];
        loop {
            // also between reads that never wait (a fast peer)
            if w.cancelled() {
                return Err(Fail::Cancelled);
            }
            if self.is_closed() {
                return Err(self.closed("read"));
            }
            let deadline = self.deadline();
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return Err(self.timed_out("read"));
            }
            match (&*s).read(&mut chunk) {
                // a close from another task shuts the socket down: that
                // end of stream is the close
                Ok(0) if self.is_closed() => return Err(self.closed("read")),
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    return Ok(n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if wait_io(w, &[(s.as_raw_fd(), Interest::Read)], deadline)? == Waited::TimedOut
                    {
                        return Err(self.timed_out("read"));
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    return Err(Fail::Network(format!(
                        "read tcp {}: read: {}",
                        self.name,
                        oserr::reason(&e)
                    )));
                }
            }
        }
    }

    /// The next line without its `\n` or `\r\n`; `None` at the end of
    /// the stream. A last line without an end is returned as it is. A
    /// line longer than `limit` bytes (its end not counted) is a
    /// `'limit` error, found without buffering more than `limit` + 2
    /// bytes of it (past `limit` + 1 bytes, or `limit` + 1 bytes not
    /// ending in `\r`, no line end can make it fit).
    fn read_line(&self, w: &mut Watch, limit: usize) -> Result<Option<Vec<u8>>, Fail> {
        let mut st = lock(&self.rd);
        let buf = &mut st.buf;
        let s = self.sock("read")?;
        let too_long = || Fail::Limit(format!("line longer than {limit} bytes"));
        let mut scanned = 0;
        loop {
            if let Some(i) = buf[scanned..].iter().position(|&b| b == b'\n') {
                let mut line: Vec<u8> = buf.drain(..=scanned + i).collect();
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                if line.len() > limit {
                    return Err(too_long());
                }
                return Ok(Some(line));
            }
            scanned = buf.len();
            if buf.len() > limit + 1 || (buf.len() == limit + 1 && buf[limit] != b'\r') {
                return Err(too_long());
            }
            if self.fill(w, &s.0, buf)? == 0 {
                if buf.is_empty() {
                    return Ok(None);
                }
                // `limit` + 1 bytes ending in `\r` passed the check
                // above, waiting for a `\n` that did not come
                if buf.len() > limit {
                    return Err(too_long());
                }
                return Ok(Some(std::mem::take(buf)));
            }
        }
    }

    /// The next line with its `\n` (Go `bufio.Reader.ReadSlice('\n')`
    /// over a `limit`-byte buffer, as Go's chunked reader reads a size
    /// line): no `\n` within `limit` bytes is a `'limit` error, the end of
    /// the stream first `EOF`. The caller sees which line end came.
    fn read_raw_line(&self, w: &mut Watch, limit: usize) -> Result<Vec<u8>, Fail> {
        let mut st = lock(&self.rd);
        let buf = &mut st.buf;
        let s = self.sock("read")?;
        let mut scanned = 0;
        loop {
            let end = buf.len().min(limit);
            if let Some(i) = buf[scanned.min(end)..end].iter().position(|&b| b == b'\n') {
                return Ok(buf.drain(..=scanned + i).collect());
            }
            if buf.len() >= limit {
                return Err(Fail::Limit(format!("line longer than {limit} bytes")));
            }
            scanned = buf.len();
            if self.fill(w, &s.0, buf)? == 0 {
                return Err(Fail::Network("EOF".into()));
            }
        }
    }

    /// Exactly `n` bytes; Go's `io.ReadFull` errors when the stream ends
    /// first (`EOF` with nothing read, `unexpected EOF` after a part).
    fn read_bytes(&self, w: &mut Watch, n: usize) -> Result<Vec<u8>, Fail> {
        let mut st = lock(&self.rd);
        let buf = &mut st.buf;
        let s = self.sock("read")?;
        while buf.len() < n {
            let before = buf.len();
            if self.fill(w, &s.0, buf)? == 0 {
                let msg = if before == 0 { "EOF" } else { "unexpected EOF" };
                return Err(Fail::Network(msg.into()));
            }
        }
        Ok(buf.drain(..n).collect())
    }

    /// Everything up to the end of the stream; more than `limit` bytes
    /// is a `'limit` error (what was read stays unread).
    fn read_all(&self, w: &mut Watch, limit: usize) -> Result<Vec<u8>, Fail> {
        let mut st = lock(&self.rd);
        let buf = &mut st.buf;
        let s = self.sock("read")?;
        loop {
            if buf.len() > limit {
                return Err(Fail::Limit(format!("more than {limit} bytes")));
            }
            if self.fill(w, &s.0, buf)? == 0 {
                return Ok(std::mem::take(buf));
            }
        }
    }

    /// `read_bytes`, but the `n` bytes go raw into the body buffer, to be
    /// decoded once by `take_body` (a character split between two reads
    /// comes out whole). If the buffer would then hold more than `cap`
    /// bytes, nothing is read: `Err('limit, "body longer than <cap>
    /// bytes")`. After any error the body buffer keeps what earlier calls
    /// gathered; `take_body` empties it (the `http` code does so on every
    /// error path, `_drop`).
    fn read_bytes_into(&self, w: &mut Watch, n: usize, cap: usize) -> Result<(), Fail> {
        let mut st = lock(&self.rd);
        if st.body.len().saturating_add(n) > cap {
            return Err(Fail::Limit(format!("body longer than {cap} bytes")));
        }
        let s = self.sock("read")?;
        let st = &mut *st;
        while st.buf.len() < n {
            let before = st.buf.len();
            if self.fill(w, &s.0, &mut st.buf)? == 0 {
                let msg = if before == 0 { "EOF" } else { "unexpected EOF" };
                return Err(Fail::Network(msg.into()));
            }
        }
        st.body.extend(st.buf.drain(..n));
        Ok(())
    }

    /// The body buffer's bytes, emptied.
    fn take_body(&self) -> Vec<u8> {
        std::mem::take(&mut lock(&self.rd).body)
    }

    /// Writes all of `b`, waiting while the socket's buffer is full.
    fn write(&self, w: &mut Watch, b: &[u8]) -> Result<(), Fail> {
        let _g = lock(&self.wr);
        let s = self.sock("write")?;
        if let Some(t) = &self.tls {
            return tls::write(self, t, w, &s.0, b);
        }
        let mut off = 0;
        while off < b.len() {
            if w.cancelled() {
                return Err(Fail::Cancelled);
            }
            if self.is_closed() {
                return Err(self.closed("write"));
            }
            let deadline = self.deadline();
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return Err(self.timed_out("write"));
            }
            match (&s.0).write(&b[off..]) {
                Ok(n) => off += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if wait_io(w, &[(s.0.as_raw_fd(), Interest::Write)], deadline)?
                        == Waited::TimedOut
                    {
                        return Err(self.timed_out("write"));
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) if self.is_closed() => return Err(self.closed("write")),
                Err(e) => {
                    return Err(Fail::Network(format!(
                        "write tcp {}: write: {}",
                        self.name,
                        oserr::reason(&e)
                    )));
                }
            }
        }
        Ok(())
    }

    /// A new connection over this one's socket, which this one gives up
    /// (it reads as closed, and dropping it ends nothing). The caller
    /// must own the connection alone: while another task reads or writes
    /// it, this fails at once (`tls: the connection is in use`). What was
    /// read ahead is dropped on purpose: `start_tls` uses this once a
    /// proxy has answered a CONNECT, a TLS server says nothing until
    /// spoken to, and Go discards its bufio reader there the same way. A
    /// TLS connection cannot carry another.
    fn take_over(&self) -> Result<Conn, Fail> {
        if self.tls.is_some() {
            return Err(Fail::Network(
                "tls: a TLS connection cannot carry another".into(),
            ));
        }
        let in_use = || Fail::Network("tls: the connection is in use".into());
        let (Some(_rd), Some(_wr)) = (try_lock(&self.rd), try_lock(&self.wr)) else {
            return Err(in_use());
        };
        let s = lock(&self.sock).take().ok_or_else(|| self.closed("read"))?;
        Ok(Conn {
            sock: Mutex::new(Some(s)),
            rd: Mutex::new(ReadState::default()),
            wr: Mutex::new(()),
            deadline: Mutex::new(None),
            tls: None,
            name: self.name.clone(),
            local: self.local.clone(),
            track: Mutex::new(lock(&self.track).take()),
        })
    }

    /// Go `Close`: shuts the socket down (waking a blocked read or write
    /// of another task, which then reports the close) and lets it go; a
    /// second close is an error.
    fn close(&self) -> Result<(), Fail> {
        let s = lock(&self.sock)
            .take()
            .ok_or_else(|| self.closed("close"))?;
        if let Some(t) = &self.tls {
            tls::close_notify(t, &s.0);
        }
        let _ = s.0.shutdown(Shutdown::Both);
        drop(lock(&self.track).take());
        Ok(())
    }

    /// Go `SetDeadline(now + d)`: reads and writes after `d` time out; 0
    /// removes the deadline, a negative `d` is already past.
    fn set_deadline(&self, d: i64) -> Result<(), Fail> {
        if self.is_closed() {
            // Go names only the local address here
            return Err(Fail::Network(format!(
                "set tcp {}: use of closed network connection",
                self.local
            )));
        }
        let now = Instant::now();
        *lock(&self.deadline) = match d {
            0 => None,
            d if d < 0 => Some(now),
            // a deadline too far to hold is none
            d => now.checked_add(Duration::from_nanos(d as u64)),
        };
        Ok(())
    }
}

/// A listening socket.
pub struct Listener {
    /// `None` once closed. Locked only to take or drop a reference (a
    /// leaf lock: nothing else is locked while it is held, so a
    /// cancellation hook may take it, `close_on_cancel`).
    sock: Arc<Mutex<Option<Arc<Counted<TcpListener>>>>>,
    /// `close` writes a byte into the pipe, waking a blocked `accept`.
    closer: (io::PipeReader, Arc<io::PipeWriter>),
    /// The address, as Go's `Addr().String()`.
    addr: String,
    /// The connections accepted from it that are still open (`hold`).
    open: Arc<Tracker>,
}

/// A listener's count of open connections, and the signal `hold` waits
/// on for it to reach zero.
#[derive(Default)]
struct Tracker {
    open: Mutex<usize>,
    closed: Condvar,
}

impl Tracker {
    /// Waits until no connection is open, at most until `until`.
    fn wait_idle(&self, until: Instant) {
        let mut n = lock(&self.open);
        while *n > 0 {
            let now = Instant::now();
            if now >= until {
                return;
            }
            n = self
                .closed
                .wait_timeout(n, until - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// One open connection in a listener's count; dropping it counts the
/// connection closed.
struct Track(Arc<Tracker>);

impl Track {
    fn new(t: &Arc<Tracker>) -> Track {
        *lock(&t.open) += 1;
        Track(t.clone())
    }
}

impl Drop for Track {
    fn drop(&mut self) {
        *lock(&self.0.open) -= 1;
        self.0.closed.notify_all();
    }
}

impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<listener {}>", self.addr)
    }
}

impl Listener {
    fn closed(&self, op: &str) -> Fail {
        Fail::Network(format!(
            "{op} tcp {}: use of closed network connection",
            self.addr
        ))
    }

    fn is_closed(&self) -> bool {
        lock(&self.sock).is_none()
    }

    /// The next connection; waits until one comes, the listener is
    /// closed or the task cancelled.
    fn accept(&self, w: &mut Watch) -> Result<Conn, Fail> {
        let l = lock(&self.sock)
            .clone()
            .ok_or_else(|| self.closed("accept"))?;
        loop {
            if self.is_closed() {
                return Err(self.closed("accept"));
            }
            match l.0.accept() {
                Ok((s, peer)) => {
                    let setup = s.set_nonblocking(true).and_then(|_| sock::no_sigpipe(&s));
                    if let Err(e) = setup {
                        return Err(Fail::Network(format!(
                            "accept tcp {}: {}",
                            self.addr,
                            oserr::reason(&e)
                        )));
                    }
                    let c = Conn::new(s, peer);
                    *lock(&c.track) = Some(Track::new(&self.open));
                    return Ok(c);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let fds = [
                        (l.0.as_raw_fd(), Interest::Read),
                        (self.closer.0.as_raw_fd(), Interest::Read),
                    ];
                    wait_io(w, &fds, None)?;
                }
                // Go retries these too: the client gave up, or a signal.
                Err(e)
                    if e.kind() == io::ErrorKind::Interrupted
                        || e.raw_os_error() == Some(libc::ECONNABORTED) => {}
                Err(e) => {
                    let call = if cfg!(target_os = "linux") {
                        "accept4"
                    } else {
                        "accept"
                    };
                    return Err(Fail::Network(format!(
                        "accept tcp {}: {call}: {}",
                        self.addr,
                        oserr::reason(&e)
                    )));
                }
            }
        }
    }

    fn close(&self) -> Result<(), Fail> {
        if !close_listener(&self.sock, &self.closer.1) {
            return Err(self.closed("close"));
        }
        Ok(())
    }
}

/// Closes a listener's socket and wakes its `accept` (and `hold`); false
/// if it was closed already. It takes only the socket slot's lock, a
/// leaf lock, and one byte into an empty pipe never blocks: short and
/// safe enough for a cancellation hook (`close_on_cancel`).
fn close_listener(
    sock: &Mutex<Option<Arc<Counted<TcpListener>>>>,
    closer: &io::PipeWriter,
) -> bool {
    let Some(l) = lock(sock).take() else {
        return false;
    };
    let _ = (&*closer).write(&[1]);
    drop(l);
    true
}

/// Go `net.SplitHostPort`, with its messages (`address <addr>: <why>`).
fn split_host_port(hostport: &str) -> Result<(&str, &str), String> {
    let err = |why: &str| Err(format!("address {hostport}: {why}"));
    let missing_port = "missing port in address";
    let too_many_colons = "too many colons in address";
    let Some(i) = hostport.rfind(':') else {
        return err(missing_port);
    };
    let (host, j, k);
    if hostport.starts_with('[') {
        let Some(end) = hostport.find(']') else {
            return err("missing ']' in address");
        };
        if end + 1 == hostport.len() {
            return err(missing_port);
        } else if end + 1 != i {
            if hostport.as_bytes()[end + 1] == b':' {
                return err(too_many_colons);
            }
            return err(missing_port);
        }
        host = &hostport[1..end];
        (j, k) = (1, end + 1);
    } else {
        host = &hostport[..i];
        if host.contains(':') {
            return err(too_many_colons);
        }
        (j, k) = (0, 0);
    }
    if hostport[j..].contains('[') {
        return err("unexpected '[' in address");
    }
    if hostport[k..].contains(']') {
        return err("unexpected ']' in address");
    }
    Ok((host, &hostport[i + 1..]))
}

/// Go `parsePort` and its range check: a decimal port (a sign allowed),
/// `address <port>: invalid port` out of range. Go looks names such as
/// `http` up in the services database; here every name is unknown
/// (`lookup tcp/<name>: unknown port`), a known difference.
fn parse_port(service: &str) -> Result<u16, String> {
    if service.is_empty() {
        return Ok(0);
    }
    let (neg, digits) = match service.as_bytes()[0] {
        b'+' => (false, &service[1..]),
        b'-' => (true, &service[1..]),
        _ => (false, service),
    };
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("lookup tcp/{service}: unknown port"));
    }
    // more digits than any port: out of range whatever they are
    let n: u64 = digits.parse().unwrap_or(u64::MAX);
    if neg && n != 0 || n > 0xFFFF {
        return Err(format!("address {service}: invalid port"));
    }
    Ok(n as u16)
}

/// An address to dial or listen on: an IP, its IPv6 scope (0 for none)
/// and its zone as Go names it ("" for none).
struct Target {
    ip: IpAddr,
    scope: u32,
    zone: String,
}

impl Target {
    fn plain(ip: IpAddr) -> Target {
        Target {
            ip,
            scope: 0,
            zone: String::new(),
        }
    }

    fn socket(&self, port: u16) -> SocketAddr {
        match self.ip {
            IpAddr::V6(ip) => SocketAddr::V6(SocketAddrV6::new(ip, port, 0, self.scope)),
            ip => SocketAddr::new(ip, port),
        }
    }

    /// Go's `TCPAddr.String`: the zone as written (`[::1%lo0]:80`).
    fn label(&self, port: u16) -> String {
        if self.zone.is_empty() {
            SocketAddr::new(self.ip, port).to_string()
        } else {
            format!("[{}%{}]:{port}", self.ip, self.zone)
        }
    }
}

/// The addresses `host` names: an IP literal itself (an IPv6 one with
/// its zone, Go `parseIPZone`), else a lookup on a thread of its own
/// that the task waits for in `w` (cancellable, cut by `deadline`). `op`
/// is Go's `dial` or `listen`.
fn resolve(
    w: &mut Watch,
    op: &str,
    host: &str,
    deadline: Option<Instant>,
) -> Result<Vec<Target>, Fail> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![Target::plain(ip)]);
    }
    if let Some((ip, zone)) = host.split_once('%')
        && let Ok(ip) = ip.parse::<Ipv6Addr>()
        && !zone.is_empty()
    {
        return Ok(vec![Target {
            ip: IpAddr::V6(ip),
            scope: sock::zone_index(zone),
            zone: zone.to_string(),
        }]);
    }
    let slot = Arc::new(LookupSlot {
        result: Mutex::new(None),
        pipe: io::pipe().map_err(|e| Fail::Network(format!("{op} tcp: {}", oserr::reason(&e))))?,
    });
    LOOKUPS.fetch_add(1, Ordering::SeqCst);
    let (theirs, name) = (slot.clone(), host.to_string());
    let spawned = std::thread::Builder::new()
        .name("fung-lookup".into())
        .spawn(move || {
            let r = lookup(&name);
            *lock(&theirs.result) = Some(r);
            // the read end lives as long as `theirs` does
            let _ = (&theirs.pipe.1).write(&[1]);
            drop(theirs);
            LOOKUPS.fetch_sub(1, Ordering::SeqCst);
        });
    if let Err(e) = spawned {
        LOOKUPS.fetch_sub(1, Ordering::SeqCst);
        return Err(Fail::Network(format!(
            "{op} tcp: lookup {host}: {}",
            oserr::reason(&e)
        )));
    }
    let r = loop {
        if let Some(r) = lock(&slot.result).take() {
            break r;
        }
        let fds = [(slot.pipe.0.as_raw_fd(), Interest::Read)];
        if wait_io(w, &fds, deadline)? == Waited::TimedOut {
            return Err(Fail::Timeout(format!(
                "{op} tcp: lookup {host}: i/o timeout"
            )));
        }
    };
    let ips = r.map_err(|why| Fail::Network(format!("{op} tcp: lookup {host}: {why}")))?;
    Ok(ips
        .into_iter()
        .map(|(ip, scope)| Target {
            ip,
            scope,
            zone: sock::zone_name(scope),
        })
        .collect())
}

/// An address a lookup found, with its IPv6 scope (0 for none).
type Found = (IpAddr, u32);

/// Where a lookup thread leaves its answer, and the pipe it wakes the
/// waiting task with.
struct LookupSlot {
    result: Mutex<Option<Result<Vec<Found>, String>>>,
    pipe: (io::PipeReader, io::PipeWriter),
}

/// `sock::lookup`, held by the test gate for the name `gate.invalid`.
fn lookup(host: &str) -> Result<Vec<Found>, String> {
    #[cfg(test)]
    if host == "gate.invalid" {
        probe::gate();
        // the resolver's answer for a name that does not exist
        return Err(sock::lookup_reason(
            libc::EAI_NONAME,
            io::Error::from_raw_os_error(0),
        ));
    }
    sock::lookup(host)
}

/// Go's `Dialer.Dial` over the addresses in order: the first that
/// connects, else the first error (Go's dual-stack racing of the second
/// family after 300 ms is not done: the addresses are tried one by one,
/// all within the one deadline).
fn dial(w: &mut Watch, host: &str, port: u16, deadline: Option<Instant>) -> Result<Conn, Fail> {
    let ips = if host.is_empty() {
        // Go dials the local system for an empty host
        vec![Target::plain(IpAddr::V4(Ipv4Addr::LOCALHOST))]
    } else {
        resolve(w, "dial", host, deadline)?
    };
    let mut first = None;
    for ip in ips {
        let addr = ip.socket(port);
        let label = if host.is_empty() {
            format!(":{port}")
        } else {
            ip.label(port)
        };
        match dial_one(w, addr, &label, deadline) {
            Ok(c) => return Ok(c),
            Err(f @ (Fail::Cancelled | Fail::Timeout(_))) => return Err(f),
            Err(f) => {
                first.get_or_insert(f);
            }
        }
    }
    Err(first.unwrap_or_else(|| Fail::Network(format!("dial tcp: lookup {host}: no such host"))))
}

fn dial_one(
    w: &mut Watch,
    addr: SocketAddr,
    label: &str,
    deadline: Option<Instant>,
) -> Result<Conn, Fail> {
    let fail = |call: &str, e: &io::Error| {
        Fail::Network(format!("dial tcp {label}: {call}: {}", oserr::reason(e)))
    };
    let (s, connected) = sock::start_connect(&addr).map_err(|(call, e)| fail(call, &e))?;
    if !connected {
        // writable once the attempt ended, either way
        if wait_io(w, &[(s.as_raw_fd(), Interest::Write)], deadline)? == Waited::TimedOut {
            return Err(Fail::Timeout(format!("dial tcp {label}: i/o timeout")));
        }
        sock::finish_connect(&s).map_err(|e| fail("connect", &e))?;
    }
    Ok(Conn::new(s, addr))
}

/// Go `net.Listen("tcp", addr)`: an empty host listens on every address
/// (`[::]`, both families; `0.0.0.0` where IPv6 is missing), a name on
/// its first IPv4 address (else its first).
fn listen(w: &mut Watch, addr: &str) -> Result<Listener, Fail> {
    let (host, port) = split_host_port(addr)
        .and_then(|(h, p)| Ok((h, parse_port(p)?)))
        .map_err(|e| Fail::Network(format!("listen tcp: {e}")))?;
    let bound = if host.is_empty() {
        let any6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port);
        match sock::listen(&any6) {
            Err((_, e)) if e.raw_os_error() == Some(libc::EAFNOSUPPORT) => {
                let any4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
                sock::listen(&any4).map_err(|f| (f, format!(":{port}")))
            }
            r => r.map_err(|f| (f, format!(":{port}"))),
        }
    } else {
        let ips = resolve(w, "listen", host, None)?;
        let ip = ips.iter().find(|t| t.ip.is_ipv4()).unwrap_or(&ips[0]);
        sock::listen(&ip.socket(port)).map_err(|f| (f, ip.label(port)))
    };
    let l = bound.map_err(|((call, e), label)| {
        Fail::Network(format!("listen tcp {label}: {call}: {}", oserr::reason(&e)))
    })?;
    let local = l
        .local_addr()
        .map_err(|e| Fail::Network(format!("listen tcp {addr}: {}", oserr::reason(&e))))?;
    let closer = io::pipe().map_err(|e| {
        Fail::Network(format!(
            "listen tcp {}: {}",
            sock::addr_text(&local),
            oserr::reason(&e)
        ))
    })?;
    Ok(Listener {
        sock: Arc::new(Mutex::new(Some(Arc::new(Counted::new(l))))),
        closer: (closer.0, Arc::new(closer.1)),
        addr: sock::addr_text(&local),
        open: Arc::default(),
    })
}

// ---- the natives ----

/// The connection argument, borrowed as itself (an `Rc` cannot go
/// outside the token; the `Conn` it holds can).
fn need_conn<'a>(name: &str, v: &'a Value) -> Result<&'a Conn, Stop> {
    match v {
        Value::Conn(c) => Ok(&**c),
        _ => Err(nat::wrong_arg(name, "conn", v)),
    }
}

/// A count argument: an int from 0.
fn need_count(name: &str, v: &Value) -> Result<usize, Stop> {
    match v {
        Value::Int(n) if *n >= 0 => Ok(*n as usize),
        Value::Int(_) => Err(nat::wrong_arg(name, "a non-negative int", v)),
        _ => Err(nat::wrong_arg(name, "int", v)),
    }
}

fn text(b: &[u8]) -> Value {
    Value::str(&decode_go(b))
}

/// `connect(host, port, tls = false, timeout = 0s)`: `Ok(<conn>)`. A
/// timeout covers the lookup, the connection and the TLS handshake; 0 is
/// none. With `tls`, the connection is a TLS client of `host` (`tls.rs`).
fn connect_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let host = nat::need_str("host", &a[0])?.to_string();
    let port = nat::need_int("port", &a[1])?;
    let Value::Bool(tls) = a[2] else {
        return Err(nat::wrong_arg("tls", "bool", &a[2]));
    };
    let Value::Dur(timeout) = a[3] else {
        return Err(nat::wrong_arg("timeout", "dur", &a[3]));
    };
    let Ok(port) = u16::try_from(port) else {
        return Ok(fail_value(Fail::Network(format!(
            "dial tcp: address {port}: invalid port"
        ))));
    };
    let deadline = if timeout > 0 {
        Instant::now().checked_add(Duration::from_nanos(timeout as u64))
    } else {
        None
    };
    let r = outside_watched(t, move |w| {
        let mut c = dial(w, &host, port, deadline)?;
        if tls {
            c.tls = Some(Mutex::new(tls::handshake(w, &c, &host, deadline)?));
        }
        Ok(c)
    })?;
    Ok(result(r.map(|c| Value::Conn(Rc::new(c)))))
}

/// `start_tls(c, host, timeout = 0s)`: `Ok(<conn>)`, a TLS client of
/// `host` over `c`'s socket, as `connect(tls: true)` makes one (stage 4d
/// Task 4: https through an HTTP proxy's CONNECT tunnel, which fung
/// cannot do over the other primitives). `c` reads as closed afterwards;
/// the timeout covers the handshake.
fn start_tls_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let c = need_conn("c", &a[0])?;
    let host = nat::need_str("host", &a[1])?.to_string();
    let Value::Dur(timeout) = a[2] else {
        return Err(nat::wrong_arg("timeout", "dur", &a[2]));
    };
    let deadline = if timeout > 0 {
        Instant::now().checked_add(Duration::from_nanos(timeout as u64))
    } else {
        None
    };
    let plain = match c.take_over() {
        Ok(p) => p,
        Err(f) => return Ok(fail_value(f)),
    };
    let r = outside_watched(t, move |w| {
        let mut c = plain;
        c.tls = Some(Mutex::new(tls::handshake(w, &c, &host, deadline)?));
        Ok(c)
    })?;
    Ok(result(r.map(|c| Value::Conn(Rc::new(c)))))
}

/// `listen(addr)`: `Ok(<listener>)`.
fn listen_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let addr = nat::need_str("addr", &a[0])?.to_string();
    let r = outside_watched(t, move |w| listen(w, &addr))?;
    Ok(result(r.map(|l| Value::Listener(Rc::new(l)))))
}

/// `local_addr(x)`: a connection's or listener's own address.
fn local_addr_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    match &a[0] {
        Value::Conn(c) => Ok(Value::str(&c.local)),
        Value::Listener(l) => Ok(Value::str(&l.addr)),
        v => Err(nat::wrong_arg("x", "conn | listener", v)),
    }
}

/// `accept(l)`: `Ok(<conn>)`, the next connection.
fn accept_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let Value::Listener(l) = &a[0] else {
        return Err(nat::wrong_arg("l", "listener", &a[0]));
    };
    let l: &Listener = l;
    let r = outside_watched(t, |w| l.accept(w))?;
    Ok(result(r.map(|c| Value::Conn(Rc::new(c)))))
}

/// `read_line(c, limit)`: `Ok(line)` without its end, `Ok(nil)` at the
/// end of the stream.
fn read_line_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let c = need_conn("c", &a[0])?;
    let limit = need_count("limit", &a[1])?;
    let r = outside_watched(t, |w| c.read_line(w, limit))?;
    Ok(result(r.map(|l| l.map_or(Value::Nil, |l| text(&l)))))
}

/// `read_raw_line(c, limit)`: `Ok(line)` with its `\n`.
fn read_raw_line_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let c = need_conn("c", &a[0])?;
    let limit = need_count("limit", &a[1])?;
    let r = outside_watched(t, |w| c.read_raw_line(w, limit))?;
    Ok(result(r.map(|l| text(&l))))
}

/// `read_bytes(c, n)`: `Ok(text)` of exactly `n` bytes.
fn read_bytes_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let c = need_conn("c", &a[0])?;
    let n = need_count("n", &a[1])?;
    let r = outside_watched(t, |w| c.read_bytes(w, n))?;
    Ok(result(r.map(|b| text(&b))))
}

/// `read_all(c, limit)`: `Ok(text)` up to the end of the stream.
fn read_all_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let c = need_conn("c", &a[0])?;
    let limit = need_count("limit", &a[1])?;
    let r = outside_watched(t, |w| c.read_all(w, limit))?;
    Ok(result(r.map(|b| text(&b))))
}

/// `read_bytes_into(c, n, cap)`: `Ok(nil)` once `n` more raw bytes are
/// in `c`'s body buffer; `Err('limit)` (nothing read) if it would hold
/// more than `cap` bytes.
fn read_bytes_into_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let c = need_conn("c", &a[0])?;
    let n = need_count("n", &a[1])?;
    let cap = need_count("cap", &a[2])?;
    let r = outside_watched(t, |w| c.read_bytes_into(w, n, cap))?;
    Ok(result(r.map(|()| Value::Nil)))
}

/// `take_body(c)`: the body buffer decoded once (U+FFFD for bytes that
/// are not UTF-8), emptied. Waits for a read in progress on `c`.
fn take_body_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let c = need_conn("c", &a[0])?;
    let body = crate::vm::outside(t, || c.take_body());
    Ok(text(&body))
}

/// `write(c, s)`: `Ok(nil)` once all of `s` is written.
fn write_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let c = need_conn("c", &a[0])?;
    // a copy: only Rust values go outside the token
    let bytes = nat::need_str("s", &a[1])?.as_bytes().to_vec();
    let r = outside_watched(t, |w| c.write(w, &bytes))?;
    Ok(result(r.map(|()| Value::Nil)))
}

/// `close(x)`: closes a connection or a listener; `Ok(nil)`, an error
/// the second time (Go).
fn close_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let r = match &a[0] {
        Value::Conn(c) => c.close(),
        Value::Listener(l) => l.close(),
        v => return Err(nat::wrong_arg("x", "conn | listener", v)),
    };
    Ok(result(r.map(|()| Value::Nil)))
}

/// `set_deadline(c, d)`: reads and writes on `c` from `d` after now time
/// out (`'timeout`); `0s` removes the deadline.
fn set_deadline_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let c = need_conn("c", &a[0])?;
    let Value::Dur(d) = a[1] else {
        return Err(nat::wrong_arg("d", "dur", &a[1]));
    };
    Ok(result(c.set_deadline(d).map(|()| Value::Nil)))
}

/// `byte_len(s)`: the UTF-8 size of `s` in bytes.
fn byte_len_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    Ok(Value::Int(s.len() as i64))
}

/// `utf8_bytes(s)`: the bytes `write` would send for `s`, as ints. fung
/// sees strings as characters only; the `http` module needs the bytes to
/// percent-encode a URL path (Go `url.URL.EscapedPath`).
fn utf8_bytes_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    Ok(nat::list(
        s.bytes().map(|b| Value::Int(i64::from(b))).collect(),
    ))
}

/// `need_str_map(name, m)`: `nil`, or E0403 when `m` is not a map or a
/// value of it is not a str (the first one, in the map's order): Go
/// `http`'s check of a header map, which fung code cannot raise itself.
fn need_str_map_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let name = nat::need_str("name", &a[0])?;
    let Value::Map(m) = &a[1] else {
        return Err(nat::wrong_arg(name, "map", &a[1]));
    };
    if let Some((_, v)) = m.iter().find(|(_, v)| !matches!(v, Value::Str(_))) {
        return Err(nat::fail(
            "E0403",
            vec![
                nat::s(name),
                nat::s("a map of strings"),
                fung_front::diag::Arg::Str(format!("a map holding {}", crate::value::type_name(v))),
            ],
        ));
    }
    Ok(Value::Nil)
}

/// `hold(l, grace)`: waits until this task is cancelled or another task
/// closes `l`; then closes `l`, waits until every connection accepted
/// from it is closed, at most `grace`, and ends with E0701 when
/// cancelled, else returns nil (Go `Server.Shutdown` with a deadline,
/// which `http.serve` runs when its task is cancelled; the server's
/// accept task closes `l` when it fails). A connection still open after
/// the grace is left to whoever holds it.
fn hold_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let Value::Listener(l) = &a[0] else {
        return Err(nat::wrong_arg("l", "listener", &a[0]));
    };
    let Value::Dur(grace) = a[1] else {
        return Err(nat::wrong_arg("grace", "dur", &a[1]));
    };
    let l: &Listener = l;
    let grace = Duration::from_nanos(grace.max(0) as u64);
    outside_watched(t, |w| {
        WAITING.fetch_add(1, Ordering::SeqCst);
        // A cancellation, or `close` writing to the listener's pipe (the
        // byte stays: a blocked `accept` sees it too). Should `poll`
        // itself fail, both are looked at now and then instead.
        let closer = [(l.closer.0.as_raw_fd(), Interest::Read)];
        while !l.is_closed() {
            match w.wait(&closer, None) {
                Ok(Waited::Cancelled) => break,
                Ok(_) => {}
                Err(_) => {
                    if w.cancelled() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
        let _ = l.close();
        l.open.wait_idle(Instant::now() + grace);
        WAITING.fetch_sub(1, Ordering::SeqCst);
    })?;
    // Not cancelled (then `outside_watched` unwinds with E0701): the
    // listener was closed by another task.
    Ok(Value::Nil)
}

/// `close_on_cancel(l)`: closes `l` the moment this task is cancelled
/// (by whoever cancels it, before `cancel` returns) or ends. The `http`
/// server registers its listener so: cancelled at any tick, even before
/// it waits in `hold`, it leaves no listener open behind it (Go's
/// `Shutdown` closes the listener before the server's call returns).
///
/// The hook is not removed when `serve` returns, because `serve` never
/// returns while its task goes on: it ends cancelled (the hook has run)
/// or raises the accept loop's failure, which no fung code catches, so
/// the task ends and its node drops the hook. Until then the hook holds
/// only the closed listener's empty slot and its wake pipe's write end,
/// and running it again is a no-op (`close_listener` on a closed slot).
fn close_on_cancel_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let Value::Listener(l) = &a[0] else {
        return Err(nat::wrong_arg("l", "listener", &a[0]));
    };
    let (sock, closer) = (l.sock.clone(), l.closer.1.clone());
    crate::vm::when_cancelled(t, move || {
        close_listener(&sock, &closer);
    });
    Ok(Value::Nil)
}

/// `utf8_string(bs)`: the text of a list of bytes (ints from 0 to 255),
/// decoded as everything read is (U+FFFD for what is not UTF-8): what
/// `utf8_bytes` gives, backwards. The `http` server decodes the
/// percent-escapes of a request's path and query with it (Go
/// `url.PathUnescape`, `QueryUnescape`).
fn utf8_string_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let Value::List(items) = &a[0] else {
        return Err(nat::wrong_arg("bs", "list", &a[0]));
    };
    let mut bytes = Vec::with_capacity(items.len());
    for v in items.iter() {
        match v {
            Value::Int(n) if (0..=255).contains(n) => bytes.push(*n as u8),
            v => {
                return Err(nat::wrong_arg(
                    "bs",
                    "a list of bytes (ints from 0 to 255)",
                    v,
                ));
            }
        }
    }
    Ok(text(&bytes))
}

/// Test probes: the counters, and a gate that holds lookups of
/// `gate.invalid` (a lookup that waits as long as a test wants).
#[cfg(test)]
pub(crate) mod probe {
    use std::sync::atomic::Ordering;
    use std::sync::{Condvar, Mutex};

    use super::{LOOKUPS, READ_WAITS, SOCKETS, WAITING, WRITE_WAITS, lock};

    pub(crate) fn sockets() -> usize {
        SOCKETS.load(Ordering::SeqCst)
    }

    pub(crate) fn lookups() -> usize {
        LOOKUPS.load(Ordering::SeqCst)
    }

    pub(crate) fn waiting() -> usize {
        WAITING.load(Ordering::SeqCst)
    }

    pub(crate) fn read_waits() -> usize {
        READ_WAITS.load(Ordering::SeqCst)
    }

    pub(crate) fn write_waits() -> usize {
        WRITE_WAITS.load(Ordering::SeqCst)
    }

    static GATE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

    /// Holds lookups of `gate.invalid` until the result drops.
    pub(crate) fn hold_lookups() -> Gate {
        *lock(&GATE.0) = true;
        Gate
    }

    pub(crate) struct Gate;

    impl Drop for Gate {
        fn drop(&mut self) {
            *lock(&GATE.0) = false;
            GATE.1.notify_all();
        }
    }

    /// Waits while the gate is held.
    pub(super) fn gate() {
        let mut held = lock(&GATE.0);
        while *held {
            held = GATE
                .1
                .wait(held)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}

#[path = "tls.rs"]
mod tls;

#[cfg(test)]
#[path = "net_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tls_tests.rs"]
mod tls_tests;
