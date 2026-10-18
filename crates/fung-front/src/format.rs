//! The fung formatter (08): it rewrites a file's whitespace, indentation,
//! blank lines and alignment from the lexer's tokens and comments,
//! keeping the user's line breaks.
//!
//! It works on bytes: the lexer reads the source decoded (one U+FFFD for
//! each invalid UTF-8 sequence, as everywhere in fung), and every token
//! and comment is written back as the bytes it came from, so such bytes
//! survive unchanged. Alignment counts display columns (UAX #11: a wide
//! character takes two, a combining one none; an invalid sequence one).

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
/// the source with a leading byte order mark trimmed (the lexer skips
/// it).
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
    // The output is cut from `src` at token and comment boundaries, so it
    // stays UTF-8.
    Ok(String::from_utf8(out).unwrap_or_else(|e| crate::source::decode_lossy(e.as_bytes())))
}

/// [`format`] on bytes, as the command reads files. A leading byte order
/// mark is skipped like the lexer skips it and never written back; a
/// second one is code the lexer refuses (E0101).
pub fn format_bytes(name: &str, src: &[u8]) -> Result<Vec<u8>, Diag> {
    let lexed = lex_bytes(FILE, src)?;
    parser::parse(&lexed.tokens)?;
    // The lexer's offsets point into the source without the mark.
    let src = src.strip_prefix(BOM).unwrap_or(src);
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

/// Lexes bytes: a leading byte order mark is trimmed, and an invalid
/// UTF-8 sequence reads as one U+FFFD. Offsets and comment texts are
/// mapped back to the trimmed bytes.
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

/// Decodes bytes (one U+FFFD for each invalid UTF-8 sequence). For bytes
/// that are not UTF-8 it also gives, for each offset in the text, the
/// offset of the byte it came from.
fn decode(body: &[u8]) -> (String, Option<Vec<usize>>) {
    if let Ok(s) = std::str::from_utf8(body) {
        return (s.to_string(), None);
    }
    let mut text = String::with_capacity(body.len() + 16);
    let mut map = Vec::with_capacity(body.len() + 16);
    let mut i = 0;
    for chunk in body.utf8_chunks() {
        for (k, _) in chunk.valid().char_indices() {
            let len = chunk.valid()[k..].chars().next().map_or(0, char::len_utf8);
            map.extend(std::iter::repeat_n(i + k, len));
        }
        text.push_str(chunk.valid());
        i += chunk.valid().len();
        if !chunk.invalid().is_empty() {
            text.push('\u{fffd}');
            map.extend(std::iter::repeat_n(i, '\u{fffd}'.len_utf8()));
            i += chunk.invalid().len();
        }
    }
    map.push(body.len());
    (text, Some(map))
}

/// Each character of `s` as (its byte length, its display width); an
/// invalid UTF-8 sequence is one U+FFFD, one column wide.
fn units(s: &[u8]) -> impl Iterator<Item = (usize, usize)> + '_ {
    s.utf8_chunks().flat_map(|chunk| {
        let valid = chunk
            .valid()
            .chars()
            .map(|c| (c.len_utf8(), crate::unicode::width(c)));
        let bad = (!chunk.invalid().is_empty()).then(|| (chunk.invalid().len(), 1));
        valid.chain(bad)
    })
}

/// The columns `s` takes on screen.
pub(crate) fn display_width(s: &[u8]) -> usize {
    units(s).map(|(_, w)| w).sum()
}

/// Renders a line's tokens with the spacing table, and the byte offset
/// of the line's alignable `=>` in the result.
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
            arrow = Some(b.len());
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
