//! Collection literals: lists, maps, tuples and tags.

use super::{PResult, Parser, describe, err, s, tag_as_name};
use crate::lexer::Kind;
use crate::tree::{Head, Lit, Node};

impl Parser<'_> {
    /// Parses `[e, …]`; newlines inside are whitespace.
    pub(super) fn list_literal(&mut self) -> PResult<Node> {
        let open = self.next(); // [
        self.nest += 1;
        let r = self.list_items(Node::new(Head::List, open.pos, Vec::new()));
        self.nest -= 1;
        r
    }

    fn list_items(&mut self, mut n: Node) -> PResult<Node> {
        while !self.at(Kind::RBracket) {
            let e = self.expr(0)?;
            n.kids.push(e);
            if self.at(Kind::Comma) {
                self.next();
            } else if !self.at(Kind::RBracket) {
                return Err(self.unexpected());
            }
        }
        self.next();
        Ok(n)
    }

    /// Parses `{key: e, …}`; a key is a name or a plain string (02).
    pub(super) fn map_literal(&mut self) -> PResult<Node> {
        let open = self.next(); // {
        self.nest += 1;
        let r = self.map_entries(Node::new(Head::Map, open.pos, Vec::new()));
        self.nest -= 1;
        r
    }

    fn map_entries(&mut self, mut n: Node) -> PResult<Node> {
        let mut seen: Vec<String> = Vec::new();
        while !self.at(Kind::RBrace) {
            let t = self.peek();
            let key = if self.at_name_like() {
                // An identifier-shaped key is a string whatever its case
                // (02), as written (not NFC); keywords are plain names
                // here (01).
                t.key().to_string()
            } else if t.kind == Kind::String {
                let mut text = String::new();
                for part in &t.parts {
                    if part.is_code {
                        return Err(err("E0110", t.pos, vec![s("interpolation in a map key")]));
                    }
                    text.push_str(&part.text);
                }
                text
            } else {
                return Err(err("E0112", t.pos, vec![s("a map key"), s(describe(t))]));
            };
            if seen.contains(&key) {
                return Err(err("E0113", t.pos, vec![s(key)]));
            }
            seen.push(key.clone());
            self.next();
            let shorthand = self.at(Kind::Comma) || self.at(Kind::RBrace);
            if t.kind == Kind::Tag && shorthand {
                return Err(tag_as_name(t));
            }
            let v = if t.kind == Kind::Name && shorthand {
                // {a, b} is {a: a, b: b} (02). Keywords and strings have no
                // variable to stand for, so they take no shorthand.
                Node::leaf(Head::Name, t.pos, Lit::Str(t.text.clone()))
            } else {
                self.expect_desc(Kind::Colon, "`:` after the map key")?;
                self.expr(0)?
            };
            n.kids
                .push(Node::with_lit(Head::Entry, t.pos, Lit::Str(key), vec![v]));
            if self.at(Kind::Comma) {
                self.next();
            } else if !self.at(Kind::RBrace) {
                return Err(self.unexpected());
            }
        }
        self.next();
        Ok(n)
    }

    /// Parses `()`, `(e)`, `(e,)`, `(a, b, …)` (01: 튜플).
    pub(super) fn paren_or_tuple(&mut self) -> PResult<Node> {
        let open = self.next(); // (
        self.nest += 1;
        let r = self.paren_or_tuple_inner(open.pos);
        self.nest -= 1;
        r
    }

    fn paren_or_tuple_inner(&mut self, pos: crate::source::Pos) -> PResult<Node> {
        if self.at(Kind::RParen) {
            self.next();
            return Ok(Node::new(Head::Tuple, pos, Vec::new()));
        }
        let first = self.expr(0)?;
        if self.at(Kind::RParen) {
            self.next();
            return Ok(first);
        }
        if !self.at(Kind::Comma) {
            return Err(self.unexpected());
        }
        let mut n = Node::new(Head::Tuple, pos, vec![first]);
        while self.at(Kind::Comma) {
            self.next();
            if self.at(Kind::RParen) {
                break;
            }
            let e = self.expr(0)?;
            n.kids.push(e);
        }
        self.expect_desc(Kind::RParen, "`)`")?;
        Ok(n)
    }

    /// Parses TAG or TAG(args) (02): a bare tag is its symbol. Outside
    /// brackets a newline ends the tag; inside them it is whitespace (01).
    pub(super) fn tag_expr(&mut self) -> PResult<Node> {
        let t = self.next();
        if !self.at(Kind::LParen) {
            return Ok(Node::leaf(Head::Symbol, t.pos, Lit::Str(t.text.clone())));
        }
        let n = self.call_args(Node::with_lit(
            Head::Tag,
            t.pos,
            Lit::Str(t.text.clone()),
            Vec::new(),
        ))?;
        if let Some(k) = n.kids.iter().find(|k| k.head == Head::Named) {
            return Err(err(
                "E0110",
                k.pos,
                vec![s("named argument in a tag (tags take values by position)")],
            ));
        }
        Ok(n)
    }
}
