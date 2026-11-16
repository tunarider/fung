//! `fung dump bytecode` output:
//!
//! ```text
//! 0000  1:7   CONST        0    ; 1
//! ```

use std::fmt::Write;

use super::{Op, Operand, Proto};
use crate::consts::{format, format_for_print, type_text};

/// Renders a proto and its child functions, depth first, each after a
/// blank line.
pub fn disassemble(p: &Proto) -> String {
    let mut b = String::new();
    disasm(p, &mut b);
    b
}

/// `fung dump proto`: as `disassemble`, with every table of each
/// function written out before its code. A table with nothing in it is
/// left out; `slots` (the frame's size) is always there.
pub fn describe(p: &Proto) -> String {
    let mut b = String::new();
    tables(p, &mut b);
    b
}

/// One numbered row of a table.
fn row(b: &mut String, i: usize, text: &str) {
    let _ = writeln!(b, "{i:>4}  {text}");
}

/// A table: its title and a row per item, or nothing when it is empty.
fn table<T>(b: &mut String, title: &str, items: &[T], text: impl Fn(usize, &T) -> String) {
    if items.is_empty() {
        return;
    }
    let _ = writeln!(b, "{title}:");
    for (i, x) in items.iter().enumerate() {
        row(b, i, &text(i, x));
    }
}

fn tables(p: &Proto, b: &mut String) {
    use crate::tree::CaptureKind;
    let _ = writeln!(b, "== {} ==", display_name(&p.name));
    table(b, "params", &p.params, |i, _| param_text(p, i));
    if let Some(t) = usize::try_from(p.return_type)
        .ok()
        .and_then(|t| p.types.get(t))
    {
        let _ = writeln!(b, "returns: {}", type_text(t));
    }
    let _ = writeln!(b, "slots: {}", p.num_locals);
    if !p.held.is_empty() {
        let held: Vec<String> = p.held.iter().map(usize::to_string).collect();
        let _ = writeln!(b, "held: {}", held.join(", "));
    }
    if !p.locals.is_empty() {
        b.push_str("locals:\n");
        for l in &p.locals {
            row(b, l.slot, &format!("{}  (from {:04})", l.name, l.from));
        }
    }
    table(b, "captures", &p.captures, |i, c| {
        let from = match c.kind {
            CaptureKind::Local => format!("local {}", c.index),
            CaptureKind::Captured => format!("captured {}", c.index),
            CaptureKind::SelfFn => "self".to_string(),
            CaptureKind::Global => format!("global {}", c.index),
        };
        format!("{}  ({from})", capture_name(p, i))
    });
    table(b, "constants", &p.consts, |_, c| format(c));
    table(b, "types", &p.types, |_, t| type_text(t));
    table(b, "patterns", &p.patterns, |_, pat| pat.to_string());
    table(b, "asserts", &p.asserts, |_, a| a.text.clone());
    table(b, "functions", &p.protos, |_, c| {
        display_name(&c.name).to_string()
    });
    table(b, "globals", &p.global_names, |_, n| n.clone());
    if !p.exports.is_empty() {
        let _ = writeln!(b, "exports: {}", p.exports.join(", "));
    }
    if p.body > 0 {
        let _ = writeln!(b, "body: {:04}", p.body);
    }
    b.push_str("code:\n");
    code(p, b);
    for child in &p.protos {
        b.push('\n');
        tables(child, b);
    }
}

fn display_name(name: &str) -> &str {
    if name.is_empty() { "<lambda>" } else { name }
}

fn disasm(p: &Proto, b: &mut String) {
    let _ = writeln!(b, "== {} ==", display_name(&p.name));
    summary(p, b);
    code(p, b);
    for child in &p.protos {
        b.push('\n');
        disasm(child, b);
    }
}

/// `x`, `by: int = ?`, `...rest`: a parameter as the source declares it,
/// its default as `?` (the code that fills it is the function's start).
fn param_text(p: &Proto, i: usize) -> String {
    let prm = &p.params[i];
    let mut t = String::new();
    if prm.rest {
        t.push_str("...");
    }
    t.push_str(&prm.name);
    if let Some(ty) = usize::try_from(prm.ty).ok().and_then(|ty| p.types.get(ty)) {
        let _ = write!(t, ": {}", type_text(ty));
    }
    if prm.has_default {
        t.push_str(" = ?");
    }
    t
}

/// The lines under a function's header that the code does not show:
/// its parameters, its return type and the names of what it captures.
fn summary(p: &Proto, b: &mut String) {
    if !p.params.is_empty() {
        let all: Vec<String> = (0..p.params.len()).map(|i| param_text(p, i)).collect();
        let _ = writeln!(b, "params:   {}", all.join(", "));
    }
    if let Some(t) = usize::try_from(p.return_type)
        .ok()
        .and_then(|t| p.types.get(t))
    {
        let _ = writeln!(b, "returns:  {}", type_text(t));
    }
    if !p.captures.is_empty() {
        let names: Vec<&str> = (0..p.captures.len()).map(|i| capture_name(p, i)).collect();
        let _ = writeln!(b, "captures: {}", names.join(", "));
    }
}

fn capture_name(p: &Proto, i: usize) -> &str {
    match p.capture_names.get(i) {
        Some(n) if !n.is_empty() => n,
        _ => "?",
    }
}

/// The instructions, one per line. The names of the local slots are
/// followed along: `locals` is in the order of the code.
fn code(p: &Proto, b: &mut String) {
    let mut names: Vec<Option<&str>> = vec![None; p.num_locals];
    let mut next = 0;
    for (i, ins) in p.code.iter().enumerate() {
        while let Some(l) = p.locals.get(next).filter(|l| l.from <= i) {
            if let Some(n) = names.get_mut(l.slot) {
                *n = Some(&l.name);
            }
            next += 1;
        }
        let pos = format!("{}:{}", p.pos[i].line, p.pos[i].col);
        let op = ins.op();
        if !op.has_arg() {
            let _ = writeln!(b, "{i:04}  {pos:<5} {}", op.name());
            continue;
        }
        let arg = ins.arg();
        let _ = write!(b, "{i:04}  {pos:<5} {:<12} {arg}", op.name());
        if op.second() != Operand::None {
            let _ = write!(b, " {}", ins.b());
        }
        comment(p, &names, op, arg, ins.b(), b);
        b.push('\n');
    }
}

/// The `    ; …` explanation after an operand, if the opcode has one.
fn comment(p: &Proto, names: &[Option<&str>], op: Op, arg: usize, second: usize, b: &mut String) {
    let local = |slot: usize| names.get(slot).copied().flatten();
    let text = match op {
        Op::LoadLocal | Op::StoreLocal | Op::MoveLocal | Op::ClearLocal => match local(arg) {
            Some(n) => n.to_string(),
            None => return,
        },
        Op::JumpIfSet => match local(second) {
            Some(n) => n.to_string(),
            None => return,
        },
        Op::LoadCaptured => match p.capture_names.get(arg) {
            Some(n) if !n.is_empty() => n.clone(),
            _ => return,
        },
        Op::Const | Op::Test => format(&p.consts[arg]),
        Op::LoadGlobal | Op::StoreGlobal => match p.global_names.get(arg) {
            Some(n) => n.clone(),
            None => return,
        },
        Op::Call | Op::TailCall | Op::CallSpread | Op::TailCallSpread => {
            format!("{arg} positional, {second} named")
        }
        Op::Closure => match p.protos.get(arg) {
            Some(c) => display_name(&c.name).to_string(),
            None => return,
        },
        Op::CheckType => match (p.params.get(arg), p.types.get(second)) {
            (Some(prm), Some(t)) => format!("{}: {}", prm.name, type_text(t)),
            _ => return,
        },
        Op::CheckReturn | Op::TailCheck => match p.types.get(arg) {
            Some(t) => format!("-> {}", type_text(t)),
            None => return,
        },
        Op::Tag => match p.consts.get(second) {
            Some(name) => format!("{}, {arg} args", format(name)),
            None => return,
        },
        Op::Map => match p.consts.get(second) {
            Some(keys) => format!("keys {}", format(keys)),
            None => return,
        },
        Op::Slice => {
            let mut parts = Vec::new();
            if arg & 1 != 0 {
                parts.push("start");
            }
            if arg & 2 != 0 {
                parts.push("end");
            }
            if arg & 4 != 0 {
                parts.push("inclusive");
            }
            parts.join(", ")
        }
        Op::Field | Op::Import => format_for_print(&p.consts[arg]),
        Op::Assert => match p.asserts.get(arg) {
            Some(a) => a.text.clone(),
            None => return,
        },
        Op::Match => match p.patterns.get(second) {
            Some(pat) => format!("{pat}, else {arg}"),
            None => return,
        },
        Op::MatchOrFail => match p.patterns.get(arg) {
            Some(pat) => pat.to_string(),
            None => return,
        },
        Op::Range if arg == 1 => "inclusive".to_string(),
        _ => return,
    };
    let _ = write!(b, "    ; {text}");
}
