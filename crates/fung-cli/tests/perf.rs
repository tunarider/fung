//! The accumulation and tail-recursion shapes that would go quadratic with
//! full copies must finish 20,000 elements in well under 0.3 s. Release
//! builds only: a debug build is
//! too slow to say anything about the data structures.
#![cfg(not(debug_assertions))]

mod common;

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use common::{Reaped, fung_bin};

/// One timing test at a time: they run as threads of one process, and a
/// heavy neighbour (2,000 connections, 20,000 tasks, a large compile)
/// slowed a 0.3 s measurement past its limit on a three-core CI runner.
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    // A failed assertion in another test poisons the lock; the `()` it
    // guards cannot be left inconsistent, so go on.
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

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
        "list prepend",
        "xs = fold(0..40000, [], (acc, i) => [i] + acc)\nprint(len(xs), xs[0], last(xs))",
        "40000 39999 0\n",
    ),
    (
        "prepend a pair to a slice",
        "base = map(0..20000, x => x)\nxs = fold(0..20000, base, (acc, i) => [i, i] + acc[1..])\nprint(len(xs), xs[0])",
        "40000 19999\n",
    ),
    (
        "send many views of one vector",
        "(tx, rx) = channel(1)\n(t2, _r2) = channel(1)\nbig = map(0..200000, i => i) + [t2]\nvs = map(0..4000, i => big[i..])\nsend(tx, vs)\nprint(len(recv(rx)!))",
        "4000\n",
    ),
    (
        "rest pattern",
        "fn count(xs, acc) = match xs:\n  [] => acc\n  [_, ...rest] => count(rest, acc + 1)\n;;\nprint(count(map(0..20000, x => x), 0))",
        "20000\n",
    ),
    (
        "a queue in an atom",
        "q = atom(map(0..100000, x => x))\neach(0..2000, _ => swap(q, xs => xs[1..]))\nprint(len(deref(q)))",
        "98000\n",
    ),
    (
        "discard slices of a long list",
        "fn id(v) = v\nxs = map(0..100000, x => x)\nfor i in 0..2000: id(xs[i..]) ;;\neach(0..2000, i => xs[..i + 50000])\nprint(len(xs))",
        "100000\n",
    ),
    (
        "show and compare a deep list",
        "d = fold(1..=100000, [], (acc, _) => [acc])\ne = fold(1..=100000, [], (acc, _) => [acc])\nprint(len(str(d)), d == e, d < e, len(unique([d, e])))",
        "200002 true false 1\n",
    ),
    (
        "slice tail",
        "fn go(ys, acc) = len(ys) == 0 then acc else go(ys[1..], acc + 1)\nprint(go(map(0..20000, x => x), 0))",
        "20000\n",
    ),
    (
        "index a long string",
        "s = repeat(\"\u{e9}\", 1000000)\nprint(fold(0..200, 0, (n, i) => n + len(s[i] + s[i..i + 2])))",
        "600\n",
    ),
    (
        "a shared list equals itself",
        "x = fold(0..24, [1], (a, _) => [a, a])\nprint(x == x, [x] == [x])",
        "true true\n",
    ),
    (
        "a list used as a stack",
        "base = map(0..20000, x => x)\nprint(len(fold(0..20000, base, (s, i) => s[..len(s) - 1] + [i])))",
        "20000\n",
    ),
    (
        "unique",
        "print(len(unique(map(0..20000, x => x % 7000))))",
        "7000\n",
    ),
    (
        "grow a string",
        "print(len(fold(0..100000, \"\", (s, _i) => s + \"x\")), len(fold(0..50000, \"\", (s, _i) => s + \"\u{ac00}\")))",
        "100000 50000\n",
    ),
    (
        "small slices of a long list",
        "xs = map(0..50000, i => i)\nprint(fold(0..50000, 0, (n, i) => n + len(xs[i..])))",
        "1250025000\n",
    ),
    (
        "each character of a long non-ASCII string",
        "s = repeat(\"\u{ac00}b\", 25000)\nprint(fold(0..50000, 0, (n, i) => n + byte_len(s[i])))",
        "100000\n",
    ),
    (
        "walk a list and its slice",
        "xs = map(0..200000, x => x)\nprint(fold(xs, 0, (a, _x) => a + 1) + fold(xs[1..], 0, (a, _x) => a + 1))",
        "399999\n",
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

/// `run` for a program too long for one argument (Linux allows 128 KiB):
/// from a file, removed afterwards.
fn run_file(name: &str, src: &str) -> (String, Duration) {
    let path = std::env::temp_dir().join(format!("fung-perf-{name}-{}.fung", std::process::id()));
    std::fs::write(&path, src).unwrap();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_fung"))
        .arg(&path)
        .output()
        .unwrap();
    let took = start.elapsed();
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    (String::from_utf8_lossy(&out.stdout).into_owned(), took)
}

#[test]
fn collections_stay_linear() {
    let _serial = serial();
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

/// `regex.find_all` over 1,000,000 characters of multibyte text with
/// 100,000 matches: rune positions must not be recounted from the start of
/// the subject for every match (a cursor keeps the count), and the whole
/// program
/// must finish within a second. Measured on an Apple M-series release
/// build: about 0.06 s with the cursor, about 8 s when it recounts from
/// the start for every match.
#[test]
fn regex_find_all_stays_linear() {
    let _serial = serial();
    let src = "import regex\ns = repeat(\"가나다라 마바 1 \", 100000)\nms = regex.find_all(s, `\\d`)\nprint(len(s), len(ms), last(ms).end)";
    run(src);
    let (out, took) = run(src);
    eprintln!("regex find_all {took:?}");
    assert_eq!(out, "1000000 100000 999999\n");
    assert!(
        took < Duration::from_secs(1),
        "find_all over 1,000,000 characters took {took:?}"
    );
}

/// A program with 32,768 distinct constants compiles in well under a
/// second: the constant pool finds an equal constant by a hash index, not
/// by comparing with every earlier one (that took about 1.3 s here, and a
/// REPL input defining 65,537 globals about 5 s, timing out on a slow CI
/// runner).
#[test]
fn many_constants_compile_in_linear_time() {
    let _serial = serial();
    let items: Vec<String> = (0..32_768).map(|i| format!("str({i})")).collect();
    let src = format!("print(len([{}]))", items.join(", "));
    run_file("consts", &src);
    let (out, took) = run_file("consts", &src);
    eprintln!("many constants {took:?}");
    assert_eq!(out, "32768\n");
    assert!(
        took < Duration::from_millis(500),
        "32,768 constants took {took:?}"
    );
}

/// A chain of 16,000 operators compiles in linear time: constant folding
/// looks at each operator once, bottom up, instead of walking its whole
/// left chain again at every operator (VM18: about 1.3 s here, against
/// about 0.02 s once).
#[test]
fn a_long_operator_chain_compiles_in_linear_time() {
    let _serial = serial();
    for (name, op, want) in [("sum", "+", "16000\n"), ("product", "*", "1\n")] {
        let chain = vec!["x"; 16_000].join(&format!(" {op} "));
        let src = format!("fn f(x) = {chain}\nprint(f(1))");
        run_file(name, &src);
        let (out, took) = run_file(name, &src);
        eprintln!("16,000 operators ({name}) {took:?}");
        assert_eq!(out, want);
        assert!(
            took < Duration::from_millis(300),
            "16,000 operators ({name}) took {took:?}"
        );
    }
}

/// A function with 40,000 locals alive at once compiles in linear time:
/// the lifetime analysis changes one live set in place and keeps only
/// what each statement or branch lets go, instead of a copy of the live
/// set per statement (VM24: 30.7 s here, against about 0.1 s once).
#[test]
fn forty_thousand_live_locals_compile_in_linear_time() {
    let _serial = serial();
    let n = 40_000;
    let mut src = String::from("fn f():\n");
    for i in 0..n {
        src.push_str(&format!("  v{i} = {i}\n"));
    }
    let names: Vec<String> = (0..n).map(|i| format!("v{i}")).collect();
    src.push_str(&format!("  [{}]\n;;\nprint(len(f()))\n", names.join(", ")));
    run_file("live", &src);
    let (out, took) = run_file("live", &src);
    eprintln!("40,000 live locals {took:?}");
    assert_eq!(out, "40000\n");
    assert!(
        took < Duration::from_secs(1),
        "40,000 live locals took {took:?}"
    );
}

/// A literal nested 8,000 deep with a variable at the bottom compiles in
/// linear time: whether each level is a constant is found in the one
/// folding walk (VM25: each level walked all below it, 11.5 s here).
#[test]
fn a_deep_literal_with_a_variable_compiles_in_linear_time() {
    let _serial = serial();
    let items: Vec<String> = (0..20).map(|i| i.to_string()).collect();
    let mut lit = String::from("x");
    for _ in 0..8000 {
        lit = format!("[{}, {lit}]", items.join(", "));
    }
    let src = format!("fn f(x) = len({lit})\nprint(f(1))\n");
    run_file("nest", &src);
    let (out, took) = run_file("nest", &src);
    eprintln!("a literal 8,000 deep {took:?}");
    assert_eq!(out, "21\n");
    assert!(
        took < Duration::from_secs(1),
        "a literal 8,000 deep took {took:?}"
    );
}

/// A call with 4,095 named arguments binds them in time linear in the
/// pairs times the parameters: one pass finds and marks each slot (VM34:
/// a scan per pair before, 28 s here).
#[test]
fn four_thousand_named_arguments_bind_quickly() {
    let _serial = serial();
    let n = 4095;
    let params: Vec<String> = (0..n).map(|i| format!("a{i} = 0")).collect();
    let args: Vec<String> = (0..n).rev().map(|i| format!("a{i}: {i}")).collect();
    let names: Vec<String> = (0..n).map(|i| format!("a{i}")).collect();
    let src = format!(
        "fn f({}) = sum([{}])\nprint(f({}))\n",
        params.join(", "),
        names.join(", "),
        args.join(", ")
    );
    run_file("named", &src);
    let (out, took) = run_file("named", &src);
    eprintln!("4,095 named arguments {took:?}");
    assert_eq!(out, format!("{}\n", n * (n - 1) / 2));
    assert!(
        took < Duration::from_secs(1),
        "4,095 named arguments took {took:?}"
    );
}

/// `fung check` on `src`: its output and how long it took.
fn check_file(name: &str, src: &str) -> (std::process::Output, Duration) {
    let path = std::env::temp_dir().join(format!("fung-perf-{name}-{}.fung", std::process::id()));
    std::fs::write(&path, src).unwrap();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_fung"))
        .arg("check")
        .arg(&path)
        .output()
        .unwrap();
    let took = start.elapsed();
    let _ = std::fs::remove_file(&path);
    (out, took)
}

/// 8,000 undefined names among 8,000 names each get a "similar name"
/// help in about linear time: the names are indexed once instead of
/// measured against every name at every error (FE36: 26 s here before,
/// about 0.2 s after).
#[test]
fn similar_names_for_many_errors_stay_linear() {
    let _serial = serial();
    let n = 8_000;
    let mut src = String::new();
    for i in 0..n {
        src += &format!("v{i} = 1\n");
    }
    for i in 0..n {
        src += &format!("print(u{i})\n");
    }
    check_file("similar", &src);
    let (out, took) = check_file("similar", &src);
    eprintln!("8,000 undefined names {took:?}");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(err.matches("error[E0204]").count(), n, "{}", &err[..500]);
    assert!(err.contains("help: a similar name exists: `v7999`"));
    assert!(
        took < Duration::from_secs(2),
        "8,000 undefined names took {took:?}"
    );
}

/// 9,000 nested lambdas compile in linear time: desugar reads a function's
/// code for names only when it has both a pattern parameter and a default
/// (FE37: about 0.6 s here before, 0.03 s after).
#[test]
fn nested_lambdas_desugar_in_linear_time() {
    let _serial = serial();
    let params: String = (0..9_000).map(|i| format!("_x{i} => ")).collect();
    let src = format!("f = {params}_x0\nprint(f(1)(2)(3) != nil)\n");
    check_file("lambdas", &src);
    let (out, took) = check_file("lambdas", &src);
    eprintln!("9,000 nested lambdas {took:?}");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        took < Duration::from_millis(300),
        "9,000 nested lambdas took {took:?}"
    );
}

/// `fmt` aligns a run of 20,000 match arms in linear time: each line of
/// a run starts no run of its own (it took 6.8 s here when every arm
/// scanned the rest of the run again; now about 0.2 s with the pipes, so
/// 2 s tells the two apart on a slow runner).
#[test]
fn fmt_aligns_a_long_run_in_linear_time() {
    use std::io::Write;
    let _serial = serial();
    let mut src = String::from("x = match 1:\n");
    for i in 0..20_000 {
        src += &format!("  {i} => {i}\n");
    }
    src += ";;\n";
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fung"))
        .args(["fmt", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(src.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let took = start.elapsed();
    eprintln!("fmt 20,000 arms {took:?}");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("\n  19999 => 19999\n"));
    assert!(took < Duration::from_secs(2), "20,000 arms took {took:?}");
}

/// 20,000 tasks wait at once, each on a rendezvous channel of its own,
/// until the entry sends to each (R1b: a task is no thread, so there is
/// no thread limit; the thread runtime before failed with E0801 on macOS,
/// past its 8,192 threads). Within 3 s.
#[test]
fn twenty_thousand_waiting_tasks() {
    let _serial = serial();
    let src = "fn worker(s):\n  (tx, rx) = channel()\n  t = spawn(s, () => recv(rx)!)\n  [tx, t]\n;;\nprint(scope(fn(s):\n  ws = map(0..20000, _ => worker(s))\n  each(ws, w => send(w[0], 1))\n  fold(ws, 0, (a, w) => a + await(w[1])!)\n;;)!)";
    let (out, took) = run(src);
    eprintln!("20,000 tasks {took:?}");
    assert_eq!(out, "20000\n");
    assert!(took < Duration::from_secs(3), "{took:?}");
}

/// One program holds 2,000 HTTP connections at once (R4a): a server
/// whose handler waits on a channel, and 2,000 client tasks, connected
/// 100 at a time (below macOS's listen backlog of 128, past which the
/// kernel resets new connections). While all 2,000 handlers wait, the
/// process has only a few threads: no socket wait holds one. It starts
/// under `ulimit -Sn 256`, so it also needs fung's raised open file
/// limit (4,000 sockets). Then everything answers within 10 s.
#[test]
fn two_thousand_connections_at_once() {
    let _serial = serial();
    let child = Command::new("sh")
        .arg("-c")
        .arg("ulimit -Sn 256 && exec \"$0\" -e \"$1\" 20")
        .arg(fung_bin())
        .arg(CONNS)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut child = Reaped::new(child);
    let start = Instant::now();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    out.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n", "{:?}", child.stderr.take());
    let threads = threads_of(child.id());
    eprintln!(
        "2,000 connections: ready after {:?}, {threads} threads",
        start.elapsed()
    );
    assert!(threads < 100, "{threads} threads");
    child.stdin.take().unwrap().write_all(b"\n").unwrap();
    let mut rest = String::new();
    std::io::Read::read_to_string(&mut out, &mut rest).unwrap();
    assert_eq!(rest, "2000\n");
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "{:?}",
        start.elapsed()
    );
}

/// `fs.read` of a FIFO keeps up with a writer that streams 4 MB in
/// pieces larger than the FIFO's buffer (`head | cat`): within 1 s.
/// macOS reports no readiness for a piece after the first of one large
/// write, so a read that waited only for the event looked again only
/// every 50 ms (`pipe::RELOOK`): about 12 s for 4 MB before. The writer
/// starts after fung: its open waits until fung's read end is there.
#[test]
fn a_fifo_read_keeps_up_with_a_streaming_writer() {
    let _serial = serial();
    let dir = std::env::temp_dir().join(format!("fung-perf-fifo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir(&dir).unwrap();
    let fifo = dir.join("pipe");
    let made = Command::new("mkfifo").arg(&fifo).status().unwrap();
    assert!(made.success());
    let start = Instant::now();
    let reader = Command::new(fung_bin())
        .args(["-e", "import fs\nprint(byte_len(fs.read(args[0])!))"])
        .arg(&fifo)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let reader = Reaped::new(reader);
    let writer = Command::new("sh")
        .arg("-c")
        .arg("head -c 4000000 /dev/zero | cat > \"$0\"")
        .arg(&fifo)
        .spawn()
        .unwrap();
    let mut writer = Reaped::new(writer);
    let (out, killed) = reader.output_within(Duration::from_secs(30)).unwrap();
    let took = start.elapsed();
    assert!(writer.wait().unwrap().success());
    let _ = std::fs::remove_dir_all(&dir);
    eprintln!("4 MB through a FIFO {took:?}");
    assert!(!killed, "{took:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "4000000\n",
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(took < Duration::from_secs(1), "{took:?}");
}

/// The threads of process `pid` now.
fn threads_of(pid: u32) -> usize {
    if cfg!(target_os = "linux") {
        return std::fs::read_dir(format!("/proc/{pid}/task"))
            .unwrap()
            .count();
    }
    let ps = Command::new("ps")
        .args(["-M", "-p", &pid.to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&ps.stdout).lines().count() - 1
}

/// The program of `two_thousand_connections_at_once`: its argument is
/// the number of batches of 100 clients.
const CONNS: &str = "import http\nfn wait_for(cond) = cond() then nil else fn(): sleep(1ms); wait_for(cond) ;;()\nbatches = parse_int(args[0])!\n_n = batches * 100\n(go_tx, go_rx) = channel()\ncount = atom(0)\nh = fn(_req):\n  swap(count, c => c + 1)\n  _ = recv(go_rx)\n  \"ok\"\n;;\nfn ok(r) = match r:\n  Ok(_) => 1\n  _ => 0\n;;\nsupervise(fn(sc):\n  (port_tx, port) = channel(1)\n  s = spawn(sc, () => http.serve(\"127.0.0.1:0\", h, ready: a => send(port_tx, a)))\n  addr = recv(port)!\n  got = scope(fn(cs):\n    # in batches below the listen backlog (128 on macOS)\n    fn batch(_b) = map(0..100, _ => spawn(cs, () => http.get(\"http://\" + addr + \"/\")))\n    ts = flatten(map(0..batches, fn(b):\n      t = batch(b)\n      wait_for(() => deref(count) == (b + 1) * 100)\n      t\n    ;;))\n    print(\"ready\")\n    read_line()\n    close(go_tx)\n    fold(ts, 0, (a, t) => a + ok(await(t)!))\n  ;;)!\n  print(got)\n  cancel(s)\n;;, _e => nil)";

/// Fifty tasks wait on commands at once (R4b): no thread waits for one,
/// so the process has only a few threads (each command had four before:
/// a waiter, two output readers, an input feeder).
#[test]
fn fifty_commands_at_once() {
    let _serial = serial();
    let dir = std::env::temp_dir().join(format!("fung-fifty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = format!(
        "import fs\nfn wait_for(cond) = cond() then nil else fn(): sleep(5ms); wait_for(cond) ;;()\nd = \"{}\"\nprint(timeout(() => scope(fn(s):\n  ts = map(0..50, _i => spawn(s, () => sh(\"touch {}/$$; sleep 1\")!.code))\n  wait_for(() => len(fs.list(d)!) == 50)\n  print(\"ready\")\n  read_line()\n  fold(ts, 0, (a, t) => a + await(t)!)\n;;)!, 20s)!)",
        dir.display(),
        dir.display()
    );
    let child = Command::new(fung_bin())
        .args(["-e", &src])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut child = Reaped::new(child);
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    out.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n", "{:?}", child.stderr.take());
    let threads = threads_of(child.id());
    eprintln!("50 commands: {threads} threads");
    child.stdin.take().unwrap().write_all(b"\n").unwrap();
    let mut rest = String::new();
    std::io::Read::read_to_string(&mut out, &mut rest).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(rest, "0\n");
    assert!(threads < 20, "{threads} threads");
}

/// 50,000 tasks wait to receive on one channel, then get a value each:
/// a wait that ends leaves the channel's queue at no cost (R4b review: a
/// scan per wait made this quadratic).
#[test]
fn many_receivers_on_one_channel() {
    let _serial = serial();
    let src = "(tx, rx) = channel()\nprint(scope(fn(s):\n  ts = map(0..50000, _ => spawn(s, () => recv(rx)!))\n  each(0..50000, _i => send(tx, 1))\n  fold(ts, 0, (a, t) => a + await(t)!)\n;;)!)";
    let (out, took) = run(src);
    eprintln!("50,000 receivers on one channel {took:?}");
    assert_eq!(out, "50000\n");
    assert!(took < Duration::from_secs(1), "{took:?}");
}

/// 50,000 tasks await one task under a 1 ms deadline, then 50,000 more
/// await it until it ends: an `await` cut short leaves the task's
/// waiting list, and joining the list costs O(1) (review C3: the cut
/// waits stayed listed and every await scanned the list). Measured on an
/// Apple M2 Max release build: 2.1 s before, 0.22 s after. Within 1.5 s.
#[test]
fn many_awaits_on_one_task() {
    let _serial = serial();
    let src = "(go_tx, go) = channel()\nprint(scope(fn(s):\n  t = spawn(s, () => recv(go))\n  scope(c => each(0..50000, _ => spawn(c, fn(): _ = timeout(() => await(t), 1ms); nil ;;)))\n  ws = map(0..50000, _ => spawn(s, () => await(t)))\n  close(go_tx)\n  fold(ws, 0, (a, w) => a + (is_err(await(w)!!) then 1 else 0))\n;;)!)";
    let (out, took) = run(src);
    eprintln!("100,000 awaits on one task {took:?}");
    assert_eq!(out, "50000\n");
    assert!(took < Duration::from_millis(1500), "{took:?}");
}

/// Counts the tasks that reached their wait (`ready`), so the entry
/// cancels only once all of them wait.
const UNTIL_ALL: &str = "fn wait_for(cond) = cond() then nil else fn(): sleep(1ms); wait_for(cond) ;;()\nready = atom(0)\n";

/// 80,000 tasks wait to receive on one channel and are all cancelled:
/// each wait that ends leaves the queue at O(1) (review C4: a scan of
/// both queues per wait). 3.6 s before, 0.25 s after. Within 1.5 s.
#[test]
fn many_cancelled_receivers_on_one_channel() {
    let _serial = serial();
    let src = format!(
        "{UNTIL_ALL}(_tx, rx) = channel()\nprint(scope(fn(s):\n  each(0..80000, _ => spawn(s, fn(): swap(ready, n => n + 1); recv(rx) ;;))\n  wait_for(() => deref(ready) == 80000)\n  cancel(s)\n;;))"
    );
    let (out, took) = run(&src);
    eprintln!("80,000 cancelled receivers {took:?}");
    assert_eq!(out, "Err({kind: 'cancelled, message: \"cancelled\"})\n");
    assert!(took < Duration::from_millis(1500), "{took:?}");
}

/// 80,000 tasks are cancelled while in `with`'s `use`, and each `release`
/// parks: a release that ends drops its shield at O(1) (review C5: a
/// scan of every running release's shield). 2.2 s before, 0.43 s after
/// (80,000 timers). Within 1.5 s.
#[test]
fn many_releases_that_park() {
    let _serial = serial();
    let src = format!(
        "{UNTIL_ALL}print(scope(fn(s):\n  each(0..80000, i => spawn(s, () => with(() => i, fn(_r): swap(ready, n => n + 1); sleep(10s) ;;, _r => sleep(1ms))))\n  wait_for(() => deref(ready) == 80000)\n  cancel(s)\n;;))"
    );
    let (out, took) = run(&src);
    eprintln!("80,000 releases that park {took:?}");
    assert_eq!(out, "Err({kind: 'cancelled, message: \"cancelled\"})\n");
    assert!(took < Duration::from_millis(1500), "{took:?}");
}

/// Spec 10.4's latency, recorded only: a light request beside one whose
/// handler parses a big JSON text (about 6 MB). The heavy request reaches
/// its handler first and the parse starts once the light one is on its
/// way, so the light one's time includes what the parse holds of the run
/// thread. Printed for the R7 note; the bound is loose. The run is
/// bounded inside (`timeout`) and out (killed after 30 s).
#[test]
fn a_light_request_beside_a_heavy_parse() {
    let _serial = serial();
    for mode in ["light", "heavy"] {
        let child = Command::new(fung_bin())
            .args(["-e", LATENCY, mode])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (out, late) = Reaped::new(child)
            .output_within(Duration::from_secs(30))
            .unwrap();
        assert!(!late, "{mode}: still running after 30 s");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "{text}\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        eprintln!("latency ({mode}): {}", text.trim().replace('\n', ", "));
        let light = text
            .lines()
            .find(|l| l.starts_with("light "))
            .unwrap_or_else(|| panic!("{mode}: {text}"));
        assert!(light.ends_with(" true"), "{text}");
    }
}

/// A long native gives its turn about every millisecond (L6): beside a
/// big `json.parse`, `json.stringify`, `sort`, `unique` or `split`, a task
/// that ticks every millisecond is never kept waiting long (before L6:
/// 78 ms beside a 6 MB parse, the whole native). A busy machine only
/// makes a gap longer (a CI runner once took 13 ms where this one takes
/// 2.4), so the best of three runs counts: a native that holds the run
/// thread is long in all three.
#[test]
fn long_natives_give_their_turn() {
    let _serial = serial();
    for (what, work) in LONG_WORK {
        let src = format!("{LONG_SETUP}{GAP}work = () => {work}\nprint(gap(work))");
        let ms = (0..3)
            .map(|_| {
                let (out, _) = run_file("gap", &src);
                out.trim()
                    .parse::<f64>()
                    .unwrap_or_else(|_| panic!("{what}: {out}"))
            })
            .fold(f64::INFINITY, f64::min);
        eprintln!("worst gap beside {what}: {ms} ms");
        assert!(ms < 10.0, "{what}: a 1 ms ticker waited {ms} ms");
    }
}

/// A deadline cuts a long native short (L6): before, `timeout(…, 5ms)`
/// around a big `json.parse` or `sort` gave its value 75 ms late.
#[test]
fn deadlines_cut_long_natives() {
    let _serial = serial();
    for (what, work) in LONG_WORK {
        let src = format!(
            "{LONG_SETUP}t0 = time.monotonic()\nr = timeout(() => {work}, 5ms)\ntook = time.monotonic() - t0\nprint(match r:\n  Err({{kind: 'timeout}}) => \"timeout\"\n  _ => \"value\"\n;;, took < 30ms)"
        );
        let (out, _) = run_file("deadline", &src);
        assert_eq!(out, "timeout true\n", "{what}");
    }
}

/// Inputs that take about 50 to 300 ms each in one go (release build).
const LONG_SETUP: &str = "import json\nimport time\n_rows = map(0..200000, i => {a: i, b: \"xxxxxxxx\"})\n_big = json.stringify(_rows)\n_xs = map(0..500000, i => (i * 7919) % 1000003)\n_text = repeat(\"word \", 1000000)\n";

const LONG_WORK: &[(&str, &str)] = &[
    ("json.parse", "len(json.parse(_big)!)"),
    ("json.stringify", "len(json.stringify(_rows))"),
    ("sort", "len(sort(_xs))"),
    ("sort_by", "len(sort_by(_xs, x => x))"),
    ("unique", "len(unique(_xs))"),
    ("split", "len(split(_text, \" \"))"),
    ("join", "len(join(_xs, \",\"))"),
    ("replace", "len(replace(_text, \"o\", \"00\"))"),
];

/// `gap(work)`: runs `work` beside a task that sleeps 1 ms at a time and
/// gives the longest wait between its wake-ups, in milliseconds. The
/// ticker has started before the work does (a state, not a sleep).
const GAP: &str = "fn gap(job) = scope(fn(s):\n  (started_tx, started) = channel(1)\n  (done_tx, done) = channel(1)\n  fn tick(prev, worst, start):\n    if start: send(started_tx, true) else: nil ;;\n    sleep(1ms)\n    now = time.monotonic()\n    worst2 = max([worst, now - prev])\n    match recv_now(done):\n      Ok(_) => worst2\n      _ => tick(now, worst2, false)\n    ;;\n  ;;\n  w = spawn(s, () => tick(time.monotonic(), 0ms, true))\n  recv(started)!\n  job()\n  send(done_tx, true)\n  ms(await(w)!)\n;;)!\nfn ms(d) = d / 1ms\n";

const LATENCY: &str = "import http\nimport json\nimport time\nbig = json.stringify(map(0..100000, i => {id: i, name: \"item\", tags: [\"a\", \"b\"]}))\n(go_tx, go) = channel(1)\n(in_tx, arrived) = channel(1)\nh = fn(req):\n  if req.path == \"/heavy\":\n    send(in_tx, true)\n    recv(go)\n    _ = len(json.parse(big)!)\n    \"heavy\"\n  else:\n    \"ok\"\n  ;;\n;;\nprint(timeout(() => supervise(fn(sc):\n  (port_tx, port) = channel(1)\n  s = spawn(sc, () => http.serve(\"127.0.0.1:0\", h, ready: a => send(port_tx, a)))\n  addr = recv(port)!\n  heavy = args[0] == \"heavy\"\n  hv = heavy then spawn(sc, fn():\n    t0 = time.now()\n    http.get(\"http://\" + addr + \"/heavy\")!\n    time.now() - t0\n  ;;) else nil\n  if heavy: recv(arrived) else: nil ;;\n  t0 = time.now()\n  if heavy: send(go_tx, true) else: nil ;;\n  http.get(\"http://\" + addr + \"/light\")!\n  took = time.now() - t0\n  print(\"light\", took, took < 30ms)\n  if heavy: print(\"heavy\", await(hv)!) else: nil ;;\n  cancel(s)\n;;, _e => nil), 20s))\n";
