//! The manual's function reference (`docs/builtins.html`) against the
//! Rust implementation's own parameter tables (stage 6a: the manual
//! describes the Rust implementation). Every public prelude name and
//! every public member of the standard modules has exactly one entry,
//! the entry's signature is the real one (names, defaults, rest), and the
//! "fung으로 작성" mark is on exactly the functions written in fung.
//!
//! Where the tables come from: natives from `fung_vm` (the core prelude,
//! the concurrency natives and the `_native.*` modules, with their
//! default values), fung functions from the embedded sources the Rust
//! boot and loader read (`fung_front::extras`).

mod common;

use std::collections::BTreeMap;

use fung_front::extras;
use fung_vm::value::{Native, Value, format};

/// The standard modules the manual documents, in its order.
const MODULES: &[&str] = &["fs", "path", "cli", "json", "time", "regex", "http", "conc"];

/// One documented name: its signature as the manual writes it
/// (`get(m, k, default = nil)`, or `args` for a plain value) and whether
/// it is written in fung.
#[derive(Debug, PartialEq)]
struct Entry {
    sig: String,
    fung: bool,
}

/// A native's signature: `name(a, b = default, ...rest)`.
fn native_sig(name: &str, n: &Native) -> String {
    let params: Vec<String> = n
        .params
        .iter()
        .map(|p| match (&p.default, p.rest) {
            (_, true) => format!("...{}", p.name),
            (Some(d), _) => format!("{} = {}", p.name, format(&d.to_value())),
            (None, _) => p.name.to_string(),
        })
        .collect();
    format!("{name}({})", params.join(", "))
}

/// The index of the `)` that closes the `(` at the start of `s`, outside
/// strings.
fn closing(s: &str) -> usize {
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '`' => quote = Some(c),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    panic!("no closing parenthesis in {s:?}")
}

/// Splits `s` at the `sep`s outside brackets and strings.
fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            out.last_mut().unwrap().push(c);
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.last_mut().unwrap().push(n);
                }
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '`' => quote = Some(c),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if c == sep && depth == 0 {
            out.push(String::new());
        } else {
            out.last_mut().unwrap().push(c);
        }
    }
    out
}

/// A fung parameter as the manual writes it: the type annotation
/// dropped (`url: str` → `url`, `headers: map = {}` → `headers = {}`).
fn fung_param(p: &str) -> String {
    let p = p.trim();
    if p.starts_with("...") {
        return p.to_string();
    }
    let (head, default) = match p.find(" = ") {
        Some(i) => (&p[..i], Some(p[i + 3..].trim())),
        None => (p, None),
    };
    let name = head.split(':').next().unwrap().trim();
    match default {
        Some(d) => format!("{name} = {d}"),
        None => name.to_string(),
    }
}

/// The public names a fung source defines at its top level: `fn` with
/// its signature, or an alias of a `_native.*` member (`read = _n.read`)
/// with that native's.
fn fung_entries(src: &str) -> BTreeMap<String, Entry> {
    let mut aliases: BTreeMap<String, String> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for line in src.lines() {
        if line.starts_with(char::is_whitespace) || line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("import ") {
            if let Some((key, alias)) = rest.split_once(" as ") {
                aliases.insert(alias.trim().to_string(), key.trim().to_string());
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("fn ") {
            let open = rest.find('(').unwrap();
            let name = rest[..open].trim();
            if name.starts_with('_') {
                continue;
            }
            // The parameter list ends at the `)` that closes the first `(`.
            let after = &rest[open..];
            let list = &after[1..closing(after)];
            let params: Vec<String> = split_top(list, ',')
                .iter()
                .filter(|p| !p.trim().is_empty())
                .map(|p| fung_param(p))
                .collect();
            out.insert(
                name.to_string(),
                Entry {
                    sig: format!("{name}({})", params.join(", ")),
                    fung: true,
                },
            );
            continue;
        }
        if let Some((name, value)) = line
            .split_once(" = ")
            .filter(|(n, _)| n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        {
            if name.starts_with('_') {
                continue;
            }
            let (alias, member) = value
                .trim()
                .split_once('.')
                .unwrap_or_else(|| panic!("a public value that is not an alias: {line}"));
            let key = &aliases[alias];
            let module = fung_vm::boot::native_module(key).unwrap();
            let Some(Value::Native(n)) = module.get(member) else {
                panic!("{key}.{member} is not a native");
            };
            out.insert(
                name.to_string(),
                Entry {
                    sig: native_sig(name, n),
                    fung: false,
                },
            );
            continue;
        }
        // The rest: test blocks, type aliases, continued expressions.
    }
    out
}

/// What the manual must document, by its entry id (`get`, `fs-read`).
fn expected() -> BTreeMap<String, Entry> {
    let mut out = BTreeMap::new();
    for (name, v) in fung_vm::prelude::core()
        .into_iter()
        .chain(fung_vm::conc::natives())
    {
        let sig = match &v {
            Value::Native(n) => native_sig(name, n),
            _ => name.to_string(),
        };
        out.insert(name.to_string(), Entry { sig, fung: false });
    }
    // `dbg`'s `text` and `where` are filled in by the compiler (M6): a
    // program calls `dbg(x)`.
    let dbg = out.get_mut("dbg").unwrap();
    assert_eq!(dbg.sig, "dbg(x, text = nil, where = nil)");
    dbg.sig = "dbg(x)".to_string();
    for (_, src) in extras::PRELUDE_FILES {
        out.extend(fung_entries(src));
    }
    for m in MODULES {
        for (name, mut e) in fung_entries(extras::source(m).unwrap()) {
            e.sig = format!("{m}.{}", e.sig);
            out.insert(format!("{m}-{name}"), e);
        }
    }
    out
}

/// The entries of `docs/builtins.html`: `<div class="fn" id="fn-ID"><code>SIG</code>`
/// and whether a `<span class="by">` mark follows.
fn documented(html: &str) -> BTreeMap<String, Entry> {
    let mut out = BTreeMap::new();
    let marker = "<div class=\"fn\" id=\"fn-";
    let mut rest = html;
    while let Some(i) = rest.find(marker) {
        rest = &rest[i + marker.len()..];
        let id = &rest[..rest.find('"').unwrap()];
        let code = rest.find("><code>").unwrap() + "><code>".len();
        let end = rest[code..].find("</code>").unwrap() + code;
        let sig = unescape(&rest[code..end]);
        let line_end = rest[end..].find('\n').unwrap() + end;
        let fung = rest[end..line_end].contains("<span class=\"by\"");
        let old = out.insert(id.to_string(), Entry { sig, fung });
        assert!(old.is_none(), "fn-{id} twice");
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[test]
fn the_function_reference_matches_the_parameter_tables() {
    let html = std::fs::read_to_string(common::repo().join("docs/builtins.html")).unwrap();
    let want = expected();
    let got = documented(&html);
    let mut bad = Vec::new();
    for (id, w) in &want {
        match got.get(id) {
            None => bad.push(format!("missing fn-{id}: {}", w.sig)),
            Some(g) if g != w => bad.push(format!("fn-{id}: manual {g:?}, Rust {w:?}")),
            Some(_) => {}
        }
    }
    for id in got.keys().filter(|id| !want.contains_key(*id)) {
        bad.push(format!("fn-{id} is not a Rust built-in"));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    let count = format!("<p id=\"fn-count\">{}개 함수</p>", want.len());
    assert!(html.contains(&count), "want {count}");
}

#[test]
fn signatures_are_read_like_the_manual_writes_them() {
    assert_eq!(fung_param("headers: map = {}"), "headers = {}".to_string());
    assert_eq!(fung_param(" body: str | nil = nil"), "body = nil");
    assert_eq!(fung_param("...parts"), "...parts");
    assert_eq!(
        split_top("a, b = [1, 2], c = \"x,)\"", ','),
        ["a", " b = [1, 2]", " c = \"x,)\""]
    );
    let e =
        fung_entries("import _native.fs as _n\nread = _n.read\nfn f(a, b = 1) = a\nfn _g() = 1\n");
    assert_eq!(
        e["read"],
        Entry {
            sig: "read(path)".into(),
            fung: false
        }
    );
    assert_eq!(
        e["f"],
        Entry {
            sig: "f(a, b = 1)".into(),
            fung: true
        }
    );
    assert!(!e.contains_key("_g"));
}
