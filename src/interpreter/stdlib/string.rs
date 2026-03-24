use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &[
    "split", "join", "trim", "trim_start", "trim_end",
    "upper", "lower", "replace", "contains",
    "starts_with", "ends_with", "lines", "chars",
];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    match name {
        "split" => {
            let (s, sep) = str_str_args(name, args, span)?;
            let parts: Vec<Value> = s.split(sep.as_str()).map(|p| Value::Str(p.to_string())).collect();
            Ok(Value::List(Rc::new(RefCell::new(parts))))
        }
        "join" => {
            // join(list, sep)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let mut args = args;
            let list = args.remove(0).as_list(span)?;
            let sep = args.remove(0);
            let sep_str = sep.as_str(span)?;
            let parts: Vec<String> = list.borrow().iter().map(|v| v.display()).collect();
            Ok(Value::Str(parts.join(sep_str)))
        }
        "trim"       => { let s = one_str(args, span)?; Ok(Value::Str(s.trim().to_string())) }
        "trim_start" => { let s = one_str(args, span)?; Ok(Value::Str(s.trim_start().to_string())) }
        "trim_end"   => { let s = one_str(args, span)?; Ok(Value::Str(s.trim_end().to_string())) }
        "upper"      => { let s = one_str(args, span)?; Ok(Value::Str(s.to_uppercase())) }
        "lower"      => { let s = one_str(args, span)?; Ok(Value::Str(s.to_lowercase())) }
        "lines"      => {
            let s = one_str(args, span)?;
            let items: Vec<Value> = s.lines().map(|l| Value::Str(l.to_string())).collect();
            Ok(Value::List(Rc::new(RefCell::new(items))))
        }
        "chars" => {
            let s = one_str(args, span)?;
            let items: Vec<Value> = s.chars().map(|c| Value::Str(c.to_string())).collect();
            Ok(Value::List(Rc::new(RefCell::new(items))))
        }
        "replace" => {
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let mut args = args;
            let s    = str_val(args.remove(0), span)?;
            let from = str_val(args.remove(0), span)?;
            let to   = str_val(args.remove(0), span)?;
            Ok(Value::Str(s.replace(from.as_str(), to.as_str())))
        }
        "contains" => {
            let (s, sub) = str_str_args(name, args, span)?;
            Ok(Value::Bool(s.contains(sub.as_str())))
        }
        "starts_with" => {
            let (s, pre) = str_str_args(name, args, span)?;
            Ok(Value::Bool(s.starts_with(pre.as_str())))
        }
        "ends_with" => {
            let (s, suf) = str_str_args(name, args, span)?;
            Ok(Value::Bool(s.ends_with(suf.as_str())))
        }
        _ => Err(CapError::Runtime { message: format!("unknown string builtin: {name}"), span: span.clone() }),
    }
}

fn one_str(args: Vec<Value>, span: &Span) -> Result<String, CapError> {
    let v = args.into_iter().next().unwrap_or(Value::Null);
    Ok(v.as_str(span)?.to_string())
}

fn str_val(v: Value, span: &Span) -> Result<String, CapError> {
    Ok(v.as_str(span)?.to_string())
}

fn str_str_args(name: &str, args: Vec<Value>, span: &Span) -> Result<(String, String), CapError> {
    if args.len() < 2 {
        return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
    }
    let mut args = args;
    let b = str_val(args.remove(1), span)?;
    let a = str_val(args.remove(0), span)?;
    Ok((a, b))
}
