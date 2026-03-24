# Cap

A scripting language for AI agents. No blocks, no semicolons, no boilerplate — just expressions and pipelines.

```
# fetch, transform, store in 3 lines
rows = http_get("https://api.example.com/data")["body"]
  |> json_parse
  |> filter(|r| r["score"] > 80)

csv_write("top_scores.csv", rows)
```

---

## Install

### macOS / Linux (one command)

```bash
git clone https://github.com/GilCaplan/cap
cd cap
./install.sh
```

The installer:
1. Installs Rust via `rustup` (if not already present)
2. Builds `cap` in release mode
3. Copies the binary to `/usr/local/bin` (or `~/.local/bin`)
4. Adds it to your `PATH`

After install, open a new terminal and run:

```bash
cap repl               # interactive prompt
cap my_script.cap      # run a script
```

### Windows

```powershell
# 1. Install Rust from https://rustup.rs
# 2. Clone and build
git clone https://github.com/GilCaplan/cap
cd cap
cargo build --release
# 3. Add target\release\cap.exe to your PATH
```

### From source (manual)

```bash
# Requires Rust 1.70+
cargo build --release
./target/release/cap --help
```

---

## CLI

```bash
cap <file.cap>           # run a script
cap repl                 # interactive REPL
cap check <file.cap>     # syntax check (no execution)
cap ast <file.cap>       # print AST (debug)
```

---

## Language at a Glance

```
# Variables — no let/var/const
x = 42
name = "Alice"
active = true

# Functions are lambdas
double  = |x| x * 2
add     = |a, b| a + b
greet   = |name| "Hello, {name}!"

# Pipelines
result = [1, 2, 3, 4, 5]
  |> filter(|x| x > 2)
  |> map(|x| x * 10)
  |> sum
# => 120

# if is an expression — single-line or multi-line
label = if score >= 90 then "A" else "B"

grade = if score >= 90
  then "A"
  elif score >= 80
  then "B"
  else "C"

# match with pattern guards
msg = match code,
  0 -> "ok",
  n if n > 0 -> "warn",
  _ -> "error"

# Classes
class Point(x, y),
  dist = || (x*x + y*y) ** 0.5,
  str  = || "({x}, {y})"

p = Point(3, 4)
p.dist()   # => 5.0

# do...end for sequential blocks
result = do
  a = fetch_data()
  b = clean(a)
  b |> transform
end
```

---

## Standard Library

Cap has a large built-in stdlib — no imports needed.

| Module | Functions | Requires |
|--------|-----------|---------|
| **Core** | `len`, `range`, `type`, `str`, `int`, `float`, `bool`, `keys`, `values`, `items`, `set`, `merge`, `try`, `error` | — |
| **List** | `map`, `filter`, `reduce`, `each`, `sort`, `zip`, `flatten`, `sum`, `min`, `max`, `first`, `last`, `any`, `all`, `find`, `enumerate`, `reverse`, `append`, `extend`, `tap` | — |
| **String** | `split`, `join`, `trim`, `upper`, `lower`, `replace`, `contains`, `starts_with`, `ends_with`, `lines`, `chars` | — |
| **I/O** | `read`, `write`, `file_append`, `exists`, `ls`, `input` | — |
| **File System** | `fs_read/write/append/delete/exists/mkdir/mkdir_all/rmdir/copy/move/ls/stat/is_file/is_dir`, `os_cwd/chdir/home/pid/hostname/username/sep/path_join/basename/dirname/ext/abs` | — |
| **JSON / YAML / TOML** | `json_parse/stringify`, `yaml_parse/stringify`, `toml_parse/stringify` | — |
| **HTTP** | `http_get`, `http_post`, `http_put`, `http_delete`, `http_request` | — |
| **Regex** | `regex_match`, `regex_find`, `regex_find_all`, `regex_replace` | — |
| **Shell** | `shell`, `shell_lines`, `env`, `env_all` | — |
| **CSV** | `csv_read`, `csv_read_raw`, `csv_write`, `csv_parse` | — |
| **Time** | `time_now`, `time_format`, `time_parse`, `time_add`, `time_diff`, `time_sleep`, `time_year/month/day/hour/minute/second/weekday` | — |
| **SQLite** | `sql_open`, `sql_exec`, `sql_query`, `sql_query_one`, `sql_begin/commit/rollback`, `sql_tables`, `sql_schema` | bundled |
| **Streaming / Gzip** | `stream_lines/bytes/write/append`, `gz_read/write/compress/decompress` | — |
| **ZIP / TAR** | `zip_list/extract/create/add/read_entry`, `tar_list/extract/create` | — |
| **Crypto** | `hash_md5/sha1/sha256/sha512/hash_file`, `hmac_sha256/sha512`, `b64_encode/decode/url_encode/url_decode`, `hex_encode/decode`, `uuid_v4/v5`, `rand_int/float/bytes/choice/shuffle`, `pbkdf2_hash/verify` | — |
| **HTTP Server** | `server_serve_once`, `server_start`, `server_stop`, `server_mock`, `server_poll` | `pip install flask` |
| **LLM** | `llm_chat/complete/embed` (generic), `ollama_chat/complete/embed/list/pull`, `openai_chat/embed`, `anthropic_chat`, `gemini_chat`, `llm_system/user/assistant/messages` | Ollama / API keys |
| **Vector DB** | `chroma_*` (ChromaDB), `pinecone_*` (Pinecone), `vec_cosine_sim/dot/norm` | `pip install chromadb pinecone-client` |
| **DataFrames** | `df_read/write/head/tail/shape/columns/describe/select/filter/sort/groupby/join/drop/rename/fillna/apply` | `pip install pandas openpyxl` |
| **Plots** | `plt_line/bar/scatter/hist/heatmap/boxplot/pie/show` | `pip install matplotlib seaborn` |
| **Image** | `img_open/info/save/size/mode/resize/crop/rotate/flip/grayscale/blur/sharpen/brightness/contrast/thumbnail/convert/pixels/draw_text/paste/from_array/to_array/show` (all path-based) | `pip install Pillow` |
| **PDF** | `pdf_pages/text/page_text/tables/metadata/images/create/merge` | `pip install pdfplumber pypdf reportlab` |
| **Scikit-Learn** | `sklearn_train/predict/score/save/load/scale/normalize/encode_labels/train_test_split/metrics/kmeans/dbscan/feature_importance/cross_val/grid_search` | `pip install scikit-learn` |
| **PyTorch** | `torch_device/tensor/zeros/ones/train_linear/predict/save/load` | `pip install torch` |
| **Arrow / Parquet** | `arrow_from_list/to_list/schema/cast/from_csv/to_csv/from_parquet/to_parquet/filter/select/sort/aggregate` | `pip install pyarrow pandas` |
| **Cluster** | `cluster_map/map_reduce/dask_read/dask_groupby/info` | `pip install ray dask` |
| **FFI** | `ffi_load/call/sizeof/struct/array` | Python ctypes (built-in) |
| **WebAssembly** | `wasm_load/call/exports/memory_read` | `pip install wasmtime` |
| **Python bridge** | `python(code)`, `pyval(code)` — run arbitrary Python | Python 3 |

---

## Python dependencies

Most stdlib modules work with zero deps. Advanced modules need Python packages:

```bash
# Core data science
pip install pandas openpyxl matplotlib seaborn numpy

# ML / AI
pip install scikit-learn torch transformers

# RAG / Vector
pip install chromadb pinecone-client

# Document / image
pip install Pillow pdfplumber pypdf reportlab

# Web / server
pip install flask

# Distributed
pip install pyarrow ray dask

# WebAssembly
pip install wasmtime
```

---

## Examples

```bash
# Run an example
cap programs/maya_report.cap

# REPL session
cap repl
>>> x = [1, 2, 3, 4, 5] |> map(|n| n * n) |> sum
>>> println(x)   # 55
```

**Chat with Ollama:**
```
msgs = [llm_user("What is 2+2?")]
reply = ollama_chat("llama3.2", msgs)
println(reply["content"])
```

**Build a vector search index:**
```
docs = ["The sky is blue", "Rust is fast", "Python is popular"]
embeddings = docs |> map(|d| ollama_embed("nomic-embed-text", d))
chroma_add("my_index", ["0","1","2"], embeddings, docs, opts={"path": "./chroma_db"})

query_emb = ollama_embed("nomic-embed-text", "What color is the sky?")
results = chroma_query("my_index", query_emb, 1, {"path": "./chroma_db"})
println(results[0]["document"])  # => "The sky is blue"
```

**Serve a webhook and process the payload:**
```
println("Waiting for webhook on :8080...")
req = server_serve_once(8080, 30)
data = json_parse(req["body"])
println("Got: {data}")
```

---

## Documentation

- **[QUICKSTART.md](QUICKSTART.md)** — full language reference with examples
- **[LANGUAGE.md](LANGUAGE.md)** — formal grammar and semantics

---

## Requirements

- **Rust 1.70+** (installed automatically by `install.sh`)
- **Python 3.8+** (for Python-backed stdlib modules)
- macOS, Linux, or Windows
