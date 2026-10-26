//! Aligning match arms and trailing comments, by display column.

use super::display_width;

/// A line ready for assembly.
pub(super) struct Rendered {
    pub indent: usize,
    pub code: Vec<u8>,
    pub comment: Vec<u8>,
    /// Byte offset of a `=>` to align.
    pub arrow: Option<usize>,
    pub blank: bool,
    /// Starts with `;;` or a closing bracket.
    pub closing: bool,
}

/// Pads consecutive lines so match arms share the `=>` column (pattern
/// width max + 1) and trailing comments start in one column (code width
/// max + 2; emit adds the two spaces). A run is broken by a blank line, a
/// comment line, a line without the feature, or a different indent
/// (spec: 정렬). Arms go first so comment columns see final code.
pub(super) fn align(rs: &mut [Rendered]) {
    // An arm whose body sits on deeper lines, or ends with a closing line
    // at the arm's own indent (`;;`, `;;)`), still belongs to the run.
    align_runs(
        rs,
        |r, run_indent| {
            !r.blank
                && !r.code.is_empty()
                && (r.indent > run_indent || (r.indent == run_indent && r.closing))
        },
        |r| r.arrow.is_some(),
        |r| display_width(&r.code[..r.arrow.unwrap_or(0)]),
        |r, width| {
            let at = r.arrow.unwrap_or(0);
            let n = width.saturating_sub(display_width(&r.code[..at]));
            r.code.splice(at..at, std::iter::repeat_n(b' ', n));
            r.arrow = Some(at + n);
        },
    );
    align_runs(
        rs,
        |_, _| false,
        |r| !r.code.is_empty() && !r.comment.is_empty(),
        |r| display_width(&r.code),
        |r, width| {
            let n = width.saturating_sub(display_width(&r.code));
            r.code.resize(r.code.len() + n, b' ');
        },
    );
}

/// Pads every run of lines that has the feature at one indent; `through`
/// says which non-member lines may sit inside a run without ending it.
fn align_runs(
    rs: &mut [Rendered],
    through: impl Fn(&Rendered, usize) -> bool,
    has: impl Fn(&Rendered) -> bool,
    width: impl Fn(&Rendered) -> usize,
    pad: impl Fn(&mut Rendered, usize),
) {
    // A line already padded as a member of a run starts none of its own:
    // its run is done (starting one at every member scanned the rest of
    // the run again, quadratic in its length).
    let mut aligned = vec![false; rs.len()];
    let mut i = 0;
    while i < rs.len() {
        if aligned[i] || !has(&rs[i]) {
            i += 1;
            continue;
        }
        let indent = rs[i].indent;
        let mut members = Vec::new();
        for (j, r) in rs.iter().enumerate().skip(i) {
            if has(r) && r.indent == indent {
                members.push(j);
            } else if !through(r, indent) {
                break;
            }
        }
        let max = members.iter().map(|&k| width(&rs[k])).max().unwrap_or(0);
        for &k in &members {
            pad(&mut rs[k], max);
            aligned[k] = true;
        }
        // Lines passed through may start runs of their own (an inner
        // match), so continue right after this run's first member.
        i += 1;
    }
}
