//! The tool commands against `expected/{check,cli,walk,test,fmt,repl}/`:
//! `fung check`, `version`, `help` and unknown commands, byte for byte
//! (standard output, standard error, exit status), the files the path
//! walker `fung test` and `fung fmt` share (`src/paths.rs`) visits,
//! `fung test` on its fixtures and the repository, `fung fmt` (output,
//! `--check`, in place, standard input, arguments) on the repository and
//! thousands of messed-up variants of it, and the REPL on piped sessions
//! (the terminal editor is tested in `repl_tty.rs`).
//!
//! What is covered, and how:
//! - `check` on every `.fung` file of `tests/lang` (with `_lib`),
//!   `examples` and `extras` (with `extras/rust`), the warning fixtures in
//!   `tests/data/check`, `-e` programs, and argument errors.
//! - A `# only:` file and a Rust-only example
//!   (`common::RUST_ONLY_EXAMPLES`: agents and the module `conc`) have no
//!   fixed `check` or `test` output here: a `# only: rust` file and such
//!   an example must check with no errors (a `# only: go` file is left
//!   out).
//! - The manual: its runnable examples pass `check`, `fmt --check` and a
//!   run (`manual_examples_check_format_and_run`); their formatting has a
//!   fixed output like any source.
//! - File names that are not UTF-8 are tested where the file system
//!   allows them, by comparison with the same file under a UTF-8 name
//!   (`*_non_utf8_*`), so the fixed outputs are the same on every system.

mod common;
#[allow(dead_code)]
#[path = "../src/paths.rs"]
mod paths;

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use common::expected::{self, Entry};
use common::{RUST_ONLY_EXAMPLES, repo, run, run_status, rust_fung};

pub use common::par_map;

pub fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

/// The largest output kept as text (larger ones are kept as a hash).
const LIMIT: usize = 64 * 1024;

/// `(from, to)` replacements for paths that change from run to run.
type Subs = Vec<(String, String)>;

/// The repository's path as `$REPO`, then `extra`.
fn subs(extra: &[(&Path, &str)]) -> Subs {
    let mut out: Subs = extra
        .iter()
        .map(|(p, to)| (p.to_string_lossy().into_owned(), to.to_string()))
        .collect();
    // Longer paths first: a scratch directory lies below the repository.
    out.push((repo().to_string_lossy().into_owned(), "$REPO".into()));
    out
}

fn sub_refs(subs: &Subs) -> Vec<(&str, &str)> {
    subs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect()
}

/// A label with the replacements applied.
fn label_with(label: &str, subs: &Subs) -> String {
    String::from_utf8_lossy(&expected::replace(label.as_bytes(), &sub_refs(subs))).into_owned()
}

/// Runs every case with `bin` (in parallel, from the repository root).
pub fn entries(bin: &Path, cases: &[(String, Vec<OsString>)], subs: &Subs) -> Vec<Entry> {
    par_map(cases, |(label, args)| {
        Entry::run(label_with(label, subs), &run(bin, args), &sub_refs(subs))
    })
}

/// Checks `cases` against the file `name` (see `expected::verify`). A
/// recording (stage 6b task 1 only; task 5 removes it) runs them with the
/// reference implementation built as `tag` too.
fn verify_cases(
    name: &str,
    header: &str,
    tag: &str,
    cases: &[(String, Vec<OsString>)],
    subs: &Subs,
) {
    let got = entries(&rust_fung(), cases, subs);
    let reference = expected::recording().then(|| {
        let bin = expected::reference_fung(tag);
        let r = entries(&bin, cases, subs);
        let _ = std::fs::remove_file(&bin);
        r
    });
    verify_entries(name, header, &got, reference.as_deref());
}

/// `expected::verify` with this file's limit and the update command.
fn verify_entries(name: &str, header: &str, got: &[Entry], reference: Option<&[Entry]>) {
    verify_entries_with(name, header, LIMIT, got, reference);
}

fn verify_entries_with(
    name: &str,
    header: &str,
    limit: usize,
    got: &[Entry],
    reference: Option<&[Entry]>,
) {
    expected::verify(
        name,
        &format!(
            "{header} (crates/fung-cli/tests/tools_expected.rs).\n\
             $REPO stands for the repository, $DIR for the test's scratch directory.\n\
             Update: FUNG_EXPECTED=bless cargo test -p fung-cli --test tools_expected"
        ),
        limit,
        got,
        reference,
    );
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

/// Whether both implementations are meant to treat a repository file
/// alike: not a `# only:` file, not a Rust-only example.
pub fn shared(path: &str) -> bool {
    let src = std::fs::read_to_string(repo().join(path)).unwrap();
    !RUST_ONLY_EXAMPLES.contains(&path) && only(&src).is_none()
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

/// How many cases each test has (a case must not go missing unnoticed;
/// change these together with the cases).
const CHECK_REPOSITORY_CASES: usize = 158;
const CHECK_WARNING_CASES: usize = 9;
const CHECK_ARGUMENT_CASES: usize = 25;
const HELP_CASES: usize = 8;
const VERSION_CASES: usize = 2;
const WALK_CASES: usize = 29;
const WALK_UNREADABLE_CASES: usize = 2;
const TEST_FIXTURE_CASES: usize = 29;
const TEST_REPOSITORY_CASES: usize = 159;
const FMT_INPUTS: usize = 4536;
const FMT_STDIN_CASES: usize = 414;
const FMT_FORMAT_CASES: usize = 124;
const FMT_ARGUMENT_CASES: usize = 33;
const REPL_CASES: usize = 34;

#[test]
fn check_matches_the_expected_output_on_the_repository() {
    let files = corpus(&["tests/lang", "examples", "extras"]);
    assert!(files.len() > 150, "corpus: {} files", files.len());
    assert!(files.iter().any(|f| f.starts_with("extras/rust/")));
    assert!(files.iter().any(|f| f.starts_with("tests/lang/_lib/")));
    let (same, own): (Vec<&String>, Vec<&String>) = files.iter().partition(|f| shared(f));
    let cases: Vec<_> = same.iter().map(|f| check(f)).collect();
    assert_eq!(cases.len(), CHECK_REPOSITORY_CASES, "cases");
    verify_cases(
        "check/repository.txt",
        "`fung check` on the repository's .fung files",
        "tools-check",
        &cases,
        &subs(&[]),
    );
    // A `# only: rust` file compiles (its run is golden-tested), and so
    // does a Rust-only example (its tests run in `test_runs_the_examples`).
    let mut rust_only = 0;
    for f in own {
        let src = std::fs::read_to_string(repo().join(f)).unwrap();
        if only(&src).as_deref() == Some("rust") || RUST_ONLY_EXAMPLES.contains(&f.as_str()) {
            rust_only += 1;
            let (_, err, status) = run_status(&rust_fung(), &os_args(&["check", f]), None);
            assert!(
                status.success() && !String::from_utf8_lossy(&err).contains("error["),
                "{f}: {status}\n{}",
                String::from_utf8_lossy(&err)
            );
        }
    }
    assert!(
        rust_only >= RUST_ONLY_EXAMPLES.len(),
        "{rust_only} Rust-only files"
    );
}

#[test]
fn check_matches_the_expected_output_on_warning_fixtures() {
    let files = corpus(&["crates/fung-cli/tests/data/check"]);
    let cases: Vec<_> = files.iter().map(|f| check(f)).collect();
    assert_eq!(cases.len(), CHECK_WARNING_CASES, "{files:?}");
    verify_cases(
        "check/warnings.txt",
        "`fung check` on the warning fixtures",
        "tools-warn",
        &cases,
        &subs(&[]),
    );
    // The fixtures do warn: two silent runs would prove nothing.
    let (_, err, _) = run(
        &rust_fung(),
        &os_args(&["check", "crates/fung-cli/tests/data/check/unused_vars.fung"]),
    );
    assert!(String::from_utf8_lossy(&err).contains("warning[W0102]"));
}

/// Runs `bin` with `args` in `dir`, killing it after `limit`: the
/// standard output and error, and the exit code (`None`: killed).
fn run_limited(
    bin: &Path,
    dir: &Path,
    args: &[OsString],
    limit: std::time::Duration,
) -> (String, String, Option<i32>) {
    use std::process::{Command, Stdio};
    let mut child = Command::new(bin)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    let mut killed = false;
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > limit {
            let _ = child.kill();
            killed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let o = child.wait_with_output().unwrap();
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        if killed { None } else { o.status.code() },
    )
}

/// The arguments a manual example runs with: its `<pre data-args="...">`
/// split at blanks.
fn manual_example_args(label: &str) -> Vec<String> {
    let (page, n) = label.split_once(':').unwrap();
    let n: usize = n.parse().unwrap();
    let html = std::fs::read_to_string(repo().join(format!("docs/{page}.html"))).unwrap();
    let mut seen = 0;
    let mut rest = html.as_str();
    while let Some(i) = rest.find("<pre") {
        rest = &rest[i..];
        let open_end = rest.find('>').unwrap();
        let attrs = &rest[4..open_end];
        rest = &rest[open_end..];
        if excluded(attrs) {
            continue;
        }
        seen += 1;
        if seen == n {
            return attrs
                .find("data-args=\"")
                .map(|i| {
                    let v = &attrs[i + "data-args=\"".len()..];
                    v[..v.find('"').unwrap()]
                        .split_whitespace()
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
        }
    }
    panic!("no example {label}")
}

/// Rust only (stage 6a: the manual describes the Rust implementation).
/// Every runnable manual example passes `fung check` with no errors, is
/// formatted (`fmt --check`), and runs to the end with exit code 0, in a
/// scratch directory, with its `data-args` as arguments, and within a few
/// seconds.
#[test]
fn manual_examples_check_format_and_run() {
    let rust = rust_fung();
    let dir = scratch("tools-manual");
    let examples = manual_examples();
    assert!(examples.len() > 40, "{} examples", examples.len());
    let results = par_map(&examples, |(label, src)| {
        let name = format!("{}.fung", label.replace(':', "_"));
        // A `<pre>` ends without the final newline a file has.
        std::fs::write(dir.join(&name), format!("{src}\n")).unwrap();
        let limit = std::time::Duration::from_secs(20);
        let mut bad = Vec::new();
        let (_, err, code) = run_limited(&rust, &dir, &os_args(&["check", &name]), limit);
        if code != Some(0) || err.contains("error[") {
            bad.push(format!("{label}: check: {code:?}\n{err}"));
        }
        let (_, err, code) = run_limited(&rust, &dir, &os_args(&["fmt", "--check", &name]), limit);
        if code != Some(0) {
            bad.push(format!("{label}: fmt --check: {code:?}\n{err}"));
        }
        let mut args = vec!["run".to_string(), name.clone()];
        args.extend(manual_example_args(label));
        let a: Vec<OsString> = args.iter().map(OsString::from).collect();
        let (_, err, code) = run_limited(&rust, &dir, &a, limit);
        if code != Some(0) {
            bad.push(format!("{label}: run: {code:?}\n{err}"));
        }
        bad
    });
    let bad: Vec<String> = results.into_iter().flatten().collect();
    assert!(bad.is_empty(), "{}", bad.join("\n"));
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
fn check_matches_the_expected_output_on_inline_code_and_arguments() {
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
    // A file name with a character the excerpt must keep as is.
    let wide = dir.join("넓은.fung");
    std::fs::write(&wide, "fn f():\n  x = 1\n  2\n;;\n").unwrap();
    cases.push((
        "check wide name".into(),
        vec!["check".into(), wide.into_os_string()],
    ));
    assert_eq!(cases.len(), CHECK_ARGUMENT_CASES, "cases");
    verify_cases(
        "check/arguments.txt",
        "`fung check` on -e programs and argument errors",
        "tools-args",
        &cases,
        &subs(&[(&dir, "$DIR")]),
    );
}

/// A file whose name is not UTF-8 is reported under its name as it is:
/// the output is the one for the same file under a UTF-8 name, with the
/// name's bytes swapped. Only where the file system allows such names
/// (macOS refuses them, EILSEQ); the same check with a UTF-8 stand-in
/// name runs everywhere, so the test itself is exercised on every system.
#[test]
fn check_reports_a_non_utf8_file_name_as_it_is() {
    check_reports_a_file_name_as_it_is("tools-non-utf8-check", b"w\xfe.fung");
    check_reports_a_file_name_as_it_is("tools-stand-in-check", "wé.fung".as_bytes());
}

/// See `check_reports_a_non_utf8_file_name_as_it_is`; returns early when
/// the file system refuses `name`.
fn check_reports_a_file_name_as_it_is(tag: &str, name: &[u8]) {
    let dir = scratch(tag);
    let src = "fn f():\n  x = 1\n  2\n;;\n";
    let odd = dir.join(OsString::from_vec(name.to_vec()));
    if std::fs::write(&odd, src).is_err() {
        return;
    }
    let plain = dir.join("w_.fung");
    std::fs::write(&plain, src).unwrap();
    let got = run(
        &rust_fung(),
        &[OsString::from("check"), odd.into_os_string()],
    );
    let want = run(
        &rust_fung(),
        &[OsString::from("check"), plain.into_os_string()],
    );
    let swap = |b: &[u8]| {
        let needle = b"w_.fung";
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i..].starts_with(needle) {
                out.extend_from_slice(name);
                i += needle.len();
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        out
    };
    assert!(
        String::from_utf8_lossy(&want.1).contains("w_.fung"),
        "{want:?}"
    );
    assert!(contains(&got.1, name), "{got:?}");
    assert_eq!(
        (got.0, got.1, got.2),
        (swap(&want.0), swap(&want.1), want.2)
    );
}

/// Whether `needle` occurs in `hay`.
fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}
#[test]
fn help_and_unknown_commands_match_the_expected_output() {
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
    assert_eq!(cases.len(), HELP_CASES, "cases");
    verify_cases(
        "cli/help.txt",
        "`fung help` and unknown commands",
        "tools-help",
        &cases,
        &subs(&[]),
    );
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
fn version_matches_the_expected_output() {
    let v = rust_version();
    let want = option_env!("FUNG_BUILD_VERSION")
        .map(|s| s.strip_prefix('v').unwrap_or(s))
        .filter(|s| !s.is_empty() && *s != "(devel)")
        .unwrap_or("0.0.0-dev");
    assert_eq!(v, want);
    let cases = vec![
        ("version".to_string(), os_args(&["version"])),
        ("version extra".to_string(), os_args(&["version", "extra"])),
    ];
    assert_eq!(cases.len(), VERSION_CASES, "cases");
    // The version itself is `$VERSION`: it depends on the build.
    let mut s = subs(&[]);
    s.insert(0, (v.clone(), "$VERSION".into()));
    let got = entries(&rust_fung(), &cases, &s);
    // Recording (stage 6b task 1 only; task 5 removes it): the reference
    // implementation linked to the same version.
    let reference = expected::recording().then(|| {
        let bin = expected::reference_fung_with(
            "tools-version",
            &[&format!("-ldflags=-X main.linkedVersion={v}")],
        );
        let r = entries(&bin, &cases, &s);
        let _ = std::fs::remove_file(&bin);
        r
    });
    verify_entries(
        "cli/version.txt",
        "`fung version` ($VERSION: the build's version)",
        &got,
        reference.as_deref(),
    );
}

// ---- the path walker (`src/paths.rs`) ----

/// A tree with every case the walkers treat specially. Every `.fung`
/// file has one passing test and is not formatted (`x=1`), so
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

/// The files a `fung test` or `fung fmt --check` run visited, read from
/// what it printed, or its path error (`error: ...` with the usage exit
/// code 3). Every fixture file is named: `fung test` prints `ok   PATH: t`
/// (or `FAIL PATH: <load>`), `fmt --check` prints the path; either says
/// `error: cannot read PATH: ...` when the file is a directory and shows
/// `--> PATH:` for a file that is not fung. The files are handled in
/// sorted order, so the names are sorted back together from both streams.
fn listed(bin: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    let (out, err, status) = run_status(bin, &os_args(args), None);
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

/// A listing as an entry: the files, one per line, or the error.
fn listing(label: &str, r: &Result<Vec<String>, String>, subs: &Subs) -> Entry {
    let s = sub_refs(subs);
    let e = Entry::new(label_with(label, subs));
    match r {
        Ok(files) => e.field(
            "files",
            expected::replace(
                files
                    .iter()
                    .map(|f| format!("{f}\n"))
                    .collect::<String>()
                    .as_bytes(),
                &s,
            ),
        ),
        Err(err) => e.field(
            "error",
            expected::replace(format!("{err}\n").as_bytes(), &s),
        ),
    }
}

/// What the walker visits from each root, as the commands list it: the
/// walker functions against the expected listings, and from relative
/// roots the commands themselves (relative roots resolve against the
/// working directory, and this process does not move: other tests run in
/// parallel with it).
#[test]
fn walker_matches_the_expected_listings() {
    let dir = scratch("tools-walk");
    walker_tree(&dir);
    let abs = |p: &str| dir.join(p).to_string_lossy().into_owned();
    let rel = relative(&dir).expect("the scratch directory lies below the repository");
    // The absolute scratch path first: it contains the relative one.
    let s = subs(&[(&dir, "$DIR"), (Path::new(&rel), "$REL")]);
    let rust = rust_fung();
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
    let mut groups: Vec<Vec<String>> = roots.iter().map(|r| vec![r.clone()]).collect();
    groups.push(vec![
        abs("tree/sub"),
        abs("lone.fung"),
        abs("tree/a.fung"),
        abs("_under"),
    ]);
    groups.push(vec![abs("lone.fung"), abs("lone.fung")]);
    groups.push(vec![abs("tree/sub"), abs("nope"), abs("lone.fung")]);
    let mut commands: Vec<Vec<String>> = Vec::new();
    for root in [format!("{rel}/tree"), format!("./{rel}/tree/sub")] {
        for cmd in [&["test"][..], &["fmt", "--check"]] {
            let mut args: Vec<String> = cmd.iter().map(|c| c.to_string()).collect();
            args.push(root.clone());
            commands.push(args);
        }
    }
    // Each case: its label and how a binary lists it (`None`: the walker
    // functions of this build).
    let collect = |bin: Option<&Path>| -> Vec<Entry> {
        let mut out = Vec::new();
        for root in &roots {
            let r = match bin {
                Some(b) => listed(b, &["test", root]),
                None => ours(paths::test_files(root.as_bytes())),
            };
            out.push(listing(&format!("test {root}"), &r, &s));
        }
        for args in &commands {
            let a: Vec<&str> = args.iter().map(String::as_str).collect();
            let r = listed(bin.unwrap_or(&rust), &a);
            assert!(r.as_ref().is_ok_and(|f| !f.is_empty()), "{args:?}: {r:?}");
            out.push(listing(&args.join(" "), &r, &s));
        }
        for g in &groups {
            let r = match bin {
                Some(b) => {
                    let mut args = vec!["fmt", "--check"];
                    args.extend(g.iter().map(String::as_str));
                    listed(b, &args)
                }
                None => {
                    let raw: Vec<Vec<u8>> = g.iter().map(|p| p.as_bytes().to_vec()).collect();
                    ours(paths::fung_files(&raw))
                }
            };
            out.push(listing(&format!("fmt --check {}", g.join(" ")), &r, &s));
        }
        out
    };
    let got = collect(None);
    assert_eq!(got.len(), WALK_CASES, "cases");
    // Recording (stage 6b task 1 only; task 5 removes it).
    let reference = expected::recording().then(|| {
        let bin = expected::reference_fung("tools-walk");
        let r = collect(Some(&bin));
        let _ = std::fs::remove_file(&bin);
        r
    });
    verify_entries(
        "walk/listings.txt",
        "The files `fung test` and `fung fmt --check` visit from each root ($REL: $DIR relative to $REPO)",
        &got,
        reference.as_deref(),
    );
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
fn walker_reports_unreadable_directories() {
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
    let s = subs(&[(&dir, "$DIR")]);
    let got = vec![
        listing(
            &format!("test {root}"),
            &ours(paths::test_files(root.as_bytes())),
            &s,
        ),
        listing(
            &format!("fmt --check {root}"),
            &ours(paths::fung_files(&[root.clone().into_bytes()])),
            &s,
        ),
    ];
    assert_eq!(got.len(), WALK_UNREADABLE_CASES, "cases");
    // Recording (stage 6b task 1 only; task 5 removes it).
    let reference = expected::recording().then(|| {
        let bin = expected::reference_fung("tools-walk-perm");
        let r = vec![
            listing(&format!("test {root}"), &listed(&bin, &["test", &root]), &s),
            listing(
                &format!("fmt --check {root}"),
                &listed(&bin, &["fmt", "--check", &root]),
                &s,
            ),
        ];
        let _ = std::fs::remove_file(&bin);
        r
    });
    std::fs::set_permissions(dir.join("t/locked"), std::fs::Permissions::from_mode(0o755)).unwrap();
    // Root reads the locked directory anyway: the expected listings are
    // a normal user's.
    if !readable {
        verify_entries(
            "walk/unreadable.txt",
            "The walkers on a tree with a directory they cannot read",
            &got,
            reference.as_deref(),
        );
        assert!(got[0].get("error").is_some(), "{got:?}");
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
fn test_matches_the_expected_output_on_fixtures() {
    let files = corpus(&[TEST_DATA]);
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
    assert_eq!(cases.len(), TEST_FIXTURE_CASES, "{files:?}");
    verify_cases(
        "test/fixtures.txt",
        "`fung test` on its fixtures",
        "tools-test",
        &cases,
        &subs(&[]),
    );
    // The fixtures do fail in every way the outputs are about.
    let (out, err, _) = run(&rust_fung(), &os_args(&["test", &root]));
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
    let dir = repo().join(TEST_DATA).join("_lib");
    let entry = |bin: &Path| {
        let o = std::process::Command::new(bin)
            .arg("test")
            .current_dir(&dir)
            .output()
            .unwrap();
        Entry::run(
            format!("test (in {TEST_DATA}/_lib)"),
            &(o.stdout, o.stderr, format!("{:?}", o.status)),
            &sub_refs(&subs(&[])),
        )
    };
    let got = vec![entry(&rust_fung())];
    // Recording (stage 6b task 1 only; task 5 removes it).
    let reference = expected::recording().then(|| {
        let bin = expected::reference_fung("tools-test-cwd");
        let r = vec![entry(&bin)];
        let _ = std::fs::remove_file(&bin);
        r
    });
    verify_entries(
        "test/working_directory.txt",
        "`fung test` with no path",
        &got,
        reference.as_deref(),
    );
    assert!(String::from_utf8_lossy(got[0].get("stdout").unwrap()).contains("ok   good.fung"));
}

#[test]
fn test_matches_the_expected_output_on_the_repository() {
    let files = corpus(&["tests/lang", "examples", "extras"]);
    let mut cases: Vec<_> = files
        .iter()
        .filter(|f| shared(f))
        .map(|f| test_case(f))
        .collect();
    // The examples directory in one run, without the Rust-only examples.
    let base = scratch("tools-test-examples");
    let dir = base.join("examples");
    let mut stack = vec![repo().join("examples")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            let rel = p.strip_prefix(repo()).unwrap();
            let to = dir.join(p.strip_prefix(repo().join("examples")).unwrap());
            if p.is_dir() {
                std::fs::create_dir_all(&to).unwrap();
                stack.push(p);
            } else if !RUST_ONLY_EXAMPLES.contains(&rel.to_string_lossy().as_ref()) {
                std::fs::create_dir_all(to.parent().unwrap()).unwrap();
                std::fs::copy(&p, &to).unwrap();
            }
        }
    }
    let d = dir.to_string_lossy().into_owned();
    cases.push(test_case(&d));
    assert_eq!(cases.len(), TEST_REPOSITORY_CASES, "cases");
    verify_cases(
        "test/repository.txt",
        "`fung test` on the repository's .fung files and the examples directory",
        "tools-test-repo",
        &cases,
        &subs(&[(&base, "$DIR")]),
    );
    let (out, _, _) = run(&rust_fung(), &os_args(&["test", &d]));
    let out = String::from_utf8_lossy(&out);
    assert!(
        out.contains("ok   ") && out.ends_with(" passed, 0 failed\n"),
        "{out}"
    );
}

/// Rust only (stage 6a). The examples directory as it is, the Rust-only
/// examples included: every `test` block passes, nothing on standard
/// error.
#[test]
fn test_runs_the_examples() {
    let (out, err, status) = run_status(&rust_fung(), &os_args(&["test", "examples"]), None);
    let (out, err) = (String::from_utf8_lossy(&out), String::from_utf8_lossy(&err));
    assert!(
        status.success() && out.ends_with("12 passed, 0 failed\n") && err.is_empty(),
        "{status}\n{out}\n{err}"
    );
    assert!(out.contains("ok   examples/api.fung: "), "{out}");
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

/// Recording only (stage 6b task 1; task 5 removes it with the reference
/// implementation's sources): `strconv.Unquote` for the escapes of its
/// `format_test.go`.
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

/// Recording only (stage 6b task 1; task 5 removes it): the
/// `"input": "want"` cases of `internal/format/format_test.go`.
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
fn tree(dir: &Path) -> Tree {
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

/// The formatted tree as one entry: per file, `unchanged` or its length
/// and hash (the formatted texts themselves would repeat the corpus).
fn tree_entry(label: &str, before: &Tree, after: &Tree) -> Entry {
    let mut text = String::new();
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "fmt added or removed files"
    );
    for (name, a) in after {
        if before[name] == *a {
            text.push_str(&format!("{name} unchanged\n"));
        } else {
            text.push_str(&format!(
                "{name} {} {:016x}\n",
                a.len(),
                expected::fnv1a64(a)
            ));
        }
    }
    Entry::new(label).field("files", text)
}

type Tree = std::collections::BTreeMap<String, Vec<u8>>;

/// `fmt --check` and `fmt` in place over the repository's files and
/// thousands of messed-up variants of them, in one directory: the
/// listing, errors and exit code, and the rewritten files. Then the
/// output is stable (a second `--check` lists nothing) and keeps every
/// token and comment.
#[test]
fn fmt_matches_the_expected_output_on_files_and_variants() {
    let base = scratch("tools-fmt");
    // `bin` formats a fresh copy of the inputs in `dir`: the entries, and
    // the inputs and the formatted files.
    let format = |bin: &Path, dir: &Path| -> (Vec<Entry>, Tree, Tree) {
        let n = fmt_inputs(dir);
        assert_eq!(n, FMT_INPUTS, "inputs");
        let before = tree(&dir.join("in"));
        let check = run_in(bin, dir, &os_args(&["fmt", "--check", "in"]), None);
        let write = run_in(bin, dir, &os_args(&["fmt", "in"]), None);
        let after = tree(&dir.join("in"));
        let owned = subs(&[(dir, "$DIR")]);
        let s = sub_refs(&owned);
        let entries = vec![
            Entry::run("fmt --check in", &check, &s),
            Entry::run("fmt in", &write, &s),
            tree_entry("the formatted files", &before, &after),
        ];
        (entries, before, after)
    };
    let rdir = base.join("rust");
    let (got, before, after) = format(&rust_fung(), &rdir);
    // Recording (stage 6b task 1 only; task 5 removes it).
    let reference = expected::recording().then(|| {
        let bin = expected::reference_fung("tools-fmt");
        let (r, _, _) = format(&bin, &base.join("reference"));
        let _ = std::fs::remove_file(&bin);
        r
    });
    // Kept as text: the listings and the tree are one line per file.
    verify_entries_with(
        "fmt/variants.txt",
        "`fung fmt --check` and `fung fmt` on the repository's files and messed-up variants of them;\n\
         the formatted files: per file `unchanged`, or its length and FNV-1a hash",
        1024 * 1024,
        &got,
        reference.as_deref(),
    );
    // The variants do exercise the formatter: many need formatting, many
    // are refused as syntax errors, and most still parse.
    let n = FMT_INPUTS;
    let out = String::from_utf8_lossy(got[0].get("stdout").unwrap()).into_owned();
    let err = String::from_utf8_lossy(got[0].get("stderr").unwrap()).into_owned();
    let listed = out.lines().count();
    let refused = err.matches("\n  --> in/").count();
    assert!(
        listed > n / 3 && refused > 300 && refused < n / 2,
        "{listed} listed, {refused} refused of {n}"
    );
    // A byte order mark is dropped, never refused: every `@bom` variant
    // that parses is listed, and the safeguard refuses nothing.
    assert!(!err.contains("formatter changed the code"), "{err}");
    let boms = out.lines().filter(|l| l.ends_with("@bom.fung")).count();
    assert!(boms > 100, "{boms} @bom variants listed");
    // Idempotent: the formatted files need nothing more.
    let check = os_args(&["fmt", "--check", "in"]);
    let (out, _, _) = run_in(&rust_fung(), &rdir, &check, None);
    assert_eq!(String::from_utf8_lossy(&out), "", "second fmt --check");
    // The code survived: same tokens (newlines aside) and comments.
    for (name, after) in &after {
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

/// Formatter cases with an input and its formatted text: in a recording
/// (stage 6b task 1 only; task 5 removes it) the reference
/// implementation's own formatter tests, otherwise the `stdin` fields of
/// `fmt/stdin.txt`, whose `stdout` fields are those formatted texts.
fn format_cases() -> Vec<String> {
    if expected::recording() {
        return go_format_cases().into_iter().map(|(src, _)| src).collect();
    }
    expected::load(FMT_STDIN)
        .into_iter()
        .filter(|(label, _)| label.starts_with("format case "))
        .map(|(label, fields)| {
            let (_, v) = fields
                .iter()
                .find(|(n, _)| n == "stdin")
                .unwrap_or_else(|| panic!("{label}: no stdin"));
            match v {
                expected::Stored::Bytes(b) => String::from_utf8(b.clone()).unwrap(),
                expected::Stored::Hash { .. } => panic!("{label}: a hashed input"),
            }
        })
        .collect()
}

const FMT_STDIN: &str = "fmt/stdin.txt";

/// Standard input (`-`): the formatter cases (`format_cases`), inputs
/// only byte-level handling explains (not UTF-8, a byte order mark, CRLF,
/// a lone `\r`), broken inputs, the manual's examples and every corpus
/// file. `--check -` only checks standard input.
#[test]
fn fmt_matches_the_expected_output_on_standard_input() {
    let cases = format_cases();
    assert_eq!(cases.len(), FMT_FORMAT_CASES, "formatter cases");
    // (label, input, whether the entry keeps the input)
    let mut inputs: Vec<(String, Vec<u8>, bool)> = cases
        .iter()
        .map(|src| {
            (
                expected::short_label(&format!("format case {src:?}")),
                src.clone().into_bytes(),
                true,
            )
        })
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
        inputs.push((
            format!("{:?}", String::from_utf8_lossy(raw)),
            raw.to_vec(),
            false,
        ));
    }
    for (label, src) in manual_examples() {
        inputs.push((format!("manual {label}"), src.into_bytes(), false));
    }
    for f in fmt_corpus() {
        let src = std::fs::read(repo().join(&f)).unwrap();
        inputs.push((f, src, false));
    }
    let srcs: [&[u8]; 7] = [
        b"x=1\n",
        b"x = 1\n",
        b"",
        b"x = )\n",
        b"\xef\xbb\xbfx = 1\n",
        b"\xef\xbb\xbf\xef\xbb\xbfx\n",
        b"x = \"a\xffb\"\n",
    ];
    // `--check -` (either order) names `<stdin>` and exits 1 when the
    // input needs formatting, prints nothing and exits 0 when it does
    // not; errors are as without `--check`.
    let mut runs: Vec<(String, Vec<OsString>, Vec<u8>, bool)> = inputs
        .into_iter()
        .map(|(label, src, keep)| (label, os_args(&["fmt", "-"]), src, keep))
        .collect();
    for order in [["fmt", "--check", "-"], ["fmt", "-", "--check"]] {
        for src in srcs {
            runs.push((
                format!("{} {:?}", order.join(" "), String::from_utf8_lossy(src)),
                os_args(&order),
                src.to_vec(),
                false,
            ));
        }
    }
    assert_eq!(runs.len(), FMT_STDIN_CASES, "cases");
    let dir = scratch("tools-fmt-stdin");
    let owned = subs(&[(&dir, "$DIR")]);
    let s = sub_refs(&owned);
    let entries = |bin: &Path| -> Vec<Entry> {
        par_map(&runs, |(label, args, src, keep)| {
            let e = Entry::new(label.clone());
            let e = if *keep {
                e.field("stdin", src.clone())
            } else {
                e
            };
            let r = Entry::run("", &run_in(bin, &dir, args, Some(src)), &s);
            Entry {
                fields: e.fields.into_iter().chain(r.fields).collect(),
                ..e
            }
        })
    };
    let got = entries(&rust_fung());
    // Recording (stage 6b task 1 only; task 5 removes it): the reference
    // implementation, whose formatter tests' own expectations must hold.
    let reference = expected::recording().then(|| {
        let bin = expected::reference_fung("tools-fmt-stdin");
        let r = entries(&bin);
        let _ = std::fs::remove_file(&bin);
        for ((src, want), e) in go_format_cases().iter().zip(&r) {
            assert_eq!(
                String::from_utf8_lossy(e.get("stdout").unwrap()),
                *want,
                "{src:?}"
            );
        }
        r
    });
    // Outputs over 2 KiB (the corpus files, formatted) are kept as hashes.
    verify_entries_with(
        FMT_STDIN,
        "`fung fmt -` and `fung fmt --check -`; a `format case` keeps its input as `stdin`",
        2048,
        &got,
        reference.as_deref(),
    );
    let a = os_args(&["fmt", "--check", "-"]);
    let (out, _, status) = run_status(&rust_fung(), &a, Some(b"x=1\n"));
    assert_eq!((&out[..], status.code()), (&b"<stdin>\n"[..], Some(1)));
    let (out, _, status) = run_status(&rust_fung(), &a, Some(b"x = 1\n"));
    assert_eq!((&out[..], status.code()), (&b""[..], Some(0)));
}

/// Arguments, mixed paths and file errors: a file and a directory
/// together, a file named twice, `-` among paths, a missing path, an
/// unknown option, a link to a directory named `x.fung` (read error), an
/// unreadable and a read-only file needing formatting (read and write
/// errors), a missing name that is not UTF-8, a file with a syntax error
/// next to good ones (exit 2 wins over 1). (An existing file whose name
/// is not UTF-8: `fmt_handles_a_non_utf8_file_name`.)
#[test]
fn fmt_arguments_match_the_expected_output() {
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
    };
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
    let files = [
        "t/a.fung",
        "t/sub/b.fung",
        "t/sub/_fix/c.fung",
        "t/.hid/d.fung",
        "lone.fung",
        "넓은.fung",
        "ro.fung",
        "locked.fung",
    ];
    // Every case in turn with `bin` in a fresh tree `d`, then the files
    // and the read-only file's mode (kept).
    let entries = |bin: &Path, d: &Path| -> Vec<Entry> {
        setup(d);
        let owned = subs(&[(d, "$DIR")]);
        let s = sub_refs(&owned);
        let mut out = Vec::new();
        for a in cases {
            out.push(Entry::run(
                a.join(" "),
                &run_in(bin, d, &os_args(a), None),
                &s,
            ));
        }
        let non_utf8 = vec!["fmt".into(), OsString::from_vec(b"q\xfe.fung".to_vec())];
        out.push(Entry::run(
            "fmt (non-UTF-8 name)",
            &run_in(bin, d, &non_utf8, None),
            &s,
        ));
        std::fs::set_permissions(
            d.join("locked.fung"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        for f in files {
            out.push(
                Entry::new(format!("file {f}")).field("text", std::fs::read(d.join(f)).unwrap()),
            );
        }
        let mode = std::fs::metadata(d.join("ro.fung"))
            .unwrap()
            .permissions()
            .mode();
        out.push(Entry::new("mode ro.fung").field("mode", format!("{mode:o}")));
        out
    };
    let got = entries(&rust_fung(), &base.join("rust"));
    assert_eq!(got.len(), FMT_ARGUMENT_CASES, "cases");
    // Root reads and writes `locked.fung` and `ro.fung` anyway: the
    // expected output is a normal user's.
    let probe = base.join("probe");
    std::fs::write(&probe, "").unwrap();
    std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o000)).unwrap();
    let readable = std::fs::read(&probe).is_ok();
    if readable {
        return;
    }
    // Recording (stage 6b task 1 only; task 5 removes it).
    let reference = expected::recording().then(|| {
        let bin = expected::reference_fung("tools-fmt-args");
        let r = entries(&bin, &base.join("reference"));
        let _ = std::fs::remove_file(&bin);
        r
    });
    verify_entries(
        "fmt/arguments.txt",
        "`fung fmt` arguments and file errors, run in turn in one tree; then the files and a mode",
        &got,
        reference.as_deref(),
    );
}

/// A file whose name is not UTF-8 is listed and formatted under its name
/// as it is: the output is the one for the same file under a UTF-8 name,
/// with the name swapped (compared as lossy text; the output must also
/// hold the raw name).
/// Only where the file system allows such names (macOS refuses them,
/// EILSEQ); the same check with a UTF-8 stand-in name runs everywhere.
#[test]
fn fmt_handles_a_non_utf8_file_name() {
    fmt_handles_a_file_name("tools-non-utf8-fmt", b"q\xfe.fung");
    fmt_handles_a_file_name("tools-stand-in-fmt", "qé.fung".as_bytes());
}

/// See `fmt_handles_a_non_utf8_file_name`; returns early when the file
/// system refuses `name`.
fn fmt_handles_a_file_name(tag: &str, name: &[u8]) {
    let base = scratch(tag);
    let (odd, plain) = (base.join("odd"), base.join("plain"));
    std::fs::create_dir_all(&odd).unwrap();
    std::fs::create_dir_all(&plain).unwrap();
    let file = odd.join(OsString::from_vec(name.to_vec()));
    if std::fs::write(&file, "q=1\n").is_err() {
        return;
    }
    std::fs::write(plain.join("q_.fung"), "q=1\n").unwrap();
    let shown = String::from_utf8_lossy(name).into_owned();
    let swap = |b: Vec<u8>| expected::replace(&b, &[("q_.fung", shown.as_str())]);
    let lossy = |b: Vec<u8>| String::from_utf8_lossy(&b).as_bytes().to_vec();
    let rust = rust_fung();
    let check = vec![OsString::from("fmt"), "--check".into(), ".".into()];
    // `--check` lists the file, formatting in place names it.
    let cases = [
        (check.clone(), check),
        (
            vec![OsString::from("fmt"), OsString::from_vec(name.to_vec())],
            vec![OsString::from("fmt"), "q_.fung".into()],
        ),
    ];
    for (args_odd, args_plain) in cases {
        let got = run_in(&rust, &odd, &args_odd, None);
        let want = run_in(&rust, &plain, &args_plain, None);
        // The output holds the name's own bytes.
        assert!(contains(&got.0, name), "{got:?}");
        assert_eq!(
            (lossy(got.0), lossy(got.1), got.2),
            (swap(want.0), swap(want.1), want.2),
            "{args_odd:?}"
        );
    }
    assert_eq!(std::fs::read(&file).unwrap(), b"q = 1\n");
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

/// Whole sessions over a pipe: values, definitions, redefinition,
/// continued lines, errors and going on, commands, `exit`, empty input,
/// the end of input, `dbg`, tasks and handlers, byte for byte (both
/// streams and the status). A deadlock and a spawned task's `exit` have
/// tests of their own (`repl_recovers_after_a_deadlock`,
/// `repl_ends_on_a_spawned_exit`).
#[test]
fn repl_matches_the_expected_output_on_piped_sessions() {
    let cases: &[(&str, &str)] = &[
        // the basics
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
            "excerpts come from the current input",
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
    let dir = repl_dir("piped");
    let owned = subs(&[(&dir, "$DIR")]);
    let s = sub_refs(&owned);
    let mut sessions: Vec<(String, Vec<u8>)> = cases
        .iter()
        .map(|(label, input)| (label.to_string(), input.as_bytes().to_vec()))
        .collect();
    // Raw bytes that are not UTF-8.
    sessions.push((
        "invalid bytes".into(),
        b"print(\"\xff\xfe\")\n\"\xe2\x82\"\n".to_vec(),
    ));
    assert_eq!(sessions.len(), REPL_CASES, "cases");
    let entries = |bin: &Path| -> Vec<Entry> {
        sessions
            .iter()
            .map(|(label, input)| Entry::run(label.clone(), &repl_in(bin, &dir, input), &s))
            .collect()
    };
    let got = entries(&rust_fung());
    // Recording (stage 6b task 1 only; task 5 removes it).
    let reference = expected::recording().then(|| {
        let bin = expected::reference_fung("repl");
        let r = entries(&bin);
        let _ = std::fs::remove_file(&bin);
        r
    });
    let _ = std::fs::remove_dir_all(&dir);
    verify_entries(
        "repl/sessions.txt",
        "REPL sessions over a pipe (`fung` with no arguments)",
        &got,
        reference.as_deref(),
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
