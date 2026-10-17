//! Grouping tokens into source lines (Go `lines.go`).

use std::collections::{HashMap, HashSet};

use super::{Cmt, Tok};
use crate::lexer::Kind;

/// One source line: its tokens (never newlines), an optional trailing
/// comment, and what layout decides for it. A blank line has neither
/// tokens nor comment; a comment line has only the comment.
#[derive(Clone, Debug)]
pub(super) struct Line {
    pub toks: Vec<Tok>,
    /// The trailing comment (empty for none), or a comment line's text.
    pub comment: Vec<u8>,
    pub src_line: u32,
    /// In levels, decided by layout.
    pub indent: usize,
    /// Index of the `=>` token to align.
    pub arrow: Option<usize>,
    /// The last token is a block-opening `:` (blank-line rule).
    pub opens_block: bool,
}

impl Line {
    fn new(n: u32) -> Self {
        Line {
            toks: Vec::new(),
            comment: Vec::new(),
            src_line: n,
            indent: 0,
            arrow: None,
            opens_block: false,
        }
    }

    pub fn blank(&self) -> bool {
        self.toks.is_empty() && self.comment.is_empty()
    }

    pub fn is_comment(&self) -> bool {
        self.toks.is_empty() && !self.comment.is_empty()
    }
}

fn newlines(s: &[u8]) -> u32 {
    s.iter().filter(|b| **b == b'\n').count() as u32
}

/// Groups tokens by the source line they start on and attaches comments:
/// a comment on a line that has tokens is its trailing comment, otherwise
/// it is a comment line. Lines inside a multi-line token (a raw string)
/// are not lines of their own, and a final newline makes no line.
pub(super) fn split_lines(src: &[u8], toks: &[Tok], comments: &[Cmt]) -> Vec<Line> {
    let mut by_line: HashMap<u32, Line> = HashMap::new();
    let mut covered: HashSet<u32> = HashSet::new();
    // A line inside a multi-line token → the token's first line.
    let mut owner: HashMap<u32, u32> = HashMap::new();
    let mut last = 0;
    // What follows a raw string on its last line belongs to the line it
    // started on.
    let mut add = |by_line: &mut HashMap<u32, Line>, owner: &HashMap<u32, u32>, n: u32| -> u32 {
        let n = owner.get(&n).copied().unwrap_or(n);
        by_line.entry(n).or_insert_with(|| {
            if n > last {
                last = n;
            }
            Line::new(n)
        });
        n
    };
    for t in toks {
        if matches!(t.kind, Kind::Newline | Kind::Eof) {
            continue;
        }
        let n = add(&mut by_line, &owner, t.line);
        let l = by_line.get_mut(&n).expect("just added");
        l.toks.push(*t);
        let src_line = l.src_line;
        for i in 1..=newlines(&src[t.off..t.end]) {
            covered.insert(t.line + i);
            owner.insert(t.line + i, src_line);
        }
    }
    for c in comments {
        let n = add(&mut by_line, &owner, c.line);
        by_line.get_mut(&n).expect("just added").comment = c.text.clone();
    }
    let mut total = newlines(src);
    if !src.is_empty() && !src.ends_with(b"\n") {
        total += 1; // the last line has no newline but has content
    }
    total = total.max(last);
    let mut out = Vec::new();
    for n in 1..=total {
        if covered.contains(&n) {
            continue;
        }
        out.push(by_line.remove(&n).unwrap_or_else(|| Line::new(n)));
    }
    out
}
