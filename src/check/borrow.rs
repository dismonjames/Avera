use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use crate::diagnostics::span::Span;
use crate::mir::body::{Body, Stmt};
use crate::mir::rvalue::Rvalue;
use crate::mir::terminator::Terminator;
use crate::symbol::LocalId;
use std::collections::{HashMap, HashSet};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct BorrowOrigin {
    owner: LocalId,
    is_mut: bool,
}

// holder local -> every borrow origin that may reach this program point.
// A set is required because a holder can refer to different owners on
// different CFG paths before those paths join.
type BorrowMap = HashMap<LocalId, HashSet<BorrowOrigin>>;

pub fn check_borrow(body: &Body, diags: &mut DiagnosticList) {
    let mut checker = BorrowChecker { body, diags };
    checker.run();
}

struct BorrowChecker<'a> {
    body: &'a Body,
    diags: &'a mut DiagnosticList,
}

impl<'a> BorrowChecker<'a> {
    fn run(&mut self) {
        let nblocks = self.body.blocks.len();
        if nblocks == 0 {
            return;
        }

        let mut in_states: Vec<BorrowMap> = vec![HashMap::new(); nblocks];
        let mut reached = vec![false; nblocks];
        let mut in_worklist = vec![false; nblocks];
        let mut worklist = vec![0usize];
        reached[0] = true;
        in_worklist[0] = true;

        while let Some(idx) = worklist.pop() {
            in_worklist[idx] = false;

            let mut out_state = in_states[idx].clone();
            for stmt in &self.body.blocks[idx].stmts {
                self.transfer_stmt(stmt, &mut out_state);
            }

            for succ in successors(&self.body.blocks[idx].term) {
                if succ >= nblocks {
                    continue;
                }

                let first_visit = !reached[succ];
                let new_state = if first_visit {
                    out_state.clone()
                } else {
                    meet_borrows(&in_states[succ], &out_state)
                };

                if first_visit || new_state != in_states[succ] {
                    in_states[succ] = new_state;
                    reached[succ] = true;
                    if !in_worklist[succ] {
                        in_worklist[succ] = true;
                        worklist.push(succ);
                    }
                }
            }
        }
    }

    fn transfer_stmt(&mut self, stmt: &Stmt, state: &mut BorrowMap) {
        match stmt {
            Stmt::Assign { place, value } => {
                // Reassigning a holder ends the borrow represented by its old value.
                state.remove(&place.local);

                // The destination may also be an owner borrowed by another local.
                if has_any_borrow(state, place.local) {
                    self.error(
                        "cannot assign to a place while it is borrowed",
                        DiagnosticKind::EImmutableMutate,
                    );
                }

                self.handle_rvalue(value, place.local, state);
            }
            Stmt::StorageLive(id) => {
                // A fresh storage lifetime cannot keep an old borrow origin.
                state.remove(id);
            }
            Stmt::StorageDead(id) => {
                if has_any_borrow(state, *id) {
                    self.error(
                        "cannot end storage for a place that is currently borrowed",
                        DiagnosticKind::EMoveWhileBorrowed,
                    );
                }
                state.remove(id);
            }
            Stmt::Drop(place) => {
                if has_any_borrow(state, place.local) {
                    self.error(
                        "cannot drop a place that is currently borrowed",
                        DiagnosticKind::EMoveWhileBorrowed,
                    );
                }
            }
            Stmt::Call { dest, .. } => {
                state.remove(&dest.local);
                if has_any_borrow(state, dest.local) {
                    self.error(
                        "cannot overwrite a place while it is borrowed",
                        DiagnosticKind::EImmutableMutate,
                    );
                }
            }
            Stmt::Assert { .. } => {}
        }
    }

    fn handle_rvalue(&mut self, rv: &Rvalue, dest: LocalId, state: &mut BorrowMap) {
        match rv {
            Rvalue::Borrow { place, is_mut, .. } => {
                let conflicts = if *is_mut {
                    has_any_borrow(state, place.local)
                } else {
                    has_mut_borrow(state, place.local)
                };

                if conflicts {
                    self.error(
                        if *is_mut {
                            "cannot mutably borrow: another borrow is already active"
                        } else {
                            "cannot shared borrow: a mutable borrow is already active"
                        },
                        DiagnosticKind::EMutableAlias,
                    );
                }

                state.entry(dest).or_default().insert(BorrowOrigin {
                    owner: place.local,
                    is_mut: *is_mut,
                });
            }
            Rvalue::Move { place, .. } => {
                if has_any_borrow(state, place.local) {
                    self.error(
                        "cannot move a place that is currently borrowed",
                        DiagnosticKind::EMoveWhileBorrowed,
                    );
                }
            }
            Rvalue::MagnetAttach { place, is_mut, .. } => {
                // Magnets alias owner storage. Respect active MIR borrows even
                // though magnet lifetime tracking itself lives in magnet.rs.
                let conflicts = if *is_mut {
                    has_any_borrow(state, place.local)
                } else {
                    has_mut_borrow(state, place.local)
                };
                if conflicts {
                    self.error(
                        "cannot attach magnet: an incompatible borrow is already active",
                        DiagnosticKind::EMutableAlias,
                    );
                }
            }
            _ => {}
        }
    }

    fn error(&mut self, msg: &str, kind: DiagnosticKind) {
        let span = Span::new(Default::default(), 0, 0);
        self.diags
            .push(Diagnostic::new(kind, span, msg.to_string()));
    }
}

fn has_any_borrow(state: &BorrowMap, owner: LocalId) -> bool {
    state
        .values()
        .any(|origins| origins.iter().any(|origin| origin.owner == owner))
}

fn has_mut_borrow(state: &BorrowMap, owner: LocalId) -> bool {
    state.values().any(|origins| {
        origins
            .iter()
            .any(|origin| origin.owner == owner && origin.is_mut)
    })
}

fn meet_borrows(a: &BorrowMap, b: &BorrowMap) -> BorrowMap {
    let mut result = a.clone();
    for (holder, origins) in b {
        result
            .entry(*holder)
            .or_default()
            .extend(origins.iter().copied());
    }
    result
}

fn successors(term: &Terminator) -> Vec<usize> {
    match term {
        Terminator::Goto(block) => vec![block.get() as usize],
        Terminator::SwitchInt {
            targets, otherwise, ..
        } => {
            let mut result: Vec<usize> = targets
                .iter()
                .map(|(_, target)| target.get() as usize)
                .collect();
            result.push(otherwise.get() as usize);
            result
        }
        Terminator::Switch {
            targets, otherwise, ..
        } => {
            let mut result: Vec<usize> = targets
                .iter()
                .map(|(_, target)| target.get() as usize)
                .collect();
            if let Some(otherwise) = otherwise {
                result.push(otherwise.get() as usize);
            }
            result
        }
        Terminator::Return { .. } | Terminator::Abort | Terminator::Unreachable => Vec::new(),
    }
}
