use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &["read", "write", "file_append", "exists", "ls", "input"];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    match name {
        "read" => {
            let path = str_arg(args, span)?;
            std::fs::read_to_string(&path).map(Value::Str).map_err(|e| CapError::Io {
                message: format!("read({path:?}): {e}"),
                span: span.clone(),
            })
        }
        "write" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let mut args = args;
            let path    = str_arg_val(args.remove(0), span)?;
            let content = args.remove(0).display();
            std::fs::write(&path, &content).map_err(|e| CapError::Io {
                message: format!("write({path:?}): {e}"),
                span: span.clone(),
            })?;
            Ok(Value::Null)
        }
        "file_append" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let mut args = args;
            let path    = str_arg_val(args.remove(0), span)?;
            let content = args.remove(0).display();
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new().append(true).create(true).open(&path)
                .map_err(|e| CapError::Io { message: format!("append({path:?}): {e}"), span: span.clone() })?;
            file.write_all(content.as_bytes()).map_err(|e| CapError::Io { message: e.to_string(), span: span.clone() })?;
            Ok(Value::Null)
        }
        "exists" => {
            let path = str_arg(args, span)?;
            Ok(Value::Bool(std::path::Path::new(&path).exists()))
        }
        "ls" => {
            let path = if args.is_empty() { ".".to_string() } else { str_arg(args, span)? };
            let entries = std::fs::read_dir(&path).map_err(|e| CapError::Io {
                message: format!("ls({path:?}): {e}"),
                span: span.clone(),
            })?;
            let mut names = Vec::new();
            for entry in entries.flatten() {
                names.push(Value::Str(entry.file_name().to_string_lossy().into_owned()));
            }
            Ok(Value::List(Rc::new(RefCell::new(names))))
        }
        "input" => {
            let prompt = if args.is_empty() { String::new() } else { str_arg(args, span)? };
            if !prompt.is_empty() { print!("{prompt}"); use std::io::Write; std::io::stdout().flush().ok(); }
            let mut line = String::new();
            std::io::stdin().read_line(&mut line).map_err(|e| CapError::Io { message: e.to_string(), span: span.clone() })?;
            Ok(Value::Str(line.trim_end_matches('\n').trim_end_matches('\r').to_string()))
        }
        _ => Err(CapError::Runtime { message: format!("unknown io builtin: {name}"), span: span.clone() }),
    }
}

fn str_arg(args: Vec<Value>, span: &Span) -> Result<String, CapError> {
    let v = args.into_iter().next().unwrap_or(Value::Null);
    str_arg_val(v, span)
}

fn str_arg_val(v: Value, span: &Span) -> Result<String, CapError> {
    Ok(v.as_str(span)?.to_string())
}
