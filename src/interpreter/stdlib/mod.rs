pub mod core;
pub mod list;
pub mod string;
pub mod io;
pub mod json;
pub mod net;
pub mod sys;
pub mod csv;
pub mod plot;
pub mod df;
pub mod torch;
pub mod fs;
pub mod time;
pub mod sql;
pub mod stream;
pub mod arrow;
pub mod task;
pub mod ffi;
pub mod cluster;
pub mod wasm;
pub mod llm;
pub mod vector;
pub mod server;
pub mod image;
pub mod crypto;
pub mod zip_archive;
pub mod sklearn;
pub mod pdf;

use crate::interpreter::value::Value;
use crate::interpreter::env::Env;

/// Register all built-in functions into the root environment.
pub fn register_all(env: &mut Env) {
    for name in core::BUILTINS        { env.set(name, Value::BuiltinFn(name)); }
    for name in list::BUILTINS        { env.set(name, Value::BuiltinFn(name)); }
    for name in string::BUILTINS      { env.set(name, Value::BuiltinFn(name)); }
    for name in io::BUILTINS          { env.set(name, Value::BuiltinFn(name)); }
    for name in json::BUILTINS        { env.set(name, Value::BuiltinFn(name)); }
    for name in net::BUILTINS         { env.set(name, Value::BuiltinFn(name)); }
    for name in sys::BUILTINS         { env.set(name, Value::BuiltinFn(name)); }
    for name in csv::BUILTINS         { env.set(name, Value::BuiltinFn(name)); }
    for name in plot::BUILTINS        { env.set(name, Value::BuiltinFn(name)); }
    for name in df::BUILTINS          { env.set(name, Value::BuiltinFn(name)); }
    for name in torch::BUILTINS       { env.set(name, Value::BuiltinFn(name)); }
    for name in fs::BUILTINS          { env.set(name, Value::BuiltinFn(name)); }
    for name in time::BUILTINS        { env.set(name, Value::BuiltinFn(name)); }
    for name in sql::BUILTINS         { env.set(name, Value::BuiltinFn(name)); }
    for name in stream::BUILTINS      { env.set(name, Value::BuiltinFn(name)); }
    for name in arrow::BUILTINS       { env.set(name, Value::BuiltinFn(name)); }
    for name in task::BUILTINS        { env.set(name, Value::BuiltinFn(name)); }
    for name in ffi::BUILTINS         { env.set(name, Value::BuiltinFn(name)); }
    for name in cluster::BUILTINS     { env.set(name, Value::BuiltinFn(name)); }
    for name in wasm::BUILTINS        { env.set(name, Value::BuiltinFn(name)); }
    for name in llm::BUILTINS         { env.set(name, Value::BuiltinFn(name)); }
    for name in vector::BUILTINS      { env.set(name, Value::BuiltinFn(name)); }
    for name in server::BUILTINS      { env.set(name, Value::BuiltinFn(name)); }
    for name in image::BUILTINS       { env.set(name, Value::BuiltinFn(name)); }
    for name in crypto::BUILTINS      { env.set(name, Value::BuiltinFn(name)); }
    for name in zip_archive::BUILTINS { env.set(name, Value::BuiltinFn(name)); }
    for name in sklearn::BUILTINS     { env.set(name, Value::BuiltinFn(name)); }
    for name in pdf::BUILTINS         { env.set(name, Value::BuiltinFn(name)); }
}
