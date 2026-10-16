//! Crate allowlist gate (std only, no tools needed).
//!
//! `deny.toml` `[bans] allow` is the single source of the allowlist. Every
//! package in `Cargo.lock` that has a `source` (that is, comes from a
//! registry or git; workspace crates have none) must be named there.
//! `fung-front` must stay free of dependencies.
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

/// Entries under `[dependencies]` or `[target.*.dependencies]` of a manifest.
fn dependency_entries(manifest: &str) -> Vec<String> {
    let mut in_deps = false;
    let mut out = Vec::new();
    for raw in manifest.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            let table = line.trim_matches(|c| c == '[' || c == ']');
            in_deps = table == "dependencies"
                || (table.starts_with("target.") && table.ends_with(".dependencies"));
        } else if in_deps && !line.is_empty() {
            out.push(line.to_string());
        }
    }
    out
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
fn dependency_scanner_sees_plain_and_target_tables() {
    let clean =
        "[package]\nname = \"x\"\n\n[dependencies]\n# nothing\n\n[dev-dependencies]\nq = \"1\"\n";
    assert!(dependency_entries(clean).is_empty());
    let dirty = "[dependencies]\nlibc = \"0.2\"\n";
    assert_eq!(dependency_entries(dirty), vec!["libc = \"0.2\""]);
    let target = "[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n";
    assert_eq!(dependency_entries(target), vec!["libc = \"0.2\""]);
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
