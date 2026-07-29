use crate::codegen::abi;
use crate::mir::body::{Body, Stmt};
use crate::mir::place::Place;
use crate::mir::rvalue::{BinOp, Const, Rvalue, UnOp};
use crate::mir::terminator::Terminator;
use crate::resolve::defs::Defs;
use crate::types::intern::TyCtxt;
use crate::types::ty::TyData;

use cranelift_codegen::ir::{types, AbiParam, InstBuilder, MemFlags, Signature, StackSlotData};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{FuncId, Linkage, Module};
use std::collections::HashMap;

struct Locals {
    vars: HashMap<u32, Variable>,
    var_tys: HashMap<u32, types::Type>,
    slots: HashMap<u32, cranelift_codegen::ir::StackSlot>,
    slot_tys: HashMap<u32, types::Type>,
}

impl Locals {
    fn new() -> Self {
        Locals {
            vars: HashMap::new(),
            var_tys: HashMap::new(),
            slots: HashMap::new(),
            slot_tys: HashMap::new(),
        }
    }

    fn cast_to(
        &self,
        builder: &mut FunctionBuilder,
        v: cranelift_codegen::ir::Value,
        to: types::Type,
    ) -> cranelift_codegen::ir::Value {
        let from = builder.func.dfg.value_type(v);
        if from == to {
            return v;
        }
        match (from, to) {
            // Integer narrowing / widening: use uextend for widening, ireduce
            // for narrowing. Both are value-preserving in the v0.1 subset.
            (a, b) if a.is_int() && b.is_int() => {
                if to.bits() > from.bits() {
                    builder.ins().uextend(to, v)
                } else {
                    builder.ins().ireduce(to, v)
                }
            }
            // Float conversions.
            (types::F32, types::F64) => builder.ins().fpromote(types::F64, v),
            (types::F64, types::F32) => builder.ins().fdemote(types::F32, v),
            // Mixed int/float shouldn't happen in well-typed MIR; pass through.
            _ => v,
        }
    }

    fn def(
        &self,
        builder: &mut FunctionBuilder,
        local_id: &crate::symbol::LocalId,
        v: cranelift_codegen::ir::Value,
    ) {
        let ty = self.var_tys.get(&local_id.get()).copied();
        if let Some(&var) = self.vars.get(&local_id.get()) {
            let v = if let Some(t) = ty {
                self.cast_to(builder, v, t)
            } else {
                v
            };
            builder.def_var(var, v);
        }
        if let Some(&ss) = self.slots.get(&local_id.get()) {
            let slot_ty = self
                .slot_tys
                .get(&local_id.get())
                .copied()
                .unwrap_or(types::I64);
            let v = self.cast_to(builder, v, slot_ty);
            let addr = builder.ins().stack_addr(types::I64, ss, 0);
            builder.ins().store(MemFlags::trusted(), v, addr, 0);
        }
    }

    fn use_val(
        &self,
        builder: &mut FunctionBuilder,
        local_id: &crate::symbol::LocalId,
    ) -> cranelift_codegen::ir::Value {
        if let Some(&ss) = self.slots.get(&local_id.get()) {
            let slot_ty = self
                .slot_tys
                .get(&local_id.get())
                .copied()
                .unwrap_or(types::I64);
            let addr = builder.ins().stack_addr(types::I64, ss, 0);
            builder.ins().load(slot_ty, MemFlags::trusted(), addr, 0)
        } else {
            builder.use_var(self.vars[&local_id.get()])
        }
    }

    fn addr(
        &self,
        builder: &mut FunctionBuilder,
        local_id: &crate::symbol::LocalId,
    ) -> Option<cranelift_codegen::ir::Value> {
        self.slots
            .get(&local_id.get())
            .map(|&ss| builder.ins().stack_addr(types::I64, ss, 0))
    }

    fn declare_var(&mut self, local_id: &crate::symbol::LocalId, var: Variable, ty: types::Type) {
        self.vars.insert(local_id.get(), var);
        self.var_tys.insert(local_id.get(), ty);
    }

    fn declare_slot(
        &mut self,
        builder: &mut FunctionBuilder,
        local_id: &crate::symbol::LocalId,
        ty: types::Type,
    ) {
        let ss = builder.create_sized_stack_slot(StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            ty.bytes(),
            1,
        ));
        self.slots.insert(local_id.get(), ss);
        self.slot_tys.insert(local_id.get(), ty);
    }

    fn var(&self, local_id: &crate::symbol::LocalId) -> Option<&Variable> {
        self.vars.get(&local_id.get())
    }
}

pub fn compile_bodies(
    module: &mut cranelift_object::ObjectModule,
    bodies: &[Body],
    cx: &TyCtxt,
    defs: &Defs,
) -> Result<HashMap<String, FuncId>, String> {
    let mut func_ids = HashMap::new();
    let mut sigs: HashMap<String, Signature> = HashMap::new();
    // First pass: declare all user functions.
    for body in bodies {
        let sig = make_signature(body, cx);
        sigs.insert(body.name.clone(), sig.clone());
        let id = module
            .declare_function(&body.name, Linkage::Export, &sig)
            .map_err(|e| e.to_string())?;
        func_ids.insert(body.name.clone(), id);
    }
    // Declare runtime support functions (always available).
    for rt in &[
        "avera_putc",
        "avera_print_i64",
        "avera_print_f64",
        "avera_print_u64",
        "avera_print_addr",
        "avera_read_i64",
        "avera_alloc",
        "avera_free",
        "avera_exit",
    ] {
        let sig = runtime_sig(rt);
        sigs.insert(rt.to_string(), sig.clone());
        module
            .declare_function(rt, Linkage::Import, &sig)
            .map_err(|e| e.to_string())?;
    }

    // Second pass: compile each body.
    let mut builder_ctx = FunctionBuilderContext::new();
    for body in bodies {
        let id = func_ids[&body.name];
        let mut ctx = cranelift_codegen::Context::new();
        ctx.func.signature = make_signature(body, cx);
        compile_body(
            module,
            &mut ctx,
            body,
            &func_ids,
            &sigs,
            &mut builder_ctx,
            cx,
            defs,
        )?;
        module.define_function(id, &mut ctx).map_err(|e| {
            eprintln!(
                "=== Cranelift IR for {} ===\n{}",
                body.name,
                ctx.func.display()
            );
            e.to_string()
        })?;
    }
    Ok(func_ids)
}

fn runtime_sig(name: &str) -> Signature {
    let mut sig = Signature::new(CallConv::SystemV);
    match name {
        "avera_putc" => sig.params.push(AbiParam::new(types::I64)),
        "avera_print_i64" => sig.params.push(AbiParam::new(types::I64)),
        "avera_print_f64" => sig.params.push(AbiParam::new(types::F64)),
        "avera_print_u64" => sig.params.push(AbiParam::new(types::I64)),
        "avera_print_addr" => sig.params.push(AbiParam::new(types::I64)),
        "avera_read_i64" => sig.returns.push(AbiParam::new(types::I64)),
        "avera_alloc" => {
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
        }
        "avera_free" => sig.params.push(AbiParam::new(types::I64)),
        "avera_exit" => sig.params.push(AbiParam::new(types::I64)),
        _ => {}
    }
    sig
}

fn make_signature(body: &Body, cx: &TyCtxt) -> Signature {
    let mut sig = Signature::new(CallConv::SystemV);
    // v0.1: all integer/bool params are I64 in codegen.
    for &p in &body.params {
        let local = body.local(p);
        if matches!(cx.data(local.ty), TyData::F32 | TyData::F64) {
            sig.params.push(AbiParam::new(types::F64));
        } else {
            sig.params.push(AbiParam::new(types::I64));
        }
    }
    match cx.data(body.ret_ty) {
        TyData::Unit | TyData::Never => {}
        TyData::F32 | TyData::F64 => {
            sig.returns.push(AbiParam::new(types::F64));
        }
        _ => {
            sig.returns.push(AbiParam::new(types::I64));
        }
    }
    sig
}

fn compile_body(
    module: &mut cranelift_object::ObjectModule,
    ctx: &mut cranelift_codegen::Context,
    body: &Body,
    func_ids: &HashMap<String, FuncId>,
    sigs: &HashMap<String, Signature>,
    builder_ctx: &mut FunctionBuilderContext,
    cx: &TyCtxt,
    defs: &Defs,
) -> Result<(), String> {
    let mut builder = FunctionBuilder::new(&mut ctx.func, builder_ctx);

    // Map MIR LocalId -> Cranelift Variable.
    let mut local_vars = Locals::new();
    for (i, local) in body.locals.iter().enumerate() {
        let var = Variable::from_u32(i as u32);
        // R144/R145 Typed MIR: the MIR Body tracks each local's real type for
        // the semantic checkers (ownership, borrow, magnet, validate). For
        // CODEGEN, all integer locals use I64 and all float locals use F64 as
        // their storage representation. This avoids sign/zero-extension bugs
        // when widening narrow types to I64 for calls/returns/prints: a -5
        // stored as I32 then zero-extended to I64 would become 4294967291,
        // corrupting signed values. Using I64 uniformly for integers is the
        // safe v0.1 ABI — the type precision lives in MIR, not in the SSA var.
        let ty = match cx.data(local.ty) {
            TyData::F32 | TyData::F64 => types::F64,
            _ => types::I64,
        };
        builder.declare_var(var, ty);
        local_vars.declare_var(&local.id, var, ty);
        // Address-taken locals get a stack slot backing their storage. The
        // slot is ALWAYS 8 bytes (I64) because magnet/field/choice operations
        // store and load through the slot in 8-byte units.
        if body.address_taken.contains(&local.id) {
            local_vars.declare_slot(&mut builder, &local.id, types::I64);
        }
    }
    // Block map: index -> Cranelift block.
    let blocks: Vec<cranelift_codegen::ir::Block> =
        body.blocks.iter().map(|_| builder.create_block()).collect();
    // Entry block params bind to function params.
    builder.switch_to_block(blocks[0]);
    builder.seal_block(blocks[0]);
    builder.append_block_params_for_function_params(blocks[0]);
    if !body.params.is_empty() {
        let vals = builder.block_params(blocks[0]).to_vec();
        for (i, &p) in body.params.iter().enumerate() {
            if let Some(&var) = local_vars.var(&p) {
                if let Some(&v) = vals.get(i) {
                    // Params arrive at the signature type (I64/F64), which
                    // matches the var's declared codegen type (I64/F64). No
                    // cast needed.
                    builder.def_var(var, v);
                }
            }
        }
    }

    // Cache of foreign function FuncRefs.
    let mut ext_refs: HashMap<String, cranelift_codegen::ir::FuncRef> = HashMap::new();

    for (i, block) in body.blocks.iter().enumerate() {
        let blk = blocks[i];
        builder.switch_to_block(blk);
        // Don't seal blocks here — they may have back-edges we haven't
        // seen yet. We seal all blocks at the end.
        for stmt in &block.stmts {
            lower_stmt(
                &mut builder,
                body,
                stmt,
                &local_vars,
                cx,
                defs,
                module,
                &mut ext_refs,
                func_ids,
                sigs,
            );
        }
        lower_terminator(&mut builder, &block.term, &blocks, &local_vars);
    }
    // Seal all blocks at once — by now all predecessors are known. Block 0
    // was already sealed up front (so its entry params can be bound), so we
    // skip it here to avoid a double-seal panic.
    for (i, b) in blocks.iter().enumerate() {
        if i == 0 {
            continue;
        }
        builder.seal_block(*b);
    }
    builder.finalize();
    Ok(())
}

fn place_val(
    builder: &mut FunctionBuilder,
    _body: &Body,
    place: &Place,
    local_vars: &Locals,
    _cx: &TyCtxt,
    _defs: &Defs,
) -> cranelift_codegen::ir::Value {
    local_vars.use_val(builder, &place.local)
}

fn lower_stmt(
    builder: &mut FunctionBuilder,
    body: &Body,
    stmt: &Stmt,
    local_vars: &Locals,
    cx: &TyCtxt,
    defs: &Defs,
    module: &mut cranelift_object::ObjectModule,
    ext_refs: &mut HashMap<String, cranelift_codegen::ir::FuncRef>,
    func_ids: &HashMap<String, FuncId>,
    sigs: &HashMap<String, Signature>,
) {
    match stmt {
        Stmt::Assign { place, value } => {
            let v = lower_rvalue(
                builder, body, value, local_vars, cx, defs, module, ext_refs, func_ids, sigs,
            );
            local_vars.def(builder, &place.local, v);
        }
        Stmt::StorageLive(_) | Stmt::StorageDead(_) | Stmt::Drop(_) => {}
        Stmt::Call {
            dest, callee, args, ..
        } => {
            let v = lower_call(
                builder, callee, args, local_vars, body, cx, defs, module, ext_refs, func_ids, sigs,
            );
            local_vars.def(builder, &dest.local, v);
        }
        Stmt::Assert { cond, .. } => {
            let c = place_val(builder, body, cond, local_vars, cx, defs);
            builder
                .ins()
                .trapz(c, cranelift_codegen::ir::TrapCode::unwrap_user(1));
        }
    }
}

fn lower_rvalue(
    builder: &mut FunctionBuilder,
    body: &Body,
    rv: &Rvalue,
    local_vars: &Locals,
    cx: &TyCtxt,
    defs: &Defs,
    module: &mut cranelift_object::ObjectModule,
    ext_refs: &mut HashMap<String, cranelift_codegen::ir::FuncRef>,
    func_ids: &HashMap<String, FuncId>,
    sigs: &HashMap<String, Signature>,
) -> cranelift_codegen::ir::Value {
    match rv {
        Rvalue::Use(p) => place_val(builder, body, p, local_vars, cx, defs),
        Rvalue::Move { place, .. } => {
            // `^x` — move. Codegen is identical to Use (the value is copied);
            // the ownership checker enforces the move semantics.
            place_val(builder, body, place, local_vars, cx, defs)
        }
        Rvalue::Const(c) => const_val(builder, c),
        Rvalue::BinOp { op, lhs, rhs, ty } => {
            let l = place_val(builder, body, lhs, local_vars, cx, defs);
            let r = place_val(builder, body, rhs, local_vars, cx, defs);
            binop(builder, *op, l, r, *ty, cx)
        }
        Rvalue::UnOp { op, operand, ty } => {
            let v = place_val(builder, body, operand, local_vars, cx, defs);
            unop(builder, *op, v, *ty, cx)
        }
        Rvalue::Cast { operand, to, .. } => {
            let v = place_val(builder, body, operand, local_vars, cx, defs);
            let target = abi::scalar_type(*to, cx).unwrap_or(types::I64);
            builder.ins().uextend(target, v)
        }
        Rvalue::MagnetAttach { place, .. } => {
            // The magnet points at the REAL owner storage. If the owner is
            // address-taken (backed by a stack slot), take that slot's
            // address. Otherwise, allocate a fresh slot, store the value,
            // and use that (for expression targets).
            if let Some(addr) = local_vars.addr(builder, &place.local) {
                addr
            } else {
                let val = place_val(builder, body, place, local_vars, cx, defs);
                let ss = builder.create_sized_stack_slot(StackSlotData::new(
                    cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                    types::I64.bytes(),
                    1,
                ));
                let addr = builder.ins().stack_addr(types::I64, ss, 0);
                builder.ins().store(MemFlags::trusted(), val, addr, 0);
                addr
            }
        }
        Rvalue::MagnetAddress { magnet, .. } => {
            // The magnet local holds the address; load it.
            place_val(builder, body, magnet, local_vars, cx, defs)
        }
        Rvalue::MagnetOffset { magnet, .. } => {
            // ~m.offset — the byte offset within the pointed-to region.
            // v0.1 magnets are single-slot, so the offset is always 0.
            let _ = magnet;
            builder.ins().iconst(types::I64, 0)
        }
        Rvalue::MagnetNone { .. } => {
            // A detached magnet holds a null address.
            builder.ins().iconst(types::I64, 0)
        }
        Rvalue::MagnetFromAddr { addr, .. } => {
            // Raw magnet from an integer address (raw block only).
            place_val(builder, body, addr, local_vars, cx, defs)
        }
        Rvalue::MagnetAdd { magnet, delta, .. } => {
            // ~m += n — pointer arithmetic: add n bytes to the magnet address.
            let m = place_val(builder, body, magnet, local_vars, cx, defs);
            let d = place_val(builder, body, delta, local_vars, cx, defs);
            builder.ins().iadd(m, d)
        }
        Rvalue::Load { addr, offset, ty } => {
            let addr_val = place_val(builder, body, addr, local_vars, cx, defs);
            let mem_ty = abi::scalar_type(*ty, cx).unwrap_or(types::I64);
            let mem_flags = MemFlags::trusted();
            builder
                .ins()
                .load(mem_ty, mem_flags, addr_val, *offset as i32)
        }
        Rvalue::Store {
            addr,
            value,
            offset,
            ..
        } => {
            let addr_val = place_val(builder, body, addr, local_vars, cx, defs);
            let val = place_val(builder, body, value, local_vars, cx, defs);
            let mem_flags = MemFlags::trusted();
            builder
                .ins()
                .store(mem_flags, val, addr_val, *offset as i32);
            addr_val
        }
        Rvalue::Call { callee, args, .. } => lower_call(
            builder, callee, args, local_vars, body, cx, defs, module, ext_refs, func_ids, sigs,
        ),
        // The following Rvalue variants (Borrow, ChoiceCtor, ShapeCtor,
        // ArrayCtor, Try) are never emitted by the current lowering: the
        // lowering expands all of these into MirStmt::Call + Rvalue::Store/
        // Load sequences before codegen. If a future lowering path emits one
        // of these directly, we must NOT silently return 0 (that would hide
        // a real bug). Trap with a distinct code so the failure is loud.
        Rvalue::Borrow { .. }
        | Rvalue::ChoiceCtor { .. }
        | Rvalue::ShapeCtor { .. }
        | Rvalue::ArrayCtor { .. }
        | Rvalue::Try { .. } => {
            // Trap is a block terminator; the iconst after it is dead but
            // satisfies the match-arm type (Value). It will never execute.
            builder
                .ins()
                .trap(cranelift_codegen::ir::TrapCode::unwrap_user(101));
            builder.ins().iconst(types::I64, 0)
        }
    }
}

fn lower_call(
    builder: &mut FunctionBuilder,
    callee: &str,
    args: &[Place],
    local_vars: &Locals,
    body: &Body,
    cx: &TyCtxt,
    defs: &Defs,
    module: &mut cranelift_object::ObjectModule,
    ext_refs: &mut HashMap<String, cranelift_codegen::ir::FuncRef>,
    func_ids: &HashMap<String, FuncId>,
    sigs: &HashMap<String, Signature>,
) -> cranelift_codegen::ir::Value {
    let arg_vals: Vec<cranelift_codegen::ir::Value> = args
        .iter()
        .map(|p| {
            let v = place_val(builder, body, p, local_vars, cx, defs);
            // Typed-MIR: locals may be narrower than I64 (e.g. I32), but all
            // runtime/external functions take I64 args per the v0.1 ABI. Widen
            // to I64 so the call signature matches. (avera_print_i64 etc. all
            // expect I64; widening a small int to I64 is lossless.)
            let from = builder.func.dfg.value_type(v);
            if from != types::I64 && from.is_int() {
                builder.ins().uextend(types::I64, v)
            } else {
                v
            }
        })
        .collect();
    // Internal user function.
    if let Some(&id) = func_ids.get(callee) {
        let fref = module.declare_func_in_func(id, builder.func);
        let call = builder.ins().call(fref, &arg_vals);
        if builder.inst_results(call).is_empty() {
            return builder.ins().iconst(types::I64, 0);
        }
        return builder.inst_results(call)[0];
    }
    // External function.
    let fref = if let Some(&fr) = ext_refs.get(callee) {
        fr
    } else {
        // Build a signature based on the call: each arg is I64, return I64.
        let sig = sigs.get(callee).cloned().unwrap_or_else(|| {
            let mut s = Signature::new(CallConv::SystemV);
            for _ in 0..args.len() {
                s.params.push(AbiParam::new(types::I64));
            }
            s.returns.push(AbiParam::new(types::I64));
            s
        });
        let id = match module.declare_function(callee, Linkage::Import, &sig) {
            Ok(id) => id,
            Err(e) => {
                // Genuine codegen failure: the external function could not be
                // declared (bad name/sig). Trap loudly rather than returning a
                // bogus 0 that would silently propagate.
                eprintln!("cannot declare external function `{}`: {}", callee, e);
                builder
                    .ins()
                    .trap(cranelift_codegen::ir::TrapCode::unwrap_user(103));
                return builder.ins().iconst(types::I64, 0);
            }
        };
        let fref = module.declare_func_in_func(id, builder.func);
        ext_refs.insert(callee.to_string(), fref);
        fref
    };
    let call = builder.ins().call(fref, &arg_vals);
    if builder.inst_results(call).is_empty() {
        builder.ins().iconst(types::I64, 0)
    } else {
        builder.inst_results(call)[0]
    }
}

fn const_val(builder: &mut FunctionBuilder, c: &Const) -> cranelift_codegen::ir::Value {
    match c {
        Const::Int(i) => builder.ins().iconst(types::I64, *i),
        Const::U64(u) => builder.ins().iconst(types::I64, *u as i64),
        Const::Bool(b) => builder.ins().iconst(types::I64, *b as i64),
        Const::Float(f) => builder.ins().f64const(*f),
        Const::Char(ch) => builder.ins().iconst(types::I64, *ch as i64),
        Const::Unit => builder.ins().iconst(types::I64, 0),
        // Const::Str/Addr are never produced by the current lowering (string
        // literals are built via avera_text_new calls). Trap loudly if reached
        // rather than silently returning 0.
        Const::Str(_) | Const::Addr(_) => {
            builder
                .ins()
                .trap(cranelift_codegen::ir::TrapCode::unwrap_user(102));
            builder.ins().iconst(types::I64, 0)
        }
    }
}

fn bool_to_i8(
    builder: &mut FunctionBuilder,
    c: cranelift_codegen::ir::Value,
) -> cranelift_codegen::ir::Value {
    // Convert a B1 comparison result to I64 (v0.1 uses I64 for all ints).
    let zero = builder.ins().iconst(types::I64, 0);
    let one = builder.ins().iconst(types::I64, 1);
    builder.ins().select(c, one, zero)
}

fn binop(
    builder: &mut FunctionBuilder,
    op: BinOp,
    l: cranelift_codegen::ir::Value,
    r: cranelift_codegen::ir::Value,
    ty: crate::types::ty::TyId,
    cx: &TyCtxt,
) -> cranelift_codegen::ir::Value {
    use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
    let is_float = matches!(cx.data(ty), TyData::F32 | TyData::F64);
    // All integer codegen locals are I64 (see compile_body). Normalize both
    // operands to the operation's target type. For integers, the target is
    // ALWAYS I64 (not the MIR type's scalar) so that signed values aren't
    // corrupted by narrowing-then-zero-extending. For floats, use the real
    // scalar type (F32/F64).
    let target_ty = if is_float {
        abi::scalar_type(ty, cx).unwrap_or(types::F64)
    } else {
        types::I64
    };
    let lt = builder.func.dfg.value_type(l);
    let rt = builder.func.dfg.value_type(r);
    let l = if lt != target_ty && lt.is_int() && target_ty.is_int() {
        if target_ty.bits() > lt.bits() {
            builder.ins().uextend(target_ty, l)
        } else {
            builder.ins().ireduce(target_ty, l)
        }
    } else if lt != target_ty && lt.is_float() && target_ty.is_float() {
        if target_ty == types::F64 {
            builder.ins().fpromote(types::F64, l)
        } else {
            builder.ins().fdemote(types::F32, l)
        }
    } else {
        l
    };
    let r = if rt != target_ty && rt.is_int() && target_ty.is_int() {
        if target_ty.bits() > rt.bits() {
            builder.ins().uextend(target_ty, r)
        } else {
            builder.ins().ireduce(target_ty, r)
        }
    } else if rt != target_ty && rt.is_float() && target_ty.is_float() {
        if target_ty == types::F64 {
            builder.ins().fpromote(types::F64, r)
        } else {
            builder.ins().fdemote(types::F32, r)
        }
    } else {
        r
    };
    // Compute the comparison first to avoid double-borrowing builder.
    let cmp = match op {
        BinOp::Eq => {
            if is_float {
                builder.ins().fcmp(FloatCC::Equal, l, r)
            } else {
                builder.ins().icmp(IntCC::Equal, l, r)
            }
        }
        BinOp::Ne => {
            if is_float {
                builder.ins().fcmp(FloatCC::NotEqual, l, r)
            } else {
                builder.ins().icmp(IntCC::NotEqual, l, r)
            }
        }
        BinOp::Lt => {
            if is_float {
                builder.ins().fcmp(FloatCC::LessThan, l, r)
            } else if cx.data(ty).is_signed() {
                builder.ins().icmp(IntCC::SignedLessThan, l, r)
            } else {
                builder.ins().icmp(IntCC::UnsignedLessThan, l, r)
            }
        }
        BinOp::Le => {
            if is_float {
                builder.ins().fcmp(FloatCC::LessThanOrEqual, l, r)
            } else if cx.data(ty).is_signed() {
                builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r)
            } else {
                builder.ins().icmp(IntCC::UnsignedLessThanOrEqual, l, r)
            }
        }
        BinOp::Gt => {
            if is_float {
                builder.ins().fcmp(FloatCC::GreaterThan, l, r)
            } else if cx.data(ty).is_signed() {
                builder.ins().icmp(IntCC::SignedGreaterThan, l, r)
            } else {
                builder.ins().icmp(IntCC::UnsignedGreaterThan, l, r)
            }
        }
        BinOp::Ge => {
            if is_float {
                builder.ins().fcmp(FloatCC::GreaterThanOrEqual, l, r)
            } else if cx.data(ty).is_signed() {
                builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, l, r)
            } else {
                builder.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, l, r)
            }
        }
        BinOp::Add => {
            return if is_float {
                builder.ins().fadd(l, r)
            } else {
                builder.ins().iadd(l, r)
            };
        }
        BinOp::Sub => {
            return if is_float {
                builder.ins().fsub(l, r)
            } else {
                builder.ins().isub(l, r)
            };
        }
        BinOp::Mul => {
            return if is_float {
                builder.ins().fmul(l, r)
            } else {
                builder.ins().imul(l, r)
            };
        }
        BinOp::Div => {
            return if is_float {
                builder.ins().fdiv(l, r)
            } else {
                builder.ins().udiv(l, r)
            };
        }
        BinOp::Mod => {
            return builder.ins().urem(l, r);
        }
        BinOp::BitAnd => {
            return builder.ins().band(l, r);
        }
        BinOp::BitOr => {
            return builder.ins().bor(l, r);
        }
        BinOp::BitXor => {
            return builder.ins().bxor(l, r);
        }
        BinOp::Shl => {
            return builder.ins().ishl(l, r);
        }
        BinOp::Shr => {
            return if cx.data(ty).is_signed() {
                builder.ins().sshr(l, r)
            } else {
                builder.ins().ushr(l, r)
            };
        }
    };
    // cmp is a B1 result; convert to I8.
    bool_to_i8(builder, cmp)
}

fn unop(
    builder: &mut FunctionBuilder,
    op: UnOp,
    v: cranelift_codegen::ir::Value,
    _ty: crate::types::ty::TyId,
    _cx: &TyCtxt,
) -> cranelift_codegen::ir::Value {
    match op {
        UnOp::Not => {
            // Logical NOT: produce 1 if v==0, 0 otherwise. (bnot would flip
            // all bits, so `!false` = !0 = 0xFFFF...F, which is truthy but not
            // == 1 — the SwitchInt on bool would fail.) Use icmp_eq to 0.
            let zero = builder.ins().iconst(types::I64, 0);
            let is_zero =
                builder
                    .ins()
                    .icmp(cranelift_codegen::ir::condcodes::IntCC::Equal, v, zero);
            bool_to_i8(builder, is_zero)
        }
        UnOp::Neg => builder.ins().ineg(v),
    }
}

fn lower_terminator(
    builder: &mut FunctionBuilder,
    term: &Terminator,
    blocks: &[cranelift_codegen::ir::Block],
    local_vars: &Locals,
) {
    match term {
        Terminator::Goto(b) => {
            let idx = b.get() as usize;
            if idx < blocks.len() {
                builder.ins().jump(blocks[idx], &[]);
            }
        }
        Terminator::Return { value } => {
            if let Some(p) = value {
                let v = local_vars.use_val(builder, &p.local);
                // The function signature returns I64 for integers (see
                // make_signature). Typed-MIR locals may be narrower (I32/I8);
                // widen to I64 so the return value matches the signature.
                let from = builder.func.dfg.value_type(v);
                let v = if from.is_int() && from != types::I64 {
                    builder.ins().uextend(types::I64, v)
                } else if from.is_float() && from != types::F64 {
                    builder.ins().fpromote(types::F64, v)
                } else {
                    v
                };
                builder.ins().return_(&[v]);
            } else {
                builder.ins().return_(&[]);
            }
        }
        Terminator::Abort => {
            builder
                .ins()
                .trap(cranelift_codegen::ir::TrapCode::unwrap_user(0));
        }
        Terminator::Unreachable => {
            builder
                .ins()
                .trap(cranelift_codegen::ir::TrapCode::unwrap_user(4));
        }
        Terminator::SwitchInt {
            discr,
            targets,
            otherwise,
        } => {
            let v = local_vars.use_val(builder, &discr.local);
            // The discriminant may be narrower than I64 (typed MIR); widen for
            // comparison against the I64 constants below.
            let from = builder.func.dfg.value_type(v);
            let v = if from.is_int() && from != types::I64 {
                builder.ins().uextend(types::I64, v)
            } else {
                v
            };
            for (val, target) in targets {
                let cmp = builder.ins().iconst(types::I64, *val as i64);
                let c = builder
                    .ins()
                    .icmp(cranelift_codegen::ir::condcodes::IntCC::Equal, v, cmp);
                let next = builder.create_block();
                builder
                    .ins()
                    .brif(c, blocks[target.get() as usize], &[], next, &[]);
                builder.switch_to_block(next);
                builder.seal_block(next);
            }
            builder.ins().jump(blocks[otherwise.get() as usize], &[]);
        }
        Terminator::Switch {
            discr,
            targets,
            otherwise,
        } => {
            let v = local_vars.use_val(builder, &discr.local);
            // The discriminant may be narrower than I64 (typed MIR); widen for
            // comparison against the I64 constants below.
            let from = builder.func.dfg.value_type(v);
            let v = if from.is_int() && from != types::I64 {
                builder.ins().uextend(types::I64, v)
            } else {
                v
            };
            for (val, target) in targets {
                let cmp = builder.ins().iconst(types::I64, *val as i64);
                let c = builder
                    .ins()
                    .icmp(cranelift_codegen::ir::condcodes::IntCC::Equal, v, cmp);
                let next = builder.create_block();
                builder
                    .ins()
                    .brif(c, blocks[target.get() as usize], &[], next, &[]);
                builder.switch_to_block(next);
                builder.seal_block(next);
            }
            if let Some(o) = otherwise {
                builder.ins().jump(blocks[o.get() as usize], &[]);
            } else {
                builder
                    .ins()
                    .trap(cranelift_codegen::ir::TrapCode::unwrap_user(5));
            }
        }
    }
}
