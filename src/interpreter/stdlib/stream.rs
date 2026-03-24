/// Streaming file I/O with optional gzip support (flate2)
use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use std::cell::RefCell;
use std::io::{BufRead, Write};
use std::rc::Rc;

pub const BUILTINS: &[&str] = &[
    "stream_lines", "stream_bytes", "stream_write", "stream_append",
    "gz_read", "gz_write", "gz_compress", "gz_decompress",
];

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "stream_lines" => {
            // stream_lines(path) → list of lines (lazy-collected but returned as list)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let file = std::fs::File::open(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            let reader = std::io::BufReader::new(file);
            let lines: Result<Vec<Value>, _> = reader.lines()
                .map(|l| l.map(Value::Str).map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() }))
                .collect();
            Ok(Value::List(Rc::new(RefCell::new(lines?))))
        }
        "stream_bytes" => {
            // stream_bytes(path) → list of ints (bytes)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let bytes = std::fs::read(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            let vals: Vec<Value> = bytes.iter().map(|&b| Value::Int(b as i64)).collect();
            Ok(Value::List(Rc::new(RefCell::new(vals))))
        }
        "stream_write" => {
            // stream_write(path, content_str)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let content = args.remove(0).as_str(span)?.to_string();
            std::fs::write(&path, content)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Null)
        }
        "stream_append" => {
            // stream_append(path, content_str)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let content = args.remove(0).as_str(span)?.to_string();
            let mut file = std::fs::OpenOptions::new()
                .append(true).create(true).open(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            file.write_all(content.as_bytes())
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            Ok(Value::Null)
        }
        "gz_read" => {
            // gz_read(path) → str (decompressed content)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let file = std::fs::File::open(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            let mut decoder = flate2::read::GzDecoder::new(file);
            let mut content = String::new();
            use std::io::Read;
            decoder.read_to_string(&mut content)
                .map_err(|e| CapError::Io { message: format!("gz_read: {e}"), span: span.clone() })?;
            Ok(Value::Str(content))
        }
        "gz_write" => {
            // gz_write(path, content_str) — compresses and writes
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let content = args.remove(0).as_str(span)?.to_string();
            let file = std::fs::File::create(&path)
                .map_err(|e| CapError::Io { message: format!("{e}"), span: span.clone() })?;
            let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            encoder.write_all(content.as_bytes())
                .map_err(|e| CapError::Io { message: format!("gz_write: {e}"), span: span.clone() })?;
            encoder.finish()
                .map_err(|e| CapError::Io { message: format!("gz_write finish: {e}"), span: span.clone() })?;
            Ok(Value::Null)
        }
        "gz_compress" => {
            // gz_compress(str) → str (base64-encoded gzip bytes)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let content = args.remove(0).as_str(span)?.to_string();
            let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(content.as_bytes())
                .map_err(|e| CapError::Runtime { message: format!("gz_compress: {e}"), span: span.clone() })?;
            let compressed = encoder.finish()
                .map_err(|e| CapError::Runtime { message: format!("gz_compress finish: {e}"), span: span.clone() })?;
            Ok(Value::Str(base64_encode(&compressed)))
        }
        "gz_decompress" => {
            // gz_decompress(base64_str) → str
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let b64 = args.remove(0).as_str(span)?.to_string();
            let bytes = base64_decode(&b64)
                .map_err(|e| CapError::Runtime { message: format!("gz_decompress: invalid base64: {e}"), span: span.clone() })?;
            let mut decoder = flate2::read::GzDecoder::new(bytes.as_slice());
            let mut out = String::new();
            use std::io::Read;
            decoder.read_to_string(&mut out)
                .map_err(|e| CapError::Runtime { message: format!("gz_decompress: {e}"), span: span.clone() })?;
            Ok(Value::Str(out))
        }
        _ => Err(CapError::Runtime { message: format!("unknown stream builtin: {name}"), span: span.clone() }),
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((n >> 18) & 63) as usize] as char);
        out.push(CHARS[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { CHARS[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { CHARS[(n & 63) as usize] as char } else { '=' });
    }
    out
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    const INV: [i8; 128] = {
        let mut t = [-1i8; 128];
        let mut i = 0usize;
        while i < 26 { t[b'A' as usize + i] = i as i8; i += 1; }
        let mut i = 0usize;
        while i < 26 { t[b'a' as usize + i] = (26 + i) as i8; i += 1; }
        let mut i = 0usize;
        while i < 10 { t[b'0' as usize + i] = (52 + i) as i8; i += 1; }
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
        t
    };
    let s: String = s.chars().filter(|&c| c != '=').collect();
    let bytes = s.as_bytes();
    if bytes.len() % 4 != 0 && (bytes.len() + (4 - bytes.len() % 4)) % 4 != 0 {
        // allow non-padded
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 < bytes.len() {
        let v: Result<Vec<i8>, _> = bytes[i..i+4].iter().map(|&b| {
            if b as usize >= 128 || INV[b as usize] < 0 {
                Err(format!("invalid char: {}", b as char))
            } else {
                Ok(INV[b as usize])
            }
        }).collect();
        let v = v?;
        let n = ((v[0] as u32) << 18) | ((v[1] as u32) << 12) | ((v[2] as u32) << 6) | (v[3] as u32);
        out.push(((n >> 16) & 0xff) as u8);
        out.push(((n >> 8) & 0xff) as u8);
        out.push((n & 0xff) as u8);
        i += 4;
    }
    // Handle remaining
    if i + 2 == bytes.len() {
        let a = INV[bytes[i] as usize] as u32;
        let b = INV[bytes[i+1] as usize] as u32;
        out.push(((a << 2) | (b >> 4)) as u8);
    } else if i + 3 == bytes.len() {
        let a = INV[bytes[i] as usize] as u32;
        let b = INV[bytes[i+1] as usize] as u32;
        let c = INV[bytes[i+2] as usize] as u32;
        out.push(((a << 2) | (b >> 4)) as u8);
        out.push(((b << 4) | (c >> 2)) as u8);
    }
    Ok(out)
}
