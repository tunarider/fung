//! The lexer: turns fung source text into tokens. It implements the
//! character-level rules of the grammar: newlines are tokens that the parser
//! interprets, comments are kept aside as trivia for the formatter, and only
//! the first error is reported.

use crate::diag::{Arg, Diag, error};
use crate::source::{FileId, Pos};
use crate::unicode;

mod number;
mod string;

/// Classifies a token.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Eof,
    Newline,

    // Literals and names
    Int,
    Float,
    String,
    RawString,
    Duration,
    Symbol,
    Name,
    Tag,

    // Keywords
    KwFn,
    KwIf,
    KwElse,
    KwThen,
    KwMatch,
    KwWhen,
    KwFor,
    KwIn,
    KwNot,
    KwAnd,
    KwOr,
    KwReturn,
    KwImport,
    KwAs,
    KwType,
    KwTest,
    KwAssert,
    KwTrue,
    KwFalse,
    KwNil,

    // Punctuation and operators
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Semi,
    DoubleSemi,
    Dot,
    DotDot,
    DotDotEq,
    Ellipsis,
    FatArrow,
    Arrow,
    Assign,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Plus,
    Minus,
    Star,
    StarStar,
    Slash,
    SlashSlash,
    Percent,
    Pipe,
    Bar,
    Coalesce,
    Question,
    Bang,
}

const KEYWORDS: [(&str, Kind); 20] = [
    ("fn", Kind::KwFn),
    ("if", Kind::KwIf),
    ("else", Kind::KwElse),
    ("then", Kind::KwThen),
    ("match", Kind::KwMatch),
    ("when", Kind::KwWhen),
    ("for", Kind::KwFor),
    ("in", Kind::KwIn),
    ("not", Kind::KwNot),
    ("and", Kind::KwAnd),
    ("or", Kind::KwOr),
    ("return", Kind::KwReturn),
    ("import", Kind::KwImport),
    ("as", Kind::KwAs),
    ("type", Kind::KwType),
    ("test", Kind::KwTest),
    ("assert", Kind::KwAssert),
    ("true", Kind::KwTrue),
    ("false", Kind::KwFalse),
    ("nil", Kind::KwNil),
];

impl Kind {
    /// Reports a reserved word. Where only a name can appear (after `.`, as a
    /// map key) the parser accepts keywords as plain names.
    pub fn is_keyword(self) -> bool {
        (Kind::KwFn as u8..=Kind::KwNil as u8).contains(&(self as u8))
    }

    /// The reserved words and their kinds.
    pub fn keywords() -> &'static [(&'static str, Kind)] {
        &KEYWORDS
    }

    /// How the kind reads in an error message.
    pub fn describe(self) -> &'static str {
        use Kind::*;
        match self {
            Eof => "end of file",
            Newline => "newline",
            Int => "integer",
            Float => "float",
            String => "string",
            RawString => "raw string",
            Duration => "duration",
            Symbol => "symbol",
            Name => "name",
            Tag => "tag",
            KwFn => "fn",
            KwIf => "if",
            KwElse => "else",
            KwThen => "then",
            KwMatch => "match",
            KwWhen => "when",
            KwFor => "for",
            KwIn => "in",
            KwNot => "not",
            KwAnd => "and",
            KwOr => "or",
            KwReturn => "return",
            KwImport => "import",
            KwAs => "as",
            KwType => "type",
            KwTest => "test",
            KwAssert => "assert",
            KwTrue => "true",
            KwFalse => "false",
            KwNil => "nil",
            LParen => "(",
            RParen => ")",
            LBracket => "[",
            RBracket => "]",
            LBrace => "{",
            RBrace => "}",
            Comma => ",",
            Colon => ":",
            Semi => ";",
            DoubleSemi => ";;",
            Dot => ".",
            DotDot => "..",
            DotDotEq => "..=",
            Ellipsis => "...",
            FatArrow => "=>",
            Arrow => "->",
            Assign => "=",
            Eq => "==",
            Ne => "!=",
            Lt => "<",
            Gt => ">",
            Le => "<=",
            Ge => ">=",
            Plus => "+",
            Minus => "-",
            Star => "*",
            StarStar => "**",
            Slash => "/",
            SlashSlash => "//",
            Percent => "%",
            Pipe => "|>",
            Bar => "|",
            Coalesce => "??",
            Question => "?",
            Bang => "!",
        }
    }
}

/// One piece of a `"..."` string: literal text, or the source of an
/// interpolated `${...}` expression that the parser parses later.
#[derive(Clone, Debug)]
pub struct Part {
    pub is_code: bool,
    pub text: String,
    pub pos: Pos,
    /// Byte offset of `text` in the whole source (`lex_at` adds `base`).
    pub off: usize,
}

/// One lexical unit. `text` is the source slice (for names, tags and symbols
/// it is the identifier without any sigil). `int`, `float`, `dur` (nanoseconds)
/// and `parts` carry the decoded value of literals.
#[derive(Clone, Debug)]
pub struct Token {
    pub kind: Kind,
    pub text: String,
    pub pos: Pos,
    /// Byte offset of the token's first byte in the source.
    pub off: usize,
    /// Byte offset just past the token.
    pub end: usize,
    pub int: i64,
    pub float: f64,
    pub dur: i64,
    pub parts: Vec<Part>,
}

/// Trivia kept aside for the formatter.
#[derive(Clone, Debug)]
pub struct Comment {
    pub pos: Pos,
    pub text: String,
    /// Byte offset of the `#` in the source (the formatter maps it back
    /// to the source bytes).
    pub off: usize,
}

/// Lexes source text into tokens. The last token is always `Eof`. Only the
/// first error is reported. A leading byte order mark (U+FEFF) is skipped and
/// does not count as a column.
pub fn lex(file: FileId, src: &str) -> Result<(Vec<Token>, Vec<Comment>), Diag> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    lex_at(
        file,
        src,
        Pos {
            file,
            line: 1,
            col: 1,
        },
        0,
    )
}

/// Lexes text that sits inside a larger file, such as the code of a `${...}`
/// interpolation, so that positions refer to the enclosing file.
pub fn lex_at(
    file: FileId,
    src: &str,
    start: Pos,
    base: usize,
) -> Result<(Vec<Token>, Vec<Comment>), Diag> {
    let mut lx = Lexer {
        file,
        src,
        off: 0,
        start: 0,
        base,
        line: start.line,
        col: start.col,
        toks: Vec::new(),
        comments: Vec::new(),
    };
    lx.run()?;
    Ok((lx.toks, lx.comments))
}

struct Lexer<'a> {
    file: FileId,
    src: &'a str,
    /// Byte offset of the next char.
    off: usize,
    /// Byte offset where the token being lexed began.
    start: usize,
    /// Offset of `src` inside the whole file, added to spans.
    base: usize,
    line: u32,
    /// Char column of the next char, 1-based.
    col: u32,
    toks: Vec<Token>,
    comments: Vec<Comment>,
}

fn is_digit(c: Option<char>) -> bool {
    matches!(c, Some('0'..='9'))
}

fn is_ident_start(c: Option<char>) -> bool {
    c.is_some_and(|c| c == '_' || unicode::is_name_start(c))
}

fn is_ident_part(c: Option<char>) -> bool {
    c.is_some_and(unicode::is_name_continue)
}

fn is_hex_digit(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_ascii_hexdigit())
}

impl<'a> Lexer<'a> {
    fn pos(&self) -> Pos {
        Pos {
            file: self.file,
            line: self.line,
            col: self.col,
        }
    }

    fn err(&self, code: &str, pos: Pos, arg: &str) -> Diag {
        error(code, pos, vec![Arg::Str(arg.to_string())])
    }

    /// The next char without consuming it, or `None` at end of input.
    fn peek(&self) -> Option<char> {
        self.peek_at(0)
    }

    /// The n-th char ahead (0 = next), or `None` past the end.
    fn peek_at(&self, n: usize) -> Option<char> {
        self.src[self.off..].chars().nth(n)
    }

    /// Consumes one char. At end of input it returns U+FFFD (no char) and
    /// still bumps the column.
    fn advance(&mut self) -> char {
        let Some(c) = self.src[self.off..].chars().next() else {
            self.col += 1;
            return '\u{fffd}';
        };
        self.off += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        c
    }

    fn token(&self, kind: Kind, text: &str, pos: Pos) -> Token {
        Token {
            kind,
            text: text.to_string(),
            pos,
            off: self.base + self.start,
            end: self.base + self.off,
            int: 0,
            float: 0.0,
            dur: 0,
            parts: Vec::new(),
        }
    }

    fn emit(&mut self, kind: Kind, text: &str, pos: Pos) -> &mut Token {
        let t = self.token(kind, text, pos);
        self.toks.push(t);
        self.toks.last_mut().expect("just pushed")
    }

    fn run(&mut self) -> Result<(), Diag> {
        loop {
            let c = self.peek();
            self.start = self.off;
            match c {
                None => {
                    let p = self.pos();
                    self.emit(Kind::Eof, "", p);
                    return Ok(());
                }
                Some('\r') if self.peek_at(1) == Some('\n') => {
                    // Windows line ending: one newline token at the \r.
                    let p = self.pos();
                    self.advance();
                    self.advance();
                    self.emit(Kind::Newline, "\n", p);
                }
                Some(' ' | '\t' | '\r') => {
                    self.advance();
                }
                Some('\n') => {
                    let p = self.pos();
                    self.advance();
                    self.emit(Kind::Newline, "\n", p);
                }
                Some('#') => self.comment(),
                c if is_digit(c) => self.number()?,
                Some('"') => self.str()?,
                Some('`') => self.raw_str()?,
                Some('\'') => self.symbol()?,
                c if is_ident_start(c) => self.ident(),
                _ => self.punct()?,
            }
        }
    }

    fn comment(&mut self) {
        let p = self.pos();
        let start = self.off;
        while !matches!(self.peek(), None | Some('\n' | '\r')) {
            self.advance();
        }
        self.comments.push(Comment {
            pos: p,
            text: self.src[start..self.off].to_string(),
            off: self.base + start,
        });
    }

    fn ident(&mut self) {
        let p = self.pos();
        let start = self.off;
        let first = self.advance();
        while is_ident_part(self.peek()) {
            self.advance();
        }
        // A name's text is its NFC form (01); the source keeps its bytes.
        let text = unicode::nfc(&self.src[start..self.off]).into_owned();
        let kind = if let Some((_, k)) = KEYWORDS.iter().find(|(w, _)| *w == text) {
            *k
        } else if unicode::is_upper(text.chars().next().unwrap_or(first)) {
            Kind::Tag
        } else {
            Kind::Name
        };
        self.emit(kind, &text, p);
    }

    /// Tries the longest operator first so that `..=` wins over `..` and `;;`
    /// over `;` (the lexer always cuts `;;` as long as it can).
    fn punct(&mut self) -> Result<(), Diag> {
        let p = self.pos();
        let rest = &self.src[self.off..];
        let three: String = rest.chars().take(3).collect();
        let two: String = rest.chars().take(2).collect();
        let one: String = rest.chars().take(1).collect();
        let (kind, len) = if let Some(k) = punct3(&three) {
            (k, 3)
        } else if let Some(k) = punct2(&two) {
            (k, 2)
        } else if let Some(k) = punct1(&one) {
            (k, 1)
        } else {
            return Err(self.err("E0101", p, &one));
        };
        let text: String = rest.chars().take(len).collect();
        for _ in 0..len {
            self.advance();
        }
        self.emit(kind, &text, p);
        Ok(())
    }
}

fn punct3(s: &str) -> Option<Kind> {
    match s {
        "..=" => Some(Kind::DotDotEq),
        "..." => Some(Kind::Ellipsis),
        _ => None,
    }
}

fn punct2(s: &str) -> Option<Kind> {
    use Kind::*;
    Some(match s {
        "==" => Eq,
        "!=" => Ne,
        "<=" => Le,
        ">=" => Ge,
        "=>" => FatArrow,
        "->" => Arrow,
        "//" => SlashSlash,
        "**" => StarStar,
        "|>" => Pipe,
        "??" => Coalesce,
        ".." => DotDot,
        ";;" => DoubleSemi,
        _ => return None,
    })
}

fn punct1(s: &str) -> Option<Kind> {
    use Kind::*;
    Some(match s {
        "(" => LParen,
        ")" => RParen,
        "[" => LBracket,
        "]" => RBracket,
        "{" => LBrace,
        "}" => RBrace,
        "," => Comma,
        ":" => Colon,
        ";" => Semi,
        "." => Dot,
        "=" => Assign,
        "<" => Lt,
        ">" => Gt,
        "+" => Plus,
        "-" => Minus,
        "*" => Star,
        "/" => Slash,
        "%" => Percent,
        "!" => Bang,
        "?" => Question,
        "|" => Bar,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceMap;

    pub(super) fn lex_src(src: &str) -> Result<(Vec<Token>, Vec<Comment>), Diag> {
        let mut sm = SourceMap::new();
        let f = sm.add("t.fung", src);
        lex(f, src)
    }

    fn kinds(src: &str) -> Vec<Kind> {
        let (toks, _) = lex_src(src).expect("lex");
        toks[..toks.len() - 1].iter().map(|t| t.kind).collect()
    }

    fn p(line: u32, col: u32) -> (u32, u32) {
        (line, col)
    }

    #[test]
    fn punctuation_longest_match() {
        use Kind::*;
        assert_eq!(
            kinds(";; ;;; ;;;;"),
            vec![DoubleSemi, DoubleSemi, Semi, DoubleSemi, DoubleSemi]
        );
        assert_eq!(kinds(". .. ..= ..."), vec![Dot, DotDot, DotDotEq, Ellipsis]);
        assert_eq!(kinds("a;;b"), vec![Name, DoubleSemi, Name]);
        assert_eq!(
            kinds("( ) [ ] { } , : ;"),
            vec![
                LParen, RParen, LBracket, RBracket, LBrace, RBrace, Comma, Colon, Semi
            ]
        );
        assert_eq!(
            kinds("= == => -> - <= < >= > !="),
            vec![Assign, Eq, FatArrow, Arrow, Minus, Le, Lt, Ge, Gt, Ne]
        );
        assert_eq!(
            kinds("+ * ** / // % |> | ?? ? !"),
            vec![
                Plus, Star, StarStar, Slash, SlashSlash, Percent, Pipe, Bar, Coalesce, Question,
                Bang
            ]
        );
    }

    #[test]
    fn identifiers_keywords_and_tags() {
        use Kind::*;
        let (toks, _) = lex_src("fn is_log Ok _x 파일목록 Red true nil x1").unwrap();
        let want = [
            (KwFn, "fn"),
            (Name, "is_log"),
            (Tag, "Ok"),
            (Name, "_x"),
            (Name, "파일목록"),
            (Tag, "Red"),
            (KwTrue, "true"),
            (KwNil, "nil"),
            (Name, "x1"),
        ];
        for (i, (k, text)) in want.iter().enumerate() {
            assert_eq!(
                (toks[i].kind, toks[i].text.as_str()),
                (*k, *text),
                "token {i}"
            );
        }
    }

    #[test]
    fn all_keywords_are_recognized() {
        for (word, kind) in Kind::keywords() {
            let (toks, _) = lex_src(word).unwrap();
            assert_eq!(toks[0].kind, *kind, "{word}");
            assert!(kind.is_keyword());
        }
        assert_eq!(Kind::keywords().len(), 20, "01. reserved words");
        assert!(!Kind::Name.is_keyword());
        assert!(!Kind::Bang.is_keyword());
        assert_eq!(Kind::Eof.describe(), "end of file");
        assert_eq!(Kind::KwFn.describe(), "fn");
        assert_eq!(Kind::Bang.describe(), "!");
    }

    #[test]
    fn newlines_comments_and_positions() {
        use Kind::*;
        let src = "a # first\r\n  b\n\n# only comment\nc";
        let (toks, comments) = lex_src(src).unwrap();
        let want = [Name, Newline, Name, Newline, Newline, Newline, Name, Eof];
        let got: Vec<Kind> = toks.iter().map(|t| t.kind).collect();
        assert_eq!(got, want);
        let want_pos = [p(1, 1), p(1, 10), p(2, 3)];
        for (i, w) in want_pos.iter().enumerate() {
            assert_eq!((toks[i].pos.line, toks[i].pos.col), *w, "token {i}");
        }
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].text, "# first");
        assert_eq!(comments[1].text, "# only comment");
        assert_eq!((comments[1].pos.line, comments[1].pos.col), (4, 1));
    }

    #[test]
    fn columns_count_chars_not_bytes() {
        let (toks, _) = lex_src("참치 x").unwrap();
        assert_eq!(toks[1].pos.col, 4);
    }

    #[test]
    fn columns_count_chars_and_bom_is_skipped() {
        let src = "\u{feff}x = \"한글\" y";
        let (toks, _) = lex_src(src).unwrap();
        assert_eq!((toks[0].pos.line, toks[0].pos.col), (1, 1));
        assert_eq!(toks[3].pos.col, 10); // y comes after a 2-char string
    }

    #[test]
    fn unknown_escape_is_e0103() {
        let d = lex_src("\tx = \"\\q\"").unwrap_err();
        assert_eq!((d.code.as_str(), d.pos.col), ("E0103", 7));
        assert_eq!(d.args, vec![Arg::Str("q".to_string())]);
    }

    #[test]
    fn unexpected_character() {
        let d = lex_src("a @ b").unwrap_err();
        assert_eq!(d.code, "E0101");
        assert_eq!((d.pos.line, d.pos.col), (1, 3));
        assert_eq!(d.args, vec![Arg::Str("@".to_string())]);
    }

    #[test]
    fn empty_source_is_just_eof() {
        let (toks, _) = lex_src("").unwrap();
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].kind, Kind::Eof);
    }

    #[test]
    fn nul_byte_is_rejected_not_treated_as_eof() {
        let d = lex_src("x\x00y").unwrap_err();
        assert_eq!((d.code.as_str(), d.pos.col), ("E0101", 2));
        assert!(lex_src("print(1)\n\x00print(2)").is_err());
    }

    #[test]
    fn leading_bom_is_skipped() {
        let (toks, _) = lex_src("\u{feff}x").unwrap();
        assert_eq!(toks[0].kind, Kind::Name);
        assert_eq!(toks[0].text, "x");
        assert_eq!(toks[0].pos.col, 1);
        // Only a leading BOM is special; elsewhere it is an unexpected character.
        let d = lex_src("x\u{feff}").unwrap_err();
        assert_eq!(d.code, "E0101");
    }

    #[test]
    fn lex_at_starts_from_given_position() {
        let mut sm = SourceMap::new();
        let f = sm.add("t.fung", "x + 1");
        let start = Pos {
            file: f,
            line: 3,
            col: 10,
        };
        let (toks, _) = lex_at(f, "x + 1", start, 0).unwrap();
        assert_eq!((toks[0].pos.line, toks[0].pos.col), (3, 10));
        assert_eq!(toks[2].pos.col, 14);
    }

    #[test]
    fn lex_at_adds_base_to_spans() {
        let mut sm = SourceMap::new();
        let f = sm.add("t.fung", "x");
        let start = Pos {
            file: f,
            line: 1,
            col: 1,
        };
        let (toks, _) = lex_at(f, "ab", start, 100).unwrap();
        assert_eq!((toks[0].off, toks[0].end), (100, 102));
    }

    #[test]
    fn tokens_carry_byte_spans() {
        let src = "참치 = \"a b\" + 1 `raw` 'sym 1.5 2h";
        let (toks, _) = lex_src(src).unwrap();
        let want = [
            "참치", "=", "\"a b\"", "+", "1", "`raw`", "'sym", "1.5", "2h",
        ];
        for (i, w) in want.iter().enumerate() {
            assert_eq!(&src[toks[i].off..toks[i].end], *w, "token {i}");
        }
        let eof = toks.last().unwrap();
        assert_eq!((eof.off, eof.end), (src.len(), src.len()));
    }
}
