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
//!   `examples` and `extras`, the warning fixtures in
//!   `tests/data/check`, `-e` programs, and argument errors.
//! - The manual: its checked examples (`manual/**/*.md`, which the
//!   generator in `manual_build.rs` checks and runs) have a fixed
//!   formatter output like any source.
//! - File names that are not UTF-8 are tested where the file system
//!   allows them, by comparison with the same file under a UTF-8 name
//!   (`*_non_utf8_*`), so the fixed outputs are the same on every system.

mod common;
#[path = "manual_site/markup.rs"]
mod markup;
#[allow(dead_code)]
#[path = "../src/paths.rs"]
mod paths;

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use common::expected::{self, Entry};
use common::{fung_bin, repo, run, run_status};

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

/// Checks `cases` against the file `name` (see `expected::verify`).
fn verify_cases(name: &str, header: &str, cases: &[(String, Vec<OsString>)], subs: &Subs) {
    let got = entries(&fung_bin(), cases, subs);
    verify_entries(name, header, &got);
}

/// `expected::verify` with this file's limit and the update command.
fn verify_entries(name: &str, header: &str, got: &[Entry]) {
    verify_entries_with(name, header, LIMIT, got);
}

fn verify_entries_with(name: &str, header: &str, limit: usize, got: &[Entry]) {
    expected::verify(
        name,
        &format!(
            "{header} (crates/fung-cli/tests/tools_expected.rs).\n\
             $REPO stands for the repository, $DIR for the test's scratch directory.\n\
             Update: FUNG_EXPECTED=bless cargo test -p fung-cli --test tools_expected"
        ),
        limit,
        got,
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

/// The manual's checked examples: every `fung run` and `fung check`
/// fence of `manual/**/*.md`, as `(label, source)` with the generator's
/// labels (`reference/functions.md:12`). The generator
/// (`manual_build.rs`) checks and runs them; here they feed the
/// formatter's corpus.
pub fn manual_examples() -> Vec<(String, String)> {
    let root = repo().join("manual");
    let mut files = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "md") {
                files.push(p);
            }
        }
    }
    files.sort();
    let mut out = Vec::new();
    for f in files {
        let rel = f
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let page = markup::parse(&rel, &std::fs::read_to_string(&f).unwrap()).unwrap();
        for (label, code) in markup::examples(&page) {
            if matches!(code.kind, markup::Kind::Run | markup::Kind::Check) {
                out.push((label, code.src.clone()));
            }
        }
    }
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
const CHECK_REPOSITORY_CASES: usize = 230;
const CHECK_WARNING_CASES: usize = 9;
const CHECK_ARGUMENT_CASES: usize = 25;
const HELP_CASES: usize = 8;
const VERSION_CASES: usize = 2;
const WALK_CASES: usize = 29;
const WALK_UNREADABLE_CASES: usize = 2;
const TEST_FIXTURE_CASES: usize = 24;
const TEST_REPOSITORY_CASES: usize = 231;
const FMT_INPUTS: usize = 5658;
const FMT_STDIN_CASES: usize = 1279;
const FMT_FORMAT_CASES: usize = 124;
const FMT_ARGUMENT_CASES: usize = 33;
const REPL_CASES: usize = 30;

#[test]
fn check_matches_the_expected_output_on_the_repository() {
    let files = corpus(&["tests/lang", "examples", "extras"]);
    assert!(files.len() > 150, "corpus: {} files", files.len());
    assert!(files.iter().any(|f| f.starts_with("extras/")));
    assert!(files.iter().any(|f| f.starts_with("tests/lang/_lib/")));
    let cases: Vec<_> = files.iter().map(|f| check(f)).collect();
    assert_eq!(cases.len(), CHECK_REPOSITORY_CASES, "cases");
    verify_cases(
        "check/repository.txt",
        "`fung check` on the repository's .fung files",
        &cases,
        &subs(&[]),
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
        &cases,
        &subs(&[]),
    );
    // The fixtures do warn: two silent runs would prove nothing.
    let (_, err, _) = run(
        &fung_bin(),
        &os_args(&["check", "crates/fung-cli/tests/data/check/unused_vars.fung"]),
    );
    assert!(String::from_utf8_lossy(&err).contains("warning[W0102]"));
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
        &fung_bin(),
        &[OsString::from("check"), odd.into_os_string()],
    );
    let want = run(
        &fung_bin(),
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
        &cases,
        &subs(&[]),
    );
}

/// The Rust binary's version, as `fung version` prints it.
fn built_version() -> String {
    let (out, _, _) = run(&fung_bin(), &os_args(&["version"]));
    let text = String::from_utf8(out).unwrap();
    text.strip_prefix("fung ")
        .and_then(|v| v.strip_suffix('\n'))
        .unwrap_or_else(|| panic!("version output {text:?}"))
        .to_string()
}

#[test]
fn version_matches_the_expected_output() {
    let v = built_version();
    let want = option_env!("FUNG_BUILD_VERSION")
        .map(|s| s.strip_prefix('v').unwrap_or(s))
        .filter(|s| !s.is_empty())
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
    let got = entries(&fung_bin(), &cases, &s);
    verify_entries(
        "cli/version.txt",
        "`fung version` ($VERSION: the build's version)",
        &got,
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
    let bin = fung_bin();
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
    // Each case: its label and the listing. A root on its own is listed
    // by the walker functions; a relative root by the commands.
    let mut got = Vec::new();
    for root in &roots {
        let r = ours(paths::test_files(root.as_bytes()));
        got.push(listing(&format!("test {root}"), &r, &s));
    }
    for args in &commands {
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        let r = listed(&bin, &a);
        assert!(r.as_ref().is_ok_and(|f| !f.is_empty()), "{args:?}: {r:?}");
        got.push(listing(&args.join(" "), &r, &s));
    }
    for g in &groups {
        let raw: Vec<Vec<u8>> = g.iter().map(|p| p.as_bytes().to_vec()).collect();
        let r = ours(paths::fung_files(&raw));
        got.push(listing(&format!("fmt --check {}", g.join(" ")), &r, &s));
    }
    assert_eq!(got.len(), WALK_CASES, "cases");
    verify_entries(
        "walk/listings.txt",
        "The files `fung test` and `fung fmt --check` visit from each root ($REL: $DIR relative to $REPO)",
        &got,
    );
    // The tree does exercise the rules.
    let all = ours(paths::fung_files(&[abs("tree").into_bytes()])).unwrap();
    assert!(all.iter().any(|f| f.ends_with("/.h.fung")));
    assert!(all.iter().any(|f| f.ends_with("/link.fung")));
    assert!(
        !all.iter()
            .any(|f| f.contains("/_skip/") || f.contains("/.hidden/"))
    );
    // `test` and `fmt` choose the same files: `.` and `_` directories
    // skipped, a link named as the root followed, links below it not
    for root in ["tree", "rootlink", "tree/sub"] {
        let tests = ours(paths::test_files(abs(root).as_bytes()));
        let fmts = ours(paths::fung_files(&[abs(root).into_bytes()]));
        assert_eq!(tests, fmts, "{root}");
    }
    let tests = ours(paths::test_files(abs("tree").as_bytes())).unwrap();
    assert!(
        !tests
            .iter()
            .any(|f| f.contains("/.hidden/") || f.contains("/_skip/"))
    );
    assert!(!tests.iter().any(|f| f.contains("/linkdir/")));
    // a link met in the walk that names a directory is not a file
    let all = ours(paths::fung_files(&[abs(".").into_bytes()])).unwrap();
    assert!(!all.iter().any(|f| f.ends_with("/sublink.fung")), "{all:?}");
    let linked = ours(paths::test_files(abs("rootlink").as_bytes())).unwrap();
    assert!(linked.iter().any(|f| f.ends_with("/a.fung")), "{linked:?}");
    assert_eq!(
        ours(paths::test_files(abs("nope").as_bytes())),
        Err(format!(
            "cannot read `{}`: no such file or directory",
            abs("nope")
        ))
    );
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
    std::fs::set_permissions(dir.join("t/locked"), std::fs::Permissions::from_mode(0o755)).unwrap();
    // Root reads the locked directory anyway: the expected listings are
    // a normal user's.
    if !readable {
        verify_entries(
            "walk/unreadable.txt",
            "The walkers on a tree with a directory they cannot read",
            &got,
        );
        assert!(got[0].get("error").is_some(), "{got:?}");
    }
}

#[test]
fn manual_examples_are_extracted() {
    let ex = manual_examples();
    assert!(ex.len() > 40, "{} examples", ex.len());
    assert!(
        ex.iter()
            .any(|(_, s)| s.contains("fn slug(s) = lower(s) |> split(\" \")"))
    );
}

// ---- `fung test` (stage 5 task 2) ----

/// The `fung test` fixtures: deliberate failures, `assert` displays,
/// load errors shared through `_lib`, and one file per way a file could
/// leak into the next (exit, a `with` released by an exit, a deadlock,
/// an exit inside a task). `test_ends` holds the ones about how a
/// file's run ends (atoms, skipped rests), run on their own.
const TEST_DATA: &str = "crates/fung-cli/tests/data/test";
const TEST_ENDS: &str = "crates/fung-cli/tests/data/test_ends";

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
        &cases,
        &subs(&[]),
    );
    // The fixtures do fail in every way the outputs are about.
    let (out, err, _) = run(&fung_bin(), &os_args(&["test", &root]));
    let (out, err) = (String::from_utf8_lossy(&out), String::from_utf8_lossy(&err));
    for want in [
        "  exit(4) called inside a test file",
        "    slug(\"Hello World\") = \"hello world\"",
        "FAIL crates/fung-cli/tests/data/test/deadlock.fung: deadlocks",
        "FAIL crates/fung-cli/tests/data/test/exit_in_task.fung: a task exits",
        "ok   crates/fung-cli/tests/data/test/z_clean.fung: nothing leaked",
        "FAIL crates/fung-cli/tests/data/test/uses_lib_too.fung: <load>",
        "FAIL crates/fung-cli/tests/data/test/top_exit.fung: <top level>",
        // the exit inside a test still runs its `with`'s release
        "released\nFAIL crates/fung-cli/tests/data/test/exit_in_test.fung: exits",
    ] {
        assert!(out.contains(want), "{want}\n{out}");
    }
    assert!(!out.contains("must not run") && !out.contains("only when it is the entry"));
    assert!(err.contains("error[E0702]"));
    assert_eq!(err.matches("undefined name `missing`").count(), 1);
}

/// A file `fmt` cannot write back is named as the `fs` module names it.
#[test]
fn fmt_names_a_file_it_cannot_write() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("fmt-unwritable");
    let p = dir.join("ro.fung");
    std::fs::write(&p, "x=1\n").unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o444)).unwrap();
    if std::fs::OpenOptions::new().write(true).open(&p).is_ok() {
        eprintln!("skipping: the file is writable anyway (running as root?)");
        return;
    }
    let arg = p.to_string_lossy().into_owned();
    let (_, err, _) = run(&fung_bin(), &os_args(&["fmt", &arg]));
    assert_eq!(
        String::from_utf8_lossy(&err),
        format!("error: cannot write `{arg}`: permission denied\n")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A test file that cannot be read fails like one that cannot load: its
/// `FAIL <load>` line, a failure in the summary, exit code 2.
#[test]
fn test_counts_an_unreadable_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("test-unreadable");
    let p = dir.join("locked.fung");
    std::fs::write(&p, "test \"t\": assert true ;;\n").unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&p).is_ok() {
        eprintln!("skipping: the file is readable anyway (running as root?)");
        return;
    }
    let arg = p.to_string_lossy().into_owned();
    let (out, err, status) = run(&fung_bin(), &os_args(&["test", &arg]));
    let (out, err) = (String::from_utf8_lossy(&out), String::from_utf8_lossy(&err));
    assert_eq!(
        out,
        format!("FAIL {arg}: <load>\n0 passed, 1 failed\n"),
        "{err}"
    );
    assert_eq!(
        err,
        format!("error: cannot read `{arg}`: permission denied\n")
    );
    assert_eq!(status, "ExitStatus(unix_wait_status(512))");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `fung test` with no path walks the working directory.
#[test]
fn test_without_a_path_walks_the_working_directory() {
    let dir = repo().join(TEST_DATA).join("_lib");
    let entry = |bin: &Path| {
        let o = common::output(
            std::process::Command::new(bin)
                .env("TZ", common::TZ)
                .arg("test")
                .current_dir(&dir),
        )
        .unwrap();
        Entry::run(
            format!("test (in {TEST_DATA}/_lib)"),
            &(o.stdout, o.stderr, format!("{:?}", o.status)),
            &sub_refs(&subs(&[])),
        )
    };
    let got = vec![entry(&fung_bin())];
    verify_entries(
        "test/working_directory.txt",
        "`fung test` with no path",
        &got,
    );
    assert!(String::from_utf8_lossy(got[0].get("stdout").unwrap()).contains("ok   good.fung"));
}

#[test]
fn test_matches_the_expected_output_on_the_repository() {
    let files = corpus(&["tests/lang", "examples", "extras"]);
    let mut cases: Vec<_> = files.iter().map(|f| test_case(f)).collect();
    // The examples directory in one run, from a copy (the expected output
    // names it `$DIR/examples`).
    let base = scratch("tools-test-examples");
    let dir = base.join("examples");
    let mut stack = vec![repo().join("examples")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            let to = dir.join(p.strip_prefix(repo().join("examples")).unwrap());
            if p.is_dir() {
                std::fs::create_dir_all(&to).unwrap();
                stack.push(p);
            } else {
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
        &cases,
        &subs(&[(&base, "$DIR")]),
    );
    let (out, _, _) = run(&fung_bin(), &os_args(&["test", &d]));
    let out = String::from_utf8_lossy(&out);
    assert!(
        out.contains("ok   ") && out.ends_with(" passed, 0 failed\n"),
        "{out}"
    );
}

/// The examples directory as it is: every `test` block passes, nothing
/// on standard error.
#[test]
fn test_runs_the_examples() {
    let (out, err, status) = run_status(&fung_bin(), &os_args(&["test", "examples"]), None);
    let (out, err) = (String::from_utf8_lossy(&out), String::from_utf8_lossy(&err));
    assert!(
        status.success() && out.ends_with("12 passed, 0 failed\n") && err.is_empty(),
        "{status}\n{out}\n{err}"
    );
    assert!(out.contains("ok   examples/api.fung: "), "{out}");
}

/// A file's globals (here an atom) end with it. A deadlock or an exit asked for by a
/// spawned task skips the rest of the file, like a direct `exit(n)`
/// (08), after the `with` around the task's scope is released. The next
/// file starts clean.
#[test]
fn test_ends_each_files_run() {
    let start = std::time::Instant::now();
    let (out, err, status) = run_status(&fung_bin(), &os_args(&["test", TEST_ENDS]), None);
    assert!(start.elapsed() < std::time::Duration::from_secs(20));
    let p = |f: &str| format!("{TEST_ENDS}/{f}");
    let after = p("after_cancel.fung");
    let exit = p("task_exit.fung");
    let top = p("top_task_exit.fung");
    let want = [
        format!("ok   {after}: before"),
        format!("FAIL {after}: deadlocks"),
        "  error[E0702]: deadlock: all tasks are waiting".to_string(),
        format!("    --> {after}:4:7"),
        "    |".to_string(),
        "  4 |   recv(rx)".to_string(),
        "    |       ^".to_string(),
        "    |".to_string(),
        "   call path (innermost first):".to_string(),
        format!("     {after}:4 test: deadlocks"),
        "  a deadlock was reported; the rest of the file was skipped".to_string(),
        format!("ok   {}: swap", p("atoms.fung")),
        "released".to_string(),
        format!("FAIL {exit}: a task exits"),
        "  error[E0701]: task cancelled".to_string(),
        format!("    --> {exit}:4:10"),
        "    |".to_string(),
        "  4 |     await(t)".to_string(),
        "    |          ^".to_string(),
        "    |".to_string(),
        "   call path (innermost first):".to_string(),
        format!("     {exit}:4 <lambda>"),
        format!("     {exit}:2 <lambda>"),
        format!("     {exit}:2 test: a task exits"),
        "  exit(3) called inside a test file; the rest of the file was skipped".to_string(),
        format!("FAIL {top}: <top level>"),
        "  error[E0701]: task cancelled".to_string(),
        format!("    --> {top}:7:6"),
        "    |".to_string(),
        "  7 | scope(fn(s):".to_string(),
        "    |      ^".to_string(),
        "    |".to_string(),
        "   call path (innermost first):".to_string(),
        format!("     {top}:7 <entry>"),
        format!("ok   {}: a fresh file", p("z_next.fung")),
        "3 passed, 3 failed".to_string(),
    ]
    .map(|l| l + "\n")
    .concat();
    assert_eq!(String::from_utf8_lossy(&out), want);
    assert_eq!(
        String::from_utf8_lossy(&err),
        format!(
            "error[E0702]: deadlock: all tasks are waiting\n  --> {after}:4:7\n  |\n4 |   recv(rx)\n  |       ^\n  |\n call path (innermost first):\n   {after}:4 test: deadlocks\n help: this task waits to receive, but it still holds that channel's sending end itself (a variable of `test: deadlocks`), so the channel cannot close: close it with `close` first, or leave the sending end to the producers only\n"
        )
    );
    assert_eq!(status.code(), Some(1));
}

/// When a task in a top-level scope asks for an exit, the top level
/// fails and no block runs. The fixture orders the exit by state (the
/// task sends and exits in one turn on the run thread), so the output is
/// fixed; the loop shows it.
#[test]
fn a_task_exit_fails_the_top_level() {
    let top = format!("{TEST_ENDS}/top_task_exit.fung");
    let want = format!(
        "FAIL {top}: <top level>\n  error[E0701]: task cancelled\n    --> {top}:7:6\n    |\n  7 | scope(fn(s):\n    |      ^\n    |\n   call path (innermost first):\n     {top}:7 <entry>\n0 passed, 1 failed\n"
    );
    for _ in 0..20 {
        let (out, err, status) = run_status(&fung_bin(), &os_args(&["test", &top]), None);
        assert_eq!(String::from_utf8_lossy(&out), want);
        assert_eq!(String::from_utf8_lossy(&err), "");
        assert_eq!(status.code(), Some(1));
    }
}

// ---- `fung fmt` (stage 5 task 3) ----

/// Every `.fung` file of the repository the formatter is compared on:
/// the golden tests (with `_lib` and the deliberate `err_*` syntax
/// errors, whose error output must match), the examples, the extras
/// and the CLI test fixtures.
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
    // killed and waited for if anything below panics
    let mut child = common::Reaped::new(
        common::spawn(
            Command::new(bin)
                .env("TZ", common::TZ)
                .args(args)
                .current_dir(dir)
                .stdin(if stdin.is_some() {
                    Stdio::piped()
                } else {
                    Stdio::null()
                })
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
        )
        .unwrap(),
    );
    if let Some(data) = stdin {
        let mut pipe = child.stdin.take().unwrap();
        let data = data.to_vec();
        std::thread::spawn(move || {
            let _ = pipe.write_all(&data);
        });
    }
    let (o, _) = child.output_within(common::RUN_LIMIT).unwrap();
    (o.stdout, o.stderr, format!("{:?}", o.status))
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
    let (got, before, after) = format(&fung_bin(), &rdir);
    // Kept as text: the listings and the tree are one line per file.
    verify_entries_with(
        "fmt/variants.txt",
        "`fung fmt --check` and `fung fmt` on the repository's files and messed-up variants of them;\n\
         the formatted files: per file `unchanged`, or its length and FNV-1a hash",
        1024 * 1024,
        &got,
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
    let (out, _, _) = run_in(&fung_bin(), &rdir, &check, None);
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

/// The formatter cases: inputs kept in `data/fmt/cases.txt`, each one's
/// formatted text in `fmt/stdin.txt`.
fn format_cases() -> Vec<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/fmt/cases.txt");
    let text = std::fs::read_to_string(&path).unwrap();
    expected::parse(&text)
        .into_iter()
        .map(|(label, fields)| match &fields[..] {
            [(n, expected::Stored::Bytes(b))] if n == "src" => String::from_utf8(b.clone())
                .unwrap_or_else(|_| panic!("{}: {label} is not UTF-8", path.display())),
            _ => panic!("{}: {label}: want one `src` field", path.display()),
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
    let got = entries(&fung_bin());
    // Outputs over 2 KiB (the corpus files, formatted) are kept as hashes.
    verify_entries_with(
        FMT_STDIN,
        "`fung fmt -` and `fung fmt --check -`; a `format case` keeps its input as `stdin`",
        2048,
        &got,
    );
    let a = os_args(&["fmt", "--check", "-"]);
    let (out, _, status) = run_status(&fung_bin(), &a, Some(b"x=1\n"));
    assert_eq!((&out[..], status.code()), (&b"<stdin>\n"[..], Some(1)));
    let (out, _, status) = run_status(&fung_bin(), &a, Some(b"x = 1\n"));
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
    let got = entries(&fung_bin(), &base.join("rust"));
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
    verify_entries(
        "fmt/arguments.txt",
        "`fung fmt` arguments and file errors, run in turn in one tree; then the files and a mode",
        &got,
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
    let bin = fung_bin();
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
        let got = run_in(&bin, &odd, &args_odd, None);
        let want = run_in(&bin, &plain, &args_plain, None);
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

/// `fmt` rewrites a file by writing a temporary file beside it and
/// renaming it over (a crash never leaves half a file): a plain file gets
/// a new inode, its mode is kept, and no temporary file is left. A
/// symbolic link stays a link (the file it names is rewritten), and a
/// file with several hard links is rewritten in place so every name sees
/// the new text.
#[test]
fn fmt_in_place_renames_over_and_keeps_links() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let dir = scratch("fmt-atomic");
    let bin = fung_bin();
    let fmt = |name: &str| run_in(&bin, &dir, &[OsString::from("fmt"), name.into()], None);
    let plain = dir.join("a.fung");
    std::fs::write(&plain, "a=1\n").unwrap();
    std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o640)).unwrap();
    let before = std::fs::metadata(&plain).unwrap().ino();
    assert!(fmt("a.fung").1.is_empty());
    let after = std::fs::metadata(&plain).unwrap();
    assert_eq!(std::fs::read_to_string(&plain).unwrap(), "a = 1\n");
    assert_ne!(after.ino(), before, "renamed over");
    assert_eq!(after.permissions().mode() & 0o777, 0o640);
    // a link stays a link
    std::fs::write(dir.join("t.fung"), "t=1\n").unwrap();
    std::os::unix::fs::symlink("t.fung", dir.join("l.fung")).unwrap();
    assert!(fmt("l.fung").1.is_empty());
    assert!(
        std::fs::symlink_metadata(dir.join("l.fung"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("t.fung")).unwrap(),
        "t = 1\n"
    );
    // hard links all see the change
    std::fs::write(dir.join("h.fung"), "h=1\n").unwrap();
    std::fs::hard_link(dir.join("h.fung"), dir.join("h2.fung")).unwrap();
    assert!(fmt("h.fung").1.is_empty());
    assert_eq!(
        std::fs::read_to_string(dir.join("h2.fung")).unwrap(),
        "h = 1\n"
    );
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["a.fung", "h.fung", "h2.fung", "l.fung", "t.fung"]);
}

// ---- the REPL (stage 5 task 4) ----

/// Runs the REPL (`fung` without arguments) in `dir` with `stdin` piped
/// in and `HOME` pointing at `dir`, so no real history file is touched.
fn repl_in(bin: &Path, dir: &Path, stdin: &[u8]) -> common::Run {
    use std::io::Write;
    use std::process::{Command, Stdio};
    // killed and waited for if anything below panics
    let mut child = common::Reaped::new(
        common::spawn(
            Command::new(bin)
                .env("TZ", common::TZ)
                .current_dir(dir)
                .env("HOME", dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
        )
        .unwrap(),
    );
    let mut pipe = child.stdin.take().unwrap();
    let data = stdin.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = pipe.write_all(&data);
    });
    let (o, _) = child.output_within(common::RUN_LIMIT).unwrap();
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
/// the end of input, `dbg`, `with` releases, byte for byte (both
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
            "with and exit",
            "with(() => nil, _ => exit(4), _ => print(\"bye\"))\n",
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
            "excerpts come from the input that defined the code",
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
            "releases in order after exit",
            "with(() => 1, _ => with(() => 2, _ => exit(7), r => print(\"release\", r)), r => print(\"release\", r))\nprint(\"never\")\n",
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
    let got = entries(&fung_bin());
    let _ = std::fs::remove_dir_all(&dir);
    verify_entries(
        "repl/sessions.txt",
        "REPL sessions over a pipe (`fung` with no arguments)",
        &got,
    );
}

/// An atom an input stores stays usable in later inputs, and the REPL
/// exits at once when its input ends.
#[test]
fn repl_atoms_stay_across_inputs() {
    let dir = repl_dir("atoms");
    let start = std::time::Instant::now();
    let (out, err, status) = repl_in(
        &fung_bin(),
        &dir,
        b"a = atom(1)\nswap(a, (n) => n + 1)\nderef(a)\nderef(atom(5))\nswap(a, (n) => n * 10)\nwith(() => nil, _ => nil, _ => print(deref(a)))\n",
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(start.elapsed() < std::time::Duration::from_secs(20));
    assert_eq!(String::from_utf8_lossy(&err), "");
    assert_eq!(
        String::from_utf8_lossy(&out),
        "> > 2\n> 2\n> 5\n> 20\n> 20\nnil\n> \n"
    );
    assert_eq!(status, "ExitStatus(unix_wait_status(0))");
}

/// Runs a REPL session: (stdout, stderr, exit code).
fn repl_session(tag: &str, input: &str) -> (String, String, Option<i32>) {
    let dir = repl_dir(tag);
    // killed and waited for if anything below panics
    let mut child = common::Reaped::new(
        common::spawn(
            std::process::Command::new(fung_bin())
                .env("TZ", common::TZ)
                .current_dir(&dir)
                .env("HOME", &dir)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped()),
        )
        .unwrap(),
    );
    let mut pipe = child.stdin.take().unwrap();
    let data = input.as_bytes().to_vec();
    let writer = std::thread::spawn(move || {
        use std::io::Write;
        let _ = pipe.write_all(&data);
    });
    let (o, _) = child.output_within(common::RUN_LIMIT).unwrap();
    writer.join().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        o.status.code(),
    )
}

/// Standard input from `/dev/null` (a character device, not a terminal)
/// is read as plain lines: no line editor, so no history file.
#[test]
fn repl_reads_a_device_that_is_no_terminal_plainly() {
    let dir = repl_dir("devnull");
    let o = common::spawn(
        std::process::Command::new(fung_bin())
            .env("TZ", common::TZ)
            .current_dir(&dir)
            .env("HOME", &dir)
            .stdin(std::fs::File::open("/dev/null").unwrap())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped()),
    )
    .unwrap()
    .wait_with_output()
    .unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), "> \n");
    assert!(o.status.success());
    assert!(
        !dir.join(".fung_history").exists(),
        "a history file was made"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// An error in code an earlier input defined is excerpted from that
/// input, and so is a failing child that a scope re-raises.
#[test]
fn repl_excerpts_the_input_that_defined_the_code() {
    let (_, err, code) = repl_session(
        "excerpts",
        "fn f(x):\n  x / 0\n;;\n1\nf(1)\nscope(fn(s):\n  spawn(s, () => [1][5])\n;;)\n",
    );
    assert!(
        err.contains("  --> <repl>:2:5\n  |\n2 |   x / 0\n"),
        "the defining input's line: {err}"
    );
    assert!(
        err.contains("2 |   spawn(s, () => [1][5])\n"),
        "the child failure's line: {err}"
    );
    assert_eq!(code, Some(0));
}

/// The session's globals have no fixed limit: an input defining 65,537
/// of them runs, and later inputs see them.
#[test]
fn repl_globals_have_no_fixed_limit() {
    let many: String = (0..65_537).map(|i| format!("g{i} = {i}\n")).collect();
    let input = format!(
        "a = 1\n{}\ng65536 + a\n",
        many.replace('\n', "; ").trim_end_matches("; ")
    );
    let (out, err, code) = repl_session("globals", &input);
    assert_eq!(err, "");
    assert!(out.ends_with("> 65537\n> \n"), "{out}");
    assert_eq!(code, Some(0));
}

/// Decided 2026-10-17 (08 REPL): a deadlock fails only the input it
/// happened in, with the deadlock report (here a scope's body and its
/// blocked child; the report shows where both wait, so the input's
/// error is not printed again); the session goes on on a fresh run root,
/// globals and atoms stay, later inputs can spawn again, and the
/// recovered deadlock does not set the exit code.
#[test]
fn repl_recovers_after_a_deadlock() {
    let (out, err, code) = repl_session(
        "deadlock",
        "a = atom(1)\n(_tx, c) = channel()\nscope(fn(s):\n  t = spawn(s, () => recv(c))\n  recv(c)\n;;)\nprint(\"after\")\nscope(s => await(spawn(s, () => 40 + 2)))\nswap(a, (n) => n + 1)\n",
    );
    assert_eq!(out, "> > > . . . > after\nnil\n> 42\n> 2\n> \n");
    assert_eq!(
        err,
        "error[E0702]: deadlock: all tasks are waiting\n  --> <repl>:3:7\n  |\n3 |   recv(c)\n  |       ^\n  |\n call path (innermost first):\n   <repl>:3   <lambda>\n   <repl>:1   <entry>\n help: this task waits to receive, but it still holds that channel's sending end itself (the global `_tx`), so the channel cannot close: close it with `close(_tx)` first, or leave the sending end to the producers only\n 1 other task is waiting:\n   recv at <repl>:2:26 (<lambda>, holds its own sending end)\n"
    );
    assert_eq!(code, Some(0));
}

/// Decided 2026-10-17: a spawned task's `exit(n)` ends the
/// session like `exit(n)` typed at the prompt: the input it cut short
/// reports nothing, later lines are not run, the `with` around it is
/// released, and the code is n.
#[test]
fn repl_ends_on_a_spawned_exit() {
    let (out, err, code) = repl_session(
        "spawned-exit",
        "(tx, c) = channel()\nwith(() => nil, _ => scope(fn(s):\n  t = spawn(s, fn():\n    recv(c)\n    exit(3)\n  ;;)\n  [send(tx, 1), await(t)]\n;;), _ => print(\"bye\"))\nprint(\"never\")\n",
    );
    assert_eq!(out, "> > . . . . . . bye\n");
    assert_eq!(err, "");
    assert_eq!(code, Some(3));
}
