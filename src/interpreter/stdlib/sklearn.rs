/// Scikit-Learn ML operations via Python subprocess
use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use crate::interpreter::stdlib::sys::run_python;
use crate::interpreter::stdlib::json::{json_to_value, value_to_json};

pub const BUILTINS: &[&str] = &[
    // Supervised learning
    "sklearn_train", "sklearn_predict", "sklearn_score",
    // Model persistence
    "sklearn_save", "sklearn_load",
    // Preprocessing
    "sklearn_scale", "sklearn_normalize", "sklearn_encode_labels",
    "sklearn_train_test_split",
    // Metrics
    "sklearn_metrics",
    // Clustering
    "sklearn_kmeans", "sklearn_dbscan",
    // Feature importance
    "sklearn_feature_importance",
    // Cross validation
    "sklearn_cross_val",
    // Pipeline / grid search
    "sklearn_grid_search",
];

fn run_sk(code: &str, span: &Span) -> Result<Value, CapError> {
    let wrapped = format!(
        "import json as _json, sys as _sys\n\
         def cap_return(__v): print(_json.dumps(__v)); _sys.exit(0)\n\
         {code}"
    );
    let out = run_python(&wrapped, None, span)?;
    if let Value::Str(s) = &out {
        if s.is_empty() { return Ok(Value::Null); }
        let j: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| CapError::Runtime { message: format!("sklearn: invalid JSON: {e}"), span: span.clone() })?;
        json_to_value(j, span)
    } else { Ok(Value::Null) }
}

fn v2j(v: &Value, span: &Span) -> Result<String, CapError> {
    Ok(value_to_json(v, span)?.to_string())
}

/// Map a model name string to sklearn constructor
fn model_code(model_name: &str, params_json: &str) -> String {
    format!(r#"
import json
_params = json.loads('''{params_json}''')
_model_map = {{
    "linear":            "LinearRegression",
    "logistic":          "LogisticRegression",
    "ridge":             "Ridge",
    "lasso":             "Lasso",
    "svm":               "SVC",
    "svr":               "SVR",
    "tree":              "DecisionTreeClassifier",
    "decision_tree":     "DecisionTreeClassifier",
    "dtree":             "DecisionTreeClassifier",
    "dtree_reg":         "DecisionTreeRegressor",
    "forest":            "RandomForestClassifier",
    "random_forest":     "RandomForestClassifier",
    "rf":                "RandomForestClassifier",
    "rf_reg":            "RandomForestRegressor",
    "gbm":               "GradientBoostingClassifier",
    "gradient_boosting": "GradientBoostingClassifier",
    "gbm_reg":           "GradientBoostingRegressor",
    "knn":               "KNeighborsClassifier",
    "knn_reg":           "KNeighborsRegressor",
    "nb":                "GaussianNB",
    "mlp":               "MLPClassifier",
    "mlp_reg":           "MLPRegressor",
    "xgb":               "XGBClassifier",
}}
_cls_name = _model_map.get("{model_name}", "{model_name}")
import importlib
for _mod in ["sklearn.linear_model", "sklearn.svm", "sklearn.tree",
             "sklearn.ensemble", "sklearn.neighbors", "sklearn.naive_bayes",
             "sklearn.neural_network", "xgboost"]:
    try:
        _mod_obj = importlib.import_module(_mod)
        if hasattr(_mod_obj, _cls_name):
            _ModelClass = getattr(_mod_obj, _cls_name)
            break
    except ImportError:
        continue
_params.pop("model", None)
model = _ModelClass(**_params)
"#)
}

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "sklearn_train" => {
            // sklearn_train(X, y, params?)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let x_json = v2j(&args.remove(0), span)?;
            let y_json = v2j(&args.remove(0), span)?;
            let params = if !args.is_empty() { args.remove(0) } else { Value::Map(std::rc::Rc::new(std::cell::RefCell::new(indexmap::IndexMap::new()))) };
            
            let mut model_name = "random_forest".to_string();
            if let Value::Map(ref m) = params {
                if let Some(Value::Str(s)) = m.borrow().get(&crate::interpreter::value::MapKey::Str("model".into())) {
                    model_name = s.clone();
                }
            }
            let params_json = v2j(&params, span)?;
            let mc = model_code(&model_name, &params_json);
            let code = format!(r#"
import json, pickle, base64
import numpy as np
{mc}
X = np.array(json.loads('''{x_json}'''))
y = np.array(json.loads('''{y_json}'''))
model.fit(X, y)
score = float(model.score(X, y))
model_b64 = base64.b64encode(pickle.dumps(model)).decode()
cap_return({{"model": model_b64, "score": score, "model_name": "{model_name}"}})
"#);
            run_sk(&code, span)
        }
        "sklearn_predict" => {
            // sklearn_predict(model_b64, X) → list of predictions
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let model_b64 = match &args[0] {
                Value::Map(m) => {
                    let b = m.borrow();
                    match b.get(&crate::interpreter::value::MapKey::Str("model".into())) {
                        Some(Value::Str(s)) => s.clone(),
                        _ => return Err(CapError::Runtime { message: "sklearn_predict: expected model map with 'model' key".into(), span: span.clone() }),
                    }
                }
                Value::Str(s) => s.clone(),
                _ => return Err(CapError::TypeError { expected: "model or str", got: args[0].type_name().to_string(), span: span.clone() }),
            };
            let x_json = v2j(&args[1], span)?;
            let code = format!(r#"
import json, pickle, base64
import numpy as np
model = pickle.loads(base64.b64decode({model_b64_json}))
X = np.array(json.loads('''{x_json}'''))
preds = model.predict(X).tolist()
cap_return(preds)
"#, model_b64_json = serde_json::json!(model_b64).to_string());
            run_sk(&code, span)
        }
        "sklearn_score" => {
            // sklearn_score(model_map, X, y) → float
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let model_b64 = match &args[0] {
                Value::Map(m) => {
                    let b = m.borrow();
                    match b.get(&crate::interpreter::value::MapKey::Str("model".into())) {
                        Some(Value::Str(s)) => s.clone(),
                        _ => return Err(CapError::Runtime { message: "expected model map".into(), span: span.clone() }),
                    }
                }
                Value::Str(s) => s.clone(),
                _ => return Err(CapError::TypeError { expected: "model map or str", got: args[0].type_name().to_string(), span: span.clone() }),
            };
            let x_json = v2j(&args[1], span)?;
            let y_json = v2j(&args[2], span)?;
            let code = format!(r#"
import json, pickle, base64
import numpy as np
model = pickle.loads(base64.b64decode({model_b64_json}))
X = np.array(json.loads('''{x_json}'''))
y = np.array(json.loads('''{y_json}'''))
cap_return(float(model.score(X, y)))
"#, model_b64_json = serde_json::json!(model_b64).to_string());
            run_sk(&code, span)
        }
        "sklearn_save" => {
            // sklearn_save(model_map, path)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let model_b64 = match &args[0] {
                Value::Map(m) => {
                    let b = m.borrow();
                    match b.get(&crate::interpreter::value::MapKey::Str("model".into())) {
                        Some(Value::Str(s)) => s.clone(),
                        _ => return Err(CapError::Runtime { message: "expected model map".into(), span: span.clone() }),
                    }
                }
                Value::Str(s) => s.clone(),
                _ => return Err(CapError::TypeError { expected: "model map or str", got: args[0].type_name().to_string(), span: span.clone() }),
            };
            let path = args[1].as_str(span)?.to_string();
            let code = format!(r#"
import base64
data = base64.b64decode({model_b64_json})
with open("{path}", "wb") as f:
    f.write(data)
cap_return("{path}")
"#, model_b64_json = serde_json::json!(model_b64).to_string());
            run_sk(&code, span)
        }
        "sklearn_load" => {
            // sklearn_load(path) → model map
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import pickle, base64
with open("{path}", "rb") as f:
    data = f.read()
model_b64 = base64.b64encode(data).decode()
cap_return({{"model": model_b64}})
"#);
            run_sk(&code, span)
        }
        "sklearn_scale" => {
            // sklearn_scale(X) → {X_scaled, mean, std}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let x_json = v2j(&args[0], span)?;
            let code = format!(r#"
import json, numpy as np
from sklearn.preprocessing import StandardScaler
X = np.array(json.loads('''{x_json}'''))
scaler = StandardScaler()
X_scaled = scaler.fit_transform(X)
cap_return({{"X_scaled": X_scaled.tolist(), "mean": scaler.mean_.tolist(), "std": scaler.scale_.tolist()}})
"#);
            run_sk(&code, span)
        }
        "sklearn_normalize" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let x_json = v2j(&args[0], span)?;
            let code = format!(r#"
import json, numpy as np
from sklearn.preprocessing import normalize
X = np.array(json.loads('''{x_json}'''))
cap_return(normalize(X).tolist())
"#);
            run_sk(&code, span)
        }
        "sklearn_encode_labels" => {
            // sklearn_encode_labels(labels_list) → {encoded, classes}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let y_json = v2j(&args[0], span)?;
            let code = format!(r#"
import json
from sklearn.preprocessing import LabelEncoder
y = json.loads('''{y_json}''')
le = LabelEncoder()
encoded = le.fit_transform(y).tolist()
cap_return({{"encoded": encoded, "classes": le.classes_.tolist()}})
"#);
            run_sk(&code, span)
        }
        "sklearn_train_test_split" => {
            // sklearn_train_test_split(X, y, test_size?, random_state?)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let x_json = v2j(&args.remove(0), span)?;
            let y_json = v2j(&args.remove(0), span)?;
            let test_size = if !args.is_empty() {
                match args.remove(0) { Value::Float(f) => f, Value::Int(n) => n as f64, _ => 0.2 }
            } else { 0.2 };
            let seed = if !args.is_empty() { match args.remove(0) { Value::Int(n) => n, _ => 42 } } else { 42 };
            let code = format!(r#"
import json, numpy as np
from sklearn.model_selection import train_test_split
X = np.array(json.loads('''{x_json}'''))
y = np.array(json.loads('''{y_json}'''))
X_tr, X_te, y_tr, y_te = train_test_split(X, y, test_size={test_size}, random_state={seed})
cap_return({{"X_train": X_tr.tolist(), "X_test": X_te.tolist(),
             "y_train": y_tr.tolist(), "y_test": y_te.tolist()}})
"#);
            run_sk(&code, span)
        }
        "sklearn_metrics" => {
            // sklearn_metrics(y_true, y_pred, task?) — task: "classification" or "regression"
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let yt_json = v2j(&args.remove(0), span)?;
            let yp_json = v2j(&args.remove(0), span)?;
            let task = if !args.is_empty() { args.remove(0).as_str(span)?.to_string() } else { "auto".into() };
            let code = format!(r#"
import json, numpy as np
from sklearn import metrics
y_true = np.array(json.loads('''{yt_json}'''))
y_pred = np.array(json.loads('''{yp_json}'''))
task = "{task}"
is_cls = task == "classification" or (task == "auto" and y_true.dtype == object or len(np.unique(y_true)) < 20)
if is_cls:
    cap_return({{
        "accuracy": float(metrics.accuracy_score(y_true, y_pred)),
        "precision": float(metrics.precision_score(y_true, y_pred, average="weighted", zero_division=0)),
        "recall": float(metrics.recall_score(y_true, y_pred, average="weighted", zero_division=0)),
        "f1": float(metrics.f1_score(y_true, y_pred, average="weighted", zero_division=0)),
    }})
else:
    cap_return({{
        "mse": float(metrics.mean_squared_error(y_true, y_pred)),
        "rmse": float(metrics.mean_squared_error(y_true, y_pred, squared=False)),
        "mae": float(metrics.mean_absolute_error(y_true, y_pred)),
        "r2": float(metrics.r2_score(y_true, y_pred)),
    }})
"#);
            run_sk(&code, span)
        }
        "sklearn_kmeans" => {
            // sklearn_kmeans(X, k, opts?) → {labels, centroids, inertia}
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let x_json = v2j(&args.remove(0), span)?;
            let k = match args.remove(0) { Value::Int(n) => n, _ => 3 };
            let opts_json = if !args.is_empty() { v2j(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import json, numpy as np
from sklearn.cluster import KMeans
X = np.array(json.loads('''{x_json}'''))
opts = json.loads('''{opts_json}''')
km = KMeans(n_clusters={k}, random_state=opts.get("seed", 42), n_init="auto")
km.fit(X)
cap_return({{"labels": km.labels_.tolist(), "centroids": km.cluster_centers_.tolist(),
             "inertia": float(km.inertia_)}})
"#);
            run_sk(&code, span)
        }
        "sklearn_dbscan" => {
            // sklearn_dbscan(X, eps?, min_samples?) → {labels, n_clusters, noise_count}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let x_json = v2j(&args.remove(0), span)?;
            let eps = if !args.is_empty() { match args.remove(0) { Value::Float(f) => f, Value::Int(n) => n as f64, _ => 0.5 } } else { 0.5 };
            let min_s = if !args.is_empty() { match args.remove(0) { Value::Int(n) => n, _ => 5 } } else { 5 };
            let code = format!(r#"
import json, numpy as np
from sklearn.cluster import DBSCAN
X = np.array(json.loads('''{x_json}'''))
db = DBSCAN(eps={eps}, min_samples={min_s}).fit(X)
labels = db.labels_.tolist()
n_clusters = len(set(l for l in labels if l != -1))
noise = labels.count(-1)
cap_return({{"labels": labels, "n_clusters": n_clusters, "noise_count": noise}})
"#);
            run_sk(&code, span)
        }
        "sklearn_feature_importance" => {
            // sklearn_feature_importance(model_map, feature_names?)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let model_b64 = match &args[0] {
                Value::Map(m) => {
                    let b = m.borrow();
                    match b.get(&crate::interpreter::value::MapKey::Str("model".into())) {
                        Some(Value::Str(s)) => s.clone(),
                        _ => return Err(CapError::Runtime { message: "expected model map".into(), span: span.clone() }),
                    }
                }
                Value::Str(s) => s.clone(),
                _ => return Err(CapError::TypeError { expected: "model map", got: args[0].type_name().to_string(), span: span.clone() }),
            };
            let names_json = if args.len() > 1 { v2j(&args[1], span)? } else { "null".into() };
            let code = format!(r#"
import json, pickle, base64
model = pickle.loads(base64.b64decode({model_b64_json}))
names = json.loads('''{names_json}''')
imp = getattr(model, "feature_importances_", None)
if imp is None:
    coef = getattr(model, "coef_", None)
    imp = abs(coef).flatten().tolist() if coef is not None else []
else:
    imp = imp.tolist()
if names:
    result = [dict(feature=n, importance=i) for n, i in zip(names, imp)]
else:
    result = [dict(feature=str(i), importance=v) for i, v in enumerate(imp)]
result.sort(key=lambda x: -x["importance"])
cap_return(result)
"#, model_b64_json = serde_json::json!(model_b64).to_string());
            run_sk(&code, span)
        }
        "sklearn_cross_val" => {
            // sklearn_cross_val(params_with_model, X, y, cv_opts?)
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let params = args.remove(0);
            let x_json = v2j(&args.remove(0), span)?;
            let y_json = v2j(&args.remove(0), span)?;
            let cv_opts = if !args.is_empty() { args.remove(0) } else { Value::Null };
            
            let mut model_name = "random_forest".to_string();
            if let Value::Map(ref m) = params {
                if let Some(Value::Str(s)) = m.borrow().get(&crate::interpreter::value::MapKey::Str("model".into())) {
                    model_name = s.clone();
                }
            }
            
            let mut cv = 5;
            if let Value::Map(ref m) = cv_opts {
                 if let Some(Value::Int(n)) = m.borrow().get(&crate::interpreter::value::MapKey::Str("cv".into())) {
                     cv = *n;
                 }
            }
            
            let params_json = v2j(&params, span)?;
            let mc = model_code(&model_name, &params_json);
            let code = format!(r#"
import json, numpy as np
from sklearn.model_selection import cross_val_score
{mc}
X = np.array(json.loads('''{x_json}'''))
y = np.array(json.loads('''{y_json}'''))
scores = cross_val_score(model, X, y, cv={cv})
cap_return({{"scores": scores.tolist(), "mean": float(scores.mean()), "std": float(scores.std())}})
"#);
            run_sk(&code, span)
        }
        "sklearn_grid_search" => {
            // sklearn_grid_search(param_grid, X, y, cv_opts?)
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let param_grid = args.remove(0);
            let x_json = v2j(&args.remove(0), span)?;
            let y_json = v2j(&args.remove(0), span)?;
            let cv_opts = if !args.is_empty() { args.remove(0) } else { Value::Null };
            
            let mut model_name = "random_forest".to_string();
            if let Value::Map(ref m) = param_grid {
                if let Some(Value::Str(s)) = m.borrow().get(&crate::interpreter::value::MapKey::Str("model".into())) {
                    model_name = s.clone();
                }
            }
            
            let mut cv = 5;
            if let Value::Map(ref m) = cv_opts {
                 if let Some(Value::Int(n)) = m.borrow().get(&crate::interpreter::value::MapKey::Str("cv".into())) {
                     cv = *n;
                 }
            }

            let mc = model_code(&model_name, "{}");
            let grid_json = v2j(&param_grid, span)?;
            let code = format!(r#"
import json, numpy as np
from sklearn.model_selection import GridSearchCV
{mc}
X = np.array(json.loads('''{x_json}'''))
y = np.array(json.loads('''{y_json}'''))
param_grid = json.loads('''{grid_json}''')
param_grid.pop("model", None)
gs = GridSearchCV(model, param_grid, cv={cv})
gs.fit(X, y)
cap_return({{"best_params": gs.best_params_, "best_score": float(gs.best_score_)}})
"#);
            run_sk(&code, span)
        }
        _ => Err(CapError::Runtime { message: format!("unknown sklearn builtin: {name}"), span: span.clone() }),
    }
}
