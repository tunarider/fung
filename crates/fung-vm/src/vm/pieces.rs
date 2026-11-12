//! Long natives done a piece at a time (L6). The run thread is shared by
//! every task (M:1, `sched.rs`): fung code gives its turn every
//! `YIELD_EVERY` calls, but a native that ran to its end in one go (a big
//! `json.parse`, a sort of millions) kept every other task waiting and
//! its own deadline unseen for as long as its input was big.
//!
//! A native whose work grows with its input does it as [`Work`]: a piece
//! of about `PIECE` (about a millisecond on the run thread) at a time.
//! After each piece the task's deadline is looked at as at a turn's end,
//! and the task gives its turn: it parks with a wake that is already due
//! (`Wake::soon`), so the scheduler queues it again behind the others, and
//! its box goes on where it stopped. A cancellation or a deadline that
//! came meanwhile unwinds it there; what it made so far is dropped (the
//! work changes nothing outside, so there is nothing to finish, 05).
//!
//! A small input is one piece: the native gives its result at once, with
//! no box and no trip through the scheduler. In a swap's function, which
//! cannot park (`atom.rs`), the pieces run one after another.

use std::rc::Rc;
use std::time::{Duration, Instant};

use super::cont::{Cont, Flow, Next, Wake};
use super::{Stop, Task};
use crate::value::{List, Value};

/// How long a piece runs before the task gives its turn: steps are done
/// one after another until this much time has passed. A count alone
/// (L6's first take) made pieces as long as the machine was slow: the
/// counts were measured here, and a CI runner took three to four times as
/// long a step (the L6 note, 6).
#[cfg(not(test))]
const PIECE_TIME: Duration = Duration::from_millis(1);
/// In the unit tests every step is a piece, so the parks they count do
/// not hang on the machine's speed.
#[cfg(test)]
const PIECE_TIME: Duration = Duration::ZERO;

/// About an eighth of a millisecond of work for a step of the natives
/// below, by what they count: bytes read or written, items, comparisons,
/// items copied. Measured on an Apple M-series release build (L6 note):
/// JSON reads about 75 MB/s and writes about 150 MB/s, scanning a string
/// for a separator well over 1 GB/s, a merge step about 5 ns, a copy
/// about 3 ns. Slower machines take longer steps but not longer pieces.
pub(crate) const STEP_BYTES: usize = 8 * 1024;
pub(crate) const STEP_SCAN: usize = 128 * 1024;
pub(crate) const STEP_ITEMS: usize = 1024;
pub(crate) const STEP_COMPARES: usize = 16 * 1024;
pub(crate) const STEP_COPIES: usize = 8 * 1024;

/// A long native's work.
pub(crate) trait Work: 'static {
    /// Does about one step; the native's result once it is all done.
    fn step(&mut self, t: &mut Task<'_>) -> Result<Option<Value>, Stop>;
}

/// Runs `w` as a native's body: its result if it ends in the first piece,
/// else a park that gives the turn and goes on in a box.
pub(crate) fn run(t: &mut Task<'_>, mut w: impl Work) -> Result<Flow, Stop> {
    let mut piece = Piece::new();
    loop {
        if let Some(v) = w.step(t)? {
            return Ok(Flow::Done(v));
        }
        if piece.over() && t.piece_done() {
            return Ok(Flow::Park(Box::new(Pieces(w)), Wake::soon()));
        }
    }
}

/// The clock of a piece (VA30, VA34). It is read first when the first
/// step is done and the work goes on: a native done in one step, the
/// common small input, reads no clock, and a walk that calls back fung
/// code per element reads none per element.
pub(crate) struct Piece(Option<Instant>);

impl Piece {
    pub(crate) const fn new() -> Piece {
        Piece(None)
    }

    /// Whether the piece has had its time; if so, the next one starts now
    /// (in a swap's function, which cannot give its turn, the pieces
    /// follow one another). The first call starts the clock.
    pub(crate) fn over(&mut self) -> bool {
        if PIECE_TIME.is_zero() {
            return true;
        }
        let now = Instant::now();
        match self.0 {
            Some(start) if now.duration_since(start) < PIECE_TIME => false,
            Some(_) => {
                self.0 = Some(now);
                true
            }
            None => {
                self.0 = Some(now);
                false
            }
        }
    }
}

/// [`run`]'s box between pieces.
struct Pieces<W>(W);

impl<W: Work> Cont for Pieces<W> {
    fn resume(&mut self, t: &mut Task<'_>, _: Value) -> Result<Next, Stop> {
        t.check_cancelled()?;
        let mut piece = Piece::new();
        loop {
            if let Some(v) = self.0.step(t)? {
                return Ok(Next::Done(v));
            }
            if piece.over() && t.piece_done() {
                return Ok(Next::Park(Wake::soon()));
            }
        }
    }
}

/// Hands `then` the items of `list` as a vector. A long list's are copied
/// a piece at a time, and the turn is given once more after the copy, so
/// that the copy's last piece and `then`'s first are not one long turn.
pub(crate) fn with_items<F>(t: &mut Task<'_>, list: &Rc<List>, then: F) -> Result<Flow, Stop>
where
    F: FnOnce(&mut Task<'_>, Vec<Value>) -> Result<Flow, Stop> + 'static,
{
    if list.len() <= STEP_COPIES {
        return then(t, list.to_vec());
    }
    let mut c = Copying {
        list: list.clone(),
        xs: Vec::with_capacity(list.len()),
        then: Some(then),
        next: None,
    };
    let mut piece = Piece::new();
    loop {
        let done = c.copy();
        if (done || piece.over()) && t.piece_done() {
            return Ok(Flow::Park(Box::new(c), Wake::soon()));
        }
        // In a swap's function: no turn to give, so on at once.
        if done {
            return c.hand_over(t);
        }
    }
}

/// [`with_items`]'s box: the copy, then what `then` went on with.
struct Copying<F> {
    list: Rc<List>,
    xs: Vec<Value>,
    then: Option<F>,
    next: Option<Box<dyn Cont>>,
}

impl<F> Copying<F>
where
    F: FnOnce(&mut Task<'_>, Vec<Value>) -> Result<Flow, Stop>,
{
    /// Copies about a step; whether all is copied.
    fn copy(&mut self) -> bool {
        let end = (self.xs.len() + STEP_COPIES).min(self.list.len());
        while self.xs.len() < end {
            let run = self.list.run_from(self.xs.len());
            let take = run.len().min(end - self.xs.len());
            self.xs.extend_from_slice(&run[..take]);
        }
        self.xs.len() == self.list.len()
    }

    fn hand_over(&mut self, t: &mut Task<'_>) -> Result<Flow, Stop> {
        let then = self.then.take().expect("handed over once");
        then(t, std::mem::take(&mut self.xs))
    }
}

impl<F> Cont for Copying<F>
where
    F: FnOnce(&mut Task<'_>, Vec<Value>) -> Result<Flow, Stop>,
{
    fn resume(&mut self, t: &mut Task<'_>, v: Value) -> Result<Next, Stop> {
        if let Some(k) = &mut self.next {
            return k.resume(t, v);
        }
        t.check_cancelled()?;
        if self.xs.len() < self.list.len() {
            let mut piece = Piece::new();
            loop {
                let done = self.copy();
                if (done || piece.over()) && t.piece_done() {
                    return Ok(Next::Park(Wake::soon()));
                }
                if done {
                    break;
                }
            }
        }
        Ok(match self.hand_over(t)? {
            Flow::Done(v) => Next::Done(v),
            Flow::Call(k, f, n) => {
                self.next = Some(k);
                Next::Call(f, n)
            }
            Flow::Park(k, wake) => {
                self.next = Some(k);
                Next::Park(wake)
            }
        })
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        match &mut self.next {
            Some(k) => k.unwind(t, stop),
            None => Err(stop),
        }
    }

    fn waits_for(&self) -> Option<&'static str> {
        self.next.as_ref().and_then(|k| k.waits_for())
    }

    fn receives_on(&self) -> Option<&Rc<super::Receiver>> {
        self.next.as_ref().and_then(|k| k.receives_on())
    }

    fn releasing(&self) -> bool {
        self.next.as_ref().is_some_and(|k| k.releasing())
    }
}
