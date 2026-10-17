//! The tool commands compared with the Go implementation (stage 5):
//! `fung check`, `version`, `help` and unknown commands, byte for byte
//! (standard output, standard error, exit status), plus the path walker
//! `fung test` and `fung fmt` share (`src/paths.rs`), compared with the
//! files Go's commands visit. Later tasks add `test` and `fmt` here.
//!
//! What is compared, and what is not:
//! - `check` on every `.fung` file of `tests/lang` (with `_lib`),
//!   `examples` and `extras` (with `extras/rust`), the warning fixtures in
//!   `tests/data/check`, the manual's runnable examples, `-e` programs,
//!   and argument errors.
//! - Not compared: a `# only: go` file and the atom examples (atoms are
//!   Go only since stage 3b; Rust reports `atom` as undefined by design)
//!   and a manual example using atoms (stage 6 rewrites them). A
//!   `# only: rust` file is checked by Rust alone and must have no errors.

mod common;
#[allow(dead_code)]
#[path = "../src/paths.rs"]
mod paths;

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use common::{diff, go_fung, repo, run, run_status, rust_fung};

/// Both implementations: Go built for this test program, Rust under test.
pub struct Both {
    pub go: PathBuf,
    pub rust: PathBuf,
}

/// Both binaries, or `None` when Go is unavailable and parity is not
/// required (see `common::go_fung`).
pub fn both(tag: &str) -> Option<Both> {
    Some(Both {
        go: go_fung(tag)?,
        rust: rust_fung(),
    })
}

pub fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

/// Runs every case with both implementations (in parallel, from the
/// repository root) and returns `label: where they differ` for each case
/// that differs.
pub fn differences(b: &Both, cases: &[(String, Vec<OsString>)]) -> Vec<String> {
    let results = par_map(cases, |(_, args)| (run(&b.go, args), run(&b.rust, args)));
    cases
        .iter()
        .zip(results)
        .filter_map(|((label, _), (go, rust))| diff(&go, &rust).map(|d| format!("{label}: {d}")))
        .collect()
}

/// Panics listing every differing case.
pub fn assert_same(b: &Both, cases: &[(String, Vec<OsString>)]) {
    let bad = differences(b, cases);
    assert!(
        bad.is_empty(),
        "{} of {} cases differ:\n{}",
        bad.len(),
        cases.len(),
        bad.join("\n")
    );
}

/// `f` over `xs` on a few threads, results in order.
pub fn par_map<T: Sync, R: Send>(xs: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let n = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let chunk = xs.len().div_ceil(n).max(1);
    std::thread::scope(|s| {
        let handles: Vec<_> = xs
            .chunks(chunk)
            .map(|c| s.spawn(|| c.iter().map(&f).collect::<Vec<R>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    })
}

/// Every `.fung` file below the given directories (relative to the
/// repository root, `_` fixtures included), sorted.
pub fn corpus(dirs: &[&str]) -> Vec<String> {
    let mut files = Vec::new();
    for dir in dirs {
        let mut stack = vec![repo().join(dir)];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "fung") {
                    let rel = p.strip_prefix(repo()).unwrap();
                    files.push(rel.to_string_lossy().into_owned());
                }
            }
        }
    }
    files.sort();
    files
}

/// The value of a file's `# only:` line (`go` or `rust`), if it has one.
pub fn only(src: &str) -> Option<String> {
    src.lines().find_map(|l| {
        l.trim()
            .strip_prefix("# only:")
            .map(|v| v.trim().to_string())
    })
}

/// The examples that still use atoms, which only Go has (as in
/// `parity.rs`); stage 6 rewrites them to agents.
pub const ATOM_EXAMPLES: &[&str] = &["examples/api.fung", "examples/serve.fung"];

/// Whether both implementations are meant to treat a repository file
/// alike: not a `# only:` file, not an atom example.
pub fn shared(path: &str) -> bool {
    let src = std::fs::read_to_string(repo().join(path)).unwrap();
    !ATOM_EXAMPLES.contains(&path) && only(&src).is_none()
}

/// The manual's runnable examples: every `<pre>` of `docs/*.html` that is
/// not a fragment, a plain drawing or a core tree (`class="frag"`,
/// `"plain"`, `"core"`), as `(page:n, source)`. CLAUDE.md requires them
/// to pass `fung check`, `fmt --check` and a run.
pub fn manual_examples() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut pages: Vec<PathBuf> = std::fs::read_dir(repo().join("docs"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "html"))
        .collect();
    pages.sort();
    for page in pages {
        let html = std::fs::read_to_string(&page).unwrap();
        let name = page.file_stem().unwrap().to_string_lossy().into_owned();
        let mut rest = html.as_str();
        let mut n = 0;
        while let Some(i) = rest.find("<pre") {
            rest = &rest[i..];
            let open_end = rest.find('>').unwrap();
            let attrs = &rest[4..open_end];
            let body = &rest[open_end + 1..];
            let end = body.find("</pre>").unwrap();
            let block = &body[..end];
            rest = &body[end..];
            if attrs.contains("class=") {
                continue;
            }
            let code = block
                .strip_prefix("<code>")
                .and_then(|b| b.strip_suffix("</code>"))
                .unwrap_or_else(|| panic!("{name}: a <pre> without one <code>: {block:.40}"));
            n += 1;
            out.push((format!("{name}:{n}"), unescape(code)));
        }
    }
    out
}

/// The HTML entities the manual's code uses.
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let semi = rest[i..].find(';').unwrap() + i;
        out.push_str(match &rest[i..=semi] {
            "&lt;" => "<",
            "&gt;" => ">",
            "&amp;" => "&",
            "&quot;" => "\"",
            "&#x27;" | "&#39;" => "'",
            e => panic!("unknown entity {e} in a manual example"),
        });
        rest = &rest[semi + 1..];
    }
    out.push_str(rest);
    out
}

/// A fresh scratch directory below Cargo's target directory (inside the
/// repository, so tests can also name it by a relative path).
pub fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `path` relative to the repository root, when it lies below it.
pub fn relative(path: &Path) -> Option<String> {
    path.strip_prefix(repo())
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

fn check(arg: &str) -> (String, Vec<OsString>) {
    (format!("check {arg}"), os_args(&["check", arg]))
}

#[test]
fn check_matches_go_on_the_repository() {
    let Some(b) = both("tools-check") else { return };
    let files = corpus(&["tests/lang", "examples", "extras"]);
    assert!(files.len() > 150, "corpus: {} files", files.len());
    assert!(files.iter().any(|f| f.starts_with("extras/rust/")));
    assert!(files.iter().any(|f| f.starts_with("tests/lang/_lib/")));
    let (same, own): (Vec<&String>, Vec<&String>) = files.iter().partition(|f| shared(f));
    let cases: Vec<_> = same.iter().map(|f| check(f)).collect();
    assert_same(&b, &cases);
    // A `# only: rust` file compiles in Rust (its run is golden-tested).
    for f in own {
        let src = std::fs::read_to_string(repo().join(f)).unwrap();
        if only(&src).as_deref() == Some("rust") {
            let (_, err, status) = run_status(&b.rust, &os_args(&["check", f]), None);
            assert!(
                status.success() && !String::from_utf8_lossy(&err).contains("error["),
                "{f}: {status}\n{}",
                String::from_utf8_lossy(&err)
            );
        }
    }
}

#[test]
fn check_matches_go_on_warning_fixtures() {
    let Some(b) = both("tools-warn") else { return };
    let files = corpus(&["crates/fung-cli/tests/data/check"]);
    assert!(files.len() >= 9, "{files:?}");
    let cases: Vec<_> = files.iter().map(|f| check(f)).collect();
    assert_same(&b, &cases);
    // The fixtures do warn: comparing two silent runs would prove nothing.
    let (_, err, _) = run(
        &b.go,
        &os_args(&["check", "crates/fung-cli/tests/data/check/unused_vars.fung"]),
    );
    assert!(String::from_utf8_lossy(&err).contains("warning[W0102]"));
}

#[test]
fn check_matches_go_on_manual_examples() {
    let Some(b) = both("tools-manual") else {
        return;
    };
    let dir = scratch("tools-manual");
    let examples = manual_examples();
    assert!(examples.len() > 40, "{} examples", examples.len());
    let mut cases = Vec::new();
    for (label, src) in &examples {
        if src.contains("atom(") {
            continue; // atoms are Go only (stage 6 rewrites the manual)
        }
        let path = dir.join(format!("{}.fung", label.replace(':', "_")));
        std::fs::write(&path, src).unwrap();
        cases.push((
            format!("check {label}"),
            vec!["check".into(), path.into_os_string()],
        ));
    }
    assert!(cases.len() + 2 >= examples.len());
    assert_same(&b, &cases);
}

const CHECK_E: &[&str] = &[
    "import fs",
    "import fs as f\nprint(f.read)",
    "fn f():\n  x = 1\n  2\n;;",
    "fn map(xs) = xs\nlen = 1",
    "print(if true: _y = 1 ;;)",
    "fn f():\n  x = 1\n  len = 2\n  x\n;;",
    "fn f():\n  이름 = 1\n  2\n;;",
    "x = )",
    "print(nope)",
    "test \"t\":\n  assert 1 == 1\n;;",
    "fn f():\n  test \"t\":\n    assert true\n  ;;\n;;",
    "",
];

#[test]
fn check_matches_go_on_inline_code_and_arguments() {
    let Some(b) = both("tools-args") else { return };
    let mut cases: Vec<(String, Vec<OsString>)> = CHECK_E
        .iter()
        .map(|src| (format!("check -e {src:?}"), os_args(&["check", "-e", src])))
        .collect();
    let arg_cases: &[&[&str]] = &[
        &["check"],
        &["check", "-e"],
        &["check", "-e", "print(1)", "ignored", "too"],
        &["check", "a.fung", "b.fung"],
        // Mixed paths: check takes one file, never a directory.
        &["check", "tests/lang", "tests/lang/arith.fung"],
        &["check", "tests/lang"],
        &["check", "nope.fung"],
        &["check", "nope/"],
        &["check", "tests/lang/arith.fung", "-e"],
        &["check", "./tests/lang/../lang/arith.fung"],
        &["check", "--help"],
    ];
    for a in arg_cases {
        cases.push((a.join(" "), os_args(a)));
    }
    // A name that is not UTF-8 is echoed unchanged.
    cases.push((
        "check non-UTF-8 name".into(),
        vec!["check".into(), OsString::from_vec(b"q\xfe.fung".to_vec())],
    ));
    let dir = scratch("tools-args");
    let odd = dir.join(OsString::from_vec(b"w\xfe.fung".to_vec()));
    // macOS file systems refuse names that are not UTF-8 (EILSEQ).
    if std::fs::write(&odd, "fn f():\n  x = 1\n  2\n;;\n").is_ok() {
        cases.push((
            "check non-UTF-8 file".into(),
            vec!["check".into(), odd.into_os_string()],
        ));
    }
    // A file name with a character the excerpt must keep as is.
    let wide = dir.join("넓은.fung");
    std::fs::write(&wide, "fn f():\n  x = 1\n  2\n;;\n").unwrap();
    cases.push((
        "check wide name".into(),
        vec!["check".into(), wide.into_os_string()],
    ));
    assert_same(&b, &cases);
}

#[test]
fn help_and_unknown_commands_match_go() {
    let Some(b) = both("tools-help") else { return };
    let arg_cases: &[&[&str]] = &[
        &["help"],
        &["-h"],
        &["--help"],
        &["help", "check"],
        &["frobnicate"],
        &["Check"],
        &["--version"],
        &["-x"],
    ];
    let cases: Vec<_> = arg_cases
        .iter()
        .map(|a| (a.join(" "), os_args(a)))
        .collect();
    assert_same(&b, &cases);
}

/// The Rust binary's version, as `fung version` prints it.
fn rust_version() -> String {
    let (out, _, _) = run(&rust_fung(), &os_args(&["version"]));
    let text = String::from_utf8(out).unwrap();
    text.strip_prefix("fung ")
        .and_then(|v| v.strip_suffix('\n'))
        .unwrap_or_else(|| panic!("version output {text:?}"))
        .to_string()
}

#[test]
fn version_matches_go_built_with_the_same_version() {
    // Go's development builds report a VCS pseudo-version, so the two are
    // compared with Go linked to the version the Rust binary reports (the
    // release scripts link both the same way).
    let v = rust_version();
    let want = option_env!("FUNG_VERSION")
        .map(|s| s.strip_prefix('v').unwrap_or(s))
        .filter(|s| !s.is_empty() && *s != "(devel)")
        .unwrap_or("0.0.0-dev");
    assert_eq!(v, want);
    let Some(go) = common::go_fung_with(
        "tools-version",
        &[&format!("-ldflags=-X main.linkedVersion={v}")],
    ) else {
        return;
    };
    let b = Both {
        go,
        rust: rust_fung(),
    };
    let cases = vec![
        ("version".to_string(), os_args(&["version"])),
        ("version extra".to_string(), os_args(&["version", "extra"])),
    ];
    assert_same(&b, &cases);
}

// ---- the path walker (`src/paths.rs`) against Go's commands ----

/// A tree with every case Go's walkers treat specially. Every `.fung`
/// file has one passing test and is not formatted (`x=1`), so Go's
/// `fung test` and `fung fmt --check` each name every file they visit.
fn walker_tree(dir: &Path) {
    use std::os::unix::fs::symlink;
    let file = "x=1\ntest \"t\":\n  assert true\n;;\n";
    let t = dir.join("tree");
    for f in [
        "a.fung",
        "B.fung",
        "_f.fung",
        ".h.fung",
        "sub-x.fung",
        "sub.fung",
        "_skip/x.fung",
        ".hidden/y.fung",
        "sub/z.fung",
        "sub/_inner/w.fung",
        "sub/.git/v.fung",
        "sub/deep/er/u.fung",
        "다/가.fung",
    ] {
        let p = t.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, file).unwrap();
    }
    std::fs::write(t.join("notes.txt"), "not fung").unwrap();
    std::fs::write(t.join("fung"), "not fung either").unwrap();
    std::fs::create_dir_all(t.join("empty.fung")).unwrap(); // a directory named .fung
    symlink("a.fung", t.join("link.fung")).unwrap(); // a link to a file: kept
    symlink("sub", t.join("linkdir")).unwrap(); // a link to a directory: not entered
    symlink("tree", dir.join("rootlink")).unwrap();
    symlink("tree/sub", dir.join("sublink.fung")).unwrap();
    std::fs::create_dir_all(dir.join("_under/.dot")).unwrap();
    std::fs::write(dir.join("_under/q.fung"), file).unwrap();
    std::fs::write(dir.join("_under/.dot/r.fung"), file).unwrap();
    std::fs::write(dir.join("lone.fung"), file).unwrap();
}

fn lossy(paths: &[Vec<u8>]) -> Vec<String> {
    paths
        .iter()
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect()
}

/// The files Go's command visited, read from what it printed, or its
/// path error (`error: ...` with the usage exit code 3). Every fixture file is named:
/// `fung test` prints `ok   PATH: t` (or `FAIL PATH: <load>`), `fmt
/// --check` prints the path; either says `error: cannot read PATH: ...`
/// when the file is a directory and shows `--> PATH:` for a file that is
/// not fung. Go handles the files in sorted order, so the names are
/// sorted back together from both streams.
fn go_listed(go: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    let (out, err, status) = run_status(go, &os_args(args), None);
    let (out, err) = (String::from_utf8_lossy(&out), String::from_utf8_lossy(&err));
    if status.code() == Some(3) {
        let e = err
            .strip_prefix("error: ")
            .unwrap_or_else(|| panic!("{args:?}: {err}"));
        return Err(e.lines().next().unwrap().to_string());
    }
    let mut files: Vec<String> = Vec::new();
    for l in out.lines() {
        if let Some(rest) = l.strip_prefix("ok   ") {
            files.push(rest.strip_suffix(": t").unwrap().to_string());
        } else if let Some(rest) = l.strip_prefix("FAIL ") {
            files.push(rest.strip_suffix(": <load>").unwrap().to_string());
        } else if args[0] == "fmt" {
            files.push(l.to_string());
        }
    }
    for l in err.lines() {
        if let Some(rest) = l.strip_prefix("error: cannot read ") {
            files.push(rest.split(": ").next().unwrap().to_string());
        } else if args[0] == "fmt"
            && let Some(rest) = l.strip_prefix("  --> ")
        {
            files.push(rest.rsplitn(3, ':').last().unwrap().to_string());
        }
    }
    files.sort();
    Ok(files)
}

fn ours(r: Result<Vec<Vec<u8>>, Vec<u8>>) -> Result<Vec<String>, String> {
    r.map(|f| lossy(&f))
        .map_err(|e| String::from_utf8_lossy(&e).into_owned())
}

#[test]
fn walker_matches_go() {
    let Some(go) = go_fung("tools-walk") else {
        return;
    };
    // Go runs from the repository root, so the walker does too, for the
    // relative roots. Every other test here names files by absolute
    // paths, so moving this process does not disturb them.
    std::env::set_current_dir(repo()).unwrap();
    let dir = scratch("tools-walk");
    walker_tree(&dir);
    let abs = |p: &str| dir.join(p).to_string_lossy().into_owned();
    let mut roots = vec![
        abs("tree"),
        abs("tree/"),
        abs("tree//sub/../"),
        abs("rootlink"),
        abs("rootlink/"),
        abs("sublink.fung"),
        abs("_under"),
        abs("lone.fung"),
        abs("tree/notes.txt"),
        abs("nope"),
        abs("lone.fung/x"),
    ];
    if let Some(rel) = relative(&dir) {
        roots.push(format!("{rel}/tree"));
        roots.push(format!("./{rel}/tree/sub"));
    }
    let mut bad = Vec::new();
    for root in &roots {
        let want = go_listed(&go, &["test", root]);
        let got = ours(paths::test_files(root.as_bytes()));
        if want != got {
            bad.push(format!("test {root}:\n  go:   {want:?}\n  rust: {got:?}"));
        }
    }
    let mut groups: Vec<Vec<String>> = roots.iter().map(|r| vec![r.clone()]).collect();
    groups.push(vec![
        abs("tree/sub"),
        abs("lone.fung"),
        abs("tree/a.fung"),
        abs("_under"),
    ]);
    groups.push(vec![abs("lone.fung"), abs("lone.fung")]);
    groups.push(vec![abs("tree/sub"), abs("nope"), abs("lone.fung")]);
    for g in &groups {
        let mut args = vec!["fmt", "--check"];
        args.extend(g.iter().map(String::as_str));
        let want = go_listed(&go, &args);
        let raw: Vec<Vec<u8>> = g.iter().map(|p| p.as_bytes().to_vec()).collect();
        let got = ours(paths::fung_files(&raw));
        if want != got {
            bad.push(format!("fmt {g:?}:\n  go:   {want:?}\n  rust: {got:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    // The tree does exercise the rules.
    let all = ours(paths::fung_files(&[abs("tree").into_bytes()])).unwrap();
    assert!(all.iter().any(|f| f.ends_with("/.h.fung")));
    assert!(all.iter().any(|f| f.ends_with("/link.fung")));
    assert!(
        !all.iter()
            .any(|f| f.contains("/_skip/") || f.contains("/.hidden/"))
    );
    let tests = ours(paths::test_files(abs("tree").as_bytes())).unwrap();
    assert!(tests.iter().any(|f| f.contains("/.hidden/")));
    assert!(!tests.iter().any(|f| f.contains("/_skip/")));
}

#[test]
fn walker_reports_unreadable_directories_like_go() {
    let Some(go) = go_fung("tools-walk-perm") else {
        return;
    };
    let dir = scratch("tools-walk-perm");
    std::fs::create_dir_all(dir.join("t/locked")).unwrap();
    std::fs::write(
        dir.join("t/a.fung"),
        "x=1\ntest \"t\":\n  assert true\n;;\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir.join("t/locked"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let root = dir.join("t").to_string_lossy().into_owned();
    let readable = std::fs::read_dir(dir.join("t/locked")).is_ok(); // running as root
    let want = go_listed(&go, &["test", &root]);
    let got = ours(paths::test_files(root.as_bytes()));
    let want2 = go_listed(&go, &["fmt", "--check", &root]);
    let got2 = ours(paths::fung_files(&[root.clone().into_bytes()]));
    std::fs::set_permissions(dir.join("t/locked"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(want, got);
    assert_eq!(want2, got2);
    if !readable {
        assert!(got.is_err(), "{got:?}");
    }
}

#[test]
fn manual_examples_are_extracted() {
    let ex = manual_examples();
    assert!(
        ex.iter()
            .any(|(_, s)| s.contains("fn slug(s) = lower(s) |> split(\" \")"))
    );
    assert!(
        ex.iter()
            .all(|(_, s)| !s.contains("&lt;") && !s.contains("<code>"))
    );
    assert_eq!(
        unescape("a &lt;b&gt; &amp; &quot;c&quot; &#x27;d&#39;"),
        "a <b> & \"c\" 'd'"
    );
    assert_eq!(only("x\n  # only: rust\n").as_deref(), Some("rust"));
    assert_eq!(only("# onlyx\n"), None);
}

// ---- `fung test` (stage 5 task 2) ----

/// The `fung test` fixtures: deliberate failures, `assert` displays,
/// load errors shared through `_lib`, and one file per way a file could
/// leak into the next (exit, at_exit, a deadlock, an unretrieved
/// failure, tasks left running). `test_rust` holds the Rust-only ones.
const TEST_DATA: &str = "crates/fung-cli/tests/data/test";
const TEST_RUST: &str = "crates/fung-cli/tests/data/test_rust";

fn test_case(arg: &str) -> (String, Vec<OsString>) {
    (format!("test {arg}"), os_args(&["test", arg]))
}

#[test]
fn test_matches_go_on_fixtures() {
    let Some(b) = both("tools-test") else { return };
    let files = corpus(&[TEST_DATA]);
    assert!(files.len() >= 19, "{files:?}");
    let mut cases: Vec<_> = files.iter().map(|f| test_case(f)).collect();
    // Every file in one run: what one file leaves must not reach the
    // next (z_clean.fung runs last), a broken library is reported once,
    // and the exit code adds the files up. A `_` root is walked.
    let root = format!("{TEST_DATA}/");
    for arg in [
        TEST_DATA,
        root.as_str(),
        &format!("{TEST_DATA}/_lib"),
        &format!("./{TEST_DATA}/../test/z_clean.fung"),
        &repo().join(TEST_DATA).to_string_lossy(),
    ] {
        cases.push(test_case(arg));
    }
    let arg_cases: &[&[&str]] = &[
        &["test", "a.fung", "b.fung"],
        &["test", "nope"],
        &["test", "nope.fung"],
        &["test", "tests/lang/test_blocks_ignored.fung", "-e"],
        &["test", "--help"],
    ];
    for a in arg_cases {
        cases.push((a.join(" "), os_args(a)));
    }
    assert_same(&b, &cases);
    // The fixtures do fail in every way the comparison is about.
    let (out, err, _) = run(&b.go, &os_args(&["test", &root]));
    let (out, err) = (String::from_utf8_lossy(&out), String::from_utf8_lossy(&err));
    for want in [
        "  exit(4) called inside a test file",
        "FAIL crates/fung-cli/tests/data/test/unretrieved.fung: <tasks>",
        "    slug(\"Hello World\") = \"hello world\"",
        "FAIL crates/fung-cli/tests/data/test/deadlock.fung: deadlocks",
        "FAIL crates/fung-cli/tests/data/test/exit_in_task.fung: a task exits",
        "ok   crates/fung-cli/tests/data/test/blocked_top.fung: again",
        "ok   crates/fung-cli/tests/data/test/top_task.fung: talks to a top-level task",
        "ok   crates/fung-cli/tests/data/test/z_clean.fung: nothing leaked",
        "FAIL crates/fung-cli/tests/data/test/uses_lib_too.fung: <load>",
        "FAIL crates/fung-cli/tests/data/test/top_exit.fung: <top level>",
    ] {
        assert!(out.contains(want), "{want}\n{out}");
    }
    assert!(!out.contains("must not run") && !out.contains("only when it is the entry"));
    assert!(err.contains("error[E0702]") && err.contains("handler broke"));
    assert_eq!(err.matches("undefined name `missing`").count(), 1);
}

/// `fung test` with no path walks the working directory.
#[test]
fn test_without_a_path_walks_the_working_directory() {
    let Some(b) = both("tools-test-cwd") else {
        return;
    };
    let dir = repo().join(TEST_DATA).join("_lib");
    let go = std::process::Command::new(&b.go)
        .arg("test")
        .current_dir(&dir)
        .output()
        .unwrap();
    let rust = std::process::Command::new(&b.rust)
        .arg("test")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(
        (String::from_utf8_lossy(&rust.stdout), rust.status.code()),
        (String::from_utf8_lossy(&go.stdout), go.status.code())
    );
    assert_eq!(rust.stderr, go.stderr);
    assert!(String::from_utf8_lossy(&go.stdout).contains("ok   good.fung"));
}

#[test]
fn test_matches_go_on_the_repository() {
    let Some(b) = both("tools-test-repo") else {
        return;
    };
    let files = corpus(&["tests/lang", "examples", "extras"]);
    let cases: Vec<_> = files
        .iter()
        .filter(|f| shared(f))
        .map(|f| test_case(f))
        .collect();
    assert!(cases.len() > 140, "{} files", cases.len());
    assert_same(&b, &cases);
    // The examples directory in one run, without the atom examples.
    let dir = scratch("tools-test-examples").join("examples");
    let mut stack = vec![repo().join("examples")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            let rel = p.strip_prefix(repo()).unwrap();
            let to = dir.join(p.strip_prefix(repo().join("examples")).unwrap());
            if p.is_dir() {
                std::fs::create_dir_all(&to).unwrap();
                stack.push(p);
            } else if !ATOM_EXAMPLES.contains(&rel.to_string_lossy().as_ref()) {
                std::fs::create_dir_all(to.parent().unwrap()).unwrap();
                std::fs::copy(&p, &to).unwrap();
            }
        }
    }
    let d = dir.to_string_lossy().into_owned();
    let (out, _, _) = run(&b.go, &os_args(&["test", &d]));
    let out = String::from_utf8_lossy(&out);
    assert!(
        out.contains("ok   ") && out.ends_with(" passed, 0 failed\n"),
        "{out}"
    );
    assert_same(&b, &[test_case(&d)]);
}

/// Rust only. Agents (stage 3b) end with their file, after its exit
/// handlers, which may still use them: one that holds its own handle and
/// one whose `f` left a task sleeping too. A deadlock or an exit asked
/// for by a spawned task skips the rest of the file, like a direct
/// `exit(n)` (08; Go runs a remaining block that makes no fung call to
/// the end and fails one that does with E0701). The next file starts
/// clean.
#[test]
fn test_ends_each_files_agents() {
    let start = std::time::Instant::now();
    let (out, err, status) = run_status(&rust_fung(), &os_args(&["test", TEST_RUST]), None);
    assert!(start.elapsed() < std::time::Duration::from_secs(20));
    let p = |f: &str| format!("{TEST_RUST}/{f}");
    let after = p("after_cancel.fung");
    let exit = p("task_exit.fung");
    let want = [
        format!("ok   {after}: before"),
        format!("FAIL {after}: deadlocks"),
        "  error[E0702]: deadlock: all tasks are waiting".to_string(),
        format!("    --> {after}:4:7"),
        "    |".to_string(),
        "  4 |   recv(ch)".to_string(),
        "    |       ^".to_string(),
        "    |".to_string(),
        "   call path (innermost first):".to_string(),
        format!("     {after}:4 test: deadlocks"),
        "  a deadlock was reported; the rest of the file was skipped".to_string(),
        format!("ok   {}: swap", p("agents.fung")),
        format!(
            "ok   {}: an agent that holds its own handle",
            p("agents.fung")
        ),
        format!("ok   {}: a background loop", p("agents.fung")),
        "1".to_string(),
        format!("FAIL {exit}: a task exits"),
        "  error[E0701]: task cancelled".to_string(),
        format!("    --> {exit}:4:8"),
        "    |".to_string(),
        "  4 |   await(t)".to_string(),
        "    |        ^".to_string(),
        "    |".to_string(),
        "   call path (innermost first):".to_string(),
        format!("     {exit}:4 test: a task exits"),
        "  exit(3) called inside a test file; the rest of the file was skipped".to_string(),
        "handler still runs".to_string(),
        format!("ok   {}: a fresh file", p("z_next.fung")),
        "5 passed, 2 failed".to_string(),
    ]
    .map(|l| l + "\n")
    .concat();
    assert_eq!(String::from_utf8_lossy(&out), want);
    assert_eq!(
        String::from_utf8_lossy(&err),
        "error[E0702]: deadlock: all tasks are waiting\n"
    );
    assert_eq!(status.code(), Some(1));
}
