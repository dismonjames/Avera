use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use crate::diagnostics::span::Span;
use crate::mir::body::{Body, Stmt};
use crate::mir::rvalue::Rvalue;
use crate::symbol::LocalId;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum MagnetState {
    Detached,
    Attached,
    Moved,
    Dropped,
}

type MagnetMap = HashMap<LocalId, MagnetState>;

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
        for block in &self.body.blocks {
            let mut state: MagnetMap = HashMap::new();
            for s in &block.stmts {
                self.transfer_stmt(s, &mut state);
            }
        }
    }

    fn transfer_stmt(&mut self, s: &Stmt, state: &mut MagnetMap) {
        match s {
            Stmt::Assign { place, value } => {
                self.handle_rvalue(value, state, place.local);
                // Writing to a magnet local: if it was a magnet, it's being
                // reassigned. The new rvalue determines the state.
                // handle_rvalue already set the state if it was a magnet op.
                // For non-magnet assignments, clear the magnet state — UNLESS
                // this is a write-through: `Assign { place: m, value: Store {
                // addr: m, ... } }` writes through the magnet's own address
                // into the storage it points at, leaving the magnet attached.
                if !is_magnet_rvalue(value) {
                    let write_through =
                        matches!(value, Rvalue::Store { addr, .. } if addr.local == place.local);
                    if !write_through {
                        state.remove(&place.local);
                    }
                }
            }
            Stmt::StorageLive(_) | Stmt::StorageDead(_) => {}
            Stmt::Drop(place) => {
                if let Some(st) = state.get(&place.local) {
                    if *st == MagnetState::Attached {
                        state.insert(place.local, MagnetState::Dropped);
                    }
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
            Rvalue::MagnetAttach { .. } => {
                state.insert(dest, MagnetState::Attached);
            }
            Rvalue::MagnetNone { .. } => {
                state.insert(dest, MagnetState::Detached);
            }
            Rvalue::MagnetFromAddr { .. } => {
                state.insert(dest, MagnetState::Attached);
            }
            Rvalue::MagnetAddress { magnet, .. } => {
                let st = state
                    .get(&magnet.local)
                    .copied()
                    .unwrap_or(MagnetState::Detached);
                match st {
                    MagnetState::Attached => {}
                    MagnetState::Detached => {
                        self.error(
                            "cannot read address of a detached magnet",
                            DiagnosticKind::EMagnetDetached,
                        );
                    }
                    MagnetState::Moved => {
                        self.error(
                            "cannot read address: magnet target has moved",
                            DiagnosticKind::EMagnetUseAfterMove,
                        );
                    }
                    MagnetState::Dropped => {
                        self.error(
                            "cannot read address: magnet has been dropped",
                            DiagnosticKind::EMagnetUseAfterMove,
                        );
                    }
                }
            }
            Rvalue::MagnetOffset { magnet, .. } => {
                let st = state
                    .get(&magnet.local)
                    .copied()
                    .unwrap_or(MagnetState::Detached);
                if st != MagnetState::Attached {
                    self.error(
                        "cannot read offset of a non-attached magnet",
                        DiagnosticKind::EMagnetDetached,
                    );
                }
            }
            Rvalue::MagnetAdd { magnet, .. } => {
                let st = state
                    .get(&magnet.local)
                    .copied()
                    .unwrap_or(MagnetState::Detached);
                if st != MagnetState::Attached {
                    self.error(
                        "cannot perform pointer arithmetic on a non-attached magnet",
                        DiagnosticKind::EMagnetDetached,
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

fn is_magnet_rvalue(rv: &Rvalue) -> bool {
    matches!(
        rv,
        Rvalue::MagnetAttach { .. } | Rvalue::MagnetNone { .. } | Rvalue::MagnetFromAddr { .. }
    )
}
