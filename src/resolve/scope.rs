use crate::resolve::defs::DefId;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Binding {
    pub name: String,
    pub local: u32,
    pub ty: crate::types::ty::TyId,
    pub kind: BindingKind,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BindingKind {
    Owner,
    Const,
    Param,
    Magnet,
    MagnetMut,
}

#[derive(Default)]
pub struct ScopeTable {
    scopes: Vec<HashMap<String, Binding>>,
}

impl ScopeTable {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }
    pub fn enter(&mut self) {
        self.scopes.push(HashMap::new());
    }
    pub fn leave(&mut self) {
        self.scopes.pop();
    }
    pub fn define(&mut self, b: Binding) {
        let top = self.scopes.last_mut().unwrap();
        top.insert(b.name.clone(), b);
    }
    pub fn lookup(&self, name: &str) -> Option<&Binding> {
        for s in self.scopes.iter().rev() {
            if let Some(b) = s.get(name) {
                return Some(b);
            }
        }
        None
    }
    pub fn lookup_mut(&mut self, name: &str) -> Option<&mut Binding> {
        for s in self.scopes.iter_mut().rev() {
            if let Some(b) = s.get_mut(name) {
                return Some(b);
            }
        }
        None
    }
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct TypeRef {
    pub def: DefId,
}
