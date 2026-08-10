use avera_compiler::check::{check_borrow, check_magnet, check_ownership, validate_mir};
use avera_compiler::diagnostics::diagnostic::{DiagnosticKind, DiagnosticList};
use avera_compiler::mir::body::{BasicBlock, Body, Local, Stmt};
use avera_compiler::mir::place::Place;
use avera_compiler::mir::rvalue::Rvalue;
use avera_compiler::mir::terminator::Terminator;
use avera_compiler::symbol::{BlockId, LocalId, TypeId};

fn local(id: u32, name: &str) -> Local {
    Local {
        id: LocalId::new(id),
        name: name.to_string(),
        ty: TypeId::new(0),
        mutable: true,
    }
}

#[test]
fn ownership_rejects_uninitialized_mir_use() {
    let x = LocalId::new(0);
    let mut body = Body::new("uninit".to_string(), TypeId::new(0));
    body.locals.push(local(0, "x"));
    body.blocks.push(BasicBlock {
        id: BlockId::new(0),
        stmts: vec![Stmt::Assert {
            cond: Place::local(x),
            msg: "touch x".to_string(),
        }],
        term: Terminator::Return { value: None },
    });

    let mut diags = DiagnosticList::new();
    check_ownership(&body, &mut diags);

    assert!(diags.iter().any(|diag| {
        diag.kind == DiagnosticKind::EUseAfterDrop && diag.message.contains("uninitialized value")
    }));
}

#[test]
fn borrow_survives_cfg_edge_and_blocks_move() {
    let owner = LocalId::new(0);
    let holder = LocalId::new(1);
    let moved = LocalId::new(2);
    let ty = TypeId::new(0);

    let mut body = Body::new("borrow_cfg".to_string(), ty);
    body.locals
        .extend([local(0, "owner"), local(1, "holder"), local(2, "moved")]);
    body.params.push(owner);
    body.blocks.push(BasicBlock {
        id: BlockId::new(0),
        stmts: vec![Stmt::Assign {
            place: Place::local(holder),
            value: Rvalue::Borrow {
                place: Place::local(owner),
                is_mut: false,
                ty,
            },
        }],
        term: Terminator::Goto(BlockId::new(1)),
    });
    body.blocks.push(BasicBlock {
        id: BlockId::new(1),
        stmts: vec![Stmt::Assign {
            place: Place::local(moved),
            value: Rvalue::Move {
                place: Place::local(owner),
                ty,
            },
        }],
        term: Terminator::Return { value: None },
    });

    let mut diags = DiagnosticList::new();
    check_borrow(&body, &mut diags);

    assert!(diags
        .iter()
        .any(|diag| diag.kind == DiagnosticKind::EMoveWhileBorrowed));
}

#[test]
fn magnet_survives_cfg_edge() {
    let owner = LocalId::new(0);
    let magnet = LocalId::new(1);
    let address = LocalId::new(2);
    let ty = TypeId::new(0);

    let mut body = Body::new("magnet_cfg".to_string(), ty);
    body.locals
        .extend([local(0, "owner"), local(1, "magnet"), local(2, "address")]);
    body.params.push(owner);
    body.blocks.push(BasicBlock {
        id: BlockId::new(0),
        stmts: vec![Stmt::Assign {
            place: Place::local(magnet),
            value: Rvalue::MagnetAttach {
                place: Place::local(owner),
                is_mut: false,
                ty,
            },
        }],
        term: Terminator::Goto(BlockId::new(1)),
    });
    body.blocks.push(BasicBlock {
        id: BlockId::new(1),
        stmts: vec![Stmt::Assign {
            place: Place::local(address),
            value: Rvalue::MagnetAddress {
                magnet: Place::local(magnet),
                ty,
            },
        }],
        term: Terminator::Return { value: None },
    });

    let mut diags = DiagnosticList::new();
    check_magnet(&body, &mut diags);

    assert!(diags.is_empty(), "unexpected diagnostics after CFG edge");
}

#[test]
fn magnet_blocks_move_of_live_target() {
    let owner = LocalId::new(0);
    let magnet = LocalId::new(1);
    let moved = LocalId::new(2);
    let ty = TypeId::new(0);

    let mut body = Body::new("magnet_move".to_string(), ty);
    body.locals
        .extend([local(0, "owner"), local(1, "magnet"), local(2, "moved")]);
    body.params.push(owner);
    body.blocks.push(BasicBlock {
        id: BlockId::new(0),
        stmts: vec![
            Stmt::Assign {
                place: Place::local(magnet),
                value: Rvalue::MagnetAttach {
                    place: Place::local(owner),
                    is_mut: false,
                    ty,
                },
            },
            Stmt::Assign {
                place: Place::local(moved),
                value: Rvalue::Move {
                    place: Place::local(owner),
                    ty,
                },
            },
        ],
        term: Terminator::Return { value: None },
    });

    let mut diags = DiagnosticList::new();
    check_magnet(&body, &mut diags);

    assert!(diags
        .iter()
        .any(|diag| diag.kind == DiagnosticKind::EMoveWhileBorrowed));
}

#[test]
fn immutable_magnet_rejects_write_through() {
    let owner = LocalId::new(0);
    let magnet = LocalId::new(1);
    let value = LocalId::new(2);
    let ty = TypeId::new(0);

    let mut body = Body::new("magnet_const".to_string(), ty);
    body.locals
        .extend([local(0, "owner"), local(1, "magnet"), local(2, "value")]);
    body.params.extend([owner, value]);
    body.blocks.push(BasicBlock {
        id: BlockId::new(0),
        stmts: vec![
            Stmt::Assign {
                place: Place::local(magnet),
                value: Rvalue::MagnetAttach {
                    place: Place::local(owner),
                    is_mut: false,
                    ty,
                },
            },
            Stmt::Assign {
                place: Place::local(magnet),
                value: Rvalue::Store {
                    addr: Place::local(magnet),
                    value: Place::local(value),
                    offset: 0,
                    ty,
                },
            },
        ],
        term: Terminator::Return { value: None },
    });

    let mut diags = DiagnosticList::new();
    check_magnet(&body, &mut diags);

    assert!(diags
        .iter()
        .any(|diag| diag.kind == DiagnosticKind::EImmutableMutate));
}

#[test]
fn validator_checks_return_operand_local() {
    let mut body = Body::new("bad_return".to_string(), TypeId::new(0));
    body.locals.push(local(0, "ok"));
    body.blocks.push(BasicBlock {
        id: BlockId::new(0),
        stmts: Vec::new(),
        term: Terminator::Return {
            value: Some(Place::local(LocalId::new(99))),
        },
    });

    let mut diags = DiagnosticList::new();
    validate_mir(&body, &mut diags);

    assert!(diags
        .iter()
        .any(|diag| diag.kind == DiagnosticKind::MirInvalidLocal));
}
