//! The indentation state machine (Go `layout.go`).

use super::Tok;
use super::lines::Line;
use super::spacing::{is_closer, is_opener};
use crate::lexer::Kind;

/// Something that indents the lines after it until it is closed: a block
/// (fn/if/match/for/test, `:` … `;;`) or a bracket.
struct Opener {
    /// `KwIf`, `KwMatch`, `KwFn`, `KwFor`, `KwTest` for blocks; `LParen`,
    /// `LBracket`, `LBrace` for brackets.
    kind: Kind,
    block: bool,
    /// Indent of the line that opened it.
    base: usize,
}

/// A block keyword waiting for its `:` at a bracket depth.
struct Pending {
    kind: Kind,
    /// Bracket depth where the keyword appeared.
    depth: usize,
}

struct Layouter {
    stack: Vec<Opener>,
    pend: Vec<Pending>,
    /// Unmatched `then` per bracket frame (index = bracket depth).
    thens: Vec<usize>,
    /// The previous code line ended with a token that cannot end a
    /// statement (01).
    cont_next: bool,
}

/// A line-final token after which the statement goes on (01: 줄 이어짐
/// 규칙 1), brackets and block colons aside — those indent through the
/// opener stack.
fn ends_open(k: Kind) -> bool {
    use Kind::*;
    matches!(
        k,
        Plus | Minus
            | Star
            | Slash
            | SlashSlash
            | Percent
            | StarStar
            | Eq
            | Ne
            | Lt
            | Gt
            | Le
            | Ge
            | KwIn
            | Coalesce
            | Pipe
            | DotDot
            | DotDotEq
            | KwAnd
            | KwOr
            | KwNot
            | Assign
            | FatArrow
            | Arrow
            | Dot
            | KwThen
            | KwElse
            | Bar
    )
}

fn is_continuation(k: Kind) -> bool {
    use Kind::*;
    matches!(k, Pipe | Dot | Coalesce | KwAnd | KwOr | KwThen)
}

fn block_keyword(k: Kind) -> bool {
    use Kind::*;
    matches!(k, KwFn | KwIf | KwMatch | KwFor | KwTest)
}

/// Whether the `fn` at `toks[i]` starts a function, which is followed by
/// its name or `(` (01). Otherwise it is the type name `fn` (07: `h: fn`,
/// `-> fn`), which waits for no `:` — a `:` of a later annotation or the
/// function's own block colon must not be taken for it.
fn fn_head(toks: &[Tok], i: usize) -> bool {
    toks.get(i + 1)
        .is_some_and(|t| matches!(t.kind, Kind::Name | Kind::LParen))
}

pub(super) fn is_closing(k: Kind) -> bool {
    k == Kind::DoubleSemi || is_closer(k)
}

/// A token a match pattern can begin with (01).
fn pattern_start(k: Kind) -> bool {
    use Kind::*;
    matches!(
        k,
        Name | Tag
            | Int
            | Float
            | String
            | RawString
            | Symbol
            | Duration
            | Minus
            | LParen
            | LBracket
            | LBrace
            | KwTrue
            | KwFalse
            | KwNil
    )
}

/// Whether the line reads as a match arm head, the parser's rule (01):
/// `pattern [when expr] =>` — a pattern-shaped start, a `=>` outside
/// brackets, and no `=` before it (`g = x => x` is a statement that
/// happens to hold a lambda). Gives the index of the `=>`.
fn top_arrow(toks: &[Tok]) -> Option<usize> {
    if !toks.first().is_some_and(|t| pattern_start(t.kind)) {
        return None;
    }
    let mut depth = 0i64;
    for (i, t) in toks.iter().enumerate() {
        if is_opener(t.kind) {
            depth += 1;
        } else if is_closer(t.kind) {
            depth -= 1;
        } else if t.kind == Kind::Assign && depth == 0 {
            return None;
        } else if t.kind == Kind::FatArrow && depth == 0 {
            return Some(i);
        }
    }
    None
}

/// Decides every code line's indent, then gives comment lines the indent
/// of the code line that follows them (inside a closer's block when that
/// line closes one), or of the last code line after it.
pub(super) fn layout(lines: &mut [Line]) {
    let mut l = Layouter {
        stack: Vec::new(),
        pend: Vec::new(),
        thens: vec![0],
        cont_next: false,
    };
    for ln in lines.iter_mut() {
        if ln.toks.is_empty() {
            continue;
        }
        ln.indent = l.line_indent(ln);
        l.consume(ln);
        l.end_line(ln);
    }
    let mut next = lines
        .iter()
        .rev()
        .find(|ln| !ln.toks.is_empty())
        .map_or(0, |ln| ln.indent);
    for ln in lines.iter_mut().rev() {
        if !ln.toks.is_empty() {
            next = ln.indent;
            if is_closing(ln.toks[0].kind) {
                next += 1;
            }
            continue;
        }
        if ln.is_comment() {
            ln.indent = next;
        }
    }
}

impl Layouter {
    /// Whether a line continues the statement before it: it starts with a
    /// continuation token, is the `else` of a `then`, or the previous line
    /// ended open.
    fn continues(&self, first: Kind) -> bool {
        is_continuation(first) || (first == Kind::KwElse && self.then_else()) || self.cont_next
    }

    fn brackets(&self) -> usize {
        self.stack.iter().filter(|o| !o.block).count()
    }

    fn innermost(&self) -> Option<&Opener> {
        self.stack.last()
    }

    /// Whether an `else` here continues a `then` expression of the
    /// current statement rather than an if-chain.
    fn then_else(&self) -> bool {
        self.thens[self.brackets()] > 0
    }

    /// The indent of a code line from the state at its start.
    fn line_indent(&self, ln: &Line) -> usize {
        let first = ln.toks[0].kind;
        let inn = self.innermost();
        // A closing token drops to the line that opened what it closes.
        if is_closing(first) {
            return inn.map_or(0, |o| o.base);
        }
        // An if-chain `else` sits with its `if`.
        if first == Kind::KwElse
            && !self.then_else()
            && let Some(o) = inn
            && o.block
            && o.kind == Kind::KwIf
        {
            return o.base;
        }
        let indent = inn.map_or(0, |o| o.base + 1);
        // A continuation line goes one deeper than its statement — unless
        // a bracket already indents it (every line inside brackets sits at
        // +1).
        let in_bracket = inn.is_some_and(|o| !o.block);
        if self.continues(first) && !in_bracket {
            return indent + 1;
        }
        // Inside a match block a line that is not an arm belongs to the
        // previous arm's body.
        if inn.is_some_and(|o| o.block && o.kind == Kind::KwMatch) && top_arrow(&ln.toks).is_none()
        {
            return indent + 1;
        }
        indent
    }

    /// Updates the state with a line's tokens.
    fn consume(&mut self, ln: &mut Line) {
        let first = ln.toks[0].kind;
        if !self.continues(first) {
            // A new statement: forget block keywords that never got their
            // `:` (fn f() = e) and unmatched thens at this bracket depth.
            let d = self.brackets();
            while self.pend.last().is_some_and(|p| p.depth == d) {
                self.pend.pop();
            }
            self.thens[d] = 0;
        }
        if let Some(i) = top_arrow(&ln.toks)
            && self
                .innermost()
                .is_some_and(|o| o.block && o.kind == Kind::KwMatch)
        {
            ln.arrow = Some(i);
        }
        let n = ln.toks.len();
        for i in 0..n {
            let k = ln.toks[i].kind;
            let d = self.brackets();
            if is_opener(k) {
                self.stack.push(Opener {
                    kind: k,
                    block: false,
                    base: ln.indent,
                });
                self.thens.push(0);
            } else if is_closer(k) {
                self.pop_bracket();
            } else if k == Kind::DoubleSemi {
                self.pop_block();
            } else if k == Kind::KwThen {
                self.thens[d] += 1;
            } else if k == Kind::KwElse {
                if self.thens[d] > 0 {
                    self.thens[d] -= 1;
                    continue;
                }
                // if-chain: the if block ends here and `else …:` reopens one.
                if self
                    .innermost()
                    .is_some_and(|o| o.block && o.kind == Kind::KwIf)
                {
                    self.stack.pop();
                }
                self.pend.push(Pending {
                    kind: Kind::KwIf,
                    depth: d,
                });
            } else if block_keyword(k) && (k != Kind::KwFn || fn_head(&ln.toks, i)) {
                self.pend.push(Pending { kind: k, depth: d });
            } else if k == Kind::Colon {
                let Some(p) = self.pend.last() else {
                    continue;
                };
                if p.depth != d {
                    continue;
                }
                let p = self.pend.pop().expect("checked above");
                // `else if c:` leaves the `else` pending behind: drop it.
                while p.kind == Kind::KwIf
                    && self
                        .pend
                        .last()
                        .is_some_and(|q| q.depth == d && q.kind == Kind::KwIf)
                {
                    self.pend.pop();
                }
                self.stack.push(Opener {
                    kind: p.kind,
                    block: true,
                    base: ln.indent,
                });
                if i == n - 1 {
                    ln.opens_block = true;
                }
            }
        }
    }

    /// Records whether the next line continues this one.
    fn end_line(&mut self, ln: &Line) {
        self.cont_next = ends_open(ln.toks[ln.toks.len() - 1].kind);
    }

    fn pop_bracket(&mut self) {
        if let Some(n) = self.stack.iter().rposition(|o| !o.block) {
            self.stack.truncate(n);
            if self.thens.len() > 1 {
                self.thens.pop();
            }
        }
    }

    fn pop_block(&mut self) {
        if let Some(n) = self.stack.iter().rposition(|o| o.block) {
            self.stack.truncate(n);
        }
    }
}
