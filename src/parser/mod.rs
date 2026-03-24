pub mod ast;
pub mod pratt;

pub use ast::*;

use crate::error::{CapError, Span};
use crate::lexer::{StrPart, Token, TokenKind};
use pratt::{infix_binding_power, postfix_binding_power, PREFIX_BP};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    // ── Token navigation ─────────────────────────────────────────────────────

    fn peek(&self) -> &TokenKind {
        self.tokens.get(self.pos).map(|t| &t.kind).unwrap_or(&TokenKind::Eof)
    }

    fn peek_tok(&self) -> &Token {
        static EOF: std::sync::OnceLock<Token> = std::sync::OnceLock::new();
        self.tokens.get(self.pos).unwrap_or_else(|| {
            EOF.get_or_init(|| Token::new(TokenKind::Eof, Span::dummy()))
        })
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn at(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(kind)
    }

    fn expect(&mut self, kind: TokenKind) -> Result<&Token, CapError> {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(&kind) {
            Ok(self.advance())
        } else {
            let got = self.peek().to_string();
            let span = self.peek_tok().span.clone();
            Err(CapError::UnexpectedToken {
                got,
                span,
                expected: token_kind_name(kind),
            })
        }
    }

    /// Skip any NEWLINE tokens.
    fn skip_newlines(&mut self) {
        while self.at(&TokenKind::Newline) {
            self.advance();
        }
    }

    fn span_here(&self) -> Span {
        self.peek_tok().span.clone()
    }

    pub fn is_at_eof(&self) -> bool {
        self.at(&TokenKind::Eof)
    }

    // ── Program ───────────────────────────────────────────────────────────────

    pub fn parse_program(&mut self) -> Result<Vec<Stmt>, CapError> {
        let mut stmts = Vec::new();
        self.skip_newlines();
        while !self.at(&TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
            // Expect a newline or EOF after each statement.
            if self.at(&TokenKind::Newline) {
                self.skip_newlines();
            } else if !self.at(&TokenKind::Eof) {
                let got = self.peek().to_string();
                let span = self.span_here();
                return Err(CapError::UnexpectedToken { got, span, expected: "newline or EOF" });
            }
        }
        Ok(stmts)
    }

    // ── Statements ────────────────────────────────────────────────────────────

    fn parse_stmt(&mut self) -> Result<Stmt, CapError> {
        let start = self.span_here();

        // class Name(params), method = expr, ...
        if self.at(&TokenKind::Class) {
            return self.parse_class_def(start);
        }

        // Map destructure: {name, age} = map
        if self.at(&TokenKind::LBrace) && self.looks_like_map_destructure() {
            return self.parse_map_destructure(start);
        }

        // An assignment starts with an Ident followed by `=` (not `==`).
        // We peek ahead to distinguish `x = expr` from `x == ...`.
        // Also detect tuple destructure: `a, b = tuple`
        if let TokenKind::Ident(_) = self.peek() {
            if self.looks_like_tuple_destructure() {
                return self.parse_tuple_destructure(start);
            }
            if self.tokens.get(self.pos + 1).map(|t| &t.kind) == Some(&TokenKind::Assign) {
                return self.parse_assign(start);
            }
        }

        // Otherwise: expression statement.
        let expr = self.parse_expr(0)?;
        let span = start.to(&expr.span);

        // Check for compound assignment: expr might have ended on a `=`
        // but only if the expr is a simple lvalue. Handle after parsing.
        // (For now, only Ident = expr is handled above; field/index assign is below.)
        if self.at(&TokenKind::Assign) {
            // Field or index assignment: `obj.field = val`  or  `obj[idx] = val`
            match &expr.node {
                ExprKind::FieldAccess { obj, field } => {
                    let field = field.clone();
                    let obj = obj.clone();
                    self.advance(); // consume `=`
                    self.skip_newlines();
                    let value = self.parse_expr(0)?;
                    let span = span.to(&value.span);
                    return Ok(Stmt {
                        node: StmtKind::Assign {
                            target: AssignTarget::Field { obj, field },
                            value: Box::new(value),
                        },
                        span,
                    });
                }
                ExprKind::Index { obj, index } => {
                    let obj = obj.clone();
                    let index = index.clone();
                    self.advance(); // consume `=`
                    self.skip_newlines();
                    let value = self.parse_expr(0)?;
                    let span = span.to(&value.span);
                    return Ok(Stmt {
                        node: StmtKind::Assign {
                            target: AssignTarget::Index { obj, index },
                            value: Box::new(value),
                        },
                        span,
                    });
                }
                _ => {
                    let got = "=".to_string();
                    return Err(CapError::UnexpectedToken { got, span: self.span_here(), expected: "expression" });
                }
            }
        }

        Ok(Stmt { node: StmtKind::ExprStmt(Box::new(expr)), span })
    }

    fn parse_assign(&mut self, start: Span) -> Result<Stmt, CapError> {
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(s) => s,
            _ => unreachable!(),
        };
        self.advance(); // consume `=`
        self.skip_newlines(); // allow value on next line: `x =\n  expr`
        let value = self.parse_expr(0)?;
        let span = start.to(&value.span);
        Ok(Stmt {
            node: StmtKind::Assign {
                target: AssignTarget::Ident(name),
                value: Box::new(value),
            },
            span,
        })
    }

    // ── Expressions (Pratt parser) ────────────────────────────────────────────

    pub fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, CapError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            // Postfix: call f(...), index a[...], field a.b
            if let Some(bp) = postfix_binding_power(self.peek()) {
                if bp < min_bp { break; }
                lhs = self.parse_postfix(lhs)?;
                continue;
            }

            // Infix operators
            let op_kind = self.peek().clone();
            if let Some((left_bp, right_bp)) = infix_binding_power(&op_kind) {
                if left_bp < min_bp { break; }
                let op_span = self.span_here();
                self.advance(); // consume operator

                lhs = match &op_kind {
                    TokenKind::PipeArrow => self.parse_pipe_rhs(lhs, op_span)?,
                    _ => {
                        let rhs = self.parse_expr(right_bp)?;
                        let span = lhs.span.to(&rhs.span);
                        self.make_binop(op_kind, lhs, rhs, span)?
                    }
                };
                continue;
            }

            break;
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr, CapError> {
        let tok = self.peek_tok().clone();
        let start = tok.span.clone();

        match &tok.kind {
            // Literals
            TokenKind::Int(n) => {
                let n = *n;
                self.advance();
                Ok(self.lit(LiteralValue::Int(n), start))
            }
            TokenKind::Float(f) => {
                let f = *f;
                self.advance();
                Ok(self.lit(LiteralValue::Float(f), start))
            }
            TokenKind::Bool(b) => {
                let b = *b;
                self.advance();
                Ok(self.lit(LiteralValue::Bool(b), start))
            }
            TokenKind::Null => {
                self.advance();
                Ok(self.lit(LiteralValue::Null, start))
            }
            TokenKind::Str(parts) => {
                let parts = parts.clone();
                self.advance();
                // If no interpolation, emit a plain Literal.
                if parts.len() == 1 {
                    if let StrPart::Literal(s) = &parts[0] {
                        return Ok(self.lit(LiteralValue::Str(s.clone()), start));
                    }
                }
                Ok(Expr { node: ExprKind::InterpolatedStr(parts), span: start })
            }

            // Identifier
            TokenKind::Ident(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr { node: ExprKind::Ident(s), span: start })
            }

            // Unary operators
            TokenKind::Minus => {
                self.advance();
                let operand = self.parse_expr(PREFIX_BP)?;
                let span = start.to(&operand.span);
                Ok(Expr { node: ExprKind::UnaryOp { op: UnaryOp::Neg, operand: Box::new(operand) }, span })
            }
            TokenKind::Not => {
                self.advance();
                let operand = self.parse_expr(PREFIX_BP)?;
                let span = start.to(&operand.span);
                Ok(Expr { node: ExprKind::UnaryOp { op: UnaryOp::Not, operand: Box::new(operand) }, span })
            }

            // Grouped expression or tuple: (expr) or (a, b, ...)
            TokenKind::LParen => {
                self.advance();
                self.skip_newlines();
                if self.at(&TokenKind::RParen) {
                    let end = self.span_here();
                    self.advance();
                    // Empty parens = empty tuple
                    return Ok(Expr { node: ExprKind::Tuple(vec![]), span: start.to(&end) });
                }
                let first = self.parse_expr(0)?;
                self.skip_newlines();
                if self.at(&TokenKind::Comma) {
                    // Tuple
                    let mut items = vec![first];
                    while self.at(&TokenKind::Comma) {
                        self.advance();
                        self.skip_newlines();
                        if self.at(&TokenKind::RParen) { break; }
                        items.push(self.parse_expr(0)?);
                        self.skip_newlines();
                    }
                    let end = self.expect(TokenKind::RParen)?.span.clone();
                    Ok(Expr { node: ExprKind::Tuple(items), span: start.to(&end) })
                } else {
                    let end = self.expect(TokenKind::RParen)?.span.clone();
                    // Plain grouped expr — preserve span but unwrap node
                    Ok(Expr { node: first.node, span: start.to(&end) })
                }
            }

            // List: [e1, e2, ...]
            TokenKind::LBracket => {
                self.advance();
                self.skip_newlines();
                let mut items = Vec::new();
                while !self.at(&TokenKind::RBracket) && !self.at(&TokenKind::Eof) {
                    items.push(self.parse_expr(0)?);
                    self.skip_newlines();
                    if self.at(&TokenKind::Comma) {
                        self.advance();
                        self.skip_newlines();
                    } else {
                        break;
                    }
                }
                let end = self.expect(TokenKind::RBracket)?.span.clone();
                Ok(Expr { node: ExprKind::List(items), span: start.to(&end) })
            }

            // Map: {"key": val, ...}
            TokenKind::LBrace => {
                self.advance();
                self.skip_newlines();
                let mut pairs = Vec::new();
                while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
                    let key = self.parse_expr(0)?;
                    self.expect(TokenKind::Colon)?;
                    self.skip_newlines();
                    let val = self.parse_expr(0)?;
                    pairs.push((key, val));
                    self.skip_newlines();
                    if self.at(&TokenKind::Comma) {
                        self.advance();
                        self.skip_newlines();
                    } else {
                        break;
                    }
                }
                let end = self.expect(TokenKind::RBrace)?.span.clone();
                Ok(Expr { node: ExprKind::Map(pairs), span: start.to(&end) })
            }

            // Lambda: |x, y| body_expr
            TokenKind::Pipe => {
                self.advance(); // consume opening |
                let mut params = Vec::new();
                while !self.at(&TokenKind::Pipe) && !self.at(&TokenKind::Eof) {
                    match self.peek().clone() {
                        TokenKind::Ident(s) => { params.push(s); self.advance(); }
                        _ => {
                            let got = self.peek().to_string();
                            return Err(CapError::UnexpectedToken { got, span: self.span_here(), expected: "parameter name" });
                        }
                    }
                    if self.at(&TokenKind::Comma) { self.advance(); }
                }
                self.expect(TokenKind::Pipe)?; // closing |
                let body = self.parse_expr(0)?;
                let span = start.to(&body.span);
                Ok(Expr { node: ExprKind::Lambda { params, body: Box::new(body) }, span })
            }

            // if cond then a elif cond2 then b else c
            TokenKind::If => {
                self.advance();
                self.parse_if_expr(start)
            }

            // match subject, pat -> val, pat -> val
            TokenKind::Match => {
                self.advance();
                self.parse_match_expr(start)
            }

            // while cond do body end
            TokenKind::While => {
                self.advance();
                let cond = self.parse_expr(0)?;
                self.skip_newlines();
                self.expect(TokenKind::Do)?;
                self.skip_newlines();
                let mut body = Vec::new();
                while !self.at(&TokenKind::End) && !self.at(&TokenKind::Eof) {
                    body.push(self.parse_stmt()?);
                    self.skip_newlines();
                }
                let end_span = self.expect(TokenKind::End)?.span.clone();
                Ok(Expr { node: ExprKind::While { cond: Box::new(cond), body }, span: start.to(&end_span) })
            }

            // for var in iter do body end
            TokenKind::For => {
                self.advance();
                let var = match self.peek().clone() {
                    TokenKind::Ident(s) => { self.advance(); s }
                    _ => return Err(CapError::Runtime { message: "for: expected variable name".into(), span: self.span_here() }),
                };
                self.expect(TokenKind::In)?;
                let iter = self.parse_expr(0)?;
                self.skip_newlines();
                self.expect(TokenKind::Do)?;
                self.skip_newlines();
                let mut body = Vec::new();
                while !self.at(&TokenKind::End) && !self.at(&TokenKind::Eof) {
                    body.push(self.parse_stmt()?);
                    self.skip_newlines();
                }
                let end_span = self.expect(TokenKind::End)?.span.clone();
                Ok(Expr { node: ExprKind::For { var, iter: Box::new(iter), body }, span: start.to(&end_span) })
            }

            // do stmt1\nstmt2\n...end  — sequential block
            // Statements are separated by newlines (or just run back-to-back when
            // newlines are suppressed inside parens).
            TokenKind::Do => {
                self.advance(); // consume `do`
                self.skip_newlines();
                let mut stmts = Vec::new();
                while !self.at(&TokenKind::End) && !self.at(&TokenKind::Eof) {
                    stmts.push(self.parse_stmt()?);
                    // Skip any trailing newlines (or just continue if suppressed)
                    self.skip_newlines();
                }
                let end_span = self.expect(TokenKind::End)?.span.clone();
                let span = start.to(&end_span);
                Ok(Expr { node: ExprKind::Block(stmts), span })
            }

            _ => {
                let got = tok.kind.to_string();
                Err(CapError::UnexpectedToken { got, span: start, expected: "expression" })
            }
        }
    }

    fn parse_postfix(&mut self, lhs: Expr) -> Result<Expr, CapError> {
        match self.peek().clone() {
            // Call: lhs(args, kw=val)
            TokenKind::LParen => {
                let _open = self.advance().span.clone();
                self.skip_newlines();
                let mut args = Vec::new();
                let mut kwargs = Vec::new();
                while !self.at(&TokenKind::RParen) && !self.at(&TokenKind::Eof) {
                    // Peek ahead for `ident =` keyword arg
                    let is_kwarg = matches!(self.peek(), TokenKind::Ident(_))
                        && self.tokens.get(self.pos + 1).map(|t| &t.kind) == Some(&TokenKind::Assign);
                    if is_kwarg {
                        let name = match self.advance().kind.clone() { TokenKind::Ident(s) => s, _ => unreachable!() };
                        self.advance(); // consume =
                        kwargs.push((name, self.parse_expr(0)?));
                    } else {
                        args.push(self.parse_expr(0)?);
                    }
                    self.skip_newlines();
                    if self.at(&TokenKind::Comma) { self.advance(); self.skip_newlines(); }
                }
                let end = self.expect(TokenKind::RParen)?.span.clone();
                let span = lhs.span.to(&end);
                Ok(Expr { node: ExprKind::Call { callee: Box::new(lhs), args, kwargs }, span })
            }
            // Index: lhs[idx]
            TokenKind::LBracket => {
                self.advance();
                self.skip_newlines();
                let index = self.parse_expr(0)?;
                self.skip_newlines();
                let end = self.expect(TokenKind::RBracket)?.span.clone();
                let span = lhs.span.to(&end);
                Ok(Expr { node: ExprKind::Index { obj: Box::new(lhs), index: Box::new(index) }, span })
            }
            // Field access: lhs.field
            TokenKind::Dot => {
                self.advance();
                match self.peek().clone() {
                    TokenKind::Ident(field) => {
                        let end = self.advance().span.clone();
                        let span = lhs.span.to(&end);
                        Ok(Expr { node: ExprKind::FieldAccess { obj: Box::new(lhs), field }, span })
                    }
                    _ => {
                        let got = self.peek().to_string();
                        Err(CapError::UnexpectedToken { got, span: self.span_here(), expected: "field name" })
                    }
                }
            }
            // Optional chaining: lhs?.field, lhs?.(args)
            TokenKind::QuestionDot => {
                self.advance(); // consume `?.`
                match self.peek().clone() {
                    TokenKind::Ident(field) => {
                        let end = self.advance().span.clone();
                        let span = lhs.span.to(&end);
                        Ok(Expr { node: ExprKind::OptChain { obj: Box::new(lhs), access: OptAccess::Field(field) }, span })
                    }
                    TokenKind::LParen => {
                        self.advance();
                        self.skip_newlines();
                        let mut args = Vec::new();
                        while !self.at(&TokenKind::RParen) && !self.at(&TokenKind::Eof) {
                            args.push(self.parse_expr(0)?);
                            self.skip_newlines();
                            if self.at(&TokenKind::Comma) { self.advance(); self.skip_newlines(); }
                        }
                        let end = self.expect(TokenKind::RParen)?.span.clone();
                        let span = lhs.span.to(&end);
                        Ok(Expr { node: ExprKind::OptChain { obj: Box::new(lhs), access: OptAccess::Call(args) }, span })
                    }
                    _ => {
                        let got = self.peek().to_string();
                        Err(CapError::UnexpectedToken { got, span: self.span_here(), expected: "field name or `(`" })
                    }
                }
            }
            // Optional index: lhs?[idx]
            TokenKind::QuestionBracket => {
                self.advance(); // consume `?[`
                self.skip_newlines();
                let index = self.parse_expr(0)?;
                self.skip_newlines();
                let end = self.expect(TokenKind::RBracket)?.span.clone();
                let span = lhs.span.to(&end);
                Ok(Expr { node: ExprKind::OptChain { obj: Box::new(lhs), access: OptAccess::Index(Box::new(index)) }, span })
            }
            _ => Ok(lhs),
        }
    }

    /// Pipe desugaring: `lhs |> f(args)` → `f(lhs, args)`, `lhs |> f` → `f(lhs)`.
    fn parse_pipe_rhs(&mut self, lhs: Expr, op_span: Span) -> Result<Expr, CapError> {
        // Parse at the right BP of |> (6) — high enough to stop at the next |>
        // (left BP 5) but low enough to include call arguments (postfix BP 80).
        let rhs = self.parse_expr(6)?;
        match rhs.node {
            // `lhs |> f(args)` → `f(lhs, args)`
            ExprKind::Call { callee, mut args, kwargs } => {
                args.insert(0, lhs);
                let span = op_span.to(&callee.span);
                Ok(Expr { node: ExprKind::Call { callee, args, kwargs }, span })
            }
            // `lhs |> f` or `lhs |> (expr)` or `lhs |> f >> g` — treat RHS as callable
            _ => {
                let span = op_span.to(&rhs.span);
                Ok(Expr {
                    node: ExprKind::Call { callee: Box::new(rhs), args: vec![lhs], kwargs: vec![] },
                    span,
                })
            }
        }
    }

    // ── if expression ─────────────────────────────────────────────────────────

    fn parse_if_expr(&mut self, start: Span) -> Result<Expr, CapError> {
        let cond = self.parse_expr(0)?;
        // Allow `then` on the next line:  if cond\n  then ...
        self.skip_newlines();
        self.expect(TokenKind::Then)?;
        self.skip_newlines(); // allow value on next line after `then`
        let then_ = self.parse_expr(0)?;
        // Allow `elif`/`else` on the next line:  then val\n  else ...
        self.skip_newlines();

        let mut elif_ = Vec::new();
        loop {
            if self.at(&TokenKind::Elif) {
                self.advance();
                let elif_cond = self.parse_expr(0)?;
                self.skip_newlines();
                self.expect(TokenKind::Then)?;
                self.skip_newlines(); // allow elif value on next line
                let elif_then = self.parse_expr(0)?;
                elif_.push((Box::new(elif_cond), Box::new(elif_then)));
                self.skip_newlines();
            } else {
                break;
            }
        }

        if !self.at(&TokenKind::Else) {
            return Err(CapError::IfMissingElse { span: self.span_here() });
        }
        self.advance();
        self.skip_newlines(); // allow else value on next line
        let else_ = self.parse_expr(0)?;
        let span = start.to(&else_.span);

        Ok(Expr {
            node: ExprKind::If {
                cond: Box::new(cond),
                then_: Box::new(then_),
                elif_,
                else_: Box::new(else_),
            },
            span,
        })
    }

    // ── match expression ──────────────────────────────────────────────────────

    fn parse_match_expr(&mut self, start: Span) -> Result<Expr, CapError> {
        let subject = self.parse_expr(0)?;
        self.expect(TokenKind::Comma)?;
        self.skip_newlines();

        let mut arms = Vec::new();
        loop {
            let arm_start = self.span_here();
            let pattern = self.parse_pattern()?;
            self.expect(TokenKind::Arrow)?;
            let body = self.parse_expr(0)?;
            let arm_span = arm_start.to(&body.span);
            arms.push(MatchArm { pattern, body: Box::new(body), span: arm_span });

            // Save position before skipping newlines: if no comma follows, restore
            // so that parse_program still sees the statement-terminating newline.
            let saved_pos = self.pos;
            self.skip_newlines();
            if self.at(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
                // Allow trailing comma (no more arms after it)
                if !self.is_pattern_start() { break; }
            } else {
                self.pos = saved_pos;
                break;
            }
        }

        let span = start.to(&arms.last().map(|a| a.span.clone()).unwrap_or(start.clone()));
        Ok(Expr { node: ExprKind::Match { subject: Box::new(subject), arms }, span })
    }

    fn is_pattern_start(&self) -> bool {
        matches!(self.peek(),
            TokenKind::Int(_) | TokenKind::Float(_) | TokenKind::Bool(_)
            | TokenKind::Null | TokenKind::Str(_) | TokenKind::Ident(_)
            | TokenKind::Minus)
    }

    fn parse_pattern(&mut self) -> Result<Pattern, CapError> {
        let pat = self.parse_single_pattern()?;
        // Check for OR patterns: `200 | 201 | 202` or `"a" or "b"` (both forms accepted)
        let is_or_sep = |p: &TokenKind| matches!(p, TokenKind::Pipe | TokenKind::Or);
        let pat = if is_or_sep(self.peek()) {
            let mut alts = vec![pat];
            while is_or_sep(self.peek()) {
                self.advance();
                alts.push(self.parse_single_pattern()?);
            }
            Pattern::Or(alts)
        } else {
            pat
        };
        // Check for guard: `pattern if condition`
        if self.at(&TokenKind::If) {
            self.advance();
            let guard = self.parse_expr(0)?;
            return Ok(Pattern::Guard { pattern: Box::new(pat), guard: Box::new(guard) });
        }
        Ok(pat)
    }

    fn parse_single_pattern(&mut self) -> Result<Pattern, CapError> {
        match self.peek().clone() {
            TokenKind::Int(n)    => { self.advance(); Ok(Pattern::Literal(LiteralValue::Int(n))) }
            TokenKind::Float(f)  => { self.advance(); Ok(Pattern::Literal(LiteralValue::Float(f))) }
            TokenKind::Bool(b)   => { self.advance(); Ok(Pattern::Literal(LiteralValue::Bool(b))) }
            TokenKind::Null      => { self.advance(); Ok(Pattern::Literal(LiteralValue::Null)) }
            TokenKind::Str(parts) => {
                self.advance();
                let s = if let [StrPart::Literal(s)] = parts.as_slice() { s.clone() } else { String::new() };
                Ok(Pattern::Literal(LiteralValue::Str(s)))
            }
            TokenKind::Ident(s)  => {
                let s = s.clone();
                self.advance();
                if s == "_" { Ok(Pattern::Wildcard) } else { Ok(Pattern::Bind(s)) }
            }
            // Negative number literal
            TokenKind::Minus => {
                self.advance();
                match self.peek().clone() {
                    TokenKind::Int(n)   => { self.advance(); Ok(Pattern::Literal(LiteralValue::Int(-n))) }
                    TokenKind::Float(f) => { self.advance(); Ok(Pattern::Literal(LiteralValue::Float(-f))) }
                    _ => Err(CapError::UnexpectedToken { got: self.peek().to_string(), span: self.span_here(), expected: "number" }),
                }
            }
            _ => Err(CapError::UnexpectedToken {
                got: self.peek().to_string(),
                span: self.span_here(),
                expected: "pattern",
            }),
        }
    }

    // ── class desugaring ──────────────────────────────────────────────────────
    //
    // `class Point(x, y), dist = || expr, add = |other| expr`
    //
    // Desugars at parse time into:
    // `Point = |x, y| {"dist": || expr, "add": |other| expr}`
    //
    // No new AST nodes or interpreter support needed.

    fn parse_class_def(&mut self, start: Span) -> Result<Stmt, CapError> {
        self.advance(); // consume `class`

        // Name
        let name = match self.peek().clone() {
            TokenKind::Ident(s) => { self.advance(); s }
            _ => return Err(CapError::UnexpectedToken {
                got: self.peek().to_string(),
                span: self.span_here(),
                expected: "class name",
            }),
        };

        // (params)
        self.expect(TokenKind::LParen)?;
        let mut params: Vec<String> = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at(&TokenKind::Eof) {
            match self.peek().clone() {
                TokenKind::Ident(p) => { params.push(p); self.advance(); }
                _ => return Err(CapError::UnexpectedToken {
                    got: self.peek().to_string(),
                    span: self.span_here(),
                    expected: "parameter name",
                }),
            }
            if self.at(&TokenKind::Comma) { self.advance(); }
        }
        self.expect(TokenKind::RParen)?;

        // Optional: `extends BaseClass(args...)`
        let mut parent_call: Option<Expr> = None;
        if let TokenKind::Ident(kw) = self.peek().clone() {
            if kw == "extends" {
                self.advance(); // consume `extends`
                parent_call = Some(self.parse_expr(0)?);
            }
        }

        // , method = expr, method = expr, ...
        // Methods are separated by commas and can span multiple lines.
        // We skip newlines before AND after consuming each comma.
        let mut pairs: Vec<(Expr, Expr)> = Vec::new();
        loop {
            // Peek ahead (skipping newlines) to see if there is a comma.
            // If there isn't one, restore the position so the newlines stay
            // in the stream for parse_program to consume as a statement end.
            let saved_pos = self.pos;
            self.skip_newlines();
            if !self.at(&TokenKind::Comma) {
                self.pos = saved_pos;
                break;
            }
            self.advance(); // consume `,`
            self.skip_newlines(); // skip newlines after comma (next method on new line)

            // Stop at EOF or if next token isn't an identifier (trailing comma)
            if self.at(&TokenKind::Eof) { break; }
            let method_span = self.span_here();
            let method_name = match self.peek().clone() {
                TokenKind::Ident(s) => { self.advance(); s }
                _ => break, // trailing comma or non-method token — stop cleanly
            };
            self.expect(TokenKind::Assign)?;
            let body = self.parse_expr(0)?;

            // Key is a plain string literal
            let key = Expr {
                node: ExprKind::Literal(LiteralValue::Str(method_name)),
                span: method_span.clone(),
            };
            pairs.push((key, body));
        }

        // Build: |params...| { "method": expr, ... }
        // With `extends`: |params...| merge(Parent(args), { "method": expr, ... })
        let map_span = start.clone();
        let map_expr = Expr {
            node: ExprKind::Map(pairs),
            span: map_span.clone(),
        };
        let body_expr = if let Some(parent) = parent_call {
            Expr {
                node: ExprKind::Call {
                    callee: Box::new(Expr { node: ExprKind::Ident("merge".to_string()), span: map_span.clone() }),
                    args: vec![parent, map_expr],
                    kwargs: vec![],
                },
                span: map_span.clone(),
            }
        } else {
            map_expr
        };
        let lambda_expr = Expr {
            node: ExprKind::Lambda {
                params,
                body: Box::new(body_expr),
            },
            span: map_span.clone(),
        };

        let span = start.to(&lambda_expr.span);
        Ok(Stmt {
            node: StmtKind::Assign {
                target: AssignTarget::Ident(name),
                value: Box::new(lambda_expr),
            },
            span,
        })
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn lit(&self, val: LiteralValue, span: Span) -> Expr {
        Expr { node: ExprKind::Literal(val), span }
    }

    // ── Destructuring helpers ─────────────────────────────────────────────────

    /// True when the token stream starting at `pos` looks like `{ ident (, ident)* } =`.
    fn looks_like_map_destructure(&self) -> bool {
        let mut i = self.pos + 1; // skip `{`
        match self.tokens.get(i).map(|t| &t.kind) {
            Some(TokenKind::Ident(_)) => i += 1,
            _ => return false,
        }
        loop {
            match self.tokens.get(i).map(|t| &t.kind) {
                Some(TokenKind::RBrace) => { i += 1; break; }
                Some(TokenKind::Comma)  => { i += 1; }
                _ => return false,
            }
            match self.tokens.get(i).map(|t| &t.kind) {
                Some(TokenKind::Ident(_)) => { i += 1; }
                Some(TokenKind::RBrace)   => { i += 1; break; } // trailing comma
                _ => return false,
            }
        }
        matches!(self.tokens.get(i).map(|t| &t.kind), Some(TokenKind::Assign))
    }

    /// True when the token stream starting at `pos` looks like `ident , ident (, ident)* =`.
    fn looks_like_tuple_destructure(&self) -> bool {
        let mut i = self.pos + 1; // skip first ident
        if !matches!(self.tokens.get(i).map(|t| &t.kind), Some(TokenKind::Comma)) {
            return false;
        }
        i += 1;
        loop {
            match self.tokens.get(i).map(|t| &t.kind) {
                Some(TokenKind::Ident(_)) => { i += 1; }
                _ => return false,
            }
            match self.tokens.get(i).map(|t| &t.kind) {
                Some(TokenKind::Assign) => return true,
                Some(TokenKind::Comma)  => { i += 1; }
                _ => return false,
            }
        }
    }

    fn parse_map_destructure(&mut self, start: Span) -> Result<Stmt, CapError> {
        self.advance(); // consume `{`
        let mut names = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            match self.peek().clone() {
                TokenKind::Ident(s) => { names.push(s); self.advance(); }
                _ => return Err(CapError::UnexpectedToken {
                    got: self.peek().to_string(),
                    span: self.span_here(),
                    expected: "field name",
                }),
            }
            if self.at(&TokenKind::Comma) { self.advance(); }
        }
        self.expect(TokenKind::RBrace)?;
        self.advance(); // consume `=`
        self.skip_newlines();
        let value = self.parse_expr(0)?;
        let span = start.to(&value.span);
        Ok(Stmt {
            node: StmtKind::Assign {
                target: AssignTarget::MapDestructure(names),
                value: Box::new(value),
            },
            span,
        })
    }

    fn parse_tuple_destructure(&mut self, start: Span) -> Result<Stmt, CapError> {
        let mut names = Vec::new();
        match self.peek().clone() {
            TokenKind::Ident(s) => { names.push(s); self.advance(); }
            _ => unreachable!(),
        }
        while self.at(&TokenKind::Comma) {
            self.advance(); // consume `,`
            match self.peek().clone() {
                TokenKind::Ident(s) => { names.push(s); self.advance(); }
                _ => break,
            }
        }
        self.advance(); // consume `=`
        self.skip_newlines();
        let value = self.parse_expr(0)?;
        let span = start.to(&value.span);
        Ok(Stmt {
            node: StmtKind::Assign {
                target: AssignTarget::TupleDestructure(names),
                value: Box::new(value),
            },
            span,
        })
    }

    fn make_binop(&self, op_kind: TokenKind, lhs: Expr, rhs: Expr, span: Span) -> Result<Expr, CapError> {
        let op = match op_kind {
            TokenKind::Plus     => BinOp::Add,
            TokenKind::Minus    => BinOp::Sub,
            TokenKind::Star     => BinOp::Mul,
            TokenKind::Slash    => BinOp::Div,
            TokenKind::Percent  => BinOp::Mod,
            TokenKind::StarStar => BinOp::Pow,
            TokenKind::Eq       => BinOp::Eq,
            TokenKind::NotEq    => BinOp::NotEq,
            TokenKind::Lt       => BinOp::Lt,
            TokenKind::Gt       => BinOp::Gt,
            TokenKind::LtEq     => BinOp::LtEq,
            TokenKind::GtEq     => BinOp::GtEq,
            TokenKind::And      => BinOp::And,
            TokenKind::Or       => BinOp::Or,
            TokenKind::GtGt     => BinOp::Compose,
            TokenKind::NullCoalesce => {
                return Ok(Expr { node: ExprKind::NullCoalesce { left: Box::new(lhs), right: Box::new(rhs) }, span });
            }
            TokenKind::DotDot    => {
                return Ok(Expr { node: ExprKind::Range { start: Box::new(lhs), end: Box::new(rhs), inclusive: false }, span });
            }
            TokenKind::DotDotEq  => {
                return Ok(Expr { node: ExprKind::Range { start: Box::new(lhs), end: Box::new(rhs), inclusive: true }, span });
            }
            _ => unreachable!("unhandled infix op: {op_kind:?}"),
        };
        Ok(Expr { node: ExprKind::BinOp { op, left: Box::new(lhs), right: Box::new(rhs) }, span })
    }
}

/// Map a TokenKind to a human-readable name for error messages.
fn token_kind_name(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::Newline   => "newline",
        TokenKind::LParen    => "`(`",
        TokenKind::RParen    => "`)`",
        TokenKind::LBracket  => "`[`",
        TokenKind::RBracket  => "`]`",
        TokenKind::LBrace    => "`{`",
        TokenKind::RBrace    => "`}`",
        TokenKind::Colon     => "`:`",
        TokenKind::Comma     => "`,`",
        TokenKind::Arrow     => "`->`",
        TokenKind::Pipe      => "`|`",
        TokenKind::Assign    => "`=`",
        TokenKind::Then      => "`then`",
        TokenKind::Else      => "`else`",
        TokenKind::Class     => "`class`",
        TokenKind::Do        => "`do`",
        TokenKind::End       => "`end`",
        TokenKind::While     => "`while`",
        TokenKind::For       => "`for`",
        TokenKind::In        => "`in`",
        TokenKind::Eof       => "EOF",
        _                    => "token",
    }
}
