//! String, raw string and symbol literals.

use super::{Kind, Lexer, Part, is_hex_digit, is_ident_part, is_ident_start};
use crate::diag::Diag;
use crate::source::Pos;
use crate::text::Unit;

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
                });
            }
        };
        loop {
            match self.peek() {
                None | Some('\n') => return Err(self.err_no_arg("E0104", p)),
                Some('"') => {
                    self.advance();
                    flush(&mut parts, &mut text);
                    let raw = self.src[start..self.off].to_string();
                    let mut tok = self.token(Kind::String, &raw, p);
                    tok.off = self.base + start;
                    tok.parts = parts;
                    self.toks.push(tok);
                    return Ok(());
                }
                Some('\\') => match self.escape()? {
                    Unit::Char(c) => text.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
                    Unit::Byte(b) => text.push(b),
                },
                Some('$') if self.peek_at(1) == Some('{') => {
                    self.advance();
                    self.advance();
                    flush(&mut parts, &mut text);
                    let code_pos = self.pos();
                    let code_start = self.off;
                    self.skip_interpolation(p)?;
                    parts.push(Part {
                        is_code: true,
                        text: self.src[code_start..self.off - 1].to_string(),
                        bytes: None,
                        pos: code_pos,
                        off: self.base + code_start,
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
    /// is one byte, whatever it spells (L4).
    fn escape(&mut self) -> Result<Unit, Diag> {
        let p = self.pos();
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
                    _ => Err(self.err("E0103", p, &format!("x{hex}"))),
                };
            }
            'u' => {
                if self.peek() != Some('{') {
                    return Err(self.err("E0103", p, "u"));
                }
                self.advance();
                let hex_start = self.off;
                while is_hex_digit(self.peek()) {
                    self.advance();
                }
                let hex = self.src[hex_start..self.off].to_string();
                if self.peek() != Some('}') || hex.is_empty() || hex.len() > 6 {
                    return Err(self.err("E0103", p, &format!("u{{{hex}")));
                }
                self.advance();
                // char::from_u32 rejects values above 0x10FFFF and surrogates.
                return u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .map(Unit::Char)
                    .ok_or_else(|| self.err("E0103", p, &format!("u{{{hex}}}")));
            }
            _ => {}
        }
        Err(self.err("E0103", p, &c.to_string()))
    }

    /// Consumes the code of a `${...}` up to and including its closing brace,
    /// tracking nested braces and skipping over nested string literals so a
    /// quote inside them does not end the outer string. The code is inside
    /// the quotes, so a newline there is E0104 like anywhere in them (01).
    fn skip_interpolation(&mut self, str_pos: Pos) -> Result<(), Diag> {
        let mut depth = 1;
        loop {
            match self.peek() {
                None | Some('\n') => return Err(self.err_no_arg("E0104", str_pos)),
                Some('{') => {
                    depth += 1;
                    self.advance();
                }
                Some('}') => {
                    depth -= 1;
                    self.advance();
                    if depth == 0 {
                        return Ok(());
                    }
                }
                Some('"') => self.skip_quoted(str_pos)?,
                Some('`') => {
                    self.advance();
                    while self.peek() != Some('`') {
                        if matches!(self.peek(), None | Some('\n')) {
                            return Err(self.err_no_arg("E0104", str_pos));
                        }
                        self.advance();
                    }
                    self.advance();
                }
                Some(_) => {
                    self.advance();
                }
            }
        }
    }

    /// Skips a nested `"..."` literal inside an interpolation, honoring
    /// escapes and its own `${}` parts.
    fn skip_quoted(&mut self, str_pos: Pos) -> Result<(), Diag> {
        self.advance(); // opening quote
        loop {
            match self.peek() {
                None | Some('\n') => return Err(self.err_no_arg("E0104", str_pos)),
                Some('\\') => {
                    self.advance();
                    self.advance();
                }
                Some('"') => {
                    self.advance();
                    return Ok(());
                }
                Some('$') => {
                    self.advance();
                    if self.peek() == Some('{') {
                        self.advance();
                        self.skip_interpolation(str_pos)?;
                    }
                }
                Some(_) => {
                    self.advance();
                }
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
        let t = one(r#""x = ${x + 1}, m = ${m["k"]}!""#);
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
            assert_eq!((got.is_code, got.text.as_str()), (*code, *text), "part {i}");
            if *code {
                assert_eq!((got.pos.line, got.pos.col), (1, *col), "part {i}");
            }
        }
        // Off points at the code inside the whole source.
        assert_eq!(t.parts[1].off, 7);
    }

    #[test]
    fn nested_braces_inside_interpolation() {
        let t = one(r#""${ {a: 1}.a }""#);
        assert_eq!(t.parts.len(), 1);
        assert_eq!(t.parts[0].text, " {a: 1}.a ");
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
        assert_eq!(lex_src(r#""\u{zz}""#).unwrap_err().args, s("u{"));
        assert_eq!(lex_src(r#""\u""#).unwrap_err().args, s("u"));
        assert_eq!(lex_src(r#""\u{110000}""#).unwrap_err().args, s("u{110000}"));
        assert_eq!(
            lex_src(r#""\u{1234567}""#).unwrap_err().args,
            s("u{1234567")
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
