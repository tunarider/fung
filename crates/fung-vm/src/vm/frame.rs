//! Call frames (09: 프레임). A frame is a running function: the closure
//! it runs (a module body runs one with nothing captured), which holds
//! its proto and its module's globals, the next instruction and where
//! its slots start on the task's value stack.

use std::rc::Rc;

use super::RtProto;
use crate::value::{Closure, Globals};

pub(super) struct Frame {
    pub closure: Rc<Closure>,
    pub ip: usize,
    /// Index into the task stack where this frame's slots start (`base`).
    /// Kept in 32 bits with `floor` so the frame stays 48 bytes: a bigger
    /// frame made every call slower (VM16).
    base: u32,
    /// Where the stack is cut back to when the frame returns (`floor`):
    /// `base`, or `base - 1` for a frame a CALL entered, whose callee slot
    /// below its arguments stays (empty) so the arguments need not move
    /// (VM16).
    floor: u32,
    /// How many tail calls reused this frame (shown in call paths).
    pub tail_calls: u64,
    /// The function of a built-in module that user code called and that
    /// this frame runs inside of (itself, or a helper it went on to,
    /// through calls and tail calls): what E0707 names (`user_callee`).
    pub entry: Option<Rc<RtProto>>,
    /// What a frame rarely needs, boxed so the frame stays small: the
    /// frame of user code a tail call replaced, and the return checks its
    /// tail calls took along. Only a frame that needs one pays for it.
    pub extra: Option<Box<Extra>>,
}

/// The rare parts of a frame (`Frame::extra`).
#[derive(Default)]
pub(super) struct Extra {
    /// The frame of user code a tail call into a built-in function
    /// replaced (L8): where an error inside is put (`user_pos`) and a
    /// line of the call path. Kept through the built-in function's own
    /// tail calls.
    pub tail_from: Option<TailFrom>,
    /// The return types a function with one handed on with its tail
    /// calls (TAILCHECK, D44): the frame's value is checked against each
    /// when it returns, the latest first. One per place that took it
    /// along, so tail calls going round (`a` ↔ `b`) do not pile them up.
    pub checks: Vec<Check>,
}

/// A return check a tail call took along (D44): `proto`'s `types[ty]`,
/// reported at `pos` (where that function's value is returned).
pub(super) struct Check {
    pub proto: Rc<RtProto>,
    /// The return type to check, or `None` for the bool that the right
    /// side of an `and`/`or` must be (D97).
    pub ty: Option<usize>,
    pub pos: fung_front::source::Pos,
}

/// A frame of user code that a tail call into a built-in function
/// replaced: its function, the call it made, and its tail calls before.
pub(super) struct TailFrom {
    pub proto: Rc<RtProto>,
    pub pos: Option<fung_front::source::Pos>,
    pub tail_calls: u64,
}

impl Frame {
    /// A frame for `closure` whose slots start at `base`, returning to
    /// `floor`.
    pub fn new(
        closure: Rc<Closure>,
        base: usize,
        floor: usize,
        entry: Option<Rc<RtProto>>,
    ) -> Frame {
        let slot = |i: usize| u32::try_from(i).expect("a stack index within 32 bits");
        Frame {
            closure,
            ip: 0,
            base: slot(base),
            floor: slot(floor),
            tail_calls: 0,
            entry,
            extra: None,
        }
    }

    /// A frame for a module body `proto` with its `globals`.
    pub fn body(proto: Rc<RtProto>, globals: Globals, base: usize) -> Frame {
        let closure = Closure {
            code: proto.code.clone(),
            proto,
            captured: Default::default(),
            globals,
        };
        Frame::new(Rc::new(closure), base, base, None)
    }

    /// Index into the task stack where this frame's slots start.
    #[inline(always)]
    pub fn base(&self) -> usize {
        self.base as usize
    }

    /// Where the stack is cut back to when the frame returns.
    #[inline(always)]
    pub fn floor(&self) -> usize {
        self.floor as usize
    }

    /// The frame of user code a tail call into a built-in function
    /// replaced, if any (L8).
    pub fn tail_from(&self) -> Option<&TailFrom> {
        self.extra.as_ref()?.tail_from.as_ref()
    }

    /// Whether the frame's value has return checks to pass (D44).
    #[inline(always)]
    pub fn has_checks(&self) -> bool {
        self.extra.as_ref().is_some_and(|x| !x.checks.is_empty())
    }

    /// Whether the frame carries the check `types[ty]` of `proto`
    /// already: every check runs on the frame's value, so taking it again
    /// would add nothing (VM44).
    #[inline(always)]
    pub fn has_check(&self, proto: &Rc<RtProto>, ty: usize) -> bool {
        self.extra.as_ref().is_some_and(|x| {
            x.checks
                .iter()
                .any(|c| c.ty == Some(ty) && Rc::ptr_eq(&c.proto, proto))
        })
    }

    /// Takes `c` along: one check per place, the latest last (D44).
    pub fn add_check(&mut self, c: Check) {
        let checks = &mut self.extra.get_or_insert_with(Default::default).checks;
        if let Some(i) = checks
            .iter()
            .position(|d| d.pos == c.pos && Rc::ptr_eq(&d.proto, &c.proto))
        {
            checks.remove(i);
        }
        checks.push(c);
    }

    pub fn proto(&self) -> &Rc<RtProto> {
        &self.closure.proto
    }

    pub fn globals(&self) -> &Globals {
        &self.closure.globals
    }

    /// The position of the instruction the frame is executing (the one
    /// before `ip`), if it has started.
    pub fn current_pos(&self) -> Option<fung_front::source::Pos> {
        let i = self.ip.checked_sub(1)?;
        self.closure.proto.pos.get(i).copied()
    }
}

#[cfg(test)]
mod tests {
    /// A frame stays 48 bytes: at 56 every call got slower (`fib` +20%,
    /// VM16), since pushing a frame copies it through the Rust stack.
    #[test]
    #[cfg(target_pointer_width = "64")]
    fn a_frame_is_48_bytes() {
        assert_eq!(std::mem::size_of::<super::Frame>(), 48);
    }
}
