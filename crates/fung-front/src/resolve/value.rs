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
//! resolve notes by position (`Pure`); on effect-free arguments such a
//! call is itself effect-free inside a bigger value (FE77: `n == len(xs)`).
//!
//! A pass over the desugared tree, before names are resolved. `Place` says
//! whether the node's value is used. A block's items before the last are
//! discarded; its last is in the block's place. A function's body is its
//! value, so its last item is used, block-bodied or not (L2: a function
//! that ends in an if without else would be a silent nil to a caller that
//! uses it). The branches of a block `if`, a `match` and `then … else`
//! are in their node's place (D121: a discarded `c then f() else nil` is
//! E0234 on the `nil`, as the statement `if` is). A `for` body is in its `for`'s place (`map`, not the `each`
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

/// A call of a function known to have no effect (D39).
pub(super) struct PureCall {
    /// The callee as the error shows it: as written (`s`, `fs.read`).
    pub(super) shown: String,
    /// The function's own name, for the help (`sort`).
    pub(super) own: String,
    /// Where the callee is written.
    pub(super) at: Pos,
}

/// The calls of functions known to have no effect, by `(line, col)`.
pub(super) type Pure = std::collections::HashMap<(u32, u32), PureCall>;

/// The pure builtins that take a function (D90): their E0234 says where a
/// pass for its effects goes.
const RUNS_A_FUNCTION: &[&str] = &[
    "map", "try_map", "filter", "fold", "sort_by", "find", "any", "all", "group_by", "iterate",
    "until",
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
    if place == Place::Discarded && pointless(n, cx.pure) {
        cx.errs.push(discarded(n, cx.pure));
        // Still check what it holds (a lambda's body), as a value.
        return check(n, Place::Used, cx);
    }
    // A pure call whose arguments compute something still drops its
    // result (D39).
    if place == Place::Discarded
        && let Some(e) = pure_call(n, cx.pure)
    {
        cx.errs.push(e);
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
                } else if pointless(k, cx.pure) {
                    cx.errs.push(discarded(k, cx.pure));
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
            for (i, b) in n.kids[1..].iter().enumerate() {
                // The `nil` the parser puts for a missing `else` is not a
                // statement.
                if i == 1 && branch != Place::Used && b.head == Head::Nil {
                    continue;
                }
                check(b, branch, cx);
            }
            // Only a block if can lack else: the parser gives it a nil leaf.
            if used && n.kids[1].head == Head::Do && n.kids[2].head == Head::Nil {
                cx.errs.push(error("E0223", n.pos, vec![]).span(2));
            }
        }
        Head::Match => {
            check(&n.kids[0], Place::Used, cx);
            let arms = &n.kids[1..];
            for (i, arm) in arms.iter().enumerate() {
                check(&arm.kids[1], Place::Used, cx);
                let from = cx.errs.len();
                check(&arm.kids[2], place, cx);
                // `1 => y => y + 1`: `y =>` was read as the next arm, so
                // this one is empty (FE25).
                let body = &arm.kids[2];
                if used
                    && body.kids.is_empty()
                    && let Some(next) = arms.get(i + 1)
                    && next.kids[0].head == Head::Pbind
                    && next.kids[0].pos.line == body.pos.line
                    && let Some(e) = cx.errs[from..].iter_mut().find(|e| e.code == "E0224")
                {
                    e.help = Some(
                        "a lambda as an arm's value goes in parentheses: `1 => (y => y + 1)`"
                            .to_string(),
                    );
                }
            }
        }
        // `c then a else b` puts its branches in its own place, as a block
        // `if` does: discarded, each branch is discarded (D121).
        Head::Then => {
            check(&n.kids[0], Place::Used, cx);
            for b in &n.kids[1..] {
                check(b, place, cx);
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

/// An effect-free value that is an error where it is discarded, `nil`
/// alone too: a branch that does nothing is left out (`if` without
/// `else`) or empty (`_ =>`, D121).
fn pointless(n: &Node, pure: &Pure) -> bool {
    effect_free(n, pure)
}

/// Whether computing n can do nothing but make a value (FE24): names,
/// literals, lambdas, partial applications, calls of a builtin known to
/// have no effect (FE77), and operators, fields, indexes, slices, ranges,
/// tuples, lists, maps and tag values of only those. Other calls,
/// assignments, `?`, `return`, `assert`, definitions and blocks may do
/// something. A discarded `Err` computed at run time is still E0606.
fn effect_free(n: &Node, pure: &Pure) -> bool {
    let all = |n: &Node| n.kids.iter().all(|k| effect_free(k, pure));
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
        Head::Do => partial(n, pure),
        Head::Call => pure.contains_key(&(n.pos.line, n.pos.col)) && all(n),
        // `and` and `or` (an `if` over no blocks), and `then … else`.
        Head::If => n.kids[1].head != Head::Do && all(n),
        Head::Then => all(n),
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
        | Head::Tag => all(n),
        _ => false,
    }
}

/// A partial application (`desugar::partial_with`): hidden assignments
/// of the callee and arguments, then the lambda over the holes. One whose
/// callee or fixed arguments compute something (`h(g(), _)`) is not
/// effect-free.
fn partial(n: &Node, pure: &Pure) -> bool {
    partial_shape(n)
        && n.kids
            .iter()
            .rev()
            .skip(1)
            .all(|l| effect_free(&l.kids[1], pure))
}

/// The shape of a partial application, whatever its arguments compute.
pub(super) fn partial_shape(n: &Node) -> bool {
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
fn discarded(n: &Node, pure: &Pure) -> Diag {
    if let Some(e) = pure_call(n, pure) {
        return e;
    }
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
        Head::Nil => ("`nil`".to_string(), 3),
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
        // `nil` was how a branch said "nothing" (D121).
        Head::Nil => {
            "a branch with nothing to do is left empty: `_ =>` in a match, no `else` after an if"
                .to_string()
        }
        _ => return error("E0234", n.pos, vec![Arg::Str(what)]).span(len),
    };
    error("E0234", n.pos, vec![Arg::Str(what)])
        .span(len)
        .with_help(&help)
}

/// E0234 for a discarded call of a builtin known to have no effect (D39),
/// whatever its arguments compute; `None` for any other node.
fn pure_call(n: &Node, pure: &Pure) -> Option<Diag> {
    if n.head != Head::Call {
        return None;
    }
    let PureCall { shown, own, at } = pure.get(&(n.pos.line, n.pos.col))?;
    let help = if RUNS_A_FUNCTION.contains(&own.as_str()) {
        format!(
            "`{own}` computes a value with its function; use the value, or run the function for its effects with `each` or a `for` statement"
        )
    } else {
        format!(
            "`{own}` has no effect: it only computes its result (a changed copy, a converted value); use the result, or write `_ = …` to discard it on purpose"
        )
    };
    Some(
        error(
            "E0234",
            *at,
            vec![Arg::Str(format!("the result of `{shown}(…)`"))],
        )
        .span(rune_len(shown))
        .with_help(&help),
    )
}

/// E0234's help for a discarded `==`, and the same in a test body.
const EQ_HELP: &str = "did you mean `=`? `==` compares; `=` assigns";
const TEST_EQ_HELP: &str =
    "did you mean `assert` before it? a test checks a comparison with `assert a == b`";

/// A function the code writes without a name: a lambda or an anonymous
/// `fn` (not the function a `for` body becomes).
pub(super) fn is_lambda(n: &Node) -> bool {
    n.head == Head::Fn && fn_name(n).is_empty() && !for_body(n)
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

/// The type the syntax alone says `n`'s value has (D99, FE81), named as
/// the run names it (`type_name` in fung-vm): a literal of any kind (a
/// number, a string with or without `${…}`, a bool, `nil`, a symbol or
/// bare tag, a duration), a list, map or tuple literal, a tag value (its
/// tag's name), a range, and a function the code writes (a lambda, an
/// anonymous `fn`, a partial application). `None` when the syntax does
/// not say. Every check of what literals settle reads the type here; a
/// type annotation's check (D113) can compare it with the annotation's
/// type names, which are the same names.
pub(crate) fn known_type(n: &Node) -> Option<&str> {
    Some(match n.head {
        Head::Int => "int",
        Head::Float => "float",
        // a string with `${…}` is a concat in the core tree
        Head::String | Head::Interp | Head::Concat => "str",
        Head::Bool => "bool",
        Head::Nil => "nil",
        Head::Symbol => "sym",
        Head::Duration => "dur",
        Head::List => "list",
        Head::Map => "map",
        Head::Tuple => "tuple",
        Head::Range => "range",
        Head::Tag => lit_str(n),
        Head::Fn if fn_name(n).is_empty() && !for_body(n) => "fn",
        Head::Do if partial_shape(n) => "fn",
        _ => return None,
    })
}

/// Whether a value of type `t` is a result (`Ok(…)`, `Err(…)`).
fn is_result(t: &str) -> bool {
    matches!(t, "Ok" | "Err")
}

/// Whether the binary operator `op` has a meaning for values of the types
/// `a` and `b` (as `fung-vm`'s `value::arith` and `compare` decide), and
/// the types its error names, in the run's order: `None` when the values
/// decide (two lists or tuples compare item by item).
fn operator_types<'t>(op: &str, a: &'t str, b: &'t str) -> Option<Result<(), (&'t str, &'t str)>> {
    let num = |t: &str| t == "int" || t == "float";
    let both = |t: &str| a == t && b == t;
    let fine = match op {
        "+" => (num(a) && num(b)) || both("str") || both("dur") || both("list"),
        "-" => (num(a) && num(b)) || both("dur"),
        // `n * d` scales the duration: its error names the duration first
        "*" => {
            if b == "dur" && a != "dur" && !num(a) {
                return Some(Err(("dur", a)));
            }
            (num(a) && num(b)) || (a == "dur" && num(b)) || (num(a) && b == "dur")
        }
        "/" => (num(a) && num(b)) || both("dur") || (a == "dur" && num(b)),
        "//" | "%" => (num(a) && num(b)) || both("dur"),
        "**" => num(a) && num(b),
        "<" | ">" | "<=" | ">=" => {
            if both("list") || both("tuple") {
                return None;
            }
            (num(a) && num(b)) || both("str") || both("dur")
        }
        _ => return None,
    };
    Some(if fine { Ok(()) } else { Err((a, b)) })
}

/// What the types of the literals in `n` settle before the program runs
/// (D99, FE81): the error the run would stop with, under its number and
/// with its help. Only what holds whatever the values are: an overflow,
/// a division by zero or two lists that compare item by item are the
/// run's to find. The type comes from `known_type`, one place for every
/// check.
fn literal_type_error(n: &Node) -> Option<Diag> {
    let not_bool = |c: &Node| {
        known_type(c)
            .filter(|t| *t != "bool")
            .map(|t| error("E0505", c.pos, vec![Arg::Str(t.into())]))
    };
    match n.head {
        // a value that is no function called
        Head::Call => {
            let t = known_type(&n.kids[0]).filter(|t| *t != "fn")?;
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
        Head::Op if n.kids.len() == 1 && lit_str(n) == "-" => {
            let t = known_type(&n.kids[0])?;
            if matches!(t, "int" | "float" | "dur") {
                return None;
            }
            let e = error(
                "E0506",
                n.pos,
                vec![Arg::Str("negate".into()), Arg::Str(t.into())],
            );
            Some(run_help(e, &[t]))
        }
        Head::Op if n.kids.len() == 2 => {
            let (a, b) = (known_type(&n.kids[0])?, known_type(&n.kids[1])?);
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
            let (x, y) = operator_types(op, a, b)?.err()?;
            let e = error(
                "E0501",
                n.pos,
                vec![
                    Arg::Str(verb.into()),
                    Arg::Str(x.into()),
                    Arg::Str(y.into()),
                ],
            );
            // `+` of text and another value says how to join them, as
            // the run does (SP7).
            if op == "+" && (a == "str") != (b == "str") {
                return Some(e.with_help(
                    "put the value into the text with `\"${…}\"`, or convert it with `str(…)`",
                ));
            }
            Some(run_help(e, &[a, b]))
        }
        // `??` after a value that is never nil or a result
        Head::Coalesce => {
            let what = match known_type(&n.kids[0])? {
                "nil" | "Ok" | "Err" => return None,
                "int" | "float" => "a number",
                "str" => "a string",
                "bool" => "a bool",
                "sym" => "a symbol",
                "dur" => "a duration",
                "list" => "a list",
                "map" => "a map",
                "tuple" => "a tuple",
                "range" => "a range",
                "fn" => "a function",
                _ => "a tag value",
            };
            Some(error("E0243", n.pos, vec![Arg::Str(what.into())]).span(2))
        }
        _ => None,
    }
}

/// The run's help for an operator's type error on a result (VA39).
fn run_help(e: Diag, types: &[&str]) -> Diag {
    if types.iter().any(|t| is_result(t)) {
        e.with_help("unwrap it with `!` or `?` first")
    } else {
        e
    }
}
