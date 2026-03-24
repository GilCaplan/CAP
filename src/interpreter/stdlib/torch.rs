/// PyTorch module — thin wrappers for common neural-network tasks.
///
/// These cover device detection, tensor creation, simple training loops,
/// and model I/O. For complex architectures use `pyval()` directly.
use crate::error::{CapError, Span};
use crate::interpreter::stdlib::json::{json_to_value, value_to_json};
use crate::interpreter::stdlib::sys::run_python;
use crate::interpreter::value::Value;

pub const BUILTINS: &[&str] = &[
    "torch_device",
    "torch_tensor", "torch_zeros", "torch_ones",
    "torch_train_linear",
    "torch_save", "torch_load",
    "torch_predict",
];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "torch_device" => {
            // torch_device() → "cuda" | "mps" | "cpu"
            let code = r#"
import json
try:
    import torch
    if torch.cuda.is_available(): d = 'cuda'
    elif hasattr(torch.backends, 'mps') and torch.backends.mps.is_available(): d = 'mps'
    else: d = 'cpu'
except ImportError:
    d = 'cpu'
print(json.dumps(d))
"#;
            py_val(run_python(code, None, span)?, span)
        }
        "torch_tensor" => {
            // torch_tensor(list) → {shape, dtype, data} map
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let data = args.remove(0);
            let json = serialize(data, span)?;
            let code = r#"
import torch, json, sys
data = json.loads(sys.stdin.read())
t = torch.tensor(data, dtype=torch.float32)
print(json.dumps({'shape': list(t.shape), 'dtype': str(t.dtype), 'data': t.tolist()}))
"#;
            py_val(run_python(code, Some(Value::Str(json)), span)?, span)
        }
        "torch_zeros" => {
            // torch_zeros(shape_list) → {shape, data}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let shape = args.remove(0);
            let json = serialize(shape, span)?;
            let code = r#"
import torch, json, sys
shape = json.loads(sys.stdin.read())
t = torch.zeros(shape)
print(json.dumps({'shape': list(t.shape), 'data': t.tolist()}))
"#;
            py_val(run_python(code, Some(Value::Str(json)), span)?, span)
        }
        "torch_ones" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let shape = args.remove(0);
            let json = serialize(shape, span)?;
            let code = r#"
import torch, json, sys
shape = json.loads(sys.stdin.read())
t = torch.ones(shape)
print(json.dumps({'shape': list(t.shape), 'data': t.tolist()}))
"#;
            py_val(run_python(code, Some(Value::Str(json)), span)?, span)
        }
        "torch_train_linear" => {
            // torch_train_linear(X, y, opts)
            // opts: {lr, epochs, hidden, activation}
            // Returns: {weights, losses, model_state (base64)}
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let x = args.remove(0);
            let y = args.remove(0);
            let opts = args.into_iter().next().unwrap_or(Value::Null);
            let xj = serialize(x, span)?;
            let yj = serialize(y, span)?;
            let opts_j = serialize(opts, span)?;
            let input = format!(r#"{{"X": {xj}, "y": {yj}, "opts": {opts_j}}}"#);
            let code = r#"
import torch, torch.nn as nn, json, sys, base64, io
payload = json.loads(sys.stdin.read())
X = torch.tensor(payload['X'], dtype=torch.float32)
y = torch.tensor(payload['y'], dtype=torch.float32)
if y.dim() == 1: y = y.unsqueeze(1)
opts = payload.get('opts') or {}
lr = opts.get('lr', 0.01)
epochs = opts.get('epochs', 100)
hidden = opts.get('hidden', 16)
in_dim = X.shape[1] if X.dim() > 1 else 1
model = nn.Sequential(nn.Linear(in_dim, hidden), nn.ReLU(), nn.Linear(hidden, y.shape[1]))
opt = torch.optim.Adam(model.parameters(), lr=lr)
criterion = nn.MSELoss()
losses = []
for epoch in range(epochs):
    opt.zero_grad()
    pred = model(X)
    loss = criterion(pred, y)
    loss.backward()
    opt.step()
    if epoch % max(1, epochs // 20) == 0:
        losses.append({'epoch': epoch, 'loss': float(loss)})
buf = io.BytesIO()
torch.save(model.state_dict(), buf)
state_b64 = base64.b64encode(buf.getvalue()).decode()
print(json.dumps({'losses': losses, 'epochs': epochs, 'model_state': state_b64}))
"#;
            py_val(run_python(code, Some(Value::Str(input)), span)?, span)
        }
        "torch_predict" => {
            // torch_predict(model_state_b64, X, hidden?) → list of predictions
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let state = args.remove(0).as_str(span)?.to_string();
            let x = args.remove(0);
            let hidden = args.into_iter().next()
                .and_then(|v| if let Value::Int(n) = v { Some(n) } else { None })
                .unwrap_or(16);
            let xj = serialize(x, span)?;
            let input = format!(r#"{{"state": {state:?}, "X": {xj}, "hidden": {hidden}}}"#);
            let code = r#"
import torch, torch.nn as nn, json, sys, base64, io
payload = json.loads(sys.stdin.read())
X = torch.tensor(payload['X'], dtype=torch.float32)
in_dim = X.shape[1] if X.dim() > 1 else 1
hidden = payload.get('hidden', 16)
model = nn.Sequential(nn.Linear(in_dim, hidden), nn.ReLU(), nn.Linear(hidden, 1))
buf = io.BytesIO(base64.b64decode(payload['state']))
model.load_state_dict(torch.load(buf, map_location='cpu'))
model.eval()
with torch.no_grad():
    preds = model(X).squeeze(1).tolist()
print(json.dumps(preds))
"#;
            py_val(run_python(code, Some(Value::Str(input)), span)?, span)
        }
        "torch_save" => {
            // torch_save(model_state_b64, path)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let state = args.remove(0).as_str(span)?.to_string();
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import base64, json, sys
payload = json.loads(sys.stdin.read())
data = base64.b64decode(payload['state'])
with open(payload['path'], 'wb') as f: f.write(data)
print('"ok"')
"#);
            let input = format!(r#"{{"state": {state:?}, "path": {path:?}}}"#);
            py_val(run_python(&code, Some(Value::Str(input)), span)?, span)
        }
        "torch_load" => {
            // torch_load(path) → model_state_b64 string
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import base64, json
with open({path:?}, 'rb') as f: data = f.read()
print(json.dumps(base64.b64encode(data).decode()))
"#);
            py_val(run_python(&code, None, span)?, span)
        }

        _ => Err(CapError::Runtime { message: format!("unknown torch builtin: {name}"), span: span.clone() }),
    }
}

fn serialize(v: Value, span: &Span) -> Result<String, CapError> {
    let j = value_to_json(&v, span)?;
    Ok(serde_json::to_string(&j).unwrap())
}

fn py_val(out: Value, span: &Span) -> Result<Value, CapError> {
    match out {
        Value::Str(s) => {
            let j: serde_json::Value = serde_json::from_str(&s)
                .map_err(|e| CapError::Runtime {
                    message: format!("torch: invalid JSON: {e}\nOutput: {s}"),
                    span: span.clone(),
                })?;
            json_to_value(j, span)
        }
        other => Ok(other),
    }
}
