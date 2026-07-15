use crate::mir::place::Place;
use crate::types::ty::TyId;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UnOp {
    Not,
    Neg,
}

#[derive(Clone, Debug)]
pub enum Const {
    Int(i64),
    U64(u64),
    Float(f64),
    Bool(bool),
    Str(String),
    Char(char),
    Unit,
    Addr(u64),
}

#[derive(Clone, Debug)]
pub enum Rvalue {
    Use(Place),
    Const(Const),
    BinOp {
        op: BinOp,
        lhs: Place,
        rhs: Place,
        ty: TyId,
    },
    UnOp {
        op: UnOp,
        operand: Place,
        ty: TyId,
    },
    Borrow {
        place: Place,
        is_mut: bool,
        ty: TyId,
    },
    Move {
        place: Place,
        ty: TyId,
    },
    MagnetAttach {
        place: Place,
        is_mut: bool,
        ty: TyId,
    },
    MagnetNone {
        ty: TyId,
    },
    MagnetFromAddr {
        addr: Place,
        ty: TyId,
    },
    MagnetAddress {
        magnet: Place,
        ty: TyId,
    },
    MagnetOffset {
        magnet: Place,
        ty: TyId,
    },
    MagnetAdd {
        magnet: Place,
        delta: Place,
        ty: TyId,
    },
    Cast {
        operand: Place,
        from: TyId,
        to: TyId,
    },
    ChoiceCtor {
        def: u32,
        variant: u32,
        args: Vec<Place>,
        ty: TyId,
    },
    ShapeCtor {
        def: u32,
        fields: Vec<Place>,
        ty: TyId,
    },
    ArrayCtor {
        elems: Vec<Place>,
        ty: TyId,
    },
    Call {
        callee: String,
        args: Vec<Place>,
        ret_ty: TyId,
    },
    Try {
        operand: Place,
        ok_ty: TyId,
        err_ty: TyId,
    },
    Load {
        addr: Place,
        offset: i64,
        ty: TyId,
    },
    Store {
        addr: Place,
        value: Place,
        offset: i64,
        ty: TyId,
    },
}
