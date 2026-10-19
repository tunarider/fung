//! Statements, bodies, `if`, `test`, `import` and `type`.

use super::{PResult, Parser, describe, err, s};
use crate::lexer::{Kind, Token};
use crate::source::Pos;
use crate::tree::{Head, Import, Lit, Node, TypeAlias};

/// The text of a plain "..." token, or None when it interpolates.
fn plain_text(t: &Token) -> Option<String> {
    let mut text = String::new();
    for part in &t.parts {
        if part.is_code {
            return None;
        }
        text.push_str(&part.text);
    }
    Some(text)
}

impl<'a> Parser<'a> {
    /// Parses `NAME = expr` or an expression statement. Assignment is a
    /// statement, not an expression (01), so `x = y = 1` is a syntax error.
    pub(super) fn statement(&mut self) -> PResult<Node> {
        let t = self.peek();
        match t.kind {
            Kind::KwType => return self.type_alias(),
            Kind::KwImport => return self.import_stmt(),
            Kind::KwTest => return self.test_block(),
            Kind::KwAssert => {
                self.next();
                if self.at(Kind::Eof)
                    || self.at(Kind::Newline)
                    || self.at(Kind::Semi)
                    || self.at(Kind::DoubleSemi)
                {
                    return Err(self.unexpected());
                }
                let e = self.expr(0)?;
                return Ok(Node::new(Head::Assert, t.pos, vec![e]));
            }
            _ => {}
        }
        // `_ = e` evaluates e and discards it (03): the wildcard pattern.
        if t.kind == Kind::Name && t.text == "_" && self.kind_after() == Kind::Assign {
            self.next();
            self.next();
            self.skip_newlines_after_operator();
            let rhs = self.expr(0)?;
            let wild = Node::new(Head::Pwild, t.pos, Vec::new());
            return Ok(Node::new(Head::Assign, t.pos, vec![wild, rhs]));
        }
        // Destructuring: try `pattern =` first when the statement starts like
        // a pattern (01: 문장 시작에서 패턴 =를 먼저 시도).
        if matches!(
            t.kind,
            Kind::LParen | Kind::LBracket | Kind::LBrace | Kind::Tag
        ) && let Some(n) = self.try_destructure()?
        {
            return Ok(n);
        }
        if t.kind == Kind::KwFn && self.kind_after() == Kind::Name {
            self.next();
            let name = self.next();
            return self.fn_def(t, name.text.clone());
        }
        if t.kind == Kind::Name && self.kind_after() == Kind::Assign {
            self.next();
            self.next();
            self.skip_newlines_after_operator();
            let rhs = self.expr(0)?;
            let target = Node::leaf(Head::Name, t.pos, Lit::Str(t.text.clone()));
            return Ok(Node::new(Head::Assign, t.pos, vec![target, rhs]));
        }
        self.expr(0)
    }

    /// Parses statements up to (not including) `;;` or `else`. Inside a
    /// block newlines separate statements even when the block sits inside
    /// parentheses (01: 괄호 안에서 블록이 열리면 그 ;;까지는 줄바꿈이 구분자).
    /// A block inside a match guard is ordinary code again: bare lambdas
    /// are allowed there (01).
    pub(super) fn body(&mut self, pos: Pos) -> PResult<Node> {
        let (saved_nest, saved_guard) = (self.nest, self.guard);
        self.nest = 0;
        self.guard = false;
        self.block_depth += 1;
        let r = self.body_inner(pos);
        self.block_depth -= 1;
        self.nest = saved_nest;
        self.guard = saved_guard;
        r
    }

    fn body_inner(&mut self, pos: Pos) -> PResult<Node> {
        let mut blk = Node::new(Head::Block, pos, Vec::new());
        self.skip_seps();
        while !self.at(Kind::DoubleSemi) && !self.at(Kind::KwElse) && !self.at(Kind::Eof) {
            let stmt = self.spanned_statement()?;
            blk.kids.push(stmt);
            match self.peek().kind {
                Kind::Newline | Kind::Semi | Kind::DoubleSemi | Kind::KwElse | Kind::Eof => {}
                _ => return Err(self.unexpected()),
            }
            self.skip_seps();
        }
        Ok(blk)
    }

    /// Parses a whole if/else-if/else chain closed by one `;;`.
    pub(super) fn if_expr(&mut self) -> PResult<Node> {
        let n = self.if_chain()?;
        self.expect_desc(Kind::DoubleSemi, "`;;` to close the if")?;
        Ok(n)
    }

    /// Parses `if cond: body` plus an optional else part, without the
    /// closing `;;` (the whole chain shares one).
    fn if_chain(&mut self) -> PResult<Node> {
        let t = self.next(); // if
        let cond = self.expr(0)?;
        self.expect_desc(Kind::Colon, "`:` after the if condition")?;
        let then_b = self.body(t.pos)?;
        let else_b = if self.at(Kind::KwElse) {
            let e = self.next();
            if self.at(Kind::KwIf) {
                self.if_chain()?
            } else if self.at(Kind::Colon) {
                self.next();
                self.body(e.pos)?
            } else {
                let p = self.peek();
                return Err(err(
                    "E0112",
                    p.pos,
                    vec![s("`:` or `if` after else"), s(describe(p))],
                ));
            }
        } else {
            Node::new(Head::Nil, t.pos, Vec::new())
        };
        Ok(Node::new(Head::If, t.pos, vec![cond, then_b, else_b]))
    }

    /// Parses `pattern = expr` at the statement start. When the text is not
    /// an assignment it restores the position and returns None.
    fn try_destructure(&mut self) -> PResult<Option<Node>> {
        let (save, save_nest, save_depth) = (self.i, self.nest, self.depth);
        let pat = match self.pattern() {
            Ok(pat) if self.at(Kind::Assign) => pat,
            _ => {
                self.i = save;
                self.nest = save_nest;
                self.depth = save_depth;
                return Ok(None);
            }
        };
        self.next();
        self.skip_newlines_after_operator();
        let rhs = self.expr(0)?;
        Ok(Some(Node::new(Head::Assign, pat.pos, vec![pat, rhs])))
    }

    /// Parses `test "name": body ;;` (08). Tests live at the top level only;
    /// `fung run` drops them, `fung test` runs them.
    fn test_block(&mut self) -> PResult<Node> {
        let t = self.next(); // test
        if self.block_depth > 0 {
            return Err(err("E0114", t.pos, Vec::new()));
        }
        if !self.at(Kind::String) {
            let p = self.peek();
            return Err(err(
                "E0112",
                p.pos,
                vec![s("a test name string after `test`"), s(describe(p))],
            ));
        }
        let st = self.next();
        let Some(name) = plain_text(st) else {
            return Err(err(
                "E0112",
                st.pos,
                vec![s("a plain test name"), s("a string with interpolation")],
            ));
        };
        self.expect_desc(Kind::Colon, "`:` after the test name")?;
        let body = self.body(t.pos)?;
        self.expect_desc(Kind::DoubleSemi, "`;;` to close the test")?;
        Ok(Node::with_lit(
            Head::Test,
            t.pos,
            Lit::Str(name),
            vec![body],
        ))
    }

    /// Parses `import NAME|STRING [as NAME] [(NAME, …)]` (01 EBNF, 04). The
    /// path string must be plain: no interpolation.
    fn import_stmt(&mut self) -> PResult<Node> {
        let t = self.next(); // import
        let mut im = Import {
            spec: String::new(),
            is_path: false,
            alias: String::new(),
            names: None,
            name_pos: Vec::new(),
        };
        match self.peek().kind {
            Kind::Name => {
                // Dotted names: `_native.fs` for the stdlib's own layer (06)
                // and, later, packages.
                im.spec = self.next().text.clone();
                while self.at(Kind::Dot) && self.kind_after() == Kind::Name {
                    self.next();
                    im.spec.push('.');
                    im.spec.push_str(&self.next().text);
                }
            }
            Kind::String => {
                let st = self.next();
                let Some(text) = plain_text(st) else {
                    return Err(err(
                        "E0112",
                        st.pos,
                        vec![
                            s("a plain path string after `import`"),
                            s("a string with interpolation"),
                        ],
                    ));
                };
                im.spec = text;
                im.is_path = true;
            }
            _ => {
                let p = self.peek();
                return Err(err(
                    "E0112",
                    p.pos,
                    vec![
                        s("a module name or a path string after `import`"),
                        s(describe(p)),
                    ],
                ));
            }
        }
        if self.at(Kind::KwAs) {
            self.next();
            im.alias = self
                .expect_desc(Kind::Name, "a name after `as`")?
                .text
                .clone();
        }
        if self.at(Kind::LParen) {
            self.next();
            // newlines inside the parentheses are whitespace (01)
            self.nest += 1;
            let r = self.import_names(&mut im);
            self.nest -= 1;
            r?;
        }
        Ok(Node::leaf(Head::Import, t.pos, Lit::Import(Box::new(im))))
    }

    /// The names of `import m (a, b)` after the `(`, through the `)`.
    fn import_names(&mut self, im: &mut Import) -> PResult<()> {
        let mut names = Vec::new();
        loop {
            let name = self.expect_desc(Kind::Name, "a name to import")?;
            names.push(name.text.clone());
            im.name_pos.push(name.pos);
            if !self.at(Kind::Comma) {
                break;
            }
            self.next();
        }
        im.names = Some(names);
        self.expect_desc(Kind::RParen, "`)`")?;
        Ok(())
    }

    /// Parses `type name = type` (07). Alias names are lowercase.
    fn type_alias(&mut self) -> PResult<Node> {
        let t = self.next(); // type
        let name = self.expect_desc(Kind::Name, "a lowercase alias name after `type`")?;
        self.expect_desc(Kind::Assign, "`=`")?;
        self.skip_newlines_after_operator();
        let alts = self.type_spec()?;
        let alias = TypeAlias {
            name: name.text.clone(),
            name_pos: name.pos,
            alts,
        };
        Ok(Node::leaf(
            Head::Typealias,
            t.pos,
            Lit::Alias(Box::new(alias)),
        ))
    }
}
