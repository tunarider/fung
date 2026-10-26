//! The whole site: every page of one version and its assets, the links
//! between them checked; the shared files (`versions.js`, the old
//! addresses); and comparing with, writing to and freezing into `docs/`.

use std::collections::BTreeMap;
use std::path::Path;

use super::examples::run_all;
use super::markup;
use super::render::{Ctx, Version, og, out_path, page_html};
use super::toc;

/// The old single-page addresses and where they live now (relative to
/// `docs/`). The anchors are the same.
pub const OLD: &[(&str, &str)] = &[
    ("index.html", "latest/index.html"),
    ("syntax.html", "latest/reference/language.html"),
    ("builtins.html", "latest/reference/functions.html"),
    ("internals.html", "latest/vm/internals.html"),
];

/// Builds version `v` from the manuscripts in `manual`: paths relative
/// to `docs/<v.dir>/`.
pub fn build(manual: &Path, bin: &Path, v: &Version) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let read = |p: &Path| std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()));
    let toc = toc::parse(&read(&manual.join("toc.txt"))?)?;
    let mut wanted: Vec<String> = vec!["index.md".into()];
    for part in &toc.parts {
        wanted.extend(part.pages().map(String::from));
    }
    // every manuscript is in the contents
    for md in manuscripts(manual) {
        if !wanted.contains(&md) {
            return Err(format!("{md}: a manuscript that toc.txt does not list"));
        }
    }
    let mut pages = BTreeMap::new();
    for md in &wanted {
        let page = markup::parse(md, &read(&manual.join(md))?)?;
        pages.insert(md.clone(), page);
    }
    let examples: Vec<(String, &markup::Code)> =
        pages.values().flat_map(markup::examples).collect();
    let results = run_all(bin, &examples).map_err(|e| e.join("\n"))?;
    let wordmark = read(&manual.join("assets/wordmark.svg"))?;
    // the error catalog beside the manual (`spec/errors.txt`): a manual
    // without error entries needs none, one with them must have it
    let catalog = match manual.parent().map(|r| r.join("spec/errors.txt")) {
        Some(p) if p.exists() => read_catalog(&read(&p)?)?,
        _ if error_entries(&pages).is_empty() => BTreeMap::new(),
        _ => {
            return Err(
                "spec/errors.txt is missing beside the manual, which has `:::error` entries".into(),
            );
        }
    };
    if !catalog.is_empty() {
        check_catalog(&pages, &catalog)?;
    }
    let ctx = Ctx {
        toc: &toc,
        pages: &pages,
        results: &results,
        version: v,
        wordmark: wordmark.trim_end(),
        catalog: &catalog,
    };
    let mut files = BTreeMap::new();
    for md in &wanted {
        files.insert(out_path(md), page_html(&ctx, md).into_bytes());
    }
    for e in std::fs::read_dir(manual.join("assets")).map_err(|e| e.to_string())? {
        let p = e.map_err(|e| e.to_string())?.path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if name != "wordmark.svg" {
            files.insert(
                format!("assets/{name}"),
                std::fs::read(&p).map_err(|e| e.to_string())?,
            );
        }
    }
    let mut broken = Vec::new();
    for (name, bytes) in files.iter().filter(|(n, _)| n.ends_with(".html")) {
        let mut seen: Vec<String> = Vec::new();
        for id in ids(&String::from_utf8_lossy(bytes)) {
            if seen.contains(&id) {
                broken.push(format!("{name}: the id `{id}` is used twice"));
            }
            seen.push(id);
        }
    }
    broken.extend(check_links(&files));
    if !broken.is_empty() {
        return Err(broken.join("\n"));
    }
    Ok(files)
}

/// `spec/errors.txt`: code → (message, help). Every line has the four
/// tab-separated fields (code, message, label, help) the catalog test
/// writes; one that does not is a broken file.
pub fn read_catalog(text: &str) -> Result<BTreeMap<String, (String, String)>, String> {
    let mut out = BTreeMap::new();
    for (i, l) in text.lines().enumerate() {
        if l.starts_with('#') || l.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() != 4 {
            return Err(format!(
                "spec/errors.txt:{}: {} fields, not 4: {l:?}",
                i + 1,
                f.len()
            ));
        }
        out.insert(f[0].to_string(), (f[1].to_string(), f[3].to_string()));
    }
    Ok(out)
}

/// The `:::error` entries of the pages: (code, page, line).
fn error_entries(pages: &BTreeMap<String, markup::Page>) -> Vec<(String, String, usize)> {
    fn walk(blocks: &[markup::Block], page: &str, out: &mut Vec<(String, String, usize)>) {
        for b in blocks {
            if let markup::Block::Container {
                kind,
                arg,
                line,
                blocks,
            } = b
            {
                if kind == "error" {
                    out.push((arg.clone(), page.to_string(), *line));
                }
                walk(blocks, page, out);
            }
        }
    }
    let mut entries = Vec::new();
    for (path, page) in pages {
        walk(&page.blocks, path, &mut entries);
    }
    entries
}

/// Every code of the catalog has exactly one `:::error` entry, and every
/// entry is a code of the catalog.
fn check_catalog(
    pages: &BTreeMap<String, markup::Page>,
    catalog: &BTreeMap<String, (String, String)>,
) -> Result<(), String> {
    let entries = error_entries(pages);
    let mut bad = Vec::new();
    // an entry whose examples fail must show its own error in one of them
    fn failing(blocks: &[markup::Block], out: &mut Vec<String>) {
        for b in blocks {
            match b {
                markup::Block::Code(c) if c.exit != 0 => {
                    out.extend(c.error.clone());
                }
                markup::Block::Container { blocks, .. } => failing(blocks, out),
                _ => {}
            }
        }
    }
    fn own(blocks: &[markup::Block], page: &str, bad: &mut Vec<String>) {
        for b in blocks {
            if let markup::Block::Container {
                kind,
                arg,
                line,
                blocks,
            } = b
            {
                if kind == "error" {
                    let mut shown = Vec::new();
                    failing(blocks, &mut shown);
                    if !shown.is_empty() && !shown.contains(arg) {
                        bad.push(format!(
                            "{page}:{line}: `:::error {arg}` shows {} but never {arg}",
                            shown.join(", ")
                        ));
                    }
                }
                own(blocks, page, bad);
            }
        }
    }
    for (path, page) in pages {
        own(&page.blocks, path, &mut bad);
    }
    for (code, page, line) in &entries {
        if !catalog.contains_key(code) {
            bad.push(format!(
                "{page}:{line}: `:::error {code}` is not in spec/errors.txt"
            ));
        }
        if entries.iter().filter(|(c, ..)| c == code).count() > 1 {
            bad.push(format!("{page}:{line}: a second `:::error {code}`"));
        }
    }
    for code in catalog.keys() {
        if !entries.iter().any(|(c, ..)| c == code) {
            bad.push(format!(
                "{code} (spec/errors.txt) has no `:::error {code}` entry in the manual"
            ));
        }
    }
    if bad.is_empty() {
        Ok(())
    } else {
        Err(bad.join("\n"))
    }
}

/// Every `.md` under `manual`, relative to it.
fn manuscripts(manual: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![manual.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "md") {
                out.push(
                    p.strip_prefix(manual)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    out.sort();
    out
}

/// The files shared by every version: paths relative to `docs/`. The
/// old front page keeps the cover's link preview (`description`).
pub fn shared(
    versions: &[Version],
    description: Option<&str>,
    builtins: &BTreeMap<String, String>,
) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    files.insert(
        "versions.js".to_string(),
        versions_js(versions).into_bytes(),
    );
    for (old, new) in OLD {
        let meta = if *old == "index.html" {
            og("https://blog.tunarider.net/fung/", description)
        } else {
            String::new()
        };
        let anchors = if *old == "builtins.html" {
            builtins.clone()
        } else {
            BTreeMap::new()
        };
        files.insert(old.to_string(), forward(new, &anchors, &meta).into_bytes());
    }
    files
}

/// Where the old single function page's anchors went: an entry's
/// `fn-…` id to its page and anchor, and a group's id (the old
/// `#strings`) to the page that now holds that group.
pub fn builtins_anchors(built: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (path, bytes) in built {
        let Some(stem) = ["reference/prelude/", "reference/modules/"]
            .iter()
            .find_map(|d| path.strip_prefix(d))
            .and_then(|f| f.strip_suffix(".html"))
        else {
            continue;
        };
        out.insert(stem.to_string(), format!("latest/{path}"));
        for id in ids(&String::from_utf8_lossy(bytes)) {
            if id.starts_with("fn-") {
                out.insert(id.clone(), format!("latest/{path}#{id}"));
            }
        }
    }
    out
}
/// A page that sends its visitor (and the `#anchor`) to `to`, or, for
/// an anchor in `anchors`, to where that anchor went.
fn forward(to: &str, anchors: &BTreeMap<String, String>, meta: &str) -> String {
    let script = if anchors.is_empty() {
        format!("location.replace(\"{to}\" + location.hash)")
    } else {
        let pairs: Vec<String> = anchors
            .iter()
            .map(|(k, v)| format!("\"{k}\": \"{v}\""))
            .collect();
        format!(
            "var m = {{{}}};\nvar h = location.hash.slice(1);\nlocation.replace(m[h] || \"{to}\" + location.hash)",
            pairs.join(",\n")
        )
    };
    format!(
        "<!doctype html>\n<html lang=\"ko\">\n<head>\n<meta charset=\"utf-8\">\n<title>fung 매뉴얼</title>\n\
         <script>{script}</script>\n\
         <meta http-equiv=\"refresh\" content=\"0; url={to}\">\n<link rel=\"canonical\" href=\"{to}\">\n{meta}</head>\n\
         <body>\n<p>매뉴얼이 옮겨졌다: <a href=\"{to}\">{to}</a></p>\n</body>\n</html>\n"
    )
}
/// `docs/versions.js`.
pub fn versions_js(versions: &[Version]) -> String {
    let mut s = String::from("window.FUNG_VERSIONS = [\n");
    let lines: Vec<String> = versions
        .iter()
        .map(|v| match &v.tag {
            Some(t) => format!(
                "{{\"dir\": \"{}\", \"name\": \"{}\", \"tag\": \"{t}\"}}",
                v.dir, v.name
            ),
            None => format!("{{\"dir\": \"{}\", \"name\": \"{}\"}}", v.dir, v.name),
        })
        .collect();
    s += &lines.join(",\n");
    s += "\n];\n";
    s
}

/// The versions `versions_js` wrote.
pub fn read_versions(js: &str) -> Vec<Version> {
    let field = |line: &str, key: &str| {
        let k = format!("\"{key}\": \"");
        line.find(&k).map(|i| {
            let rest = &line[i + k.len()..];
            rest[..rest.find('"').unwrap()].to_string()
        })
    };
    js.lines()
        .filter(|l| l.trim_start().starts_with('{'))
        .map(|l| Version {
            dir: field(l, "dir").unwrap(),
            name: field(l, "name").unwrap(),
            tag: field(l, "tag"),
        })
        .collect()
}

/// Broken relative links and anchors among `files`. Links that leave the
/// version's directory (`../versions.js`, `../assets/logo/…`) go to the
/// shared files and are not checked here.
pub fn check_links(files: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    let mut bad = Vec::new();
    for (name, bytes) in files.iter().filter(|(n, _)| n.ends_with(".html")) {
        let html = String::from_utf8_lossy(bytes);
        let dir: Vec<&str> =
            name.split('/').collect::<Vec<_>>()[..name.matches('/').count()].to_vec();
        for attr in ["href=\"", "src=\""] {
            for (i, _) in html.match_indices(attr) {
                let rest = &html[i + attr.len()..];
                let link = &rest[..rest.find('"').unwrap_or(0)];
                if link.is_empty()
                    || ["http:", "https:", "mailto:", "data:"]
                        .iter()
                        .any(|s| link.starts_with(s))
                {
                    continue;
                }
                if link.starts_with('/') {
                    bad.push(format!(
                        "{name}: an absolute link `{link}` (links are relative)"
                    ));
                    continue;
                }
                let (file, anchor) = link.split_once('#').unwrap_or((link, ""));
                let target = if file.is_empty() {
                    Some(name.clone())
                } else {
                    resolve(&dir, file)
                };
                let Some(target) = target else {
                    continue; // outside the version: a shared file
                };
                let Some(t) = files.get(&target) else {
                    bad.push(format!("{name}: a link to `{link}`, which is not built"));
                    continue;
                };
                if !anchor.is_empty()
                    && !ids(&String::from_utf8_lossy(t)).iter().any(|i| i == anchor)
                {
                    bad.push(format!(
                        "{name}: a link to `{link}`, whose anchor is not there"
                    ));
                }
            }
        }
    }
    bad
}

/// The page's element ids (`id="…"` as an attribute, not `data-id`).
fn ids(html: &str) -> Vec<String> {
    html.match_indices("id=\"")
        .filter(|(i, _)| html[..*i].ends_with(char::is_whitespace))
        .map(|(i, _)| {
            let rest = &html[i + 4..];
            rest[..rest.find('"').unwrap_or(0)].to_string()
        })
        .collect()
}

/// `file` relative to the directory `dir`, or `None` when it leaves the
/// version's root.
fn resolve(dir: &[&str], file: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.to_vec();
    for seg in file.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

/// Compares `docs/latest/` and the shared files with what was built;
/// with `update`, writes them instead and removes stray files. The
/// differences (or, with `update`, what changed).
pub fn sync(
    docs: &Path,
    built: &BTreeMap<String, Vec<u8>>,
    shared: &BTreeMap<String, Vec<u8>>,
    update: bool,
) -> Vec<String> {
    let mut wanted: BTreeMap<String, &Vec<u8>> = BTreeMap::new();
    for (k, v) in built {
        wanted.insert(format!("latest/{k}"), v);
    }
    for (k, v) in shared {
        wanted.insert(k.clone(), v);
    }
    let mut diff = Vec::new();
    for (rel, want) in &wanted {
        let path = docs.join(rel);
        match std::fs::read(&path) {
            Ok(have) if &have == *want => {}
            have => {
                diff.push(format!(
                    "{rel}: {}",
                    if have.is_ok() { "differs" } else { "missing" }
                ));
                if update {
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(&path, want).unwrap();
                }
            }
        }
    }
    let latest = docs.join("latest");
    let mut stack = vec![latest.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let rel = format!(
                "latest/{}",
                p.strip_prefix(&latest).unwrap().to_string_lossy()
            );
            if !wanted.contains_key(&rel) {
                diff.push(format!("{rel}: not built (a stray file)"));
                if update {
                    std::fs::remove_file(&p).unwrap();
                }
            }
        }
    }
    diff
}

/// Freezes the manuscripts as release `name` into `docs/<name>/` and
/// adds it to `docs/versions.js`, after `latest`.
pub fn freeze(docs: &Path, manual: &Path, bin: &Path, name: &str) -> Result<(), String> {
    let out = docs.join(name);
    if out.exists() {
        return Err(format!(
            "{} already exists: a frozen version is never rebuilt",
            out.display()
        ));
    }
    let v = Version {
        dir: name.to_string(),
        name: name.to_string(),
        tag: None,
    };
    let files = build(manual, bin, &v)?;
    for (rel, bytes) in &files {
        let p = out.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
    }
    let mut versions = match std::fs::read_to_string(docs.join("versions.js")) {
        Ok(js) => read_versions(&js),
        Err(_) => vec![latest()],
    };
    versions.insert(1.min(versions.len()), v);
    std::fs::write(docs.join("versions.js"), versions_js(&versions)).map_err(|e| e.to_string())
}

/// The `latest` version: main, not released yet.
pub fn latest() -> Version {
    Version {
        dir: "latest".into(),
        name: "latest".into(),
        tag: Some("unreleased".into()),
    }
}
