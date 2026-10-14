//! Diagnostics: catalog-numbered errors and the way fung prints them.

mod catalog;

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
}

pub fn error(code: &str, pos: Pos, args: Vec<Arg>) -> Diag {
    Diag {
        code: code.to_string(),
        pos,
        len: 0,
        args,
        help: None,
        notes: Vec::new(),
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
}

/// Renders the catalog template for a code with Go `fmt` verb semantics.
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
            ('s' | 'v', a) => out.push_str(&a.plain()),
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
        let src = sources.text(d.pos.file);
        if !src.is_empty() {
            write_excerpt(&mut b, d, src);
        }
    }
    for note in &d.notes {
        b.push_str(&format!("  {note}\n"));
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

const TAB_WIDTH: usize = 4;

/// Appends the gutter, the source line, and the caret line. Tabs are
/// expanded so the caret lands under the right character.
fn write_excerpt(b: &mut String, d: &Diag, src: &str) {
    let line_no = d.pos.line as usize;
    let Some(raw) = src.split('\n').nth(line_no - 1) else {
        return;
    };
    let line = raw.strip_suffix('\r').unwrap_or(raw);
    let num = line_no.to_string();
    let gutter = " ".repeat(num.len());

    let mut shown = String::new();
    let mut shown_chars = 0;
    let mut col: Option<usize> = None;
    for (i, r) in line.chars().enumerate() {
        if i + 1 == d.pos.col as usize {
            col = Some(shown_chars);
        }
        if r == '\t' {
            shown.push_str(&" ".repeat(TAB_WIDTH));
            shown_chars += TAB_WIDTH;
        } else {
            shown.push(r);
            shown_chars += 1;
        }
    }
    let col = col.unwrap_or(shown_chars);
    let n = d.len.max(1);
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

/// Whether a code belongs to the compile-time categories (syntax E01xx,
/// names E02xx, modules E03xx).
pub fn is_compile_error(code: &str) -> bool {
    code.starts_with("E01") || code.starts_with("E02") || code.starts_with("E03")
}

/// Go's `strconv.Quote`.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{7}' => out.push_str("\\a"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{b}' => out.push_str("\\v"),
            c if is_print(c) => out.push(c),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c if (c as u32) < 0x10000 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push_str(&format!("\\U{:08x}", c as u32)),
        }
    }
    out.push('"');
    out
}

/// Approximates Go's `unicode.IsPrint`: everything except control,
/// format, surrogate, private-use and noncharacter code points, and
/// spaces other than U+0020. Unassigned code points are not detected
/// (the standard library carries no Unicode category tables).
pub(crate) fn is_print(c: char) -> bool {
    if c == ' ' {
        return true;
    }
    if c.is_control() || c.is_whitespace() {
        return false;
    }
    let u = c as u32;
    !matches!(u,
        0x00ad
        | 0x0600..=0x0605
        | 0x061c
        | 0x06dd
        | 0x070f
        | 0x0890..=0x0891
        | 0x08e2
        | 0x180e
        | 0x200b..=0x200f
        | 0x202a..=0x202e
        | 0x2060..=0x206f
        | 0xe000..=0xf8ff
        | 0xfdd0..=0xfdef
        | 0xfeff
        | 0xfff9..=0xfffb
        | 0x110bd
        | 0x110cd
        | 0x13430..=0x1343f
        | 0x1bca0..=0x1bca3
        | 0x1d173..=0x1d17a
        | 0xe0001
        | 0xe0020..=0xe007f
        | 0xf0000..=0x10ffff)
        && (u & 0xfffe) != 0xfffe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_print_cases() {
        for u in [0xA0u32, 0x2003, 0x200B, 0x8E2, 0xFDD0, 0xE0001] {
            assert!(!is_print(char::from_u32(u).unwrap()), "U+{u:X}");
        }
        // Every format (Cf) character of Go's tables, by range ends.
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
        let d = error("E0103", pos, vec![Arg::Str("q".into())]);
        assert_eq!(
            format(&d, &sm),
            "error[E0103]: unknown escape \\q in string\n  --> -e:1:7\n  |\n1 |     x = \"\\q\"\n  |          ^\n  |\n"
        );
    }

    #[test]
    fn columns_and_underline_count_chars() {
        let mut sm = SourceMap::new();
        let pos = at(&mut sm, "k.fung", "s = \"한글\"\nprint(없는)\n", 2, 7);
        let d = error("E0204", pos, vec![Arg::Str("없는".into())]).span(2);
        assert_eq!(
            format(&d, &sm),
            "error[E0204]: undefined name `없는`\n  --> k.fung:2:7\n  |\n2 | print(없는)\n  |       ^^ not found in this scope\n  |\n"
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
            "error[E0110]: unexpected `)`\n  --> bom.fung:1:5\n  |\n1 | \u{feff}x = )\n  |     ^\n  |\n"
        );
    }

    #[test]
    fn quote_matches_go() {
        assert_eq!(quote("a\tb\"c "), "\"a\\tb\\\"c \"");
        assert_eq!(quote("\u{0}"), "\"\\x00\"");
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

    // Ported from Go internal/diag/error_test.go.

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
            ("E0204", true),
            ("E0301", true),
            ("E0401", false),
            ("E0502", false),
            ("E0801", false),
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
            &["E0114", "E0807", "W0101", "W0102", "W0103"],
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
}
