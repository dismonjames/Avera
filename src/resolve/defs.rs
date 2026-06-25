use crate::symbol::FileId;
use crate::types::ty::{DefKind, VariantInfo};
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DefClass {
    Shape,
    Choice,
    Ability,
    Action,
    ForeignAction,
    Field,
}

#[derive(Clone, Debug)]
pub struct DefInfo {
    pub name: String,
    pub span: crate::diagnostics::span::Span,
    pub file: FileId,
    pub class: DefClass,
    pub kind: DefKind,
    pub is_contract: bool,
}

pub type DefId = u32;

#[derive(Default)]
pub struct Defs {
    pub items: Vec<DefInfo>,
    pub by_name: HashMap<String, DefId>,
}

impl Defs {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, info: DefInfo) -> DefId {
        let id = self.items.len() as DefId;
        self.by_name.insert(info.name.clone(), id);
        self.items.push(info);
        id
    }
    pub fn find(&self, name: &str) -> Option<DefId> {
        self.by_name.get(name).copied()
    }
    pub fn info(&self, id: DefId) -> &DefInfo {
        &self.items[id as usize]
    }
    pub fn info_mut(&mut self, id: DefId) -> &mut DefInfo {
        &mut self.items[id as usize]
    }
    pub fn class(&self, id: DefId) -> DefClass {
        self.items[id as usize].class
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

pub fn choice_variants(info: &DefInfo) -> &[VariantInfo] {
    match &info.kind {
        DefKind::Choice { variants, .. } => variants,
        _ => &[],
    }
}
