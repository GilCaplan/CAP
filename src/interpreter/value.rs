use crate::error::{CapError, Span};
use crate::parser::ast::{LiteralValue, Param, Stmt};
use crate::interpreter::env::EnvSnapshot;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

// ── Runtime value ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Rc<RefCell<Vec<Value>>>),
    Map(Rc<RefCell<IndexMap<MapKey, Value>>>),
    Tuple(Vec<Value>),
    Function(Rc<FunctionValue>),
    BuiltinFn(&'static str),
}

// ── User-defined function ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FunctionValue {
    pub name: Option<String>,
    pub params: Vec<Param>,
    pub body: FunctionBody,
    pub closure: EnvSnapshot,
}

#[derive(Debug, Clone)]
pub enum FunctionBody {
    /// A function defined as a lambda: `|x| expr`
    Expr(Box<crate::parser::ast::Expr>),
    /// A function defined by a list of statements (future: multi-statement lambdas)
    Stmts(Vec<Stmt>),
    /// A partially-applied method: `receiver.method` returns this, which
    /// when called prepends `receiver` to the args and dispatches as a builtin.
    MethodPartial { receiver: Box<Value>, method: String },
    /// `f >> g` composition: call f with args, then g with the result.
    BuiltinCompose { f: Box<Value>, g: Box<Value> },
}

// ── Map key (only hashable values) ───────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MapKey {
    Str(String),
    Int(i64),
    Bool(bool),
    Tuple(Vec<MapKey>),
}

impl fmt::Display for MapKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapKey::Str(s)    => write!(f, "{s}"),
            MapKey::Int(n)    => write!(f, "{n}"),
            MapKey::Bool(b)   => write!(f, "{b}"),
            MapKey::Tuple(v)  => {
                write!(f, "(")?;
                for (i, k) in v.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{k}")?;
                }
                write!(f, ")")
            }
        }
    }
}

// ── Value methods ─────────────────────────────────────────────────────────────

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null         => "null",
            Value::Bool(_)      => "bool",
            Value::Int(_)       => "int",
            Value::Float(_)     => "float",
            Value::Str(_)       => "str",
            Value::List(_)      => "list",
            Value::Map(_)       => "map",
            Value::Tuple(_)     => "tuple",
            Value::Function(_)
            | Value::BuiltinFn(_) => "function",
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null         => false,
            Value::Bool(b)      => *b,
            Value::Int(n)       => *n != 0,
            Value::Float(f)     => *f != 0.0,
            Value::Str(s)       => !s.is_empty(),
            Value::List(l)      => !l.borrow().is_empty(),
            Value::Map(m)       => !m.borrow().is_empty(),
            Value::Tuple(t)     => !t.is_empty(),
            Value::Function(_)
            | Value::BuiltinFn(_) => true,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn to_map_key(&self, span: &Span) -> Result<MapKey, CapError> {
        match self {
            Value::Str(s)    => Ok(MapKey::Str(s.clone())),
            Value::Int(n)    => Ok(MapKey::Int(*n)),
            Value::Bool(b)   => Ok(MapKey::Bool(*b)),
            Value::Tuple(v)  => {
                let keys = v.iter()
                    .map(|e| e.to_map_key(span))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(MapKey::Tuple(keys))
            }
            _                => Err(CapError::UnhashableKey { key_type: self.type_name(), span: span.clone() }),
        }
    }

    pub fn as_int(&self, span: &Span) -> Result<i64, CapError> {
        match self {
            Value::Int(n)   => Ok(*n),
            Value::Float(f) => Ok(*f as i64),
            _ => Err(CapError::TypeError { expected: "int", got: self.type_name().to_string(), span: span.clone() }),
        }
    }

    pub fn as_str(&self, span: &Span) -> Result<&str, CapError> {
        match self {
            Value::Str(s) => Ok(s),
            _ => Err(CapError::TypeError { expected: "str", got: self.type_name().to_string(), span: span.clone() }),
        }
    }

    pub fn as_list(&self, span: &Span) -> Result<Rc<RefCell<Vec<Value>>>, CapError> {
        match self {
            Value::List(l) => Ok(l.clone()),
            _ => Err(CapError::TypeError { expected: "list", got: self.type_name().to_string(), span: span.clone() }),
        }
    }

    pub fn as_map(&self, span: &Span) -> Result<Rc<RefCell<IndexMap<MapKey, Value>>>, CapError> {
        match self {
            Value::Map(m) => Ok(m.clone()),
            _ => Err(CapError::TypeError { expected: "map", got: self.type_name().to_string(), span: span.clone() }),
        }
    }

    /// User-facing display (no quotes on strings).
    pub fn display(&self) -> String {
        match self {
            Value::Null        => "null".to_string(),
            Value::Bool(b)     => b.to_string(),
            Value::Int(n)      => n.to_string(),
            Value::Float(f)    => {
                if f.fract() == 0.0 { format!("{f:.1}") } else { f.to_string() }
            }
            Value::Str(s)      => s.clone(),
            Value::List(l)     => {
                let items: Vec<_> = l.borrow().iter().map(|v| v.repr()).collect();
                format!("[{}]", items.join(", "))
            }
            Value::Map(m)      => {
                let pairs: Vec<_> = m.borrow().iter().map(|(k, v)| format!("{k}: {}", v.repr())).collect();
                format!("{{{}}}", pairs.join(", "))
            }
            Value::Tuple(t)    => {
                let items: Vec<_> = t.iter().map(|v| v.repr()).collect();
                format!("({})", items.join(", "))
            }
            Value::Function(f) => format!("<fn {}>", f.name.as_deref().unwrap_or("anonymous")),
            Value::BuiltinFn(n)=> format!("<builtin {n}>"),
        }
    }

    /// Debug/repr display (strings are quoted).
    pub fn repr(&self) -> String {
        match self {
            Value::Str(s) => format!("\"{s}\""),
            other         => other.display(),
        }
    }
}

// ── Equality ─────────────────────────────────────────────────────────────────

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null,    Value::Null)    => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a),  Value::Int(b))  => a == b,
            (Value::Float(a),Value::Float(b))=> a == b,
            (Value::Int(a),  Value::Float(b))=> (*a as f64) == *b,
            (Value::Float(a),Value::Int(b))  => *a == (*b as f64),
            (Value::Str(a),  Value::Str(b))  => a == b,
            (Value::List(a), Value::List(b)) => {
                let a = a.borrow();
                let b = b.borrow();
                *a == *b
            }
            (Value::Tuple(a),Value::Tuple(b))=> a == b,
            _ => false,
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Int(a),   Value::Int(b))   => a.partial_cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Int(a),   Value::Float(b)) => (*a as f64).partial_cmp(b),
            (Value::Float(a), Value::Int(b))   => a.partial_cmp(&(*b as f64)),
            (Value::Str(a),   Value::Str(b))   => a.partial_cmp(b),
            _ => None,
        }
    }
}

// ── Conversion from AST literals ─────────────────────────────────────────────

impl From<LiteralValue> for Value {
    fn from(lit: LiteralValue) -> Self {
        match lit {
            LiteralValue::Null      => Value::Null,
            LiteralValue::Bool(b)   => Value::Bool(b),
            LiteralValue::Int(n)    => Value::Int(n),
            LiteralValue::Float(f)  => Value::Float(f),
            LiteralValue::Str(s)    => Value::Str(s),
        }
    }
}

// ── Param (needed here to avoid circular imports) ────────────────────────────

// Re-export Param from ast so interpreter can use it.
// (Param is defined in ast.rs.)
