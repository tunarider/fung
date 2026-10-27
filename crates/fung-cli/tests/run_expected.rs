//! `fung run` and `fung -e` against `expected/run/runs.txt`: every
//! `tests/lang` golden file (with its `# args:` and `# stdin:` lines), the
//! benchmarks, handwritten `-e` programs, and
//! the other ways to start a script. The concurrent ones also run
//! `REPEAT` times in the release build (`concurrent_runs_are_stable`).

mod common;

use std::ffi::OsString;

use common::expected::{self, Entry};
use common::{Run, fung_bin, par_map, repo, run_with_stdin};

const CASES: &[&str] = &[
    "fn f(n) = n == 0 then 1 / 0 else f(n - 1)\nfn g(x) = f(x) + 1\nprint(g(3))",
    "with(() => nil, fn(_): print([1, 2] + [3], {a: 1}, (1, \"x\"), Ok(2), 1..3, 1.5, 90m, 's, nil); exit(4) ;;, _ => print(\"bye\"))",
    "fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)\nprint(count(1000000, 0))",
    "fn d(n) = n == 0 then 0 else 1 + d(n - 1)\nprint(d(200000))",
    "print(fold(0..3, 0, (a, x) => a + fold(0..3, 0, (b, y) => b + 1 / (y - 2))))",
    "with(() => nil, _ => print(\"main\"), _ => fail(\"in release\"))",
    "scope(s => spawn(s, () => 1))",
];

/// More `-e` programs: exit codes the OS cuts to 8 bits, the order of
/// nested `with` releases, errors and exits inside releases, standard
/// input and arguments.
const MORE: &[&str] = &[
    "exit(300)",
    "exit(-1)",
    "exit(1099511627777)",
    "exit(9223372036854775807)",
    "exit(0)",
    "with(() => 1, _ => with(() => 2, _ => print(0), print), print)",
    "with(() => nil, _ => 1 / 0, _ => print(\"h\"))",
    "with(() => nil, _ => exit(5), _ => exit(3))",
    "with(() => nil, _ => 1 / 0, _ => exit(3))",
    "with(() => nil, _ => exit(5), _ => 1 / 0)",
    "with(() => nil, _ => fail(\"a\"), _ => fail(\"b\"))",
    "print(args)",
    "print(read_line(), read_line())",
    "fn f(x) = map([1, 2], (y) => y / x)\nprint(f(0))",
    "x = )",
    "print(nope)",
];

/// Concurrent programs (stage 3a), compared once in the run comparison
/// and `REPEAT` times in the release build (`concurrent_runs_are_stable`).
/// Each one's output must not depend on how the tasks are scheduled.
const CONCURRENT: &[&str] = &[
    // Tasks.
    "import conc\nsupervise(s => print(await(spawn(s, () => 1 + 1)), await(spawn(s, () => 1 / 0)), conc.pmap([1, 2, 3], x => x * x, limit: 2)), _ => nil)",
    "scope(fn(s):\n  t = spawn(s, () => sleep(10s))\n  cancel(t)\n  u = spawn(s, () => sleep(10s))\n  print(await(t), timeout(1ms, () => await(u)))\n  cancel(u)\n;;)",
    "(tx, rx) = channel(1)\nprint(scope(fn(s):\n  send(tx, spawn(s, () => sleep(10s)))\n  cancel(s)\n;;), await(recv(rx)!))",
    "import conc\nprint(conc.retry(() => Err(1), times: 2, delay: 1ms))",
    // Channels.
    "(tx, rx) = channel()\nscope(fn(s):\n  spawn(s, fn(): each(1..=5, x => send(tx, x)); close(tx) ;;)\n  print(fold(rx, 0, (a, x) => a + x))\n;;)",
    "(buf_tx, buf) = channel(2)\nsend(buf_tx, \"a\")\nclose(buf_tx)\n(_tx, never) = channel()\nprint(recv(buf), recv(buf), send(buf_tx, \"b\"), timeout(5ms, () => recv(never)))",
    "(tx, _rx) = channel()\nscope(fn(s):\n  t = spawn(s, () => each(1..=5, x => send(tx, x)))\n  cancel(t)\n  print(await(t))\n;;)",
    "print(channel(-1))",
    // Atoms: `tests/lang/atoms.fung`.
    // Exit rules: an exit from any task cancels every other
    // task, wherever it waits; each `with` is released once.
    "with(() => nil, _ => scope(fn(s):\n  spawn(s, fn(): sleep(10ms); exit(5) ;;)\n  sleep(10s)\n  print(\"never\")\n;;), _ => print(\"bye\"))",
    "(_tx, rx) = channel()\nwith(() => nil, _ => scope(fn(s):\n  spawn(s, () => recv(rx))\n  spawn(s, fn(): sleep(10ms); exit(3) ;;)\n  recv(rx)\n;;), _ => print(\"bye\"))",
    "scope(fn(s):\n  t = spawn(s, () => each(1..=3, x => sleep(10s)))\n  spawn(s, fn(): sleep(10ms); cancel(t) ;;)\n  print(await(t))\n;;)",
    "scope(fn(s):\n  spawn(s, () => sleep(10s))\n  fail(\"entry\")\n;;)",
    "import conc\nprint(conc.retry(() => Err(1), times: 2, delay: 1ms), conc.pmap([1, 2, 3], x => x * x, limit: 2), scope(s => conc.fan_in(s, []) |> map(x => x)))",
    // `tests/lang/conc_helpers.fung`.
    "import conc\n(a_tx, a) = channel(2)\n(b_tx, b) = channel(2)\neach([1, 2], x => send(a_tx, x))\nclose(a_tx)\nsend(b_tx, 3)\nclose(b_tx)\nprint(scope(s => conc.fan_in(s, [a, b]) |> map(x => x) |> sort), conc.pmap([1, 2, 3], x => x * x, limit: 2), conc.pmap([], x => x))\nprint(conc.retry(() => Err(\"never\"), times: 2))",
    "print(scope(fn(s):\n  ts = for i in 0..200: spawn(s, () => i) ;;\n  fold(ts, 0, (a, t) => a + await(t))\n;;))",
    "scope(fn(s):\n  spawn(s, fn(): sleep(10ms); exit(4) ;;)\n  sleep(10s)\n  print(\"main\")\n;;)",
    "with(() => nil, _ => scope(fn(s):\n  spawn(s, () => each(1..=3, x => x == 2 then exit(6) else print(x)))\n  sleep(10s)\n;;), _ => print(\"h\"))",
    "(tx, rx) = channel()\nscope(fn(s):\n  spawn(s, fn(): recv(rx); exit(2) ;;)\n  with(() => nil, _ => each(1..=3, x => send(tx, x)), _ => print(\"once\"))\n;;)",
    // Standard input and processes from tasks.
    "print(scope(fn(s):\n  ts = for _ in 0..2: spawn(s, () => read_line()) ;;\n  sort(for t in ts: await(t)! ;;)\n;;))",
    "print(scope(s => await(spawn(s, () => sh(\"echo hi; sleep 0.05; echo there\")))))",
];

/// `# args:` and `# stdin:` lines, as the golden runner reads them.
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
    stdin: Option<Vec<u8>>,
}

fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    for f in fung_files("tests/lang") {
        let src = std::fs::read_to_string(repo().join(&f)).unwrap();
        let (args, stdin) = directives(&src);
        let mut a: Vec<OsString> = vec!["run".into(), f.clone().into()];
        a.extend(args.into_iter().map(OsString::from));
        out.push(Case {
            kind: Kind::Lang,
            label: f,
            args: a,
            stdin: Some(stdin.into_bytes()),
        });
    }
    for f in fung_files("bench") {
        out.push(Case {
            kind: Kind::Bench,
            label: f.clone(),
            args: vec!["run".into(), f.into()],
            stdin: None,
        });
    }
    for code in CASES.iter().chain(MORE).chain(CONCURRENT) {
        out.push(Case {
            kind: Kind::Other,
            label: format!("-e {code:?}"),
            args: vec!["-e".into(), code.into(), "x".into(), "y z".into()],
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
            stdin: None,
        });
    }
    out
}

/// How many cases there are, by kind (a case must not go missing
/// unnoticed; change these together with the cases).
const LANG_CASES: usize = 205;
const BENCH_CASES: usize = 10;
const ALL_CASES: usize = 265;

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
    let bin = fung_bin();
    let cases = cases();
    assert_eq!(cases.len(), ALL_CASES, "cases");
    // On a few workers: some cases wait on sleeps and timeouts.
    let got: Vec<Entry> = par_map(&cases, |c| {
        entry(c, &run_with_stdin(&bin, &c.args, c.stdin.as_deref()))
    });
    // A silent run would match an empty expected output for the wrong
    // reason: every golden file prints something.
    for (c, e) in cases.iter().zip(&got) {
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
    );
}

/// The concurrency builtins and the `conc` module's helpers (called by
/// their module name): a `tests/lang` file that calls one is a concurrent
/// case.
#[cfg(not(debug_assertions))]
const CONCURRENCY: &[&str] = &[
    "scope",
    "supervise",
    "timeout",
    "with",
    "spawn",
    "await",
    "cancel",
    "channel",
    "send",
    "recv",
    "close",
    "sleep",
    "atom",
    "swap",
    "deref",
    "reset",
    "conc.pmap",
    "conc.fan_in",
    "conc.retry",
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
    assert!(calls("x = spawn(s, f)", "spawn"));
    assert!(calls("spawn(s, f)", "spawn"));
    assert!(!calls("closure(1)", "close"));
    assert!(!calls("resend(1)", "send"));
    assert!(!calls("m.close(1)", "close"));
    assert!(!calls("# close the file", "close"));
    assert!(calls("x = atom(0)\n", "atom"));
    assert!(calls("conc.pmap(xs, f)", "conc.pmap"));
    assert!(!calls("x.conc.pmap(xs, f)", "conc.pmap"));
}

/// How many times `concurrent_runs_are_stable` runs each case.
#[cfg(not(debug_assertions))]
const REPEAT: usize = 50;

/// How many concurrent cases there are.
#[cfg(not(debug_assertions))]
const CONCURRENT_CASES: usize = 60;

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
/// A case still running after a second only sleeps, so all its other
/// runs start right then, in parallel; the whole test then takes little
/// more than that case.
#[cfg(not(debug_assertions))]
#[test]
fn concurrent_runs_are_stable() {
    use std::sync::Mutex;
    use std::sync::mpsc::{RecvTimeoutError, channel};
    use std::time::{Duration, Instant};
    const POOL: usize = 16;
    let bin = fung_bin();
    let bin = bin.as_path();
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
    let one = |c: &Case| run_with_stdin(bin, &c.args, c.stdin.as_deref());
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
