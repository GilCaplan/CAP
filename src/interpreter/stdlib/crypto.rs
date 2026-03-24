/// Cryptography, hashing, encoding, random — all via Python stdlib (no extra deps)
use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use crate::interpreter::stdlib::sys::run_python;
use crate::interpreter::stdlib::json::json_to_value;

pub const BUILTINS: &[&str] = &[
    // Hashing
    "hash_md5", "hash_sha1", "hash_sha256", "hash_sha512",
    "hash_file",
    // HMAC
    "hmac_sha256", "hmac_sha512",
    // Base64
    "b64_encode", "b64_decode",
    "b64_url_encode", "b64_url_decode",
    // Hex
    "hex_encode", "hex_decode",
    // UUID
    "uuid_v4", "uuid_v5",
    // Random
    "rand_int", "rand_float", "rand_bytes", "rand_choice", "rand_shuffle",
    // Password hashing
    "pbkdf2_hash", "pbkdf2_verify",
];

fn run_crypto(code: &str, span: &Span) -> Result<Value, CapError> {
    let wrapped = format!(
        "import json as _json, sys as _sys\n\
         def cap_return(__v): print(_json.dumps(__v)); _sys.exit(0)\n\
         {code}"
    );
    let out = run_python(&wrapped, None, span)?;
    if let Value::Str(s) = &out {
        if s.is_empty() { return Ok(Value::Null); }
        let j: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| CapError::Runtime { message: format!("crypto: invalid JSON: {e}"), span: span.clone() })?;
        json_to_value(j, span)
    } else { Ok(Value::Null) }
}

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        // ── Hashing ───────────────────────────────────────────────────────────
        "hash_md5" | "hash_sha1" | "hash_sha256" | "hash_sha512" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let text = args.remove(0).as_str(span)?.to_string();
            let algo = name.strip_prefix("hash_").unwrap_or("sha256").replace("sha", "sha");
            let code = format!(r#"
import hashlib
h = hashlib.{algo}({text_json}.encode()).hexdigest()
cap_return(h)
"#, text_json = serde_json::json!(text).to_string());
            run_crypto(&code, span)
        }
        "hash_file" => {
            // hash_file(path, algo?) → hex string
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let algo = if !args.is_empty() { args.remove(0).as_str(span)?.to_string() } else { "sha256".into() };
            let code = format!(r#"
import hashlib
h = hashlib.{algo}()
with open("{path}", "rb") as f:
    for chunk in iter(lambda: f.read(8192), b""):
        h.update(chunk)
cap_return(h.hexdigest())
"#);
            run_crypto(&code, span)
        }
        // ── HMAC ──────────────────────────────────────────────────────────────
        "hmac_sha256" | "hmac_sha512" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let key = args.remove(0).as_str(span)?.to_string();
            let msg = args.remove(0).as_str(span)?.to_string();
            let algo = if name == "hmac_sha256" { "sha256" } else { "sha512" };
            let code = format!(r#"
import hmac, hashlib
h = hmac.new({key_json}.encode(), {msg_json}.encode(), hashlib.{algo})
cap_return(h.hexdigest())
"#,
                key_json = serde_json::json!(key).to_string(),
                msg_json = serde_json::json!(msg).to_string());
            run_crypto(&code, span)
        }
        // ── Base64 ────────────────────────────────────────────────────────────
        "b64_encode" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let text = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import base64
cap_return(base64.b64encode({text_json}.encode()).decode())
"#, text_json = serde_json::json!(text).to_string());
            run_crypto(&code, span)
        }
        "b64_decode" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let text = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import base64
cap_return(base64.b64decode({text_json}).decode("utf-8", errors="replace"))
"#, text_json = serde_json::json!(text).to_string());
            run_crypto(&code, span)
        }
        "b64_url_encode" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let text = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import base64
cap_return(base64.urlsafe_b64encode({text_json}.encode()).decode().rstrip("="))
"#, text_json = serde_json::json!(text).to_string());
            run_crypto(&code, span)
        }
        "b64_url_decode" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let text = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import base64
padded = {text_json} + "=" * (4 - len({text_json}) % 4)
cap_return(base64.urlsafe_b64decode(padded).decode("utf-8", errors="replace"))
"#, text_json = serde_json::json!(text).to_string());
            run_crypto(&code, span)
        }
        // ── Hex ───────────────────────────────────────────────────────────────
        "hex_encode" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let text = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
cap_return({text_json}.encode().hex())
"#, text_json = serde_json::json!(text).to_string());
            run_crypto(&code, span)
        }
        "hex_decode" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let hex = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
cap_return(bytes.fromhex({hex_json}).decode("utf-8", errors="replace"))
"#, hex_json = serde_json::json!(hex).to_string());
            run_crypto(&code, span)
        }
        // ── UUID ──────────────────────────────────────────────────────────────
        "uuid_v4" => {
            let code = "import uuid; cap_return(str(uuid.uuid4()))";
            run_crypto(code, span)
        }
        "uuid_v5" => {
            // uuid_v5(namespace, name) — namespace: "dns", "url", "oid", "x500"
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let ns   = args.remove(0).as_str(span)?.to_string().to_uppercase();
            let name = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import uuid
ns = getattr(uuid, f"NAMESPACE_{ns}", uuid.NAMESPACE_DNS)
cap_return(str(uuid.uuid5(ns, {name_json})))
"#, name_json = serde_json::json!(name).to_string());
            run_crypto(&code, span)
        }
        // ── Random ────────────────────────────────────────────────────────────
        "rand_int" => {
            // rand_int(min, max) → int in [min, max]
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let lo = match args.remove(0) { Value::Int(n) => n, _ => 0 };
            let hi = match args.remove(0) { Value::Int(n) => n, _ => 100 };
            let code = format!("import random; cap_return(random.randint({lo}, {hi}))");
            run_crypto(&code, span)
        }
        "rand_float" => {
            // rand_float() → float in [0, 1)
            let code = "import random; cap_return(random.random())";
            run_crypto(code, span)
        }
        "rand_bytes" => {
            // rand_bytes(n) → hex string of n random bytes
            let n = if args.is_empty() { 16i64 } else { match args.remove(0) { Value::Int(n) => n, _ => 16 } };
            let code = format!("import os; cap_return(os.urandom({n}).hex())");
            run_crypto(&code, span)
        }
        "rand_choice" => {
            // rand_choice(list) → random element
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let list_json = crate::interpreter::stdlib::json::value_to_json(&args[0], span)?.to_string();
            let code = format!(r#"
import random, json
lst = json.loads('''{list_json}''')
cap_return(random.choice(lst))
"#);
            run_crypto(&code, span)
        }
        "rand_shuffle" => {
            // rand_shuffle(list) → shuffled list (new)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let list_json = crate::interpreter::stdlib::json::value_to_json(&args[0], span)?.to_string();
            let code = format!(r#"
import random, json
lst = json.loads('''{list_json}''')
random.shuffle(lst)
cap_return(lst)
"#);
            run_crypto(&code, span)
        }
        // ── Password hashing ──────────────────────────────────────────────────
        "pbkdf2_hash" => {
            // pbkdf2_hash(password, salt?) → {hash, salt}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let pw = args.remove(0).as_str(span)?.to_string();
            let salt_arg = if !args.is_empty() { args.remove(0).as_str(span)?.to_string() } else { String::new() };
            let salt_code = if salt_arg.is_empty() {
                "import os; salt = os.urandom(16).hex()".to_string()
            } else {
                format!("salt = {}", serde_json::json!(salt_arg))
            };
            let code = format!(r#"
import hashlib
{salt_code}
dk = hashlib.pbkdf2_hmac("sha256", {pw_json}.encode(), salt.encode(), 100000)
import binascii
cap_return({{"hash": binascii.hexlify(dk).decode(), "salt": salt}})
"#, pw_json = serde_json::json!(pw).to_string());
            run_crypto(&code, span)
        }
        "pbkdf2_verify" => {
            // pbkdf2_verify(password, hash, salt) → bool
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let pw   = args.remove(0).as_str(span)?.to_string();
            let hash = args.remove(0).as_str(span)?.to_string();
            let salt = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import hashlib, binascii, hmac
dk = hashlib.pbkdf2_hmac("sha256", {pw_json}.encode(), {salt_json}.encode(), 100000)
computed = binascii.hexlify(dk).decode()
cap_return(hmac.compare_digest(computed, {hash_json}))
"#,
                pw_json   = serde_json::json!(pw).to_string(),
                salt_json = serde_json::json!(salt).to_string(),
                hash_json = serde_json::json!(hash).to_string());
            run_crypto(&code, span)
        }
        _ => Err(CapError::Runtime { message: format!("unknown crypto builtin: {name}"), span: span.clone() }),
    }
}
