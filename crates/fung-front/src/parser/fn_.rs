//! Functions: `fn` definitions, parameter lists, type annotations and
//! lambdas.

use std::collections::HashSet;

use super::{MAX_DEPTH, PResult, Parser, describe, err, s};
use crate::lexer::{Kind, Token};
use crate::source::Pos;
use crate::tree::{FnInfo, Head, Lit, Node, Param, TypeTerm};

fn param(name: String, pos: Pos) -> Param {
    Param {
        name,
        pos,
        types: None,
        has_default: false,
        rest: false,
        pattern: None,
    }
}

fn pattern_param(pat: Node) -> Param {
    let pos = pat.pos;
    Param {
        pattern: Some(Box::new(pat)),
        ..param(String::new(), pos)
    }
}

/// E0119 when `name` is already bound by an earlier parameter, plain or
/// inside a pattern (03); a pattern binding a name twice by itself is the
/// resolver's E0221.
fn note_param(seen: &mut HashSet<String>, name: &str, pos: Pos) -> PResult<()> {
    if !seen.insert(name.to_string()) {
        let n = name.chars().count();
        return Err(Box::new(
            crate::diag::error("E0119", pos, vec![s(name)]).span(n),
        ));
    }
    Ok(())
}

/// Records every name a pattern parameter binds (`note_param`).
fn note_pattern(seen: &mut HashSet<String>, pat: &Node) -> PResult<()> {
    // each name once: a pattern binding a name twice by itself is E0221
    let mut names: Vec<(String, Pos)> = Vec::new();
    let mut own: HashSet<String> = HashSet::new();
    pat.bound_names(&mut |name, pos| {
        if own.insert(name.to_string()) {
            names.push((name.to_string(), pos));
        }
    });
    for (name, pos) in names {
        note_param(seen, &name, pos)?;
    }
    Ok(())
}

fn fn_info(name: String, name_pos: Pos) -> FnInfo {
    FnInfo {
        name,
        name_pos,
        params: Vec::new(),
        ret: None,
        for_body: false,
        lambda: false,
        num_locals: 0,
        captures: Vec::new(),
    }
}

impl<'a> Parser<'a> {
    /// Parses `fn NAME(params) [-> type] (= expr | block)` after `fn` (and
    /// the name, for a statement) has been consumed. Anonymous functions
    /// (name "") only have the block form (01).
    pub(super) fn fn_def(
        &mut self,
        fn_tok: &'a Token,
        name: String,
        name_pos: Pos,
    ) -> PResult<Node> {
        // a `fn` statement is a block like `if`'s, nested or not (FE60);
        // an anonymous `fn` is an expression, counted there already
        if name.is_empty() {
            return self.fn_def_inner(fn_tok, name, name_pos);
        }
        if self.depth >= MAX_DEPTH {
            let pos = self.peek().pos;
            return Err(err(
                "E0115",
                pos,
                vec![crate::diag::Arg::Int(MAX_DEPTH as i64)],
            ));
        }
        self.depth += 1;
        let r = self.fn_def_inner(fn_tok, name, name_pos);
        self.depth -= 1;
        r
    }

    fn fn_def_inner(&mut self, fn_tok: &'a Token, name: String, name_pos: Pos) -> PResult<Node> {
        let named = !name.is_empty();
        let mut info = fn_info(name, name_pos);
        self.expect_desc(Kind::LParen, "`(` after fn")?;
        let mut kids = self.params(&mut info)?;
        if self.at(Kind::Arrow) {
            self.next();
            self.skip_newlines_after_operator();
            info.ret = Some(self.type_spec()?);
        }
        let body = if self.at(Kind::Assign) && named {
            let eq = self.next();
            self.skip_newlines_after_operator();
            // the body is the function's end: it may be a `return` (D50)
            self.return_ok = true;
            let e = self.expr(0)?;
            Node::new(Head::Block, eq.pos, vec![e])
        } else if self.at(Kind::Colon) {
            self.next();
            let body = self.body(fn_tok.pos)?;
            self.close("function", fn_tok)?;
            body
        } else if self.at(Kind::Assign) {
            let pos = self.peek().pos;
            return Err(err("E0124", pos, Vec::new()));
        } else {
            return Err(self.unexpected());
        };
        kids.push(body);
        Ok(Node::with_lit(
            Head::Fn,
            fn_tok.pos,
            Lit::Fn(Box::new(info)),
            kids,
        ))
    }

    /// Parses the parameter list after `(` up to and including `)`,
    /// returning the default expressions in parameter order (03: 필수 →
    /// 기본값 → 마지막에 ...rest 하나).
    fn params(&mut self, info: &mut FnInfo) -> PResult<Vec<Node>> {
        let open = self.toks[self.last].pos; // the `(` just read
        self.nest += 1;
        let r = self.params_inner(info, open);
        self.nest -= 1;
        r
    }

    fn params_inner(&mut self, info: &mut FnInfo, open: Pos) -> PResult<Vec<Node>> {
        let mut defaults = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut saw_default = false;
        let mut rest_name: Option<&str> = None;
        while !self.at(Kind::RParen) {
            if let Some(r) = rest_name {
                let t = self.peek();
                let mut e = err(
                    "E0112",
                    t.pos,
                    vec![s("`)` after the rest parameter"), s(describe(t))],
                );
                e.help = Some(format!(
                    "the rest parameter `...{r}` takes the surplus arguments, so it comes last"
                ));
                return Err(e);
            }
            let mut rest = false;
            if self.at(Kind::Ellipsis) {
                self.next();
                rest = true;
            }
            if !rest && self.pattern_param_ahead() {
                // A destructuring parameter (03): no type, no default.
                let pat = self.destructure()?;
                if self.at(Kind::Colon) || self.at(Kind::Assign) {
                    let mut e = self.list_end(open, Kind::RParen);
                    e.help = Some("a destructuring parameter takes no type and no default".into());
                    return Err(e);
                }
                if saw_default {
                    return Err(err("E0120", pat.pos, Vec::new()));
                }
                note_pattern(&mut seen, &pat)?;
                info.params.push(pattern_param(pat));
                if self.at(Kind::Comma) {
                    self.next();
                    continue;
                }
                if !self.at(Kind::RParen) {
                    return Err(self.list_end(open, Kind::RParen));
                }
                continue;
            }
            let name = self.expect_desc(Kind::Name, "a parameter name")?;
            if rest {
                rest_name = Some(&name.text);
            }
            note_param(&mut seen, &name.text, name.pos)?;
            let mut prm = param(name.text.clone(), name.pos);
            prm.rest = rest;
            if self.at(Kind::Colon) {
                self.next();
                prm.types = Some(self.type_spec()?);
            }
            if self.at(Kind::Assign) {
                if prm.rest {
                    let mut e = self.list_end(open, Kind::RParen);
                    e.help = Some(
                        "a rest parameter takes no default: it is `[]` when no arguments are left"
                            .into(),
                    );
                    return Err(e);
                }
                self.next();
                // a `return` here is read so resolve names the rule (E0211)
                self.return_ok = true;
                defaults.push(self.expr(0)?);
                prm.has_default = true;
                saw_default = true;
            } else if saw_default && !prm.rest {
                return Err(err("E0120", name.pos, Vec::new()));
            }
            info.params.push(prm);
            if self.at(Kind::Comma) {
                self.next();
                continue;
            }
            if !self.at(Kind::RParen) {
                return Err(self.list_end(open, Kind::RParen));
            }
        }
        self.next(); // )
        Ok(defaults)
    }

    /// Parses `term { | term }` where term = NAME | TAG (07). A type has
    /// no element type: it would not be checked (D52, E0130).
    pub(super) fn type_spec(&mut self) -> PResult<Vec<TypeTerm>> {
        let mut alts = Vec::new();
        loop {
            alts.push(self.type_term()?);
            if !self.at(Kind::Bar) {
                return Ok(alts);
            }
            self.next();
            // a line ending in `|` continues (01 rule 1, FE7)
            self.skip_newlines_after_operator();
        }
    }

    /// One term of a type.
    fn type_term(&mut self) -> PResult<TypeTerm> {
        let t = self.peek();
        // `nil` and `fn` are keywords but also type names (07).
        if !matches!(t.kind, Kind::Name | Kind::Tag | Kind::KwNil | Kind::KwFn) {
            return Err(err("E0110", t.pos, vec![s(describe(t))]));
        }
        self.next();
        if self.toks[self.i].kind == Kind::LBracket {
            let pos = self.toks[self.i].pos;
            return Err(err("E0130", pos, Vec::new()));
        }
        Ok(TypeTerm::plain(&t.text, t.pos))
    }

    /// Whether a parameter starts like a pattern rather than a name: a tuple,
    /// list, map, tag, or `_`.
    fn pattern_param_ahead(&mut self) -> bool {
        let t = self.peek();
        match t.kind {
            Kind::LParen | Kind::LBracket | Kind::LBrace | Kind::Tag => true,
            Kind::Name => t.text == "_",
            _ => false,
        }
    }

    /// Whether the tokens at the current index form `( … ) =>` with balanced
    /// brackets, without consuming anything. The contents are parsed as
    /// patterns by `lambda` (03). Newlines inside the parentheses are
    /// whitespace, and before `=>` too when the lambda itself sits inside
    /// brackets (01).
    pub(super) fn lambda_params_ahead(&self) -> bool {
        if self.toks[self.i].kind != Kind::LParen {
            return false;
        }
        match self.pairs[self.i] {
            usize::MAX => false,
            // the `=>` is on the line of the `)` (D85)
            j => self.toks[j + 1].kind == Kind::FatArrow,
        }
    }

    /// Parses `NAME => expr` or `( patterns ) => expr`. The body is one
    /// expression read as far as possible (01).
    pub(super) fn lambda(&mut self) -> PResult<Node> {
        let start = self.peek();
        let mut info = fn_info(String::new(), start.pos);
        info.lambda = true;
        if start.kind == Kind::Name {
            self.next();
            if start.text == "_" {
                let wild = Node::new(Head::Pwild, start.pos, Vec::new());
                info.params.push(pattern_param(wild));
            } else {
                info.params.push(param(start.text.clone(), start.pos));
            }
        } else {
            let open = self.next().pos; // (
            self.nest += 1;
            let r = self.lambda_params(&mut info, open);
            self.nest -= 1;
            r?;
            self.next(); // )
        }
        let arrow = self.next(); // =>
        self.skip_newlines_after_operator();
        // a `return` here is read so resolve names the rule (E0237, D42)
        self.return_ok = true;
        let body = self.expr(0)?;
        Ok(Node::with_lit(
            Head::Fn,
            start.pos,
            Lit::Fn(Box::new(info)),
            vec![Node::new(Head::Block, arrow.pos, vec![body])],
        ))
    }

    fn lambda_params(&mut self, info: &mut FnInfo, open: Pos) -> PResult<()> {
        let mut seen = HashSet::new();
        while !self.at(Kind::RParen) {
            // The same parameters as `fn` (SP8): a name or a destructuring
            // pattern, not a literal (FE32).
            let pat = self.destructure()?;
            note_pattern(&mut seen, &pat)?;
            if pat.head == Head::Pbind
                && let Lit::Str(name) = &pat.lit
            {
                info.params.push(param(name.clone(), pat.pos));
            } else {
                info.params.push(pattern_param(pat));
            }
            if self.at(Kind::Comma) {
                self.next();
                continue;
            }
            if !self.at(Kind::RParen) {
                return Err(self.list_end(open, Kind::RParen));
            }
        }
        Ok(())
    }
}
