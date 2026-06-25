pub mod defs;
pub mod scope;

pub use defs::{DefClass, DefId, DefInfo, Defs};
pub use scope::ScopeTable;

use crate::ast::{DirectiveKind, Item, ItemKind, Module};
use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use crate::symbol::FileId;

pub struct ResolvedModule {
    pub file: FileId,
    pub def_ids: Vec<DefId>,
    pub imports: Vec<Import>,
    pub diags: DiagnosticList,
}

#[derive(Clone, Debug)]
pub struct Import {
    pub path: crate::ast::path::Path,
    pub alias: Option<String>,
    pub span: crate::diagnostics::span::Span,
}

pub fn resolve_module(defs: &mut Defs, file: FileId, module: &Module) -> ResolvedModule {
    let mut diags = DiagnosticList::new();
    let mut imports = Vec::new();
    let mut def_ids = Vec::new();
    for d in &module.directives {
        if let DirectiveKind::Import { path, alias } = &d.kind {
            imports.push(Import {
                path: path.clone(),
                alias: alias.clone(),
                span: d.span,
            });
        }
    }
    for item in &module.items {
        if let Some(id) = resolve_item(defs, file, item, &mut diags) {
            def_ids.push(id);
        }
    }
    ResolvedModule {
        file,
        def_ids,
        imports,
        diags,
    }
}

fn resolve_item(
    defs: &mut Defs,
    file: FileId,
    item: &Item,
    diags: &mut DiagnosticList,
) -> Option<DefId> {
    let (name, span, class) = match &item.kind {
        ItemKind::Shape(s) => (s.name.clone(), s.span, DefClass::Shape),
        ItemKind::Choice(c) => (c.name.clone(), c.span, DefClass::Choice),
        ItemKind::Ability(a) => (a.name.clone(), a.span, DefClass::Ability),
        ItemKind::Action(a) => {
            // Register action. For methods, use "Type.method" as the key.
            let full_name = if let Some(recv) = &a.sig.receiver {
                format!("{}.{}", recv.as_str(), a.sig.name)
            } else {
                a.sig.name.clone()
            };
            let is_contract = a.body.is_none();
            if let Some(existing) = defs.find(&full_name) {
                // A `.mav` contract declaration (no body) and its `.av`
                // implementation (with body) name the same symbol: that is a
                // contract match, not a duplicate.
                let prev = defs.info(existing);
                let both_contract_match = prev.is_contract || is_contract; // one is contract, other is impl
                if both_contract_match && (prev.is_contract != is_contract) {
                    // Mark the merged def as an implementation (the contract
                    // is satisfied). Keep the existing registration.
                    defs.info_mut(existing).is_contract = false;
                    return Some(existing);
                }
                diags.push(
                    Diagnostic::error(
                        DiagnosticKind::EDuplicate,
                        a.span,
                        format!("`{}` is already defined", full_name),
                    )
                    .with_related(prev.span, "previous definition here"),
                );
                return Some(existing);
            }
            return Some(defs.push(DefInfo {
                name: full_name,
                span: a.span,
                file,
                class: DefClass::Action,
                kind: crate::types::ty::DefKind::Ability {
                    generics: Vec::new(),
                },
                is_contract,
            }));
        }
        _ => return None, // impls/foreign are resolved in a second pass.
    };
    if let Some(existing) = defs.find(&name) {
        diags.push(
            Diagnostic::error(
                DiagnosticKind::EDuplicate,
                span,
                format!("`{}` is already defined", name),
            )
            .with_related(defs.info(existing).span, "previous definition here"),
        );
        return Some(existing);
    }
    // Populate the DefKind with actual fields/variants.
    let kind = match &item.kind {
        ItemKind::Shape(s) => {
            let fields: Vec<crate::types::ty::FieldInfo> = s
                .fields
                .iter()
                .map(|f| crate::types::ty::FieldInfo {
                    name: f.name.clone(),
                    ty: crate::symbol::TypeId::placeholder(),
                    span: f.span,
                })
                .collect();
            crate::types::ty::DefKind::Shape {
                generics: s.generics.clone(),
                fields,
            }
        }
        ItemKind::Choice(c) => {
            let variants: Vec<crate::types::ty::VariantInfo> = c
                .variants
                .iter()
                .map(|v| crate::types::ty::VariantInfo {
                    name: v.name.clone(),
                    payload: None,
                    span: v.span,
                })
                .collect();
            crate::types::ty::DefKind::Choice {
                generics: c.generics.clone(),
                variants,
            }
        }
        _ => crate::types::ty::DefKind::Ability {
            generics: Vec::new(),
        },
    };
    Some(defs.push(DefInfo {
        name,
        span,
        file,
        class,
        kind,
        is_contract: false,
    }))
}
