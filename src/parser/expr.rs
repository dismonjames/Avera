use crate::ast::expr::{BinOp, Expr, ExprKind, Lit, UnOp};
use crate::ast::path::Path;
use crate::lexer::token::{Keyword, TokenKind};
use crate::parser::Parser;

// TODO: bổ sung thêm operator precedence cho Pratt parser nếu thêm toán tử mới
    // dbg!(&lhs);
    // TODO: bo sung precedence cho binary operators neu them op moi
    pub fn parse_expr(p: &mut Parser) -> Expr {
    parse_binary(p, 1)
}

fn parse_binary(p: &mut Parser, min_prec: u8) -> Expr {
    let mut lhs = parse_unary(p);
    loop {
        // newline-suppressed: continuation operators keep going
        p.skip_newlines();
        let Some(op) = match_binop(p) else { break };
        let prec = op.precedence();
        if prec < min_prec {
            break;
        }
        p.bump();
        p.skip_newlines();
        let rhs = parse_binary(p, prec + 1);
        let span = crate::diagnostics::span::Span::union(lhs.span, rhs.span);
        lhs = Expr::new(
            span,
            ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        );
    }
    lhs
}

fn match_binop(p: &Parser) -> Option<BinOp> {
    Some(match &p.current().kind {
        TokenKind::Plus => BinOp::Add,
        TokenKind::Minus => BinOp::Sub,
        TokenKind::Star => BinOp::Mul,
        TokenKind::Slash => BinOp::Div,
        TokenKind::Percent => BinOp::Mod,
        TokenKind::EqEq => BinOp::Eq,
        TokenKind::NotEq => BinOp::Ne,
        TokenKind::Lt => BinOp::Lt,
        TokenKind::LtEq => BinOp::Le,
        TokenKind::Gt => BinOp::Gt,
        TokenKind::GtEq => BinOp::Ge,
        TokenKind::AmpAmp => BinOp::And,
        TokenKind::PipePipe => BinOp::Or,
        TokenKind::Amp => BinOp::BitAnd,
        TokenKind::Pipe => BinOp::BitOr,
        TokenKind::Caret => BinOp::BitXor,
        TokenKind::Shl => BinOp::Shl,
        TokenKind::Shr => BinOp::Shr,
        _ => return None,
    })
}

fn parse_unary(p: &mut Parser) -> Expr {
    let start = p.current_span().start;
    match &p.current().kind {
        TokenKind::Bang => {
            p.bump();
            let operand = Box::new(parse_unary(p));
            Expr::new(
                p.span_from(start),
                ExprKind::Unary {
                    op: UnOp::Not,
                    operand,
                },
            )
        }
        TokenKind::Minus => {
            p.bump();
            let operand = Box::new(parse_unary(p));
            Expr::new(
                p.span_from(start),
                ExprKind::Unary {
                    op: UnOp::Neg,
                    operand,
                },
            )
        }
        TokenKind::Amp => {
            p.bump();
            let op = if p.eat(TokenKind::Bang) {
                UnOp::BorrowMut
            } else {
                UnOp::Borrow
            };
            let operand = Box::new(parse_unary(p));
            Expr::new(p.span_from(start), ExprKind::Unary { op, operand })
        }
        TokenKind::Caret => {
            p.bump();
            let operand = Box::new(parse_unary(p));
            Expr::new(
                p.span_from(start),
                ExprKind::Unary {
                    op: UnOp::Move,
                    operand,
                },
            )
        }
        TokenKind::Tilde => {
            // `~m` — magnet reference. If followed by `.property`, create
            // a MagnetMeta expression (e.g. `~m.address`).
            p.bump();
            // Parse just the bare identifier (not a dotted path), so we
            // can intercept `.address` ourselves.
            let id_start = p.current_span().start;
            let (name, _) = p.expect_ident("magnet name");
            let target = Expr::new(
                p.span_from(id_start),
                ExprKind::Path(Path::single(p.span_from(id_start), name)),
            );
            if p.eat(TokenKind::Dot) {
                let (prop, _) = p.expect_ident("magnet property name");
                let span = p.span_from(start);
                Expr::new(
                    span,
                    ExprKind::MagnetMeta {
                        target: Box::new(target),
                        property: prop,
                    },
                )
            } else {
                // Bare `~m` — just a reference to the magnet local.
                target
            }
        }
        _ => parse_postfix(p),
    }
}

fn parse_postfix(p: &mut Parser) -> Expr {
    let mut e = parse_primary(p);
    loop {
        match &p.current().kind {
            TokenKind::Dot => {
                p.bump();
                // Could be a method call `obj.method(args)`, a field `obj.field`,
                // or a magnet meta `~obj.address`.
                let meta = p.eat(TokenKind::Tilde);
                let (name, _) = p.expect_ident("field or method name");
                if meta {
                    let span =
                        crate::diagnostics::span::Span::union(e.span, p.span_from(e.span.start));
                    e = Expr::new(
                        span,
                        ExprKind::MagnetMeta {
                            target: Box::new(e),
                            property: name,
                        },
                    );
                } else if p.at(TokenKind::LParen) {
                    p.bump();
                    p.skip_newlines();
                    let args = parse_args(p);
                    let span =
                        crate::diagnostics::span::Span::union(e.span, p.span_from(e.span.start));
                    e = Expr::new(
                        span,
                        ExprKind::MethodCall {
                            receiver: Box::new(e),
                            method: name,
                            args,
                        },
                    );
                } else {
                    let span =
                        crate::diagnostics::span::Span::union(e.span, p.span_from(e.span.start));
                    e = Expr::new(
                        span,
                        ExprKind::Field {
                            receiver: Box::new(e),
                            name,
                        },
                    );
                }
            }
            TokenKind::LBracket => {
                p.bump();
                p.skip_newlines();
                // slice `a..b` vs index `i`
                let lo = if p.at(TokenKind::DotDot) {
                    None
                } else {
                    Some(Box::new(parse_expr(p)))
                };
                if p.eat(TokenKind::DotDot) {
                    p.skip_newlines();
                    let hi = if p.at(TokenKind::RBracket) {
                        None
                    } else {
                        Some(Box::new(parse_expr(p)))
                    };
                    p.expect(TokenKind::RBracket, "`]`");
                    let span =
                        crate::diagnostics::span::Span::union(e.span, p.span_from(e.span.start));
                    e = Expr::new(
                        span,
                        ExprKind::Slice {
                            base: Box::new(e),
                            lo,
                            hi,
                        },
                    );
                } else {
                    // No slice — `lo` is the index expression.
                    let index = lo.expect("index expression");
                    p.expect(TokenKind::RBracket, "`]`");
                    let span =
                        crate::diagnostics::span::Span::union(e.span, p.span_from(e.span.start));
                    e = Expr::new(
                        span,
                        ExprKind::Index {
                            base: Box::new(e),
                            index,
                        },
                    );
                }
            }
            TokenKind::LParen => {
                p.bump();
                p.skip_newlines();
                let args = parse_args(p);
                let span = crate::diagnostics::span::Span::union(e.span, p.span_from(e.span.start));
                e = Expr::new(
                    span,
                    ExprKind::Call {
                        callee: Box::new(e),
                        args,
                    },
                );
            }
            TokenKind::Cast => {
                p.bump();
                p.skip_newlines();
                let target = super::ty::parse_ty(p);
                let span = crate::diagnostics::span::Span::union(e.span, target.span);
                e = Expr::new(
                    span,
                    ExprKind::Cast {
                        operand: Box::new(e),
                        target,
                    },
                );
            }
            TokenKind::Question => {
                // `expr?` — try propagation. Could also be magnet validity
                // test when the receiver is a bare magnet name; the checker
                // distinguishes by type. We emit `Try` for now.
                p.bump();
                let span = crate::diagnostics::span::Span::union(e.span, p.span_from(e.span.start));
                e = Expr::new(span, ExprKind::Try(Box::new(e)));
            }
            TokenKind::Bang => {
                // `expr!` — explicit early drop. This is a statement, not an
                // expression postfix; we don't consume it here so the stmt
                // parser can recognise it.
                break;
            }
            _ => break,
        }
    }
    e
}

fn parse_args(p: &mut Parser) -> Vec<Expr> {
    let _start = p.current_span().start;
    let mut args = Vec::new();
    if !p.at(TokenKind::RParen) {
        args.push(parse_named_or_positional_arg(p));
        while p.eat(TokenKind::Comma) {
            p.skip_newlines();
            if p.at(TokenKind::RParen) {
                break;
            }
            args.push(parse_named_or_positional_arg(p));
        }
    }
    p.skip_newlines();
    p.expect(TokenKind::RParen, "`)`");
    args
}

fn parse_named_or_positional_arg(p: &mut Parser) -> Expr {
    // peek for `name = expr`
    if let TokenKind::Ident(_) = &p.current().kind {
        let saved = p.pos;
        p.bump();
        p.skip_newlines();
        if p.eat(TokenKind::Eq) {
            p.skip_newlines();
            // named arg — we lose the name for plain calls; that's fine
            // because function calls are positional. Shape construction is
            // detected by the callee being a shape type path, handled by
            // the resolver/type-checker via Call lowering.
            return parse_expr(p);
        }
        p.pos = saved;
    }
    parse_expr(p)
}

fn parse_primary(p: &mut Parser) -> Expr {
    let start = p.current_span().start;
    let kind = match &p.current().kind {
        TokenKind::IntLit(s) => {
            let s = s.clone();
            p.bump();
            ExprKind::Literal(Lit::Int(s))
        }
        TokenKind::FloatLit(s) => {
            let s = s.clone();
            p.bump();
            ExprKind::Literal(Lit::Float(s))
        }
        TokenKind::StrLit(s) => {
            let s = s.clone();
            p.bump();
            ExprKind::Literal(Lit::Str(s))
        }
        TokenKind::CharLit(c) => {
            let c = *c;
            p.bump();
            ExprKind::Literal(Lit::Char(c))
        }
        TokenKind::Kw(Keyword::True) => {
            p.bump();
            ExprKind::Literal(Lit::Bool(true))
        }
        TokenKind::Kw(Keyword::False) => {
            p.bump();
            ExprKind::Literal(Lit::Bool(false))
        }
        TokenKind::Kw(Keyword::None) => {
            p.bump();
            ExprKind::None
        }
        TokenKind::Kw(Keyword::SelfKw) => {
            p.bump();
            ExprKind::SelfRef
        }
        TokenKind::Dot => {
            // `.some(...)`, `.none`, `.some(value)` — choice construction.
            p.bump();
            // Handle `none` keyword as a variant.
            let variant = if p.at_kw(crate::lexer::token::Keyword::None) {
                p.bump();
                "none".to_string()
            } else {
                let (v, _) = p.expect_ident("variant name");
                v
            };
            let args = if p.at(TokenKind::LParen) {
                p.bump();
                p.skip_newlines();
                parse_args(p)
            } else {
                Vec::new()
            };
            ExprKind::ChoiceCtor { variant, args }
        }
        TokenKind::LParen => {
            p.bump();
            p.skip_newlines();
            if p.eat(TokenKind::RParen) {
                ExprKind::Literal(Lit::Unit)
            } else {
                let inner = parse_expr(p);
                p.skip_newlines();
                p.expect(TokenKind::RParen, "`)`");
                ExprKind::Paren(Box::new(inner))
            }
        }
        TokenKind::LBracket => {
            // array literal `[a, b, c]`
            p.bump();
            p.skip_newlines();
            let mut elems = Vec::new();
            if !p.at(TokenKind::RBracket) {
                elems.push(parse_expr(p));
                while p.eat(TokenKind::Comma) {
                    p.skip_newlines();
                    if p.at(TokenKind::RBracket) {
                        break;
                    }
                    elems.push(parse_expr(p));
                }
            }
            p.skip_newlines();
            p.expect(TokenKind::RBracket, "`]`");
            // represent as a call to a builtin array constructor
            ExprKind::Call {
                callee: Box::new(Expr::new(
                    p.span_from(start),
                    ExprKind::Path(Path::single(p.span_from(start), "@array")),
                )),
                args: elems,
            }
        }
        TokenKind::Ident(_) => {
            // Path: possibly dotted, possibly generic instantiation, possibly
            // a choice construction `Maybe<I32>.some(...)`.
            let path = parse_value_path(p);
            // generic instantiation `Foo<I32>(...)` -> Call with type args
            // is handled by the postfix `( ... )` below via Call.
            // Choice construction `Path.variant(args)` — we look ahead after
            // the path for `.variant`.
            ExprKind::Path(path)
        }
        other => {
            let span = p.current_span();
            p.error(
                span,
                crate::diagnostics::diagnostic::DiagnosticKind::Parse,
                format!("expected an expression but found `{}`", other),
            );
            p.bump();
            ExprKind::Literal(Lit::Unit)
        }
    };
    // Special case: `Path.variant(args)` is choice construction. Detect
    // here so the rest of postfix sees a normal `Field` then `(` as a
    // method call on a type path — we want it as ChoiceCtor instead.
    if let ExprKind::Path(path) = &kind {
        if path.segments.len() >= 2 && p.at(TokenKind::Dot) {
            // peek: `.name` then `(`?
            let saved = p.pos;
            p.bump(); // dot
            if let TokenKind::Ident(name) = &p.current().kind {
                let name = name.clone();
                let next = p.peek(1);
                let is_call = matches!(next.kind, TokenKind::LParen);
                let is_no_payload = matches!(
                    next.kind,
                    TokenKind::Newline
                        | TokenKind::Eof
                        | TokenKind::Semicolon
                        | TokenKind::Comma
                        | TokenKind::RParen
                        | TokenKind::RBracket
                );
                if is_call || is_no_payload {
                    // split path into type path + variant
                    let mut segs = path.segments.clone();
                    let _ = segs.pop();
                    let type_path = Path::new(
                        crate::diagnostics::span::Span::new(
                            path.span.file,
                            path.span.start,
                            p.prev_end(),
                        ),
                        segs,
                    );
                    let args = if is_call {
                        p.bump(); // consume ident
                        p.bump(); // (
                        p.skip_newlines();
                        parse_args(p)
                    } else {
                        p.bump(); // consume ident
                        Vec::new()
                    };
                    let span = p.span_from(start);
                    let _ = type_path;
                    return Expr::new(
                        span,
                        ExprKind::ChoiceCtor {
                            variant: name,
                            args,
                        },
                    );
                }
            }
            p.pos = saved;
        }
    }
    Expr::new(p.span_from(start), kind)
}

pub fn parse_value_path(p: &mut Parser) -> Path {
    let start = p.current_span().start;
    let (first, _) = p.expect_ident("identifier");
    let mut segments = vec![first];
    while p.at(TokenKind::Dot) {
        // only continue if next is an identifier (not `~` magnet meta, that's postfix)
        if let TokenKind::Ident(_) = &p.peek(1).kind {
            p.bump();
            let (seg, _) = p.expect_ident("path segment");
            segments.push(seg);
        } else {
            break;
        }
    }
    Path::new(p.span_from(start), segments)
}
