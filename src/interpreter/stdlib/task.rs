/// Task / concurrency utilities
/// Since cap is single-threaded (Rc<RefCell<>>), "parallel" operations
/// are simulated via sequential execution with retry/timeout logic.
/// task_par_map runs multiple shell subprocesses and collects results.
use crate::error::{CapError, Span};
use crate::interpreter::value::{MapKey, Value};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &[
    "task_sleep", "task_retry", "task_timeout",
    "task_par_shell", "task_debounce",
    "task_measure",
];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "task_sleep" => {
            // task_sleep(seconds)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let secs = match args.remove(0) {
                Value::Int(n) => n as f64,
                Value::Float(f) => f,
                other => return Err(CapError::TypeError { expected: "number", got: other.type_name().to_string(), span: span.clone() }),
            };
            std::thread::sleep(std::time::Duration::from_secs_f64(secs));
            Ok(Value::Null)
        }
        "task_retry" => {
            // task_retry(n, fn) → runs fn() up to n times until success
            // Returns {ok: bool, value: ..., attempts: int, error: str|null}
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let n = match args.remove(0) {
                Value::Int(n) => n as usize,
                other => return Err(CapError::TypeError { expected: "int", got: other.type_name().to_string(), span: span.clone() }),
            };
            // We can't call cap functions here (no interpreter reference),
            // so task_retry is a shell-command retrier.
            // For function-level retry, use the try() builtin in a loop.
            // Instead, we accept a shell command string.
            let cmd = args.remove(0).as_str(span)?.to_string();
            let mut last_err = String::new();
            for attempt in 1..=n {
                let output = std::process::Command::new("sh")
                    .arg("-c").arg(&cmd)
                    .output()
                    .map_err(|e| CapError::Runtime { message: format!("task_retry: {e}"), span: span.clone() })?;
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                    let mut map = IndexMap::new();
                    map.insert(MapKey::Str("ok".into()), Value::Bool(true));
                    map.insert(MapKey::Str("value".into()), Value::Str(stdout.trim_end().to_string()));
                    map.insert(MapKey::Str("attempts".into()), Value::Int(attempt as i64));
                    map.insert(MapKey::Str("error".into()), Value::Null);
                    return Ok(Value::Map(Rc::new(RefCell::new(map))));
                }
                last_err = String::from_utf8_lossy(&output.stderr).into_owned();
            }
            let mut map = IndexMap::new();
            map.insert(MapKey::Str("ok".into()), Value::Bool(false));
            map.insert(MapKey::Str("value".into()), Value::Null);
            map.insert(MapKey::Str("attempts".into()), Value::Int(n as i64));
            map.insert(MapKey::Str("error".into()), Value::Str(last_err.trim_end().to_string()));
            Ok(Value::Map(Rc::new(RefCell::new(map))))
        }
        "task_timeout" => {
            // task_timeout(seconds, shell_cmd) → {ok: bool, value: str, timed_out: bool}
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let secs = match args.remove(0) {
                Value::Int(n) => n as f64,
                Value::Float(f) => f,
                other => return Err(CapError::TypeError { expected: "number", got: other.type_name().to_string(), span: span.clone() }),
            };
            let cmd = args.remove(0).as_str(span)?.to_string();
            let timeout_cmd = format!("timeout {} sh -c {}", secs, shell_escape(&cmd));
            let output = std::process::Command::new("sh")
                .arg("-c").arg(&timeout_cmd)
                .output()
                .map_err(|e| CapError::Runtime { message: format!("task_timeout: {e}"), span: span.clone() })?;
            let timed_out = output.status.code() == Some(124);
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let mut map = IndexMap::new();
            map.insert(MapKey::Str("ok".into()), Value::Bool(output.status.success()));
            map.insert(MapKey::Str("value".into()), Value::Str(stdout.trim_end().to_string()));
            map.insert(MapKey::Str("timed_out".into()), Value::Bool(timed_out));
            Ok(Value::Map(Rc::new(RefCell::new(map))))
        }
        "task_par_shell" => {
            // task_par_shell([cmd1, cmd2, ...]) → [result1, result2, ...]
            // Runs shell commands in parallel using threads, returns list of
            // {status, stdout, stderr} maps.
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let cmds = args.remove(0).as_list(span)?;
            let cmds_vec: Vec<String> = cmds.borrow().iter()
                .map(|v| v.as_str(span).map(|s| s.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            // Spawn all processes, then collect
            let mut children = Vec::new();
            for cmd in &cmds_vec {
                let child = std::process::Command::new("sh")
                    .arg("-c").arg(cmd)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|e| CapError::Runtime { message: format!("task_par_shell: {e}"), span: span.clone() })?;
                children.push(child);
            }
            let results: Vec<Value> = children.into_iter()
                .map(|c| {
                    c.wait_with_output()
                        .map(|o| {
                            let mut map = IndexMap::new();
                            map.insert(MapKey::Str("status".into()), Value::Int(o.status.code().unwrap_or(-1) as i64));
                            map.insert(MapKey::Str("stdout".into()), Value::Str(String::from_utf8_lossy(&o.stdout).trim_end().to_string()));
                            map.insert(MapKey::Str("stderr".into()), Value::Str(String::from_utf8_lossy(&o.stderr).trim_end().to_string()));
                            Value::Map(Rc::new(RefCell::new(map)))
                        })
                        .unwrap_or(Value::Null)
                })
                .collect();
            Ok(Value::List(Rc::new(RefCell::new(results))))
        }
        "task_debounce" => {
            // task_debounce(delay_seconds, cmd) — runs cmd after delay
            // (simple sequential version: just sleep then run)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let secs = match args.remove(0) {
                Value::Int(n) => n as f64,
                Value::Float(f) => f,
                other => return Err(CapError::TypeError { expected: "number", got: other.type_name().to_string(), span: span.clone() }),
            };
            std::thread::sleep(std::time::Duration::from_secs_f64(secs));
            let cmd = args.remove(0).as_str(span)?.to_string();
            let output = std::process::Command::new("sh")
                .arg("-c").arg(&cmd)
                .output()
                .map_err(|e| CapError::Runtime { message: format!("task_debounce: {e}"), span: span.clone() })?;
            let stdout = String::from_utf8_lossy(&output.stdout).trim_end().to_string();
            Ok(Value::Str(stdout))
        }
        "task_measure" => {
            // task_measure(shell_cmd) → {value: str, elapsed_ms: int}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let cmd = args.remove(0).as_str(span)?.to_string();
            let start = std::time::Instant::now();
            let output = std::process::Command::new("sh")
                .arg("-c").arg(&cmd)
                .output()
                .map_err(|e| CapError::Runtime { message: format!("task_measure: {e}"), span: span.clone() })?;
            let elapsed_ms = start.elapsed().as_millis() as i64;
            let stdout = String::from_utf8_lossy(&output.stdout).trim_end().to_string();
            let mut map = IndexMap::new();
            map.insert(MapKey::Str("value".into()), Value::Str(stdout));
            map.insert(MapKey::Str("elapsed_ms".into()), Value::Int(elapsed_ms));
            Ok(Value::Map(Rc::new(RefCell::new(map))))
        }
        _ => Err(CapError::Runtime { message: format!("unknown task builtin: {name}"), span: span.clone() }),
    }
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
