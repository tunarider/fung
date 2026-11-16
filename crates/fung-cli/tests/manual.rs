//! The manual's function reference (`docs/latest/reference/{prelude,modules}/`,
//! generated from `manual/reference/`) against the
//! runtime's own parameter tables. Every public prelude name and
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

/// One documented name: its signature as the manual writes it
/// (`get(m, k, default = nil)`, or `args` for a plain value) and whether
/// it is written in fung.
#[derive(Debug, PartialEq)]
struct Entry {
    sig: String,
    fung: bool,
}

/// A native's signature: `name(a: type, b = default, ...rest)`, with the
/// types its table names (D165).
fn native_sig(name: &str, n: &Native) -> String {
    let params: Vec<String> = n
        .params
        .iter()
        .map(|p| {
            let types = p.types.map(|t| format!(": {t}")).unwrap_or_default();
            match (&p.default, p.rest) {
                (_, true) => format!("...{}{types}", p.name),
                (Some(d), _) => format!("{}{types} = {}", p.name, format(&d.to_value())),
                (None, _) => format!("{}{types}", p.name),
            }
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

/// A fung parameter as the manual writes it: as the source does, with
/// its type annotation (`headers: map = {}`).
fn fung_param(p: &str) -> String {
    p.trim().to_string()
}

/// The public names a fung source defines at its top level: `fn` with
/// its signature, or an alias of a `_native.*` member (`read = _n.read`)
/// with that native's.
fn fung_entries(src: &str) -> BTreeMap<String, Entry> {
    let mut aliases: BTreeMap<String, String> = BTreeMap::new();
    let mut out = BTreeMap::new();
    let mut at = 0;
    for line in src.lines() {
        let start = at;
        at += line.len() + 1;
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
            // The parameter list ends at the `)` that closes the first
            // `(`, which may be on a later line (`fmt` keeps a long head
            // over lines): look in the source from there on.
            let after = &src[start + "fn ".len() + open..];
            let list = &after[1..closing(after)];
            let params: Vec<String> = split_top(list, ',')
                .iter()
                // a line break and the next line's indent read as one space
                .map(|p| {
                    p.split('\n')
                        .map(str::trim_start)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .filter(|p| !p.trim().is_empty())
                .map(|p| fung_param(&p))
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
        .chain([("args", Value::Nil)])
    {
        let sig = match &v {
            Value::Native(n) => native_sig(name, n),
            _ => name.to_string(),
        };
        out.insert(name.to_string(), Entry { sig, fung: false });
    }
    for (_, src) in extras::PRELUDE_FILES {
        out.extend(fung_entries(src));
    }
    for m in extras::MODULES {
        for (name, mut e) in fung_entries(extras::source(m).unwrap()) {
            e.sig = format!("{m}.{}", e.sig);
            out.insert(format!("{m}-{name}"), e);
        }
    }
    out
}

/// The function entries of a generated reference page:
/// `<section class="entry" id="fn-ID">`, its `<h3>` (with a
/// `<span class="by">` mark for one written in fung) and
/// `<div class="sig"><code>SIG</code></div>`.
fn documented(html: &str) -> BTreeMap<String, Entry> {
    let mut out = BTreeMap::new();
    let marker = "<section class=\"entry\" id=\"fn-";
    let mut rest = html;
    while let Some(i) = rest.find(marker) {
        rest = &rest[i + marker.len()..];
        let id = &rest[..rest.find('"').unwrap()];
        let entry = &rest[..rest.find("</section>").unwrap()];
        let h3 = &entry[entry.find("<h3>").unwrap()..entry.find("</h3>").unwrap()];
        let fung = h3.contains("<span class=\"by\"");
        let sig_at =
            entry.find("<div class=\"sig\"><code>").unwrap() + "<div class=\"sig\"><code>".len();
        let sig = unescape(&entry[sig_at..sig_at + entry[sig_at..].find("</code>").unwrap()]);
        let old = out.insert(id.to_string(), Entry { sig, fung });
        assert!(old.is_none(), "fn-{id} twice");
    }
    out
}

/// Every function entry of the generated reference pages.
fn documented_all() -> BTreeMap<String, Entry> {
    let root = common::repo().join("docs/latest/reference");
    let mut out = BTreeMap::new();
    for dir in ["prelude", "modules"] {
        let mut files: Vec<_> = std::fs::read_dir(root.join(dir))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        files.sort();
        for f in files {
            for (id, e) in documented(&std::fs::read_to_string(&f).unwrap()) {
                let old = out.insert(id.clone(), e);
                assert!(old.is_none(), "fn-{id} on two pages");
            }
        }
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
    let html = std::fs::read_to_string(common::repo().join("docs/latest/reference/functions.html"))
        .unwrap();
    let want = expected();
    let got = documented_all();
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
    assert_eq!(
        fung_param("headers: map = {}"),
        "headers: map = {}".to_string()
    );
    assert_eq!(
        fung_param(" body: str | nil = nil"),
        "body: str | nil = nil"
    );
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
            sig: "read(path: str)".into(),
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
    // a head over several lines, as `fmt` keeps it
    let e = fung_entries("fn h(a,\n  b = [1,\n    2], c = \"x  y\"):\n  a\n;;\n");
    assert_eq!(e["h"].sig, "h(a, b = [1, 2], c = \"x  y\")");
}

/// What the instruction reference writes for an operand.
fn operand_text(o: fung_front::bytecode::Operand) -> &'static str {
    use fung_front::bytecode::Operand::*;
    match o {
        None => "없음",
        Const => "상수 번호",
        Global => "전역 번호",
        Local => "칸 번호",
        Proto => "함수 번호",
        Captured => "붙잡은 값 번호",
        Type => "타입 번호",
        Pattern => "패턴 번호",
        Assert => "assert 번호",
        Target => "명령 번호",
        Count => "개수",
        Flags => "플래그",
    }
}

/// The instruction reference (`manual/vm/instructions.md`) against the
/// front end's instruction table: every instruction has one row, and the
/// row's operand and stack columns say what the table says. The last
/// column is prose.
#[test]
fn the_instruction_reference_matches_the_instruction_table() {
    use fung_front::bytecode::{Effect, Op, Operand};
    let page = std::fs::read_to_string(common::repo().join("manual/vm/instructions.md")).unwrap();
    let mut rows: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut bad = Vec::new();
    for l in page.lines() {
        let Some(rest) = l.trim().strip_prefix("<tr><td><code>") else {
            continue;
        };
        let cells: Vec<&str> = rest.split("</td><td>").collect();
        let Some(name) = cells[0].strip_suffix("</code>") else {
            continue;
        };
        if cells.len() < 4 || !name.chars().all(|c| c.is_ascii_uppercase()) {
            continue;
        }
        let row = (cells[1].to_string(), cells[2].to_string());
        if rows.insert(name.to_string(), row).is_some() {
            bad.push(format!("{name} has two rows"));
        }
    }
    for op in Op::ALL {
        let info = op.info();
        let operand = match op.second() {
            Operand::None => operand_text(info.operand).to_string(),
            b => format!("{}, {}", operand_text(info.operand), operand_text(b)),
        };
        let stack = match info.effect {
            Effect::Fixed(a, b) => format!("{a} → {b}"),
            Effect::Items => "n → 1".to_string(),
            Effect::Sliced => "1~3 → 1".to_string(),
            Effect::Called => "1 + 인자 → 1".to_string(),
            Effect::Asserted => "1~2 → 0".to_string(),
        };
        match rows.remove(op.name()) {
            None => bad.push(format!("{} has no row", op.name())),
            Some(got) if got != (operand.clone(), stack.clone()) => bad.push(format!(
                "{}: manual {got:?}, Rust {:?}",
                op.name(),
                (operand, stack)
            )),
            Some(_) => {}
        }
    }
    for name in rows.keys() {
        bad.push(format!("{name} is not an instruction"));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    let count = format!("명령 {}가지", Op::ALL.len());
    assert!(page.contains(&count), "want {count}");
}
