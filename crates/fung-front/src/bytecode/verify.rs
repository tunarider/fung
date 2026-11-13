//! The bytecode verifier (VM22): checks what the VM takes on trust from
//! the compiler. The VM does not check these while it runs; a broken
//! promise there is a panic (`VM stack underflow`, `CHECKTYPE on an
//! unassigned slot`) or worse, code run from the wrong place.
//!
//! For every proto, and every child proto, it checks
//!
//! 1. each operand is in range of the table it indexes (`Operand`), and
//!    names what the VM reads there (VM40): the constant of a FIELD,
//!    IMPORT or TEST is a string, a TAG's name a symbol, a MAP's keys a
//!    list of as many strings as it takes values; a pattern binds slots
//!    and globals that exist and has a key for each pattern of a map; a
//!    spread call has its list; flags name bits the VM knows,
//! 2. each jump goes forward, to an instruction of the same proto,
//! 3. no path falls off the end of the code: every one ends in an
//!    instruction that stops (`Flow::Stop`: RETURN, TAILCALL, TAILCALLSPREAD,
//!    NOMATCH),
//! 4. the stack never runs short, and has the same height wherever paths
//!    meet,
//! 5. no read of a local slot (LOADLOCAL, MOVELOCAL, CHECKTYPE, a
//!    CLOSURE's capture) can find it empty: an abstract run keeps, at
//!    each instruction, the slots assigned on every path to it. Only
//!    JUMPIFSET asks whether a slot is empty, of a parameter with a
//!    default (the prologue's test),
//! 6. a call that gives every argument starts at `Proto::body`: an
//!    instruction of the proto, reached with an empty stack.
//!
//! The compiler emits forward jumps only (there are no loops, 03), so one
//! pass in code order sees every path into an instruction before the
//! instruction. Code no path reaches is checked for operands only.
//!
//! The compiler runs it on every program it makes, before the program
//! runs (`compile`, VM41), so it must answer for any proto, however
//! wrong, without failing itself. A test runs it on every `.fung` file in
//! the repository.

use std::collections::HashMap;
use std::fmt;

use super::{Op, Operand, Pattern, Proto};
use crate::bytecode::PatKind;
use crate::consts::Const;
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

/// What is wrong with pattern `p` of a proto with `locals` slots, if
/// anything: a name bound past the slots or the globals, a map pattern
/// whose keys and patterns differ in number, a literal without one.
fn pattern_fault(p: &Pattern, locals: usize, globals: usize) -> Option<String> {
    match p.kind {
        PatKind::Bind if p.global && p.index >= globals => {
            return Some(format!(" binds global {}: globals has {globals}", p.index));
        }
        PatKind::Bind if !p.global && p.index >= locals => {
            return Some(format!(" binds slot {}: local slots has {locals}", p.index));
        }
        PatKind::Map if p.keys.len() != p.kids.len() => {
            return Some(format!(
                ": a map pattern with {} keys for {} patterns",
                p.keys.len(),
                p.kids.len()
            ));
        }
        PatKind::Lit if p.lit.is_none() => {
            return Some(": a literal pattern without its literal".into());
        }
        _ => {}
    }
    p.kids
        .iter()
        .chain(p.rest.as_deref())
        .find_map(|k| pattern_fault(k, locals, globals))
}

/// Whether `c` is a string constant: text or bytes (L4).
fn is_str(c: &Const) -> bool {
    matches!(c, Const::Str(_) | Const::Bytes(_))
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
    // The patterns MATCH and MATCHORFAIL read: the matcher writes the
    // slots they name without looking.
    for (i, pattern) in p.patterns.iter().enumerate() {
        if let Some(what) = pattern_fault(pattern, p.num_locals, globals) {
            return Err(fail(0, format!("pattern {i}{what}")));
        }
    }
    if p.body >= len.max(1) {
        return Err(fail(
            p.body,
            format!("the body starts at {}: past the end", p.body),
        ));
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
            Operand::Type if b >= p.types.len() => {
                return Err(fail(
                    at,
                    format!("{} second {b}: types has {}", op.name(), p.types.len()),
                ));
            }
            Operand::Local if b >= p.num_locals => {
                return Err(fail(
                    at,
                    format!("{} second {b}: local slots has {}", op.name(), p.num_locals),
                ));
            }
            _ => {}
        }
        // What the constant must be for the VM to read it (VM40).
        let name = op.name();
        match op {
            Op::Tag if !matches!(p.consts[b], Const::Sym(_)) => {
                return Err(fail(at, format!("{name} {arg}: its name is not a symbol")));
            }
            Op::Map => match &p.consts[b] {
                Const::List(keys) if keys.iter().all(is_str) => {
                    if keys.len() != arg {
                        return Err(fail(
                            at,
                            format!(
                                "{name} {arg}: its {} keys do not pair with them",
                                keys.len()
                            ),
                        ));
                    }
                }
                _ => {
                    return Err(fail(
                        at,
                        format!("{name} {arg}: its keys are not a list of strings"),
                    ));
                }
            },
            Op::Field | Op::Import | Op::Test if p.consts.get(arg).is_some_and(|c| !is_str(c)) => {
                return Err(fail(at, format!("{name} {arg}: not a string")));
            }
            Op::Slice if arg > 7 => {
                return Err(fail(at, format!("{name} {arg}: flags past 7")));
            }
            Op::Range if arg > 1 => {
                return Err(fail(at, format!("{name} {arg}: flags past 1")));
            }
            // The last positional argument is the list it spreads.
            Op::CallSpread | Op::TailCallSpread if arg == 0 => {
                return Err(fail(at, format!("{name} {arg}: no list to spread")));
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
    // (6) A call that gives every argument starts at the body, past the
    // prologue: one more path into it, with an empty stack and only the
    // parameters in their slots.
    if p.body > 0 {
        let mut direct = Slots::new(p.num_locals);
        for i in 0..p.params.len() {
            direct.set(i, true);
        }
        incoming.insert(
            p.body,
            State {
                height: 0,
                slots: direct,
            },
        );
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
        let (op, arg, b) = (p.code[at].op(), p.code[at].arg(), p.code[at].b());
        let split = op == Op::Assert && p.asserts[arg].cmp.is_some();
        let (pops, pushes) = op.stack_effect(arg, b, split);
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
            // Only the prologue asks whether a slot is empty, of a
            // parameter with a default (VM42).
            Op::JumpIfSet if !p.params.get(b).is_some_and(|prm| prm.has_default) => {
                return Err(fail(
                    at,
                    format!("JUMPIFSET {b}: not a parameter with a default"),
                ));
            }
            Op::LoadLocal => read(&s, arg, "LOADLOCAL")?,
            Op::MoveLocal => {
                read(&s, arg, "MOVELOCAL")?;
                s.slots.set(arg, false);
            }
            Op::StoreLocal => s.slots.set(arg, true),
            Op::ClearLocal => s.slots.set(arg, false),
            Op::CheckType if arg >= p.params.len() => {
                return Err(fail(at, format!("CHECKTYPE {arg}: not a parameter")));
            }
            Op::CheckType => read(&s, arg, "CHECKTYPE")?,
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
            // The bool check goes along with the same call, before the
            // return type's (D97).
            Op::TailCheckBool
                if !matches!(
                    next_op,
                    Some(Op::TailCheck | Op::TailCall | Op::TailCallSpread)
                ) =>
            {
                return Err(fail(
                    at,
                    "TAILCHECKBOOL is not followed by a tail call".into(),
                ));
            }
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
            // The jump of JUMPIFSET is taken when its slot holds a value.
            if op == Op::JumpIfSet {
                t.slots.set(b, true);
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
