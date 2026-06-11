pub mod expr;
pub mod item;
pub mod pat;
pub mod path;
pub mod stmt;
pub mod ty;
pub mod visit;

pub use expr::{BinOp, Expr, ExprKind, UnOp};
pub use item::{ForeignItem, Item, ItemKind, ShapeField, Variant};
pub use pat::{Pat, PatKind};
pub use path::Path;
pub use stmt::{AssignOp, Stmt, StmtKind};
pub use ty::{Ty, TyKind};

use crate::diagnostics::span::Span;

#[derive(Clone, Debug)]
pub struct Module {
    pub span: Span,
    pub directives: Vec<Directive>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub struct Directive {
    pub span: Span,
    pub kind: DirectiveKind,
}

#[derive(Clone, Debug)]
pub enum DirectiveKind {
    Import { path: Path, alias: Option<String> },
    Module(Path),
    Header(String),
    Source(String),
    Depends(Path),
    Cfg(String),
    Target(String),
    Link(String),
    Other(String, Vec<String>),
}
