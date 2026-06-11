use crate::ast::path::Path;
use crate::ast::stmt::Block;
use crate::ast::ty::Ty;
use crate::diagnostics::span::Span;

#[derive(Clone, Debug)]
pub struct Item {
    pub span: Span,
    pub kind: ItemKind,
    pub attrs: Vec<Attr>,
    pub exported: bool,
}

#[derive(Clone, Debug)]
pub struct Attr {
    pub span: Span,
    pub name: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum ItemKind {
    Shape(ShapeDecl),
    Choice(ChoiceDecl),
    Ability(AbilityDecl),
    Impl(ImplDecl),
    Action(ActionDecl),
    Foreign(ForeignDecl),
}

#[derive(Clone, Debug)]
pub struct ShapeDecl {
    pub span: Span,
    pub name: String,
    pub generics: Vec<String>,
    pub fields: Vec<ShapeField>,
}

#[derive(Clone, Debug)]
pub struct ShapeField {
    pub span: Span,
    pub name: String,
    pub ty: Ty,
}

#[derive(Clone, Debug)]
pub struct ChoiceDecl {
    pub span: Span,
    pub name: String,
    pub generics: Vec<String>,
    pub variants: Vec<Variant>,
}

#[derive(Clone, Debug)]
pub struct Variant {
    pub span: Span,
    pub name: String,
    pub payload: Option<Vec<(String, Ty)>>,
}

#[derive(Clone, Debug)]
pub struct AbilityDecl {
    pub span: Span,
    pub name: String,
    pub generics: Vec<String>,
    pub actions: Vec<ActionSig>,
}

#[derive(Clone, Debug)]
pub struct ActionSig {
    pub span: Span,
    pub name: String,
    pub receiver: Option<Path>,
    pub params: Vec<Param>,
    pub ret: Option<Ty>,
}

#[derive(Clone, Debug)]
pub struct ImplDecl {
    pub span: Span,
    pub ability: Option<Path>,
    pub target: Ty,
    pub generics: Vec<String>,
    pub actions: Vec<ActionDecl>,
}

#[derive(Clone, Debug)]
pub struct ActionDecl {
    pub span: Span,
    pub sig: ActionSig,
    pub body: Option<Block>,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub span: Span,
    pub mode: ParamMode,
    pub name: String,
    pub ty: Ty,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ParamMode {
    Value,
    Borrow,
    BorrowMut,
    Move,
    SelfRef,
    SelfMut,
    SelfMove,
}

#[derive(Clone, Debug)]
pub struct ForeignDecl {
    pub span: Span,
    pub abi: String,
    pub lib: Option<String>,
    pub items: Vec<ForeignItem>,
}

#[derive(Clone, Debug)]
pub struct ForeignItem {
    pub span: Span,
    pub sig: ActionSig,
    pub link_name: Option<String>,
    pub raw: bool,
}
