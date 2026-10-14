//! Source files and positions.

/// Index of a file registered in a [`SourceMap`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FileId(pub u32);

/// Owns the names and texts of every source file seen by a run.
#[derive(Default, Debug)]
pub struct SourceMap {
    files: Vec<(String, String)>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, name: &str, text: &str) -> FileId {
        self.files.push((name.to_string(), text.to_string()));
        FileId((self.files.len() - 1) as u32)
    }

    pub fn name(&self, id: FileId) -> &str {
        &self.files[id.0 as usize].0
    }

    pub fn text(&self, id: FileId) -> &str {
        &self.files[id.0 as usize].1
    }

    /// The number of files registered.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// A source position. `line` and `col` are 1-based and `col` counts
/// characters. Line 0 means "unknown".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pos {
    pub file: FileId,
    pub line: u32,
    pub col: u32,
}

impl Pos {
    pub fn is_zero(&self) -> bool {
        self.line == 0
    }
}

/// Decodes bytes the way Go reads a string rune by rune: every byte that
/// is not part of a valid UTF-8 sequence becomes one U+FFFD (Rust's
/// `from_utf8_lossy` would merge a whole invalid run into one).
pub fn decode_go(mut bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    loop {
        match std::str::from_utf8(bytes) {
            Ok(s) => {
                out.push_str(s);
                return out;
            }
            Err(e) => {
                let (ok, rest) = bytes.split_at(e.valid_up_to());
                // `valid_up_to` marks a valid UTF-8 prefix.
                out.push_str(std::str::from_utf8(ok).unwrap_or_default());
                let bad = e.error_len().unwrap_or(rest.len());
                out.extend(std::iter::repeat_n('\u{fffd}', bad));
                bytes = &rest[bad..];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::decode_go;

    #[test]
    fn decode_go_replaces_each_bad_byte() {
        let cases: &[(&[u8], &str)] = &[
            (b"abc", "abc"),
            (b"", ""),
            (b"\xeb\x8b", "\u{fffd}\u{fffd}"),
            (
                b"x = \"\xeb\x8b\" + nope",
                "x = \"\u{fffd}\u{fffd}\" + nope",
            ),
            (b"a\xffb", "a\u{fffd}b"),
            (b"\xff\xfe", "\u{fffd}\u{fffd}"),
            (b"\xed\xa0\x80", "\u{fffd}\u{fffd}\u{fffd}"),
            (b"\xf0\x9f\x98", "\u{fffd}\u{fffd}\u{fffd}"),
            (b"\xed\x95\x9c\xc3", "\u{d55c}\u{fffd}"),
            (b"\xf0\x9f\x98a", "\u{fffd}\u{fffd}\u{fffd}a"),
            ("한글".as_bytes(), "한글"),
        ];
        for (input, want) in cases {
            assert_eq!(decode_go(input), *want, "{input:?}");
        }
    }
}
