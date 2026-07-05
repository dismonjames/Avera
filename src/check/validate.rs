use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use crate::diagnostics::span::Span;
use crate::mir::body::{Body, Stmt};
use crate::mir::place::{Place, PlaceElem};
use crate::mir::rvalue::Rvalue;
use crate::mir::terminator::Terminator;
use crate::symbol::{BlockId, LocalId};
use crate::types::intern::TyCtxt;
use crate::types::ty::TyData;

pub fn validate_mir(body: &Body, diags: &mut DiagnosticList) {
    let mut v = Validator { body, diags };
    v.run();
}

struct Validator<'a> {
    body: &'a Body,
    diags: &'a mut DiagnosticList,
}

impl<'a> Validator<'a> {
    fn run(&mut self) {
        self.check_structure();
        self.check_blocks();
        self.check_terminators();
    }

    fn check_structure(&mut self) {
        if self.body.blocks.is_empty() {
            self.diag("MIR body has no blocks", DiagnosticKind::MirEmptyBody);
            return;
        }
        if self.body.entry.get() != 0 {
            self.diag(
                &format!(
                    "entry block must be block 0, but is block {}",
                    self.body.entry.get()
                ),
                DiagnosticKind::MirInvalidEntry,
            );
        }
        for (i, b) in self.body.blocks.iter().enumerate() {
            if b.id.get() as usize != i {
                self.diag(
                    &format!(
                        "block at index {} has id {} (must match index)",
                        i,
                        b.id.get()
                    ),
                    DiagnosticKind::MirBlockIdMismatch,
                );
            }
        }
    }

    fn check_blocks(&mut self) {
        for b in &self.body.blocks {
            for s in &b.stmts {
                self.check_stmt(s);
            }
        }
    }

    fn check_stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Assign { place, value } => {
                self.check_place(place);
                self.check_rvalue(value);
            }
            Stmt::StorageLive(id) | Stmt::StorageDead(id) => {
                self.check_local(*id);
            }
            Stmt::Drop(place) => {
                self.check_place(place);
            }
            Stmt::Call { dest, args, .. } => {
                self.check_place(dest);
                for a in args {
                    self.check_place(a);
                }
            }
            Stmt::Assert { cond, .. } => {
                self.check_place(cond);
            }
        }
    }

    fn check_rvalue(&mut self, rv: &Rvalue) {
        match rv {
            Rvalue::Use(p) => self.check_place(p),
            Rvalue::Const(_) => {}
            Rvalue::BinOp { lhs, rhs, .. } => {
                self.check_place(lhs);
                self.check_place(rhs);
            }
            Rvalue::UnOp { operand, .. } => self.check_place(operand),
            Rvalue::Borrow { place, .. } => self.check_place(place),
            Rvalue::Move { place, .. } => self.check_place(place),
            Rvalue::MagnetAttach { place, .. } => self.check_place(place),
            Rvalue::MagnetNone { .. } => {}
            Rvalue::MagnetFromAddr { addr, .. } => self.check_place(addr),
            Rvalue::MagnetAddress { magnet, .. } => self.check_place(magnet),
            Rvalue::MagnetOffset { magnet, .. } => self.check_place(magnet),
            Rvalue::MagnetAdd { magnet, delta, .. } => {
                self.check_place(magnet);
                self.check_place(delta);
            }
            Rvalue::Cast { operand, .. } => self.check_place(operand),
            Rvalue::ChoiceCtor { args, .. } => {
                for a in args {
                    self.check_place(a);
                }
            }
            Rvalue::ShapeCtor { fields, .. } => {
                for f in fields {
                    self.check_place(f);
                }
            }
            Rvalue::ArrayCtor { elems, .. } => {
                for e in elems {
                    self.check_place(e);
                }
            }
            Rvalue::Call { args, .. } => {
                for a in args {
                    self.check_place(a);
                }
            }
            Rvalue::Try { operand, .. } => self.check_place(operand),
            Rvalue::Load { addr, .. } => self.check_place(addr),
            Rvalue::Store { addr, value, .. } => {
                self.check_place(addr);
                self.check_place(value);
            }
        }
    }

    fn check_place(&mut self, p: &Place) {
        self.check_local(p.local);
        for e in &p.elems {
            if let PlaceElem::Index(id) = e {
                self.check_local(*id);
            }
        }
    }

    fn check_local(&mut self, id: LocalId) {
        let idx = id.get() as usize;
        if idx >= self.body.locals.len() {
            self.diag(
                &format!(
                    "local {} is out of range ({} locals declared)",
                    id.get(),
                    self.body.locals.len()
                ),
                DiagnosticKind::MirInvalidLocal,
            );
        }
    }

    fn check_terminators(&mut self) {
        for b in &self.body.blocks {
            self.check_terminator(&b.term);
        }
    }

    fn check_terminator(&mut self, t: &Terminator) {
        let nblocks = self.body.blocks.len();
        match t {
            Terminator::Goto(b) => {
                self.check_target(*b, nblocks);
            }
            Terminator::SwitchInt {
                targets, otherwise, ..
            } => {
                for (_, t) in targets {
                    self.check_target(*t, nblocks);
                }
                self.check_target(*otherwise, nblocks);
            }
            Terminator::Switch {
                targets, otherwise, ..
            } => {
                for (_, t) in targets {
                    self.check_target(*t, nblocks);
                }
                if let Some(o) = otherwise {
                    self.check_target(*o, nblocks);
                }
            }
            Terminator::Return { .. } | Terminator::Abort | Terminator::Unreachable => {}
        }
    }

    fn check_target(&mut self, target: BlockId, nblocks: usize) {
        let idx = target.get() as usize;
        if idx >= nblocks {
            self.diag(
                &format!(
                    "jump target block {} is out of range ({} blocks)",
                    target.get(),
                    nblocks
                ),
                DiagnosticKind::MirInvalidTarget,
            );
        }
    }

    fn diag(&mut self, msg: &str, kind: DiagnosticKind) {
        let span = Span::new(Default::default(), 0, 0);
        self.diags
            .push(Diagnostic::new(kind, span, msg.to_string()));
    }
}

pub fn validate_return_consistency(body: &Body, cx: &TyCtxt, diags: &mut DiagnosticList) {
    let ret_is_unit = matches!(cx.data(body.ret_ty), TyData::Unit | TyData::Never);
    if ret_is_unit {
        return;
    }
    for b in &body.blocks {
        if let Terminator::Return { value: None } = &b.term {
            let span = Span::new(Default::default(), 0, 0);
            diags.push(Diagnostic::new(
                DiagnosticKind::MirValidation,
                span,
                format!(
                    "function `{}` returns without a value (expected non-unit type)",
                    body.name
                ),
            ));
        }
    }
}
