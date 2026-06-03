use crate::diagnostics::span::Span;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Path {
    pub span: Span,
    pub segments: Vec<String>,
}

impl Path {
    pub fn new(span: Span, segments: Vec<String>) -> Self {
        Self { span, segments }
    }
    pub fn single(span: Span, name: impl Into<String>) -> Self {
        Self {
            span,
            segments: vec![name.into()],
        }
    }
    pub fn is_wildcard(&self) -> bool {
        self.segments.last().map(|s| s == "*").unwrap_or(false)
    }
    pub fn as_str(&self) -> String {
        self.segments.join(".")
    }
}
