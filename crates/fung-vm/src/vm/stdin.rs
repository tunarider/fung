//! Standard input, shared by every task (05).
//!
//! One reader thread owns the input and makes the reads; `read_line`
//! takes the next line from a queue. So two tasks reading at once each
//! get whole lines, every line once.
//!
//! Reads happen only on demand: the reader thread reads only while a task
//! waits for a line and none is queued, one buffer (`BUFFER`, 64 KiB;
//! `fung` gives the input that buffer) at a time, and queues every whole
//! line in it. What the
//! program never asked for stays in the input: a command after fung in
//! a shell reads it, and keys typed ahead on a terminal are not taken.
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
use std::io::BufRead;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use super::block::Waiter;

/// The buffer the command reads standard input with: one read takes at
/// most this much. 64 KiB is the pipe buffer size on Linux, so one read
/// empties a full pipe.
pub const BUFFER: usize = 64 * 1024;

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
    /// Lines read, each with its line ending; `None` is the end of the
    /// input (or a read error).
    lines: VecDeque<Option<Vec<u8>>>,
    /// Tasks waiting for a line, woken when one comes.
    waiting: Vec<Arc<Waiter>>,
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

fn lock(m: &Mutex<PumpState>) -> MutexGuard<'_, PumpState> {
    // The state is plain containers that no panic leaves half-changed.
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Stdin {
    pub(crate) fn new(source: Box<dyn BufRead + Send>) -> Stdin {
        Stdin(Arc::new(Pump {
            state: Mutex::new(PumpState {
                source: Some(source),
                lines: VecDeque::new(),
                waiting: Vec::new(),
                closed: false,
            }),
            demand: Condvar::new(),
        }))
    }

    /// The next line if one is queued (`Some(None)` at the end of the
    /// input), taken off for `me`; otherwise `me` is put on the waiting
    /// list, the reader thread is asked for a read, and `None`: the task
    /// parks until the reader thread wakes it (`read_on_demand`). Starts
    /// the reader thread the first time.
    pub(crate) fn poll(&self, me: &Arc<Waiter>) -> Option<Option<Vec<u8>>> {
        let mut s = lock(&self.0.state);
        if let Some(source) = s.source.take() {
            let pump = self.0.clone();
            std::thread::Builder::new()
                .name("fung-stdin".into())
                .spawn(move || pump.read_on_demand(source))
                .expect("start the standard input reader");
        }
        if let Some(line) = s.lines.pop_front() {
            s.waiting.retain(|w| !Arc::ptr_eq(w, me));
            return Some(line);
        }
        if !s.waiting.iter().any(|w| Arc::ptr_eq(w, me)) {
            s.waiting.push(me.clone());
            self.0.demand.notify_one();
        }
        None
    }

    /// A wait that ended without a line (cancelled, or past its
    /// deadline): `me` leaves the waiting list.
    pub(crate) fn leave(&self, me: &Arc<Waiter>) {
        lock(&self.0.state).waiting.retain(|w| !Arc::ptr_eq(w, me));
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
    /// still wants a line.
    fn read_on_demand(&self, mut source: Box<dyn BufRead + Send>) {
        let mut part = Vec::new();
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
            let mut lines = Vec::new();
            let end = match source.fill_buf() {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                // The end, or an error.
                Err(_) => true,
                Ok([]) => true,
                Ok(chunk) => {
                    let n = chunk.len();
                    let mut rest = chunk;
                    while let Some(i) = rest.iter().position(|&b| b == b'\n') {
                        part.extend_from_slice(&rest[..=i]);
                        lines.push(std::mem::take(&mut part));
                        rest = &rest[i + 1..];
                    }
                    part.extend_from_slice(rest);
                    source.consume(n);
                    false
                }
            };
            if end && !part.is_empty() {
                lines.push(std::mem::take(&mut part));
            }
            if lines.is_empty() && !end {
                continue;
            }
            let mut s = lock(&self.state);
            s.lines.extend(lines.into_iter().map(Some));
            if end {
                s.lines.push_back(None);
            }
            for w in s.waiting.drain(..) {
                w.wake();
            }
        }
    }
}
