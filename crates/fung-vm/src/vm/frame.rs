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
    pub tail_calls: u32,
}

impl Frame {
    /// The position of the instruction the frame is executing (the one
    /// before `ip`), if it has started.
    pub fn current_pos(&self) -> Option<fung_front::source::Pos> {
        let i = self.ip.checked_sub(1)?;
        self.proto.pos.get(i).copied()
    }
}
