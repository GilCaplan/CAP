use crate::error::{CapError, Span};
use crate::interpreter::value::{MapKey, Value};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &[
    "json_parse", "json_stringify",
    "yaml_parse", "yaml_stringify",
    "toml_parse", "toml_stringify",
];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    match name {
        "json_parse" => {
            let s = args.into_iter().next().unwrap_or(Value::Null);
            let s = s.as_str(span)?.to_string();
            let v: serde_json::Value = serde_json::from_str(&s)
                .map_err(|e| CapError::Json { message: e.to_string(), span: span.clone() })?;
            Ok(json_to_value(v, span)?)
        }
        "json_stringify" => {
            let v = args.into_iter().next().unwrap_or(Value::Null);
            let j = value_to_json(&v, span)?;
            Ok(Value::Str(j.to_string()))
        }
        "yaml_parse" => {
            let s = args.into_iter().next().unwrap_or(Value::Null);
            let s = s.as_str(span)?.to_string();
            let v: serde_json::Value = serde_yaml::from_str(&s)
                .map_err(|e| CapError::Runtime { message: format!("yaml_parse: {e}"), span: span.clone() })?;
            json_to_value(v, span)
        }
        "yaml_stringify" => {
            let v = args.into_iter().next().unwrap_or(Value::Null);
            let j = value_to_json(&v, span)?;
            let yaml = serde_yaml::to_string(&j)
                .map_err(|e| CapError::Runtime { message: format!("yaml_stringify: {e}"), span: span.clone() })?;
            Ok(Value::Str(yaml))
        }
        "toml_parse" => {
            let s = args.into_iter().next().unwrap_or(Value::Null);
            let s = s.as_str(span)?.to_string();
            let v: toml::Value = toml::from_str(&s)
                .map_err(|e| CapError::Runtime { message: format!("toml_parse: {e}"), span: span.clone() })?;
            let j = toml_to_json(v);
            json_to_value(j, span)
        }
        "toml_stringify" => {
            let v = args.into_iter().next().unwrap_or(Value::Null);
            let j = value_to_json(&v, span)?;
            let t = json_to_toml(j)
                .ok_or_else(|| CapError::Runtime { message: "toml_stringify: cannot represent value as TOML".into(), span: span.clone() })?;
            let s = toml::to_string(&t)
                .map_err(|e| CapError::Runtime { message: format!("toml_stringify: {e}"), span: span.clone() })?;
            Ok(Value::Str(s))
        }
        _ => Err(CapError::Runtime { message: format!("unknown json builtin: {name}"), span: span.clone() }),
    }
}

/// Convert a `serde_json::Value` into a cap `Value`.
pub fn json_to_value(v: serde_json::Value, span: &Span) -> Result<Value, CapError> {
    match v {
        serde_json::Value::Null        => Ok(Value::Null),
        serde_json::Value::Bool(b)     => Ok(Value::Bool(b)),
        serde_json::Value::Number(n)   => {
            if let Some(i) = n.as_i64()  { return Ok(Value::Int(i)); }
            if let Some(f) = n.as_f64()  { return Ok(Value::Float(f)); }
            Err(CapError::Json { message: format!("unrepresentable number: {n}"), span: span.clone() })
        }
        serde_json::Value::String(s)   => Ok(Value::Str(s)),
        serde_json::Value::Array(arr)  => {
            let items: Result<Vec<Value>, _> = arr.into_iter()
                .map(|v| json_to_value(v, span))
                .collect();
            Ok(Value::List(Rc::new(RefCell::new(items?))))
        }
        serde_json::Value::Object(obj) => {
            let mut map = IndexMap::new();
            for (k, v) in obj {
                map.insert(MapKey::Str(k), json_to_value(v, span)?);
            }
            Ok(Value::Map(Rc::new(RefCell::new(map))))
        }
    }
}

/// Convert a cap `Value` into a `serde_json::Value`.
pub fn value_to_json(v: &Value, span: &Span) -> Result<serde_json::Value, CapError> {
    match v {
        Value::Null        => Ok(serde_json::Value::Null),
        Value::Bool(b)     => Ok(serde_json::Value::Bool(*b)),
        Value::Int(n)      => Ok(serde_json::Value::Number((*n).into())),
        Value::Float(f)    => {
            serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .ok_or_else(|| CapError::Json { message: format!("cannot serialize float: {f}"), span: span.clone() })
        }
        Value::Str(s)      => Ok(serde_json::Value::String(s.clone())),
        Value::List(l)     => {
            let items: Result<Vec<_>, _> = l.borrow().iter()
                .map(|v| value_to_json(v, span))
                .collect();
            Ok(serde_json::Value::Array(items?))
        }
        Value::Map(m)      => {
            let mut obj = serde_json::Map::new();
            for (k, v) in m.borrow().iter() {
                obj.insert(k.to_string(), value_to_json(v, span)?);
            }
            Ok(serde_json::Value::Object(obj))
        }
        Value::Tuple(t)    => {
            let items: Result<Vec<_>, _> = t.iter()
                .map(|v| value_to_json(v, span))
                .collect();
            Ok(serde_json::Value::Array(items?))
        }
        Value::Function(_) | Value::BuiltinFn(_) => {
            Err(CapError::Json { message: "cannot serialize function to JSON".into(), span: span.clone() })
        }
    }
}

/// Convert `toml::Value` → `serde_json::Value` (for uniform handling)
fn toml_to_json(v: toml::Value) -> serde_json::Value {
    match v {
        toml::Value::String(s)   => serde_json::Value::String(s),
        toml::Value::Integer(n)  => serde_json::Value::Number(n.into()),
        toml::Value::Float(f)    => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        toml::Value::Boolean(b)  => serde_json::Value::Bool(b),
        toml::Value::Array(arr)  => serde_json::Value::Array(arr.into_iter().map(toml_to_json).collect()),
        toml::Value::Table(tbl)  => {
            let mut obj = serde_json::Map::new();
            for (k, v) in tbl { obj.insert(k, toml_to_json(v)); }
            serde_json::Value::Object(obj)
        }
        toml::Value::Datetime(d) => serde_json::Value::String(d.to_string()),
    }
}

/// Convert `serde_json::Value` → `toml::Value` (best-effort)
fn json_to_toml(v: serde_json::Value) -> Option<toml::Value> {
    match v {
        serde_json::Value::Null        => None,
        serde_json::Value::Bool(b)     => Some(toml::Value::Boolean(b)),
        serde_json::Value::Number(n)   => {
            if let Some(i) = n.as_i64()  { return Some(toml::Value::Integer(i)); }
            if let Some(f) = n.as_f64()  { return Some(toml::Value::Float(f)); }
            None
        }
        serde_json::Value::String(s)   => Some(toml::Value::String(s)),
        serde_json::Value::Array(arr)  => {
            let items: Vec<toml::Value> = arr.into_iter().filter_map(json_to_toml).collect();
            Some(toml::Value::Array(items))
        }
        serde_json::Value::Object(obj) => {
            let mut tbl = toml::map::Map::new();
            for (k, v) in obj {
                if let Some(tv) = json_to_toml(v) { tbl.insert(k, tv); }
            }
            Some(toml::Value::Table(tbl))
        }
    }
}
