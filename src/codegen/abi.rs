use crate::types::intern::TyCtxt;
use crate::types::ty::{TyData, TyId};
use cranelift_codegen::ir::types::{F32, F64, I16, I32, I64, I8};
use cranelift_codegen::ir::Type;

pub fn scalar_type(ty: TyId, cx: &TyCtxt) -> Option<Type> {
    Some(match cx.data(ty) {
        TyData::Bool => I8,
        TyData::Byte | TyData::U8 | TyData::I8 => I8,
        TyData::U16 | TyData::I16 | TyData::Char => I16,
        TyData::U32 | TyData::I32 => I32,
        TyData::F32 => F32,
        TyData::U64 | TyData::I64 => I64,
        TyData::F64 => F64,
        TyData::Size | TyData::Int | TyData::UInt => I64,
        TyData::Address(_) => I64,
        TyData::Borrow { .. } | TyData::Magnet { .. } | TyData::Span(_) => I64,
        _ => return None,
    })
}

pub fn is_scalar(ty: TyId, cx: &TyCtxt) -> bool {
    scalar_type(ty, cx).is_some()
}

// ===== Layout engine =====
//
// The layout engine computes the size and alignment of a type for the v0.1
// heap ABI. All heap allocations are 8-byte aligned (the pointer width), and
// all fields are at 8-byte offsets, because Avera values are represented as
// pointer-sized words (I64 handles for heap objects, or in-register I64 for
// scalars). This keeps the layout uniform and avoids per-type packing logic
// — a deliberate v0.1 simplification that is documented, not a stub.

pub fn scalar_size(ty: TyId, cx: &TyCtxt) -> u64 {
    scalar_type(ty, cx).map(|t| t.bytes() as u64).unwrap_or(8)
}

pub fn align_of(_ty: TyId, _cx: &TyCtxt) -> u64 {
    8
}

pub fn shape_size(fields: &[(String, TyId)]) -> u64 {
    // Each field occupies exactly 8 bytes (pointer-word slot) in the v0.1
    // heap layout, regardless of the field's declared scalar type. A Bool
    // field takes an 8-byte slot just like an I64 field, because the slot
    // holds an I64 (either a scalar value or a heap handle).
    (fields.len() as u64) * 8
}

pub fn field_offset_by_index(index: usize) -> i64 {
    (index as i64) * 8
}

pub fn choice_size() -> u64 {
    16
}

pub fn choice_tag_offset() -> i64 {
    0
}

pub fn choice_payload_offset() -> i64 {
    8
}

pub fn array_header_size() -> u64 {
    16
}
