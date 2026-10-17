//! The fung formatter (08), a port of Go `internal/format`: it rewrites a
//! file's whitespace, indentation, blank lines and alignment from the
//! lexer's tokens and comments, keeping the user's line breaks.
//!
//! It works on bytes, as Go does on its strings: the lexer reads the
//! source decoded like Go (one U+FFFD per byte that is not UTF-8), and
//! every token and comment is written back as the bytes it came from, so
//! such bytes survive unchanged.

use crate::diag::{Arg, Diag, error};
use crate::lexer::{self, Kind, Token};
use crate::parser;
use crate::source::{FileId, Pos};

mod align;
mod layout;
mod lines;
mod spacing;
#[cfg(test)]
mod tests;

use align::{Rendered, align};
use layout::{is_closing, layout};
use lines::{Line, split_lines};
use spacing::{gap, prefix_context};

/// The file a [`format`] diagnostic points into: the caller registers
/// the source first in its `SourceMap`.
pub const FILE: FileId = FileId(0);

/// A token as the formatter sees it: `off..end` are byte offsets into
/// the source with any byte order mark trimmed (Go's lexer trims it), and
/// they are used on the untrimmed source, as Go does.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tok {
    kind: Kind,
    line: u32,
    off: usize,
    end: usize,
}

/// A comment: its line and its bytes.
struct Cmt {
    line: u32,
    text: Vec<u8>,
}

/// What lexing a source gave: the lexer's tokens (for the parser) and the
/// formatter's view of them and of the comments.
struct Lexed {
    tokens: Vec<Token>,
    toks: Vec<Tok>,
    comments: Vec<Cmt>,
}

const BOM: &[u8] = "\u{feff}".as_bytes();

/// Rewrites `src` in the single fung style (08). A syntax error comes
/// back as is and nothing is formatted; an output whose tokens differ
/// from the input is an internal error (E0801), never returned as text.
/// `name` is the file's name for that error; positions point into
/// [`FILE`].
pub fn format(name: &str, src: &str) -> Result<String, Diag> {
    let out = format_bytes(name, src.as_bytes())?;
    // Only a source with a byte order mark could give bytes that are not
    // UTF-8, and the safeguard refuses those (see `format_bytes`).
    Ok(String::from_utf8(out).unwrap_or_else(|e| crate::source::decode_go(e.as_bytes())))
}

/// [`format`] on bytes, as the command reads files. Go's lexer trims a
/// leading byte order mark but its formatter slices the untrimmed
/// source with the lexer's offsets; this keeps that (a file with a mark
/// and code fails the safeguard, one with only comments loses the mark).
pub fn format_bytes(name: &str, src: &[u8]) -> Result<Vec<u8>, Diag> {
    let lexed = lex_bytes(FILE, src)?;
    parser::parse(&lexed.tokens)?;
    let mut lines = split_lines(src, &lexed.toks, &lexed.comments);
    layout(&mut lines);
    let lines = tidy_blank(lines);
    let out = emit(src, &lines);
    if !same(FILE, src, &lexed, &out) {
        return Err(error(
            "E0801",
            Pos {
                file: FILE,
                line: 0,
                col: 0,
            },
            vec![Arg::Str(format!("formatter changed the code of {name}"))],
        ));
    }
    Ok(out)
}

/// Lexes bytes like Go's lexer reads a string: a leading byte order mark
/// is trimmed, and a byte that is not UTF-8 reads as one U+FFFD. Offsets
/// and comment texts are mapped back to the trimmed bytes.
fn lex_bytes(file: FileId, src: &[u8]) -> Result<Lexed, Diag> {
    let body = src.strip_prefix(BOM).unwrap_or(src);
    let (text, map) = decode(body);
    let at = |off: usize| map.as_ref().map_or(off, |m| m[off]);
    let start = Pos {
        file,
        line: 1,
        col: 1,
    };
    let (tokens, comments) = lexer::lex_at(file, &text, start, 0)?;
    let toks = tokens
        .iter()
        .map(|t| Tok {
            kind: t.kind,
            line: t.pos.line,
            off: at(t.off),
            end: at(t.end),
        })
        .collect();
    let comments = comments
        .iter()
        .map(|c| Cmt {
            line: c.pos.line,
            text: body[at(c.off)..at(c.off + c.text.len())].to_vec(),
        })
        .collect();
    Ok(Lexed {
        tokens,
        toks,
        comments,
    })
}

/// Decodes bytes like Go (one U+FFFD per invalid byte). For bytes that
/// are not UTF-8 it also gives, for each offset in the text, the offset
/// of the byte it came from.
fn decode(body: &[u8]) -> (String, Option<Vec<usize>>) {
    if let Ok(s) = std::str::from_utf8(body) {
        return (s.to_string(), None);
    }
    let mut text = String::with_capacity(body.len() + 16);
    let mut map = Vec::with_capacity(body.len() + 16);
    let mut i = 0;
    while i < body.len() {
        let w = go_rune_width(&body[i..]);
        let c = std::str::from_utf8(&body[i..i + w])
            .ok()
            .and_then(|s| s.chars().next())
            .unwrap_or('\u{fffd}');
        for _ in 0..c.len_utf8() {
            map.push(i);
        }
        text.push(c);
        i += w;
    }
    map.push(body.len());
    (text, Some(map))
}

/// The width of the first rune of `s` the way Go decodes it: a valid
/// UTF-8 sequence, or 1 for a byte that does not start one.
pub(crate) fn go_rune_width(s: &[u8]) -> usize {
    let w = match s.first() {
        None => return 0,
        Some(b) if *b < 0x80 => return 1,
        Some(b) if b >> 5 == 0b110 => 2,
        Some(b) if b >> 4 == 0b1110 => 3,
        Some(b) if b >> 3 == 0b11110 => 4,
        Some(_) => return 1,
    };
    if s.len() >= w && std::str::from_utf8(&s[..w]).is_ok() {
        w
    } else {
        1
    }
}

/// Go `utf8.RuneCount`: each invalid byte counts as one rune.
pub(crate) fn rune_count(mut s: &[u8]) -> usize {
    let mut n = 0;
    while !s.is_empty() {
        s = &s[go_rune_width(s)..];
        n += 1;
    }
    n
}

/// Renders a line's tokens with the spacing table, and the rune offset of
/// the line's alignable `=>` in the result.
fn render(src: &[u8], ln: &Line) -> (Vec<u8>, Option<usize>) {
    let mut b = Vec::new();
    let mut arrow = None;
    let mut sign = false;
    let import_line = ln.toks.first().is_some_and(|t| t.kind == Kind::KwImport);
    for (i, t) in ln.toks.iter().enumerate() {
        if i > 0 {
            let n = gap(ln.toks[i - 1].kind, t.kind, sign, import_line);
            b.resize(b.len() + n, b' ');
        }
        if ln.arrow == Some(i) {
            arrow = Some(rune_count(&b));
        }
        b.extend_from_slice(&src[t.off..t.end]);
        sign = t.kind == Kind::Minus && (i == 0 || prefix_context(ln.toks[i - 1].kind));
        if sign && i >= 2 && ln.toks[i - 1].kind.is_keyword() && ln.toks[i - 2].kind == Kind::Dot {
            sign = false; // m.type - 1: the keyword is a field name here (01)
        }
    }
    (b, arrow)
}

/// Drops trailing whitespace from a comment (spec: 줄 끝 공백은 없다).
fn trim_comment(c: &[u8]) -> &[u8] {
    let n = c.iter().rposition(|b| *b != b' ' && *b != b'\t');
    &c[..n.map_or(0, |n| n + 1)]
}

/// Assembles the lines: indentation, code, trailing comment.
fn emit(src: &[u8], lines: &[Line]) -> Vec<u8> {
    let mut rs: Vec<Rendered> = lines
        .iter()
        .map(|ln| {
            let (code, arrow) = render(src, ln);
            Rendered {
                indent: ln.indent,
                code,
                comment: trim_comment(&ln.comment).to_vec(),
                arrow,
                blank: ln.blank(),
                closing: ln.toks.first().is_some_and(|t| is_closing(t.kind)),
            }
        })
        .collect();
    align(&mut rs);
    let mut b = Vec::new();
    for r in &rs {
        if r.blank {
            b.push(b'\n');
            continue;
        }
        b.resize(b.len() + 2 * r.indent, b' ');
        b.extend_from_slice(&r.code);
        if !r.comment.is_empty() {
            if !r.code.is_empty() {
                b.extend_from_slice(b"  ");
            }
            b.extend_from_slice(&r.comment);
        }
        b.push(b'\n');
    }
    b
}

/// The safety property: the output lexes to the same tokens and comments
/// as the input, newlines aside.
fn same(file: FileId, src: &[u8], a: &Lexed, out: &[u8]) -> bool {
    let Ok(b) = lex_bytes(file, out) else {
        return false;
    };
    a.comments.len() == b.comments.len()
        && a.comments
            .iter()
            .zip(&b.comments)
            .all(|(x, y)| trim_comment(&x.text) == trim_comment(&y.text))
        && code(src, &a.toks) == code(out, &b.toks)
}

fn code<'a>(src: &'a [u8], toks: &[Tok]) -> Vec<&'a [u8]> {
    toks.iter()
        .filter(|t| !matches!(t.kind, Kind::Newline | Kind::Eof))
        .map(|t| &src[t.off..t.end])
        .collect()
}

/// Collapses runs of blank lines to one and removes them at the start and
/// end of the file, right after a block-opening `:` and right before a
/// `;;` (spec: 빈 줄과 파일 끝).
fn tidy_blank(lines: Vec<Line>) -> Vec<Line> {
    let mut keep = vec![true; lines.len()];
    let mut last: Option<usize> = None; // the last line kept
    for i in 0..lines.len() {
        let ln = &lines[i];
        if ln.blank() {
            if last.is_none_or(|j| lines[j].blank() || lines[j].opens_block) {
                keep[i] = false;
                continue;
            }
            if next_code(&lines, i).is_some_and(|n| n.toks[0].kind == Kind::DoubleSemi) {
                keep[i] = false;
                continue;
            }
        }
        last = Some(i);
    }
    let mut out: Vec<Line> = lines
        .into_iter()
        .zip(keep)
        .filter_map(|(l, k)| k.then_some(l))
        .collect();
    while out.last().is_some_and(Line::blank) {
        out.pop();
    }
    out
}

/// The next code line after index `from`, skipping blank lines only; a
/// comment line ends the search.
fn next_code(lines: &[Line], from: usize) -> Option<&Line> {
    for l in &lines[from + 1..] {
        if !l.toks.is_empty() {
            return Some(l);
        }
        if !l.blank() {
            return None;
        }
    }
    None
}
