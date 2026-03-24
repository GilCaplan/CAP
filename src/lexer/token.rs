use crate::error::Span;
use std::fmt;

/// A segment of an interpolated string: `"hello {name}!"` →
/// [Literal("hello "), Interp("name"), Literal("!")]
#[derive(Debug, Clone, PartialEq)]
pub enum StrPart {
    Literal(String),
    /// Raw source text of the interpolated expression.
    /// Resolved at runtime by re-parsing through the interpreter.
    Interp(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // ── Literals ────────────────────────────────────────────────────────────
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Vec<StrPart>),
    Null,

    // ── Identifiers & keywords ───────────────────────────────────────────────
    Ident(String),
    // control flow
    If,
    Then,
    Elif,
    Else,
    Match,
    And,
    Or,
    Not,
    Class,
    Do,
    End,
    While,
    For,
    In,

    // ── Operators ────────────────────────────────────────────────────────────
    Plus,        // +
    Minus,       // -
    Star,        // *
    Slash,       // /
    Percent,     // %
    StarStar,    // **   (power)
    Eq,          // ==
    NotEq,       // !=
    Lt,          // <
    Gt,          // >
    LtEq,        // <=
    GtEq,        // >=
    Assign,      // =
    Arrow,       // ->   (match arms)
    PipeArrow,   // |>   (pipe)
    Pipe,        // |    (lambda delimiter: |x| expr)
    Question,    // ?    (used to form ?? and ?.)
    NullCoalesce,// ??
    QuestionDot,     // ?.   (optional chaining: field/call)
    QuestionBracket, // ?[   (optional chaining: index)
    GtGt,        // >>   (function composition)
    Dot,         // .
    DotDot,      // ..
    DotDotEq,    // ..=

    // ── Delimiters ───────────────────────────────────────────────────────────
    Comma,       // ,
    Colon,       // :
    LParen,      // (
    RParen,      // )
    LBracket,    // [
    RBracket,    // ]
    LBrace,      // {
    RBrace,      // }

    // ── Layout ───────────────────────────────────────────────────────────────
    /// Logical newline — statement terminator.
    /// Suppressed when paren_depth > 0.
    Newline,

    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TokenKind::Int(n)        => return write!(f, "{n}"),
            TokenKind::Float(n)      => return write!(f, "{n}"),
            TokenKind::Bool(b)       => return write!(f, "{b}"),
            TokenKind::Str(_)        => "string literal",
            TokenKind::Null          => "null",
            TokenKind::Ident(s)      => return write!(f, "{s}"),
            TokenKind::If            => "if",
            TokenKind::Then          => "then",
            TokenKind::Elif          => "elif",
            TokenKind::Else          => "else",
            TokenKind::Match         => "match",
            TokenKind::And           => "and",
            TokenKind::Or            => "or",
            TokenKind::Not           => "not",
            TokenKind::Class         => "class",
            TokenKind::Do            => "do",
            TokenKind::End           => "end",
            TokenKind::While         => "while",
            TokenKind::For           => "for",
            TokenKind::In            => "in",
            TokenKind::Plus          => "+",
            TokenKind::Minus         => "-",
            TokenKind::Star          => "*",
            TokenKind::Slash         => "/",
            TokenKind::Percent       => "%",
            TokenKind::StarStar      => "**",
            TokenKind::Eq            => "==",
            TokenKind::NotEq         => "!=",
            TokenKind::Lt            => "<",
            TokenKind::Gt            => ">",
            TokenKind::LtEq          => "<=",
            TokenKind::GtEq          => ">=",
            TokenKind::Assign        => "=",
            TokenKind::Arrow         => "->",
            TokenKind::PipeArrow     => "|>",
            TokenKind::Pipe          => "|",
            TokenKind::Question      => "?",
            TokenKind::NullCoalesce  => "??",
            TokenKind::QuestionDot      => "?.",
            TokenKind::QuestionBracket  => "?[",
            TokenKind::GtGt          => ">>",
            TokenKind::Dot           => ".",
            TokenKind::DotDot        => "..",
            TokenKind::DotDotEq      => "..=",
            TokenKind::Comma         => ",",
            TokenKind::Colon         => ":",
            TokenKind::LParen        => "(",
            TokenKind::RParen        => ")",
            TokenKind::LBracket      => "[",
            TokenKind::RBracket      => "]",
            TokenKind::LBrace        => "{",
            TokenKind::RBrace        => "}",
            TokenKind::Newline       => "newline",
            TokenKind::Eof           => "EOF",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token { kind, span }
    }
}
