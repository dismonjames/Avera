use crate::diagnostics::span::Span;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    // --- identifiers / keywords ---
    Ident(String),
    Kw(Keyword),
    // --- literals ---
    IntLit(String),
    FloatLit(String),
    StrLit(String),
    CharLit(char),
    // --- punctuation / operators ---
    Colon,
    ColonColon,
    Semicolon,
    Comma,
    Dot,
    DotDot,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Lt,
    Gt,
    LtEq,
    GtEq,
    EqEq,
    NotEq,
    Eq,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Amp,
    AmpBang,
    AmpAmp,
    Pipe,
    PipePipe,
    Caret,
    Bang,
    Shl,
    Shr,
    Cast,
    Arrow,
    FatArrow,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    PipeEq,
    AmpEq,
    CaretEq,
    Tilde,
    TildeBang,
    Question,
    At,
    Hash,
    Dollar,
    // --- structural ---
    Newline,
    Eof,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Keyword {
    If,
    Else,
    While,
    Loop,
    For,
    In,
    Match,
    Action,
    Shape,
    Choice,
    Ability,
    Implement,
    Foreign,
    Export,
    Return,
    Break,
    Continue,
    Will,
    Finish,
    Raw,
    SelfKw,
    True,
    False,
    ForKwUnused,
    None,
}

impl Keyword {
    pub fn as_str(self) -> &'static str {
        match self {
            Keyword::If => "if",
            Keyword::Else => "else",
            Keyword::While => "while",
            Keyword::Loop => "loop",
            Keyword::For => "for",
            Keyword::In => "in",
            Keyword::Match => "match",
            Keyword::Action => "action",
            Keyword::Shape => "shape",
            Keyword::Choice => "choice",
            Keyword::Ability => "ability",
            Keyword::Implement => "implement",
            Keyword::Foreign => "foreign",
            Keyword::Export => "export",
            Keyword::Return => "return",
            Keyword::Break => "break",
            Keyword::Continue => "continue",
            Keyword::Will => "will",
            Keyword::Finish => "finish",
            Keyword::Raw => "raw",
            Keyword::SelfKw => "self",
            Keyword::True => "true",
            Keyword::False => "false",
            Keyword::ForKwUnused => "",
            Keyword::None => "none",
        }
    }
    pub fn from_str(s: &str) -> Option<Keyword> {
        Some(match s {
            "if" => Keyword::If,
            "else" => Keyword::Else,
            "while" => Keyword::While,
            "loop" => Keyword::Loop,
            "for" => Keyword::For,
            "in" => Keyword::In,
            "match" => Keyword::Match,
            "action" => Keyword::Action,
            "shape" => Keyword::Shape,
            "choice" => Keyword::Choice,
            "ability" => Keyword::Ability,
            "implement" => Keyword::Implement,
            "foreign" => Keyword::Foreign,
            "export" => Keyword::Export,
            "return" => Keyword::Return,
            "break" => Keyword::Break,
            "continue" => Keyword::Continue,
            "will" => Keyword::Will,
            "finish" => Keyword::Finish,
            "raw" => Keyword::Raw,
            "self" => Keyword::SelfKw,
            "true" => Keyword::True,
            "false" => Keyword::False,
            "none" => Keyword::None,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Ident(s) => write!(f, "{}", s),
            TokenKind::Kw(k) => write!(f, "{}", k.as_str()),
            TokenKind::IntLit(s) => write!(f, "{}", s),
            TokenKind::FloatLit(s) => write!(f, "{}", s),
            TokenKind::StrLit(_) => f.write_str("\"...\""),
            TokenKind::CharLit(c) => write!(f, "'{}'", c),
            TokenKind::Colon => f.write_str(":"),
            TokenKind::ColonColon => f.write_str("::"),
            TokenKind::Semicolon => f.write_str(";"),
            TokenKind::Comma => f.write_str(","),
            TokenKind::Dot => f.write_str("."),
            TokenKind::DotDot => f.write_str(".."),
            TokenKind::LParen => f.write_str("("),
            TokenKind::RParen => f.write_str(")"),
            TokenKind::LBracket => f.write_str("["),
            TokenKind::RBracket => f.write_str("]"),
            TokenKind::Lt => f.write_str("<"),
            TokenKind::Gt => f.write_str(">"),
            TokenKind::LtEq => f.write_str("<="),
            TokenKind::GtEq => f.write_str(">="),
            TokenKind::EqEq => f.write_str("=="),
            TokenKind::NotEq => f.write_str("!="),
            TokenKind::Eq => f.write_str("="),
            TokenKind::Plus => f.write_str("+"),
            TokenKind::Minus => f.write_str("-"),
            TokenKind::Star => f.write_str("*"),
            TokenKind::Slash => f.write_str("/"),
            TokenKind::Percent => f.write_str("%"),
            TokenKind::Amp => f.write_str("&"),
            TokenKind::AmpBang => f.write_str("&!"),
            TokenKind::AmpAmp => f.write_str("&&"),
            TokenKind::Pipe => f.write_str("|"),
            TokenKind::PipePipe => f.write_str("||"),
            TokenKind::Caret => f.write_str("^"),
            TokenKind::Bang => f.write_str("!"),
            TokenKind::Shl => f.write_str("<<"),
            TokenKind::Shr => f.write_str(">>"),
            TokenKind::Cast => f.write_str(":>"),
            TokenKind::Arrow => f.write_str("->"),
            TokenKind::FatArrow => f.write_str("=>"),
            TokenKind::PlusEq => f.write_str("+="),
            TokenKind::MinusEq => f.write_str("-="),
            TokenKind::StarEq => f.write_str("*="),
            TokenKind::SlashEq => f.write_str("/="),
            TokenKind::PercentEq => f.write_str("%="),
            TokenKind::PipeEq => f.write_str("|="),
            TokenKind::AmpEq => f.write_str("&="),
            TokenKind::CaretEq => f.write_str("^="),
            TokenKind::Tilde => f.write_str("~"),
            TokenKind::TildeBang => f.write_str("~!"),
            TokenKind::Question => f.write_str("?"),
            TokenKind::At => f.write_str("@"),
            TokenKind::Hash => f.write_str("#"),
            TokenKind::Dollar => f.write_str("$"),
            TokenKind::Newline => f.write_str("\\n"),
            TokenKind::Eof => f.write_str("<eof>"),
        }
    }
}

impl TokenKind {
    pub fn continues_line(&self) -> bool {
        matches!(
            self,
            TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash
                | TokenKind::Percent
                | TokenKind::Amp
                | TokenKind::AmpAmp
                | TokenKind::Pipe
                | TokenKind::PipePipe
                | TokenKind::Caret
                | TokenKind::Shl
                | TokenKind::Shr
                | TokenKind::EqEq
                | TokenKind::NotEq
                | TokenKind::Lt
                | TokenKind::Gt
                | TokenKind::LtEq
                | TokenKind::GtEq
                | TokenKind::Eq
                | TokenKind::PlusEq
                | TokenKind::MinusEq
                | TokenKind::StarEq
                | TokenKind::SlashEq
                | TokenKind::PercentEq
                | TokenKind::PipeEq
                | TokenKind::AmpEq
                | TokenKind::CaretEq
                | TokenKind::Cast
                | TokenKind::Comma
                | TokenKind::Dot
                | TokenKind::DotDot
                | TokenKind::Arrow
                | TokenKind::FatArrow
                | TokenKind::Colon
                | TokenKind::ColonColon
                | TokenKind::Tilde
                | TokenKind::TildeBang
                | TokenKind::AmpBang
                | TokenKind::Hash
                | TokenKind::At
        )
    }
}
