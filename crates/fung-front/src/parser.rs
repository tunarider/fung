//! The parser: builds the surface tree from tokens. Statements use
//! recursive descent; expressions use a Pratt loop (`expr.rs`). Newlines
//! are tokens, and the parser decides from context whether one ends a
//! statement (01. 줄 이어짐 규칙).
//!
//! Recursion is bounded by `MAX_DEPTH` so absurd input yields E0115
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
use crate::tree::{Head, Lit, Node};

/// Bounds expression and pattern nesting.
pub const MAX_DEPTH: usize = 10_000;

/// Errors are boxed inside the parser to keep the recursive frames small.
type PResult<T> = Result<T, Box<Diag>>;

struct Parser<'a> {
    toks: &'a [Token],
    /// For each opening bracket token, the index of the closer that
    /// brings the depth back (any kind, as `lambda_params_ahead` counts);
    /// `usize::MAX` when there is none. Built once, so looking ahead for
    /// `( … ) =>` costs O(1) instead of a scan per `(` (FE16).
    pairs: Vec<usize>,
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
    /// The `nest` of the slice brackets being read, `xs[ … ]`: a `..` at
    /// exactly that nesting is the slice's, so it ends the expression
    /// before it (FE27). Blocks clear it; a `..` deeper is a range.
    slice: Option<usize>,
    /// Set right before an expression whose start may be a `return`: a
    /// statement, a branch's end, a function's `=` body (D50). The next
    /// operand takes it, so a `return` anywhere else is E0132.
    return_ok: bool,
    /// The index of the first token of the statement begun last: a word
    /// of another language there is named at an error (FE43).
    stmt_start: usize,
}

/// Returns the program as `(program stmt...)`. Only the first syntax error
/// is reported (08). `toks` must end with an `Eof` token, as `lex` returns.
pub fn parse(toks: &[Token]) -> Result<Node, Diag> {
    Parser::new(toks, 0).program().map_err(|d| *d)
}

fn err(code: &str, pos: Pos, args: Vec<Arg>) -> Box<Diag> {
    Box::new(error(code, pos, args))
}

/// E0225: an uppercase name where a variable is bound or read by a
/// shorthand (01: 대문자로 시작하는 이름은 태그 전용).
fn tag_as_name(t: &Token) -> Box<Diag> {
    Box::new(error("E0225", t.pos, vec![s(t.text.clone())]).span(t.text.chars().count()))
}

/// A string literal's payload: text, or bytes that are not UTF-8 (L4).
fn str_lit(v: Result<String, Vec<u8>>) -> Lit {
    v.map_or_else(Lit::Bytes, Lit::Str)
}

fn s(text: impl Into<String>) -> Arg {
    Arg::Str(text.into())
}

/// How a token reads in an error message. The end of a `${…}`'s code is
/// its closing brace (the lexer gives that end the text `}`).
fn describe(t: &Token) -> String {
    match t.kind {
        Kind::Eof if t.text == "}" => "`}`".to_string(),
        Kind::Eof | Kind::Newline => t.kind.describe().to_string(),
        Kind::Int
        | Kind::Float
        | Kind::Duration
        | Kind::String
        | Kind::RawString
        | Kind::Name
        | Kind::Tag => format!("{} `{}`", t.kind.describe(), headline_text(&t.text)),
        Kind::Symbol => format!("symbol `'{}`", t.text),
        k => format!("`{}`", k.describe()),
    }
}

/// How many characters an error underlines at a token (FE61): its text,
/// or one for a string, a newline and the end.
fn tok_len(t: &Token) -> usize {
    match t.kind {
        Kind::Eof | Kind::Newline | Kind::String | Kind::RawString => 1,
        Kind::Symbol => 1 + t.text.chars().count(),
        _ => t.raw.as_deref().unwrap_or(&t.text).chars().count().max(1),
    }
}

/// E0113 at a map key written again, with where it was first (FE61).
fn duplicate_key(t: &Token, key: String, first: Pos) -> Box<Diag> {
    let mut d = error("E0113", t.pos, vec![s(key.clone())]).span(tok_len(t));
    d.notes.push(format!(
        "note: the key `{key}` is first written at {}:{}",
        first.line, first.col
    ));
    Box::new(d)
}

/// How many characters of a token's text a headline shows (FE42).
const SHOWN_TOKEN: usize = 40;

/// A token's text for a one-line headline: its first line, cut to
/// `SHOWN_TOKEN` characters, `…` where it was cut (FE42).
fn headline_text(text: &str) -> String {
    let line = text.split('\n').next().unwrap_or(text);
    match line.char_indices().nth(SHOWN_TOKEN) {
        Some((end, _)) => format!("{}\u{2026}", &line[..end]),
        None if line.len() < text.len() => format!("{line}\u{2026}"),
        None => line.to_string(),
    }
}

/// An arithmetic operator that other languages join with `=` (`+=`).
fn is_arith(k: Kind) -> bool {
    matches!(
        k,
        Kind::Plus
            | Kind::Minus
            | Kind::Star
            | Kind::StarStar
            | Kind::Slash
            | Kind::SlashSlash
            | Kind::Percent
    )
}

/// The closer of every opening bracket (`Parser::pairs`), with one stack
/// pass. A closer with nothing open is skipped.
fn bracket_pairs(toks: &[Token]) -> Vec<usize> {
    let mut pairs = vec![usize::MAX; toks.len()];
    let mut open = Vec::new();
    for (j, t) in toks.iter().enumerate() {
        match t.kind {
            Kind::LParen | Kind::LBracket | Kind::LBrace => open.push(j),
            Kind::RParen | Kind::RBracket | Kind::RBrace => {
                if let Some(o) = open.pop() {
                    pairs[o] = j;
                }
            }
            _ => {}
        }
    }
    pairs
}

impl<'a> Parser<'a> {
    fn new(toks: &'a [Token], depth: usize) -> Self {
        Parser {
            toks,
            pairs: bracket_pairs(toks),
            i: 0,
            nest: 0,
            depth,
            last: 0,
            block_depth: 0,
            guard: false,
            slice: None,
            return_ok: false,
            stmt_start: 0,
        }
    }

    /// Runs `f` on an expression nested in slice brackets that is not one
    /// of the slice's ends (an `if` condition, a `match` subject, a `for`
    /// iterable, a `then` branch): a `..` there is a range (FE27).
    fn outside_slice<T>(&mut self, f: impl FnOnce(&mut Self) -> PResult<T>) -> PResult<T> {
        let saved = self.slice.take();
        let r = f(self);
        self.slice = saved;
        r
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
        self.stmt_start = self.i;
        let mut n = self.statement()?;
        self.span(&mut n, start);
        Ok(n)
    }

    /// The current token. A newline is a token everywhere: inside
    /// brackets it is space only where the parser skips it (`gap`), next
    /// to the brackets and around `,`, and elsewhere by the continuation
    /// rules, as outside (01, FE78).
    fn peek(&mut self) -> &'a Token {
        &self.toks[self.i]
    }

    /// Skips the newlines at the current token: inside brackets, after the
    /// opener, before the closer and around `,` (01, FE78).
    fn gap(&mut self) {
        while self.toks[self.i].kind == Kind::Newline {
            self.i += 1;
        }
    }

    /// Whether the next token past newlines is `,` or `closer`: an item
    /// ends here. The newlines are skipped when it is.
    fn item_ends(&mut self, closer: Kind) -> bool {
        let mut j = self.i;
        while self.toks[j].kind == Kind::Newline {
            j += 1;
        }
        let end = matches!(self.toks[j].kind, Kind::Comma) || self.toks[j].kind == closer;
        if end {
            self.i = j;
        }
        end
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

    /// Whether the current token opens brackets (or is a tag before its
    /// `(`) whose closer is followed by one of `kinds`: a statement or an
    /// arm head known by one look at `pairs` (FE52).
    fn brackets_close_before(&mut self, kinds: &[Kind]) -> bool {
        self.peek();
        let mut open = self.i;
        if self.toks[open].kind == Kind::Tag {
            open += 1;
        }
        if !matches!(
            self.toks[open].kind,
            Kind::LParen | Kind::LBracket | Kind::LBrace
        ) {
            return false;
        }
        let close = self.pairs[open];
        close != usize::MAX && kinds.contains(&self.kind_from(close + 1))
    }

    /// The raw token after the current one (newlines included).
    fn kind_after(&self) -> Kind {
        self.toks[self.i + 1].kind
    }

    /// The kind of token `j`, skipping newlines when inside brackets and
    /// the token after them is `,` or a closer, where they are space (01,
    /// FE78).
    fn kind_from(&self, j: usize) -> Kind {
        let mut k = j;
        while self.nest > 0 && self.toks[k].kind == Kind::Newline {
            k += 1;
        }
        match self.toks[k].kind {
            Kind::Comma | Kind::RParen | Kind::RBracket | Kind::RBrace => self.toks[k].kind,
            _ => self.toks[j].kind,
        }
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
        let d = error("E0110", t.pos, vec![s(describe(t))]).span(tok_len(t));
        self.habit(Box::new(d), None)
    }

    /// E0112 "expected `what`, found …" for the current token; `k` is the
    /// kind that was wanted, when one was.
    fn expected(&mut self, what: &str, k: Option<Kind>) -> Box<Diag> {
        let t = self.peek();
        let d = error("E0112", t.pos, vec![s(what), s(describe(t))]).span(tok_len(t));
        self.habit(Box::new(d), k)
    }

    /// Expects the `;;` that closes the block `what` opened by the token
    /// `open` (`fn`, `if`, …); the error says where it opened (FE44).
    fn close(&mut self, what: &str, open: &Token) -> PResult<()> {
        if self.at(Kind::DoubleSemi) {
            self.next();
            return Ok(());
        }
        Err(self.unclosed(what, open))
    }

    /// E0112 for a block `open` began that is not closed here.
    fn unclosed(&mut self, what: &str, open: &Token) -> Box<Diag> {
        let mut d = self.expected(&format!("`;;` to close the {what}"), Some(Kind::DoubleSemi));
        d.notes.push(format!(
            "note: the `{}` opened at {}:{}",
            open.text, open.pos.line, open.pos.col
        ));
        // `end` on a line of its own reads as a name (FE43)
        if self.at(Kind::Eof) && d.help.is_none() {
            let toks = self.toks;
            let end = (1..self.i)
                .rev()
                .take_while(|&j| toks[j].off > open.off)
                .find(|&j| {
                    toks[j].kind == Kind::Name
                        && crate::habits::is_block_end(&toks[j].text)
                        && toks[j - 1].kind == Kind::Newline
                        && matches!(toks[j + 1].kind, Kind::Newline | Kind::Eof)
                });
            if let Some(j) = end {
                d.help = Some(format!(
                    "a block closes with `;;`; the `{}` on line {} is read as a name",
                    toks[j].text, toks[j].pos.line
                ));
            } else if let Some(line) = self.outdented(open) {
                d.notes.push(format!(
                    "note: line {line} is not indented under the `{}`: was the block meant to close before it?",
                    open.text
                ));
            }
        }
        d
    }

    /// The first line after `open`'s that starts no deeper than the line
    /// `open` stands on (a continuation such as `else` aside): where an
    /// unclosed block was most likely meant to end (FE44).
    fn outdented(&self, open: &Token) -> Option<u32> {
        let toks = self.toks;
        let at = toks[..self.i].iter().position(|t| t.off == open.off)?;
        let line_start = toks[..at]
            .iter()
            .rposition(|t| t.kind == Kind::Newline)
            .map_or(0, |j| j + 1);
        let indent = toks[line_start].pos.col;
        (at + 1..self.i)
            .find(|&j| {
                let t = &toks[j];
                toks[j - 1].kind == Kind::Newline
                    && !matches!(t.kind, Kind::Newline | Kind::Eof | Kind::DoubleSemi)
                    && !t.kind.starts_continuation()
                    && t.pos.col <= indent
            })
            .map(|j| toks[j].pos.line)
    }

    /// Names a habit of another language at an error the current token
    /// caused, from the one table (`habits.rs`, FE43): a word other
    /// languages have as a keyword, written at a statement's start; a
    /// reserved word bound as a name; an operator fung spells another
    /// way. `expected` is the kind an E0112 wanted.
    fn habit(&self, mut d: Box<Diag>, expected: Option<Kind>) -> Box<Diag> {
        use crate::habits;
        let toks = self.toks;
        let i = self.i;
        let cur = &toks[i];
        let w = self.stmt_start;
        let word = &toks[w];
        // the word stands right before the error, or before brackets that
        // close right before it: `while true:`, `while(x < 3):`
        let next_to = i == w + 1
            || (i > w + 1
                && toks[w + 1].kind == Kind::LParen
                && self.pairs[w + 1].checked_add(1) == Some(i));
        if word.kind == Kind::Name
            && next_to
            && !matches!(cur.kind, Kind::Eof | Kind::Newline)
            && let Some(h) = habits::word(&word.text)
        {
            let mut e = error("E0134", word.pos, vec![s(word.text.clone())]).span(tok_len(word));
            e.help = Some(h.to_string());
            return Box::new(e);
        }
        // a keyword of other languages where the error is, inside a
        // statement: `if a: 1 elif b: 2 ;;` (FE82)
        if cur.kind == Kind::Name
            && expected.is_none()
            && let Some(h) =
                habits::word(&cur.text).filter(|_| habits::value_word(&cur.text).is_none())
        {
            let mut e = error("E0134", cur.pos, vec![s(cur.text.clone())]).span(tok_len(cur));
            e.help = Some(h.to_string());
            return Box::new(e);
        }
        let prev = i.checked_sub(1).map(|j| &toks[j]);
        let touching = |p: &Token| p.end == cur.off;
        match (prev, cur.kind) {
            // `match = 1`
            (Some(p), Kind::Assign) if p.kind.is_keyword() && i - 1 == w => {
                let mut e = error(
                    "E0110",
                    p.pos,
                    vec![s(format!("reserved word `{}`", p.text))],
                )
                .span(tok_len(p));
                e.help = Some(habits::reserved(&p.text));
                return Box::new(e);
            }
            // `x += 1`
            (Some(p), Kind::Assign) if is_arith(p.kind) && touching(p) && expected.is_none() => {
                let mut e =
                    error("E0110", p.pos, vec![s(format!("`{}=`", p.text))]).span(tok_len(p) + 1);
                e.help = Some(habits::compound(&p.text));
                return Box::new(e);
            }
            // `x++`, and `++` between two values (FE82)
            (Some(p), Kind::Plus) if p.kind == Kind::Plus && touching(p) => {
                let mut e = error("E0110", p.pos, vec![s("`++`")]).span(2);
                let ends = matches!(
                    toks[i + 1].kind,
                    Kind::Newline
                        | Kind::Eof
                        | Kind::Semi
                        | Kind::DoubleSemi
                        | Kind::RParen
                        | Kind::RBracket
                        | Kind::RBrace
                        | Kind::Comma
                );
                e.help = if ends {
                    habits::operator("++").map(str::to_string)
                } else {
                    Some(habits::JOIN.to_string())
                };
                return Box::new(e);
            }
            // `if x = 1:`
            (_, Kind::Assign) if expected == Some(Kind::Colon) => {
                d.help = Some(habits::COMPARE.to_string());
            }
            _ => {}
        }
        let op = match cur.kind {
            Kind::Bar if toks[i + 1].kind == Kind::Bar && toks[i + 1].off == cur.end => {
                d.args[0] = s("`||`");
                d.len = 2;
                Some("||")
            }
            Kind::Bang if expected.is_none() => Some("!"),
            Kind::Arrow => Some("->"),
            Kind::LBrace
                if expected == Some(Kind::Colon)
                    || prev.is_some_and(|p| p.kind == Kind::RParen) =>
            {
                Some("{")
            }
            _ => None,
        };
        if let Some(h) = op.and_then(habits::operator) {
            d.help = Some(h.to_string());
        }
        d
    }

    /// E0112 for a bracketed list stopped by something other than `,` or
    /// its closer, with a note where the bracket opened (FE26).
    fn list_end(&mut self, open: Pos, close: Kind) -> Box<Diag> {
        let opener = match close {
            Kind::RBracket => "[",
            Kind::RBrace => "{",
            _ => "(",
        };
        let t = self.peek();
        let mut d = Box::new(
            error(
                "E0112",
                t.pos,
                vec![s(format!("`,` or `{}`", close.describe())), s(describe(t))],
            )
            .span(tok_len(t)),
        );
        d.notes.push(format!(
            "note: the `{opener}` opened at {}:{}",
            open.line, open.col
        ));
        // `f(x = 1)`: a named argument or a comparison (FE43)
        if t.kind == Kind::Assign {
            d.help = Some(crate::habits::NAMED_ARG.to_string());
        }
        d
    }

    /// `expect` with an E0112 "expected X, found Y" message.
    fn expect_desc(&mut self, k: Kind, what: &str) -> PResult<&'a Token> {
        if !self.at(k) {
            return Err(self.expected(what, Some(k)));
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

    /// program = body at nesting 0, terminated by EOF. Imports come
    /// before every other statement (D55).
    fn program(&mut self) -> PResult<Node> {
        let mut prog = Node::new(Head::Program, self.peek().pos, Vec::new());
        self.skip_seps();
        while !self.at(Kind::Eof) {
            if self.at(Kind::KwImport) && prog.kids.iter().any(|k| k.head != Head::Import) {
                let pos = self.peek().pos;
                return Err(Box::new(error("E0313", pos, Vec::new()).span(6)));
            }
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
