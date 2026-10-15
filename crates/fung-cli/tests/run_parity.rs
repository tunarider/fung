//! Runs compared with the Go implementation: every `tests/lang` golden file
//! (with its `# args:` and `# stdin:` lines) except the skip list, the
//! benchmarks, and handwritten `-e` programs.

mod common;

use std::ffi::OsString;

use common::{diff, go_fung, repo, run_with_stdin, rust_fung};

const CASES: &[&str] = &[
    "fn f(n) = n == 0 then 1 / 0 else f(n - 1)\nfn g(x) = f(x) + 1\nprint(g(3))",
    "at_exit(() => print(\"bye\"))\nprint([1, 2] + [3], {a: 1}, (1, \"x\"), Ok(2), 1..3, 1.5, 90m, 's, nil)\nexit(4)",
    "fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)\nprint(count(1000000, 0))",
    "fn d(n) = n == 0 then 0 else 1 + d(n - 1)\nprint(d(200000))",
    "print(fold(0..3, 0, (a, x) => a + fold(0..3, 0, (b, y) => b + 1 / (y - 2))))",
    "at_exit(() => fail(\"in handler\"))\nat_exit(() => exit(7))\nprint(\"main\")",
    "spawn(() => 1)",
];

/// More `-e` programs: exit codes the OS cuts to 8 bits, the exit handler
/// order, errors and exits inside handlers, standard input and arguments.
const MORE: &[&str] = &[
    "exit(300)",
    "exit(-1)",
    "exit(1099511627777)",
    "exit(9223372036854775807)",
    "exit(0)",
    "at_exit(() => print(1))\nat_exit(() => print(2))\nprint(0)",
    "at_exit(() => print(\"h\"))\n1 / 0",
    "at_exit(() => exit(3))\nexit(5)",
    "at_exit(() => exit(3))\n1 / 0",
    "at_exit(() => 1 / 0)\nexit(5)",
    "at_exit(() => at_exit(() => print(\"nested\")))\nprint(\"main\")",
    "at_exit(() => fail(\"a\"))\nat_exit(() => fail(\"b\"))",
    "print(args)",
    "print(read_line(), read_line())",
    "fn f(x) = map([1, 2], (y) => y / x)\nprint(f(0))",
    "x = )",
    "print(nope)",
];

/// The skip list: `<file name> <reason>`; the reason must be one of these,
/// with a word in the file that shows it.
const REASONS: &[(&str, &[&str])] = &[
    (
        "concurrency",
        &[
            "spawn", "await", "cancel", "channel", "send", "recv", "close", "sleep", "atom",
            "swap", "deref", "pmap", "fan_in", "retry",
        ],
    ),
    (
        "native-module",
        &[
            "import fs",
            "import json",
            "import time",
            "import regex",
            "import path",
            "import http",
            "import cli",
            "import _native.fs",
        ],
    ),
];

fn skip_list() -> Vec<(String, String)> {
    let text = include_str!("run_parity_skip.txt");
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let (Some(name), Some(reason), None) = (parts.next(), parts.next(), parts.next()) else {
            panic!("malformed skip line `{line}` (want `<file> <reason>`)");
        };
        out.push((name.to_string(), reason.to_string()));
    }
    out
}

#[test]
fn the_skip_list_names_real_files_for_real_reasons() {
    let dir = repo().join("tests/lang");
    for (name, reason) in skip_list() {
        let src = std::fs::read_to_string(dir.join(&name))
            .unwrap_or_else(|e| panic!("skip list: cannot read tests/lang/{name}: {e}"));
        let Some((_, words)) = REASONS.iter().find(|(r, _)| *r == reason) else {
            panic!("skip list: unknown reason `{reason}` for {name}");
        };
        assert!(
            words.iter().any(|w| src.contains(w)),
            "skip list: {name} does not use anything that is `{reason}`"
        );
    }
}

/// `# args:` and `# stdin:` lines, as Go `internal/golden` reads them.
fn directives(src: &str) -> (Vec<String>, String) {
    let (mut args, mut stdin) = (Vec::new(), String::new());
    for line in src.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("# args:") {
            args.extend(rest.split_whitespace().map(String::from));
        } else if let Some(rest) = line.strip_prefix("# stdin:") {
            stdin.push_str(rest.strip_prefix(' ').unwrap_or(rest));
            stdin.push('\n');
        }
    }
    (args, stdin)
}

/// Every `*.fung` directly in `dir` (relative to the repository root).
fn fung_files(dir: &str) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(repo().join(dir))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "fung"))
        .map(|p| format!("{dir}/{}", p.file_name().unwrap().to_string_lossy()))
        .collect();
    files.sort();
    files
}

/// One case: a label, the arguments and the standard input.
struct Case {
    label: String,
    args: Vec<OsString>,
    stdin: Option<Vec<u8>>,
}

fn cases() -> Vec<Case> {
    let skipped: Vec<String> = skip_list().into_iter().map(|(n, _)| n).collect();
    let mut out = Vec::new();
    for f in fung_files("tests/lang") {
        let name = f.rsplit('/').next().unwrap();
        if skipped.iter().any(|s| s == name) {
            continue;
        }
        let src = std::fs::read_to_string(repo().join(&f)).unwrap();
        let (args, stdin) = directives(&src);
        let mut a: Vec<OsString> = vec!["run".into(), f.clone().into()];
        a.extend(args.into_iter().map(OsString::from));
        out.push(Case {
            label: f,
            args: a,
            stdin: Some(stdin.into_bytes()),
        });
    }
    for f in fung_files("bench") {
        out.push(Case {
            label: f.clone(),
            args: vec!["run".into(), f.into()],
            stdin: None,
        });
    }
    for code in CASES.iter().take(CASES.len() - 1).chain(MORE) {
        out.push(Case {
            label: format!("-e {code:?}"),
            args: vec!["-e".into(), (*code).into(), "x".into(), "y z".into()],
            stdin: Some(b"one\ntwo".to_vec()),
        });
    }
    // The other ways to start a script, and usage errors.
    for args in [
        vec!["tests/lang/args.fung", "a", "b"],
        vec!["./tests/lang/arith.fung"],
        vec!["run"],
        vec!["-e"],
        vec!["run", "no/such/file.fung"],
        vec!["run", "tests"],
        vec!["frobnicate"],
    ] {
        out.push(Case {
            label: args.join(" "),
            args: args.into_iter().map(OsString::from).collect(),
            stdin: None,
        });
    }
    out
}

#[test]
fn runs_match_the_go_implementation() {
    let Some(go) = go_fung("run") else {
        eprintln!("skipping run parity: `go` is not available");
        return;
    };
    let rust = rust_fung();
    let cases = cases();
    let mut failures = Vec::new();
    for c in &cases {
        let g = run_with_stdin(&go, &c.args, c.stdin.as_deref());
        let r = run_with_stdin(&rust, &c.args, c.stdin.as_deref());
        if let Some(what) = diff(&g, &r) {
            failures.push(format!("{}: {what}", c.label));
        }
    }
    let _ = std::fs::remove_file(&go);
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

/// Concurrency is not part of the Rust port yet: Go runs the task, the
/// Rust port reports E0805 at the call.
#[test]
fn spawn_is_not_supported_yet() {
    let code = CASES[CASES.len() - 1];
    let o = std::process::Command::new(rust_fung())
        .args(["-e", code])
        .output()
        .unwrap();
    let (out, err) = (o.stdout, o.stderr);
    assert_eq!(String::from_utf8_lossy(&out), "");
    assert_eq!(
        String::from_utf8_lossy(&err),
        "error[E0805]: not supported yet: spawn\n  --> -e:1:6\n  |\n1 | spawn(() => 1)\n  |      ^\n  |\n call path (innermost first):\n   -e:1       <entry>\n"
    );
    assert_eq!(o.status.code(), Some(1));
}
