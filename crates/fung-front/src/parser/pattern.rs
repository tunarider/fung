//! Patterns, `match` and `for`.

use super::{MAX_DEPTH, PResult, Parser, describe, err, s, tag_as_name};
use crate::diag::Arg;
use crate::lexer::Kind;
use crate::source::Pos;
use crate::tree::{Head, Lit, Node};

impl Parser<'_> {
    /// pattern = pat_term { "|" pat_term }   (01 EBNF)
    pub(super) fn pattern(&mut self) -> PResult<Node> {
        let first = self.pat_term()?;
        if !self.at(Kind::Bar) {
            return Ok(first);
        }
        let mut alt = Node::new(Head::Palt, first.pos, vec![first]);
        while self.at(Kind::Bar) {
            self.next();
            // a line ending in `|` continues (01 rule 1, FE7)
            self.skip_newlines_after_operator();
            let term = self.pat_term()?;
            alt.kids.push(term);
        }
        Ok(alt)
    }

    /// destructure = d_term { "|" d_term }: the pattern of an assignment,
    /// a `for` head and a parameter (FE32). Each alternative is a name,
    /// `_`, a tuple, list or map, or a tag with values; a literal (number,
    /// string, symbol, bare tag, duration, `true`/`false`/`nil`) cannot
    /// stand at the top, since it would only check a value, but may sit
    /// inside (`(1, y) = p`).
    pub(super) fn destructure(&mut self) -> PResult<Node> {
        let first = self.destructure_term()?;
        if !self.at(Kind::Bar) {
            return Ok(first);
        }
        let mut alt = Node::new(Head::Palt, first.pos, vec![first]);
        while self.at(Kind::Bar) {
            self.next();
            self.skip_newlines_after_operator();
            let term = self.destructure_term()?;
            alt.kids.push(term);
        }
        Ok(alt)
    }

    fn destructure_term(&mut self) -> PResult<Node> {
        let t = self.peek();
        let ok = match t.kind {
            Kind::Name | Kind::LParen | Kind::LBracket | Kind::LBrace => true,
            Kind::Tag => self.kind_from(self.i + 1) == Kind::LParen,
            _ => false,
        };
        if !ok {
            return Err(err(
                "E0112",
                t.pos,
                vec![s("a name or a destructuring pattern"), s(describe(t))],
            ));
        }
        self.pat_term()
    }

    fn pat_term(&mut self) -> PResult<Node> {
        if self.depth >= MAX_DEPTH {
            let pos = self.peek().pos;
            return Err(err("E0115", pos, vec![Arg::Int(MAX_DEPTH as i64)]));
        }
        self.depth += 1;
        let r = self.pat_term_inner();
        self.depth -= 1;
        r
    }

    fn pat_term_inner(&mut self) -> PResult<Node> {
        let t = self.peek();
        let leaf = |head, lit| Ok(Node::leaf(head, t.pos, lit));
        match t.kind {
            Kind::Name => {
                self.next();
                if t.text == "_" {
                    return Ok(Node::new(Head::Pwild, t.pos, Vec::new()));
                }
                leaf(Head::Pbind, Lit::Str(t.text.clone()))
            }
            Kind::Int => {
                // like an expression: only `-` makes this magnitude valid (01)
                if super::expr::is_min_int_magnitude(t) {
                    return Err(err("E0102", t.pos, vec![s(t.text.clone())]));
                }
                self.next();
                leaf(Head::Plit, Lit::Int(t.int))
            }
            Kind::Float => {
                self.next();
                leaf(Head::Plit, Lit::Float(t.float))
            }
            Kind::Minus => {
                self.next();
                let n = self.peek();
                match n.kind {
                    Kind::Int => {
                        self.next();
                        // the literal of the least int is read as its
                        // magnitude, so negating wraps back to it
                        leaf(Head::Plit, Lit::Int(n.int.wrapping_neg()))
                    }
                    Kind::Float => {
                        self.next();
                        leaf(Head::Plit, Lit::Float(-n.float))
                    }
                    // `-1s` is a value, so a pattern too (FE32)
                    Kind::Duration => {
                        self.next();
                        leaf(Head::Plit, Lit::Dur(-n.dur))
                    }
                    _ => Err(self.unexpected()),
                }
            }
            Kind::String => {
                self.next();
                let Some(text) = t.plain() else {
                    return Err(err("E0122", t.pos, vec![s("a pattern")]));
                };
                leaf(Head::Plit, super::str_lit(text))
            }
            Kind::RawString => {
                self.next();
                leaf(Head::Plit, Lit::Str(t.text.clone()))
            }
            Kind::Symbol => {
                self.next();
                leaf(Head::Psym, Lit::Str(t.text.clone()))
            }
            Kind::Duration => {
                self.next();
                leaf(Head::Plit, Lit::Dur(t.dur))
            }
            Kind::KwTrue | Kind::KwFalse => {
                self.next();
                leaf(Head::Plit, Lit::Bool(t.kind == Kind::KwTrue))
            }
            Kind::KwNil => {
                self.next();
                Ok(Node::new(Head::Pnil, t.pos, Vec::new()))
            }
            Kind::Tag => {
                self.next();
                // Outside brackets a newline ends the tag; inside them it is
                // whitespace (01), so `(` may follow on the next line.
                if !self.at(Kind::LParen) {
                    // A bare tag matches the symbol only (03의 함정).
                    return leaf(Head::Psym, Lit::Str(t.text.clone()));
                }
                let open = self.next().pos;
                // `TAG()` is E0117 here too: the bare tag is the symbol.
                if self.at(Kind::RParen) {
                    return Err(err("E0117", t.pos, vec![s(&t.text)]));
                }
                let n = Node::with_lit(Head::Ptag, t.pos, Lit::Str(t.text.clone()), Vec::new());
                let n = self.bracketed(|p| p.pat_list(n, open, Kind::RParen))?;
                // A result holds exactly one value (02, SP12): another
                // count could never match.
                if matches!(t.text.as_str(), "Ok" | "Err") && n.kids.len() != 1 {
                    let count = Arg::Int(n.kids.len() as i64);
                    return Err(err("E0236", t.pos, vec![s(&t.text), count]));
                }
                Ok(n)
            }
            Kind::LParen => {
                self.next();
                self.bracketed(|p| p.ptuple(t.pos))
            }
            Kind::LBracket => {
                self.next();
                self.bracketed(|p| p.plist(Node::new(Head::Plist, t.pos, Vec::new())))
            }
            Kind::LBrace => {
                self.next();
                self.bracketed(|p| p.pmap(Node::new(Head::Pmap, t.pos, Vec::new())))
            }
            _ => Err(self.unexpected()),
        }
    }

    /// Parses the inside of a bracketed pattern, where newlines are
    /// whitespace (01). `f` consumes the closing bracket last.
    fn bracketed(&mut self, f: impl FnOnce(&mut Self) -> PResult<Node>) -> PResult<Node> {
        self.nest += 1;
        let r = f(self);
        self.nest -= 1;
        r
    }

    /// A `(…)` pattern after the `(`: `()` or a tuple with at least one comma.
    fn ptuple(&mut self, pos: Pos) -> PResult<Node> {
        if self.at(Kind::RParen) {
            self.next();
            return Ok(Node::new(Head::Ptuple, pos, Vec::new()));
        }
        let first = self.pattern()?;
        if !self.at(Kind::Comma) {
            let p = self.peek();
            return Err(err(
                "E0112",
                p.pos,
                vec![
                    s(
                        "`,` in a tuple pattern (a parenthesized pattern is not a pattern; write `(p,)` for a one-item tuple)",
                    ),
                    s(describe(p)),
                ],
            ));
        }
        self.next();
        self.pat_list(Node::new(Head::Ptuple, pos, vec![first]), pos, Kind::RParen)
    }

    /// The items of a `[…]` pattern after the `[`: patterns, then at most
    /// one `...rest` at the end.
    fn plist(&mut self, mut n: Node) -> PResult<Node> {
        while !self.at(Kind::RBracket) {
            if self.at(Kind::Ellipsis) {
                self.next();
                let mut rest = Node::new(Head::Prest, n.pos, Vec::new());
                if self.at(Kind::Name) {
                    // `..._` is `...`: `_` binds nothing (FE11)
                    let name = self.next();
                    if name.text != "_" {
                        rest.lit = Lit::Str(name.text.clone());
                    }
                }
                n.kids.push(rest);
                if self.at(Kind::Comma) {
                    self.next();
                }
                if !self.at(Kind::RBracket) {
                    return Err(self.unexpected());
                }
                break;
            }
            let k = self.pattern()?;
            n.kids.push(k);
            if self.at(Kind::Comma) {
                self.next();
            } else if !self.at(Kind::RBracket) {
                return Err(self.list_end(n.pos, Kind::RBracket));
            }
        }
        self.next();
        Ok(n)
    }

    /// The entries of a `{…}` pattern after the `{`.
    fn pmap(&mut self, mut n: Node) -> PResult<Node> {
        let mut seen = std::collections::HashSet::new();
        while !self.at(Kind::RBrace) {
            // The keys a map literal takes (SP3): identifier-shaped, or a
            // string without interpolation (a pattern's keys are text).
            let p = self.peek();
            let text = if self.at_name_like() {
                p.key().to_string()
            } else if p.kind == Kind::String {
                match p.plain() {
                    Some(Ok(text)) => text,
                    Some(Err(_)) => {
                        return Err(err(
                            "E0112",
                            p.pos,
                            vec![s("a map key that is text"), s(describe(p))],
                        ));
                    }
                    None => return Err(err("E0122", p.pos, vec![s("a map pattern's key")])),
                }
            } else {
                return Err(err("E0112", p.pos, vec![s("a map key"), s(describe(p))]));
            };
            if !seen.insert(text.clone()) {
                return Err(err("E0113", p.pos, vec![s(text)]));
            }
            let key = self.next();
            let shorthand = self.at(Kind::Comma) || self.at(Kind::RBrace);
            let v = if key.kind == Kind::String {
                self.expect_desc(Kind::Colon, "`:` after the map key")?;
                self.pattern()?
            } else if key.kind == Kind::Tag && shorthand {
                return Err(tag_as_name(key));
            } else if key.kind == Kind::Name && shorthand {
                // {a, b} binds a and b (02); keywords take no shorthand.
                Node::leaf(Head::Pbind, key.pos, Lit::Str(key.text.clone()))
            } else {
                self.expect_desc(Kind::Colon, "`:`")?;
                self.pattern()?
            };
            // The key as written; a shorthand's variable is the NFC name.
            n.kids.push(Node::with_lit(
                Head::Pentry,
                key.pos,
                Lit::Str(text),
                vec![v],
            ));
            if self.at(Kind::Comma) {
                self.next();
            } else if !self.at(Kind::RBrace) {
                return Err(self.list_end(n.pos, Kind::RBrace));
            }
        }
        self.next();
        Ok(n)
    }

    /// Parses comma-separated patterns into n until the closing token.
    fn pat_list(&mut self, mut n: Node, open: Pos, closer: Kind) -> PResult<Node> {
        while !self.at(closer) {
            let k = self.pattern()?;
            n.kids.push(k);
            if self.at(Kind::Comma) {
                self.next();
            } else if !self.at(closer) {
                return Err(self.list_end(open, closer));
            }
        }
        self.next();
        Ok(n)
    }

    /// Parses `pattern [when expr]` up to the arm's `=>` when the tokens at
    /// the current index form an arm head (01: 절 판정은 시도로), so each
    /// head is parsed once. Otherwise nothing is consumed and the result is
    /// `None`.
    /// - A pattern followed by `when` is an arm head: `when` appears nowhere
    ///   else, so an error in the guard (or a missing `=>`) is reported as it
    ///   is.
    /// - A nesting-limit error (E0115) is always fatal, so a stack overflow
    ///   cannot hide behind "not an arm head".
    fn arm_head(&mut self) -> PResult<Option<(Node, Node)>> {
        let (save, save_nest, save_depth) = (self.i, self.nest, self.depth);
        let back = |p: &mut Self| {
            p.i = save;
            p.nest = save_nest;
            p.depth = save_depth;
        };
        let pat = match self.pattern() {
            Ok(pat) => pat,
            Err(e) if e.code == "E0115" => return Err(e),
            Err(_) => {
                back(self);
                return Ok(None);
            }
        };
        if self.at(Kind::KwWhen) {
            self.next();
            let guard = self.guard_expr()?;
            if !self.at(Kind::FatArrow) {
                return Err(self.unexpected());
            }
            return Ok(Some((pat, guard)));
        }
        if !self.at(Kind::FatArrow) {
            back(self);
            return Ok(None);
        }
        let guard = Node::new(Head::Nil, pat.pos, Vec::new());
        Ok(Some((pat, guard)))
    }

    /// Parses a `when` condition with bare lambdas disabled so the arm's
    /// `=>` is not read as a lambda arrow.
    fn guard_expr(&mut self) -> PResult<Node> {
        let saved = self.guard;
        self.guard = true;
        let r = self.expr(0);
        self.guard = saved;
        r
    }

    /// Parses `match e: arms ;;`. An arm's body runs until the next line
    /// that parses as an arm head or the closing `;;` (01). Arm bodies are
    /// ordinary code even when the match sits in a guard.
    pub(super) fn match_expr(&mut self) -> PResult<Node> {
        let t = self.next(); // match
        let subject = self.outside_slice(|p| p.expr(0))?;
        self.expect_desc(Kind::Colon, "`:` after the match subject")?;
        let (saved_nest, saved_guard, saved_slice) = (self.nest, self.guard, self.slice);
        self.nest = 0;
        self.guard = false;
        self.slice = None;
        let r = self.match_arms(Node::new(Head::Match, t.pos, vec![subject]));
        self.nest = saved_nest;
        self.guard = saved_guard;
        self.slice = saved_slice;
        r
    }

    fn match_arms(&mut self, mut n: Node) -> PResult<Node> {
        self.skip_seps();
        if self.at(Kind::DoubleSemi) {
            let p = self.peek();
            return Err(err("E0112", p.pos, vec![s("a match arm"), s(describe(p))]));
        }
        let mut next = Some(self.first_arm_head()?);
        while let Some((pat, guard)) = next {
            let arrow = self.next(); // =>
            let (body, following) = self.arm_body(Node::new(Head::Block, arrow.pos, Vec::new()))?;
            if body.kids.is_empty() {
                let p = self.peek();
                let mut e = err(
                    "E0112",
                    p.pos,
                    vec![s("the arm's body after `=>`"), s(describe(p))],
                );
                // `1 => y => y + 1`: `y =>` was read as the next arm (FE25).
                if following.is_some() {
                    e.help = Some(
                        "a lambda as an arm's value goes in parentheses: `1 => (y => y + 1)`"
                            .to_string(),
                    );
                }
                return Err(e);
            }
            n.kids
                .push(Node::new(Head::Arm, pat.pos, vec![pat, guard, body]));
            next = following;
        }
        self.next(); // ;;
        Ok(n)
    }

    /// The head of the first arm, which must be one: when it is not, the
    /// pattern is parsed for real so its own error is reported.
    fn first_arm_head(&mut self) -> PResult<(Node, Node)> {
        if let Some(head) = self.arm_head()? {
            return Ok(head);
        }
        if self.at(Kind::Eof) {
            return Err(self.unexpected());
        }
        self.pattern()?;
        Err(self.unexpected())
    }

    /// The statements of one arm, up to the next arm head (returned, parsed)
    /// or `;;` (not consumed).
    fn arm_body(&mut self, mut body: Node) -> PResult<(Node, Option<(Node, Node)>)> {
        self.skip_seps();
        while !self.at(Kind::DoubleSemi) {
            if self.at(Kind::Eof) {
                return Err(self.unexpected());
            }
            if let Some(head) = self.arm_head()? {
                return Ok((body, Some(head)));
            }
            self.block_depth += 1;
            let stmt = self.spanned_statement();
            self.block_depth -= 1;
            body.kids.push(stmt?);
            match self.peek().kind {
                Kind::Newline | Kind::Semi | Kind::DoubleSemi | Kind::Eof => {}
                _ => return Err(self.unexpected()),
            }
            self.skip_seps();
        }
        Ok((body, None))
    }

    /// Parses `for pat in e: body ;;`.
    pub(super) fn for_expr(&mut self) -> PResult<Node> {
        let t = self.next(); // for
        let pat = self.destructure()?;
        self.expect_desc(Kind::KwIn, "`in` after the for pattern")?;
        // a line ending in `in` continues (01 rule 1, FE7)
        self.skip_newlines_after_operator();
        let iter = self.outside_slice(|p| p.expr(0))?;
        self.expect_desc(Kind::Colon, "`:` after the for header")?;
        let body = self.body(t.pos)?;
        self.expect_desc(Kind::DoubleSemi, "`;;` to close the for")?;
        Ok(Node::new(Head::For, t.pos, vec![pat, iter, body]))
    }
}
