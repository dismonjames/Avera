use crate::symbol::TypeId;
use crate::types::ty::{Abilities, CopyKind, TyData, TyId};
use std::collections::HashMap;

static NO_ABILITIES: Abilities = Abilities {
    copy: CopyKind::No,
    drop: false,
    display: false,
    equal: false,
    order: false,
};

#[derive(Clone)]
pub struct TyCtxt {
    types: Vec<TyData>,
    cache: HashMap<String, TyId>,
    abilities: Vec<Option<Abilities>>,
    pub bool: TyId,
    pub byte: TyId,
    pub char: TyId,
    pub i8: TyId,
    pub i16: TyId,
    pub i32: TyId,
    pub i64: TyId,
    pub u8: TyId,
    pub u16: TyId,
    pub u32: TyId,
    pub u64: TyId,
    pub size: TyId,
    pub int: TyId,
    pub uint: TyId,
    pub f32: TyId,
    pub f64: TyId,
    pub text: TyId,
    pub bytes: TyId,
    pub unit: TyId,
    pub never: TyId,
}

impl Default for TyCtxt {
    fn default() -> Self {
        Self::new()
    }
}

impl TyCtxt {
    pub fn new() -> Self {
        Self::fresh()
    }

    pub fn new_from(other: &TyCtxt) -> Self {
        other.clone()
    }

    fn fresh() -> Self {
        let mut s = Self {
            types: Vec::new(),
            cache: HashMap::new(),
            abilities: Vec::new(),
            bool: TyId::placeholder(),
            byte: TyId::placeholder(),
            char: TyId::placeholder(),
            i8: TyId::placeholder(),
            i16: TyId::placeholder(),
            i32: TyId::placeholder(),
            i64: TyId::placeholder(),
            u8: TyId::placeholder(),
            u16: TyId::placeholder(),
            u32: TyId::placeholder(),
            u64: TyId::placeholder(),
            size: TyId::placeholder(),
            int: TyId::placeholder(),
            uint: TyId::placeholder(),
            f32: TyId::placeholder(),
            f64: TyId::placeholder(),
            text: TyId::placeholder(),
            bytes: TyId::placeholder(),
            unit: TyId::placeholder(),
            never: TyId::placeholder(),
        };
        s.bool = s.intern_prim(TyData::Bool);
        s.byte = s.intern_prim(TyData::Byte);
        s.char = s.intern_prim(TyData::Char);
        s.i8 = s.intern_prim(TyData::I8);
        s.i16 = s.intern_prim(TyData::I16);
        s.i32 = s.intern_prim(TyData::I32);
        s.i64 = s.intern_prim(TyData::I64);
        s.u8 = s.intern_prim(TyData::U8);
        s.u16 = s.intern_prim(TyData::U16);
        s.u32 = s.intern_prim(TyData::U32);
        s.u64 = s.intern_prim(TyData::U64);
        s.size = s.intern_prim(TyData::Size);
        s.int = s.intern_prim(TyData::Int);
        s.uint = s.intern_prim(TyData::UInt);
        s.f32 = s.intern_prim(TyData::F32);
        s.f64 = s.intern_prim(TyData::F64);
        s.text = s.intern_prim(TyData::Text);
        s.bytes = s.intern_prim(TyData::Bytes);
        s.unit = s.intern_prim(TyData::Unit);
        s.never = s.intern_prim(TyData::Never);
        s
    }

    fn intern_prim(&mut self, data: TyData) -> TyId {
        let id = self.intern(data.clone());
        self.ensure_abilities(id, Abilities::primitives());
        id
    }

    pub fn intern(&mut self, data: TyData) -> TyId {
        let key = type_key(&data, self);
        if let Some(&id) = self.cache.get(&key) {
            return id;
        }
        let id = TypeId::from_usize(self.types.len());
        self.types.push(data);
        self.abilities.push(None);
        self.cache.insert(key, id);
        id
    }

    pub fn data(&self, id: TyId) -> &TyData {
        &self.types[id.to_usize()]
    }

    pub fn abilities(&self, id: TyId) -> &Abilities {
        self.abilities
            .get(id.to_usize())
            .and_then(Option::as_ref)
            .unwrap_or(&NO_ABILITIES)
    }

    pub fn set_abilities(&mut self, id: TyId, ab: Abilities) {
        let slot = &mut self.abilities[id.to_usize()];
        if slot.is_none() {
            *slot = Some(ab);
        } else {
            let cur = slot.as_mut().unwrap();
            cur.copy = ab.copy;
            if ab.drop {
                cur.drop = true;
            }
            if ab.display {
                cur.display = true;
            }
            if ab.equal {
                cur.equal = true;
            }
            if ab.order {
                cur.order = true;
            }
        }
    }

    fn ensure_abilities(&mut self, id: TyId, ab: Abilities) {
        let slot = &mut self.abilities[id.to_usize()];
        if slot.is_none() {
            *slot = Some(ab);
        }
    }

    pub fn borrow(&mut self, inner: TyId, is_mut: bool) -> TyId {
        self.intern(TyData::Borrow { inner, is_mut })
    }

    pub fn magnet(&mut self, inner: TyId, is_mut: bool) -> TyId {
        self.intern(TyData::Magnet { inner, is_mut })
    }

    pub fn maybe(&mut self, inner: TyId) -> TyId {
        self.intern(TyData::Maybe(inner))
    }

    pub fn outcome(&mut self, ok: TyId, err: TyId) -> TyId {
        self.intern(TyData::Outcome { ok, err })
    }

    pub fn array(&mut self, elem: TyId, size: u64) -> TyId {
        self.intern(TyData::Array { elem, size })
    }

    pub fn address(&mut self, inner: TyId) -> TyId {
        self.intern(TyData::Address(inner))
    }

    pub fn span(&mut self, inner: TyId) -> TyId {
        self.intern(TyData::Span(inner))
    }

    pub fn fn_type(&mut self, params: Vec<TyId>, ret: TyId) -> TyId {
        self.intern(TyData::Fn { params, ret })
    }

    pub fn show(&self, id: TyId) -> String {
        match self.data(id) {
            TyData::Bool => "Bool".into(),
            TyData::Byte => "Byte".into(),
            TyData::Char => "Char".into(),
            TyData::I8 => "I8".into(),
            TyData::I16 => "I16".into(),
            TyData::I32 => "I32".into(),
            TyData::I64 => "I64".into(),
            TyData::U8 => "U8".into(),
            TyData::U16 => "U16".into(),
            TyData::U32 => "U32".into(),
            TyData::U64 => "U64".into(),
            TyData::F32 => "F32".into(),
            TyData::F64 => "F64".into(),
            TyData::Size => "Size".into(),
            TyData::Int => "Int".into(),
            TyData::UInt => "UInt".into(),
            TyData::Text => "Text".into(),
            TyData::Bytes => "Bytes".into(),
            TyData::Unit => "Unit".into(),
            TyData::Never => "Never".into(),
            TyData::Shape { def, args } => {
                if args.is_empty() {
                    format!("<shape#{}>", def)
                } else {
                    let a: Vec<String> = args.iter().map(|&a| self.show(a)).collect();
                    format!("<shape#{}><{}>", def, a.join(", "))
                }
            }
            TyData::Choice { def, args } => {
                if args.is_empty() {
                    format!("<choice#{}>", def)
                } else {
                    let a: Vec<String> = args.iter().map(|&a| self.show(a)).collect();
                    format!("<choice#{}><{}>", def, a.join(", "))
                }
            }
            TyData::Array { elem, size } => format!("[{}; {}]", self.show(*elem), size),
            TyData::Borrow { inner, is_mut } => {
                if *is_mut {
                    format!("&!{}", self.show(*inner))
                } else {
                    format!("&{}", self.show(*inner))
                }
            }
            TyData::Magnet { inner, is_mut } => {
                if *is_mut {
                    format!("MagnetMut<{}>", self.show(*inner))
                } else {
                    format!("Magnet<{}>", self.show(*inner))
                }
            }
            TyData::Address(inner) => format!("Address<{}>", self.show(*inner)),
            TyData::Span(inner) => format!("Span<{}>", self.show(*inner)),
            TyData::Maybe(inner) => format!("Maybe<{}>", self.show(*inner)),
            TyData::Outcome { ok, err } => {
                format!("Outcome<{}, {}>", self.show(*ok), self.show(*err))
            }
            TyData::Fn { params, ret } => {
                let p: Vec<String> = params.iter().map(|&p| self.show(p)).collect();
                format!("action({}) : {}", p.join(", "), self.show(*ret))
            }
            TyData::Var { name, .. } => name.clone(),
            TyData::Infer => "_".into(),
        }
    }
}

fn type_key(data: &TyData, _cx: &TyCtxt) -> String {
    match data {
        TyData::Bool => "Bool".into(),
        TyData::Byte => "Byte".into(),
        TyData::Char => "Char".into(),
        TyData::I8 => "I8".into(),
        TyData::I16 => "I16".into(),
        TyData::I32 => "I32".into(),
        TyData::I64 => "I64".into(),
        TyData::U8 => "U8".into(),
        TyData::U16 => "U16".into(),
        TyData::U32 => "U32".into(),
        TyData::U64 => "U64".into(),
        TyData::F32 => "F32".into(),
        TyData::F64 => "F64".into(),
        TyData::Size => "Size".into(),
        TyData::Int => "Int".into(),
        TyData::UInt => "UInt".into(),
        TyData::Text => "Text".into(),
        TyData::Bytes => "Bytes".into(),
        TyData::Unit => "Unit".into(),
        TyData::Never => "Never".into(),
        TyData::Shape { def, args } => {
            let a: Vec<String> = args.iter().map(|id| format!("t{}", id.get())).collect();
            format!("S{}<{}>", def, a.join(","))
        }
        TyData::Choice { def, args } => {
            let a: Vec<String> = args.iter().map(|id| format!("t{}", id.get())).collect();
            format!("C{}<{}>", def, a.join(","))
        }
        TyData::Array { elem, size } => format!("A{}[{}]", elem.get(), size),
        TyData::Borrow { inner, is_mut } => {
            format!("B{}{}", if *is_mut { "m" } else { "" }, inner.get())
        }
        TyData::Magnet { inner, is_mut } => {
            format!("M{}{}", if *is_mut { "m" } else { "" }, inner.get())
        }
        TyData::Address(inner) => format!("Ad{}", inner.get()),
        TyData::Span(inner) => format!("Sp{}", inner.get()),
        TyData::Maybe(inner) => format!("Mb{}", inner.get()),
        TyData::Outcome { ok, err } => format!("O{}{}", ok.get(), err.get()),
        TyData::Fn { params, ret } => {
            let p: Vec<String> = params.iter().map(|id| format!("t{}", id.get())).collect();
            format!("F({}){}", p.join(","), ret.get())
        }
        TyData::Var { idx, name } => format!("V{}{}", idx, name),
        TyData::Infer => "_".into(),
    }
}
