//! The accumulation and tail-recursion shapes that would go quadratic with
//! full copies must finish 20,000 elements in well under 0.3 s, as in the
//! Go suite (tests/perf_test.go). Release builds only: a debug build is
//! too slow to say anything about the data structures.
#![cfg(not(debug_assertions))]

use std::process::Command;
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_millis(300);

/// `(name, source, expected output)`.
const CASES: &[(&str, &str, &str)] = &[
    (
        "map put fold",
        "m = fold(0..20000, {}, (acc, i) => put(acc, str(i), i))\nprint(len(m))",
        "20000\n",
    ),
    (
        "map put again",
        "m = fold(0..20000, {}, (acc, i) => put(acc, str(i % 5000), i))\nprint(len(m))",
        "5000\n",
    ),
    (
        "list append",
        "xs = fold(0..20000, [], (acc, i) => acc + [i])\nprint(len(xs))",
        "20000\n",
    ),
    (
        "rest pattern",
        "fn count(xs, acc) = match xs:\n  [] => acc\n  [_, ...rest] => count(rest, acc + 1)\n;;\nprint(count(map(0..20000, x => x), 0))",
        "20000\n",
    ),
    (
        "slice tail",
        "fn go(ys, acc) = len(ys) == 0 then acc else go(ys[1..], acc + 1)\nprint(go(map(0..20000, x => x), 0))",
        "20000\n",
    ),
];

/// Runs `src` with `fung -e` and returns its standard output and the time
/// the whole process took.
fn run(src: &str) -> (String, Duration) {
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_fung"))
        .args(["-e", src])
        .output()
        .unwrap();
    let took = start.elapsed();
    assert!(
        out.status.success(),
        "{src}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    (String::from_utf8_lossy(&out.stdout).into_owned(), took)
}

#[test]
fn collections_stay_linear() {
    let mut slow = Vec::new();
    for (name, src, want) in CASES {
        run(src); // warm-up: the first run pays for loading the binary
        let (out, took) = run(src);
        eprintln!("{name:<14} {took:?}");
        assert_eq!(out, *want, "{name}");
        if took > LIMIT {
            slow.push(format!("{name} took {took:?}"));
        }
    }
    assert!(slow.is_empty(), "{}", slow.join("\n"));
}
