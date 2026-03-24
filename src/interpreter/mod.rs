pub mod env;
pub mod value;
pub mod stdlib;

use crate::error::{CapError, Span};
use crate::lexer::StrPart;
use crate::parser::ast::*;
use env::Env;
use value::{FunctionBody, FunctionValue, MapKey, Value};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

const MAX_CALL_DEPTH: usize = 8000;

pub struct Interpreter {
    env: Env,
    call_depth: usize,
}

impl Interpreter {
    pub fn new() -> Self {
        let mut env = Env::new();
        stdlib::register_all(&mut env);
        Interpreter { env, call_depth: 0 }
    }

    /// Inject a variable into the root environment (used to expose `args` etc.)
    pub fn set_var(&mut self, name: &str, value: Value) {
        self.env.set(name, value);
    }

    // ── Program entry point ───────────────────────────────────────────────────

    pub fn run_program(&mut self, stmts: &[Stmt]) -> Result<Value, CapError> {
        let mut last = Value::Null;
        for stmt in stmts {
            last = self.eval_stmt(stmt)?;
        }
        Ok(last)
    }

    // ── Statements ────────────────────────────────────────────────────────────

    fn eval_stmt(&mut self, stmt: &Stmt) -> Result<Value, CapError> {
        match &stmt.node {
            StmtKind::Assign { target, value } => {
                let val = self.eval_expr(value)?;
                // When assigning a lambda to a simple Ident, give it that name.
                // This enables recursion: call_function injects the name back into
                // the call env so the body can find the function by name.
                let val = if let (AssignTarget::Ident(name), Value::Function(func)) = (target, &val) {
                    if func.name.is_none() {
                        let named = FunctionValue {
                            name: Some(name.clone()),
                            params: func.params.clone(),
                            body: func.body.clone(),
                            closure: func.closure.clone(),
                        };
                        Value::Function(Rc::new(named))
                    } else {
                        val
                    }
                } else {
                    val
                };
                self.assign_target(target, val, &stmt.span)?;
                Ok(Value::Null)
            }
            StmtKind::ExprStmt(expr) => self.eval_expr(expr),
        }
    }

    fn assign_target(&mut self, target: &AssignTarget, val: Value, span: &Span) -> Result<(), CapError> {
        match target {
            AssignTarget::Ident(name) => {
                self.env.assign(name, val);
            }
            AssignTarget::Field { obj, field } => {
                let map_val = self.eval_expr(obj)?;
                let map = map_val.as_map(span)?;
                let key = MapKey::Str(field.clone());
                map.borrow_mut().insert(key, val);
            }
            AssignTarget::Index { obj, index } => {
                let collection = self.eval_expr(obj)?;
                let idx_val = self.eval_expr(index)?;
                match &collection {
                    Value::List(list) => {
                        let idx = idx_val.as_int(span)?;
                        let mut l = list.borrow_mut();
                        let len = l.len();
                        let pos = normalize_index(idx, len, span)?;
                        l[pos] = val;
                    }
                    Value::Map(map) => {
                        let key = idx_val.to_map_key(span)?;
                        map.borrow_mut().insert(key, val);
                    }
                    other => return Err(CapError::TypeError {
                        expected: "list or map",
                        got: other.type_name().to_string(),
                        span: span.clone(),
                    }),
                }
            }
            AssignTarget::MapDestructure(names) => {
                let map = val.as_map(span)?;
                for name in names {
                    let v = map.borrow().get(&MapKey::Str(name.clone())).cloned().unwrap_or(Value::Null);
                    self.env.assign(name, v);
                }
            }
            AssignTarget::TupleDestructure(names) => {
                let items = match &val {
                    Value::List(l)  => l.borrow().clone(),
                    Value::Tuple(t) => t.clone(),
                    other => return Err(CapError::TypeError {
                        expected: "list or tuple",
                        got: other.type_name().to_string(),
                        span: span.clone(),
                    }),
                };
                for (i, name) in names.iter().enumerate() {
                    let v = items.get(i).cloned().unwrap_or(Value::Null);
                    self.env.assign(name, v);
                }
            }
        }
        Ok(())
    }

    // ── Expressions ───────────────────────────────────────────────────────────

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, CapError> {
        match &expr.node {
            ExprKind::Literal(lit) => Ok(lit.clone().into()),

            ExprKind::Ident(name) => {
                self.env.get(name).ok_or_else(|| CapError::UndefinedVariable {
                    name: name.clone(),
                    span: expr.span.clone(),
                })
            }

            ExprKind::InterpolatedStr(parts) => self.eval_interp_str(parts, &expr.span),

            ExprKind::List(items) => {
                let vals: Result<Vec<_>, _> = items.iter().map(|e| self.eval_expr(e)).collect();
                Ok(Value::List(Rc::new(RefCell::new(vals?))))
            }

            ExprKind::Map(pairs) => {
                let mut map = IndexMap::new();
                for (k, v) in pairs {
                    let key_val = self.eval_expr(k)?;
                    let key = key_val.to_map_key(&k.span)?;
                    let val = self.eval_expr(v)?;
                    map.insert(key, val);
                }
                Ok(Value::Map(Rc::new(RefCell::new(map))))
            }

            ExprKind::Tuple(items) => {
                let vals: Result<Vec<_>, _> = items.iter().map(|e| self.eval_expr(e)).collect();
                Ok(Value::Tuple(vals?))
            }

            ExprKind::Lambda { params, body } => {
                Ok(Value::Function(Rc::new(FunctionValue {
                    name: None,
                    params: params.iter().map(|p| Param { name: p.clone(), default: None }).collect(),
                    body: FunctionBody::Expr(body.clone()),
                    closure: self.env.snapshot(),
                })))
            }

            ExprKind::Call { callee, args: call_args, kwargs } => {
                // Special: import("file.cap") — run another cap file, return bindings as a map
                if let ExprKind::Ident(name) = &callee.node {
                    if name == "import" {
                        let path_val = if let Some(first) = call_args.first() {
                            self.eval_expr(first)?
                        } else {
                            return Err(CapError::TooFewArgs { expected: 1, got: 0, span: expr.span.clone() });
                        };
                        let path = path_val.as_str(&expr.span)?.to_string();
                        return self.eval_import(&path, &expr.span);
                    }
                }
                self.eval_call(callee, call_args, kwargs, &expr.span)
            }

            ExprKind::FieldAccess { obj, field } => {
                let val = self.eval_expr(obj)?;
                self.eval_field_access(val, field, &expr.span)
            }

            ExprKind::Index { obj, index } => {
                let obj_val = self.eval_expr(obj)?;
                let idx_val = self.eval_expr(index)?;
                self.eval_index(obj_val, idx_val, &expr.span)
            }

            ExprKind::BinOp { op, left, right } => {
                // Short-circuit for `and` / `or`
                match op {
                    BinOp::And => {
                        let l = self.eval_expr(left)?;
                        return if !l.is_truthy() { Ok(l) } else { self.eval_expr(right) };
                    }
                    BinOp::Or => {
                        let l = self.eval_expr(left)?;
                        return if l.is_truthy() { Ok(l) } else { self.eval_expr(right) };
                    }
                    _ => {}
                }
                let l = self.eval_expr(left)?;
                let r = self.eval_expr(right)?;
                self.eval_binop(op, l, r, &expr.span)
            }

            ExprKind::UnaryOp { op, operand } => {
                let val = self.eval_expr(operand)?;
                match op {
                    UnaryOp::Not => Ok(Value::Bool(!val.is_truthy())),
                    UnaryOp::Neg => match val {
                        Value::Int(n)   => Ok(Value::Int(-n)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        other => Err(CapError::TypeError { expected: "number", got: other.type_name().to_string(), span: expr.span.clone() }),
                    },
                }
            }

            ExprKind::If { cond, then_, elif_, else_ } => {
                if self.eval_expr(cond)?.is_truthy() {
                    return self.eval_expr(then_);
                }
                for (elif_cond, elif_then) in elif_ {
                    if self.eval_expr(elif_cond)?.is_truthy() {
                        return self.eval_expr(elif_then);
                    }
                }
                self.eval_expr(else_)
            }

            ExprKind::Match { subject, arms } => {
                let subject_val = self.eval_expr(subject)?;
                for arm in arms {
                    if let Some(bindings) = match_pattern(&arm.pattern, &subject_val) {
                        // Push a scope and bind any pattern variables
                        self.env.push_scope();
                        for (name, val) in &bindings {
                            self.env.set(name, val.clone());
                        }
                        // Evaluate guard if present (variables are bound so guard can reference them)
                        let guard_ok = if let Pattern::Guard { guard, .. } = &arm.pattern {
                            match self.eval_expr(guard) {
                                Ok(v) => v.is_truthy(),
                                Err(e) => { self.env.pop_scope(); return Err(e); }
                            }
                        } else {
                            true
                        };
                        if guard_ok {
                            let result = self.eval_expr(&arm.body);
                            self.env.pop_scope();
                            return result;
                        }
                        self.env.pop_scope();
                        // Guard failed — try next arm
                    }
                }
                Err(CapError::Runtime {
                    message: format!("no match arm matched value: {}", subject_val.repr()),
                    span: subject.span.clone(),
                })
            }

            ExprKind::NullCoalesce { left, right } => {
                let l = self.eval_expr(left)?;
                if l.is_null() { self.eval_expr(right) } else { Ok(l) }
            }

            ExprKind::Range { start, end, inclusive } => {
                let s = self.eval_expr(start)?.as_int(&start.span)?;
                let e = self.eval_expr(end)?.as_int(&end.span)?;
                let items: Vec<Value> = if *inclusive {
                    (s..=e).map(Value::Int).collect()
                } else {
                    (s..e).map(Value::Int).collect()
                };
                Ok(Value::List(Rc::new(RefCell::new(items))))
            }

            ExprKind::Block(stmts) => {
                self.env.push_scope();
                let mut last = Value::Null;
                let result: Result<Value, CapError> = (|| {
                    for stmt in stmts {
                        last = self.eval_stmt(stmt)?;
                    }
                    Ok(last)
                })();
                self.env.pop_scope();
                result
            }

            ExprKind::While { cond, body } => {
                let mut last = Value::Null;
                loop {
                    let cond_val = self.eval_expr(cond)?;
                    if !cond_val.is_truthy() { break; }
                    self.env.push_scope();
                    let mut result = Ok(Value::Null);
                    for stmt in body {
                        result = self.eval_stmt(stmt);
                        if result.is_err() { break; }
                    }
                    self.env.pop_scope();
                    match result {
                        Ok(v) => last = v,
                        Err(e) => return Err(e),
                    }
                }
                Ok(last)
            }

            ExprKind::For { var, iter, body } => {
                let iter_val = self.eval_expr(iter)?;
                let items = match &iter_val {
                    Value::List(l) => l.borrow().clone(),
                    Value::Tuple(t) => t.clone(),
                    _ => return Err(CapError::TypeError {
                        expected: "list or tuple",
                        got: iter_val.type_name().to_string(),
                        span: expr.span.clone(),
                    }),
                };
                let mut last = Value::Null;
                for item in items {
                    self.env.push_scope();
                    self.env.set(var.as_str(), item);
                    let mut result = Ok(Value::Null);
                    for stmt in body {
                        result = self.eval_stmt(stmt);
                        if result.is_err() { break; }
                    }
                    self.env.pop_scope();
                    match result {
                        Ok(v) => last = v,
                        Err(e) => return Err(e),
                    }
                }
                Ok(last)
            }

            ExprKind::OptChain { obj, access } => {
                let obj_val = self.eval_expr(obj)?;
                if obj_val.is_null() {
                    return Ok(Value::Null);
                }
                match access {
                    OptAccess::Field(field) => self.eval_field_access(obj_val, field, &expr.span),
                    OptAccess::Index(idx_expr) => {
                        let idx = self.eval_expr(idx_expr)?;
                        self.eval_index(obj_val, idx, &expr.span)
                    }
                    OptAccess::Call(arg_exprs) => {
                        let args: Result<Vec<_>, _> = arg_exprs.iter().map(|e| self.eval_expr(e)).collect();
                        self.call_value(obj_val, args?, &expr.span)
                    }
                }
            }
        }
    }

    // ── Field access (also handles method-style dispatch) ─────────────────────

    fn eval_field_access(&mut self, val: Value, field: &str, span: &Span) -> Result<Value, CapError> {
        match &val {
            Value::Map(map) => {
                let key = MapKey::Str(field.to_string());
                map.borrow().get(&key).cloned().ok_or_else(|| CapError::KeyError {
                    key: field.to_string(),
                    span: span.clone(),
                })
            }
            // Property-style shorthands (no-arg): val.len, val.type, val.repr
            Value::Str(s) => match field {
                "len"    => Ok(Value::Int(s.chars().count() as i64)),
                "upper"  => Ok(Value::Str(s.to_uppercase())),
                "lower"  => Ok(Value::Str(s.to_lowercase())),
                "trim"   => Ok(Value::Str(s.trim().to_string())),
                "lines"  => {
                    let items: Vec<Value> = s.lines().map(|l| Value::Str(l.to_string())).collect();
                    Ok(Value::List(Rc::new(RefCell::new(items))))
                }
                "chars"  => {
                    let items: Vec<Value> = s.chars().map(|c| Value::Str(c.to_string())).collect();
                    Ok(Value::List(Rc::new(RefCell::new(items))))
                }
                // For method calls like s.split(","), return a partial fn
                _ => Ok(method_partial(val.clone(), field, &self.env.snapshot())),
            },
            Value::List(list) => match field {
                "len"     => Ok(Value::Int(list.borrow().len() as i64)),
                "first"   => Ok(list.borrow().first().cloned().unwrap_or(Value::Null)),
                "last"    => Ok(list.borrow().last().cloned().unwrap_or(Value::Null)),
                "reverse" => {
                    let mut items = list.borrow().clone();
                    items.reverse();
                    Ok(Value::List(Rc::new(RefCell::new(items))))
                }
                _ => Ok(method_partial(val.clone(), field, &self.env.snapshot())),
            },
            Value::Tuple(items) => match field {
                "len" => Ok(Value::Int(items.len() as i64)),
                other => Err(CapError::Runtime {
                    message: format!("tuple has no field `{other}`"),
                    span: span.clone(),
                }),
            },
            other => Err(CapError::Runtime {
                message: format!("cannot access field `{field}` on {}", other.type_name()),
                span: span.clone(),
            }),
        }
    }

    // ── Index access ──────────────────────────────────────────────────────────

    fn eval_index(&self, obj: Value, idx: Value, span: &Span) -> Result<Value, CapError> {
        match &obj {
            Value::List(list) => {
                match &idx {
                    Value::Int(n) => {
                        let l = list.borrow();
                        let pos = normalize_index(*n, l.len(), span)?;
                        Ok(l[pos].clone())
                    }
                    // Range slicing: list[1..5] — idx evaluates to a list of ints
                    Value::List(range) => {
                        let l = list.borrow();
                        let slice: Result<Vec<Value>, CapError> = range.borrow().iter().map(|i| {
                            let n = i.as_int(span)?;
                            let pos = normalize_index(n, l.len(), span)?;
                            Ok(l[pos].clone())
                        }).collect();
                        Ok(Value::List(Rc::new(RefCell::new(slice?))))
                    }
                    other => Err(CapError::TypeError { expected: "int or range", got: other.type_name().to_string(), span: span.clone() }),
                }
            }
            Value::Map(map) => {
                let key = idx.to_map_key(span)?;
                // Missing key returns null (not KeyError) so that `map["key"] ?? default`
                // works as intended with the null-coalescing operator.
                Ok(map.borrow().get(&key).cloned().unwrap_or(Value::Null))
            }
            Value::Tuple(t) => {
                let n = idx.as_int(span)?;
                let pos = normalize_index(n, t.len(), span)?;
                Ok(t[pos].clone())
            }
            Value::Str(s) => {
                let chars: Vec<char> = s.chars().collect();
                match &idx {
                    Value::Int(n) => {
                        let pos = normalize_index(*n, chars.len(), span)?;
                        Ok(Value::Str(chars[pos].to_string()))
                    }
                    // Range slicing: str[1..5]
                    Value::List(range) => {
                        let slice: Result<String, CapError> = range.borrow().iter().map(|i| {
                            let n = i.as_int(span)?;
                            let pos = normalize_index(n, chars.len(), span)?;
                            Ok(chars[pos])
                        }).collect();
                        Ok(Value::Str(slice?))
                    }
                    other => Err(CapError::TypeError { expected: "int or range", got: other.type_name().to_string(), span: span.clone() }),
                }
            }
            other => Err(CapError::TypeError { expected: "list/map/tuple/str", got: other.type_name().to_string(), span: span.clone() }),
        }
    }

    // ── Function calls ────────────────────────────────────────────────────────

    fn eval_call(
        &mut self,
        callee: &Expr,
        arg_exprs: &[Expr],
        _kwargs: &[(String, Expr)],
        span: &Span,
    ) -> Result<Value, CapError> {
        let callee_val = self.eval_expr(callee)?;
        let args: Result<Vec<_>, _> = arg_exprs.iter().map(|e| self.eval_expr(e)).collect();
        let args = args?;
        self.call_value(callee_val, args, span)
    }

    pub fn call_value(&mut self, callee: Value, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(CapError::StackOverflow { span: span.clone() });
        }
        match callee {
            Value::Function(func) => {
                self.call_depth += 1;
                let result = self.call_function(&func, args, span);
                self.call_depth -= 1;
                result
            }
            Value::BuiltinFn(name) => self.call_builtin(name, args, span),
            other => Err(CapError::NotCallable { value: other.type_name().to_string(), span: span.clone() }),
        }
    }

    fn call_function(&mut self, func: &FunctionValue, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
        // Arity check: count params that are required (no default, not __rest)
        let required = func.params.iter()
            .filter(|p| p.name != "__rest" && p.default.is_none())
            .count();
        if args.len() < required {
            return Err(CapError::TooFewArgs { expected: required, got: args.len(), span: span.clone() });
        }

        match &func.body {
            FunctionBody::Expr(body_expr) => {
                // Bind params in a new env derived from the closure
                let mut child_env = Env::from_snapshot(&func.closure);
                // Self-reference: inject the function's own name so recursive
                // calls (and class constructor references) resolve correctly.
                if let Some(name) = &func.name {
                    child_env.set(name, Value::Function(Rc::new(func.clone())));
                }
                for (i, param) in func.params.iter().enumerate() {
                    if param.name == "__rest" {
                        // Variadic rest parameter (used by method partials)
                        break;
                    }
                    let val = args.get(i).cloned().unwrap_or(Value::Null);
                    child_env.set(&param.name, val);
                }
                let prev = std::mem::replace(&mut self.env, child_env);
                let result = self.eval_expr(body_expr);
                self.env = prev;
                result
            }
            FunctionBody::Stmts(stmts) => {
                let mut child_env = Env::from_snapshot(&func.closure);
                for (i, param) in func.params.iter().enumerate() {
                    let val = args.get(i).cloned().unwrap_or(Value::Null);
                    child_env.set(&param.name, val);
                }
                let prev = std::mem::replace(&mut self.env, child_env);
                let mut last = Value::Null;
                let result = (|| {
                    for stmt in stmts {
                        last = self.eval_stmt(stmt)?;
                    }
                    Ok(last)
                })();
                self.env = prev;
                result
            }
            FunctionBody::MethodPartial { receiver, method } => {
                // Method-style call: receiver.method(extra_args...)
                // Prepend receiver to args and dispatch as builtin
                let mut full_args = vec![*receiver.clone()];
                full_args.extend(args);
                self.call_builtin_str(method, full_args, span)
            }
            FunctionBody::BuiltinCompose { f, g } => {
                // `f >> g`: call f(args...), then g(result)
                let f = *f.clone();
                let g = *g.clone();
                let intermediate = self.call_value(f, args, span)?;
                self.call_value(g, vec![intermediate], span)
            }
        }
    }

    fn call_builtin(&mut self, name: &'static str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
        self.call_builtin_str(name, args, span)
    }

    fn call_builtin_str(&mut self, name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
        // `try(fn)` — call fn(), wrap result/error in {ok, value/error} map
        if name == "try" {
            let func = args.into_iter().next().unwrap_or(Value::Null);
            return match self.call_value(func, vec![], span) {
                Ok(val) => {
                    let mut map = IndexMap::new();
                    map.insert(MapKey::Str("ok".into()), Value::Bool(true));
                    map.insert(MapKey::Str("value".into()), val);
                    Ok(Value::Map(Rc::new(RefCell::new(map))))
                }
                Err(e) => {
                    let mut map = IndexMap::new();
                    map.insert(MapKey::Str("ok".into()), Value::Bool(false));
                    map.insert(MapKey::Str("error".into()), Value::Str(e.to_string()));
                    Ok(Value::Map(Rc::new(RefCell::new(map))))
                }
            };
        }

        // `append` is smart: file-append if first arg is str, list-append otherwise
        if name == "append" {
            if args.first().map(|v| matches!(v, Value::Str(_))).unwrap_or(false) {
                return stdlib::io::call("file_append", args, span);
            }
            return call_list_builtin(self, "append", args, span);
        }

        if stdlib::list::BUILTINS.contains(&name) {
            return call_list_builtin(self, name, args, span);
        }
        if stdlib::string::BUILTINS.contains(&name) {
            return stdlib::string::call(name, args, span);
        }
        if stdlib::io::BUILTINS.contains(&name) {
            return stdlib::io::call(name, args, span);
        }
        if stdlib::json::BUILTINS.contains(&name) {
            return stdlib::json::call(name, args, span);
        }
        if stdlib::net::BUILTINS.contains(&name) {
            return stdlib::net::call(name, args, span);
        }
        if stdlib::sys::BUILTINS.contains(&name) {
            return stdlib::sys::call(name, args, span);
        }
        if stdlib::csv::BUILTINS.contains(&name) {
            return stdlib::csv::call(name, args, span);
        }
        if stdlib::plot::BUILTINS.contains(&name) {
            return stdlib::plot::call(name, args, span);
        }
        if stdlib::df::BUILTINS.contains(&name) {
            return stdlib::df::call(name, args, span);
        }
        if stdlib::torch::BUILTINS.contains(&name) {
            return stdlib::torch::call(name, args, span);
        }
        if stdlib::fs::BUILTINS.contains(&name) {
            return stdlib::fs::call(name, args, span);
        }
        if stdlib::time::BUILTINS.contains(&name) {
            return stdlib::time::call(name, args, span);
        }
        if stdlib::sql::BUILTINS.contains(&name) {
            return stdlib::sql::call(name, args, span);
        }
        if stdlib::stream::BUILTINS.contains(&name) {
            return stdlib::stream::call(name, args, span);
        }
        if stdlib::arrow::BUILTINS.contains(&name) {
            return stdlib::arrow::call(name, args, span);
        }
        if stdlib::task::BUILTINS.contains(&name) {
            return stdlib::task::call(name, args, span);
        }
        if stdlib::ffi::BUILTINS.contains(&name) {
            return stdlib::ffi::call(name, args, span);
        }
        if stdlib::cluster::BUILTINS.contains(&name) {
            return stdlib::cluster::call(name, args, span);
        }
        if stdlib::wasm::BUILTINS.contains(&name) {
            return stdlib::wasm::call(name, args, span);
        }
        if stdlib::llm::BUILTINS.contains(&name) {
            return stdlib::llm::call(name, args, span);
        }
        if stdlib::vector::BUILTINS.contains(&name) {
            return stdlib::vector::call(name, args, span);
        }
        if stdlib::server::BUILTINS.contains(&name) {
            return stdlib::server::call(name, args, span);
        }
        if stdlib::image::BUILTINS.contains(&name) {
            return stdlib::image::call(name, args, span);
        }
        if stdlib::crypto::BUILTINS.contains(&name) {
            return stdlib::crypto::call(name, args, span);
        }
        if stdlib::zip_archive::BUILTINS.contains(&name) {
            return stdlib::zip_archive::call(name, args, span);
        }
        if stdlib::sklearn::BUILTINS.contains(&name) {
            return stdlib::sklearn::call(name, args, span);
        }
        if stdlib::pdf::BUILTINS.contains(&name) {
            return stdlib::pdf::call(name, args, span);
        }
        stdlib::core::call(name, args, span)
    }

    // ── Import ────────────────────────────────────────────────────────────────

    fn eval_import(&mut self, path: &str, span: &Span) -> Result<Value, CapError> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| CapError::Io { message: format!("import({path:?}): {e}"), span: span.clone() })?;

        let tokens = crate::lexer::Lexer::new(&source).tokenize_all()
            .map_err(|e| CapError::Runtime { message: format!("import({path:?}) lex error: {e}"), span: span.clone() })?;
        let mut parser = crate::parser::Parser::new(tokens);
        let stmts = parser.parse_program()
            .map_err(|e| CapError::Runtime { message: format!("import({path:?}): {e}"), span: span.clone() })?;

        self.env.push_scope();
        for stmt in &stmts {
            self.eval_stmt(stmt)?;
        }
        let bindings = self.env.current_scope_bindings();
        self.env.pop_scope();

        let mut map = IndexMap::new();
        for (k, v) in bindings {
            map.insert(MapKey::Str(k), v);
        }
        Ok(Value::Map(Rc::new(RefCell::new(map))))
    }

    // ── Binary operations ─────────────────────────────────────────────────────

    fn eval_binop(&self, op: &BinOp, l: Value, r: Value, span: &Span) -> Result<Value, CapError> {
        match op {
            BinOp::Add => match (&l, &r) {
                (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(a + b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
                (Value::Int(a),   Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
                (Value::Float(a), Value::Int(b))   => Ok(Value::Float(a + *b as f64)),
                (Value::Str(a),   Value::Str(b))   => Ok(Value::Str(format!("{a}{b}"))),
                (Value::List(a),  Value::List(b))  => {
                    let mut out = a.borrow().clone();
                    out.extend(b.borrow().iter().cloned());
                    Ok(Value::List(Rc::new(RefCell::new(out))))
                }
                _ => Err(CapError::TypeError {
                    expected: "number or str",
                    got: format!("{} and {}", l.type_name(), r.type_name()),
                    span: span.clone(),
                }),
            },
            BinOp::Sub => numeric_op!(l, r, span, -, -),
            BinOp::Mul => match (&l, &r) {
                (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(a * b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
                (Value::Int(a),   Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
                (Value::Float(a), Value::Int(b))   => Ok(Value::Float(a * *b as f64)),
                (Value::Str(s),   Value::Int(n))   => Ok(Value::Str(s.repeat(*n as usize))),
                _ => Err(CapError::TypeError {
                    expected: "number or str * int",
                    got: format!("{} and {}", l.type_name(), r.type_name()),
                    span: span.clone(),
                }),
            },
            BinOp::Div => {
                // Check for zero denominator before unpacking values
                let is_zero = matches!(&r, Value::Int(0)) || matches!(&r, Value::Float(f) if *f == 0.0);
                if is_zero {
                    return Err(CapError::Runtime { message: "division by zero".into(), span: span.clone() });
                }
                match (&l, &r) {
                    (Value::Int(a),   Value::Int(b))   => Ok(Value::Float(*a as f64 / *b as f64)),
                    (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
                    (Value::Int(a),   Value::Float(b)) => Ok(Value::Float(*a as f64 / b)),
                    (Value::Float(a), Value::Int(b))   => Ok(Value::Float(a / *b as f64)),
                    _ => Err(CapError::TypeError { expected: "number", got: l.type_name().to_string(), span: span.clone() }),
                }
            }
            BinOp::Mod => match (&l, &r) {
                (Value::Int(_),   Value::Int(0))   => Err(CapError::Runtime { message: "modulo by zero".into(), span: span.clone() }),
                (Value::Float(_), Value::Float(b)) if *b == 0.0 => Err(CapError::Runtime { message: "modulo by zero".into(), span: span.clone() }),
                (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(a % b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a % b)),
                (Value::Int(a),   Value::Float(b)) => Ok(Value::Float(*a as f64 % b)),
                (Value::Float(a), Value::Int(b))   => Ok(Value::Float(a % *b as f64)),
                _ => Err(CapError::TypeError { expected: "number", got: l.type_name().to_string(), span: span.clone() }),
            },
            BinOp::Pow => match (&l, &r) {
                (Value::Int(a),   Value::Int(b))   => Ok(Value::Float((*a as f64).powf(*b as f64))),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.powf(*b))),
                (Value::Int(a),   Value::Float(b)) => Ok(Value::Float((*a as f64).powf(*b))),
                (Value::Float(a), Value::Int(b))   => Ok(Value::Float(a.powf(*b as f64))),
                _ => Err(CapError::TypeError { expected: "number", got: l.type_name().to_string(), span: span.clone() }),
            },
            BinOp::Eq    => Ok(Value::Bool(l == r)),
            BinOp::NotEq => Ok(Value::Bool(l != r)),
            BinOp::Lt    => cmp_op!(l, r, span, <),
            BinOp::Gt    => cmp_op!(l, r, span, >),
            BinOp::LtEq  => cmp_op!(l, r, span, <=),
            BinOp::GtEq  => cmp_op!(l, r, span, >=),
            BinOp::And | BinOp::Or => unreachable!("and/or short-circuited above"),
            BinOp::Compose => {
                // `f >> g` produces a new function that calls f then g.
                Ok(Value::Function(Rc::new(FunctionValue {
                    name: None,
                    params: vec![Param { name: "__x".to_string(), default: None }],
                    body: FunctionBody::BuiltinCompose { f: Box::new(l), g: Box::new(r) },
                    closure: self.env.snapshot(),
                })))
            }
        }
    }

    // ── String interpolation ──────────────────────────────────────────────────

    fn eval_interp_str(&mut self, parts: &[StrPart], span: &Span) -> Result<Value, CapError> {
        let mut out = String::new();
        for part in parts {
            match part {
                StrPart::Literal(s) => out.push_str(s),
                StrPart::Interp(src) => {
                    // Re-parse and evaluate the interpolated expression.
                    let tokens = crate::lexer::Lexer::new(src).tokenize_all()
                        .map_err(|e| CapError::Runtime { message: format!("interpolation error: {e}"), span: span.clone() })?;
                    let mut parser = crate::parser::Parser::new(tokens);
                    let expr = parser.parse_expr(0)
                        .map_err(|e| CapError::Runtime { message: format!("interpolation error: {e}"), span: span.clone() })?;
                    // BUG-12: ensure no unconsumed tokens remain after the expression
                    if !parser.is_at_eof() {
                        return Err(CapError::Runtime {
                            message: format!("interpolation error: unexpected tokens after expression in {{{src}}}"),
                            span: span.clone(),
                        });
                    }
                    let val = self.eval_expr(&expr)?;
                    out.push_str(&val.display());
                }
            }
        }
        Ok(Value::Str(out))
    }
}

// ── Method partial helper ─────────────────────────────────────────────────────

fn method_partial(receiver: Value, method: &str, snap: &env::EnvSnapshot) -> Value {
    Value::Function(Rc::new(FunctionValue {
        name: Some(format!("{}.{method}", receiver.type_name())),
        params: vec![],
        body: FunctionBody::MethodPartial {
            receiver: Box::new(receiver),
            method: method.to_string(),
        },
        closure: snap.clone(),
    }))
}

// ── Pattern matching ──────────────────────────────────────────────────────────

/// Try to match `val` against `pat`. Returns `Some(bindings)` on success,
/// `None` on failure. Bindings is a list of (name, value) pairs to add to scope.
fn match_pattern(pat: &Pattern, val: &Value) -> Option<Vec<(String, Value)>> {
    match pat {
        Pattern::Wildcard => Some(vec![]),
        Pattern::Literal(lit) => {
            let lit_val: Value = lit.clone().into();
            if lit_val == *val { Some(vec![]) } else { None }
        }
        Pattern::Bind(name) => Some(vec![(name.clone(), val.clone())]),
        Pattern::Or(alts) => {
            for alt in alts {
                if let Some(bindings) = match_pattern(alt, val) {
                    return Some(bindings);
                }
            }
            None
        }
        Pattern::Guard { pattern, guard } => {
            // Guard evaluation happens in the interpreter — not supported in free fn.
            // For now, guards require the pattern to match first (guard is checked in caller).
            match_pattern(pattern, val)
        }
    }
}

// ── List builtin dispatch (needs mutable self) ────────────────────────────────

fn call_list_builtin(interp: &mut Interpreter, name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    stdlib::list::call(name, args, span, |func, cb_args, cb_span| {
        interp.call_value(func, cb_args, cb_span)
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn normalize_index(idx: i64, len: usize, span: &Span) -> Result<usize, CapError> {
    let len_i = len as i64;
    let pos = if idx < 0 { len_i + idx } else { idx };
    if pos < 0 || pos >= len_i {
        Err(CapError::IndexOutOfBounds { index: idx, len, span: span.clone() })
    } else {
        Ok(pos as usize)
    }
}

// ── Macros for numeric/comparison ops ────────────────────────────────────────

macro_rules! numeric_op {
    ($l:expr, $r:expr, $span:expr, $op_int:tt, $op_float:tt) => {
        match (&$l, &$r) {
            (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(a $op_int b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a $op_float b)),
            (Value::Int(a),   Value::Float(b)) => Ok(Value::Float((*a as f64) $op_float b)),
            (Value::Float(a), Value::Int(b))   => Ok(Value::Float(a $op_float (*b as f64))),
            _ => Err(CapError::TypeError { expected: "number", got: $l.type_name().to_string(), span: $span.clone() }),
        }
    }
}

macro_rules! cmp_op {
    ($l:expr, $r:expr, $span:expr, $op:tt) => {
        $l.partial_cmp(&$r)
            .map(|ord| Value::Bool(ord $op std::cmp::Ordering::Equal))
            .ok_or_else(|| CapError::TypeError {
                expected: "comparable value",
                got: $l.type_name().to_string(),
                span: $span.clone(),
            })
    }
}

use {cmp_op, numeric_op};
