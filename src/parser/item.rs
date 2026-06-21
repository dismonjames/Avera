use crate::ast::item::{
    AbilityDecl, ActionDecl, ActionSig, Attr, ChoiceDecl, ForeignDecl, ForeignItem, ImplDecl, Item,
    ItemKind, Param, ParamMode, ShapeDecl, ShapeField, Variant,
};
use crate::ast::path::Path;
use crate::ast::ty::Ty;
use crate::lexer::token::{Keyword, TokenKind};
use crate::parser::Parser;

impl Parser {
    pub(crate) fn parse_item(&mut self) -> Option<Item> {
        let start = self.current_span().start;
        let mut attrs = Vec::new();
        while self.at(TokenKind::At) {
            attrs.push(self.parse_attr());
        }
        let exported = self.eat_kw(Keyword::Export);
        let span_start = if exported {
            start
        } else {
            self.current_span().start
        };
        let kind = match &self.current().kind {
            TokenKind::Kw(Keyword::Shape) => ItemKind::Shape(self.parse_shape()),
            TokenKind::Kw(Keyword::Choice) => ItemKind::Choice(self.parse_choice()),
            TokenKind::Kw(Keyword::Ability) => ItemKind::Ability(self.parse_ability()),
            TokenKind::Kw(Keyword::Implement) => ItemKind::Impl(self.parse_impl()),
            TokenKind::Kw(Keyword::Action) => ItemKind::Action(self.parse_action(None)),
            TokenKind::Kw(Keyword::Foreign) => ItemKind::Foreign(self.parse_foreign()),
            _ => {
                let span = self.current_span();
                self.error(
                    span,
                    crate::diagnostics::diagnostic::DiagnosticKind::Parse,
                    format!("expected a declaration but found `{}`", self.current().kind),
                );
                self.synchronize_top();
                return None;
            }
        };
        Some(Item {
            span: self.span_from(span_start),
            kind,
            attrs,
            exported,
        })
    }

    fn parse_attr(&mut self) -> Attr {
        let start = self.current_span().start;
        self.bump(); // @
        let (name, _) = self.expect_ident("attribute name");
        // optional `( args )`
        let mut args = Vec::new();
        if self.eat(TokenKind::LParen) {
            self.skip_newlines();
            if !self.at(TokenKind::RParen) {
                args.push(self.parse_attr_arg());
                while self.eat(TokenKind::Comma) {
                    self.skip_newlines();
                    if self.at(TokenKind::RParen) {
                        break;
                    }
                    args.push(self.parse_attr_arg());
                }
            }
            self.skip_newlines();
            self.expect(TokenKind::RParen, "`)`");
        }
        Attr {
            span: self.span_from(start),
            name,
            args,
        }
    }

    fn parse_attr_arg(&mut self) -> String {
        match &self.current().kind {
            TokenKind::Ident(s) => {
                let s = s.clone();
                self.bump();
                s
            }
            TokenKind::StrLit(s) => {
                let s = s.clone();
                self.bump();
                s
            }
            TokenKind::IntLit(s) => {
                let s = s.clone();
                self.bump();
                s
            }
            _ => {
                let span = self.current_span();
                self.error(
                    span,
                    crate::diagnostics::diagnostic::DiagnosticKind::Parse,
                    "expected an attribute argument",
                );
                String::new()
            }
        }
    }

    fn parse_shape(&mut self) -> ShapeDecl {
        let start = self.current_span().start;
        self.bump(); // shape
        let (name, _) = self.expect_ident("shape name");
        let generics = self.parse_generic_params();
        self.skip_newlines();
        self.expect_kw(Keyword::Will, "`will`");
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_kw(Keyword::Finish) || self.at_eof() {
                break;
            }
            let fstart = self.current_span().start;
            let (fname, _) = self.expect_ident("field name");
            self.expect(TokenKind::Colon, "`:`");
            self.skip_newlines();
            let ty = super::ty::parse_ty(self);
            fields.push(ShapeField {
                span: self.span_from(fstart),
                name: fname,
                ty,
            });
            self.eat_separator("field declaration");
        }
        self.expect_kw(Keyword::Finish, "`finish`");
        ShapeDecl {
            span: self.span_from(start),
            name,
            generics,
            fields,
        }
    }

    fn parse_choice(&mut self) -> ChoiceDecl {
        let start = self.current_span().start;
        self.bump(); // choice
        let (name, _) = self.expect_ident("choice name");
        let generics = self.parse_generic_params();
        self.skip_newlines();
        self.expect_kw(Keyword::Will, "`will`");
        let mut variants = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_kw(Keyword::Finish) || self.at_eof() {
                break;
            }
            let vstart = self.current_span().start;
            let (vname, _) = self.expect_ident("variant name");
            let payload = if self.eat(TokenKind::LParen) {
                self.skip_newlines();
                let mut fields = Vec::new();
                if !self.at(TokenKind::RParen) {
                    fields.push(self.parse_payload_field());
                    while self.eat(TokenKind::Comma) {
                        self.skip_newlines();
                        if self.at(TokenKind::RParen) {
                            break;
                        }
                        fields.push(self.parse_payload_field());
                    }
                }
                self.skip_newlines();
                self.expect(TokenKind::RParen, "`)`");
                Some(fields)
            } else {
                None
            };
            variants.push(Variant {
                span: self.span_from(vstart),
                name: vname,
                payload,
            });
            self.eat_separator("variant declaration");
        }
        self.expect_kw(Keyword::Finish, "`finish`");
        ChoiceDecl {
            span: self.span_from(start),
            name,
            generics,
            variants,
        }
    }

    fn parse_payload_field(&mut self) -> (String, Ty) {
        let (name, _) = self.expect_ident("payload field name");
        self.expect(TokenKind::Colon, "`:`");
        self.skip_newlines();
        let ty = super::ty::parse_ty(self);
        (name, ty)
    }

    fn parse_ability(&mut self) -> AbilityDecl {
        let start = self.current_span().start;
        self.bump(); // ability
        let (name, _) = self.expect_ident("ability name");
        let generics = self.parse_generic_params();
        self.skip_newlines();
        self.expect_kw(Keyword::Will, "`will`");
        let mut actions = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_kw(Keyword::Finish) || self.at_eof() {
                break;
            }
            // `action sig` (no body, no `will`)
            self.expect_kw(Keyword::Action, "`action`");
            let sig = self.parse_action_sig();
            actions.push(sig);
            self.eat_separator("ability action");
        }
        self.expect_kw(Keyword::Finish, "`finish`");
        AbilityDecl {
            span: self.span_from(start),
            name,
            generics,
            actions,
        }
    }

    fn parse_impl(&mut self) -> ImplDecl {
        let start = self.current_span().start;
        self.bump(); // implement
                     // `Ability for Type` or inherent `Type`
        let first_ty = super::ty::parse_ty(self);
        let (ability, target) = if self.eat_kw(Keyword::For) {
            // wait, there's no `for` keyword used here in the spec — the spec
            // uses `implement Equal for User`. We reserved `For` for loops;
            // re-use it as the impl separator.
            let target = super::ty::parse_ty(self);
            // extract path from first_ty (must be Named)
            let path = match &first_ty.kind {
                crate::ast::ty::TyKind::Named { path, .. } => Some(path.clone()),
                _ => None,
            };
            (path, target)
        } else {
            (None, first_ty)
        };
        let generics = self.parse_generic_params();
        self.skip_newlines();
        self.expect_kw(Keyword::Will, "`will`");
        let mut actions = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_kw(Keyword::Finish) || self.at_eof() {
                break;
            }
            self.expect_kw(Keyword::Action, "`action`");
            let a = self.parse_action(ability.clone());
            actions.push(a);
        }
        self.expect_kw(Keyword::Finish, "`finish`");
        ImplDecl {
            span: self.span_from(start),
            ability,
            target,
            generics,
            actions,
        }
    }

    fn parse_action(&mut self, ability: Option<Path>) -> ActionDecl {
        let start = self.current_span().start;
        self.bump(); // action (already consumed by caller for impl)
        let sig = self.parse_action_sig_with(ability);
        // module-contract declarations (`.mav`) have no body.
        let body = if self.at_kw(Keyword::Will) {
            self.bump();
            let b = super::stmt::parse_block_body(self);
            self.expect_kw(Keyword::Finish, "`finish`");
            Some(b)
        } else {
            // no body — header sig.
            None
        };
        ActionDecl {
            span: self.span_from(start),
            sig,
            body,
        }
    }

    fn parse_action_sig(&mut self) -> ActionSig {
        self.parse_action_sig_with(None)
    }

    fn parse_action_sig_with(&mut self, _ability: Option<Path>) -> ActionSig {
        let start = self.current_span().start;
        let (name, _) = self.expect_ident("action name");
        // optional `.method` continuation for namespaced methods:
        // `User.rename(...)`. The first segment is the type name.
        let (name, receiver) = if self.eat(TokenKind::Dot) {
            let (method, _) = self.expect_ident("method name");
            let recv_path = Path::single(self.span_from(start), name);
            (method, Some(recv_path))
        } else {
            (name, None)
        };
        self.skip_newlines();
        // params
        let params = if self.eat(TokenKind::LParen) {
            self.skip_newlines();
            let mut ps = Vec::new();
            if !self.at(TokenKind::RParen) {
                ps.push(self.parse_param());
                while self.eat(TokenKind::Comma) {
                    self.skip_newlines();
                    if self.at(TokenKind::RParen) {
                        break;
                    }
                    ps.push(self.parse_param());
                }
            }
            self.skip_newlines();
            self.expect(TokenKind::RParen, "`)`");
            ps
        } else {
            Vec::new()
        };
        // return type
        let ret = if self.eat(TokenKind::Colon) {
            self.skip_newlines();
            Some(super::ty::parse_ty(self))
        } else {
            None
        };
        ActionSig {
            span: self.span_from(start),
            name,
            receiver,
            params,
            ret,
        }
    }

    fn parse_param(&mut self) -> Param {
        let start = self.current_span().start;
        let (mode, is_self) = match &self.current().kind {
            TokenKind::Amp => {
                self.bump();
                if self.eat(TokenKind::Bang) {
                    (ParamMode::BorrowMut, false)
                } else {
                    (ParamMode::Borrow, false)
                }
            }
            TokenKind::Caret => {
                self.bump();
                (ParamMode::Move, false)
            }
            TokenKind::Kw(Keyword::SelfKw) => {
                self.bump();
                // `&self` / `&!self` / `^self` — but we already consumed `self`,
                // so this only handles bare `self` which means value-self.
                // The common forms `&self`/`&!self` are handled above by the
                // borrow/move branch detecting the trailing `self` ident.
                (ParamMode::SelfRef, true)
            }
            _ => (ParamMode::Value, false),
        };
        // For `&self`, `&!self`, `^self`: the mode was set above by the
        // prefix operator; now consume the `self` identifier.
        let (name, mode) = if is_self {
            ("self".to_string(), mode)
        } else if let TokenKind::Ident(n) = &self.current().kind {
            let n = n.clone();
            // self-check: if `self` was the ident (no prefix), mode is SelfRef.
            if n == "self" {
                self.bump();
                (n, ParamMode::SelfRef)
            } else {
                self.bump();
                (n, mode)
            }
        } else {
            (String::new(), mode)
        };
        // For borrow/move modes that targeted self, mark self-mode.
        let mode = if name == "self" {
            match mode {
                ParamMode::Borrow => ParamMode::SelfRef,
                ParamMode::BorrowMut => ParamMode::SelfMut,
                ParamMode::Move => ParamMode::SelfMove,
                other => other,
            }
        } else {
            mode
        };
        // type annotation
        self.expect(TokenKind::Colon, "`:`");
        self.skip_newlines();
        let ty = super::ty::parse_ty(self);
        Param {
            span: self.span_from(start),
            mode,
            name,
            ty,
        }
    }

    fn parse_generic_params(&mut self) -> Vec<String> {
        if !self.eat(TokenKind::Lt) {
            return Vec::new();
        }
        self.skip_newlines();
        let mut gs = Vec::new();
        if !self.at(TokenKind::Gt) {
            let (g, _) = self.expect_ident("generic parameter");
            gs.push(g);
            while self.eat(TokenKind::Comma) {
                self.skip_newlines();
                // optional `: Ability` bound — skip it for v0.1 (we record
                // only the parameter name; bounds are checked later).
                let (gname, _) = self.expect_ident("generic parameter");
                if self.eat(TokenKind::Colon) {
                    let _ = super::ty::parse_ty(self);
                }
                gs.push(gname);
            }
        }
        self.skip_newlines();
        self.expect(TokenKind::Gt, "`>`");
        gs
    }

    fn parse_foreign(&mut self) -> ForeignDecl {
        let start = self.current_span().start;
        self.bump(); // foreign
        let abi = match &self.current().kind {
            TokenKind::Ident(s) => {
                let s = s.clone();
                self.bump();
                s
            }
            _ => {
                let span = self.current_span();
                self.error(
                    span,
                    crate::diagnostics::diagnostic::DiagnosticKind::Parse,
                    "expected an ABI name after `foreign`",
                );
                String::new()
            }
        };
        // optional `("libc")`
        let lib = if self.eat(TokenKind::LParen) {
            self.skip_newlines();
            let l = if let TokenKind::StrLit(s) = &self.current().kind {
                let s = s.clone();
                self.bump();
                Some(s)
            } else {
                None
            };
            self.skip_newlines();
            self.expect(TokenKind::RParen, "`)`");
            l
        } else {
            None
        };
        self.skip_newlines();
        self.expect_kw(Keyword::Will, "`will`");
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_kw(Keyword::Finish) || self.at_eof() {
                break;
            }
            // optional `@link("name")`
            let mut link_name = None;
            if self.eat(TokenKind::At) {
                let (attr_name, _) = self.expect_ident("attribute name");
                if attr_name == "link" {
                    self.expect(TokenKind::LParen, "`(`");
                    self.skip_newlines();
                    if let TokenKind::StrLit(s) = &self.current().kind {
                        link_name = Some(s.clone());
                        self.bump();
                    }
                    self.skip_newlines();
                    self.expect(TokenKind::RParen, "`)`");
                }
            }
            let raw = self.eat_kw(Keyword::Raw);
            self.expect_kw(Keyword::Action, "`action`");
            let sig = self.parse_action_sig();
            items.push(ForeignItem {
                span: self.span_from(start),
                sig,
                link_name,
                raw,
            });
            self.eat_separator("foreign declaration");
        }
        self.expect_kw(Keyword::Finish, "`finish`");
        ForeignDecl {
            span: self.span_from(start),
            abi,
            lib,
            items,
        }
    }
}
