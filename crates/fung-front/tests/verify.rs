//! The bytecode verifier (VM22) on everything the compiler makes from the
//! repository's fung code: the language tests, the examples, the
//! benchmarks, the embedded modules and the manual's programs. The
//! compiler runs it itself in debug builds; this test runs it in release
//! builds too, and fails on the first broken promise.

use std::path::{Path, PathBuf};

use fung_front::bytecode::{Proto, verify};
use fung_front::loader::{Loader, Stage};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The `.fung` files under `dir`, at any depth, sorted.
fn fung_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            fung_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "fung") {
            out.push(p);
        }
    }
}

/// The programs in the manual's `fung run`, `fung check` and
/// `dump bytecode` fences, with where each is.
fn manual_programs(dir: &Path, out: &mut Vec<(String, String)>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            manual_programs(&p, out);
            continue;
        }
        if p.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap();
        let mut fence: Option<(usize, String)> = None;
        for (i, line) in text.lines().enumerate() {
            match &mut fence {
                Some((at, body)) if line.trim_start() == "```" => {
                    out.push((format!("{}:{}", p.display(), *at + 1), std::mem::take(body)));
                    fence = None;
                }
                Some((_, body)) => {
                    body.push_str(line);
                    body.push('\n');
                }
                None => {
                    let info = line.trim_start().strip_prefix("```").unwrap_or("");
                    if info.starts_with("fung run")
                        || info.starts_with("fung check")
                        || info.starts_with("dump bytecode")
                    {
                        fence = Some((i, String::new()));
                    }
                }
            }
        }
    }
}

/// Verifies what compiling `src` makes, the entry and every module it
/// loads. Returns how many protos (with their children) passed, 0 when it
/// does not compile (an error test, or a fence needing files of its own).
fn check(loader: &Loader, file: &str, src: &str) -> usize {
    let (build, errs) = loader.compile(file, src, Stage::All);
    if !errs.is_empty() {
        return 0;
    }
    let mut protos: Vec<&Proto> = build.proto.iter().collect();
    protos.extend(build.modules.iter().filter_map(|m| m.proto.as_ref()));
    for p in &protos {
        if let Err(v) = verify(p) {
            panic!("{file}: {v}");
        }
    }
    protos.len()
}

#[test]
fn everything_the_compiler_makes_from_the_repository_verifies() {
    let root = repo();
    let user = Loader {
        tests: true,
        ..Loader::default()
    };
    let mut files = Vec::new();
    for dir in ["tests/lang", "examples", "bench"] {
        fung_files(&root.join(dir), &mut files);
    }
    let mut verified = 0;
    let mut compiled = 0;
    for f in &files {
        let src = std::fs::read_to_string(f).unwrap();
        let n = check(&user, &f.display().to_string(), &src);
        verified += n;
        compiled += usize::from(n > 0);
    }
    assert!(
        compiled > 200,
        "only {compiled} of {} files compiled",
        files.len()
    );

    let internal = Loader {
        internal: true,
        library: true,
        tests: true,
        ..Loader::default()
    };
    let mut extras = Vec::new();
    fung_files(&root.join("extras"), &mut extras);
    for f in &extras {
        let src = std::fs::read_to_string(f).unwrap();
        assert!(
            check(&internal, &f.display().to_string(), &src) > 0,
            "{}",
            f.display()
        );
    }

    let mut fences = Vec::new();
    manual_programs(&root.join("manual"), &mut fences);
    let mut fenced = 0;
    for (at, src) in &fences {
        let n = check(&user, at, src);
        verified += n;
        fenced += usize::from(n > 0);
    }
    assert!(
        fenced > 500,
        "only {fenced} of {} fences compiled",
        fences.len()
    );
    assert!(verified > 1000, "{verified}");
}
