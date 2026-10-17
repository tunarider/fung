//! Reading input lines (Go `internal/repl/line_plain.go` and
//! `line_unix.go`): plain line reading for pipes and files, and the
//! raw-mode line editor for a terminal.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, RawFd};

use fung_front::source::decode_go;
use fung_vm::term::RawMode;

use super::editor::{Editor, Key};
use super::history::History;

/// Go `bufio.Reader`'s default size, which both readers use.
const BUFFER: usize = 4096;

/// Supplies one line per call; `None` ends the session.
pub trait LineReader {
    fn read_line(&mut self, prompt: &str) -> Option<String>;
}

/// Lines without editing: pipes, files, tests (08). The prompt is
/// echoed.
pub struct PlainReader<R: Read, W: Write> {
    input: BufReader<R>,
    out: W,
}

impl<R: Read, W: Write> PlainReader<R, W> {
    pub fn new(input: R, out: W) -> Self {
        PlainReader {
            input: BufReader::with_capacity(BUFFER, input),
            out,
        }
    }
}

impl<R: Read, W: Write> LineReader for PlainReader<R, W> {
    fn read_line(&mut self, prompt: &str) -> Option<String> {
        let _ = self.out.write_all(prompt.as_bytes());
        let mut line = Vec::new();
        // A read error ends the input like its end (Go: the error with
        // an empty line).
        let _ = self.input.read_until(b'\n', &mut line);
        if line.is_empty() {
            return None;
        }
        let line = line.strip_suffix(b"\n").unwrap_or(&line);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        Some(decode_go(line))
    }
}

/// Decodes key presses from the bytes a raw-mode terminal sends (Go
/// `readKey`): control characters, escape sequences for the arrows,
/// Home, End and Delete, or a UTF-8 rune.
pub struct Keys<R: Read> {
    input: BufReader<R>,
    /// Continuation bytes of a broken sequence already taken: each is
    /// one U+FFFD still to come (Go takes them one at a time).
    pending: usize,
}

impl<R: Read> Keys<R> {
    pub fn new(input: R) -> Self {
        Keys {
            input: BufReader::with_capacity(BUFFER, input),
            pending: 0,
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.input.fill_buf().ok()?.first().copied()
    }

    fn byte(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.input.consume(1);
        Some(b)
    }

    /// One rune the way Go's `bufio.Reader.ReadRune` decodes it: a byte
    /// that does not start a valid UTF-8 sequence is one U+FFFD.
    fn rune(&mut self) -> Option<char> {
        if self.pending > 0 {
            self.pending -= 1;
            return Some(char::REPLACEMENT_CHARACTER);
        }
        let b = self.byte()?;
        let n = match b {
            0x00..=0x7f => return Some(b as char),
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => return Some(char::REPLACEMENT_CHARACTER),
        };
        let mut seq = vec![b];
        // Take continuation bytes while the prefix can still be valid;
        // the first one that cannot stays for the next rune.
        while seq.len() < n {
            let Some(c) = self.peek() else { break };
            let mut next = seq.clone();
            next.push(c);
            if !valid_prefix(&next) {
                break;
            }
            self.input.consume(1);
            seq = next;
        }
        if seq.len() < n {
            // Go consumes only the first byte of a broken sequence; the
            // continuation bytes already taken become U+FFFD each.
            self.pending = seq.len() - 1;
            return Some(char::REPLACEMENT_CHARACTER);
        }
        std::str::from_utf8(&seq).ok()?.chars().next()
    }
}

/// Whether `b` (a lead byte and continuation bytes) can begin a valid
/// UTF-8 sequence.
fn valid_prefix(b: &[u8]) -> bool {
    match std::str::from_utf8(b) {
        Ok(_) => true,
        Err(e) => e.valid_up_to() == 0 && e.error_len().is_none(),
    }
}

impl<R: Read> Keys<R> {
    /// The next key; `None` at the end of the input or on a read error.
    pub fn key(&mut self) -> Option<Key> {
        loop {
            let r = self.rune()?;
            return Some(match r {
                '\r' | '\n' => Key::Enter,
                '\x7f' | '\x08' => Key::Backspace,
                '\x01' => Key::Home,
                '\x05' => Key::End,
                '\x03' => Key::CtrlC,
                '\x04' => Key::CtrlD,
                '\x0c' => Key::CtrlL,
                '\x1b' => match self.escape()? {
                    Some(k) => k,
                    None => continue,
                },
                r if r < ' ' => continue, // other control keys are ignored
                r => Key::Rune(r),
            });
        }
    }

    /// What follows ESC (Go `readEscape`): `Some(None)` when it is no key
    /// the editor knows, which is skipped like Go does.
    fn escape(&mut self) -> Option<Option<Key>> {
        let b = self.byte()?;
        if b != b'[' && b != b'O' {
            return Some(None);
        }
        let c = self.byte()?;
        let k = match c {
            b'A' => Key::Up,
            b'B' => Key::Down,
            b'C' => Key::Right,
            b'D' => Key::Left,
            b'H' => Key::Home,
            b'F' => Key::End,
            b'0'..=b'9' => {
                // ESC [ n ~ : 1 home, 3 delete, 4 end, 7 home, 8 end
                let mut n = u64::from(c - b'0');
                loop {
                    let d = self.byte()?;
                    if d == b'~' {
                        break;
                    }
                    if d.is_ascii_digit() {
                        n = n.wrapping_mul(10).wrapping_add(u64::from(d - b'0'));
                    }
                }
                match n {
                    1 | 7 => Key::Home,
                    3 => Key::Delete,
                    4 | 8 => Key::End,
                    _ => return Some(None),
                }
            }
            _ => return Some(None),
        };
        Some(Some(k))
    }

    /// One line without editing, for a terminal raw mode cannot be set
    /// on (Go's fallback: all trailing CR and LF are cut).
    fn plain_line(&mut self) -> Option<String> {
        let mut line = Vec::new();
        let _ = self.input.read_until(b'\n', &mut line);
        if line.is_empty() {
            return None;
        }
        while line.last().is_some_and(|&c| c == b'\n' || c == b'\r') {
            line.pop();
        }
        Some(decode_go(&line))
    }
}

/// The raw-mode line editor for an interactive terminal (08): the
/// terminal delivers keys one at a time and this code draws the line.
pub struct TermReader<R: Read + AsRawFd, W: Write> {
    fd: RawFd,
    keys: Keys<R>,
    out: W,
    editor: Editor,
    /// `None`: history stays in memory.
    history: Option<History>,
}

impl<R: Read + AsRawFd, W: Write> TermReader<R, W> {
    /// A line editor when `input` is a terminal (Go `TermReader`: a
    /// character device; raw mode is set per line, and a device it
    /// cannot be set on reads plain lines).
    pub fn new(input: R, out: W) -> Option<Self> {
        let fd = input.as_raw_fd();
        if !fung_vm::term::is_char_device(fd) {
            return None;
        }
        Some(TermReader {
            fd,
            keys: Keys::new(input),
            out,
            editor: Editor::default(),
            history: None,
        })
    }

    /// Loads `path` into the editor's history and records every accepted
    /// line there (08). An unusable path leaves history in memory.
    pub fn use_history_file(&mut self, path: &std::path::Path) {
        if let Some(h) = History::open(path) {
            self.editor.set_history(&h.lines);
            self.history = Some(h);
        }
    }

    fn write(&mut self, s: &str) {
        let _ = self.out.write_all(s.as_bytes());
    }

    /// Repaints the prompt and the line, then places the cursor. Every
    /// rune is taken as one column wide (Go: wide characters are an
    /// open question).
    fn draw(&mut self, prompt: &str) {
        let (text, cur) = self.editor.render();
        let back = text.chars().count() - cur;
        let mut s = format!("\r{prompt}{text}\x1b[K");
        if back > 0 {
            s.push_str(&format!("\x1b[{back}D"));
        }
        self.write(&s);
    }
}

impl<R: Read + AsRawFd, W: Write> LineReader for TermReader<R, W> {
    fn read_line(&mut self, prompt: &str) -> Option<String> {
        let Ok(_raw) = RawMode::enable(self.fd) else {
            // Not a usable terminal after all: plain reading.
            self.write(prompt);
            return self.keys.plain_line();
        };
        // `_raw` restores the terminal when this returns or unwinds.
        self.draw(prompt);
        loop {
            let Some(k) = self.keys.key() else {
                self.write("\n");
                return None;
            };
            match k {
                Key::CtrlD if self.editor.is_empty() => {
                    self.write("\n");
                    return None;
                }
                Key::CtrlC => {
                    self.editor.key(k);
                    self.write("^C\n");
                    self.draw(prompt);
                    continue;
                }
                Key::CtrlL => {
                    self.write("\x1b[2J\x1b[H");
                    self.draw(prompt);
                    continue;
                }
                _ => {}
            }
            if let Some(line) = self.editor.key(k) {
                if let Some(h) = &mut self.history {
                    h.append(&line);
                }
                self.write("\n");
                return Some(line);
            }
            self.draw(prompt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(b: &[u8]) -> Vec<Key> {
        let mut k = Keys::new(b);
        std::iter::from_fn(|| k.key()).collect()
    }

    #[test]
    fn plain_reader_trims_one_line_ending() {
        let mut out = Vec::new();
        let mut r = PlainReader::new(&b"a\r\r\nb\n\nc"[..], &mut out);
        let lines: Vec<_> = std::iter::from_fn(|| r.read_line("> ")).collect();
        assert_eq!(lines, ["a\r", "b", "", "c"]);
        assert_eq!(out, b"> > > > > ");
    }

    #[test]
    fn keys_decode_like_go() {
        use Key::*;
        assert_eq!(
            keys(b"a\r\n\x7f\x08\x01\x05\x03\x04\x0c\x02z"),
            [
                Rune('a'),
                Enter,
                Enter,
                Backspace,
                Backspace,
                Home,
                End,
                CtrlC,
                CtrlD,
                CtrlL,
                Rune('z')
            ]
        );
        assert_eq!(
            keys(b"\x1b[A\x1b[B\x1bOC\x1b[D\x1b[H\x1bOF"),
            [Up, Down, Right, Left, Home, End]
        );
        assert_eq!(
            keys(b"\x1b[1~\x1b[3~\x1b[4~\x1b[7~\x1b[8~\x1b[2~\x1b[1;5~x"),
            [Home, Delete, End, Home, End, Rune('x')],
            "unknown numbers are skipped; other bytes before ~ are ignored"
        );
        assert_eq!(
            keys(b"\x1bxy\x1b[Zq"),
            [Rune('y'), Rune('q')],
            "unknown escapes are skipped"
        );
        assert_eq!(
            keys("참\u{1F600}".as_bytes()),
            [Rune('참'), Rune('\u{1F600}')]
        );
        let f = Rune('\u{fffd}');
        assert_eq!(
            keys(b"\xff\xe2\x82a"),
            [f, f, f, Rune('a')],
            "Go: one U+FFFD per byte"
        );
        assert_eq!(keys(b"\xe0\x80a"), [f, f, Rune('a')], "an overlong form");
        assert_eq!(keys(b"\xe2\x82"), [f, f], "cut at the end");
        assert_eq!(keys(b"\x1b["), [], "cut inside an escape");
    }

    #[test]
    fn a_device_that_is_not_a_terminal_reads_plain_lines() {
        let null = std::fs::File::open("/dev/null").unwrap();
        let mut out = Vec::new();
        let mut r = TermReader::new(null, &mut out).expect("a character device");
        assert_eq!(r.read_line("> "), None);
        assert_eq!(out, b"> ");
        let (rd, _w) = std::io::pipe().unwrap();
        assert!(
            TermReader::new(rd, Vec::new()).is_none(),
            "a pipe gets the plain reader"
        );
    }
}
