//! Values that are used must not be a silent nil (01: 값을 쓰는 자리, L3,
//! FE4): an if without else there is E0223, and a block, if branch or
//! match arm there that is empty or ends in a statement (an assignment,
//! `assert`, a `fn` definition) is E0224 (E0231 merged into it, SP24).
//! The hidden
//! assignment that unpacks a pattern parameter is not a statement of the
//! body.
//!
//! Values that are discarded must do something (FE24): a name, a literal,
//! a lambda, a partial application, or operators, fields, indexes, tuples,
//! lists, maps and tag values of only those there is E0234. `nil` alone is exempt: it
//! is how a branch says "nothing". So is a discarded call of a builtin
//! known to have no effect (D39: `put(m, k, v)` changes nothing), which
//! resolve notes by position (`Pure`).
//!
//! A pass over the desugared tree, before names are resolved. `Place` says
//! whether the node's value is used. A block's items before the last are
//! discarded; its last is in the block's place. A function's body is its
//! value, so its last item is used, block-bodied or not (L2: a function
//! that ends in an if without else would be a silent nil to a caller that
//! uses it). A `for` body is in its `for`'s place (`map`, not the `each`
//! of a discarded `for`). A test block's body is discarded: `fung test`
//! drops its value.

use super::{lit_str, rune_len};
use crate::diag::{Arg, Diag, error};
use crate::source::Pos;
use crate::tree::{Head, Lit, Node};

/// Where a value goes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Used,
    Discarded,
    /// The file's last value in the REPL: printed, so neither used as a
    /// value nor discarded.
    Kept,
}

/// The calls of builtins known to have no effect, by `(line, col)`, with
/// the name their error shows and where it is written (D39).
pub(super) type Pure = std::collections::HashMap<(u32, u32), (String, Pos)>;

/// What the walk carries: the pure calls, and the errors found.
struct Ctx<'a> {
    pure: &'a Pure,
    errs: &'a mut Vec<Diag>,
}

/// Checks a file. `keep_value`: its last item is the REPL's value.
pub(super) fn check_file(n: &Node, keep_value: bool, pure: &Pure, errs: &mut Vec<Diag>) {
    let place = if keep_value {
        Place::Kept
    } else {
        Place::Discarded
    };
    check(n, place, &mut Ctx { pure, errs });
}

fn check(n: &Node, place: Place, cx: &mut Ctx<'_>) {
    if place == Place::Discarded && pointless(n) {
        cx.errs.push(discarded(n, None));
        // Still check what it holds (a lambda's body), as a value.
        return check(n, Place::Used, cx);
    }
    if place == Place::Discarded
        && n.head == Head::Call
        && let Some((name, at)) = cx.pure.get(&(n.pos.line, n.pos.col))
    {
        cx.errs.push(
            error("E0234", *at, vec![Arg::Str(format!("the result of `{name}(…)`"))])
                .span(rune_len(name))
                .with_help(&format!(
                    "`{name}` has no effect: it only computes its result (a changed copy, a converted value); use the result, or write `_ = …` to discard it on purpose"
                )),
        );
    }
    let used = place == Place::Used;
    match n.head {
        // `h(g(), _)` computes g(): its function is a value, not a statement.
        Head::Do if partial_shape(n) => {
            let (f, lets) = n.kids.split_last().expect("a partial has its lambda");
            for l in lets {
                check(l, Place::Discarded, cx);
            }
            check(f, Place::Used, cx);
        }
        Head::Do => {
            let last = n.kids.len().saturating_sub(1);
            for (i, k) in n.kids.iter().enumerate() {
                if i == last && !hidden(k) {
                    check(k, place, cx);
                } else if pointless(k) {
                    // Knowing the next statement explains a `- 2` line.
                    cx.errs.push(discarded(k, n.kids.get(i + 1)));
                    check(k, Place::Used, cx);
                } else {
                    check(k, Place::Discarded, cx);
                }
            }
            if used {
                match n.kids.iter().rev().find(|k| !hidden(k)) {
                    None => cx.errs.push(not_a_value(n.pos, "is empty".into())),
                    Some(k) if k.head == Head::Assert => {
                        cx.errs
                            .push(not_a_value(k.pos, "ends with `assert`".into()).span(6));
                    }
                    Some(k) if k.head == Head::Fn && !fn_name(k).is_empty() => cx.errs.push(
                        not_a_value(
                            k.pos,
                            format!("ends with the definition of `{}`", fn_name(k)),
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
                // The caret is under the name, or at a pattern (FE28).
                let e = match t.head {
                    Head::Name => {
                        let name = lit_str(t);
                        not_a_value(n.pos, format!("ends with an assignment to `{name}`"))
                            .span(rune_len(name))
                    }
                    Head::Pwild => not_a_value(t.pos, "ends with an assignment to `_`".into()),
                    _ => not_a_value(t.pos, "ends with an assignment to a pattern".into()),
                };
                cx.errs.push(e);
            }
            check(&n.kids[1], Place::Used, cx);
        }
        Head::If => {
            check(&n.kids[0], Place::Used, cx);
            // `a and b`, `a or b` and `c then a else b` (no blocks) are
            // operators: their operands are used.
            let branch = if n.kids[1].head == Head::Do {
                place
            } else {
                Place::Used
            };
            for b in &n.kids[1..] {
                check(b, branch, cx);
            }
            // Only a block if can lack else: the parser gives it a nil leaf.
            if used && n.kids[1].head == Head::Do && n.kids[2].head == Head::Nil {
                cx.errs.push(error("E0223", n.pos, vec![]).span(2));
            }
        }
        Head::Match => {
            check(&n.kids[0], Place::Used, cx);
            for arm in &n.kids[1..] {
                check(&arm.kids[1], Place::Used, cx);
                check(&arm.kids[2], place, cx);
            }
        }
        Head::Fn => function(n, Place::Used, cx),
        // `fung test` discards what a test block returns (E0606 for an Err).
        // A discarded `==` there is most likely a missing `assert`.
        Head::Test => {
            let from = cx.errs.len();
            function(&n.kids[0], Place::Discarded, cx);
            for e in &mut cx.errs[from..] {
                if e.code == "E0234" && e.help.as_deref() == Some(EQ_HELP) {
                    e.help = Some(TEST_EQ_HELP.to_string());
                }
            }
        }
        Head::Call => {
            check(&n.kids[0], Place::Used, cx);
            for k in &n.kids[1..] {
                if k.head == Head::Fn && for_body(k) {
                    function(k, place, cx);
                } else {
                    check(k, Place::Used, cx);
                }
            }
        }
        // Patterns and leaves hold no values to check.
        Head::Import | Head::Typealias => {}
        _ => {
            for k in &n.kids {
                check(k, Place::Used, cx);
            }
        }
    }
}

/// A function: its defaults are used, its body's last item is in `body`.
fn function(n: &Node, body: Place, cx: &mut Ctx<'_>) {
    let (b, defaults) = n.kids.split_last().expect("fn has a body");
    for d in defaults {
        check(d, Place::Used, cx);
    }
    check(b, body, cx);
}

/// An effect-free value that is an error where it is discarded: all but
/// `nil` alone, which is how a branch says "nothing".
fn pointless(n: &Node) -> bool {
    n.head != Head::Nil && effect_free(n)
}

/// Whether computing n can do nothing but make a value (FE24): names,
/// literals, lambdas, partial applications, and operators, fields,
/// indexes, slices, ranges, tuples, lists, maps and tag values of only
/// those. Calls, assignments, `?`, `return`, `assert`, definitions and
/// blocks may do something. A discarded `Err` computed at run time is
/// still E0606.
fn effect_free(n: &Node) -> bool {
    match n.head {
        Head::Name
        | Head::Prelude
        | Head::SelfFn
        | Head::Global
        | Head::Local
        | Head::Captured
        | Head::Member
        | Head::Int
        | Head::Float
        | Head::String
        | Head::Bool
        | Head::Nil
        | Head::Symbol
        | Head::Duration
        | Head::Missing => true,
        Head::Fn => fn_name(n).is_empty() && !for_body(n),
        Head::Do => partial(n),
        // `and`, `or` and `then … else` without blocks.
        Head::If => n.kids[1].head != Head::Do && n.kids.iter().all(effect_free),
        Head::Op
        | Head::Field
        | Head::Index
        | Head::Slice
        | Head::Range
        | Head::Tuple
        | Head::List
        | Head::Map
        | Head::Entry
        | Head::Interp
        | Head::Coalesce
        | Head::Concat
        | Head::Tag => n.kids.iter().all(effect_free),
        _ => false,
    }
}

/// A partial application (`desugar::partial_with`): hidden assignments
/// of the callee and arguments, then the lambda over the holes. One whose
/// callee or fixed arguments compute something (`h(g(), _)`) is not
/// effect-free.
fn partial(n: &Node) -> bool {
    partial_shape(n) && n.kids.iter().rev().skip(1).all(|l| effect_free(&l.kids[1]))
}

/// The shape of a partial application, whatever its arguments compute.
fn partial_shape(n: &Node) -> bool {
    match n.kids.split_last() {
        Some((f, lets)) => {
            f.head == Head::Fn
                && fn_name(f).is_empty()
                && !lets.is_empty()
                && lets.iter().all(|l| {
                    l.head == Head::Let
                        && l.kids[0].head == Head::Name
                        && crate::tree::is_hidden(lit_str(&l.kids[0]))
                })
        }
        None => false,
    }
}

/// E0234 for an effect-free value in a discarded place, with help for
/// the usual slips: a call without `()`, `==` for `=`, a line that starts
/// with `-` meant to continue the line above (`next` is the statement
/// after n, if any).
fn discarded(n: &Node, next: Option<&Node>) -> Diag {
    let op = lit_str(n);
    let (what, len) = match n.head {
        Head::Name => (format!("the value of `{op}`"), rune_len(op)),
        Head::Fn => ("a lambda".to_string(), 1),
        Head::Do => ("a partial application".to_string(), 1),
        Head::Op if matches!(op, "==" | "!=" | "<" | "<=" | ">" | ">=") => {
            (format!("the comparison `{op}`"), rune_len(op))
        }
        Head::Op => (format!("the result of `{op}`"), rune_len(op)),
        Head::Tag => (format!("the tag value `{op}(…)`"), rune_len(op)),
        _ => ("this value".to_string(), 1),
    };
    let help = if next.is_some_and(unary_minus) {
        "the next line starts with `-`, so it is a new statement; to continue this expression, end this line with `-`".to_string()
    } else if unary_minus(n) {
        "a line that starts with `-` is a new statement; to continue the line above, end that line with `-`".to_string()
    } else {
        match n.head {
            Head::Name => {
                format!("to call it, add `()`: `{op}()`; to discard it on purpose, write `_ = {op}`")
            }
            Head::Fn => "call it, or assign it to a name".to_string(),
            Head::Do => "a call with `_` makes a function without calling it; pass every argument to call it".to_string(),
            Head::Op if op == "==" => EQ_HELP.to_string(),
            _ => return error("E0234", n.pos, vec![Arg::Str(what)]).span(len),
        }
    };
    error("E0234", n.pos, vec![Arg::Str(what)])
        .span(len)
        .with_help(&help)
}

/// E0234's help for a discarded `==`, and the same in a test body.
const EQ_HELP: &str = "did you mean `=`? `==` compares; `=` assigns";
const TEST_EQ_HELP: &str =
    "did you mean `assert` before it? a test checks a comparison with `assert a == b`";

/// `-x`: a statement that starts with `-` (01: 줄 이어짐).
fn unary_minus(n: &Node) -> bool {
    n.head == Head::Op && n.kids.len() == 1 && lit_str(n) == "-"
}

fn for_body(n: &Node) -> bool {
    matches!(&n.lit, Lit::Fn(info) if info.for_body)
}

/// The assignment that unpacks a pattern parameter at the top of a body
/// (`desugar::fn_node`): its value is a hidden `$` name.
fn hidden(n: &Node) -> bool {
    n.head == Head::Let
        && n.kids[1].head == Head::Name
        && crate::tree::is_hidden(lit_str(&n.kids[1]))
}

/// E0224: a block used as a value that `what` (is empty, ends with a
/// statement), so it is a silent nil.
fn not_a_value(pos: Pos, what: String) -> Diag {
    error("E0224", pos, vec![Arg::Str(what)])
}

/// A named fn's name; empty for a lambda.
fn fn_name(n: &Node) -> &str {
    match &n.lit {
        Lit::Fn(info) => &info.name,
        _ => "",
    }
}
