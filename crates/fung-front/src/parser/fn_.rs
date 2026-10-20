//! Functions: `fn` definitions, parameter lists, type annotations and
//! lambdas.

use super::{PResult, Parser, describe, err, s};
use crate::lexer::{Kind, Token};
use crate::source::Pos;
use crate::tree::{FnInfo, Head, Lit, Node, Param};

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

/// The names a parameter pattern binds, each once, with where it is bound.
fn pattern_names(n: &Node, out: &mut Vec<(String, Pos)>) {
    if matches!(n.head, Head::Pbind | Head::Prest)
        && let Lit::Str(name) = &n.lit
        && !out.iter().any(|(m, _)| m == name)
    {
        out.push((name.clone(), n.pos));
    }
    for k in &n.kids {
        pattern_names(k, out);
    }
}

/// E0110 when `name` is already bound by an earlier parameter, plain or
/// inside a pattern (03); a pattern binding a name twice by itself is the
/// resolver's E0221.
fn note_param(seen: &mut Vec<String>, name: &str, pos: Pos) -> PResult<()> {
    if seen.iter().any(|m| m == name) {
        return Err(err(
            "E0110",
            pos,
            vec![s(format!("duplicate parameter `{name}`"))],
        ));
    }
    seen.push(name.to_string());
    Ok(())
}

/// Records every name a pattern parameter binds (`note_param`).
fn note_pattern(seen: &mut Vec<String>, pat: &Node) -> PResult<()> {
    let mut names = Vec::new();
    pattern_names(pat, &mut names);
    for (name, pos) in names {
        note_param(seen, &name, pos)?;
    }
    Ok(())
}

fn fn_info(name: String) -> FnInfo {
    FnInfo {
        name,
        params: Vec::new(),
        ret: None,
        for_body: false,
        expr_body: false,
        num_locals: 0,
        captures: Vec::new(),
    }
}

impl<'a> Parser<'a> {
    /// Parses `fn NAME(params) [-> type] (= expr | block)` after `fn` (and
    /// the name, for a statement) has been consumed. Anonymous functions
    /// (name "") only have the block form (01).
    pub(super) fn fn_def(&mut self, fn_tok: &'a Token, name: String) -> PResult<Node> {
        let named = !name.is_empty();
        let mut info = fn_info(name);
        self.expect_desc(Kind::LParen, "`(` after fn")?;
        let mut kids = self.params(&mut info)?;
        if self.at(Kind::Arrow) {
            self.next();
            self.skip_newlines_after_operator();
            info.ret = Some(self.type_spec()?);
        }
        let body = if self.at(Kind::Assign) && named {
            info.expr_body = true;
            let eq = self.next();
            self.skip_newlines_after_operator();
            let e = self.expr(0)?;
            Node::new(Head::Block, eq.pos, vec![e])
        } else if self.at(Kind::Colon) {
            self.next();
            let body = self.body(fn_tok.pos)?;
            self.expect_desc(Kind::DoubleSemi, "`;;` to close the function")?;
            body
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
        self.nest += 1;
        let r = self.params_inner(info);
        self.nest -= 1;
        r
    }

    fn params_inner(&mut self, info: &mut FnInfo) -> PResult<Vec<Node>> {
        let mut defaults = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        let (mut saw_default, mut saw_rest) = (false, false);
        while !self.at(Kind::RParen) {
            if saw_rest {
                return Err(self.unexpected());
            }
            let mut rest = false;
            if self.at(Kind::Ellipsis) {
                self.next();
                rest = true;
                saw_rest = true;
            }
            if !rest && self.pattern_param_ahead() {
                // A destructuring parameter (03): no type, no default.
                let pat = self.pattern()?;
                if self.at(Kind::Colon) || self.at(Kind::Assign) {
                    return Err(self.unexpected());
                }
                if saw_default {
                    return Err(err(
                        "E0110",
                        pat.pos,
                        vec![s("required parameter after an optional one")],
                    ));
                }
                note_pattern(&mut seen, &pat)?;
                info.params.push(pattern_param(pat));
                if self.at(Kind::Comma) {
                    self.next();
                    continue;
                }
                if !self.at(Kind::RParen) {
                    return Err(self.unexpected());
                }
                continue;
            }
            let name = self.expect_desc(Kind::Name, "a parameter name")?;
            note_param(&mut seen, &name.text, name.pos)?;
            let mut prm = param(name.text.clone(), name.pos);
            prm.rest = rest;
            if self.at(Kind::Colon) {
                self.next();
                prm.types = Some(self.type_spec()?);
            }
            if self.at(Kind::Assign) {
                if prm.rest {
                    return Err(self.unexpected());
                }
                self.next();
                defaults.push(self.expr(0)?);
                prm.has_default = true;
                saw_default = true;
            } else if saw_default && !prm.rest {
                return Err(err(
                    "E0110",
                    name.pos,
                    vec![s(format!(
                        "required parameter `{}` after an optional one",
                        name.text
                    ))],
                ));
            }
            info.params.push(prm);
            if self.at(Kind::Comma) {
                self.next();
                continue;
            }
            if !self.at(Kind::RParen) {
                return Err(self.unexpected());
            }
        }
        self.next(); // )
        Ok(defaults)
    }

    /// Parses `term { | term }` where term = NAME [ "[" type "]" ] | TAG.
    /// Element types are parsed and dropped (07: 런타임은 바깥 타입만 검사).
    pub(super) fn type_spec(&mut self) -> PResult<Vec<String>> {
        let mut alts = Vec::new();
        loop {
            let t = self.peek();
            // `nil` and `fn` are keywords but also type names (07).
            if !matches!(t.kind, Kind::Name | Kind::Tag | Kind::KwNil | Kind::KwFn) {
                return Err(err("E0110", t.pos, vec![s(describe(t))]));
            }
            self.next();
            alts.push(t.text.clone());
            if self.at(Kind::LBracket) {
                self.next();
                self.type_spec()?;
                self.expect_desc(Kind::RBracket, "`]`")?;
            }
            if !self.at(Kind::Bar) {
                return Ok(alts);
            }
            self.next();
        }
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
        let toks = self.toks;
        if toks[self.i].kind != Kind::LParen {
            return false;
        }
        let mut depth = 0usize;
        for (j, t) in toks.iter().enumerate().skip(self.i) {
            match t.kind {
                Kind::LParen | Kind::LBracket | Kind::LBrace => depth += 1,
                Kind::RParen | Kind::RBracket | Kind::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        return self.kind_from(j + 1) == Kind::FatArrow;
                    }
                }
                Kind::Eof => return false,
                _ => {}
            }
        }
        false
    }

    /// Parses `NAME => expr` or `( patterns ) => expr`. The body is one
    /// expression read as far as possible (01).
    pub(super) fn lambda(&mut self) -> PResult<Node> {
        let start = self.peek();
        let mut info = fn_info(String::new());
        if start.kind == Kind::Name {
            self.next();
            if start.text == "_" {
                let wild = Node::new(Head::Pwild, start.pos, Vec::new());
                info.params.push(pattern_param(wild));
            } else {
                info.params.push(param(start.text.clone(), start.pos));
            }
        } else {
            self.next(); // (
            self.nest += 1;
            let r = self.lambda_params(&mut info);
            self.nest -= 1;
            r?;
            self.next(); // )
        }
        let arrow = self.next(); // =>
        info.expr_body = true;
        self.skip_newlines_after_operator();
        let body = self.expr(0)?;
        Ok(Node::with_lit(
            Head::Fn,
            start.pos,
            Lit::Fn(Box::new(info)),
            vec![Node::new(Head::Block, arrow.pos, vec![body])],
        ))
    }

    fn lambda_params(&mut self, info: &mut FnInfo) -> PResult<()> {
        let mut seen = Vec::new();
        while !self.at(Kind::RParen) {
            let pat = self.pattern()?;
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
                return Err(self.unexpected());
            }
        }
        Ok(())
    }
}
