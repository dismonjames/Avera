use crate::ast::*;

pub trait Visit {
    fn visit_module(&mut self, m: &Module) {
        walk_module(self, m);
    }
    fn visit_item(&mut self, i: &Item) {
        walk_item(self, i);
    }
    fn visit_stmt(&mut self, s: &Stmt) {
        walk_stmt(self, s);
    }
    fn visit_expr(&mut self, e: &Expr) {
        walk_expr(self, e);
    }
    fn visit_pat(&mut self, p: &Pat) {
        walk_pat(self, p);
    }
    fn visit_ty(&mut self, t: &Ty) {
        walk_ty(self, t);
    }
}

pub fn walk_module<V: Visit + ?Sized>(v: &mut V, m: &Module) {
    for i in &m.items {
        v.visit_item(i);
    }
}

pub fn walk_item<V: Visit + ?Sized>(v: &mut V, i: &Item) {
    match &i.kind {
        ItemKind::Shape(s) => {
            for f in &s.fields {
                v.visit_ty(&f.ty);
            }
        }
        ItemKind::Choice(c) => {
            for var in &c.variants {
                if let Some(p) = &var.payload {
                    for (_, t) in p {
                        v.visit_ty(t);
                    }
                }
            }
        }
        ItemKind::Ability(a) => {
            for sig in &a.actions {
                walk_sig(v, sig);
            }
        }
        ItemKind::Impl(im) => {
            v.visit_ty(&im.target);
            for a in &im.actions {
                walk_sig(v, &a.sig);
                if let Some(b) = &a.body {
                    for s in b {
                        v.visit_stmt(s);
                    }
                }
            }
        }
        ItemKind::Action(a) => {
            walk_sig(v, &a.sig);
            if let Some(b) = &a.body {
                for s in b {
                    v.visit_stmt(s);
                }
            }
        }
        ItemKind::Foreign(f) => {
            for it in &f.items {
                walk_sig(v, &it.sig);
            }
        }
    }
}

fn walk_sig<V: Visit + ?Sized>(v: &mut V, sig: &item::ActionSig) {
    for p in &sig.params {
        v.visit_ty(&p.ty);
    }
    if let Some(r) = &sig.ret {
        v.visit_ty(r);
    }
}

pub fn walk_stmt<V: Visit + ?Sized>(v: &mut V, s: &Stmt) {
    match &s.kind {
        StmtKind::OwnerBinding { ty, value, .. }
        | StmtKind::ConstBinding { ty, value, .. }
        | StmtKind::MagnetBinding {
            ty, target: value, ..
        }
        | StmtKind::MagnetMutBinding {
            ty, target: value, ..
        } => {
            if let Some(t) = ty {
                v.visit_ty(t);
            }
            v.visit_expr(value);
        }
        StmtKind::MagnetRetarget { magnet, target } => {
            v.visit_expr(magnet);
            v.visit_expr(target);
        }
        StmtKind::Assignment { place, value, .. } => {
            v.visit_expr(place);
            v.visit_expr(value);
        }
        StmtKind::Drop(e) | StmtKind::Panic(e) | StmtKind::Expr(e) => v.visit_expr(e),
        StmtKind::If {
            branches,
            else_branch,
        } => {
            for (cond, body) in branches {
                v.visit_expr(cond);
                for s in body {
                    v.visit_stmt(s);
                }
            }
            if let Some(e) = else_branch {
                for s in e {
                    v.visit_stmt(s);
                }
            }
        }
        StmtKind::While { cond, body } => {
            v.visit_expr(cond);
            for s in body {
                v.visit_stmt(s);
            }
        }
        StmtKind::Loop(b) | StmtKind::Raw(b) => {
            for s in b {
                v.visit_stmt(s);
            }
        }
        StmtKind::For {
            pat, iter, body, ..
        } => {
            v.visit_pat(pat);
            v.visit_expr(iter);
            for s in body {
                v.visit_stmt(s);
            }
        }
        StmtKind::Match { scrutinee, arms } => {
            v.visit_expr(scrutinee);
            for arm in arms {
                v.visit_pat(&arm.pat);
                if let Some(g) = &arm.guard {
                    v.visit_expr(g);
                }
                for s in &arm.body {
                    v.visit_stmt(s);
                }
            }
        }
        StmtKind::Return(Some(e)) => v.visit_expr(e),
        StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => {}
    }
}

pub fn walk_expr<V: Visit + ?Sized>(v: &mut V, e: &Expr) {
    match &e.kind {
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfRef | ExprKind::None => {}
        ExprKind::ChoiceCtor { args, .. } => {
            for a in args {
                v.visit_expr(a);
            }
        }
        ExprKind::Unary { operand, .. } => v.visit_expr(operand),
        ExprKind::Binary { lhs, rhs, .. } => {
            v.visit_expr(lhs);
            v.visit_expr(rhs);
        }
        ExprKind::Call { callee, args }
        | ExprKind::MethodCall {
            receiver: callee,
            args,
            ..
        } => {
            v.visit_expr(callee);
            for a in args {
                v.visit_expr(a);
            }
        }
        ExprKind::Field { receiver, .. } => v.visit_expr(receiver),
        ExprKind::Index { base, index } => {
            v.visit_expr(base);
            v.visit_expr(index);
        }
        ExprKind::Slice { base, lo, hi } => {
            v.visit_expr(base);
            if let Some(l) = lo {
                v.visit_expr(l);
            }
            if let Some(h) = hi {
                v.visit_expr(h);
            }
        }
        ExprKind::Cast { operand, target } => {
            v.visit_expr(operand);
            v.visit_ty(target);
        }
        ExprKind::Try(e) | ExprKind::MagnetValid(e) | ExprKind::Paren(e) => v.visit_expr(e),
        ExprKind::MagnetMeta { target, .. } => v.visit_expr(target),
        ExprKind::Range { lo, hi } => {
            v.visit_expr(lo);
            v.visit_expr(hi);
        }
    }
}

pub fn walk_pat<V: Visit + ?Sized>(_v: &mut V, p: &Pat) {
    match &p.kind {
        PatKind::Variant { binds, .. } => {
            for b in binds {
                let _ = b;
            }
        }
        PatKind::VariantNoPayload(_) | PatKind::Bind(_) | PatKind::Wild | PatKind::IntLit(_) => {}
    }
}

pub fn walk_ty<V: Visit + ?Sized>(v: &mut V, t: &Ty) {
    match &t.kind {
        TyKind::Named { args, .. } => {
            for a in args {
                v.visit_ty(a);
            }
        }
        TyKind::Borrow { inner, .. }
        | TyKind::Magnet { inner, .. }
        | TyKind::Address(inner)
        | TyKind::Span(inner)
        | TyKind::Maybe(inner) => v.visit_ty(inner),
        TyKind::Array { elem, .. } => {
            v.visit_ty(elem);
        }
        TyKind::Outcome { ok, err } => {
            v.visit_ty(ok);
            v.visit_ty(err);
        }
        TyKind::Tuple(ts) => {
            for t in ts {
                v.visit_ty(t);
            }
        }
        TyKind::SelfTy | TyKind::Unit | TyKind::Infer => {}
    }
}
