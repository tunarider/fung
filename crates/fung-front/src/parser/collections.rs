//! Collection literals: lists, maps, tuples and tags.

use std::collections::HashSet;

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
                return Err(self.list_end(n.pos, Kind::RBracket));
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
        let mut seen: HashSet<Vec<u8>> = HashSet::new();
        while !self.at(Kind::RBrace) {
            let t = self.peek();
            let key = if self.at_name_like() {
                // An identifier-shaped key is a string whatever its case
                // (02), as written (not NFC); keywords are plain names
                // here (01).
                Ok(t.key().to_string())
            } else if t.kind == Kind::String {
                let Some(text) = t.plain() else {
                    return Err(err("E0122", t.pos, vec![s("a map key")]));
                };
                text
            } else {
                return Err(err("E0112", t.pos, vec![s("a map key"), s(describe(t))]));
            };
            let bytes = match &key {
                Ok(k) => k.as_bytes(),
                Err(b) => b,
            };
            if !seen.insert(bytes.to_vec()) {
                return Err(err("E0113", t.pos, vec![s(crate::text::lossy(bytes))]));
            }
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
            n.kids.push(Node::with_lit(
                Head::Entry,
                t.pos,
                super::str_lit(key),
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
        let mut first = self.expr(0)?;
        if self.at(Kind::RParen) {
            self.next();
            first.paren = true;
            return Ok(first);
        }
        if !self.at(Kind::Comma) {
            return Err(self.list_end(pos, Kind::RParen));
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
        if !self.at(Kind::RParen) {
            return Err(self.list_end(pos, Kind::RParen));
        }
        self.next();
        Ok(n)
    }

    /// Parses TAG or TAG(args) (02): a bare tag is its symbol. `TAG()` is
    /// read here and refused by `desugar` (E0117) unless a pipe fills it:
    /// `x |> Ok()` is `Ok(x)`. Outside brackets a newline ends the tag;
    /// inside them it is whitespace (01).
    pub(super) fn tag_expr(&mut self) -> PResult<Node> {
        let t = self.next();
        if !self.at(Kind::LParen) {
            return Ok(Node::leaf(Head::BareTag, t.pos, Lit::Str(t.text.clone())));
        }
        let n = self.call_args(Node::with_lit(
            Head::Tag,
            t.pos,
            Lit::Str(t.text.clone()),
            Vec::new(),
        ))?;
        if let Some(k) = n.kids.iter().find(|k| k.head == Head::Named) {
            return Err(err("E0121", k.pos, Vec::new()));
        }
        Ok(n)
    }
}
