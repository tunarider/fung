//! Standard input, shared by every task (05; Go `Task.ReadLine`).
//!
//! One reader thread owns the input and reads lines ahead into a
//! queue; `read_line` takes the next one. So two tasks reading at once
//! each get whole lines, every line once (Go guards its `bufio.Reader`
//! with a mutex for the same reason).
//!
//! A task takes a queued line with the run token held, without waiting.
//! When the queue is empty it waits outside the token (`outside`), so the
//! other tasks run meanwhile and the wait is not a blocked task for the
//! deadlock check. Unlike a read the task made itself, that wait can end
//! early: a cancelled task stops waiting and unwinds with E0701, so an
//! `exit(n)` from another task ends the program while a task (or the
//! entry) still waits for a line nobody types. Go's read cannot be cut
//! short, so Go's program waits for the line first (open-questions).
//! The line, if it comes, stays queued for the next reader.
//!
//! The reader thread starts at the first read (most programs never read
//! standard input), reads ahead about `AHEAD_BYTES` (Go's `bufio`
//! reads ahead too), and stops once the runtime is gone and it is not
//! inside a read; a read the operating system never ends keeps it parked
//! there until the process exits. It handles bytes only: decoding into
//! fung strings happens with the token held.

use std::collections::VecDeque;
use std::io::BufRead;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use super::block::Waiter;
use super::task::CancelNode;

/// How far the reader thread reads ahead of the program.
const AHEAD_BYTES: usize = 64 << 10;

/// The world's handle on standard input. Dropping it (with the world)
/// tells the reader thread to stop.
pub(crate) struct Stdin(Arc<Pump>);

struct Pump {
    state: Mutex<PumpState>,
    /// The reader thread waits here for room in the queue.
    room: Condvar,
}

struct PumpState {
    /// The input until the reader thread takes it.
    source: Option<Box<dyn BufRead + Send>>,
    /// Lines read ahead, each with its line ending; `None` is the end of
    /// the input (or a read error), queued once until it is taken.
    lines: VecDeque<Option<Vec<u8>>>,
    /// The bytes in `lines`.
    bytes: usize,
    /// The end of the input is queued (the reader thread waits until it
    /// is taken before it reads again).
    ended: bool,
    /// The reader thread waits for room (`Pump::room`).
    parked: bool,
    /// Tasks waiting for a line, woken when one comes.
    waiting: Vec<Arc<Waiter>>,
    /// The runtime is gone: the reader thread stops.
    closed: bool,
}

impl PumpState {
    /// Whether the reader thread should read another line now.
    fn wants_more(&self) -> bool {
        !self.closed && self.bytes < AHEAD_BYTES && !self.ended
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
                bytes: 0,
                ended: false,
                parked: false,
                waiting: Vec::new(),
                closed: false,
            }),
            room: Condvar::new(),
        }))
    }

    /// The next line if one is queued (`Some(None)` at the end of the
    /// input), `None` if the caller has to wait. Starts the reader thread
    /// at the first call.
    pub(crate) fn try_take(&self) -> Option<Option<Vec<u8>>> {
        let mut s = lock(&self.0.state);
        if let Some(source) = s.source.take() {
            let pump = self.0.clone();
            std::thread::Builder::new()
                .name("fung-stdin".into())
                .spawn(move || pump.read_ahead(source))
                .expect("start the standard input reader");
        }
        self.0.take(&mut s)
    }

    /// A handle for waiting outside the run token.
    pub(crate) fn handle(&self) -> Reader {
        Reader(self.0.clone())
    }

    /// How many tasks wait for a line (a test probe).
    #[cfg(test)]
    pub(crate) fn waiting(&self) -> usize {
        lock(&self.0.state).waiting.len()
    }
}

/// Waits for standard input without the run token (`Stdin::handle`).
pub(crate) struct Reader(Arc<Pump>);

impl Reader {
    /// Waits for the next line without the run token (the caller runs it
    /// in `outside`): the line, or `None` once `cancel` is cancelled.
    /// `me` must be registered on `cancel` and its woken flag cleared
    /// before the call, so no wake is lost.
    pub(crate) fn wait(&self, me: &Arc<Waiter>, cancel: &CancelNode) -> Option<Option<Vec<u8>>> {
        loop {
            {
                let mut s = lock(&self.0.state);
                if let Some(line) = self.0.take(&mut s) {
                    s.waiting.retain(|w| !Arc::ptr_eq(w, me));
                    return Some(line);
                }
                if cancel.is_cancelled() {
                    s.waiting.retain(|w| !Arc::ptr_eq(w, me));
                    return None;
                }
                if !s.waiting.iter().any(|w| Arc::ptr_eq(w, me)) {
                    s.waiting.push(me.clone());
                }
            }
            while !me.take_woken() {
                std::thread::park();
            }
        }
    }
}

impl Drop for Stdin {
    fn drop(&mut self) {
        lock(&self.0.state).closed = true;
        self.0.room.notify_all();
    }
}

impl Pump {
    /// Takes the next queued line and makes room for the reader thread.
    fn take(&self, s: &mut PumpState) -> Option<Option<Vec<u8>>> {
        let line = s.lines.pop_front()?;
        match &line {
            Some(b) => s.bytes -= b.len(),
            None => s.ended = false,
        }
        // The reader thread is woken once the queue is half empty (or the
        // end was taken), not for every line: it then reads a run of lines
        // while the program takes them.
        if s.parked && (s.bytes <= AHEAD_BYTES / 2 || line.is_none()) {
            s.parked = false;
            self.room.notify_one();
        }
        Some(line)
    }

    /// The reader thread: while there is room, reads what the input has
    /// (one `fill_buf`: whatever is there, waiting only when nothing is),
    /// queues the whole lines in it at once and wakes the tasks waiting
    /// for one. A line split across reads waits in `part` for its end. A
    /// terminal gives a line per read, so a typed line is queued at once.
    fn read_ahead(&self, mut source: Box<dyn BufRead + Send>) {
        let mut part = Vec::new();
        loop {
            {
                let mut s = lock(&self.state);
                while !s.wants_more() {
                    if s.closed {
                        return;
                    }
                    s.parked = true;
                    s = self.room.wait(s).unwrap_or_else(PoisonError::into_inner);
                    s.parked = false;
                }
            }
            let mut lines = Vec::new();
            let end = match source.fill_buf() {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                // The end, or an error (Go: `io.EOF` once nothing is left).
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
            for line in lines {
                s.bytes += line.len();
                s.lines.push_back(Some(line));
            }
            if end {
                s.lines.push_back(None);
                s.ended = true;
            }
            for w in s.waiting.drain(..) {
                w.wake();
            }
        }
    }
}
