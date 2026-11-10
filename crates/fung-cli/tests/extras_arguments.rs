//! One wrong argument for every public function written in fung
//! (`extras/*.fung`), against `expected/extras/arguments.txt` (SP18):
//! the error names the argument the user gave, as a native's would, and
//! comes before any side effect (nothing printed, no file made).
//!
//! The table must cover every public `fn` of the extras: a new one fails
//! `every_public_extras_function_has_a_case` until it has a case here.

mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use common::expected::{self, Entry};
use common::fung_bin;

/// (module, function, program, error code, argument the error names).
/// Each program runs with `fung -e` in an empty directory.
type Case = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

const CASES: &[Case] = &[
    // cli
    (
        "cli",
        "parse",
        r#"import cli
print(cli.parse("x"))"#,
        "E0403",
        "argv",
    ),
    (
        "cli",
        "parse",
        r#"import cli
print(cli.parse([1]))"#,
        "E0403",
        "argv",
    ),
    (
        "cli",
        "parse",
        r#"import cli
print(cli.parse([], bools: [1]))"#,
        "E0403",
        "bools",
    ),
    // conc
    (
        "conc",
        "fan_in",
        r#"import conc
scope(s => conc.fan_in(s, 1))"#,
        "E0403",
        "rxs",
    ),
    (
        "conc",
        "fan_in",
        r#"import conc
conc.fan_in(1, [])"#,
        "E0403",
        "s",
    ),
    (
        "conc",
        "pmap",
        r#"import conc
print(conc.pmap([1], x => print(x), limit: "a"))"#,
        "E0403",
        "limit",
    ),
    (
        "conc",
        "pmap",
        r#"import conc
print(conc.pmap([1, 2, 3, 4], x => print(x), limit: 2.5))"#,
        "E0403",
        "limit",
    ),
    (
        "conc",
        "pmap",
        r#"import conc
print(conc.pmap([1], x => print(x), limit: 0))"#,
        "E0403",
        "limit",
    ),
    (
        "conc",
        "pmap",
        r#"import conc
print(conc.pmap([1], x => print(x), limit: -1))"#,
        "E0403",
        "limit",
    ),
    (
        "conc",
        "pmap",
        r#"import conc
print(conc.pmap([1], 1))"#,
        "E0403",
        "f",
    ),
    (
        "conc",
        "retry",
        r#"import conc
print(conc.retry(() => print(1), times: "2"))"#,
        "E0403",
        "times",
    ),
    (
        "conc",
        "retry",
        r#"import conc
print(conc.retry(() => print(1), times: 0))"#,
        "E0403",
        "times",
    ),
    (
        "conc",
        "retry",
        r#"import conc
print(conc.retry(() => print(1), times: -1))"#,
        "E0403",
        "times",
    ),
    (
        "conc",
        "retry",
        r#"import conc
print(conc.retry(() => print(1), times: 2, delay: 5))"#,
        "E0403",
        "delay",
    ),
    (
        "conc",
        "retry",
        r#"import conc
print(conc.retry(() => print(1), times: 2, delay: -1s))"#,
        "E0403",
        "delay",
    ),
    (
        "conc",
        "retry",
        r#"import conc
print(conc.retry(1))"#,
        "E0403",
        "f",
    ),
    // fs
    (
        "fs",
        "read_lines",
        r#"import fs
print(fs.read_lines(1))"#,
        "E0403",
        "path",
    ),
    (
        "fs",
        "with_open",
        r#"import fs
print(fs.with_open("made.txt", 1, mode: 'write))"#,
        "E0403",
        "f",
    ),
    (
        "fs",
        "with_open",
        r#"import fs
print(fs.with_open(1, f => print(f)))"#,
        "E0403",
        "path",
    ),
    (
        "fs",
        "with_open",
        r#"import fs
print(fs.with_open("made.txt", f => print(f), mode: "write"))"#,
        "E0403",
        "mode",
    ),
    (
        "fs",
        "with_open",
        r#"import fs
print(fs.with_open("made.txt", f => print(f), mode: 'write, perm: "644"))"#,
        "E0403",
        "perm",
    ),
    (
        "fs",
        "with_temp_dir",
        r#"import fs
print(fs.with_temp_dir(1))"#,
        "E0403",
        "f",
    ),
    // http
    (
        "http",
        "get",
        r#"import http
print(http.get("http://127.0.0.1:1/", max_body: -1))"#,
        "E0403",
        "max_body",
    ),
    (
        "http",
        "post",
        r#"import http
print(http.post("http://127.0.0.1:1/", 1))"#,
        "E0403",
        "body",
    ),
    (
        "http",
        "request",
        r#"import http
print(http.request("GET", "http://127.0.0.1:1/", timeout: -1s))"#,
        "E0403",
        "timeout",
    ),
    (
        "http",
        "serve",
        r#"import http
print(http.serve("127.0.0.1:0", 1))"#,
        "E0403",
        "handler",
    ),
    // prelude
    ("prelude", "is_empty", "print(is_empty(1))", "E0506", ""),
    ("prelude", "zip", "print(zip(1, [1]))", "E0506", ""),
    ("prelude", "first", "print(first(1))", "E0506", ""),
    ("prelude", "last", "print(last(1))", "E0506", ""),
    ("prelude", "take", "print(take([1], -1))", "E0403", "n"),
    ("prelude", "drop", "print(drop([1], \"1\"))", "E0403", "n"),
    ("prelude", "contains", "print(contains(1, 1))", "E0512", ""),
    (
        "prelude",
        "pad_left",
        "print(pad_left(\"a\", -3))",
        "E0403",
        "n",
    ),
    (
        "prelude",
        "pad_right",
        "print(pad_right(\"a\", 3, \"\"))",
        "E0403",
        "pad",
    ),
    ("prelude", "is_ok", "print(is_ok())", "E0407", "x"),
    ("prelude", "is_err", "print(is_err())", "E0407", "x"),
    (
        "prelude",
        "until",
        "print(until(1, 2, x => true))",
        "E0403",
        "f",
    ),
    (
        "prelude",
        "iterate",
        "print(iterate(1, x => x, -3))",
        "E0403",
        "n",
    ),
];

/// How many cases the table holds.
const CASE_COUNT: usize = 39;

fn run(bin: &Path, dir: &Path, src: &str) -> (String, String, Option<i32>) {
    let o = common::output(
        Command::new(bin)
            .args(["-e", src])
            .current_dir(dir)
            .env("TZ", common::TZ),
    )
    .unwrap();
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        o.status.code(),
    )
}

/// Each case in a directory of its own, which must stay empty.
fn entries(bin: &Path) -> (Vec<Entry>, Vec<String>) {
    let base = std::env::temp_dir().join(format!("fung-extras-args-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let indexed: Vec<(usize, &Case)> = CASES.iter().enumerate().collect();
    let results = common::par_map(&indexed, |(i, (module, name, src, code, arg))| {
        let dir = base.join(i.to_string());
        std::fs::create_dir_all(&dir).unwrap();
        let (out, err, status) = run(bin, &dir, src);
        let mut bad = Vec::new();
        let label = format!(
            "{module}.{name} {}",
            expected::short_label(&format!("{src:?}"))
        );
        if !out.is_empty() {
            bad.push(format!("{label}: printed {out:?} before failing"));
        }
        if !err.starts_with(&format!("error[{code}]")) {
            bad.push(format!("{label}: want {code}, got {err:?}"));
        }
        if !arg.is_empty() && !err.contains(&format!("argument `{arg}`")) {
            bad.push(format!(
                "{label}: want the argument `{arg}` named, got {err:?}"
            ));
        }
        if std::fs::read_dir(&dir).unwrap().next().is_some() {
            bad.push(format!("{label}: left a file in its directory"));
        }
        let entry = Entry::new(label)
            .field("stdout", out)
            .field("stderr", err)
            .field("exit", status.map_or("signal".into(), |c| c.to_string()));
        (entry, bad)
    });
    let _ = std::fs::remove_dir_all(&base);
    let mut got = Vec::new();
    let mut bad = Vec::new();
    for (e, b) in results {
        got.push(e);
        bad.extend(b);
    }
    (got, bad)
}

#[test]
fn a_wrong_argument_names_itself_before_any_side_effect() {
    assert_eq!(CASES.len(), CASE_COUNT, "cases");
    let (got, bad) = entries(&fung_bin());
    assert!(bad.is_empty(), "{} wrong:\n{}", bad.len(), bad.join("\n"));
    expected::verify(
        "extras/arguments.txt",
        "One wrong argument for each public function written in fung (crates/fung-cli/tests/extras_arguments.rs), each run with `fung -e`.\n\
         Update: FUNG_EXPECTED=bless cargo test -p fung-cli --test extras_arguments",
        64 * 1024,
        &got,
    );
}

/// The public `fn`s of `extras/*.fung` (a name not starting with `_`).
fn public_functions() -> BTreeSet<(String, String)> {
    let mut names = BTreeSet::new();
    let dir = common::repo().join("extras");
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("fung") {
            continue;
        }
        let module = path.file_stem().unwrap().to_str().unwrap().to_string();
        for line in std::fs::read_to_string(&path).unwrap().lines() {
            let Some(rest) = line.strip_prefix("fn ") else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.starts_with('_') {
                names.insert((module.clone(), name));
            }
        }
    }
    names
}

#[test]
fn every_public_extras_function_has_a_case() {
    let covered: BTreeSet<(String, String)> = CASES
        .iter()
        .map(|(m, n, ..)| (m.to_string(), n.to_string()))
        .collect();
    let missing: Vec<String> = public_functions()
        .difference(&covered)
        .map(|(m, n)| format!("{m}.{n}"))
        .collect();
    assert!(missing.is_empty(), "no case for: {}", missing.join(", "));
}
