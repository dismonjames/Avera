use crate::ast::path::Path;
use crate::ast::ty::TyKind;
use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use crate::diagnostics::span::Span;
use crate::types::intern::TyCtxt;
use crate::types::ty::{DefKind, TyData, TyId};

pub type DefTable = std::collections::HashMap<String, u32>;

pub struct TyLowerer<'a> {
    pub cx: &'a mut TyCtxt,
    pub defs: &'a DefTable,
    pub diags: &'a mut DiagnosticList,
    pub file: crate::symbol::FileId,
}

impl<'a> TyLowerer<'a> {
    pub fn lower(&mut self, ty: &crate::ast::ty::Ty) -> TyId {
        match &ty.kind {
            TyKind::Named { path, args } => self.lower_named(path, args, ty.span),
            TyKind::Borrow { is_mut, inner } => {
                let inner = self.lower(inner);
                self.cx.borrow(inner, *is_mut)
            }
            TyKind::Magnet { is_mut, inner } => {
                let inner = self.lower(inner);
                self.cx.magnet(inner, *is_mut)
            }
            TyKind::Address(inner) => {
                let inner = self.lower(inner);
                self.cx.address(inner)
            }
            TyKind::Array { elem, size } => {
                let elem = self.lower(elem);
                let size = eval_const_size(size);
                self.cx.array(elem, size)
            }
            TyKind::Span(inner) => {
                let inner = self.lower(inner);
                self.cx.span(inner)
            }
            TyKind::Maybe(inner) => {
                let inner = self.lower(inner);
                self.cx.maybe(inner)
            }
            TyKind::Outcome { ok, err } => {
                let ok = self.lower(ok);
                let err = self.lower(err);
                self.cx.outcome(ok, err)
            }
            TyKind::SelfTy => {
                self.diags.push(Diagnostic::error(
                    DiagnosticKind::Parse,
                    ty.span,
                    "`Self` is only valid inside an ability implementation",
                ));
                self.cx.unit
            }
            TyKind::Unit => self.cx.unit,
            TyKind::Infer => self.cx.intern(TyData::Infer),
            TyKind::Tuple(ts) => {
                if ts.is_empty() {
                    self.cx.unit
                } else {
                    // v0.1: tuples are reserved; lower to first element as
                    // a fallback so we still produce *a* type.
                    self.diags.push(Diagnostic::error(
                        DiagnosticKind::Parse,
                        ty.span,
                        "tuples are reserved in v0.1",
                    ));
                    self.lower(&ts[0])
                }
            }
        }
    }

    fn lower_named(&mut self, path: &Path, args: &[crate::ast::ty::Ty], span: Span) -> TyId {
        let name = path
            .segments
            .last()
            .map(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let arg_ids: Vec<TyId> = args.iter().map(|a| self.lower(a)).collect();
        // Primitives first.
        let prim = match name.as_str() {
            "Bool" => Some(self.cx.bool),
            "Byte" => Some(self.cx.byte),
            "Char" => Some(self.cx.char),
            "I8" => Some(self.cx.i8),
            "I16" => Some(self.cx.i16),
            "I32" => Some(self.cx.i32),
            "I64" => Some(self.cx.i64),
            "U8" => Some(self.cx.u8),
            "U16" => Some(self.cx.u16),
            "U32" => Some(self.cx.u32),
            "U64" => Some(self.cx.u64),
            "Size" => Some(self.cx.size),
            "Int" => Some(self.cx.int),
            "UInt" => Some(self.cx.uint),
            "F32" => Some(self.cx.f32),
            "F64" => Some(self.cx.f64),
            "Text" => Some(self.cx.text),
            "Bytes" => Some(self.cx.bytes),
            "Unit" => Some(self.cx.unit),
            _ => None,
        };
        if let Some(p) = prim {
            if !arg_ids.is_empty() {
                self.diags.push(Diagnostic::error(
                    DiagnosticKind::ETypeMismatch,
                    span,
                    format!("`{}` takes no type arguments", name),
                ));
            }
            return p;
        }
        // User-defined shape/choice.
        if let Some(&def) = self.defs.get(&name) {
            // For choices like Maybe<T> we still need the def + args.
            return self.cx.intern(match def_kind_class(def, self.defs) {
                "shape" => TyData::Shape { def, args: arg_ids },
                "choice" => TyData::Choice { def, args: arg_ids },
                "ability" => {
                    self.diags.push(Diagnostic::error(
                        DiagnosticKind::ETypeMismatch,
                        span,
                        format!("`{}` is an ability and cannot be used as a type", name),
                    ));
                    return self.cx.unit;
                }
                _ => TyData::Shape { def, args: arg_ids },
            });
        }
        self.diags.push(Diagnostic::error(
            DiagnosticKind::EUndefinedName,
            span,
            format!("unknown type `{}`", name),
        ));
        self.cx.unit
    }
}

fn eval_const_size(e: &crate::ast::expr::Expr) -> u64 {
    use crate::ast::expr::{ExprKind, Lit};
    match &e.kind {
        ExprKind::Literal(Lit::Int(s)) => s.parse().unwrap_or(0),
        ExprKind::Binary { op, lhs, rhs } => {
            let l = eval_const_size(lhs);
            let r = eval_const_size(rhs);
            use crate::ast::expr::BinOp;
            match op {
                BinOp::Add => l + r,
                BinOp::Sub => l.saturating_sub(r),
                BinOp::Mul => l * r,
                _ => 0,
            }
        }
        _ => 0,
    }
}

fn def_kind_class(def: u32, defs: &DefTable) -> &'static str {
    // We don't carry DefKind here; the resolver fills a parallel table.
    // For now we use a heuristic: if it's in the def table at all, treat it
    // as a "shape" unless the resolver registered it as a choice via a
    // naming convention. The real classification happens in `resolve`.
    let _ = def;
    let _ = defs;
    "shape"
}

pub fn record_def_kind(_def: u32, _kind: &DefKind, _defs: &DefTable) {
    // no-op placeholder; real classification lives in the resolver.
}
