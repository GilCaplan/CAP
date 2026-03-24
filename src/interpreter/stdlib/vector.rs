/// Vector database support: ChromaDB + Pinecone via Python subprocess
use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use crate::interpreter::stdlib::sys::run_python;
use crate::interpreter::stdlib::json::{json_to_value, value_to_json};

pub const BUILTINS: &[&str] = &[
    // ChromaDB
    "chroma_client", "chroma_collection", "chroma_add", "chroma_query",
    "chroma_get", "chroma_delete", "chroma_count", "chroma_list_collections",
    "chroma_delete_collection",
    // Pinecone
    "pinecone_init", "pinecone_upsert", "pinecone_query",
    "pinecone_delete", "pinecone_fetch", "pinecone_describe",
    // Generic cosine similarity
    "vec_cosine_sim", "vec_dot", "vec_norm",
];

fn run_vec(code: &str, span: &Span) -> Result<Value, CapError> {
    let wrapped = format!(
        "import json as _json, sys as _sys\n\
         def cap_return(__v): print(_json.dumps(__v)); _sys.exit(0)\n\
         {code}"
    );
    let out = run_python(&wrapped, None, span)?;
    if let Value::Str(s) = &out {
        if s.is_empty() { return Ok(Value::Null); }
        let j: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| CapError::Runtime { message: format!("vector: invalid JSON: {e}"), span: span.clone() })?;
        json_to_value(j, span)
    } else { Ok(Value::Null) }
}

fn v2j(v: &Value, span: &Span) -> Result<String, CapError> {
    Ok(value_to_json(v, span)?.to_string())
}

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        // ── ChromaDB ──────────────────────────────────────────────────────────
        "chroma_client" => {
            // chroma_client(opts?) → client handle (just a config map)
            // opts: {host, port, path}  — path → PersistentClient, host/port → HttpClient
            let opts_json = if !args.is_empty() { v2j(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import chromadb, json
opts = json.loads('''{opts_json}''')
if "path" in opts:
    client = chromadb.PersistentClient(path=opts["path"])
    cap_return({{"type": "persistent", "path": opts["path"]}})
elif "host" in opts:
    cap_return({{"type": "http", "host": opts.get("host","localhost"), "port": opts.get("port", 8000)}})
else:
    client = chromadb.EphemeralClient()
    cap_return({{"type": "ephemeral"}})
"#);
            run_vec(&code, span)
        }
        "chroma_collection" => {
            // chroma_collection(name, opts?) → creates/gets collection
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let col_name = args.remove(0).as_str(span)?.to_string();
            let opts_json = if !args.is_empty() { v2j(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import chromadb, json
opts = json.loads('''{opts_json}''')
if "path" in opts:
    client = chromadb.PersistentClient(path=opts["path"])
elif "host" in opts:
    client = chromadb.HttpClient(host=opts.get("host","localhost"), port=opts.get("port",8000))
else:
    client = chromadb.EphemeralClient()
col = client.get_or_create_collection("{col_name}")
cap_return({{"name": col.name, "count": col.count()}})
"#);
            run_vec(&code, span)
        }
        "chroma_add" => {
            // chroma_add(collection_name, ids, embeddings, documents?, metadatas?, opts?)
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let col_name = args.remove(0).as_str(span)?.to_string();
            let ids_json = v2j(&args.remove(0), span)?;
            let embs_json = v2j(&args.remove(0), span)?;
            let docs_json = if !args.is_empty() { v2j(&args.remove(0), span)? } else { "null".into() };
            let meta_json = if !args.is_empty() { v2j(&args.remove(0), span)? } else { "null".into() };
            let opts_json = if !args.is_empty() { v2j(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import chromadb, json
opts = json.loads('''{opts_json}''')
ids  = json.loads('''{ids_json}''')
embs = json.loads('''{embs_json}''')
docs = json.loads('''{docs_json}''')
meta = json.loads('''{meta_json}''')
if "path" in opts:
    client = chromadb.PersistentClient(path=opts["path"])
elif "host" in opts:
    client = chromadb.HttpClient(host=opts.get("host","localhost"), port=opts.get("port",8000))
else:
    client = chromadb.EphemeralClient()
col = client.get_or_create_collection("{col_name}")
kwargs = {{"ids": ids, "embeddings": embs}}
if docs is not None: kwargs["documents"] = docs
if meta is not None: kwargs["metadatas"] = meta
col.upsert(**kwargs)
cap_return({{"ok": True, "count": col.count()}})
"#);
            run_vec(&code, span)
        }
        "chroma_query" => {
            // chroma_query(collection_name, query_embeddings, n_results?, opts?)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let col_name = args.remove(0).as_str(span)?.to_string();
            let embs_json = v2j(&args.remove(0), span)?;
            let n = if !args.is_empty() { match args.remove(0) { Value::Int(n) => n, _ => 10 } } else { 10 };
            let opts_json = if !args.is_empty() { v2j(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import chromadb, json
opts = json.loads('''{opts_json}''')
embs = json.loads('''{embs_json}''')
if not isinstance(embs[0], list): embs = [embs]
if "path" in opts:
    client = chromadb.PersistentClient(path=opts["path"])
elif "host" in opts:
    client = chromadb.HttpClient(host=opts.get("host","localhost"), port=opts.get("port",8000))
else:
    client = chromadb.EphemeralClient()
col = client.get_or_create_collection("{col_name}")
include = opts.get("include", ["documents", "metadatas", "distances"])
results = col.query(query_embeddings=embs, n_results={n}, include=include)
# Flatten to list of result objects
out = []
for i, ids in enumerate(results["ids"]):
    for j, rid in enumerate(ids):
        item = {{"id": rid}}
        if "distances" in results and results["distances"]: item["distance"] = results["distances"][i][j]
        if "documents" in results and results["documents"]: item["document"] = results["documents"][i][j]
        if "metadatas" in results and results["metadatas"]: item["metadata"] = results["metadatas"][i][j]
        out.append(item)
cap_return(out)
"#);
            run_vec(&code, span)
        }
        "chroma_get" => {
            // chroma_get(collection_name, ids_list, opts?)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let col_name = args.remove(0).as_str(span)?.to_string();
            let ids_json = v2j(&args.remove(0), span)?;
            let opts_json = if !args.is_empty() { v2j(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import chromadb, json
opts = json.loads('''{opts_json}''')
ids  = json.loads('''{ids_json}''')
if "path" in opts:
    client = chromadb.PersistentClient(path=opts["path"])
elif "host" in opts:
    client = chromadb.HttpClient(host=opts.get("host","localhost"), port=opts.get("port",8000))
else:
    client = chromadb.EphemeralClient()
col = client.get_or_create_collection("{col_name}")
res = col.get(ids=ids, include=["documents","metadatas","embeddings"])
out = []
for i, rid in enumerate(res["ids"]):
    item = {{"id": rid}}
    if res.get("documents"): item["document"] = res["documents"][i]
    if res.get("metadatas"): item["metadata"] = res["metadatas"][i]
    out.append(item)
cap_return(out)
"#);
            run_vec(&code, span)
        }
        "chroma_delete" => {
            // chroma_delete(collection_name, ids_list, opts?)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let col_name = args.remove(0).as_str(span)?.to_string();
            let ids_json = v2j(&args.remove(0), span)?;
            let opts_json = if !args.is_empty() { v2j(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import chromadb, json
opts = json.loads('''{opts_json}''')
ids  = json.loads('''{ids_json}''')
if "path" in opts:
    client = chromadb.PersistentClient(path=opts["path"])
elif "host" in opts:
    client = chromadb.HttpClient(host=opts.get("host","localhost"), port=opts.get("port",8000))
else:
    client = chromadb.EphemeralClient()
col = client.get_or_create_collection("{col_name}")
col.delete(ids=ids)
cap_return({{"ok": True, "count": col.count()}})
"#);
            run_vec(&code, span)
        }
        "chroma_count" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let col_name = args.remove(0).as_str(span)?.to_string();
            let opts_json = if !args.is_empty() { v2j(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import chromadb, json
opts = json.loads('''{opts_json}''')
if "path" in opts:
    client = chromadb.PersistentClient(path=opts["path"])
else:
    client = chromadb.EphemeralClient()
col = client.get_or_create_collection("{col_name}")
cap_return(col.count())
"#);
            run_vec(&code, span)
        }
        "chroma_list_collections" => {
            let opts_json = if !args.is_empty() { v2j(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import chromadb, json
opts = json.loads('''{opts_json}''')
if "path" in opts:
    client = chromadb.PersistentClient(path=opts["path"])
else:
    client = chromadb.EphemeralClient()
cap_return([c.name for c in client.list_collections()])
"#);
            run_vec(&code, span)
        }
        "chroma_delete_collection" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let col_name = args.remove(0).as_str(span)?.to_string();
            let opts_json = if !args.is_empty() { v2j(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import chromadb, json
opts = json.loads('''{opts_json}''')
if "path" in opts:
    client = chromadb.PersistentClient(path=opts["path"])
else:
    client = chromadb.EphemeralClient()
client.delete_collection("{col_name}")
cap_return({{"ok": True}})
"#);
            run_vec(&code, span)
        }

        // ── Pinecone ──────────────────────────────────────────────────────────
        "pinecone_init" => {
            // pinecone_init(api_key, index_name) → {ok, dimension, metric}
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let api_key = args.remove(0).as_str(span)?.to_string();
            let index   = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
from pinecone import Pinecone
pc = Pinecone(api_key="{api_key}")
idx = pc.Index("{index}")
stats = idx.describe_index_stats()
cap_return({{"ok": True, "index": "{index}",
             "dimension": stats.get("dimension", 0),
             "total_vectors": stats.get("total_vector_count", 0)}})
"#);
            run_vec(&code, span)
        }
        "pinecone_upsert" => {
            // pinecone_upsert(api_key, index_name, vectors_list)
            // vectors_list: [{"id": str, "values": [...], "metadata": {...}}, ...]
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let api_key  = args.remove(0).as_str(span)?.to_string();
            let index    = args.remove(0).as_str(span)?.to_string();
            let vecs_json = v2j(&args[0], span)?;
            let code = format!(r#"
import json
from pinecone import Pinecone
pc = Pinecone(api_key="{api_key}")
idx = pc.Index("{index}")
vectors = json.loads('''{vecs_json}''')
idx.upsert(vectors=[(v["id"], v["values"], v.get("metadata", {{}})) for v in vectors])
cap_return({{"ok": True, "upserted": len(vectors)}})
"#);
            run_vec(&code, span)
        }
        "pinecone_query" => {
            // pinecone_query(api_key, index_name, vector, top_k?, opts?)
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let api_key   = args.remove(0).as_str(span)?.to_string();
            let index     = args.remove(0).as_str(span)?.to_string();
            let vec_json  = v2j(&args.remove(0), span)?;
            let top_k = if !args.is_empty() { match args.remove(0) { Value::Int(n) => n, _ => 10 } } else { 10 };
            let opts_json = if !args.is_empty() { v2j(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import json
from pinecone import Pinecone
pc = Pinecone(api_key="{api_key}")
idx = pc.Index("{index}")
vec  = json.loads('''{vec_json}''')
opts = json.loads('''{opts_json}''')
res  = idx.query(vector=vec, top_k={top_k}, include_metadata=True, **{{k:v for k,v in opts.items() if k != "namespace"}})
matches = [{{
    "id": m["id"],
    "score": m["score"],
    "metadata": m.get("metadata", {{}})
}} for m in res["matches"]]
cap_return(matches)
"#);
            run_vec(&code, span)
        }
        "pinecone_delete" => {
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let api_key  = args.remove(0).as_str(span)?.to_string();
            let index    = args.remove(0).as_str(span)?.to_string();
            let ids_json = v2j(&args[0], span)?;
            let code = format!(r#"
import json
from pinecone import Pinecone
pc = Pinecone(api_key="{api_key}")
idx = pc.Index("{index}")
ids = json.loads('''{ids_json}''')
idx.delete(ids=ids)
cap_return({{"ok": True}})
"#);
            run_vec(&code, span)
        }
        "pinecone_fetch" => {
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let api_key  = args.remove(0).as_str(span)?.to_string();
            let index    = args.remove(0).as_str(span)?.to_string();
            let ids_json = v2j(&args[0], span)?;
            let code = format!(r#"
import json
from pinecone import Pinecone
pc = Pinecone(api_key="{api_key}")
idx = pc.Index("{index}")
ids = json.loads('''{ids_json}''')
res = idx.fetch(ids=ids)
vectors = {{k: {{"id": k, "values": v["values"], "metadata": v.get("metadata", {{}})}}
            for k, v in res["vectors"].items()}}
cap_return(vectors)
"#);
            run_vec(&code, span)
        }
        "pinecone_describe" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let api_key = args.remove(0).as_str(span)?.to_string();
            let index   = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
from pinecone import Pinecone
pc = Pinecone(api_key="{api_key}")
idx = pc.Index("{index}")
stats = idx.describe_index_stats()
cap_return({{"dimension": stats.get("dimension", 0),
             "total_vectors": stats.get("total_vector_count", 0),
             "namespaces": list(stats.get("namespaces", {{}}).keys())}})
"#);
            run_vec(&code, span)
        }

        // ── Pure math vector ops (native Rust) ────────────────────────────────
        "vec_cosine_sim" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let a = to_f64_vec(&args[0], span)?;
            let b = to_f64_vec(&args[1], span)?;
            let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
            let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
            let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
            if na == 0.0 || nb == 0.0 { return Ok(Value::Float(0.0)); }
            Ok(Value::Float(dot / (na * nb)))
        }
        "vec_dot" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let a = to_f64_vec(&args[0], span)?;
            let b = to_f64_vec(&args[1], span)?;
            Ok(Value::Float(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()))
        }
        "vec_norm" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let a = to_f64_vec(&args[0], span)?;
            Ok(Value::Float(a.iter().map(|x| x * x).sum::<f64>().sqrt()))
        }
        _ => Err(CapError::Runtime { message: format!("unknown vector builtin: {name}"), span: span.clone() }),
    }
}

fn to_f64_vec(v: &Value, span: &Span) -> Result<Vec<f64>, CapError> {
    match v {
        Value::List(l) => l.borrow().iter().map(|v| match v {
            Value::Float(f) => Ok(*f),
            Value::Int(n)   => Ok(*n as f64),
            _ => Err(CapError::TypeError { expected: "number", got: v.type_name().to_string(), span: span.clone() }),
        }).collect(),
        _ => Err(CapError::TypeError { expected: "list", got: v.type_name().to_string(), span: span.clone() }),
    }
}
