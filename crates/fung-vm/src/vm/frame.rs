//! Call frames (09: 프레임). A frame is a running function: its proto, the
//! next instruction, where its slots start on the task's value stack, the
//! closure it runs (none for a module body) and its module's globals.

use std::rc::Rc;

use super::RtProto;
use crate::value::{Closure, Globals};

pub(super) struct Frame {
    pub proto: Rc<RtProto>,
    pub ip: usize,
    /// Index into the task stack where this frame's slots start.
    pub base: usize,
    pub closure: Option<Rc<Closure>>,
    pub globals: Globals,
    /// How many tail calls reused this frame (shown in call paths).
    pub tail_calls: u64,
    /// The function of a built-in module that user code called and that
    /// this frame runs inside of (itself, or a helper it went on to,
    /// through calls and tail calls): what E0707 names (`user_callee`).
    pub entry: Option<Rc<RtProto>>,
    /// The frame of user code a tail call into a built-in function
    /// replaced (L8): where an error inside is put (`user_pos`) and a
    /// line of the call path. Kept through the built-in function's own
    /// tail calls; only that crossing pays for it.
    pub tail_from: Option<Box<TailFrom>>,
}

/// A frame of user code that a tail call into a built-in function
/// replaced: its function, the call it made, and its tail calls before.
pub(super) struct TailFrom {
    pub proto: Rc<RtProto>,
    pub pos: Option<fung_front::source::Pos>,
    pub tail_calls: u64,
}

impl Frame {
    /// The position of the instruction the frame is executing (the one
    /// before `ip`), if it has started.
    pub fn current_pos(&self) -> Option<fung_front::source::Pos> {
        let i = self.ip.checked_sub(1)?;
        self.proto.pos.get(i).copied()
    }
}
