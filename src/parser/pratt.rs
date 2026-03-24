/// Binding power pairs for the Pratt expression parser.
/// (left_bp, right_bp) — higher = tighter binding.
/// Right-associative ops have right_bp < left_bp.
use crate::lexer::TokenKind;

pub fn infix_binding_power(kind: &TokenKind) -> Option<(u8, u8)> {
    let bp = match kind {
        // Function composition: lower than pipe, left-associative
        // `f >> g >> h` = `h(g(f(x)))` applied left-to-right
        TokenKind::GtGt        => (3,  4),

        // Pipe: lowest precedence after >>, left-associative
        TokenKind::PipeArrow   => (5,  6),

        // Null-coalescing, right-associative
        TokenKind::NullCoalesce => (8, 7),

        // Boolean
        TokenKind::Or          => (10, 11),
        TokenKind::And         => (12, 13),

        // Comparisons (non-associative — chaining is an error, but we allow it for simplicity)
        TokenKind::Eq
        | TokenKind::NotEq
        | TokenKind::Lt
        | TokenKind::Gt
        | TokenKind::LtEq
        | TokenKind::GtEq      => (20, 21),

        // Ranges
        TokenKind::DotDot
        | TokenKind::DotDotEq  => (25, 26),

        // Arithmetic
        TokenKind::Plus
        | TokenKind::Minus     => (30, 31),

        TokenKind::Star
        | TokenKind::Slash
        | TokenKind::Percent   => (40, 41),

        // Power: right-associative
        TokenKind::StarStar    => (50, 49),

        // Member access & call are handled as postfix (see parser), not here.
        // We list Dot so the Pratt loop knows to stop at the right place.
        TokenKind::Dot         => (70, 71),

        _ => return None,
    };
    Some(bp)
}

pub fn postfix_binding_power(kind: &TokenKind) -> Option<u8> {
    match kind {
        // Call: f(...)
        TokenKind::LParen    => Some(80),
        // Index: a[...]
        TokenKind::LBracket  => Some(80),
        // Field access: a.b  (treated as postfix in the main loop)
        TokenKind::Dot       => Some(80),
        // Optional chaining: a?.field, a?.method(args)
        TokenKind::QuestionDot     => Some(80),
        // Optional index: a?[idx]
        TokenKind::QuestionBracket => Some(80),
        _ => None,
    }
}

pub const PREFIX_BP: u8 = 60; // for unary - and not
