use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub col: u32,
}

impl Span {
    pub fn new(start: usize, end: usize, line: u32, col: u32) -> Self {
        Span { start, end, line, col }
    }

    pub fn dummy() -> Self {
        Span { start: 0, end: 0, line: 1, col: 0 }
    }

    /// Merge two spans (from start of `self` to end of `other`).
    pub fn to(&self, other: &Span) -> Span {
        Span {
            start: self.start,
            end: other.end,
            line: self.line,
            col: self.col,
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

#[derive(Debug, Clone, Error)]
pub enum CapError {
    // Lexer
    #[error("SyntaxError: unterminated string at {span}")]
    UnterminatedString { span: Span },

    #[error("SyntaxError: tabs are not allowed for indentation (use spaces) at {span}")]
    TabIndent { span: Span },

    #[error("SyntaxError: unexpected character `{ch}` at {span}")]
    UnexpectedChar { ch: char, span: Span },

    // Parser
    #[error("SyntaxError: unexpected `{got}`, expected {expected} at {span}")]
    UnexpectedToken { got: String, span: Span, expected: &'static str },

    #[error("SyntaxError: `|>` right-hand side must be a function call at {span}")]
    PipeRhsMustBeCallable { span: Span },

    #[error("SyntaxError: `if` expression requires `else` branch at {span}")]
    IfMissingElse { span: Span },

    // Interpreter (runtime)
    #[error("NameError: `{name}` is not defined at {span}")]
    UndefinedVariable { name: String, span: Span },

    #[error("TypeError: expected {expected}, got {got} at {span}")]
    TypeError { expected: &'static str, got: String, span: Span },

    #[error("TypeError: `{value}` is not callable at {span}")]
    NotCallable { value: String, span: Span },

    #[error("ArgumentError: too few arguments — expected {expected}, got {got} at {span}")]
    TooFewArgs { expected: usize, got: usize, span: Span },

    #[error("IndexError: index {index} out of bounds (len={len}) at {span}")]
    IndexOutOfBounds { index: i64, len: usize, span: Span },

    #[error("KeyError: key `{key}` not found at {span}")]
    KeyError { key: String, span: Span },

    #[error("TypeError: `{key_type}` cannot be used as a map key at {span}")]
    UnhashableKey { key_type: &'static str, span: Span },

    #[error("RuntimeError: maximum call depth exceeded at {span}")]
    StackOverflow { span: Span },

    #[error("RuntimeError: {message}")]
    Runtime { message: String, span: Span },

    #[error("IOError: {message}")]
    Io { message: String, span: Span },

    #[error("HTTPError: {message}")]
    Http { message: String, span: Span },

    #[error("JSONError: {message}")]
    Json { message: String, span: Span },

}

impl CapError {
    pub fn span(&self) -> Option<&Span> {
        match self {
            CapError::UnterminatedString { span }
            | CapError::TabIndent { span }
            | CapError::UnexpectedChar { span, .. }
            | CapError::UnexpectedToken { span, .. }
            | CapError::PipeRhsMustBeCallable { span }
            | CapError::IfMissingElse { span }
            | CapError::UndefinedVariable { span, .. }
            | CapError::TypeError { span, .. }
            | CapError::NotCallable { span, .. }
            | CapError::TooFewArgs { span, .. }
            | CapError::IndexOutOfBounds { span, .. }
            | CapError::KeyError { span, .. }
            | CapError::UnhashableKey { span, .. }
            | CapError::StackOverflow { span }
            | CapError::Runtime { span, .. }
            | CapError::Io { span, .. }
            | CapError::Http { span, .. }
            | CapError::Json { span, .. } => Some(span),
        }
    }
}

/// Render a CapError with a source-context caret pointing at the offending token.
pub fn format_error(err: &CapError, source: &str, filename: &str) -> String {
    let Some(span) = err.span() else {
        return format!("{err}");
    };

    let line_text = source.lines().nth((span.line.saturating_sub(1)) as usize).unwrap_or("");
    let col = span.col as usize;
    let caret_offset = " ".repeat(col);
    let caret_len = (span.end.saturating_sub(span.start)).max(1);
    let caret = "^".repeat(caret_len);

    format!(
        "\n  --> {filename}:{line}:{col}\n   |\n{line_num:>4} | {line_text}\n   | {caret_offset}{caret}\n\n{err}",
        line = span.line,
        col = span.col,
        line_num = span.line,
        line_text = line_text,
        caret_offset = caret_offset,
        caret = caret,
        err = err
    )
}
