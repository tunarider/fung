//! Constant folding of arithmetic (VM10 g): an operator whose operands are
//! number literals, or folded operators, is computed once here. Only
//! where the VM's result is certain: an int that would overflow, a
//! division by zero or a float that is not finite is left to the VM, so
//! the error comes at the same time and place as before.

use std::collections::HashMap;

use super::lit_str;
use crate::consts::Const;
use crate::tree::{Head, Lit, Node};

/// The constants of the operators in `root`'s tree (nested functions
/// too) that fold, by node: one walk, bottom up, so each operator is
/// looked at once (VM18: folding each operator from scratch walked its
/// whole left chain again, quadratic in a long `x + x + …`).
pub(super) fn constants(root: &Node) -> HashMap<*const Node, Const> {
    let mut out = HashMap::new();
    walk(root, &mut out);
    out
}

/// The constant `n` stands for, if it is a number literal or arithmetic
/// on such; folded operators below `n` go into `out`.
fn walk(n: &Node, out: &mut HashMap<*const Node, Const>) -> Option<Const> {
    let mut operands = [None, None];
    for (i, k) in n.kids.iter().enumerate() {
        let c = walk(k, out);
        if let Some(slot) = operands.get_mut(i) {
            *slot = c;
        }
    }
    let [a, b] = operands;
    let c = match (n.head, &n.lit) {
        (Head::Int, Lit::Int(i)) => Some(Const::Int(*i)),
        (Head::Float, Lit::Float(f)) => Some(Const::Float(*f)),
        (Head::Op, _) if n.kids.len() == 1 && lit_str(n) == "-" => match a? {
            Const::Int(i) => i.checked_neg().map(Const::Int),
            Const::Float(f) => float(-f),
            _ => None,
        },
        (Head::Op, _) if n.kids.len() == 2 => binary(lit_str(n), a?, b?),
        _ => None,
    };
    if n.head == Head::Op
        && let Some(c) = &c
    {
        out.insert(n as *const Node, c.clone());
    }
    c
}

fn binary(op: &str, a: Const, b: Const) -> Option<Const> {
    match (a, b) {
        (Const::Int(x), Const::Int(y)) => ints(op, x, y),
        (Const::Int(x), Const::Float(y)) => floats(op, x as f64, y),
        (Const::Float(x), Const::Int(y)) => floats(op, x, y as f64),
        (Const::Float(x), Const::Float(y)) => floats(op, x, y),
        _ => None,
    }
}

/// Ints as the VM computes them (`value/arith.rs`): `/` is always a float,
/// `//` floors and `%` takes the divisor's sign.
fn ints(op: &str, x: i64, y: i64) -> Option<Const> {
    let r = match op {
        "+" => x.checked_add(y)?,
        "-" => x.checked_sub(y)?,
        "*" => x.checked_mul(y)?,
        "/" => return floats(op, x as f64, y as f64),
        "//" => {
            if y == 0 || (x == i64::MIN && y == -1) {
                return None;
            }
            let q = x / y;
            if x % y != 0 && (x < 0) != (y < 0) {
                q - 1
            } else {
                q
            }
        }
        "%" => {
            if y == 0 {
                return None;
            }
            if x == i64::MIN && y == -1 {
                0
            } else {
                let r = x % y;
                if r != 0 && (r < 0) != (y < 0) {
                    r + y
                } else {
                    r
                }
            }
        }
        _ => return None,
    };
    Some(Const::Int(r))
}

/// Floats: `+ - * /` with a finite result; `//` and `%` are left to the VM.
fn floats(op: &str, x: f64, y: f64) -> Option<Const> {
    let r = match op {
        "+" => x + y,
        "-" => x - y,
        "*" => x * y,
        "/" if y != 0.0 => x / y,
        _ => return None,
    };
    float(r)
}

/// A finite float as a constant. A negative zero is left to the VM: the
/// constant pool keeps one zero (`ConstKey`), and `-0.0` prints as such.
fn float(r: f64) -> Option<Const> {
    (r.is_finite() && !(r == 0.0 && r.is_sign_negative())).then_some(Const::Float(r))
}
