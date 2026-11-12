//! Constant folding (VM10 g, VM25, VM31): an operator whose operands are
//! constants, or folded operators, is computed once here; so is a list,
//! map, tuple, tag or range literal whose parts are all constants (L5),
//! and an interpolation of string and int literals. Only where the VM's
//! result is certain: an int that would overflow, a division by zero or
//! a float that is not finite is left to the VM, so the error comes at
//! the same time and place as before.

use std::collections::HashMap;

use super::{lit_str, str_const};
use crate::consts::Const;
use crate::tree::{Head, Lit, Node};

/// The constants of the operators, the literals of constants and the
/// interpolations that fold in `root`'s tree (nested functions too), by
/// node number (`Node::id`): one walk, bottom up, so each node is looked
/// at once (VM18: folding each operator from scratch walked its whole
/// left chain again; VM25: so did asking each nested literal whether it
/// is a constant).
pub(super) fn constants(root: &Node) -> HashMap<u32, Const> {
    let mut out = HashMap::new();
    walk(root, &mut out);
    out
}

/// The constant `n` stands for, if any; what folds below `n` goes into
/// `out`.
fn walk(n: &Node, out: &mut HashMap<u32, Const>) -> Option<Const> {
    let all_kids = matches!(
        n.head,
        Head::List
            | Head::Tuple
            | Head::Tag
            | Head::Map
            | Head::Range
            | Head::Concat
            | Head::Op
            | Head::Entry
    );
    let mut cs: Vec<Option<Const>> = Vec::new();
    for k in &n.kids {
        let c = walk(k, out);
        if all_kids {
            cs.push(c);
        }
    }
    let all = |cs: Vec<Option<Const>>| cs.into_iter().collect::<Option<Vec<_>>>();
    let c = match (n.head, &n.lit) {
        (Head::Int, Lit::Int(i)) => Some(Const::Int(*i)),
        (Head::Float, Lit::Float(f)) => Some(Const::Float(*f)),
        (Head::Bool, Lit::Bool(b)) => Some(Const::Bool(*b)),
        (Head::Nil, _) => Some(Const::Nil),
        (Head::String, Lit::Str(_) | Lit::Bytes(_)) => Some(str_const(n)),
        (Head::Symbol, Lit::Str(s)) => Some(Const::Sym(s.clone())),
        (Head::Duration, Lit::Dur(d)) => Some(Const::Dur(*d)),
        (Head::Op, _) if cs.len() == 1 && lit_str(n) == "-" => match cs.pop()?? {
            Const::Int(i) => i.checked_neg().map(Const::Int),
            Const::Float(f) => float(-f),
            _ => None,
        },
        (Head::Op, _) if cs.len() == 2 => {
            let b = cs.pop()??;
            let a = cs.pop()??;
            binary(lit_str(n), a, b)
        }
        (Head::Entry, _) => cs.pop().flatten(),
        (Head::List, _) => all(cs).map(Const::List),
        (Head::Tuple, _) => all(cs).map(Const::Tuple),
        (Head::Tag, Lit::Str(name)) => all(cs).map(|xs| Const::Tag(name.clone(), xs)),
        (Head::Map, _) => n
            .kids
            .iter()
            .zip(cs)
            .map(|(e, v)| Some((str_const(e), v?)))
            .collect::<Option<Vec<_>>>()
            .map(Const::Map),
        (Head::Range, _) => match all(cs)?.as_slice() {
            // A start after the end is not folded: the range is made at
            // run time, where it is E0522 (D7).
            [Const::Int(a), Const::Int(b)]
                if i128::from(*a) <= i128::from(*b) + i128::from(lit_str(n) == "..=") =>
            {
                Some(Const::Range(*a, *b, lit_str(n) == "..="))
            }
            _ => None,
        },
        (Head::Concat, _) => interpolation(all(cs)?),
        _ => None,
    };
    if matches!(
        n.head,
        Head::Op | Head::List | Head::Tuple | Head::Tag | Head::Map | Head::Range | Head::Concat
    ) && n.id != 0
        && let Some(c) = &c
    {
        out.insert(n.id, c.clone());
    }
    c
}

/// The bytes of a string constant.
fn bytes_of(c: &Const) -> Option<&[u8]> {
    match c {
        Const::Str(s) => Some(s.as_bytes()),
        Const::Bytes(b) => Some(b),
        _ => None,
    }
}

/// A string constant of `b`: text when it is valid UTF-8, as the VM's
/// strings are (L4).
fn text(b: Vec<u8>) -> Const {
    match String::from_utf8(b) {
        Ok(s) => Const::Str(s),
        Err(e) => Const::Bytes(e.into_bytes()),
    }
}

/// An interpolation whose parts are string and int literals (VM31); other
/// parts (floats, bools, values the VM refuses) are left to CONCAT.
fn interpolation(parts: Vec<Const>) -> Option<Const> {
    let mut b = Vec::new();
    for p in &parts {
        match p {
            Const::Int(i) => b.extend_from_slice(i.to_string().as_bytes()),
            p => b.extend_from_slice(bytes_of(p)?),
        }
    }
    Some(text(b))
}

fn binary(op: &str, a: Const, b: Const) -> Option<Const> {
    // Two string literals joined (VM31).
    if op == "+"
        && let (Some(x), Some(y)) = (bytes_of(&a), bytes_of(&b))
    {
        return Some(text([x, y].concat()));
    }
    match (a, b) {
        (Const::Int(x), Const::Int(y)) => ints(op, x, y),
        (Const::Int(x), Const::Float(y)) => floats(op, exact(x)?, y),
        (Const::Float(x), Const::Int(y)) => floats(op, x, exact(y)?),
        (Const::Float(x), Const::Float(y)) => floats(op, x, y),
        _ => None,
    }
}

/// An int meeting a float, when a float holds it exactly; otherwise the
/// VM's E0523 (D48).
fn exact(i: i64) -> Option<f64> {
    let f = i as f64;
    (f < 9223372036854775808.0 && f as i64 == i).then_some(f)
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
