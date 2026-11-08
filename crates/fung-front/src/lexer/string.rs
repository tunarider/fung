//! String, raw string and symbol literals.

use super::{Kind, Lexer, Part, Token, is_hex_digit, is_ident_part, is_ident_start};
use crate::diag::{Arg, Diag};
use crate::source::Pos;
use crate::text::Unit;

/// The text a string token keeps for messages: its source, cut short
/// when it interpolates and is long, so nested strings do not each copy
/// the text of all those inside them (FE16).
fn shown(raw: &str, parts: &[Part]) -> String {
    const MAX: usize = 60;
    if !parts.iter().any(|p| p.is_code) || raw.chars().count() <= MAX {
        return raw.to_string();
    }
    let mut s: String = raw.chars().take(MAX).collect();
    s.push('…');
    s
}

impl Lexer<'_> {
    /// Lexes a `"..."` string into text and `${code}` parts. Escapes are a
    /// closed list; anything else is E0103. A literal newline inside the
    /// quotes is E0104 (use a raw string or `\n`).
    pub(super) fn str(&mut self) -> Result<(), Diag> {
        let p = self.pos();
        let start = self.off;
        self.advance(); // opening quote
        let mut parts: Vec<Part> = Vec::new();
        // Bytes, not chars: `\xHH` may spell a string that is not UTF-8.
        let mut text: Vec<u8> = Vec::new();
        let flush = |parts: &mut Vec<Part>, text: &mut Vec<u8>| {
            if !text.is_empty() {
                let (text, bytes) = match String::from_utf8(std::mem::take(text)) {
                    Ok(s) => (s, None),
                    Err(e) => (String::new(), Some(e.into_bytes())),
                };
                parts.push(Part {
                    is_code: false,
                    text,
                    bytes,
                    pos: Pos {
                        file: p.file,
                        line: 0,
                        col: 0,
                    },
                    off: 0,
                    end: 0,
                    toks: Vec::new(),
                });
            }
        };
        loop {
            match self.peek() {
                None | Some('\n') => return Err(self.err_no_arg("E0104", p)),
                Some('"') => {
                    self.advance();
                    flush(&mut parts, &mut text);
                    let raw = shown(&self.src[start..self.off], &parts);
                    let mut tok = self.token(Kind::String, &raw, p);
                    tok.off = self.base + start;
                    tok.parts = parts;
                    self.toks.push(tok);
                    return Ok(());
                }
                Some('\\') => match self.escape(p)? {
                    Unit::Char(c) => text.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
                    Unit::Byte(b) => text.push(b),
                },
                Some('$') if self.peek_at(1) == Some('{') => {
                    self.advance();
                    self.advance();
                    flush(&mut parts, &mut text);
                    let code_pos = self.pos();
                    let code_start = self.off;
                    let toks = self.interpolation(p)?;
                    parts.push(Part {
                        is_code: true,
                        text: String::new(),
                        bytes: None,
                        pos: code_pos,
                        off: self.base + code_start,
                        end: self.base + self.off - 1,
                        toks,
                    });
                }
                Some(_) => {
                    let c = self.advance();
                    text.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
                }
            }
        }
    }

    fn err_no_arg(&self, code: &str, pos: Pos) -> Diag {
        crate::diag::error(code, pos, Vec::new())
    }

    /// Decodes one backslash escape; `off` is at the backslash. `\xHH`
    /// is one byte, whatever it spells (L4). A backslash before the end
    /// of the line or input escapes nothing: the string that began at
    /// `str_pos` is unterminated (FE14). An error shows the escape as
    /// written, quoted.
    fn escape(&mut self, str_pos: Pos) -> Result<Unit, Diag> {
        let p = self.pos();
        if matches!(self.peek_at(1), None | Some('\n'))
            || (self.peek_at(1) == Some('\r') && self.peek_at(2) == Some('\n'))
        {
            return Err(self.err_no_arg("E0104", str_pos));
        }
        self.advance(); // backslash
        let c = self.advance();
        let ch = |c| Ok(Unit::Char(c));
        match c {
            'n' => return ch('\n'),
            't' => return ch('\t'),
            'r' => return ch('\r'),
            '\\' => return ch('\\'),
            '"' => return ch('"'),
            '$' => return ch('$'),
            'x' => {
                let hex: String = (0..2)
                    .map_while(|_| is_hex_digit(self.peek()).then(|| self.advance()))
                    .collect();
                return match u8::from_str_radix(&hex, 16) {
                    Ok(b) if hex.len() == 2 => Ok(Unit::Byte(b)),
                    _ => Err(self.err("E0103", p, &format!("\\x{hex}"))),
                };
            }
            'u' => {
                if self.peek() != Some('{') {
                    return Err(self.err("E0103", p, "\\u"));
                }
                self.advance();
                let hex_start = self.off;
                while is_hex_digit(self.peek()) {
                    self.advance();
                }
                let hex = self.src[hex_start..self.off].to_string();
                if self.peek() != Some('}') || hex.is_empty() || hex.len() > 6 {
                    return Err(self.err("E0103", p, &format!("\\u{{{hex}")));
                }
                self.advance();
                // char::from_u32 rejects values above 0x10FFFF and surrogates.
                return u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .map(Unit::Char)
                    .ok_or_else(|| self.err("E0103", p, &format!("\\u{{{hex}}}")));
            }
            _ => {}
        }
        Err(self.err("E0103", p, &format!("\\{c}")))
    }

    /// Lexes the code of a `${...}` after the `${`, up to and including
    /// its closing brace, into its own tokens (ending in an `Eof` at the
    /// brace). Nested strings are lexed by `str` as they come, so every
    /// character is read once however deep the nesting (FE16); the depth
    /// is bounded like the parser's (E0115). The code is inside the
    /// quotes of the string that began at `str_pos`, so a newline there is
    /// E0104 like anywhere in them (01).
    fn interpolation(&mut self, str_pos: Pos) -> Result<Vec<Token>, Diag> {
        if self.interp_depth >= crate::parser::MAX_DEPTH {
            return Err(crate::diag::error(
                "E0115",
                str_pos,
                vec![Arg::Int(crate::parser::MAX_DEPTH as i64)],
            ));
        }
        self.interp_depth += 1;
        let outer = std::mem::take(&mut self.toks);
        let r = self.interpolation_tokens(str_pos);
        let inner = std::mem::replace(&mut self.toks, outer);
        self.interp_depth -= 1;
        r.map(|()| inner)
    }

    fn interpolation_tokens(&mut self, str_pos: Pos) -> Result<(), Diag> {
        let mut depth = 0usize;
        loop {
            match self.peek() {
                None | Some('\n') => return Err(self.err_no_arg("E0104", str_pos)),
                Some('\r') if self.peek_at(1) == Some('\n') => {
                    return Err(self.err_no_arg("E0104", str_pos));
                }
                Some('}') if depth == 0 => {
                    self.start = self.off;
                    let p = self.pos();
                    self.emit(Kind::Eof, "", p).end = self.base + self.off;
                    self.advance();
                    return Ok(());
                }
                Some('{') => {
                    depth += 1;
                    self.step()?;
                }
                Some('}') => {
                    depth -= 1;
                    self.step()?;
                }
                Some('`') => {
                    self.step()?;
                    let raw = self.toks.last().expect("a raw string token");
                    if raw.text.contains('\n') {
                        return Err(self.err_no_arg("E0104", str_pos));
                    }
                }
                Some(_) => self.step()?,
            }
        }
    }

    /// Lexes a `` `...` `` string: no escapes, no interpolation, newlines allowed.
    pub(super) fn raw_str(&mut self) -> Result<(), Diag> {
        let p = self.pos();
        self.advance();
        let start = self.off;
        while self.peek() != Some('`') {
            if self.peek().is_none() {
                return Err(self.err_no_arg("E0104", p));
            }
            self.advance();
        }
        let text = self.src[start..self.off].to_string();
        self.advance();
        self.emit(Kind::RawString, &text, p);
        Ok(())
    }

    /// Lexes `'name`.
    pub(super) fn symbol(&mut self) -> Result<(), Diag> {
        let p = self.pos();
        self.advance();
        if !is_ident_start(self.peek()) {
            return Err(self.err("E0101", p, "'"));
        }
        let start = self.off;
        while is_ident_part(self.peek()) {
            self.advance();
        }
        // A symbol is a name: its text is the NFC form (01).
        let text = crate::unicode::nfc(&self.src[start..self.off]).into_owned();
        self.emit(Kind::Symbol, &text, p);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::lexer::tests::lex_src;
    use crate::lexer::{Kind, Part, Token};

    fn one(src: &str) -> Token {
        let (mut toks, _) = lex_src(src).unwrap_or_else(|d| panic!("lex {src:?}: {}", d.code));
        assert_eq!(toks.len(), 2, "{src:?} should be one token + EOF");
        toks.remove(0)
    }

    #[test]
    fn plain_string_and_escapes() {
        let t = one(r#""a\tb\n\\\"\$\u{1F600}""#);
        assert_eq!(t.kind, Kind::String);
        assert_eq!(t.parts.len(), 1);
        assert!(!t.parts[0].is_code);
        assert_eq!(t.parts[0].text, "a\tb\n\\\"$\u{1F600}");
    }

    #[test]
    fn interpolation_parts() {
        let src = r#""x = ${x + 1}, m = ${m["k"]}!""#;
        let t = one(src);
        let want: [(bool, &str, u32); 5] = [
            (false, "x = ", 0),
            (true, "x + 1", 8),
            (false, ", m = ", 0),
            (true, r#"m["k"]"#, 22),
            (false, "!", 0),
        ];
        assert_eq!(t.parts.len(), want.len(), "{:?}", t.parts);
        for (i, (code, text, col)) in want.iter().enumerate() {
            let got: &Part = &t.parts[i];
            let shown = if got.is_code {
                &src[got.off..got.end]
            } else {
                got.text.as_str()
            };
            assert_eq!((got.is_code, shown), (*code, *text), "part {i}");
            if *code {
                assert_eq!((got.pos.line, got.pos.col), (1, *col), "part {i}");
            }
        }
        // Off points at the code inside the whole source, and the code's
        // tokens are there, ending at the closing brace.
        assert_eq!(t.parts[1].off, 7);
        let kinds: Vec<Kind> = t.parts[1].toks.iter().map(|t| t.kind).collect();
        assert_eq!(kinds, [Kind::Name, Kind::Plus, Kind::Int, Kind::Eof]);
        assert_eq!(t.parts[1].toks[3].off, 12);
    }

    #[test]
    fn nested_braces_inside_interpolation() {
        let src = r#""${ {a: 1}.a }""#;
        let t = one(src);
        assert_eq!(t.parts.len(), 1);
        assert_eq!(&src[t.parts[0].off..t.parts[0].end], " {a: 1}.a ");
    }

    #[test]
    fn empty_string() {
        let t = one(r#""""#);
        assert_eq!(t.kind, Kind::String);
        assert!(t.parts.is_empty());
    }

    #[test]
    fn raw_string() {
        let t = one("`C:\\new\nline ${x}`");
        assert_eq!(t.kind, Kind::RawString);
        assert_eq!(t.text, "C:\\new\nline ${x}");
        let (toks, _) = lex_src("`a\nb` c").unwrap();
        assert_eq!((toks[1].pos.line, toks[1].pos.col), (2, 4));
    }

    #[test]
    fn symbols() {
        let (toks, _) = lex_src("'ok 'not_found 'Red").unwrap();
        for (i, want) in ["ok", "not_found", "Red"].iter().enumerate() {
            assert_eq!((toks[i].kind, toks[i].text.as_str()), (Kind::Symbol, *want));
        }
    }

    #[test]
    fn string_errors() {
        let cases = [
            ("\"abc", "E0104"),
            ("\"a\nb\"", "E0104"),
            (r#""\q""#, "E0103"),
            (r#""\u{zz}""#, "E0103"),
            (r#""${x""#, "E0104"),
            // the code of `${…}` is inside the quotes too (01)
            ("\"a${1 +\n2}b\"", "E0104"),
            ("\"a${`x\ny`}b\"", "E0104"),
            // a comment runs to the line's end, over the closing quote
            ("\"${1 # x}\"", "E0104"),
            ("`abc", "E0104"),
            ("'", "E0101"),
            ("' x", "E0101"),
        ];
        for (src, want) in cases {
            match lex_src(src) {
                Err(d) => assert_eq!(d.code, want, "{src:?}"),
                Ok(_) => panic!("{src:?}: expected {want}"),
            }
        }
    }

    #[test]
    fn escape_error_arguments() {
        use crate::diag::Arg;
        let s = |x: &str| vec![Arg::Str(x.to_string())];
        assert_eq!(lex_src(r#""\u{zz}""#).unwrap_err().args, s("\\u{"));
        assert_eq!(lex_src(r#""\u""#).unwrap_err().args, s("\\u"));
        assert_eq!(
            lex_src(r#""\u{110000}""#).unwrap_err().args,
            s("\\u{110000}")
        );
        assert_eq!(
            lex_src(r#""\u{1234567}""#).unwrap_err().args,
            s("\\u{1234567")
        );
    }

    /// A backslash at the end of a line or of the input does not escape
    /// anything: the string is unterminated (FE14). An unknown escape
    /// shows the character quoted.
    #[test]
    fn backslash_at_line_or_input_end() {
        use crate::diag::message;
        for src in ["\"a\\\n\")", "\"a\\", "\"a\\\r\n\""] {
            assert_eq!(lex_src(src).unwrap_err().code, "E0104", "{src:?}");
        }
        let d = lex_src("\"\\\t\"").unwrap_err();
        assert_eq!(d.code, "E0103");
        assert_eq!(
            message(&d.code, &d.args),
            "unknown escape \"\\\\\\t\" in string"
        );
        let d = lex_src(r#""\q""#).unwrap_err();
        assert_eq!(
            message(&d.code, &d.args),
            r#"unknown escape "\\q" in string"#
        );
    }

    #[test]
    fn surrogate_escape_is_rejected() {
        for src in [r#""\u{D800}""#, r#""\u{DFFF}""#] {
            assert_eq!(lex_src(src).unwrap_err().code, "E0103", "{src}");
        }
        assert!(lex_src(r#""\u{E000}""#).is_ok());
    }
}
