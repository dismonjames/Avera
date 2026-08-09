use crate::ast::expr::{BinOp as ABin, Expr, ExprKind, Lit, UnOp as AUn};
use crate::ast::item::ActionDecl;
use crate::ast::stmt::{Stmt, StmtKind};
use crate::ast::ItemKind;
use crate::ast::Module;
use crate::codegen::abi;
use crate::mir::body::{BasicBlock, Body, Local, Stmt as MirStmt};
use crate::mir::place::Place;
use crate::mir::rvalue::{BinOp, Const, Rvalue, UnOp};
use crate::mir::terminator::Terminator;
use crate::resolve::Defs;
use crate::symbol::{BlockId, LocalId};
use crate::types::intern::TyCtxt;
use crate::types::ty::TyId;
use std::collections::HashMap;

pub fn lower_module(cx: &mut TyCtxt, defs: &Defs, module: &Module) -> Result<Vec<Body>, String> {
    let mut bodies = Vec::new();
    for item in &module.items {
        if let ItemKind::Action(a) = &item.kind {
            if a.body.is_some() {
                bodies.push(lower_action(cx, defs, a, module)?);
            }
        }
    }
    Ok(bodies)
}

fn lower_action(
    cx: &mut TyCtxt,
    defs: &Defs,
    a: &ActionDecl,
    module: &Module,
) -> Result<Body, String> {
    let ret_ty = a
        .sig
        .ret
        .as_ref()
        .map(|t| infer_ty(cx, t))
        .unwrap_or(cx.unit);
    let action_name = if let Some(recv) = &a.sig.receiver {
        format!("{}.{}", recv.as_str(), a.sig.name)
    } else {
        a.sig.name.clone()
    };
    let mut lower = Lowerer::new(cx, action_name, ret_ty);
    // Populate shape field info from the module AST, using the Lowerer's own
    // TyCtxt so the TypeIds match what guess_ty compares against.
    for item in &module.items {
        if let ItemKind::Shape(s) = &item.kind {
            let fields: Vec<(String, TyId)> = s
                .fields
                .iter()
                .map(|f| (f.name.clone(), infer_ty(&lower.cx, &f.ty)))
                .collect();
            lower.shapes.insert(s.name.clone(), fields);
        }
        if let ItemKind::Choice(c) = &item.kind {
            // Record variant names in declaration order so discriminants are
            // deterministic (tag = index in this list), not hash-based.
            let variants: Vec<String> = c.variants.iter().map(|v| v.name.clone()).collect();
            lower.choices.insert(c.name.clone(), variants);
        }
    }
    // Populate action names from Defs.
    for info in &defs.items {
        if info.class == crate::resolve::DefClass::Action {
            lower.action_names.push(info.name.clone());
        }
    }
    // allocate params as locals
    for p in &a.sig.params {
        let ty = infer_ty(cx, &p.ty);
        lower.alloc_param(p.name.clone(), ty);
    }
    lower.lower_block(a.body.as_ref().unwrap())?;
    lower.finish();
    Ok(lower.into_body())
}

fn infer_ty(cx: &TyCtxt, t: &crate::ast::ty::Ty) -> TyId {
    use crate::ast::ty::TyKind;
    match &t.kind {
        TyKind::Named { path, .. } => {
            let name = path.segments.last().map(|s| s.as_str()).unwrap_or("");
            match name {
                "Bool" => cx.bool,
                "Byte" => cx.byte,
                "Char" => cx.char,
                "I8" => cx.i8,
                "I16" => cx.i16,
                "I32" => cx.i32,
                "I64" => cx.i64,
                "U8" => cx.u8,
                "U16" => cx.u16,
                "U32" => cx.u32,
                "U64" => cx.u64,
                "Size" => cx.size,
                "Int" => cx.int,
                "UInt" => cx.uint,
                "F32" => cx.f32,
                "F64" => cx.f64,
                "Text" => cx.text,
                "Bytes" => cx.bytes,
                "Unit" => cx.unit,
                _ => cx.i64, // unknown -> pointer-sized for codegen
            }
        }
        _ => cx.i64,
    }
}

struct Lowerer {
    cx: TyCtxt,
    name: String,
    ret_ty: TyId,
    locals: Vec<Local>,
    scopes: Vec<HashMap<String, LocalId>>,
    next_local: u32,
    pub address_taken: std::collections::HashSet<LocalId>,
    owns_heap: std::collections::HashSet<LocalId>,
    magnets: HashMap<LocalId, bool>,
    params: Vec<LocalId>,
    cur_stmts: Vec<MirStmt>,
    blocks: Vec<BasicBlock>,
    cur_block: usize,
    loop_ctx: Vec<(BlockId, BlockId)>,
    shapes: HashMap<String, Vec<(String, TyId)>>,
    choices: HashMap<String, Vec<String>>,
    action_names: Vec<String>,
}

impl Lowerer {
    fn new(cx: &TyCtxt, name: String, ret_ty: TyId) -> Self {
        let entry = BlockId::new(0);
        let entry_block = BasicBlock {
            id: entry,
            stmts: Vec::new(),
            term: Terminator::Unreachable,
        };
        Self {
            cx: TyCtxt::new_from(cx),
            name,
            ret_ty,
            locals: Vec::new(),
            scopes: vec![HashMap::new()],
            next_local: 0,
            address_taken: std::collections::HashSet::new(),
            owns_heap: std::collections::HashSet::new(),
            magnets: HashMap::new(),
            params: Vec::new(),
            cur_stmts: Vec::new(),
            blocks: vec![entry_block],
            cur_block: 0,
            loop_ctx: Vec::new(),
            shapes: HashMap::new(),
            choices: HashMap::new(),
            action_names: Vec::new(),
        }
    }

    fn alloc_local(&mut self, name: String, ty: TyId, mutable: bool) -> LocalId {
        let id = LocalId::new(self.next_local);
        self.next_local += 1;
        self.locals.push(Local {
            id,
            name: name.clone(),
            ty,
            mutable,
        });
        if !name.is_empty() {
            if let Some(scope) = self.scopes.last_mut() {
                scope.insert(name, id);
            }
        }
        id
    }

    fn alloc_param(&mut self, name: String, ty: TyId) -> LocalId {
        let id = self.alloc_local(name, ty, true);
        self.params.push(id);
        id
    }

    fn anon(&mut self, ty: TyId) -> LocalId {
        self.alloc_local(String::new(), ty, false)
    }

    fn lookup(&self, name: &str) -> Option<LocalId> {
        // Walk the scope stack innermost-first.
        for scope in self.scopes.iter().rev() {
            if let Some(&id) = scope.get(name) {
                return Some(id);
            }
        }
        None
    }

    fn field_offset(&self, name: &str) -> Option<i64> {
        for fields in self.shapes.values() {
            if let Some(idx) = fields.iter().position(|(f, _)| f == name) {
                return Some(abi::field_offset_by_index(idx));
            }
        }
        None
    }

    fn variant_tag(&self, variant: &str) -> i64 {
        // (1) Canonical std variants.
        match variant {
            "none" => return 0,
            "some" => return 1,
            "ok" => return 1,
            "err" => return 2,
            _ => {}
        }
        // (2) Index in a known choice's variant list.
        for variants in self.choices.values() {
            if let Some(idx) = variants.iter().position(|v| v == variant) {
                return idx as i64;
            }
        }
        // (3) Deterministic fallback (FNV-1a), same at construct & match time.
        name_hash(variant) as i64
    }

    fn declared_in_current_scope(&self, name: &str) -> bool {
        self.scopes
            .last()
            .map(|s| s.contains_key(name))
            .unwrap_or(false)
    }

    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn new_block_id(&mut self) -> BlockId {
        let id = BlockId::new(self.blocks.len() as u32);
        self.blocks.push(BasicBlock {
            id,
            stmts: Vec::new(),
            term: Terminator::Unreachable,
        });
        id
    }

    fn emit(&mut self, stmt: MirStmt) {
        self.cur_stmts.push(stmt);
    }

    fn assign(&mut self, dest: LocalId, value: Rvalue) {
        self.emit(MirStmt::Assign {
            place: Place::local(dest),
            value,
        });
    }

    fn emit_heap_call(&mut self, dest: LocalId, callee: &str, args: Vec<Place>, ret_ty: TyId) {
        if self.owns_heap.contains(&dest) {
            let void_dest = self.anon(self.cx.unit);
            self.emit(MirStmt::Call {
                dest: Place::local(void_dest),
                callee: "avera_free".to_string(),
                args: vec![Place::local(dest)],
                ret_ty: self.cx.unit,
            });
        }
        self.owns_heap.insert(dest);
        self.emit(MirStmt::Call {
            dest: Place::local(dest),
            callee: callee.to_string(),
            args,
            ret_ty,
        });
    }

    fn flush(&mut self, term: Terminator) {
        let stmts = std::mem::take(&mut self.cur_stmts);
        self.blocks[self.cur_block].stmts = stmts;
        self.blocks[self.cur_block].term = term;
    }

    fn start_block(&mut self, id: BlockId) {
        self.cur_block = id.to_usize();
    }

    fn lower_block(&mut self, stmts: &[Stmt]) -> Result<(), String> {
        // A block introduces a new lexical scope: `:x` inside an if/for/while
        // body may shadow an outer `x`, but `:x` twice in the same block errors.
        self.enter_scope();
        let result = (|| {
            for s in stmts {
                self.lower_stmt(s)?;
            }
            Ok(())
        })();
        self.exit_scope();
        result
    }

    fn lower_match(
        &mut self,
        scrutinee: &Expr,
        arms: &[crate::ast::stmt::MatchArm],
    ) -> Result<(), String> {
        use crate::ast::pat::PatKind;
        // Evaluate the scrutinee into a temp.
        let scr = self.anon(self.cx.i64);
        self.lower_expr_into(scrutinee, scr)?;
        let merge_blk = self.new_block_id();
        // Check if any arm is a catch-all (wildcard or bind).
        let has_catchall = arms
            .iter()
            .any(|a| matches!(a.pat.kind, PatKind::Wild | PatKind::Bind(_)));
        let otherwise_blk = if has_catchall {
            None
        } else {
            let blk = self.new_block_id();
            Some(blk)
        };
        // Check if we're matching on a choice (variant patterns).
        let is_choice_match = arms.iter().any(|a| {
            matches!(
                a.pat.kind,
                PatKind::Variant { .. } | PatKind::VariantNoPayload(_)
            )
        });
        // For choice matching, load the tag from the scrutinee (offset 0).
        let discr_local = if is_choice_match {
            let tag = self.anon(self.cx.i64);
            self.assign(
                tag,
                Rvalue::Load {
                    addr: Place::local(scr),
                    offset: 0,
                    ty: self.cx.i64,
                },
            );
            tag
        } else {
            scr
        };
        // Collect (value, target) pairs for SwitchInt.
        let mut targets: Vec<(i128, BlockId)> = Vec::new();
        let mut arm_blocks: Vec<BlockId> = Vec::new();
        for arm in arms {
            let blk = self.new_block_id();
            arm_blocks.push(blk);
            match &arm.pat.kind {
                PatKind::Wild | PatKind::Bind(_) => {
                    // catch-all: handled by otherwise
                }
                PatKind::IntLit(v) => {
                    targets.push((*v as i128, blk));
                }
                PatKind::VariantNoPayload(name) => {
                    let tag = self.variant_tag(name);
                    targets.push((tag as i128, blk));
                }
                PatKind::Variant { name, .. } => {
                    let tag = self.variant_tag(name);
                    targets.push((tag as i128, blk));
                }
            }
        }
        // Determine the otherwise block (catch-all arm or the synthetic one).
        let otherwise = if let Some(ob) = otherwise_blk {
            ob
        } else {
            let mut idx = 0;
            for (i, arm) in arms.iter().enumerate() {
                if matches!(arm.pat.kind, PatKind::Wild | PatKind::Bind(_)) {
                    idx = i;
                    break;
                }
            }
            arm_blocks[idx]
        };
        self.flush(Terminator::SwitchInt {
            discr: Place::local(discr_local),
            targets,
            otherwise,
        });
        // Lower each arm body.
        for (i, arm) in arms.iter().enumerate() {
            self.start_block(arm_blocks[i]);
            // Bind pattern variables.
            match &arm.pat.kind {
                PatKind::Bind(name) => {
                    let b = self.alloc_local(name.clone(), self.cx.i64, true);
                    self.assign(b, Rvalue::Use(Place::local(scr)));
                }
                PatKind::Variant { binds, .. } => {
                    if let Some(bind) = binds.first() {
                        let b = self.alloc_local(bind.name.clone(), self.cx.i64, true);
                        self.assign(
                            b,
                            Rvalue::Load {
                                addr: Place::local(scr),
                                offset: 8,
                                ty: self.cx.i64,
                            },
                        );
                    }
                }
                _ => {}
            }
            self.lower_block(&arm.body)?;
            self.flush(Terminator::Goto(merge_blk));
        }
        if let Some(ob) = otherwise_blk {
            self.start_block(ob);
            self.flush(Terminator::Goto(merge_blk));
        }
        self.start_block(merge_blk);
        Ok(())
    }

    fn lower_stmt(&mut self, s: &Stmt) -> Result<(), String> {
        match &s.kind {
            StmtKind::OwnerBinding { name, value, .. } => {
                // `:x = expr` creates a NEW mutable owned binding. If `x` is
                // already declared in the CURRENT scope, this is a re-declaration
                // error. Shadowing an outer-scope binding is allowed. The RHS
                // expression resolves `x` to the outer binding (if any) before
                // the new binding is installed.
                if self.declared_in_current_scope(name) {
                    return Err(format!(
                        "`{}` is already declared in this scope (use `{} = ...` to assign, \
                         or move to an inner scope to shadow)",
                        name, name
                    ));
                }
                let ty = self.guess_ty(value);
                let dest = self.alloc_local(name.clone(), ty, true);
                self.lower_expr_into(value, dest)?;
            }
            StmtKind::ConstBinding { name, value, .. } => {
                // `::x = expr` — immutable owned binding, same re-decl rule.
                if self.declared_in_current_scope(name) {
                    return Err(format!("`{}` is already declared in this scope", name));
                }
                let ty = self.guess_ty(value);
                let dest = self.alloc_local(name.clone(), ty, false);
                self.lower_expr_into(value, dest)?;
            }
            StmtKind::MagnetBinding { name, target, .. }
            | StmtKind::MagnetMutBinding { name, target, .. } => {
                let is_mut = matches!(s.kind, StmtKind::MagnetMutBinding { .. });
                // When the target is an existing owner local (e.g. `~m = value`),
                // the magnet must point at the OWNER'S STORAGE, not a copy.
                // We use the owner local directly as the magnet's place so the
                // codegen can take its real address. Otherwise (e.g. an
                // expression), we allocate a temp to hold the value.
                let target_place = match &target.kind {
                    ExprKind::Path(p) if p.segments.len() == 1 => self.lookup(&p.segments[0]),
                    _ => None,
                };
                let magnet = self.alloc_local(name.clone(), self.cx.i64, is_mut);
                if let Some(owner) = target_place {
                    // `~m = owner` — magnet aliases the owner's storage.
                    // Mark the owner as address-taken so codegen materializes
                    // it into a stack slot (see lower.rs address_taken set).
                    self.address_taken.insert(owner);
                    self.assign(
                        magnet,
                        Rvalue::MagnetAttach {
                            place: Place::local(owner),
                            is_mut,
                            ty: self.cx.i64,
                        },
                    );
                } else {
                    // `~m = <expr>` — evaluate into a fresh temp and attach
                    // the magnet to that temp's storage.
                    let target_local = self.anon(self.cx.i64);
                    self.lower_expr_into(target, target_local)?;
                    self.address_taken.insert(target_local);
                    self.assign(
                        magnet,
                        Rvalue::MagnetAttach {
                            place: Place::local(target_local),
                            is_mut,
                            ty: self.cx.i64,
                        },
                    );
                }
                // Record the magnet so that `m = value` is lowered as a store
                // THROUGH the magnet (to the target storage), not a reassignment
                // of the magnet local (which holds the address).
                self.magnets.insert(magnet, is_mut);
            }
            StmtKind::Return(Some(e)) => {
                let tmp = self.anon(self.ret_ty);
                self.lower_expr_into(e, tmp)?;
                self.flush(Terminator::Return {
                    value: Some(Place::local(tmp)),
                });
                // After a return, the current block is terminated. Allocate a
                // fresh dead block for any subsequent statements (they are
                // unreachable but the builder still needs a current block).
                // This prevents the fallthrough `flush(Goto(merge))` of an
                // enclosing if/while/for from clobbering the Return.
                let dead = self.new_block_id();
                self.start_block(dead);
            }
            StmtKind::Return(None) => {
                self.flush(Terminator::Return { value: None });
                let dead = self.new_block_id();
                self.start_block(dead);
            }
            StmtKind::If {
                branches,
                else_branch,
            } => {
                // Lower if / else-if / else as a chain of conditional branches.
                // The merge block is shared by all branch bodies.
                let merge_blk = self.new_block_id();
                for (i, (cond, then_body)) in branches.iter().enumerate() {
                    let cond_local = self.anon(self.cx.bool);
                    self.lower_expr_into(cond, cond_local)?;
                    let then_blk = self.new_block_id();
                    let else_blk = self.new_block_id();
                    self.flush(Terminator::SwitchInt {
                        discr: Place::local(cond_local),
                        targets: vec![(1, then_blk)],
                        otherwise: else_blk,
                    });
                    // Then branch.
                    self.start_block(then_blk);
                    self.lower_block(then_body)?;
                    self.flush(Terminator::Goto(merge_blk));
                    // Continue building the else block for the next branch
                    // (or the final else).
                    self.start_block(else_blk);
                    // If this is the last branch and there's no else_branch,
                    // the else_blk falls through to merge.
                    if i == branches.len() - 1 && else_branch.is_none() {
                        self.flush(Terminator::Goto(merge_blk));
                    }
                }
                // Lower the final else body (if present) in the current block.
                if let Some(eb) = else_branch {
                    self.lower_block(eb)?;
                    self.flush(Terminator::Goto(merge_blk));
                }
                self.start_block(merge_blk);
            }
            StmtKind::While { cond, body } => {
                let header = self.new_block_id();
                let body_blk = self.new_block_id();
                let exit_blk = self.new_block_id();
                self.flush(Terminator::Goto(header));
                self.start_block(header);
                let cond_local = self.anon(self.cx.bool);
                self.lower_expr_into(cond, cond_local)?;
                self.flush(Terminator::SwitchInt {
                    discr: Place::local(cond_local),
                    targets: vec![(1, body_blk)],
                    otherwise: exit_blk,
                });
                self.start_block(body_blk);
                self.loop_ctx.push((header, exit_blk));
                self.lower_block(body)?;
                self.loop_ctx.pop();
                self.flush(Terminator::Goto(header));
                self.start_block(exit_blk);
            }
            StmtKind::Loop(body) => {
                let header = self.new_block_id();
                let exit_blk = self.new_block_id();
                self.flush(Terminator::Goto(header));
                self.start_block(header);
                self.loop_ctx.push((header, exit_blk));
                self.lower_block(body)?;
                self.loop_ctx.pop();
                self.flush(Terminator::Goto(header));
                self.start_block(exit_blk);
            }
            StmtKind::For {
                pat, iter, body, ..
            } => {
                // Lower `for pat in lo..hi` as a counted loop.
                // Evaluate lo and hi, then loop with a counter.
                if let ExprKind::Range { lo, hi } = &iter.kind {
                    let lo_local = self.anon(self.cx.i64);
                    self.lower_expr_into(lo, lo_local)?;
                    let hi_local = self.anon(self.cx.i64);
                    self.lower_expr_into(hi, hi_local)?;
                    let counter = self.alloc_local(
                        match &pat.kind {
                            crate::ast::pat::PatKind::Bind(name) => name.clone(),
                            _ => String::from("__i"),
                        },
                        self.cx.i64,
                        true,
                    );
                    self.assign(counter, Rvalue::Use(Place::local(lo_local)));
                    let header = self.new_block_id();
                    let body_blk = self.new_block_id();
                    let exit_blk = self.new_block_id();
                    self.flush(Terminator::Goto(header));
                    self.start_block(header);
                    // cond = counter < hi
                    let cmp = self.anon(self.cx.bool);
                    self.assign(
                        cmp,
                        Rvalue::BinOp {
                            op: BinOp::Lt,
                            lhs: Place::local(counter),
                            rhs: Place::local(hi_local),
                            ty: self.cx.bool,
                        },
                    );
                    self.flush(Terminator::SwitchInt {
                        discr: Place::local(cmp),
                        targets: vec![(1, body_blk)],
                        otherwise: exit_blk,
                    });
                    self.start_block(body_blk);
                    self.loop_ctx.push((header, exit_blk));
                    self.lower_block(body)?;
                    self.loop_ctx.pop();
                    // counter += 1
                    let one = self.anon(self.cx.i64);
                    self.assign(one, Rvalue::Const(Const::Int(1)));
                    self.assign(
                        counter,
                        Rvalue::BinOp {
                            op: BinOp::Add,
                            lhs: Place::local(counter),
                            rhs: Place::local(one),
                            ty: self.cx.i64,
                        },
                    );
                    self.flush(Terminator::Goto(header));
                    self.start_block(exit_blk);
                } else {
                    // Non-range iterator: evaluate but don't loop (v0.1 limit).
                    let tmp = self.anon(self.cx.i64);
                    self.lower_expr_into(iter, tmp)?;
                }
            }
            StmtKind::Break => {
                if let Some((_, exit)) = self.loop_ctx.last().copied() {
                    self.flush(Terminator::Goto(exit));
                    let dead = self.new_block_id();
                    self.start_block(dead);
                }
            }
            StmtKind::Continue => {
                if let Some((header, _)) = self.loop_ctx.last().copied() {
                    self.flush(Terminator::Goto(header));
                    let dead = self.new_block_id();
                    self.start_block(dead);
                }
            }
            StmtKind::Match { scrutinee, arms } => {
                self.lower_match(scrutinee, arms)?;
            }
            StmtKind::MagnetRetarget { magnet, target } => {
                // `m -> target` — re-point the magnet at a new value.
                // Lower the target into a temp, create a stack slot for it,
                // and store the new address into the magnet local.
                let target_local = self.anon(self.cx.i64);
                self.lower_expr_into(target, target_local)?;
                // Resolve the magnet local from the magnet expression (must be a path).
                let m_local = match &magnet.kind {
                    ExprKind::Path(p) if p.segments.len() == 1 => self.lookup(&p.segments[0]),
                    _ => None,
                };
                if let Some(m) = m_local {
                    self.assign(
                        m,
                        Rvalue::MagnetAttach {
                            place: Place::local(target_local),
                            is_mut: true,
                            ty: self.cx.i64,
                        },
                    );
                }
            }
            StmtKind::Drop(e) => {
                // `place!` — explicit early drop. Emit a MIR `Stmt::Drop` so the
                // ownership checker records the drop and can flag use-after-drop
                // and double-drop. We do NOT reassign the local (a prior assign
                // would reset its state to Init and mask a double-drop).
                match &e.kind {
                    ExprKind::Path(p) if p.segments.len() == 1 => {
                        if let Some(pl) = self.lookup(&p.segments[0]) {
                            self.emit(MirStmt::Drop(Place::local(pl)));
                        }
                    }
                    _ => {}
                }
            }
            StmtKind::Expr(e) => {
                let tmp = self.anon(self.cx.i32);
                self.lower_expr_into(e, tmp)?;
            }
            StmtKind::Assignment { place, op, value } => {
                use crate::ast::stmt::AssignOp;
                // Resolve the assignment target into one of:
                //   (a) a direct local (scalar/owner): `x = v`
                //   (b) a field of a local struct: `pt.x = v` / `pt.x.y = v`
                //   (c) a field through a mutable magnet: `m.x = v`
                //       (m holds the address of the struct pointer; load it,
                //        then store to struct_ptr + field_offset).
                // For (b)/(c) the write is a Store to memory, not an SSA def.

                // --- (c) Magnet field store: place is `m.field` via Field expr,
                //     where m is a magnet local. ---
                if let crate::ast::expr::ExprKind::Field { receiver, name } = &place.kind {
                    if let crate::ast::expr::ExprKind::Path(p) = &receiver.kind {
                        if p.segments.len() == 1 {
                            if let Some(m) = self.lookup(&p.segments[0]) {
                                if self.magnets.contains_key(&m) {
                                    // m is a mutable magnet; load the struct
                                    // pointer it points at, then store the field.
                                    if let Some(off) = self.field_offset(name) {
                                        let struct_ptr = self.anon(self.cx.i64);
                                        self.assign(
                                            struct_ptr,
                                            Rvalue::Load {
                                                addr: Place::local(m),
                                                offset: 0,
                                                ty: self.cx.i64,
                                            },
                                        );
                                        let val_local = self.anon(self.cx.i64);
                                        self.lower_expr_into(value, val_local)?;
                                        self.assign(
                                            struct_ptr,
                                            Rvalue::Store {
                                                addr: Place::local(struct_ptr),
                                                offset: off,
                                                value: Place::local(val_local),
                                                ty: self.cx.i64,
                                            },
                                        );
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                }

                // --- (b) Field store on a local struct: place is a multi-segment
                //     path `pt.x` or `pt.x.y`. If the base is a magnet, the
                //     first deref loads the struct pointer THROUGH the magnet
                //     (magnet holds the address of the owner slot, which holds
                //     the struct pointer); otherwise the base IS the struct
                //     pointer. ---
                if let crate::ast::expr::ExprKind::Path(p) = &place.kind {
                    if p.segments.len() >= 2 {
                        if let Some(base) = self.lookup(&p.segments[0]) {
                            let is_magnet = self.magnets.contains_key(&base);
                            let cur = self.anon(self.cx.i64);
                            if is_magnet {
                                // base is a magnet: load the struct pointer
                                // through it (load from the address base holds).
                                self.assign(
                                    cur,
                                    Rvalue::Load {
                                        addr: Place::local(base),
                                        offset: 0,
                                        ty: self.cx.i64,
                                    },
                                );
                            } else {
                                // base is a struct owner: its value is the
                                // struct pointer.
                                self.assign(cur, Rvalue::Use(Place::local(base)));
                            }
                            let n = p.segments.len() - 1; // field segments
                            for (i, seg) in p.segments[1..].iter().enumerate() {
                                if let Some(off) = self.field_offset(seg) {
                                    if i == n - 1 {
                                        // Last field: STORE here.
                                        let val_local = self.anon(self.cx.i64);
                                        self.lower_expr_into(value, val_local)?;
                                        self.assign(
                                            cur,
                                            Rvalue::Store {
                                                addr: Place::local(cur),
                                                offset: off,
                                                value: Place::local(val_local),
                                                ty: self.cx.i64,
                                            },
                                        );
                                        return Ok(());
                                    } else {
                                        // Intermediate field: load.
                                        self.assign(
                                            cur,
                                            Rvalue::Load {
                                                addr: Place::local(cur),
                                                offset: off,
                                                ty: self.cx.i64,
                                            },
                                        );
                                    }
                                } else {
                                    return Ok(());
                                }
                            }
                        }
                    }
                }

                // --- (b') Field store on a local struct via Field expr ---
                if let crate::ast::expr::ExprKind::Field { receiver, name } = &place.kind {
                    // Evaluate receiver to get the struct pointer.
                    let base = self.anon(self.cx.i64);
                    self.lower_expr_into(receiver, base)?;
                    if let Some(off) = self.field_offset(name) {
                        let val_local = self.anon(self.cx.i64);
                        self.lower_expr_into(value, val_local)?;
                        self.assign(
                            base,
                            Rvalue::Store {
                                addr: Place::local(base),
                                offset: off,
                                value: Place::local(val_local),
                                ty: self.cx.i64,
                            },
                        );
                        return Ok(());
                    }
                }

                // --- (a) Direct local assignment ---
                let place_local = match &place.kind {
                    crate::ast::expr::ExprKind::Path(p) if p.segments.len() == 1 => {
                        self.lookup(&p.segments[0])
                    }
                    _ => None,
                };
                if let Some(pl) = place_local {
                    // Magnet store-through: if `pl` is a magnet local, then
                    // `m = value` must store `value` THROUGH the magnet's held
                    // address (i.e. into the target/owner storage), NOT rebind
                    // the magnet local (which would clobber the address).
                    // The magnet local itself is only ever written by the
                    // MagnetAttach rvalue that initialized it.
                    if self.magnets.contains_key(&pl) {
                        let val_local = self.anon(self.cx.i64);
                        self.lower_expr_into(value, val_local)?;
                        self.assign(
                            pl,
                            Rvalue::Store {
                                addr: Place::local(pl),
                                offset: 0,
                                value: Place::local(val_local),
                                ty: self.cx.i64,
                            },
                        );
                        return Ok(());
                    }
                    match op {
                        AssignOp::Assign => {
                            self.lower_expr_into(value, pl)?;
                        }
                        AssignOp::Add => {
                            let rhs = self.anon(self.cx.i64);
                            self.lower_expr_into(value, rhs)?;
                            let cur = self.anon(self.cx.i64);
                            self.assign(cur, Rvalue::Use(Place::local(pl)));
                            self.assign(
                                pl,
                                Rvalue::BinOp {
                                    op: BinOp::Add,
                                    lhs: Place::local(cur),
                                    rhs: Place::local(rhs),
                                    ty: self.cx.i64,
                                },
                            );
                        }
                        AssignOp::Sub => {
                            let rhs = self.anon(self.cx.i64);
                            self.lower_expr_into(value, rhs)?;
                            let cur = self.anon(self.cx.i64);
                            self.assign(cur, Rvalue::Use(Place::local(pl)));
                            self.assign(
                                pl,
                                Rvalue::BinOp {
                                    op: BinOp::Sub,
                                    lhs: Place::local(cur),
                                    rhs: Place::local(rhs),
                                    ty: self.cx.i64,
                                },
                            );
                        }
                        _ => {
                            self.lower_expr_into(value, pl)?;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn lower_expr_into(&mut self, e: &Expr, dest: LocalId) -> Result<(), String> {
        match &e.kind {
            ExprKind::Literal(Lit::Int(s)) => {
                let v: i64 = parse_int(s);
                self.assign(dest, Rvalue::Const(Const::Int(v)));
            }
            ExprKind::Literal(Lit::Float(s)) => {
                let v: f64 = s.parse().unwrap_or(0.0);
                self.assign(dest, Rvalue::Const(Const::Float(v)));
            }
            ExprKind::Literal(Lit::Bool(b)) => {
                self.assign(dest, Rvalue::Const(Const::Bool(*b)));
            }
            ExprKind::Literal(Lit::Char(c)) => {
                self.assign(dest, Rvalue::Const(Const::Char(*c)));
            }
            ExprKind::Literal(Lit::Str(s)) => {
                // A string literal assigned to a variable becomes a heap-allocated
                // Text value: avera_text_new(n) followed by avera_text_set for each byte.
                let n = s.len() as i64;
                let n_local = self.anon(self.cx.i64);
                self.assign(n_local, Rvalue::Const(Const::Int(n)));
                self.emit_heap_call(
                    dest,
                    "avera_text_new",
                    vec![Place::local(n_local)],
                    self.cx.i64,
                );
                for (i, b) in s.bytes().enumerate() {
                    let val = self.anon(self.cx.i64);
                    self.assign(val, Rvalue::Const(Const::U64(b as u64)));
                    let idx = self.anon(self.cx.i64);
                    self.assign(idx, Rvalue::Const(Const::Int(i as i64)));
                    let void_dest = self.anon(self.cx.unit);
                    self.emit(MirStmt::Call {
                        dest: Place::local(void_dest),
                        callee: "avera_text_set".to_string(),
                        args: vec![Place::local(dest), Place::local(idx), Place::local(val)],
                        ret_ty: self.cx.unit,
                    });
                }
            }
            ExprKind::Path(p) => {
                if p.segments.len() == 1 {
                    if let Some(id) = self.lookup(&p.segments[0]) {
                        if id != dest {
                            self.assign(dest, Rvalue::Use(Place::local(id)));
                        }
                    }
                } else if p.segments.len() >= 2 {
                    // Multi-segment path: could be a field access (pt.x),
                    // a magnet field access (m.x), or a module path (std.io).
                    // Check if the first segment is a local variable.
                    if let Some(id) = self.lookup(&p.segments[0]) {
                        let is_magnet = self.magnets.contains_key(&id);
                        // Load the struct pointer: if the base is a magnet,
                        // load through it (magnet holds the address of the
                        // owner slot, which holds the struct pointer);
                        // otherwise the base IS the struct pointer.
                        if is_magnet {
                            self.assign(
                                dest,
                                Rvalue::Load {
                                    addr: Place::local(id),
                                    offset: 0,
                                    ty: self.cx.i64,
                                },
                            );
                        } else if id != dest {
                            self.assign(dest, Rvalue::Use(Place::local(id)));
                        }
                        for seg in &p.segments[1..] {
                            // Look up the field index and type.
                            let mut field_info: Option<(usize, TyId)> = None;
                            for fields in self.shapes.values() {
                                if let Some(idx) = fields.iter().position(|(f, _)| f == seg) {
                                    field_info = Some((idx, fields[idx].1));
                                    break;
                                }
                            }
                            if let Some((idx, _fty)) = field_info {
                                let offset = abi::field_offset_by_index(idx);
                                // Load is always I64 (see field-access note).
                                self.assign(
                                    dest,
                                    Rvalue::Load {
                                        addr: Place::local(dest),
                                        offset,
                                        ty: self.cx.i64,
                                    },
                                );
                            }
                        }
                    }
                    // If first segment isn't a local, it might be a type
                    // name or module path — skip (v0.1 limitation).
                }
            }
            ExprKind::SelfRef => {}
            ExprKind::ChoiceCtor { variant, args } => {
                // Choice construction: .some(value) or .none
                // Layout: tag at choice_tag_offset(), payload at choice_payload_offset()
                // Tag: deterministic discriminant (see variant_tag).
                let tag = self.variant_tag(variant);
                let tag_local = self.anon(self.cx.i64);
                self.assign(tag_local, Rvalue::Const(Const::Int(tag)));
                let size_local = self.anon(self.cx.i64);
                self.assign(
                    size_local,
                    Rvalue::Const(Const::Int(abi::choice_size() as i64)),
                );
                self.emit_heap_call(
                    dest,
                    "avera_alloc",
                    vec![Place::local(size_local)],
                    self.cx.i64,
                );
                // Store tag at offset 0
                self.assign(
                    dest,
                    Rvalue::Store {
                        addr: Place::local(dest),
                        value: Place::local(tag_local),
                        offset: abi::choice_tag_offset(),
                        ty: self.cx.i64,
                    },
                );
                // Store payload at offset 8 (if any)
                if let Some(arg) = args.first() {
                    let payload_local = self.anon(self.cx.i64);
                    self.lower_expr_into(arg, payload_local)?;
                    self.assign(
                        dest,
                        Rvalue::Store {
                            addr: Place::local(dest),
                            value: Place::local(payload_local),
                            offset: abi::choice_payload_offset(),
                            ty: self.cx.i64,
                        },
                    );
                }
            }
            ExprKind::MagnetMeta { target, property } => {
                // `~m.address` — load the address the magnet points to.
                if property == "address" {
                    if let ExprKind::Path(p) = &target.kind {
                        if p.segments.len() == 1 {
                            if let Some(m) = self.lookup(&p.segments[0]) {
                                self.assign(
                                    dest,
                                    Rvalue::MagnetAddress {
                                        magnet: Place::local(m),
                                        ty: self.cx.i64,
                                    },
                                );
                            }
                        }
                    }
                } else if property == "offset" {
                    // `~m.offset` — the byte offset within the pointed-to region.
                    // v0.1 magnets are single-slot, so offset is always 0.
                    if let ExprKind::Path(p) = &target.kind {
                        if p.segments.len() == 1 {
                            if let Some(m) = self.lookup(&p.segments[0]) {
                                self.assign(
                                    dest,
                                    Rvalue::MagnetOffset {
                                        magnet: Place::local(m),
                                        ty: self.cx.i64,
                                    },
                                );
                            }
                        }
                    }
                } else if property == "value" || property == "deref" {
                    // `~m.value` — load the value the magnet points to.
                    if let ExprKind::Path(p) = &target.kind {
                        if p.segments.len() == 1 {
                            if let Some(m) = self.lookup(&p.segments[0]) {
                                self.assign(
                                    dest,
                                    Rvalue::Load {
                                        addr: Place::local(m),
                                        offset: 0,
                                        ty: self.cx.i64,
                                    },
                                );
                            }
                        }
                    }
                } else if let Some(off) = self.field_offset(property) {
                    // `~m.field` — field read through a magnet. The magnet
                    // points at the owner storage, which holds the struct
                    // pointer. Load the struct pointer through the magnet,
                    // then load the field at its byte offset.
                    if let ExprKind::Path(p) = &target.kind {
                        if p.segments.len() == 1 {
                            if let Some(m) = self.lookup(&p.segments[0]) {
                                // struct_ptr = *(magnet_addr)
                                self.assign(
                                    dest,
                                    Rvalue::Load {
                                        addr: Place::local(m),
                                        offset: 0,
                                        ty: self.cx.i64,
                                    },
                                );
                                // field = *(struct_ptr + field_offset)
                                self.assign(
                                    dest,
                                    Rvalue::Load {
                                        addr: Place::local(dest),
                                        offset: off,
                                        ty: self.cx.i64,
                                    },
                                );
                            }
                        }
                    }
                }
            }
            ExprKind::Unary { op, operand } => {
                // `^x` — move. Lower to a Move rvalue pointing at the SOURCE
                // local (not a temp copy) so the ownership checker marks the
                // source as Moved. Codegen treats Move identically to Use.
                if *op == AUn::Move {
                    let operand_ty = self.guess_ty(operand);
                    if let ExprKind::Path(p) = &operand.kind {
                        if p.segments.len() == 1 {
                            if let Some(src) = self.lookup(&p.segments[0]) {
                                self.assign(
                                    dest,
                                    Rvalue::Move {
                                        place: Place::local(src),
                                        ty: operand_ty,
                                    },
                                );
                                return Ok(());
                            }
                        }
                    }
                    // Non-path move target: lower into a temp and move it.
                    let tmp = self.anon(operand_ty);
                    self.lower_expr_into(operand, tmp)?;
                    self.assign(
                        dest,
                        Rvalue::Move {
                            place: Place::local(tmp),
                            ty: operand_ty,
                        },
                    );
                    return Ok(());
                }
                self.lower_expr_into(operand, dest)?;
                let mop = match op {
                    AUn::Not => UnOp::Not,
                    AUn::Neg => UnOp::Neg,
                    _ => return Ok(()),
                };
                self.assign(
                    dest,
                    Rvalue::UnOp {
                        op: mop,
                        operand: Place::local(dest),
                        ty: self.cx.i32,
                    },
                );
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let ty = self.guess_ty(e);
                // Detect Text-typed operands (for concat / equality dispatch).
                let operand_ty = {
                    let lt = self.guess_ty(lhs);
                    let rt = self.guess_ty(rhs);
                    if lt == self.cx.text || rt == self.cx.text {
                        self.cx.text
                    } else {
                        ty
                    }
                };
                // Text concatenation: "a" + "b" -> avera_text_concat(a, b)
                if operand_ty == self.cx.text && *op == crate::ast::expr::BinOp::Add {
                    self.lower_expr_into(lhs, dest)?;
                    let rhs_local = self.anon(self.cx.i64);
                    self.lower_expr_into(rhs, rhs_local)?;
                    self.emit(MirStmt::Call {
                        dest: Place::local(dest),
                        callee: "avera_text_concat".to_string(),
                        args: vec![Place::local(dest), Place::local(rhs_local)],
                        ret_ty: self.cx.i64,
                    });
                    return Ok(());
                }
                // Text equality: a == b / a != b -> avera_text_eq(a, b)
                if operand_ty == self.cx.text
                    && (*op == crate::ast::expr::BinOp::Eq || *op == crate::ast::expr::BinOp::Ne)
                {
                    self.lower_expr_into(lhs, dest)?;
                    let rhs_local = self.anon(self.cx.i64);
                    self.lower_expr_into(rhs, rhs_local)?;
                    let eq_local = self.anon(self.cx.i64);
                    self.emit(MirStmt::Call {
                        dest: Place::local(eq_local),
                        callee: "avera_text_eq".to_string(),
                        args: vec![Place::local(dest), Place::local(rhs_local)],
                        ret_ty: self.cx.i64,
                    });
                    if *op == crate::ast::expr::BinOp::Eq {
                        self.assign(dest, Rvalue::Use(Place::local(eq_local)));
                    } else {
                        // Ne: invert the eq result (1 - eq).
                        let one = self.anon(self.cx.i64);
                        self.assign(one, Rvalue::Const(Const::Int(1)));
                        self.assign(
                            dest,
                            Rvalue::BinOp {
                                op: crate::mir::rvalue::BinOp::Sub,
                                lhs: Place::local(one),
                                rhs: Place::local(eq_local),
                                ty: self.cx.i64,
                            },
                        );
                    }
                    return Ok(());
                }
                self.lower_expr_into(lhs, dest)?;
                let rhs_local = self.anon(ty);
                self.lower_expr_into(rhs, rhs_local)?;
                let mop = map_binop(*op);
                self.assign(
                    dest,
                    Rvalue::BinOp {
                        op: mop,
                        lhs: Place::local(dest),
                        rhs: Place::local(rhs_local),
                        ty,
                    },
                );
            }
            ExprKind::Call { callee, args } => {
                if let ExprKind::Path(p) = &callee.kind {
                    let callee_name = p.segments.last().map(|s| s.as_str()).unwrap_or("");
                    // Handle multi-segment path call: pt.sum() -> method call
                    if p.segments.len() >= 2 && self.lookup(&p.segments[0]).is_some() {
                        // Built-in methods on arrays/magnets first.
                        let recv_local = self.anon(self.cx.i64);
                        if let Some(id) = self.lookup(&p.segments[0]) {
                            self.assign(recv_local, Rvalue::Use(Place::local(id)));
                        }
                        match callee_name {
                            "len" => {
                                self.emit(MirStmt::Call {
                                    dest: Place::local(dest),
                                    callee: "avera_array_len".to_string(),
                                    args: vec![Place::local(recv_local)],
                                    ret_ty: self.cx.i64,
                                });
                                return Ok(());
                            }
                            "capacity" | "cap" => {
                                self.emit(MirStmt::Call {
                                    dest: Place::local(dest),
                                    callee: "avera_array_cap".to_string(),
                                    args: vec![Place::local(recv_local)],
                                    ret_ty: self.cx.i64,
                                });
                                return Ok(());
                            }
                            "push" => {
                                let val = self.anon(self.cx.i64);
                                if let Some(a) = args.first() {
                                    self.lower_expr_into(a, val)?;
                                }
                                self.emit(MirStmt::Call {
                                    dest: Place::local(dest),
                                    callee: "avera_array_push".to_string(),
                                    args: vec![Place::local(recv_local), Place::local(val)],
                                    ret_ty: self.cx.i64,
                                });
                                return Ok(());
                            }
                            "pop" => {
                                self.emit(MirStmt::Call {
                                    dest: Place::local(dest),
                                    callee: "avera_array_pop".to_string(),
                                    args: vec![Place::local(recv_local)],
                                    ret_ty: self.cx.i64,
                                });
                                return Ok(());
                            }
                            _ => {}
                        }
                        // User method call on a shape: var.method(args).
                        // The callee is "Type.method" — look it up.
                        let suffix = format!(".{}", callee_name);
                        let full_callee = self
                            .action_names
                            .iter()
                            .find(|n| n.ends_with(&suffix))
                            .cloned()
                            .unwrap_or_else(|| callee_name.to_string());
                        // First arg is the receiver (recv_local already bound above).
                        let mut arg_places = vec![Place::local(recv_local)];
                        for a in args {
                            let tmp = self.anon(self.cx.i32);
                            self.lower_expr_into(a, tmp).ok();
                            arg_places.push(Place::local(tmp));
                        }
                        self.emit(MirStmt::Call {
                            dest: Place::local(dest),
                            callee: full_callee,
                            args: arg_places,
                            ret_ty: self.cx.i32,
                        });
                        return Ok(());
                    }
                    match callee_name {
                        "print" => self.lower_print(args, dest, true)?,
                        "write" => self.lower_print(args, dest, false)?,
                        "eprint" => self.lower_print_eprint(args, dest)?,
                        "panic" => {
                            self.flush(Terminator::Abort);
                            let dead = self.new_block_id();
                            self.start_block(dead);
                        }
                        "input" => {
                            // input() is typed Text, NOT I64: it reads a whole
                            // line from stdin into a real Text object (see
                            // avera_text_read_line), so the result can be
                            // printed/concatenated/inspected as Text.
                            self.emit(MirStmt::Call {
                                dest: Place::local(dest),
                                callee: "avera_text_read_line".to_string(),
                                args: vec![],
                                ret_ty: self.cx.text,
                            });
                        }
                        "@array" => {
                            // Array literal: [1, 2, 3]
                            let len = args.len() as i64;
                            let size_local = self.anon(self.cx.i64);
                            self.assign(size_local, Rvalue::Const(Const::Int(len)));
                            let elem_size = self.anon(self.cx.i64);
                            self.assign(elem_size, Rvalue::Const(Const::Int(8)));
                            self.emit(MirStmt::Call {
                                dest: Place::local(dest),
                                callee: "avera_alloc_array".to_string(),
                                args: vec![Place::local(size_local), Place::local(elem_size)],
                                ret_ty: self.cx.i64,
                            });
                            for (i, arg) in args.iter().enumerate() {
                                let val_local = self.anon(self.cx.i64);
                                self.lower_expr_into(arg, val_local)?;
                                let idx_local = self.anon(self.cx.i64);
                                self.assign(idx_local, Rvalue::Const(Const::Int(i as i64)));
                                self.emit(MirStmt::Call {
                                    dest: Place::local(dest),
                                    callee: "avera_array_set".to_string(),
                                    args: vec![
                                        Place::local(dest),
                                        Place::local(idx_local),
                                        Place::local(val_local),
                                        Place::local(elem_size),
                                    ],
                                    ret_ty: self.cx.unit,
                                });
                            }
                        }
                        _ if self.shapes.contains_key(callee_name) => {
                            // Shape construction: User("name", age)
                            // Allocate memory for the struct.
                            let shape_fields =
                                self.shapes.get(callee_name).cloned().unwrap_or_default();
                            let size_local = self.anon(self.cx.i64);
                            self.assign(
                                size_local,
                                Rvalue::Const(Const::Int(abi::shape_size(&shape_fields) as i64)),
                            );
                            self.emit(MirStmt::Call {
                                dest: Place::local(dest),
                                callee: "avera_alloc".to_string(),
                                args: vec![Place::local(size_local)],
                                ret_ty: self.cx.i64,
                            });
                            // Store each field at its offset.
                            for (i, arg) in args.iter().enumerate() {
                                let field_local = self.anon(self.cx.i64);
                                self.lower_expr_into(arg, field_local)?;
                                self.assign(
                                    dest,
                                    Rvalue::Store {
                                        addr: Place::local(dest),
                                        value: Place::local(field_local),
                                        offset: abi::field_offset_by_index(i),
                                        ty: self.cx.i64,
                                    },
                                );
                            }
                        }
                        _ => {
                            let arg_places: Vec<Place> = args
                                .iter()
                                .map(|a| {
                                    let tmp = self.anon(self.cx.i32);
                                    self.lower_expr_into(a, tmp).ok();
                                    Place::local(tmp)
                                })
                                .collect();
                            self.emit(MirStmt::Call {
                                dest: Place::local(dest),
                                callee: callee_name.to_string(),
                                args: arg_places,
                                ret_ty: self.cx.i32,
                            });
                        }
                    }
                }
            }
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                // Special: arr.len() -> avera_array_len(arr)
                if method == "len" && args.is_empty() {
                    self.lower_expr_into(receiver, dest)?;
                    self.emit(MirStmt::Call {
                        dest: Place::local(dest),
                        callee: "avera_array_len".to_string(),
                        args: vec![Place::local(dest)],
                        ret_ty: self.cx.i64,
                    });
                    return Ok(());
                }
                // Method call: receiver.method(args) -> Type.method(receiver, args)
                // Find the action name that ends with ".method".
                let suffix = format!(".{}", method);
                let callee = self
                    .action_names
                    .iter()
                    .find(|n| n.ends_with(&suffix))
                    .cloned()
                    .unwrap_or_else(|| method.clone());
                let mut arg_places: Vec<Place> = Vec::new();
                // First arg is the receiver (by value/pointer).
                let recv_local = self.anon(self.cx.i64);
                self.lower_expr_into(receiver, recv_local)?;
                arg_places.push(Place::local(recv_local));
                for a in args {
                    let tmp = self.anon(self.cx.i32);
                    self.lower_expr_into(a, tmp).ok();
                    arg_places.push(Place::local(tmp));
                }
                self.emit(MirStmt::Call {
                    dest: Place::local(dest),
                    callee,
                    args: arg_places,
                    ret_ty: self.cx.i32,
                });
            }
            ExprKind::Field { receiver, name } => {
                // Field access: u.name -> load from struct pointer at offset
                // First, evaluate the receiver to get the struct pointer.
                self.lower_expr_into(receiver, dest)?;
                // Look up the field index and type. We try all shapes and use
                // the first match (v0.1 shapes are nominal, no shadowing).
                let mut field_info: Option<(usize, TyId)> = None;
                for fields in self.shapes.values() {
                    if let Some(idx) = fields.iter().position(|(f, _)| f == name) {
                        field_info = Some((idx, fields[idx].1));
                        break;
                    }
                }
                if let Some((idx, _fty)) = field_info {
                    let offset = abi::field_offset_by_index(idx);
                    // Codegen declares all locals as I64, so the load type must
                    // stay I64 even when the field's logical type is Text/F64.
                    // guess_ty uses _fty to pick the right printer.
                    self.assign(
                        dest,
                        Rvalue::Load {
                            addr: Place::local(dest),
                            offset,
                            ty: self.cx.i64,
                        },
                    );
                }
            }
            ExprKind::Index { base, index } => {
                // Array index: arr[i] -> avera_array_get(arr, i, 8)
                self.lower_expr_into(base, dest)?;
                let idx = self.anon(self.cx.i64);
                self.lower_expr_into(index, idx)?;
                let elem_size = self.anon(self.cx.i64);
                self.assign(elem_size, Rvalue::Const(Const::Int(8)));
                self.emit(MirStmt::Call {
                    dest: Place::local(dest),
                    callee: "avera_array_get".to_string(),
                    args: vec![
                        Place::local(dest),
                        Place::local(idx),
                        Place::local(elem_size),
                    ],
                    ret_ty: self.cx.i64,
                });
                // Dereference the pointer returned by avera_array_get.
                self.assign(
                    dest,
                    Rvalue::Load {
                        addr: Place::local(dest),
                        offset: 0,
                        ty: self.cx.i64,
                    },
                );
            }
            ExprKind::Cast { operand, target: _ } => {
                self.lower_expr_into(operand, dest)?;
                self.assign(
                    dest,
                    Rvalue::Cast {
                        operand: Place::local(dest),
                        from: self.cx.i32,
                        to: self.cx.i64,
                    },
                );
            }
            ExprKind::Paren(inner) => {
                self.lower_expr_into(inner, dest)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn lower_print(&mut self, args: &[Expr], dest: LocalId, newline: bool) -> Result<(), String> {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                // space separator
                let ch = self.anon(self.cx.u8);
                self.assign(ch, Rvalue::Const(Const::U64(b' ' as u64)));
                self.emit(MirStmt::Call {
                    dest: Place::local(dest),
                    callee: "avera_putc".to_string(),
                    args: vec![Place::local(ch)],
                    ret_ty: self.cx.unit,
                });
            }
            self.lower_print_arg(arg, dest)?;
        }
        if newline {
            let ch = self.anon(self.cx.u8);
            self.assign(ch, Rvalue::Const(Const::U64(b'\n' as u64)));
            self.emit(MirStmt::Call {
                dest: Place::local(dest),
                callee: "avera_putc".to_string(),
                args: vec![Place::local(ch)],
                ret_ty: self.cx.unit,
            });
        }
        Ok(())
    }

    fn lower_print_eprint(&mut self, args: &[Expr], dest: LocalId) -> Result<(), String> {
        for arg in args {
            self.lower_print_arg(arg, dest)?;
        }
        let ch = self.anon(self.cx.u8);
        self.assign(ch, Rvalue::Const(Const::U64(b'\n' as u64)));
        self.emit(MirStmt::Call {
            dest: Place::local(dest),
            callee: "avera_putc".to_string(),
            args: vec![Place::local(ch)],
            ret_ty: self.cx.unit,
        });
        Ok(())
    }

    fn lower_print_arg(&mut self, arg: &Expr, dest: LocalId) -> Result<(), String> {
        match &arg.kind {
            ExprKind::Literal(Lit::Str(s)) => {
                self.lower_print_str(s, dest)?;
            }
            ExprKind::Literal(Lit::Int(s)) => {
                let v: i64 = parse_int(s);
                let tmp = self.anon(self.cx.i64);
                self.assign(tmp, Rvalue::Const(Const::Int(v)));
                self.emit(MirStmt::Call {
                    dest: Place::local(dest),
                    callee: "avera_print_i64".to_string(),
                    args: vec![Place::local(tmp)],
                    ret_ty: self.cx.unit,
                });
            }
            ExprKind::Literal(Lit::Float(s)) => {
                let v: f64 = s.parse().unwrap_or(0.0);
                let tmp = self.anon(self.cx.f64);
                self.assign(tmp, Rvalue::Const(Const::Float(v)));
                self.emit(MirStmt::Call {
                    dest: Place::local(dest),
                    callee: "avera_print_f64".to_string(),
                    args: vec![Place::local(tmp)],
                    ret_ty: self.cx.unit,
                });
            }
            ExprKind::Literal(Lit::Bool(b)) => {
                let s = if *b { "true" } else { "false" };
                self.lower_print_str(s, dest)?;
            }
            ExprKind::Literal(Lit::Char(c)) => {
                let tmp = self.anon(self.cx.u8);
                self.assign(tmp, Rvalue::Const(Const::U64(*c as u64)));
                self.emit(MirStmt::Call {
                    dest: Place::local(dest),
                    callee: "avera_putc".to_string(),
                    args: vec![Place::local(tmp)],
                    ret_ty: self.cx.unit,
                });
            }
            _ => {
                let ty = self.guess_ty(arg);
                // Use a fresh temp of the right type, NOT dest (which is I64).
                let tmp = self.anon(ty);
                self.lower_expr_into(arg, tmp)?;
                if ty == self.cx.f64 || ty == self.cx.f32 {
                    self.emit(MirStmt::Call {
                        dest: Place::local(dest),
                        callee: "avera_print_f64".to_string(),
                        args: vec![Place::local(tmp)],
                        ret_ty: self.cx.unit,
                    });
                } else if ty == self.cx.text {
                    // Print a Text value's bytes via avera_text_print.
                    self.emit(MirStmt::Call {
                        dest: Place::local(dest),
                        callee: "avera_text_print".to_string(),
                        args: vec![Place::local(tmp)],
                        ret_ty: self.cx.unit,
                    });
                } else {
                    self.emit(MirStmt::Call {
                        dest: Place::local(dest),
                        callee: "avera_print_i64".to_string(),
                        args: vec![Place::local(tmp)],
                        ret_ty: self.cx.unit,
                    });
                }
            }
        }
        Ok(())
    }

    fn lower_print_str(&mut self, s: &str, dest: LocalId) -> Result<(), String> {
        for b in s.bytes() {
            let ch = self.anon(self.cx.u8);
            self.assign(ch, Rvalue::Const(Const::U64(b as u64)));
            self.emit(MirStmt::Call {
                dest: Place::local(dest),
                callee: "avera_putc".to_string(),
                args: vec![Place::local(ch)],
                ret_ty: self.cx.unit,
            });
        }
        Ok(())
    }

    fn guess_ty(&self, e: &Expr) -> TyId {
        match &e.kind {
            ExprKind::Literal(Lit::Int(_)) => self.cx.i32,
            ExprKind::Literal(Lit::Float(_)) => self.cx.f64,
            ExprKind::Literal(Lit::Bool(_)) => self.cx.bool,
            ExprKind::Literal(Lit::Char(_)) => self.cx.char,
            ExprKind::Literal(Lit::Str(_)) => self.cx.text,
            ExprKind::Path(p) if p.segments.len() == 1 => {
                // Look up the variable's declared type.
                if let Some(id) = self.lookup(&p.segments[0]) {
                    let idx = id.to_usize();
                    if idx < self.locals.len() {
                        return self.locals[idx].ty;
                    }
                }
                self.cx.i64
            }
            ExprKind::Path(p) if p.segments.len() == 2 => {
                // Multi-segment path like `u.name` is a field access.
                // Look up the field type by scanning all shapes for a matching
                // field name (v0.1 shapes don't shadow fields).
                let fname = &p.segments[1];
                for fields in self.shapes.values() {
                    if let Some((_, fty)) = fields.iter().find(|(f, _)| f == fname) {
                        return *fty;
                    }
                }
                self.cx.i64
            }
            ExprKind::Binary { op, lhs, rhs } if op.is_comparison() => self.cx.bool,
            ExprKind::Binary { lhs, rhs, .. } => {
                let lt = self.guess_ty(lhs);
                let rt = self.guess_ty(rhs);
                if lt == self.cx.text || rt == self.cx.text {
                    self.cx.text
                } else if lt == self.cx.f64 || rt == self.cx.f64 {
                    self.cx.f64
                } else if lt == self.cx.f32 || rt == self.cx.f32 {
                    self.cx.f32
                } else {
                    self.cx.i32
                }
            }
            ExprKind::Unary { operand, .. } => self.guess_ty(operand),
            ExprKind::Paren(inner) => self.guess_ty(inner),
            ExprKind::Call { callee, .. } => {
                // Built-in functions with non-I64 result types. The most
                // important one is `input()`, which is typed Text (it reads a
                // whole line), so that `print(input())` prints the line rather
                // than a pointer integer. Other calls default to I64 (a
                // pointer-sized word: function results in v0.1 are typically
                // heap handles or I64 values).
                if let ExprKind::Path(p) = &callee.kind {
                    if p.segments.len() == 1 && p.segments[0] == "input" {
                        return self.cx.text;
                    }
                }
                self.cx.i64
            }
            ExprKind::MethodCall { .. } => {
                // Method calls return I64 (pointer-sized word) by default in
                // v0.1: results are heap handles or I64 values.
                self.cx.i64
            }
            // Heap-constructing expressions produce I64 pointers.
            ExprKind::ChoiceCtor { .. } => self.cx.i64,
            ExprKind::Field { receiver, name } => {
                // Infer the field's type by looking up the receiver's shape and
                // finding the field by name. Falls back to i64 if unknown.
                let _ = self.guess_ty(receiver);
                // Search all shapes for a field with this name and return its
                // declared type. (v0.1 shapes don't shadow fields.)
                for fields in self.shapes.values() {
                    if let Some((_, fty)) = fields.iter().find(|(f, _)| f == name) {
                        return *fty;
                    }
                }
                self.cx.i64
            }
            // Default: I64 (pointer-sized word). This is safer than I32 for
            // a 64-bit target — unknown expressions are typically pointer
            // handles, and narrowing an I64 pointer to I32 would lose bits.
            _ => self.cx.i64,
        }
    }

    fn finish(&mut self) {
        // If the current block still has the placeholder `Unreachable`
        // terminator, flush it with a Return so the verifier is happy.
        // BUT: if the current block is a dead block created after a return/
        // abort (it has no predecessors and is genuinely unreachable), leave
        // it as Unreachable — flushing Return{None} there would be a false
        // "returns without a value" error.
        // EXCEPTION: block 0 (the entry block) always has an implicit
        // predecessor (the caller), so it's never dead.
        if matches!(self.blocks[self.cur_block].term, Terminator::Unreachable) {
            let cur = self.cur_block;
            // Block 0 (entry) is always live.
            if cur == 0 {
                self.flush(Terminator::Return { value: None });
                return;
            }
            // Check if this block has any predecessors by scanning all blocks'
            // terminators for jumps to `cur`.
            let mut has_pred = false;
            for (i, b) in self.blocks.iter().enumerate() {
                if i == cur {
                    continue;
                }
                has_pred = match &b.term {
                    Terminator::Goto(t) => t.get() as usize == cur,
                    Terminator::SwitchInt {
                        targets, otherwise, ..
                    } => {
                        targets.iter().any(|(_, t)| t.get() as usize == cur)
                            || otherwise.get() as usize == cur
                    }
                    Terminator::Switch {
                        targets, otherwise, ..
                    } => {
                        targets.iter().any(|(_, t)| t.get() as usize == cur)
                            || otherwise.map(|o| o.get() as usize == cur).unwrap_or(false)
                    }
                    _ => false,
                };
                if has_pred {
                    break;
                }
            }
            if has_pred {
                self.flush(Terminator::Return { value: None });
            }
        }
    }

    fn into_body(self) -> Body {
        let entry = BlockId::new(0);
        let mut blocks = self.blocks;
        // Replace remaining `Unreachable` terminators. A block is unreachable
        // only if no predecessor jumps to it (dead code after a return/break/
        // continue). Those dead blocks should terminate with `Unreachable`
        // (a trap) so that if control somehow reaches them, the program traps
        // rather than silently returning a bogus value. The return-consistency
        // validator skips `Unreachable` terminators.
        // (We do NOT replace them with Return{None} — that would mask dead
        // blocks as valid return points and could trigger a "returns without a
        // value" false positive.)
        for b in &mut blocks {
            if matches!(b.term, Terminator::Unreachable) {
                // Keep Unreachable — it's a trap, not a silent return.
            }
        }
        Body {
            name: self.name,
            locals: self.locals,
            blocks,
            entry,
            ret_ty: self.ret_ty,
            params: self.params,
            address_taken: self.address_taken,
            owns_heap: self.owns_heap,
        }
    }
}

fn map_binop(op: ABin) -> BinOp {
    match op {
        ABin::Add => BinOp::Add,
        ABin::Sub => BinOp::Sub,
        ABin::Mul => BinOp::Mul,
        ABin::Div => BinOp::Div,
        ABin::Mod => BinOp::Mod,
        ABin::Eq => BinOp::Eq,
        ABin::Ne => BinOp::Ne,
        ABin::Lt => BinOp::Lt,
        ABin::Le => BinOp::Le,
        ABin::Gt => BinOp::Gt,
        ABin::Ge => BinOp::Ge,
        ABin::And => BinOp::BitAnd,
        ABin::Or => BinOp::BitOr,
        ABin::BitAnd => BinOp::BitAnd,
        ABin::BitOr => BinOp::BitOr,
        ABin::BitXor => BinOp::BitXor,
        ABin::Shl => BinOp::Shl,
        ABin::Shr => BinOp::Shr,
    }
}

fn parse_int(s: &str) -> i64 {
    // Handle hex/binary/decimal.
    let s = s.trim_start_matches('_').replace('_', "");
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).unwrap_or(0)
    } else if let Some(b) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        i64::from_str_radix(b, 2).unwrap_or(0)
    } else {
        s.parse().unwrap_or(0)
    }
}

fn name_hash(name: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in name.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
