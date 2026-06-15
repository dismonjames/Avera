use crate::ast::path::Path;
use crate::ast::{Directive, DirectiveKind};
use crate::lexer::token::TokenKind;
use crate::parser::Parser;

pub fn parse_directive(p: &mut Parser) -> Option<Directive> {
    let start = p.current_span().start;
    p.bump(); // #
              // The directive name/keyword is the first identifier.
    let (name, _) = p.expect_ident("directive name");
    p.skip_newlines();
    let span = p.span_from(start);
    let kind = match name.as_str() {
        "module" => DirectiveKind::Module(parse_path_directive(p)),
        "header" => DirectiveKind::Header(parse_string_arg(p)),
        "source" => DirectiveKind::Source(parse_string_arg(p)),
        "depends" => DirectiveKind::Depends(parse_path_directive(p)),
        "cfg" => DirectiveKind::Cfg(parse_ident_arg(p)),
        "target" => DirectiveKind::Target(parse_ident_arg(p)),
        "link" => DirectiveKind::Link(parse_string_arg(p)),
        // an import: `#std.fs.File`, `#std.collections.*`, `#std.fs as fs`
        _ => {
            // The first segment was `name`; collect the rest as a dotted path.
            let mut segments = vec![name.clone()];
            while p.at(TokenKind::Dot) {
                p.bump();
                let (seg, _) = p.expect_ident("path segment");
                p.skip_newlines();
                segments.push(seg);
                if segments.last().map(|s| s == "*").unwrap_or(false) {
                    break;
                }
            }
            // optional `as alias`
            let mut alias = None;
            let saved = p.pos;
            p.skip_newlines();
            if let TokenKind::Ident(s) = &p.current().kind {
                if s == "as" {
                    p.bump();
                    p.skip_newlines();
                    let (al, _) = p.expect_ident("alias name");
                    alias = Some(al);
                } else {
                    p.pos = saved;
                }
            } else {
                p.pos = saved;
            }
            DirectiveKind::Import {
                path: Path::new(span, segments),
                alias,
            }
        }
    };
    Some(Directive { span, kind })
}

fn parse_path_directive(p: &mut Parser) -> Path {
    let start = p.current_span().start;
    let (first, _) = p.expect_ident("path");
    let mut segments = vec![first];
    while p.at(TokenKind::Dot) {
        p.bump();
        let (seg, _) = p.expect_ident("path segment");
        segments.push(seg);
    }
    Path::new(p.span_from(start), segments)
}

fn parse_ident_arg(p: &mut Parser) -> String {
    let (s, _) = p.expect_ident("identifier");
    s
}

fn parse_string_arg(p: &mut Parser) -> String {
    if let TokenKind::StrLit(s) = &p.current().kind {
        let v = s.clone();
        p.bump();
        v
    } else {
        p.error(
            p.current_span(),
            crate::diagnostics::diagnostic::DiagnosticKind::Parse,
            "expected a string literal",
        );
        String::new()
    }
}
