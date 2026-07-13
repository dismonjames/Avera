use crate::mir::body::{Body, Stmt};
use crate::mir::rvalue::Rvalue;
use crate::mir::terminator::Terminator;
use crate::symbol::LocalId;

fn is_heap_alloc_callee(callee: &str) -> bool {
    matches!(
        callee,
        "avera_alloc"
            | "avera_alloc_array"
            | "avera_text_new"
            | "avera_text_read_line"
            | "avera_text_concat"
    )
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum DropState {
    Uninit,
    Init,
    Moved,
    Dropped,
}

impl DropState {
    fn meet(self, other: DropState) -> DropState {
        use DropState::*;
        match (self, other) {
            (Dropped, _) | (_, Dropped) => Dropped,
            (Moved, _) | (_, Moved) => Moved,
            (Uninit, _) | (_, Uninit) => Uninit,
            (Init, Init) => Init,
        }
    }
}

fn successors(t: &Terminator) -> Vec<usize> {
    match t {
        Terminator::Goto(b) => vec![b.get() as usize],
        Terminator::SwitchInt {
            targets, otherwise, ..
        } => {
            let mut v: Vec<usize> = targets.iter().map(|(_, t)| t.get() as usize).collect();
            v.push(otherwise.get() as usize);
            v
        }
        Terminator::Switch {
            targets, otherwise, ..
        } => {
            let mut v: Vec<usize> = targets.iter().map(|(_, t)| t.get() as usize).collect();
            if let Some(o) = otherwise {
                v.push(o.get() as usize);
            }
            v
        }
        Terminator::Return { .. } | Terminator::Abort | Terminator::Unreachable => vec![],
    }
}

pub fn elaborate_drops(body: &mut Body) {
    // ---- 1. Identify heap-owning locals. ----
    let mut owns_heap: std::collections::HashSet<LocalId> = std::collections::HashSet::new();
    for block in &body.blocks {
        for s in &block.stmts {
            if let Stmt::Call { dest, callee, .. } = s {
                if is_heap_alloc_callee(callee) {
                    owns_heap.insert(dest.local);
                }
            }
            // A choice/shape constructor stores into `dest` (which holds a
            // heap pointer from avera_alloc) — mark the dest as owning heap.
            if let Stmt::Assign { place, value } = s {
                if let Rvalue::ChoiceCtor { .. }
                | Rvalue::ShapeCtor { .. }
                | Rvalue::ArrayCtor { .. } = value
                {
                    owns_heap.insert(place.local);
                }
            }
        }
    }
    // Merge with any heap-ownership info the lowering already recorded.
    for id in &body.owns_heap {
        owns_heap.insert(*id);
    }
    body.owns_heap = owns_heap.clone();

    if body.locals.is_empty() {
        return;
    }
    let all_locals: Vec<LocalId> = body.locals.iter().map(|l| l.id).collect();
    let nblocks = body.blocks.len();

    // ---- 2. Forward dataflow for ownership state (to decide frees). ----
    use std::collections::HashMap;
    type StateMap = HashMap<LocalId, DropState>;
    let mut in_states: Vec<StateMap> = vec![HashMap::new(); nblocks];
    let mut in_worklist: Vec<bool> = vec![true; nblocks];

    // Entry: params are Init (caller-owned, but we still track state), others Uninit.
    for &p in &body.params {
        in_states[0].insert(p, DropState::Init);
    }
    for local in &body.locals {
        in_states[0].entry(local.id).or_insert(DropState::Uninit);
    }

    // Predecessor map.
    let mut preds: Vec<std::collections::HashSet<usize>> =
        vec![std::collections::HashSet::new(); nblocks];
    for (i, b) in body.blocks.iter().enumerate() {
        for succ in successors(&b.term) {
            if succ < nblocks {
                preds[succ].insert(i);
            }
        }
    }

    let mut worklist: Vec<usize> = vec![0];
    while let Some(idx) = worklist.pop() {
        in_worklist[idx] = false;
        let in_state = in_states[idx].clone();
        let out_state = transfer_block(body, idx, &in_state);
        for succ in successors(&body.blocks[idx].term) {
            if succ >= nblocks {
                continue;
            }
            let old = in_states[succ].clone();
            let mut new = old.clone();
            for (k, &v) in &out_state {
                let e = new.entry(*k).or_insert(v);
                *e = e.meet(v);
            }
            if new != old {
                in_states[succ] = new;
                if !in_worklist[succ] {
                    in_worklist[succ] = true;
                    worklist.push(succ);
                }
            }
        }
    }

    // ---- 3. Insert avera_free + StorageDead at Return blocks. ----
    // For each Return block, for each heap-owning local that is still Init in
    // the block's OUT-state (i.e., it owns a heap allocation and was not moved
    // or explicitly dropped), emit a avera_free call. Then emit StorageDead for
    // all locals in reverse declaration order.
    //
    // Precompute out-states BEFORE mutating blocks (transfer_block borrows
    // body immutably, so we can't call it while iterating blocks mutably).
    let out_states: Vec<std::collections::HashMap<LocalId, DropState>> = body
        .blocks
        .iter()
        .enumerate()
        .map(|(i, _)| transfer_block(body, i, &in_states[i]))
        .collect();
    let unit_ty = body.ret_ty; // ret_ty is only metadata for avera_free's void return
    for (idx, block) in body.blocks.iter_mut().enumerate() {
        if matches!(block.term, Terminator::Return { .. }) {
            let out_state = &out_states[idx];
            // Free heap-owning locals that are still Init. Skip params (caller owns them —
            // freeing a param would double-free when the caller also drops it).
            for id in all_locals.iter().rev() {
                if body.params.contains(id) {
                    // Params are caller-owned: do not free here. Still emit StorageDead
                    // for the ownership state machine.
                    block.stmts.push(Stmt::StorageDead(*id));
                    continue;
                }
                if owns_heap.contains(id) {
                    let st = out_state.get(id).copied().unwrap_or(DropState::Uninit);
                    if st == DropState::Init {
                        // Emit avera_free(local) to release the heap allocation.
                        block.stmts.push(Stmt::Call {
                            dest: Place::local(*id), // unused void dest; reuse the local
                            callee: "avera_free".to_string(),
                            args: vec![Place::local(*id)],
                            ret_ty: unit_ty,
                        });
                    }
                }
                block.stmts.push(Stmt::StorageDead(*id));
            }
        }
    }
}

fn transfer_block(
    body: &Body,
    idx: usize,
    in_state: &std::collections::HashMap<LocalId, DropState>,
) -> std::collections::HashMap<LocalId, DropState> {
    use std::collections::HashMap;
    let mut state: HashMap<LocalId, DropState> = in_state.clone();
    for s in &body.blocks[idx].stmts {
        match s {
            Stmt::Assign { place, value } => {
                // Use rvalue operands (a Move marks the source Moved).
                use_rvalue(value, &mut state);
                state.insert(place.local, DropState::Init);
            }
            Stmt::StorageLive(id) => {
                state.insert(*id, DropState::Uninit);
            }
            Stmt::StorageDead(id) => {
                let cur = state.get(id).copied().unwrap_or(DropState::Uninit);
                if cur == DropState::Init {
                    state.insert(*id, DropState::Dropped);
                }
            }
            Stmt::Drop(place) => {
                let cur = state
                    .get(&place.local)
                    .copied()
                    .unwrap_or(DropState::Uninit);
                if cur == DropState::Init {
                    state.insert(place.local, DropState::Dropped);
                }
            }
            Stmt::Call { dest, args, .. } => {
                for a in args {
                    use_place(a, &mut state);
                }
                state.insert(dest.local, DropState::Init);
            }
            Stmt::Assert { cond, .. } => {
                use_place(cond, &mut state);
            }
        }
    }
    state
}

fn use_rvalue(rv: &Rvalue, state: &mut std::collections::HashMap<LocalId, DropState>) {
    match rv {
        Rvalue::Move { place, .. } => {
            // A move marks the source as Moved — the new owner will free it.
            let cur = state
                .get(&place.local)
                .copied()
                .unwrap_or(DropState::Uninit);
            if cur == DropState::Init {
                state.insert(place.local, DropState::Moved);
            }
        }
        Rvalue::Use(p) => {
            let _ = p;
        }
        Rvalue::Const(_) => {}
        Rvalue::BinOp { lhs, rhs, .. } => {
            let _ = (lhs, rhs);
        }
        Rvalue::UnOp { operand, .. } => {
            let _ = operand;
        }
        Rvalue::Borrow { place, .. } => {
            let _ = place;
        }
        Rvalue::MagnetAttach { place, .. } => {
            let _ = place;
        }
        Rvalue::MagnetNone { .. } => {}
        Rvalue::MagnetFromAddr { addr, .. } => {
            let _ = addr;
        }
        Rvalue::MagnetAddress { magnet, .. } => {
            let _ = magnet;
        }
        Rvalue::MagnetOffset { magnet, .. } => {
            let _ = magnet;
        }
        Rvalue::MagnetAdd { magnet, delta, .. } => {
            let _ = (magnet, delta);
        }
        Rvalue::Cast { operand, .. } => {
            let _ = operand;
        }
        Rvalue::ChoiceCtor { args, .. } => {
            let _ = args;
        }
        Rvalue::ShapeCtor { fields, .. } => {
            let _ = fields;
        }
        Rvalue::ArrayCtor { elems, .. } => {
            let _ = elems;
        }
        Rvalue::Call { args, .. } => {
            let _ = args;
        }
        Rvalue::Try { operand, .. } => {
            let _ = operand;
        }
        Rvalue::Load { addr, .. } => {
            let _ = addr;
        }
        Rvalue::Store { addr, value, .. } => {
            let _ = (addr, value);
        }
    }
}

fn use_place(
    _p: &crate::mir::place::Place,
    _state: &mut std::collections::HashMap<LocalId, DropState>,
) {
    // Plain uses don't change ownership state for freeing purposes.
}

// Re-export Place for the Call dest construction above.
use crate::mir::place::Place;
