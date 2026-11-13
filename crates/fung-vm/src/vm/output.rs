//! Standard output and error, written by a thread of their own (CC15,
//! D64).
//!
//! A write to a pipe or a terminal blocks while the reader does not
//! read. Made on the run thread, such a write held every task, every
//! deadline and the first signal's stop until the reader read. So the
//! run thread only queues what `print`, `eprint` and `dbg` write, and a
//! writer thread writes it, in order. The descriptors stay as they are:
//! `O_NONBLOCK` would reach the processes that share them (the shell, the
//! other end of a pipe).
//!
//! **Room.** A lane holds at most `ROOM` bytes that are not written yet,
//! like a pipe's buffer: a write that finds the room used up queues its
//! bytes and parks until the writer thread has written enough
//! (`write_std`). A park the world outside ends: no deadlock, and a
//! deadline, a cancellation or a stop ends it (what was queued is still
//! written). A program that prints much faster than its reader reads
//! waits as it did; a quick reader sees one write per batch, not per
//! line.
//!
//! **Lanes.** Standard output and error that go to one place (a
//! terminal, `2>&1`) share one lane, so the lines keep the order they
//! were printed in. Apart (`fung x | less`), each has its own lane and
//! thread: a reader that stops reading one does not hold the other.
//!
//! **The end.** A run waits until everything queued is written before it
//! returns (`Task::run_root`), and so does the runtime's release and
//! drop: what the command writes afterwards (a report, the REPL's prompt)
//! comes after, and nothing queued is lost at the exit. A write that
//! failed (no space left) fails the next write to that stream, or the run
//! at its end (E0801). A closed pipe ends the process by SIGPIPE in the
//! writer thread (`sigpipe.rs`), as it did in the run thread, but only
//! once what the program wrote before to the other lane is out: every
//! piece has a number in the order it was queued, across both lanes, so
//! a later `print` that meets the closed pipe does not lose an earlier
//! `eprint` still waiting in the other lane.
//!
//! The writer thread starts at the first write and stops when the runtime
//! is gone. Where no thread can be made the caller writes itself, as
//! before.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError, Weak};

use super::block::{WaitList, Waiter};
use super::{Flow, Stop, Task, Wake};
use crate::value::Value;

/// How many bytes a lane holds that are not written yet before a task's
/// write waits (64 KiB, a Linux pipe's buffer).
pub(crate) const ROOM: usize = 64 * 1024;

/// Which standard stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Std {
    Out = 0,
    Err = 1,
}

/// The world's standard output and error: a lane each, or one for both.
pub(crate) struct Output {
    lanes: [Arc<Lane>; 2],
}

/// Bytes on their way to one or both streams, and the thread writing
/// them.
pub(crate) struct Lane {
    state: Mutex<LaneState>,
    /// The writer thread waits here for bytes.
    more: Condvar,
    /// `drain` waits here for the writer thread.
    written: Condvar,
    /// The streams; taken by the writer thread while it writes a piece
    /// (only one writes at a time: the thread, or a caller without one).
    sinks: Mutex<[Option<Box<dyn Write + Send>>; 2]>,
    /// The next piece's number, shared by the two lanes.
    order: Arc<AtomicU64>,
    /// The other lane, when the streams are written apart.
    other: OnceLock<Weak<Lane>>,
}

/// Bytes queued for one stream: the writes numbered `first..=last`,
/// queued one after the other with nothing queued between them.
struct Piece {
    which: Std,
    bytes: Vec<u8>,
    first: u64,
    last: u64,
}

struct LaneState {
    /// What was queued, in order.
    queue: VecDeque<Piece>,
    /// The number of the piece being written, if one is.
    writing: Option<u64>,
    /// Bytes queued and not yet written (the piece being written
    /// included).
    pending: usize,
    /// Tasks waiting for room.
    waiting: WaitList,
    /// A write that failed, per stream, until a write or the end takes
    /// it.
    failed: [Option<io::Error>; 2],
    /// The writer thread was started (or could not be).
    started: bool,
    /// No writer thread could be made: callers write themselves.
    inline: bool,
    /// The writer thread waits for bytes.
    idle: bool,
    /// The runtime is gone: the writer thread ends once the queue is
    /// empty.
    closed: bool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // Plain containers that no panic leaves half-changed.
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Output {
    /// Standard output and error; `apart` gives each a lane of its own
    /// (they go to different places).
    pub(crate) fn new(
        stdout: Box<dyn Write + Send>,
        stderr: Box<dyn Write + Send>,
        apart: bool,
    ) -> Output {
        let order = Arc::new(AtomicU64::new(0));
        if apart {
            let out = Lane::new([Some(stdout), None], order.clone());
            let err = Lane::new([None, Some(stderr)], order);
            let _ = out.other.set(Arc::downgrade(&err));
            let _ = err.other.set(Arc::downgrade(&out));
            Output { lanes: [out, err] }
        } else {
            let one = Lane::new([Some(stdout), Some(stderr)], order);
            Output {
                lanes: [one.clone(), one],
            }
        }
    }

    /// The lane `which` is written through.
    pub(crate) fn lane(&self, which: Std) -> &Arc<Lane> {
        &self.lanes[which as usize]
    }

    /// A writer that queues on `which` and never waits (reports, and
    /// writes off the run thread): `Write` for the world's streams.
    pub(crate) fn queued(&self, which: Std) -> Queued {
        Queued {
            lane: self.lane(which).clone(),
            which,
        }
    }

    /// Waits until everything queued is written; the first failed write
    /// not reported yet, if any.
    pub(crate) fn drain(&self) -> Option<io::Error> {
        let mut first = None;
        for (i, lane) in self.lanes.iter().enumerate() {
            if i == 1 && Arc::ptr_eq(lane, &self.lanes[0]) {
                break;
            }
            lane.drain();
        }
        for which in [Std::Out, Std::Err] {
            if let Some(e) = self.lane(which).take_error(which) {
                first.get_or_insert(e);
            }
        }
        first
    }
}

impl Drop for Output {
    /// Writes what is left, then lets the writer threads end.
    fn drop(&mut self) {
        for lane in &self.lanes {
            lane.drain();
            lock(&lane.state).closed = true;
            lane.more.notify_all();
        }
    }
}

impl Lane {
    fn new(sinks: [Option<Box<dyn Write + Send>>; 2], order: Arc<AtomicU64>) -> Arc<Lane> {
        Arc::new(Lane {
            state: Mutex::new(LaneState {
                queue: VecDeque::new(),
                writing: None,
                pending: 0,
                waiting: WaitList::default(),
                failed: [None, None],
                started: false,
                inline: false,
                idle: false,
                closed: false,
            }),
            more: Condvar::new(),
            written: Condvar::new(),
            sinks: Mutex::new(sinks),
            order,
            other: OnceLock::new(),
        })
    }

    /// Queues `bytes` for `which`; whether the lane still has room (else
    /// the caller waits for it, `room`). Without a writer thread they are
    /// written here.
    pub(crate) fn put(self: &Arc<Self>, which: Std, bytes: Vec<u8>) -> bool {
        if bytes.is_empty() {
            return true;
        }
        let mut s = lock(&self.state);
        let n = self.order.fetch_add(1, Ordering::Relaxed);
        if !s.started {
            s.started = true;
            s.inline = self.start().is_err();
        }
        // The runtime is gone (a late report from the closer): no thread
        // writes any more.
        if s.inline || s.closed {
            drop(s);
            let r = write_to(&mut lock(&self.sinks), which, &bytes);
            match r {
                // Does not come back, but in a test that records it.
                Err(e) if e.kind() == io::ErrorKind::BrokenPipe => self.die_after_earlier(n),
                Err(e) => {
                    lock(&self.state).failed[which as usize].get_or_insert(e);
                }
                Ok(()) => {}
            }
            return true;
        }
        s.pending += bytes.len();
        match s.queue.back_mut() {
            Some(p) if p.which == which && p.last + 1 == n => {
                p.bytes.extend_from_slice(&bytes);
                p.last = n;
            }
            _ => s.queue.push_back(Piece {
                which,
                bytes,
                first: n,
                last: n,
            }),
        }
        if s.idle {
            s.idle = false;
            self.more.notify_one();
        }
        s.pending <= ROOM
    }

    /// Whether there is room again: then `me` leaves the waiting list;
    /// else it is on it, and the writer thread wakes it.
    fn room(&self, me: &Arc<Waiter>) -> bool {
        let mut s = lock(&self.state);
        if s.pending <= ROOM {
            s.waiting.remove(me);
            return true;
        }
        s.waiting.add(me);
        false
    }

    /// A wait for room that ended otherwise (a stop): `me` leaves.
    fn leave(&self, me: &Waiter) {
        lock(&self.state).waiting.remove(me);
    }

    /// The failure of an earlier write to `which`, taken.
    pub(crate) fn take_error(&self, which: Std) -> Option<io::Error> {
        lock(&self.state).failed[which as usize].take()
    }

    /// Waits until every piece numbered below `n` is written.
    fn written_before(&self, n: u64) {
        #[cfg(test)]
        if let Some((_, waiting)) = *lock(&TEST_DIE) {
            waiting();
        }
        let mut s = lock(&self.state);
        while s
            .writing
            .or_else(|| s.queue.front().map(|p| p.first))
            .is_some_and(|first| first < n)
        {
            s = self.written.wait(s).unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// A write to a stream whose reader has gone (EPIPE), numbered `n`:
    /// once the other lane has written what was queued before it, the
    /// process ends by SIGPIPE.
    fn die_after_earlier(&self, n: u64) {
        if let Some(other) = self.other.get().and_then(Weak::upgrade) {
            other.written_before(n);
        }
        die_by_sigpipe();
    }

    /// Waits until everything queued is written.
    fn drain(&self) {
        let mut s = lock(&self.state);
        while s.pending > 0 {
            s = self.written.wait(s).unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn start(self: &Arc<Self>) -> io::Result<()> {
        #[cfg(test)]
        if NO_THREAD.get() {
            return Err(io::Error::other("no thread (test)"));
        }
        let lane = self.clone();
        std::thread::Builder::new()
            .name("fung-output".into())
            .spawn(move || lane.write_on())
            .map(drop)
    }

    /// The writer thread: writes what was queued, a piece at a time in
    /// order, and wakes the tasks waiting for room once there is.
    fn write_on(&self) {
        loop {
            let piece = {
                let mut s = lock(&self.state);
                while s.queue.is_empty() {
                    if s.closed {
                        return;
                    }
                    s.idle = true;
                    s = self.more.wait(s).unwrap_or_else(PoisonError::into_inner);
                }
                let p = s.queue.pop_front().expect("a piece");
                s.writing = Some(p.first);
                p
            };
            let r = match write_to(&mut lock(&self.sinks), piece.which, &piece.bytes) {
                // Does not come back, but in a test that records it.
                Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
                    self.die_after_earlier(piece.first);
                    Ok(())
                }
                r => r,
            };
            let mut s = lock(&self.state);
            if let Err(e) = r {
                s.failed[piece.which as usize].get_or_insert(e);
            }
            s.writing = None;
            s.pending -= piece.bytes.len();
            if s.pending <= ROOM {
                s.waiting.wake_all();
            }
            // `drain` and the other lane's `written_before` wait here.
            self.written.notify_all();
        }
    }
}

/// Writes `bytes` to the stream `which` of `sinks` (each write goes out
/// at once: the command's streams flush).
fn write_to(
    sinks: &mut [Option<Box<dyn Write + Send>>; 2],
    which: Std,
    bytes: &[u8],
) -> io::Result<()> {
    match &mut sinks[which as usize] {
        Some(w) => w.write_all(bytes).and_then(|()| w.flush()),
        None => Ok(()),
    }
}

/// Ends the process by SIGPIPE (`sigpipe::die`); a test records it.
fn die_by_sigpipe() {
    #[cfg(test)]
    if let Some((record, _)) = *lock(&TEST_DIE) {
        record();
        return;
    }
    crate::sigpipe::die();
}

/// Set by a test: what a death by SIGPIPE does instead, and what a lane
/// does as it starts to wait for the other lane's earlier pieces.
#[cfg(test)]
static TEST_DIE: Mutex<Option<TestDie>> = Mutex::new(None);

/// A test's death by SIGPIPE: what it records, and what it does as the
/// dying lane starts to wait.
#[cfg(test)]
type TestDie = (fn(), fn());

/// Makes a death by SIGPIPE call `record` instead (a test's own stream
/// that fails with EPIPE), and calls `waiting` when the dying lane starts
/// to wait for the other lane.
#[cfg(test)]
pub(crate) fn die_for_tests(record: fn(), waiting: fn()) {
    *lock(&TEST_DIE) = Some((record, waiting));
}

#[cfg(test)]
thread_local! {
    /// Set by a test: the writer thread cannot start (lanes made on this
    /// thread's first write).
    static NO_THREAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Makes the writer thread's start fail or not (tests, on the run thread).
#[cfg(test)]
pub(crate) fn no_thread_for_tests(on: bool) {
    NO_THREAD.set(on);
}

/// A stream's `Write` that queues and never waits for room: the reports
/// the runtime writes itself (a deadlock, a failure nobody handled) and
/// the writes made off the run thread (the closer's). The run's end
/// writes them out.
pub(crate) struct Queued {
    lane: Arc<Lane>,
    which: Std,
}

impl Write for Queued {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.lane.put(self.which, b.to_vec());
        Ok(b.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// How a task waits for room: the writer thread ends the wait, so it is
/// no deadlock.
const ROOM_WAKE: Wake = Wake {
    until: None,
    timed: true,
};

/// A task's write of `bytes` to `which` (`print`, `eprint`, `dbg`): an
/// earlier write that failed fails this one (E0801); else the bytes are
/// queued, and the task parks while the lane has no room. Its value is
/// `v`. A stop ends the wait; the bytes stay queued.
pub(crate) fn write_std(
    t: &mut Task<'_>,
    which: Std,
    bytes: Vec<u8>,
    v: Value,
) -> Result<Flow, Stop> {
    let lane = t.world.output.lane(which).clone();
    if let Some(e) = lane.take_error(which) {
        return Err(crate::native::nat::write_err(e));
    }
    if lane.put(which, bytes) {
        return Ok(Flow::Done(v));
    }
    let left = lane.clone();
    let mut v = Some(v);
    super::park_until_then(
        t,
        move |_, me| {
            lane.room(me)
                .then(|| Ok(v.take().expect("a write ends once")))
        },
        ROOM_WAKE,
        move |t| left.leave(&t.waiter_handle()),
    )
}

/// Whether the process's standard output and error go to different
/// places: different files, or one of them not open. Then they are
/// written apart (see the module notes).
pub fn std_streams_apart() -> bool {
    use std::os::fd::AsFd;
    use std::os::unix::fs::MetadataExt;
    let id = |fd: std::os::fd::BorrowedFd<'_>| {
        let f = std::fs::File::from(fd.try_clone_to_owned().ok()?);
        let m = f.metadata().ok()?;
        Some((m.dev(), m.ino()))
    };
    match (id(io::stdout().as_fd()), id(io::stderr().as_fd())) {
        (Some(a), Some(b)) => a != b,
        _ => true,
    }
}
