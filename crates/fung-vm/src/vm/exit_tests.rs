//! Exit rules and standard input shared by tasks (stage 3a, Task 7): `exit`
//! from any task, whole printed lines and standard input read by several
//! tasks; cleanup at the end is `with`'s release. Expected texts are what
//! `fung -e` prints. Tests that need a task in a given state wait for that
//! state (`testutil` probes), not for a time.

use std::io::{BufReader, Read};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use crate::testutil::{Opts, Outcome, UNTIL, exec_with_stdin, probes, run, setup};
use crate::value::{Native, Value};
use crate::vm::{Stop, Task};

/// Runs `f` on its own thread and fails the test if it takes longer than
/// `secs` (a hang becomes a failure; the thread is left behind).
fn within<R: Send + 'static>(secs: u64, f: impl FnOnce() -> R + Send + 'static) -> R {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs))
        .unwrap_or_else(|_| panic!("did not end within {secs}s"))
}

/// Standard input that gives what is sent on a channel and ends when the
/// sender drops: a read blocks until then, like a terminal nobody types
/// on.
struct Fed {
    rx: Receiver<Vec<u8>>,
    left: Vec<u8>,
}

impl Read for Fed {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.left.is_empty() {
            match self.rx.recv() {
                Ok(b) => self.left = b,
                Err(_) => return Ok(0),
            }
        }
        let n = buf.len().min(self.left.len());
        buf[..n].copy_from_slice(&self.left[..n]);
        self.left.drain(..n);
        Ok(n)
    }
}

fn fed() -> (Sender<Vec<u8>>, Box<dyn std::io::BufRead + Send>) {
    let (tx, rx) = channel();
    let r = BufReader::new(Fed {
        rx,
        left: Vec::new(),
    });
    (tx, Box::new(r))
}

/// The probes plus `reading()`: how many tasks wait for a line.
fn stdin_probes() -> Vec<Value> {
    let mut v = probes();
    v.push(Value::Native(Native::leak(
        "reading",
        vec![],
        |t: &mut Task, _| Ok(Value::Int(t.stdin_waiting() as i64)),
    )));
    v
}

/// Runs `src` (after `UNTIL`) with standard input that never delivers
/// while the run lasts.
fn exec_on_silent_stdin(src: &'static str) -> Outcome {
    let (tx, stdin) = fed();
    let o = within(10, move || {
        exec_with_stdin(
            &format!("{UNTIL}{src}"),
            Opts {
                extra: stdin_probes,
                ..Opts::default()
            },
            stdin,
        )
    });
    drop(tx);
    o
}

/// A task waiting for a line is cancelled: it leaves the waiting list.
#[test]
fn a_cancelled_stdin_wait_leaves_the_queue() {
    let o = exec_on_silent_stdin(
        "scope(fn(s):\n  a = spawn(s, () => read_line())\n  until(() => reading() == 1)\n  cancel(a)\n  print(await(a), reading())\n;;)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        ("Err({kind: 'cancelled}) 0\n", "")
    );
}

// --- exit from any task ---

/// An exit from any task: the code is recorded, the entry ends with the
/// exit, and the root of the program is cancelled.
#[test]
fn exit_from_anywhere_requests_exit() {
    let s = setup("fn f() = exit(4)\nf()", Opts::default()).unwrap();
    let r = s.rt.run();
    assert!(matches!(r, Err(Stop::Exit(4))), "{r:?}");
    assert_eq!(s.rt.exit_code(), Some(4));
    assert!(s.rt.shared().root().is_cancelled(), "exit cancels the root");
}

/// An exit from a spawned task ends the entry wherever it waits, and
/// every other task; the release runs once.
#[test]
fn an_exit_from_a_task_cancels_every_task() {
    let start = Instant::now();
    let (out, err, code) = within(10, || {
        run(
            "(_tx, ch) = channel()\nwith(() => nil, _ => scope(fn(s):\n  spawn(s, () => recv(ch))\n  spawn(s, () => sleep(10s))\n  spawn(s, fn(): sleep(10ms); exit(3) ;;)\n  recv(ch)\n  print(\"never\")\n;;), _ => print(\"bye\"))",
        )
    });
    assert_eq!((out.as_str(), err.as_str(), code), ("bye\n", "", 3));
    assert!(start.elapsed() < Duration::from_secs(2));
}

/// An `exit` inside a swap's `f` is an effect (E0707): the program
/// ends with the error, the atom keeps its state, and a release still
/// reads and swaps it.
#[test]
fn an_exit_inside_a_swaps_f_is_e0707_and_leaves_the_state() {
    let (out, err, code) = within(10, || {
        run(
            "a = atom(1)\nwith(() => nil, fn(_): swap(a, x => exit(4)); print(\"never\") ;;, _ => print(\"ae\", deref(a), swap(a, x => x + 1)))",
        )
    });
    assert_eq!(out, "ae 1 2\n");
    assert!(
        err.starts_with("error[E0707]: `exit` has an effect"),
        "{err}"
    );
    assert_eq!(code, 1);
}

/// An exit inside a lambda a native calls (`each`) unwinds through the
/// native and still ends the program.
#[test]
fn an_exit_inside_a_native_callback_ends_the_program() {
    let (out, err, code) = within(10, || {
        run(
            "with(() => nil, _ => scope(fn(s):\n  spawn(s, () => each(1..=3, x => x == 2 then exit(6) else print(x)))\n  sleep(10s)\n;;), _ => print(\"h\"))",
        )
    });
    assert_eq!((out.as_str(), err.as_str(), code), ("1\nh\n", "", 6));
}

/// Lines printed by many tasks at once stay whole (each print is one write
/// on the run thread).
#[test]
fn printed_lines_stay_whole() {
    let (out, err, code) = run(
        "scope(fn(s):\n  ts = for _ in 0..50: spawn(s, () => print(join(for _ in 0..100: \"x\" ;;, \"\"))) ;;\n  each(ts, await)\n;;)",
    );
    assert_eq!((err.as_str(), code), ("", 0));
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 50);
    assert!(lines.iter().all(|l| *l == "x".repeat(100)), "{out}");
}

// --- standard input ---

/// Two tasks reading at once each get a whole line.
#[test]
fn stdin_is_shared_safely() {
    let o = crate::testutil::exec(
        "scope(fn(s):\n  t = spawn(s, () => read_line())\n  print(read_line(), await(t))\n;;)",
        Opts {
            stdin: "a\nb\n",
            ..Opts::default()
        },
    );
    assert!(
        o.out.contains("Ok(\"a\")") && o.out.contains("Ok(\"b\")"),
        "{}",
        o.out
    );
    assert_eq!((o.err.as_str(), o.exit), ("", 0));
}

/// Many tasks reading every line at once: each line goes to exactly one
/// of them, whole.
#[test]
fn concurrent_readers_get_every_line_once() {
    let input: String = (0..2000).map(|i| format!("line-{i}\n")).collect();
    let input: &'static str = Box::leak(input.into_boxed_str());
    let o = crate::testutil::exec(
        "scope(fn(s):\n  ts = for _ in 0..4: spawn(s, () => read_lines()!) ;;\n  each(ts, t => each(await(t), print))\n;;)",
        Opts {
            stdin: input,
            ..Opts::default()
        },
    );
    assert_eq!((o.err.as_str(), o.exit), ("", 0));
    let mut got: Vec<&str> = o.out.lines().collect();
    got.sort_unstable();
    let mut want: Vec<&str> = input.lines().collect();
    want.sort_unstable();
    assert_eq!(got, want);
}

/// The brief's test: with standard input silent, a task waiting for a
/// line lets the others run, and an exit ends the program without the
/// line.
#[test]
fn a_task_waiting_for_stdin_lets_others_print_and_exit() {
    let start = Instant::now();
    let o = exec_on_silent_stdin(
        "with(() => nil, _ => scope(fn(s):\n  t = spawn(s, () => read_line())\n  until(() => reading() == 1)\n  print(\"out\")\n  exit(3)\n;;), _ => print(\"bye\"))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("out\nbye\n", "", 3)
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}

/// The entry waiting for a line is cancelled by an exit from a task too.
#[test]
fn the_entry_waiting_for_stdin_ends_on_an_exit() {
    let start = Instant::now();
    let o = exec_on_silent_stdin(
        "with(() => nil, _ => scope(fn(s):\n  spawn(s, fn(): until(() => reading() == 1); print(\"out\"); exit(3) ;;)\n  print(read_line())\n;;), _ => print(\"bye\"))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("out\nbye\n", "", 3)
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}

/// A task waiting for a line is not blocked: the others are not judged
/// deadlocked, and cancelling it ends its wait.
#[test]
fn a_task_waiting_for_stdin_is_not_a_deadlock_and_can_be_cancelled() {
    let o = exec_on_silent_stdin(
        "scope(fn(s):\n  t = spawn(s, () => read_line())\n  until(() => reading() == 1)\n  cancel(t)\n  print(await(t))\n;;)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err({kind: 'cancelled})\n", "", 0)
    );
}

/// A line that comes while a task waits wakes it; the line read after a
/// wait is the next one, whole.
#[test]
fn a_line_that_comes_later_wakes_the_reader() {
    let (tx, stdin) = fed();
    let feeder = std::thread::spawn(move || {
        tx.send(b"fir".to_vec()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        tx.send(b"st\nsecond\n".to_vec()).unwrap();
    });
    let o = within(10, move || {
        exec_with_stdin(
            "print(read_line(), read_line(), read_line())",
            Opts::default(),
            stdin,
        )
    });
    feeder.join().unwrap();
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Ok(\"first\") Ok(\"second\") Ok(nil)\n", "", 0)
    );
}

// --- processes ---

/// A task waiting for a child process lets the others run: the process
/// waits for a file that a process the entry starts creates, which
/// could not happen if the first wait held the run thread.
#[test]
fn a_task_waiting_for_a_process_lets_others_run() {
    let dir = crate::testutil::temp_dir();
    let flag = dir.join("flag").to_string_lossy().into_owned();
    let src = format!(
        "(tx, ch) = channel()\nscope(fn(s):\n  t = spawn(s, fn(): send(tx, 1); sh(\"while [ ! -e {flag} ]; do sleep 0.01; done; echo done\") ;;)\n  recv(ch)\n  print(is_ok(sh(\"touch {flag}\")))\n  print(await(t))\n;;)"
    );
    let o = within(10, move || crate::testutil::exec(&src, Opts::default()));
    assert_eq!(o.err, "");
    assert_eq!(o.out, "true\nOk({out: \"done\\n\", err: \"\", code: 0})\n");
}

/// Standard input that counts the reads made on it; each read gives the
/// next chunk.
struct Counted {
    chunks: std::collections::VecDeque<&'static [u8]>,
    reads: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Read for Counted {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let Some(c) = self.chunks.pop_front() else {
            return Ok(0);
        };
        buf[..c.len()].copy_from_slice(c);
        Ok(c.len())
    }
}

/// Standard input is read only when a task asks for a line and none is
/// queued: reading one line makes one read, and what the program never
/// asked for stays in the input for whoever reads it next (a command after
/// fung in a shell pipeline).
#[test]
fn stdin_is_read_only_on_demand() {
    let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let stdin = Box::new(BufReader::with_capacity(
        4096,
        Counted {
            chunks: [b"a\n" as &[u8], b"b\n", b"c\n"].into(),
            reads: reads.clone(),
        },
    ));
    let o = within(10, move || {
        exec_with_stdin(
            "print(read_line())\nsleep(20ms)\nprint(\"end\")",
            Opts::default(),
            stdin,
        )
    });
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Ok(\"a\")\nend\n", "", 0)
    );
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
}

/// A wait for a line that never comes ends at the deadline of the
/// `timeout` around it (concurrency v2, L2), and nobody waits any more.
#[test]
fn a_stdin_read_is_cut_at_the_deadline() {
    let start = Instant::now();
    let o = exec_on_silent_stdin("print(timeout(50ms, () => read_line()))\nprint(reading())");
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err({kind: 'timeout})\n0\n", "", 0)
    );
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
}
