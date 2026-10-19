//! The accumulation and tail-recursion shapes that would go quadratic with
//! full copies must finish 20,000 elements in well under 0.3 s. Release
//! builds only: a debug build is
//! too slow to say anything about the data structures.
#![cfg(not(debug_assertions))]

mod common;

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{Reaped, fung_bin};

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
        "a queue in an atom",
        "q = atom(map(0..100000, x => x))\neach(0..2000, _ => swap(q, xs => xs[1..]))\nprint(len(deref(q)))",
        "98000\n",
    ),
    (
        "slice tail",
        "fn go(ys, acc) = len(ys) == 0 then acc else go(ys[1..], acc + 1)\nprint(go(map(0..20000, x => x), 0))",
        "20000\n",
    ),
    (
        "walk a list and its slice",
        "xs = map(0..200000, x => x)\nprint(fold(xs, 0, (a, x) => a + 1) + fold(xs[1..], 0, (a, x) => a + 1))",
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

/// `regex.find_all` over 1,000,000 characters of multibyte text with
/// 100,000 matches: rune positions must not be recounted from the start of
/// the subject for every match (a cursor keeps the count), and the whole
/// program
/// must finish within a second. Measured on an Apple M-series release
/// build: about 0.06 s with the cursor, about 8 s when it recounts from
/// the start for every match.
#[test]
fn regex_find_all_stays_linear() {
    let src = "import regex\ns = repeat(\"가나다라 마바 1 \", 100000)\nms = regex.find_all(s, `\\d`)\nprint(len(s), len(ms), ms[-1].end)";
    run(src);
    let (out, took) = run(src);
    eprintln!("regex find_all {took:?}");
    assert_eq!(out, "1000000 100000 999999\n");
    assert!(
        took < Duration::from_secs(1),
        "find_all over 1,000,000 characters took {took:?}"
    );
}

/// 20,000 tasks wait at once, each on a rendezvous channel of its own,
/// until the entry sends to each (R1b: a task is no thread, so there is
/// no thread limit; the thread runtime before failed with E0801 on macOS,
/// past its 8,192 threads). Within 3 s.
#[test]
fn twenty_thousand_waiting_tasks() {
    let src = "fn worker(s):\n  (tx, rx) = channel()\n  t = spawn(s, () => recv(rx)!)\n  [tx, t]\n;;\nprint(scope(fn(s):\n  ws = map(0..20000, _ => worker(s))\n  each(ws, w => send(w[0], 1))\n  fold(ws, 0, (a, w) => a + await(w[1]))\n;;))";
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
const CONNS: &str = "import http\nfn until(f) = f() then nil else fn(): sleep(1ms); until(f) ;;()\nbatches = parse_int(args[0])!\nn = batches * 100\n(go_tx, go_rx) = channel()\ncount = atom(0)\nh = fn(req):\n  swap(count, c => c + 1)\n  recv(go_rx)\n  \"ok\"\n;;\nfn ok(r) = match r:\n  Ok(_) => 1\n  _ => 0\n;;\nsupervise(fn(sc):\n  (port_tx, port) = channel(1)\n  s = spawn(sc, () => http.serve(\"127.0.0.1:0\", h, ready: a => send(port_tx, a)))\n  addr = recv(port)!\n  got = scope(fn(cs):\n    # in batches below the listen backlog (128 on macOS)\n    fn batch(b) = map(0..100, _ => spawn(cs, () => http.get(\"http://\" + addr + \"/\")))\n    ts = flatten(map(0..batches, fn(b):\n      t = batch(b)\n      until(() => deref(count) == (b + 1) * 100)\n      t\n    ;;))\n    print(\"ready\")\n    read_line()\n    close(go_tx)\n    fold(ts, 0, (a, t) => a + ok(await(t)))\n  ;;)\n  print(got)\n  cancel(s)\n;;, e => nil)";

/// Fifty tasks wait on commands at once (R4b): no thread waits for one,
/// so the process has only a few threads (each command had four before:
/// a waiter, two output readers, an input feeder).
#[test]
fn fifty_commands_at_once() {
    let dir = std::env::temp_dir().join(format!("fung-fifty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = format!(
        "import fs\nfn until(f) = f() then nil else fn(): sleep(5ms); until(f) ;;()\nd = \"{}\"\nprint(timeout(20s, () => scope(fn(s):\n  ts = map(0..50, i => spawn(s, () => sh(\"touch {}/$$; sleep 1\")!.code))\n  until(() => len(fs.list(d)!) == 50)\n  print(\"ready\")\n  read_line()\n  fold(ts, 0, (a, t) => a + await(t))\n;;)))",
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
    let src = "(tx, rx) = channel()\nprint(scope(fn(s):\n  ts = map(0..50000, _ => spawn(s, () => recv(rx)!))\n  each(0..50000, i => send(tx, 1))\n  fold(ts, 0, (a, t) => a + await(t))\n;;))";
    let (out, took) = run(src);
    eprintln!("50,000 receivers on one channel {took:?}");
    assert_eq!(out, "50000\n");
    assert!(took < Duration::from_secs(1), "{took:?}");
}

/// Spec 10.4's latency, recorded only: a light request beside one whose
/// handler parses a big JSON text (about 6 MB). The heavy request reaches
/// its handler first and the parse starts once the light one is on its
/// way, so the light one's time includes what the parse holds of the run
/// thread. Printed for the R7 note; the bound is loose. The run is
/// bounded inside (`timeout`) and out (killed after 30 s).
#[test]
fn a_light_request_beside_a_heavy_parse() {
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

const LATENCY: &str = "import http\nimport json\nimport time\nbig = json.stringify(map(0..100000, i => {id: i, name: \"item\", tags: [\"a\", \"b\"]}))\n(go_tx, go) = channel(1)\n(in_tx, arrived) = channel(1)\nh = fn(req):\n  if req.path == \"/heavy\":\n    send(in_tx, true)\n    recv(go)\n    len(json.parse(big)!)\n    \"heavy\"\n  else:\n    \"ok\"\n  ;;\n;;\nprint(timeout(20s, () => supervise(fn(sc):\n  (port_tx, port) = channel(1)\n  s = spawn(sc, () => http.serve(\"127.0.0.1:0\", h, ready: a => send(port_tx, a)))\n  addr = recv(port)!\n  heavy = args[0] == \"heavy\"\n  hv = heavy then spawn(sc, fn():\n    t0 = time.now()\n    http.get(\"http://\" + addr + \"/heavy\")!\n    time.now() - t0\n  ;;) else nil\n  if heavy: recv(arrived) else: nil ;;\n  t0 = time.now()\n  if heavy: send(go_tx, true) else: nil ;;\n  http.get(\"http://\" + addr + \"/light\")!\n  took = time.now() - t0\n  print(\"light\", took, took < 1s)\n  if heavy: print(\"heavy\", await(hv)) else: nil ;;\n  cancel(s)\n;;, e => nil)))\n";
