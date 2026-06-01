use crate::ast::path::Path;
use crate::diagnostics::span::Span;

#[derive(Clone, Debug)]
pub struct Ty {
    pub span: Span,
    pub kind: TyKind,
}

#[derive(Clone, Debug)]
pub enum TyKind {
    Named { path: Path, args: Vec<Ty> },
    Borrow { is_mut: bool, inner: Box<Ty> },
    Magnet { is_mut: bool, inner: Box<Ty> },
    Address(Box<Ty>),
    Array {
        elem: Box<Ty>,
        size: Box<crate::ast::expr::Expr>,
    },
    Span(Box<Ty>),
    Outcome { ok: Box<Ty>, err: Box<Ty> },
    Maybe(Box<Ty>),
    SelfTy,
    Unit,
    Infer,
    Tuple(Vec<Ty>),
}

impl Ty {
    pub fn new(span: Span, kind: TyKind) -> Self {
        Self { span, kind }
    }
    pub fn placeholder(span: Span) -> Self {
        Self {
            span,
            kind: TyKind::Infer,
        }
    }
    pub fn is_unit(&self) -> bool {
        matches!(self.kind, TyKind::Unit)
    }
}
