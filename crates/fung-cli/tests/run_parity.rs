//! Runs compared with the Go implementation: every `tests/lang` golden file
//! (with its `# args:` and `# stdin:` lines) except the skip list, the
//! benchmarks, and handwritten `-e` programs.

mod common;

use std::ffi::OsString;

use std::path::Path;

use common::{Run, diff, go_fung, repo, run_with_stdin, rust_fung, skip_list};

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
/// Each one's output must not depend on how the tasks are scheduled.
const CONCURRENT: &[&str] = &[
    // Tasks.
    "print(await(spawn(() => 1 + 1)), await(spawn(() => 1 / 0)), pmap([1, 2, 3], x => x * x, limit: 2))",
    "t = spawn(() => sleep(10s))\ncancel(t)\nu = spawn(() => sleep(10s))\nprint(await(t), await(u, timeout: 1ms))\ncancel(u)",
    "t = spawn(fn(): spawn(() => sleep(10s)) ;;)\nprint(await(await(t)))",
    "print(retry(() => Err(1), times: 2, delay: 1ms))",
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
    "print(retry(() => Err(1), times: 2, delay: 1ms), pmap([1, 2, 3], x => x * x, limit: 2), fan_in([]) |> map(x => x))",
    "ts = for i in 0..200: spawn(() => i) ;;\nprint(fold(ts, 0, (a, t) => a + await(t)))",
    "spawn(fn(): sleep(10ms); exit(4) ;;)\nsleep(10s)\nprint(\"main\")",
    "at_exit(() => print(\"h\"))\nspawn(() => each(1..=3, x => x == 2 then exit(6) else print(x)))\nsleep(10s)",
    "ch = channel()\nspawn(fn(): recv(ch); exit(2) ;;)\nat_exit(() => print(\"once\"))\neach(1..=3, x => send(ch, x))",
    // Standard input and processes from tasks.
    "ts = for _ in 0..2: spawn(() => read_line()) ;;\nprint(sort(for t in ts: await(t)! ;;))",
    "print(await(spawn(() => sh(\"echo hi; sleep 0.05; echo there\"))))",
];

/// The skip list: `<file name> <reason>`; the reason must be one of
/// these, shown by a call of a concurrency builtin or an import of a
/// module stage 2 does not have.
const CONCURRENCY: &[&str] = &[
    "spawn", "await", "cancel", "channel", "send", "recv", "close", "sleep", "agent", "swap",
    "deref", "pmap", "fan_in", "retry",
];
const NATIVE_MODULES: &[&str] = &["http"];

/// Whether `src` calls `name`: `name(` not preceded by a name character
/// (so `closure(` or `resend(` do not count, nor do words in comments
/// without a parenthesis).
fn calls(src: &str, name: &str) -> bool {
    let call = format!("{name}(");
    src.match_indices(&call).any(|(i, _)| {
        !src[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
    })
}

/// Whether a line of `src` is `import <module>` (with anything after).
fn imports(src: &str, module: &str) -> bool {
    src.lines().any(|l| {
        let mut w = l.split_whitespace();
        w.next() == Some("import") && w.next() == Some(module)
    })
}

fn shows_reason(src: &str, reason: &str) -> Option<bool> {
    match reason {
        "concurrency" => Some(CONCURRENCY.iter().any(|n| calls(src, n))),
        "native-module" => Some(NATIVE_MODULES.iter().any(|m| imports(src, m))),
        _ => None,
    }
}

#[test]
fn the_reason_checks_match_calls_and_imports_only() {
    assert!(calls("x = spawn(f)", "spawn"));
    assert!(calls("spawn(f)", "spawn"));
    assert!(!calls("closure(1)", "close"));
    assert!(!calls("resend(1)", "send"));
    assert!(!calls("m.close(1)", "close"));
    assert!(!calls("# close the file", "close"));
    assert!(imports("import json\n", "json"));
    // `cli` is pure fung and runs in the Rust port: not a skip reason;
    // nor are `fs` and `path` since stage 4a.
    assert_eq!(shows_reason("import cli\n", "native-module"), Some(false));
    assert_eq!(
        shows_reason("import fs\nimport path\n", "native-module"),
        Some(false)
    );
    assert!(!imports("import jsonx\n", "json"));
    assert!(!imports("# import json is not here\n", "json"));
}

#[test]
fn the_skip_list_names_real_files_for_real_reasons() {
    let dir = repo().join("tests/lang");
    for (name, reason) in skip_list() {
        let src = std::fs::read_to_string(dir.join(&name))
            .unwrap_or_else(|e| panic!("skip list: cannot read tests/lang/{name}: {e}"));
        let Some(shown) = shows_reason(&src, &reason) else {
            panic!("skip list: unknown reason `{reason}` for {name}");
        };
        assert!(
            shown,
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
    /// A `tests/lang` golden file: Go always prints something for it
    /// (each has a `# expect` header).
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
    let skipped: Vec<String> = skip_list().into_iter().map(|(n, _)| n).collect();
    let mut out = Vec::new();
    for f in fung_files("tests/lang") {
        let name = f.rsplit('/').next().unwrap();
        if skipped.iter().any(|s| s == name) {
            continue;
        }
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
            kind: Kind::Other,
            label: args.join(" "),
            args: args.into_iter().map(OsString::from).collect(),
            stdin: None,
        });
    }
    out
}

/// The comparison cannot pass by comparing nothing: a broken file walk or
/// an overgrown skip list fails here. The minimums sit just below the
/// counts when they were set (133 `tests/lang` files, 6 benchmarks, 189
/// cases in all, after stage 4c added the `regex` files); raise them as
/// cases are added.
#[test]
fn enough_cases_are_compared() {
    let cases = cases();
    let count = |k: Kind| cases.iter().filter(|c| c.kind == k).count();
    let (lang, bench) = (count(Kind::Lang), count(Kind::Bench));
    assert!(lang >= 132, "only {lang} tests/lang files are compared");
    assert!(bench >= 5, "only {bench} benchmarks are compared");
    assert!(
        cases.len() >= 188,
        "only {} cases are compared",
        cases.len()
    );
}

/// Runs `go` and `rust` on the same case at the same time (a case that
/// leaves a task sleeping, like `tests/lang/cancel.fung`, takes 10 s in
/// both).
fn run_both(go: &Path, rust: &Path, c: &Case) -> (Run, Run) {
    std::thread::scope(|s| {
        let g = s.spawn(|| run_with_stdin(go, &c.args, c.stdin.as_deref()));
        let r = run_with_stdin(rust, &c.args, c.stdin.as_deref());
        (g.join().unwrap(), r)
    })
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
        let (g, r) = run_both(&go, &rust, c);
        // Two silent runs would compare equal for the wrong reason.
        if c.kind == Kind::Lang && g.0.is_empty() && g.1.is_empty() {
            failures.push(format!("{}: Go printed nothing", c.label));
            continue;
        }
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

/// How many times `concurrent_runs_are_stable` runs each case with each
/// implementation.
#[cfg(not(debug_assertions))]
const REPEAT: usize = 50;

/// The concurrent cases: the `tests/lang` files that call a concurrency
/// builtin, and `CONCURRENT`.
#[cfg(not(debug_assertions))]
fn concurrent_cases() -> Vec<Case> {
    cases()
        .into_iter()
        .filter(|c| match c.kind {
            Kind::Lang => {
                let src = std::fs::read_to_string(repo().join(&c.label)).unwrap();
                shows_reason(&src, "concurrency") == Some(true)
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
/// concurrent case runs `REPEAT` times with Go and `REPEAT` times with
/// Rust, and every run must equal Go's first. A Go run that differs from
/// Go's first means the case itself depends on scheduling.
///
/// Every case starts with Go's first run, all cases at once. A case whose
/// first run ends within a second hands its other runs to `POOL` workers.
/// A case still running after a second only sleeps
/// (`tests/lang/cancel.fung` leaves a 10 s task), so all its other runs
/// start right then, in parallel, and are compared when Go's first ends;
/// the whole test then takes little more than that case.
#[cfg(not(debug_assertions))]
#[test]
fn concurrent_runs_are_stable() {
    use std::sync::mpsc::{RecvTimeoutError, channel};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    const POOL: usize = 16;
    let Some(go) = go_fung("repeat") else {
        eprintln!("skipping the repeat check: `go` is not available");
        return;
    };
    let rust = rust_fung();
    let (go, rust) = (go.as_path(), rust.as_path());
    let start = Instant::now();
    let cases = concurrent_cases();
    // One below the 26 cases there were after stage 3b left the atom
    // cases to Go.
    assert!(cases.len() >= 25, "only {} concurrent cases", cases.len());
    let failures = Mutex::new(Vec::new());
    let compare = |c: &Case, want: &Run, got: &Run, who: &str| {
        if let Some(what) = diff(want, got) {
            failures
                .lock()
                .unwrap()
                .push(format!("{} ({who}): {what}", c.label));
        }
    };
    // The other runs of a case: `REPEAT` with Rust, `REPEAT - 1` more with Go.
    let others = || {
        (0..2 * REPEAT - 1).map(|i| {
            if i < REPEAT {
                (rust, "rust")
            } else {
                (go, "go")
            }
        })
    };
    let (jobs, queue) = channel::<(&Case, Arc<Run>, &Path, &str)>();
    let queue = Mutex::new(queue);
    std::thread::scope(|s| {
        for _ in 0..POOL {
            s.spawn(|| {
                loop {
                    let job = queue.lock().unwrap().recv();
                    let Ok((c, want, bin, who)) = job else {
                        break;
                    };
                    let got = run_with_stdin(bin, &c.args, c.stdin.as_deref());
                    compare(c, &want, &got, who);
                }
            });
        }
        for c in &cases {
            let jobs = jobs.clone();
            let (compare, others) = (&compare, &others);
            s.spawn(move || {
                let (tx, first) = channel();
                std::thread::scope(|s| {
                    s.spawn(move || {
                        let _ = tx.send(run_with_stdin(go, &c.args, c.stdin.as_deref()));
                    });
                    match first.recv_timeout(Duration::from_secs(1)) {
                        Ok(want) => {
                            let want = Arc::new(want);
                            for (bin, who) in others() {
                                jobs.send((c, want.clone(), bin, who)).unwrap();
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            let runs: Vec<_> = others()
                                .map(|(bin, who)| {
                                    s.spawn(move || {
                                        (run_with_stdin(bin, &c.args, c.stdin.as_deref()), who)
                                    })
                                })
                                .collect();
                            let want = first.recv().unwrap();
                            for h in runs {
                                let (got, who) = h.join().unwrap();
                                compare(c, &want, &got, who);
                            }
                        }
                        Err(RecvTimeoutError::Disconnected) => panic!("Go's first run failed"),
                    }
                });
            });
        }
        drop(jobs);
    });
    let _ = std::fs::remove_file(go);
    let failures = failures.into_inner().unwrap();
    assert!(
        failures.is_empty(),
        "{} runs differ from Go's first:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        start.elapsed() < Duration::from_secs(30),
        "the repeat check took {:?}",
        start.elapsed()
    );
}

/// The native bridge modules are not part of the Rust port yet: Go runs
/// them, the Rust port reports E0805 at the import the program wrote,
/// also when the import is inside a module the entry imports.
#[test]
fn native_modules_are_not_supported_yet() {
    let o = std::process::Command::new(rust_fung())
        .args(["-e", "import http\nprint(1)"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), "");
    assert_eq!(
        String::from_utf8_lossy(&o.stderr),
        "error[E0805]: not supported yet: http\n  --> -e:1:1\n  |\n1 | import http\n  | ^\n  |\n call path (innermost first):\n   -e:1       <entry>\n"
    );
    assert_eq!(o.status.code(), Some(1));

    let dir = std::env::temp_dir().join(format!("fung-native-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lib.fung"), "import http\nfn f() = 1\n").unwrap();
    std::fs::write(dir.join("main.fung"), "import \"./lib\"\nprint(lib.f())\n").unwrap();
    let o = std::process::Command::new(rust_fung())
        .arg("run")
        .arg(dir.join("main.fung"))
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let err = String::from_utf8_lossy(&o.stderr);
    assert_eq!(String::from_utf8_lossy(&o.stdout), "");
    assert!(
        err.starts_with("error[E0805]: not supported yet: http\n  --> ")
            && err.contains("lib.fung:1:1\n"),
        "{err}"
    );
    assert_eq!(o.status.code(), Some(1));
}
