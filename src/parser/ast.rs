use crate::error::Span;
use crate::lexer::StrPart;

// ── Spanned wrapper ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

pub type Expr = Spanned<ExprKind>;
pub type Stmt = Spanned<StmtKind>;

// ── Statements ───────────────────────────────────────────────────────────────

/// A flux program is a flat list of statements.
/// There are only two kinds: assignment and expression-statement.
#[derive(Debug, Clone)]
pub enum StmtKind {
    /// `x = expr`  or  `x.field = expr`  or  `x[idx] = expr`
    Assign {
        target: AssignTarget,
        value: Box<Expr>,
    },
    /// Bare expression used for its side effects (e.g. `print("hi")`).
    ExprStmt(Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum AssignTarget {
    Ident(String),
    Field { obj: Box<Expr>, field: String },
    Index { obj: Box<Expr>, index: Box<Expr> },
    /// `{name, age} = map`  — extract named keys
    MapDestructure(Vec<String>),
    /// `a, b = tuple`  — unpack positional elements
    TupleDestructure(Vec<String>),
}

// ── Expressions ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ExprKind {
    // ── Atoms ────────────────────────────────────────────────────────────────
    Literal(LiteralValue),
    Ident(String),

    // ── Collections ──────────────────────────────────────────────────────────
    /// `[e1, e2, e3]`
    List(Vec<Expr>),
    /// `{"key": val, ...}`
    Map(Vec<(Expr, Expr)>),
    /// `(a, b, c)`  — requires ≥ 2 elements or trailing comma
    Tuple(Vec<Expr>),

    // ── Functions & calls ────────────────────────────────────────────────────
    /// `|x, y| body_expr`
    Lambda { params: Vec<String>, body: Box<Expr> },
    /// `f(arg1, arg2, kw=val)`
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        kwargs: Vec<(String, Expr)>,
    },

    // ── Access ───────────────────────────────────────────────────────────────
    FieldAccess { obj: Box<Expr>, field: String },
    Index { obj: Box<Expr>, index: Box<Expr> },

    // ── Operators ────────────────────────────────────────────────────────────
    BinOp { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    UnaryOp { op: UnaryOp, operand: Box<Expr> },

    // ── Strings ──────────────────────────────────────────────────────────────
    /// `"hello {name}!"` — a string with interpolated segments.
    InterpolatedStr(Vec<StrPart>),

    // ── Control flow (all expressions, no blocks) ────────────────────────────
    /// `if cond then a elif cond2 then b else c`
    If {
        cond: Box<Expr>,
        then_: Box<Expr>,
        elif_: Vec<(Box<Expr>, Box<Expr>)>,
        else_: Box<Expr>,
    },
    /// `match subject, pat -> val, pat -> val`
    Match {
        subject: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    /// `left ?? right`
    NullCoalesce { left: Box<Expr>, right: Box<Expr> },

    // ── Ranges ───────────────────────────────────────────────────────────────
    /// `start..end`  or  `start..=end`
    Range { start: Box<Expr>, end: Box<Expr>, inclusive: bool },

    /// `obj?.field`, `obj?[idx]`, `obj?.method(args)` — null-safe access
    /// Evaluates to null if obj is null, otherwise applies the access.
    OptChain { obj: Box<Expr>, access: OptAccess },

    /// `do stmt1\nstmt2\n...end` — sequential block returning last value
    Block(Vec<Stmt>),

    /// `while cond do body end`
    While {
        cond: Box<Expr>,
        body: Vec<Stmt>,
    },
    /// `for var in iter do body end`
    For {
        var: String,
        iter: Box<Expr>,
        body: Vec<Stmt>,
    },
}

// ── Optional chaining access ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum OptAccess {
    Field(String),
    Index(Box<Expr>),
    Call(Vec<Expr>),
}

// ── Match arm ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Literal(LiteralValue),
    /// `_` wildcard
    Wildcard,
    /// Binds the value to a name: `x`
    Bind(String),
    /// `200 | 201 | 202`
    Or(Vec<Pattern>),
    /// `pat if guard_expr`
    Guard { pattern: Box<Pattern>, guard: Box<Expr> },
}

// ── Function parameter ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub default: Option<Box<Expr>>,
}

// ── Operators ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod, Pow,
    Eq, NotEq, Lt, Gt, LtEq, GtEq,
    And, Or,
    /// `f >> g`  — function composition: produces `|x| g(f(x))`
    Compose,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,
    Neg,
}

// ── Literal values ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    /// Plain (non-interpolated) string.
    Str(String),
}
