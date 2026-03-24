/// ZIP / TAR archive support via Python stdlib (zipfile, tarfile)
use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use crate::interpreter::stdlib::sys::run_python;
use crate::interpreter::stdlib::json::{json_to_value, value_to_json};

pub const BUILTINS: &[&str] = &[
    "zip_list", "zip_extract", "zip_extract_all",
    "zip_create", "zip_add", "zip_read_entry",
    "tar_list", "tar_extract", "tar_extract_all", "tar_create",
];

fn run_arch(code: &str, span: &Span) -> Result<Value, CapError> {
    let wrapped = format!(
        "import json as _json, sys as _sys\n\
         def cap_return(__v): print(_json.dumps(__v)); _sys.exit(0)\n\
         {code}"
    );
    let out = run_python(&wrapped, None, span)?;
    if let Value::Str(s) = &out {
        if s.is_empty() { return Ok(Value::Null); }
        let j: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| CapError::Runtime { message: format!("zip: invalid JSON: {e}"), span: span.clone() })?;
        json_to_value(j, span)
    } else { Ok(Value::Null) }
}

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "zip_list" => {
            // zip_list(path) → list of {name, size, compressed_size, is_dir}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import zipfile
with zipfile.ZipFile("{path}", "r") as z:
    entries = [{{
        "name": i.filename,
        "size": i.file_size,
        "compressed_size": i.compress_size,
        "is_dir": i.filename.endswith("/")
    }} for i in z.infolist()]
cap_return(entries)
"#);
            run_arch(&code, span)
        }
        "zip_extract" => {
            // zip_extract(path, entry_name, output_dir?)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let path  = args.remove(0).as_str(span)?.to_string();
            let entry = args.remove(0).as_str(span)?.to_string();
            let out_dir = if !args.is_empty() { args.remove(0).as_str(span)?.to_string() } else { ".".into() };
            let code = format!(r#"
import zipfile
with zipfile.ZipFile("{path}", "r") as z:
    extracted = z.extract("{entry}", path="{out_dir}")
cap_return(extracted)
"#);
            run_arch(&code, span)
        }
        "zip_extract_all" => {
            // zip_extract_all(path, output_dir?)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path    = args.remove(0).as_str(span)?.to_string();
            let out_dir = if !args.is_empty() { args.remove(0).as_str(span)?.to_string() } else { ".".into() };
            let code = format!(r#"
import zipfile
with zipfile.ZipFile("{path}", "r") as z:
    z.extractall(path="{out_dir}")
    names = z.namelist()
cap_return({{"ok": True, "extracted": len(names), "dir": "{out_dir}"}})
"#);
            run_arch(&code, span)
        }
        "zip_create" => {
            // zip_create(output_path, [file_paths...])
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let output   = args.remove(0).as_str(span)?.to_string();
            let files_json = value_to_json(&args[0], span)?.to_string();
            let code = format!(r#"
import zipfile, json, os
files = json.loads('''{files_json}''')
with zipfile.ZipFile("{output}", "w", zipfile.ZIP_DEFLATED) as z:
    for f in files:
        z.write(f, os.path.basename(f))
cap_return("{output}")
"#);
            run_arch(&code, span)
        }
        "zip_add" => {
            // zip_add(zip_path, file_path, arcname?)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let zip_path  = args.remove(0).as_str(span)?.to_string();
            let file_path = args.remove(0).as_str(span)?.to_string();
            let arcname   = if !args.is_empty() { args.remove(0).as_str(span)?.to_string() } else { String::new() };
            let arcname_code = if arcname.is_empty() {
                "import os; arcname = os.path.basename(file_path)".to_string()
            } else {
                format!("arcname = {:?}", arcname)
            };
            let code = format!(r#"
import zipfile
file_path = "{file_path}"
{arcname_code}
mode = "a" if __import__("os").path.exists("{zip_path}") else "w"
with zipfile.ZipFile("{zip_path}", mode, zipfile.ZIP_DEFLATED) as z:
    z.write(file_path, arcname)
cap_return("{zip_path}")
"#);
            run_arch(&code, span)
        }
        "zip_read_entry" => {
            // zip_read_entry(zip_path, entry_name) → str content
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let zip_path = args.remove(0).as_str(span)?.to_string();
            let entry    = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import zipfile
with zipfile.ZipFile("{zip_path}", "r") as z:
    content = z.read("{entry}").decode("utf-8", errors="replace")
cap_return(content)
"#);
            run_arch(&code, span)
        }
        // ── TAR archives ──────────────────────────────────────────────────────
        "tar_list" => {
            // tar_list(path) → list of {name, size, is_dir}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import tarfile
with tarfile.open("{path}") as t:
    entries = [{{"name": m.name, "size": m.size, "is_dir": m.isdir()}} for m in t.getmembers()]
cap_return(entries)
"#);
            run_arch(&code, span)
        }
        "tar_extract" => {
            // tar_extract(path, entry, output_dir?)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let path    = args.remove(0).as_str(span)?.to_string();
            let entry   = args.remove(0).as_str(span)?.to_string();
            let out_dir = if !args.is_empty() { args.remove(0).as_str(span)?.to_string() } else { ".".into() };
            let code = format!(r#"
import tarfile
with tarfile.open("{path}") as t:
    member = t.getmember("{entry}")
    t.extract(member, path="{out_dir}")
cap_return("{out_dir}/{entry}")
"#);
            run_arch(&code, span)
        }
        "tar_extract_all" => {
            // tar_extract_all(path, output_dir?)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path    = args.remove(0).as_str(span)?.to_string();
            let out_dir = if !args.is_empty() { args.remove(0).as_str(span)?.to_string() } else { ".".into() };
            let code = format!(r#"
import tarfile
with tarfile.open("{path}") as t:
    t.extractall(path="{out_dir}")
    count = len(t.getmembers())
cap_return({{"ok": True, "extracted": count, "dir": "{out_dir}"}})
"#);
            run_arch(&code, span)
        }
        "tar_create" => {
            // tar_create(output_path, [file_paths], compression?)
            // compression: "gz", "bz2", "xz", "" (none)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let output     = args.remove(0).as_str(span)?.to_string();
            let files_json = value_to_json(&args.remove(0), span)?.to_string();
            let comp       = if !args.is_empty() { args.remove(0).as_str(span)?.to_string() } else { "gz".into() };
            let mode       = if comp.is_empty() { "w".to_string() } else { format!("w:{comp}") };
            let code = format!(r#"
import tarfile, json, os
files = json.loads('''{files_json}''')
with tarfile.open("{output}", "{mode}") as t:
    for f in files:
        t.add(f, arcname=os.path.basename(f))
cap_return("{output}")
"#);
            run_arch(&code, span)
        }
        _ => Err(CapError::Runtime { message: format!("unknown zip builtin: {name}"), span: span.clone() }),
    }
}
