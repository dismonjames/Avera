use crate::ast::expr::{Expr, ExprKind, Lit};
use crate::ast::pat::PatKind;
use crate::ast::path::Path;
use crate::ast::stmt::{AssignOp, Stmt, StmtKind};
use crate::ast::{ItemKind, Module};
use crate::mir::body::{Body, Local, Stmt as MirStmt};
use crate::mir::place::Place;
use crate::mir::rvalue::Rvalue;
use crate::resolve::Defs;
use crate::symbol::LocalId;
use crate::types::intern::TyCtxt;

mod legacy {
    include!("lower_legacy.rs");
}

#[derive(Clone, Debug)]
enum LoopKind {
    Other,
    RangeFor(String),
}

pub fn lower_module(cx: &mut TyCtxt, defs: &Defs, module: &Module) -> Result<Vec<Body>, String> {
    let mut checked = module.clone();
    validate_and_rewrite_module(&mut checked)?;
    let mut bodies = legacy::lower_module(cx, defs, &checked)?;
    normalize_mir(&mut bodies, cx);
    Ok(bodies)
}

fn validate_and_rewrite_module(module: &mut Module) -> Result<(), String> {
    for item in &mut module.items {
        if let ItemKind::Action(action) = &mut item.kind {
            if let Some(body) = &mut action.body {
                rewrite_block(body, &mut Vec::new())?;
            }
        }
    }
    Ok(())
}

fn rewrite_block(stmts: &mut Vec<Stmt>, loops: &mut Vec<LoopKind>) -> Result<(), String> {
    let mut rewritten = Vec::with_capacity(stmts.len());
    for mut stmt in std::mem::take(stmts) {
        match &mut stmt.kind {
            StmtKind::If {
                branches,
                else_branch,
            } => {
                for (_, body) in branches {
                    rewrite_block(body, loops)?;
                }
                if let Some(body) = else_branch {
                    rewrite_block(body, loops)?;
                }
            }
            StmtKind::While { body, .. } | StmtKind::Loop(body) => {
                loops.push(LoopKind::Other);
                rewrite_block(body, loops)?;
                loops.pop();
            }
            StmtKind::For {
                pat, iter, body, ..
            } => {
                if !matches!(iter.kind, ExprKind::Range { .. }) {
                    return Err("non-range for iterators are not implemented in stage-0".to_string());
                }
                let name = match &pat.kind {
                    PatKind::Bind(name) => name.clone(),
                    _ => {
                        return Err(
                            "range-for currently requires a simple binding pattern".to_string(),
                        )
                    }
                };
                loops.push(LoopKind::RangeFor(name));
                rewrite_block(body, loops)?;
                loops.pop();
            }
            StmtKind::Match { arms, .. } => {
                for arm in arms {
                    rewrite_block(&mut arm.body, loops)?;
                }
            }
            StmtKind::Raw(body) => rewrite_block(body, loops)?,
            StmtKind::Break => {
                if loops.is_empty() {
                    return Err("`break` used outside loop".to_string());
                }
            }
            StmtKind::Continue => {
                let Some(loop_kind) = loops.last() else {
                    return Err("`continue` used outside loop".to_string());
                };
                if let LoopKind::RangeFor(counter) = loop_kind {
                    rewritten.push(range_increment(&stmt, counter));
                }
            }
            StmtKind::Expr(expr) => reject_fake_eprint(expr)?,
            StmtKind::OwnerBinding { value, .. }
            | StmtKind::ConstBinding { value, .. }
            | StmtKind::Assignment { value, .. }
            | StmtKind::Drop(value)
            | StmtKind::Panic(value) => reject_fake_eprint(value)?,
            StmtKind::MagnetBinding { target, .. }
            | StmtKind::MagnetMutBinding { target, .. }
            | StmtKind::MagnetRetarget { target, .. } => reject_fake_eprint(target)?,
            StmtKind::Return(Some(expr)) => reject_fake_eprint(expr)?,
            StmtKind::Return(None) => {}
        }
        rewritten.push(stmt);
    }
    *stmts = rewritten;
    Ok(())
}

fn range_increment(continue_stmt: &Stmt, counter: &str) -> Stmt {
    let span = continue_stmt.span;
    Stmt::new(
        span,
        StmtKind::Assignment {
            place: Expr::new(span, ExprKind::Path(Path::single(span, counter))),
            op: AssignOp::Add,
            value: Expr::new(span, ExprKind::Literal(Lit::Int("1".to_string()))),
        },
    )
}

fn reject_fake_eprint(expr: &Expr) -> Result<(), String> {
    match &expr.kind {
        ExprKind::Call { callee, args } => {
            if let ExprKind::Path(path) = &callee.kind {
                if path.segments.last().is_some_and(|name| name == "eprint") {
                    return Err(
                        "`eprint` is not implemented as stderr output in stage-0 yet".to_string(),
                    );
                }
            }
            reject_fake_eprint(callee)?;
            for arg in args {
                reject_fake_eprint(arg)?;
            }
        }
        ExprKind::MethodCall {
            receiver, args, ..
        } => {
            reject_fake_eprint(receiver)?;
            for arg in args {
                reject_fake_eprint(arg)?;
            }
        }
        ExprKind::ChoiceCtor { args, .. } => {
            for arg in args {
                reject_fake_eprint(arg)?;
            }
        }
        ExprKind::Unary { operand, .. }
        | ExprKind::Cast { operand, .. }
        | ExprKind::Try(operand)
        | ExprKind::MagnetValid(operand)
        | ExprKind::Paren(operand) => reject_fake_eprint(operand)?,
        ExprKind::Binary { lhs, rhs, .. } => {
            reject_fake_eprint(lhs)?;
            reject_fake_eprint(rhs)?;
        }
        ExprKind::Field { receiver, .. } => reject_fake_eprint(receiver)?,
        ExprKind::Index { base, index } => {
            reject_fake_eprint(base)?;
            reject_fake_eprint(index)?;
        }
        ExprKind::Slice { base, lo, hi } => {
            reject_fake_eprint(base)?;
            if let Some(lo) = lo {
                reject_fake_eprint(lo)?;
            }
            if let Some(hi) = hi {
                reject_fake_eprint(hi)?;
            }
        }
        ExprKind::MagnetMeta { target, .. } => reject_fake_eprint(target)?,
        ExprKind::Range { lo, hi } => {
            reject_fake_eprint(lo)?;
            reject_fake_eprint(hi)?;
        }
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfRef | ExprKind::None => {}
    }
    Ok(())
}

fn normalize_mir(bodies: &mut [Body], cx: &TyCtxt) {
    for body in bodies {
        preserve_magnet_mutability(body);
        detach_unit_setter_destinations(body, cx);
    }
}

fn preserve_magnet_mutability(body: &mut Body) {
    let mut immutable = std::collections::HashSet::new();
    for block in &body.blocks {
        for stmt in &block.stmts {
            if let MirStmt::Assign {
                place,
                value: Rvalue::MagnetAttach { is_mut: false, .. },
            } = stmt
            {
                immutable.insert(place.local);
            }
        }
    }

    for block in &mut body.blocks {
        for stmt in &mut block.stmts {
            if let MirStmt::Assign {
                place,
                value: Rvalue::MagnetAttach { is_mut, .. },
            } = stmt
            {
                if immutable.contains(&place.local) {
                    *is_mut = false;
                }
            }
        }
    }
}

fn detach_unit_setter_destinations(body: &mut Body, cx: &TyCtxt) {
    for block_index in 0..body.blocks.len() {
        for stmt_index in 0..body.blocks[block_index].stmts.len() {
            let replacement = match &body.blocks[block_index].stmts[stmt_index] {
                MirStmt::Call {
                    dest, callee, args, ..
                } if matches!(callee.as_str(), "avera_array_set" | "avera_text_set")
                    && args.first().is_some_and(|arg| arg.local == dest.local) =>
                {
                    Some((dest.clone(), callee.clone(), args.clone()))
                }
                _ => None,
            };

            let Some((_old_dest, callee, args)) = replacement else {
                continue;
            };
            let id = LocalId::new(body.locals.len() as u32);
            body.locals.push(Local {
                id,
                name: String::new(),
                ty: cx.unit,
                mutable: false,
            });
            if let MirStmt::Call { dest, ret_ty, .. } =
                &mut body.blocks[block_index].stmts[stmt_index]
            {
                *dest = Place::local(id);
                *ret_ty = cx.unit;
            }
            let _ = (callee, args);
        }
    }
}
