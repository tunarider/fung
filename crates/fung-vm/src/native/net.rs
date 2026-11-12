//! `_native.net` (stage 4d): the TCP primitives the `http` module
//! (`extras/http.fung`) is written over. User code cannot import it
//! (E0310); only embedded extras files can.
//!
//! **Values.** A connection (`Value::Conn`, printed `<conn>`, `type_of`
//! `'conn`) and a listener (`Value::Listener`, `<listener>`,
//! `'listener`) work like file handles (`native/file.rs`): shared by
//! every task holding a copy, compared by identity, holding no fung
//! value (their waits park on the event loop), and closed when the last
//! copy goes (the peer then sees the end of the stream).
//!
//! **Turns.** A connection has a read side and a write side, each held
//! by one operation for its whole run (`Turnstile`): one read and one
//! write can run at once, and lines from two readers never mix. A second
//! reader (or writer) parks in line until the first ends; a cancellation
//! or a deadline ends that wait too. The socket itself sits in a slot of
//! its own, locked only to take a reference: `close` never waits for a
//! read or a write. It wakes the tasks waiting on the socket
//! (`vm::drop_io_waits`), empties the slot and shuts the socket down; the
//! woken read or write then reports the close.
//!
//! **Waiting.** Sockets are non-blocking, and every wait is a park on the
//! event loop (R4a): an operation is tried (`Attempt`), and where it
//! would block it says on which descriptor; the task parks there
//! (`vm::park_io`) and the operation is tried again from the top when the
//! descriptor is ready (`run_io`). What it did so far lives in the
//! connection (the read-ahead buffer, the TLS state) or in the native
//! (bytes written, the connect's stage). No thread waits for a socket. A
//! name lookup runs as a pool job a stop abandons, since `getaddrinfo`
//! cannot be interrupted: a cancelled `connect` returns at once and the
//! job ends when the resolver answers (it holds no fung value and no
//! socket).
//!
//! **Errors** are `Err({kind, message})`: `'timeout` when a deadline
//! passed, `'limit` when a read went past its limit, `'closed` for a
//! connection or listener closed here, `'network` for the rest. Messages are short reasons (`connection refused`, `no such
//! host`, `timed out`, `connection is closed`, `connection closed after 3
//! of 10 bytes`), errno wording from `oserr`; `_native.net` is internal,
//! and the `http` module says what was being done (`cannot request
//! `url`: <reason>`).
//!
//! **Strings.** What is read and written is bytes, as they are (L4);
//! host names, addresses and headers are text.

use std::cell::Cell;
use std::io::{self, Read, Write};
use std::net::{
    IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, SocketAddrV6, TcpListener, TcpStream,
};
use std::os::fd::{AsRawFd, RawFd};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use super::nat::{self, native, native_step, opt, param};
use super::oserr;
use super::sock;
use super::turn::{Hold, Turnstile};
use crate::value::{Args, Lit, Module, Native, Value};
use crate::vm::poller::Interest;
use crate::vm::{CancelNode, Flow, IoStep, Kind, Stop, Task, Waiter};

/// What a closed connection or listener answers.
const CLOSED: &str = "connection is closed";
const LISTENER_CLOSED: &str = "listener is closed";
/// What a deadline that passed answers.
const TIMED_OUT: &str = "timed out";
/// A line read that met the end of the stream.
const CLOSED_BY_PEER: &str = "connection closed";

/// The stream ended after `got` of the `want` bytes asked for.
fn short_read(got: usize, want: usize) -> String {
    if got == 0 {
        CLOSED_BY_PEER.to_string()
    } else {
        format!("connection closed after {got} of {want} bytes")
    }
}

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The `_native.net` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.get_or_init(|| {
        vec![
            (
                "connect",
                native_step(
                    "connect",
                    connect_fn,
                    vec![
                        param("host"),
                        param("port"),
                        opt("tls", Lit::Bool(false)),
                        opt("alpn", Lit::Nil),
                    ],
                ),
            ),
            (
                "listen",
                native_step(
                    "listen",
                    listen_fn,
                    vec![param("addr"), opt("reuse_port", Lit::Bool(false))],
                ),
            ),
            (
                "local_addr",
                native("local_addr", local_addr_fn, vec![param("x")]),
            ),
            ("accept", native_step("accept", accept_fn, vec![param("l")])),
            (
                "read_line",
                native_step("read_line", read_line_fn, vec![param("c"), param("limit")]),
            ),
            (
                "read_raw_line",
                native_step(
                    "read_raw_line",
                    read_raw_line_fn,
                    vec![param("c"), param("limit")],
                ),
            ),
            (
                "read_bytes",
                native_step("read_bytes", read_bytes_fn, vec![param("c"), param("n")]),
            ),
            (
                "read_all",
                native_step("read_all", read_all_fn, vec![param("c"), param("limit")]),
            ),
            (
                "write",
                native_step("write", write_fn, vec![param("c"), param("s")]),
            ),
            ("close", native_step("close", close_fn, vec![param("x")])),
            (
                "close_write",
                native("close_write", close_write_fn, vec![param("c")]),
            ),
            (
                "set_deadline",
                native(
                    "set_deadline",
                    set_deadline_fn,
                    vec![param("c"), param("d")],
                ),
            ),
            (
                "read_bytes_into",
                native_step(
                    "read_bytes_into",
                    read_bytes_into_fn,
                    vec![param("c"), param("n"), param("cap")],
                ),
            ),
            (
                "take_body",
                native_step("take_body", take_body_fn, vec![param("c")]),
            ),
            (
                "start_tls",
                native_step(
                    "start_tls",
                    start_tls_fn,
                    vec![param("c"), param("host"), opt("alpn", Lit::Nil)],
                ),
            ),
            (
                "read_fields",
                native_step(
                    "read_fields",
                    read_fields_fn,
                    vec![param("c"), param("budget"), param("max")],
                ),
            ),
            ("user_agent", native("user_agent", user_agent_fn, vec![])),
            (
                "url_encode",
                native("url_encode", url_encode_fn, vec![param("s")]),
            ),
            (
                "url_decode",
                native("url_decode", url_decode_fn, vec![param("s")]),
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
/// Tasks waiting in a connection's turn line (tests).
static IN_LINE: AtomicUsize = AtomicUsize::new(0);
static READ_WAITS: AtomicUsize = AtomicUsize::new(0);
static WRITE_WAITS: AtomicUsize = AtomicUsize::new(0);

/// Why an operation failed, made into an `Err` value (`fail_value`).
#[derive(Debug)]
enum Fail {
    Network(String),
    /// An address or a port that does not read (`'invalid`, D70).
    Invalid(String),
    /// The connection or listener was closed by this program (`'closed`:
    /// the server's accept loop ends on it).
    Closed(String),
    Timeout(String),
    Limit(String),
    /// The task was cancelled between two tries: it unwinds instead.
    Cancelled,
    /// The try would wait (`Attempt`): the native parks and tries again.
    Block,
}

fn fail_value(f: Fail) -> Value {
    let (kind, msg) = match f {
        Fail::Network(m) => ("network", m),
        Fail::Invalid(m) => ("invalid", m),
        Fail::Closed(m) => ("closed", m),
        Fail::Timeout(m) => ("timeout", m),
        Fail::Limit(m) => ("limit", m),
        // Never seen: `run_io` unwinds, or parks and tries again.
        Fail::Cancelled | Fail::Block => ("network", "operation was canceled".to_string()),
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

/// One try of a socket operation (R4a): it never waits. Where it would
/// wait it says what for (`wait_io`, `wait_turn`) and fails with
/// `Fail::Block`; the native parks there and tries again from the top
/// (`run_io`). So every operation can be tried again: what it did so far
/// lives in the connection (the read-ahead buffer, the TLS state) or in
/// the native's state (bytes written, the connect's stage).
pub(super) struct Attempt {
    node: Arc<CancelNode>,
    me: Arc<Waiter>,
    blocked: Option<IoStep>,
}

impl Attempt {
    /// A try outside a native (tests): `node` decides whether it is
    /// cancelled.
    #[cfg(test)]
    fn for_tests(node: Arc<CancelNode>, rt: &crate::vm::Runtime) -> Attempt {
        Attempt {
            node,
            me: rt.task().waiter_for_tests(),
            blocked: None,
        }
    }

    /// The task was cancelled: checked also between tries that do not
    /// wait (a fast peer), so that a cancelled read ends.
    fn cancelled(&mut self) -> bool {
        self.node.is_cancelled()
    }
}

/// The try would wait for `fd` to be ready for `interest`, or `deadline`.
fn wait_io(a: &mut Attempt, fd: RawFd, interest: Interest, deadline: Option<Instant>) -> Fail {
    a.blocked = Some(IoStep::Wait(fd, interest, deadline));
    Fail::Block
}

/// Which turn of a connection an operation holds for its whole run.
#[derive(Clone, Copy)]
enum Side {
    Read,
    Write,
}

/// What a socket operation holds between tries: whether its task is
/// counted as waiting, and whether it holds its connection's turn. It
/// gives both back once, when the operation ends (`end`) or when its
/// wait is dropped any other way (a look that failed, a task abandoned):
/// `Drop`.
struct IoState {
    waiting: Cell<bool>,
    hold: Hold,
    turn: Option<(Arc<Conn>, Side)>,
}

impl IoState {
    fn set_waiting(&self, w: bool) {
        if self.waiting.replace(w) != w {
            if w {
                WAITING.fetch_add(1, Ordering::SeqCst);
            } else {
                WAITING.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    fn end(&self) {
        self.set_waiting(false);
        if let Some((c, side)) = &self.turn {
            self.hold.end(c.turnstile(*side));
        }
    }
}

impl Drop for IoState {
    fn drop(&mut self) {
        self.end();
    }
}

/// Runs `step` until it ends, parking where it would wait
/// (`vm::park_io`), then makes its value with `done`. A cancellation the
/// step saw unwinds the task. The connection turn `side` of `turn` is
/// held from the first try to the end (a stop included).
fn run_io<T: 'static>(
    t: &mut Task<'_>,
    turn: Option<(Arc<Conn>, Side)>,
    mut step: impl FnMut(&mut Attempt) -> Result<T, Fail> + 'static,
    done: impl Fn(Result<T, Fail>) -> Value + 'static,
) -> Result<Flow, Stop> {
    let state = Rc::new(IoState {
        waiting: Cell::new(false),
        hold: Hold::default(),
        turn,
    });
    let st = state.clone();
    let poll = move |t: &mut Task<'_>, me: &Arc<Waiter>| {
        st.set_waiting(false);
        let mut a = Attempt {
            node: t.cancel_node(),
            me: me.clone(),
            blocked: None,
        };
        let r = (|| {
            if let Some((c, side)) = &st.turn
                && !st.hold.take(c.turnstile(*side), &a.me)
            {
                a.blocked = Some(IoStep::Park);
                return Err(Fail::Block);
            }
            step(&mut a)
        })();
        match r {
            Err(Fail::Block) => {
                let wait = a.blocked.take().expect("a blocked try says what for");
                if let IoStep::Wait(_, interest, _) = &wait {
                    match interest {
                        Interest::Read => READ_WAITS.fetch_add(1, Ordering::SeqCst),
                        Interest::Write => WRITE_WAITS.fetch_add(1, Ordering::SeqCst),
                    };
                    st.set_waiting(true);
                }
                wait
            }
            Err(Fail::Cancelled) => {
                st.end();
                IoStep::Done(t.check_cancelled().map(|()| fail_value(Fail::Cancelled)))
            }
            r => {
                st.end();
                IoStep::Done(Ok(done(r)))
            }
        }
    };
    crate::vm::park_io(t, poll, move |_| state.end())
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

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // The state is plain data that no panic leaves half-changed.
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a connection's read side keeps between reads.
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
    /// The read-ahead and body buffers, locked within one try.
    rd: Mutex<ReadState>,
    /// The read side and the write side, each held by one operation.
    rd_turn: Turnstile,
    wr_turn: Turnstile,
    /// When reads and writes time out (`set_deadline`).
    deadline: Mutex<Option<Instant>>,
    /// The rustls state of a TLS connection (`tls.rs`), locked within one
    /// try.
    tls: Option<Mutex<tls::Session>>,
    /// `local->peer`, as the connection's printed form shows it.
    name: String,
    /// The local address.
    local: String,
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
            rd_turn: Turnstile::counted(&IN_LINE),
            wr_turn: Turnstile::counted(&IN_LINE),
            deadline: Mutex::new(None),
            tls: None,
        }
    }

    fn turnstile(&self, side: Side) -> &Turnstile {
        match side {
            Side::Read => &self.rd_turn,
            Side::Write => &self.wr_turn,
        }
    }

    fn closed(&self) -> Fail {
        Fail::Closed(CLOSED.into())
    }

    fn timed_out(&self) -> Fail {
        Fail::Timeout(TIMED_OUT.into())
    }

    fn is_closed(&self) -> bool {
        lock(&self.sock).is_none()
    }

    /// The socket's descriptor while it is open.
    fn fd(&self) -> Option<RawFd> {
        lock(&self.sock).as_ref().map(|s| s.0.as_raw_fd())
    }

    /// The socket for one try, or the closed error.
    fn sock(&self) -> Result<Arc<Counted<TcpStream>>, Fail> {
        lock(&self.sock).clone().ok_or_else(|| self.closed())
    }

    fn deadline(&self) -> Option<Instant> {
        *lock(&self.deadline)
    }

    /// Reads what the socket has into `buf`: the count, 0 at the end of
    /// the stream.
    fn fill(&self, a: &mut Attempt, s: &TcpStream, buf: &mut Vec<u8>) -> Result<usize, Fail> {
        if let Some(t) = &self.tls {
            return tls::fill(self, t, a, s, buf);
        }
        let mut chunk = [0u8; CHUNK];
        loop {
            // also between reads that never wait (a fast peer)
            if a.cancelled() {
                return Err(Fail::Cancelled);
            }
            if self.is_closed() {
                return Err(self.closed());
            }
            let deadline = self.deadline();
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return Err(self.timed_out());
            }
            match (&*s).read(&mut chunk) {
                // a close from another task shuts the socket down: that
                // end of stream is the close
                Ok(0) if self.is_closed() => return Err(self.closed()),
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    return Ok(n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Err(wait_io(a, s.as_raw_fd(), Interest::Read, deadline));
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    return Err(Fail::Network(oserr::reason(&e)));
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
    fn read_line(&self, a: &mut Attempt, limit: usize) -> Result<Option<Vec<u8>>, Fail> {
        let mut st = lock(&self.rd);
        let buf = &mut st.buf;
        let s = self.sock()?;
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
            if self.fill(a, &s.0, buf)? == 0 {
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

    /// The next line with its `\n` (for a chunk size line, whose CRLF the
    /// `http` module checks): no `\n` within `limit` bytes is a `'limit`
    /// error, the end of the stream first an error. The caller sees which
    /// line end came.
    fn read_raw_line(&self, a: &mut Attempt, limit: usize) -> Result<Vec<u8>, Fail> {
        let mut st = lock(&self.rd);
        let buf = &mut st.buf;
        let s = self.sock()?;
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
            if self.fill(a, &s.0, buf)? == 0 {
                return Err(Fail::Network(CLOSED_BY_PEER.into()));
            }
        }
    }

    /// Exactly `n` bytes; it is an error when the stream ends first.
    fn read_bytes(&self, a: &mut Attempt, n: usize) -> Result<Vec<u8>, Fail> {
        let mut st = lock(&self.rd);
        let buf = &mut st.buf;
        let s = self.sock()?;
        while buf.len() < n {
            let before = buf.len();
            if self.fill(a, &s.0, buf)? == 0 {
                return Err(Fail::Network(short_read(before, n)));
            }
        }
        Ok(buf.drain(..n).collect())
    }

    /// Everything up to the end of the stream; more than `limit` bytes
    /// is a `'limit` error (what was read stays unread). Over TLS the end
    /// must be close_notify (LB23): the end of the stream is then the
    /// only sign of the text's end, and a bare TCP end can come from
    /// anyone on the path.
    fn read_all(&self, a: &mut Attempt, limit: usize) -> Result<Vec<u8>, Fail> {
        let mut st = lock(&self.rd);
        let buf = &mut st.buf;
        let s = self.sock()?;
        loop {
            if buf.len() > limit {
                return Err(Fail::Limit(format!("more than {limit} bytes")));
            }
            if self.fill(a, &s.0, buf)? == 0 {
                if let Some(t) = &self.tls
                    && !tls::ended_cleanly(t)
                {
                    return Err(Fail::Network(tls::NO_CLOSE_NOTIFY.into()));
                }
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
    fn read_bytes_into(&self, a: &mut Attempt, n: usize, cap: usize) -> Result<(), Fail> {
        let mut st = lock(&self.rd);
        if st.body.len().saturating_add(n) > cap {
            return Err(Fail::Limit(format!("body longer than {cap} bytes")));
        }
        let s = self.sock()?;
        let st = &mut *st;
        while st.buf.len() < n {
            let before = st.buf.len();
            if self.fill(a, &s.0, &mut st.buf)? == 0 {
                return Err(Fail::Network(short_read(before, n)));
            }
        }
        st.body.extend(st.buf.drain(..n));
        Ok(())
    }

    /// The body buffer's bytes, emptied.
    fn take_body(&self) -> Vec<u8> {
        std::mem::take(&mut lock(&self.rd).body)
    }

    /// Writes all of `b`, `off` bytes of it written by earlier tries.
    fn write(&self, a: &mut Attempt, b: &[u8], off: &mut usize) -> Result<(), Fail> {
        let s = self.sock()?;
        if let Some(t) = &self.tls {
            return tls::write(self, t, a, &s.0, b, off);
        }
        while *off < b.len() {
            if a.cancelled() {
                return Err(Fail::Cancelled);
            }
            if self.is_closed() {
                return Err(self.closed());
            }
            let deadline = self.deadline();
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return Err(self.timed_out());
            }
            match (&s.0).write(&b[*off..]) {
                Ok(n) => *off += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Err(wait_io(a, s.0.as_raw_fd(), Interest::Write, deadline));
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) if self.is_closed() => return Err(self.closed()),
                Err(e) => {
                    return Err(Fail::Network(oserr::reason(&e)));
                }
            }
        }
        Ok(())
    }

    /// A new connection over this one's socket, which this one gives up
    /// (it reads as closed, and dropping it ends nothing). The caller
    /// must own the connection alone: while another task reads or writes
    /// it, this fails at once (`connection is in use`). What was
    /// read ahead is dropped on purpose: `start_tls` uses this once a
    /// proxy has answered a CONNECT, a TLS server says nothing until
    /// spoken to, so nothing that matters can be read ahead. A TLS
    /// connection cannot carry another.
    fn take_over(&self) -> Result<Conn, Fail> {
        if self.tls.is_some() {
            return Err(Fail::Network(
                "a TLS connection cannot carry another".into(),
            ));
        }
        if self.rd_turn.is_busy() || self.wr_turn.is_busy() {
            return Err(Fail::Network("connection is in use".into()));
        }
        let s = lock(&self.sock).take().ok_or_else(|| self.closed())?;
        Ok(Conn {
            sock: Mutex::new(Some(s)),
            rd: Mutex::new(ReadState::default()),
            rd_turn: Turnstile::counted(&IN_LINE),
            wr_turn: Turnstile::counted(&IN_LINE),
            deadline: Mutex::new(None),
            tls: None,
            name: self.name.clone(),
            local: self.local.clone(),
        })
    }

    /// Shuts the socket down and lets it go; a second close is an error.
    /// The tasks waiting on it were woken before (`drop_io_waits`) and
    /// see the close.
    fn close(&self) -> Result<(), Fail> {
        let s = lock(&self.sock).take().ok_or_else(|| self.closed())?;
        if let Some(t) = &self.tls {
            tls::close_notify(t, &s.0);
        }
        let _ = s.0.shutdown(Shutdown::Both);
        Ok(())
    }

    /// A deadline `d` from now: reads and writes after it time out (`0`
    /// is now: they time out at once); `None` removes the deadline. `d`
    /// is never negative (`nat::opt_dur`).
    fn set_deadline(&self, d: Option<i64>) -> Result<(), Fail> {
        if self.is_closed() {
            return Err(Fail::Closed(CLOSED.into()));
        }
        let now = Instant::now();
        // a deadline too far to hold is none
        *lock(&self.deadline) = d.and_then(|d| now.checked_add(Duration::from_nanos(d as u64)));
        Ok(())
    }
}

/// A listening socket.
pub struct Listener {
    /// `None` once closed. Locked only to take or drop a reference.
    sock: Mutex<Option<Arc<Counted<TcpListener>>>>,
    /// The address it listens on (`host:port`, the port the system chose
    /// for 0).
    addr: String,
}

impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<listener {}>", self.addr)
    }
}

impl Listener {
    fn closed(&self) -> Fail {
        Fail::Closed(LISTENER_CLOSED.into())
    }

    fn fd(&self) -> Option<RawFd> {
        lock(&self.sock).as_ref().map(|s| s.0.as_raw_fd())
    }

    /// The next connection, if one is there.
    fn accept(&self, a: &mut Attempt) -> Result<Conn, Fail> {
        let l = lock(&self.sock).clone().ok_or_else(|| self.closed())?;
        loop {
            if a.cancelled() {
                return Err(Fail::Cancelled);
            }
            match l.0.accept() {
                Ok((s, peer)) => {
                    let setup = s.set_nonblocking(true).and_then(|_| sock::no_sigpipe(&s));
                    if let Err(e) = setup {
                        return Err(Fail::Network(oserr::reason(&e)));
                    }
                    return Ok(Conn::new(s, peer));
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Err(wait_io(a, l.0.as_raw_fd(), Interest::Read, None));
                }
                // Retried: the client gave up, or a signal.
                Err(e)
                    if e.kind() == io::ErrorKind::Interrupted
                        || e.raw_os_error() == Some(libc::ECONNABORTED) => {}
                Err(e) => return Err(Fail::Network(oserr::reason(&e))),
            }
        }
    }

    /// Lets the socket go (the tasks waiting in `accept` were woken
    /// before, `drop_io_waits`); false if it was closed already.
    fn close(&self) -> Result<(), Fail> {
        if lock(&self.sock).take().is_none() {
            return Err(self.closed());
        }
        Ok(())
    }
}

/// `host:port` (or `[host]:port`) split in two, or ``invalid address
/// `x` (why)``.
fn split_host_port(hostport: &str) -> Result<(&str, &str), String> {
    let err = |why: &str| Err(format!("invalid address `{hostport}` ({why})"));
    let missing_port = "missing port";
    let too_many_colons = "too many colons";
    let Some(i) = hostport.rfind(':') else {
        return err(missing_port);
    };
    let (host, j, k);
    if hostport.starts_with('[') {
        let Some(end) = hostport.find(']') else {
            return err("missing `]`");
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
        return err("unexpected `[`");
    }
    if hostport[k..].contains(']') {
        return err("unexpected `]`");
    }
    Ok((host, &hostport[i + 1..]))
}

/// A decimal port (a sign allowed) in 0..=65535, or ``invalid port `x` ``
/// (with ``(a port is a number)`` for one that is not).
/// Service names (`http`) are not looked up.
fn parse_port(service: &str) -> Result<u16, String> {
    if service.is_empty() {
        return Ok(0);
    }
    let (neg, digits) = match service.as_bytes()[0] {
        b'+' => (false, &service[1..]),
        b'-' => (true, &service[1..]),
        _ => (false, service),
    };
    // A named port (`http`) is not looked up in /etc/services, which small
    // containers lack: the same address would mean another thing there
    // (L12).
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("invalid port `{service}` (a port is a number)"));
    }
    // more digits than any port: out of range whatever they are
    let n: u64 = digits.parse().unwrap_or(u64::MAX);
    if neg && n != 0 || n > 0xFFFF {
        return Err(format!("invalid port `{service}`"));
    }
    Ok(n as u16)
}

/// An address to dial or listen on: an IP and its IPv6 scope (0 for
/// none).
struct Target {
    ip: IpAddr,
    scope: u32,
}

impl Target {
    fn plain(ip: IpAddr) -> Target {
        Target { ip, scope: 0 }
    }

    fn socket(&self, port: u16) -> SocketAddr {
        match self.ip {
            IpAddr::V6(ip) => SocketAddr::V6(SocketAddrV6::new(ip, port, 0, self.scope)),
            ip => SocketAddr::new(ip, port),
        }
    }
}

/// The addresses `host` names without a lookup: an IP literal itself
/// (an IPv6 one with its zone), the local system for an empty host;
/// `None` for a name.
fn literal(host: &str) -> Option<Vec<Target>> {
    if host.is_empty() {
        return Some(vec![Target::plain(IpAddr::V4(Ipv4Addr::LOCALHOST))]);
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Some(vec![Target::plain(ip)]);
    }
    if let Some((ip, zone)) = host.split_once('%')
        && let Ok(ip) = ip.parse::<Ipv6Addr>()
        && !zone.is_empty()
    {
        return Some(vec![Target {
            ip: IpAddr::V6(ip),
            scope: sock::zone_index(zone),
        }]);
    }
    None
}

/// The addresses a name has, looked up here (`getaddrinfo` blocks: this
/// runs as a pool job a stop abandons, `connect_fn`, `listen_fn`).
fn lookup_targets(host: &str) -> Result<Vec<Target>, Fail> {
    LOOKUPS.fetch_add(1, Ordering::SeqCst);
    let r = lookup(host);
    LOOKUPS.fetch_sub(1, Ordering::SeqCst);
    let ips = r.map_err(Fail::Network)?;
    Ok(ips
        .into_iter()
        .map(|(ip, scope)| Target { ip, scope })
        .collect())
}

/// `sock::lookup`, held by the test gate for the name `gate.invalid`.
fn lookup(host: &str) -> Result<Vec<(IpAddr, u32)>, String> {
    // an address no one answers (TEST-NET-1, RFC 5737), then the loopback
    #[cfg(test)]
    if host == "eyeballs.invalid" {
        return Ok(vec![
            (IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 0),
            (IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        ]);
    }
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

/// Whether `s` can be written now (a connect in progress has ended).
fn writable_now(s: &TcpStream) -> bool {
    let mut p = libc::pollfd {
        fd: s.as_raw_fd(),
        events: libc::POLLOUT,
        revents: 0,
    };
    // SAFETY: `p` is one valid `pollfd`; a zero timeout never blocks.
    let n = unsafe { libc::poll(&mut p, 1, 0) };
    n > 0
}

/// How long a connect has to succeed before the next address is tried
/// beside it (RFC 8305: 250 ms).
const ATTEMPT_DELAY: Duration = Duration::from_millis(250);

/// The addresses with the two families taking turns, the first address's
/// family first (RFC 8305 4): a family whose route is down then costs one
/// attempt delay, not every address of it.
fn interleave(targets: Vec<Target>) -> Vec<Target> {
    let Some(first) = targets.first().map(|t| t.ip.is_ipv6()) else {
        return targets;
    };
    let (mut same, mut other): (Vec<_>, Vec<_>) =
        targets.into_iter().partition(|t| t.ip.is_ipv6() == first);
    let mut out = Vec::with_capacity(same.len() + other.len());
    same.reverse();
    other.reverse();
    loop {
        match (same.pop(), other.pop()) {
            (None, None) => return out,
            (a, b) => out.extend(a.into_iter().chain(b)),
        }
    }
}

/// A connect between tries, racing the addresses ("Happy Eyeballs", RFC
/// 8305 5, L12): the first starts at once and each next one when the
/// ones before have not connected within `ATTEMPT_DELAY` (or all failed);
/// the first to connect wins and the others are closed. When all fail,
/// the first error. Then the TLS handshake if asked for.
struct Dial {
    targets: Vec<Target>,
    port: u16,
    next: usize,
    /// Connects in progress, and when the next address may start.
    pending: Vec<(TcpStream, SocketAddr)>,
    next_at: Option<Instant>,
    first: Option<Fail>,
    /// The connection made, and its handshake in progress (TLS).
    conn: Option<Conn>,
    tls: Option<(String, Option<tls::Session>)>,
    /// The protocols offered in the handshake (ALPN).
    alpn: Vec<Vec<u8>>,
}

impl Dial {
    fn new(targets: Vec<Target>, port: u16, tls: Option<String>) -> Dial {
        Dial {
            targets: interleave(targets),
            port,
            next: 0,
            pending: Vec::new(),
            next_at: None,
            first: None,
            conn: None,
            tls: tls.map(|h| (h, None)),
            alpn: Vec::new(),
        }
    }

    /// A TLS handshake over a connection there already (`start_tls`).
    fn over(c: Conn, host: String) -> Dial {
        Dial {
            conn: Some(c),
            ..Dial::new(Vec::new(), 0, Some(host))
        }
    }

    fn step(&mut self, a: &mut Attempt) -> Result<Conn, Fail> {
        loop {
            if a.cancelled() {
                return Err(Fail::Cancelled);
            }
            if let Some(c) = &mut self.conn {
                // the race is over: the others close as they drop
                self.pending.clear();
                if let Some((host, session)) = &mut self.tls {
                    let st = match session {
                        Some(st) => st,
                        None => session.insert(tls::start(host, &self.alpn)?),
                    };
                    tls::handshake(a, c, st)?;
                    c.tls = session.take().map(Mutex::new);
                }
                return Ok(self.conn.take().expect("the connection"));
            }
            // the connects that have ended: the first that made it wins
            let mut i = 0;
            while i < self.pending.len() {
                if !writable_now(&self.pending[i].0) {
                    i += 1;
                    continue;
                }
                let (s, addr) = self.pending.swap_remove(i);
                match sock::finish_connect(&s) {
                    Ok(()) => {
                        self.conn = Some(Conn::new(s, addr));
                        break;
                    }
                    Err(e) => {
                        self.first.get_or_insert(Fail::Network(oserr::reason(&e)));
                    }
                }
            }
            if self.conn.is_some() {
                continue;
            }
            let more = self.next < self.targets.len();
            let due = self.next_at.is_none_or(|at| Instant::now() >= at);
            if more && (self.pending.is_empty() || due) {
                let addr = self.targets[self.next].socket(self.port);
                self.next += 1;
                match sock::start_connect(&addr) {
                    Ok((s, true)) => self.conn = Some(Conn::new(s, addr)),
                    Ok((s, false)) => {
                        self.pending.push((s, addr));
                        self.next_at = Some(Instant::now() + ATTEMPT_DELAY);
                    }
                    Err(e) => {
                        self.first.get_or_insert(Fail::Network(oserr::reason(&e)));
                    }
                }
                continue;
            }
            if self.pending.is_empty() {
                return Err(self
                    .first
                    .take()
                    .unwrap_or_else(|| Fail::Network("no such host".into())));
            }
            let fds = self
                .pending
                .iter()
                .map(|(s, _)| (s.as_raw_fd(), Interest::Write))
                .collect();
            a.blocked = Some(IoStep::WaitAny(fds, self.next_at.filter(|_| more)));
            return Err(Fail::Block);
        }
    }
}

/// Connects to `targets` (and makes a TLS client of `tls`'s host) on the
/// event loop: `Ok(<conn>)`.
fn dial(
    t: &mut Task<'_>,
    targets: Vec<Target>,
    port: u16,
    tls: Option<String>,
    alpn: Vec<Vec<u8>>,
) -> Result<Flow, Stop> {
    run_dial(
        t,
        Dial {
            alpn,
            ..Dial::new(targets, port, tls)
        },
    )
}

/// The `alpn` argument: nil or a list of protocol names (strings), each
/// 1 to 255 bytes (RFC 7301).
fn need_alpn(v: &Value) -> Result<Vec<Vec<u8>>, Stop> {
    match v {
        Value::Nil => Ok(Vec::new()),
        Value::List(l) => l
            .iter()
            .map(|x| match x {
                Value::Str(s) if !s.is_empty() && s.len() <= 255 => Ok(s.to_vec()),
                _ => Err(nat::wrong_arg("alpn", "list of protocol names", v)),
            })
            .collect(),
        _ => Err(nat::wrong_arg("alpn", "list of protocol names", v)),
    }
}

fn run_dial(t: &mut Task<'_>, mut d: Dial) -> Result<Flow, Stop> {
    run_io(
        t,
        None,
        move |a| d.step(a),
        |r| result(r.map(|c| Value::Conn(Arc::new(c)))),
    )
}

/// A listener on `addr`: an empty host listens on every address
/// (`[::]`, both families; `0.0.0.0` where IPv6 is missing), a name on
/// its first IPv4 address (else its first). `found` are the addresses
/// of a name, looked up before.
fn listen(addr: &str, reuse_port: bool, found: Option<Vec<Target>>) -> Result<Listener, Fail> {
    let (host, port) = split_host_port(addr)
        .and_then(|(h, p)| Ok((h, parse_port(p)?)))
        .map_err(Fail::Invalid)?;
    let bound = if host.is_empty() {
        let any6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port);
        match sock::listen(&any6, reuse_port) {
            Err(e) if e.raw_os_error() == Some(libc::EAFNOSUPPORT) => sock::listen(
                &SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port),
                reuse_port,
            ),
            r => r,
        }
    } else {
        let ips = match found {
            Some(ips) => ips,
            None => literal(host).expect("a name is looked up first"),
        };
        let Some(ip) = ips.iter().find(|t| t.ip.is_ipv4()).or(ips.first()) else {
            return Err(Fail::Network("no such host".into()));
        };
        sock::listen(&ip.socket(port), reuse_port)
    };
    let l = bound.map_err(|e| Fail::Network(oserr::reason(&e)))?;
    let local = l
        .local_addr()
        .map_err(|e| Fail::Network(oserr::reason(&e)))?;
    Ok(Listener {
        sock: Mutex::new(Some(Arc::new(Counted::new(l)))),
        addr: sock::addr_text(&local),
    })
}

// ---- the natives ----

/// The connection argument.
fn need_conn<'a>(name: &str, v: &'a Value) -> Result<&'a Arc<Conn>, Stop> {
    match v {
        Value::Conn(c) => Ok(c),
        _ => Err(nat::wrong_arg(name, "conn", v)),
    }
}

/// `Ok(text)` of what was read, or the `Err` of text from outside past
/// the string limit (LB29, D22): `'io`, ``<what> is longer than the
/// string limit (100000000 characters)``.
fn read_result(r: Result<Vec<u8>, Fail>, what: &str) -> Value {
    match r {
        Ok(b) if !nat::fits_limit(&b) => nat::err_value("io", &nat::too_long(what), Vec::new()),
        r => result(r.map(|b| text(&b))),
    }
}

/// What was read, byte for byte (L4).
fn text(b: &[u8]) -> Value {
    Value::bytes(b.to_vec())
}

/// `connect(host, port, tls = false)`: `Ok(<conn>)`. With `tls`, the
/// connection is a TLS client of `host` (`tls.rs`). A name is looked up
/// on the pool first (a stop abandons the lookup).
fn connect_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let host = nat::need_str("host", &a[0])?.to_string();
    let port = nat::need_int("port", &a[1])?;
    let Value::Bool(tls) = a[2] else {
        return Err(nat::wrong_arg("tls", "bool", &a[2]));
    };
    let alpn = need_alpn(&a[3])?;
    let Ok(port) = u16::try_from(port) else {
        return Ok(Flow::Done(fail_value(Fail::Invalid(format!(
            "invalid port `{port}`"
        )))));
    };
    // No deadline of its own: a `timeout` around the call cuts the name
    // lookup, the connection and the handshake (`vm/deadline.rs`).
    let tls = tls.then(|| host.clone());
    if let Some(targets) = literal(&host) {
        return dial(t, targets, port, tls, alpn);
    }
    crate::vm::offload_kind(
        t,
        Kind::Lookup,
        move || lookup_targets(&host),
        move |t, r| match r {
            Ok(targets) => dial(t, targets, port, tls, alpn),
            Err(f) => Ok(Flow::Done(fail_value(f))),
        },
    )
}

/// `start_tls(c, host)`: `Ok(<conn>)`, a TLS client of
/// `host` over `c`'s socket, as `connect(tls: true)` makes one (stage 4d
/// Task 4: https through an HTTP proxy's CONNECT tunnel, which fung
/// cannot do over the other primitives). `c` reads as closed afterwards;
/// the timeout covers the handshake.
fn start_tls_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let c = need_conn("c", &a[0])?;
    let host = nat::need_str("host", &a[1])?.to_string();
    let alpn = need_alpn(&a[2])?;
    let plain = match c.take_over() {
        Ok(p) => p,
        Err(f) => return Ok(Flow::Done(fail_value(f))),
    };
    run_dial(
        t,
        Dial {
            alpn,
            ..Dial::over(plain, host)
        },
    )
}

/// `listen(addr)`: `Ok(<listener>)`. A name is looked up on the pool
/// first.
fn listen_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let addr = nat::need_str("addr", &a[0])?.to_string();
    let Value::Bool(reuse_port) = a[1] else {
        return Err(nat::wrong_arg("reuse_port", "bool", &a[1]));
    };
    let wrap = |r: Result<Listener, Fail>| result(r.map(|l| Value::Listener(Arc::new(l))));
    let host = split_host_port(&addr).map(|(h, _)| h.to_string());
    match host {
        // Only the lookup goes to the pool: the socket is made here, on the
        // run thread, which also starts the commands. Made on a pool thread
        // it could be inherited by a command started meanwhile (macOS sets
        // close-on-exec after making a socket), which would hold the port.
        Ok(h) if !h.is_empty() && literal(&h).is_none() => crate::vm::offload_kind(
            t,
            Kind::Lookup,
            move || lookup_targets(&h),
            move |_, r| {
                Ok(Flow::Done(wrap(
                    r.and_then(|ips| listen(&addr, reuse_port, Some(ips))),
                )))
            },
        ),
        _ => Ok(Flow::Done(wrap(listen(&addr, reuse_port, None)))),
    }
}

/// `local_addr(x)`: a connection's or listener's own address.
fn local_addr_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    match &a[0] {
        Value::Conn(c) => Ok(Value::str(&c.local)),
        Value::Listener(l) => Ok(Value::str(&l.addr)),
        v => Err(nat::wrong_arg("x", "conn | listener", v)),
    }
}

/// `accept(l)`: `Ok(<conn>)`, the next connection.
fn accept_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let Value::Listener(l) = &a[0] else {
        return Err(nat::wrong_arg("l", "listener", &a[0]));
    };
    let l = l.clone();
    run_io(
        t,
        None,
        move |a| l.accept(a),
        |r| result(r.map(|c| Value::Conn(Arc::new(c)))),
    )
}

/// A read of `c` that holds its read side to the end.
fn read_op<T: 'static>(
    t: &mut Task<'_>,
    c: Arc<Conn>,
    mut step: impl FnMut(&Conn, &mut Attempt) -> Result<T, Fail> + 'static,
    done: impl Fn(Result<T, Fail>) -> Value + 'static,
) -> Result<Flow, Stop> {
    let c2 = c.clone();
    run_io(t, Some((c, Side::Read)), move |a| step(&c2, a), done)
}

/// `read_line(c, limit)`: `Ok(line)` without its end, `Ok(nil)` at the
/// end of the stream.
fn read_line_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let c = need_conn("c", &a[0])?.clone();
    let limit = nat::need_count("limit", &a[1])?;
    read_op(
        t,
        c,
        move |c, a| c.read_line(a, limit),
        |r| result(r.map(|l| l.map_or(Value::Nil, |l| text(&l)))),
    )
}

/// A head's header lines being read (`read_fields`): the fields so far,
/// the one being gathered (its pieces, from folded lines), and the bytes
/// the head may still take.
struct Fields {
    out: Vec<(Vec<u8>, Vec<u8>)>,
    pending: Option<Vec<Vec<u8>>>,
    left: i64,
    max: usize,
}

/// How a `read_fields` ended, besides the stream's own failures.
enum FieldsEnd {
    /// The blank line: the fields and the budget left.
    Done(Vec<(Vec<u8>, Vec<u8>)>, i64),
    /// A line that is no header line, and why.
    Invalid(String),
    /// The stream ended before the blank line.
    Eof,
}

/// A token character (RFC 9110 5.6.2).
fn tchar(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

/// Without spaces and tabs at either end.
fn trim_ows(b: &[u8]) -> &[u8] {
    let start = b
        .iter()
        .position(|&c| c != b' ' && c != b'\t')
        .unwrap_or(b.len());
    let end = b
        .iter()
        .rposition(|&c| c != b' ' && c != b'\t')
        .map_or(start, |i| i + 1);
    &b[start..end]
}

impl Fields {
    /// The gathered field into `out`: a key of token characters (or
    /// spaces), as written, and a value without control characters but
    /// tab, without its leading blanks (the rules of `http.fung`).
    fn flush(&mut self) -> Result<(), String> {
        let Some(pieces) = self.pending.take() else {
            return Ok(());
        };
        let kv = pieces.join(&b' ');
        let colon = kv
            .iter()
            .position(|&b| b == b':')
            .expect("a line with a colon");
        let (k, v) = (&kv[..colon], &kv[colon + 1..]);
        let ok = !k.is_empty()
            && k.iter().all(|&b| b == b' ' || tchar(b))
            && !v.iter().any(|&b| (b < 32 && b != b'\t') || b == 127);
        if !ok {
            return Err(format!(
                "invalid header line `{}`",
                String::from_utf8_lossy(&kv)
            ));
        }
        let start = v
            .iter()
            .position(|&b| b != b' ' && b != b'\t')
            .unwrap_or(v.len());
        self.out.push((k.to_vec(), v[start..].to_vec()));
        Ok(())
    }

    /// Reads lines on until the blank one (or a stop): a line that starts
    /// with a space or tab continues the one before (obsolete folding,
    /// RFC 9112 5.2). Past the budget, or past `max` fields (0: no
    /// bound), it is a `'limit`.
    fn read(&mut self, c: &Conn, a: &mut Attempt) -> Result<FieldsEnd, Fail> {
        let too_long = || Fail::Limit("head too long".into());
        loop {
            if self.left < 0 {
                return Err(too_long());
            }
            let Some(line) = c.read_line(a, self.left as usize)? else {
                return Ok(match self.flush() {
                    Err(why) => FieldsEnd::Invalid(why),
                    Ok(()) => FieldsEnd::Eof,
                });
            };
            self.left -= line.len() as i64 + 2;
            let folded = line.first().is_some_and(|&b| b == b' ' || b == b'\t');
            if folded {
                match &mut self.pending {
                    None => {
                        return Ok(FieldsEnd::Invalid(format!(
                            "continuation line without a header `{}`",
                            String::from_utf8_lossy(&line)
                        )));
                    }
                    Some(p) => p.push(trim_ows(&line).to_vec()),
                }
            } else {
                if let Err(why) = self.flush() {
                    return Ok(FieldsEnd::Invalid(why));
                }
                if line.is_empty() {
                    if self.left < 0 {
                        return Err(too_long());
                    }
                    return Ok(FieldsEnd::Done(std::mem::take(&mut self.out), self.left));
                }
                if !line.contains(&b':') {
                    return Ok(FieldsEnd::Invalid(format!(
                        "header line without a colon `{}`",
                        String::from_utf8_lossy(&line)
                    )));
                }
                self.pending = Some(vec![trim_ows(&line).to_vec()]);
            }
            if self.max > 0 && self.out.len() > self.max {
                return Err(Fail::Limit("too many header lines".into()));
            }
        }
    }
}

/// `read_fields(c, budget, max)`: a head's header lines up to the blank
/// one, read here rather than line by line in fung (L13: a head of
/// thousands of lines cost the run thread hundreds of milliseconds).
/// `Ok((fields, budget left))` with `fields` a list of `(name, value)`,
/// names as written (LB26: `header_lines`); `Err` of kind `'limit` past the budget or
/// `max` fields (0: none), `'invalid` for a line that is no header line,
/// `'eof` when the stream ends first, else the stream's own.
fn read_fields_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let c = need_conn("c", &a[0])?.clone();
    let Value::Int(budget) = a[1] else {
        return Err(nat::wrong_arg("budget", "int", &a[1]));
    };
    let max = nat::need_count("max", &a[2])?;
    let mut st = Fields {
        out: Vec::new(),
        pending: None,
        left: budget,
        max,
    };
    read_op(
        t,
        c,
        move |c, a| st.read(c, a),
        |r| match r {
            Ok(FieldsEnd::Done(fields, left)) => {
                let list = fields
                    .into_iter()
                    .map(|(k, v)| Value::tuple(vec![text(&k), text(&v)]))
                    .collect();
                nat::ok(Value::tuple(vec![nat::list(list), Value::Int(left)]))
            }
            Ok(FieldsEnd::Invalid(why)) => nat::err_value("invalid", &why, vec![]),
            Ok(FieldsEnd::Eof) => nat::err_value("eof", "connection closed in the head", vec![]),
            Err(f) => fail_value(f),
        },
    )
}

/// `read_raw_line(c, limit)`: `Ok(line)` with its `\n`.
fn read_raw_line_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let c = need_conn("c", &a[0])?.clone();
    let limit = nat::need_count("limit", &a[1])?;
    read_op(
        t,
        c,
        move |c, a| c.read_raw_line(a, limit),
        |r| result(r.map(|l| text(&l))),
    )
}

/// `read_bytes(c, n)`: `Ok(text)` of exactly `n` bytes.
fn read_bytes_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let c = need_conn("c", &a[0])?.clone();
    let n = nat::need_count("n", &a[1])?;
    read_op(
        t,
        c,
        move |c, a| c.read_bytes(a, n),
        |r| read_result(r, "body"),
    )
}

/// `read_all(c, limit)`: `Ok(text)` up to the end of the stream.
fn read_all_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let c = need_conn("c", &a[0])?.clone();
    let limit = nat::need_count("limit", &a[1])?;
    read_op(
        t,
        c,
        move |c, a| c.read_all(a, limit),
        |r| read_result(r, "body"),
    )
}

/// `read_bytes_into(c, n, cap)`: `Ok(nil)` once `n` more raw bytes are
/// in `c`'s body buffer; `Err('limit)` (nothing read) if it would hold
/// more than `cap` bytes.
fn read_bytes_into_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let c = need_conn("c", &a[0])?.clone();
    let n = nat::need_count("n", &a[1])?;
    let cap = nat::need_count("cap", &a[2])?;
    read_op(
        t,
        c,
        move |c, a| c.read_bytes_into(a, n, cap),
        |r| result(r.map(|()| Value::Nil)),
    )
}

/// `take_body(c)`: `Ok(text)` of the body buffer, emptied, or its `Err`
/// past the string limit. Waits for a read in progress on `c` (in its
/// read side's line).
fn take_body_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let c = need_conn("c", &a[0])?.clone();
    read_op(t, c, |c, _| Ok(c.take_body()), |r| read_result(r, "body"))
}

/// `write(c, s)`: `Ok(nil)` once all of `s` is written.
fn write_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let c = need_conn("c", &a[0])?.clone();
    let bytes = nat::need_bytes("s", &a[1])?.to_vec();
    let mut off = 0;
    let c2 = c.clone();
    run_io(
        t,
        Some((c, Side::Write)),
        move |a| c2.write(a, &bytes, &mut off),
        |r| result(r.map(|()| Value::Nil)),
    )
}

/// `close_write(c)`: ends the sending side only (`shutdown(SHUT_WR)`,
/// after close_notify on TLS): the peer reads the end of what was sent
/// while the connection still reads what it sends (the server after 431,
/// L13). `Ok(nil)`.
fn close_write_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let c = need_conn("c", &a[0])?;
    let r = c.sock().map(|s| {
        if let Some(t) = &c.tls {
            tls::close_notify(t, &s.0);
        }
        let _ = s.0.shutdown(Shutdown::Write);
        Value::Nil
    });
    Ok(result(r))
}

/// How long `close` waits for a TLS connection's close_notify to go out.
const CLOSE_NOTIFY_WAIT: Duration = Duration::from_secs(1);

/// `close(x)`: closes a connection or a listener; `Ok(nil)`, an error
/// the second time (it was closed already: a likely bug of the caller).
/// The tasks waiting on it are woken first and see it closed. A TLS
/// connection sends close_notify first, waiting at most
/// `CLOSE_NOTIFY_WAIT` for the socket to take it (and the task's own
/// deadline and cancellation; L12).
fn close_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let r = match &a[0] {
        Value::Conn(c) if c.tls.is_some() => {
            if let Some(fd) = c.fd() {
                crate::vm::drop_io_waits(t, fd);
            }
            let s = match lock(&c.sock).take() {
                Some(s) => s,
                None => return Ok(Flow::Done(fail_value(c.closed()))),
            };
            let c = c.clone();
            tls::queue_close_notify(c.tls.as_ref().expect("a TLS connection"));
            let until = Instant::now() + CLOSE_NOTIFY_WAIT;
            return run_io(
                t,
                None,
                move |a| {
                    let st = c.tls.as_ref().expect("a TLS connection");
                    tls::flush_close(a, st, &s.0, until)?;
                    let _ = s.0.shutdown(Shutdown::Both);
                    Ok(())
                },
                |r| result(r.map(|()| Value::Nil)),
            );
        }
        Value::Conn(c) => {
            if let Some(fd) = c.fd() {
                crate::vm::drop_io_waits(t, fd);
            }
            c.close()
        }
        Value::Listener(l) => {
            if let Some(fd) = l.fd() {
                crate::vm::drop_io_waits(t, fd);
            }
            l.close()
        }
        v => return Err(nat::wrong_arg("x", "conn | listener", v)),
    };
    Ok(Flow::Done(result(r.map(|()| Value::Nil))))
}

/// `set_deadline(c, d: dur | nil)`: reads and writes on `c` from `d`
/// after now time out (`'timeout`; `0s` is now); `nil` removes the
/// deadline (LB39: as `timeout: nil` is no limit). It reaches the reads
/// and writes waiting already (L12): they are woken and look again under
/// the new deadline.
fn set_deadline_fn(t: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let c = need_conn("c", &a[0])?;
    let d = nat::opt_dur("d", &a[1])?;
    let r = c.set_deadline(d);
    if r.is_ok()
        && let Some(fd) = c.fd()
    {
        crate::vm::drop_io_waits(t, fd);
    }
    Ok(result(r.map(|()| Value::Nil)))
}

/// `user_agent()`: what the `http` client sends as `User-Agent` unless
/// the request sets one, `fung/<version>`.
fn user_agent_fn(_: &mut Task<'_>, _: Args) -> Result<Value, Stop> {
    Ok(text(
        format!("fung/{}", crate::version::version()).as_bytes(),
    ))
}

/// `url_encode(s)` (`http.url_encode`, D73): `s` with every byte but
/// the unreserved ones (RFC 3986 2.3: `A-Z a-z 0-9 - . _ ~`) written as
/// `%XX` (upper-case hex, 2.1), so it can stand as a query value or a
/// path segment whatever it holds (`&`, `=`, `/`, `+`, `#`, a space).
fn url_encode_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let s = nat::need_bytes("s", &a[0])?;
    let mut out = Vec::with_capacity(s.len());
    for &b in s {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b);
        } else {
            out.extend_from_slice(format!("%{b:02X}").as_bytes());
        }
    }
    Ok(text(&out))
}

/// `url_decode(s)` (`http.url_decode`, D73): `Ok` of `s` with each
/// `%XX` decoded to its byte (`+` stays `+`: RFC 3986, not a form), or
/// `Err({kind: 'invalid, message})` for a `%` without two hex digits, as
/// a URL with one is refused (D75).
fn url_decode_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let s = nat::need_bytes("s", &a[0])?;
    let hex = |b: u8| (b as char).to_digit(16);
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] != b'%' {
            out.push(s[i]);
            i += 1;
            continue;
        }
        match (
            s.get(i + 1).and_then(|&b| hex(b)),
            s.get(i + 2).and_then(|&b| hex(b)),
        ) {
            (Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            _ => {
                // the `%` and at most two characters after it
                let end: usize = i + fung_front::text::units(&s[i..])
                    .take(3)
                    .map(|(_, u)| u.byte_len())
                    .sum::<usize>();
                let bad = fung_front::text::lossy(&s[i..end]).into_owned();
                return Ok(nat::err_value(
                    "invalid",
                    &format!("invalid escape `{bad}`"),
                    Vec::new(),
                ));
            }
        }
    }
    Ok(nat::ok(text(&out)))
}

/// Test probes: the counters, and a gate that holds lookups of
/// `gate.invalid` (a lookup that waits as long as a test wants).
#[cfg(test)]
pub(crate) mod probe {
    use std::sync::atomic::Ordering;
    use std::sync::{Condvar, Mutex};

    use super::{IN_LINE, LOOKUPS, READ_WAITS, SOCKETS, WAITING, WRITE_WAITS, lock};

    pub(crate) fn in_line() -> usize {
        IN_LINE.load(Ordering::SeqCst)
    }

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
