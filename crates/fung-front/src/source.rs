//! Source files and positions.

/// Index of a file registered in a [`SourceMap`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FileId(pub u32);

/// Owns the names and texts of every source file seen by a run.
#[derive(Clone, Default, Debug)]
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

/// Bytes as text: each maximal invalid UTF-8 subsequence becomes one
/// U+FFFD, as Unicode recommends (02).
pub fn decode_lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::decode_lossy;

    /// Each maximal invalid subsequence becomes one U+FFFD (the Unicode
    /// recommendation): a cut-off character is one, two stray bytes two.
    #[test]
    fn invalid_utf8_reads_as_one_replacement_per_bad_part() {
        assert_eq!(decode_lossy(b"a\xE2\x82b"), "a\u{FFFD}b");
        assert_eq!(decode_lossy(b"\xFF\xFF"), "\u{FFFD}\u{FFFD}");
        assert_eq!(decode_lossy("파일".as_bytes()), "파일");
    }

    #[test]
    fn decode_lossy_cases() {
        let cases: &[(&[u8], &str)] = &[
            (b"abc", "abc"),
            (b"", ""),
            (b"\xeb\x8b", "\u{fffd}"),
            (b"x = \"\xeb\x8b\" + nope", "x = \"\u{fffd}\" + nope"),
            (b"a\xffb", "a\u{fffd}b"),
            (b"\xff\xfe", "\u{fffd}\u{fffd}"),
            (b"\xed\xa0\x80", "\u{fffd}\u{fffd}\u{fffd}"),
            (b"\xf0\x9f\x98", "\u{fffd}"),
            (b"\xed\x95\x9c\xc3", "\u{d55c}\u{fffd}"),
            (b"\xf0\x9f\x98a", "\u{fffd}a"),
            ("한글".as_bytes(), "한글"),
        ];
        for (input, want) in cases {
            assert_eq!(decode_lossy(input), *want, "{input:?}");
        }
    }
}
