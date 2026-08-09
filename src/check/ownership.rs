use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use crate::diagnostics::span::Span;
use crate::mir::body::{Body, Stmt};
use crate::mir::place::{Place, PlaceElem};
use crate::mir::rvalue::Rvalue;
use crate::mir::terminator::Terminator;
use crate::symbol::LocalId;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum State {
    Uninit,
    Init,
    Moved,
    Dropped,
}

impl State {
    fn meet(self, other: State) -> State {
        use State::*;
        match (self, other) {
            (Dropped, _) | (_, Dropped) => Dropped,
            (Moved, _) | (_, Moved) => Moved,
            (Uninit, _) | (_, Uninit) => Uninit,
            (Init, Init) => Init,
        }
    }
}

type StateMap = HashMap<LocalId, State>;

// kiem tra move / drop state cua bien
// NOTE: khong cho dung bien sau khi da move
pub fn check_ownership(body: &Body, diags: &mut DiagnosticList) {
    let mut analysis = OwnershipAnalysis::new(body, diags);
    analysis.run();
}

struct OwnershipAnalysis<'a> {
    body: &'a Body,
    diags: &'a mut DiagnosticList,
    in_states: Vec<StateMap>,
    in_worklist: Vec<bool>,
}

impl<'a> OwnershipAnalysis<'a> {
    fn new(body: &'a Body, diags: &'a mut DiagnosticList) -> Self {
        let nblocks = body.blocks.len();
        Self {
            body,
            diags,
            in_states: vec![HashMap::new(); nblocks],
            in_worklist: vec![false; nblocks],
        }
    }

    fn run(&mut self) {
        if self.body.blocks.is_empty() {
            return;
        }

        // Initialize entry block in-state: params are Init, others are Uninit.
        for &p in &self.body.params {
            self.in_states[0].insert(p, State::Init);
        }
        for local in &self.body.locals {
            self.in_states[0].entry(local.id).or_insert(State::Uninit);
        }

        self.in_worklist[0] = true;
        let mut worklist: Vec<usize> = vec![0];
        while let Some(idx) = worklist.pop() {
            self.in_worklist[idx] = false;
            let in_state = self.in_states[idx].clone();
            let out_state = self.transfer_block(idx, &in_state);
            // Propagate to successors.
            for succ in successors(&self.body.blocks[idx].term) {
                if succ >= self.body.blocks.len() {
                    continue;
                }
                let old_succ_in = self.in_states[succ].clone();
                let new_succ_in = meet_states(&old_succ_in, &out_state);
                if new_succ_in != old_succ_in {
                    self.in_states[succ] = new_succ_in;
                    if !self.in_worklist[succ] {
                        self.in_worklist[succ] = true;
                        worklist.push(succ);
                    }
                }
            }
        }
    }

    fn transfer_block(&mut self, idx: usize, in_state: &StateMap) -> StateMap {
        let mut state = in_state.clone();
        let block = &self.body.blocks[idx];
        for s in &block.stmts {
            self.transfer_stmt(s, &mut state);
        }
        state
    }

    fn transfer_stmt(&mut self, s: &Stmt, state: &mut StateMap) {
        match s {
            Stmt::Assign { place, value } => {
                // Check/use the rvalue's operands.
                self.use_rvalue(value, state);
                // Write the destination: it becomes Init.
                self.write_local(place.local, state, State::Init);
            }
            Stmt::StorageLive(id) => {
                state.insert(*id, State::Uninit);
            }
            Stmt::StorageDead(id) => {
                // Storage dead is like an implicit drop.
                let cur = state.get(id).copied().unwrap_or(State::Uninit);
                if cur == State::Init {
                    state.insert(*id, State::Dropped);
                }
            }
            Stmt::Drop(place) => {
                let cur = state.get(&place.local).copied().unwrap_or(State::Uninit);
                match cur {
                    State::Init => {
                        state.insert(place.local, State::Dropped);
                    }
                    State::Dropped => {
                        self.error(
                            "double drop: this value was already dropped",
                            DiagnosticKind::EDoubleDrop,
                        );
                    }
                    State::Moved => {
                        self.error(
                            "cannot drop: value was already moved",
                            DiagnosticKind::EUseAfterMove,
                        );
                    }
                    State::Uninit => {
                        self.error(
                            "cannot drop: value is not initialized",
                            DiagnosticKind::EUseAfterDrop,
                        );
                    }
                }
            }
            Stmt::Call { dest, args, .. } => {
                // Use each argument.
                for a in args {
                    self.use_place(a, state);
                }
                // The dest becomes Init.
                self.write_local(dest.local, state, State::Init);
            }
            Stmt::Assert { cond, .. } => {
                self.use_place(cond, state);
            }
        }
    }

    fn use_rvalue(&mut self, rv: &Rvalue, state: &mut StateMap) {
        match rv {
            Rvalue::Use(p) => self.use_place(p, state),
            Rvalue::Const(_) => {}
            Rvalue::BinOp { lhs, rhs, .. } => {
                self.use_place(lhs, state);
                self.use_place(rhs, state);
            }
            Rvalue::UnOp { operand, .. } => self.use_place(operand, state),
            Rvalue::Borrow { place, .. } => {
                // Borrowing reads the place (must be Init).
                self.use_place(place, state);
            }
            Rvalue::Move { place, .. } => {
                // `^x` — move: the source must be Init, and becomes Moved.
                self.use_place_move(place, state);
            }
            Rvalue::MagnetAttach { place, .. } => {
                self.use_place(place, state);
            }
            Rvalue::MagnetNone { .. } => {}
            Rvalue::MagnetFromAddr { addr, .. } => self.use_place(addr, state),
            Rvalue::MagnetAddress { magnet, .. } => self.use_place(magnet, state),
            Rvalue::MagnetOffset { magnet, .. } => self.use_place(magnet, state),
            Rvalue::MagnetAdd { magnet, delta, .. } => {
                self.use_place(magnet, state);
                self.use_place(delta, state);
            }
            Rvalue::Cast { operand, .. } => self.use_place(operand, state),
            Rvalue::ChoiceCtor { args, .. } => {
                for a in args {
                    self.use_place(a, state);
                }
            }
            Rvalue::ShapeCtor { fields, .. } => {
                for f in fields {
                    self.use_place(f, state);
                }
            }
            Rvalue::ArrayCtor { elems, .. } => {
                for e in elems {
                    self.use_place(e, state);
                }
            }
            Rvalue::Call { args, .. } => {
                for a in args {
                    self.use_place(a, state);
                }
            }
            Rvalue::Try { operand, .. } => self.use_place(operand, state),
            Rvalue::Load { addr, .. } => self.use_place(addr, state),
            Rvalue::Store { addr, value, .. } => {
                self.use_place(addr, state);
                self.use_place(value, state);
            }
        }
    }

    fn use_place(&mut self, p: &Place, state: &mut StateMap) {
        let cur = state.get(&p.local).copied().unwrap_or(State::Uninit);
        match cur {
            State::Init => {
                // Copy types: using doesn't change state.
                // Move types: would mark as Moved, but we don't track
                // copy-ness yet (v0.1 treats all as copy for dataflow).
            }
            State::Moved => {
                self.error(
                    "use after move: this value was moved and is no longer available",
                    DiagnosticKind::EUseAfterMove,
                );
            }
            State::Dropped => {
                self.error(
                    "use after drop: this value was dropped and is no longer available",
                    DiagnosticKind::EUseAfterDrop,
                );
            }
            State::Uninit => {
                self.error(
                    &format!("use of uninitialized value (local {})", p.local.get()),
                    DiagnosticKind::EUseAfterDrop,
                );
            }
        }
        // Check index places too.
        for e in &p.elems {
            if let PlaceElem::Index(id) = e {
                self.use_place(&Place::local(*id), state);
            }
        }
    }

    fn use_place_move(&mut self, p: &Place, state: &mut StateMap) {
        let cur = state.get(&p.local).copied().unwrap_or(State::Uninit);
        match cur {
            State::Init => {
                state.insert(p.local, State::Moved);
            }
            State::Moved => {
                self.error(
                    "use after move: this value was moved and is no longer available",
                    DiagnosticKind::EUseAfterMove,
                );
            }
            State::Dropped => {
                self.error(
                    "use after drop: this value was dropped and is no longer available",
                    DiagnosticKind::EUseAfterDrop,
                );
            }
            State::Uninit => {
                self.error(
                    &format!("use of uninitialized value (local {})", p.local.get()),
                    DiagnosticKind::EUseAfterDrop,
                );
            }
        }
        for e in &p.elems {
            if let PlaceElem::Index(id) = e {
                self.use_place(&Place::local(*id), state);
            }
        }
    }

    fn write_local(&mut self, id: LocalId, state: &mut StateMap, new: State) {
        state.insert(id, new);
    }

    fn error(&mut self, msg: &str, kind: DiagnosticKind) {
        let span = Span::new(Default::default(), 0, 0);
        self.diags
            .push(Diagnostic::new(kind, span, msg.to_string()));
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

fn meet_states(a: &StateMap, b: &StateMap) -> StateMap {
    let mut result = a.clone();
    for (k, &v) in b {
        let entry = result.entry(*k).or_insert(v);
        *entry = entry.meet(v);
    }
    result
}
