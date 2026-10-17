//! The REPL on a pseudo-terminal (Rust only; stage 5 task 4): the line
//! editor gets raw keys while it reads a line, and the terminal is
//! restored after every line, at the end of input and on `exit(n)`.
//! A pipe gets the plain reader and leaves the terminal alone.
//!
//! A scripted session's screen output, exit code and history file are
//! kept in `expected/repl/tty.txt`.
//!
//! The steps are ordered by state, not by sleeping: the test waits until
//! the terminal is in raw mode (the editor is reading) or until the
//! output holds what it expects, each with a deadline.

use std::io::{Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fung_vm::term::pty;

mod common;

const LIMIT: Duration = Duration::from_secs(20);

struct Tty {
    /// Killed and waited for when the `Tty` is dropped, as a panicking
    /// test does.
    child: common::Reaped,
    controller: std::fs::File,
    terminal: OwnedFd,
    out: Arc<Mutex<Vec<u8>>>,
    home: std::path::PathBuf,
    /// How much output there was when keys were last sent.
    sent_at: usize,
    /// How many line ends (`\r\n`) the output must hold before the
    /// editor can be on the line after the last batch: one more for every
    /// Enter and every Ctrl-C (`^C\n`) in it. Without this, an empty
    /// prompt drawn in the middle of a batch (Home on an empty line, the
    /// line after Ctrl-C) would look like the fresh line after it.
    lines_due: usize,
}

/// Starts `fung` with standard input, output and error on a new
/// pseudo-terminal, `HOME` in a scratch directory.
fn start(tag: &str, stdin_pipe: bool) -> Tty {
    start_bin(&common::rust_fung(), tag, stdin_pipe, None)
}

/// `start` for any binary, with a history file to begin with.
fn start_bin(bin: &std::path::Path, tag: &str, stdin_pipe: bool, history: Option<&str>) -> Tty {
    let home = std::env::temp_dir().join(format!("fung-tty-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    if let Some(h) = history {
        std::fs::write(home.join(".fung_history"), h).unwrap();
    }
    let (controller, terminal) = pty::open().unwrap();
    let stdin = if stdin_pipe {
        Stdio::piped()
    } else {
        Stdio::from(terminal.try_clone().unwrap())
    };
    // killed and waited for if anything below panics (and when the
    // `Tty` is dropped)
    let child = common::Reaped::new(
        Command::new(bin)
            .env("HOME", &home)
            .current_dir(&home)
            .stdin(stdin)
            .stdout(Stdio::from(terminal.try_clone().unwrap()))
            .stderr(Stdio::from(terminal.try_clone().unwrap()))
            .spawn()
            .unwrap(),
    );
    let controller = std::fs::File::from(controller);
    let out = Arc::new(Mutex::new(Vec::new()));
    {
        let mut r = controller.try_clone().unwrap();
        let out = out.clone();
        // Ends when the terminal side is closed (EIO) or on any error.
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = r.read(&mut buf) {
                if n == 0 {
                    break;
                }
                out.lock().unwrap().extend_from_slice(&buf[..n]);
            }
        });
    }
    Tty {
        child,
        controller,
        terminal,
        out,
        home,
        sent_at: 0,
        lines_due: 0,
    }
}

impl Tty {
    fn raw(&self) -> bool {
        pty::is_raw(self.terminal.as_raw_fd()).unwrap()
    }

    /// Waits until `f` holds.
    fn until(&self, what: &str, f: impl Fn(&Tty) -> bool) {
        let end = Instant::now() + LIMIT;
        while !f(self) {
            assert!(
                Instant::now() < end,
                "timed out waiting for {what}; output: {:?}",
                self.text()
            );
            std::thread::yield_now();
        }
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.out.lock().unwrap()).into_owned()
    }

    /// Whether the editor waits on the fresh line after the last batch:
    /// every line end the batch causes is out, the output grew since the
    /// keys were sent and ends with an empty line drawn after a prompt,
    /// and raw mode is on.
    fn ready(&self) -> bool {
        let out = self.out.lock().unwrap();
        line_ends(&out) >= self.lines_due
            && out.len() > self.sent_at
            && (out.ends_with(b"\r> \x1b[K") || out.ends_with(b"\r. \x1b[K"))
            && self.raw()
    }

    /// Types `keys` once the editor waits on a fresh line, so no key
    /// reaches the terminal while it is not in raw mode.
    fn send(&mut self, keys: &[u8]) {
        self.until("the editor", Tty::ready);
        let out = self.out.lock().unwrap();
        self.sent_at = out.len();
        let breaks = keys.iter().filter(|&&k| k == b'\r' || k == 0x03).count();
        self.lines_due = line_ends(&out) + breaks;
        drop(out);
        self.controller.write_all(keys).unwrap();
    }

    /// Waits for the process to end and returns its exit code.
    fn wait(&mut self) -> Option<i32> {
        let end = Instant::now() + LIMIT;
        loop {
            if let Some(st) = self.child.try_wait().unwrap() {
                return st.code();
            }
            assert!(
                Instant::now() < end,
                "fung did not end; output: {:?}",
                self.text()
            );
            std::thread::yield_now();
        }
    }
}

/// How many `\r\n` the output holds.
fn line_ends(out: &[u8]) -> usize {
    out.windows(2).filter(|w| w == b"\r\n").count()
}

impl Drop for Tty {
    fn drop(&mut self) {
        // Ended before its home goes; `Reaped` would kill it only after.
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

#[test]
fn the_editor_reads_keys_and_restores_the_terminal() {
    let mut t = start("edit", false);
    let cooked = pty::lflag(t.terminal.as_raw_fd()).unwrap();
    t.send(b"1 + 1\r");
    t.until("the value", |t| t.text().contains("> 1 + 1\x1b[K\r\n2\r\n"));
    // Editing: Backspace, Home (Ctrl-A), End (Ctrl-E), the arrows.
    t.send(b"x\x7f2 * 4\x013 * \x05\x1b[D\x1b[C\r");
    t.until("the edited line", |t| t.text().contains("\r\n24\r\n"));
    // Up brings the previous lines back, Down goes forward again.
    t.send(b"\x1b[A\x1b[A\x1b[B\r");
    t.until("history", |t| t.text().matches("\r\n24\r\n").count() == 2);
    // Ctrl-C drops the line.
    t.send(b"oops\x03");
    t.until("^C", |t| t.text().contains("oops\x1b[K^C\r\n"));
    // A line that needs more is continued with the second prompt.
    t.send(b"fn f(x):\r");
    t.send(b"  x * 10\r");
    t.send(b";;\r");
    t.send(b"f(4)\r");
    t.until("f(4)", |t| t.text().contains("40\r\n"));
    // Ctrl-D on an empty line ends the session.
    t.send(b"\x04");
    assert_eq!(t.wait(), Some(0));
    assert!(!t.raw());
    assert_eq!(pty::lflag(t.terminal.as_raw_fd()).unwrap(), cooked);
    let hist = std::fs::read_to_string(t.home.join(".fung_history")).unwrap();
    assert_eq!(hist, "1 + 1\n3 * 2 * 4\nfn f(x):\n  x * 10\n;;\nf(4)\n");
}

#[test]
fn exit_restores_the_terminal() {
    let mut t = start("exit", false);
    let cooked = pty::lflag(t.terminal.as_raw_fd()).unwrap();
    t.send(b"at_exit(() => print(\"bye\"))\r");
    t.send(b"exit(3)\r");
    assert_eq!(t.wait(), Some(3));
    assert!(!t.raw());
    assert!(t.text().contains("bye"), "{:?}", t.text());
    assert_eq!(pty::lflag(t.terminal.as_raw_fd()).unwrap(), cooked);
}

/// Standard input that is not a terminal gets plain line reading: raw
/// mode is never set (on the terminal the output goes to) and the line
/// is not drawn with escape sequences.
#[test]
fn a_pipe_never_sets_raw_mode() {
    let mut t = start("pipe", true);
    let cooked = pty::lflag(t.terminal.as_raw_fd()).unwrap();
    let mut stdin = t.child.stdin.take().unwrap();
    stdin.write_all(b"1 + 1\n").unwrap();
    t.until("the value", |t| t.text().contains("2"));
    assert!(!t.raw());
    drop(stdin);
    assert_eq!(t.wait(), Some(0));
    assert_eq!(pty::lflag(t.terminal.as_raw_fd()).unwrap(), cooked);
    let text = t.text();
    assert!(!text.contains('\x1b'), "{text:?}");
    assert_eq!(text, "> 2\r\n> \r\n");
    assert!(
        !t.home.join(".fung_history").exists(),
        "no history without the editor"
    );
}

/// One scripted session on a binary: the screen output, the exit code
/// and the history file afterwards.
fn session(bin: &std::path::Path, tag: &str) -> (String, Option<i32>, String) {
    let seed: String = (0..1002).map(|i| format!("{i}\n")).collect::<String>() + "x = 41\n";
    let mut t = start_bin(bin, tag, false, Some(&seed));
    let script: &[&[u8]] = &[
        b"\x1b[A\r",                                  // the seeded history
        "참치 = x + 1\r".as_bytes(),                  // wide characters count one column
        "참치\x1b[D\x1b[D\x1b[3~\x1b[F\r".as_bytes(), // Delete: 치
        b"\x1b[1~\x1b[4~\x1b[7~\x1b[8~\x1b[H\x1bOH\x1bOF1\r",
        b"abc\x01\x04\x05\x0c\x08\r", // Ctrl-D deletes, Ctrl-L redraws
        b"nope\r",                    // an error goes to the same terminal
        b"if true:\r",
        b"  1\r",
        b";;\r",
        b"ab\x03\x1b[A\x1b[B\x1b[B\x1b[A\x1b[A\r",
        b"\x1bx\x1b[Z\x1b[2~\x02print(1)\r",
        b"\x04",
    ];
    for keys in script {
        t.send(keys);
    }
    let code = t.wait();
    let hist = std::fs::read_to_string(t.home.join(".fung_history")).unwrap();
    (t.text(), code, hist)
}

#[test]
fn the_editor_matches_the_expected_output() {
    let (screen, code, history) = session(&common::rust_fung(), "session");
    assert!(history.starts_with("3\n"), "trimmed to the latest lines");
    let got = [common::expected::Entry::new("session")
        .field("screen", screen)
        .field("exit", code.map_or("none".to_string(), |c| c.to_string()))
        .field("history", history)];
    common::expected::verify(
        "repl/tty.txt",
        "The REPL's line editor on a pseudo-terminal: the screen output, exit code and\n\
         history file of the scripted session in crates/fung-cli/tests/repl_tty.rs (`session`).\n\
         Update: FUNG_EXPECTED=bless cargo test -p fung-cli --test repl_tty",
        64 * 1024,
        &got,
    );
}
