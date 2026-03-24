/// Apache Arrow / pyarrow integration via Python subprocess
use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use crate::interpreter::stdlib::sys::run_python;
use crate::interpreter::stdlib::json::{json_to_value, value_to_json};

pub const BUILTINS: &[&str] = &[
    "arrow_from_list", "arrow_to_list",
    "arrow_schema", "arrow_cast",
    "arrow_from_csv", "arrow_to_csv",
    "arrow_from_parquet", "arrow_to_parquet",
    "arrow_filter", "arrow_select", "arrow_sort",
    "arrow_aggregate",
];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    let run_arrow = |code: &str| -> Result<Value, CapError> {
        let wrapped = format!(
            "import json as _json, sys as _sys\n\
             def cap_return(__v): print(_json.dumps(__v)); _sys.exit(0)\n\
             {code}"
        );
        let out = run_python(&wrapped, None, span)?;
        if let Value::Str(s) = &out {
            if s.is_empty() { return Ok(Value::Null); }
            let j: serde_json::Value = serde_json::from_str(s)
                .map_err(|e| CapError::Runtime { message: format!("arrow: invalid JSON: {e}"), span: span.clone() })?;
            json_to_value(j, span)
        } else { Ok(Value::Null) }
    };

    let val_to_json_str = |v: &Value| -> Result<String, CapError> {
        let j = value_to_json(v, span)?;
        Ok(j.to_string())
    };

    match name {
        "arrow_from_list" => {
            // arrow_from_list(list_of_dicts) → opaque handle (json string of schema + data)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let data_json = val_to_json_str(&args[0])?;
            let code = format!(r#"
import pyarrow as pa, json
data = json.loads('''{data_json}''')
table = pa.Table.from_pylist(data)
schema = {{c: str(table.schema.field(c).type) for c in table.schema.names}}
cap_return({{"columns": table.schema.names, "schema": schema, "num_rows": table.num_rows, "_data": data}})
"#);
            run_arrow(&code)
        }
        "arrow_to_list" => {
            // arrow_to_list(handle) → list of dicts
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let data_json = val_to_json_str(&args[0])?;
            let code = format!(r#"
import json
handle = json.loads('''{data_json}''')
cap_return(handle.get("_data", []))
"#);
            run_arrow(&code)
        }
        "arrow_schema" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let data_json = val_to_json_str(&args[0])?;
            let code = format!(r#"
import pyarrow as pa, json
handle = json.loads('''{data_json}''')
data = handle.get("_data", [])
table = pa.Table.from_pylist(data)
schema = {{c: str(table.schema.field(c).type) for c in table.schema.names}}
cap_return(schema)
"#);
            run_arrow(&code)
        }
        "arrow_cast" => {
            // arrow_cast(handle, col, type_str) → new handle
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let data_json = val_to_json_str(&args[0])?;
            let col = args[1].as_str(span)?.to_string();
            let type_str = args[2].as_str(span)?.to_string();
            let code = format!(r#"
import pyarrow as pa, json
handle = json.loads('''{data_json}''')
data = handle.get("_data", [])
table = pa.Table.from_pylist(data)
col_idx = table.schema.get_field_index("{col}")
new_col = table.column("{col}").cast(pa.{type_str}())
table = table.set_column(col_idx, "{col}", new_col)
cap_return({{"columns": table.schema.names, "num_rows": table.num_rows, "_data": table.to_pylist()}})
"#);
            run_arrow(&code)
        }
        "arrow_from_csv" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import pyarrow as pa
import pyarrow.csv as pa_csv
table = pa_csv.read_csv("{path}")
cap_return({{"columns": table.schema.names, "num_rows": table.num_rows, "_data": table.to_pylist()}})
"#);
            run_arrow(&code)
        }
        "arrow_to_csv" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let data_json = val_to_json_str(&args[0])?;
            let path = args[1].as_str(span)?.to_string();
            let code = format!(r#"
import pyarrow as pa, json
import pyarrow.csv as pa_csv
handle = json.loads('''{data_json}''')
table = pa.Table.from_pylist(handle.get("_data", []))
pa_csv.write_csv(table, "{path}")
cap_return(None)
"#);
            run_arrow(&code)
        }
        "arrow_from_parquet" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import pyarrow.parquet as pq
table = pq.read_table("{path}")
cap_return({{"columns": table.schema.names, "num_rows": table.num_rows, "_data": table.to_pylist()}})
"#);
            run_arrow(&code)
        }
        "arrow_to_parquet" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let data_json = val_to_json_str(&args[0])?;
            let path = args[1].as_str(span)?.to_string();
            let code = format!(r#"
import pyarrow as pa, pyarrow.parquet as pq, json
handle = json.loads('''{data_json}''')
table = pa.Table.from_pylist(handle.get("_data", []))
pq.write_table(table, "{path}")
cap_return(None)
"#);
            run_arrow(&code)
        }
        "arrow_filter" => {
            // arrow_filter(handle, expr_str)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let data_json = val_to_json_str(&args[0])?;
            let expr = args[1].as_str(span)?.to_string();
            let code = format!(r#"
import pyarrow as pa, json
handle = json.loads('''{data_json}''')
import pandas as pd
df = pd.DataFrame(handle.get("_data", []))
df = df.query("{expr}")
data = df.to_dict(orient="records")
cap_return({{"columns": list(df.columns), "num_rows": len(df), "_data": data}})
"#);
            run_arrow(&code)
        }
        "arrow_select" => {
            // arrow_select(handle, [col1, col2, ...])
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let data_json = val_to_json_str(&args[0])?;
            let cols_json = val_to_json_str(&args[1])?;
            let code = format!(r#"
import pyarrow as pa, json
handle = json.loads('''{data_json}''')
cols = json.loads('''{cols_json}''')
table = pa.Table.from_pylist(handle.get("_data", []))
table = table.select(cols)
cap_return({{"columns": table.schema.names, "num_rows": table.num_rows, "_data": table.to_pylist()}})
"#);
            run_arrow(&code)
        }
        "arrow_sort" => {
            // arrow_sort(handle, col, desc=false)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let data_json = val_to_json_str(&args[0])?;
            let col = args[1].as_str(span)?.to_string();
            let desc = args.get(2).map(|v| matches!(v, Value::Bool(true))).unwrap_or(false);
            let order = if desc { "descending" } else { "ascending" };
            let code = format!(r#"
import pyarrow as pa, pyarrow.compute as pc, json
handle = json.loads('''{data_json}''')
table = pa.Table.from_pylist(handle.get("_data", []))
indices = pc.sort_indices(table, sort_keys=[("{col}", "{order}")])
table = table.take(indices)
cap_return({{"columns": table.schema.names, "num_rows": table.num_rows, "_data": table.to_pylist()}})
"#);
            run_arrow(&code)
        }
        "arrow_aggregate" => {
            // arrow_aggregate(handle, col, agg) where agg is "sum","mean","min","max","count"
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let data_json = val_to_json_str(&args[0])?;
            let col = args[1].as_str(span)?.to_string();
            let agg = args[2].as_str(span)?.to_string();
            let code = format!(r#"
import pyarrow as pa, pyarrow.compute as pc, json
handle = json.loads('''{data_json}''')
table = pa.Table.from_pylist(handle.get("_data", []))
col_arr = table.column("{col}")
result = getattr(pc, "{agg}")(col_arr).as_py()
cap_return(result)
"#);
            run_arrow(&code)
        }
        _ => Err(CapError::Runtime { message: format!("unknown arrow builtin: {name}"), span: span.clone() }),
    }
}
