//! Values that are used must not be a silent nil (01: 값을 쓰는 자리, L3):
//! an if without else there is E0223, and a block, if branch or match arm
//! there that ends in an assignment is E0224.
//!
//! A pass over the desugared tree, before names are resolved. `used` says
//! whether the node's value is used. A block's items before the last are
//! not; its last is when the block is. A block-bodied function's last item
//! is not used (a side-effect function often ends in an if), an
//! expression-bodied one's (`fn f() = e`, `x => e`) is. A `for` body is
//! used when its `for` is (`map`, not the `each` of a discarded `for`).

use super::{lit_str, name_error};
use crate::diag::{Diag, error};
use crate::tree::{Head, Lit, Node};

pub(super) fn check(n: &Node, used: bool, errs: &mut Vec<Diag>) {
    match n.head {
        Head::Do => {
            let last = n.kids.len().saturating_sub(1);
            for (i, k) in n.kids.iter().enumerate() {
                check(k, used && i == last, errs);
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
        Head::Fn => function(n, fn_body_used(n), errs),
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

fn fn_body_used(n: &Node) -> bool {
    match &n.lit {
        Lit::Fn(info) => info.expr_body,
        _ => false,
    }
}

fn for_body(n: &Node) -> bool {
    matches!(&n.lit, Lit::Fn(info) if info.for_body)
}
