use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &[
    "shell", "shell_lines",
    "env", "env_all",
    "regex_match", "regex_find", "regex_find_all", "regex_replace",
    "python", "pyval",
];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    match name {
        // ── Shell ─────────────────────────────────────────────────────────────
        "shell" => {
            // shell(cmd) → {status: int, stdout: str, stderr: str}
            let cmd = args.into_iter().next().unwrap_or(Value::Null);
            let cmd_str = cmd.as_str(span)?.to_string();
            run_shell(&cmd_str, span)
        }
        "shell_lines" => {
            // shell_lines(cmd) → list of non-empty stdout lines
            let cmd = args.into_iter().next().unwrap_or(Value::Null);
            let cmd_str = cmd.as_str(span)?.to_string();
            let result = run_shell(&cmd_str, span)?;
            if let Value::Map(m) = &result {
                let stdout = m.borrow().get(&crate::interpreter::value::MapKey::Str("stdout".into()))
                    .cloned().unwrap_or(Value::Null);
                if let Value::Str(s) = stdout {
                    let lines: Vec<Value> = s.lines()
                        .filter(|l| !l.is_empty())
                        .map(|l| Value::Str(l.to_string()))
                        .collect();
                    return Ok(Value::List(Rc::new(RefCell::new(lines))));
                }
            }
            Ok(Value::List(Rc::new(RefCell::new(vec![]))))
        }

        // ── Environment variables ─────────────────────────────────────────────
        "env" => {
            // env(name) → str or null
            let name_val = args.into_iter().next().unwrap_or(Value::Null);
            let var_name = name_val.as_str(span)?.to_string();
            Ok(std::env::var(&var_name).map(Value::Str).unwrap_or(Value::Null))
        }
        "env_all" => {
            // env_all() → map of all env vars
            use crate::interpreter::value::MapKey;
            use indexmap::IndexMap;
            let mut map = IndexMap::new();
            for (k, v) in std::env::vars() {
                map.insert(MapKey::Str(k), Value::Str(v));
            }
            Ok(Value::Map(Rc::new(RefCell::new(map))))
        }

        // ── Regex ─────────────────────────────────────────────────────────────
        "regex_match" => {
            // regex_match(pattern, text) → bool
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let mut args = args;
            let pattern = args.remove(0).as_str(span)?.to_string();
            let text = args.remove(0).as_str(span)?.to_string();
            let re = regex::Regex::new(&pattern)
                .map_err(|e| CapError::Runtime { message: format!("regex error: {e}"), span: span.clone() })?;
            Ok(Value::Bool(re.is_match(&text)))
        }
        "regex_find" => {
            // regex_find(pattern, text) → str or null (first match)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let mut args = args;
            let pattern = args.remove(0).as_str(span)?.to_string();
            let text = args.remove(0).as_str(span)?.to_string();
            let re = regex::Regex::new(&pattern)
                .map_err(|e| CapError::Runtime { message: format!("regex error: {e}"), span: span.clone() })?;
            Ok(re.find(&text).map(|m| Value::Str(m.as_str().to_string())).unwrap_or(Value::Null))
        }
        "regex_find_all" => {
            // regex_find_all(pattern, text) → list of matching strings
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let mut args = args;
            let pattern = args.remove(0).as_str(span)?.to_string();
            let text = args.remove(0).as_str(span)?.to_string();
            let re = regex::Regex::new(&pattern)
                .map_err(|e| CapError::Runtime { message: format!("regex error: {e}"), span: span.clone() })?;
            let matches: Vec<Value> = re.find_iter(&text)
                .map(|m| Value::Str(m.as_str().to_string()))
                .collect();
            Ok(Value::List(Rc::new(RefCell::new(matches))))
        }
        "regex_replace" => {
            // regex_replace(pattern, replacement, text) → str
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let mut args = args;
            let pattern = args.remove(0).as_str(span)?.to_string();
            let replacement = args.remove(0).as_str(span)?.to_string();
            let text = args.remove(0).as_str(span)?.to_string();
            let re = regex::Regex::new(&pattern)
                .map_err(|e| CapError::Runtime { message: format!("regex error: {e}"), span: span.clone() })?;
            Ok(Value::Str(re.replace_all(&text, replacement.as_str()).into_owned()))
        }

        // ── Python bridge ─────────────────────────────────────────────────────
        "python" => {
            // python(code_str) → stdout str
            // python(code_str, input_str) → stdout str (with stdin)
            let code = args.first().cloned().unwrap_or(Value::Null).as_str(span)?.to_string();
            let stdin_input = args.get(1).cloned();
            run_python(&code, stdin_input, span)
        }
        "pyval" => {
            // pyval(code_str) → cap Value parsed from JSON
            // The code must call cap_return(value) to return a structured result.
            // cap_return is injected automatically.
            let code = args.into_iter().next().unwrap_or(Value::Null).as_str(span)?.to_string();
            let wrapped = format!(
                "import json as _json, sys as _sys\n\
                 def cap_return(__v): print(_json.dumps(__v)); _sys.exit(0)\n\
                 {code}"
            );
            let out = run_python(&wrapped, None, span)?;
            if let Value::Str(s) = &out {
                if s.is_empty() { return Ok(Value::Null); }
                let j: serde_json::Value = serde_json::from_str(s)
                    .map_err(|e| CapError::Runtime {
                        message: format!("pyval: invalid JSON from Python: {e}\nOutput: {s}"),
                        span: span.clone(),
                    })?;
                crate::interpreter::stdlib::json::json_to_value(j, span)
            } else {
                Ok(Value::Null)
            }
        }

        _ => Err(CapError::Runtime { message: format!("unknown sys builtin: {name}"), span: span.clone() }),
    }
}

fn run_shell(cmd: &str, span: &Span) -> Result<Value, CapError> {
    use crate::interpreter::value::MapKey;
    use indexmap::IndexMap;

    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .map_err(|e| CapError::Runtime { message: format!("shell({cmd:?}): {e}"), span: span.clone() })?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let status = output.status.code().unwrap_or(-1) as i64;

    let mut map = IndexMap::new();
    map.insert(MapKey::Str("status".into()), Value::Int(status));
    map.insert(MapKey::Str("stdout".into()), Value::Str(stdout));
    map.insert(MapKey::Str("stderr".into()), Value::Str(stderr));
    Ok(Value::Map(Rc::new(RefCell::new(map))))
}

pub(crate) fn run_python(code: &str, stdin_input: Option<Value>, span: &Span) -> Result<Value, CapError> {
    use std::io::Write;

    let mut cmd = std::process::Command::new("python3");
    cmd.arg("-c").arg(code)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().or_else(|_| {
        // Fallback to `python` if `python3` not found
        std::process::Command::new("python")
            .arg("-c").arg(code)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    }).map_err(|e| CapError::Runtime { message: format!("python: {e}"), span: span.clone() })?;

    if let (Some(mut stdin), Some(input)) = (child.stdin.take(), stdin_input) {
        let input_str = input.display();
        stdin.write_all(input_str.as_bytes()).ok();
    }

    let output = child.wait_with_output()
        .map_err(|e| CapError::Runtime { message: format!("python wait: {e}"), span: span.clone() })?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let status = output.status.code().unwrap_or(-1) as i64;

    if status != 0 && !stderr.is_empty() {
        return Err(CapError::Runtime {
            message: format!("python: {}", stderr.trim()),
            span: span.clone(),
        });
    }

    Ok(Value::Str(stdout.trim_end_matches('\n').trim_end_matches('\r').to_string()))
}
