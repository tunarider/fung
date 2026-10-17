//! The repository's `.fung` files are in the `fung fmt` style (08), by
//! the Rust formatter: the Rust side of Go's `tests/fmt_test.go`. The
//! golden tests that are syntax errors on purpose (`err_*.fung` that do
//! not parse) are skipped: the formatter refuses them by design.

mod common;

use std::ffi::OsString;

use common::{repo, run_status, rust_fung};

fn fmt_check(args: &[&str]) -> (String, String, Option<i32>) {
    let mut a: Vec<OsString> = vec!["fmt".into(), "--check".into()];
    a.extend(args.iter().map(OsString::from));
    let (out, err, status) = run_status(&rust_fung(), &a, None);
    (
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
        status.code(),
    )
}

#[test]
fn repository_is_formatted() {
    let (out, err, code) = fmt_check(&["extras", "examples", "crates/fung-cli/tests/data"]);
    assert!(
        code == Some(0) && out.is_empty(),
        "files need formatting (run `fung fmt extras examples crates/fung-cli/tests/data`):\n{out}{err}"
    );
    let mut files = Vec::new();
    let mut stack = vec![repo().join("tests/lang")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "fung") {
                files.push(
                    p.strip_prefix(repo())
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    files.sort();
    assert!(files.len() > 140, "{} files", files.len());
    let mut refused = 0;
    for f in &files {
        let (out, err, code) = fmt_check(&[f]);
        match code {
            Some(2) => {
                // A deliberate syntax error, and only those.
                let name = f.rsplit('/').next().unwrap();
                assert!(name.starts_with("err_"), "{f} does not parse:\n{err}");
                refused += 1;
            }
            _ => assert!(
                code == Some(0) && out.is_empty(),
                "{f} needs formatting (run `fung fmt {f}`):\n{out}{err}"
            ),
        }
    }
    assert!(refused > 0, "no deliberate syntax error was skipped");
}
