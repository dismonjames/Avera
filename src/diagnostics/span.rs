use crate::symbol::{Arena, FileId};
use std::path::{Path, PathBuf};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Span {
    pub file: FileId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub const fn new(file: FileId, start: u32, end: u32) -> Self {
        Self { file, start, end }
    }
    pub fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }
    pub fn is_empty(self) -> bool {
        self.start >= self.end
    }
    pub const fn point(file: FileId, pos: u32) -> Self {
        Self {
            file,
            start: pos,
            end: pos,
        }
    }
    pub fn union(a: Span, b: Span) -> Span {
        debug_assert_eq!(a.file, b.file, "union across files");
        Span {
            file: a.file,
            start: a.start.min(b.start),
            end: a.end.max(b.end),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct LineCol {
    pub line: u32,
    pub col: u32,
}

#[derive(Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub name: String,
    pub src: String,
    pub line_starts: Vec<u32>,
}

impl SourceFile {
    pub fn new(path: impl Into<PathBuf>, name: impl Into<String>, src: impl Into<String>) -> Self {
        let src = src.into();
        let mut line_starts = vec![0u32];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self {
            path: path.into(),
            name: name.into(),
            src,
            line_starts,
        }
    }
    pub fn line_col(&self, offset: u32) -> LineCol {
        let offset = offset.min(self.src.len() as u32);
        // binary search the line index
        let idx = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let line = idx as u32 + 1;
        let line_start = self.line_starts[idx];
        let col = offset - line_start + 1;
        LineCol { line, col }
    }
    pub fn slice(&self, span: Span) -> &str {
        let s = span.start.min(self.src.len() as u32) as usize;
        let e = span.end.min(self.src.len() as u32) as usize;
        &self.src[s..e.max(s)]
    }
    pub fn line_text(&self, line: u32) -> &str {
        let i = (line as usize)
            .saturating_sub(1)
            .min(self.line_starts.len() - 1);
        let start = self.line_starts[i] as usize;
        let end = if i + 1 < self.line_starts.len() {
            (self.line_starts[i + 1] as usize).saturating_sub(1)
        } else {
            self.src.len()
        };
        let end = end.min(self.src.len());
        let mut slice = &self.src[start..end];
        // strip a trailing \r
        if slice.ends_with('\r') {
            slice = &slice[..slice.len() - 1];
        }
        slice
    }
}

#[derive(Default)]
pub struct SourceMap {
    files: Arena<FileId, SourceFile>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn load(&mut self, path: &Path, src: impl Into<String>) -> FileId {
        let name = path
            .to_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("<file {}>", self.files.len()));
        self.load_named(path, name, src)
    }
    pub fn load_named(
        &mut self,
        path: &Path,
        name: impl Into<String>,
        src: impl Into<String>,
    ) -> FileId {
        self.files.push(SourceFile::new(path, name, src))
    }
    pub fn load_str(&mut self, name: impl Into<String>, src: impl Into<String>) -> FileId {
        let name = name.into();
        self.files
            .push(SourceFile::new(Path::new(&name), name.clone(), src))
    }
    pub fn file(&self, id: FileId) -> &SourceFile {
        &self.files[id]
    }
    pub fn span_text(&self, span: Span) -> &str {
        self.files[span.file].slice(span)
    }
    pub fn line_col(&self, span: Span) -> LineCol {
        self.files[span.file].line_col(span.start)
    }
    pub fn len(&self) -> usize {
        self.files.len()
    }
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}
