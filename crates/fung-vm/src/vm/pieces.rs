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

use super::cont::{Cont, Flow, Next, Wake};
use super::{Stop, Task};
use crate::value::Value;

/// About a millisecond of work for the natives below, by what they count:
/// bytes read or written, items, comparisons. Measured on an Apple M-series
/// release build (L6 note): JSON reads about 75 MB/s and writes about
/// 150 MB/s, scanning a string for a separator well over 1 GB/s, a merge
/// step about 5 ns.
pub(crate) const PIECE_BYTES: usize = 64 * 1024;
pub(crate) const PIECE_SCAN: usize = 1024 * 1024;
pub(crate) const PIECE_ITEMS: usize = 8 * 1024;
pub(crate) const PIECE_COMPARES: usize = 128 * 1024;

/// A long native's work.
pub(crate) trait Work: 'static {
    /// Does about one piece; the native's result once it is all done.
    fn step(&mut self, t: &mut Task<'_>) -> Result<Option<Value>, Stop>;
}

/// Runs `w` as a native's body: its result if it ends in the first piece,
/// else a park that gives the turn and goes on in a box.
pub(crate) fn run(t: &mut Task<'_>, mut w: impl Work) -> Result<Flow, Stop> {
    loop {
        if let Some(v) = w.step(t)? {
            return Ok(Flow::Done(v));
        }
        if t.piece_done() {
            return Ok(Flow::Park(Box::new(Pieces(w)), Wake::soon()));
        }
    }
}

/// [`run`]'s box between pieces.
struct Pieces<W>(W);

impl<W: Work> Cont for Pieces<W> {
    fn resume(&mut self, t: &mut Task<'_>, _: Value) -> Result<Next, Stop> {
        t.check_cancelled()?;
        loop {
            if let Some(v) = self.0.step(t)? {
                return Ok(Next::Done(v));
            }
            if t.piece_done() {
                return Ok(Next::Park(Wake::soon()));
            }
        }
    }
}
