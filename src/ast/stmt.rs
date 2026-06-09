use crate::ast::expr::Expr;
use crate::ast::pat::Pat;
use crate::ast::ty::Ty;
use crate::diagnostics::span::Span;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub span: Span,
    pub kind: StmtKind,
}

impl Stmt {
    pub fn new(span: Span, kind: StmtKind) -> Self {
        Self { span, kind }
    }
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    OwnerBinding {
        name: String,
        ty: Option<Ty>,
        value: Expr,
    },
    ConstBinding {
        name: String,
        ty: Option<Ty>,
        value: Expr,
    },
    MagnetBinding {
        name: String,
        ty: Option<Ty>,
        target: Expr,
    },
    MagnetMutBinding {
        name: String,
        ty: Option<Ty>,
        target: Expr,
    },
    MagnetRetarget { magnet: Expr, target: Expr },
    Assignment {
        place: Expr,
        op: AssignOp,
        value: Expr,
    },
    Drop(Expr),
    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        else_branch: Option<Vec<Stmt>>,
    },
    While { cond: Expr, body: Vec<Stmt> },
    Loop(Vec<Stmt>),
    For {
        // `&` / `&!` / `^` modifier before the binding.
        mode: ForMode,
        pat: Pat,
        iter: Expr,
        body: Vec<Stmt>,
    },
    Match {
        scrutinee: Expr,
        arms: Vec<MatchArm>,
    },
    Return(Option<Expr>),
    Break,
    Continue,
    Panic(Expr),
    Expr(Expr),
    Raw(Vec<Stmt>),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ForMode {
    Read,
    BorrowMut,
    Move,
}

#[derive(Clone, Debug)]
pub struct MatchArm {
    pub span: Span,
    pub pat: Pat,
    pub guard: Option<Expr>,
    pub body: Vec<Stmt>,
}

pub type Block = Vec<Stmt>;
