//! The repository is built and checked by Cargo alone: no `.go` source,
//! no module file, and no workflow, script, test, fung source or current
//! document (`CLAUDE.md`, `README.md`, `docs` outside the history) that
//! calls the other toolchain. Nor does any of them name that language
//! (stage 6c), but for the sentences that compare fung with it
//! (`COMPARISONS`). The history documents (`docs/notes`, `docs/plans`,
//! `docs/superpowers`, `CHANGELOG.md`) record how the project got here
//! and are left out, and so are the manuals frozen at a release
//! (`docs/<x.y.z>/`).

mod common;

use std::path::{Path, PathBuf};

/// Directories that are not part of the source tree: build output, git's
/// own data and local tool state (every dot directory but `.github` and
/// `.cargo`).
fn skipped_dir(rel: &Path) -> bool {
    let name = rel.file_name().unwrap().to_string_lossy();
    frozen_manual(rel)
        || rel == Path::new("target")
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

/// A manual frozen at a release (`docs/0.4.0`): a record, never edited.
fn frozen_manual(rel: &Path) -> bool {
    rel.parent() == Some(Path::new("docs"))
        && rel
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(|c: char| c.is_ascii_digit())
}

/// Every file under `dir` (relative to the repository), skipping
/// [`skipped_dir`].
fn files(root: &Path, dir: &Path) -> Vec<PathBuf> {
    files_where(root, dir, &skipped_dir)
}

/// Every file under `dir`, skipping the directories `skip` names.
fn files_where(root: &Path, dir: &Path, skip: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
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
                if !skip(&rel) {
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
        "manual",
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

/// The language's name, spelt in pieces so this file does not match
/// itself.
fn lang() -> String {
    ["G", "o"].concat()
}

/// The sentences that compare fung with that language, which the current
/// documents may keep: (file, a piece of the line with `{L}` for the
/// name). Code has none.
const COMPARISONS: &[(&str, &str)] = &[
    // the influences (00, background) and the tagline that sums them up
    ("README.md", "문법의 겉모양은 {L}·Python을"),
    ("docs/design/00-concept.md", "## \"겉은 {L}, 속은 Lisp\""),
    (
        "docs/design/00-concept.md",
        "작성하는 문법은 {L}/Python 계열",
    ),
    ("docs/design/00-concept.md", "- **{L}**: 표면 문법의 간결함"),
    ("docs/design/background.md", "| {L} | 문법의 간결함 |"),
    (
        "docs/design/background.md",
        "\"겉은 {L}, 속은 Lisp\" 균일 트리",
    ),
    ("manual/assets/style.css", "(\"겉은 {L}, 속은 Lisp\")"),
    ("docs/latest/assets/style.css", "(\"겉은 {L}, 속은 Lisp\")"),
    ("manual/index.md", "fung — {L}처럼 쓰고 Lisp처럼 도는"),
    // the old front page keeps the cover's link preview
    ("docs/index.html", "fung — {L}처럼 쓰고 Lisp처럼 도는"),
    (
        "docs/latest/index.html",
        "fung — {L}처럼 쓰고 Lisp처럼 도는",
    ),
    ("manual/index.md", "lede: {L}처럼 쓰고 Lisp처럼 도는"),
    (
        "docs/latest/index.html",
        "<p class=\"lede\">{L}처럼 쓰고 Lisp처럼 도는",
    ),
    ("manual/index.md", "이것이 \"겉은 {L}, 속은 Lisp\"의 실체다"),
    (
        "docs/latest/index.html",
        "이것이 \"겉은 {L}, 속은 Lisp\"의 실체다",
    ),
    ("manual/index.md", "문법의 겉모양은 {L}·Python을"),
    ("docs/latest/index.html", "문법의 겉모양은 {L}·Python을"),
    ("manual/reference/language.md", "겉은 {L}와 Python을 닮았고"),
    (
        "docs/latest/reference/language.html",
        "겉은 {L}와 Python을 닮았고",
    ),
    ("scripts/logo/render.html", "'{L}처럼 쓰고 Lisp처럼 도는"),
    ("scripts/logo/render.html", "'가나다 {L}'"),
    // design choices explained against the language that has the feature
    ("docs/design/01-syntax.md", "({L}·Rust·Java와 같음)"),
    (
        "docs/design/01-syntax.md",
        "자체는 넣을 수 없다 ({L}와 같음)",
    ),
    (
        "docs/design/05-concurrency.md",
        "{L}처럼 패닉이 아니라 `Err`인 이유",
    ),
    ("docs/design/05-concurrency.md", "원시 연산 이름은 {L} 용어"),
    (
        "docs/design/05-concurrency.md",
        "{L} `select`에 해당하는 원시 연산은 두지 않는다",
    ),
    (
        "docs/design/05-concurrency.md",
        "즉시 중단 ({L} 고루틴 패닉)",
    ),
    (
        "docs/design/05-concurrency.md",
        "({L} `context` 트리와 같음)",
    ),
    (
        "docs/design/05-concurrency.md",
        "({L} `Server.Shutdown`, Trio의 관례)",
    ),
    (
        "docs/design/08-tooling-and-errors.md",
        "{L}의 포매터처럼 옵션 없는 단일 스타일",
    ),
    // a book title
    (
        "docs/design/09-implementation-plan.md",
        "Writing A Compiler In {L}",
    ),
];

/// Whether `line` names the language as a word. Only ASCII letters,
/// digits and `_` continue a word: Korean particles attach to it
/// (`…와`, `…의`, `…처럼`) and must not hide it.
fn names_lang(line: &str) -> bool {
    let name = lang();
    let word = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    line.match_indices(name.as_str()).any(|(i, _)| {
        !word(line[..i].chars().next_back()) && !word(line[i + name.len()..].chars().next())
    })
}

/// Whether `line` names a source file of the other toolchain (a file
/// name ending in its extension, test files included), the way a comment
/// points at what it was ported from.
fn names_its_files(line: &str) -> bool {
    let ext = [".", "go"].concat();
    let word = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    line.match_indices(ext.as_str()).any(|(i, _)| {
        word(line[..i].chars().next_back()) && !word(line[i + ext.len()..].chars().next())
    })
}

#[test]
fn source_file_names_are_found() {
    let ext = [".", "go"].concat();
    assert!(names_its_files(&format!("// ---- m4_test{ext} ----")));
    assert!(names_its_files(&format!("from `cmd/fung/main{ext}`.")));
    assert!(!names_its_files(&format!("x{ext}od and {ext}")));
    assert!(!names_its_files("see main.rs"));
}

#[test]
fn the_name_is_found_as_a_word() {
    let l = lang();
    assert!(names_lang(&format!("as {l} does")));
    assert!(names_lang(&format!("({l} `x`)")));
    assert!(names_lang(&format!("{l}'s")));
    assert!(!names_lang(&format!("{l}ogle {l}od")));
    assert!(!names_lang(&format!("{}es", l.to_lowercase())));
    // Korean particles do not make it another word
    assert!(names_lang(&format!("{l}처럼, {l}와 {l}의")));
    assert!(names_lang(&format!("가{l}")));
}

/// Stage 6c's completion rule: no code and no current document names the
/// other language, but a comparison sentence of the list.
#[test]
fn no_code_or_current_document_names_the_other_language() {
    let root = common::repo();
    // the expected outputs are code's too; the history is not
    let skip = |rel: &Path| {
        let name = rel.file_name().unwrap().to_string_lossy();
        frozen_manual(rel)
            || rel == Path::new("target")
            || (name.starts_with('.') && name != ".github" && name != ".cargo")
            || ["docs/notes", "docs/plans", "docs/superpowers"]
                .iter()
                .any(|h| rel == Path::new(h))
    };
    let allowed: Vec<(PathBuf, String)> = COMPARISONS
        .iter()
        .map(|(f, piece)| (PathBuf::from(f), piece.replace("{L}", &lang())))
        .collect();
    let mut bad = Vec::new();
    for top in [
        ".github",
        ".cargo",
        "scripts",
        "bench",
        "crates",
        "tests",
        "spec",
        "install.sh",
        "Cargo.toml",
        "deny.toml",
        "CLAUDE.md",
        "README.md",
        "extras",
        "examples",
        "manual",
        "docs",
    ] {
        let top = Path::new(top);
        let list = if root.join(top).is_dir() {
            files_where(&root, top, &skip)
        } else {
            vec![top.to_path_buf()]
        };
        for p in list {
            let Ok(text) = std::fs::read_to_string(root.join(&p)) else {
                continue;
            };
            for (n, line) in text.lines().enumerate() {
                let ok = allowed
                    .iter()
                    .any(|(f, piece)| *f == p && line.contains(piece.as_str()));
                if (names_lang(line) || names_its_files(line)) && !ok {
                    bad.push(format!("{}:{}", p.display(), n + 1));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} lines name it:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// Every allowed comparison still matches a line of its file, so the list
/// does not keep entries the documents dropped.
#[test]
fn every_comparison_is_still_in_its_file() {
    let root = common::repo();
    let stale: Vec<_> = COMPARISONS
        .iter()
        .filter(|(f, piece)| {
            let piece = piece.replace("{L}", &lang());
            let text = std::fs::read_to_string(root.join(f)).unwrap_or_default();
            !text.lines().any(|l| l.contains(piece.as_str()))
        })
        .collect();
    assert!(stale.is_empty(), "stale: {stale:?}");
}

/// No type is declared `Send` or `Sync` by hand (R5): what crosses
/// threads is plain data, and the compiler checks it.
#[test]
fn no_unsafe_send_or_sync() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let needles = [
        concat!("unsafe impl", " Send"),
        concat!("unsafe impl", " Sync"),
    ];
    let mut found = Vec::new();
    for f in files(&root, &root.join("crates")) {
        if f.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            if needles.iter().any(|n| line.contains(n)) {
                found.push(format!("{}:{}", f.display(), i + 1));
            }
        }
    }
    assert!(found.is_empty(), "{found:?}");
}

/// Code and current documents do not describe the run token or the
/// waits of the runtime before R1b as if they were there (R5): the
/// stage notes, plans, specs and the decision log keep the history.
#[test]
fn no_current_text_describes_the_run_token() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let needles = [
        "run token",
        "실행권",
        concat!("outside", "("),
        concat!("block_until", "("),
        "Census",
        "교착 집계",
        "token held",
        "holds the token",
        "holds the\ntoken",
        "takes the token",
        "keeps the token",
        "gives the token",
        "asserts the token",
        "without the token",
        "the token's holder",
        "share the token",
    ];
    let mut paths: Vec<PathBuf> = files(&root, &root.join("crates"))
        .into_iter()
        .filter(|f| f.extension().is_some_and(|e| e == "rs" || e == "fung"))
        .collect();
    paths.extend(
        files(&root, Path::new("manual"))
            .into_iter()
            .filter(|f| f.extension().is_some_and(|e| e == "md"))
            .map(|f| root.join(f)),
    );
    for doc in [
        "docs/design/00-concept.md",
        "docs/design/05-concurrency.md",
        "docs/design/06-native-and-stdlib.md",
        "docs/design/08-tooling-and-errors.md",
        "docs/design/09-implementation-plan.md",
    ] {
        paths.push(root.join(doc));
    }
    let me = Path::new(file!()).file_name().unwrap().to_owned();
    let mut found = Vec::new();
    for f in paths {
        if f.file_name() == Some(&me) {
            continue;
        }
        // the guard against the old waits names them on purpose
        if f.ends_with("crates/fung-vm/src/vm/park_tests.rs") {
            continue;
        }
        // a phrase may wrap: each line is read with the next one's start
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        for (i, raw) in lines.iter().enumerate() {
            let next = lines.get(i + 1).map_or("", |l| {
                l.trim_start().trim_start_matches(['/', '!', '#', '*', ' '])
            });
            let first_word = next.split_whitespace().next().unwrap_or("");
            let joined = format!("{raw} {first_word}");
            let line = joined.as_str();
            let hit = needles.iter().any(|n| {
                line.match_indices(n).any(|(at, _)| {
                    // a whole word: not the tail of a longer name
                    !line[..at].ends_with(|c: char| c.is_alphanumeric() || c == '_')
                })
            });
            if hit {
                found.push(format!(
                    "{}:{}",
                    f.strip_prefix(&root).unwrap_or(&f).display(),
                    i + 1
                ));
            }
        }
    }
    assert!(
        found.is_empty(),
        "{} lines:\n{}",
        found.len(),
        found.join("\n")
    );
}

#[test]
fn frozen_manuals_are_records() {
    assert!(frozen_manual(Path::new("docs/0.4.0")));
    assert!(!frozen_manual(Path::new("docs/latest")));
    assert!(!frozen_manual(Path::new("docs/design")));
    assert!(!frozen_manual(Path::new("crates/0.4.0")));
}
