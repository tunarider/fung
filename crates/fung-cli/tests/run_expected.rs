//! `fung run` and `fung -e` against `expected/run/runs.txt`: every
//! `tests/lang` golden file (with its `# args:` and `# stdin:` lines) but
//! those marked `# only:`, the benchmarks, handwritten `-e` programs, and
//! the other ways to start a script. The concurrent ones also run
//! `REPEAT` times in the release build (`concurrent_runs_are_stable`).

mod common;

use std::ffi::OsString;

use std::path::Path;

use common::expected::{self, Entry};
use common::{Run, par_map, repo, run_with_stdin, rust_fung};

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

/// Concurrent programs (stage 3a), compared once in the run comparison
/// and `REPEAT` times in the release build (`concurrent_runs_are_stable`).
/// Each one's output must not depend on how the tasks are scheduled. One
/// that starts with `import conc` runs in Go with the prelude names
/// (`go_source`).
const CONCURRENT: &[&str] = &[
    // Tasks.
    "import conc\nprint(await(spawn(() => 1 + 1)), await(spawn(() => 1 / 0)), conc.pmap([1, 2, 3], x => x * x, limit: 2))",
    "t = spawn(() => sleep(10s))\ncancel(t)\nu = spawn(() => sleep(10s))\nprint(await(t), await(u, timeout: 1ms))\ncancel(u)",
    "t = spawn(fn(): spawn(() => sleep(10s)) ;;)\nprint(await(await(t)))",
    "import conc\nprint(conc.retry(() => Err(1), times: 2, delay: 1ms))",
    // Channels.
    "ch = channel()\nspawn(fn(): each(1..=5, x => send(ch, x)); close(ch) ;;)\nprint(fold(ch, 0, (a, x) => a + x))",
    "buf = channel(2)\nsend(buf, \"a\")\nclose(buf)\nprint(recv(buf), recv(buf), send(buf, \"b\"), recv(channel(), timeout: 5ms))",
    "ch = channel()\nt = spawn(() => each(1..=5, x => send(ch, x)))\ncancel(t)\nprint(await(t))",
    "print(channel(-1))",
    // No atoms: stage 3b replaced them with agents in Rust only, so they
    // cannot be compared (Go's golden files keep testing its atoms;
    // `tests/lang/agents.fung` and `vm/agent_tests.rs` test agents).
    // Exit rules: an exit from any task cancels every other
    // task, wherever it waits; the handlers run once.
    "at_exit(() => print(\"bye\"))\nspawn(fn(): sleep(10ms); exit(5) ;;)\nsleep(10s)\nprint(\"never\")",
    "at_exit(() => print(\"bye\"))\nch = channel()\nspawn(() => recv(ch))\nspawn(fn(): sleep(10ms); exit(3) ;;)\nrecv(channel())",
    "t = spawn(() => each(1..=3, x => sleep(10s)))\nspawn(fn(): sleep(10ms); cancel(t) ;;)\nprint(await(t))",
    "t = spawn(() => sleep(10s))\nfail(\"entry\")",
    "import conc\nprint(conc.retry(() => Err(1), times: 2, delay: 1ms), conc.pmap([1, 2, 3], x => x * x, limit: 2), conc.fan_in([]) |> map(x => x))",
    // `tests/lang/conc_helpers.fung` (Go) and `conc_helpers_rust.fung`.
    "import conc\na = channel(2)\nb = channel(2)\neach([1, 2], x => send(a, x))\nclose(a)\nsend(b, 3)\nclose(b)\nprint(conc.fan_in([a, b]) |> map(x => x) |> sort, conc.pmap([1, 2, 3], x => x * x, limit: 2), conc.pmap([], x => x))\nprint(conc.retry(() => Err(\"never\"), times: 2))",
    "ts = for i in 0..200: spawn(() => i) ;;\nprint(fold(ts, 0, (a, t) => a + await(t)))",
    "spawn(fn(): sleep(10ms); exit(4) ;;)\nsleep(10s)\nprint(\"main\")",
    "at_exit(() => print(\"h\"))\nspawn(() => each(1..=3, x => x == 2 then exit(6) else print(x)))\nsleep(10s)",
    "ch = channel()\nspawn(fn(): recv(ch); exit(2) ;;)\nat_exit(() => print(\"once\"))\neach(1..=3, x => send(ch, x))",
    // Standard input and processes from tasks.
    "ts = for _ in 0..2: spawn(() => read_line()) ;;\nprint(sort(for t in ts: await(t)! ;;))",
    "print(await(spawn(() => sh(\"echo hi; sleep 0.05; echo there\"))))",
];

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

/// True when the file carries a `# only:` line, so only one implementation
/// runs it and the two cannot be compared (the value is checked by Go's
/// `internal/golden`).
fn only_one_implementation(src: &str) -> bool {
    src.lines().any(|l| l.trim().starts_with("# only:"))
}

#[test]
fn an_only_file_is_not_compared() {
    assert!(only_one_implementation("# only: rust\nprint(1)\n"));
    assert!(only_one_implementation("print(1)\n  # only: go\n"));
    assert!(!only_one_implementation("print(1)\n# expect: 1\n"));
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

/// Where a case comes from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A `tests/lang` golden file: it always prints something (each has
    /// a `# expect` header).
    Lang,
    Bench,
    Other,
}

/// One case: where it comes from, a label, the arguments and the
/// standard input.
struct Case {
    kind: Kind,
    label: String,
    args: Vec<OsString>,
    /// The reference implementation's arguments where they differ
    /// (`go_source`; recording only, task 5 removes it).
    go_args: Option<Vec<OsString>>,
    stdin: Option<Vec<u8>>,
}

impl Case {
    /// The arguments for Go (`go`) or for Rust.
    fn args_for(&self, go: bool) -> &[OsString] {
        match &self.go_args {
            Some(a) if go => a,
            _ => &self.args,
        }
    }
}

/// Recording only (stage 6b task 1; task 5 removes it).
/// The Go form of a `-e` program: one that starts with `import conc`
/// (the Rust-only module of stage 6a) calls Go's prelude names instead.
/// The import line stays as an empty line, so the lines keep their
/// numbers.
fn go_source(src: &str) -> Option<String> {
    let rest = src.strip_prefix("import conc\n")?;
    Some(format!("\n{}", rest.replace("conc.", "")))
}

#[test]
fn go_source_drops_the_conc_module() {
    assert_eq!(
        go_source("import conc\nprint(conc.pmap([1], f))").as_deref(),
        Some("\nprint(pmap([1], f))")
    );
    assert_eq!(go_source("print(1)"), None);
}

fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    for f in fung_files("tests/lang") {
        let src = std::fs::read_to_string(repo().join(&f)).unwrap();
        if only_one_implementation(&src) {
            continue;
        }
        let (args, stdin) = directives(&src);
        let mut a: Vec<OsString> = vec!["run".into(), f.clone().into()];
        a.extend(args.into_iter().map(OsString::from));
        out.push(Case {
            kind: Kind::Lang,
            label: f,
            args: a,
            go_args: None,
            stdin: Some(stdin.into_bytes()),
        });
    }
    for f in fung_files("bench") {
        out.push(Case {
            kind: Kind::Bench,
            label: f.clone(),
            args: vec!["run".into(), f.into()],
            go_args: None,
            stdin: None,
        });
    }
    for code in CASES.iter().chain(MORE).chain(CONCURRENT) {
        let e = |code: String| -> Vec<OsString> {
            vec!["-e".into(), code.into(), "x".into(), "y z".into()]
        };
        out.push(Case {
            kind: Kind::Other,
            label: format!("-e {code:?}"),
            args: e(code.to_string()),
            go_args: go_source(code).map(e),
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
            kind: Kind::Other,
            label: args.join(" "),
            args: args.into_iter().map(OsString::from).collect(),
            go_args: None,
            stdin: None,
        });
    }
    out
}

/// How many cases there are, by kind (a case must not go missing
/// unnoticed; change these together with the cases).
const LANG_CASES: usize = 132;
const BENCH_CASES: usize = 6;
const ALL_CASES: usize = 189;

#[test]
fn every_case_is_counted() {
    let cases = cases();
    let count = |k: Kind| cases.iter().filter(|c| c.kind == k).count();
    assert_eq!(count(Kind::Lang), LANG_CASES, "tests/lang files");
    assert_eq!(count(Kind::Bench), BENCH_CASES, "benchmarks");
    assert_eq!(cases.len(), ALL_CASES, "cases");
}

const RUNS: &str = "run/runs.txt";

/// A case's output as the expected-output file keeps it.
fn entry(c: &Case, r: &Run) -> Entry {
    let root = repo().to_string_lossy().into_owned();
    Entry::run(&c.label, r, &[(root.as_str(), "$REPO")])
}

#[test]
fn runs_match_the_expected_output() {
    let rust = rust_fung();
    let cases = cases();
    assert_eq!(cases.len(), ALL_CASES, "cases");
    // On a few workers: a case that leaves a task sleeping, like
    // `tests/lang/cancel.fung`, takes 10 s.
    let runs = |bin: &Path, reference: bool| -> Vec<Entry> {
        par_map(&cases, |c| {
            entry(
                c,
                &run_with_stdin(bin, c.args_for(reference), c.stdin.as_deref()),
            )
        })
    };
    let got = runs(&rust, false);
    // Recording (stage 6b task 1 only; task 5 removes it).
    let reference = expected::recording().then(|| {
        let bin = expected::reference_fung("run");
        let r = runs(&bin, true);
        let _ = std::fs::remove_file(&bin);
        r
    });
    // Two silent runs would compare equal for the wrong reason: every
    // golden file prints something.
    for (c, e) in cases.iter().zip(reference.as_deref().unwrap_or(&got)) {
        if c.kind == Kind::Lang {
            assert!(
                e.get("stdout") != Some(b"") || e.get("stderr") != Some(b""),
                "{}: printed nothing",
                c.label
            );
        }
    }
    expected::verify(
        RUNS,
        "`fung run` and `fung -e` (crates/fung-cli/tests/run_expected.rs).\n\
         $REPO stands for the repository. Update: FUNG_EXPECTED=bless cargo test -p fung-cli --test run_expected",
        64 * 1024,
        &got,
        reference.as_deref(),
    );
}

/// The concurrency builtins: a `tests/lang` file that calls one is a
/// concurrent case.
#[cfg(not(debug_assertions))]
const CONCURRENCY: &[&str] = &[
    "spawn", "await", "cancel", "channel", "send", "recv", "close", "sleep", "agent", "swap",
    "deref", "pmap", "fan_in", "retry",
];

/// Whether `src` calls `name`: `name(` not preceded by a name character
/// (so `closure(` or `resend(` do not count, nor do words in comments
/// without a parenthesis).
#[cfg(not(debug_assertions))]
fn calls(src: &str, name: &str) -> bool {
    let call = format!("{name}(");
    src.match_indices(&call).any(|(i, _)| {
        !src[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
    })
}

#[cfg(not(debug_assertions))]
#[test]
fn calls_finds_calls_only() {
    assert!(calls("x = spawn(f)", "spawn"));
    assert!(calls("spawn(f)", "spawn"));
    assert!(!calls("closure(1)", "close"));
    assert!(!calls("resend(1)", "send"));
    assert!(!calls("m.close(1)", "close"));
    assert!(!calls("# close the file", "close"));
    assert!(calls("x = agent(0)\n", "agent"));
}

/// How many times `concurrent_runs_are_stable` runs each case.
#[cfg(not(debug_assertions))]
const REPEAT: usize = 50;

/// How many concurrent cases there are.
#[cfg(not(debug_assertions))]
const CONCURRENT_CASES: usize = 26;

/// The concurrent cases: the `tests/lang` files that call a concurrency
/// builtin, and `CONCURRENT`.
#[cfg(not(debug_assertions))]
fn concurrent_cases() -> Vec<Case> {
    cases()
        .into_iter()
        .filter(|c| match c.kind {
            Kind::Lang => {
                let src = std::fs::read_to_string(repo().join(&c.label)).unwrap();
                CONCURRENCY.iter().any(|n| calls(&src, n))
            }
            Kind::Other => CONCURRENT
                .iter()
                .any(|code| c.label == format!("-e {code:?}")),
            Kind::Bench => false,
        })
        .collect()
}

/// Scheduling must not change what a concurrent program prints (release
/// build only, where tasks run fastest and interleave most): every
/// concurrent case runs `REPEAT` times, and every run must print its
/// expected output (`expected/run/runs.txt`).
///
/// Every case starts its first run, all cases at once. A case whose
/// first run ends within a second hands its other runs to `POOL` workers.
/// A case still running after a second only sleeps
/// (`tests/lang/cancel.fung` leaves a 10 s task), so all its other runs
/// start right then, in parallel; the whole test then takes little more
/// than that case.
#[cfg(not(debug_assertions))]
#[test]
fn concurrent_runs_are_stable() {
    use std::sync::Mutex;
    use std::sync::mpsc::{RecvTimeoutError, channel};
    use std::time::{Duration, Instant};
    const POOL: usize = 16;
    let rust = rust_fung();
    let rust = rust.as_path();
    let start = Instant::now();
    let cases = concurrent_cases();
    assert_eq!(cases.len(), CONCURRENT_CASES, "concurrent cases");
    let suite = expected::load(RUNS);
    let want = |c: &Case| {
        expected::find(&suite, &c.label)
            .unwrap_or_else(|| panic!("{}: no expected output in {RUNS}", c.label))
    };
    let failures = Mutex::new(Vec::new());
    let compare = |c: &Case, got: &Run| {
        if let Some(what) = expected::mismatch(want(c), &entry(c, got)) {
            failures
                .lock()
                .unwrap()
                .push(format!("{}: {what}", c.label));
        }
    };
    let one = |c: &Case| run_with_stdin(rust, &c.args, c.stdin.as_deref());
    let (jobs, queue) = channel::<&Case>();
    let queue = Mutex::new(queue);
    std::thread::scope(|s| {
        for _ in 0..POOL {
            s.spawn(|| {
                loop {
                    let job = queue.lock().unwrap().recv();
                    let Ok(c) = job else {
                        break;
                    };
                    compare(c, &one(c));
                }
            });
        }
        for c in &cases {
            let jobs = jobs.clone();
            let (compare, one) = (&compare, &one);
            s.spawn(move || {
                let (tx, first) = channel();
                std::thread::scope(|s| {
                    s.spawn(move || {
                        let _ = tx.send(one(c));
                    });
                    match first.recv_timeout(Duration::from_secs(1)) {
                        Ok(got) => {
                            compare(c, &got);
                            for _ in 1..REPEAT {
                                jobs.send(c).unwrap();
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            let runs: Vec<_> =
                                (1..REPEAT).map(|_| s.spawn(move || one(c))).collect();
                            compare(c, &first.recv().unwrap());
                            for h in runs {
                                compare(c, &h.join().unwrap());
                            }
                        }
                        Err(RecvTimeoutError::Disconnected) => panic!("the first run failed"),
                    }
                });
            });
        }
        drop(jobs);
    });
    let failures = failures.into_inner().unwrap();
    assert!(
        failures.is_empty(),
        "{} runs differ from the expected output:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        start.elapsed() < Duration::from_secs(30),
        "the repeat check took {:?}",
        start.elapsed()
    );
}
