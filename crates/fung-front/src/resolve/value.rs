//! Values that are used must not be a silent nil (01: 값을 쓰는 자리, L3,
//! FE4): an if without else there is E0223, a block, if branch or match arm
//! there that ends in an assignment is E0224, and one that is empty or ends
//! in another statement (`assert`, a `fn` definition) is E0231. The hidden
//! assignment that unpacks a pattern parameter is not a statement of the
//! body.
//!
//! A pass over the desugared tree, before names are resolved. `used` says
//! whether the node's value is used. A block's items before the last are
//! not; its last is when the block is. A function's body is its value, so
//! its last item is used, block-bodied or not (L2: a function that ends in
//! an if without else would be a silent nil to a caller that uses it). A
//! `for` body is used when its `for` is (`map`, not the `each` of a
//! discarded `for`). A test block's body is not: its value is discarded.

use super::{lit_str, name_error};
use crate::diag::{Arg, Diag, error};
use crate::tree::{Head, Lit, Node};

pub(super) fn check(n: &Node, used: bool, errs: &mut Vec<Diag>) {
    match n.head {
        Head::Do => {
            let last = n.kids.len().saturating_sub(1);
            for (i, k) in n.kids.iter().enumerate() {
                check(k, used && i == last && !hidden(k), errs);
            }
            if used {
                match n.kids.iter().rev().find(|k| !hidden(k)) {
                    None => errs.push(error("E0231", n.pos, vec![Arg::Str("is empty".into())])),
                    Some(k) if k.head == Head::Assert => errs.push(
                        error("E0231", k.pos, vec![Arg::Str("ends in `assert`".into())]).span(6),
                    ),
                    Some(k) if k.head == Head::Fn && !fn_name(k).is_empty() => errs.push(
                        error(
                            "E0231",
                            k.pos,
                            vec![Arg::Str(format!(
                                "ends in the definition of `{}`",
                                fn_name(k)
                            ))],
                        )
                        .span(2),
                    ),
                    Some(_) => {}
                }
            }
        }
        Head::Let => {
            if used {
                let t = &n.kids[0];
                let name = if t.head == Head::Name {
                    lit_str(t)
                } else {
                    "this pattern"
                };
                errs.push(name_error("E0224", n.pos, name));
            }
            check(&n.kids[1], true, errs);
        }
        Head::If => {
            check(&n.kids[0], true, errs);
            for b in &n.kids[1..] {
                check(b, used, errs);
            }
            // Only a block if can lack else: the parser gives it a nil leaf.
            if used && n.kids[1].head == Head::Do && n.kids[2].head == Head::Nil {
                errs.push(error("E0223", n.pos, vec![]).span(2));
            }
        }
        Head::Match => {
            check(&n.kids[0], true, errs);
            for arm in &n.kids[1..] {
                check(&arm.kids[1], true, errs);
                check(&arm.kids[2], used, errs);
            }
        }
        Head::Fn => function(n, true, errs),
        // `fung test` discards what a test block returns (E0606 for an Err).
        Head::Test => function(&n.kids[0], false, errs),
        Head::Call => {
            check(&n.kids[0], true, errs);
            for k in &n.kids[1..] {
                if k.head == Head::Fn && for_body(k) {
                    function(k, used, errs);
                } else {
                    check(k, true, errs);
                }
            }
        }
        // Patterns and leaves hold no values to check.
        Head::Import | Head::Typealias => {}
        _ => {
            for k in &n.kids {
                check(k, true, errs);
            }
        }
    }
}

/// A function: its defaults are used, its body's last item as `body_used`.
fn function(n: &Node, body_used: bool, errs: &mut Vec<Diag>) {
    let (body, defaults) = n.kids.split_last().expect("fn has a body");
    for d in defaults {
        check(d, true, errs);
    }
    check(body, body_used, errs);
}

fn for_body(n: &Node) -> bool {
    matches!(&n.lit, Lit::Fn(info) if info.for_body)
}

/// The assignment that unpacks a pattern parameter at the top of a body
/// (`desugar::fn_node`): its value is a hidden `$` name.
fn hidden(n: &Node) -> bool {
    n.head == Head::Let && n.kids[1].head == Head::Name && lit_str(&n.kids[1]).starts_with('$')
}

/// A named fn's name; empty for a lambda.
fn fn_name(n: &Node) -> &str {
    match &n.lit {
        Lit::Fn(info) => &info.name,
        _ => "",
    }
}
