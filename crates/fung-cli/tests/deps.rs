//! Crate allowlist gate (std only, no tools needed).
//!
//! `deny.toml` `[bans] allow` is the single source of the allowlist. Every
//! package in `Cargo.lock` that has a `source` (that is, comes from a
//! registry or git; workspace crates have none) must be named there.
//! `fung-front` must stay free of dependencies, and `fung-cli` may depend
//! only on workspace crates (`path = "../…"`, in every dependency table
//! kind): crates from outside enter through `fung-vm` alone.
//!
//! Allowlist entry forms understood by this test (and by cargo-deny):
//!   "name"                          any version
//!   "name@1.2.3"                    one version
//!   { crate = "name", ... }         inline table; only `crate` is read
//!   { crate = "name@1.2.3" }        likewise
//! Entries may span several lines; `#` comments are ignored. Workspace crates
//! are listed too (see deny.toml) so that the list is never empty.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Packages of a `Cargo.lock` that have a `source` line: `(name, version)`.
fn external_packages(lock: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let (mut name, mut version, mut source) = (None, None, false);
    let mut flush = |n: &mut Option<String>, v: &mut Option<String>, s: &mut bool| {
        if let (Some(n), true) = (n.take(), *s) {
            out.push((n, v.take().unwrap_or_default()));
        }
        *v = None;
        *s = false;
    };
    for line in lock.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            flush(&mut name, &mut version, &mut source);
        } else if let Some(v) = key_value(line, "name") {
            name = Some(v);
        } else if let Some(v) = key_value(line, "version") {
            version = Some(v);
        } else if key_value(line, "source").is_some() {
            source = true;
        }
    }
    flush(&mut name, &mut version, &mut source);
    out
}

/// `key = "value"` -> value.
fn key_value(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start().strip_prefix('=')?;
    let rest = rest.trim().strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

/// The `allow` array of the `[bans]` table, as written (`name` or `name@ver`).
fn allowlist(deny: &str) -> Vec<String> {
    let mut in_bans = false;
    let mut in_allow = false;
    let mut out = Vec::new();
    for raw in deny.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            in_bans = line == "[bans]";
            in_allow = false;
            continue;
        }
        if in_bans && !in_allow {
            if let Some(rest) = line
                .strip_prefix("allow")
                .and_then(|r| r.trim_start().strip_prefix('='))
            {
                in_allow = true;
                entries(rest, &mut out, &mut in_allow);
            }
        } else if in_bans && in_allow {
            entries(line, &mut out, &mut in_allow);
        }
    }
    out
}

/// Collect entries from one line of the array; clear `open` at the closing `]`.
fn entries(line: &str, out: &mut Vec<String>, open: &mut bool) {
    let mut rest = line;
    while let Some(i) = rest.find(['"', ']']) {
        if rest.as_bytes()[i] == b']' {
            *open = false;
            return;
        }
        let tail = &rest[i + 1..];
        let Some(j) = tail.find('"') else { return };
        let text = &tail[..j];
        rest = &tail[j + 1..];
        // In an inline table `{ crate = "x" }` the strings before the first
        // value are the keys' values; only the one after `crate =` counts.
        let before = &line[..line.len() - tail.len() - 1];
        let before = before.trim_end();
        if before.ends_with("crate =") || before.ends_with("crate=") || !before.contains('{') {
            out.push(text.to_string());
        }
    }
}

/// Packages of `lock` with a source that `allow` does not name.
///
/// `name@ver` is matched exactly against the locked version, while
/// cargo-deny reads it as a semver requirement. Use full versions or bare
/// names in deny.toml so the two agree.
fn violations(lock: &str, allow: &[String]) -> Vec<String> {
    let names: BTreeSet<&str> = allow
        .iter()
        .map(|a| a.split('@').next().unwrap_or(a))
        .collect();
    external_packages(lock)
        .into_iter()
        .filter(|(n, v)| {
            !names.contains(n.as_str())
                || !allow
                    .iter()
                    .filter(|a| a.split('@').next() == Some(n))
                    .any(|a| a.split_once('@').is_none_or(|(_, av)| av == v))
        })
        .map(|(n, v)| format!("{n} {v}"))
        .collect()
}

/// Is `table` (a header without brackets) a dependency table of any kind?
fn is_dependency_table(table: &str) -> bool {
    ["dependencies", "dev-dependencies", "build-dependencies"]
        .iter()
        .any(|kind| {
            table == *kind
                || table.starts_with(&format!("{kind}."))
                || (table.starts_with("target.")
                    && (table.ends_with(&format!(".{kind}"))
                        || table.contains(&format!(".{kind}."))))
        })
}

/// Entries of any dependency table of a manifest: `[dependencies]`,
/// `[dev-dependencies]`, `[build-dependencies]`, their `[target.*.]` forms,
/// and `[dependencies.<name>]`-style headers (the header line is the entry).
/// A line starts a new table only when it is a real header: not inside a
/// multi-line value (tracked by bracket/brace depth).
fn dependency_entries(manifest: &str) -> Vec<String> {
    let mut in_deps = false;
    let mut depth: i32 = 0;
    let mut out = Vec::new();
    for raw in manifest.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if depth == 0 && line.starts_with('[') {
            let table = line.trim_matches(|c| c == '[' || c == ']').trim();
            in_deps = is_dependency_table(table);
            if in_deps && table.contains('.') && line_has_name(table) {
                out.push(line.to_string());
            }
            continue;
        }
        if in_deps && !line.is_empty() {
            out.push(line.to_string());
        }
        let mut quoted = false;
        for c in line.chars() {
            match c {
                '"' => quoted = !quoted,
                '[' | '{' if !quoted => depth += 1,
                ']' | '}' if !quoted => depth -= 1,
                _ => {}
            }
        }
        depth = depth.max(0);
    }
    out
}

/// `[dependencies.libc]`-style: a dependency kind followed by a crate name.
fn line_has_name(table: &str) -> bool {
    ["dependencies.", "dev-dependencies.", "build-dependencies."]
        .iter()
        .any(|k| table.starts_with(k) || table.contains(&format!(".{k}")))
}

/// `key = "value"` or `key = value`: the key and the value without its
/// quotes.
fn pair(text: &str) -> Option<(String, String)> {
    let (k, v) = text.split_once('=')?;
    let v = v.trim();
    let v = v
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(v);
    Some((k.trim().to_string(), v.to_string()))
}

/// The dependencies of a manifest (every dependency table kind, plain and
/// `[dependencies.<name>]` style) that are not workspace path
/// dependencies: written as nothing but `path = "../…"`. A version, git
/// source, `workspace = true`, any other key, or an inline table spread
/// over several lines does not pass.
fn not_workspace_paths(manifest: &str) -> Vec<String> {
    // Per dependency, its keys; `None` when its form was not understood.
    type Keys = Option<Vec<(String, String)>>;
    let mut deps: Vec<(String, Keys)> = Vec::new();
    // Outside a dependency table: None. In a plain one: Some(None). In a
    // `[dependencies.<name>]` one: Some(Some(index into deps)).
    let mut table: Option<Option<usize>> = None;
    let mut depth: i32 = 0;
    for raw in manifest.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        let continued = depth > 0;
        for c in line.chars().scan(false, |q, c| {
            if c == '"' {
                *q = !*q;
            }
            Some((c, *q))
        }) {
            match c {
                ('[' | '{', false) => depth += 1,
                (']' | '}', false) => depth -= 1,
                _ => {}
            }
        }
        depth = depth.max(0);
        if line.is_empty() || continued {
            continue;
        }
        if line.starts_with('[') {
            depth = 0;
            let t = line.trim_matches(|c| c == '[' || c == ']').trim();
            table = if !is_dependency_table(t) {
                None
            } else if line_has_name(t) {
                let name = t.rsplit('.').next().unwrap_or(t).to_string();
                deps.push((name, Some(Vec::new())));
                Some(Some(deps.len() - 1))
            } else {
                Some(None)
            };
            continue;
        }
        match table {
            None => {}
            Some(Some(i)) => match (pair(line), deps[i].1.as_mut()) {
                (Some(kv), Some(keys)) => keys.push(kv),
                _ => deps[i].1 = None,
            },
            Some(None) => {
                let Some((name, value)) = line.split_once('=') else {
                    deps.push((line.to_string(), None));
                    continue;
                };
                let keys = value
                    .trim()
                    .strip_prefix('{')
                    .and_then(|v| v.strip_suffix('}'))
                    .and_then(|inner| {
                        inner
                            .split(',')
                            .filter(|p| !p.trim().is_empty())
                            .map(pair)
                            .collect::<Option<Vec<_>>>()
                    });
                deps.push((name.trim().to_string(), keys));
            }
        }
    }
    deps.into_iter()
        .filter(|(_, keys)| {
            !matches!(keys.as_deref(), Some([(k, v)]) if k == "path" && v.starts_with("../"))
        })
        .map(|(name, _)| name)
        .collect()
}

#[test]
fn repository_lock_is_inside_the_allowlist() {
    let lock = fs::read_to_string(root().join("Cargo.lock")).unwrap();
    let deny = fs::read_to_string(root().join("deny.toml")).unwrap();
    let allow = allowlist(&deny);
    assert!(
        !allow.is_empty(),
        "deny.toml [bans] allow must not be empty"
    );
    let bad = violations(&lock, &allow);
    assert!(
        bad.is_empty(),
        "packages outside deny.toml allowlist: {bad:?}"
    );
}

#[test]
fn package_outside_the_allowlist_fails() {
    let lock = "version = 4\n\n[[package]]\nname = \"fung-front\"\nversion = \"0.0.0\"\n\n\
        [[package]]\nname = \"evil\"\nversion = \"1.0.0\"\n\
        source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
        checksum = \"abc\"\n";
    let allow = vec!["fung-front".to_string(), "memchr".to_string()];
    assert_eq!(violations(lock, &allow), vec!["evil 1.0.0".to_string()]);
    // Workspace crates (no source) are not checked; a listed crate passes.
    let ok = lock.replace("evil", "memchr");
    assert!(violations(&ok, &allow).is_empty());
    // A pinned version must match.
    let pinned = vec!["evil@2.0.0".to_string()];
    assert_eq!(violations(lock, &pinned), vec!["evil 1.0.0".to_string()]);
    let pinned_ok = vec!["evil@1.0.0".to_string()];
    assert!(violations(lock, &pinned_ok).is_empty());
}

#[test]
fn allowlist_parser_reads_all_entry_forms() {
    let deny = "[licenses]\nallow = [\"MIT\"]\n\n[bans]\nmultiple-versions = \"deny\"\n\
        allow = [\n  \"a\",  # comment\n  \"b@1.2.3\",\n  { crate = \"c\", reason = \"why\" },\n\
        { crate = \"d@2.0.0\" },\n]\n\n[sources]\nallow = [\"nope\"]\n";
    assert_eq!(allowlist(deny), vec!["a", "b@1.2.3", "c", "d@2.0.0"]);
    let one_line = "[bans]\nallow = [\"x\", \"y\"]\n";
    assert_eq!(allowlist(one_line), vec!["x", "y"]);
}

#[test]
fn dependency_scanner_sees_every_dependency_table_form() {
    let clean = "[package]\nname = \"x\"\n\n[dependencies]\n# nothing\n\n[features]\nf = [\"a\"]\n";
    assert!(dependency_entries(clean).is_empty());
    for table in [
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
        "target.'cfg(unix)'.dependencies",
        "target.'cfg(unix)'.dev-dependencies",
        "target.'cfg(unix)'.build-dependencies",
    ] {
        let m = format!("[{table}]\nlibc = \"0.2\"\n");
        assert_eq!(dependency_entries(&m), vec!["libc = \"0.2\""], "{table}");
    }
    // Table-header style: the header itself is the entry.
    for header in [
        "[dependencies.libc]",
        "[dev-dependencies.libc]",
        "[build-dependencies.libc]",
        "[target.'cfg(unix)'.dependencies.libc]",
    ] {
        let m = format!("{header}\nversion = \"0.2\"\n");
        assert!(!dependency_entries(&m).is_empty(), "{header}");
    }
}

#[test]
fn only_a_real_table_header_ends_a_section() {
    // A `[` at the start of a continuation line inside a value is not a header.
    let m = "[dependencies]\nlibc = { version = \"0.2\", features = [\n[\"x\"]\n] }\n\n[features]\nf = []\n";
    assert_eq!(dependency_entries(m).len(), 3);
    // After a real header, a later `[` inside another table's value stays out.
    let m = "[features]\nf = [\n[\"x\"]\n]\n\n[package]\nname = \"x\"\n";
    assert!(dependency_entries(m).is_empty());
}

#[test]
fn fung_front_has_no_dependencies() {
    let manifest = fs::read_to_string(root().join("crates/fung-front/Cargo.toml")).unwrap();
    let deps = dependency_entries(&manifest);
    assert!(
        deps.is_empty(),
        "fung-front must have 0 dependencies: {deps:?}"
    );
}

#[test]
fn fung_cli_depends_only_on_workspace_crates() {
    let manifest = fs::read_to_string(root().join("crates/fung-cli/Cargo.toml")).unwrap();
    let bad = not_workspace_paths(&manifest);
    assert!(
        bad.is_empty(),
        "fung-cli may only depend on workspace crates (path = \"../…\"): {bad:?}"
    );
}

#[test]
fn workspace_path_fence_rejects_other_dependencies() {
    let ok = "[package]\nname = \"x\"\n\n[dependencies]\na = { path = \"../a\" }\nb = {path=\"../b\"}\n\n\
        [dev-dependencies]\nc = { path = \"../c\" }\n\n[dependencies.d]\npath = \"../d\"\n\n\
        [target.'cfg(unix)'.build-dependencies]\ne = { path = \"../e\" }\n\n[features]\nf = [\"a\"]\n";
    assert_eq!(not_workspace_paths(ok), Vec::<String>::new());
    for (manifest, want) in [
        ("[dependencies]\nregex = \"1\"\n", "regex"),
        ("[dev-dependencies]\nregex = { version = \"1\" }\n", "regex"),
        (
            "[build-dependencies]\ncc = { path = \"../cc\", version = \"1\" }\n",
            "cc",
        ),
        ("[dependencies]\nlib = { path = \"/abs/lib\" }\n", "lib"),
        ("[dependencies]\nlib = { path = \"lib\" }\n", "lib"),
        (
            "[dependencies]\ng = { git = \"https://example.com/g\" }\n",
            "g",
        ),
        ("[dependencies]\nw = { workspace = true }\n", "w"),
        (
            "[dependencies]\nm = { path = \"../m\",\n  features = [\"x\"] }\n",
            "m",
        ),
        ("[dependencies.libc]\nversion = \"0.2\"\n", "libc"),
        (
            "[dependencies.libc]\npath = \"../libc\"\nversion = \"0.2\"\n",
            "libc",
        ),
        (
            "[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n",
            "libc",
        ),
    ] {
        assert_eq!(
            not_workspace_paths(manifest),
            vec![want.to_string()],
            "{manifest}"
        );
    }
}
