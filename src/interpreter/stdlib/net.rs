use crate::error::{CapError, Span};
use crate::interpreter::value::{MapKey, Value};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &["http_get", "http_post", "http_put", "http_delete", "http_request"];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    match name {
        "http_get" => {
            // http_get(url) or http_get(url, headers_map)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let mut args = args;
            let url = args.remove(0).as_str(span)?.to_string();
            let headers = if !args.is_empty() { Some(args.remove(0)) } else { None };
            http_request_simple("GET", &url, None, headers, span)
        }
        "http_post" => {
            // http_post(url, body_str_or_map) or http_post(url, body, headers_map)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let mut args = args;
            let url = args.remove(0).as_str(span)?.to_string();
            let body = args.remove(0);
            let headers = if !args.is_empty() { Some(args.remove(0)) } else { None };
            http_request_simple("POST", &url, Some(body), headers, span)
        }
        "http_put" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let mut args = args;
            let url = args.remove(0).as_str(span)?.to_string();
            let body = args.remove(0);
            let headers = if !args.is_empty() { Some(args.remove(0)) } else { None };
            http_request_simple("PUT", &url, Some(body), headers, span)
        }
        "http_delete" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let mut args = args;
            let url = args.remove(0).as_str(span)?.to_string();
            let headers = if !args.is_empty() { Some(args.remove(0)) } else { None };
            http_request_simple("DELETE", &url, None, headers, span)
        }
        "http_request" => {
            // http_request(method, url, body?, headers?) — low-level
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let mut args = args;
            let method = args.remove(0).as_str(span)?.to_uppercase();
            let url = args.remove(0).as_str(span)?.to_string();
            let body = if !args.is_empty() { Some(args.remove(0)) } else { None };
            let headers = if !args.is_empty() { Some(args.remove(0)) } else { None };
            http_request_full(&method, &url, body, headers, span)
        }
        _ => Err(CapError::Runtime { message: format!("unknown net builtin: {name}"), span: span.clone() }),
    }
}

fn http_request_simple(
    method: &str,
    url: &str,
    body: Option<Value>,
    headers: Option<Value>,
    span: &Span,
) -> Result<Value, CapError> {
    let resp = build_and_send(method, url, body, headers, span)?;
    // Return {status, body, headers} map for full access
    Ok(resp)
}

fn http_request_full(
    method: &str,
    url: &str,
    body: Option<Value>,
    headers: Option<Value>,
    span: &Span,
) -> Result<Value, CapError> {
    build_and_send(method, url, body, headers, span)
}

fn build_and_send(
    method: &str,
    url: &str,
    body: Option<Value>,
    headers: Option<Value>,
    span: &Span,
) -> Result<Value, CapError> {
    let client = reqwest::blocking::Client::new();
    let mut req = match method {
        "GET"    => client.get(url),
        "POST"   => client.post(url),
        "PUT"    => client.put(url),
        "DELETE" => client.delete(url),
        "PATCH"  => client.patch(url),
        "HEAD"   => client.head(url),
        other    => return Err(CapError::Http { message: format!("unknown HTTP method: {other}"), span: span.clone() }),
    };

    // Set headers from map
    if let Some(h) = headers {
        let hmap = h.as_map(span)?;
        for (k, v) in hmap.borrow().iter() {
            req = req.header(k.to_string(), v.display());
        }
    }

    // Set body
    if let Some(b) = body {
        match b {
            Value::Str(s)  => req = req.body(s),
            Value::Map(_) | Value::List(_) => {
                let j = crate::interpreter::stdlib::json::value_to_json(&b, span)?;
                req = req.header("content-type", "application/json")
                         .body(j.to_string());
            }
            other => req = req.body(other.display()),
        }
    }

    let response = req.send().map_err(|e| CapError::Http {
        message: format!("{method} {url}: {e}"),
        span: span.clone(),
    })?;

    let status = response.status().as_u16() as i64;
    // Collect response headers
    let mut resp_headers = IndexMap::new();
    for (k, v) in response.headers() {
        if let Ok(v_str) = v.to_str() {
            resp_headers.insert(MapKey::Str(k.to_string()), Value::Str(v_str.to_string()));
        }
    }
    let body_str = response.text().map_err(|e| CapError::Http {
        message: format!("reading body: {e}"),
        span: span.clone(),
    })?;

    let mut map = IndexMap::new();
    map.insert(MapKey::Str("status".into()), Value::Int(status));
    map.insert(MapKey::Str("body".into()),   Value::Str(body_str));
    map.insert(MapKey::Str("headers".into()), Value::Map(Rc::new(RefCell::new(resp_headers))));
    Ok(Value::Map(Rc::new(RefCell::new(map))))
}
