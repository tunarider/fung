//! The line editor's state: one input
//! line plus the session history, apart from the terminal so that key
//! sequences can test it.

/// One decoded key press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Rune(char),
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Up,
    Down,
    CtrlC,
    CtrlD,
    CtrlL,
}

/// A rune buffer, a cursor, and history navigation (08).
#[derive(Default)]
pub struct Editor {
    buf: Vec<char>,
    cur: usize,
    history: Vec<String>,
    /// Index into `history` while browsing; `history.len()` is the draft.
    hist: usize,
    /// The line being typed before browsing started.
    draft: Vec<char>,
}

impl Editor {
    /// Applies one key. On Enter it returns the finished line.
    pub fn key(&mut self, k: Key) -> Option<String> {
        match k {
            Key::Rune(r) => {
                self.buf.insert(self.cur, r);
                self.cur += 1;
            }
            Key::Enter => {
                let line: String = self.buf.iter().collect();
                if !line.is_empty() && self.history.last() != Some(&line) {
                    self.history.push(line.clone());
                }
                self.buf.clear();
                self.cur = 0;
                self.draft.clear();
                self.hist = self.history.len();
                return Some(line);
            }
            Key::Backspace => {
                if self.cur > 0 {
                    self.buf.remove(self.cur - 1);
                    self.cur -= 1;
                }
            }
            Key::Delete | Key::CtrlD => {
                if self.cur < self.buf.len() {
                    self.buf.remove(self.cur);
                }
            }
            Key::Left => self.cur = self.cur.saturating_sub(1),
            Key::Right => {
                if self.cur < self.buf.len() {
                    self.cur += 1;
                }
            }
            Key::Home => self.cur = 0,
            Key::End => self.cur = self.buf.len(),
            Key::Up => {
                if self.hist == 0 || self.history.is_empty() {
                    return None;
                }
                if self.hist == self.history.len() {
                    self.draft = self.buf.clone();
                }
                self.hist -= 1;
                self.set(self.history[self.hist].chars().collect());
            }
            Key::Down => {
                if self.hist >= self.history.len() {
                    return None;
                }
                self.hist += 1;
                if self.hist == self.history.len() {
                    self.set(self.draft.clone());
                } else {
                    self.set(self.history[self.hist].chars().collect());
                }
            }
            Key::CtrlC => {
                self.buf.clear();
                self.cur = 0;
                self.hist = self.history.len();
            }
            Key::CtrlL => {}
        }
        None
    }

    /// Seeds the history (from the history file, 08) before the first
    /// line is read.
    pub fn set_history(&mut self, lines: &[String]) {
        self.history = lines.to_vec();
        self.hist = self.history.len();
    }

    fn set(&mut self, buf: Vec<char>) {
        self.buf = buf;
        self.cur = self.buf.len();
    }

    /// The line text and the cursor position in runes.
    pub fn render(&self) -> (String, usize) {
        (self.buf.iter().collect(), self.cur)
    }

    /// Whether nothing has been typed (Ctrl-D then ends the input).
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_keys(e: &mut Editor, s: &str) {
        for r in s.chars() {
            e.key(Key::Rune(r));
        }
    }

    #[test]
    fn insert_move_delete() {
        let mut e = Editor::default();
        type_keys(&mut e, "참치회");
        e.key(Key::Left);
        e.key(Key::Backspace);
        type_keys(&mut e, "x");
        assert_eq!(e.render(), ("참x회".to_string(), 2));
        e.key(Key::Home);
        e.key(Key::Delete);
        e.key(Key::End);
        type_keys(&mut e, "!");
        assert_eq!(e.key(Key::Enter).as_deref(), Some("x회!"));
        assert_eq!(e.render().0, "", "enter clears the buffer");
        e.key(Key::Left); // no-ops at the edges
        e.key(Key::Backspace);
        e.key(Key::Delete);
        assert_eq!(e.render(), (String::new(), 0), "edge moves");
    }

    #[test]
    fn history() {
        let mut e = Editor::default();
        for l in ["a", "b"] {
            type_keys(&mut e, l);
            e.key(Key::Enter);
        }
        type_keys(&mut e, "c");
        e.key(Key::Up);
        assert_eq!(e.render().0, "b");
        e.key(Key::Up);
        e.key(Key::Up); // stays at the oldest
        assert_eq!(e.render().0, "a");
        e.key(Key::Down);
        e.key(Key::Down);
        assert_eq!(e.render().0, "c", "down restores the draft");
        e.key(Key::CtrlC);
        assert_eq!(e.render().0, "", "ctrl-c clears");
        e.key(Key::Enter); // empty lines are not recorded
        assert_eq!(e.history.len(), 2, "{:?}", e.history);
        type_keys(&mut e, "b");
        e.key(Key::Enter); // a repeat of the last entry is not recorded twice
        assert_eq!(e.history.len(), 2, "{:?}", e.history);
    }

    #[test]
    fn seeded_history() {
        let mut e = Editor::default();
        e.set_history(&["first".to_string(), "second".to_string()]);
        e.key(Key::Up);
        assert_eq!(e.render().0, "second", "Up after seeding");
        e.key(Key::Up);
        assert_eq!(e.render().0, "first", "Up twice");
    }

    /// Ctrl-D deletes under the cursor (ending the input when the line is
    /// empty is the reader's part); Ctrl-L changes nothing; the cursor
    /// moves stay inside the line.
    #[test]
    fn other_keys() {
        let mut e = Editor::default();
        type_keys(&mut e, "abc");
        e.key(Key::Home);
        e.key(Key::CtrlD);
        e.key(Key::CtrlL);
        e.key(Key::Right);
        e.key(Key::Right);
        e.key(Key::Right);
        assert_eq!(e.render(), ("bc".to_string(), 2));
        assert!(!e.is_empty());
        e.key(Key::Down); // not browsing: nothing
        assert_eq!(e.render().0, "bc");
    }
}
