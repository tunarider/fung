//! The repository is built and checked by Cargo alone: no `.go` source,
//! no module file, and no workflow, script, test, fung source or current document
//! (`CLAUDE.md`, `README.md`, `docs` outside the history) that calls the other
//! toolchain. The history documents (`docs/notes`, `docs/plans`,
//! `docs/superpowers`, `CHANGELOG.md`) record how the project got here
//! and are left out.

mod common;

use std::path::{Path, PathBuf};

/// Directories that are not part of the source tree: build output, git's
/// own data and local tool state (every dot directory but `.github` and
/// `.cargo`).
fn skipped_dir(rel: &Path) -> bool {
    let name = rel.file_name().unwrap().to_string_lossy();
    rel == Path::new("target")
        || (name.starts_with('.') && name != ".github" && name != ".cargo")
        || [
            "docs/notes",
            "docs/plans",
            "docs/superpowers",
            "crates/fung-cli/tests/expected",
        ]
        .iter()
        .any(|h| rel == Path::new(h))
}

/// Every file under `dir` (relative to the repository), skipping
/// [`skipped_dir`].
fn files(root: &Path, dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(root.join(&d)) else {
            continue;
        };
        for e in entries {
            let e = e.unwrap();
            let rel = d.join(e.file_name());
            if e.file_type().unwrap().is_dir() {
                if !skipped_dir(&rel) {
                    stack.push(rel);
                }
            } else {
                out.push(rel);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn no_sources_or_module_file_of_another_toolchain() {
    let root = common::repo();
    let bad: Vec<_> = files(&root, Path::new(""))
        .into_iter()
        .filter(|p| {
            p.extension().is_some_and(|x| x == "go")
                || ["go.mod", "go.sum", "go.work"]
                    .iter()
                    .any(|n| p.ends_with(n))
        })
        .collect();
    assert!(bad.is_empty(), "left over: {bad:?}");
}

/// Commands that run the other toolchain, spelt in pieces so this file
/// does not match itself.
fn tool_calls() -> Vec<String> {
    let g = "go";
    vec![
        format!("{g} run"),
        format!("{g} test"),
        format!("{g} build"),
        format!("{g} vet"),
        format!("{g}fmt"),
        format!("{g} list"),
        format!("{g} mod"),
        format!("{g} install"),
        format!("{g} generate"),
        format!("{g} env"),
        format!("{g} tool"),
        format!("setup-{g}"),
        format!("Command::new(\"{g}\")"),
    ]
}

/// Whether `line` has `call` as whole words (`cargo test` is not a
/// call of the other tool's `test`).
fn calls(line: &str, call: &str) -> bool {
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    line.match_indices(call).any(|(i, _)| {
        !word(line[..i].chars().next_back()) && !word(line[i + call.len()..].chars().next())
    })
}

#[test]
fn whole_words_only() {
    let call = format!("{} test", "go");
    assert!(calls(&format!("run: {call} ./..."), &call));
    assert!(calls(&format!("`{call}`"), &call));
    assert!(!calls("cargo test -p fung-cli", &call));
    assert!(!calls(&format!("{call}s"), &call));
}

#[test]
fn no_workflow_script_or_test_calls_another_toolchain() {
    let root = common::repo();
    let mut bad = Vec::new();
    for top in [
        ".github",
        ".cargo",
        "scripts",
        "bench",
        "crates",
        "tests",
        "install.sh",
        "Cargo.toml",
        "deny.toml",
        ".gitignore",
        "CLAUDE.md",
        "README.md",
        "extras",
        "examples",
        "docs",
    ] {
        let top = Path::new(top);
        let list = if root.join(top).is_dir() {
            files(&root, top)
        } else {
            vec![top.to_path_buf()]
        };
        for p in list {
            let Ok(text) = std::fs::read_to_string(root.join(&p)) else {
                continue;
            };
            for (n, line) in text.lines().enumerate() {
                for call in tool_calls() {
                    if calls(line, &call) {
                        bad.push(format!("{}:{}: {call}", p.display(), n + 1));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "calls left:\n{}", bad.join("\n"));
}

#[test]
fn the_scan_sees_the_tree() {
    let root = common::repo();
    let all = files(&root, Path::new(""));
    assert!(all.iter().any(|p| p == Path::new("Cargo.toml")));
    assert!(
        all.iter()
            .any(|p| p == Path::new(".github/workflows/ci.yml"))
    );
    assert!(
        all.iter()
            .any(|p| p == Path::new("docs/design/open-questions.md"))
    );
    assert!(!all.iter().any(|p| p.starts_with("docs/notes")));
    assert!(!all.iter().any(|p| p.starts_with("target")));
}
