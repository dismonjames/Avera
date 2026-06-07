use crate::ast::path::Path;
use crate::ast::ty::Ty;
use crate::diagnostics::span::Span;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BinOp {
    // arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // logical
    And,
    Or,
    // bitwise
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl BinOp {
    pub fn precedence(self) -> u8 {
        match self {
            BinOp::Or => 1,
            BinOp::And => 2,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 3,
            BinOp::BitOr => 4,
            BinOp::BitXor => 5,
            BinOp::BitAnd => 6,
            BinOp::Shl | BinOp::Shr => 7,
            BinOp::Add | BinOp::Sub => 8,
            BinOp::Mul | BinOp::Div | BinOp::Mod => 9,
        }
    }
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
        )
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UnOp {
    Not,
    Neg,
    Borrow,
    BorrowMut,
    Move,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

impl Expr {
    pub fn new(span: Span, kind: ExprKind) -> Self {
        Self { span, kind }
    }
    pub fn unit(span: Span) -> Self {
        Self {
            span,
            kind: ExprKind::Literal(Lit::Unit),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Literal(Lit),
    Path(Path),
    SelfRef,
    ChoiceCtor {
        variant: String,
        args: Vec<Expr>,
    },
    Unary { op: UnOp, operand: Box<Expr> },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Call { callee: Box<Expr>, args: Vec<Expr> },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },
    Field { receiver: Box<Expr>, name: String },
    Index { base: Box<Expr>, index: Box<Expr> },
    Slice {
        base: Box<Expr>,
        lo: Option<Box<Expr>>,
        hi: Option<Box<Expr>>,
    },
    Cast { operand: Box<Expr>, target: Ty },
    Try(Box<Expr>),
    MagnetValid(Box<Expr>),
    MagnetMeta { target: Box<Expr>, property: String },
    None,
    Range { lo: Box<Expr>, hi: Box<Expr> },
    Paren(Box<Expr>),
}

#[derive(Clone, Debug)]
pub enum Lit {
    Int(String),
    Float(String),
    Str(String),
    Char(char),
    Bool(bool),
    Unit,
}
