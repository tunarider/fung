//! Patterns, `match` and `for`.

use super::{MAX_DEPTH, PResult, Parser, describe, err, s, tag_as_name};
use crate::diag::Arg;
use crate::lexer::Kind;
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
            let term = self.pat_term()?;
            alt.kids.push(term);
        }
        Ok(alt)
    }

    fn pat_term(&mut self) -> PResult<Node> {
        if self.depth >= MAX_DEPTH {
            let pos = self.peek().pos;
            return Err(err("E0802", pos, vec![Arg::Int(MAX_DEPTH as i64)]));
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
                    _ => Err(self.unexpected()),
                }
            }
            Kind::String => {
                self.next();
                let mut text = String::new();
                for part in &t.parts {
                    if part.is_code {
                        return Err(err("E0110", t.pos, vec![s("interpolation in a pattern")]));
                    }
                    text.push_str(&part.text);
                }
                leaf(Head::Plit, Lit::Str(text))
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
                self.next();
                let n = Node::with_lit(Head::Ptag, t.pos, Lit::Str(t.text.clone()), Vec::new());
                self.bracketed(|p| p.pat_list(n, Kind::RParen))
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
    fn ptuple(&mut self, pos: crate::source::Pos) -> PResult<Node> {
        if self.at(Kind::RParen) {
            self.next();
            return Ok(Node::new(Head::Ptuple, pos, Vec::new()));
        }
        let first = self.pattern()?;
        if !self.at(Kind::Comma) {
            let p = self.peek();
            let what = format!(
                "`{}` (a parenthesized pattern is not a pattern; write (p,) for a tuple)",
                describe(p)
            );
            return Err(err("E0110", p.pos, vec![s(what)]));
        }
        self.next();
        self.pat_list(Node::new(Head::Ptuple, pos, vec![first]), Kind::RParen)
    }

    /// The items of a `[…]` pattern after the `[`: patterns, then at most
    /// one `...rest` at the end.
    fn plist(&mut self, mut n: Node) -> PResult<Node> {
        while !self.at(Kind::RBracket) {
            if self.at(Kind::Ellipsis) {
                self.next();
                let mut rest = Node::new(Head::Prest, n.pos, Vec::new());
                if self.at(Kind::Name) {
                    rest.lit = Lit::Str(self.next().text.clone());
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
                return Err(self.unexpected());
            }
        }
        self.next();
        Ok(n)
    }

    /// The entries of a `{…}` pattern after the `{`.
    fn pmap(&mut self, mut n: Node) -> PResult<Node> {
        while !self.at(Kind::RBrace) {
            if !self.at_name_like() {
                let p = self.peek();
                return Err(err("E0112", p.pos, vec![s("a map key"), s(describe(p))]));
            }
            let key = self.next();
            let shorthand = self.at(Kind::Comma) || self.at(Kind::RBrace);
            let v = if key.kind == Kind::Tag && shorthand {
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
                Lit::Str(key.key().to_string()),
                vec![v],
            ));
            if self.at(Kind::Comma) {
                self.next();
            } else if !self.at(Kind::RBrace) {
                return Err(self.unexpected());
            }
        }
        self.next();
        Ok(n)
    }

    /// Parses comma-separated patterns into n until the closing token.
    fn pat_list(&mut self, mut n: Node, closer: Kind) -> PResult<Node> {
        while !self.at(closer) {
            let k = self.pattern()?;
            n.kids.push(k);
            if self.at(Kind::Comma) {
                self.next();
            } else if !self.at(closer) {
                return Err(self.unexpected());
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
    /// - A nesting-limit error (E0802) is always fatal, so a stack overflow
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
            Err(e) if e.code == "E0802" => return Err(e),
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
        let subject = self.expr(0)?;
        self.expect_desc(Kind::Colon, "`:` after the match subject")?;
        let (saved_nest, saved_guard) = (self.nest, self.guard);
        self.nest = 0;
        self.guard = false;
        let r = self.match_arms(Node::new(Head::Match, t.pos, vec![subject]));
        self.nest = saved_nest;
        self.guard = saved_guard;
        r
    }

    fn match_arms(&mut self, mut n: Node) -> PResult<Node> {
        self.skip_seps();
        let mut next = if self.at(Kind::DoubleSemi) {
            None
        } else {
            Some(self.first_arm_head()?)
        };
        while let Some((pat, guard)) = next {
            let arrow = self.next(); // =>
            let (body, following) = self.arm_body(Node::new(Head::Block, arrow.pos, Vec::new()))?;
            if body.kids.is_empty() {
                let pos = self.peek().pos;
                return Err(err("E0110", pos, vec![s("an empty match arm")]));
            }
            n.kids
                .push(Node::new(Head::Arm, pat.pos, vec![pat, guard, body]));
            next = following;
        }
        self.next(); // ;;
        if n.kids.len() == 1 {
            return Err(err("E0110", n.pos, vec![s("a match without arms")]));
        }
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
        let pat = self.pattern()?;
        self.expect_desc(Kind::KwIn, "`in` after the for pattern")?;
        let iter = self.expr(0)?;
        self.expect_desc(Kind::Colon, "`:` after the for header")?;
        let body = self.body(t.pos)?;
        self.expect_desc(Kind::DoubleSemi, "`;;` to close the for")?;
        Ok(Node::new(Head::For, t.pos, vec![pat, iter, body]))
    }
}
