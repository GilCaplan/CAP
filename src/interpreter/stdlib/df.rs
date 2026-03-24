/// DataFrame module — pandas wrappers via Python subprocess.
///
/// Cap represents DataFrames as `list of maps` (each map is a row,
/// keys are column names). This matches pandas' `to_dict('records')`.
///
/// All df_* functions accept and return list-of-maps unless noted.
use crate::error::{CapError, Span};
use crate::interpreter::stdlib::json::{json_to_value, value_to_json};
use crate::interpreter::stdlib::sys::run_python;
use crate::interpreter::value::Value;

pub const BUILTINS: &[&str] = &[
    "df_read", "df_write",
    "df_head", "df_tail",
    "df_shape", "df_columns",
    "df_describe",
    "df_select", "df_filter",
    "df_sort", "df_groupby",
    "df_join", "df_drop",
    "df_rename", "df_fillna",
    "df_apply",
];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        // ── I/O ──────────────────────────────────────────────────────────────
        "df_read" => {
            // df_read(path) → list of maps
            // Supports CSV, JSON, Excel (.xlsx), TSV (auto-detected by extension)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import pandas as pd, json, sys, os
path = sys.stdin.read().strip()
ext = os.path.splitext(path)[1].lower()
if ext in ('.xlsx', '.xls'):
    df = pd.read_excel(path)
elif ext == '.json':
    df = pd.read_json(path)
elif ext in ('.tsv', '.txt'):
    df = pd.read_csv(path, sep='\t')
else:
    df = pd.read_csv(path)
print(json.dumps(df.where(pd.notnull(df), None).to_dict('records')))
"#);
            py_returns(run_python(&code, Some(Value::Str(path)), span)?, span)
        }
        "df_write" => {
            // df_write(data, path)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let data = args.remove(0);
            let path = args.remove(0).as_str(span)?.to_string();
            let json = serialize(data, span)?;
            let code = format!(r#"
import pandas as pd, json, sys, os
payload = json.loads(sys.stdin.read())
df = pd.DataFrame(payload['data'])
path = payload['path']
ext = os.path.splitext(path)[1].lower()
if ext == '.json':
    df.to_json(path, orient='records', indent=2)
elif ext in ('.xlsx',):
    df.to_excel(path, index=False)
else:
    df.to_csv(path, index=False)
print('null')
"#);
            let input = format!(r#"{{"data": {json}, "path": {path:?}}}"#);
            py_returns(run_python(&code, Some(Value::Str(input)), span)?, span)
        }

        // ── Inspection ───────────────────────────────────────────────────────
        "df_head" => {
            // df_head(data, n=5)
            let (data, n) = data_int(args, 5, span)?;
            let json = serialize(data, span)?;
            let code = format!("import pandas as pd, json, sys\ndf = pd.DataFrame(json.loads(sys.stdin.read()))\nprint(json.dumps(df.head({n}).to_dict('records')))");
            py_returns(run_python(&code, Some(Value::Str(json)), span)?, span)
        }
        "df_tail" => {
            let (data, n) = data_int(args, 5, span)?;
            let json = serialize(data, span)?;
            let code = format!("import pandas as pd, json, sys\ndf = pd.DataFrame(json.loads(sys.stdin.read()))\nprint(json.dumps(df.tail({n}).to_dict('records')))");
            py_returns(run_python(&code, Some(Value::Str(json)), span)?, span)
        }
        "df_shape" => {
            // df_shape(data) → {rows: int, cols: int}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let json = serialize(args.remove(0), span)?;
            let code = "import pandas as pd, json, sys\ndf = pd.DataFrame(json.loads(sys.stdin.read()))\nprint(json.dumps({'rows': df.shape[0], 'cols': df.shape[1]}))";
            py_returns(run_python(code, Some(Value::Str(json)), span)?, span)
        }
        "df_columns" => {
            // df_columns(data) → list of column names
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let json = serialize(args.remove(0), span)?;
            let code = "import pandas as pd, json, sys\ndf = pd.DataFrame(json.loads(sys.stdin.read()))\nprint(json.dumps(list(df.columns)))";
            py_returns(run_python(code, Some(Value::Str(json)), span)?, span)
        }
        "df_describe" => {
            // df_describe(data) → map of {col: {count, mean, std, min, 25%, 50%, 75%, max}}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let json = serialize(args.remove(0), span)?;
            let code = "import pandas as pd, json, sys\ndf = pd.DataFrame(json.loads(sys.stdin.read()))\nprint(df.describe().to_json())";
            py_returns(run_python(code, Some(Value::Str(json)), span)?, span)
        }

        // ── Selection & filtering ─────────────────────────────────────────────
        "df_select" => {
            // df_select(data, cols_list) → filtered columns
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let data = args.remove(0);
            let cols = args.remove(0);
            let cols_json = serde_json::to_string(&value_to_json(&cols, span)?).unwrap();
            let data_json = serialize(data, span)?;
            let code = format!(r#"
import pandas as pd, json, sys
payload = json.loads(sys.stdin.read())
df = pd.DataFrame(payload['data'])
cols = payload['cols']
print(json.dumps(df[cols].to_dict('records')))
"#);
            let input = format!(r#"{{"data": {data_json}, "cols": {cols_json}}}"#);
            py_returns(run_python(&code, Some(Value::Str(input)), span)?, span)
        }
        "df_filter" => {
            // df_filter(data, query_str) — pandas query syntax: "age > 18 and city == 'NYC'"
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let data = args.remove(0);
            let query = args.remove(0).as_str(span)?.to_string();
            let data_json = serialize(data, span)?;
            let code = format!(r#"
import pandas as pd, json, sys
df = pd.DataFrame(json.loads(sys.stdin.read()))
print(json.dumps(df.query({query:?}).to_dict('records')))
"#);
            py_returns(run_python(&code, Some(Value::Str(data_json)), span)?, span)
        }
        "df_drop" => {
            // df_drop(data, cols_list) → remove columns
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let data = args.remove(0);
            let cols = args.remove(0);
            let cols_json = serde_json::to_string(&value_to_json(&cols, span)?).unwrap();
            let data_json = serialize(data, span)?;
            let code = format!(r#"
import pandas as pd, json, sys
payload = json.loads(sys.stdin.read())
df = pd.DataFrame(payload['data'])
print(json.dumps(df.drop(columns=payload['cols']).to_dict('records')))
"#);
            let input = format!(r#"{{"data": {data_json}, "cols": {cols_json}}}"#);
            py_returns(run_python(&code, Some(Value::Str(input)), span)?, span)
        }

        // ── Sorting ───────────────────────────────────────────────────────────
        "df_sort" => {
            // df_sort(data, col) or df_sort(data, col, desc)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let data = args.remove(0);
            let col = args.remove(0).as_str(span)?.to_string();
            let desc = args.into_iter().next()
                .map(|v| v.is_truthy())
                .unwrap_or(false);
            let asc = !desc;
            let data_json = serialize(data, span)?;
            let code = format!(r#"
import pandas as pd, json, sys
df = pd.DataFrame(json.loads(sys.stdin.read()))
print(json.dumps(df.sort_values({col:?}, ascending={asc}).to_dict('records')))
"#, asc = if asc { "True" } else { "False" });
            py_returns(run_python(&code, Some(Value::Str(data_json)), span)?, span)
        }

        // ── Aggregation ───────────────────────────────────────────────────────
        "df_groupby" => {
            // df_groupby(data, col, agg)
            // agg: "sum" | "mean" | "count" | "min" | "max" | "first" | "last"
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let data = args.remove(0);
            let col = args.remove(0).as_str(span)?.to_string();
            let agg = args.remove(0).as_str(span)?.to_string();
            let data_json = serialize(data, span)?;
            let code = format!(r#"
import pandas as pd, json, sys
df = pd.DataFrame(json.loads(sys.stdin.read()))
g = df.groupby({col:?})
fn_name = {agg:?}
if fn_name in ('sum', 'mean', 'std', 'var', 'min', 'max'):
    result = getattr(g, fn_name)(numeric_only=True).reset_index()
else:
    result = getattr(g, fn_name)().reset_index()
print(json.dumps(result.where(pd.notnull(result), None).to_dict('records')))
"#);
            py_returns(run_python(&code, Some(Value::Str(data_json)), span)?, span)
        }

        // ── Joins ─────────────────────────────────────────────────────────────
        "df_join" => {
            // df_join(left, right, on) or df_join(left, right, on, how)
            // how: "inner" | "left" | "right" | "outer"  (default "inner")
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let left = args.remove(0);
            let right = args.remove(0);
            let on = args.remove(0).as_str(span)?.to_string();
            let how = args.into_iter().next()
                .and_then(|v| if let Value::Str(s) = v { Some(s) } else { None })
                .unwrap_or_else(|| "inner".to_string());
            let lj = serialize(left, span)?;
            let rj = serialize(right, span)?;
            let code = format!(r#"
import pandas as pd, json, sys
payload = json.loads(sys.stdin.read())
left = pd.DataFrame(payload['left'])
right = pd.DataFrame(payload['right'])
result = left.merge(right, on={on:?}, how={how:?})
print(json.dumps(result.where(pd.notnull(result), None).to_dict('records')))
"#);
            let input = format!(r#"{{"left": {lj}, "right": {rj}}}"#);
            py_returns(run_python(&code, Some(Value::Str(input)), span)?, span)
        }

        // ── Cleanup ───────────────────────────────────────────────────────────
        "df_rename" => {
            // df_rename(data, {old_name: new_name, ...})
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let data = args.remove(0);
            let mapping = args.remove(0);
            let map_json = serde_json::to_string(&value_to_json(&mapping, span)?).unwrap();
            let data_json = serialize(data, span)?;
            let code = format!(r#"
import pandas as pd, json, sys
payload = json.loads(sys.stdin.read())
df = pd.DataFrame(payload['data'])
print(json.dumps(df.rename(columns=payload['mapping']).to_dict('records')))
"#);
            let input = format!(r#"{{"data": {data_json}, "mapping": {map_json}}}"#);
            py_returns(run_python(&code, Some(Value::Str(input)), span)?, span)
        }
        "df_fillna" => {
            // df_fillna(data, value) — fill NaN/null with value
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let data = args.remove(0);
            let fill = args.remove(0);
            let fill_json = serde_json::to_string(&value_to_json(&fill, span)?).unwrap();
            let data_json = serialize(data, span)?;
            let code = format!(r#"
import pandas as pd, json, sys
payload = json.loads(sys.stdin.read())
df = pd.DataFrame(payload['data'])
print(json.dumps(df.fillna(payload['fill']).to_dict('records')))
"#);
            let input = format!(r#"{{"data": {data_json}, "fill": {fill_json}}}"#);
            py_returns(run_python(&code, Some(Value::Str(input)), span)?, span)
        }
        "df_apply" => {
            // df_apply(data, col, python_lambda_str)
            // python_lambda_str: e.g. "lambda x: x * 2"
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let data = args.remove(0);
            let col = args.remove(0).as_str(span)?.to_string();
            let lambda = args.remove(0).as_str(span)?.to_string();
            let data_json = serialize(data, span)?;
            let code = format!(r#"
import pandas as pd, json, sys
df = pd.DataFrame(json.loads(sys.stdin.read()))
df[{col:?}] = df[{col:?}].apply({lambda})
print(json.dumps(df.where(pd.notnull(df), None).to_dict('records')))
"#);
            py_returns(run_python(&code, Some(Value::Str(data_json)), span)?, span)
        }

        _ => Err(CapError::Runtime { message: format!("unknown df builtin: {name}"), span: span.clone() }),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Serialize a cap Value to a JSON string suitable for passing to Python stdin.
fn serialize(v: Value, span: &Span) -> Result<String, CapError> {
    let j = value_to_json(&v, span)?;
    Ok(serde_json::to_string(&j).unwrap())
}

/// Parse Python stdout (JSON string) back to a cap Value.
fn py_returns(out: Value, span: &Span) -> Result<Value, CapError> {
    match out {
        Value::Str(s) => {
            let j: serde_json::Value = serde_json::from_str(&s)
                .map_err(|e| CapError::Runtime {
                    message: format!("df: invalid JSON from Python: {e}\nOutput: {s}"),
                    span: span.clone(),
                })?;
            json_to_value(j, span)
        }
        other => Ok(other),
    }
}

fn data_int(mut args: Vec<Value>, default: i64, span: &Span) -> Result<(Value, i64), CapError> {
    if args.is_empty() {
        return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
    }
    let data = args.remove(0);
    let n = args.into_iter().next()
        .and_then(|v| if let Value::Int(n) = v { Some(n) } else { None })
        .unwrap_or(default);
    Ok((data, n))
}
