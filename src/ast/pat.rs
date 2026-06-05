use crate::diagnostics::span::Span;

#[derive(Clone, Debug)]
pub struct Pat {
    pub span: Span,
    pub kind: PatKind,
}

impl Pat {
    pub fn new(span: Span, kind: PatKind) -> Self {
        Self { span, kind }
    }
}

#[derive(Clone, Debug)]
pub enum PatKind {
    Variant {
        name: String,
        binds: Vec<PatBind>,
    },
    VariantNoPayload(String),
    Bind(String),
    Wild,
    IntLit(i64),
}
#[derive(Clone, Debug)]
pub struct PatBind {
    pub span: Span,
    pub name: String,
}
