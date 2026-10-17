//! The history file (08, Go `internal/repl/history.go`): one input line
//! per line, loaded when the editor starts and appended after each
//! accepted line. Every operation fails silently: history is a
//! convenience and must never stop the REPL.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use fung_front::source::decode_go;

/// Only the latest lines stay.
pub const MAX_HISTORY: usize = 1000;

pub struct History {
    path: PathBuf,
    pub lines: Vec<String>,
}

/// Opens `path` for appending, creating it with mode 0600.
fn open_append(path: &Path) -> std::io::Result<std::fs::File> {
    OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .open(path)
}

impl History {
    /// Loads `path`, trimming (and rewriting) a file longer than
    /// `MAX_HISTORY` lines. `None` when the file cannot be written: the
    /// REPL then keeps history in memory only.
    pub fn open(path: &Path) -> Option<History> {
        let mut h = History {
            path: path.to_path_buf(),
            lines: Vec::new(),
        };
        if let Ok(b) = std::fs::read(path) {
            let b = b.strip_suffix(b"\n").unwrap_or(&b);
            if !b.is_empty() {
                h.lines = b.split(|&c| c == b'\n').map(decode_go).collect();
            }
        }
        if h.lines.len() > MAX_HISTORY {
            h.lines.drain(..h.lines.len() - MAX_HISTORY);
            let text = h.lines.join("\n") + "\n";
            let written = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(path)
                .and_then(|mut f| f.write_all(text.as_bytes()));
            written.ok()?;
        }
        open_append(path).ok()?;
        Some(h)
    }

    /// Records an accepted line, skipping empty lines and a repeat of the
    /// previous one (the rule the editor applies in memory).
    pub fn append(&mut self, line: &str) {
        if line.is_empty() || self.lines.last().is_some_and(|l| l == line) {
            return;
        }
        self.lines.push(line.to_string());
        if let Ok(mut f) = open_append(&self.path) {
            let _ = f.write_all(format!("{line}\n").as_bytes());
        }
    }
}

/// Port of Go `TestHistoryFileLoadAppendTrim`.
#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fung-hist-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_append_trim() {
        let dir = temp("lat");
        let p = dir.join("hist");
        std::fs::write(&p, "a = 1\nprint(a)\n").unwrap();
        let mut h = History::open(&p).unwrap();
        assert_eq!(h.lines.join("|"), "a = 1|print(a)", "load");
        h.append("b = 2");
        h.append("b = 2"); // consecutive duplicate: not stored
        h.append(""); // empty: not stored
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "a = 1\nprint(a)\nb = 2\n",
            "file after append"
        );
        std::fs::write(&p, "x\n".repeat(MAX_HISTORY + 50)).unwrap();
        let h = History::open(&p).unwrap();
        assert_eq!(h.lines.len(), MAX_HISTORY, "trim");
        let b = std::fs::read_to_string(&p).unwrap();
        assert_eq!(
            b.matches('\n').count(),
            MAX_HISTORY,
            "the trimmed file is rewritten"
        );
        // An unwritable path disables history silently.
        assert!(History::open(&dir.join("missing").join("hist")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_file_is_created_private_and_empty() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp("new");
        let p = dir.join("hist");
        let h = History::open(&p).unwrap();
        assert!(h.lines.is_empty());
        let mode = std::fs::metadata(&p).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        std::fs::write(&p, "\n").unwrap();
        assert!(
            History::open(&p).unwrap().lines.is_empty(),
            "one empty line is none"
        );
        std::fs::write(&p, "\n\n").unwrap();
        assert_eq!(History::open(&p).unwrap().lines, ["", ""], "Go keeps two");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
