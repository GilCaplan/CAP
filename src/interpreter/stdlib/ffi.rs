/// Native C/C++ FFI via Python ctypes subprocess
use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use crate::interpreter::stdlib::sys::run_python;
use crate::interpreter::stdlib::json::{json_to_value, value_to_json};

pub const BUILTINS: &[&str] = &[
    "ffi_call", "ffi_load", "ffi_sizeof",
    "ffi_struct", "ffi_array",
];

fn run_ffi(code: &str, span: &Span) -> Result<Value, CapError> {
    let wrapped = format!(
        "import json as _json, sys as _sys\n\
         def cap_return(__v): print(_json.dumps(__v)); _sys.exit(0)\n\
         {code}"
    );
    let out = run_python(&wrapped, None, span)?;
    if let Value::Str(s) = &out {
        if s.is_empty() { return Ok(Value::Null); }
        let j: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| CapError::Runtime { message: format!("ffi: invalid JSON: {e}"), span: span.clone() })?;
        json_to_value(j, span)
    } else { Ok(Value::Null) }
}

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "ffi_load" => {
            // ffi_load(lib_path) → checks if library can be opened via ctypes
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import ctypes
try:
    lib = ctypes.CDLL("{path}")
    cap_return({{"ok": True, "path": "{path}"}})
except Exception as e:
    cap_return({{"ok": False, "error": str(e)}})
"#);
            run_ffi(&code, span)
        }
        "ffi_call" => {
            // ffi_call(lib_path, func_name, ret_type, [arg_types], [arg_values])
            // ret_type / arg_types: "int", "float", "double", "char_p", "void"
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let lib_path = args.remove(0).as_str(span)?.to_string();
            let func_name = args.remove(0).as_str(span)?.to_string();
            let ret_type = args.remove(0).as_str(span)?.to_string();
            let arg_types_json = if !args.is_empty() {
                value_to_json(&args.remove(0), span)?.to_string()
            } else { "[]".to_string() };
            let arg_vals_json = if !args.is_empty() {
                value_to_json(&args.remove(0), span)?.to_string()
            } else { "[]".to_string() };
            let code = format!(r#"
import ctypes, json
lib = ctypes.CDLL("{lib_path}")
fn = getattr(lib, "{func_name}")
type_map = {{"int": ctypes.c_int, "long": ctypes.c_long, "float": ctypes.c_float, "double": ctypes.c_double, "char_p": ctypes.c_char_p, "void": None, "bool": ctypes.c_bool}}
arg_types = json.loads('''{arg_types_json}''')
arg_vals  = json.loads('''{arg_vals_json}''')
fn.restype  = type_map.get("{ret_type}")
fn.argtypes = [type_map[t] for t in arg_types]
result = fn(*arg_vals)
if isinstance(result, bytes): result = result.decode()
cap_return(result)
"#);
            run_ffi(&code, span)
        }
        "ffi_sizeof" => {
            // ffi_sizeof(type_name) → byte size
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let type_name = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import ctypes
type_map = {{"int": ctypes.c_int, "long": ctypes.c_long, "float": ctypes.c_float, "double": ctypes.c_double, "char_p": ctypes.c_char_p, "bool": ctypes.c_bool, "short": ctypes.c_short, "byte": ctypes.c_byte}}
t = type_map.get("{type_name}")
if t:
    cap_return(ctypes.sizeof(t))
else:
    cap_return(None)
"#);
            run_ffi(&code, span)
        }
        "ffi_struct" => {
            // ffi_struct(fields_map) → struct info
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let fields_json = value_to_json(&args[0], span)?.to_string();
            let code = format!(r#"
import ctypes, json
fields = json.loads('''{fields_json}''')
type_map = {{"int": ctypes.c_int, "long": ctypes.c_long, "float": ctypes.c_float, "double": ctypes.c_double, "char_p": ctypes.c_char_p, "bool": ctypes.c_bool, "short": ctypes.c_short, "byte": ctypes.c_byte}}
class DynStruct(ctypes.Structure):
    _fields_ = [(k, type_map[v]) for k, v in fields.items()]
size = ctypes.sizeof(DynStruct)
field_sizes = {{k: ctypes.sizeof(type_map[v]) for k, v in fields.items()}}
cap_return({{"size": size, "fields": field_sizes}})
"#);
            run_ffi(&code, span)
        }
        "ffi_array" => {
            // ffi_array(type_name, list_of_values) → base64 encoded bytes
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let type_name = args.remove(0).as_str(span)?.to_string();
            let vals_json = value_to_json(&args[0], span)?.to_string();
            let code = format!(r#"
import ctypes, json, base64
vals = json.loads('''{vals_json}''')
type_map = {{"int": ctypes.c_int, "long": ctypes.c_long, "float": ctypes.c_float, "double": ctypes.c_double, "bool": ctypes.c_bool, "short": ctypes.c_short, "byte": ctypes.c_byte}}
t = type_map["{type_name}"]
arr = (t * len(vals))(*vals)
cap_return(base64.b64encode(bytes(arr)).decode())
"#);
            run_ffi(&code, span)
        }
        _ => Err(CapError::Runtime { message: format!("unknown ffi builtin: {name}"), span: span.clone() }),
    }
}
