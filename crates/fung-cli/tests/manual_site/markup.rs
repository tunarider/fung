//! The manual's manuscript format (`manual/**/*.md`): a small part of
//! Markdown with raw HTML let through. Code that runs is always a fence,
//! so the generator can check and run it. Uses `std` only, so
//! `tools_expected.rs` can include this file on its own.

#![allow(dead_code)]

/// One manuscript page.
#[derive(Debug, PartialEq)]
pub struct Page {
    /// The manuscript path, relative to `manual/` (`vm/internals.md`).
    pub path: String,
    pub title: String,
    pub lede: Option<String>,
    pub description: Option<String>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, PartialEq)]
pub enum Block {
    /// Rendered HTML (a paragraph, a list, raw HTML let through).
    Html(String),
    Heading {
        level: u8,
        id: Option<String>,
        html: String,
    },
    Code(Code),
    /// `{{parts}}`: the cover's cards of the parts.
    Parts,
}

/// A fenced code block.
#[derive(Debug, PartialEq)]
pub struct Code {
    /// The line of the opening fence.
    pub line: usize,
    pub kind: Kind,
    pub src: String,
    pub args: Vec<String>,
    /// A fixture file given as standard input.
    pub stdin: Option<String>,
    pub env: Vec<(String, String)>,
    pub exit: i32,
    pub hide_output: bool,
    /// Fixture files (name, contents) laid out before it runs.
    pub files: Vec<(String, String)>,
    /// The shown, unchecked output of a `check` example.
    pub expect: Option<String>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Kind {
    Run,
    Check,
    Frag,
    /// `fung dump <stage>` of the source.
    Dump(String),
    Shell,
    Plain,
    Core,
}

/// HTML-escapes text.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Parses one manuscript. Errors are `path:line: reason`.
pub fn parse(path: &str, text: &str) -> Result<Page, String> {
    let lines: Vec<&str> = text.lines().collect();
    let err = |n: usize, why: &str| format!("{path}:{n}: {why}");
    let mut i = 0;
    let (mut lede, mut description) = (None, None);
    if lines.first() == Some(&"---") {
        i = 1;
        loop {
            let Some(l) = lines.get(i) else {
                return Err(err(1, "front matter without a closing `---`"));
            };
            i += 1;
            if *l == "---" {
                break;
            }
            match l.split_once(": ") {
                Some(("lede", v)) => lede = Some(v.to_string()),
                Some(("description", v)) => description = Some(v.to_string()),
                _ => return Err(err(i, "front matter is `lede: …` or `description: …`")),
            }
        }
    }
    while lines.get(i).is_some_and(|l| l.trim().is_empty()) {
        i += 1;
    }
    let title = match lines.get(i).and_then(|l| l.strip_prefix("# ")) {
        Some(t) => t.trim().to_string(),
        None => return Err(err(i + 1, "a page starts with its title: `# …`")),
    };
    i += 1;

    let mut p = Parser {
        blocks: Vec::new(),
        para: Vec::new(),
        list: None,
        files: Vec::new(),
        ids: Vec::new(),
    };
    while i < lines.len() {
        let n = i + 1;
        let line = lines[i];
        let t = line.trim();
        if t.is_empty() {
            p.flush();
            i += 1;
        } else if let Some(info) = t.strip_prefix("```") {
            p.flush();
            let start = i;
            i += 1;
            let mut body = Vec::new();
            loop {
                let Some(l) = lines.get(i) else {
                    return Err(err(start + 1, "a fence that is never closed"));
                };
                i += 1;
                if l.trim() == "```" {
                    break;
                }
                body.push(*l);
            }
            p.fence(info.trim(), body, start + 1)
                .map_err(|(n, why)| err(n, &why))?;
        } else if p.para.is_empty() && p.list.is_none() && t == "{{parts}}" {
            p.blocks.push(Block::Parts);
            i += 1;
        } else if p.para.is_empty() && p.list.is_none() && t.starts_with('#') {
            let level = t.chars().take_while(|&c| c == '#').count();
            let Some(rest) = t[level..].strip_prefix(' ') else {
                return Err(err(n, "a heading is `## text {#id}`"));
            };
            if !(2..=4).contains(&level) {
                return Err(err(
                    n,
                    "headings are `##` to `####` (the title is the only `#`)",
                ));
            }
            let (text, id) = match rest
                .trim()
                .strip_suffix('}')
                .and_then(|r| r.rsplit_once(" {#"))
            {
                Some((text, id)) => (text.trim(), Some(id.to_string())),
                None => (rest.trim(), None),
            };
            if id.is_none() && level < 4 {
                return Err(err(
                    n,
                    "a `##` or `###` heading needs an id: `## text {#id}`",
                ));
            }
            if let Some(id) = &id {
                if p.ids.contains(id) {
                    return Err(err(n, &format!("the id `{id}` is used twice")));
                }
                p.ids.push(id.clone());
            }
            p.blocks.push(Block::Heading {
                level: level as u8,
                id,
                html: inline(text),
            });
            i += 1;
        } else if p.para.is_empty() && p.list.is_none() && t.starts_with('<') {
            let mut raw = Vec::new();
            while let Some(l) = lines.get(i).filter(|l| !l.trim().is_empty()) {
                if let Some(why) = buried(l, true) {
                    return Err(err(i + 1, why));
                }
                raw.push(*l);
                i += 1;
            }
            p.blocks.push(Block::Html(raw.join("\n")));
        } else if let Some((ordered, item)) = list_item(t) {
            p.flush_para();
            match &mut p.list {
                Some((o, items)) if *o == ordered => items.push(item.to_string()),
                _ => {
                    p.flush();
                    p.list = Some((ordered, vec![item.to_string()]));
                }
            }
            i += 1;
        } else if let Some((_, items)) = &mut p.list {
            // a list item's next line
            let last = items.last_mut().unwrap();
            last.push('\n');
            if let Some(why) = buried(t, false) {
                return Err(err(n, why));
            }
            last.push_str(t);
            i += 1;
        } else {
            if let Some(why) = buried(t, false) {
                return Err(err(n, why));
            }
            p.para.push(line.trim_end());
            i += 1;
        }
    }
    p.flush();
    if let Some((_, _, n)) = p.files.first() {
        return Err(err(
            *n,
            "a `file` fence with no `fung run` or `fung check` after it",
        ));
    }
    Ok(Page {
        path: path.to_string(),
        title,
        lede,
        description,
        blocks: p.blocks,
    })
}

/// Markup that a raw HTML block, a paragraph or a list item would
/// swallow as text: a fence or heading glued to it (no blank line), or a
/// `<pre>`/`<h1>`/`<h2>` the generator must see.
fn buried(line: &str, raw: bool) -> Option<&'static str> {
    let t = line.trim_start();
    let low = t.to_ascii_lowercase();
    if ["<pre", "<h1", "<h2"].iter().any(|tag| low.contains(tag)) {
        return Some("`<pre>`, `<h1>` and `<h2>` are not raw HTML here: write a fence or `##`");
    }
    let hashes = t.chars().take_while(|&c| c == '#').count();
    if hashes > 0 && t[hashes..].starts_with(' ') {
        return Some("a heading needs a blank line before it");
    }
    if raw && t.starts_with("```") {
        return Some("a fence needs a blank line between it and raw HTML");
    }
    None
}

struct Parser<'a> {
    blocks: Vec<Block>,
    para: Vec<&'a str>,
    /// (ordered, items)
    list: Option<(bool, Vec<String>)>,
    /// fixture files waiting for their example: (name, contents, line)
    files: Vec<(String, String, usize)>,
    ids: Vec<String>,
}

impl Parser<'_> {
    fn flush_para(&mut self) {
        if !self.para.is_empty() {
            let text = self.para.join("\n");
            self.blocks
                .push(Block::Html(format!("<p>{}</p>", inline(&text))));
            self.para.clear();
        }
    }

    fn flush(&mut self) {
        self.flush_para();
        if let Some((ordered, items)) = self.list.take() {
            let tag = if ordered { "ol" } else { "ul" };
            let lis: Vec<String> = items
                .iter()
                .map(|it| format!("<li>{}</li>", inline(it)))
                .collect();
            self.blocks.push(Block::Html(format!(
                "<{tag}>\n{}\n</{tag}>",
                lis.join("\n")
            )));
        }
    }

    /// Errors are (line, reason).
    fn fence(&mut self, info: &str, body: Vec<&str>, line: usize) -> Result<(), (usize, String)> {
        let at = |why: String| (line, why);
        let words = split_words(info).map_err(at)?;
        let w: Vec<&str> = words.iter().map(String::as_str).collect();
        let src = body.join("\n");
        let (kind, attrs) = match w.as_slice() {
            ["fung", "run", rest @ ..] => (Kind::Run, rest),
            ["fung", "check"] => (Kind::Check, &[][..]),
            ["fung", "frag"] => (Kind::Frag, &[][..]),
            ["dump", stage] if ["ast", "core", "bytecode"].contains(stage) => {
                (Kind::Dump(stage.to_string()), &[][..])
            }
            ["shell"] => (Kind::Shell, &[][..]),
            ["plain"] => (Kind::Plain, &[][..]),
            ["core"] => (Kind::Core, &[][..]),
            ["file", name] => {
                let mut contents = src;
                contents.push('\n');
                self.files.push((name.to_string(), contents, line));
                return Ok(());
            }
            ["expect"] => {
                return match self.blocks.last_mut() {
                    Some(Block::Code(c)) if c.kind == Kind::Check && c.expect.is_none() => {
                        c.expect = Some(src);
                        Ok(())
                    }
                    _ => Err(at(
                        "an `expect` fence must follow a `fung check` fence".into()
                    )),
                };
            }
            _ => {
                return Err(at(format!(
                    "unknown fence `{info}`: fung run|check|frag, dump ast|core|bytecode, shell, plain, core, file NAME, expect"
                )));
            }
        };
        let mut code = Code {
            line,
            kind,
            src,
            args: Vec::new(),
            stdin: None,
            env: Vec::new(),
            exit: 0,
            hide_output: false,
            files: Vec::new(),
            expect: None,
        };
        for a in attrs {
            match a.split_once('=') {
                Some(("args", v)) => code.args = v.split_whitespace().map(String::from).collect(),
                Some(("stdin", v)) => code.stdin = Some(v.to_string()),
                Some(("env", v)) => {
                    for kv in v.split_whitespace() {
                        let (k, v) = kv
                            .split_once('=')
                            .ok_or_else(|| at(format!("`env` is `K=V K2=V2`, not `{kv}`")))?;
                        code.env.push((k.to_string(), v.to_string()));
                    }
                }
                Some(("exit", v)) => {
                    code.exit = v
                        .parse()
                        .map_err(|_| at(format!("`exit` is a number, not `{v}`")))?
                }
                None if *a == "hide-output" => code.hide_output = true,
                _ => return Err(at(format!("unknown attribute `{a}`"))),
            }
        }
        if matches!(code.kind, Kind::Run | Kind::Check) {
            code.files = self.files.drain(..).map(|(n, c, _)| (n, c)).collect();
        } else if let Some((_, _, n)) = self.files.first() {
            return Err((
                *n,
                "a `file` fence must be followed by `fung run` or `fung check`".into(),
            ));
        }
        self.blocks.push(Block::Code(code));
        Ok(())
    }
}

/// Splits a fence's info string at blanks, keeping `key="a b"` whole.
fn split_words(s: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if quoted {
        return Err("an unclosed `\"` in the fence".into());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    Ok(out)
}

/// `- item` or `1. item`: (ordered, item).
fn list_item(t: &str) -> Option<(bool, &str)> {
    if let Some(rest) = t.strip_prefix("- ") {
        return Some((false, rest));
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        return t[digits..].strip_prefix(". ").map(|rest| (true, rest));
    }
    None
}

/// The page's examples that the generator checks: `run`, `check` and
/// `dump`, labelled `path:n` from 1 in page order.
pub fn examples(page: &Page) -> Vec<(String, &Code)> {
    page.blocks
        .iter()
        .filter_map(|b| match b {
            Block::Code(c) if matches!(c.kind, Kind::Run | Kind::Check | Kind::Dump(_)) => Some(c),
            _ => None,
        })
        .enumerate()
        .map(|(i, c)| (format!("{}:{}", page.path, i + 1), c))
        .collect()
}

/// Renders inline markup: code spans (escaped), `**strong**`,
/// `[text](href)` (the text is inline markup too); anything else, inline
/// HTML included, passes through.
pub fn inline(text: &str) -> String {
    let mut out = String::new();
    let mut prose = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let code = (c == '`').then(|| rest[1..].find('`')).flatten();
        if let Some(j) = code {
            out.push_str(&strong(&std::mem::take(&mut prose)));
            out.push_str("<code>");
            out.push_str(&escape(&rest[1..1 + j]));
            out.push_str("</code>");
            rest = &rest[1 + j + 1..];
            continue;
        }
        let linked = (c == '[').then(|| link(rest)).flatten();
        if let Some((label, href, after)) = linked {
            out.push_str(&strong(&std::mem::take(&mut prose)));
            out.push_str(&format!("<a href=\"{href}\">{}</a>", inline(label)));
            rest = after;
            continue;
        }
        prose.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out.push_str(&strong(&prose));
    out
}

/// `[label](href)` at the start of `s`: (label, href, the rest). The
/// label may hold code spans (with brackets inside them).
fn link(s: &str) -> Option<(&str, &str, &str)> {
    let mut in_code = false;
    for (i, c) in s.char_indices().skip(1) {
        match c {
            '`' => in_code = !in_code,
            '[' if !in_code => return None,
            ']' if !in_code => {
                let after = s[i + 1..].strip_prefix('(')?;
                let end = after.find(')')?;
                let href = &after[..end];
                if href.is_empty() || href.contains(char::is_whitespace) {
                    return None;
                }
                return Some((&s[1..i], href, &after[end + 1..]));
            }
            _ => {}
        }
    }
    None
}

/// `**strong**`, in pairs.
fn strong(s: &str) -> String {
    let parts: Vec<&str> = s.split("**").collect();
    if parts.len() < 3 {
        return s.to_string();
    }
    let pairs = (parts.len() - 1) / 2;
    let mut out = String::new();
    for (k, part) in parts.iter().enumerate() {
        if k > 0 {
            if k <= pairs * 2 {
                out.push_str(if k % 2 == 1 { "<strong>" } else { "</strong>" });
            } else {
                out.push_str("**");
            }
        }
        out.push_str(part);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(p: &Page) -> Vec<&Code> {
        p.blocks
            .iter()
            .filter_map(|b| match b {
                Block::Code(c) => Some(c),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn headings_need_ids() {
        let p = parse("p.md", "# T\n\n## 가 {#a}\n").unwrap();
        assert_eq!(p.title, "T");
        assert_eq!(
            p.blocks,
            vec![Block::Heading {
                level: 2,
                id: Some("a".into()),
                html: "가".into()
            }]
        );
        let e = parse("p.md", "# T\n\n## 가\n").unwrap_err();
        assert!(e.starts_with("p.md:3:") && e.contains("{#id}"), "{e}");
        // a fourth level may go without one
        let p = parse("p.md", "# T\n\n#### 작은\n").unwrap();
        assert_eq!(
            p.blocks,
            vec![Block::Heading {
                level: 4,
                id: None,
                html: "작은".into()
            }]
        );
    }

    #[test]
    fn front_matter_and_title() {
        let p = parse(
            "p.md",
            "---\nlede: 한 줄 소개\ndescription: 설명\n---\n# 제목\n\n본문\n",
        )
        .unwrap();
        assert_eq!(p.title, "제목");
        assert_eq!(p.lede.as_deref(), Some("한 줄 소개"));
        assert_eq!(p.description.as_deref(), Some("설명"));
        assert_eq!(p.blocks, vec![Block::Html("<p>본문</p>".into())]);
        assert!(parse("p.md", "본문\n").unwrap_err().contains("# "));
    }

    #[test]
    fn inline_markup() {
        assert_eq!(
            inline("`a<b` **c** [d](e.html#f) <kbd>x</kbd>"),
            "<code>a&lt;b</code> <strong>c</strong> <a href=\"e.html#f\">d</a> <kbd>x</kbd>"
        );
        // markup inside a code span is not markup
        assert_eq!(inline("`**x**`"), "<code>**x**</code>");
    }

    #[test]
    fn paragraphs_and_lists() {
        let p = parse(
            "p.md",
            "# T\n\n한 줄\n두 줄\n\n- 가\n- `나`\n\n1. 하나\n2. 둘\n",
        )
        .unwrap();
        assert_eq!(
            p.blocks,
            vec![
                Block::Html("<p>한 줄\n두 줄</p>".into()),
                Block::Html("<ul>\n<li>가</li>\n<li><code>나</code></li>\n</ul>".into()),
                Block::Html("<ol>\n<li>하나</li>\n<li>둘</li>\n</ol>".into()),
            ]
        );
    }

    #[test]
    fn raw_html_passes_through_and_resumes() {
        let p = parse(
            "p.md",
            "# T\n\n<div class=\"fn\" id=\"fn-x\"><code>x()</code>\n\n```fung run\nprint(1)\n```\n\n</div>\n",
        )
        .unwrap();
        assert_eq!(p.blocks.len(), 3);
        assert_eq!(
            p.blocks[0],
            Block::Html("<div class=\"fn\" id=\"fn-x\"><code>x()</code>".into())
        );
        let c = codes(&p);
        assert_eq!(c[0].kind, Kind::Run);
        assert_eq!(c[0].src, "print(1)");
        assert_eq!(c[0].line, 5);
        assert_eq!(p.blocks[2], Block::Html("</div>".into()));
    }

    #[test]
    fn fence_attributes() {
        let p = parse(
            "p.md",
            "# T\n\n```file in.txt\nx\ny\n```\n\n```file b.txt\nb\n```\n\n\
             ```fung run args=\"a b\" stdin=in.txt env=\"K=V K2=V2\" exit=1 hide-output\nprint(args)\n```\n\n\
             ```fung check\nprint(1)\n```\n\n```expect\n1\n```\n\n\
             ```dump bytecode\nfn f() = 1\n```\n\n```shell\nls\n```\n\n```plain\nx\n```\n\n```core\n(do)\n```\n\n```fung frag\na + b\n```\n\n{{parts}}\n",
        )
        .unwrap();
        let c = codes(&p);
        assert_eq!(c.len(), 7);
        let run = c[0];
        assert_eq!(run.kind, Kind::Run);
        assert_eq!(run.args, vec!["a", "b"]);
        assert_eq!(run.stdin.as_deref(), Some("in.txt"));
        assert_eq!(
            run.env,
            vec![("K".into(), "V".into()), ("K2".into(), "V2".into())]
        );
        assert_eq!(run.exit, 1);
        assert!(run.hide_output);
        assert_eq!(
            run.files,
            vec![
                ("in.txt".into(), "x\ny\n".into()),
                ("b.txt".into(), "b\n".into())
            ]
        );
        assert_eq!(c[1].kind, Kind::Check);
        assert_eq!(c[1].expect.as_deref(), Some("1"));
        assert!(c[1].files.is_empty());
        assert_eq!(c[2].kind, Kind::Dump("bytecode".into()));
        assert_eq!(c[3].kind, Kind::Shell);
        assert_eq!(c[4].kind, Kind::Plain);
        assert_eq!(c[5].kind, Kind::Core);
        assert_eq!(c[6].kind, Kind::Frag);
        assert_eq!(p.blocks.last(), Some(&Block::Parts));
    }

    #[test]
    fn unknown_markup_is_an_error() {
        for (src, line) in [
            ("# T\n\n```fung runn\nx\n```\n", ":3:"),
            ("# T\n\n<div>\n<pre>x</pre>\n</div>\n", ":4:"),
            ("# T\n\n<div><h2 id=\"a\">x</h2></div>\n", ":3:"),
            ("# T\n\n## a {#x}\n\n## b {#x}\n", ":5:"),
            ("# T\n\n```file a.txt\nx\n```\n\n```shell\nls\n```\n", ":3:"),
            ("# T\n\n```file a.txt\nx\n```\n", ":3:"),
            ("# T\n\n```fung run\nx\n", ":3:"),
            ("# T\n\n```expect\nx\n```\n", ":3:"),
            ("# T\n\n```fung run exit=x\nx\n```\n", ":3:"),
            ("# T\n\n```fung run what=1\nx\n```\n", ":3:"),
            ("# T\n\n```dump nope\nx\n```\n", ":3:"),
        ] {
            let e = parse("p.md", src).expect_err(src);
            assert!(e.starts_with("p.md") && e.contains(line), "{src:?}: {e}");
        }
    }

    #[test]
    fn markup_inside_raw_html_or_paragraphs_is_an_error() {
        for (src, line) in [
            // a fence or a heading glued to raw HTML (no blank line)
            ("# T\n\n<div>\n```fung run\nx\n```\n</div>\n", ":4:"),
            ("# T\n\n<div>\n## H {#h}\n</div>\n", ":4:"),
            // ... or to a paragraph
            ("# T\n\ntext\n## H {#h}\n", ":4:"),
            ("# T\n\ntext\n<pre>x</pre>\n", ":4:"),
        ] {
            let e = parse("p.md", src).expect_err(src);
            assert!(e.contains(line), "{src:?}: {e}");
        }
    }

    #[test]
    fn link_text_may_be_code() {
        assert_eq!(
            inline("[`map`](#fn-map)"),
            "<a href=\"#fn-map\"><code>map</code></a>"
        );
        assert_eq!(inline("`[a](b)`"), "<code>[a](b)</code>");
    }

    #[test]
    fn example_labels() {
        let p = parse(
            "p.md",
            "# T\n\n```fung run\na\n```\n\n```fung frag\nb\n```\n\n```fung check\nc\n```\n\n```dump ast\nd\n```\n\n```shell\ne\n```\n",
        )
        .unwrap();
        let ex: Vec<(String, String)> = examples(&p)
            .into_iter()
            .map(|(l, c)| (l, c.src.clone()))
            .collect();
        assert_eq!(
            ex,
            vec![
                ("p.md:1".into(), "a".into()),
                ("p.md:2".into(), "c".into()),
                ("p.md:3".into(), "d".into()),
            ]
        );
    }

    #[test]
    fn escape_escapes() {
        assert_eq!(
            escape("<a href=\"x\">&</a>"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;"
        );
    }
}
