/// SQLite support (native rusqlite) + optional Python for Postgres/MySQL
use crate::error::{CapError, Span};
use crate::interpreter::value::{MapKey, Value};
use indexmap::IndexMap;
use rusqlite::{params_from_iter, Connection};
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &[
    "sql_open", "sql_close", "sql_exec", "sql_query", "sql_query_one",
    "sql_begin", "sql_commit", "sql_rollback",
    "sql_tables", "sql_schema",
];

thread_local! {
    static CONN: RefCell<Option<Connection>> = RefCell::new(None);
}

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "sql_open" => {
            // sql_open(path)  — open SQLite database (":memory:" for in-memory)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let conn = Connection::open(&path)
                .map_err(|e| CapError::Runtime { message: format!("sql_open: {e}"), span: span.clone() })?;
            CONN.with(|c| *c.borrow_mut() = Some(conn));
            Ok(Value::Null)
        }
        "sql_close" => {
            CONN.with(|c| *c.borrow_mut() = None);
            Ok(Value::Null)
        }
        "sql_exec" => {
            // sql_exec(sql)  or  sql_exec(sql, [param, ...])
            // Returns {rows_affected: int, last_insert_rowid: int}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let sql = args.remove(0).as_str(span)?.to_string();
            let params = if args.is_empty() { vec![] } else {
                match args.remove(0) {
                    Value::List(l) => l.borrow().clone(),
                    other => vec![other],
                }
            };
            let sql_params: Vec<rusqlite::types::Value> = params.iter()
                .map(|v| cap_to_sql(v))
                .collect();
            CONN.with(|c| {
                let mut b = c.borrow_mut();
                let conn = b.as_mut()
                    .ok_or_else(|| CapError::Runtime { message: "sql_exec: no open connection (call sql_open first)".into(), span: span.clone() })?;
                conn.execute(&sql, params_from_iter(sql_params.iter()))
                    .map_err(|e| CapError::Runtime { message: format!("sql_exec: {e}"), span: span.clone() })?;
                let rows_affected = conn.changes() as i64;
                let last_id = conn.last_insert_rowid();
                let mut map = IndexMap::new();
                map.insert(MapKey::Str("rows_affected".into()), Value::Int(rows_affected));
                map.insert(MapKey::Str("last_insert_rowid".into()), Value::Int(last_id));
                Ok(Value::Map(Rc::new(RefCell::new(map))))
            })
        }
        "sql_query" => {
            // sql_query(sql)  or  sql_query(sql, [params])
            // Returns list of maps
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let sql = args.remove(0).as_str(span)?.to_string();
            let params = if args.is_empty() { vec![] } else {
                match args.remove(0) {
                    Value::List(l) => l.borrow().clone(),
                    other => vec![other],
                }
            };
            let sql_params: Vec<rusqlite::types::Value> = params.iter()
                .map(|v| cap_to_sql(v))
                .collect();
            CONN.with(|c| {
                let mut b = c.borrow_mut();
                let conn = b.as_mut()
                    .ok_or_else(|| CapError::Runtime { message: "sql_query: no open connection".into(), span: span.clone() })?;
                let mut stmt = conn.prepare(&sql)
                    .map_err(|e| CapError::Runtime { message: format!("sql_query: {e}"), span: span.clone() })?;
                let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
                let rows = stmt.query_map(params_from_iter(sql_params.iter()), |row| {
                    let mut map = IndexMap::new();
                    for (i, col) in col_names.iter().enumerate() {
                        let val: rusqlite::types::Value = row.get(i)?;
                        map.insert(MapKey::Str(col.clone()), sql_to_cap(val));
                    }
                    Ok(Value::Map(Rc::new(RefCell::new(map))))
                }).map_err(|e| CapError::Runtime { message: format!("sql_query: {e}"), span: span.clone() })?;
                let result: Result<Vec<Value>, _> = rows
                    .map(|r| r.map_err(|e| CapError::Runtime { message: format!("sql_query row: {e}"), span: span.clone() }))
                    .collect();
                Ok(Value::List(Rc::new(RefCell::new(result?))))
            })
        }
        "sql_query_one" => {
            // sql_query_one(sql) or sql_query_one(sql, params) → first row map or null
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let sql = args.remove(0).as_str(span)?.to_string();
            let params = if args.is_empty() { vec![] } else {
                match args.remove(0) {
                    Value::List(l) => l.borrow().clone(),
                    other => vec![other],
                }
            };
            let sql_params: Vec<rusqlite::types::Value> = params.iter().map(|v| cap_to_sql(v)).collect();
            CONN.with(|c| {
                let mut b = c.borrow_mut();
                let conn = b.as_mut()
                    .ok_or_else(|| CapError::Runtime { message: "sql_query_one: no open connection".into(), span: span.clone() })?;
                let mut stmt = conn.prepare(&sql)
                    .map_err(|e| CapError::Runtime { message: format!("sql_query_one: {e}"), span: span.clone() })?;
                let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
                let mut rows = stmt.query(params_from_iter(sql_params.iter()))
                    .map_err(|e| CapError::Runtime { message: format!("sql_query_one: {e}"), span: span.clone() })?;
                match rows.next().map_err(|e| CapError::Runtime { message: format!("sql_query_one: {e}"), span: span.clone() })? {
                    None => Ok(Value::Null),
                    Some(row) => {
                        let mut map = IndexMap::new();
                        for (i, col) in col_names.iter().enumerate() {
                            let val: rusqlite::types::Value = row.get(i)
                                .map_err(|e| CapError::Runtime { message: format!("sql_query_one: {e}"), span: span.clone() })?;
                            map.insert(MapKey::Str(col.clone()), sql_to_cap(val));
                        }
                        Ok(Value::Map(Rc::new(RefCell::new(map))))
                    }
                }
            })
        }
        "sql_begin" => {
            CONN.with(|c| {
                let mut b = c.borrow_mut();
                let conn = b.as_mut()
                    .ok_or_else(|| CapError::Runtime { message: "sql_begin: no open connection".into(), span: span.clone() })?;
                conn.execute_batch("BEGIN")
                    .map_err(|e| CapError::Runtime { message: format!("sql_begin: {e}"), span: span.clone() })?;
                Ok(Value::Null)
            })
        }
        "sql_commit" => {
            CONN.with(|c| {
                let mut b = c.borrow_mut();
                let conn = b.as_mut()
                    .ok_or_else(|| CapError::Runtime { message: "sql_commit: no open connection".into(), span: span.clone() })?;
                conn.execute_batch("COMMIT")
                    .map_err(|e| CapError::Runtime { message: format!("sql_commit: {e}"), span: span.clone() })?;
                Ok(Value::Null)
            })
        }
        "sql_rollback" => {
            CONN.with(|c| {
                let mut b = c.borrow_mut();
                let conn = b.as_mut()
                    .ok_or_else(|| CapError::Runtime { message: "sql_rollback: no open connection".into(), span: span.clone() })?;
                conn.execute_batch("ROLLBACK")
                    .map_err(|e| CapError::Runtime { message: format!("sql_rollback: {e}"), span: span.clone() })?;
                Ok(Value::Null)
            })
        }
        "sql_tables" => {
            // Returns list of table names
            CONN.with(|c| {
                let mut b = c.borrow_mut();
                let conn = b.as_mut()
                    .ok_or_else(|| CapError::Runtime { message: "sql_tables: no open connection".into(), span: span.clone() })?;
                let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                    .map_err(|e| CapError::Runtime { message: format!("sql_tables: {e}"), span: span.clone() })?;
                let names: Result<Vec<Value>, _> = stmt.query_map([], |row| {
                    let name: String = row.get(0)?;
                    Ok(Value::Str(name))
                }).map_err(|e| CapError::Runtime { message: format!("sql_tables: {e}"), span: span.clone() })?
                .map(|r| r.map_err(|e| CapError::Runtime { message: format!("sql_tables row: {e}"), span: span.clone() }))
                .collect();
                Ok(Value::List(Rc::new(RefCell::new(names?))))
            })
        }
        "sql_schema" => {
            // sql_schema(table_name) → list of {name, type, notnull, dflt_value, pk}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let table = args.remove(0).as_str(span)?.to_string();
            CONN.with(|c| {
                let mut b = c.borrow_mut();
                let conn = b.as_mut()
                    .ok_or_else(|| CapError::Runtime { message: "sql_schema: no open connection".into(), span: span.clone() })?;
                let sql = format!("PRAGMA table_info({})", table);
                let mut stmt = conn.prepare(&sql)
                    .map_err(|e| CapError::Runtime { message: format!("sql_schema: {e}"), span: span.clone() })?;
                let rows: Result<Vec<Value>, _> = stmt.query_map([], |row| {
                    let mut map = IndexMap::new();
                    let col_name: String = row.get(1)?;
                    let col_type: String = row.get(2)?;
                    let notnull: i64 = row.get(3)?;
                    let pk: i64 = row.get(5)?;
                    map.insert(MapKey::Str("name".into()), Value::Str(col_name));
                    map.insert(MapKey::Str("type".into()), Value::Str(col_type));
                    map.insert(MapKey::Str("notnull".into()), Value::Bool(notnull != 0));
                    map.insert(MapKey::Str("pk".into()), Value::Bool(pk != 0));
                    Ok(Value::Map(Rc::new(RefCell::new(map))))
                }).map_err(|e| CapError::Runtime { message: format!("sql_schema: {e}"), span: span.clone() })?
                .map(|r| r.map_err(|e| CapError::Runtime { message: format!("sql_schema row: {e}"), span: span.clone() }))
                .collect();
                Ok(Value::List(Rc::new(RefCell::new(rows?))))
            })
        }
        _ => Err(CapError::Runtime { message: format!("unknown sql builtin: {name}"), span: span.clone() }),
    }
}

fn cap_to_sql(v: &Value) -> rusqlite::types::Value {
    match v {
        Value::Null        => rusqlite::types::Value::Null,
        Value::Bool(b)     => rusqlite::types::Value::Integer(if *b { 1 } else { 0 }),
        Value::Int(n)      => rusqlite::types::Value::Integer(*n),
        Value::Float(f)    => rusqlite::types::Value::Real(*f),
        Value::Str(s)      => rusqlite::types::Value::Text(s.clone()),
        _                  => rusqlite::types::Value::Text(v.display()),
    }
}

fn sql_to_cap(v: rusqlite::types::Value) -> Value {
    match v {
        rusqlite::types::Value::Null        => Value::Null,
        rusqlite::types::Value::Integer(n)  => Value::Int(n),
        rusqlite::types::Value::Real(f)     => Value::Float(f),
        rusqlite::types::Value::Text(s)     => Value::Str(s),
        rusqlite::types::Value::Blob(b)     => Value::Str(base64_encode(&b)),
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((n >> 18) & 63) as usize] as char);
        out.push(CHARS[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { CHARS[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { CHARS[(n & 63) as usize] as char } else { '=' });
    }
    out
}
