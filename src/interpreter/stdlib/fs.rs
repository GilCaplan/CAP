use crate::error::{CapError, Span};
use crate::interpreter::value::{MapKey, Value};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &[
    "fs_read", "fs_write", "fs_append", "fs_delete", "fs_exists",
    "fs_mkdir", "fs_mkdir_all", "fs_rmdir", "fs_ls", "fs_copy", "fs_move",
    "fs_stat", "fs_is_file", "fs_is_dir",
    "os_cwd", "os_chdir", "os_hostname", "os_username", "os_pid",
    "os_sep", "os_path_join", "os_path_basename", "os_path_dirname", "os_path_ext",
    "os_abs", "os_home",
];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        // ── File I/O ──────────────────────────────────────────────────────────
        "fs_read" => {
            let path = args.remove(0).as_str(span)?.to_string();
            let content = std::fs::read_to_string(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Str(content))
        }
        "fs_write" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let content = args.remove(0).as_str(span)?.to_string();
            std::fs::write(&path, content)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Null)
        }
        "fs_append" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let content = args.remove(0).as_str(span)?.to_string();
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .append(true).create(true).open(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            file.write_all(content.as_bytes())
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Null)
        }
        "fs_delete" => {
            let path = args.remove(0).as_str(span)?.to_string();
            std::fs::remove_file(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Null)
        }
        "fs_exists" => {
            let path = args.remove(0).as_str(span)?.to_string();
            Ok(Value::Bool(std::path::Path::new(&path).exists()))
        }
        "fs_is_file" => {
            let path = args.remove(0).as_str(span)?.to_string();
            Ok(Value::Bool(std::path::Path::new(&path).is_file()))
        }
        "fs_is_dir" => {
            let path = args.remove(0).as_str(span)?.to_string();
            Ok(Value::Bool(std::path::Path::new(&path).is_dir()))
        }
        "fs_mkdir" => {
            let path = args.remove(0).as_str(span)?.to_string();
            std::fs::create_dir(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Null)
        }
        "fs_mkdir_all" => {
            let path = args.remove(0).as_str(span)?.to_string();
            std::fs::create_dir_all(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Null)
        }
        "fs_rmdir" => {
            let path = args.remove(0).as_str(span)?.to_string();
            std::fs::remove_dir_all(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Null)
        }
        "fs_copy" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let src = args.remove(0).as_str(span)?.to_string();
            let dst = args.remove(0).as_str(span)?.to_string();
            std::fs::copy(&src, &dst)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Null)
        }
        "fs_move" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let src = args.remove(0).as_str(span)?.to_string();
            let dst = args.remove(0).as_str(span)?.to_string();
            std::fs::rename(&src, &dst)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Null)
        }
        "fs_ls" => {
            let path = if args.is_empty() {
                ".".to_string()
            } else {
                args.remove(0).as_str(span)?.to_string()
            };
            let entries: Result<Vec<Value>, _> = std::fs::read_dir(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?
                .map(|e| e.map(|e| Value::Str(e.file_name().to_string_lossy().into_owned())))
                .collect();
            let mut names = entries.map_err(|e: std::io::Error| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            names.sort_by(|a, b| {
                if let (Value::Str(sa), Value::Str(sb)) = (a, b) { sa.cmp(sb) } else { std::cmp::Ordering::Equal }
            });
            Ok(Value::List(Rc::new(RefCell::new(names))))
        }
        "fs_stat" => {
            let path = args.remove(0).as_str(span)?.to_string();
            let meta = std::fs::metadata(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            let mut map = IndexMap::new();
            map.insert(MapKey::Str("size".into()), Value::Int(meta.len() as i64));
            map.insert(MapKey::Str("is_file".into()), Value::Bool(meta.is_file()));
            map.insert(MapKey::Str("is_dir".into()), Value::Bool(meta.is_dir()));
            map.insert(MapKey::Str("readonly".into()), Value::Bool(meta.permissions().readonly()));
            if let Ok(modified) = meta.modified() {
                if let Ok(d) = modified.duration_since(std::time::UNIX_EPOCH) {
                    map.insert(MapKey::Str("modified".into()), Value::Int(d.as_secs() as i64));
                }
            }
            Ok(Value::Map(Rc::new(RefCell::new(map))))
        }

        // ── OS / path ─────────────────────────────────────────────────────────
        "os_cwd" => {
            let cwd = std::env::current_dir()
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Str(cwd.to_string_lossy().into_owned()))
        }
        "os_chdir" => {
            let path = args.remove(0).as_str(span)?.to_string();
            std::env::set_current_dir(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Null)
        }
        "os_hostname" => {
            let out = std::process::Command::new("hostname").output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            Ok(Value::Str(out))
        }
        "os_username" => {
            Ok(std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .map(Value::Str)
                .unwrap_or(Value::Null))
        }
        "os_pid" => {
            Ok(Value::Int(std::process::id() as i64))
        }
        "os_sep" => {
            Ok(Value::Str(std::path::MAIN_SEPARATOR.to_string()))
        }
        "os_path_join" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let mut path = std::path::PathBuf::from(args.remove(0).as_str(span)?);
            for a in args {
                path.push(a.as_str(span)?);
            }
            Ok(Value::Str(path.to_string_lossy().into_owned()))
        }
        "os_path_basename" => {
            let path = args.remove(0).as_str(span)?.to_string();
            let p = std::path::Path::new(&path);
            Ok(p.file_name().map(|n| Value::Str(n.to_string_lossy().into_owned())).unwrap_or(Value::Null))
        }
        "os_path_dirname" => {
            let path = args.remove(0).as_str(span)?.to_string();
            let p = std::path::Path::new(&path);
            Ok(p.parent().map(|n| Value::Str(n.to_string_lossy().into_owned())).unwrap_or(Value::Null))
        }
        "os_path_ext" => {
            let path = args.remove(0).as_str(span)?.to_string();
            let p = std::path::Path::new(&path);
            Ok(p.extension().map(|e| Value::Str(e.to_string_lossy().into_owned())).unwrap_or(Value::Null))
        }
        "os_abs" => {
            let path = args.remove(0).as_str(span)?.to_string();
            let abs = std::fs::canonicalize(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Str(abs.to_string_lossy().into_owned()))
        }
        "os_home" => {
            Ok(std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .map(Value::Str)
                .unwrap_or(Value::Null))
        }
        _ => Err(CapError::Runtime { message: format!("unknown fs builtin: {name}"), span: span.clone() }),
    }
}
