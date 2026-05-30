
use avera_compiler::diagnostics::span::SourceMap;
use avera_compiler::lexer::{Lexer, Token, TokenKind};

fn lex(src: &str) -> Vec<Token> {
    let mut map = SourceMap::new();
    let fid = map.load_str("test", src);
    let result = Lexer::new(fid, src).lex();
    result.tokens
}

fn lex_kinds(src: &str) -> Vec<TokenKind> {
    lex(src).into_iter().map(|t| t.kind).collect()
}

#[test]
fn test_basic_tokens() {
    let tokens = lex_kinds("action main(): I32");
    assert!(tokens.contains(&TokenKind::Kw(
        avera_compiler::lexer::token::Keyword::Action
    )));
    assert!(tokens.iter().any(|t| {
        if let TokenKind::Ident(s) = t {
            s == "main"
        } else {
            false
        }
    }));
    assert!(tokens.contains(&TokenKind::LParen));
    assert!(tokens.contains(&TokenKind::RParen));
    assert!(tokens.contains(&TokenKind::Colon));
}

#[test]
fn test_integer_literals() {
    let tokens = lex("42");
    assert!(matches!(tokens[0].kind, TokenKind::IntLit(_)));
    if let TokenKind::IntLit(s) = &tokens[0].kind {
        assert_eq!(s, "42");
    }
    let tokens = lex("0xFF");
    assert!(matches!(tokens[0].kind, TokenKind::IntLit(_)));
    if let TokenKind::IntLit(s) = &tokens[0].kind {
        assert_eq!(s, "0xFF");
    }
}

#[test]
fn test_float_literals() {
    let tokens = lex("3.14");
    assert!(matches!(tokens[0].kind, TokenKind::FloatLit(_)));
    if let TokenKind::FloatLit(s) = &tokens[0].kind {
        assert_eq!(s, "3.14");
    }
}

#[test]
fn test_string_literals() {
    let tokens = lex("\"hello world\"");
    assert!(matches!(tokens[0].kind, TokenKind::StrLit(_)));
    if let TokenKind::StrLit(s) = &tokens[0].kind {
        assert_eq!(s, "hello world");
    }
}

#[test]
fn test_operators() {
    let tokens = lex_kinds("a + b - c * d / e % f");
    assert!(tokens.contains(&TokenKind::Plus));
    assert!(tokens.contains(&TokenKind::Minus));
    assert!(tokens.contains(&TokenKind::Star));
    assert!(tokens.contains(&TokenKind::Slash));
    assert!(tokens.contains(&TokenKind::Percent));
}

#[test]
fn test_comparison_operators() {
    let tokens = lex_kinds("a == b != c < d <= e > f >= g");
    assert!(tokens.contains(&TokenKind::EqEq));
    assert!(tokens.contains(&TokenKind::NotEq));
    assert!(tokens.contains(&TokenKind::Lt));
    assert!(tokens.contains(&TokenKind::LtEq));
    assert!(tokens.contains(&TokenKind::Gt));
    assert!(tokens.contains(&TokenKind::GtEq));
}

#[test]
fn test_keywords() {
    use avera_compiler::lexer::token::Keyword;
    let tokens = lex_kinds("if else while for loop match return break continue finish will");
    assert!(tokens.contains(&TokenKind::Kw(Keyword::If)));
    assert!(tokens.contains(&TokenKind::Kw(Keyword::Else)));
    assert!(tokens.contains(&TokenKind::Kw(Keyword::While)));
    assert!(tokens.contains(&TokenKind::Kw(Keyword::For)));
    assert!(tokens.contains(&TokenKind::Kw(Keyword::Loop)));
    assert!(tokens.contains(&TokenKind::Kw(Keyword::Match)));
    assert!(tokens.contains(&TokenKind::Kw(Keyword::Return)));
    assert!(tokens.contains(&TokenKind::Kw(Keyword::Break)));
    assert!(tokens.contains(&TokenKind::Kw(Keyword::Continue)));
    assert!(tokens.contains(&TokenKind::Kw(Keyword::Finish)));
    assert!(tokens.contains(&TokenKind::Kw(Keyword::Will)));
}

#[test]
fn test_magnet_tokens() {
    let tokens = lex_kinds("~m ~!m");
    assert!(tokens.contains(&TokenKind::Tilde));
    assert!(tokens.contains(&TokenKind::TildeBang));
}

#[test]
fn test_cast_operator() {
    let tokens = lex_kinds("a :> I64");
    assert!(tokens.contains(&TokenKind::Cast));
}

#[test]
fn test_newline_handling() {
    // Newlines inside brackets should be suppressed.
    let tokens = lex_kinds("print(\n  1,\n  2\n)");
    assert!(tokens.contains(&TokenKind::LParen));
    assert!(tokens.contains(&TokenKind::RParen));
}
