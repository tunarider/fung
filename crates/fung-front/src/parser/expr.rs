//! Expressions: the Pratt loop, prefix forms, calls, indexing and string
//! interpolation.

use std::collections::HashSet;

use super::{MAX_DEPTH, PResult, Parser, describe, err, s, str_lit};
use crate::diag::Arg;
use crate::lexer::{Kind, Token};
use crate::source::Pos;
use crate::tree::{Head, Lit, Node};

// Binding powers from 07. 우선순위 (낮음 → 높음), scaled by 10 so that
// right-associative and non-associative operators can be expressed by giving
// the right side a different power than the left.
const BP_THEN: u32 = 10;
const BP_OR: u32 = 20;
const BP_AND: u32 = 30;
const BP_NOT: u32 = 40;
const BP_COMPARE: u32 = 50;
const BP_PIPE: u32 = 60;
const BP_COALESCE: u32 = 70;
pub(super) const BP_RANGE: u32 = 80;
const BP_ADD: u32 = 90;
const BP_MUL: u32 = 100;
const BP_NEG: u32 = 110;
const BP_POW: u32 = 120;
const BP_POSTFIX: u32 = 130;

struct Infix {
    left: u32,
    right: u32,
    text: &'static str,
    compare: bool,
}

/// The binary operators and their binding powers.
fn infix(k: Kind) -> Option<Infix> {
    let (left, right, text, compare) = match k {
        Kind::KwOr => (BP_OR, BP_OR + 1, "or", false),
        Kind::KwAnd => (BP_AND, BP_AND + 1, "and", false),
        Kind::Eq => (BP_COMPARE, BP_COMPARE + 1, "==", true),
        Kind::Ne => (BP_COMPARE, BP_COMPARE + 1, "!=", true),
        Kind::Lt => (BP_COMPARE, BP_COMPARE + 1, "<", true),
        Kind::Gt => (BP_COMPARE, BP_COMPARE + 1, ">", true),
        Kind::Le => (BP_COMPARE, BP_COMPARE + 1, "<=", true),
        Kind::Ge => (BP_COMPARE, BP_COMPARE + 1, ">=", true),
        Kind::Plus => (BP_ADD, BP_ADD + 1, "+", false),
        Kind::Minus => (BP_ADD, BP_ADD + 1, "-", false),
        Kind::Star => (BP_MUL, BP_MUL + 1, "*", false),
        Kind::Slash => (BP_MUL, BP_MUL + 1, "/", false),
        Kind::SlashSlash => (BP_MUL, BP_MUL + 1, "//", false),
        Kind::Percent => (BP_MUL, BP_MUL + 1, "%", false),
        // Right-associative: the same power on the right.
        Kind::StarStar => (BP_POW, BP_POW, "**", false),
        Kind::Pipe => (BP_PIPE, BP_PIPE + 1, "|>", false),
        Kind::Coalesce => (BP_COALESCE, BP_COALESCE + 1, "??", false),
        Kind::DotDot => (BP_RANGE, BP_RANGE + 1, "..", false),
        Kind::DotDotEq => (BP_RANGE, BP_RANGE + 1, "..=", false),
        Kind::KwIn => (BP_COMPARE, BP_COMPARE + 1, "in", true),
        _ => return None,
    };
    Some(Infix {
        left,
        right,
        text,
        compare,
    })
}

/// Whether a token ends a call argument.
fn is_arg_end(k: Kind) -> bool {
    matches!(k, Kind::Comma | Kind::RParen | Kind::Newline)
}

/// Whether an Int token is the lexer's special magnitude of the least int
/// (9223372036854775808, `0x8000000000000000`, …), which only a
/// preceding minus makes valid.
pub(super) fn is_min_int_magnitude(t: &Token) -> bool {
    // The lexer emits no other negative int, in any base (FE31).
    t.int == i64::MIN
}

/// The operator at the top of a slice's end that binds looser than `..`
/// (`??`, `|>`, a comparison, `in`, `not`, `and`/`or`, `then … else`), as
/// the error names it. A slice's end is the operand layer of `..`, as
/// everywhere else: reading a looser operator as the end would change
/// the meaning silently (FE27, D36).
fn looser_than_a_slice(n: &Node) -> Option<String> {
    if n.paren {
        return None;
    }
    match (n.head, &n.lit) {
        (Head::Then, _) => Some("then … else".into()),
        (Head::Coalesce, _) => Some("??".into()),
        (Head::Pipe, _) => Some("|>".into()),
        (Head::Unary, Lit::Str(op)) if op == "not" => Some("not".into()),
        (Head::Binary, Lit::Str(op)) => match op.as_str() {
            "==" | "!=" | "<" | ">" | "<=" | ">=" | "in" | "and" | "or" => Some(op.clone()),
            "not_in" => Some("not in".into()),
            _ => None,
        },
        _ => None,
    }
}

impl<'a> Parser<'a> {
    /// Looks past a newline when the next line starts with a continuation
    /// token, without consuming anything otherwise.
    fn peek_infix(&mut self) -> &'a Token {
        let t = self.peek();
        if t.kind != Kind::Newline {
            return t;
        }
        let toks = self.toks;
        let mut j = self.i;
        while toks[j].kind == Kind::Newline {
            j += 1;
        }
        if toks[j].kind.starts_continuation() {
            self.i = j;
            return &toks[j];
        }
        t
    }

    /// Parses an expression whose operators bind at least as tightly as
    /// `min_bp`.
    pub(super) fn expr(&mut self, min_bp: u32) -> PResult<Node> {
        if self.depth >= MAX_DEPTH {
            let pos = self.peek().pos;
            return Err(err("E0115", pos, vec![Arg::Int(MAX_DEPTH as i64)]));
        }
        self.depth += 1;
        let r = self.expr_loop(min_bp);
        self.depth -= 1;
        r
    }

    fn expr_loop(&mut self, min_bp: u32) -> PResult<Node> {
        let start = self.peek().off;
        // `not` binds looser than comparison (01 EBNF), so it cannot be the
        // operand of a tighter operator: `1 == not 2 == 3` would hide a
        // comparison chain.
        if min_bp > BP_NOT && self.at(Kind::KwNot) {
            let mut e = self.unexpected();
            e.help = Some("put it in parentheses: `(not x)`".into());
            return Err(e);
        }
        let lhs = self.prefix(min_bp)?;
        self.infix_loop(lhs, start, min_bp)
    }

    /// The operators after a parsed operand, as long as they bind at least
    /// as tightly as `min_bp`.
    fn infix_loop(&mut self, mut lhs: Node, start: usize, min_bp: u32) -> PResult<Node> {
        let mut last_was_compare = false;
        // `..` is not associative (01 EBNF): `1..2..3` and `xs[0..1..]`
        // are refused rather than failing at run time (FE8).
        let mut last_was_range = false;
        loop {
            self.span(&mut lhs, start);
            let t = self.peek_infix();
            // then … else: the lowest operator; both branches are full
            // expressions.
            if t.kind == Kind::KwThen && BP_THEN >= min_bp {
                return self.then_else(lhs, t, start);
            }
            // Postfix forms bind tightest: call and index only on the same
            // line, `.` also as a continuation (01), `!` and `?` right after.
            if BP_POSTFIX >= min_bp
                && matches!(
                    t.kind,
                    Kind::LParen | Kind::LBracket | Kind::Dot | Kind::Bang | Kind::Question
                )
            {
                lhs = match t.kind {
                    Kind::LParen => self.call(lhs)?,
                    Kind::LBracket => self.index_or_slice(lhs)?,
                    Kind::Dot => self.field(lhs, t)?,
                    _ => {
                        self.next();
                        let op = Lit::Str(t.kind.describe().to_string());
                        Node::with_lit(Head::Unwrap, t.pos, op, vec![lhs])
                    }
                };
                continue;
            }
            // A `..` directly inside slice brackets is the slice's, not a
            // range operator: the whole expression before it is the start
            // (`xs[a ?? 0..b]`, FE27). No range can stand before it at this
            // level: its own `..` would have stopped here first.
            if matches!(t.kind, Kind::DotDot | Kind::DotDotEq) && self.slice == Some(self.nest) {
                return Ok(lhs);
            }
            let not_in = t.kind == Kind::KwNot && self.kind_from(self.i + 1) == Kind::KwIn;
            let info = if not_in {
                Some(Infix {
                    left: BP_COMPARE,
                    right: BP_COMPARE + 1,
                    text: "not_in",
                    compare: true,
                })
            } else {
                infix(t.kind)
            };
            let Some(info) = info.filter(|i| i.left >= min_bp) else {
                return Ok(lhs);
            };
            if info.compare && last_was_compare {
                return Err(err("E0111", t.pos, Vec::new()));
            }
            let is_range = matches!(t.kind, Kind::DotDot | Kind::DotDotEq);
            if is_range && last_was_range {
                return Err(err("E0118", t.pos, Vec::new()));
            }
            self.next();
            if not_in {
                self.next();
            }
            self.skip_newlines_after_operator();
            let rhs = self.expr(info.right)?;
            lhs = match info.text {
                "|>" => {
                    // `x |> f ?? d` and `a ?? b |> f` each have two
                    // natural readings: the code says which (D35).
                    let loose = |k: &Node| k.head == Head::Coalesce && !k.paren;
                    if loose(&lhs) || loose(&rhs) {
                        // the help shows this expression's two readings
                        let help = if loose(&rhs) {
                            "say which one goes first: `(x |> f) ?? d` or `x |> (f ?? d)`"
                        } else {
                            "say which one goes first: `(a ?? b) |> f` or `a ?? (b |> f)`"
                        };
                        let c = if loose(&rhs) { &rhs } else { &lhs };
                        return Err(Box::new(
                            crate::diag::error("E0127", c.pos, Vec::new())
                                .span(2)
                                .with_help(help),
                        ));
                    }
                    Node::new(Head::Pipe, t.pos, vec![lhs, rhs])
                }
                "??" => Node::new(Head::Coalesce, t.pos, vec![lhs, rhs]),
                ".." | "..=" => Node::with_lit(
                    Head::Range,
                    t.pos,
                    Lit::Str(info.text.into()),
                    vec![lhs, rhs],
                ),
                _ => Node::with_lit(
                    Head::Binary,
                    t.pos,
                    Lit::Str(info.text.into()),
                    vec![lhs, rhs],
                ),
            };
            last_was_compare = info.compare;
            last_was_range = is_range;
        }
    }

    /// `cond then a else b`, with `then` (t) not yet consumed.
    fn then_else(&mut self, cond: Node, t: &'a Token, start: usize) -> PResult<Node> {
        self.next();
        self.skip_newlines_after_operator();
        // `else` ends this branch, so a `..` in it is a range even inside
        // slice brackets (FE27). Each branch may be a `return` (D50).
        self.return_ok = true;
        let a = self.outside_slice(|p| p.expr(0))?;
        self.peek_infix(); // an `else` on the next line continues the expression
        self.expect_desc(Kind::KwElse, "`else` after then")?;
        self.skip_newlines_after_operator();
        self.return_ok = true;
        let b = self.expr(0)?;
        let mut n = Node::new(Head::Then, t.pos, vec![cond, a, b]);
        self.span(&mut n, start);
        Ok(n)
    }

    /// `.name` after an expression, with the dot (t) not yet consumed.
    fn field(&mut self, x: Node, t: &'a Token) -> PResult<Node> {
        self.next();
        self.skip_newlines_after_operator();
        if !self.at_name_like() {
            let p = self.peek();
            return Err(err(
                "E0112",
                p.pos,
                vec![s("a field name after `.`"), s(describe(p))],
            ));
        }
        // A key as written (01); a module's member is found in NFC.
        let name = self.next();
        Ok(Node::with_lit(
            Head::Field,
            t.pos,
            Lit::Str(name.key().to_string()),
            vec![x],
        ))
    }

    /// An operand: a literal, a name, a bracketed or block form, or a
    /// prefix operator with its operand. `min_bp` is the binding power of
    /// the operator it belongs to, 0 when it is a whole expression.
    fn prefix(&mut self, min_bp: u32) -> PResult<Node> {
        let return_ok = std::mem::take(&mut self.return_ok);
        let t = self.peek();
        let leaf = |head, lit| Ok(Node::leaf(head, t.pos, lit));
        match t.kind {
            Kind::Int => {
                if is_min_int_magnitude(t) {
                    return Err(err("E0102", t.pos, vec![s(t.text.clone())]));
                }
                self.next();
                leaf(Head::Int, Lit::Int(t.int))
            }
            Kind::KwNil => {
                self.next();
                Ok(Node::new(Head::Nil, t.pos, Vec::new()))
            }
            Kind::String => {
                self.next();
                self.string_literal(t)
            }
            Kind::RawString => {
                self.next();
                leaf(Head::String, Lit::Str(t.text.clone()))
            }
            Kind::Symbol => {
                self.next();
                leaf(Head::Symbol, Lit::Str(t.text.clone()))
            }
            Kind::Duration => {
                self.next();
                leaf(Head::Duration, Lit::Dur(t.dur))
            }
            Kind::KwIf => self.if_expr(),
            Kind::KwMatch => self.match_expr(),
            Kind::KwFor => self.for_expr(),
            Kind::LBracket => self.list_literal(),
            Kind::LBrace => self.map_literal(),
            Kind::Tag => self.tag_expr(),
            Kind::KwFn => {
                self.next();
                self.fn_def(t, String::new(), t.pos)
            }
            Kind::KwReturn if return_ok => self.return_expr(t),
            Kind::KwReturn => Err(Box::new(
                crate::diag::error("E0132", t.pos, Vec::new()).span(6),
            )),
            Kind::Float => {
                self.next();
                leaf(Head::Float, Lit::Float(t.float))
            }
            Kind::KwTrue => {
                self.next();
                leaf(Head::Bool, Lit::Bool(true))
            }
            Kind::KwFalse => {
                self.next();
                leaf(Head::Bool, Lit::Bool(false))
            }
            Kind::Name => {
                if self.kind_from(self.i + 1) == Kind::FatArrow && self.bare_lambda_ok() {
                    return self.lambda_here(min_bp);
                }
                self.next();
                leaf(Head::Name, Lit::Str(t.text.clone()))
            }
            Kind::Minus => self.negation(t),
            Kind::KwNot => {
                self.next();
                self.skip_newlines_after_operator();
                let operand = self.expr(BP_NOT)?;
                Ok(Node::with_lit(
                    Head::Unary,
                    t.pos,
                    Lit::Str("not".into()),
                    vec![operand],
                ))
            }
            Kind::LParen => {
                if self.lambda_params_ahead() && self.bare_lambda_ok() {
                    return self.lambda_here(min_bp);
                }
                self.paren_or_tuple()
            }
            _ => Err(self.unexpected()),
        }
    }

    /// A lambda where an operand starts: only a whole expression may be
    /// one, since its body reads as far as it can (FE41).
    fn lambda_here(&mut self, min_bp: u32) -> PResult<Node> {
        if min_bp > 0 {
            let t = self.peek();
            let mut e = err("E0110", t.pos, vec![s("lambda")]);
            e.help = Some("put the lambda in parentheses: `(x => …)`".into());
            return Err(e);
        }
        self.lambda()
    }

    /// `return` with an optional value; t is the `return` token. Without
    /// a value it is followed by what ends a statement or a branch (FE51).
    fn return_expr(&mut self, t: &'a Token) -> PResult<Node> {
        self.next();
        let value = match self.peek().kind {
            Kind::Newline
            | Kind::Semi
            | Kind::DoubleSemi
            | Kind::RParen
            | Kind::RBracket
            | Kind::RBrace
            | Kind::KwElse
            | Kind::Eof
            | Kind::Comma => Node::new(Head::Nil, t.pos, Vec::new()),
            _ => self.expr(0)?,
        };
        Ok(Node::new(Head::Return, t.pos, vec![value]))
    }

    /// Unary minus; t is the `-` token.
    fn negation(&mut self, t: &'a Token) -> PResult<Node> {
        // -9223372036854775808: the lexer cannot know a minus precedes the
        // magnitude, so the parser folds exactly this case (01).
        // It folds only as the minus's whole operand: `-a ** b` is
        // `-(a ** b)` and a postfix binds to the magnitude, which is out
        // of range there (FE9).
        // a line ending in the minus continues (FE61)
        let mut j = self.i + 1;
        while self.toks[j].kind == Kind::Newline {
            j += 1;
        }
        let nt = &self.toks[j];
        if nt.kind == Kind::Int && is_min_int_magnitude(nt) {
            self.next();
            self.skip_newlines_after_operator();
            self.next();
            let after = self.peek_infix().kind;
            if matches!(
                after,
                Kind::StarStar
                    | Kind::Dot
                    | Kind::LBracket
                    | Kind::LParen
                    | Kind::Bang
                    | Kind::Question
            ) {
                let mut e = Box::new(
                    crate::diag::error("E0135", nt.pos, vec![s(nt.text.clone())])
                        .span(nt.text.len()),
                );
                e.help = Some(format!(
                    "{}; here the minus applies to what follows the number, so put the negative number in parentheses: `(-{})`",
                    crate::lexer::INT_RANGE,
                    nt.text
                ));
                return Err(e);
            }
            return Ok(Node::leaf(Head::Int, t.pos, Lit::Int(i64::MIN)));
        }
        self.next();
        self.skip_newlines_after_operator();
        let operand = self.expr(BP_NEG)?;
        Ok(Node::with_lit(
            Head::Unary,
            t.pos,
            Lit::Str("-".into()),
            vec![operand],
        ))
    }

    /// `(args)` after a callee.
    fn call(&mut self, callee: Node) -> PResult<Node> {
        let open = self.peek();
        self.call_args(Node::new(Head::Call, open.pos, vec![callee]))
    }

    /// Parses `(args)` into node's kids: positional, `_` holes (partial
    /// application, 03), a spread `...xs` as the last positional (D45),
    /// then named arguments. Inside the parentheses newlines are
    /// whitespace and a trailing comma is allowed.
    pub(super) fn call_args(&mut self, node: Node) -> PResult<Node> {
        let open = self.next().pos; // (
        self.nest += 1;
        let r = self.call_args_inner(node, open);
        self.nest -= 1;
        r
    }

    fn call_args_inner(&mut self, mut node: Node, open: Pos) -> PResult<Node> {
        let mut saw_named = false;
        let mut saw_spread = false;
        let mut names: HashSet<&str> = HashSet::new();
        while !self.at(Kind::RParen) {
            if self.at(Kind::Name) && self.kind_from(self.i + 1) == Kind::Colon {
                // Named argument: positional ones must come first (03).
                let name = self.next();
                if !names.insert(&name.text) {
                    return Err(err("E0116", name.pos, vec![s(&name.text)]));
                }
                self.next();
                if self.at_hole() {
                    let pos = self.peek().pos;
                    return Err(err("E0123", pos, Vec::new()));
                }
                let e = self.expr(0)?;
                node.kids.push(Node::with_lit(
                    Head::Named,
                    name.pos,
                    Lit::Str(name.text.clone()),
                    vec![e],
                ));
                saw_named = true;
            } else if saw_named {
                // Positional arguments come first (03).
                let pos = self.peek().pos;
                return Err(err("E0126", pos, Vec::new()));
            } else if saw_spread {
                // A spread is the last positional argument (D45).
                let pos = self.peek().pos;
                return Err(err(
                    "E0133",
                    pos,
                    vec![s("before another positional argument")],
                ));
            } else if self.at(Kind::Ellipsis) {
                let pos = self.next().pos;
                let e = self.expr(0)?;
                node.kids.push(Node::new(Head::Spread, pos, vec![e]));
                saw_spread = true;
            } else if self.at_hole() {
                let pos = self.next().pos;
                node.kids.push(Node::new(Head::Hole, pos, Vec::new()));
            } else {
                let arg = self.expr(0)?;
                node.kids.push(arg);
            }
            if self.at(Kind::Comma) {
                self.next();
                continue;
            }
            if !self.at(Kind::RParen) {
                return Err(self.list_end(open, Kind::RParen));
            }
        }
        self.next(); // )
        Ok(node)
    }

    /// A `_` that is a whole call argument.
    fn at_hole(&mut self) -> bool {
        self.at(Kind::Name) && self.peek().text == "_" && is_arg_end(self.kind_from(self.i + 1))
    }

    /// Turns a "..." token into a string leaf or an (interp ...) node whose
    /// code parts are parsed with positions in the enclosing file.
    fn string_literal(&mut self, t: &'a Token) -> PResult<Node> {
        if let Some(text) = t.plain() {
            return Ok(Node::leaf(Head::String, t.pos, str_lit(text)));
        }
        let mut n = Node::new(Head::Interp, t.pos, Vec::new());
        for part in &t.parts {
            if !part.is_code {
                let lit = match &part.bytes {
                    Some(b) => Lit::Bytes(b.clone()),
                    None => Lit::Str(part.text.clone()),
                };
                n.kids.push(Node::leaf(Head::String, t.pos, lit));
                continue;
            }
            // the lexer read the code with the string (FE16)
            let mut sub = Parser::new(&part.toks, self.depth);
            if sub.at(Kind::Eof) {
                let t = sub.peek();
                return Err(err(
                    "E0112",
                    t.pos,
                    vec![s("an expression inside `${…}`"), s(describe(t))],
                ));
            }
            let e = sub.expr(0)?;
            if !sub.at(Kind::Eof) {
                return Err(sub.unexpected());
            }
            n.kids.push(e);
        }
        Ok(n)
    }

    /// Parses `[e]`, `[a..b]`, `[a..=b]`, `[..b]`, `[a..]` after an
    /// expression (01). A `..` or `..=` at the top level of the brackets
    /// makes a slice, and each end is a full expression, so `xs[i ?? 0]`
    /// indexes and `xs[a ?? 0..b ?? 2]` slices; a range in parentheses or
    /// deeper is a range value used as an index (FE27).
    fn index_or_slice(&mut self, x: Node) -> PResult<Node> {
        let open = self.next(); // [
        self.nest += 1;
        let saved = self.slice.replace(self.nest);
        let r = self.index_or_slice_inner(x, open);
        self.slice = saved;
        self.nest -= 1;
        r
    }

    fn index_or_slice_inner(&mut self, x: Node, open: &'a Token) -> PResult<Node> {
        // A missing end is its own mark: a written `nil` is a value, and
        // slicing with it is E0501 like any nil bound (02, VA3).
        let missing = || Node::new(Head::Missing, open.pos, Vec::new());
        let start = if matches!(self.peek().kind, Kind::DotDot | Kind::DotDotEq) {
            missing()
        } else {
            let e = self.expr(0)?;
            if !matches!(self.peek().kind, Kind::DotDot | Kind::DotDotEq) {
                self.expect_desc(Kind::RBracket, "`]`")?;
                return Ok(Node::new(Head::Index, open.pos, vec![x, e]));
            }
            e
        };
        let inclusive = self.next().kind == Kind::DotDotEq;
        // [a..] and [..]: an open end; `..=` needs one.
        let end = if !inclusive && self.at(Kind::RBracket) {
            missing()
        } else {
            self.expr(0)?
        };
        let t = self.peek();
        if matches!(t.kind, Kind::DotDot | Kind::DotDotEq) {
            return Err(err("E0118", t.pos, Vec::new()));
        }
        self.expect_desc(Kind::RBracket, "`]`")?;
        for bound in [&start, &end] {
            if let Some(op) = looser_than_a_slice(bound) {
                return Err(err("E0125", bound.pos, vec![s(op)]));
            }
        }
        let lit = if inclusive {
            Lit::Str("..=".into())
        } else {
            Lit::None
        };
        Ok(Node::with_lit(
            Head::Slice,
            open.pos,
            lit,
            vec![x, start, end],
        ))
    }
}
