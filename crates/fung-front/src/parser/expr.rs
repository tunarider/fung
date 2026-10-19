//! Expressions: the Pratt loop, prefix forms, calls, indexing and string
//! interpolation.

use super::{MAX_DEPTH, PResult, Parser, describe, err, s};
use crate::diag::Arg;
use crate::lexer::{Kind, Token, lex_at};
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

/// Tokens that may start a continuation line (01).
fn continues_line(k: Kind) -> bool {
    matches!(
        k,
        Kind::Pipe
            | Kind::Dot
            | Kind::Coalesce
            | Kind::KwAnd
            | Kind::KwOr
            | Kind::KwThen
            | Kind::KwElse
    )
}

/// Whether a token ends a call argument.
fn is_arg_end(k: Kind) -> bool {
    matches!(k, Kind::Comma | Kind::RParen | Kind::Newline)
}

/// Whether an Int token is the lexer's special 9223372036854775808 token,
/// which only a preceding minus makes valid.
pub(super) fn is_min_int_magnitude(t: &Token) -> bool {
    t.int == i64::MIN && t.text.replace('_', "") == "9223372036854775808"
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
        if continues_line(toks[j].kind) {
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
            return Err(err("E0802", pos, vec![Arg::Int(MAX_DEPTH as i64)]));
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
        let lhs = self.prefix()?;
        self.infix_loop(lhs, start, min_bp)
    }

    /// Continues an expression whose first part, `lhs` (starting at byte
    /// `start`), is already parsed with a higher power: the result is what
    /// `expr(min_bp)` would have read from `start`.
    fn expr_from(&mut self, lhs: Node, start: usize, min_bp: u32) -> PResult<Node> {
        if self.depth >= MAX_DEPTH {
            let pos = self.peek().pos;
            return Err(err("E0802", pos, vec![Arg::Int(MAX_DEPTH as i64)]));
        }
        self.depth += 1;
        let r = self.infix_loop(lhs, start, min_bp);
        self.depth -= 1;
        r
    }

    /// The operators after a parsed operand, as long as they bind at least
    /// as tightly as `min_bp`.
    fn infix_loop(&mut self, mut lhs: Node, start: usize, min_bp: u32) -> PResult<Node> {
        let mut last_was_compare = false;
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
            let not_in = t.kind == Kind::KwNot && self.kind_after() == Kind::KwIn;
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
            self.next();
            if not_in {
                self.next();
            }
            self.skip_newlines_after_operator();
            let rhs = self.expr(info.right)?;
            lhs = match info.text {
                "|>" => Node::new(Head::Pipe, t.pos, vec![lhs, rhs]),
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
        }
    }

    /// `cond then a else b`, with `then` (t) not yet consumed.
    fn then_else(&mut self, cond: Node, t: &'a Token, start: usize) -> PResult<Node> {
        self.next();
        self.skip_newlines_after_operator();
        let a = self.expr(0)?;
        self.peek_infix(); // an `else` on the next line continues the expression
        self.expect_desc(Kind::KwElse, "`else` after then")?;
        self.skip_newlines_after_operator();
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
        let name = self.next();
        Ok(Node::with_lit(
            Head::Field,
            t.pos,
            Lit::Str(name.text.clone()),
            vec![x],
        ))
    }

    fn prefix(&mut self) -> PResult<Node> {
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
                self.fn_def(t, String::new())
            }
            Kind::KwReturn => self.return_expr(t),
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
                    return self.lambda();
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
                    return self.lambda();
                }
                self.paren_or_tuple()
            }
            _ => Err(self.unexpected()),
        }
    }

    /// `return` with an optional value; t is the `return` token.
    fn return_expr(&mut self, t: &'a Token) -> PResult<Node> {
        self.next();
        let value = match self.peek().kind {
            Kind::Newline
            | Kind::Semi
            | Kind::DoubleSemi
            | Kind::RParen
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
        let nt = &self.toks[self.i + 1];
        if nt.kind == Kind::Int && is_min_int_magnitude(nt) {
            self.next();
            self.next();
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
    /// application, 03), then named arguments. Inside the parentheses
    /// newlines are whitespace and a trailing comma is allowed.
    pub(super) fn call_args(&mut self, node: Node) -> PResult<Node> {
        self.next(); // (
        self.nest += 1;
        let r = self.call_args_inner(node);
        self.nest -= 1;
        r
    }

    fn call_args_inner(&mut self, mut node: Node) -> PResult<Node> {
        let mut saw_named = false;
        while !self.at(Kind::RParen) {
            if self.at(Kind::Name) && self.kind_after() == Kind::Colon {
                // Named argument: positional ones must come first (03).
                let name = self.next();
                self.next();
                if self.at_hole() {
                    let pos = self.peek().pos;
                    return Err(err("E0110", pos, vec![s("`_` in a named argument")]));
                }
                let e = self.expr(0)?;
                node.kids.push(Node::with_lit(
                    Head::Named,
                    name.pos,
                    Lit::Str(name.text.clone()),
                    vec![e],
                ));
                saw_named = true;
            } else if self.at_hole() {
                if saw_named {
                    return Err(self.unexpected());
                }
                let pos = self.next().pos;
                node.kids.push(Node::new(Head::Hole, pos, Vec::new()));
            } else {
                if saw_named {
                    return Err(self.unexpected());
                }
                let arg = self.expr(0)?;
                node.kids.push(arg);
            }
            if self.at(Kind::Comma) {
                self.next();
                continue;
            }
            if !self.at(Kind::RParen) {
                return Err(self.unexpected());
            }
        }
        self.next(); // )
        Ok(node)
    }

    /// A `_` that is a whole call argument.
    fn at_hole(&mut self) -> bool {
        self.at(Kind::Name) && self.peek().text == "_" && is_arg_end(self.kind_after())
    }

    /// Turns a "..." token into a string leaf or an (interp ...) node whose
    /// code parts are parsed with positions in the enclosing file.
    fn string_literal(&mut self, t: &'a Token) -> PResult<Node> {
        if !t.parts.iter().any(|p| p.is_code) {
            let text: String = t.parts.iter().map(|p| p.text.as_str()).collect();
            return Ok(Node::leaf(Head::String, t.pos, Lit::Str(text)));
        }
        let mut n = Node::new(Head::Interp, t.pos, Vec::new());
        for part in &t.parts {
            if !part.is_code {
                n.kids
                    .push(Node::leaf(Head::String, t.pos, Lit::Str(part.text.clone())));
                continue;
            }
            let (toks, _) =
                lex_at(part.pos.file, &part.text, part.pos, part.off).map_err(Box::new)?;
            let mut sub = Parser::new(&toks, self.depth);
            let e = sub.expr(0)?;
            sub.skip_seps();
            if !sub.at(Kind::Eof) {
                return Err(sub.unexpected());
            }
            n.kids.push(e);
        }
        Ok(n)
    }

    /// Parses `[e]`, `[a..b]`, `[a..=b]`, `[..b]`, `[a..]` after an
    /// expression (01). The bracket holds a full expression; a range at its
    /// top level means a slice, so `xs[i ?? 0]` indexes and `xs[1..=3]`
    /// slices.
    fn index_or_slice(&mut self, x: Node) -> PResult<Node> {
        let open = self.next(); // [
        self.nest += 1;
        let r = self.index_or_slice_inner(x, open);
        self.nest -= 1;
        r
    }

    fn index_or_slice_inner(&mut self, x: Node, open: &'a Token) -> PResult<Node> {
        let nil = || Node::new(Head::Nil, open.pos, Vec::new());
        let slice = |lit: Lit, kids: Vec<Node>| Node::with_lit(Head::Slice, open.pos, lit, kids);
        if self.at(Kind::DotDot) {
            // [..b] and [..]
            self.next();
            if self.at(Kind::RBracket) {
                self.next();
                return Ok(slice(Lit::None, vec![x, nil(), nil()]));
            }
            let end = self.expr(0)?;
            self.expect_desc(Kind::RBracket, "`]`")?;
            return Ok(slice(Lit::None, vec![x, nil(), end]));
        }
        if self.at(Kind::DotDotEq) {
            // [..=b]
            self.next();
            let end = self.expr(0)?;
            self.expect_desc(Kind::RBracket, "`]`")?;
            return Ok(slice(Lit::Str("..=".into()), vec![x, nil(), end]));
        }
        // [a..] has no expression after `..`, so the general expression
        // parser cannot read it. Read the part a range would start with,
        // check for `..]`, and otherwise go on from that part: reading the
        // contents twice made nested indexing exponential.
        let off = self.peek().off;
        let first = self.expr(BP_RANGE + 1)?;
        if self.at(Kind::DotDot) && self.kind_after() == Kind::RBracket {
            self.next();
            self.next();
            return Ok(slice(Lit::None, vec![x, first, nil()]));
        }
        let mut e = self.expr_from(first, off, 0)?;
        self.expect_desc(Kind::RBracket, "`]`")?;
        if e.head == Head::Range {
            let inclusive = matches!(&e.lit, Lit::Str(op) if op == "..=");
            let b = e.kids.pop().expect("range has two operands");
            let a = e.kids.pop().expect("range has two operands");
            let lit = if inclusive {
                Lit::Str("..=".into())
            } else {
                Lit::None
            };
            return Ok(slice(lit, vec![x, a, b]));
        }
        Ok(Node::new(Head::Index, open.pos, vec![x, e]))
    }
}
