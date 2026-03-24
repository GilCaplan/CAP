/// HTTP server utilities via Python (Flask / http.server)
/// For agents: webhook receivers, simple REST endpoints, static file serving
use crate::error::{CapError, Span};
use crate::interpreter::value::{MapKey, Value};
use crate::interpreter::stdlib::json::{json_to_value, value_to_json};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &[
    "server_serve_once",     // wait for one HTTP request, return it
    "server_start",          // start Flask server in background subprocess
    "server_stop",           // stop background server
    "server_static",         // serve a directory (blocking)
    "server_mock",           // start a mock server that records requests
    "server_poll",           // poll mock server for recorded requests
];

thread_local! {
    static SERVER_PROC: RefCell<Option<std::process::Child>> = RefCell::new(None);
    static MOCK_FILE: RefCell<Option<String>> = RefCell::new(None);
}

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "server_serve_once" => {
            // server_serve_once(port, timeout_secs?)
            // Starts a one-shot HTTP server, waits for ONE request, returns:
            // {method, path, headers, body, query}
            let port = if args.is_empty() { 8080i64 } else {
                match args.remove(0) { Value::Int(n) => n, _ => 8080 }
            };
            let timeout = if !args.is_empty() {
                match args.remove(0) { Value::Int(n) => n as f64, Value::Float(f) => f, _ => 30.0 }
            } else { 30.0 };
            let code = format!(r#"
import json, socket, threading
from http.server import BaseHTTPRequestHandler, HTTPServer

received = {{}}

class Handler(BaseHTTPRequestHandler):
    def log_message(self, format, *args): pass
    def handle_request(self):
        content_len = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_len).decode("utf-8", errors="replace") if content_len else ""
        path_parts = self.path.split("?", 1)
        received.update({{
            "method": self.command,
            "path": path_parts[0],
            "query": path_parts[1] if len(path_parts) > 1 else "",
            "headers": dict(self.headers),
            "body": body
        }})
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b'{{"ok": true}}')
    def do_GET(self): self.handle_request()
    def do_POST(self): self.handle_request()
    def do_PUT(self): self.handle_request()
    def do_DELETE(self): self.handle_request()
    def do_PATCH(self): self.handle_request()

server = HTTPServer(("0.0.0.0", {port}), Handler)
server.timeout = {timeout}
server.handle_request()
server.server_close()
cap_return(received)
"#);
            let wrapped = format!(
                "import json as _json, sys as _sys\n\
                 def cap_return(__v): print(_json.dumps(__v)); _sys.exit(0)\n\
                 {code}"
            );
            let out = crate::interpreter::stdlib::sys::run_python(&wrapped, None, span)?;
            if let Value::Str(s) = &out {
                if s.is_empty() { return Ok(Value::Null); }
                let j: serde_json::Value = serde_json::from_str(s)
                    .map_err(|e| CapError::Runtime { message: format!("server: {e}"), span: span.clone() })?;
                json_to_value(j, span)
            } else { Ok(Value::Null) }
        }

        "server_start" => {
            // server_start(port, routes_map)
            // routes_map: {"GET /path": "response body", ...}
            // Starts Flask server in a background subprocess
            let port = if args.is_empty() { 8080i64 } else {
                match args.remove(0) { Value::Int(n) => n, _ => 8080 }
            };
            let routes_json = if !args.is_empty() {
                value_to_json(&args[0], span)?.to_string()
            } else { "{}".into() };

            let script = format!(r#"
import json, sys
from flask import Flask, request, jsonify

routes = json.loads('''{routes_json}''')
app = Flask(__name__)

@app.route("/<path:path>", methods=["GET","POST","PUT","DELETE","PATCH"])
def catch_all(path):
    key = f"{{request.method}} /{{path}}"
    key2 = f"{{request.method}} /"
    resp = routes.get(key) or routes.get(key2) or {{"ok": True}}
    if isinstance(resp, str):
        return resp
    return jsonify(resp)

@app.route("/", methods=["GET","POST","PUT","DELETE","PATCH"])
def root():
    key = f"{{request.method}} /"
    resp = routes.get(key) or {{"ok": True}}
    if isinstance(resp, str):
        return resp
    return jsonify(resp)

app.run(host="0.0.0.0", port={port}, debug=False)
"#);
            // Write script to temp file and launch as background process
            let script_path = format!("/tmp/cap_server_{port}.py");
            std::fs::write(&script_path, &script)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;

            let child = std::process::Command::new("python3")
                .arg(&script_path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| CapError::Runtime { message: format!("server_start: {e}"), span: span.clone() })?;
            let pid = child.id();
            SERVER_PROC.with(|p| *p.borrow_mut() = Some(child));

            // Brief wait for Flask to start
            std::thread::sleep(std::time::Duration::from_millis(500));

            let mut map = IndexMap::new();
            map.insert(MapKey::Str("ok".into()), Value::Bool(true));
            map.insert(MapKey::Str("port".into()), Value::Int(port));
            map.insert(MapKey::Str("pid".into()), Value::Int(pid as i64));
            map.insert(MapKey::Str("url".into()), Value::Str(format!("http://localhost:{port}")));
            Ok(Value::Map(Rc::new(RefCell::new(map))))
        }

        "server_stop" => {
            SERVER_PROC.with(|p| {
                if let Some(mut child) = p.borrow_mut().take() {
                    let _ = child.kill();
                }
            });
            Ok(Value::Null)
        }

        "server_static" => {
            // server_static(dir, port) — blocking static file server
            let dir = if args.is_empty() { ".".to_string() } else {
                args.remove(0).as_str(span)?.to_string()
            };
            let port = if !args.is_empty() {
                match args.remove(0) { Value::Int(n) => n, _ => 8080 }
            } else { 8080 };
            let code = format!(r#"
import http.server, os
os.chdir("{dir}")
handler = http.server.SimpleHTTPRequestHandler
with http.server.HTTPServer(("0.0.0.0", {port}), handler) as httpd:
    print(f"Serving {{dir}} on port {port}", flush=True)
    httpd.serve_forever()
"#);
            crate::interpreter::stdlib::sys::run_python(&code, None, span)
        }

        "server_mock" => {
            // server_mock(port) → starts a mock server that records all requests to a temp file
            let port = if args.is_empty() { 8080i64 } else {
                match args.remove(0) { Value::Int(n) => n, _ => 8080 }
            };
            let log_file = format!("/tmp/cap_mock_{port}.jsonl");
            MOCK_FILE.with(|f| *f.borrow_mut() = Some(log_file.clone()));
            let _ = std::fs::write(&log_file, ""); // clear

            let script = format!(r#"
import json
from http.server import BaseHTTPRequestHandler, HTTPServer

LOG = "{log_file}"

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def handle_req(self):
        content_len = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_len).decode("utf-8", errors="replace") if content_len else ""
        path_parts = self.path.split("?", 1)
        entry = {{"method": self.command, "path": path_parts[0],
                  "query": path_parts[1] if len(path_parts) > 1 else "",
                  "headers": dict(self.headers), "body": body}}
        with open(LOG, "a") as f:
            f.write(json.dumps(entry) + "\n")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b'{{"ok": true}}')
    def do_GET(self): self.handle_req()
    def do_POST(self): self.handle_req()
    def do_PUT(self): self.handle_req()
    def do_DELETE(self): self.handle_req()

HTTPServer(("0.0.0.0", {port}), Handler).serve_forever()
"#);
            let script_path = format!("/tmp/cap_mock_server_{port}.py");
            std::fs::write(&script_path, &script)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            let child = std::process::Command::new("python3")
                .arg(&script_path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| CapError::Runtime { message: format!("server_mock: {e}"), span: span.clone() })?;
            let pid = child.id();
            SERVER_PROC.with(|p| *p.borrow_mut() = Some(child));
            std::thread::sleep(std::time::Duration::from_millis(300));
            let mut map = IndexMap::new();
            map.insert(MapKey::Str("ok".into()), Value::Bool(true));
            map.insert(MapKey::Str("port".into()), Value::Int(port));
            map.insert(MapKey::Str("pid".into()), Value::Int(pid as i64));
            map.insert(MapKey::Str("log".into()), Value::Str(log_file));
            Ok(Value::Map(Rc::new(RefCell::new(map))))
        }

        "server_poll" => {
            // server_poll() → list of recorded requests (clears log)
            let log_file = MOCK_FILE.with(|f| f.borrow().clone())
                .unwrap_or_else(|| "/tmp/cap_mock_8080.jsonl".to_string());
            let content = std::fs::read_to_string(&log_file).unwrap_or_default();
            let _ = std::fs::write(&log_file, ""); // clear after reading
            let mut requests = Vec::new();
            for line in content.lines() {
                if line.is_empty() { continue; }
                if let Ok(j) = serde_json::from_str::<serde_json::Value>(line) {
                    if let Ok(v) = json_to_value(j, span) {
                        requests.push(v);
                    }
                }
            }
            Ok(Value::List(Rc::new(RefCell::new(requests))))
        }

        _ => Err(CapError::Runtime { message: format!("unknown server builtin: {name}"), span: span.clone() }),
    }
}
