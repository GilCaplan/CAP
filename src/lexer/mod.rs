pub mod token;
pub use token::{StrPart, Token, TokenKind};

use crate::error::{CapError, Span};
use std::collections::VecDeque;
use std::iter::Peekable;
use std::str::CharIndices;

pub struct Lexer<'src> {
    src: &'src str,
    chars: Peekable<CharIndices<'src>>,
    /// Current byte offset (tracks position of `chars.peek()`).
    pos: usize,
    line: u32,
    col: u32,
    /// How many unclosed (, [, { we are inside.
    /// When > 0, Newline tokens are suppressed.
    paren_depth: u32,
    /// Buffered tokens (we may emit multiple at once, e.g. Newline then Eof).
    pending: VecDeque<Token>,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        Lexer {
            src,
            chars: src.char_indices().peekable(),
            pos: 0,
            line: 1,
            col: 0,
            paren_depth: 0,
            pending: VecDeque::new(),
        }
    }

    /// Consume the entire source and return a flat Vec<Token>.
    pub fn tokenize_all(&mut self) -> Result<Vec<Token>, CapError> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof { break; }
        }
        Ok(tokens)
    }

    // ── Internal helpers ────────────────────────────────────────────────────

    fn span_at(&self, start: usize, start_col: u32) -> Span {
        Span::new(start, self.pos, self.line, start_col)
    }

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, c)| *c)
    }

    fn advance_char(&mut self) -> Option<char> {
        let (idx, ch) = self.chars.next()?;
        self.pos = idx + ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.col = 0;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    fn eat_if(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.advance_char();
            true
        } else {
            false
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch == '\n' { break; }
            self.advance_char();
        }
    }

    fn skip_whitespace_no_newline(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch == ' ' || ch == '\r' || ch == '\t' {
                self.advance_char();
            } else {
                break;
            }
        }
    }

    fn make_tok(&self, kind: TokenKind, start: usize, start_col: u32) -> Token {
        Token::new(kind, self.span_at(start, start_col))
    }

    // ── Number lexing ────────────────────────────────────────────────────────

    fn lex_number(&mut self, first: char, start: usize, start_col: u32) -> Token {
        let mut s = String::from(first);
        let mut is_float = false;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                s.push(ch);
                self.advance_char();
            } else if ch == '.' && !is_float {
                // Look ahead: is the char after '.' a digit? (avoid .. range token)
                let rest = &self.src[self.pos..];
                let mut iter = rest.chars();
                iter.next(); // skip '.'
                if iter.next().map_or(false, |c| c.is_ascii_digit()) {
                    is_float = true;
                    s.push('.');
                    self.advance_char();
                } else {
                    break;
                }
            } else if ch == '_' {
                // Allow _ in numbers for readability: 1_000_000
                self.advance_char();
            } else {
                break;
            }
        }
        let span = self.span_at(start, start_col);
        if is_float {
            let f: f64 = s.parse().unwrap_or(0.0);
            Token::new(TokenKind::Float(f), span)
        } else {
            let n: i64 = s.parse().unwrap_or(0);
            Token::new(TokenKind::Int(n), span)
        }
    }

    // ── String lexing ─────────────────────────────────────────────────────────

    fn lex_string(&mut self, start: usize, start_col: u32) -> Result<Token, CapError> {
        let mut parts: Vec<StrPart> = Vec::new();
        let mut current = String::new();

        loop {
            match self.peek_char() {
                None | Some('\n') => {
                    return Err(CapError::UnterminatedString {
                        span: self.span_at(start, start_col),
                    });
                }
                Some('"') => {
                    self.advance_char();
                    break;
                }
                Some('{') => {
                    // Check for {{ escape
                    self.advance_char();
                    if self.peek_char() == Some('{') {
                        self.advance_char();
                        current.push('{');
                    } else {
                        // Begin interpolation: scan until matching }
                        if !current.is_empty() {
                            parts.push(StrPart::Literal(std::mem::take(&mut current)));
                        }
                        let mut interp = String::new();
                        let mut depth = 1u32;
                        loop {
                            match self.peek_char() {
                                None | Some('\n') => {
                                    return Err(CapError::UnterminatedString {
                                        span: self.span_at(start, start_col),
                                    });
                                }
                                Some('{') => { depth += 1; interp.push('{'); self.advance_char(); }
                                Some('}') => {
                                    self.advance_char();
                                    depth -= 1;
                                    if depth == 0 { break; }
                                    interp.push('}');
                                }
                                // Nested string literal inside {}: scan it whole so that
                                // {p["key"]} works without requiring backslash escaping.
                                // Both `{p["key"]}` and `{p[\"key\"]}` are accepted.
                                Some('"') => {
                                    interp.push('"');
                                    self.advance_char(); // consume opening "
                                    loop {
                                        match self.peek_char() {
                                            None | Some('\n') => {
                                                return Err(CapError::UnterminatedString {
                                                    span: self.span_at(start, start_col),
                                                });
                                            }
                                            Some('"') => {
                                                interp.push('"');
                                                self.advance_char();
                                                break;
                                            }
                                            Some('\\') => {
                                                self.advance_char();
                                                match self.peek_char() {
                                                    Some(c) => { interp.push(c); self.advance_char(); }
                                                    None    => break,
                                                }
                                            }
                                            Some(c) => { interp.push(c); self.advance_char(); }
                                        }
                                    }
                                }
                                // Backslash outside a nested string: \\" -> " (legacy compat)
                                Some('\\') => {
                                    self.advance_char();
                                    match self.peek_char() {
                                        Some('"')  => { interp.push('"');  self.advance_char(); }
                                        Some('\\') => { interp.push('\\'); self.advance_char(); }
                                        Some(c)    => { interp.push('\\'); interp.push(c); self.advance_char(); }
                                        None       => break,
                                    }
                                }
                                Some(c) => { interp.push(c); self.advance_char(); }
                            }
                        }
                        parts.push(StrPart::Interp(interp));
                    }
                }
                Some('}') => {
                    self.advance_char();
                    // }} is an escape for literal }
                    if self.peek_char() == Some('}') {
                        self.advance_char();
                        current.push('}');
                    } else {
                        current.push('}');
                    }
                }
                Some('\\') => {
                    self.advance_char();
                    let esc = match self.peek_char() {
                        Some('n')  => { self.advance_char(); '\n' }
                        Some('t')  => { self.advance_char(); '\t' }
                        Some('r')  => { self.advance_char(); '\r' }
                        Some('"')  => { self.advance_char(); '"'  }
                        Some('\\') => { self.advance_char(); '\\' }
                        Some('{')  => { self.advance_char(); '{'  }
                        Some(c)    => { current.push('\\'); c     }
                        None       => break,
                    };
                    current.push(esc);
                }
                Some(c) => {
                    current.push(c);
                    self.advance_char();
                }
            }
        }

        if !current.is_empty() || parts.is_empty() {
            parts.push(StrPart::Literal(current));
        }

        Ok(Token::new(TokenKind::Str(parts), self.span_at(start, start_col)))
    }

    // ── Triple-quoted string `"""..."""` ─────────────────────────────────────
    // Raw: no interpolation, no escape sequences. Can span multiple lines.
    // Terminates at first `"""`.

    fn lex_triple_string(&mut self, start: usize, start_col: u32) -> Result<Token, CapError> {
        let mut s = String::new();
        loop {
            match self.peek_char() {
                None => {
                    return Err(CapError::UnterminatedString {
                        span: self.span_at(start, start_col),
                    });
                }
                Some('"') => {
                    // Check if this is the closing `"""`
                    let rest = &self.src[self.pos..];
                    if rest.starts_with("\"\"\"") {
                        self.advance_char(); // consume 1st "
                        self.advance_char(); // consume 2nd "
                        self.advance_char(); // consume 3rd "
                        break;
                    }
                    // Single or double quote — include as literal
                    s.push('"');
                    self.advance_char();
                }
                Some(c) => {
                    s.push(c);
                    self.advance_char();
                }
            }
        }
        let parts = vec![StrPart::Literal(s)];
        Ok(Token::new(TokenKind::Str(parts), self.span_at(start, start_col)))
    }

    // ── Raw string lexing `r"..."` ────────────────────────────────────────────
    // No interpolation, no escape sequences — characters are taken literally.

    fn lex_raw_string(&mut self, start: usize, start_col: u32) -> Result<Token, CapError> {
        let mut s = String::new();
        loop {
            match self.peek_char() {
                None | Some('\n') => {
                    return Err(CapError::UnterminatedString {
                        span: self.span_at(start, start_col),
                    });
                }
                Some('"') => {
                    self.advance_char();
                    break;
                }
                Some(c) => {
                    s.push(c);
                    self.advance_char();
                }
            }
        }
        let parts = vec![StrPart::Literal(s)];
        Ok(Token::new(TokenKind::Str(parts), self.span_at(start, start_col)))
    }

    // ── Identifier / keyword lexing ───────────────────────────────────────────

    fn lex_ident(&mut self, first: char, start: usize, start_col: u32) -> Token {
        let mut s = String::from(first);
        while let Some(ch) = self.peek_char() {
            if ch.is_alphanumeric() || ch == '_' {
                s.push(ch);
                self.advance_char();
            } else {
                break;
            }
        }
        let span = self.span_at(start, start_col);
        let kind = match s.as_str() {
            "if"    => TokenKind::If,
            "then"  => TokenKind::Then,
            "elif"  => TokenKind::Elif,
            "else"  => TokenKind::Else,
            "match" => TokenKind::Match,
            "and"   => TokenKind::And,
            "or"    => TokenKind::Or,
            "not"   => TokenKind::Not,
            "true"  => TokenKind::Bool(true),
            "false" => TokenKind::Bool(false),
            "null"  => TokenKind::Null,
            "class" => TokenKind::Class,
            "do"    => TokenKind::Do,
            "end"   => TokenKind::End,
            "while" => TokenKind::While,
            "for"   => TokenKind::For,
            "in"    => TokenKind::In,
            _       => TokenKind::Ident(s),
        };
        Token::new(kind, span)
    }

    // ── Main token dispatch ───────────────────────────────────────────────────

    pub fn next_token(&mut self) -> Result<Token, CapError> {
        if let Some(tok) = self.pending.pop_front() {
            return Ok(tok);
        }

        loop {
            // Skip spaces and tabs on current line (but not newlines).
            self.skip_whitespace_no_newline();

            let start = self.pos;
            let start_col = self.col;

            let ch = match self.advance_char() {
                None => return Ok(self.make_tok(TokenKind::Eof, start, start_col)),
                Some(c) => c,
            };

            match ch {
                // Comments
                '#' => {
                    self.skip_line_comment();
                    // Don't emit a token; loop again.
                    continue;
                }

                // Newline — logical statement terminator
                '\n' => {
                    if self.paren_depth == 0 {
                        // Suppress consecutive/blank newlines: only emit if the
                        // previously emitted token was meaningful (not another Newline).
                        return Ok(self.make_tok(TokenKind::Newline, start, start_col));
                    }
                    // Inside parens: suppress newline, continue.
                    continue;
                }

                '\r' => continue,

                // Numbers
                c if c.is_ascii_digit() => return Ok(self.lex_number(c, start, start_col)),

                // Strings: `"""..."""` (triple-quoted, raw) or `"..."` (regular)
                '"' => {
                    // Check for triple-quote `"""`
                    if self.peek_char() == Some('"') {
                        // peek two ahead
                        let rest = &self.src[self.pos..];
                        if rest.starts_with("\"\"") {
                            self.advance_char(); // consume 2nd "
                            self.advance_char(); // consume 3rd "
                            return self.lex_triple_string(start, start_col);
                        }
                        // It's just `""` (empty string) — fall through to regular lex
                    }
                    return self.lex_string(start, start_col);
                }

                // Identifiers / keywords  (also catches `r"..."` raw strings)
                c if c.is_alphabetic() || c == '_' => {
                    // `r"..."` raw string — no interpolation, no escape processing
                    if c == 'r' && self.peek_char() == Some('"') {
                        self.advance_char(); // consume `"`
                        return self.lex_raw_string(start, start_col);
                    }
                    return Ok(self.lex_ident(c, start, start_col));
                }

                // Two-char and single-char operators
                '|' => {
                    if self.eat_if('>') {
                        return Ok(self.make_tok(TokenKind::PipeArrow, start, start_col));
                    }
                    return Ok(self.make_tok(TokenKind::Pipe, start, start_col));
                }
                '-' => {
                    if self.eat_if('>') {
                        return Ok(self.make_tok(TokenKind::Arrow, start, start_col));
                    }
                    return Ok(self.make_tok(TokenKind::Minus, start, start_col));
                }
                '*' => {
                    if self.eat_if('*') {
                        return Ok(self.make_tok(TokenKind::StarStar, start, start_col));
                    }
                    return Ok(self.make_tok(TokenKind::Star, start, start_col));
                }
                '=' => {
                    if self.eat_if('=') {
                        return Ok(self.make_tok(TokenKind::Eq, start, start_col));
                    }
                    return Ok(self.make_tok(TokenKind::Assign, start, start_col));
                }
                '!' => {
                    if self.eat_if('=') {
                        return Ok(self.make_tok(TokenKind::NotEq, start, start_col));
                    }
                    return Err(CapError::UnexpectedChar { ch: '!', span: self.span_at(start, start_col) });
                }
                '<' => {
                    if self.eat_if('=') {
                        return Ok(self.make_tok(TokenKind::LtEq, start, start_col));
                    }
                    return Ok(self.make_tok(TokenKind::Lt, start, start_col));
                }
                '>' => {
                    if self.eat_if('=') {
                        return Ok(self.make_tok(TokenKind::GtEq, start, start_col));
                    }
                    if self.eat_if('>') {
                        return Ok(self.make_tok(TokenKind::GtGt, start, start_col));
                    }
                    return Ok(self.make_tok(TokenKind::Gt, start, start_col));
                }
                '?' => {
                    if self.eat_if('?') {
                        return Ok(self.make_tok(TokenKind::NullCoalesce, start, start_col));
                    }
                    if self.eat_if('.') {
                        return Ok(self.make_tok(TokenKind::QuestionDot, start, start_col));
                    }
                    if self.eat_if('[') {
                        return Ok(self.make_tok(TokenKind::QuestionBracket, start, start_col));
                    }
                    return Ok(self.make_tok(TokenKind::Question, start, start_col));
                }
                '.' => {
                    if self.eat_if('.') {
                        if self.eat_if('=') {
                            return Ok(self.make_tok(TokenKind::DotDotEq, start, start_col));
                        }
                        return Ok(self.make_tok(TokenKind::DotDot, start, start_col));
                    }
                    return Ok(self.make_tok(TokenKind::Dot, start, start_col));
                }

                '+' => return Ok(self.make_tok(TokenKind::Plus,     start, start_col)),
                '/' => return Ok(self.make_tok(TokenKind::Slash,    start, start_col)),
                '%' => return Ok(self.make_tok(TokenKind::Percent,  start, start_col)),
                ',' => return Ok(self.make_tok(TokenKind::Comma,    start, start_col)),
                ':' => return Ok(self.make_tok(TokenKind::Colon,    start, start_col)),

                '(' => { self.paren_depth += 1; return Ok(self.make_tok(TokenKind::LParen,   start, start_col)); }
                ')' => { if self.paren_depth > 0 { self.paren_depth -= 1; } return Ok(self.make_tok(TokenKind::RParen,   start, start_col)); }
                '[' => { self.paren_depth += 1; return Ok(self.make_tok(TokenKind::LBracket, start, start_col)); }
                ']' => { if self.paren_depth > 0 { self.paren_depth -= 1; } return Ok(self.make_tok(TokenKind::RBracket, start, start_col)); }
                '{' => { self.paren_depth += 1; return Ok(self.make_tok(TokenKind::LBrace,   start, start_col)); }
                '}' => { if self.paren_depth > 0 { self.paren_depth -= 1; } return Ok(self.make_tok(TokenKind::RBrace,   start, start_col)); }

                other => {
                    return Err(CapError::UnexpectedChar {
                        ch: other,
                        span: self.span_at(start, start_col),
                    });
                }
            }
        }
    }
}
