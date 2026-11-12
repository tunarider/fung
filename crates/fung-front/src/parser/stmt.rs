//! Statements, bodies, `if`, `test`, `import` and `type`.

use super::{PResult, Parser, err, s, tag_as_name};
use crate::lexer::{Kind, Token};
use crate::source::Pos;
use crate::tree::{Head, Import, Lit, Node, TypeAlias};

/// The text of a `"..."` token where a name or a path is written: no
/// interpolation, and valid UTF-8 (L4, FE10). `what` names the place for
/// E0112.
fn plain_text(t: &Token, what: &str) -> PResult<String> {
    match t.plain() {
        Some(Ok(text)) => Ok(text),
        Some(Err(bytes)) => Err(err(
            "E0112",
            t.pos,
            vec![
                s(format!("{what} that is text (valid UTF-8)")),
                s(format!("string {}", crate::consts::quote_bytes(&bytes))),
            ],
        )),
        None => Err(err(
            "E0112",
            t.pos,
            vec![s(what), s("a string with interpolation")],
        )),
    }
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
        // `MAX = 3` would be a pattern that only matches the tag (01).
        if t.kind == Kind::Tag && self.kind_after() == Kind::Assign {
            return Err(tag_as_name(t));
        }
        // Destructuring: try `pattern =` first when the statement starts like
        // a pattern (01: 문장 시작에서 패턴 =를 먼저 시도). A literal start
        // is tried too, so `1 = x` is refused as a pattern (FE32), and a
        // name before `|`, which no expression has.
        let pattern_start = match t.kind {
            Kind::LParen
            | Kind::LBracket
            | Kind::LBrace
            | Kind::Tag
            | Kind::Int
            | Kind::Float
            | Kind::Duration
            | Kind::String
            | Kind::RawString
            | Kind::Symbol
            | Kind::KwTrue
            | Kind::KwFalse
            | Kind::KwNil
            | Kind::Minus => true,
            Kind::Name => self.kind_after() == Kind::Bar,
            _ => false,
        };
        if pattern_start && let Some(n) = self.try_destructure()? {
            return Ok(n);
        }
        if t.kind == Kind::KwFn && self.kind_after() == Kind::Name {
            self.next();
            let name = self.next();
            return self.fn_def(t, name.text.clone(), name.pos);
        }
        if t.kind == Kind::Name && self.kind_after() == Kind::Assign {
            self.next();
            self.next();
            self.skip_newlines_after_operator();
            let rhs = self.expr(0)?;
            let target = Node::leaf(Head::Name, t.pos, Lit::Str(t.text.clone()));
            return Ok(Node::new(Head::Assign, t.pos, vec![target, rhs]));
        }
        // an expression statement may be a `return` (D50)
        self.return_ok = true;
        self.expr(0)
    }

    /// Parses statements up to (not including) `;;` or `else`. Inside a
    /// block newlines separate statements even when the block sits inside
    /// parentheses (01: 괄호 안에서 블록이 열리면 그 ;;까지는 줄바꿈이 구분자).
    /// A block inside a match guard is ordinary code again: bare lambdas
    /// are allowed there (01).
    pub(super) fn body(&mut self, pos: Pos) -> PResult<Node> {
        let (saved_nest, saved_guard, saved_slice) = (self.nest, self.guard, self.slice);
        self.nest = 0;
        self.guard = false;
        self.slice = None;
        self.block_depth += 1;
        let r = self.body_inner(pos);
        self.block_depth -= 1;
        self.nest = saved_nest;
        self.guard = saved_guard;
        self.slice = saved_slice;
        r
    }

    fn body_inner(&mut self, pos: Pos) -> PResult<Node> {
        let mut blk = Node::new(Head::Block, pos, Vec::new());
        self.skip_seps();
        while !self.at(Kind::DoubleSemi) && !self.at(Kind::KwElse) && !self.at(Kind::Eof) {
            self.after_return(&blk)?;
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

    /// E0131 when a statement would follow a `return` statement in the
    /// block being read: it could never run (D50).
    pub(super) fn after_return(&mut self, blk: &Node) -> PResult<()> {
        match blk.kids.last() {
            Some(k) if k.head == Head::Return && !k.paren => {
                let pos = self.peek().pos;
                Err(err("E0131", pos, Vec::new()))
            }
            _ => Ok(()),
        }
    }

    /// Parses a whole if/else-if/else chain closed by one `;;`.
    pub(super) fn if_expr(&mut self) -> PResult<Node> {
        let open = self.peek();
        let n = self.if_chain()?;
        self.close("if", open)?;
        Ok(n)
    }

    /// Parses `if cond: body` plus an optional else part, without the
    /// closing `;;` (the whole chain shares one).
    fn if_chain(&mut self) -> PResult<Node> {
        let t = self.next(); // if
        let cond = self.outside_slice(|p| p.expr(0))?;
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
                return Err(self.expected("`:` or `if` after else", Some(Kind::Colon)));
            }
        } else {
            Node::new(Head::Nil, t.pos, Vec::new())
        };
        Ok(Node::new(Head::If, t.pos, vec![cond, then_b, else_b]))
    }

    /// Parses `pattern = expr` at the statement start. When the text is not
    /// an assignment it restores the position and returns None.
    fn try_destructure(&mut self) -> PResult<Option<Node>> {
        // Brackets that close right before `=` make an assignment: the
        // pattern's own error is the one to report (FE52).
        if self.brackets_close_before(&[Kind::Assign]) {
            return self.destructure_assign().map(Some);
        }
        let (save, save_nest, save_depth) = (self.i, self.nest, self.depth);
        match self.pattern() {
            Ok(_) if self.at(Kind::Assign) => {}
            // `Ok(a, b)` is an error read as an expression too (E0236).
            Err(e) if e.code == "E0236" => return Err(e),
            _ => {
                self.i = save;
                self.nest = save_nest;
                self.depth = save_depth;
                return Ok(None);
            }
        }
        // It is an assignment: read it again as one (FE32), which points
        // at a literal the destructuring pattern does not take.
        self.i = save;
        self.destructure_assign().map(Some)
    }

    /// `pattern = expr`, known to be an assignment.
    fn destructure_assign(&mut self) -> PResult<Node> {
        let pat = self.destructure()?;
        self.expect_desc(Kind::Assign, "`=`")?;
        self.skip_newlines_after_operator();
        let rhs = self.expr(0)?;
        Ok(Node::new(Head::Assign, pat.pos, vec![pat, rhs]))
    }

    /// Parses `test "name": body ;;` (08). Tests live at the top level only;
    /// `fung run` drops them, `fung test` runs them.
    fn test_block(&mut self) -> PResult<Node> {
        let t = self.next(); // test
        if self.block_depth > 0 {
            return Err(err("E0114", t.pos, Vec::new()));
        }
        if !self.at(Kind::String) {
            return Err(self.expected("a test name string after `test`", None));
        }
        let st = self.next();
        let name = plain_text(st, "a plain test name")?;
        self.expect_desc(Kind::Colon, "`:` after the test name")?;
        let body = self.body(t.pos)?;
        self.close("test", t)?;
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
            bind_pos: t.pos,
            bind_len: 0,
        };
        match self.peek().kind {
            Kind::Name => {
                // Dotted names: `_native.fs` for the stdlib's own layer (06)
                // and, later, packages.
                let first = self.next();
                im.bind_pos = first.pos;
                im.spec = first.text.clone();
                while self.at(Kind::Dot) && self.kind_after() == Kind::Name {
                    self.next();
                    im.spec.push('.');
                    im.spec.push_str(&self.next().text);
                }
            }
            Kind::String => {
                let st = self.next();
                let text = plain_text(st, "a plain path string after `import`")?;
                im.bind_pos = st.pos;
                im.bind_len = crate::diag::quote(&text).chars().count();
                im.spec = text;
                im.is_path = true;
            }
            _ => {
                return Err(self.expected("a module name or a path string after `import`", None));
            }
        }
        if !im.is_path {
            im.bind_len = im.spec.chars().count();
        }
        if self.at(Kind::KwAs) {
            self.next();
            // `as _` binds nothing: the module could not be used (FE61)
            if self.at(Kind::Name) && self.peek().text == "_" {
                let mut e = self.expected("a name after `as`", Some(Kind::Name));
                e.help = Some(
                    "`as _` binds nothing, so the module could not be used; give it a name".into(),
                );
                return Err(e);
            }
            let alias = self.expect_desc(Kind::Name, "a name after `as`")?;
            im.bind_pos = alias.pos;
            im.alias = alias.text.clone();
            im.bind_len = im.alias.chars().count();
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
            // a trailing comma, as in every comma-separated list (SP8)
            if self.at(Kind::RParen) {
                break;
            }
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
