//! Standard input, shared by every task (05).
//!
//! One reader thread owns the input and makes the reads; `read_line`
//! takes the next line from a queue. So two tasks reading at once each
//! get whole lines, every line once.
//!
//! Reads happen only on demand: the reader thread reads only while a task
//! waits for a line and none is queued, one buffer (`BUFFER`, 64 KiB;
//! `fung` gives the input that buffer) at a time, and queues every whole
//! line in it. A read takes what the input has, up to the buffer, so the
//! lines after the one asked for may be taken with it: a command after
//! fung in a shell reads from where that buffer ended (06), not from the
//! next line. Nothing is read before a task asks, so keys typed ahead on
//! a terminal stay there until then.
//!
//! A read error is no end of the input (CC10): it is queued after the
//! lines read before it and the read that takes it gives `Err`; the part
//! of a line read before it waits for the rest, and the next read tries
//! again. An input inherited non-blocking (O_NONBLOCK, which the
//! descriptor shares with whoever passed it on) is read through
//! [`blocking_reads`], which waits for it as a blocking read would.
//!
//! A task takes a queued line on the run thread, without waiting.
//! When the queue is empty it puts its waiter on the waiting list and
//! parks (`Stdin::poll`); the reader thread wakes it when lines come. The
//! park is one the world outside ends, so it is not a stuck task for the
//! deadlock check. Unlike a read the task made itself, that wait can end
//! early: a cancelled task stops waiting and unwinds with E0701, so an
//! `exit(n)` from another task ends the program while a task (or the
//! entry) still waits for a line nobody types. A read already made goes on; the lines it brings stay queued for the
//! next reader.
//!
//! The reader thread starts at the first read (most programs never read
//! standard input) and stops once the runtime is gone, unless it is
//! inside a read the operating system never ends; then it stays there
//! until the process exits. It handles bytes only: decoding into fung
//! strings happens on the run thread.

use std::collections::VecDeque;
use std::io::{BufRead, Read};
use std::os::fd::AsRawFd;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use super::block::{WaitList, Waiter};

/// The buffer the command reads standard input with: one read takes at
/// most this much. 64 KiB is the pipe buffer size on Linux, so one read
/// empties a full pipe.
pub const BUFFER: usize = 64 * 1024;

/// What a read of standard input brings a waiting task.
#[derive(Debug)]
pub(crate) enum Input {
    /// A line, with its line ending (the last one may have none).
    Line(Vec<u8>),
    /// The end of the input.
    End,
    /// A read failed; the next read tries again.
    Failed(std::io::Error),
}

/// A file read as standard input (`fung` gives it a duplicate of
/// descriptor 0): a read that finds nothing there on a non-blocking
/// descriptor waits until something is, with `poll(2)`, instead of
/// failing with `EAGAIN`. The descriptor's flags are shared with the
/// process that passed it on, so it is left non-blocking. Only the
/// reader thread reads it, which may block (see the module notes).
pub fn blocking_reads(f: std::fs::File) -> Box<dyn Read + Send> {
    Box::new(BlockingReads(f))
}

struct BlockingReads(std::fs::File);

/// How many reads of a `blocking_reads` input found nothing and waited
/// (a test probe; only one test reads such an input).
#[cfg(test)]
pub(crate) static NONBLOCKING_WAITS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

impl Read for BlockingReads {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            match self.0.read(buf) {
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    #[cfg(test)]
                    NONBLOCKING_WAITS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    wait_readable(self.0.as_raw_fd())?;
                }
                r => return r,
            }
        }
    }
}

/// Waits until `fd` can be read (or is at its end, or fails: the read
/// that follows says which).
fn wait_readable(fd: std::os::fd::RawFd) -> std::io::Result<()> {
    let mut p = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    loop {
        // SAFETY: `p` is one valid `pollfd` for the call.
        if unsafe { libc::poll(&mut p, 1, -1) } >= 0 {
            return Ok(());
        }
        let e = std::io::Error::last_os_error();
        if e.kind() != std::io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
}

/// The world's handle on standard input. Dropping it (with the world)
/// tells the reader thread to stop.
pub(crate) struct Stdin(Arc<Pump>);

struct Pump {
    state: Mutex<PumpState>,
    /// The reader thread waits here for a task to want a line.
    demand: Condvar,
}

struct PumpState {
    /// The input until the reader thread takes it.
    source: Option<Box<dyn BufRead + Send>>,
    /// The reader thread was started.
    started: bool,
    /// What the reads brought, in order.
    lines: VecDeque<Input>,
    /// Tasks waiting for a line, woken when one comes.
    waiting: WaitList,
    /// The runtime is gone: the reader thread stops.
    closed: bool,
}

impl PumpState {
    /// Whether the reader thread should read now: a task waits for a line
    /// and none is queued.
    fn wanted(&self) -> bool {
        !self.waiting.is_empty() && self.lines.is_empty()
    }
}

#[cfg(test)]
thread_local! {
    /// Set by a test: the reader thread cannot start (on this run thread).
    static FAIL_START: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Makes the reader thread's start fail or not (tests, on the run thread).
#[cfg(test)]
pub(crate) fn fail_start_for_tests(on: bool) {
    FAIL_START.set(on);
}

fn lock(m: &Mutex<PumpState>) -> MutexGuard<'_, PumpState> {
    // The state is plain containers that no panic leaves half-changed.
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Stdin {
    pub(crate) fn new(source: Box<dyn BufRead + Send>) -> Stdin {
        Stdin(Arc::new(Pump {
            state: Mutex::new(PumpState {
                source: Some(source),
                started: false,
                lines: VecDeque::new(),
                waiting: WaitList::default(),
                closed: false,
            }),
            demand: Condvar::new(),
        }))
    }

    /// What the next read brought if it is queued (a line, the end, or
    /// a read error), taken off for `me`; otherwise `me` is put on the
    /// waiting list, the reader thread is asked for a read, and `None`:
    /// the task parks until the reader thread wakes it
    /// (`read_on_demand`). Starts
    /// the reader thread the first time; when no thread can be made, the
    /// error (E0801 for the caller), and the input stays for a later read
    /// (review CC7).
    pub(crate) fn poll(&self, me: &Arc<Waiter>) -> std::io::Result<Option<Input>> {
        let mut s = lock(&self.0.state);
        if !s.started {
            self.start()?;
            s.started = true;
        }
        if let Some(line) = s.lines.pop_front() {
            s.waiting.remove(me);
            return Ok(Some(line));
        }
        if s.waiting.add(me) {
            self.0.demand.notify_one();
        }
        Ok(None)
    }

    /// Starts the reader thread, which takes the input once it runs: a
    /// thread that cannot be made leaves the input where it was.
    fn start(&self) -> std::io::Result<()> {
        #[cfg(test)]
        if FAIL_START.get() {
            return Err(std::io::Error::other("no thread (test)"));
        }
        let pump = self.0.clone();
        std::thread::Builder::new()
            .name("fung-stdin".into())
            .spawn(move || {
                let source = lock(&pump.state).source.take();
                if let Some(source) = source {
                    pump.read_on_demand(source);
                }
            })
            .map(drop)
    }

    /// A wait that ended without a line (cancelled, or past its
    /// deadline): `me` leaves the waiting list.
    pub(crate) fn leave(&self, me: &Arc<Waiter>) {
        lock(&self.0.state).waiting.remove(me);
    }

    /// Puts `lines` (in order) back at the front of the queue: the lines
    /// a stopped `read_lines` had taken (VA61) go to the next read. Those
    /// waiting for a line are woken to take them.
    pub(crate) fn unread(&self, lines: Vec<Vec<u8>>) {
        if lines.is_empty() {
            return;
        }
        let mut s = lock(&self.0.state);
        for l in lines.into_iter().rev() {
            s.lines.push_front(Input::Line(l));
        }
        s.waiting.wake_all();
    }

    /// How many tasks wait for a line (a test probe).
    #[cfg(test)]
    pub(crate) fn waiting(&self) -> usize {
        lock(&self.0.state).waiting.len()
    }
}

impl Drop for Stdin {
    fn drop(&mut self) {
        lock(&self.0.state).closed = true;
        self.0.demand.notify_all();
    }
}

impl Pump {
    /// The reader thread: whenever a task wants a line and none is
    /// queued, makes one read (`fill_buf`: what the input has, up to the
    /// buffer, waiting only when nothing is there), queues the whole
    /// lines in it and wakes the waiting tasks. A line split across reads
    /// waits in `part` for its end; the thread reads on while a task
    /// still wants a line. A read error is queued as it is, and `part`
    /// keeps waiting.
    fn read_on_demand(&self, mut source: Box<dyn BufRead + Send>) {
        let mut part = Vec::new();
        // A line past the string limit stops here (LB29, D22): what was
        // gathered goes and the read gives its `Err`; the line's rest is
        // read on as the next line.
        let mut count = crate::native::nat::CharCount::default();
        loop {
            {
                let mut s = lock(&self.state);
                while !s.wanted() {
                    if s.closed {
                        return;
                    }
                    s = self.demand.wait(s).unwrap_or_else(PoisonError::into_inner);
                }
            }
            let mut got = Vec::new();
            match source.fill_buf() {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => got.push(Input::Failed(e)),
                Ok([]) => {
                    if !part.is_empty() {
                        got.push(Input::Line(std::mem::take(&mut part)));
                        count = Default::default();
                    }
                    got.push(Input::End);
                }
                Ok(chunk) => {
                    let n = chunk.len();
                    let mut rest = chunk;
                    while let Some(i) = rest.iter().position(|&b| b == b'\n') {
                        part.extend_from_slice(&rest[..=i]);
                        got.push(Input::Line(std::mem::take(&mut part)));
                        count = Default::default();
                        rest = &rest[i + 1..];
                    }
                    part.extend_from_slice(rest);
                    source.consume(n);
                    if !count.within(&part) {
                        part = Vec::new();
                        count = Default::default();
                        got.push(Input::Failed(crate::native::nat::too_long_error("line")));
                    }
                }
            }
            if got.is_empty() {
                continue;
            }
            let mut s = lock(&self.state);
            s.lines.extend(got);
            s.waiting.wake_all();
        }
    }
}
