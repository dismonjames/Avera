use avera_compiler::codegen::{compile_bodies, object};
use avera_compiler::mir::body::{BasicBlock, Body, Local, Stmt};
use avera_compiler::mir::place::Place;
use avera_compiler::mir::rvalue::{Const, Rvalue};
use avera_compiler::mir::terminator::Terminator;
use avera_compiler::resolve::Defs;
use avera_compiler::symbol::{BlockId, LocalId};
use avera_compiler::types::intern::TyCtxt;

fn local(id: u32, ty: avera_compiler::types::ty::TyId) -> Local {
    Local {
        id: LocalId::new(id),
        name: format!("l{id}"),
        ty,
        mutable: true,
    }
}

#[test]
fn unsupported_mir_rvalue_fails_before_native_codegen() {
    let cx = TyCtxt::new();
    let mut body = Body::new("bad_borrow".to_string(), cx.unit);
    body.locals.extend([local(0, cx.i64), local(1, cx.i64)]);
    body.params.push(LocalId::new(0));
    body.blocks.push(BasicBlock {
        id: BlockId::new(0),
        stmts: vec![Stmt::Assign {
            place: Place::local(LocalId::new(1)),
            value: Rvalue::Borrow {
                place: Place::local(LocalId::new(0)),
                is_mut: false,
                ty: cx.i64,
            },
        }],
        term: Terminator::Return { value: None },
    });

    let mut module = object::make_object_module().unwrap();
    let err = compile_bodies(&mut module, &[body], &cx, &Defs::new())
        .expect_err("unsupported MIR must be a compile-time compiler error");
    assert!(err.contains("unsupported MIR rvalue `Borrow`"), "{err}");
}

#[test]
fn non_lowered_string_constant_fails_before_native_codegen() {
    let cx = TyCtxt::new();
    let mut body = Body::new("bad_const".to_string(), cx.unit);
    body.locals.push(local(0, cx.i64));
    body.blocks.push(BasicBlock {
        id: BlockId::new(0),
        stmts: vec![Stmt::Assign {
            place: Place::local(LocalId::new(0)),
            value: Rvalue::Const(Const::Str("not lowered".to_string())),
        }],
        term: Terminator::Return { value: None },
    });

    let mut module = object::make_object_module().unwrap();
    let err = compile_bodies(&mut module, &[body], &cx, &Defs::new())
        .expect_err("non-lowered constants must not become runtime traps");
    assert!(err.contains("non-lowered constant `Str`"), "{err}");
}
