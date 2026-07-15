use crate::symbol::LocalId;

#[derive(Clone, Debug)]
pub enum PlaceElem {
    Field(u32),
    Index(LocalId),
    Deref,
}

#[derive(Clone, Debug)]
pub struct Place {
    pub local: LocalId,
    pub elems: Vec<PlaceElem>,
}

impl Place {
    pub fn local(local: LocalId) -> Self {
        Self {
            local,
            elems: Vec::new(),
        }
    }
    pub fn field(local: LocalId, idx: u32) -> Self {
        Self {
            local,
            elems: vec![PlaceElem::Field(idx)],
        }
    }
    pub fn deref(local: LocalId) -> Self {
        Self {
            local,
            elems: vec![PlaceElem::Deref],
        }
    }
}
