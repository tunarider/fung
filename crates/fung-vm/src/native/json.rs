//! `_native.json` (06): a parser (RFC 8259) that keeps object key order
//! and integers exact, and an encoder over fung values.
//!
//! Neither parsing nor writing recurses: deep input stops at the depth
//! limit, and values of any depth are written with an explicit stack.

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::OnceLock;

use fung_front::diag::{Arg, quote};

use super::nat::{self, native, opt, param};
use crate::prelude::{MAX_CHARS, too_many_chars};
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
// parse

/// The deepest nesting `parse` accepts.
const MAX_DEPTH: usize = 10_000;

/// A parse failure with a 1-based position (line, and column in
/// characters).
#[derive(Debug, PartialEq)]
struct JsonError {
    line: usize,
    col: usize,
    msg: String,
}

/// The line and character column of byte offset `off` in `s`.
fn line_col(s: &str, off: usize) -> (usize, usize) {
    let before = &s[..off.min(s.len())];
    let line = before.matches('\n').count() + 1;
    let start = before.rfind('\n').map_or(0, |i| i + 1);
    (line, before[start..].chars().count() + 1)
}

/// An array or object being read.
enum Open {
    List(ListBuilder),
    Map {
        map: MapBuilder,
        seen: HashSet<Rc<String>>,
        key: Option<Rc<String>>,
    },
}

/// Reads one JSON value (RFC 8259) and nothing after it. Arrays and
/// objects are kept on a stack, so deep input needs no recursion.
struct Parser<'a> {
    s: &'a str,
    b: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, at: usize, msg: impl Into<String>) -> JsonError {
        let (line, col) = line_col(self.s, at);
        JsonError {
            line,
            col,
            msg: msg.into(),
        }
    }

    /// The error for what is at `at`: the end of input, or that character.
    fn unexpected(&self, at: usize) -> JsonError {
        match self.s[at..].chars().next() {
            None => self.err(at, "unexpected end of input"),
            Some(c) => {
                let q = quote(c.encode_utf8(&mut [0; 4]));
                self.err(at, format!("unexpected character '{}'", &q[1..q.len() - 1]))
            }
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.b.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn literal(&mut self, word: &str, v: Value) -> Result<Value, JsonError> {
        for &w in word.as_bytes() {
            if self.peek() != Some(w) {
                return Err(self.unexpected(self.i));
            }
            self.i += 1;
        }
        Ok(v)
    }

    /// Four hex digits after `\u`.
    fn hex4(&mut self, at: usize) -> Result<u32, JsonError> {
        let h = self
            .s
            .get(self.i..self.i + 4)
            .filter(|h| h.bytes().all(|c| c.is_ascii_hexdigit()));
        let Some(h) = h else {
            return Err(self.err(at, "invalid \\u escape"));
        };
        self.i += 4;
        Ok(u32::from_str_radix(h, 16).expect("hex digits"))
    }

    /// A string, the opening quote at `self.i`.
    fn string(&mut self) -> Result<String, JsonError> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let at = self.i;
            match self.peek() {
                None => return Err(self.err(at, "unexpected end of input")),
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.i += 1;
                    let c = match self.peek() {
                        Some(b'"') => '"',
                        Some(b'\\') => '\\',
                        Some(b'/') => '/',
                        Some(b'b') => '\u{8}',
                        Some(b'f') => '\u{c}',
                        Some(b'n') => '\n',
                        Some(b'r') => '\r',
                        Some(b't') => '\t',
                        Some(b'u') => {
                            self.i += 1;
                            let hi = self.hex4(at)?;
                            let cp = if (0xD800..0xDC00).contains(&hi) {
                                if !self.s[self.i..].starts_with("\\u") {
                                    return Err(self.err(at, "unpaired surrogate"));
                                }
                                self.i += 2;
                                let lo = self.hex4(at)?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(self.err(at, "unpaired surrogate"));
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else if (0xDC00..0xE000).contains(&hi) {
                                return Err(self.err(at, "unpaired surrogate"));
                            } else {
                                hi
                            };
                            out.push(char::from_u32(cp).expect("a scalar value"));
                            continue;
                        }
                        _ => return Err(self.err(at, "invalid escape")),
                    };
                    self.i += 1;
                    out.push(c);
                }
                Some(c) if c < 0x20 => return Err(self.err(at, "control character in a string")),
                Some(_) => {
                    let c = self.s[at..].chars().next().expect("a character");
                    self.i += c.len_utf8();
                    out.push(c);
                }
            }
        }
    }

    /// A number: an int when it has no fraction or exponent (it must fit
    /// 64 bits; 06: 조용히 float로 바꾸지 않는다), else a finite float.
    fn number(&mut self) -> Result<Value, JsonError> {
        let start = self.i;
        let digits = |p: &mut Self| {
            let from = p.i;
            while matches!(p.peek(), Some(b'0'..=b'9')) {
                p.i += 1;
            }
            p.i > from
        };
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        let int_at = self.i;
        if !digits(self) {
            return Err(self.unexpected(self.i));
        }
        // No leading zeros: the digit after a leading `0` is at fault.
        if self.b[int_at] == b'0' && self.i - int_at > 1 {
            return Err(self.err(int_at + 1, "invalid number"));
        }
        let mut float = false;
        if self.peek() == Some(b'.') {
            self.i += 1;
            float = true;
            if !digits(self) {
                return Err(self.unexpected(self.i));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            float = true;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !digits(self) {
                return Err(self.unexpected(self.i));
            }
        }
        let lit = &self.s[start..self.i];
        let out_of_range = || self.err(start, "number out of range");
        if float {
            match lit.parse::<f64>() {
                Ok(f) if f.is_finite() => Ok(Value::Float(f)),
                _ => Err(out_of_range()),
            }
        } else {
            lit.parse::<i64>()
                .map(Value::Int)
                .map_err(|_| out_of_range())
        }
    }

    /// An object key and its `:`, the key's quote at `self.i`.
    fn key(&mut self, seen: &mut HashSet<Rc<String>>) -> Result<Rc<String>, JsonError> {
        let at = self.i;
        if self.peek() != Some(b'"') {
            return Err(match self.peek() {
                None => self.unexpected(at),
                Some(_) => self.err(at, "expected a string for an object key"),
            });
        }
        let k = Rc::new(self.string()?);
        if !seen.insert(k.clone()) {
            return Err(self.err(at, format!("duplicate key {}", quote(&k))));
        }
        self.skip_ws();
        match self.peek() {
            Some(b':') => self.i += 1,
            None => return Err(self.unexpected(self.i)),
            Some(_) => return Err(self.err(self.i, "expected `:` after an object key")),
        }
        self.skip_ws();
        Ok(k)
    }
}

/// `s` as one JSON value.
fn decode(s: &str) -> Result<Value, JsonError> {
    let mut p = Parser {
        s,
        b: s.as_bytes(),
        i: 0,
    };
    let mut open: Vec<Open> = Vec::new();
    p.skip_ws();
    loop {
        // A value starts here: a scalar, or a container that may be empty.
        let at = p.i;
        let mut done = match p.peek() {
            Some(b'[' | b'{') if open.len() == MAX_DEPTH => {
                return Err(p.err(at, format!("nesting too deep (limit {MAX_DEPTH})")));
            }
            Some(b'[') => {
                p.i += 1;
                p.skip_ws();
                if p.peek() == Some(b']') {
                    p.i += 1;
                    Value::List(Rc::new(ListBuilder::with_capacity(0).finish()))
                } else {
                    open.push(Open::List(ListBuilder::with_capacity(0)));
                    continue;
                }
            }
            Some(b'{') => {
                p.i += 1;
                p.skip_ws();
                if p.peek() == Some(b'}') {
                    p.i += 1;
                    Value::Map(Rc::new(MapBuilder::default().finish()))
                } else {
                    let mut seen = HashSet::new();
                    let key = p.key(&mut seen)?;
                    open.push(Open::Map {
                        map: MapBuilder::default(),
                        seen,
                        key: Some(key),
                    });
                    continue;
                }
            }
            Some(b'"') => Value::str(&p.string()?),
            Some(b't') => p.literal("true", Value::Bool(true))?,
            Some(b'f') => p.literal("false", Value::Bool(false))?,
            Some(b'n') => p.literal("null", Value::Nil)?,
            Some(b'-' | b'0'..=b'9') => p.number()?,
            _ => return Err(p.unexpected(at)),
        };
        // Close containers until one wants another value.
        loop {
            p.skip_ws();
            let Some(top) = open.last_mut() else {
                if p.i < s.len() {
                    return Err(p.err(p.i, "unexpected text after the value"));
                }
                return Ok(done);
            };
            let (close, expect) = match top {
                Open::List(b) => {
                    b.push(done);
                    (b']', "expected `,` or `]`")
                }
                Open::Map { map, key, .. } => {
                    map.set(key.take().expect("a key"), done);
                    (b'}', "expected `,` or `}`")
                }
            };
            match p.peek() {
                Some(b',') => {
                    p.i += 1;
                    p.skip_ws();
                    if let Open::Map { seen, key, .. } = top {
                        *key = Some(p.key(seen)?);
                    }
                    break;
                }
                Some(c) if c == close => {
                    p.i += 1;
                    done = match open.pop().expect("an open container") {
                        Open::List(b) => Value::List(Rc::new(b.finish())),
                        Open::Map { map, .. } => Value::Map(Rc::new(map.finish())),
                    };
                }
                None => return Err(p.unexpected(p.i)),
                Some(_) => return Err(p.err(p.i, expect)),
            }
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
    if out.len() as i64 > MAX_CHARS && out.chars().count() as i64 > MAX_CHARS {
        return Err(too_many_chars());
    }
    Ok(Value::Str(Rc::new(out)))
}

/// A JSON string (RFC 8259): only `"`, `\` and characters below U+0020
/// are escaped; a fung string is valid UTF-8, so nothing needs replacing.
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
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A line break and `indent * depth` spaces; when the output would pass
/// the string limit it is E0806 (02) before anything is written.
pub(super) fn newline(out: &mut String, indent: i64, depth: i64) -> Result<(), Stop> {
    if indent > 0 {
        let count = indent
            .checked_mul(depth)
            .filter(|&c| c <= MAX_CHARS)
            .ok_or_else(too_many_chars)?;
        // Bytes are at least the characters, so count them only when the
        // bytes alone pass the limit.
        let fits = |n: usize| (n as i64).saturating_add(1 + count) <= MAX_CHARS;
        if !fits(out.len()) && !fits(out.chars().count()) {
            return Err(too_many_chars());
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

/// `stringify` (06: stringify 대응표) with an explicit stack: maps
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
