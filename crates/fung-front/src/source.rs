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
