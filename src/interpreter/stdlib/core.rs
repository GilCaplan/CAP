use crate::error::{CapError, Span};
use crate::interpreter::value::{MapKey, Value};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &[
    "print", "println", "str", "int", "float", "bool",
    "len", "range", "type", "repr", "error",
    "keys", "values", "items",
    "set", "from_pairs", "merge",
    "try",
];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    match name {
        "print" => {
            let parts: Vec<String> = args.iter().map(|v| v.display()).collect();
            print!("{}", parts.join(" "));
            Ok(Value::Null)
        }
        "println" => {
            let parts: Vec<String> = args.iter().map(|v| v.display()).collect();
            println!("{}", parts.join(" "));
            Ok(Value::Null)
        }
        "str" => {
            let v = args.into_iter().next().unwrap_or(Value::Null);
            Ok(Value::Str(v.display()))
        }
        "repr" => {
            let v = args.into_iter().next().unwrap_or(Value::Null);
            Ok(Value::Str(v.repr()))
        }
        "int" => {
            let v = args.into_iter().next().unwrap_or(Value::Null);
            match v {
                Value::Int(n)   => Ok(Value::Int(n)),
                Value::Float(f) => Ok(Value::Int(f as i64)),
                Value::Str(s)   => s.trim().parse::<i64>()
                    .map(Value::Int)
                    .map_err(|_| CapError::Runtime { message: format!("cannot convert {s:?} to int"), span: span.clone() }),
                Value::Bool(b)  => Ok(Value::Int(if b { 1 } else { 0 })),
                other           => Err(CapError::TypeError { expected: "int-convertible", got: other.type_name().to_string(), span: span.clone() }),
            }
        }
        "float" => {
            let v = args.into_iter().next().unwrap_or(Value::Null);
            match v {
                Value::Float(f) => Ok(Value::Float(f)),
                Value::Int(n)   => Ok(Value::Float(n as f64)),
                Value::Str(s)   => s.trim().parse::<f64>()
                    .map(Value::Float)
                    .map_err(|_| CapError::Runtime { message: format!("cannot convert {s:?} to float"), span: span.clone() }),
                other           => Err(CapError::TypeError { expected: "float-convertible", got: other.type_name().to_string(), span: span.clone() }),
            }
        }
        "bool" => {
            let v = args.into_iter().next().unwrap_or(Value::Null);
            Ok(Value::Bool(v.is_truthy()))
        }
        "type" => {
            let v = args.into_iter().next().unwrap_or(Value::Null);
            Ok(Value::Str(v.type_name().to_string()))
        }
        "len" => {
            let v = args.into_iter().next().unwrap_or(Value::Null);
            match v {
                Value::Str(s)  => Ok(Value::Int(s.chars().count() as i64)),
                Value::List(l) => Ok(Value::Int(l.borrow().len() as i64)),
                Value::Map(m)  => Ok(Value::Int(m.borrow().len() as i64)),
                Value::Tuple(t)=> Ok(Value::Int(t.len() as i64)),
                other          => Err(CapError::TypeError { expected: "str/list/map/tuple", got: other.type_name().to_string(), span: span.clone() }),
            }
        }
        "range" => {
            match args.as_slice() {
                [Value::Int(end)] => {
                    let items = (0..*end).map(Value::Int).collect();
                    Ok(Value::List(Rc::new(RefCell::new(items))))
                }
                [Value::Int(start), Value::Int(end)] => {
                    let items = (*start..*end).map(Value::Int).collect();
                    Ok(Value::List(Rc::new(RefCell::new(items))))
                }
                [Value::Int(start), Value::Int(end), Value::Int(step)] => {
                    if *step == 0 {
                        return Err(CapError::Runtime { message: "range() step cannot be zero".into(), span: span.clone() });
                    }
                    let mut items = Vec::new();
                    let mut i = *start;
                    if *step > 0 { while i < *end { items.push(Value::Int(i)); i += step; } }
                    else { while i > *end { items.push(Value::Int(i)); i += step; } }
                    Ok(Value::List(Rc::new(RefCell::new(items))))
                }
                _ => Err(CapError::Runtime { message: "range(end) or range(start, end) or range(start, end, step)".into(), span: span.clone() }),
            }
        }
        "error" => {
            let msg = args.into_iter().next().map(|v| v.display()).unwrap_or_default();
            Err(CapError::Runtime { message: msg, span: span.clone() })
        }
        "keys" => {
            let m = args.into_iter().next().unwrap_or(Value::Null);
            let map = m.as_map(span)?;
            let keys: Vec<Value> = map.borrow().keys().map(|k| Value::Str(k.to_string())).collect();
            Ok(Value::List(Rc::new(RefCell::new(keys))))
        }
        "values" => {
            let m = args.into_iter().next().unwrap_or(Value::Null);
            let map = m.as_map(span)?;
            let vals: Vec<Value> = map.borrow().values().cloned().collect();
            Ok(Value::List(Rc::new(RefCell::new(vals))))
        }
        "items" => {
            let m = args.into_iter().next().unwrap_or(Value::Null);
            let map = m.as_map(span)?;
            let pairs: Vec<Value> = map.borrow().iter().map(|(k, v)| {
                Value::Tuple(vec![Value::Str(k.to_string()), v.clone()])
            }).collect();
            Ok(Value::List(Rc::new(RefCell::new(pairs))))
        }
        "set" => {
            // set(collection, key, value) — mutate in-place, return value
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let mut args = args;
            let collection = args.remove(0);
            let key = args.remove(0);
            let val = args.remove(0);
            match &collection {
                Value::Map(m) => {
                    let map_key = key.to_map_key(span)?;
                    m.borrow_mut().insert(map_key, val.clone());
                    Ok(val)
                }
                Value::List(l) => {
                    let idx = key.as_int(span)?;
                    let mut lst = l.borrow_mut();
                    let len = lst.len() as i64;
                    let pos = if idx < 0 { len + idx } else { idx };
                    if pos < 0 || pos >= len {
                        return Err(CapError::IndexOutOfBounds { index: idx, len: lst.len(), span: span.clone() });
                    }
                    lst[pos as usize] = val.clone();
                    Ok(val)
                }
                other => Err(CapError::TypeError { expected: "map or list", got: other.type_name().to_string(), span: span.clone() }),
            }
        }
        "from_pairs" => {
            // from_pairs([(k, v), ...]) -> {k: v, ...}
            let v = args.into_iter().next().unwrap_or(Value::Null);
            let list = v.as_list(span)?;
            let mut map = IndexMap::new();
            for item in list.borrow().iter() {
                match item {
                    Value::Tuple(t) if t.len() >= 2 => {
                        let key = t[0].to_map_key(span)?;
                        map.insert(key, t[1].clone());
                    }
                    Value::List(inner) => {
                        let borrowed = inner.borrow();
                        if borrowed.len() < 2 {
                            return Err(CapError::Runtime { message: "from_pairs: each pair must have at least 2 elements".into(), span: span.clone() });
                        }
                        let key = borrowed[0].to_map_key(span)?;
                        map.insert(key, borrowed[1].clone());
                    }
                    other => return Err(CapError::Runtime { message: format!("from_pairs: expected (key, value) pair, got {}", other.repr()), span: span.clone() }),
                }
            }
            Ok(Value::Map(Rc::new(RefCell::new(map))))
        }
        "merge" => {
            // merge(base_map, overlay_map) -> new map (overlay keys win)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let mut args = args;
            let base = args.remove(0).as_map(span)?;
            let overlay = args.remove(0).as_map(span)?;
            let mut merged = base.borrow().clone();
            for (k, v) in overlay.borrow().iter() {
                merged.insert(k.clone(), v.clone());
            }
            Ok(Value::Map(Rc::new(RefCell::new(merged))))
        }
        "try" => {
            // Handled specially in the interpreter (needs call_value).
            // Should never reach here — the interpreter intercepts "try" first.
            Err(CapError::Runtime { message: "try: internal dispatch error".into(), span: span.clone() })
        }
        _ => Err(CapError::Runtime { message: format!("unknown builtin: {name}"), span: span.clone() }),
    }
}
