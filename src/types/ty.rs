use crate::symbol::{FieldId, TypeId};

pub type TyId = TypeId;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum CopyKind {
    Primitive,
    User,
    #[default]
    No,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Abilities {
    pub copy: CopyKind,
    pub drop: bool,
    pub display: bool,
    pub equal: bool,
    pub order: bool,
}

impl Abilities {
    pub fn none() -> Self {
        Self::default()
    }
    pub fn primitives() -> Self {
        Self {
            copy: CopyKind::Primitive,
            drop: false,
            display: true,
            equal: true,
            order: true,
        }
    }
}

#[derive(Clone, Debug)]
pub enum TyData {
    Bool,
    Byte,
    Char,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Size,
    Int,
    UInt,
    Text,
    Bytes,
    Unit,
    Never,
    Shape {
        def: u32,
        args: Vec<TyId>,
    },
    Choice {
        def: u32,
        args: Vec<TyId>,
    },
    Array {
        elem: TyId,
        size: u64,
    },
    Borrow {
        inner: TyId,
        is_mut: bool,
    },
    Magnet {
        inner: TyId,
        is_mut: bool,
    },
    Address(TyId),
    Span(TyId),
    Maybe(TyId),
    Outcome {
        ok: TyId,
        err: TyId,
    },
    Fn {
        params: Vec<TyId>,
        ret: TyId,
    },
    Var {
        idx: u32,
        name: String,
    },
    Infer,
}

impl TyData {
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            TyData::Bool
                | TyData::Byte
                | TyData::Char
                | TyData::I8
                | TyData::I16
                | TyData::I32
                | TyData::I64
                | TyData::U8
                | TyData::U16
                | TyData::U32
                | TyData::U64
                | TyData::F32
                | TyData::F64
                | TyData::Size
                | TyData::Int
                | TyData::UInt
        )
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            TyData::I8
                | TyData::I16
                | TyData::I32
                | TyData::I64
                | TyData::U8
                | TyData::U16
                | TyData::U32
                | TyData::U64
                | TyData::Size
                | TyData::Int
                | TyData::UInt
        )
    }

    pub fn is_float(&self) -> bool {
        matches!(self, TyData::F32 | TyData::F64)
    }

    pub fn is_signed(&self) -> bool {
        matches!(
            self,
            TyData::I8 | TyData::I16 | TyData::I32 | TyData::I64 | TyData::Int
        )
    }

    pub fn naive_size(&self) -> u64 {
        match self {
            TyData::Bool | TyData::Byte | TyData::U8 | TyData::I8 => 1,
            TyData::Char | TyData::U16 | TyData::I16 => 2,
            TyData::U32 | TyData::I32 | TyData::F32 => 4,
            TyData::U64 | TyData::I64 | TyData::F64 | TyData::Size | TyData::Int | TyData::UInt => {
                8
            }
            _ => 0,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            TyData::Bool => "Bool",
            TyData::Byte => "Byte",
            TyData::Char => "Char",
            TyData::I8 => "I8",
            TyData::I16 => "I16",
            TyData::I32 => "I32",
            TyData::I64 => "I64",
            TyData::U8 => "U8",
            TyData::U16 => "U16",
            TyData::U32 => "U32",
            TyData::U64 => "U64",
            TyData::F32 => "F32",
            TyData::F64 => "F64",
            TyData::Size => "Size",
            TyData::Int => "Int",
            TyData::UInt => "UInt",
            TyData::Text => "Text",
            TyData::Bytes => "Bytes",
            TyData::Unit => "Unit",
            TyData::Never => "Never",
            TyData::Shape { .. } => "shape",
            TyData::Choice { .. } => "choice",
            TyData::Array { .. } => "array",
            TyData::Borrow { .. } => "borrow",
            TyData::Magnet { .. } => "Magnet",
            TyData::Address(_) => "Address",
            TyData::Span(_) => "Span",
            TyData::Maybe(_) => "Maybe",
            TyData::Outcome { .. } => "Outcome",
            TyData::Fn { .. } => "action",
            TyData::Var { .. } => "type-var",
            TyData::Infer => "_",
        }
    }
}

#[derive(Clone, Debug)]
pub struct FieldInfo {
    pub name: String,
    pub ty: TyId,
    pub offset: u64,
}

#[derive(Clone, Debug)]
pub struct VariantInfo {
    pub name: String,
    pub fields: Vec<FieldId>,
    pub discr: u64,
}

#[derive(Clone, Debug)]
pub enum DefKind {
    Shape {
        fields: Vec<FieldId>,
        abilities: Abilities,
    },
    Choice {
        variants: Vec<VariantInfo>,
        abilities: Abilities,
    },
    Ability,
}
