use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &[
    "map", "filter", "reduce", "each", "tap",
    "sort", "sort_by", "zip", "flatten",
    "first", "last", "any", "all", "find",
    "enumerate", "reverse", "append", "extend",
    "sum", "min", "max",
];

/// Call a list builtin. The interpreter passes a callback for higher-order functions.
pub fn call<F>(
    name: &str,
    mut args: Vec<Value>,
    span: &Span,
    mut call_fn: F,
) -> Result<Value, CapError>
where
    F: FnMut(Value, Vec<Value>, &Span) -> Result<Value, CapError>,
{
    match name {
        "map" => {
            let (list, func) = two_args(name, &mut args, span)?;
            let list = list.as_list(span)?;
            let items: Vec<Value> = list
                .borrow()
                .iter()
                .map(|item| call_fn(func.clone(), vec![item.clone()], span))
                .collect::<Result<_, _>>()?;
            Ok(Value::List(Rc::new(RefCell::new(items))))
        }
        "filter" => {
            let (list, func) = two_args(name, &mut args, span)?;
            let list = list.as_list(span)?;
            let items: Vec<Value> = list
                .borrow()
                .iter()
                .filter_map(|item| {
                    match call_fn(func.clone(), vec![item.clone()], span) {
                        Ok(v) if v.is_truthy() => Some(Ok(item.clone())),
                        Ok(_) => None,
                        Err(e) => Some(Err(e)),
                    }
                })
                .collect::<Result<_, _>>()?;
            Ok(Value::List(Rc::new(RefCell::new(items))))
        }
        "reduce" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let func = args.remove(1);
            let list = args.remove(0).as_list(span)?;
            let borrowed = list.borrow();
            let mut items = borrowed.iter();
            let mut acc = if args.is_empty() {
                items.next().cloned().ok_or_else(|| CapError::Runtime {
                    message: "reduce() on empty list requires an initial value".into(),
                    span: span.clone(),
                })?
            } else {
                args.remove(0)
            };
            for item in items {
                acc = call_fn(func.clone(), vec![acc, item.clone()], span)?;
            }
            Ok(acc)
        }
        "each" => {
            let (list, func) = two_args(name, &mut args, span)?;
            let list = list.as_list(span)?;
            for item in list.borrow().iter() {
                call_fn(func.clone(), vec![item.clone()], span)?;
            }
            Ok(Value::Null)
        }
        "tap" => {
            // tap(list, fn) — call fn(list), return list unchanged
            let (list, func) = two_args(name, &mut args, span)?;
            call_fn(func, vec![list.clone()], span)?;
            Ok(list)
        }
        "sort" => {
            let list = one_arg(name, &mut args, span)?.as_list(span)?;
            let mut items = list.borrow().clone();
            items.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            Ok(Value::List(Rc::new(RefCell::new(items))))
        }
        "sort_by" => {
            let (list, func) = two_args(name, &mut args, span)?;
            let list = list.as_list(span)?;
            let mut items = list.borrow().clone();
            let mut err: Option<CapError> = None;
            items.sort_by(|a, b| {
                if err.is_some() { return std::cmp::Ordering::Equal; }
                let ka = call_fn(func.clone(), vec![a.clone()], span);
                let kb = call_fn(func.clone(), vec![b.clone()], span);
                match (ka, kb) {
                    (Ok(ka), Ok(kb)) => ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal),
                    (Err(e), _) | (_, Err(e)) => { err = Some(e); std::cmp::Ordering::Equal }
                }
            });
            if let Some(e) = err { return Err(e); }
            Ok(Value::List(Rc::new(RefCell::new(items))))
        }
        "reverse" => {
            let list = one_arg(name, &mut args, span)?.as_list(span)?;
            let mut items = list.borrow().clone();
            items.reverse();
            Ok(Value::List(Rc::new(RefCell::new(items))))
        }
        "zip" => {
            let (a, b) = two_args(name, &mut args, span)?;
            let a = a.as_list(span)?;
            let b = b.as_list(span)?;
            let pairs: Vec<Value> = a.borrow().iter().zip(b.borrow().iter())
                .map(|(x, y)| Value::Tuple(vec![x.clone(), y.clone()]))
                .collect();
            Ok(Value::List(Rc::new(RefCell::new(pairs))))
        }
        "flatten" => {
            let list = one_arg(name, &mut args, span)?.as_list(span)?;
            let mut out = Vec::new();
            for item in list.borrow().iter() {
                match item {
                    Value::List(inner) => out.extend(inner.borrow().iter().cloned()),
                    other => out.push(other.clone()),
                }
            }
            Ok(Value::List(Rc::new(RefCell::new(out))))
        }
        "first" => {
            let list = one_arg(name, &mut args, span)?.as_list(span)?;
            let v = list.borrow().first().cloned().unwrap_or(Value::Null);
            Ok(v)
        }
        "last" => {
            let list = one_arg(name, &mut args, span)?.as_list(span)?;
            let v = list.borrow().last().cloned().unwrap_or(Value::Null);
            Ok(v)
        }
        "any" => {
            let (list, func) = two_args(name, &mut args, span)?;
            let list = list.as_list(span)?;
            for item in list.borrow().iter() {
                if call_fn(func.clone(), vec![item.clone()], span)?.is_truthy() {
                    return Ok(Value::Bool(true));
                }
            }
            Ok(Value::Bool(false))
        }
        "all" => {
            let (list, func) = two_args(name, &mut args, span)?;
            let list = list.as_list(span)?;
            for item in list.borrow().iter() {
                if !call_fn(func.clone(), vec![item.clone()], span)?.is_truthy() {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(Value::Bool(true))
        }
        "find" => {
            let (list, func) = two_args(name, &mut args, span)?;
            let list = list.as_list(span)?;
            for item in list.borrow().iter() {
                if call_fn(func.clone(), vec![item.clone()], span)?.is_truthy() {
                    return Ok(item.clone());
                }
            }
            Ok(Value::Null)
        }
        "enumerate" => {
            let list = one_arg(name, &mut args, span)?.as_list(span)?;
            let pairs: Vec<Value> = list.borrow().iter().enumerate()
                .map(|(i, v)| Value::Tuple(vec![Value::Int(i as i64), v.clone()]))
                .collect();
            Ok(Value::List(Rc::new(RefCell::new(pairs))))
        }
        "append" => {
            let (list, item) = two_args(name, &mut args, span)?;
            let list_ref = list.as_list(span)?;
            list_ref.borrow_mut().push(item);
            Ok(list) // return the (mutated) list
        }
        "extend" => {
            let (list, other) = two_args(name, &mut args, span)?;
            let list_ref = list.as_list(span)?;
            let other_ref = other.as_list(span)?;
            list_ref.borrow_mut().extend(other_ref.borrow().iter().cloned());
            Ok(list)
        }
        "sum" => {
            let list = one_arg(name, &mut args, span)?.as_list(span)?;
            // Accumulate as i64 while all values are ints; switch to f64 on first float.
            let mut int_total: i64 = 0;
            let mut float_total: f64 = 0.0;
            let mut has_float = false;
            for v in list.borrow().iter() {
                match v {
                    Value::Int(n) => {
                        if has_float { float_total += *n as f64; }
                        else { int_total = int_total.saturating_add(*n); }
                    }
                    Value::Float(f) => {
                        if !has_float {
                            float_total = int_total as f64;
                            has_float = true;
                        }
                        float_total += f;
                    }
                    other => return Err(CapError::TypeError { expected: "number", got: other.type_name().to_string(), span: span.clone() }),
                }
            }
            if has_float { Ok(Value::Float(float_total)) } else { Ok(Value::Int(int_total)) }
        }
        "min" => {
            let list = one_arg(name, &mut args, span)?.as_list(span)?;
            let borrowed = list.borrow();
            borrowed.iter().cloned().reduce(|a, b| {
                if a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal) == std::cmp::Ordering::Less { a } else { b }
            }).ok_or_else(|| CapError::Runtime { message: "min() on empty list".into(), span: span.clone() })
        }
        "max" => {
            let list = one_arg(name, &mut args, span)?.as_list(span)?;
            let borrowed = list.borrow();
            borrowed.iter().cloned().reduce(|a, b| {
                if a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal) == std::cmp::Ordering::Greater { a } else { b }
            }).ok_or_else(|| CapError::Runtime { message: "max() on empty list".into(), span: span.clone() })
        }
        _ => Err(CapError::Runtime { message: format!("unknown list builtin: {name}"), span: span.clone() }),
    }
}

fn one_arg(name: &str, args: &mut Vec<Value>, span: &Span) -> Result<Value, CapError> {
    if args.is_empty() {
        return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
    }
    Ok(args.remove(0))
}

fn two_args(name: &str, args: &mut Vec<Value>, span: &Span) -> Result<(Value, Value), CapError> {
    if args.len() < 2 {
        return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
    }
    let b = args.remove(1);
    let a = args.remove(0);
    Ok((a, b))
}
