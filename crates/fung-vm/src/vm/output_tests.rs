//! Standard output and error through the writer thread (`output.rs`,
//! CC15). Each test runs on a thread of its own, bounded, so a write that
//! held the run thread fails the test instead of hanging it.

use std::io::Write;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use crate::testutil::{Buf, Opts, Outcome, Streams, exec};

/// Runs `src` with `streams`, bounded.
fn bounded(src: &'static str, streams: fn(Buf, Buf) -> Streams) -> Outcome {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(exec(
            src,
            Opts {
                streams,
                ..Opts::default()
            },
        ));
    });
    rx.recv_timeout(Duration::from_secs(20))
        .expect("the run did not end")
}

/// A gate a test's standard output waits at until its standard error is
/// written: a reader that reads only after the program said something
/// else. One per test (tests run side by side).
struct Gate(Mutex<bool>, Condvar);

impl Gate {
    const fn new() -> Gate {
        Gate(Mutex::new(false), Condvar::new())
    }

    fn open(&self) {
        *self.0.lock().unwrap() = true;
        self.1.notify_all();
    }

    fn wait(&self) {
        let mut open = self.0.lock().unwrap();
        while !*open {
            open = self.1.wait(open).unwrap();
        }
    }
}

/// Writes to `buf` once `gate` is open.
struct Gated(&'static Gate, Buf);

impl Write for Gated {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.wait();
        self.1.write(b)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Writes to `buf` and opens `gate`.
struct Opener(&'static Gate, Buf);

impl Write for Opener {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        let n = self.1.write(b)?;
        self.0.open();
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A `print` that waits for room while nobody reads keeps its deadline:
/// the `timeout` ends at its time and its `eprint` goes out on the other
/// lane, which lets the reader read; what was queued before is written
/// then (whole lines).
#[test]
fn a_deadline_ends_a_wait_for_room() {
    static GATE: Gate = Gate::new();
    let o = bounded(
        "line = repeat(\"x\", 999)\nr = timeout(fn(): each(0..200, _ => print(line)); \"done\" ;;, 50ms)\neprint(r)",
        |out, err| {
            (
                Box::new(Gated(&GATE, out)),
                Box::new(Opener(&GATE, err)),
                true,
            )
        },
    );
    assert_eq!(
        (o.err.as_str(), o.exit),
        (
            "Err({kind: 'timeout, message: \"timed out after 50ms\"})\n",
            0
        )
    );
    let n = o.out.len();
    assert!(n > 0 && n < 200_000 && n.is_multiple_of(1000), "{n}");
}

/// One lane keeps the order of the two streams' lines: what a terminal
/// shows of `print` and `eprint` together.
#[test]
fn one_lane_keeps_the_order_of_both_streams() {
    static ORDER: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    struct Tagged(u8);
    impl Write for Tagged {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            let mut o = ORDER.lock().unwrap();
            for line in b.split_inclusive(|&c| c == b'\n') {
                o.push(self.0);
                o.extend_from_slice(line);
            }
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let o = bounded(
        "each(0..3, i => if i % 2 == 0: print(i) else: eprint(i) ;;)\ndbg(9)\nwrite(\"w\\n\")\newrite(\"v\\n\")\nprint(\"end\")",
        |_, _| (Box::new(Tagged(b'o')), Box::new(Tagged(b'e')), false),
    );
    assert_eq!(o.exit, 0);
    // write and ewrite go the same way (D73)
    assert_eq!(
        String::from_utf8_lossy(&ORDER.lock().unwrap()),
        "o0\ne1\no2\ne[-e:2] 9 = 9\now\nev\noend\n"
    );
}

/// Without a writer thread (none could be made) the task writes itself,
/// as before.
#[test]
fn without_a_thread_the_task_writes() {
    let o = bounded(
        "print(\"a\")\neprint(\"b\")\nprint(dbg(1) + 1)",
        |out, err| {
            super::output::no_thread_for_tests(true);
            (Box::new(out), Box::new(err), true)
        },
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("a\n2\n", "b\n[-e:3] 1 = 1\n", 0)
    );
}

/// A write that failed after `print` queued it fails the run at its end
/// (E0801), as a failed write did in `print`.
#[test]
fn a_failed_write_fails_the_run() {
    struct Full;
    impl Write for Full {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("no space left on device"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let o = bounded("print(\"a\")", |_, err| {
        (Box::new(Full), Box::new(err), true)
    });
    assert_eq!(o.exit, 1, "{}", o.err);
    assert!(
        o.err.contains("E0801") && o.err.contains("no space left on device"),
        "{}",
        o.err
    );
}
