//! Reading input lines: plain line reading for pipes, files and devices,
//! and the raw-mode line editor for a terminal. Both take a line without
//! its end: one LF and one CR before it (a CR elsewhere is the line's).

use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, RawFd};

use fung_front::source::decode_lossy;
use fung_vm::term::RawMode;

use super::editor::{Editor, Key};
use super::history::History;

/// The read buffer of both readers.
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
        // a read error ends the input like its end
        let _ = self.input.read_until(b'\n', &mut line);
        if line.is_empty() {
            return None;
        }
        Some(decode_lossy(without_line_end(&line)))
    }
}

/// A line read up to its LF, without the LF and one CR before it.
fn without_line_end(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// Decodes key presses from the bytes a raw-mode terminal sends: control
/// characters, escape sequences for the arrows,
/// Home, End and Delete, or a UTF-8 rune.
pub struct Keys<R: Read> {
    input: BufReader<R>,
}

impl<R: Read> Keys<R> {
    pub fn new(input: R) -> Self {
        Keys {
            input: BufReader::with_capacity(BUFFER, input),
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

    /// One character: an invalid UTF-8 sequence (the longest start of a
    /// valid one, or a byte that starts none) is one U+FFFD, as fung
    /// decodes text everywhere.
    fn rune(&mut self) -> Option<char> {
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
            // the bytes taken are one invalid sequence
            return Some(char::REPLACEMENT_CHARACTER);
        }
        std::str::from_utf8(&seq).ok()?.chars().next()
    }
}

/// What repaints a line: back to column 0, the prompt and the text, the
/// rest of the screen line cleared, and the cursor moved back over what
/// follows character `cur`, by display columns (a wide character takes
/// two, a combining one none).
fn redraw(prompt: &str, text: &str, cur: usize) -> String {
    let back: usize = text.chars().skip(cur).map(fung_front::unicode::width).sum();
    let mut s = format!("\r{prompt}{text}\x1b[K");
    if back > 0 {
        s.push_str(&format!("\x1b[{back}D"));
    }
    s
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

    /// What follows ESC: `Some(None)` when it is no key the editor knows,
    /// which is skipped.
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
    /// on, ended as [`PlainReader`] ends one.
    fn plain_line(&mut self) -> Option<String> {
        let mut line = Vec::new();
        let _ = self.input.read_until(b'\n', &mut line);
        if line.is_empty() {
            return None;
        }
        Some(decode_lossy(without_line_end(&line)))
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
    /// A line editor when `input` is a terminal (raw mode is set per
    /// line; should it fail, the line is read plainly).
    pub fn new(input: R, out: W) -> Option<Self> {
        let fd = input.as_raw_fd();
        if !fung_vm::term::is_terminal(fd) {
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

    /// Repaints the prompt and the line, then places the cursor.
    fn draw(&mut self, prompt: &str) {
        let (text, cur) = self.editor.render();
        let s = redraw(prompt, &text, cur);
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
    fn the_plain_fallback_trims_one_line_ending_too() {
        let mut k = Keys::new(&b"a\r\r\nb\n"[..]);
        assert_eq!(k.plain_line().as_deref(), Some("a\r"));
        assert_eq!(k.plain_line().as_deref(), Some("b"));
        assert_eq!(k.plain_line(), None);
    }

    #[test]
    fn keys_decode() {
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
        // one U+FFFD for each invalid sequence, as everywhere in fung
        assert_eq!(
            keys(b"\xff\xe2\x82a"),
            [f, f, Rune('a')],
            "a stray byte, then a cut sequence"
        );
        assert_eq!(keys(b"\xe0\x80a"), [f, f, Rune('a')], "an overlong form");
        assert_eq!(keys(b"\xe2\x82"), [f], "cut at the end");
        assert_eq!(keys(b"\x1b["), [], "cut inside an escape");
    }

    #[test]
    fn the_cursor_moves_back_by_display_columns() {
        // the cursor after `한`: `a` and `글` are 3 columns to its right
        assert_eq!(redraw("> ", "한a글", 1), "\r> 한a글\x1b[K\x1b[3D");
        assert_eq!(redraw("> ", "ab", 2), "\r> ab\x1b[K");
        // a combining mark takes no column
        assert_eq!(redraw("> ", "e\u{301}x", 0), "\r> e\u{301}x\x1b[K\x1b[2D");
    }

    #[test]
    fn a_device_that_is_not_a_terminal_reads_plain_lines() {
        let null = std::fs::File::open("/dev/null").unwrap();
        assert!(
            TermReader::new(null, Vec::new()).is_none(),
            "a character device that is no terminal gets the plain reader"
        );
        let (rd, _w) = std::io::pipe().unwrap();
        assert!(
            TermReader::new(rd, Vec::new()).is_none(),
            "a pipe gets the plain reader"
        );
    }
}
