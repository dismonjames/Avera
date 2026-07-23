use crate::ast::expr::{BinOp, Expr, Lit, UnOp};
use crate::ast::item::ActionSig;
use crate::ast::stmt::{AssignOp, Stmt, StmtKind};
use crate::ast::{Directive, DirectiveKind, Item, ItemKind, Module};
use std::fmt::Write;

pub fn format_module(m: &Module) -> String {
    let mut out = String::new();
    for d in &m.directives {
        format_directive(&mut out, d);
    }
    for (i, item) in m.items.iter().enumerate() {
        if i > 0 || !m.directives.is_empty() {
            out.push('\n');
        }
        format_item(&mut out, item);
    }
    out
}

fn format_directive(out: &mut String, d: &Directive) {
    match &d.kind {
        DirectiveKind::Import { path, alias } => {
            let _ = write!(out, "#{}", path.as_str());
            if let Some(a) = alias {
                let _ = write!(out, " as {}", a);
            }
            out.push('\n');
        }
        DirectiveKind::Module(p) => {
            let _ = writeln!(out, "#module {}", p.as_str());
        }
        DirectiveKind::Header(s) => {
            let _ = writeln!(out, "#header \"{}\"", s);
        }
        DirectiveKind::Source(s) => {
            let _ = writeln!(out, "#source \"{}\"", s);
        }
        DirectiveKind::Depends(p) => {
            let _ = writeln!(out, "#depends {}", p.as_str());
        }
        DirectiveKind::Cfg(s) => {
            let _ = writeln!(out, "#cfg {}", s);
        }
        DirectiveKind::Target(s) => {
            let _ = writeln!(out, "#target {}", s);
        }
        DirectiveKind::Link(s) => {
            let _ = writeln!(out, "#link \"{}\"", s);
        }
        DirectiveKind::Other(name, args) => {
            let _ = write!(out, "#{}", name);
            for a in args {
                let _ = write!(out, " {}", a);
            }
            out.push('\n');
        }
    }
}

fn format_item(out: &mut String, item: &Item) {
    for a in &item.attrs {
        let _ = write!(out, "@{}", a.name);
        if !a.args.is_empty() {
            let _ = write!(out, "({})", a.args.join(", "));
        }
        out.push('\n');
    }
    if item.exported {
        out.push_str("export ");
    }
    match &item.kind {
        ItemKind::Shape(s) => {
            let _ = write!(out, "shape {}", s.name);
            if !s.generics.is_empty() {
                let _ = write!(out, "<{}>", s.generics.join(", "));
            }
            out.push_str(" will\n");
            for f in &s.fields {
                let _ = writeln!(out, "    {}: {}", f.name, format_ty(&f.ty));
            }
            out.push_str("finish\n");
        }
        ItemKind::Choice(c) => {
            let _ = write!(out, "choice {}", c.name);
            if !c.generics.is_empty() {
                let _ = write!(out, "<{}>", c.generics.join(", "));
            }
            out.push_str(" will\n");
            for v in &c.variants {
                if let Some(p) = &v.payload {
                    let p: Vec<String> = p
                        .iter()
                        .map(|(n, t)| format!("{}: {}", n, format_ty(t)))
                        .collect();
                    let _ = writeln!(out, "    {}({})", v.name, p.join(", "));
                } else {
                    let _ = writeln!(out, "    {}", v.name);
                }
            }
            out.push_str("finish\n");
        }
        ItemKind::Ability(a) => {
            let _ = write!(out, "ability {}", a.name);
            if !a.generics.is_empty() {
                let _ = write!(out, "<{}>", a.generics.join(", "));
            }
            out.push_str(" will\n");
            for sig in &a.actions {
                let _ = writeln!(out, "    {}", format_sig(sig));
            }
            out.push_str("finish\n");
        }
        ItemKind::Impl(im) => {
            out.push_str("implement ");
            if let Some(ab) = &im.ability {
                let _ = write!(out, "{} for ", ab.as_str());
            }
            let _ = write!(out, "{}", format_ty(&im.target));
            out.push_str(" will\n");
            for a in &im.actions {
                let _ = write!(out, "    {}", format_sig(&a.sig));
                if let Some(body) = &a.body {
                    out.push_str(" will\n");
                    format_block(out, body, 2);
                    out.push_str("    finish\n");
                } else {
                    out.push('\n');
                }
            }
            out.push_str("finish\n");
        }
        ItemKind::Action(a) => {
            let _ = write!(out, "{}", format_sig(&a.sig));
            if let Some(body) = &a.body {
                out.push_str(" will\n");
                format_block(out, body, 1);
                out.push_str("finish\n");
            } else {
                out.push('\n');
            }
        }
        ItemKind::Foreign(f) => {
            let _ = write!(out, "foreign {}", f.abi);
            if let Some(lib) = &f.lib {
                let _ = write!(out, "(\"{}\")", lib);
            }
            out.push_str(" will\n");
            for it in &f.items {
                if it.raw {
                    out.push_str("    raw ");
                } else {
                    out.push_str("    ");
                }
                // format_sig already emits the `action` keyword prefix.
                let _ = writeln!(out, "{}", format_sig(&it.sig));
            }
            out.push_str("finish\n");
        }
    }
}

fn format_block(out: &mut String, stmts: &[Stmt], indent: usize) {
    for s in stmts {
        format_stmt(out, s, indent);
    }
}

fn format_stmt(out: &mut String, s: &Stmt, indent: usize) {
    let pad = "    ".repeat(indent);
    match &s.kind {
        StmtKind::OwnerBinding { name, ty, value } => {
            let _ = write!(out, "{}:{}", pad, name);
            if let Some(t) = ty {
                let _ = write!(out, ": {}", format_ty(t));
            }
            let _ = writeln!(out, " = {}", format_expr(value));
        }
        StmtKind::ConstBinding { name, ty, value } => {
            let _ = write!(out, "{}::{}", pad, name);
            if let Some(t) = ty {
                let _ = write!(out, ": {}", format_ty(t));
            }
            let _ = writeln!(out, " = {}", format_expr(value));
        }
        StmtKind::MagnetBinding { name, ty, target } => {
            let _ = write!(out, "{}~{}", pad, name);
            if let Some(t) = ty {
                let _ = write!(out, ": {}", format_ty(t));
            }
            let _ = writeln!(out, " = {}", format_expr(target));
        }
        StmtKind::MagnetMutBinding { name, ty, target } => {
            let _ = write!(out, "{}~!{}", pad, name);
            if let Some(t) = ty {
                let _ = write!(out, ": {}", format_ty(t));
            }
            let _ = writeln!(out, " = {}", format_expr(target));
        }
        StmtKind::MagnetRetarget { magnet, target } => {
            let _ = writeln!(
                out,
                "{}{} -> {}",
                pad,
                format_expr(magnet),
                format_expr(target)
            );
        }
        StmtKind::Assignment { place, op, value } => {
            let o = match op {
                AssignOp::Assign => "=",
                AssignOp::Add => "+=",
                AssignOp::Sub => "-=",
                AssignOp::Mul => "*=",
                AssignOp::Div => "/=",
                AssignOp::Mod => "%=",
                AssignOp::BitAnd => "&=",
                AssignOp::BitOr => "|=",
                AssignOp::BitXor => "^=",
                AssignOp::Shl => "<<=",
                AssignOp::Shr => ">>=",
            };
            let _ = writeln!(
                out,
                "{}{} {} {}",
                pad,
                format_expr(place),
                o,
                format_expr(value)
            );
        }
        StmtKind::Drop(e) => {
            let _ = writeln!(out, "{}{}!", pad, format_expr(e));
        }
        StmtKind::If {
            branches,
            else_branch,
        } => {
            let _first = true;
            for (i, (cond, body)) in branches.iter().enumerate() {
                if i == 0 {
                    let _ = writeln!(out, "{}if {} will", pad, format_expr(cond));
                } else {
                    let _ = writeln!(out, "{}else if {} will", pad, format_expr(cond));
                }
                format_block(out, body, indent + 1);
            }
            if let Some(eb) = else_branch {
                let _ = writeln!(out, "{}else", pad);
                format_block(out, eb, indent + 1);
            }
            let _ = writeln!(out, "{}finish", pad);
        }
        StmtKind::While { cond, body } => {
            let _ = writeln!(out, "{}while {} will", pad, format_expr(cond));
            format_block(out, body, indent + 1);
            let _ = writeln!(out, "{}finish", pad);
        }
        StmtKind::Loop(body) => {
            let _ = writeln!(out, "{}loop will", pad);
            format_block(out, body, indent + 1);
            let _ = writeln!(out, "{}finish", pad);
        }
        StmtKind::For {
            mode,
            pat,
            iter,
            body,
        } => {
            let m = match mode {
                crate::ast::stmt::ForMode::Read => "",
                crate::ast::stmt::ForMode::BorrowMut => "&!",
                crate::ast::stmt::ForMode::Move => "^",
            };
            let _ = writeln!(
                out,
                "{}for {}{} in {} will",
                pad,
                m,
                format_pat(pat),
                format_expr(iter)
            );
            format_block(out, body, indent + 1);
            let _ = writeln!(out, "{}finish", pad);
        }
        StmtKind::Match { scrutinee, arms } => {
            let _ = writeln!(out, "{}match {} will", pad, format_expr(scrutinee));
            for arm in arms {
                let p = format_pat(&arm.pat);
                if let Some(g) = &arm.guard {
                    let _ = writeln!(out, "{}    {} if {} =>", pad, p, format_expr(g));
                } else {
                    let _ = writeln!(out, "{}    {} =>", pad, p);
                }
                // body: each stmt on its own line, indented one more level.
                format_block(out, &arm.body, indent + 2);
            }
            let _ = writeln!(out, "{}finish", pad);
        }
        StmtKind::Return(e) => {
            if let Some(e) = e {
                let _ = writeln!(out, "{}return {}", pad, format_expr(e));
            } else {
                let _ = writeln!(out, "{}return", pad);
            }
        }
        StmtKind::Break => {
            let _ = writeln!(out, "{}break", pad);
        }
        StmtKind::Continue => {
            let _ = writeln!(out, "{}continue", pad);
        }
        StmtKind::Panic(e) => {
            let _ = writeln!(out, "{}panic({})", pad, format_expr(e));
        }
        StmtKind::Expr(e) => {
            let _ = writeln!(out, "{}{}", pad, format_expr(e));
        }
        StmtKind::Raw(body) => {
            let _ = writeln!(out, "{}raw will", pad);
            format_block(out, body, indent + 1);
            let _ = writeln!(out, "{}finish", pad);
        }
    }
}

fn format_sig(sig: &ActionSig) -> String {
    let mut s = String::new();
    // Methods are written `action Type.method(params)` — the `action` keyword
    // comes first, then the receiver type, then the method name.
    let _ = write!(s, "action ");
    if let Some(recv) = &sig.receiver {
        let _ = write!(s, "{}.", recv.as_str());
    }
    let _ = write!(s, "{}", sig.name);
    let params: Vec<String> = sig.params.iter().map(format_param).collect();
    let _ = write!(s, "({})", params.join(", "));
    if let Some(r) = &sig.ret {
        let _ = write!(s, ": {}", format_ty(r));
    }
    s
}

fn format_param(p: &crate::ast::item::Param) -> String {
    let prefix = match p.mode {
        crate::ast::item::ParamMode::Borrow => "&",
        crate::ast::item::ParamMode::BorrowMut => "&!",
        crate::ast::item::ParamMode::Move => "^",
        _ => "",
    };
    format!("{}{}: {}", prefix, p.name, format_ty(&p.ty))
}

fn format_ty(t: &crate::ast::ty::Ty) -> String {
    use crate::ast::ty::TyKind;
    match &t.kind {
        TyKind::Named { path, args } => {
            let mut s = path.as_str();
            if !args.is_empty() {
                let a: Vec<String> = args.iter().map(format_ty).collect();
                s.push_str(&format!("<{}>", a.join(", ")));
            }
            s
        }
        TyKind::Borrow { is_mut, inner } => {
            if *is_mut {
                format!("&!{}", format_ty(inner))
            } else {
                format!("&{}", format_ty(inner))
            }
        }
        TyKind::Magnet { is_mut, inner } => {
            if *is_mut {
                format!("MagnetMut<{}>", format_ty(inner))
            } else {
                format!("Magnet<{}>", format_ty(inner))
            }
        }
        TyKind::Address(inner) => format!("Address<{}>", format_ty(inner)),
        TyKind::Array { elem, size } => {
            format!("[{}; {}]", format_ty(elem), format_expr(size))
        }
        TyKind::Span(inner) => format!("Span<{}>", format_ty(inner)),
        TyKind::Maybe(inner) => format!("Maybe<{}>", format_ty(inner)),
        TyKind::Outcome { ok, err } => format!("Outcome<{}, {}>", format_ty(ok), format_ty(err)),
        TyKind::SelfTy => "Self".into(),
        TyKind::Unit => "Unit".into(),
        TyKind::Infer => "_".into(),
        TyKind::Tuple(ts) => {
            let a: Vec<String> = ts.iter().map(format_ty).collect();
            format!("({})", a.join(", "))
        }
    }
}

fn format_pat(p: &crate::ast::pat::Pat) -> String {
    use crate::ast::pat::PatKind;
    match &p.kind {
        PatKind::Variant { name, binds } => {
            let b: Vec<String> = binds.iter().map(|b| b.name.clone()).collect();
            format!(".{}({})", name, b.join(", "))
        }
        PatKind::VariantNoPayload(name) => format!(".{}", name),
        PatKind::Bind(name) => name.clone(),
        PatKind::Wild => "_".into(),
        PatKind::IntLit(v) => v.to_string(),
    }
}

fn format_expr(e: &Expr) -> String {
    use crate::ast::expr::ExprKind;
    match &e.kind {
        ExprKind::Literal(Lit::Int(s)) => s.clone(),
        ExprKind::Literal(Lit::Float(s)) => s.clone(),
        ExprKind::Literal(Lit::Str(s)) => format!("\"{}\"", s),
        ExprKind::Literal(Lit::Char(c)) => format!("'{}'", c),
        ExprKind::Literal(Lit::Bool(b)) => b.to_string(),
        ExprKind::Literal(Lit::Unit) => "()".into(),
        ExprKind::Path(p) => p.as_str(),
        ExprKind::SelfRef => "self".into(),
        ExprKind::None => "none".into(),
        ExprKind::ChoiceCtor { variant, args } => {
            let a: Vec<String> = args.iter().map(format_expr).collect();
            format!(".{}({})", variant, a.join(", "))
        }
        ExprKind::Unary { op, operand } => {
            let o = match op {
                UnOp::Not => "!",
                UnOp::Neg => "-",
                UnOp::Borrow => "&",
                UnOp::BorrowMut => "&!",
                UnOp::Move => "^",
            };
            format!("{}{}", o, format_expr(operand))
        }
        ExprKind::Binary { op, lhs, rhs } => {
            format!(
                "{} {} {}",
                format_expr(lhs),
                binop_str(*op),
                format_expr(rhs)
            )
        }
        ExprKind::Call { callee, args } => {
            // Array literal is parsed as `@array(a, b, c)`; format it back as
            // `[a, b, c]` so the formatter is idempotent on `[...]` sources.
            if let ExprKind::Path(p) = &callee.kind {
                if p.segments.len() == 1 && p.segments[0] == "@array" {
                    let a: Vec<String> = args.iter().map(format_expr).collect();
                    return format!("[{}]", a.join(", "));
                }
            }
            let a: Vec<String> = args.iter().map(format_expr).collect();
            format!("{}({})", format_expr(callee), a.join(", "))
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            let a: Vec<String> = args.iter().map(format_expr).collect();
            format!("{}.{}({})", format_expr(receiver), method, a.join(", "))
        }
        ExprKind::Field { receiver, name } => {
            format!("{}.{}", format_expr(receiver), name)
        }
        ExprKind::Index { base, index } => {
            format!("{}[{}]", format_expr(base), format_expr(index))
        }
        ExprKind::Slice { base, lo, hi } => {
            let l = lo.as_ref().map(|e| format_expr(e)).unwrap_or_default();
            let h = hi.as_ref().map(|e| format_expr(e)).unwrap_or_default();
            format!("{}[{}..{}]", format_expr(base), l, h)
        }
        ExprKind::Cast { operand, target } => {
            format!("{} :> {}", format_expr(operand), format_ty(target))
        }
        ExprKind::Try(e) => format!("{}?", format_expr(e)),
        ExprKind::MagnetValid(e) => format!("{}?", format_expr(e)),
        ExprKind::MagnetMeta { target, property } => {
            format!("~{}.{}", format_expr(target), property)
        }
        ExprKind::Paren(e) => format!("({})", format_expr(e)),
        ExprKind::Range { lo, hi } => format!("{}..{}", format_expr(lo), format_expr(hi)),
    }
}

fn binop_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::BitXor => "^",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
    }
}
