//! `_native.json` (Go `internal/native/json.go`, 06): a decoder that keeps
//! object key order and integers exact, and an encoder over fung values.
//!
//! Go reads tokens with `encoding/json`'s `Decoder.Token`, which in Go 1.27
//! runs over `encoding/json/jsontext` (the `jsonv2` experiment is on by
//! default). Its error texts and positions are part of what a program
//! sees, so this file ports the three layers that decide them, down to
//! how much of the input Go has read when it reports an error:
//!
//! - [`Wire`]: `jsontext.decoderState` (`ReadToken`, `PeekKind`, the
//!   state machine and its 10,000-level limit) and the `jsonwire` scanners;
//! - [`Decoder`]: the v1 shim (`v2_stream.go`: `Token`, `More`,
//!   `InputOffset`, `transformSyntacticError`);
//! - [`decode`]: Go fung's `decodeJSON`/`readValue`, made iterative.
//!
//! Neither parsing nor writing recurses: deep input stops at Go's limit,
//! and values of any depth are written with an explicit stack.

use std::rc::Rc;
use std::sync::OnceLock;

use fung_front::diag::{Arg, quote};
use fung_front::unicode::is_print;

use super::nat::{self, native, opt, param};
use super::timefmt::quote_bytes;
use crate::value::{
    ListBuilder, ListIter, Lit, MapBuilder, MapIter, Module, Native, Value, format, type_name,
};
use crate::vm::{Stop, Task};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The `_native.json` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.get_or_init(|| {
        vec![
            ("parse", native("parse", parse_fn, vec![param("s")])),
            (
                "stringify",
                native(
                    "stringify",
                    stringify_fn,
                    vec![param("v"), opt("indent", Lit::Nil)],
                ),
            ),
        ]
    });
    let mut m = Module::new("json", "_native.json");
    for (name, n) in natives {
        m.set(name, Value::Native(n));
    }
    Rc::new(m)
}

fn parse_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    Ok(match decode(s) {
        Ok(v) => nat::ok(v),
        Err(e) => nat::err_value(
            "parse",
            &format!(
                "invalid JSON at line {}, column {}: {}",
                e.line, e.col, e.msg
            ),
            vec![
                ("line", Value::Int(e.line as i64)),
                ("col", Value::Int(e.col as i64)),
            ],
        ),
    })
}

// ---------------------------------------------------------------------------
// jsontext: the token reader

/// Go `jsontext.maxNestingDepth`.
const MAX_DEPTH: usize = 10_000;

/// A token as `jsontext.ReadToken` returns it.
enum Tok {
    Delim(u8),
    Str(String),
    Bool(bool),
    Null,
    /// The literal as written (`Decoder.UseNumber`).
    Num(String),
}

/// The error a `jsontext` reader reports (its `Err` field).
#[derive(Clone)]
enum Cause {
    /// `jsonwire.InvalidTextError`.
    Text {
        label: &'static str,
        what: Vec<u8>,
        place: String,
    },
    Msg(&'static str),
    /// `io.ErrUnexpectedEOF`: the input ends inside a token or value.
    UnexpectedEof,
    /// `errMismatchDelim`; worded by [`Wire::wrap`].
    Mismatch,
}

const NON_STRING_NAME: &str = "object member name must be a string";
const MISSING_VALUE: &str = "missing value after object name";
const MAX_DEPTH_MSG: &str = "exceeded max depth";

impl Cause {
    /// `InvalidTextError.Error`, or the error's own text.
    fn text(&self) -> String {
        match self {
            Cause::Text { label, what, place } => {
                let shown = if rune_count(what) == 1 {
                    quote_rune_bytes(what)
                } else if needs_escape(what) {
                    quote_bytes(what)
                } else {
                    format!("`{}`", String::from_utf8_lossy(what))
                };
                format!("invalid {label} {shown} {place}")
                    .trim_end_matches(' ')
                    .to_string()
            }
            Cause::Msg(m) => (*m).to_string(),
            Cause::UnexpectedEof => "unexpected EOF".into(),
            Cause::Mismatch => "mismatching structural token for object or array".into(),
        }
    }
}

/// What a read fails with: `io.EOF` (no more top-level tokens) or a
/// `jsontext.SyntacticError` at a byte offset.
#[derive(Clone)]
enum ReadErr {
    Eof,
    Syntax { offset: usize, cause: Cause },
}

/// `jsonwire.NewInvalidCharacterError(b, place)`: the character at the
/// start of `b`.
fn invalid_char(b: &[u8], place: &str) -> Cause {
    Cause::Text {
        label: "character",
        what: b[..first_rune_len(b)].to_vec(),
        place: place.into(),
    }
}

/// `jsonwire.NewInvalidEscapeSequenceError`.
fn invalid_escape(what: &[u8]) -> Cause {
    Cause::Text {
        label: if what.len() > 6 {
            "surrogate pair"
        } else {
            "escape sequence"
        },
        what: what.to_vec(),
        place: "in string".into(),
    }
}

/// One entry of the `jsontext` state machine: an object or array and how
/// many names and values it has read so far.
#[derive(Clone, Copy)]
struct Entry {
    object: bool,
    len: u64,
}

impl Entry {
    fn need_name(self) -> bool {
        self.object && self.len.is_multiple_of(2)
    }

    fn need_value(self) -> bool {
        self.object && !self.len.is_multiple_of(2)
    }

    fn need_comma(self, next: u8) -> bool {
        !self.need_value() && self.len > 0 && next != b'}' && next != b']'
    }
}

/// Go `jsontext.Kind(c).normalize()`: the kind a byte starts, 0 if none.
fn kind(c: u8) -> u8 {
    match c {
        b'n' | b'f' | b't' | b'"' | b'{' | b'}' | b'[' | b']' => c,
        b'-' | b'0'..=b'9' => b'0',
        _ => 0,
    }
}

fn is_ws(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n')
}

fn skip_ws(b: &[u8], mut pos: usize) -> usize {
    while pos < b.len() && is_ws(b[pos]) {
        pos += 1;
    }
    pos
}

/// Go `jsontext.decoderState` reading Go fung's `strings.Reader`. Go
/// reads the input in growing chunks, and an error names what is in its
/// buffer at that moment (a character cut at the buffer's end shows as
/// its first byte), so the buffer's growth is kept exactly: positions are
/// offsets into the whole input, and `end` is how much of it Go has read.
struct Wire<'a> {
    input: &'a [u8],
    /// The input offset of the buffer's first byte (Go `baseOffset`).
    base: usize,
    /// The input offset past the buffer's last byte.
    end: usize,
    /// The buffer's capacity (Go `cap(d.buf)`).
    cap: usize,
    prev_start: usize,
    prev_end: usize,
    /// A cached `PeekKind` result: the next token's position or the error.
    peek: Option<Result<usize, ReadErr>>,
    /// The enclosing entries; the bottom one stands for the top level
    /// (Go treats the top level as an array).
    stack: Vec<Entry>,
    last: Entry,
}

impl<'a> Wire<'a> {
    fn new(input: &'a [u8]) -> Wire<'a> {
        Wire {
            input,
            base: 0,
            end: 0,
            cap: 0,
            prev_start: 0,
            prev_end: 0,
            peek: None,
            stack: Vec::new(),
            last: Entry {
                object: false,
                len: 0,
            },
        }
    }

    /// The buffered input from `pos` on (Go `d.buf[pos:]`).
    fn buf(&self, pos: usize) -> &'a [u8] {
        &self.input[pos..self.end]
    }

    /// Go `decoderState.fetch`: reads more input, false at its end. The
    /// buffer keeps what follows the previous token's start, grows by
    /// doubling, and each read fills it as far as it can.
    fn fetch(&mut self) -> bool {
        if self.cap == 0 {
            self.cap = 64;
        }
        let len = self.end - self.base;
        let prev_start = self.prev_start - self.base;
        let grow = (self.cap <= 4096 / 2 && self.cap < self.prev_end / 2)
            || (prev_start == 0 && len >= 3 * self.cap / 4);
        if grow {
            self.cap *= 2;
        }
        self.base = self.prev_start;
        let room = self.cap - (self.end - self.base);
        let n = room.min(self.input.len() - self.end);
        self.end += n;
        n > 0
    }

    /// Go `invalidatePreviousRead`: the previous token's text is given up
    /// (which changes what a later `fetch` keeps).
    fn invalidate_previous_read(&mut self) {
        if self.prev_start < self.prev_end && self.prev_start < self.end {
            self.prev_start = self.prev_end;
        }
    }

    /// Go `consumeWhitespace`: past whitespace, reading more as needed;
    /// `Err` with the position at the end of input.
    fn consume_ws(&mut self, mut pos: usize) -> Result<usize, usize> {
        loop {
            pos = skip_ws(&self.input[..self.end], pos);
            if pos < self.end {
                return Ok(pos);
            }
            if !self.fetch() {
                return Err(pos);
            }
        }
    }

    fn depth(&self) -> usize {
        self.stack.len() + 1
    }

    /// Go `stateMachine.needDelim`.
    fn need_delim(&self, next: u8) -> u8 {
        if self.last.need_value() {
            b':'
        } else if self.last.need_comma(next) && !self.stack.is_empty() {
            b','
        } else {
            0
        }
    }

    fn append_literal(&mut self) -> Result<(), Cause> {
        if self.last.need_name() {
            return Err(Cause::Msg(NON_STRING_NAME));
        }
        self.last.len += 1;
        Ok(())
    }

    fn push(&mut self, object: bool) -> Result<(), Cause> {
        if self.last.need_name() {
            return Err(Cause::Msg(NON_STRING_NAME));
        }
        if self.stack.len() == MAX_DEPTH {
            return Err(Cause::Msg(MAX_DEPTH_MSG));
        }
        self.last.len += 1;
        self.stack.push(self.last);
        self.last = Entry { object, len: 0 };
        Ok(())
    }

    fn pop(&mut self, object: bool) -> Result<(), Cause> {
        if object {
            if !self.last.object {
                return Err(Cause::Mismatch);
            }
            if self.last.need_value() {
                return Err(Cause::Msg(MISSING_VALUE));
            }
        } else if self.last.object || self.stack.is_empty() {
            return Err(Cause::Mismatch);
        }
        self.last = self.stack.pop().expect("an enclosing entry");
        Ok(())
    }

    /// Go `wrapSyntacticError` under `ReportErrorsWithLegacySemantics`:
    /// the offset moves past the offending text (or one byte).
    fn wrap(&self, cause: Cause, pos: usize) -> ReadErr {
        let cause = match cause {
            Cause::Mismatch => {
                let mut place = "at start of value";
                if !self.stack.is_empty() && self.last.len > 0 {
                    place = if self.last.object {
                        "after object value (expecting ',' or '}')"
                    } else {
                        "after array element (expecting ',' or ']')"
                    };
                }
                invalid_char(self.buf(pos), place)
            }
            c => c,
        };
        let offset = match &cause {
            Cause::UnexpectedEof => pos,
            Cause::Text { what, .. } => pos + what.len(),
            _ => pos + 1,
        };
        ReadErr::Syntax { offset, cause }
    }

    /// Go `checkDelim`: `delim` does not fit the next token.
    fn check_delim(&self, delim: u8, next: u8) -> ReadErr {
        let place = match self.need_delim(next) {
            b':' => "after object name (expecting ':')",
            b',' if self.last.object => "after object value (expecting ',' or '}')",
            b',' => "after array element (expecting ',' or ']')",
            _ => "at start of value",
        };
        debug_assert_ne!(self.need_delim(next), delim);
        let pos = skip_ws(&self.input[..self.end], self.prev_end);
        self.wrap(invalid_char(self.buf(pos), place), pos)
    }

    /// Go `checkDelimBeforeIOError`.
    fn check_delim_before_eof(&self, delim: u8, err: ReadErr) -> ReadErr {
        if self.need_delim(b'"') != delim {
            return self.check_delim(delim, b'"');
        }
        err
    }

    /// The position of the next token past whitespace and its separator
    /// (the shared start of `ReadToken` and `PeekKind`).
    fn next_pos(&mut self) -> Result<usize, ReadErr> {
        self.invalidate_previous_read();
        let mut pos = match self.consume_ws(self.prev_end) {
            Ok(pos) => pos,
            // io.EOF between top-level values, else an unexpected end
            Err(_) if self.depth() == 1 => return Err(ReadErr::Eof),
            Err(pos) => return Err(self.wrap(Cause::UnexpectedEof, pos)),
        };
        let mut delim = 0;
        if matches!(self.input[pos], b':' | b',') {
            delim = self.input[pos];
            pos = match self.consume_ws(pos + 1) {
                Ok(pos) => pos,
                Err(pos) => {
                    let err = self.wrap(Cause::UnexpectedEof, pos);
                    return Err(self.check_delim_before_eof(delim, err));
                }
            };
        }
        let next = kind(self.input[pos]);
        if self.need_delim(next) != delim {
            return Err(self.check_delim(delim, next));
        }
        Ok(pos)
    }

    /// Go `PeekKind`: the next token's kind, 0 on an error (cached for the
    /// next read) or a byte that starts no token.
    fn peek_kind(&mut self) -> u8 {
        if self.peek.is_none() {
            let next = self.next_pos();
            self.peek = Some(next);
        }
        match &self.peek {
            Some(Ok(pos)) => kind(self.input[*pos]),
            _ => 0,
        }
    }

    /// Runs a `jsonwire` scanner on the buffer at `pos`, reading more and
    /// scanning again while it needs more (the end of the buffer is not
    /// yet the end of input). `ends_open` tells a scanner whose token can
    /// end at the buffer's end but might go on (a number).
    fn scan(
        &mut self,
        pos: usize,
        f: impl Fn(&[u8]) -> Result<usize, (usize, Cause)>,
        ends_open: bool,
    ) -> Result<usize, ReadErr> {
        loop {
            match f(self.buf(pos)) {
                Ok(n) if !(ends_open && pos + n == self.end) => return Ok(n),
                Ok(n) => {
                    if !self.fetch() {
                        return Ok(n);
                    }
                }
                Err((n, Cause::UnexpectedEof)) => {
                    if !self.fetch() {
                        // Go reports a number's end at its start; the
                        // offset of an unexpected end is not shown.
                        let at = if ends_open { pos } else { pos + n };
                        return Err(self.wrap(Cause::UnexpectedEof, at));
                    }
                }
                Err((n, cause)) => return Err(self.wrap(cause, pos + n)),
            }
        }
    }

    /// Go `ReadToken`.
    fn read_token(&mut self) -> Result<Tok, ReadErr> {
        let pos = match self.peek.take() {
            Some(r) => r?,
            None => self.next_pos()?,
        };
        match kind(self.input[pos]) {
            k @ (b'n' | b'f' | b't') => {
                let (lit, tok) = match k {
                    b'n' => ("null", Tok::Null),
                    b'f' => ("false", Tok::Bool(false)),
                    _ => ("true", Tok::Bool(true)),
                };
                self.scan(pos, |b| consume_literal(b, lit).map(|()| lit.len()), false)?;
                let end = pos + lit.len();
                if let Err(c) = self.append_literal() {
                    return Err(self.wrap(c, pos));
                }
                self.prev_start = end;
                self.prev_end = end;
                Ok(tok)
            }
            b'"' => {
                let n = self.scan(pos, consume_string, false)?;
                // Names are strings too; a name only advances the count.
                self.last.len += 1;
                self.prev_start = pos;
                self.prev_end = pos + n;
                Ok(Tok::Str(unquote(&self.input[pos..pos + n])))
            }
            b'0' => {
                let n = self.scan(pos, consume_number, true)?;
                if let Err(c) = self.append_literal() {
                    return Err(self.wrap(c, pos));
                }
                self.prev_start = pos;
                self.prev_end = pos + n;
                Ok(Tok::Num(
                    String::from_utf8_lossy(&self.input[pos..pos + n]).into_owned(),
                ))
            }
            k @ (b'{' | b'[') => {
                if let Err(c) = self.push(k == b'{') {
                    return Err(self.wrap(c, pos));
                }
                self.prev_start = pos + 1;
                self.prev_end = pos + 1;
                Ok(Tok::Delim(k))
            }
            k @ (b'}' | b']') => {
                if let Err(c) = self.pop(k == b'}') {
                    return Err(self.wrap(c, pos));
                }
                self.prev_start = pos + 1;
                self.prev_end = pos + 1;
                Ok(Tok::Delim(k))
            }
            _ => Err(self.wrap(invalid_char(self.buf(pos), "at start of value"), pos)),
        }
    }

    /// The buffered input after the last token read (Go `UnreadBuffer`).
    fn unread(&self) -> &'a [u8] {
        self.buf(self.prev_end)
    }
}

/// Go `jsonwire.ConsumeLiteral`: Ok when `b` starts with `lit`, else how
/// far it matched and why it stopped.
fn consume_literal(b: &[u8], lit: &str) -> Result<(), (usize, Cause)> {
    let lit = lit.as_bytes();
    for i in 0..b.len().min(lit.len()) {
        if b[i] != lit[i] {
            let place = format!(
                "in literal {} (expecting '{}')",
                String::from_utf8_lossy(lit),
                lit[i] as char
            );
            return Err((i, invalid_char(&b[i..], &place)));
        }
    }
    if b.len() < lit.len() {
        return Err((b.len(), Cause::UnexpectedEof));
    }
    Ok(())
}

fn hex4(b: &[u8]) -> Option<u16> {
    if b.len() != 4 {
        return None;
    }
    let mut v: u16 = 0;
    for &c in b {
        v = v * 16 + (c as char).to_digit(16)? as u16;
    }
    Some(v)
}

/// Go `hasEscapedUTF16Prefix` (for a first half): `b` could still become
/// a `\uXXXX` escape once more input is read.
fn escape_prefix(b: &[u8]) -> bool {
    b.iter().enumerate().all(|(i, &c)| match i {
        0 => c == b'\\',
        1 => c == b'u',
        2..=5 => c.is_ascii_hexdigit(),
        _ => true,
    })
}

/// Go `jsonwire.ConsumeString` without UTF-8 validation (v1 allows
/// invalid UTF-8, and lone surrogates): the length of the string token at
/// the start of the buffered input `b`, or where and why it stops
/// (`UnexpectedEof` when more input could finish it).
fn consume_string(b: &[u8]) -> Result<usize, (usize, Cause)> {
    let mut n = 1; // the opening quote
    loop {
        while n < b.len() && b[n] < 0x80 && b[n] >= b' ' && b[n] != b'\\' && b[n] != b'"' {
            n += 1;
        }
        if n >= b.len() {
            return Err((n, Cause::UnexpectedEof));
        }
        match b[n] {
            b'"' => return Ok(n + 1),
            b'\\' => {
                if b.len() < n + 2 {
                    return Err((n, Cause::UnexpectedEof));
                }
                match b[n + 1] {
                    b'/' | b'"' | b'\\' | b'b' | b'f' | b'n' | b'r' | b't' => n += 2,
                    b'u' => {
                        if b.len() < n + 6 {
                            if escape_prefix(&b[n..]) {
                                return Err((n, Cause::UnexpectedEof));
                            }
                            return Err((n, invalid_escape(&b[n..])));
                        }
                        if hex4(&b[n + 2..n + 6]).is_none() {
                            return Err((n, invalid_escape(&b[n..n + 6])));
                        }
                        n += 6;
                    }
                    _ => return Err((n, invalid_escape(&b[n..n + 2]))),
                }
            }
            // The input is valid UTF-8, so a character that does not decode
            // is cut at the buffer's end (Go: `!utf8.FullRune`).
            c if c >= 0x80 => match rune_len(&b[n..]) {
                Some(len) => n += len,
                None => return Err((n, Cause::UnexpectedEof)),
            },
            _ => {
                return Err((
                    n,
                    invalid_char(&b[n..], "in string (expecting non-control character)"),
                ));
            }
        }
    }
}

/// Go `jsonwire.AppendUnquote` of a string token `consume_string` read: a
/// broken surrogate becomes U+FFFD.
fn unquote(src: &[u8]) -> String {
    let mut out: Vec<u8> = Vec::with_capacity(src.len());
    let end = src.len() - 1; // the closing quote
    let mut n = 1;
    while n < end {
        let c = src[n];
        if c != b'\\' {
            out.push(c);
            n += 1;
            continue;
        }
        let r = match src[n + 1] {
            b'"' | b'\\' | b'/' => src[n + 1] as u32,
            b'b' => 8,
            b'f' => 12,
            b'n' => 10,
            b'r' => 13,
            b't' => 9,
            _ => {
                let v1 = hex4(&src[n + 2..n + 6]).expect("checked by consume_string") as u32;
                n += 6;
                let r = if (0xD800..0xE000).contains(&v1) {
                    // Go assumes failure unless a low half follows.
                    let low = (src.len() >= n + 6 && src[n] == b'\\' && src[n + 1] == b'u')
                        .then(|| hex4(&src[n + 2..n + 6]))
                        .flatten()
                        .map(u32::from);
                    match low {
                        Some(v2)
                            if (0xD800..0xDC00).contains(&v1) && (0xDC00..0xE000).contains(&v2) =>
                        {
                            n += 6;
                            0x10000 + ((v1 - 0xD800) << 10) + (v2 - 0xDC00)
                        }
                        _ => 0xFFFD,
                    }
                } else {
                    v1
                };
                let ch = char::from_u32(r).unwrap_or('\u{fffd}');
                out.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
                continue;
            }
        };
        out.push(r as u8);
        n += 2;
    }
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

/// Go `jsonwire.ConsumeNumber`: the length of the number at the start of
/// the buffered input `b`, or where and why it stops.
fn consume_number(b: &[u8]) -> Result<usize, (usize, Cause)> {
    let digits = |mut n: usize| {
        while n < b.len() && b[n].is_ascii_digit() {
            n += 1;
        }
        n
    };
    let expect_digit = |n: usize| -> Result<(), (usize, Cause)> {
        if n >= b.len() {
            Err((n, Cause::UnexpectedEof))
        } else if b[n].is_ascii_digit() {
            Ok(())
        } else {
            Err((n, invalid_char(&b[n..], "in number (expecting digit)")))
        }
    };
    let mut n = usize::from(b.first() == Some(&b'-'));
    expect_digit(n)?;
    n = if b[n] == b'0' { n + 1 } else { digits(n) };
    if n < b.len() && b[n] == b'.' {
        n += 1;
        expect_digit(n)?;
        n = digits(n);
    }
    if n < b.len() && (b[n] == b'e' || b[n] == b'E') {
        n += 1;
        if n < b.len() && (b[n] == b'-' || b[n] == b'+') {
            n += 1;
        }
        expect_digit(n)?;
        n = digits(n);
    }
    Ok(n)
}

// ---------------------------------------------------------------------------
// encoding/json v1 over jsontext (v2_stream.go)

/// What the v1 `Decoder` reports.
#[derive(Clone, PartialEq)]
enum V1Err {
    Eof,
    /// `io.ErrUnexpectedEOF`, unwrapped (input ends inside a token).
    UnexpectedEof,
    /// `*json.SyntaxError`.
    Syntax {
        msg: String,
        offset: usize,
    },
}

/// Go `transformSyntacticError`.
fn transform(e: ReadErr) -> V1Err {
    match e {
        ReadErr::Eof => V1Err::Eof,
        ReadErr::Syntax { offset, cause } => {
            let mut msg = match cause {
                Cause::UnexpectedEof => "unexpected end of JSON input".to_string(),
                c => c.text(),
            };
            if let Some(i) = msg.find(" (expecting")
                && !msg.contains(" in literal")
            {
                msg.truncate(i);
            }
            V1Err::Syntax {
                msg: replace_v2_words(&msg),
                offset,
            }
        }
    }
}

/// Go `syntaxErrorReplacer`: v1's words for v2's.
fn replace_v2_words(s: &str) -> String {
    const PAIRS: [(&str, &str); 5] = [
        ("object name", "object key"),
        ("at start of value", "looking for beginning of value"),
        (
            "at start of string",
            "looking for beginning of object key string",
        ),
        ("after object value", "after object key:value pair"),
        ("in number", "in numeric literal"),
    ];
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    'scan: while !rest.is_empty() {
        for (old, new) in PAIRS {
            if let Some(after) = rest.strip_prefix(old) {
                out.push_str(new);
                rest = after;
                continue 'scan;
            }
        }
        let c = rest.chars().next().expect("not empty");
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// Go `encoding/json.Decoder` (the v1 API over `jsontext`).
struct Decoder<'a> {
    w: Wire<'a>,
    /// Sticky: set by `More` when its peek fails.
    err: Option<V1Err>,
    had_peeked: bool,
    had_eof: bool,
}

impl<'a> Decoder<'a> {
    fn new(s: &'a str) -> Decoder<'a> {
        Decoder {
            w: Wire::new(s.as_bytes()),
            err: None,
            had_peeked: false,
            had_eof: false,
        }
    }

    fn token(&mut self) -> Result<Tok, V1Err> {
        if let Some(e) = &self.err {
            return Err(e.clone());
        }
        match self.w.read_token() {
            Ok(t) => {
                self.had_peeked = false;
                self.had_eof = false;
                Ok(t)
            }
            Err(ReadErr::Syntax {
                cause: Cause::UnexpectedEof,
                ..
            }) => {
                // v1 reports a truncated stream that is a prefix of a
                // valid value as io.EOF.
                let rest = self.w.unread();
                if rest
                    .iter()
                    .all(|c| matches!(c, b' ' | b'\r' | b'\n' | b'\t' | b',' | b':'))
                {
                    self.had_eof = true;
                    return Err(V1Err::Eof);
                }
                Err(V1Err::UnexpectedEof)
            }
            Err(e) => Err(transform(e)),
        }
    }

    fn more(&mut self) -> bool {
        self.had_peeked = true;
        let k = self.w.peek_kind();
        if k == 0 {
            if self.err.is_none() {
                let err = match self.w.read_token() {
                    Err(e) => e,
                    Ok(_) => unreachable!("a read after a failed peek fails"),
                };
                self.err = Some(transform(err));
            }
            return self.err != Some(V1Err::Eof);
        }
        k != b']' && k != b'}'
    }

    fn input_offset(&self) -> usize {
        let mut offset = self.w.prev_end;
        if self.had_peeked || self.had_eof {
            let unread = self.w.unread();
            let lead = unread.iter().take_while(|c| is_ws(**c)).count();
            if lead < unread.len() {
                offset += lead;
                if self.had_eof && matches!(unread[lead], b',' | b':') {
                    offset += 1;
                }
            }
        }
        offset
    }
}

// ---------------------------------------------------------------------------
// Go fung's decodeJSON

/// A parse failure with a 1-based position.
#[derive(Debug, PartialEq)]
struct JsonError {
    line: usize,
    col: usize,
    msg: String,
}

/// Go `lineCol`: the line, and the column counted in runes (each byte of
/// an invalid sequence is one) up to the byte offset.
fn line_col(s: &[u8], off: usize) -> (usize, usize) {
    let before = &s[..off.min(s.len())];
    let line = before.iter().filter(|&&c| c == b'\n').count() + 1;
    let start = before
        .iter()
        .rposition(|&c| c == b'\n')
        .map_or(0, |i| i + 1);
    (line, rune_count(&before[start..]) + 1)
}

/// Go `jsonErr`: a syntax error names its own offset (moved back onto the
/// offending character), anything else the offset handed in.
fn json_err(s: &str, off: usize, err: Option<V1Err>, fallback: &str) -> JsonError {
    let (mut off, mut msg) = (off, fallback.to_string());
    match err {
        Some(V1Err::Syntax { msg: m, offset }) => {
            off = offset;
            if off > 0 && !m.starts_with("unexpected end") {
                off -= 1;
            }
            msg = m;
        }
        Some(V1Err::UnexpectedEof) => msg = "unexpected EOF".into(),
        Some(V1Err::Eof) | None => {}
    }
    let (line, col) = line_col(s.as_bytes(), off);
    JsonError { line, col, msg }
}

/// An array or object being read.
enum Open {
    List(ListBuilder),
    Map(MapBuilder, Option<Rc<String>>),
}

/// Go `decodeJSON`: one value and nothing after it. Go's `readValue`
/// recurses per level; this keeps the open levels on a stack.
fn decode(s: &str) -> Result<Value, JsonError> {
    let mut dec = Decoder::new(s);
    let mut open: Vec<Open> = Vec::new();
    let value = 'read: loop {
        // readValue: the next value, or the start of a nested one
        let start = skip_ws(s.as_bytes(), dec.input_offset());
        let mut done = match dec.token() {
            Err(V1Err::Eof) => return Err(json_err(s, start, None, "unexpected end of input")),
            Err(e) => return Err(json_err(s, start, Some(e), "invalid token")),
            Ok(Tok::Delim(b'{')) => {
                open.push(Open::Map(MapBuilder::default(), None));
                None
            }
            Ok(Tok::Delim(b'[')) => {
                open.push(Open::List(ListBuilder::with_capacity(0)));
                None
            }
            Ok(Tok::Delim(d)) => {
                return Err(json_err(
                    s,
                    start,
                    None,
                    &format!("unexpected {}", d as char),
                ));
            }
            Ok(Tok::Str(x)) => Some(Value::str(&x)),
            Ok(Tok::Bool(b)) => Some(Value::Bool(b)),
            Ok(Tok::Null) => Some(Value::Nil),
            Ok(Tok::Num(lit)) => Some(number(&lit, s, start)?),
        };
        // finish levels until one wants another value
        loop {
            let Some(top) = open.last_mut() else {
                break 'read done.expect("a finished value");
            };
            if let Some(v) = done.take() {
                match top {
                    Open::List(b) => b.push(v),
                    Open::Map(m, key) => m.set(key.take().expect("a key"), v),
                }
            }
            if dec.more() {
                if let Open::Map(_, key) = top {
                    match dec.token() {
                        Ok(Tok::Str(k)) => *key = Some(Rc::new(k)),
                        Ok(_) => {
                            let off = dec.input_offset();
                            return Err(json_err(s, off, None, "object keys must be strings"));
                        }
                        Err(e) => {
                            let off = dec.input_offset();
                            return Err(json_err(s, off, Some(e), "invalid object key"));
                        }
                    }
                }
                continue 'read;
            }
            let unterminated = match top {
                Open::List(_) => "unterminated array",
                Open::Map(..) => "unterminated object",
            };
            if let Err(e) = dec.token() {
                let off = dec.input_offset();
                return Err(json_err(s, off, Some(e), unterminated));
            }
            done = Some(match open.pop().expect("an open level") {
                Open::List(b) => Value::List(Rc::new(b.finish())),
                Open::Map(m, _) => Value::Map(Rc::new(m.finish())),
            });
        }
    };
    let rest = skip_ws(s.as_bytes(), dec.input_offset());
    match dec.token() {
        Err(V1Err::Eof) => Ok(value),
        Err(e) => Err(json_err(
            s,
            rest,
            Some(e),
            "unexpected text after the value",
        )),
        Ok(_) => Err(json_err(s, rest, None, "unexpected text after the value")),
    }
}

/// Go `numberValue`: a literal without `.`, `e` or `E` is an int and must
/// fit 64 bits (06: 조용히 float로 바꾸지 않는다).
fn number(lit: &str, s: &str, start: usize) -> Result<Value, JsonError> {
    if !lit.contains(['.', 'e', 'E']) {
        return match lit.parse::<i64>() {
            Ok(n) => Ok(Value::Int(n)),
            Err(_) => Err(json_err(
                s,
                start,
                None,
                &format!("integer {lit} is out of range"),
            )),
        };
    }
    match lit.parse::<f64>() {
        Ok(f) if f.is_finite() => Ok(Value::Float(f)),
        _ => Err(json_err(
            s,
            start,
            None,
            &format!("number {lit} is out of range"),
        )),
    }
}

// ---------------------------------------------------------------------------
// text helpers (Go `utf8`, `strconv`, `unicode`)

/// Go `utf8.DecodeRune` width: an invalid byte is one.
fn first_rune_len(b: &[u8]) -> usize {
    match b.utf8_chunks().next() {
        Some(c) if !c.valid().is_empty() => c.valid().chars().next().map_or(1, char::len_utf8),
        _ => b.len().min(1),
    }
}

/// The length of the character at the start of `b`, `None` if it does
/// not decode.
fn rune_len(b: &[u8]) -> Option<usize> {
    let c = b.utf8_chunks().next()?;
    c.valid().chars().next().map(char::len_utf8)
}

/// Go `utf8.RuneCount`.
fn rune_count(b: &[u8]) -> usize {
    b.utf8_chunks()
        .map(|c| c.valid().chars().count() + c.invalid().len())
        .sum()
}

/// Go `unicode.IsSpace`.
fn is_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

/// The test in Go `InvalidTextError.Error` for quoting with escapes.
fn needs_escape(b: &[u8]) -> bool {
    b.utf8_chunks().any(|c| {
        !c.invalid().is_empty()
            || c.valid()
                .chars()
                .any(|r| r == '`' || r == '\u{fffd}' || is_space(r) || !is_print(r))
    })
}

/// Go `jsonwire.QuoteRune`: the first rune of `b` single-quoted, an
/// invalid byte as `'\xNN'`.
fn quote_rune_bytes(b: &[u8]) -> String {
    match std::str::from_utf8(&b[..first_rune_len(b)])
        .ok()
        .and_then(|s| s.chars().next())
    {
        Some(c) => quote_rune(c),
        None => format!("'\\x{:x}'", b[0]),
    }
}

/// Go `strconv.QuoteRune`.
fn quote_rune(c: char) -> String {
    match c {
        '\'' => "'\\''".into(),
        '"' => "'\"'".into(),
        _ => {
            let q = quote(c.encode_utf8(&mut [0; 4]));
            format!("'{}'", &q[1..q.len() - 1])
        }
    }
}

// ---------------------------------------------------------------------------
// stringify

fn stringify_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let indent = match &a[1] {
        Value::Nil => 0,
        Value::Int(n) if *n < 0 => {
            return Err(nat::fail(
                "E0403",
                vec![
                    nat::s("indent"),
                    nat::s("a non-negative int"),
                    Arg::Str(format(&a[1])),
                ],
            ));
        }
        Value::Int(n) => *n,
        v => return Err(nat::wrong_arg("indent", "int | nil", v)),
    };
    let mut out = String::new();
    encode(&mut out, &a[0], indent)?;
    Ok(Value::Str(Rc::new(out)))
}

/// Go `quoteJSON`: `encoding/json` without HTML escaping. U+2028 and
/// U+2029 are still escaped (jsontext `EscapeForJS`); a fung string is
/// valid UTF-8, so nothing needs replacing.
fn quote_json(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Go writes a line break and `strings.Repeat(" ", indent*depth)`; the
/// product wraps like Go's `int`, and a count Go cannot allocate panics
/// there (`maxAlloc` is 2^48 bytes), which its VM reports as E0801.
fn newline(out: &mut String, indent: i64, depth: i64) -> Result<(), Stop> {
    if indent > 0 {
        let count = indent.wrapping_mul(depth);
        if count < 0 {
            return Err(nat::fail(
                "E0801",
                vec![nat::s("strings: negative Repeat count")],
            ));
        }
        if count > 1 << 48 {
            return Err(nat::fail(
                "E0801",
                vec![nat::s("runtime error: makeslice: len out of range")],
            ));
        }
        out.push('\n');
        out.extend(std::iter::repeat_n(' ', count as usize));
    }
    Ok(())
}

/// A container being written.
enum Items<'a> {
    List(ListIter<'a>),
    Tuple(std::slice::Iter<'a, Value>),
    Map(MapIter<'a>),
}

struct Level<'a> {
    items: Items<'a>,
    depth: i64,
    first: bool,
}

/// Go `encodeJSON` (06: stringify 대응표) with an explicit stack: maps
/// keep their order; indent 0 is one line, otherwise nested values go on
/// their own lines.
fn encode(out: &mut String, v: &Value, indent: i64) -> Result<(), Stop> {
    let colon = if indent > 0 { ": " } else { ":" };
    let mut stack: Vec<Level<'_>> = Vec::new();
    let mut next = Some((v, 0i64));
    loop {
        if let Some((v, depth)) = next.take() {
            let opened = match v {
                Value::Nil => {
                    out.push_str("null");
                    None
                }
                Value::Bool(_) | Value::Int(_) | Value::Float(_) => {
                    out.push_str(&format(v));
                    None
                }
                Value::Str(s) => {
                    quote_json(out, s);
                    None
                }
                Value::Sym(s) => {
                    quote_json(out, s.name());
                    None
                }
                Value::Time(t) => {
                    quote_json(out, &t.iso());
                    None
                }
                Value::List(l) if l.is_empty() => {
                    out.push_str("[]");
                    None
                }
                Value::List(l) => Some((b'[', Items::List(l.iter()))),
                Value::Tuple(t) if t.is_empty() => {
                    out.push_str("[]");
                    None
                }
                Value::Tuple(t) => Some((b'[', Items::Tuple(t.iter()))),
                Value::Map(m) if m.is_empty() => {
                    out.push_str("{}");
                    None
                }
                Value::Map(m) => Some((b'{', Items::Map(m.iter()))),
                _ => {
                    return Err(nat::fail(
                        "E0513",
                        vec![Arg::Str(type_name(v)), nat::s("JSON")],
                    ));
                }
            };
            if let Some((open, items)) = opened {
                out.push(open as char);
                stack.push(Level {
                    items,
                    depth,
                    first: true,
                });
            }
        }
        let Some(top) = stack.last_mut() else {
            return Ok(());
        };
        let (key, item) = match &mut top.items {
            Items::List(it) => (None, it.next()),
            Items::Tuple(it) => (None, it.next()),
            Items::Map(it) => match it.next() {
                Some((k, v)) => (Some(k), Some(v)),
                None => (None, None),
            },
        };
        let depth = top.depth;
        match item {
            Some(item) => {
                if !top.first {
                    out.push(',');
                }
                top.first = false;
                newline(out, indent, depth + 1)?;
                if let Some(k) = key {
                    quote_json(out, k);
                    out.push_str(colon);
                }
                next = Some((item, depth + 1));
            }
            None => {
                let close = if matches!(top.items, Items::Map(_)) {
                    '}'
                } else {
                    ']'
                };
                stack.pop();
                newline(out, indent, depth)?;
                out.push(close);
            }
        }
    }
}

#[cfg(test)]
#[path = "json_tests.rs"]
mod tests;
