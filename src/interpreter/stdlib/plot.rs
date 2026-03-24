/// Plot module — matplotlib / seaborn wrappers via Python subprocess.
///
/// All functions accept an optional last argument `opts` (a cap map) with keys:
///   title, xlabel, ylabel, save (file path), color, figsize ([w, h])
///
/// Default save path: /tmp/cap_plot_<unix_ms>.png
/// Return value: the file path string.
use crate::error::{CapError, Span};
use crate::interpreter::stdlib::json::value_to_json;
use crate::interpreter::stdlib::sys::run_python;
use crate::interpreter::value::Value;

pub const BUILTINS: &[&str] = &[
    "plt_line", "plt_bar", "plt_scatter", "plt_hist",
    "plt_heatmap", "plt_boxplot", "plt_pie", "plt_show",
];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    match name {
        "plt_line" => {
            // plt_line(x, y) or plt_line(x, y, opts)
            let (x, y, opts) = xy_opts(args, span)?;
            let save = save_path(&opts);
            let input = build_xy_input(&x, &y, &opts, span)?;
            let code = format!(r#"
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt, json, sys
d = json.loads(sys.stdin.read())
fig, ax = plt.subplots(figsize=d.get('figsize', [8, 5]))
ax.plot(d['x'], d['y'], color=d.get('color') or 'steelblue', label=d.get('label'))
if d.get('title'): ax.set_title(d['title'])
if d.get('xlabel'): ax.set_xlabel(d['xlabel'])
if d.get('ylabel'): ax.set_ylabel(d['ylabel'])
if d.get('label'): ax.legend()
fig.tight_layout(); fig.savefig({save:?}); print({save:?})
"#);
            run_python(&code, Some(Value::Str(input)), span)
        }
        "plt_bar" => {
            // plt_bar(labels, values) or plt_bar(labels, values, opts)
            let (labels, values, opts) = xy_opts(args, span)?;
            let save = save_path(&opts);
            let input = build_xy_input(&labels, &values, &opts, span)?;
            let code = format!(r#"
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt, json, sys
d = json.loads(sys.stdin.read())
fig, ax = plt.subplots(figsize=d.get('figsize', [8, 5]))
ax.bar(d['x'], d['y'], color=d.get('color') or 'steelblue')
if d.get('title'): ax.set_title(d['title'])
if d.get('xlabel'): ax.set_xlabel(d['xlabel'])
if d.get('ylabel'): ax.set_ylabel(d['ylabel'])
fig.tight_layout(); fig.savefig({save:?}); print({save:?})
"#);
            run_python(&code, Some(Value::Str(input)), span)
        }
        "plt_scatter" => {
            // plt_scatter(x, y) or plt_scatter(x, y, opts)
            let (x, y, opts) = xy_opts(args, span)?;
            let save = save_path(&opts);
            let input = build_xy_input(&x, &y, &opts, span)?;
            let code = format!(r#"
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt, json, sys
d = json.loads(sys.stdin.read())
fig, ax = plt.subplots(figsize=d.get('figsize', [8, 5]))
ax.scatter(d['x'], d['y'], c=d.get('color') or 'steelblue', alpha=0.7)
if d.get('title'): ax.set_title(d['title'])
if d.get('xlabel'): ax.set_xlabel(d['xlabel'])
if d.get('ylabel'): ax.set_ylabel(d['ylabel'])
fig.tight_layout(); fig.savefig({save:?}); print({save:?})
"#);
            run_python(&code, Some(Value::Str(input)), span)
        }
        "plt_hist" => {
            // plt_hist(data) or plt_hist(data, opts)
            let (data, opts) = single_opts(args, span)?;
            let save = save_path(&opts);
            let bins = opts_int(&opts, "bins", 20);
            let input = build_single_input(&data, &opts, span)?;
            let code = format!(r#"
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt, json, sys
d = json.loads(sys.stdin.read())
fig, ax = plt.subplots(figsize=d.get('figsize', [8, 5]))
ax.hist(d['values'], bins={bins}, color=d.get('color') or 'steelblue', edgecolor='white')
if d.get('title'): ax.set_title(d['title'])
if d.get('xlabel'): ax.set_xlabel(d['xlabel'])
if d.get('ylabel'): ax.set_ylabel(d['ylabel'])
fig.tight_layout(); fig.savefig({save:?}); print({save:?})
"#);
            run_python(&code, Some(Value::Str(input)), span)
        }
        "plt_boxplot" => {
            // plt_boxplot(list_of_lists) or plt_boxplot(list_of_lists, opts)
            // opts may include labels: ["A","B","C"]
            let (data, opts) = single_opts(args, span)?;
            let save = save_path(&opts);
            let input = build_single_input(&data, &opts, span)?;
            let code = format!(r#"
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt, json, sys
d = json.loads(sys.stdin.read())
fig, ax = plt.subplots(figsize=d.get('figsize', [8, 5]))
ax.boxplot(d['values'], labels=d.get('labels') or None)
if d.get('title'): ax.set_title(d['title'])
if d.get('xlabel'): ax.set_xlabel(d['xlabel'])
if d.get('ylabel'): ax.set_ylabel(d['ylabel'])
fig.tight_layout(); fig.savefig({save:?}); print({save:?})
"#);
            run_python(&code, Some(Value::Str(input)), span)
        }
        "plt_heatmap" => {
            // plt_heatmap(matrix) or plt_heatmap(matrix, opts)
            // opts may include xticklabels, yticklabels, annot (bool)
            let (data, opts) = single_opts(args, span)?;
            let save = save_path(&opts);
            let input = build_single_input(&data, &opts, span)?;
            let code = format!(r#"
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt, json, sys
d = json.loads(sys.stdin.read())
try:
    import seaborn as sns
    fig, ax = plt.subplots(figsize=d.get('figsize', [8, 6]))
    sns.heatmap(d['values'], ax=ax, annot=bool(d.get('annot')),
                xticklabels=d.get('xticklabels', 'auto'),
                yticklabels=d.get('yticklabels', 'auto'))
except ImportError:
    fig, ax = plt.subplots(figsize=d.get('figsize', [8, 6]))
    im = ax.imshow(d['values'], aspect='auto')
    plt.colorbar(im, ax=ax)
if d.get('title'): ax.set_title(d['title'])
fig.tight_layout(); fig.savefig({save:?}); print({save:?})
"#);
            run_python(&code, Some(Value::Str(input)), span)
        }
        "plt_pie" => {
            // plt_pie(values, labels, opts?)
            let mut args = args;
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let values = args.remove(0);
            let labels = args.remove(0);
            let opts = args.into_iter().next().unwrap_or(Value::Null);
            let save = save_path(&opts);
            let vj = value_to_json(&values, span)?;
            let lj = value_to_json(&labels, span)?;
            let mut obj = serde_json::Map::new();
            obj.insert("values".into(), vj);
            obj.insert("labels".into(), lj);
            append_opts(&opts, &mut obj, span)?;
            let input = serde_json::to_string(&obj).unwrap();
            let code = format!(r#"
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt, json, sys
d = json.loads(sys.stdin.read())
fig, ax = plt.subplots(figsize=d.get('figsize', [7, 7]))
ax.pie(d['values'], labels=d['labels'], autopct='%1.1f%%')
if d.get('title'): ax.set_title(d['title'])
fig.tight_layout(); fig.savefig({save:?}); print({save:?})
"#);
            run_python(&code, Some(Value::Str(input)), span)
        }
        "plt_show" => {
            // plt_show(path) — open in system viewer
            let mut args = args;
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let cmd = if cfg!(target_os = "macos") {
                format!("open {path:?}")
            } else if cfg!(target_os = "windows") {
                format!("start {path:?}")
            } else {
                format!("xdg-open {path:?}")
            };
            std::process::Command::new("sh").arg("-c").arg(&cmd).spawn().ok();
            Ok(Value::Null)
        }
        _ => Err(CapError::Runtime { message: format!("unknown plot builtin: {name}"), span: span.clone() }),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn default_save_path() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("/tmp/cap_plot_{ms}.png")
}

fn save_path(opts: &Value) -> String {
    if let Value::Map(m) = opts {
        if let Some(Value::Str(s)) = m.borrow().get(&crate::interpreter::value::MapKey::Str("save".into())) {
            return s.clone();
        }
    }
    default_save_path()
}

fn opts_int(opts: &Value, key: &str, default: i64) -> i64 {
    if let Value::Map(m) = opts {
        if let Some(v) = m.borrow().get(&crate::interpreter::value::MapKey::Str(key.into())) {
            if let Value::Int(n) = v { return *n; }
        }
    }
    default
}

fn append_opts(opts: &Value, obj: &mut serde_json::Map<String, serde_json::Value>, span: &Span) -> Result<(), CapError> {
    if let Value::Map(m) = opts {
        for (k, v) in m.borrow().iter() {
            if k.to_string() == "save" { continue; }
            obj.insert(k.to_string(), value_to_json(v, span)?);
        }
    }
    Ok(())
}

fn xy_opts(mut args: Vec<Value>, span: &Span) -> Result<(Value, Value, Value), CapError> {
    if args.len() < 2 {
        return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
    }
    let x = args.remove(0);
    let y = args.remove(0);
    let opts = args.into_iter().next().unwrap_or(Value::Null);
    Ok((x, y, opts))
}

fn single_opts(mut args: Vec<Value>, span: &Span) -> Result<(Value, Value), CapError> {
    if args.is_empty() {
        return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
    }
    let data = args.remove(0);
    let opts = args.into_iter().next().unwrap_or(Value::Null);
    Ok((data, opts))
}

fn build_xy_input(x: &Value, y: &Value, opts: &Value, span: &Span) -> Result<String, CapError> {
    let mut obj = serde_json::Map::new();
    obj.insert("x".into(), value_to_json(x, span)?);
    obj.insert("y".into(), value_to_json(y, span)?);
    append_opts(opts, &mut obj, span)?;
    Ok(serde_json::to_string(&obj).unwrap())
}

fn build_single_input(data: &Value, opts: &Value, span: &Span) -> Result<String, CapError> {
    let mut obj = serde_json::Map::new();
    obj.insert("values".into(), value_to_json(data, span)?);
    append_opts(opts, &mut obj, span)?;
    Ok(serde_json::to_string(&obj).unwrap())
}
