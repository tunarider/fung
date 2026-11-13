//! `fung dump bytecode` output:
//!
//! ```text
//! 0000  1:7   CONST        0    ; 1
//! ```

use std::fmt::Write;

use super::{Op, Proto};
use crate::consts::{format, format_for_print, type_text};

/// Renders a proto and its child functions, depth first, each after a
/// blank line.
pub fn disassemble(p: &Proto) -> String {
    let mut b = String::new();
    disasm(p, &mut b);
    b
}

fn display_name(name: &str) -> &str {
    if name.is_empty() { "<lambda>" } else { name }
}

fn disasm(p: &Proto, b: &mut String) {
    let _ = writeln!(b, "== {} ==", display_name(&p.name));
    for (i, ins) in p.code.iter().enumerate() {
        let pos = format!("{}:{}", p.pos[i].line, p.pos[i].col);
        let op = ins.op();
        if !op.has_arg() {
            let _ = writeln!(b, "{i:04}  {pos:<5} {}", op.name());
            continue;
        }
        let arg = ins.arg();
        let _ = write!(b, "{i:04}  {pos:<5} {:<12} {arg}", op.name());
        if op.second() != super::Operand::None {
            let _ = write!(b, " {}", ins.b());
        }
        comment(p, op, arg, ins.b(), b);
        b.push('\n');
    }
    for child in &p.protos {
        b.push('\n');
        disasm(child, b);
    }
}

/// The `    ; …` explanation after an operand, if the opcode has one.
fn comment(p: &Proto, op: Op, arg: usize, second: usize, b: &mut String) {
    let text = match op {
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
