pub mod directive;
pub mod expr;
pub mod item;
pub mod pat;
pub mod stmt;
pub mod ty;

use crate::ast::Module;
use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use crate::diagnostics::span::Span;
use crate::lexer::token::{Keyword, Token, TokenKind};
use crate::symbol::FileId;

pub struct Parsed {
    pub module: Module,
    pub diags: DiagnosticList,
}

pub struct Parser {
    file: FileId,
    tokens: Vec<Token>,
    pos: usize,
    diags: DiagnosticList,
}

impl Parser {
    pub fn new(file: FileId, tokens: Vec<Token>) -> Self {
        Self {
            file,
            tokens,
            pos: 0,
            diags: DiagnosticList::new(),
        }
    }

    pub fn parse(mut self) -> Parsed {
        let start_span = self.current_span();
        let mut directives = Vec::new();
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_eof() {
                break;
            }
            // directives (`#...`) must come before items.
            if self.at(TokenKind::Hash) {
                while self.at(TokenKind::Hash) {
                    if let Some(d) = directive::parse_directive(&mut self) {
                        directives.push(d);
                    }
                    self.skip_newlines();
                }
                continue;
            }
            // attributes (`@...`) then an item
            if let Some(item) = self.parse_item() {
                items.push(item);
            } else {
                // recover: skip to next declaration start
                self.synchronize_top();
            }
            self.skip_newlines();
        }
        let span = Span::new(self.file, start_span.start, self.prev_end());
        Parsed {
            module: Module {
                span,
                directives,
                items,
            },
            diags: self.diags,
        }
    }

    // ---- cursor helpers ----

    pub(crate) fn current(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    pub(crate) fn current_span(&self) -> Span {
        self.current().span
    }

    pub(crate) fn prev_end(&self) -> u32 {
        if self.pos == 0 {
            0
        } else {
            self.tokens[(self.pos - 1).min(self.tokens.len() - 1)]
                .span
                .end
        }
    }

    pub(crate) fn peek(&self, ahead: usize) -> &Token {
        let i = (self.pos + ahead).min(self.tokens.len() - 1);
        &self.tokens[i]
    }

    pub(crate) fn at(&self, k: TokenKind) -> bool {
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(&k)
    }

    pub(crate) fn at_kw(&self, k: Keyword) -> bool {
        matches!(&self.current().kind, TokenKind::Kw(kw) if *kw == k)
    }

    pub(crate) fn at_eof(&self) -> bool {
        self.at(TokenKind::Eof)
    }

    pub(crate) fn at_any(&self, ks: &[TokenKind]) -> bool {
        ks.iter().any(|k| self.at(k.clone()))
    }

    pub(crate) fn bump(&mut self) -> Token {
        let t = self.current().clone();
        if !self.at_eof() {
            self.pos += 1;
        }
        t
    }

    pub(crate) fn eat(&mut self, k: TokenKind) -> bool {
        if self.at(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(crate) fn eat_kw(&mut self, k: Keyword) -> bool {
        if self.at_kw(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(crate) fn expect(&mut self, k: TokenKind, what: &str) -> Token {
        if self.at(k.clone()) {
            self.bump()
        } else {
            let span = self.current_span();
            self.error(
                span,
                DiagnosticKind::Parse,
                format!("expected {} but found `{}`", what, self.current().kind),
            );
            Token::new(k, span)
        }
    }

    pub(crate) fn expect_kw(&mut self, k: Keyword, what: &str) -> Token {
        if self.at_kw(k) {
            self.bump()
        } else {
            let span = self.current_span();
            self.error(
                span,
                DiagnosticKind::Parse,
                format!("expected {} but found `{}`", what, self.current().kind),
            );
            Token::new(TokenKind::Kw(k), span)
        }
    }

    pub(crate) fn expect_ident(&mut self, what: &str) -> (String, Span) {
        if let TokenKind::Ident(name) = &self.current().kind {
            let span = self.current_span();
            let name = name.clone();
            self.bump();
            (name, span)
        } else {
            let span = self.current_span();
            self.error(
                span,
                DiagnosticKind::Parse,
                format!("expected {} but found `{}`", what, self.current().kind),
            );
            (String::new(), span)
        }
    }

    pub(crate) fn skip_newlines(&mut self) {
        while self.at(TokenKind::Newline) || self.at(TokenKind::Semicolon) {
            self.bump();
        }
    }

    pub(crate) fn eat_separator(&mut self, ctx: &str) {
        if self.at(TokenKind::Semicolon) {
            self.bump();
            self.skip_newlines();
            return;
        }
        if self.at(TokenKind::Newline) {
            self.bump();
            self.skip_newlines();
            return;
        }
        // allow no separator right before block terminators
        if self.at_kw(Keyword::Finish) || self.at_kw(Keyword::Else) || self.at_eof() {
            return;
        }
        let span = self.current_span();
        self.error(
            span,
            DiagnosticKind::Parse,
            format!(
                "expected end of {} but found `{}`",
                ctx,
                self.current().kind
            ),
        );
    }

    pub(crate) fn error(&mut self, span: Span, kind: DiagnosticKind, msg: impl Into<String>) {
        self.diags.push(Diagnostic::error(kind, span, msg));
    }

    pub(crate) fn span_from(&self, start: u32) -> Span {
        Span::new(self.file, start, self.prev_end())
    }

    pub(crate) fn synchronize_top(&mut self) {
        while !self.at_eof() {
            if self.at(TokenKind::Newline) {
                self.bump();
                // peek the next non-newline token
                if self.at_eof() {
                    break;
                }
                if let TokenKind::Kw(k) = &self.current().kind {
                    if matches!(
                        k,
                        Keyword::Shape
                            | Keyword::Choice
                            | Keyword::Ability
                            | Keyword::Implement
                            | Keyword::Action
                            | Keyword::Foreign
                            | Keyword::Export
                    ) {
                        return;
                    }
                }
                if self.at(TokenKind::Hash) || self.at(TokenKind::At) {
                    return;
                }
                continue;
            }
            self.bump();
        }
    }
}

pub fn parse(file: FileId, tokens: Vec<Token>) -> Parsed {
    Parser::new(file, tokens).parse()
}
