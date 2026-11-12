//! The bytecode verifier (VM22): checks what the VM takes on trust from
//! the compiler. The VM does not check these while it runs; a broken
//! promise there is a panic (`VM stack underflow`, `CHECKTYPE on an
//! unassigned slot`) or worse, code run from the wrong place.
//!
//! For every proto, and every child proto, it checks
//!
//! 1. each operand is in range of the table it indexes (`Operand`),
//! 2. each jump goes forward, to an instruction of the same proto,
//! 3. no path falls off the end of the code: every one ends in an
//!    instruction that stops (`Flow::Stop`: RETURN, TAILCALL, NOMATCH),
//! 4. the stack never runs short, and has the same height wherever paths
//!    meet,
//! 5. no read of a local slot (LOADLOCAL, MOVELOCAL, CHECKTYPE, a
//!    CLOSURE's capture) can find it empty: an abstract run keeps, at
//!    each instruction, the slots assigned on every path to it.
//!
//! The compiler emits forward jumps only (there are no loops, 03), so one
//! pass in code order sees every path into an instruction before the
//! instruction. Code no path reaches is checked for operands only.
//!
//! The compiler runs it on what it makes in debug builds (`compile`), and
//! a test runs it on every `.fung` file in the repository.

use std::collections::HashMap;
use std::fmt;

use super::{Op, Operand, Pattern, Proto, decode_call, decode_slot_type};
use crate::bytecode::PatKind;
use crate::tree::CaptureKind;

/// What the verifier learned about a proto that passed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Verified {
    /// The deepest the operand stack gets in any one frame of it (its
    /// children included).
    pub max_stack: usize,
}

/// A broken promise: in which function, at which instruction, and what.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Violation {
    /// The function's name (`<main>`, `<lambda>`), with its parents',
    /// outermost first: `<main>/f/<lambda>`.
    pub proto: String,
    pub at: usize,
    pub what: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {:04}: {}", self.proto, self.at, self.what)
    }
}

/// Verifies `main`, a file's compiled program, and all functions in it.
pub fn verify(main: &Proto) -> Result<Verified, Violation> {
    let mut max_stack = 0;
    let path = display_name(&main.name).to_string();
    one(main, main.global_names.len(), &path, &mut max_stack)?;
    Ok(Verified { max_stack })
}

fn display_name(name: &str) -> &str {
    if name.is_empty() { "<lambda>" } else { name }
}

/// The slots assigned on every path so far, as bits.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Slots(Vec<u64>);

impl Slots {
    fn new(n: usize) -> Slots {
        Slots(vec![0; n.div_ceil(64)])
    }

    fn get(&self, i: usize) -> bool {
        self.0[i / 64] >> (i % 64) & 1 == 1
    }

    fn set(&mut self, i: usize, on: bool) {
        if on {
            self.0[i / 64] |= 1 << (i % 64);
        } else {
            self.0[i / 64] &= !(1 << (i % 64));
        }
    }

    /// Keeps the slots assigned in both.
    fn meet(&mut self, other: &Slots) {
        for (a, b) in self.0.iter_mut().zip(&other.0) {
            *a &= b;
        }
    }
}

/// The abstract state before an instruction: the stack height and the
/// slots surely assigned.
#[derive(Clone, Debug)]
struct State {
    height: usize,
    slots: Slots,
}

/// The local slots a pattern surely binds when it matches: every name in
/// it, and of `a | b` the names both sides bind.
fn binds(p: &Pattern, out: &mut Vec<usize>) {
    match p.kind {
        PatKind::Bind if !p.global => out.push(p.index),
        PatKind::Alt => {
            let mut sides = p.kids.iter().map(|k| {
                let mut v = Vec::new();
                binds(k, &mut v);
                v
            });
            if let Some(mut all) = sides.next() {
                for side in sides {
                    all.retain(|s| side.contains(s));
                }
                out.extend(all);
            }
        }
        _ => {
            for k in &p.kids {
                binds(k, out);
            }
            if let Some(r) = &p.rest {
                binds(r, out);
            }
        }
    }
}

/// Verifies one proto and its children.
fn one(p: &Proto, globals: usize, path: &str, max_stack: &mut usize) -> Result<(), Violation> {
    let fail = |at: usize, what: String| Violation {
        proto: path.to_string(),
        at,
        what,
    };
    let len = p.code.len();
    if p.pos.len() != len {
        return Err(fail(
            0,
            format!("{} positions for {len} instructions", p.pos.len()),
        ));
    }
    // What each child copies from this frame when it is made.
    for child in &p.protos {
        for c in &child.captures {
            let ok = match c.kind {
                CaptureKind::Local => c.index < p.num_locals,
                CaptureKind::Captured => c.index < p.captures.len(),
                CaptureKind::Global => c.index < globals,
                CaptureKind::SelfFn => true,
            };
            if !ok {
                let name = display_name(&child.name);
                return Err(fail(0, format!("{name}'s capture {c:?} out of range")));
            }
        }
    }
    if p.params.len() > p.num_locals {
        return Err(fail(0, "more parameters than local slots".into()));
    }

    // (1) and (2): every instruction, reachable or not.
    for (at, ins) in p.code.iter().enumerate() {
        let (op, arg) = (ins.op(), ins.arg());
        let size = |n: usize, table: &str| {
            if arg < n {
                Ok(())
            } else {
                Err(fail(at, format!("{} {arg}: {table} has {n}", op.name())))
            }
        };
        // The second operand (VM32): a table index.
        let b = ins.b();
        match op.second() {
            Operand::Const if b >= p.consts.len() => {
                return Err(fail(
                    at,
                    format!("{} second {b}: consts has {}", op.name(), p.consts.len()),
                ));
            }
            Operand::Pattern if b >= p.patterns.len() => {
                return Err(fail(
                    at,
                    format!(
                        "{} second {b}: patterns has {}",
                        op.name(),
                        p.patterns.len()
                    ),
                ));
            }
            _ => {}
        }
        match op.info().operand {
            Operand::None | Operand::Count | Operand::Flags => {}
            Operand::Const => size(p.consts.len(), "consts")?,
            Operand::Global => size(globals, "globals")?,
            Operand::Local => size(p.num_locals, "local slots")?,
            Operand::Proto => size(p.protos.len(), "protos")?,
            Operand::Captured => size(p.captures.len(), "captures")?,
            Operand::Type => size(p.types.len(), "types")?,
            Operand::Pattern => size(p.patterns.len(), "patterns")?,
            Operand::Assert => size(p.asserts.len(), "asserts")?,
            Operand::SlotType => {
                let (slot, ty) = decode_slot_type(arg);
                if slot >= p.params.len() || ty >= p.types.len() {
                    return Err(fail(at, format!("CHECKTYPE {slot}/{ty} out of range")));
                }
            }
            Operand::Call => {
                let (positional, named) = decode_call(arg);
                if positional > 0xFFF || named > 0xFFF {
                    return Err(fail(at, format!("{} {arg} out of range", op.name())));
                }
            }
            Operand::Target => {
                if arg >= len {
                    return Err(fail(at, format!("{} {arg}: past the end", op.name())));
                }
                if arg <= at {
                    return Err(fail(at, format!("{} {arg}: not forward", op.name())));
                }
            }
        }
    }

    // (3) to (5): one pass in code order. `incoming` holds, for each
    // instruction a jump reaches, the meet of the states the jumps bring.
    let mut incoming: HashMap<usize, State> = HashMap::new();
    let arrive =
        |incoming: &mut HashMap<usize, State>, to: usize, s: State| match incoming.get_mut(&to) {
            None => {
                incoming.insert(to, s);
                Ok(())
            }
            Some(have) if have.height != s.height => Err(fail(
                to,
                format!(
                    "paths meet with stack heights {} and {}",
                    have.height, s.height
                ),
            )),
            Some(have) => {
                have.slots.meet(&s.slots);
                Ok(())
            }
        };
    let mut entry = Slots::new(p.num_locals);
    for (i, prm) in p.params.iter().enumerate() {
        entry.set(i, !prm.has_default);
    }
    // The state falling through into the current instruction, if any.
    let mut cur = Some(State {
        height: 0,
        slots: entry,
    });
    for at in 0..len {
        let jumped = incoming.remove(&at);
        let mut s = match (cur.take(), jumped) {
            (None, None) => continue,
            (Some(s), None) | (None, Some(s)) => s,
            (Some(mut s), Some(j)) => {
                if s.height != j.height {
                    return Err(fail(
                        at,
                        format!(
                            "paths meet with stack heights {} and {}",
                            s.height, j.height
                        ),
                    ));
                }
                s.slots.meet(&j.slots);
                s
            }
        };
        let (op, arg) = (p.code[at].op(), p.code[at].arg());
        let split = op == Op::Assert && !p.asserts[arg].op.is_empty();
        let (pops, pushes) = op.stack_effect(arg, split);
        if s.height < pops {
            return Err(fail(
                at,
                format!(
                    "{} takes {pops} values from a stack of {}",
                    op.name(),
                    s.height
                ),
            ));
        }
        let read = |s: &State, slot: usize, what: &str| {
            if s.slots.get(slot) {
                Ok(())
            } else {
                Err(fail(
                    at,
                    format!("{what} reads slot {slot}, which may be unassigned"),
                ))
            }
        };
        // What a MATCH binds reaches the code after it, not its jump.
        let mut binding = None;
        let next_op = p.code.get(at + 1).map(|i| i.op());
        match op {
            // The prologue loads a parameter with a default to test it:
            // the slot may be empty there (JUMPIFSET).
            Op::LoadLocal if next_op == Some(Op::JumpIfSet) => {}
            Op::LoadLocal => read(&s, arg, "LOADLOCAL")?,
            Op::MoveLocal => {
                read(&s, arg, "MOVELOCAL")?;
                s.slots.set(arg, false);
            }
            Op::StoreLocal => s.slots.set(arg, true),
            Op::ClearLocal => s.slots.set(arg, false),
            Op::CheckType => read(&s, decode_slot_type(arg).0, "CHECKTYPE")?,
            Op::Closure => {
                for c in &p.protos[arg].captures {
                    if c.kind == CaptureKind::Local {
                        read(&s, c.index, "CLOSURE")?;
                    }
                }
            }
            Op::MatchOrFail => {
                let mut bound = Vec::new();
                binds(&p.patterns[arg], &mut bound);
                for slot in bound {
                    s.slots.set(slot, true);
                }
            }
            Op::Match => {
                let mut bound = Vec::new();
                binds(&p.patterns[p.code[at].b()], &mut bound);
                binding = Some(bound);
            }
            // The VM reads the call TAILCHECK hands its check to (D44).
            Op::TailCheck if !matches!(next_op, Some(Op::TailCall | Op::TailCallSpread)) => {
                return Err(fail(at, "TAILCHECK is not followed by a tail call".into()));
            }
            _ => {}
        }
        s.height = s.height - pops + pushes;
        *max_stack = (*max_stack).max(s.height);
        let flow = op.info().flow;
        let taken = || {
            let mut t = s.clone();
            if op == Op::Coalesce {
                t.height += 1;
            }
            // The jump of LOADLOCAL i; JUMPIFSET is taken when slot i
            // holds a value.
            if op == Op::JumpIfSet
                && let Some(prev) = at.checked_sub(1).map(|i| p.code[i])
                && prev.op() == Op::LoadLocal
            {
                t.slots.set(prev.arg(), true);
            }
            t
        };
        match flow {
            super::Flow::Stop => {}
            super::Flow::Goto => arrive(&mut incoming, arg, taken())?,
            super::Flow::Next => cur = Some(s),
            super::Flow::Branch | super::Flow::BranchKeep => {
                let t = taken();
                arrive(&mut incoming, arg, t)?;
                if let Some(bound) = binding {
                    for slot in bound {
                        s.slots.set(slot, true);
                    }
                }
                cur = Some(s);
            }
        }
        if cur.is_some() && at + 1 == len {
            return Err(fail(at, "the code falls off its end".into()));
        }
    }
    for child in &p.protos {
        let child_path = format!("{path}/{}", display_name(&child.name));
        one(child, globals, &child_path, max_stack)?;
    }
    Ok(())
}
