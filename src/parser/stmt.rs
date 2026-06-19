use crate::ast::stmt::{AssignOp, ForMode, MatchArm, Stmt, StmtKind};
use crate::lexer::token::{Keyword, TokenKind};
use crate::parser::Parser;

    // parse cau lenh if, while, for, let
    // println!("parsing stmt...");
    pub fn parse_stmt(p: &mut Parser) -> Option<Stmt> {
    p.skip_newlines();
    if p.at_eof() || p.at_kw(Keyword::Finish) || p.at_kw(Keyword::Else) {
        return None;
    }
    let start = p.current_span().start;
    let kind = match &p.current().kind {
        TokenKind::Colon => parse_owner_binding(p, false),
        TokenKind::ColonColon => parse_owner_binding(p, true),
        TokenKind::Tilde => parse_magnet_binding(p, false),
        TokenKind::TildeBang => parse_magnet_binding(p, true),
        TokenKind::Kw(Keyword::If) => parse_if(p),
        TokenKind::Kw(Keyword::While) => parse_while(p),
        TokenKind::Kw(Keyword::Loop) => parse_loop(p),
        TokenKind::Kw(Keyword::For) => parse_for(p),
        TokenKind::Kw(Keyword::Match) => parse_match(p),
        TokenKind::Kw(Keyword::Return) => {
            p.bump();
            // return may have an expression on the same line
            let value = if p.at(TokenKind::Newline)
                || p.at(TokenKind::Semicolon)
                || p.at_eof()
                || p.at_kw(Keyword::Finish)
            {
                None
            } else {
                Some(super::expr::parse_expr(p))
            };
            StmtKind::Return(value)
        }
        TokenKind::Kw(Keyword::Break) => {
            p.bump();
            StmtKind::Break
        }
        TokenKind::Kw(Keyword::Continue) => {
            p.bump();
            StmtKind::Continue
        }
        TokenKind::Kw(Keyword::Raw) => {
            p.bump();
            p.expect_kw(Keyword::Will, "`will`");
            let body = parse_block_body(p);
            p.expect_kw(Keyword::Finish, "`finish`");
            StmtKind::Raw(body)
        }
        _ => {
            // Either an assignment `place = value`, a compound `place op= value`,
            // an early drop `place!`, or a bare expression statement.
            parse_expr_or_assignment(p)
        }
    };
    Some(Stmt::new(p.span_from(start), kind))
}

fn parse_owner_binding(p: &mut Parser, is_const: bool) -> StmtKind {
    p.bump(); // `:` or `::`
    let (name, _) = p.expect_ident("binding name");
    let ty = if p.eat(TokenKind::Colon) {
        p.skip_newlines();
        Some(super::ty::parse_ty(p))
    } else {
        None
    };
    p.skip_newlines();
    p.expect(TokenKind::Eq, "`=`");
    p.skip_newlines();
    let value = super::expr::parse_expr(p);
    if is_const {
        StmtKind::ConstBinding { name, ty, value }
    } else {
        StmtKind::OwnerBinding { name, ty, value }
    }
}

fn parse_magnet_binding(p: &mut Parser, is_mut: bool) -> StmtKind {
    p.bump(); // `~` or `~!`
    let (name, _) = p.expect_ident("magnet name");
    let ty = if p.eat(TokenKind::Colon) {
        p.skip_newlines();
        Some(super::ty::parse_ty(p))
    } else {
        None
    };
    p.skip_newlines();
    p.expect(TokenKind::Eq, "`=`");
    p.skip_newlines();
    let target = super::expr::parse_expr(p);
    if is_mut {
        StmtKind::MagnetMutBinding { name, ty, target }
    } else {
        StmtKind::MagnetBinding { name, ty, target }
    }
}

fn parse_if(p: &mut Parser) -> StmtKind {
    p.bump(); // if
    p.skip_newlines();
    let cond = super::expr::parse_expr(p);
    p.skip_newlines();
    p.expect_kw(Keyword::Will, "`will`");
    let body = parse_block_body(p);
    let mut branches = vec![(cond, body)];
    let mut else_branch = None;
    loop {
        p.skip_newlines();
        if p.at_kw(Keyword::Else) {
            p.bump();
            p.skip_newlines();
            if p.at_kw(Keyword::If) {
                p.bump();
                p.skip_newlines();
                let c = super::expr::parse_expr(p);
                p.skip_newlines();
                p.expect_kw(Keyword::Will, "`will`");
                let b = parse_block_body(p);
                branches.push((c, b));
                continue;
            }
            // plain else
            // optional `will`
            let has_will = p.eat_kw(Keyword::Will);
            let b = if has_will {
                parse_block_body(p)
            } else {
                // single-statement else
                match parse_stmt(p) {
                    Some(s) => vec![s],
                    None => Vec::new(),
                }
            };
            else_branch = Some(b);
            break;
        }
        break;
    }
    p.expect_kw(Keyword::Finish, "`finish`");
    StmtKind::If {
        branches,
        else_branch,
    }
}

fn parse_while(p: &mut Parser) -> StmtKind {
    p.bump();
    p.skip_newlines();
    let cond = super::expr::parse_expr(p);
    p.skip_newlines();
    p.expect_kw(Keyword::Will, "`will`");
    let body = parse_block_body(p);
    p.expect_kw(Keyword::Finish, "`finish`");
    StmtKind::While { cond, body }
}

fn parse_loop(p: &mut Parser) -> StmtKind {
    p.bump();
    p.expect_kw(Keyword::Will, "`will`");
    let body = parse_block_body(p);
    p.expect_kw(Keyword::Finish, "`finish`");
    StmtKind::Loop(body)
}

fn parse_for(p: &mut Parser) -> StmtKind {
    p.bump();
    p.skip_newlines();
    let mode = if p.eat(TokenKind::Amp) {
        let _ = p.eat(TokenKind::Bang);
        ForMode::BorrowMut
    } else if p.eat(TokenKind::Caret) {
        ForMode::Move
    } else {
        ForMode::Read
    };
    let pat = super::pat::parse_pat(p);
    p.skip_newlines();
    p.expect_kw(Keyword::In, "`in`");
    p.skip_newlines();
    // Parse the iterator, which may be a range `lo..hi`.
    let lo = super::expr::parse_expr(p);
    let iter = if p.eat(TokenKind::DotDot) {
        p.skip_newlines();
        let hi = super::expr::parse_expr(p);
        let span = crate::diagnostics::span::Span::union(lo.span, hi.span);
        crate::ast::expr::Expr::new(
            span,
            crate::ast::expr::ExprKind::Range {
                lo: Box::new(lo),
                hi: Box::new(hi),
            },
        )
    } else {
        lo
    };
    p.skip_newlines();
    p.expect_kw(Keyword::Will, "`will`");
    let body = parse_block_body(p);
    p.expect_kw(Keyword::Finish, "`finish`");
    StmtKind::For {
        mode,
        pat,
        iter,
        body,
    }
}

fn parse_match(p: &mut Parser) -> StmtKind {
    p.bump();
    p.skip_newlines();
    let scrutinee = super::expr::parse_expr(p);
    p.skip_newlines();
    p.expect_kw(Keyword::Will, "`will`");
    let mut arms = Vec::new();
    loop {
        p.skip_newlines();
        if p.at_kw(Keyword::Finish) || p.at_eof() {
            break;
        }
        let arm_start = p.current_span().start;
        let pat = super::pat::parse_pat(p);
        p.skip_newlines();
        // optional `if guard`
        let guard = if p.at_kw(Keyword::If) {
            p.bump();
            Some(super::expr::parse_expr(p))
        } else {
            None
        };
        p.skip_newlines();
        p.expect(TokenKind::FatArrow, "`=>`");
        p.skip_newlines();
        // body: either a block (will-less stmts until a blank line / next arm)
        // or a single expression statement.
        let body = parse_arm_body(p);
        arms.push(MatchArm {
            span: p.span_from(arm_start),
            pat,
            guard,
            body,
        });
    }
    p.expect_kw(Keyword::Finish, "`finish`");
    StmtKind::Match { scrutinee, arms }
}

pub fn parse_block_body(p: &mut Parser) -> Vec<Stmt> {
    let mut stmts = Vec::new();
    loop {
        p.skip_newlines();
        if p.at_kw(Keyword::Finish) || p.at_kw(Keyword::Else) || p.at_eof() {
            break;
        }
        match parse_stmt(p) {
            Some(s) => stmts.push(s),
            None => break,
        }
    }
    stmts
}

fn parse_arm_body(p: &mut Parser) -> Vec<Stmt> {
    p.skip_newlines();
    if p.at_kw(Keyword::Will) {
        p.bump();
        let body = parse_block_body(p);
        // arm blocks don't require `finish` — they end at the next arm or
        // the enclosing `finish`. We allow an optional one.
        let _ = p.eat_kw(Keyword::Finish);
        return body;
    }
    match parse_stmt(p) {
        Some(s) => vec![s],
        None => Vec::new(),
    }
}

fn parse_expr_or_assignment(p: &mut Parser) -> StmtKind {
    // First, special magnet retarget: `m -> target`. We parse a postfix
    // expression then look for `->`.
    let e = super::expr::parse_expr(p);
    p.skip_newlines();
    // `place!` early drop: detected when the last token of the expr was `!`.
    // The expr parser leaves `!` alone, so we look at the current token.
    if p.at(TokenKind::Bang) {
        p.bump();
        return StmtKind::Drop(e);
    }
    if p.at(TokenKind::Arrow) {
        p.bump();
        p.skip_newlines();
        let target = super::expr::parse_expr(p);
        return StmtKind::MagnetRetarget { magnet: e, target };
    }
    let op = match &p.current().kind {
        TokenKind::Eq => Some(AssignOp::Assign),
        TokenKind::PlusEq => Some(AssignOp::Add),
        TokenKind::MinusEq => Some(AssignOp::Sub),
        TokenKind::StarEq => Some(AssignOp::Mul),
        TokenKind::SlashEq => Some(AssignOp::Div),
        TokenKind::PercentEq => Some(AssignOp::Mod),
        TokenKind::AmpEq => Some(AssignOp::BitAnd),
        TokenKind::PipeEq => Some(AssignOp::BitOr),
        TokenKind::CaretEq => Some(AssignOp::BitXor),
        TokenKind::Shl => {
            // could be `<<= ` but we don't have that token; treat as shl assign
            // only if followed by `=`. We don't support it v0.1.
            None
        }
        _ => None,
    };
    if let Some(op) = op {
        p.bump();
        p.skip_newlines();
        let value = super::expr::parse_expr(p);
        return StmtKind::Assignment {
            place: e,
            op,
            value,
        };
    }
    // magnet validity test as a statement: `m?` — represented as Try expr
    // statement; the checker handles conditional use. We already consumed
    // `?` as postfix in the expr parser, so it's inside `e`.
    StmtKind::Expr(e)
}

// end of module
