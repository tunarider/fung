//! Patterns, `match` and `for`.

use super::{MAX_DEPTH, PResult, Parser, describe, err, s};
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
                if !self.at(Kind::LParen) || self.peek().pos.line != t.pos.line {
                    // A bare tag matches the symbol only (03의 함정).
                    return leaf(Head::Psym, Lit::Str(t.text.clone()));
                }
                self.next();
                let n = Node::with_lit(Head::Ptag, t.pos, Lit::Str(t.text.clone()), Vec::new());
                self.pat_list(n, Kind::RParen)
            }
            Kind::LParen => {
                self.next();
                if self.at(Kind::RParen) {
                    self.next();
                    return Ok(Node::new(Head::Ptuple, t.pos, Vec::new()));
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
                self.pat_list(Node::new(Head::Ptuple, t.pos, vec![first]), Kind::RParen)
            }
            Kind::LBracket => {
                self.next();
                self.plist(Node::new(Head::Plist, t.pos, Vec::new()))
            }
            Kind::LBrace => {
                self.next();
                self.pmap(Node::new(Head::Pmap, t.pos, Vec::new()))
            }
            _ => Err(self.unexpected()),
        }
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
            let v = if matches!(key.kind, Kind::Name | Kind::Tag)
                && (self.at(Kind::Comma) || self.at(Kind::RBrace))
            {
                // {a, b} binds a and b (02); keywords take no shorthand.
                Node::leaf(Head::Pbind, key.pos, Lit::Str(key.text.clone()))
            } else {
                self.expect_desc(Kind::Colon, "`:`")?;
                self.pattern()?
            };
            n.kids.push(Node::with_lit(
                Head::Pentry,
                key.pos,
                Lit::Str(key.text.clone()),
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

    /// Whether `pattern [when expr] =>` parses at the current index, without
    /// consuming anything (01: 절 판정은 시도로). A nesting-limit error inside
    /// the speculative parse is fatal (E0802) rather than "not an arm head",
    /// so a stack overflow cannot hide behind a syntax error.
    fn arm_head_ahead(&mut self) -> PResult<bool> {
        let (save, save_nest, save_depth) = (self.i, self.nest, self.depth);
        let r = self.arm_head_try();
        self.i = save;
        self.nest = save_nest;
        self.depth = save_depth;
        r
    }

    fn arm_head_try(&mut self) -> PResult<bool> {
        if let Err(e) = self.pattern() {
            return if e.code == "E0802" { Err(e) } else { Ok(false) };
        }
        if self.at(Kind::KwWhen) {
            self.next();
            if let Err(e) = self.guard_expr() {
                return if e.code == "E0802" { Err(e) } else { Ok(false) };
            }
        }
        Ok(self.at(Kind::FatArrow))
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
    /// that parses as an arm head or the closing `;;` (01).
    pub(super) fn match_expr(&mut self) -> PResult<Node> {
        let t = self.next(); // match
        let subject = self.expr(0)?;
        self.expect_desc(Kind::Colon, "`:` after the match subject")?;
        let saved = self.nest;
        self.nest = 0;
        let r = self.match_arms(Node::new(Head::Match, t.pos, vec![subject]));
        self.nest = saved;
        r
    }

    fn match_arms(&mut self, mut n: Node) -> PResult<Node> {
        self.skip_seps();
        while !self.at(Kind::DoubleSemi) {
            let is_arm = self.arm_head_ahead()?;
            if self.at(Kind::Eof) || !is_arm {
                return Err(self.unexpected());
            }
            let pat = self.pattern()?;
            let guard = if self.at(Kind::KwWhen) {
                self.next();
                self.guard_expr()?
            } else {
                Node::new(Head::Nil, pat.pos, Vec::new())
            };
            let arrow = self.next(); // =>
            let body = self.arm_body(Node::new(Head::Block, arrow.pos, Vec::new()))?;
            if body.kids.is_empty() {
                let pos = self.peek().pos;
                return Err(err("E0110", pos, vec![s("an empty match arm")]));
            }
            n.kids
                .push(Node::new(Head::Arm, pat.pos, vec![pat, guard, body]));
        }
        self.next(); // ;;
        if n.kids.len() == 1 {
            return Err(err("E0110", n.pos, vec![s("a match without arms")]));
        }
        Ok(n)
    }

    /// The statements of one arm, up to the next arm head or `;;`.
    fn arm_body(&mut self, mut body: Node) -> PResult<Node> {
        self.skip_seps();
        while !self.at(Kind::DoubleSemi) && !self.at(Kind::Eof) {
            if self.arm_head_ahead()? {
                break;
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
        Ok(body)
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
