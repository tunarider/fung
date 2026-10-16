//! Golden files checked against their own expectations, with no Go
//! involved: every `tests/lang/**/*.fung` (directories starting with `_`
//! are module fixtures) that is not `# only: go` runs with `fung run`,
//! and is judged by the rules of Go's
//! `internal/golden` (`Parse`, `Check`).

mod common;

use std::ffi::OsString;
use std::path::Path;

use common::{repo, rust_fung};

/// What a golden file expects (Go: `golden.Case`).
#[derive(Debug, Default, PartialEq)]
struct Case {
    stdout: String,
    error_code: String,
    args: Vec<String>,
    stdin: String,
    exit_code: i32,
    only: String,
}

/// What a run did (Go: `golden.Result`).
struct Outcome {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

/// Go: `golden.Parse`.
fn parse(path: &str, src: &str) -> Result<Case, String> {
    let mut c = Case::default();
    for (i, line) in src.lines().enumerate() {
        let n = i + 1;
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("# expect-error:") {
            let code = rest.trim();
            let b = code.as_bytes();
            if !(b.len() == 5 && b[0] == b'E' && b[1..].iter().all(u8::is_ascii_digit)) {
                return Err(format!(
                    "{path}:{n}: malformed error code {code:?} (want like E0204)"
                ));
            }
            if !c.error_code.is_empty() {
                return Err(format!(
                    "{path}:{n}: second expect-error line (already expecting {})",
                    c.error_code
                ));
            }
            c.error_code = code.to_string();
        } else if let Some(rest) = line.strip_prefix("# expect-exit:") {
            c.exit_code = rest
                .trim()
                .parse()
                .map_err(|_| format!("{path}:{n}: malformed exit code in {line:?}"))?;
        } else if let Some(rest) = line.strip_prefix("# only:") {
            let v = rest.trim();
            if v != "go" && v != "rust" {
                return Err(format!(
                    "{path}:{n}: malformed only line {v:?} (want go or rust)"
                ));
            }
            if !c.only.is_empty() {
                return Err(format!("{path}:{n}: second only line (already {})", c.only));
            }
            c.only = v.to_string();
        } else if let Some(rest) = line.strip_prefix("# args:") {
            c.args.extend(rest.split_whitespace().map(String::from));
        } else if let Some(rest) = line.strip_prefix("# stdin:") {
            c.stdin.push_str(rest.strip_prefix(' ').unwrap_or(rest));
            c.stdin.push('\n');
        } else if let Some(rest) = line.strip_prefix("# expect:") {
            c.stdout.push_str(rest.strip_prefix(' ').unwrap_or(rest));
            c.stdout.push('\n');
        }
    }
    if !c.error_code.is_empty() && c.exit_code != 0 {
        return Err(format!(
            "{path}: expect-exit and expect-error cannot be combined"
        ));
    }
    Ok(c)
}

/// The first line where two texts differ.
fn first_diff(want: &str, got: &str) -> String {
    for (i, (w, g)) in want.lines().zip(got.lines()).enumerate() {
        if w != g {
            return format!(
                "first difference at line {}:\n  want: {w}\n  got:  {g}",
                i + 1
            );
        }
    }
    format!(
        "lengths differ: want {} lines, got {} lines",
        want.lines().count(),
        got.lines().count()
    )
}

/// Go: `golden.Check`. The usage exit code is 3.
fn check(c: &Case, r: &Outcome) -> Result<(), String> {
    const EXIT_USAGE: i32 = 3;
    if r.exit_code == EXIT_USAGE && c.exit_code != EXIT_USAGE {
        return Err(format!(
            "fung could not start (exit {})\nstderr:\n{}",
            r.exit_code, r.stderr
        ));
    }
    if r.stdout != c.stdout {
        return Err(format!(
            "stdout mismatch, {}\nwant:\n{}\ngot:\n{}",
            first_diff(&c.stdout, &r.stdout),
            c.stdout,
            r.stdout
        ));
    }
    if c.error_code.is_empty() {
        if r.exit_code != c.exit_code {
            return Err(format!(
                "exit code {}, want {}\nstderr:\n{}",
                r.exit_code, c.exit_code, r.stderr
            ));
        }
        if !r.stderr.is_empty() {
            return Err(format!("unexpected stderr output:\n{}", r.stderr));
        }
        return Ok(());
    }
    if r.exit_code == 0 {
        return Err(format!(
            "expected error {} but the script succeeded",
            c.error_code
        ));
    }
    if !r.stderr.contains(&format!("error[{}]", c.error_code)) {
        return Err(format!(
            "expected error {} not reported\nstderr:\n{}",
            c.error_code, r.stderr
        ));
    }
    Ok(())
}

fn case(src: &str) -> Case {
    parse("t.fung", src).unwrap()
}

fn outcome(stdout: &str, stderr: &str, exit_code: i32) -> Outcome {
    Outcome {
        stdout: stdout.into(),
        stderr: stderr.into(),
        exit_code,
    }
}

#[test]
fn parse_reads_every_directive() {
    let c = case(
        "# expect: 1\n# expect:\n  # expect: 2\n# args: a  b\n# stdin: x\n# stdin:\n# expect-exit: 4\n# only: rust\n",
    );
    assert_eq!(c.stdout, "1\n\n2\n");
    assert_eq!(c.args, ["a", "b"]);
    assert_eq!(c.stdin, "x\n\n");
    assert_eq!(c.exit_code, 4);
    assert_eq!(c.only, "rust");
}

#[test]
fn parse_rejects_what_go_rejects() {
    for bad in [
        "# expect-error: E12\n",
        "# expect-error: E0001\n# expect-error: E0002\n",
        "# expect-exit: x\n",
        "# only: both\n",
        "# only: go\n# only: rust\n",
        "# expect-error: E0001\n# expect-exit: 1\n",
    ] {
        assert!(parse("t.fung", bad).is_err(), "{bad:?}");
    }
}

#[test]
fn a_matching_run_passes() {
    assert!(check(&case("# expect: 1\n"), &outcome("1\n", "", 0)).is_ok());
    assert!(check(&case("# expect-exit: 4\n"), &outcome("", "", 4)).is_ok());
    let e = case("# expect-error: E0204\n");
    assert!(check(&e, &outcome("", "error[E0204]: x\n", 1)).is_ok());
}

#[test]
fn different_stdout_fails() {
    let err = check(&case("# expect: 1\n"), &outcome("2\n", "", 0)).unwrap_err();
    assert!(
        err.contains("stdout mismatch") && err.contains("line 1"),
        "{err}"
    );
}

#[test]
fn a_missing_error_code_fails() {
    let e = case("# expect-error: E0702\n");
    assert!(check(&e, &outcome("", "error[E0703]: x\n", 1)).is_err());
    assert!(check(&e, &outcome("", "", 1)).is_err());
    // Succeeding is not an error either.
    assert!(check(&e, &outcome("", "error[E0702]: x\n", 0)).is_err());
}

#[test]
fn unexpected_stderr_fails() {
    assert!(check(&case(""), &outcome("", "warning\n", 0)).is_err());
}

#[test]
fn a_wrong_exit_code_fails() {
    assert!(check(&case(""), &outcome("", "", 1)).is_err());
    assert!(check(&case("# expect-exit: 4\n"), &outcome("", "", 0)).is_err());
    // A usage failure is reported as such unless expected.
    let err = check(&case(""), &outcome("", "usage\n", 3)).unwrap_err();
    assert!(err.contains("could not start"), "{err}");
}

/// Every `*.fung` under `dir`, skipping directories that start with `_`.
fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if p.is_dir() {
            if !name.starts_with('_') {
                walk(&p, out);
            }
        } else if name.ends_with(".fung") {
            out.push(p);
        }
    }
}

#[test]
fn golden_files_meet_their_expectations() {
    let root = repo();
    let mut files = Vec::new();
    walk(&root.join("tests/lang"), &mut files);
    files.sort();
    let (mut checked, mut only) = (0, 0);
    let mut failures = Vec::new();
    for f in &files {
        let rel = f
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let src = std::fs::read_to_string(f).unwrap();
        let c = match parse(&rel, &src) {
            Ok(c) => c,
            Err(e) => {
                failures.push(e);
                continue;
            }
        };
        if c.only == "go" {
            only += 1;
            continue;
        }
        let mut args: Vec<OsString> = vec!["run".into(), rel.clone().into()];
        args.extend(c.args.iter().map(OsString::from));
        let (out, err, status) = common::run_status(&rust_fung(), &args, Some(c.stdin.as_bytes()));
        let r = Outcome {
            stdout: String::from_utf8_lossy(&out).into_owned(),
            stderr: String::from_utf8_lossy(&err).into_owned(),
            exit_code: status.code().unwrap_or(-1),
        };
        checked += 1;
        if let Err(e) = check(&c, &r) {
            failures.push(format!("{rel}: {e}"));
        }
    }
    eprintln!("golden: {checked} checked, {only} only-go");
    assert!(
        failures.is_empty(),
        "{} of {checked} golden files fail:\n{}",
        failures.len(),
        failures.join("\n")
    );
    // The check cannot pass by checking nothing: one below the 139 files
    // checked after stage 4d's `http_serve_rust`; raise it as files are
    // added.
    assert!(checked >= 138, "only {checked} golden files were checked");
}
