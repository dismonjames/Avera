use crate::types::intern::TyCtxt;
use crate::types::ty::{DefKind, TyData, TyId};

#[derive(Copy, Clone, Debug)]
pub struct Layout {
    pub size: u64,
    pub align: u64,
    pub scalar: bool,
}

impl Layout {
    pub const fn scalar(size: u64) -> Self {
        Self {
            size,
            align: size,
            scalar: true,
        }
    }
    pub const fn aggregate(size: u64, align: u64) -> Self {
        Self {
            size,
            align,
            scalar: false,
        }
    }
}

pub fn layout_of(cx: &TyCtxt, ty: TyId, defs: &crate::resolve::defs::Defs) -> Layout {
    match cx.data(ty) {
        TyData::Bool | TyData::Byte | TyData::U8 | TyData::I8 => Layout::scalar(1),
        TyData::Char | TyData::U16 | TyData::I16 => Layout::scalar(2),
        TyData::U32 | TyData::I32 | TyData::F32 => Layout::scalar(4),
        TyData::U64 | TyData::I64 | TyData::F64 => Layout::scalar(8),
        TyData::Size | TyData::Int | TyData::UInt => Layout::scalar(8),
        TyData::Address(_) => Layout::scalar(8),
        TyData::Borrow { .. } | TyData::Magnet { .. } | TyData::Span(_) => Layout::scalar(16),
        TyData::Text | TyData::Bytes => Layout::aggregate(24, 8),
        TyData::Unit => Layout::scalar(0),
        TyData::Never => Layout::scalar(0),
        TyData::Array { elem, size } => {
            let el = layout_of(cx, *elem, defs);
            Layout::aggregate(el.size * size, el.align)
        }
        TyData::Shape { def, .. } => {
            let info = defs.info(*def);
            let mut offset = 0u64;
            let mut max_align = 1u64;
            if let DefKind::Shape { fields, .. } = &info.kind {
                for f in fields {
                    let fl = layout_of(cx, f.ty, defs);
                    offset = align_up(offset, fl.align);
                    offset += fl.size;
                    max_align = max_align.max(fl.align);
                }
            }
            Layout::aggregate(offset, max_align)
        }
        TyData::Choice { def, .. } => {
            let info = defs.info(*def);
            let mut max_payload = 0u64;
            let mut max_align = 1u64;
            if let DefKind::Choice { variants, .. } = &info.kind {
                for v in variants {
                    if let Some(payload) = &v.payload {
                        let mut sz = 0u64;
                        let mut al = 1u64;
                        for (_, t) in payload {
                            let l = layout_of(cx, *t, defs);
                            sz = align_up(sz, l.align) + l.size;
                            al = al.max(l.align);
                        }
                        max_payload = max_payload.max(sz);
                        max_align = max_align.max(al);
                    }
                }
            }
            let tag_size = if max_payload == 0 && variant_count(cx, *def, defs) <= 1 {
                0
            } else {
                1
            };
            Layout::aggregate(max_payload + tag_size, max_align.max(1))
        }
        _ => Layout::aggregate(8, 8),
    }
}

fn variant_count(_cx: &TyCtxt, def: u32, defs: &crate::resolve::defs::Defs) -> u64 {
    let info = defs.info(def);
    if let DefKind::Choice { variants, .. } = &info.kind {
        variants.len() as u64
    } else {
        0
    }
}

fn align_up(off: u64, align: u64) -> u64 {
    if align == 0 {
        return off;
    }
    let mask = align - 1;
    (off + mask) & !mask
}
