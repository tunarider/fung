//! The tool commands compared with the Go implementation (stage 5):
//! `fung check`, `version`, `help` and unknown commands, byte for byte
//! (standard output, standard error, exit status), plus the path walker
//! `fung test` and `fung fmt` share (`src/paths.rs`), compared with the
//! files Go's commands visit, `fung test` on its fixtures and the
//! repository, and `fung fmt` (output, `--check`, in place, standard
//! input, arguments) on the repository and thousands of messed-up
//! variants of it, and the REPL on piped sessions (the terminal editor
//! is compared in `repl_tty.rs`).
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

/// The value of a file's `# only:` line (`go` or `rust`), if it has one,
/// by the golden runner's rules (`golden.rs` `parse`): the line starts
/// with it (after blanks), and any other value or a second such line is
/// a broken fixture (a panic).
pub fn only(src: &str) -> Option<String> {
    let mut found = None;
    for line in src.lines() {
        let Some(rest) = line.trim().strip_prefix("# only:") else {
            continue;
        };
        let v = rest.trim();
        assert!(
            v == "go" || v == "rust",
            "malformed only line {v:?} (want go or rust)"
        );
        assert!(found.is_none(), "second only line (already {found:?})");
        found = Some(v.to_string());
    }
    found
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
            if excluded(attrs) {
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

/// Whether a `<pre>` with these attributes is left out of the runnable
/// examples: `class="frag"`, `"plain"` or `"core"`. Any other class is
/// unknown here and panics, so a new kind of block is decided on, not
/// silently skipped.
fn excluded(attrs: &str) -> bool {
    let Some(i) = attrs.find("class=") else {
        return false;
    };
    let class = attrs[i + "class=".len()..]
        .strip_prefix('"')
        .and_then(|v| v.split('"').next())
        .unwrap_or_else(|| panic!("an unquoted class in <pre{attrs}>"));
    assert!(
        matches!(class, "frag" | "plain" | "core"),
        "unknown <pre> class {class:?}: add it to `excluded` or drop it"
    );
    true
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
    let dir = scratch("tools-walk");
    walker_tree(&dir);
    let abs = |p: &str| dir.join(p).to_string_lossy().into_owned();
    let roots = vec![
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
    let mut bad = Vec::new();
    for root in &roots {
        let want = go_listed(&go, &["test", root]);
        let got = ours(paths::test_files(root.as_bytes()));
        if want != got {
            bad.push(format!("test {root}:\n  go:   {want:?}\n  rust: {got:?}"));
        }
    }
    // Relative roots resolve against the working directory, so they are
    // compared through the commands, which both run from the repository
    // root (`run_status`): this process does not move (other tests run
    // in parallel with it).
    if let Some(rel) = relative(&dir) {
        for root in [format!("{rel}/tree"), format!("./{rel}/tree/sub")] {
            for cmd in [&["test"][..], &["fmt", "--check"]] {
                let args = [cmd, &[root.as_str()]].concat();
                let want = go_listed(&go, &args);
                let got = go_listed(&rust_fung(), &args);
                assert!(
                    want.as_ref().is_ok_and(|f| !f.is_empty()),
                    "{args:?}: {want:?}"
                );
                if want != got {
                    bad.push(format!("{args:?}:\n  go:   {want:?}\n  rust: {got:?}"));
                }
            }
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
    assert_eq!(
        only("x = 1 # only: go\n"),
        None,
        "not at the start of a line"
    );
    for bad in ["# only: java\n", "# only:\n", "# only: go\n# only: rust\n"] {
        assert!(
            std::panic::catch_unwind(|| only(bad)).is_err(),
            "{bad:?} is accepted"
        );
    }
    assert!(excluded(r#" class="frag""#) && excluded(r#" class="plain""#));
    assert!(excluded(r#" class="core""#));
    assert!(!excluded("") && !excluded(r#" data-args=". 3""#));
    assert!(std::panic::catch_unwind(|| excluded(r#" class="code""#)).is_err());
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
    let rest = p("rest_skipped.fung");
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
        format!("ok   {rest}: receives"),
        format!("FAIL {rest}: <rest of file>"),
        "  exit(3) called inside a test file; the rest of the file was skipped".to_string(),
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
        format!("FAIL {}: <top level>", p("top_pending.fung")),
        "  exit(3) called inside a test file; the rest of the file was skipped".to_string(),
        format!("ok   {}: a fresh file", p("z_next.fung")),
        "6 passed, 4 failed".to_string(),
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

/// Rust only. When the rest of a file is skipped after a block that
/// passed (a task it woke asked for an exit), the skip is a failure of
/// its own (08: a test run never ends silently); when the top level ends
/// with an exit pending, the top level fails. Both fixtures order the
/// exit by state (the task sends and exits on one turn of the run
/// token), so the output is fixed; the loop shows it.
#[test]
fn a_skipped_rest_is_a_failure() {
    let rest = format!("{TEST_RUST}/rest_skipped.fung");
    let top = format!("{TEST_RUST}/top_pending.fung");
    let note = "  exit(3) called inside a test file; the rest of the file was skipped\n";
    let cases = [
        (
            &rest,
            format!(
                "ok   {rest}: receives\nFAIL {rest}: <rest of file>\n{note}1 passed, 1 failed\n"
            ),
        ),
        (
            &top,
            format!("FAIL {top}: <top level>\n{note}0 passed, 1 failed\n"),
        ),
    ];
    for _ in 0..20 {
        for (file, want) in &cases {
            let (out, err, status) = run_status(&rust_fung(), &os_args(&["test", file]), None);
            assert_eq!(String::from_utf8_lossy(&out), *want);
            assert_eq!(String::from_utf8_lossy(&err), "");
            assert_eq!(status.code(), Some(1));
        }
    }
}

// ---- `fung fmt` (stage 5 task 3) ----

/// Every `.fung` file of the repository the formatter is compared on:
/// the golden tests (with `_lib` and the deliberate `err_*` syntax
/// errors, whose error output must match), the examples, the extras
/// (with `extras/rust`) and the CLI test fixtures.
fn fmt_corpus() -> Vec<String> {
    let files = corpus(&[
        "tests/lang",
        "examples",
        "extras",
        "crates/fung-cli/tests/data",
    ]);
    assert!(files.len() > 180, "corpus: {} files", files.len());
    files
}

/// Runs `bin` in `dir` (a scratch copy) with `stdin` piped in.
fn run_in(bin: &Path, dir: &Path, args: &[OsString], stdin: Option<&[u8]>) -> common::Run {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(bin)
        .args(args)
        .current_dir(dir)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(data) = stdin {
        let mut pipe = child.stdin.take().unwrap();
        let data = data.to_vec();
        std::thread::spawn(move || {
            let _ = pipe.write_all(&data);
        });
    }
    let o = child.wait_with_output().unwrap();
    (o.stdout, o.stderr, format!("{:?}", o.status))
}

/// Go `strconv.Unquote` for the escapes `format_test.go` uses.
fn go_unquote(s: &str) -> (String, &str) {
    let mut out = String::new();
    let mut cs = s.strip_prefix('"').expect("a Go string").char_indices();
    while let Some((i, c)) = cs.next() {
        match c {
            '"' => return (out, &s[i + 2..]),
            '\\' => out.push(match cs.next().unwrap().1 {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '"' => '"',
                '\\' => '\\',
                'u' => {
                    let hex: String = (0..4).map(|_| cs.next().unwrap().1).collect();
                    char::from_u32(u32::from_str_radix(&hex, 16).unwrap()).unwrap()
                }
                e => panic!("escape \\{e} in format_test.go"),
            }),
            c => out.push(c),
        }
    }
    panic!("unterminated Go string {s}")
}

/// The `"input": "want"` cases of Go's `internal/format/format_test.go`,
/// read from the file so the comparison follows the Go tests.
fn go_format_cases() -> Vec<(String, String)> {
    let text = std::fs::read_to_string(repo().join("internal/format/format_test.go")).unwrap();
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim_start();
        if !l.starts_with('"') {
            continue;
        }
        let (src, rest) = go_unquote(l);
        let Some(rest) = rest.trim_start().strip_prefix(':') else {
            continue;
        };
        let (want, rest) = go_unquote(rest.trim_start());
        assert_eq!(rest.trim(), ",", "{line}");
        out.push((src, want));
    }
    assert!(out.len() > 110, "{} Go cases", out.len());
    out
}

/// The kinds of token a messed-up variant may glue to its neighbour.
fn is_word(k: fung_front::lexer::Kind) -> bool {
    use fung_front::lexer::Kind::*;
    k.is_keyword()
        || matches!(
            k,
            Int | Float | String | RawString | Duration | Symbol | Name | Tag
        )
}

fn is_bracket_or_sep(k: fung_front::lexer::Kind) -> bool {
    use fung_front::lexer::Kind::*;
    matches!(
        k,
        LParen | RParen | LBracket | RBracket | LBrace | RBrace | Comma | Semi | Colon
    )
}

/// Deliberately messed-up variants of a source, deterministic, as
/// `(name, bytes)`. Some no longer parse; those compare error output.
/// Text edits: indentation stripped, re-indented with tabs, 3 or 4
/// spaces per level; blank lines removed or tripled; CRLF; trailing
/// blanks; no final newline; truncated at a line and at half its bytes; a byte
/// order mark (dropped). Token edits (from the lexer's spans): one space between
/// all tokens on a line, no space where a bracket, separator or a
/// word/operator pair allows it; lines joined after an opener or a
/// comma, split after them, after line-ending operators and before
/// continuation tokens; comments glued to the code or pushed away, `#x`
/// for `# x`, comment lines indented anyhow.
fn variants(src: &str) -> Vec<(&'static str, Vec<u8>)> {
    use fung_front::lexer::{Kind, lex};
    let mut out: Vec<(&'static str, Vec<u8>)> = Vec::new();
    let lines: Vec<&str> = src.split_inclusive('\n').collect();
    let map_lines = |f: &dyn Fn(&str) -> String| -> Vec<u8> {
        lines.iter().map(|l| f(l)).collect::<String>().into_bytes()
    };
    let lead = |l: &str| l.len() - l.trim_start_matches([' ', '\t']).len();
    out.push((
        "dedent",
        map_lines(&|l| l.trim_start_matches([' ', '\t']).to_string()),
    ));
    out.push((
        "tabs",
        map_lines(&|l| {
            let n = lead(l);
            "\t".repeat(n / 2) + &" ".repeat(n % 2) + &l[n..]
        }),
    ));
    out.push((
        "wide4",
        map_lines(&|l| " ".repeat(lead(l) * 2) + &l[lead(l)..]),
    ));
    out.push((
        "odd3",
        map_lines(&|l| " ".repeat(lead(l) * 3 / 2) + &l[lead(l)..]),
    ));
    out.push((
        "noblank",
        map_lines(&|l| {
            if l.trim().is_empty() {
                String::new()
            } else {
                l.to_string()
            }
        }),
    ));
    out.push(("moreblank", src.replace('\n', "\n\n\n").into_bytes()));
    out.push(("crlf", src.replace('\n', "\r\n").into_bytes()));
    out.push((
        "trailing",
        map_lines(&|l| match l.strip_suffix('\n') {
            Some(body) => format!("{body}  \t\n"),
            None => format!("{l} "),
        }),
    ));
    out.push(("nofinal", src.trim_end_matches('\n').as_bytes().to_vec()));
    out.push(("halflines", lines[..lines.len() / 2].concat().into_bytes()));
    // Cut anywhere, even inside a character: the bytes need not be UTF-8.
    out.push(("halfbytes", src.as_bytes()[..src.len() / 2].to_vec()));
    out.push(("bom", ["\u{feff}", src].concat().into_bytes()));
    // Token edits.
    let Ok((toks, comments)) = lex(fung_front::source::FileId(0), src) else {
        return out;
    };
    let toks: Vec<_> = toks.into_iter().filter(|t| t.kind != Kind::Eof).collect();
    // Rebuilds the source: `piece(i, gap)` gives what replaces the text
    // between token i-1 and token i, whether token i's own text stays,
    // and what follows it.
    let rebuild = |piece: &dyn Fn(usize, &str) -> (String, bool, &'static str)| {
        let mut s = String::new();
        let mut prev = 0;
        for (i, t) in toks.iter().enumerate() {
            let g = &src[prev..t.off];
            let (gap, keep, after) = if i == 0 {
                (g.to_string(), true, "")
            } else {
                piece(i, g)
            };
            s.push_str(&gap);
            if keep {
                s.push_str(&src[t.off..t.end]);
            }
            s.push_str(after);
            prev = t.end;
        }
        s.push_str(&src[prev..]);
        s.into_bytes()
    };
    let same = |g: &str| (g.to_string(), true, "");
    let inline = |i: usize, g: &str| {
        toks[i].kind != Kind::Newline
            && toks[i - 1].kind != Kind::Newline
            && !g.contains(['\n', '#'])
    };
    out.push((
        "spaced",
        rebuild(&|i, g| {
            if inline(i, g) {
                (" ".into(), true, "")
            } else {
                same(g)
            }
        }),
    ));
    out.push((
        "tight",
        rebuild(&|i, g| {
            let (p, n) = (toks[i - 1].kind, toks[i].kind);
            let glue = is_bracket_or_sep(p) || is_bracket_or_sep(n) || is_word(p) != is_word(n);
            if inline(i, g) && glue {
                (String::new(), true, "")
            } else {
                same(g)
            }
        }),
    ));
    let opener_or_comma = |k: Kind| {
        matches!(
            k,
            Kind::LParen | Kind::LBracket | Kind::LBrace | Kind::Comma
        )
    };
    // The newline after an opener or a comma (with no comment before it)
    // becomes one space, and the next line's indentation goes.
    out.push((
        "joined",
        rebuild(&|i, g| {
            if toks[i].kind == Kind::Newline
                && opener_or_comma(toks[i - 1].kind)
                && !g.contains('#')
            {
                (" ".into(), false, "")
            } else if i >= 2
                && toks[i - 1].kind == Kind::Newline
                && opener_or_comma(toks[i - 2].kind)
                && !src[toks[i - 2].end..toks[i - 1].off].contains('#')
            {
                (String::new(), true, "")
            } else {
                same(g)
            }
        }),
    ));
    let next_is_newline = |i: usize| toks.get(i + 1).is_none_or(|t| t.kind == Kind::Newline);
    out.push((
        "split",
        rebuild(&|i, g| {
            let after = if opener_or_comma(toks[i].kind) && !next_is_newline(i) {
                "\n"
            } else {
                ""
            };
            (g.to_string(), true, after)
        }),
    ));
    let ends_open = |k: Kind| {
        matches!(
            k,
            Kind::Plus
                | Kind::Minus
                | Kind::Star
                | Kind::Slash
                | Kind::Pipe
                | Kind::Coalesce
                | Kind::KwAnd
                | Kind::KwOr
                | Kind::Assign
                | Kind::FatArrow
                | Kind::Eq
                | Kind::Lt
                | Kind::KwThen
                | Kind::KwElse
        )
    };
    out.push((
        "opsplit",
        rebuild(&|i, g| {
            let after = if ends_open(toks[i].kind) && !next_is_newline(i) {
                "\n"
            } else {
                ""
            };
            (g.to_string(), true, after)
        }),
    ));
    let continuation = |k: Kind| {
        matches!(
            k,
            Kind::Pipe
                | Kind::Dot
                | Kind::Coalesce
                | Kind::KwAnd
                | Kind::KwOr
                | Kind::KwThen
                | Kind::KwElse
        )
    };
    out.push((
        "contsplit",
        rebuild(&|i, g| {
            if continuation(toks[i].kind) && inline(i, g) {
                ("\n".into(), true, "")
            } else {
                same(g)
            }
        }),
    ));
    // Comment edits, on lines: a trailing comment glued to its code or
    // pushed far away, `# x` written `#x`, comment lines indented by
    // their index.
    let comment_lines: std::collections::HashMap<u32, (usize, String)> = comments
        .iter()
        .enumerate()
        .map(|(n, c)| (c.pos.line, (n, c.text.clone())))
        .collect();
    let edit_comments = |how: u8| -> Vec<u8> {
        let mut s = String::new();
        for (n, l) in lines.iter().enumerate() {
            let Some((k, text)) = comment_lines.get(&(n as u32 + 1)) else {
                s.push_str(l);
                continue;
            };
            let Some(at) = l.rfind(text.as_str()) else {
                s.push_str(l);
                continue;
            };
            let (code, rest) = l.split_at(at);
            let code_part = code.trim_end_matches([' ', '\t']);
            let own_line = code_part.trim().is_empty();
            match how {
                0 if own_line => s.push_str(&format!("{}{rest}", " ".repeat(k % 7))),
                0 => s.push_str(&format!("{code_part}{rest}")),
                1 if own_line => s.push_str(&format!("\t{rest}")),
                1 => s.push_str(&format!("{code_part}{}{rest}", " ".repeat(10 + k % 5))),
                _ => s.push_str(&format!("{code}{}", rest.replacen("# ", "#", 1))),
            }
        }
        s.into_bytes()
    };
    out.push(("cglued", edit_comments(0)));
    out.push(("cpushed", edit_comments(1)));
    out.push(("cnospace", edit_comments(2)));
    out
}

/// A file name for a corpus path in a flat directory: `/` → `~` (no
/// directory, so `_lib` files are not skipped by the walker).
fn flat(path: &str) -> String {
    path.replace('/', "~")
}

/// Fills `dir/in` with every corpus file and every variant of it.
fn fmt_inputs(dir: &Path) -> usize {
    let input = dir.join("in");
    std::fs::create_dir_all(&input).unwrap();
    let mut n = 0;
    for f in fmt_corpus() {
        let src = std::fs::read_to_string(repo().join(&f)).unwrap();
        let name = flat(&f);
        std::fs::write(input.join(&name), &src).unwrap();
        n += 1;
        for (v, bytes) in variants(&src) {
            std::fs::write(
                input.join(format!("{}@{v}.fung", name.trim_end_matches(".fung"))),
                bytes,
            )
            .unwrap();
            n += 1;
        }
    }
    n
}

/// The files of a directory, by name.
fn tree(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect()
}

/// `fmt --check` and `fmt` in place over the repository's files and
/// thousands of messed-up variants of them, in one directory: the same
/// listing, errors and exit code, and the same rewritten files. Then the
/// Rust output is stable (a second `--check` lists nothing) and keeps
/// every token and comment.
#[test]
fn fmt_matches_go_on_files_and_variants() {
    let Some(b) = both("tools-fmt") else { return };
    let base = scratch("tools-fmt");
    let (gdir, rdir) = (base.join("go"), base.join("rust"));
    let n = fmt_inputs(&gdir);
    fmt_inputs(&rdir);
    assert!(n > 3000, "{n} inputs");
    let check = os_args(&["fmt", "--check", "in"]);
    let go = run_in(&b.go, &gdir, &check, None);
    let rust = run_in(&b.rust, &rdir, &check, None);
    assert_eq!(diff(&go, &rust), None, "fmt --check in");
    // The variants do exercise the formatter: many need formatting, many
    // are refused as syntax errors, and most still parse.
    let listed = String::from_utf8_lossy(&go.0).lines().count();
    let refused = String::from_utf8_lossy(&go.1)
        .matches("\n  --> in/")
        .count();
    assert!(
        listed > n / 3 && refused > 300 && refused < n / 2,
        "{listed} listed, {refused} refused of {n}"
    );
    // A byte order mark is dropped, never refused: every `@bom` variant
    // that parses is listed, and the safeguard refuses nothing.
    let err = String::from_utf8_lossy(&go.1);
    assert!(!err.contains("formatter changed the code"), "{err}");
    let out = String::from_utf8_lossy(&go.0);
    let boms = out.lines().filter(|l| l.ends_with("@bom.fung")).count();
    assert!(boms > 100, "{boms} @bom variants listed");
    let before = tree(&rdir.join("in"));
    let write = os_args(&["fmt", "in"]);
    let go = run_in(&b.go, &gdir, &write, None);
    let rust = run_in(&b.rust, &rdir, &write, None);
    assert_eq!(diff(&go, &rust), None, "fmt in");
    let (gt, rt) = (tree(&gdir.join("in")), tree(&rdir.join("in")));
    let bad: Vec<&String> = gt.keys().filter(|k| gt[*k] != rt[*k]).collect();
    assert!(
        bad.is_empty(),
        "{} files formatted differently: {:?}",
        bad.len(),
        &bad[..bad.len().min(20)]
    );
    // Idempotent: the formatted files need nothing more.
    let (out, _, _) = run_in(&b.rust, &rdir, &check, None);
    assert_eq!(String::from_utf8_lossy(&out), "", "second fmt --check");
    // The code survived: same tokens (newlines aside) and comments.
    for (name, after) in &rt {
        let src = &before[name];
        if src != after {
            assert_eq!(code_of(after), code_of(src), "{name}: tokens changed");
        }
    }
}

/// The tokens' texts (newlines aside) and the comments, trimmed.
fn code_of(src: &[u8]) -> (Vec<String>, Vec<String>) {
    use fung_front::lexer::{Kind, lex};
    let text = String::from_utf8_lossy(src);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let (toks, comments) = lex(fung_front::source::FileId(0), text).unwrap();
    (
        toks.iter()
            .filter(|t| !matches!(t.kind, Kind::Newline | Kind::Eof))
            .map(|t| text[t.off..t.end].to_string())
            .collect(),
        comments
            .iter()
            .map(|c| c.text.trim_end_matches([' ', '\t']).to_string())
            .collect(),
    )
}

/// Standard input (`-`): the inputs of Go's formatter tests, inputs only
/// Go's byte-level handling explains (not UTF-8, a byte order mark, CRLF,
/// a lone `\r`), broken inputs, the manual's examples and every corpus
/// file. `--check -` only checks standard input.
#[test]
fn fmt_matches_go_on_standard_input() {
    let Some(b) = both("tools-fmt-stdin") else {
        return;
    };
    let go_cases = go_format_cases();
    let mut inputs: Vec<(String, Vec<u8>)> = go_cases
        .iter()
        .map(|(src, _)| (format!("go case {src:?}"), src.clone().into_bytes()))
        .collect();
    for raw in [
        &b"f(\n"[..],
        b"\"open",
        b"",
        b"\n\n\n",
        b"x = \"a\xffb\" # c\xfe\nlong = 1 # d\n",
        b"match v:\n\"\xff\" => 1\n\"ab\" => 2\n;;\n",
        b"\xff",
        b"x = 1 # \xe2\x82\n",
        b"\xef\xbb\xbfx = 1\n",
        b"\xef\xbb\xbf# only a comment\n",
        b"\xef\xbb\xbf",
        b"\xef\xbb\xbf\xef\xbb\xbfx\n",
        b"\xef\xbb\xbfx = )\n",
        b"x = 1\r\ny = 2\r\n",
        b"x = 1\ry = 2\n",
        b"s = `a\r\nb`\n",
        b"x = 1   \t\n\t\ty = 2\n",
        b"fn f():\n  x\n",
        b";;\n",
        b"x = [1,\n",
        b"\tmatch v:\n\t\t1 => 2\n\t;;\n",
    ] {
        inputs.push((format!("{:?}", String::from_utf8_lossy(raw)), raw.to_vec()));
    }
    for (label, src) in manual_examples() {
        inputs.push((format!("manual {label}"), src.into_bytes()));
    }
    for f in fmt_corpus() {
        let src = std::fs::read(repo().join(&f)).unwrap();
        inputs.push((f, src));
    }
    let dir = scratch("tools-fmt-stdin");
    let runs = par_map(&inputs, |(_, src)| {
        let a = os_args(&["fmt", "-"]);
        (
            run_in(&b.go, &dir, &a, Some(src)),
            run_in(&b.rust, &dir, &a, Some(src)),
        )
    });
    let bad: Vec<String> = inputs
        .iter()
        .zip(&runs)
        .filter_map(|((label, _), (go, rust))| diff(go, rust).map(|d| format!("{label}: {d}")))
        .collect();
    assert!(
        bad.is_empty(),
        "{} of {} differ:\n{}",
        bad.len(),
        inputs.len(),
        bad.join("\n")
    );
    // Go's own expectations hold for Rust too.
    for ((src, want), (_, rust)) in go_cases.iter().zip(&runs) {
        assert_eq!(String::from_utf8_lossy(&rust.0), *want, "{src:?}");
    }
    // `--check -` (either order) names `<stdin>` and exits 1 when the
    // input needs formatting, prints nothing and exits 0 when it does
    // not; errors are as without `--check`.
    let srcs: [&[u8]; 7] = [
        b"x=1\n",
        b"x = 1\n",
        b"",
        b"x = )\n",
        b"\xef\xbb\xbfx = 1\n",
        b"\xef\xbb\xbf\xef\xbb\xbfx\n",
        b"x = \"a\xffb\"\n",
    ];
    for order in [["fmt", "--check", "-"], ["fmt", "-", "--check"]] {
        let a = os_args(&order);
        for src in srcs {
            let (go, rust) = (
                run_in(&b.go, &dir, &a, Some(src)),
                run_in(&b.rust, &dir, &a, Some(src)),
            );
            assert_eq!(diff(&go, &rust), None, "{order:?} {src:?}");
        }
    }
    let a = os_args(&["fmt", "--check", "-"]);
    let (out, _, status) = run_status(&b.rust, &a, Some(b"x=1\n"));
    assert_eq!((&out[..], status.code()), (&b"<stdin>\n"[..], Some(1)));
    let (out, _, status) = run_status(&b.rust, &a, Some(b"x = 1\n"));
    assert_eq!((&out[..], status.code()), (&b""[..], Some(0)));
}

/// Arguments, mixed paths and file errors: a file and a directory
/// together, a file named twice, `-` among paths, a missing path, an
/// unknown option, a link to a directory named `x.fung` (read error), an
/// unreadable and a read-only file needing formatting (read and write
/// errors), a name that is not UTF-8, a file with a syntax error next to
/// good ones (exit 2 wins over 1).
#[test]
fn fmt_arguments_match_go() {
    let Some(b) = both("tools-fmt-args") else {
        return;
    };
    use std::os::unix::fs::{PermissionsExt, symlink};
    let base = scratch("tools-fmt-args");
    let setup = |d: &Path| {
        std::fs::create_dir_all(d.join("t/sub/_fix")).unwrap();
        std::fs::create_dir_all(d.join("t/.hid")).unwrap();
        for (f, s) in [
            ("t/a.fung", "x=1\n"),
            ("t/ok.fung", "x = 1\n"),
            ("t/sub/b.fung", "fn f():\nx\n;;\n"),
            ("t/sub/_fix/c.fung", "y=2\n"),
            ("t/.hid/d.fung", "y=2\n"),
            ("t/bad.fung", "x = )\n"),
            ("t/notes.txt", "x=1\n"),
            ("lone.fung", "z  =  3\n"),
            ("넓은.fung", "w=4\n"),
            ("ro.fung", "r=1\n"),
            ("locked.fung", "l=1\n"),
        ] {
            std::fs::write(d.join(f), s).unwrap();
        }
        symlink("sub", d.join("t/dirlink.fung")).unwrap();
        symlink("t", d.join("tlink")).unwrap();
        std::fs::set_permissions(d.join("ro.fung"), std::fs::Permissions::from_mode(0o444))
            .unwrap();
        std::fs::set_permissions(
            d.join("locked.fung"),
            std::fs::Permissions::from_mode(0o000),
        )
        .unwrap();
        // macOS file systems refuse names that are not UTF-8.
        let _ = std::fs::write(d.join(OsString::from_vec(b"q\xfe.fung".to_vec())), "q=1\n");
    };
    let (gdir, rdir) = (base.join("go"), base.join("rust"));
    setup(&gdir);
    setup(&rdir);
    let cases: &[&[&str]] = &[
        &["fmt"],
        &["fmt", "--check"],
        &["fmt", "--check", "-x"],
        &["fmt", "-x", "lone.fung"],
        &["fmt", "--chek", "lone.fung"],
        &["fmt", "-", "lone.fung"],
        &["fmt", "nope.fung"],
        &["fmt", "lone.fung", "nope", "t"],
        &["fmt", ""],
        &["fmt", "--check", "t"],
        &["fmt", "--check", "tlink"],
        &["fmt", "--check", "tlink/"],
        &[
            "fmt",
            "--check",
            "t/sub",
            "lone.fung",
            "t/a.fung",
            "lone.fung",
        ],
        &["fmt", "--check", "t/sub/_fix/c.fung", "t/.hid"],
        &["fmt", "--check", "t/ok.fung"],
        &["fmt", "--check", "t/notes.txt"],
        &["fmt", "--check", "locked.fung", "ro.fung"],
        &["fmt", "--check", "넓은.fung", "lone.fung", "--check"],
        &["fmt", "--check", "."],
        &["fmt", "--check", "./t/../lone.fung"],
        // In place: these change the trees, compared below.
        &["fmt", "ro.fung", "locked.fung", "lone.fung"],
        &["fmt", "t", "넓은.fung"],
        &["fmt", "."],
    ];
    let mut bad = Vec::new();
    for a in cases {
        let args = os_args(a);
        let (go, rust) = (
            run_in(&b.go, &gdir, &args, None),
            run_in(&b.rust, &rdir, &args, None),
        );
        if let Some(d) = diff(&go, &rust) {
            bad.push(format!("{}: {d}", a.join(" ")));
        }
    }
    let non_utf8 = vec!["fmt".into(), OsString::from_vec(b"q\xfe.fung".to_vec())];
    let (go, rust) = (
        run_in(&b.go, &gdir, &non_utf8, None),
        run_in(&b.rust, &rdir, &non_utf8, None),
    );
    if let Some(d) = diff(&go, &rust) {
        bad.push(format!("non-UTF-8 name: {d}"));
    }
    std::fs::set_permissions(
        gdir.join("locked.fung"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    std::fs::set_permissions(
        rdir.join("locked.fung"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    for f in [
        "t/a.fung",
        "t/sub/b.fung",
        "t/sub/_fix/c.fung",
        "t/.hid/d.fung",
        "lone.fung",
        "넓은.fung",
        "ro.fung",
        "locked.fung",
    ] {
        let (g, r) = (
            std::fs::read(gdir.join(f)).unwrap(),
            std::fs::read(rdir.join(f)).unwrap(),
        );
        if g != r {
            bad.push(format!(
                "{f}: {:?} vs {:?}",
                String::from_utf8_lossy(&g),
                String::from_utf8_lossy(&r)
            ));
        }
    }
    // The file modes are kept.
    let mode = |d: &Path| {
        std::fs::metadata(d.join("ro.fung"))
            .unwrap()
            .permissions()
            .mode()
    };
    assert_eq!(mode(&gdir), mode(&rdir));
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// ---- the REPL (stage 5 task 4) ----

/// Runs the REPL (`fung` without arguments) in `dir` with `stdin` piped
/// in and `HOME` pointing at `dir`, so no real history file is touched.
fn repl_in(bin: &Path, dir: &Path, stdin: &[u8]) -> common::Run {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(bin)
        .current_dir(dir)
        .env("HOME", dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut pipe = child.stdin.take().unwrap();
    let data = stdin.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = pipe.write_all(&data);
    });
    let o = child.wait_with_output().unwrap();
    writer.join().unwrap();
    (o.stdout, o.stderr, format!("{:?}", o.status))
}

/// A scratch directory for REPL runs, with a library to `:load`.
fn repl_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fung-repl-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lib.fung"), "fn twice(x) = x * 2\nk = 10\n").unwrap();
    std::fs::write(dir.join("bad.fung"), "x = )\n").unwrap();
    std::fs::write(dir.join("m.fung"), "fn boom(x) = x / 0\n").unwrap();
    dir
}

/// Go `repl_test.go` and more, as whole sessions over a pipe: values,
/// definitions, redefinition, continued lines, errors and going on,
/// commands, `exit`, empty input, the end of input, `dbg`, tasks and
/// handlers. Compared byte for byte (both streams and the status).
/// Left out on purpose: a deadlock and a spawned task's `exit`, which
/// cancel the program. Go's session stays cancelled (Go sets a new
/// task's cancellation flag from a `context.AfterFunc` goroutine, so a
/// short input after it usually still runs to the end) and ends with
/// code 1 after a deadlock. Rust deliberately recovers from a deadlock
/// and ends the session on a spawned exit (decided 2026-10-17); see
/// `repl_recovers_after_a_deadlock` and `repl_ends_on_a_spawned_exit`.
#[test]
fn repl_matches_go_on_piped_sessions() {
    let Some(b) = both("repl") else { return };
    let cases: &[(&str, &str)] = &[
        // repl_test.go
        (
            "values",
            "x = 1\nx + 1\nfn f(n) = n * x\nf(5)\n[1, \"a\"]\nnil\n",
        ),
        (
            "redefinition and continued lines",
            "fn g() = 1\nfn f() = g()\nfn g() = 2\nf()\nfn h(a):\n  a + 1\n;;\nh(1)\n1 +\n2\nimport fs\nimport fs\nfs.exists(\"/\")\n",
        ),
        ("errors go on", "print(y)\n1 / 0\n1 + 1\n"),
        ("syntax error not at the end", "x = )\n"),
        ("failed assignment", "y = 1 / 0\ny\n"),
        ("unfinished block at the end", "fn f():\n"),
        ("commands", ":help\n:bogus\n:quit\nprint(1)\n"),
        (
            "at_exit and exit",
            "at_exit(() => print(\"bye\"))\nexit(4)\n",
        ),
        ("load", ":load lib.fung\ntwice(k)\n"),
        // more
        (
            "load errors",
            ":load\n:load nope.fung\n:load bad.fung\n:load .\n  :load   lib.fung  \nk\n",
        ),
        ("quit with code", "x = 1\n:quit now\nx\n"),
        ("exit codes", "exit(300)\n"),
        (
            "exit inside a block",
            "fn f():\n  exit(2)\n;;\nf()\nprint(1)\n",
        ),
        ("empty lines", "\n\n   \n1\n\n"),
        ("no input", ""),
        ("no final newline", "1 + 2"),
        ("unfinished at the end without newline", "if true:\n  1"),
        ("crlf", "1 + 1\r\n[1,\r\n2]\r\n"),
        ("invalid utf-8", "print(\"\u{ff}\")\n"),
        ("dbg", "x = 2\ndbg(x * 3)\nd = dbg\nd(1)\n"),
        (
            "statements print nothing",
            "type t = int\ntest \"t\":\n  1\n;;\nassert 1 == 1\nimport fs as f\nfn q() = 1\nz = 3\nassert z == 4\nz\n",
        ),
        (
            "multi-line forms",
            "x = match 2:\n  1 => \"one\"\n  _ => \"many\"\n;;\nx\nif x == \"many\":\n  \"yes\"\nelse:\n  \"no\"\n;;\n[\n  1,\n  2,\n]\n\"a\" +\n  \"b\"\n",
        ),
        (
            "excerpts come from the current input (Go)",
            "fn f(x):\n  x / 0\n;;\nif true:\n  f(1)\n;;\nf(2)\nprint(\"a\" + 1)\n",
        ),
        (
            "an imported module's error",
            "import \"./m\"\nm.boom(1)\n1\nm.boom(2)\n",
        ),
        (
            "redefining modules and names",
            "import fs\nfs = 1\nfs\nimport fs\nfs.exists(\"/\")\nfn fs() = 2\nfs()\n",
        ),
        (
            "a task waits for a later input",
            "c = channel()\nt = spawn(() => recv(c))\nsend(c, 5)\nawait(t)\n",
        ),
        // Ordered by state: the input waits for the value the task sends
        // into a buffered channel, and the task fails right after that
        // without another call (which could see the cancellation that
        // ends the session and turn the failure into E0701). With the
        // failure first, both Go and Rust sometimes lost the report.
        (
            "a failure nobody awaited",
            "c = channel(1)\nt = spawn(fn():\n  send(c, 1)\n  [1][5]\n;;)\nrecv(c)\n2\n",
        ),
        (
            "handler errors and order",
            "at_exit(() => 1 / 0)\nat_exit(() => print(\"bye\"))\nat_exit(() => exit(7))\n",
        ),
        (
            "quit cancels a sleeping task",
            "t = spawn(() => sleep(10s))\n:quit\n",
        ),
        (
            "the end of input cancels a sleeping task",
            "t = spawn(() => sleep(10s))\n",
        ),
        ("help text", ":help extra\n"),
        ("colon inside code", "x = {a: 1}\nx.a\n"),
        (
            "a value of a call",
            "print(1)\nlen([1, 2])\n(1, 2)\n'sym\n1.5\n\"s\"\n",
        ),
    ];
    let dir = repl_dir("go");
    let mut bad = Vec::new();
    for (label, input) in cases {
        let go = repl_in(&b.go, &dir, input.as_bytes());
        let rust = repl_in(&b.rust, &dir, input.as_bytes());
        if let Some(d) = diff(&go, &rust) {
            bad.push(format!("{label}: {d}"));
        }
    }
    // Raw bytes that are not UTF-8.
    let go = repl_in(&b.go, &dir, b"print(\"\xff\xfe\")\n\"\xe2\x82\"\n");
    let rust = repl_in(&b.rust, &dir, b"print(\"\xff\xfe\")\n\"\xe2\x82\"\n");
    if let Some(d) = diff(&go, &rust) {
        bad.push(format!("invalid bytes: {d}"));
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        bad.is_empty(),
        "{} cases differ:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// Rust only (agents are Rust's, stage 3b): an agent an input stores
/// stays usable in later inputs, and every background task ends with the
/// session, so the REPL exits at once. (That a temporary agent's loop
/// ends with its input is checked by counting background tasks in
/// `fung-vm`'s `repl_tests.rs`.)
#[test]
fn repl_agents_end_with_the_session() {
    let dir = repl_dir("agents");
    let start = std::time::Instant::now();
    let (out, err, status) = repl_in(
        &rust_fung(),
        &dir,
        b"a = agent(1)\nswap(a, (n) => n + 1)\nderef(a)\nderef(agent(5))\nswap(a, (n) => n * 10)\nat_exit(() => print(deref(a)))\n",
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(start.elapsed() < std::time::Duration::from_secs(20));
    assert_eq!(String::from_utf8_lossy(&err), "");
    assert_eq!(
        String::from_utf8_lossy(&out),
        "> > 2\n> 2\n> 5\n> 20\n> nil\n> \n20\n"
    );
    assert_eq!(status, "ExitStatus(unix_wait_status(0))");
}

/// Runs a Rust-only REPL session: (stdout, stderr, exit code).
fn rust_repl(tag: &str, input: &str) -> (String, String, Option<i32>) {
    let dir = repl_dir(tag);
    let mut child = std::process::Command::new(rust_fung())
        .current_dir(&dir)
        .env("HOME", &dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut pipe = child.stdin.take().unwrap();
    let data = input.as_bytes().to_vec();
    let writer = std::thread::spawn(move || {
        use std::io::Write;
        let _ = pipe.write_all(&data);
    });
    let o = child.wait_with_output().unwrap();
    writer.join().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        o.status.code(),
    )
}

/// Rust only (decided 2026-10-17, a deliberate divergence from Go): a
/// deadlock fails only the input it happened in, with Go's report; the
/// session goes on on a fresh run root, the tasks earlier inputs left
/// blocked are cancelled, globals and agents stay, and the recovered
/// deadlock does not set the exit code.
#[test]
fn repl_recovers_after_a_deadlock() {
    let (out, err, code) = rust_repl(
        "deadlock",
        "a = agent(1)\nc = channel()\nt = spawn(() => recv(c))\nrecv(channel())\nprint(\"after\")\nawait(t)\nu = spawn(() => 40 + 2)\nawait(u)\nswap(a, (n) => n + 1)\n",
    );
    assert_eq!(
        out,
        "> > > > > after\nnil\n> Err({kind: 'cancelled})\n> > 42\n> 2\n> \n"
    );
    assert_eq!(
        err,
        "error[E0702]: deadlock: all tasks are waiting\nerror[E0702]: deadlock: all tasks are waiting\n  --> <repl>:1:5\n  |\n1 | recv(channel())\n  |     ^\n  |\n call path (innermost first):\n   <repl>:1   <entry>\n"
    );
    assert_eq!(code, Some(0));
}

/// Rust only (decided 2026-10-17): a spawned task's `exit(n)` ends the
/// session like `exit(n)` typed at the prompt: the input it cut short
/// reports nothing, later lines are not run, the handlers run, and the
/// code is n.
#[test]
fn repl_ends_on_a_spawned_exit() {
    let (out, err, code) = rust_repl(
        "spawned-exit",
        "at_exit(() => print(\"bye\"))\nc = channel()\nt = spawn(fn():\n  recv(c)\n  exit(3)\n;;)\n[send(c, 1), await(t)]\nprint(\"never\")\n",
    );
    assert_eq!(out, "> nil\n> > . . . > bye\n");
    assert_eq!(err, "");
    assert_eq!(code, Some(3));
}

/// Rust only: a spawned task's `exit(n)` that comes while a line is read
/// ends the session before that line runs, a `:` command included (one
/// that runs no fung code, which the exit would have cut short). The
/// task prints `going` and asks for the exit on one turn of the run token,
/// so once `going` is out, the REPL's check after the read waits for that
/// turn and sees the exit. (The task's sleep only makes the exit come
/// after the check before the read, so the check after it is the one
/// exercised; if it came earlier, the result would be the same.)
#[test]
fn repl_checks_a_spawned_exit_before_a_command() {
    use std::io::{Read, Write};
    let dir = repl_dir("exit-before-command");
    let mut child = std::process::Command::new(rust_fung())
        .current_dir(&dir)
        .env("HOME", &dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    stdin
        .write_all(b"t = spawn(fn():\n  sleep(100ms)\n  print(\"going\")\n  exit(3)\n;;)\n")
        .unwrap();
    let mut out = Vec::new();
    let mut buf = [0u8; 256];
    while !String::from_utf8_lossy(&out).contains("going\n") {
        let n = stdout.read(&mut buf).unwrap();
        assert!(n > 0, "{}", String::from_utf8_lossy(&out));
        out.extend_from_slice(&buf[..n]);
    }
    // A command that does something before any fung code runs: the
    // help text, and a file that fails to compile.
    stdin.write_all(b":help\n:load bad.fung\n").unwrap();
    drop(stdin);
    stdout.read_to_end(&mut out).unwrap();
    let o = child.wait_with_output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(String::from_utf8_lossy(&out), "> . . . . > going\n");
    assert_eq!(String::from_utf8_lossy(&o.stderr), "");
    assert_eq!(o.status.code(), Some(3));
}
