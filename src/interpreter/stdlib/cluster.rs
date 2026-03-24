/// Distributed computing via Python Ray or Dask (subprocess)
use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use crate::interpreter::stdlib::sys::run_python;
use crate::interpreter::stdlib::json::{json_to_value, value_to_json};

pub const BUILTINS: &[&str] = &[
    "cluster_map", "cluster_map_reduce",
    "cluster_dask_read", "cluster_dask_groupby",
    "cluster_info",
];

fn run_cluster(code: &str, span: &Span) -> Result<Value, CapError> {
    let wrapped = format!(
        "import json as _json, sys as _sys\n\
         def cap_return(__v): print(_json.dumps(__v)); _sys.exit(0)\n\
         {code}"
    );
    let out = run_python(&wrapped, None, span)?;
    if let Value::Str(s) = &out {
        if s.is_empty() { return Ok(Value::Null); }
        let j: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| CapError::Runtime { message: format!("cluster: invalid JSON: {e}"), span: span.clone() })?;
        json_to_value(j, span)
    } else { Ok(Value::Null) }
}

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "cluster_info" => {
            // cluster_info() → {ray: bool, dask: bool, cpus: int}
            let code = r#"
import os
ray_ok = False
dask_ok = False
try:
    import ray
    ray_ok = True
except ImportError:
    pass
try:
    import dask
    dask_ok = True
except ImportError:
    pass
import multiprocessing
cap_return({"ray": ray_ok, "dask": dask_ok, "cpus": multiprocessing.cpu_count()})
"#;
            run_cluster(code, span)
        }
        "cluster_map" => {
            // cluster_map(python_fn_code, list) → list of results
            // python_fn_code is a Python lambda/def as string, e.g. "lambda x: x * 2"
            // Uses multiprocessing.Pool as fallback if Ray not available
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let fn_code = args.remove(0).as_str(span)?.to_string();
            let data_json = value_to_json(&args[0], span)?.to_string();
            let code = format!(r#"
import json
data = json.loads('''{data_json}''')
fn = {fn_code}
try:
    import ray
    if not ray.is_initialized():
        ray.init(ignore_reinit_error=True, num_cpus=4, log_to_driver=False)
    remote_fn = ray.remote(fn)
    futures = [remote_fn.remote(x) for x in data]
    results = ray.get(futures)
except Exception:
    results = list(map(fn, data))
cap_return(results)
"#);
            run_cluster(&code, span)
        }
        "cluster_map_reduce" => {
            // cluster_map_reduce(map_fn_code, reduce_fn_code, list)
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let map_fn = args.remove(0).as_str(span)?.to_string();
            let reduce_fn = args.remove(0).as_str(span)?.to_string();
            let data_json = value_to_json(&args[0], span)?.to_string();
            let code = format!(r#"
import json, functools
data = json.loads('''{data_json}''')
map_fn   = {map_fn}
reduce_fn = {reduce_fn}
mapped   = list(map(map_fn, data))
result   = functools.reduce(reduce_fn, mapped)
cap_return(result)
"#);
            run_cluster(&code, span)
        }
        "cluster_dask_read" => {
            // cluster_dask_read(path) → {columns, num_rows, _data} (sample)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import dask.dataframe as dd
df = dd.read_csv("{path}")
sample = df.head(100).to_dict(orient="records")
cols = list(df.columns)
cap_return({{"columns": cols, "num_rows": len(df), "_data": sample}})
"#);
            run_cluster(&code, span)
        }
        "cluster_dask_groupby" => {
            // cluster_dask_groupby(path, group_col, agg_col, agg)
            if args.len() < 4 {
                return Err(CapError::TooFewArgs { expected: 4, got: args.len(), span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let group_col = args.remove(0).as_str(span)?.to_string();
            let agg_col = args.remove(0).as_str(span)?.to_string();
            let agg = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import dask.dataframe as dd
df = dd.read_csv("{path}")
result = getattr(df.groupby("{group_col}")["{agg_col}"], "{agg}")().compute()
cap_return(result.reset_index().to_dict(orient="records"))
"#);
            run_cluster(&code, span)
        }
        _ => Err(CapError::Runtime { message: format!("unknown cluster builtin: {name}"), span: span.clone() }),
    }
}
