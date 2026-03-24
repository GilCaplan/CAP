/// LLM inference: Ollama, OpenAI-compatible, Anthropic, Google Gemini
use crate::error::{CapError, Span};
use crate::interpreter::value::{MapKey, Value};
use crate::interpreter::stdlib::json::{json_to_value, value_to_json};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &[
    // Generic
    "llm_chat", "llm_complete", "llm_embed",
    // Ollama
    "ollama_chat", "ollama_complete", "ollama_embed", "ollama_list", "ollama_pull",
    // OpenAI-compatible (OpenAI, Groq, LLMod, etc.)
    "openai_chat", "openai_embed",
    // Anthropic
    "anthropic_chat",
    // Google Gemini
    "gemini_chat",
    // LangChain-style helpers
    "llm_messages", "llm_system", "llm_user", "llm_assistant",
];

fn run_llm(code: &str, span: &Span) -> Result<Value, CapError> {
    let wrapped = format!(
        "import json as _json, sys as _sys\n\
         def cap_return(__v): print(_json.dumps(__v)); _sys.exit(0)\n\
         {code}"
    );
    let out = crate::interpreter::stdlib::sys::run_python(&wrapped, None, span)?;
    if let Value::Str(s) = &out {
        if s.is_empty() { return Ok(Value::Null); }
        let j: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| CapError::Runtime { message: format!("llm: invalid JSON: {e}\nOutput: {s}"), span: span.clone() })?;
        json_to_value(j, span)
    } else { Ok(Value::Null) }
}

fn val_to_json_str(v: &Value, span: &Span) -> Result<String, CapError> {
    Ok(value_to_json(v, span)?.to_string())
}

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        // ── Ollama ────────────────────────────────────────────────────────────
        "ollama_chat" | "llm_chat" => {
            // ollama_chat(model, messages, opts?)
            // messages: list of {role, content}
            // opts: {temperature, max_tokens, host}
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let model = args.remove(0).as_str(span)?.to_string();
            let msgs_json = val_to_json_str(&args.remove(0), span)?;
            let opts_json = if !args.is_empty() { val_to_json_str(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import json, urllib.request
msgs   = json.loads('''{msgs_json}''')
opts   = json.loads('''{opts_json}''')
host   = opts.get("host", "http://localhost:11434")
payload = {{"model": "{model}", "messages": msgs, "stream": False,
             "options": {{k: v for k, v in opts.items() if k not in ("host",)}}}}
req = urllib.request.Request(f"{{host}}/api/chat",
    data=json.dumps(payload).encode(), headers={{"Content-Type": "application/json"}})
with urllib.request.urlopen(req, timeout=120) as r:
    resp = json.loads(r.read())
msg = resp.get("message", {{}})
cap_return({{"content": msg.get("content", ""), "role": msg.get("role", "assistant"),
             "model": resp.get("model", "{model}"),
             "tokens": resp.get("eval_count", 0)}})
"#);
            run_llm(&code, span)
        }
        "ollama_complete" | "llm_complete" => {
            // ollama_complete(model, prompt, opts?)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 2, got: 0, span: span.clone() });
            }
            let model = args.remove(0).as_str(span)?.to_string();
            let prompt = args.remove(0).as_str(span)?.to_string();
            let opts_json = if !args.is_empty() { val_to_json_str(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import json, urllib.request
opts  = json.loads('''{opts_json}''')
host  = opts.get("host", "http://localhost:11434")
payload = {{"model": "{model}", "prompt": {prompt_json}, "stream": False,
             "options": {{k: v for k, v in opts.items() if k not in ("host",)}}}}
req = urllib.request.Request(f"{{host}}/api/generate",
    data=json.dumps(payload).encode(), headers={{"Content-Type": "application/json"}})
with urllib.request.urlopen(req, timeout=120) as r:
    resp = json.loads(r.read())
cap_return({{"content": resp.get("response", ""), "model": "{model}",
             "tokens": resp.get("eval_count", 0)}})
"#, prompt_json = serde_json::json!(prompt).to_string());
            run_llm(&code, span)
        }
        "ollama_embed" | "llm_embed" => {
            // ollama_embed(model, text_or_list)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let model = args.remove(0).as_str(span)?.to_string();
            let input_json = val_to_json_str(&args[0], span)?;
            let opts_json = if args.len() > 1 { val_to_json_str(&args[1], span)? } else { "{}".into() };
            let code = format!(r#"
import json, urllib.request
input_val = json.loads('''{input_json}''')
opts = json.loads('''{opts_json}''')
host = opts.get("host", "http://localhost:11434")
if isinstance(input_val, str):
    payload = {{"model": "{model}", "input": input_val}}
    req = urllib.request.Request(f"{{host}}/api/embed",
        data=json.dumps(payload).encode(), headers={{"Content-Type": "application/json"}})
    with urllib.request.urlopen(req, timeout=60) as r:
        resp = json.loads(r.read())
    cap_return(resp.get("embeddings", [[]])[0])
else:
    payload = {{"model": "{model}", "input": input_val}}
    req = urllib.request.Request(f"{{host}}/api/embed",
        data=json.dumps(payload).encode(), headers={{"Content-Type": "application/json"}})
    with urllib.request.urlopen(req, timeout=60) as r:
        resp = json.loads(r.read())
    cap_return(resp.get("embeddings", []))
"#);
            run_llm(&code, span)
        }
        "ollama_list" => {
            let opts_json = if !args.is_empty() { val_to_json_str(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import json, urllib.request
opts = json.loads('''{opts_json}''')
host = opts.get("host", "http://localhost:11434")
with urllib.request.urlopen(f"{{host}}/api/tags", timeout=10) as r:
    resp = json.loads(r.read())
cap_return([m["name"] for m in resp.get("models", [])])
"#);
            run_llm(&code, span)
        }
        "ollama_pull" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let model = args.remove(0).as_str(span)?.to_string();
            let opts_json = if !args.is_empty() { val_to_json_str(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import json, urllib.request
opts = json.loads('''{opts_json}''')
host = opts.get("host", "http://localhost:11434")
payload = {{"name": "{model}", "stream": False}}
req = urllib.request.Request(f"{{host}}/api/pull",
    data=json.dumps(payload).encode(), headers={{"Content-Type": "application/json"}})
with urllib.request.urlopen(req, timeout=300) as r:
    resp = json.loads(r.read())
cap_return({{"ok": True, "status": resp.get("status", ""), "model": "{model}"}})
"#);
            run_llm(&code, span)
        }

        // ── OpenAI-compatible ─────────────────────────────────────────────────
        "openai_chat" => {
            // openai_chat(model, messages, opts?)
            // opts: {api_key, base_url, temperature, max_tokens, ...}
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let model = args.remove(0).as_str(span)?.to_string();
            let msgs_json = val_to_json_str(&args.remove(0), span)?;
            let opts_json = if !args.is_empty() { val_to_json_str(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import json, urllib.request, os
msgs = json.loads('''{msgs_json}''')
opts = json.loads('''{opts_json}''')
api_key  = opts.pop("api_key", None) or os.environ.get("OPENAI_API_KEY", "")
base_url = opts.pop("base_url", "https://api.openai.com/v1")
payload  = {{"model": "{model}", "messages": msgs, **opts}}
req = urllib.request.Request(f"{{base_url}}/chat/completions",
    data=json.dumps(payload).encode(),
    headers={{"Content-Type": "application/json", "Authorization": f"Bearer {{api_key}}"}})
with urllib.request.urlopen(req, timeout=120) as r:
    resp = json.loads(r.read())
choice = resp["choices"][0]["message"]
usage  = resp.get("usage", {{}})
cap_return({{"content": choice["content"], "role": choice["role"],
             "model": resp.get("model", "{model}"),
             "input_tokens": usage.get("prompt_tokens", 0),
             "output_tokens": usage.get("completion_tokens", 0)}})
"#);
            run_llm(&code, span)
        }
        "openai_embed" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let model = args.remove(0).as_str(span)?.to_string();
            let input_json = val_to_json_str(&args.remove(0), span)?;
            let opts_json = if !args.is_empty() { val_to_json_str(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import json, urllib.request, os
input_val = json.loads('''{input_json}''')
opts = json.loads('''{opts_json}''')
api_key  = opts.pop("api_key", None) or os.environ.get("OPENAI_API_KEY", "")
base_url = opts.pop("base_url", "https://api.openai.com/v1")
payload  = {{"model": "{model}", "input": input_val}}
req = urllib.request.Request(f"{{base_url}}/embeddings",
    data=json.dumps(payload).encode(),
    headers={{"Content-Type": "application/json", "Authorization": f"Bearer {{api_key}}"}})
with urllib.request.urlopen(req, timeout=60) as r:
    resp = json.loads(r.read())
data = resp["data"]
if isinstance(input_val, str):
    cap_return(data[0]["embedding"])
else:
    cap_return([d["embedding"] for d in data])
"#);
            run_llm(&code, span)
        }

        // ── Anthropic ─────────────────────────────────────────────────────────
        "anthropic_chat" => {
            // anthropic_chat(model, messages, opts?)
            // opts: {api_key, system, max_tokens, temperature}
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let model = args.remove(0).as_str(span)?.to_string();
            let msgs_json = val_to_json_str(&args.remove(0), span)?;
            let opts_json = if !args.is_empty() { val_to_json_str(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import json, urllib.request, os
msgs    = json.loads('''{msgs_json}''')
opts    = json.loads('''{opts_json}''')
api_key = opts.pop("api_key", None) or os.environ.get("ANTHROPIC_API_KEY", "")
system  = opts.pop("system", None)
max_tok = opts.pop("max_tokens", 1024)
payload = {{"model": "{model}", "messages": msgs, "max_tokens": max_tok, **opts}}
if system: payload["system"] = system
req = urllib.request.Request("https://api.anthropic.com/v1/messages",
    data=json.dumps(payload).encode(),
    headers={{"Content-Type": "application/json",
              "x-api-key": api_key,
              "anthropic-version": "2023-06-01"}})
with urllib.request.urlopen(req, timeout=120) as r:
    resp = json.loads(r.read())
content = resp["content"][0]["text"] if resp.get("content") else ""
usage   = resp.get("usage", {{}})
cap_return({{"content": content, "role": "assistant",
             "model": resp.get("model", "{model}"),
             "input_tokens": usage.get("input_tokens", 0),
             "output_tokens": usage.get("output_tokens", 0)}})
"#);
            run_llm(&code, span)
        }

        // ── Google Gemini ──────────────────────────────────────────────────────
        "gemini_chat" => {
            // gemini_chat(model, messages, opts?)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let model = args.remove(0).as_str(span)?.to_string();
            let msgs_json = val_to_json_str(&args.remove(0), span)?;
            let opts_json = if !args.is_empty() { val_to_json_str(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import json, urllib.request, os
msgs    = json.loads('''{msgs_json}''')
opts    = json.loads('''{opts_json}''')
api_key = opts.pop("api_key", None) or os.environ.get("GOOGLE_API_KEY", "") or os.environ.get("GEMINI_API_KEY", "")
# Convert cap-style messages to Gemini contents format
contents = []
for m in msgs:
    role = "model" if m["role"] == "assistant" else "user"
    contents.append({{"role": role, "parts": [{{"text": m["content"]}}]}})
payload = {{"contents": contents}}
if "temperature" in opts: payload["generationConfig"] = {{"temperature": opts["temperature"]}}
url = f"https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={{api_key}}"
req = urllib.request.Request(url, data=json.dumps(payload).encode(),
    headers={{"Content-Type": "application/json"}})
with urllib.request.urlopen(req, timeout=120) as r:
    resp = json.loads(r.read())
text = resp["candidates"][0]["content"]["parts"][0]["text"]
usage = resp.get("usageMetadata", {{}})
cap_return({{"content": text, "role": "assistant", "model": "{model}",
             "input_tokens": usage.get("promptTokenCount", 0),
             "output_tokens": usage.get("candidatesTokenCount", 0)}})
"#);
            run_llm(&code, span)
        }

        // ── Message builder helpers ────────────────────────────────────────────
        "llm_messages" => {
            // llm_messages(msg1, msg2, ...) → list of message maps
            // Each arg may be a map (from llm_system/llm_user/llm_assistant) or a string (treated as user message)
            let mut msgs: Vec<Value> = vec![];
            for a in args {
                match a {
                    Value::Map(_) => msgs.push(a),
                    other => {
                        let s = other.as_str(span)?.to_string();
                        let mut m = IndexMap::new();
                        m.insert(MapKey::Str("role".into()), Value::Str("user".into()));
                        m.insert(MapKey::Str("content".into()), Value::Str(s));
                        msgs.push(Value::Map(Rc::new(RefCell::new(m))));
                    }
                }
            }
            Ok(Value::List(Rc::new(RefCell::new(msgs))))
        }
        "llm_system" => {
            // llm_system(content) → {role: "system", content: ...}
            let content = args.into_iter().next().unwrap_or(Value::Null).as_str(span)?.to_string();
            let mut m = IndexMap::new();
            m.insert(MapKey::Str("role".into()), Value::Str("system".into()));
            m.insert(MapKey::Str("content".into()), Value::Str(content));
            Ok(Value::Map(Rc::new(RefCell::new(m))))
        }
        "llm_user" => {
            let content = args.into_iter().next().unwrap_or(Value::Null).as_str(span)?.to_string();
            let mut m = IndexMap::new();
            m.insert(MapKey::Str("role".into()), Value::Str("user".into()));
            m.insert(MapKey::Str("content".into()), Value::Str(content));
            Ok(Value::Map(Rc::new(RefCell::new(m))))
        }
        "llm_assistant" => {
            let content = args.into_iter().next().unwrap_or(Value::Null).as_str(span)?.to_string();
            let mut m = IndexMap::new();
            m.insert(MapKey::Str("role".into()), Value::Str("assistant".into()));
            m.insert(MapKey::Str("content".into()), Value::Str(content));
            Ok(Value::Map(Rc::new(RefCell::new(m))))
        }
        _ => Err(CapError::Runtime { message: format!("unknown llm builtin: {name}"), span: span.clone() }),
    }
}
