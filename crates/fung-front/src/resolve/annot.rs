//! Literals where a type annotation stands (D113): a parameter's default,
//! a value the function returns, an argument of a call whose function is
//! known. When the syntax shows the value's type (`value::known_type`)
//! and the annotation does not take it, the run's error (E0403, E0404)
//! is a compile error, with the run's wording: the union as the run
//! writes it (aliases expanded, joined with ` | `), the type as the run
//! names it.

use super::call::Sig;
use super::lit_str;
use super::value::known_type;
use crate::diag::{Arg, Diag, error};
use crate::tree::{Head, Node, Param, TypeTerm};

/// The type `lit`'s syntax shows, when the union `types` does not take
/// it (as `fung-vm`'s `TypeSet::matches`): `any` takes everything, `num`
/// an int or a float, a tag name the tag's values and its symbol.
fn refused<'n>(types: &[String], lit: &'n Node) -> Option<&'n str> {
    let t = known_type(lit)?;
    let takes = |a: &str| match a {
        "any" => true,
        "num" => matches!(t, "int" | "float"),
        _ => a == t || (lit.head == Head::Symbol && lit_str(lit) == a),
    };
    (!types.iter().any(|a| takes(a))).then_some(t)
}

/// E0403 for a literal `lit` bound to parameter `name` of type `types`.
fn argument(name: &str, types: &[String], lit: &Node) -> Option<Diag> {
    let got = refused(types, lit)?;
    let args = vec![
        Arg::Str(name.to_string()),
        Arg::Str(types.join(" | ")),
        Arg::Str(got.to_string()),
    ];
    Some(error("E0403", lit.pos, args))
}

fn names(types: &[TypeTerm]) -> Vec<String> {
    types.iter().map(|t| t.name.clone()).collect()
}

/// A parameter's default (`fn f(a: int = "x")`), `p` with its types
/// expanded.
pub(super) fn default_error(p: &Param, default: &Node) -> Option<Diag> {
    argument(&p.name, &names(p.types.as_ref()?), default)
}

/// The first literal argument of a call that its parameter's annotation
/// refuses, in the order of the parameters (as the run checks them).
/// Positional arguments before a spread, and named ones.
pub(super) fn argument_error(sig: &Sig, args: &[Node]) -> Option<Diag> {
    let n = sig.params.len();
    let fixed = if n > 0 && sig.params[n - 1].rest {
        n - 1
    } else {
        n
    };
    let mut bound: Vec<Option<&Node>> = vec![None; fixed];
    let mut i = 0;
    let mut spread = false;
    for a in args {
        match a.head {
            Head::Spread => spread = true,
            Head::Named => {
                let at = sig.params[..fixed]
                    .iter()
                    .position(|p| p.name == lit_str(a));
                if let Some(j) = at {
                    bound[j] = a.kids.first();
                }
            }
            _ if !spread => {
                if i < fixed {
                    bound[i] = Some(a);
                }
                i += 1;
            }
            _ => {}
        }
    }
    sig.params[..fixed]
        .iter()
        .zip(bound)
        .find_map(|(p, lit)| argument(&p.name, p.types.as_ref()?, lit?))
}

/// E0404 for each literal the function returns that its return type
/// `ret` (expanded) refuses: where the body ends, every branch end of an
/// `if` with `else`, `match` or `then … else` there, and every `return`
/// (not those of a function inside).
pub(super) fn return_errors(ret: &[TypeTerm], body: &Node, errs: &mut Vec<Diag>) {
    let mut lits = Vec::new();
    ends(body, &mut lits);
    returns(body, &mut lits);
    lits.sort_by_key(|n| (n.pos.line, n.pos.col));
    let ret = names(ret);
    for lit in lits {
        if let Some(got) = refused(&ret, lit) {
            let args = vec![Arg::Str(ret.join(" | ")), Arg::Str(got.to_string())];
            errs.push(error("E0404", lit.pos, args));
        }
    }
}

/// The literals `n`'s value ends in.
fn ends<'n>(n: &'n Node, out: &mut Vec<&'n Node>) {
    if known_type(n).is_some() {
        out.push(n);
        return;
    }
    match n.head {
        Head::Do => {
            if let Some(last) = n.kids.last() {
                ends(last, out);
            }
        }
        // a block if; the `nil` the parser puts for a missing `else` is
        // E0223's
        Head::If if n.kids[1].head == Head::Do => {
            ends(&n.kids[1], out);
            if n.kids[2].head != Head::Nil {
                ends(&n.kids[2], out);
            }
        }
        Head::Then => {
            ends(&n.kids[1], out);
            ends(&n.kids[2], out);
        }
        Head::Match => {
            for arm in &n.kids[1..] {
                ends(&arm.kids[2], out);
            }
        }
        _ => {}
    }
}

/// The literals the `return`s in `n` return, outside functions inside.
fn returns<'n>(n: &'n Node, out: &mut Vec<&'n Node>) {
    match n.head {
        Head::Fn => {}
        Head::Return => ends(&n.kids[0], out),
        _ => {
            for k in &n.kids {
                returns(k, out);
            }
        }
    }
}
