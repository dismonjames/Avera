use crate::symbol::{Arena, FieldId, TypeId, VariantId};

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
    // ---- primitives ----
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
    // ---- aggregates ----
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
    pub span: crate::diagnostics::span::Span,
}

#[derive(Clone, Debug)]
pub struct VariantInfo {
    pub name: String,
    pub payload: Option<Vec<(String, TyId)>>,
    pub span: crate::diagnostics::span::Span,
}

#[derive(Clone, Debug)]
pub enum DefKind {
    Shape {
        generics: Vec<String>,
        fields: Vec<FieldInfo>,
    },
    Choice {
        generics: Vec<String>,
        variants: Vec<VariantInfo>,
    },
    Ability {
        generics: Vec<String>,
    },
}

#[derive(Default)]
pub struct Defs {
    pub kinds: Vec<DefKind>,
    pub names: Vec<String>,
    pub abilities: Vec<Abilities>,
    pub spans: Vec<crate::diagnostics::span::Span>,
}

impl Defs {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(
        &mut self,
        name: String,
        kind: DefKind,
        abilities: Abilities,
        span: crate::diagnostics::span::Span,
    ) -> u32 {
        let id = self.kinds.len() as u32;
        self.kinds.push(kind);
        self.names.push(name);
        self.abilities.push(abilities);
        self.spans.push(span);
        id
    }
    pub fn kind(&self, def: u32) -> &DefKind {
        &self.kinds[def as usize]
    }
    pub fn name(&self, def: u32) -> &str {
        &self.names[def as usize]
    }
    pub fn abilities(&self, def: u32) -> &Abilities {
        &self.abilities[def as usize]
    }
    pub fn find(&self, name: &str) -> Option<u32> {
        self.names.iter().position(|n| n == name).map(|i| i as u32)
    }
}

impl std::ops::Index<FieldId> for Defs {
    type Output = FieldInfo;
    fn index(&self, _id: FieldId) -> &FieldInfo {
        panic!("FieldId indexing not supported on Defs; use the shape's fields vec")
    }
}
impl std::ops::Index<VariantId> for Defs {
    type Output = VariantInfo;
    fn index(&self, _id: VariantId) -> &VariantInfo {
        panic!("VariantId indexing not supported on Defs; use the choice's variants vec")
    }
}

// re-export Arena so other modules can build typed arenas of FieldInfo.
pub type FieldArena = Arena<FieldId, FieldInfo>;
pub type VariantArena = Arena<VariantId, VariantInfo>;
