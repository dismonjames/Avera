use crate::ast::pat::{Pat, PatBind, PatKind};
use crate::lexer::token::TokenKind;
use crate::parser::Parser;

pub fn parse_pat(p: &mut Parser) -> Pat {
    let start = p.current_span().start;
    let kind = match &p.current().kind {
        TokenKind::Dot => {
            p.bump();
            // Handle `none` keyword as a variant pattern.
            if p.at_kw(crate::lexer::token::Keyword::None) {
                p.bump();
                return Pat::new(
                    p.span_from(start),
                    PatKind::VariantNoPayload("none".to_string()),
                );
            }
            let (name, nspan) = p.expect_ident("variant name");
            if p.eat(TokenKind::LParen) {
                p.skip_newlines();
                let mut binds = Vec::new();
                if !p.at(TokenKind::RParen) {
                    binds.push(parse_pat_bind(p));
                    while p.eat(TokenKind::Comma) {
                        p.skip_newlines();
                        if p.at(TokenKind::RParen) {
                            break;
                        }
                        binds.push(parse_pat_bind(p));
                    }
                }
                p.skip_newlines();
                p.expect(TokenKind::RParen, "`)`");
                PatKind::Variant { name, binds }
            } else {
                let _ = nspan;
                PatKind::VariantNoPayload(name)
            }
        }
        TokenKind::Ident(name) => {
            let name = name.clone();
            p.bump();
            if name == "_" {
                PatKind::Wild
            } else {
                PatKind::Bind(name)
            }
        }
        TokenKind::IntLit(s) => {
            let s = s.clone();
            p.bump();
            let s = s.trim_start_matches('_').replace('_', "");
            let v = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                i64::from_str_radix(h, 16).unwrap_or(0)
            } else if let Some(b) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
                i64::from_str_radix(b, 2).unwrap_or(0)
            } else {
                s.parse().unwrap_or(0)
            };
            PatKind::IntLit(v)
        }
        _ => {
            // wildcard `_` is lexed as an identifier `_`.
            let span = p.current_span();
            p.error(
                span,
                crate::diagnostics::diagnostic::DiagnosticKind::Parse,
                "expected a pattern",
            );
            p.bump();
            PatKind::Wild
        }
    };
    Pat::new(p.span_from(start), kind)
}

fn parse_pat_bind(p: &mut Parser) -> PatBind {
    let start = p.current_span().start;
    let (name, _) = p.expect_ident("binding name");
    PatBind {
        span: p.span_from(start),
        name,
    }
}
