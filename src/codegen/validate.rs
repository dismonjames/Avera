use crate::mir::body::{Body, Stmt};
use crate::mir::rvalue::{Const, Rvalue};

pub fn validate_codegen_bodies(bodies: &[Body]) -> Result<(), String> {
    for body in bodies {
        for block in &body.blocks {
            for stmt in &block.stmts {
                validate_stmt(&body.name, stmt)?;
            }
        }
    }
    Ok(())
}

fn validate_stmt(body: &str, stmt: &Stmt) -> Result<(), String> {
    if let Stmt::Assign { value, .. } = stmt {
        validate_rvalue(body, value)?;
    }
    Ok(())
}

fn validate_rvalue(body: &str, value: &Rvalue) -> Result<(), String> {
    match value {
        Rvalue::Borrow { .. }
        | Rvalue::ChoiceCtor { .. }
        | Rvalue::ShapeCtor { .. }
        | Rvalue::ArrayCtor { .. }
        | Rvalue::Try { .. } => Err(format!(
            "internal compiler error: codegen received unsupported MIR rvalue `{}` in `{body}`",
            rvalue_name(value)
        )),
        Rvalue::Const(Const::Str(_)) | Rvalue::Const(Const::Addr(_)) => Err(format!(
            "internal compiler error: codegen received non-lowered constant `{}` in `{body}`",
            const_name(value)
        )),
        _ => Ok(()),
    }
}

fn rvalue_name(value: &Rvalue) -> &'static str {
    match value {
        Rvalue::Borrow { .. } => "Borrow",
        Rvalue::ChoiceCtor { .. } => "ChoiceCtor",
        Rvalue::ShapeCtor { .. } => "ShapeCtor",
        Rvalue::ArrayCtor { .. } => "ArrayCtor",
        Rvalue::Try { .. } => "Try",
        _ => "supported",
    }
}

fn const_name(value: &Rvalue) -> &'static str {
    match value {
        Rvalue::Const(Const::Str(_)) => "Str",
        Rvalue::Const(Const::Addr(_)) => "Addr",
        _ => "supported",
    }
}
