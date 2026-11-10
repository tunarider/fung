//! The fung formatter (08): it rewrites a file's whitespace, indentation,
//! blank lines and alignment from the lexer's tokens and comments,
//! keeping the user's line breaks.
//!
//! Its source is text, as for `run` and `check`: a file that is not
//! UTF-8 is refused before it gets here (D31). Every token and comment
//! is written back as the bytes it came from. Alignment counts display
//! columns (UAX #11: a wide character takes two, a combining one none).

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

/// Rewrites `src` in the single fung style (08). A syntax error comes
/// back as is and nothing is formatted; an output whose tokens differ
/// from the input is an internal error (E0801), never returned as text.
/// `name` is the file's name for that error; positions point into
/// [`FILE`]. The source is text like the compiler's (the command reads
/// it with `decode_source`, so a file that is not UTF-8 never gets here,
/// D31). A leading byte order mark is skipped like the lexer skips it and
/// never written back; a second one is code the lexer refuses (E0101).
pub fn format(name: &str, src: &str) -> Result<String, Diag> {
    let lexed = lex_str(FILE, src)?;
    parser::parse(&lexed.tokens)?;
    // The lexer's offsets point into the source without the mark.
    let src = src.strip_prefix('\u{feff}').unwrap_or(src).as_bytes();
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
    // The output is cut from `src` at token and comment boundaries, so it
    // stays UTF-8.
    Ok(String::from_utf8(out).expect("the output is cut from UTF-8 text at token boundaries"))
}

/// Lexes text; a leading byte order mark is trimmed.
fn lex_str(file: FileId, src: &str) -> Result<Lexed, Diag> {
    let body = src.strip_prefix('\u{feff}').unwrap_or(src);
    let start = Pos {
        file,
        line: 1,
        col: 1,
    };
    let (tokens, comments) = lexer::lex_at(file, body, start, 0)?;
    let toks = tokens
        .iter()
        .map(|t| Tok {
            kind: t.kind,
            line: t.pos.line,
            off: t.off,
            end: t.end,
        })
        .collect();
    let comments = comments
        .iter()
        .map(|c| Cmt {
            line: c.pos.line,
            text: body.as_bytes()[c.off..c.off + c.text.len()].to_vec(),
        })
        .collect();
    Ok(Lexed {
        tokens,
        toks,
        comments,
    })
}

/// The columns `s` takes on screen: its grapheme clusters' widths
/// (`unicode::display_width`, L17). `s` is a part of the source or of the
/// output cut at token boundaries, so it is UTF-8.
pub(crate) fn display_width(s: &[u8]) -> usize {
    crate::unicode::display_width(
        std::str::from_utf8(s).expect("a cut at token boundaries is UTF-8"),
    )
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
    let Ok(b) = std::str::from_utf8(out)
        .map_err(drop)
        .and_then(|t| lex_str(file, t).map_err(drop))
    else {
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
