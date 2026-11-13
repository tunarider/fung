//! Diagnostics: catalog-numbered errors and the way fung prints them.

pub(crate) mod catalog;

use crate::source::{Pos, SourceMap};

/// Process exit codes of the fung command.
pub const EXIT_OK: i32 = 0;
pub const EXIT_RUNTIME: i32 = 1;
pub const EXIT_COMPILE: i32 = 2;
pub const EXIT_USAGE: i32 = 3;

/// A value that fills a message template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    Str(String),
    Int(i64),
}

impl Arg {
    fn plain(&self) -> String {
        match self {
            Arg::Str(s) => s.clone(),
            Arg::Int(n) => n.to_string(),
        }
    }
}

/// One reported error or warning.
#[derive(Clone, Debug)]
pub struct Diag {
    pub code: String,
    pub pos: Pos,
    /// Characters to underline; 0 means 1.
    pub len: usize,
    pub args: Vec<Arg>,
    /// Overrides the catalog help line when set.
    pub help: Option<String>,
    /// Extra lines after the excerpt.
    pub notes: Vec<String>,
    /// Runtime call path, innermost first; set by the VM.
    pub call_path: Vec<CallSite>,
}

/// One frame of the call path shown with runtime errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallSite {
    pub file: String,
    pub line: u32,
    /// `<main>` for the entry file.
    pub func: String,
    /// Tail calls that reused this frame.
    pub elided: u64,
}

pub fn error(code: &str, pos: Pos, args: Vec<Arg>) -> Diag {
    Diag {
        code: code.to_string(),
        pos,
        len: 0,
        args,
        help: None,
        notes: Vec::new(),
        call_path: Vec::new(),
    }
}

impl Diag {
    pub fn span(mut self, n: usize) -> Self {
        self.len = n;
        self
    }

    pub fn with_help(mut self, h: &str) -> Self {
        self.help = Some(h.to_string());
        self
    }

    /// Fills in the position if the error does not have one yet.
    pub fn with_pos(mut self, p: Pos) -> Self {
        if self.pos.is_zero() {
            self.pos = p;
        }
        self
    }

    /// Records the call path if none was recorded yet. The innermost run
    /// loop records it; outer loops leave it alone.
    pub fn with_call_path(mut self, p: Vec<CallSite>) -> Self {
        if self.call_path.is_empty() {
            self.call_path = p;
        }
        self
    }
}

/// Renders the catalog template for a code: `%s`, `%d`, `%q` (a fung
/// string literal) and `%v` verbs.
pub fn message(code: &str, args: &[Arg]) -> String {
    let Some(entry) = catalog::entry(code) else {
        let list: Vec<String> = args.iter().map(Arg::plain).collect();
        return format!("unknown error {} [{}]", code, list.join(" "));
    };
    let mut out = String::new();
    let mut next = 0;
    let mut chars = entry.message.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let Some(verb) = chars.next() else {
            out.push_str("%!(NOVERB)");
            break;
        };
        if verb == '%' {
            out.push('%');
            continue;
        }
        let Some(arg) = args.get(next) else {
            out.push_str(&format!("%!{verb}(MISSING)"));
            continue;
        };
        next += 1;
        match (verb, arg) {
            // an argument from the source or a program shows a control
            // character as its escape, as the excerpt does (FE42)
            ('s' | 'v', a) => out.push_str(&escaped(&a.plain())),
            ('q', Arg::Str(s)) => out.push_str(&quote(s)),
            ('d', Arg::Int(n)) => out.push_str(&n.to_string()),
            (v, Arg::Str(s)) => out.push_str(&format!("%!{v}(string={s})")),
            (v, Arg::Int(n)) => out.push_str(&format!("%!{v}(int={n})")),
        }
    }
    out
}

/// `error[E0204]: ...` or `warning[W0101]: ...`.
pub fn headline(d: &Diag) -> String {
    let kind = if d.code.starts_with('W') {
        "warning"
    } else {
        "error"
    };
    format!("{kind}[{}]: {}", d.code, message(&d.code, &d.args))
}

/// Renders a diagnostic the way the fung command prints it.
pub fn format(d: &Diag, sources: &SourceMap) -> String {
    let mut b = headline(d);
    b.push('\n');
    if !d.pos.is_zero() {
        b.push_str(&format!(
            "  --> {}:{}:{}\n",
            sources.name(d.pos.file),
            d.pos.line,
            d.pos.col
        ));
        if !sources.text(d.pos.file).is_empty()
            && let Some(line) = sources.line(d.pos.file, d.pos.line)
        {
            write_excerpt(&mut b, d, line);
        }
    }
    for note in &d.notes {
        b.push_str(&format!("  {note}\n"));
    }
    if !d.call_path.is_empty() {
        write_call_path(&mut b, &d.call_path);
    }
    let help = match &d.help {
        Some(h) => h.as_str(),
        None => catalog::entry(&d.code).map_or("", |e| e.help.as_str()),
    };
    if !help.is_empty() {
        b.push_str(&format!(" help: {help}\n"));
    }
    b
}

/// Long call paths (deep recursion) show the innermost and outermost frames.
const CALL_PATH_HEAD: usize = 8;
const CALL_PATH_TAIL: usize = 2;

fn write_call_path(b: &mut String, path: &[CallSite]) {
    b.push_str(" call path (innermost first):\n");
    let write = |b: &mut String, c: &CallSite| {
        let func = if c.func == "<main>" {
            "<entry>"
        } else {
            &c.func
        };
        let place = format!("{}:{}", c.file, c.line);
        b.push_str(&format!("   {place:<10} {func}"));
        if c.elided > 0 {
            let calls = if c.elided == 1 { "call" } else { "calls" };
            b.push_str(&format!(" ({} tail {calls} elided)", c.elided));
        }
        b.push('\n');
    };
    if path.len() <= CALL_PATH_HEAD + CALL_PATH_TAIL + 2 {
        for c in path {
            write(b, c);
        }
        return;
    }
    for c in &path[..CALL_PATH_HEAD] {
        write(b, c);
    }
    let more = path.len() - CALL_PATH_HEAD - CALL_PATH_TAIL;
    b.push_str(&format!("   \u{2026} {more} more\n"));
    for c in &path[path.len() - CALL_PATH_TAIL..] {
        write(b, c);
    }
}

const TAB_WIDTH: usize = 4;

/// How many characters of a long line the excerpt shows on each side of
/// the place it points at (FE53).
const EXCERPT_CONTEXT: usize = 60;

/// A line of at most this many characters is shown whole: only a line
/// no one wrote by hand is cut.
const EXCERPT_WHOLE: usize = 200;

/// Appends the gutter, the source line, and the caret line. The caret
/// line is drawn in terminal columns (L17): tabs are expanded, and the
/// text before the caret and under it is measured by grapheme cluster
/// (`unicode::display_width`), so the carets land under a wide character
/// or an emoji sequence. `d.pos.col` and `d.len` count characters.
fn write_excerpt(b: &mut String, d: &Diag, raw: &str) {
    let line_no = d.pos.line as usize;
    let line = raw.strip_suffix('\r').unwrap_or(raw);
    let num = line_no.to_string();
    let gutter = " ".repeat(num.len());

    let width = |s: &str| crate::unicode::display_width(&visible(s));
    // the lexer counts columns after a leading BOM (zero columns on screen)
    let line = line.strip_prefix('\u{feff}').unwrap_or(line);
    // Only the part of the line the excerpt can show is taken apart
    // (FE75): a long line with many errors is not copied for each.
    let want = (d.pos.col as usize).saturating_sub(1);
    let reach = want
        .saturating_add(d.len.max(1))
        .saturating_add(3 * EXCERPT_CONTEXT)
        .max(EXCERPT_WHOLE + 1);
    // `chars` holds the line's characters from `base` on, `total` is how
    // many the line has.
    let (base, chars, total): (usize, Vec<char>, usize) = if line.is_ascii() {
        let from = want.min(line.len()).saturating_sub(EXCERPT_CONTEXT);
        let from = if line.len() <= EXCERPT_WHOLE { 0 } else { from };
        let to = reach.min(line.len());
        (from, line[from..to].chars().collect(), line.len())
    } else {
        let part: Vec<char> = line.chars().take(reach).collect();
        let n = if part.len() < reach {
            part.len()
        } else {
            line.chars().count()
        };
        (0, part, n)
    };
    let part =
        |from: usize, to: usize| -> String { chars[from - base..to - base].iter().collect() };
    let at = want.min(total);
    let len = d.len.max(1);
    let end = (at + len).min(total);
    // A window of the line around the place, `…` where it is cut (FE53):
    // a line of megabytes would print megabytes, twice.
    let (lo, under_end, hi) = if total <= EXCERPT_WHOLE {
        (0, end, total)
    } else {
        let under_end = end.min(at + 2 * EXCERPT_CONTEXT);
        let hi = (under_end + EXCERPT_CONTEXT).min(total);
        (at.saturating_sub(EXCERPT_CONTEXT), under_end, hi)
    };
    let cut_lo = if lo > 0 { "\u{2026}" } else { "" };
    let cut_hi = if hi < total { "\u{2026}" } else { "" };
    let shown = format!("{cut_lo}{}{cut_hi}", visible(&part(lo, hi)));
    let before = part(lo, at);
    let under = part(at, under_end);
    let col = width(cut_lo) + width(&before);
    // past the line's end (an end of input) the carets count characters
    let past = if under_end == end { at + len - end } else { 0 };
    let n = (width(&under) + past).max(1);
    b.push_str(&format!("{gutter} |\n"));
    b.push_str(&format!("{num} | {shown}\n"));
    b.push_str(&format!("{gutter} | {}{}", " ".repeat(col), "^".repeat(n)));
    if let Some(e) = catalog::entry(&d.code)
        && !e.label.is_empty()
    {
        b.push(' ');
        b.push_str(&e.label);
    }
    b.push('\n');
    b.push_str(&format!("{gutter} |\n"));
}

/// A source line as the excerpt draws it: tabs expanded, and a control
/// character as its escape (`\r`, `\u{1B}`), so the line cannot move the
/// terminal's cursor or change its state (FE13).
fn visible(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\t' => out.push_str(&" ".repeat(TAB_WIDTH)),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:X}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// A message's argument with each control character as its escape
/// (`\n`, `\t`, `\u{1B}`), so a headline stays one line (FE42).
fn escaped(s: &str) -> String {
    if !s.chars().any(char::is_control) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:X}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Whether a code belongs to the compile-time categories (syntax E01xx,
/// names E02xx, modules E03xx).
pub fn is_compile_error(code: &str) -> bool {
    code.starts_with("E01") || code.starts_with("E02") || code.starts_with("E03")
}

/// `s` as a fung string literal (01), for messages and value text:
/// `\n` `\t` `\r` `\\` `\"`, `\$` before `{`, and `\u{X}` for what is
/// not printable.
pub fn quote(s: &str) -> String {
    crate::consts::quote_str(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unicode::is_print;

    #[test]
    fn is_print_cases() {
        for u in [0xA0u32, 0x2003, 0x200B, 0x8E2, 0xFDD0, 0xE0001] {
            assert!(!is_print(char::from_u32(u).unwrap()), "U+{u:X}");
        }
        // Every format (Cf) character, by range ends.
        for u in [
            0x890u32, 0x891, 0x110BD, 0x110CD, 0x13430, 0x1343F, 0x1BCA0, 0x1BCA3, 0x1D173, 0x1D17A,
        ] {
            assert!(!is_print(char::from_u32(u).unwrap()), "U+{u:X}");
        }
        for u in [0x8E3u32, 0x13440, 0x1D172, 0x1D17B] {
            assert!(is_print(char::from_u32(u).unwrap()), "U+{u:X}");
        }
        assert!(is_print('한') && is_print('a') && is_print(' '));
    }

    use crate::source::{Pos, SourceMap};

    fn at(sm: &mut SourceMap, name: &str, text: &str, line: u32, col: u32) -> Pos {
        let file = sm.add(name, text);
        Pos { file, line, col }
    }

    #[test]
    fn format_undefined_name_with_label() {
        let mut sm = SourceMap::new();
        let pos = at(&mut sm, "-e", "print(nope)", 1, 7);
        let d = error("E0204", pos, vec![Arg::Str("nope".into())]).span(4);
        assert_eq!(
            format(&d, &sm),
            "error[E0204]: undefined name `nope`\n  --> -e:1:7\n  |\n1 | print(nope)\n  |       ^^^^ not found in this scope\n  |\n"
        );
    }

    #[test]
    fn format_with_catalog_help() {
        let mut sm = SourceMap::new();
        let pos = at(&mut sm, "-e", "y = 1 < 2 < 3", 1, 11);
        let d = error("E0111", pos, vec![]);
        assert_eq!(
            format(&d, &sm),
            "error[E0111]: comparison operators cannot be chained\n  --> -e:1:11\n  |\n1 | y = 1 < 2 < 3\n  |           ^\n  |\n help: split it with `and`: a < b and b < c\n"
        );
    }

    #[test]
    fn excerpt_expands_tabs() {
        let mut sm = SourceMap::new();
        let pos = at(&mut sm, "-e", "\tx = \"\\q\"", 1, 7);
        let d = error("E0103", pos, vec![Arg::Str("`\\q`".into())]);
        assert_eq!(
            format(&d, &sm),
            "error[E0103]: unknown escape `\\q` in string\n  --> -e:1:7\n  |\n1 |     x = \"\\q\"\n  |          ^\n  |\n"
        );
    }

    /// The `-->` column counts characters (an editor's column), but the
    /// caret line is drawn in terminal columns (L17): a wide character
    /// takes two, a ZWJ emoji sequence or `❤️` is one cluster of two.
    #[test]
    fn the_underline_is_drawn_in_columns() {
        let mut sm = SourceMap::new();
        let pos = at(&mut sm, "k.fung", "s = \"한글\"\nprint(없는)\n", 2, 7);
        let d = error("E0204", pos, vec![Arg::Str("없는".into())]).span(2);
        assert_eq!(
            format(&d, &sm),
            "error[E0204]: undefined name `없는`\n  --> k.fung:2:7\n  |\n2 | print(없는)\n  |       ^^^^ not found in this scope\n  |\n"
        );
        for (line, col, spaces) in [
            ("x = \"한글\" + nope", 12, 13),
            (
                "x = \"\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\" + nope",
                15,
                11,
            ),
            ("x = \"\u{2764}\u{FE0F}\" + nope", 12, 11),
            ("\tx = nope", 6, 8),
        ] {
            let mut sm = SourceMap::new();
            let pos = at(&mut sm, "e.fung", &format!("{line}\n"), 1, col);
            let d = error("E0204", pos, vec![Arg::Str("nope".into())]).span(4);
            let got = format(&d, &sm);
            let caret = got.lines().nth(4).unwrap();
            assert_eq!(
                caret,
                format!("  | {}^^^^ not found in this scope", " ".repeat(spaces)),
                "{line}"
            );
        }
    }

    /// A control character in the shown line is drawn as its escape, so
    /// a carriage return or an escape sequence cannot redraw the terminal
    /// (FE13); the caret counts the escape's columns.
    #[test]
    fn excerpt_shows_control_characters() {
        let mut sm = SourceMap::new();
        let pos = at(&mut sm, "cr.fung", "x = 1\ry = )\n", 1, 6);
        let d = error("E0101", pos, vec![Arg::Str("\r".into())]);
        assert_eq!(
            format(&d, &sm),
            "error[E0101]: unexpected character \"\\r\"\n  --> cr.fung:1:6\n  |\n1 | x = 1\\ry = )\n  |      ^^\n  |\n"
        );
        let pos = at(&mut sm, "esc.fung", "s = \"\u{1b}[2J\" + nope\n", 1, 14);
        let d = error("E0204", pos, vec![Arg::Str("nope".into())]).span(4);
        let got = format(&d, &sm);
        assert!(got.contains("1 | s = \"\\u{1B}[2J\" + nope\n"), "{got}");
        let caret = format!("  | {}^^^^ ", " ".repeat(18));
        assert!(got.contains(&caret), "{got}");
    }

    /// FE42: a control character in a message's argument is shown as its
    /// escape too, so the headline stays one line and cannot redraw the
    /// terminal.
    #[test]
    fn message_arguments_show_control_characters() {
        let m = message("E0113", &[Arg::Str("\u{1b}[31ma\nb\tc\r".into())]);
        assert!(m.contains("\\u{1B}[31ma\\nb\\tc\\r"), "{m}");
        assert!(!m.contains('\u{1b}') && !m.contains('\n'), "{m}");
    }

    /// FE53: a long line shows a window around the caret, with `…` where
    /// it was cut, and the caret stays under the place.
    #[test]
    fn a_long_line_is_cut_around_the_caret() {
        let mut sm = SourceMap::new();
        let line = format!("x = [{}] nope", "1, ".repeat(10_000));
        let col = line.find("nope").unwrap() as u32 + 1;
        let pos = at(&mut sm, "long.fung", &format!("{line}\n"), 1, col);
        let d = error("E0204", pos, vec![Arg::Str("nope".into())]).span(4);
        let got = format(&d, &sm);
        assert!(got.len() < 400, "{}", got.len());
        let shown = got.lines().nth(3).unwrap();
        let caret = got.lines().nth(4).unwrap();
        assert!(shown.starts_with("1 | …"), "{shown}");
        assert!(shown.ends_with("] nope"), "{shown}");
        let at_nope = shown.chars().count() - 4;
        assert_eq!(caret.find('^'), Some(at_nope), "{shown}\n{caret}");
        // the other side is cut too
        let line = format!("nope + [{}]", "1, ".repeat(10_000));
        let pos = at(&mut sm, "long2.fung", &format!("{line}\n"), 1, 1);
        let d = error("E0204", pos, vec![Arg::Str("nope".into())]).span(4);
        let got = format(&d, &sm);
        let shown = got.lines().nth(3).unwrap();
        assert!(shown.starts_with("1 | nope + [1, "), "{shown}");
        assert!(shown.ends_with('…'), "{shown}");
        assert!(got.len() < 400, "{}", got.len());
        // a line someone wrote is shown whole
        let line = format!("print({}nope)", "a + ".repeat(45));
        let pos = at(&mut sm, "whole.fung", &format!("{line}\n"), 1, 187);
        let d = error("E0204", pos, vec![Arg::Str("nope".into())]).span(4);
        assert!(format(&d, &sm).contains(&format!("1 | {line}\n")));
    }

    /// Each excerpt finds its line without reading the file from the
    /// start, so a file's many diagnostics show in linear time (FE16).
    #[test]
    fn many_excerpts_are_linear() {
        let lines = 100_000;
        let text = "x = 1 # a line of some length\n".repeat(lines);
        let mut sm = SourceMap::new();
        let file = sm.add("big.fung", &text);
        let t = std::time::Instant::now();
        for line in 1..=lines as u32 {
            let pos = Pos { file, line, col: 1 };
            let d = error("E0204", pos, vec![Arg::Str("x".into())]);
            assert!(format(&d, &sm).contains("| x = 1 #"));
        }
        assert!(
            t.elapsed() < std::time::Duration::from_secs(3),
            "{:?}",
            t.elapsed()
        );
    }

    #[test]
    fn crlf_and_bom_lines() {
        let mut sm = SourceMap::new();
        let pos = at(&mut sm, "crlf.fung", "x = 1\r\ny = )\r\n", 2, 5);
        let d = error("E0110", pos, vec![Arg::Str("`)`".into())]);
        assert_eq!(
            format(&d, &sm),
            "error[E0110]: unexpected `)`\n  --> crlf.fung:2:5\n  |\n2 | y = )\n  |     ^\n  |\n"
        );
        let pos = at(&mut sm, "bom.fung", "\u{feff}x = )\n", 1, 5);
        let d = error("E0110", pos, vec![Arg::Str("`)`".into())]);
        assert_eq!(
            format(&d, &sm),
            // (the BOM is not shown: it takes no column)
            "error[E0110]: unexpected `)`\n  --> bom.fung:1:5\n  |\n1 | x = )\n  |     ^\n  |\n"
        );
    }

    #[test]
    fn quote_uses_fung_literals() {
        assert_eq!(quote("a\tb\"c "), "\"a\\tb\\\"c \"");
        assert_eq!(quote("\u{0}"), "\"\\u{0}\"");
        assert_eq!(quote("\u{A0}"), "\"\\u{a0}\"");
        assert_eq!(quote("\u{7}"), "\"\\u{7}\"");
        assert_eq!(quote("${x}"), "\"\\${x}\"");
        assert_eq!(quote("한"), "\"한\"");
    }

    #[test]
    fn verbs() {
        assert_eq!(
            message("E0101", &[Arg::Str("@".into())]),
            "unexpected character \"@\""
        );
        assert!(
            is_compile_error("E0110") && is_compile_error("E0204") && is_compile_error("E0301")
        );
        assert!(!is_compile_error("E0502"));
    }

    #[test]
    fn error_uses_catalog_message() {
        let mut sm = SourceMap::new();
        let pos = at(&mut sm, "a.fung", "", 3, 7);
        let d = error("E0204", pos, vec![Arg::Str("fiels".into())]);
        assert_eq!(headline(&d), "error[E0204]: undefined name `fiels`");
    }

    #[test]
    fn format_shows_position_and_hint() {
        let mut sm = SourceMap::new();
        let pos = at(&mut sm, "a.fung", "", 1, 9);
        let d = error("E0111", pos, vec![]);
        assert_eq!(
            format(&d, &sm),
            "error[E0111]: comparison operators cannot be chained\n  --> a.fung:1:9\n help: split it with `and`: a < b and b < c\n"
        );
    }

    fn nowhere(sm: &mut SourceMap) -> Pos {
        let file = sm.add("", "");
        Pos {
            file,
            line: 0,
            col: 0,
        }
    }

    #[test]
    fn format_without_hint_or_position() {
        let mut sm = SourceMap::new();
        let d = error("E0502", nowhere(&mut sm), vec![]);
        assert_eq!(format(&d, &sm), "error[E0502]: division by zero\n");
    }

    #[test]
    fn unknown_code_still_formats() {
        let mut sm = SourceMap::new();
        let d = error("E9999", nowhere(&mut sm), vec![Arg::Str("x".into())]);
        assert_eq!(headline(&d), "error[E9999]: unknown error E9999 [x]");
    }

    #[test]
    fn is_compile_error_table() {
        for (code, want) in [
            ("E0101", true),
            ("E0115", true),
            ("E0230", true),
            ("E0204", true),
            ("E0301", true),
            ("E0401", false),
            ("E0502", false),
            ("E0801", false),
            ("E0806", false),
        ] {
            assert_eq!(is_compile_error(code), want, "{code}");
        }
    }

    #[test]
    fn pos_is_zero() {
        let mut sm = SourceMap::new();
        assert!(nowhere(&mut sm).is_zero());
        assert!(!at(&mut sm, "a.fung", "", 3, 7).is_zero());
    }

    #[test]
    fn format_with_source_excerpt() {
        let mut sm = SourceMap::new();
        let pos = at(&mut sm, "rename.fung", "x = 1\n  for f in fiels:\n", 2, 12);
        let d = error("E0204", pos, vec![Arg::Str("fiels".into())])
            .span(5)
            .with_help("a similar name exists: `files`");
        assert_eq!(
            format(&d, &sm),
            "error[E0204]: undefined name `fiels`\n  --> rename.fung:2:12\n  |\n2 |   for f in fiels:\n  |            ^^^^^ not found in this scope\n  |\n help: a similar name exists: `files`\n"
        );
    }

    #[test]
    fn excerpt_expands_tabs_and_widens_gutter() {
        let mut sm = SourceMap::new();
        let src = format!("{}\tprint(1 / 0)\n", "\n".repeat(9));
        let pos = at(&mut sm, "a.fung", &src, 10, 10);
        let d = error("E0502", pos, vec![]);
        assert_eq!(
            format(&d, &sm),
            "error[E0502]: division by zero\n  --> a.fung:10:10\n   |\n10 |     print(1 / 0)\n   |             ^\n   |\n"
        );
    }

    #[test]
    fn no_excerpt_without_source_or_for_out_of_range_line() {
        let mut sm = SourceMap::new();
        let d = error("E0502", at(&mut sm, "a.fung", "", 3, 1), vec![]);
        assert_eq!(
            format(&d, &sm),
            "error[E0502]: division by zero\n  --> a.fung:3:1\n"
        );
        let d = error("E0502", at(&mut sm, "a.fung", "one line\n", 3, 1), vec![]);
        assert_eq!(
            format(&d, &sm),
            "error[E0502]: division by zero\n  --> a.fung:3:1\n"
        );
    }

    #[test]
    fn help_overrides_catalog_hint() {
        let mut sm = SourceMap::new();
        let d = error("E0111", nowhere(&mut sm), vec![]).with_help("custom");
        assert_eq!(
            format(&d, &sm),
            "error[E0111]: comparison operators cannot be chained\n help: custom\n"
        );
    }

    #[test]
    fn retired_code_stays_out_of_the_catalog() {
        assert!(message("E0207", &[]).starts_with("unknown error"));
    }

    #[test]
    fn warnings_and_notes() {
        let mut sm = SourceMap::new();
        let pos = at(&mut sm, "t.fung", "", 1, 1);
        let w = error("W0101", pos, vec![Arg::Str("fs".into())]);
        assert_eq!(headline(&w), "warning[W0101]: unused import `fs`");
        let pos = at(&mut sm, "t.fung", "x = 1\nassert f(x) == 3\n", 2, 8);
        let mut e = error("E0807", pos, vec![Arg::Str("f(x) == 3".into())]);
        e.notes = vec!["f(x) = 4".to_string()];
        let out = format(&e, &sm);
        assert!(out.contains("error[E0807]: assertion failed: f(x) == 3"));
        assert!(out.contains("\n  f(x) = 4\n"));
    }

    #[test]
    fn codes_exist_in_the_catalog() {
        let groups: [&[&str]; 5] = [
            &["E0112", "E0205", "E0206", "E0803"],
            &[
                "E0208", "E0209", "E0210", "E0211", "E0212", "E0402", "E0403", "E0404", "E0405",
                "E0406", "E0407", "E0804", "E0805",
            ],
            &[
                "E0113", "E0221", "E0301", "E0302", "E0303", "E0304", "E0305", "E0306", "E0307",
                "E0308", "E0309", "E0310", "E0311", "E0513", "E0514",
            ],
            &["E0114", "E0807", "W0101", "W0102", "W0103", "W0104"],
            &["E0701", "E0702", "E0703"],
        ];
        let args = [
            Arg::Str("a".into()),
            Arg::Str("b".into()),
            Arg::Str("c".into()),
        ];
        for code in groups.into_iter().flatten() {
            assert!(
                !message(code, &args).starts_with("unknown error"),
                "{code} missing"
            );
        }
    }

    #[test]
    fn verbs_and_missing_args() {
        assert_eq!(
            message("E0804", &[Arg::Int(100000)]),
            "stack overflow: call depth exceeded 100000"
        );
        assert_eq!(
            message("E0804", &[Arg::Str("x".into())]),
            "stack overflow: call depth exceeded %!d(string=x)"
        );
        assert!(message("E0101", &[]).contains("%!q(MISSING)"));
    }

    #[test]
    fn exit_codes() {
        assert_eq!(
            (EXIT_OK, EXIT_RUNTIME, EXIT_COMPILE, EXIT_USAGE),
            (0, 1, 2, 3)
        );
    }

    // Call paths.

    fn site(file: &str, line: u32, func: &str, elided: u64) -> CallSite {
        CallSite {
            file: file.into(),
            line,
            func: func.into(),
            elided,
        }
    }

    #[test]
    fn format_call_path() {
        let mut sm = SourceMap::new();
        let pos = at(&mut sm, "r.fung", "", 7, 11);
        let d = error(
            "E0501",
            pos,
            vec![
                Arg::Str("add".into()),
                Arg::Str("int".into()),
                Arg::Str("str".into()),
            ],
        )
        .with_call_path(vec![
            site("r.fung", 7, "format_size", 0),
            site("r.fung", 15, "report", 3),
            site("r.fung", 22, "<main>", 0),
        ]);
        assert_eq!(
            format(&d, &sm),
            "error[E0501]: cannot add int and str\n  --> r.fung:7:11\n call path (innermost first):\n   r.fung:7   format_size\n   r.fung:15  report (3 tail calls elided)\n   r.fung:22  <entry>\n"
        );
    }

    /// FE61: one elided tail call is said in the singular.
    #[test]
    fn one_tail_call_is_singular() {
        let mut sm = SourceMap::new();
        let d = error("E0502", nowhere(&mut sm), vec![])
            .with_call_path(vec![site("r.fung", 1, "f", 1), site("r.fung", 2, "g", 2)]);
        let got = format(&d, &sm);
        assert!(
            got.contains("   r.fung:1   f (1 tail call elided)\n"),
            "{got}"
        );
        assert!(
            got.contains("   r.fung:2   g (2 tail calls elided)\n"),
            "{got}"
        );
    }

    #[test]
    fn format_call_path_is_truncated() {
        let mut sm = SourceMap::new();
        let pos = at(&mut sm, "a.fung", "", 1, 1);
        let path: Vec<CallSite> = (0..30).map(|i| site("a.fung", i + 1, "f", 0)).collect();
        let d = error("E0804", pos, vec![Arg::Int(100000)]).with_call_path(path);
        let got = format(&d, &sm);
        assert!(got.contains("   \u{2026} 20 more\n"), "{got}");
        assert_eq!(got.matches("\n   a.fung").count(), 10, "{got}");
    }

    #[test]
    fn with_call_path_keeps_existing() {
        let mut sm = SourceMap::new();
        let d = error("E0502", nowhere(&mut sm), vec![])
            .with_call_path(vec![site("", 0, "a", 0)])
            .with_call_path(vec![site("", 0, "b", 0)]);
        assert_eq!(d.call_path.len(), 1);
        assert_eq!(d.call_path[0].func, "a");
    }

    #[test]
    fn with_pos_fills_only_when_zero() {
        let mut sm = SourceMap::new();
        let first = at(&mut sm, "a.fung", "", 1, 2);
        let second = at(&mut sm, "a.fung", "", 3, 4);
        let d = error("E0502", nowhere(&mut sm), vec![])
            .with_pos(first)
            .with_pos(second);
        assert_eq!((d.pos.line, d.pos.col), (1, 2));
    }

    #[test]
    fn format_call_path_from_task_brief() {
        let mut sm = SourceMap::new();
        let src = "fn f(n) = n == 0 then 1 / 0 else f(n - 1)\nfn g(x) = f(x) + 1\nprint(g(3))";
        let file = sm.add("-e", src);
        let d = error(
            "E0502",
            Pos {
                file,
                line: 1,
                col: 25,
            },
            vec![],
        )
        .with_call_path(vec![
            site("-e", 1, "f", 3),
            site("-e", 2, "g", 0),
            site("-e", 3, "<main>", 0),
        ]);
        assert_eq!(
            format(&d, &sm),
            "error[E0502]: division by zero\n  --> -e:1:25\n  |\n1 | fn f(n) = n == 0 then 1 / 0 else f(n - 1)\n  |                         ^\n  |\n call path (innermost first):\n   -e:1       f (3 tail calls elided)\n   -e:2       g\n   -e:3       <entry>\n"
        );
    }
}
