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

/// The pure builtins that take a function (D90): their E0234 says where a
/// pass for its effects goes.
const RUNS_A_FUNCTION: &[&str] = &[
    "map", "try_map", "filter", "fold", "sort_by", "find", "any", "all", "group_by",
];

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
    if let Some(e) = literal_type_error(n) {
        cx.errs.push(e);
    }
    if place == Place::Discarded && pointless(n) {
        cx.errs.push(discarded(n));
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
                .with_help(&if RUNS_A_FUNCTION.contains(&name.as_str()) {
                    format!(
                        "`{name}` computes a value with its function; use the value, or run the function for its effects with `each` or a `for` statement"
                    )
                } else {
                    format!(
                        "`{name}` has no effect: it only computes its result (a changed copy, a converted value); use the result, or write `_ = …` to discard it on purpose"
                    )
                }),
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
                    cx.errs.push(discarded(k));
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
        // `a and b` and `a or b` (an `if` over no blocks) are operators:
        // their operands are used. `c then a else b` is its own node.
        Head::If => {
            check(&n.kids[0], Place::Used, cx);
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
            // A test with nothing in it passes without checking anything
            // (D100).
            let body = n.kids[0].kids.last().expect("fn has a body");
            if body.head == Head::Do && body.kids.is_empty() {
                cx.errs
                    .push(error("E0242", n.pos, vec![Arg::Str(lit_str(n).to_string())]).span(4));
            }
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
        // `and` and `or` (an `if` over no blocks), and `then … else`.
        Head::If => n.kids[1].head != Head::Do && n.kids.iter().all(effect_free),
        Head::Then => n.kids.iter().all(effect_free),
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
/// the usual slips: a call without `()`, `==` for `=`. (A line that
/// starts with `-` after a statement is the parser's E0136, D85.)
fn discarded(n: &Node) -> Diag {
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
    let help = match n.head {
        Head::Name => {
            format!("to call it, add `()`: `{op}()`; to discard it on purpose, write `_ = {op}`")
        }
        Head::Fn => "call it, or assign it to a name".to_string(),
        Head::Do => {
            "a call with `_` makes a function without calling it; pass every argument to call it"
                .to_string()
        }
        Head::Op if op == "==" => EQ_HELP.to_string(),
        _ => return error("E0234", n.pos, vec![Arg::Str(what)]).span(len),
    };
    error("E0234", n.pos, vec![Arg::Str(what)])
        .span(len)
        .with_help(&help)
}

/// E0234's help for a discarded `==`, and the same in a test body.
const EQ_HELP: &str = "did you mean `=`? `==` compares; `=` assigns";
const TEST_EQ_HELP: &str =
    "did you mean `assert` before it? a test checks a comparison with `assert a == b`";

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

/// The type of a scalar literal, as the run's messages name it.
fn literal_type(n: &Node) -> Option<&'static str> {
    Some(match n.head {
        Head::Int => "int",
        Head::Float => "float",
        Head::String => "str",
        Head::Bool => "bool",
        Head::Nil => "nil",
        Head::Symbol => "sym",
        _ => return None,
    })
}

/// What the types of the literals in `n` settle before the program runs
/// (D99): the error the run would stop with, under its number. Only
/// what holds whatever the values are: an overflow or a division by
/// zero is the run's to find.
fn literal_type_error(n: &Node) -> Option<Diag> {
    let not_bool = |c: &Node| {
        literal_type(c)
            .filter(|t| *t != "bool")
            .map(|t| error("E0505", c.pos, vec![Arg::Str(t.into())]))
    };
    match n.head {
        // a literal called
        Head::Call => {
            let t = literal_type(&n.kids[0])?;
            Some(error("E0401", n.kids[0].pos, vec![Arg::Str(t.into())]))
        }
        // a condition, and the left side of `and`/`or`
        Head::If | Head::Then => not_bool(&n.kids[0]).or_else(|| {
            // `a and b` is `(if a (bool b) false)`, `a or b` `(if a true (bool b))`
            n.kids[1..]
                .iter()
                .filter(|k| k.head == Head::Op && lit_str(k) == crate::tree::BOOL_CHECK)
                .find_map(|k| not_bool(&k.kids[0]))
        }),
        Head::Op if n.kids.len() == 1 && lit_str(n) == "not" => not_bool(&n.kids[0]),
        Head::Op if n.kids.len() == 2 => {
            let (a, b) = (literal_type(&n.kids[0])?, literal_type(&n.kids[1])?);
            let num = |t: &str| t == "int" || t == "float";
            let op = lit_str(n);
            let verb = match op {
                "+" => "add",
                "-" => "subtract",
                "*" => "multiply",
                "/" | "//" => "divide",
                "%" => "take the remainder of",
                "**" => "raise",
                "<" | ">" | "<=" | ">=" => "compare",
                _ => return None,
            };
            let fine = (num(a) && num(b))
                || (a == "str" && b == "str" && matches!(verb, "add" | "compare"));
            (!fine).then(|| {
                error(
                    "E0501",
                    n.pos,
                    vec![
                        Arg::Str(verb.into()),
                        Arg::Str(a.into()),
                        Arg::Str(b.into()),
                    ],
                )
            })
        }
        // `??` after a literal that is never nil or a result
        Head::Coalesce => {
            let left = &n.kids[0];
            let what = match left.head {
                Head::Nil => return None,
                Head::List => "a list",
                Head::Map => "a map",
                Head::Tuple => "a tuple",
                _ => match literal_type(left)? {
                    "int" | "float" => "a number",
                    "str" => "a string",
                    "bool" => "a bool",
                    _ => "a symbol",
                },
            };
            Some(error("E0243", n.pos, vec![Arg::Str(what.into())]).span(2))
        }
        _ => None,
    }
}
