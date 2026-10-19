//! The parser: builds the surface tree from tokens. Statements use
//! recursive descent; expressions use a Pratt loop (`expr.rs`). Newlines
//! are tokens, and the parser decides from context whether one ends a
//! statement (01. 줄 이어짐 규칙).
//!
//! Recursion is bounded by `MAX_DEPTH` so absurd input yields E0802
//! instead of a stack overflow. A thread's stack does not grow: callers
//! that may parse input nested close to the limit must run the parser on a thread with a large stack (the tests use
//! 512 MiB).

mod collections;
mod expr;
mod fn_;
mod pattern;
mod stmt;
#[cfg(test)]
mod tests;

use crate::diag::{Arg, Diag, error};
use crate::lexer::{Kind, Token};
use crate::source::Pos;
use crate::tree::{Head, Node};

/// Bounds expression and pattern nesting.
pub const MAX_DEPTH: usize = 10_000;

/// Errors are boxed inside the parser to keep the recursive frames small.
type PResult<T> = Result<T, Box<Diag>>;

struct Parser<'a> {
    toks: &'a [Token],
    i: usize,
    /// Depth of `(` `[` `{`: newlines are whitespace inside.
    nest: usize,
    /// Recursion depth of `expr` and `pat_term`, bounded by `MAX_DEPTH`.
    depth: usize,
    /// Index of the last token consumed by `next`, for spans.
    last: usize,
    /// Open bodies (fn, if, match arms…): test blocks are only allowed at
    /// depth 0 (E0114).
    block_depth: usize,
    /// Set while parsing a match arm's `when` condition: there a bare
    /// `name =>` is the arm's arrow, not a lambda (lambdas need parentheses
    /// around them, as in a call argument, where nest > 0).
    guard: bool,
}

/// Returns the program as `(program stmt...)`. Only the first syntax error
/// is reported (08). `toks` must end with an `Eof` token, as `lex` returns.
pub fn parse(toks: &[Token]) -> Result<Node, Diag> {
    Parser::new(toks, 0).program().map_err(|d| *d)
}

fn err(code: &str, pos: Pos, args: Vec<Arg>) -> Box<Diag> {
    Box::new(error(code, pos, args))
}

fn s(text: impl Into<String>) -> Arg {
    Arg::Str(text.into())
}

/// How a token reads in an error message.
fn describe(t: &Token) -> String {
    match t.kind {
        Kind::Eof | Kind::Newline => t.kind.describe().to_string(),
        Kind::Int
        | Kind::Float
        | Kind::Duration
        | Kind::String
        | Kind::RawString
        | Kind::Name
        | Kind::Tag => format!("{} `{}`", t.kind.describe(), t.text),
        Kind::Symbol => format!("symbol `'{}`", t.text),
        k => format!("`{}`", k.describe()),
    }
}

impl<'a> Parser<'a> {
    fn new(toks: &'a [Token], depth: usize) -> Self {
        Parser {
            toks,
            i: 0,
            nest: 0,
            depth,
            last: 0,
            block_depth: 0,
            guard: false,
        }
    }

    /// Whether `x => …` / `(x) => …` may start a lambda here (not at the top
    /// level of a match guard).
    fn bare_lambda_ok(&self) -> bool {
        !self.guard || self.nest > 0
    }

    /// Records n's byte range: from `start` to the end of the last consumed
    /// token (assert and dbg show source text, 08).
    fn span(&self, n: &mut Node, start: usize) {
        n.off = start;
        n.end = self.toks[self.last].end;
    }

    /// Parses a statement and records its span.
    fn spanned_statement(&mut self) -> PResult<Node> {
        let start = self.peek().off;
        let mut n = self.statement()?;
        self.span(&mut n, start);
        Ok(n)
    }

    /// The next significant token. Inside brackets, newlines are skipped
    /// here so every caller sees them as whitespace.
    fn peek(&mut self) -> &'a Token {
        let toks = self.toks;
        while self.nest > 0 && toks[self.i].kind == Kind::Newline {
            self.i += 1;
        }
        &toks[self.i]
    }

    fn next(&mut self) -> &'a Token {
        let t = self.peek();
        if t.kind != Kind::Eof {
            self.last = self.i;
            self.i += 1;
        }
        t
    }

    fn at(&mut self, k: Kind) -> bool {
        self.peek().kind == k
    }

    /// The raw token after the current one (newlines included).
    fn kind_after(&self) -> Kind {
        self.toks[self.i + 1].kind
    }

    /// The kind of token `j`, skipping newlines when inside brackets, where
    /// they are whitespace (01).
    fn kind_from(&self, mut j: usize) -> Kind {
        while self.nest > 0 && self.toks[j].kind == Kind::Newline {
            j += 1;
        }
        self.toks[j].kind
    }

    /// A token usable as a name where nothing else can appear (01): a name,
    /// a tag, or a keyword (`m.type`, `{match: 1}`).
    fn at_name_like(&mut self) -> bool {
        let k = self.peek().kind;
        k == Kind::Name || k == Kind::Tag || k.is_keyword()
    }

    /// E0110 for the current token.
    fn unexpected(&mut self) -> Box<Diag> {
        let t = self.peek();
        err("E0110", t.pos, vec![s(describe(t))])
    }

    /// `expect` with an E0112 "expected X, found Y" message.
    fn expect_desc(&mut self, k: Kind, what: &str) -> PResult<&'a Token> {
        if !self.at(k) {
            let t = self.peek();
            return Err(err("E0112", t.pos, vec![s(what), s(describe(t))]));
        }
        Ok(self.next())
    }

    fn skip_seps(&mut self) {
        while self.at(Kind::Newline) || self.at(Kind::Semi) {
            self.next();
        }
    }

    /// Implements "a line ending in an operator continues".
    fn skip_newlines_after_operator(&mut self) {
        while self.toks[self.i].kind == Kind::Newline {
            self.i += 1;
        }
    }

    /// program = body at nesting 0, terminated by EOF.
    fn program(&mut self) -> PResult<Node> {
        let mut prog = Node::new(Head::Program, self.peek().pos, Vec::new());
        self.skip_seps();
        while !self.at(Kind::Eof) {
            let stmt = self.spanned_statement()?;
            prog.kids.push(stmt);
            if !self.at(Kind::Eof) && !self.at(Kind::Newline) && !self.at(Kind::Semi) {
                return Err(self.unexpected());
            }
            self.skip_seps();
        }
        Ok(prog)
    }
}
