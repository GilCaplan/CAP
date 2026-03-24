use crate::error::{CapError, Span};
use crate::interpreter::value::{MapKey, Value};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &[
    "csv_read", "csv_read_raw", "csv_write", "csv_parse",
];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "csv_read" => {
            // csv_read(path) → list of maps (first row = headers)
            // csv_read(path, sep) → custom separator
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let sep = sep_from_args(&mut args, span)?;
            let content = std::fs::read_to_string(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            parse_csv_with_headers(&content, sep, span)
        }
        "csv_read_raw" => {
            // csv_read_raw(path) → list of lists (no header interpretation)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let sep = sep_from_args(&mut args, span)?;
            let content = std::fs::read_to_string(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            parse_csv_raw(&content, sep, span)
        }
        "csv_write" => {
            // csv_write(path, list_of_maps)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let data = args.remove(0);
            write_csv(&path, data, span)
        }
        "csv_parse" => {
            // csv_parse(str) → list of maps (first row = headers)
            // csv_parse(str, sep) → custom separator
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let content = args.remove(0).as_str(span)?.to_string();
            let sep = sep_from_args(&mut args, span)?;
            parse_csv_with_headers(&content, sep, span)
        }
        _ => Err(CapError::Runtime { message: format!("unknown csv builtin: {name}"), span: span.clone() }),
    }
}

fn sep_from_args(args: &mut Vec<Value>, span: &Span) -> Result<u8, CapError> {
    if args.is_empty() {
        return Ok(b',');
    }
    let s = args.remove(0).as_str(span)?.to_string();
    Ok(s.chars().next().map(|c| c as u8).unwrap_or(b','))
}

fn parse_csv_with_headers(content: &str, sep: u8, span: &Span) -> Result<Value, CapError> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(sep)
        .from_reader(content.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| CapError::Runtime { message: format!("csv: {e}"), span: span.clone() })?
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut rows = Vec::new();
    for result in rdr.records() {
        let record = result
            .map_err(|e| CapError::Runtime { message: format!("csv: {e}"), span: span.clone() })?;
        let mut map = IndexMap::new();
        for (i, field) in record.iter().enumerate() {
            let key = headers.get(i).map(String::as_str).unwrap_or("");
            map.insert(MapKey::Str(key.to_string()), infer_value(field));
        }
        rows.push(Value::Map(Rc::new(RefCell::new(map))));
    }
    Ok(Value::List(Rc::new(RefCell::new(rows))))
}

fn parse_csv_raw(content: &str, sep: u8, span: &Span) -> Result<Value, CapError> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(sep)
        .has_headers(false)
        .from_reader(content.as_bytes());
    let mut rows = Vec::new();
    for result in rdr.records() {
        let record = result
            .map_err(|e| CapError::Runtime { message: format!("csv: {e}"), span: span.clone() })?;
        let row: Vec<Value> = record.iter().map(infer_value).collect();
        rows.push(Value::List(Rc::new(RefCell::new(row))));
    }
    Ok(Value::List(Rc::new(RefCell::new(rows))))
}

fn write_csv(path: &str, data: Value, span: &Span) -> Result<Value, CapError> {
    let list = data.as_list(span)?;
    let borrowed = list.borrow();
    if borrowed.is_empty() {
        std::fs::write(path, "")
            .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
        return Ok(Value::Null);
    }
    // Collect column names from first row
    let first = &borrowed[0];
    let headers: Vec<String> = match first {
        Value::Map(m) => m.borrow().keys().map(|k| k.to_string()).collect(),
        other => return Err(CapError::TypeError {
            expected: "list of maps",
            got: other.type_name().to_string(),
            span: span.clone(),
        }),
    };
    let mut wtr = csv::WriterBuilder::new()
        .from_path(path)
        .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
    wtr.write_record(&headers)
        .map_err(|e| CapError::Runtime { message: format!("csv write: {e}"), span: span.clone() })?;
    for row in borrowed.iter() {
        if let Value::Map(m) = row {
            let vals: Vec<String> = headers.iter().map(|h| {
                m.borrow()
                    .get(&MapKey::Str(h.clone()))
                    .map(|v| v.display())
                    .unwrap_or_default()
            }).collect();
            wtr.write_record(&vals)
                .map_err(|e| CapError::Runtime { message: format!("csv write: {e}"), span: span.clone() })?;
        }
    }
    wtr.flush()
        .map_err(|e| CapError::Runtime { message: format!("csv flush: {e}"), span: span.clone() })?;
    Ok(Value::Null)
}

/// Try to infer int/float; fall back to string.
fn infer_value(s: &str) -> Value {
    if let Ok(n) = s.trim().parse::<i64>() { return Value::Int(n); }
    if let Ok(f) = s.trim().parse::<f64>() { return Value::Float(f); }
    Value::Str(s.to_string())
}
