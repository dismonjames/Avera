use crate::ast::expr::{Expr, ExprKind, Lit};
use crate::ast::path::Path;
use crate::ast::ty::{Ty, TyKind};
use crate::lexer::token::{Keyword, TokenKind};
use crate::parser::Parser;

pub fn parse_ty(p: &mut Parser) -> Ty {
    parse_ty_prec(p)
}

fn parse_ty_prec(p: &mut Parser) -> Ty {
    p.skip_newlines();
    let start = p.current_span().start;
    let kind = match &p.current().kind {
        TokenKind::Amp => {
            p.bump();
            let is_mut = p.eat(TokenKind::Bang);
            p.skip_newlines();
            let inner = Box::new(parse_ty_prec(p));
            TyKind::Borrow { is_mut, inner }
        }
        TokenKind::Kw(Keyword::SelfKw) => {
            p.bump();
            TyKind::SelfTy
        }
        TokenKind::LBracket => {
            p.bump();
            p.skip_newlines();
            let elem = Box::new(parse_ty_prec(p));
            let size = if p.eat(TokenKind::Semicolon) {
                p.skip_newlines();
                Some(Box::new(expr::parse_expr(p)))
            } else {
                None
            };
            p.expect(TokenKind::RBracket, "`]`");
            // Array requires a size expression; if absent, error but keep going.
            let size = match size {
                Some(s) => s,
                None => {
                    let span = p.span_from(start);
                    p.error(
                        span,
                        crate::diagnostics::diagnostic::DiagnosticKind::Parse,
                        "array type requires a size `[T; N]`",
                    );
                    Box::new(Expr::new(span, ExprKind::Literal(Lit::Int("0".into()))))
                }
            };
            TyKind::Array { elem, size }
        }
        TokenKind::LParen => {
            // tuple or unit
            p.bump();
            p.skip_newlines();
            if p.eat(TokenKind::RParen) {
                TyKind::Unit
            } else {
                let mut elems = Vec::new();
                elems.push(parse_ty_prec(p));
                while p.eat(TokenKind::Comma) {
                    p.skip_newlines();
                    if p.at(TokenKind::RParen) {
                        break;
                    }
                    elems.push(parse_ty_prec(p));
                }
                p.skip_newlines();
                p.expect(TokenKind::RParen, "`)`");
                TyKind::Tuple(elems)
            }
        }
        TokenKind::Ident(name) => {
            let name = name.clone();
            let path = parse_named_path(p, name);
            // generic args
            let args = if p.at(TokenKind::Lt) {
                p.bump();
                p.skip_newlines();
                let mut a = Vec::new();
                a.push(parse_ty_prec(p));
                while p.eat(TokenKind::Comma) {
                    p.skip_newlines();
                    if p.at(TokenKind::Gt) {
                        break;
                    }
                    a.push(parse_ty_prec(p));
                }
                p.skip_newlines();
                p.expect(TokenKind::Gt, "`>`");
                a
            } else {
                Vec::new()
            };
            // Specialise common stdlib type constructors.
            let segs = &path.segments;
            let last = segs.last().map(|s| s.as_str()).unwrap_or("");
            let nargs = args.len();
            match (last, nargs) {
                ("Magnet", 1) => TyKind::Magnet {
                    is_mut: false,
                    inner: Box::new(args.into_iter().next().unwrap()),
                },
                ("Address", 1) => TyKind::Address(Box::new(args.into_iter().next().unwrap())),
                ("Span", 1) => TyKind::Span(Box::new(args.into_iter().next().unwrap())),
                ("Maybe", 1) => TyKind::Maybe(Box::new(args.into_iter().next().unwrap())),
                ("Outcome", 2) => {
                    let mut it = args.into_iter();
                    TyKind::Outcome {
                        ok: Box::new(it.next().unwrap()),
                        err: Box::new(it.next().unwrap()),
                    }
                }
                _ => TyKind::Named { path, args },
            }
        }
        other => {
            let span = p.current_span();
            p.error(
                span,
                crate::diagnostics::diagnostic::DiagnosticKind::Parse,
                format!("expected a type but found `{}`", other),
            );
            // consume to avoid infinite loop
            p.bump();
            TyKind::Infer
        }
    };
    Ty::new(p.span_from(start), kind)
}

fn parse_named_path(p: &mut Parser, first: String) -> Path {
    let start = p.current_span().start;
    // We are positioned at the identifier; back up isn't possible, so
    // read it as the first segment via expect_ident (it's still current).
    let _ = p.bump(); // consume the identifier token
    let mut segments = vec![first];
    while p.at(TokenKind::Dot) {
        p.bump();
        let (seg, _) = p.expect_ident("path segment");
        segments.push(seg);
    }
    Path::new(p.span_from(start), segments)
}

// bring the expr parser into scope for the array-size case
use super::expr;
