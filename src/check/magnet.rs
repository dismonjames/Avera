use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use crate::diagnostics::span::Span;
use crate::mir::body::{Body, Stmt};
use crate::mir::rvalue::Rvalue;
use crate::mir::terminator::Terminator;
use crate::symbol::LocalId;
use std::collections::{HashMap, HashSet};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum MagnetState {
    Detached,
    Attached,
    Moved,
    Dropped,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct MagnetFact {
    state: MagnetState,
    target: Option<LocalId>,
    is_mut: bool,
}

type MagnetMap = HashMap<LocalId, HashSet<MagnetFact>>;

pub fn check_magnet(body: &Body, diags: &mut DiagnosticList) {
    let mut checker = MagnetChecker { body, diags };
    checker.run();
}

struct MagnetChecker<'a> {
    body: &'a Body,
    diags: &'a mut DiagnosticList,
}

impl<'a> MagnetChecker<'a> {
    fn run(&mut self) {
        let nblocks = self.body.blocks.len();
        if nblocks == 0 {
            return;
        }

        let mut in_states: Vec<MagnetMap> = vec![HashMap::new(); nblocks];
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
                    meet_magnets(&in_states[succ], &out_state)
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

    fn transfer_stmt(&mut self, stmt: &Stmt, state: &mut MagnetMap) {
        match stmt {
            Stmt::Assign { place, value } => {
                self.handle_rvalue(value, state, place.local);

                if !is_magnet_rvalue(value) {
                    let write_through =
                        matches!(value, Rvalue::Store { addr, .. } if addr.local == place.local);
                    if !write_through {
                        state.remove(&place.local);
                    }
                }
            }
            Stmt::StorageLive(id) => {
                state.remove(id);
            }
            Stmt::StorageDead(id) => {
                self.invalidate_target(*id, MagnetState::Dropped, state);
                state.remove(id);
            }
            Stmt::Drop(place) => {
                self.invalidate_target(place.local, MagnetState::Dropped, state);
                if let Some(facts) = state.get_mut(&place.local) {
                    *facts = facts
                        .iter()
                        .map(|fact| MagnetFact {
                            state: MagnetState::Dropped,
                            ..*fact
                        })
                        .collect();
                }
            }
            Stmt::Call { dest, .. } => {
                state.remove(&dest.local);
            }
            Stmt::Assert { .. } => {}
        }
    }

    fn handle_rvalue(&mut self, rv: &Rvalue, state: &mut MagnetMap, dest: LocalId) {
        match rv {
            Rvalue::MagnetAttach { place, is_mut, .. } => {
                if state.get(&dest).is_some_and(|facts| {
                    facts
                        .iter()
                        .any(|fact| fact.state == MagnetState::Attached && !fact.is_mut)
                }) {
                    self.error(
                        "cannot retarget an immutable magnet",
                        DiagnosticKind::EImmutableMutate,
                    );
                }
                state.insert(
                    dest,
                    HashSet::from([MagnetFact {
                        state: MagnetState::Attached,
                        target: Some(place.local),
                        is_mut: *is_mut,
                    }]),
                );
            }
            Rvalue::MagnetNone { .. } => {
                state.insert(
                    dest,
                    HashSet::from([MagnetFact {
                        state: MagnetState::Detached,
                        target: None,
                        is_mut: false,
                    }]),
                );
            }
            Rvalue::MagnetFromAddr { .. } => {
                state.insert(
                    dest,
                    HashSet::from([MagnetFact {
                        state: MagnetState::Attached,
                        target: None,
                        is_mut: false,
                    }]),
                );
            }
            Rvalue::MagnetAddress { magnet, .. }
            | Rvalue::MagnetOffset { magnet, .. }
            | Rvalue::MagnetAdd { magnet, .. } => {
                self.require_attached(magnet.local, state);
            }
            Rvalue::Store { addr, .. } => {
                if let Some(facts) = state.get(&addr.local) {
                    self.require_attached(addr.local, state);
                    if facts
                        .iter()
                        .any(|fact| fact.state == MagnetState::Attached && !fact.is_mut)
                    {
                        self.error(
                            "cannot write through an immutable magnet",
                            DiagnosticKind::EImmutableMutate,
                        );
                    }
                }
            }
            Rvalue::Move { place, .. } => {
                self.invalidate_target(place.local, MagnetState::Moved, state);
            }
            _ => {}
        }
    }

    fn require_attached(&mut self, magnet: LocalId, state: &MagnetMap) {
        let Some(facts) = state.get(&magnet) else {
            self.error(
                "cannot use a detached magnet",
                DiagnosticKind::EMagnetDetached,
            );
            return;
        };

        if facts.iter().all(|fact| fact.state == MagnetState::Attached) {
            return;
        }

        if facts.iter().any(|fact| fact.state == MagnetState::Moved) {
            self.error(
                "cannot use magnet: its target has moved",
                DiagnosticKind::EMagnetUseAfterMove,
            );
        } else if facts.iter().any(|fact| fact.state == MagnetState::Dropped) {
            self.error(
                "cannot use magnet: it or its target has been dropped",
                DiagnosticKind::EMagnetUseAfterMove,
            );
        } else {
            self.error(
                "cannot use a detached magnet",
                DiagnosticKind::EMagnetDetached,
            );
        }
    }

    fn invalidate_target(
        &mut self,
        target: LocalId,
        new_state: MagnetState,
        state: &mut MagnetMap,
    ) {
        let holders: Vec<LocalId> = state
            .iter()
            .filter_map(|(holder, facts)| {
                facts
                    .iter()
                    .any(|fact| fact.state == MagnetState::Attached && fact.target == Some(target))
                    .then_some(*holder)
            })
            .collect();

        for holder in holders {
            self.error(
                match new_state {
                    MagnetState::Moved => "cannot move a value while a magnet points to it",
                    MagnetState::Dropped => "cannot drop a value while a magnet points to it",
                    _ => "cannot invalidate a value while a magnet points to it",
                },
                DiagnosticKind::EMoveWhileBorrowed,
            );
            if let Some(facts) = state.get_mut(&holder) {
                *facts = facts
                    .iter()
                    .map(|fact| {
                        if fact.state == MagnetState::Attached && fact.target == Some(target) {
                            MagnetFact {
                                state: new_state,
                                ..*fact
                            }
                        } else {
                            *fact
                        }
                    })
                    .collect();
            }
        }
    }

    fn error(&mut self, msg: &str, kind: DiagnosticKind) {
        let span = Span::new(Default::default(), 0, 0);
        self.diags
            .push(Diagnostic::new(kind, span, msg.to_string()));
    }
}

fn meet_magnets(a: &MagnetMap, b: &MagnetMap) -> MagnetMap {
    let mut result = a.clone();
    for (magnet, facts) in b {
        result
            .entry(*magnet)
            .or_default()
            .extend(facts.iter().copied());
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

fn is_magnet_rvalue(rv: &Rvalue) -> bool {
    matches!(
        rv,
        Rvalue::MagnetAttach { .. } | Rvalue::MagnetNone { .. } | Rvalue::MagnetFromAddr { .. }
    )
}
