use crate::mir::place::Place;
use crate::mir::rvalue::Rvalue;
use crate::mir::terminator::Terminator;
use crate::symbol::{BlockId, LocalId};
use crate::types::ty::TyId;

#[derive(Clone, Debug)]
pub struct Local {
    pub id: LocalId,
    pub name: String,
    pub ty: TyId,
    pub mutable: bool,
}

#[derive(Clone, Debug)]
pub struct BasicBlock {
    pub id: BlockId,
    pub stmts: Vec<Stmt>,
    pub term: Terminator,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Assign {
        place: Place,
        value: Rvalue,
    },
    StorageLive(LocalId),
    StorageDead(LocalId),
    Drop(Place),
    Call {
        dest: Place,
        callee: String,
        args: Vec<Place>,
        ret_ty: TyId,
    },
    Assert {
        cond: Place,
        msg: String,
    },
}

#[derive(Clone, Debug)]
pub struct Body {
    pub name: String,
    pub locals: Vec<Local>,
    pub blocks: Vec<BasicBlock>,
    pub entry: BlockId,
    pub ret_ty: TyId,
    pub params: Vec<LocalId>,
    pub address_taken: std::collections::HashSet<LocalId>,
    pub owns_heap: std::collections::HashSet<LocalId>,
}

impl Body {
    pub fn new(name: String, ret_ty: TyId) -> Self {
        Self {
            name,
            locals: Vec::new(),
            blocks: Vec::new(),
            entry: BlockId::new(0),
            ret_ty,
            params: Vec::new(),
            address_taken: std::collections::HashSet::new(),
            owns_heap: std::collections::HashSet::new(),
        }
    }
    pub fn add_local(&mut self, l: Local) -> LocalId {
        let id = l.id;
        self.locals.push(l);
        id
    }
    pub fn add_block(&mut self, b: BasicBlock) -> BlockId {
        let id = b.id;
        self.blocks.push(b);
        id
    }
    pub fn local(&self, id: LocalId) -> &Local {
        &self.locals[id.to_usize()]
    }
    pub fn block(&self, id: BlockId) -> &BasicBlock {
        &self.blocks[id.to_usize()]
    }
}
