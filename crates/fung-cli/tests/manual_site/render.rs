//! One page of the manual as HTML: the frame (banner, top bar, sidebar
//! with the version picker, breadcrumbs, pager) around the manuscript.

use std::collections::{BTreeMap, HashMap};

use super::examples::Outcome;
use super::markup::{Block, Code, Kind, Page, escape, examples, inline};
use super::toc::{Entry, Part, Toc};

/// A version of the manual: `latest` (tagged `unreleased`) or a frozen
/// release.
#[derive(Debug, Clone, PartialEq)]
pub struct Version {
    /// The directory under `docs/`.
    pub dir: String,
    pub name: String,
    pub tag: Option<String>,
}

/// What every page of one build shares.
pub struct Ctx<'a> {
    pub toc: &'a Toc,
    /// Every manuscript by its path, `index.md` included.
    pub pages: &'a BTreeMap<String, Page>,
    /// Example outcomes by label (`path:n`).
    pub results: &'a HashMap<String, Outcome>,
    pub version: &'a Version,
    /// The wordmark's inline SVG.
    pub wordmark: &'a str,
}

/// The page's HTML file path: `vm/internals.md` → `vm/internals.html`.
pub fn out_path(manuscript: &str) -> String {
    format!(
        "{}.html",
        manuscript.strip_suffix(".md").unwrap_or(manuscript)
    )
}

const CHEV: &str = r#"<svg class="chev" viewBox="0 0 12 12" aria-hidden="true"><path d="M3 4.5l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const SEP: &str = r#"<svg class="sep" viewBox="0 0 8 12" aria-hidden="true"><path d="M2.5 2l3.5 4-3.5 4" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const MENU: &str = r#"<svg class="menu" viewBox="0 0 16 16" aria-hidden="true"><path d="M2.5 4h11M2.5 8h11M2.5 12h7" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>"#;
const CLOSE: &str = r#"<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>"#;
const SITE: &str = "https://blog.tunarider.net/fung";
/// The whole HTML document of the manuscript at `path`.
pub fn page_html(ctx: &Ctx, path: &str) -> String {
    let page = &ctx.pages[path];
    let up = "../".repeat(path.matches('/').count());
    let cover = path == "index.md";
    let place = ctx.toc.locate(path);
    let label = |part: &Part, n: usize, title: &str| {
        if part.numbered {
            format!("{n}. {title}")
        } else {
            title.to_string()
        }
    };

    let mut h = String::new();
    // head
    let title = if cover {
        "fung 매뉴얼".to_string()
    } else {
        format!("{} · fung 매뉴얼", page.title)
    };
    h += "<!doctype html>\n<html lang=\"ko\">\n<head>\n<meta charset=\"utf-8\">\n";
    h += "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n";
    h += "<script>document.documentElement.classList.add(\"js\")</script>\n";
    h += &format!("<title>{}</title>\n", escape(&title));
    if let Some(d) = &page.description {
        h += &format!("<meta name=\"description\" content=\"{}\">\n", escape(d));
    }
    h += "<link rel=\"preconnect\" href=\"https://fonts.googleapis.com\">\n";
    h += "<link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin>\n";
    h += "<link href=\"https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500;600&family=IBM+Plex+Sans+KR:wght@400;500;600&display=swap\" rel=\"stylesheet\">\n";
    h += &format!("<link rel=\"stylesheet\" href=\"{up}assets/style.css\">\n");
    h += &format!(
        "<link rel=\"icon\" href=\"{up}../assets/logo/icon-light.svg\" type=\"image/svg+xml\">\n"
    );
    h += &format!(
        "<link rel=\"icon\" href=\"{up}../assets/logo/favicon-32.png\" sizes=\"32x32\" type=\"image/png\">\n"
    );
    h += &format!(
        "<link rel=\"apple-touch-icon\" href=\"{up}../assets/logo/apple-touch-icon.png\">\n"
    );
    if cover {
        h += &og(
            &format!("{SITE}/{}/", ctx.version.dir),
            page.description.as_deref(),
        );
    }
    h += "</head>\n";
    let body_class = place.map_or("front", |(p, _, _)| p.key.as_str());
    h += &format!("<body class=\"{body_class}\">\n");

    // banner
    if ctx.version.tag.is_some() {
        h += &format!(
            "<div class=\"banner\" role=\"note\"><span class=\"banner-mark\" aria-hidden=\"true\"></span><p><b>{}</b>는 아직 릴리스되지 않은 main의 문서입니다. 설치한 fung보다 앞선 내용이 있을 수 있습니다.</p></div>\n",
            escape(&ctx.version.name)
        );
    }

    // top bar (small screens)
    let (mark, here) = match place {
        Some((part, n, _)) => (
            format!("<span class=\"glyph\">{}</span>", escape(&part.glyph)),
            label(part, n, &page.title),
        ),
        None => (String::new(), "fung 매뉴얼".to_string()),
    };
    h += "<header class=\"mbar\">\n<button class=\"mbar-open\" type=\"button\" aria-controls=\"side\" aria-expanded=\"false\">";
    h += &format!(
        "{mark}<span class=\"mbar-where\"><span class=\"mbar-title\">{}</span><span class=\"mbar-sec\" aria-hidden=\"true\"></span></span>{MENU}<span class=\"sr\">목차 열기</span></button>\n</header>\n",
        escape(&here)
    );
    h += "<div class=\"scrim\" hidden></div>\n<div class=\"frame\">\n";

    // sidebar
    h += &format!(
        "<aside class=\"side\" id=\"side\" aria-label=\"목차\">\n<button class=\"side-close\" type=\"button\" aria-label=\"목차 닫기\">{CLOSE}</button>\n"
    );
    h += &format!(
        "<a class=\"brand\" href=\"{up}index.html\">{}</a>\n",
        ctx.wordmark
    );
    let v = ctx.version;
    let tag = match &v.tag {
        Some(t) => format!(
            "<span class=\"ver-tag\" id=\"ver-tag\">{}</span>",
            escape(t)
        ),
        None => "<span class=\"ver-tag\" id=\"ver-tag\" hidden></span>".to_string(),
    };
    let option = match &v.tag {
        Some(t) => format!("{} ({})", v.name, t),
        None => v.name.clone(),
    };
    h += &format!(
        "<div class=\"ver\"><span class=\"ver-label\" id=\"ver-label\">버전</span><div class=\"ver-box\"><span class=\"ver-name\" id=\"ver-name\">{}</span>{tag}{CHEV}<select id=\"ver\" aria-labelledby=\"ver-label\" data-current=\"{}\"><option value=\"{}\">{}</option></select></div></div>\n",
        escape(&v.name),
        escape(&v.dir),
        escape(&v.dir),
        escape(&option)
    );
    h += "<nav aria-label=\"매뉴얼\"><ol class=\"parts-nav\">\n";
    for part in &ctx.toc.parts {
        let first = part.pages().next().unwrap();
        h += &format!(
            "<li class=\"{}\"><a href=\"{up}{}\"><span class=\"glyph\">{}</span><span>{}<small>{}</small></span></a>",
            part.key,
            out_path(first),
            escape(&part.glyph),
            escape(&part.title),
            escape(&part.blurb)
        );
        if place.is_some_and(|(p, _, _)| p.key == part.key) {
            h += "\n<ol class=\"chapters\">\n";
            let mut n = 0;
            for e in &part.entries {
                match e {
                    Entry::Group(g) => h += &format!("<li class=\"sub\">{}</li>\n", escape(g)),
                    Entry::Page(p) => {
                        n += 1;
                        let title = &ctx.pages[p.as_str()].title;
                        let cur = if p == path {
                            " aria-current=\"page\""
                        } else {
                            ""
                        };
                        h += &format!(
                            "<li><a href=\"{up}{}\"{cur}>{}</a>",
                            out_path(p),
                            escape(&label(part, n, title))
                        );
                        if p == path {
                            let sections: Vec<String> = sections(page)
                                .map(|(id, html)| format!("<li><a href=\"#{id}\">{html}</a></li>"))
                                .collect();
                            if !sections.is_empty() {
                                h += &format!("<ol class=\"toc2\">{}</ol>", sections.concat());
                            }
                        }
                        h += "</li>\n";
                    }
                }
            }
            h += "</ol>\n";
        }
        h += "</li>\n";
    }
    h += "</ol></nav>\n</aside>\n";

    // main
    h += "<main><div class=\"col\">\n";
    if let Some((part, n, group)) = place {
        h += &format!(
            "<nav class=\"crumbs\" aria-label=\"위치\"><a class=\"crumb-part\" href=\"{up}{}\"><span class=\"glyph\">{}</span>{}</a>",
            out_path(part.pages().next().unwrap()),
            escape(&part.glyph),
            escape(&part.title)
        );
        if let Some(g) = group {
            h += &format!("{SEP}<span class=\"crumb-group\">{}</span>", escape(g));
        }
        if part.numbered {
            h += &format!("{SEP}<span>{n}장</span>");
        }
        h += "</nav>\n";
        h += &format!("<h1>{}</h1>\n", inline(&page.title));
    } else {
        h += &format!("<header class=\"hero\">\n<h1>{}</h1>\n", ctx.wordmark);
    }
    if let Some(lede) = &page.lede {
        h += &format!("<p class=\"lede\">{}</p>\n", inline(lede));
    }
    if place.is_none() {
        h += "</header>\n";
    }
    let labels: HashMap<*const Code, String> = examples(page)
        .into_iter()
        .map(|(l, c)| (c as *const Code, l))
        .collect();
    for b in &page.blocks {
        match b {
            Block::Html(x) => h += x,
            Block::Heading { level, id, html } => {
                let id = id
                    .as_ref()
                    .map_or(String::new(), |i| format!(" id=\"{i}\""));
                h += &format!("<h{level}{id}>{html}</h{level}>");
            }
            Block::Parts => h += &cards(ctx, &up),
            Block::Code(c) => {
                let outcome = labels
                    .get(&(c as *const Code))
                    .and_then(|l| ctx.results.get(l));
                h += &code_html(c, outcome);
            }
        }
        h += "\n";
    }
    if let Some((part, n, _)) = place {
        h += &pager(ctx, part, n, &up);
    }
    h += "</div></main>\n</div>\n";
    h += &format!(
        "<script src=\"{up}../versions.js\"></script>\n<script src=\"{up}assets/manual.js\"></script>\n"
    );
    h += "</body>\n</html>\n";
    h
}

/// The page's `##` sections: (id, html).
pub fn sections(page: &Page) -> impl Iterator<Item = (&str, &str)> {
    page.blocks.iter().filter_map(|b| match b {
        Block::Heading {
            level: 2,
            id: Some(id),
            html,
        } => Some((id.as_str(), html.as_str())),
        _ => None,
    })
}

/// The cover's link-preview tags (the cover's `description:` is the
/// preview's text).
pub fn og(url: &str, description: Option<&str>) -> String {
    let description = description.map_or(String::new(), |d| {
        format!(
            "<meta property=\"og:description\" content=\"{}\">\n",
            escape(d)
        )
    });
    format!(
        "<meta property=\"og:type\" content=\"website\">\n<meta property=\"og:site_name\" content=\"fung\">\n\
         <meta property=\"og:title\" content=\"fung 매뉴얼\">\n{description}\
         <meta property=\"og:url\" content=\"{url}\">\n<meta property=\"og:image\" content=\"{SITE}/assets/logo/social-preview-light.png\">\n\
         <meta property=\"og:image:width\" content=\"1280\">\n<meta property=\"og:image:height\" content=\"640\">\n\
         <meta name=\"twitter:card\" content=\"summary_large_image\">\n"
    )
}

/// The cover's cards, one per part.
fn cards(ctx: &Ctx, up: &str) -> String {
    let mut h = String::from("<div class=\"cover\">\n");
    for part in &ctx.toc.parts {
        let who = part.who.as_ref().map_or(String::new(), |w| {
            format!("<div class=\"who\"><b>{}</b></div>", escape(w))
        });
        h += &format!(
            "<a class=\"{}\" href=\"{up}{}\"><span class=\"glyph\">{}</span><h3>{}</h3><p>{}</p>{who}</a>\n",
            part.key,
            out_path(part.pages().next().unwrap()),
            escape(&part.glyph),
            escape(&part.title),
            escape(&part.blurb)
        );
    }
    h += "</div>";
    h
}

/// Previous and next page in the same part.
fn pager(ctx: &Ctx, part: &Part, n: usize, up: &str) -> String {
    let pages: Vec<&str> = part.pages().collect();
    if pages.len() < 2 {
        return String::new();
    }
    let kind = if part.numbered { " 장" } else { "" };
    let link = |class: &str, word: &str, i: usize| {
        let p = pages[i];
        let title = &ctx.pages[p].title;
        let title = if part.numbered {
            format!("{}. {title}", i + 1)
        } else {
            title.clone()
        };
        format!(
            "<a class=\"{class}\" href=\"{up}{}\"><small>{word}{kind}</small>{}</a>",
            out_path(p),
            escape(&title)
        )
    };
    let mut h = String::from("<nav class=\"pager\" aria-label=\"이전과 다음\">");
    if n > 1 {
        h += &link("prev", "이전", n - 2);
    }
    if n < pages.len() {
        h += &link("next", "다음", n);
    }
    h += "</nav>\n";
    h
}

fn pre(class: &str, text: &str) -> String {
    let c = if class.is_empty() {
        String::new()
    } else {
        format!(" class=\"{class}\"")
    };
    format!("<pre{c}><code>{}</code></pre>", escape(text))
}

/// A fence: the code, its fixtures, and its real output.
fn code_html(c: &Code, outcome: Option<&Outcome>) -> String {
    let mut h = String::new();
    let mut invocation = String::new();
    if !c.env.is_empty() || !c.args.is_empty() || c.stdin.is_some() {
        let env: Vec<String> = c.env.iter().map(|(k, v)| format!("{k}={v} ")).collect();
        invocation = format!("{}fung example.fung", env.concat());
        for a in &c.args {
            invocation += &format!(" {a}");
        }
        if let Some(s) = &c.stdin {
            invocation += &format!(" < {s}");
        }
    }
    if !c.files.is_empty() || !invocation.is_empty() {
        let names: Vec<&str> = c.files.iter().map(|(n, _)| n.as_str()).collect();
        let summary = if names.is_empty() {
            "실행 방법".to_string()
        } else {
            format!("준비물: {}", names.join(", "))
        };
        h += &format!(
            "<details class=\"fixture\"><summary>{}</summary>\n",
            escape(&summary)
        );
        for (name, contents) in &c.files {
            h += &format!(
                "<p><code>{}</code></p>\n{}\n",
                escape(name),
                pre("plain", contents.trim_end_matches('\n'))
            );
        }
        if !invocation.is_empty() {
            h += &format!("{}\n", pre("plain shell", &invocation));
        }
        h += "</details>\n";
    }
    match &c.kind {
        Kind::Run | Kind::Dump(_) => {
            let o = outcome.expect("every run and dump example has an outcome");
            let quiet = o.output.is_empty() && o.code == 0;
            let shown = !c.hide_output && !quiet;
            if !shown {
                h += &pre("", &c.src);
                return h;
            }
            let out = o.output.trim_end_matches('\n');
            let out = match &c.kind {
                Kind::Dump(stage) => format!("$ fung dump {stage} example.fung\n{out}"),
                _ => out.to_string(),
            };
            let class = match (&c.kind, o.code) {
                (Kind::Dump(_), _) => "out dump plain".to_string(),
                (_, 0) => "out plain".to_string(),
                _ => "out err plain".to_string(),
            };
            let data = if o.code != 0 {
                format!(" data-code=\"{}\"", o.code)
            } else {
                String::new()
            };
            h += &format!(
                "<div class=\"run\">{}<pre class=\"{class}\"{data}><code>{}</code></pre></div>",
                pre("", &c.src),
                escape(&out)
            );
        }
        Kind::Check => {
            h += &pre("", &c.src);
            if let Some(e) = &c.expect {
                h += "\n<p class=\"unchecked\">검사만 하는 예제다. 실행하지 않으므로 아래 출력은 예상이다.</p>\n";
                h += &pre("plain", e);
            }
        }
        Kind::Frag => h += &pre("frag", &c.src),
        Kind::Shell => h += &pre("plain shell", &c.src),
        Kind::Plain => h += &pre("plain", &c.src),
        Kind::Core => h += &pre("core", &c.src),
    }
    h
}
