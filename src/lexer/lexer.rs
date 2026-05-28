use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use crate::diagnostics::span::{SourceMap, Span};
use crate::lexer::token::{Keyword, Token, TokenKind};
use crate::symbol::FileId;

pub struct Lexed {
    pub tokens: Vec<Token>,
    pub diags: DiagnosticList,
}

pub struct Lexer<'a> {
    file: FileId,
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
    tokens: Vec<Token>,
    diags: DiagnosticList,
    bracket_depth: i32,
    pending_newline: bool,
    newline_pos: u32,
    last_significant: Option<TokenKind>,
}

impl<'a> Lexer<'a> {
    pub fn new(file: FileId, src: &'a str) -> Self {
        Self {
            file,
            src,
            bytes: src.as_bytes(),
            pos: 0,
            tokens: Vec::new(),
            diags: DiagnosticList::new(),
            bracket_depth: 0,
            pending_newline: false,
            newline_pos: 0,
            last_significant: None,
        }
    }

    pub fn lex(mut self) -> Lexed {
        while self.pos < self.bytes.len() {
            self.skip_trivia_and_handle_newlines();
            if self.pos >= self.bytes.len() {
                break;
            }
            let b = self.bytes[self.pos];
            if b.is_ascii_whitespace() {
                continue;
            }
            // identifier / keyword
            if b == b'_' || b.is_ascii_alphabetic() {
                self.lex_ident();
                continue;
            }
            // number
            if b.is_ascii_digit() {
                self.lex_number();
                continue;
            }
            // string / char
            if b == b'"' {
                self.lex_string();
                continue;
            }
            if b == b'\'' {
                self.lex_char();
                continue;
            }
            // operators / punctuation
            self.lex_punct();
        }
        // flush any trailing newline? We let Eof terminate statements.
        self.emit(TokenKind::Eof, self.pos, self.pos);
        Lexed {
            tokens: self.tokens,
            diags: self.diags,
        }
    }

    fn skip_trivia_and_handle_newlines(&mut self) {
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            match b {
                b' ' | b'\t' | b'\r' => {
                    self.pos += 1;
                }
                b'\n' => {
                    if self.bracket_depth == 0 {
                        if !self.pending_newline {
                            self.newline_pos = self.pos as u32;
                        }
                        self.pending_newline = true;
                    }
                    self.pos += 1;
                }
                b'/' if self.peek(1) == Some(b'/') => {
                    // line comment
                    self.pos += 2;
                    while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
                        self.pos += 1;
                    }
                }
                b'/' if self.peek(1) == Some(b'*') => {
                    // block comment (non-nested v0.1)
                    let start = self.pos;
                    self.pos += 2;
                    let mut closed = false;
                    while self.pos < self.bytes.len() {
                        if self.bytes[self.pos] == b'*' && self.peek(1) == Some(b'/') {
                            self.pos += 2;
                            closed = true;
                            break;
                        }
                        if self.bytes[self.pos] == b'\n' && self.bracket_depth == 0 {
                            if !self.pending_newline {
                                self.newline_pos = self.pos as u32;
                            }
                            self.pending_newline = true;
                        }
                        self.pos += 1;
                    }
                    if !closed {
                        self.error_at(start as u32, self.pos as u32, "unterminated block comment");
                    }
                }
                _ => return,
            }
        }
    }

    fn peek(&self, ahead: usize) -> Option<u8> {
        self.bytes.get(self.pos + ahead).copied()
    }

    fn error_at(&mut self, start: u32, end: u32, msg: &str) {
        self.diags.push(Diagnostic::error(
            DiagnosticKind::Lex,
            Span::new(self.file, start, end),
            msg,
        ));
    }

    fn emit(&mut self, kind: TokenKind, start: usize, end: usize) {
        let flush_newline = self.pending_newline
            && self.bracket_depth == 0
            && match &self.last_significant {
                None => false,
                Some(k) => !k.continues_line(),
            };
        if flush_newline {
            let np = self.newline_pos;
            self.tokens
                .push(Token::new(TokenKind::Newline, Span::point(self.file, np)));
            self.pending_newline = false;
        }
        let span = Span::new(self.file, start as u32, end as u32);
        self.tokens.push(Token::new(kind.clone(), span));
        self.last_significant = Some(kind);
    }

    fn lex_ident(&mut self) {
        let start = self.pos;
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b == b'_' || b.is_ascii_alphanumeric() {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text = &self.src[start..self.pos];
        let kind = match Keyword::from_str(text) {
            Some(k) => TokenKind::Kw(k),
            None => TokenKind::Ident(text.to_string()),
        };
        self.emit(kind, start, self.pos);
    }

    fn lex_number(&mut self) {
        let start = self.pos;
        // hex / binary / octal prefix
        if self.bytes[self.pos] == b'0' {
            if let Some(_base @ (b'x' | b'X')) = self.peek(1) {
                self.pos += 2;
                while self.pos < self.bytes.len()
                    && (self.bytes[self.pos].is_ascii_hexdigit() || self.bytes[self.pos] == b'_')
                {
                    self.pos += 1;
                }
                let text: String = self.src[start..self.pos]
                    .chars()
                    .filter(|c| *c != '_')
                    .collect();
                self.emit(TokenKind::IntLit(text), start, self.pos);
                return;
            }
            if let Some(_base @ (b'b' | b'B')) = self.peek(1) {
                self.pos += 2;
                while self.pos < self.bytes.len()
                    && (self.bytes[self.pos] == b'0'
                        || self.bytes[self.pos] == b'1'
                        || self.bytes[self.pos] == b'_')
                {
                    self.pos += 1;
                }
                let text: String = self.src[start..self.pos]
                    .chars()
                    .filter(|c| *c != '_')
                    .collect();
                self.emit(TokenKind::IntLit(text), start, self.pos);
                return;
            }
        }
        // decimal int or float
        let mut is_float = false;
        while self.pos < self.bytes.len()
            && (self.bytes[self.pos].is_ascii_digit() || self.bytes[self.pos] == b'_')
        {
            self.pos += 1;
        }
        // fractional part: digits then `.` then digits. Only treat `.` as
        // part of a number if followed by a digit.
        if self.peek(0) == Some(b'.') && self.peek(1).is_some_and(|b| b.is_ascii_digit()) {
            is_float = true;
            self.pos += 1; // dot
            while self.pos < self.bytes.len()
                && (self.bytes[self.pos].is_ascii_digit() || self.bytes[self.pos] == b'_')
            {
                self.pos += 1;
            }
        }
        // exponent e[+/-]digits
        if matches!(self.peek(0), Some(b'e') | Some(b'E'))
            && self
                .peek(1)
                .is_some_and(|b| b.is_ascii_digit() || b == b'+' || b == b'-')
        {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(0), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            while self.pos < self.bytes.len()
                && (self.bytes[self.pos].is_ascii_digit() || self.bytes[self.pos] == b'_')
            {
                self.pos += 1;
            }
        }
        let text: String = self.src[start..self.pos]
            .chars()
            .filter(|c| *c != '_')
            .collect();
        let kind = if is_float {
            TokenKind::FloatLit(text)
        } else {
            TokenKind::IntLit(text)
        };
        self.emit(kind, start, self.pos);
    }

    fn lex_string(&mut self) {
        let start = self.pos;
        self.pos += 1; // opening quote
        let mut value = String::new();
        loop {
            if self.pos >= self.bytes.len() {
                self.error_at(start as u32, self.pos as u32, "unterminated string literal");
                break;
            }
            let b = self.bytes[self.pos];
            if b == b'"' {
                self.pos += 1;
                break;
            }
            if b == b'\\' {
                self.pos += 1;
                if self.pos >= self.bytes.len() {
                    self.error_at(start as u32, self.pos as u32, "unterminated escape");
                    break;
                }
                let e = self.bytes[self.pos];
                match e {
                    b'n' => value.push('\n'),
                    b't' => value.push('\t'),
                    b'r' => value.push('\r'),
                    b'\\' => value.push('\\'),
                    b'"' => value.push('"'),
                    b'\'' => value.push('\''),
                    b'0' => value.push('\0'),
                    b'x' => {
                        self.pos += 1;
                        let h = self.read_hex_escape(2, &mut value);
                        if !h {
                            // error already emitted
                        }
                        // we already advanced 2 hex digits inside read_hex_escape
                        continue;
                    }
                    b'u' => {
                        self.pos += 1;
                        // expect `{XXXX}`
                        if self.peek(0) == Some(b'{') {
                            self.pos += 1;
                            let mut code = String::new();
                            while self.pos < self.bytes.len() && self.bytes[self.pos] != b'}' {
                                code.push(self.bytes[self.pos] as char);
                                self.pos += 1;
                            }
                            if self.peek(0) == Some(b'}') {
                                self.pos += 1;
                            }
                            match u32::from_str_radix(&code, 16) {
                                Ok(cp) if cp <= 0x10FFFF => match char::from_u32(cp) {
                                    Some(c) => value.push(c),
                                    None => self.error_at(
                                        self.pos as u32,
                                        self.pos as u32,
                                        "invalid unicode scalar",
                                    ),
                                },
                                _ => self.error_at(
                                    self.pos as u32,
                                    self.pos as u32,
                                    "invalid unicode escape",
                                ),
                            }
                        } else {
                            self.error_at(
                                self.pos as u32,
                                self.pos as u32,
                                "expected `{` in \\u escape",
                            );
                        }
                        continue;
                    }
                    other => {
                        value.push(other as char);
                    }
                }
                self.pos += 1;
            } else {
                // raw byte; copy as utf8 chunk until next special char
                let chunk_start = self.pos;
                while self.pos < self.bytes.len()
                    && self.bytes[self.pos] != b'"'
                    && self.bytes[self.pos] != b'\\'
                    && self.bytes[self.pos] != b'\n'
                {
                    self.pos += 1;
                }
                value.push_str(&self.src[chunk_start..self.pos]);
                if self.pos < self.bytes.len() && self.bytes[self.pos] == b'\n' {
                    self.error_at(start as u32, self.pos as u32, "unterminated string literal");
                    break;
                }
            }
        }
        self.emit(TokenKind::StrLit(value), start, self.pos);
    }

    fn read_hex_escape(&mut self, n: usize, out: &mut String) -> bool {
        let mut val: u32 = 0;
        for _ in 0..n {
            if self.pos >= self.bytes.len() || !self.bytes[self.pos].is_ascii_hexdigit() {
                self.error_at(self.pos as u32, self.pos as u32, "invalid hex escape");
                return false;
            }
            let h = self.bytes[self.pos];
            val = val * 16 + (h as char).to_digit(16).unwrap_or(0);
            self.pos += 1;
        }
        out.push(val as u8 as char);
        true
    }

    fn lex_char(&mut self) {
        let start = self.pos;
        self.pos += 1; // opening quote
        let ch;
        if self.pos >= self.bytes.len() {
            self.error_at(start as u32, self.pos as u32, "unterminated char literal");
            self.emit(TokenKind::CharLit('\0'), start, self.pos);
            return;
        }
        if self.bytes[self.pos] == b'\\' {
            self.pos += 1;
            if self.pos >= self.bytes.len() {
                self.error_at(start as u32, self.pos as u32, "unterminated escape");
                self.emit(TokenKind::CharLit('\0'), start, self.pos);
                return;
            }
            let e = self.bytes[self.pos];
            ch = match e {
                b'n' => '\n',
                b't' => '\t',
                b'r' => '\r',
                b'\\' => '\\',
                b'"' => '"',
                b'\'' => '\'',
                b'0' => '\0',
                b'x' => {
                    self.pos += 1;
                    let mut s = String::new();
                    self.read_hex_escape(2, &mut s);
                    s.chars().next().unwrap_or('\0')
                }
                other => other as char,
            };
            self.pos += 1;
        } else {
            // UTF-8 char: copy one char's worth
            let cs = self.pos;
            // find the end of one char
            let first = self.bytes[self.pos];
            let len = utf8_len(first);
            self.pos += len;
            let s = &self.src[cs..self.pos];
            ch = s.chars().next().unwrap_or('\0');
        }
        if self.peek(0) == Some(b'\'') {
            self.pos += 1;
        } else {
            self.error_at(start as u32, self.pos as u32, "unterminated char literal");
        }
        self.emit(TokenKind::CharLit(ch), start, self.pos);
    }

    fn lex_punct(&mut self) {
        let start = self.pos;
        let b = self.bytes[self.pos];
        // match longest token first
        macro_rules! t {
            ($len:expr, $k:expr) => {{
                self.pos += $len;
                self.emit($k, start, self.pos);
                return;
            }};
        }
        // three-char
        match self.src.as_bytes().get(self.pos..).unwrap_or(&[]) {
            [b':', b':', ..] if self.at_bol_after(2) => {} // handled below
            _ => {}
        }
        // two-char tokens
        let two = if self.pos + 2 <= self.bytes.len() {
            &self.src[self.pos..self.pos + 2]
        } else {
            ""
        };
        match two {
            "::" => t!(2, TokenKind::ColonColon),
            "&!" => t!(2, TokenKind::AmpBang),
            ":>" => t!(2, TokenKind::Cast),
            "->" => t!(2, TokenKind::Arrow),
            "=>" => t!(2, TokenKind::FatArrow),
            "==" => t!(2, TokenKind::EqEq),
            "!=" => t!(2, TokenKind::NotEq),
            "<=" => t!(2, TokenKind::LtEq),
            ">=" => t!(2, TokenKind::GtEq),
            "&&" => t!(2, TokenKind::AmpAmp),
            "||" => t!(2, TokenKind::PipePipe),
            "<<" => t!(2, TokenKind::Shl),
            ">>" => t!(2, TokenKind::Shr),
            "+=" => t!(2, TokenKind::PlusEq),
            "-=" => t!(2, TokenKind::MinusEq),
            "*=" => t!(2, TokenKind::StarEq),
            "/=" => t!(2, TokenKind::SlashEq),
            "%=" => t!(2, TokenKind::PercentEq),
            "|=" => t!(2, TokenKind::PipeEq),
            "&=" => t!(2, TokenKind::AmpEq),
            "^=" => t!(2, TokenKind::CaretEq),
            "~!" => t!(2, TokenKind::TildeBang),
            ".." => t!(2, TokenKind::DotDot),
            _ => {}
        }
        // single-char
        let k = match b {
            b':' => TokenKind::Colon,
            b';' => TokenKind::Semicolon,
            b',' => TokenKind::Comma,
            b'.' => TokenKind::Dot,
            b'(' => {
                self.bracket_depth += 1;
                TokenKind::LParen
            }
            b')' => {
                self.bracket_depth = (self.bracket_depth - 1).max(0);
                TokenKind::RParen
            }
            b'[' => {
                self.bracket_depth += 1;
                TokenKind::LBracket
            }
            b']' => {
                self.bracket_depth = (self.bracket_depth - 1).max(0);
                TokenKind::RBracket
            }
            b'<' => TokenKind::Lt,
            b'>' => TokenKind::Gt,
            b'=' => TokenKind::Eq,
            b'+' => TokenKind::Plus,
            b'-' => TokenKind::Minus,
            b'*' => TokenKind::Star,
            b'/' => TokenKind::Slash,
            b'%' => TokenKind::Percent,
            b'&' => TokenKind::Amp,
            b'|' => TokenKind::Pipe,
            b'^' => TokenKind::Caret,
            b'!' => TokenKind::Bang,
            b'~' => TokenKind::Tilde,
            b'?' => TokenKind::Question,
            b'@' => TokenKind::At,
            b'#' => TokenKind::Hash,
            b'$' => TokenKind::Dollar,
            other => {
                self.pos += 1;
                self.error_at(
                    start as u32,
                    self.pos as u32,
                    &format!("unexpected character `{}`", other as char),
                );
                return;
            }
        };
        self.pos += 1;
        self.emit(k, start, self.pos);
    }

    #[inline]
    fn at_bol_after(&self, _n: usize) -> bool {
        true
    }
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first < 0xC0 {
        1
    } else if first < 0xE0 {
        2
    } else if first < 0xF0 {
        3
    } else {
        4
    }
}

pub fn lex_all(map: &mut SourceMap, name: &str, src: &str) -> (FileId, Lexed) {
    let fid = map.load_str(name, src);
    let lexed = Lexer::new(fid, src).lex();
    (fid, lexed)
}
