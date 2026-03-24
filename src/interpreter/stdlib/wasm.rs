/// WebAssembly integration via Python wasmtime-py subprocess
use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use crate::interpreter::stdlib::sys::run_python;
use crate::interpreter::stdlib::json::{json_to_value, value_to_json};

pub const BUILTINS: &[&str] = &[
    "wasm_load", "wasm_call", "wasm_exports", "wasm_memory_read",
];

fn run_wasm(code: &str, span: &Span) -> Result<Value, CapError> {
    let wrapped = format!(
        "import json as _json, sys as _sys\n\
         def cap_return(__v): print(_json.dumps(__v)); _sys.exit(0)\n\
         {code}"
    );
    let out = run_python(&wrapped, None, span)?;
    if let Value::Str(s) = &out {
        if s.is_empty() { return Ok(Value::Null); }
        let j: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| CapError::Runtime { message: format!("wasm: invalid JSON: {e}"), span: span.clone() })?;
        json_to_value(j, span)
    } else { Ok(Value::Null) }
}

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "wasm_load" => {
            // wasm_load(path) → {ok: bool, exports: [str]}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
try:
    from wasmtime import Engine, Module, Store, Linker
    engine = Engine()
    store = Store(engine)
    module = Module(engine, open("{path}", "rb").read())
    exports = [e.name for e in module.exports]
    cap_return({{"ok": True, "exports": exports, "path": "{path}"}})
except Exception as e:
    cap_return({{"ok": False, "error": str(e)}})
"#);
            run_wasm(&code, span)
        }
        "wasm_exports" => {
            // wasm_exports(path) → list of export names
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
from wasmtime import Engine, Module, Store
engine = Engine()
store = Store(engine)
module = Module(engine, open("{path}", "rb").read())
cap_return([e.name for e in module.exports])
"#);
            run_wasm(&code, span)
        }
        "wasm_call" => {
            // wasm_call(path, fn_name, [args...])
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let fn_name = args.remove(0).as_str(span)?.to_string();
            let fn_args_json = if args.is_empty() {
                "[]".to_string()
            } else {
                value_to_json(&args[0], span)?.to_string()
            };
            let code = format!(r#"
import json
from wasmtime import Engine, Module, Store, Linker
engine = Engine()
store = Store(engine)
module = Module(engine, open("{path}", "rb").read())
linker = Linker(engine)
instance = linker.instantiate(store, module)
fn = instance.exports(store)["{fn_name}"]
fn_args = json.loads('''{fn_args_json}''')
result = fn(store, *fn_args)
cap_return(result)
"#);
            run_wasm(&code, span)
        }
        "wasm_memory_read" => {
            // wasm_memory_read(path, offset, length)
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let offset = match args.remove(0) {
                Value::Int(n) => n,
                other => return Err(CapError::TypeError { expected: "int", got: other.type_name().to_string(), span: span.clone() }),
            };
            let length = match args.remove(0) {
                Value::Int(n) => n,
                other => return Err(CapError::TypeError { expected: "int", got: other.type_name().to_string(), span: span.clone() }),
            };
            let code = format!(r#"
from wasmtime import Engine, Module, Store, Linker
engine = Engine()
store = Store(engine)
module = Module(engine, open("{path}", "rb").read())
linker = Linker(engine)
instance = linker.instantiate(store, module)
memory = instance.exports(store)["memory"]
data = bytes(memory.data_ptr(store)[{offset}:{offset}+{length}])
cap_return(list(data))
"#);
            run_wasm(&code, span)
        }
        _ => Err(CapError::Runtime { message: format!("unknown wasm builtin: {name}"), span: span.clone() }),
    }
}
