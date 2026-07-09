use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use crate::diagnostics::span::Span;
use crate::mir::body::{Body, Stmt};
use crate::mir::rvalue::Rvalue;
use crate::symbol::LocalId;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BorrowState {
    None,
    Shared,
    Mut,
}

type BorrowMap = HashMap<LocalId, BorrowState>;

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
        // v0.1: per-block analysis. Borrows in our MIR are created and
        // consumed within the same function via fresh temporaries.
        for block in &self.body.blocks {
            let mut state: BorrowMap = HashMap::new();
            for s in &block.stmts {
                self.transfer_stmt(s, &mut state);
            }
        }
    }

    fn transfer_stmt(&mut self, s: &Stmt, state: &mut BorrowMap) {
        match s {
            Stmt::Assign { place, value } => {
                // If writing to a borrowed place, that's an error.
                let borrowed = state
                    .get(&place.local)
                    .copied()
                    .unwrap_or(BorrowState::None);
                if borrowed == BorrowState::Mut {
                    self.error(
                        "cannot assign to a place that is mutably borrowed",
                        DiagnosticKind::EImmutableMutate,
                    );
                }
                self.handle_rvalue(value, state);
                // The dest local is a fresh borrow local or a regular
                // assignment; clear its borrow state.
                state.insert(place.local, BorrowState::None);
            }
            Stmt::StorageLive(_) | Stmt::StorageDead(_) => {}
            Stmt::Drop(place) => {
                let borrowed = state
                    .get(&place.local)
                    .copied()
                    .unwrap_or(BorrowState::None);
                if borrowed != BorrowState::None {
                    self.error(
                        "cannot drop a place that is currently borrowed",
                        DiagnosticKind::EMoveWhileBorrowed,
                    );
                }
            }
            Stmt::Call { dest, .. } => {
                state.insert(dest.local, BorrowState::None);
            }
            Stmt::Assert { .. } => {}
        }
    }

    fn handle_rvalue(&mut self, rv: &Rvalue, state: &mut BorrowMap) {
        match rv {
            Rvalue::Borrow { place, is_mut, .. } => {
                let cur = state
                    .get(&place.local)
                    .copied()
                    .unwrap_or(BorrowState::None);
                if *is_mut {
                    match cur {
                        BorrowState::None => {
                            state.insert(place.local, BorrowState::Mut);
                        }
                        BorrowState::Shared => {
                            self.error(
                                "cannot mutably borrow: a shared borrow is already active",
                                DiagnosticKind::EMutableAlias,
                            );
                        }
                        BorrowState::Mut => {
                            self.error(
                                "cannot mutably borrow: a mutable borrow is already active",
                                DiagnosticKind::EMutableAlias,
                            );
                        }
                    }
                } else {
                    match cur {
                        BorrowState::None | BorrowState::Shared => {
                            state.insert(place.local, BorrowState::Shared);
                        }
                        BorrowState::Mut => {
                            self.error(
                                "cannot shared borrow: a mutable borrow is already active",
                                DiagnosticKind::EMutableAlias,
                            );
                        }
                    }
                }
            }
            Rvalue::MagnetAttach { place, .. } => {
                // A magnet attaches to a place, acting like a borrow.
                let cur = state
                    .get(&place.local)
                    .copied()
                    .unwrap_or(BorrowState::None);
                if cur == BorrowState::Mut {
                    self.error(
                        "cannot attach magnet: a mutable borrow is already active",
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
