# CAP — Quick Start Guide

CAP is a scripting language built for agents. No blocks, no semicolons, no boilerplate — just expressions and pipelines.

---

## 1. Install & Run

```bash
# One-shot install (installs Rust if needed, builds cap, adds to PATH)
./install.sh

# After install:
cap my_script.cap         # run a script
cap repl                  # interactive prompt
cap check my_script.cap   # syntax check only
cap ast my_script.cap     # print AST
```

---

## 2. Variables

```
x = 42
name = "Alice"
active = true
nothing = null
```

No `let`, `var`, `const`. Just `name = value`.

---

## 3. Strings

Double-quoted. Embed any expression with `{...}`:

```
greeting = "Hello, {name}! Next year you'll be {age + 1}."
```

Escape: `\"` `\\` `\n` `\t`. Literal braces: `{{` and `}}`.

---

## 4. Functions

Functions are lambdas assigned to variables:

```
double  = |x| x * 2
add     = |a, b| a + b
greet   = |name| "Hello, {name}!"
zero    = || 42          # no-arg lambda
```

Call them like any function:

```
double(5)       # => 10
add(3, 4)       # => 7
```

Closures capture the enclosing scope:

```
make_adder = |n| |x| x + n
add5 = make_adder(5)
add5(3)    # => 8
```

---

## 5. Pipelines

`|>` passes the left value as the **first argument** of the right call:

```
result = [1, 2, 3, 4, 5]
  |> filter(|x| x > 2)
  |> map(|x| x * 10)
  |> sum
# => 120
```

Wrap in `()` for multi-line:

```
result = (
  read("data.txt")
  |> lines
  |> filter(|l| l != "")
  |> map(trim)
  |> sort
)
```

---

## 6. Control Flow

`if` is an expression — always requires `then` and `else`. Both single-line and multi-line styles work:

```
# Single-line
label = if score >= 90 then "A" elif score >= 80 then "B" else "C"

# Multi-line (newlines allowed between keywords)
label = if score >= 90
  then "A"
  elif score >= 80
  then "B"
  else "C"
```

`match` on a value with comma-separated arms:

```
msg = match code, 0 -> "ok", 1 -> "warn", _ -> "error"
```

Multi-line match (wrap in parens):

```
result = match status,
  200 -> "success",
  404 -> "not found",
  _ -> "server error"
```

---

## 6b. Loops

**`while` loop:**
```
i = 0
total = 0
while i < 5 do
  total = total + i
  i = i + 1
end
println(total)   # => 10
```

**`for` loop:**
```
# Over a list
for name in ["Alice", "Bob", "Charlie"] do
  println("Hello, {name}!")
end

# Over a range
squares = []
for x in 1..=5 do
  append(squares, x * x)
end
# squares => [1, 4, 9, 16, 25]
```

---

## 7. Collections

**List:**
```
nums = [1, 2, 3]
nums[0]      # => 1
nums[-1]     # => 3  (negative index)
nums.len     # => 3
```

**Map:**
```
user = {"name": "Alice", "age": 30}
user["name"]   # => "Alice"
user.age       # => 30  (dot shorthand)
user["email"] = "alice@example.com"   # mutate
```

**Tuple** (immutable):
```
point = (3, 4)
point[0]   # => 3
```

**Range:**
```
1..5    # [1, 2, 3, 4]     exclusive
1..=5   # [1, 2, 3, 4, 5]  inclusive
```

---

## 8. List Operations

```
nums = [1, 2, 3, 4, 5]

nums |> map(|x| x * 2)          # [2, 4, 6, 8, 10]
nums |> filter(|x| x > 2)       # [3, 4, 5]
nums |> reduce(|a, b| a + b)    # 15
nums |> each(println)            # prints each, returns null
nums |> sort                     # [1, 2, 3, 4, 5]
nums |> reverse                  # [5, 4, 3, 2, 1]
nums |> sum                      # 15
nums |> min                      # 1
nums |> max                      # 5
nums |> first                    # 1
nums |> last                     # 5
nums |> enumerate                # [(0,1),(1,2),(2,3)...]
nums |> any(|x| x > 4)          # true
nums |> all(|x| x > 0)          # true
nums |> find(|x| x > 3)         # 4
zip([1,2], ["a","b"])            # [(1,"a"),(2,"b")]
flatten([[1,2],[3,4]])           # [1,2,3,4]
```

---

## 9. String Operations

```
s = "  hello world  "

s.trim               # "hello world"
s.upper              # "  HELLO WORLD  "
s.lower              # "  hello world  "
s.len                # 15
s.lines              # list of lines
s.chars              # list of chars

split(s, " ")        # ["", "", "hello", "world", "", ""]
join(["a","b"], ",") # "a,b"
replace(s, "o", "0") # "  hell0 w0rld  "
contains(s, "hello") # true
starts_with(s, " ")  # true
```

All string functions also work as methods: `s.split(" ")` = `split(s, " ")`.

---

## 10. Null Safety

`??` returns the left value unless it's `null`, then the right:

```
port  = config["port"] ?? 8080
name  = input("Name: ") ?? "anonymous"
value = map["key"] ?? "default"
```

---

## 11. OOP (Classes)

`class` desugars to a constructor lambda returning a map of closures:

```
class Point(x, y),
  dist = || (x*x + y*y) ** 0.5,
  add  = |other| Point(x + other["x"], y + other["y"]),
  str  = || "({x}, {y})"

p = Point(3, 4)
p.dist()              # => 5.0   (dot syntax)
p.str()               # => "(3, 4)"
p["dist"]()           # also works
```

Methods access constructor args via closure — no `self` needed.

**Inheritance** with `extends`:

```
class Animal(name), speak = || "{name} says ..."
class Dog(name) extends Animal(name),
  speak = || "{name} says woof",   # override
  fetch = || "{name} fetches"      # new method

d = Dog("Rex")
d.speak()   # => "Rex says woof"
d.fetch()   # => "Rex fetches"
```

---

## 12. File I/O

```
content = read("data.txt")
write("output.txt", "hello\n")
file_append("log.txt", "new line\n")
exists("file.txt")    # true/false
ls()                  # list current directory
ls("/tmp")            # list given path
line = input("Enter: ")   # read from stdin
```

---

## 12b. HTTP (`http_*`)

All HTTP functions return `{status: int, body: str, headers: map}`.

```
r = http_get("https://api.example.com/data")
r = http_get("https://api.example.com/data", {"Authorization": "Bearer token"})

r = http_post("https://api.example.com/items", json_stringify({"name": "Alice"}),
              {"Content-Type": "application/json"})

r = http_put("https://api.example.com/items/1", body)
r = http_delete("https://api.example.com/items/1")
r = http_request("PATCH", url, body, headers)  # any method

# Check response
if r["status"] == 200 then json_parse(r["body"]) else error(r["body"])
```

---

## 12c. Regex (`regex_*`)

```
regex_match("^\\d+$", "42")                    # true — full-string match
regex_find("\\d+", "foo 42 bar")               # "42" — first match or null
regex_find_all("\\d+", "a1 b2 c3")             # ["1", "2", "3"]
regex_replace("\\s+", "_", "hello world")      # "hello_world"
```

---

## 13. Core Builtins

```
len([1,2,3])     # 3
type(42)         # "int"
str(42)          # "42"
int("42")        # 42
float("3.14")    # 3.14
bool(0)          # false
repr([1,2])      # "[1, 2]"
range(5)         # [0,1,2,3,4]
range(2, 6)      # [2,3,4,5]
range(0, 10, 2)  # [0,2,4,6,8]
keys(map)        # list of keys
values(map)      # list of values
items(map)       # list of (key, value) tuples
error("oops")    # stop execution with message
```

---

## 14. Advanced Features

### Sequential blocks (`do...end`)

```
result = do
  a = step_one()
  b = step_two(a)
  b * 2
end
```

### Error handling (`try`)

```
r = try(|| read("maybe_missing.txt"))
content = if r["ok"] then r["value"] else "default"
```

### Pattern guards

```
label = match score,
  n if n >= 90 -> "A",
  n if n >= 80 -> "B",
  n if n >= 70 -> "C",
  _ -> "F"
```

### Map utilities

```
m = from_pairs([("x", 1), ("y", 2)])   # build from pairs
set(m, "z", 3)                          # mutate in-place
merged = merge(defaults, overrides)     # merge maps
```

---

## 14b. Import / Module System

Split code across files with `import`. It evaluates a `.cap` file in an
isolated scope and returns all its top-level bindings as a map.

**math_utils.cap:**
```
double = |x| x * 2
PI     = 3.14159
circle_area = |r| PI * r * r
```

**main.cap:**
```
u = import("math_utils.cap")
println(u.double(5))          # => 10
println(u.circle_area(3.0))   # => 28.274...
```

---

## 15. CSV

```
rows  = csv_read("data.csv")            # list of maps, types auto-inferred
rows  = csv_read("data.tsv", "\t")      # TSV
raw   = csv_read_raw("data.csv")        # list of lists
rows  = csv_parse("a,b\n1,2\n3,4")     # from string
csv_write("out.csv", rows)              # write list-of-maps
```

---

## 16. DataFrames (pandas)

```
df = df_read("sales.csv")              # also supports .json, .xlsx
df_shape(df)                           # {rows: 1000, cols: 8}
df_columns(df)                         # ["name", "age", "city"]
df_head(df, 10)                        # first 10 rows
df_describe(df)                        # per-column stats
top = df_filter(df, "revenue > 10000") # pandas query syntax
by_region = df_groupby(top, "region", "mean")
sorted = df_sort(df, "score", true)    # desc=true
subset = df_select(df, ["name", "score"])
df_write(by_region, "summary.xlsx")
```

Requires: `pip install pandas openpyxl`

---

## 17. Plots (matplotlib)

```
# Plots save to /tmp/*.png and return the path
path = plt_line(x, y, {"title": "Growth", "xlabel": "Month"})
path = plt_bar(["Q1","Q2","Q3"], [10,20,15], {"color": "steelblue"})
path = plt_scatter(x, y, {"color": "red"})
path = plt_hist(data, {"bins": 30})
path = plt_heatmap(matrix, {"annot": true})
path = plt_pie(values, labels)
plt_show(path)              # open in system viewer

# Custom save path:
plt_line(x, y, {"save": "chart.png"})
```

Requires: `pip install matplotlib seaborn`

---

## 18. PyTorch

```
device = torch_device()    # "cuda", "mps", or "cpu"

# Train a simple neural network
result = torch_train_linear(X, y, {"lr": 0.01, "epochs": 200, "hidden": 32})
result["losses"][-1]["loss"]   # final loss

# Inference
preds = torch_predict(result["model_state"], X_test)

# Save/load
torch_save(result["model_state"], "model.pt")
state = torch_load("model.pt")
```

Requires: `pip install torch`

---

## 19. YAML & TOML

```
# YAML
config = yaml_parse("name: Alice\nage: 30\n")
yaml_stringify(config)           # back to YAML string

# TOML
cfg = toml_parse("[db]\nhost = \"localhost\"\nport = 5432\n")
cfg["db"]["host"]                # => "localhost"
toml_stringify(cfg)              # back to TOML string
```

---

## 20. File System & OS (`fs_*` / `os_*`)

```
# File ops
fs_read("data.txt")              # read file → str
fs_write("out.txt", "hello\n")  # write file
fs_append("log.txt", "line\n")  # append
fs_delete("tmp.txt")             # delete file
fs_exists("file.txt")            # → bool
fs_is_file("data.txt")           # → bool
fs_is_dir("src")                 # → bool
fs_mkdir("newdir")               # create dir
fs_mkdir_all("a/b/c")            # create with parents
fs_rmdir("olddir")               # remove dir tree
fs_copy("src.txt", "dst.txt")    # copy file
fs_move("old.txt", "new.txt")    # rename/move
fs_ls()                          # list cwd
fs_ls("/tmp")                    # list given path
fs_stat("file.txt")              # {size, is_file, is_dir, modified, readonly}

# OS / path
os_cwd()                         # current working directory
os_chdir("/tmp")                 # change directory
os_hostname()                    # machine hostname
os_username()                    # current user
os_pid()                         # process ID
os_home()                        # home directory
os_sep()                         # path separator ("/" on Unix)
os_path_join("/usr", "local", "bin")  # => "/usr/local/bin"
os_path_basename("/usr/local/bin")    # => "bin"
os_path_dirname("/usr/local/bin")     # => "/usr/local"
os_path_ext("data.csv")               # => "csv"
os_abs("relative/path")               # absolute path
```

---

## 21. Time & Date (`time_*`)

```
now = time_now()                 # current local time as {unix, tz}
utc = time_now_utc()             # UTC time
t   = time_unix(1700000000)      # from unix timestamp

time_format(now, "%Y-%m-%d %H:%M:%S")   # => "2024-11-14 22:13:20"
t2  = time_parse("2024-01-15", "%Y-%m-%d")
t3  = time_add(now, 3600)        # add 1 hour (seconds)
diff = time_diff(t3, now)        # => 3600

time_year(now)     # => 2024
time_month(now)    # => 11
time_day(now)      # => 14
time_hour(now)     # => 22
time_minute(now)   # => 13
time_second(now)   # => 20
time_weekday(now)  # => "Thursday"
time_sleep(1.5)    # sleep 1.5 seconds
```

---

## 22. SQLite (`sql_*`)

```
sql_open("mydb.sqlite")          # open (or create) database
sql_open(":memory:")             # in-memory database

# DDL / DML
sql_exec("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INT)")
sql_exec("INSERT INTO users (name, age) VALUES (?, ?)", ["Alice", 30])

# Queries — return list of maps
rows = sql_query("SELECT * FROM users WHERE age > ?", [25])
rows[0]["name"]   # => "Alice"

# Single row
row = sql_query_one("SELECT * FROM users WHERE id = ?", [1])

# Transactions
sql_begin()
sql_exec("UPDATE users SET age = age + 1")
sql_commit()
# sql_rollback()

# Schema inspection
sql_tables()                     # => ["users", ...]
sql_schema("users")              # => [{name, type, notnull, pk}, ...]

sql_close()
```

---

## 23. Streaming I/O & Gzip (`stream_*` / `gz_*`)

```
# Buffered line reading (memory-efficient for large files)
lines = stream_lines("large.log")    # list of lines
bytes = stream_bytes("data.bin")     # list of byte ints

stream_write("out.txt", "hello\n")   # write
stream_append("log.txt", "more\n")   # append

# Gzip
gz_write("data.gz", content)         # compress and write
content = gz_read("data.gz")         # decompress and read
compressed = gz_compress("hello!")   # → base64 string
original   = gz_decompress(compressed)
```

---

## 24. Apache Arrow (`arrow_*`)

```
# Requires: pip install pyarrow pandas

rows = [{"name": "Alice", "age": 30}, {"name": "Bob", "age": 25}]
tbl  = arrow_from_list(rows)

arrow_schema(tbl)                # => {"name": "string", "age": "int64"}
arrow_to_list(tbl)               # => list of dicts

# Read/write Parquet / CSV
tbl = arrow_from_parquet("data.parquet")
arrow_to_parquet(tbl, "out.parquet")
tbl = arrow_from_csv("data.csv")
arrow_to_csv(tbl, "out.csv")

# Operations
filtered = arrow_filter(tbl, "age > 26")      # pandas query syntax
selected = arrow_select(tbl, ["name"])
sorted   = arrow_sort(tbl, "age", true)       # desc=true
total    = arrow_aggregate(tbl, "age", "sum") # => 55
tbl2     = arrow_cast(tbl, "age", "float64")
```

---

## 25. Task Utilities (`task_*`)

```
task_sleep(1.5)                  # sleep N seconds

# Retry a shell command up to N times
r = task_retry(3, "curl -f https://api.example.com/ping")
if r["ok"] then println("done in {r[\"attempts\"]} attempts")

# Timeout a shell command
r = task_timeout(5.0, "sleep 10")
r["timed_out"]   # => true

# Run shell commands in parallel → list of {status, stdout, stderr}
results = task_par_shell(["ls /tmp", "echo hello", "date"])

# Measure elapsed time
r = task_measure("python -c 'sum(range(10**6))'")
println("took {r[\"elapsed_ms\"]}ms")
```

---

## 26. Native FFI via ctypes (`ffi_*`)

```
# Requires: Python with ctypes (built-in)

ffi_load("/usr/lib/libm.so.6")   # {ok: bool, path: str}
ffi_sizeof("double")             # => 8

# Call a C function: ffi_call(lib, fn, ret_type, [arg_types], [args])
result = ffi_call("/usr/lib/libm.dylib", "sqrt", "double",
                  ["double"], [9.0])  # => 3.0

# Get struct layout
ffi_struct({"x": "float", "y": "float"})  # => {size: 8, fields: {...}}

# Encode array as base64 bytes
ffi_array("int", [1, 2, 3, 4])
```

---

## 27. Distributed Compute (`cluster_*`)

```
# Requires: pip install ray  (or dask for cluster_dask_*)

cluster_info()   # => {ray: bool, dask: bool, cpus: 8}

# Parallel map (uses Ray if available, multiprocessing fallback)
result = cluster_map("lambda x: x * x", [1, 2, 3, 4, 5])

# Map-reduce
total = cluster_map_reduce("lambda x: x * 2", "lambda a, b: a + b", [1,2,3])

# Dask large-file CSV ops
tbl = cluster_dask_read("huge.csv")          # {columns, num_rows, _data}
by_city = cluster_dask_groupby("huge.csv", "city", "revenue", "sum")
```

---

## 28. WebAssembly (`wasm_*`)

```
# Requires: pip install wasmtime

wasm_exports("module.wasm")         # list of exported function names
wasm_load("module.wasm")            # {ok: bool, exports: [...]}
result = wasm_call("module.wasm", "add", [3, 4])   # => 7
bytes  = wasm_memory_read("module.wasm", 0, 16)    # raw bytes as int list
```

---

## 29. LLM Inference (`ollama_*`, `openai_*`, `anthropic_*`, `gemini_*`)

```
# Build messages with helpers
msgs = [llm_system("You are a helpful assistant"), llm_user("What is 2+2?")]

# Ollama (local — no API key needed)
reply = ollama_chat("llama3.2", msgs)
println(reply["content"])          # => "4"

text = ollama_complete("llama3.2", "The sky is")
emb  = ollama_embed("nomic-embed-text", "hello world")
ollama_list()                      # list pulled models
ollama_pull("llama3.2")            # pull a model

# Generic aliases (default to Ollama chat/complete/embed)
reply = llm_chat("llama3.2", msgs)
text  = llm_complete("llama3.2", "The sky is")
emb   = llm_embed("nomic-embed-text", "hello world")

# OpenAI
reply = openai_chat("gpt-4o-mini", msgs)           # reads OPENAI_API_KEY
emb   = openai_embed("text-embedding-3-small", "hello")

# Anthropic
reply = anthropic_chat("claude-haiku-4-5-20251001", msgs)  # reads ANTHROPIC_API_KEY

# Gemini
reply = gemini_chat("gemini-1.5-flash", msgs)      # reads GEMINI_API_KEY
```

All replies: `{content: str, model: str, tokens: {in: int, out: int}}`

No pip dependencies for basic usage — uses Python's built-in `urllib`.

---

## 30. Crypto & Randomness

No pip dependencies — uses Python's built-in `hashlib`, `hmac`, `secrets`.

```
hash_sha256("hello")          # => hex string
hash_md5("data")
hash_file("big.bin")          # SHA-256 of file content

hmac_sha256("secret", "msg")  # => hex HMAC

b64_encode("hello")           # => "aGVsbG8="
b64_decode("aGVsbG8=")        # => "hello"
hex_encode("hi") / hex_decode("6869")

uuid_v4()                     # random UUID string
rand_int(1, 100)              # random int in [1, 100)
rand_float()                  # random float [0, 1)
rand_choice(["a","b","c"])    # random element
rand_shuffle([1,2,3,4,5])     # shuffled copy

h = pbkdf2_hash("mypassword")                          # {hash: str, salt: str}
pbkdf2_verify("mypassword", h["hash"], h["salt"])      # => true
```

---

## 31. Vector Databases (`chroma_*`, `pinecone_*`, `vec_*`)

```
# Requires: pip install chromadb  (for chroma_*)

# Store embeddings
docs = ["The sky is blue", "Rust is fast"]
embs = docs |> map(|d| ollama_embed("nomic-embed-text", d))
chroma_add("my_index", ["0","1"], embs, docs, {"path": "./chroma_db"})

# Query
q_emb   = ollama_embed("nomic-embed-text", "What color is the sky?")
results = chroma_query("my_index", q_emb, 1, {"path": "./chroma_db"})
results[0]["document"]    # => "The sky is blue"

# Native Rust vector math (no pip)
vec_cosine_sim(emb1, emb2)   # similarity score
vec_dot(a, b)
vec_norm(v)
```

---

## 32. Image Processing (`img_*`)

All image functions work with **file paths**, not image objects. Transform functions
write a new file and return the output path.

```
# Requires: pip install Pillow

# Metadata (take one path, return info)
img_open("photo.jpg")              # → {path, width, height, mode, format}
img_info("photo.jpg")              # → {path, width, height, mode, format, size_bytes}
img_size("photo.jpg")              # → {width: int, height: int}
img_mode("photo.jpg")              # → "RGB" / "RGBA" / "L" etc.
img_pixels("photo.jpg")            # → list of [r,g,b] per pixel
img_to_array("photo.jpg")          # → nested list H×W×C (needs numpy)

# Transforms: img_*(src, dst, ...) → dst path
img_save("photo.jpg", "copy.png")
img_resize("in.jpg", "out.jpg", 320, 240)
img_crop("in.jpg", "out.jpg", 0, 0, 100, 100)   # left top right bottom
img_rotate("in.jpg", "out.jpg", 90)
img_flip("in.jpg", "out.jpg", "horizontal")       # or "vertical"
img_grayscale("in.jpg", "out.jpg")
img_blur("in.jpg", "out.jpg", 2.0)               # radius
img_sharpen("in.jpg", "out.jpg")
img_brightness("in.jpg", "out.jpg", 1.5)         # 1.0 = original
img_contrast("in.jpg", "out.jpg", 1.2)
img_thumbnail("in.jpg", "out.jpg", 128)           # max dimension
img_convert("in.jpg", "out.jpg", "RGB")
img_draw_text("in.jpg", "out.jpg", "Hello", 10, 10, {"font_size": 24, "color": "white"})
img_paste("base.jpg", "overlay.png", "out.jpg", 50, 50)   # paste at (x,y)
img_from_array(pixel_list, "out.png", "RGB")      # create from array (needs numpy)

img_show("photo.jpg")              # open in system viewer
```

---

## 33. Scikit-Learn (`sklearn_*`)

```
# Requires: pip install scikit-learn

X = [[1.0],[2.0],[3.0],[4.0]]
y = [0, 0, 1, 1]

model = sklearn_train(X, y, {"model": "random_forest", "n_estimators": 100})
# model names: "linear", "logistic", "ridge", "lasso", "svm", "svr",
#              "decision_tree", "random_forest", "gradient_boosting", "knn"

preds = sklearn_predict(model, [[1.5],[3.5]])
score = sklearn_score(model, X_test, y_test)

# Evaluation
metrics = sklearn_metrics(y_true, y_pred)      # {accuracy, precision, recall, f1, ...}
fi = sklearn_feature_importance(model)          # list of {feature, importance}

# Preprocessing
X_scaled  = sklearn_scale(X)
X_norm    = sklearn_normalize(X)
y_encoded = sklearn_encode_labels(["cat","dog","cat"])
split     = sklearn_train_test_split(X, y, 0.2)  # → {X_train, X_test, y_train, y_test}

# Clustering
labels = sklearn_kmeans(X, 3)
labels = sklearn_dbscan(X, {"eps": 0.5, "min_samples": 5})

# Save / load
sklearn_save(model, "model.pkl")
model = sklearn_load("model.pkl")

# Cross-validation and tuning
cv   = sklearn_cross_val({"model": "svm"}, X, y, {"cv": 5})
best = sklearn_grid_search(param_grid, X, y)   # param_grid: {"C": [0.1,1,10], ...}
```

---

## 34. PDF (`pdf_*`)

```
# Requires: pip install pdfplumber pypdf reportlab

pdf_pages("doc.pdf")               # page count
pdf_text("doc.pdf")                # full text of all pages
pdf_page_text("doc.pdf", 0)        # text of page 0
pdf_tables("doc.pdf", 0)           # list of tables on page 0 (each table is list of rows)
pdf_metadata("doc.pdf")            # {title, author, creator, ...}
pdf_images("doc.pdf", 0)           # list of image info maps on page 0

# Create / merge
pdf_create("out.pdf", ["Page one text", "Page two text"])
pdf_merge(["a.pdf","b.pdf"], "combined.pdf")
```

---

## 35. ZIP / TAR Archives

```
# No pip — uses Python's built-in zipfile / tarfile

zip_list("archive.zip")                            # [{name, size, compressed_size, is_dir}]
zip_extract("archive.zip", "readme.txt", "./out")  # extract one file
zip_extract_all("archive.zip", "./out")            # extract all
zip_read_entry("archive.zip", "readme.txt")        # read entry as string, no extraction
zip_create("new.zip", ["file1.txt","dir/"])
zip_add("existing.zip", "extra.txt")

tar_list("archive.tar.gz")                         # [{name, size}]
tar_extract("archive.tar.gz", "file.txt", "./out") # extract one file
tar_extract_all("archive.tar.gz", "./out")
tar_create("new.tar.gz", ["file1.txt","dir/"])
```

---

## 36. HTTP Server (`server_*`)

```
# Requires: pip install flask

# Receive one HTTP request then exit
println("Waiting on :8080...")
req = server_serve_once(8080, 30)    # port, timeout_seconds
data = json_parse(req["body"])
println("Got: {data}")

# Background server for multiple requests
server_start(8080)
req = server_poll(30)
server_stop()
```

---

## 37. Python bridge (`pyval`)

Run arbitrary Python and get a structured value back:

```
pi     = pyval("import math; cap_return(math.pi)")
result = pyval("""
import numpy as np
arr = np.array([1, 2, 3, 4, 5])
cap_return({'mean': float(arr.mean()), 'std': float(arr.std())})
""")
println("mean={result[\"mean\"]}, std={result[\"std\"]}")
```

---

## 38. Common Patterns

**Process a file:**
```
read("data.txt") |> lines |> filter(|l| l != "") |> map(trim) |> each(println)
```

**Filter and transform data:**
```
users = [{"name": "Alice", "score": 95}, {"name": "Bob", "score": 72}]
top = users |> filter(|u| u["score"] >= 80) |> map(|u| u["name"]) |> sort
```

**Recursive function:**
```
fib = |n| if n <= 1 then n else fib(n-1) + fib(n-2)
fib(10)   # => 55
```

**Compose functions:**
```
pipe = |f, g| |x| g(f(x))
clean = pipe(trim, lower)
clean("  HELLO  ")   # => "hello"
```

**Safe map access:**
```
val = data["key"] ?? "fallback"
```

**Debug a pipeline with `tap`:**
```
result = data
  |> tap(|xs| println("before filter: {xs.len} items"))
  |> filter(|x| x > 0)
  |> tap(|xs| println("after filter: {xs.len} items"))
```

---

## 39. Rules You Must Know (avoid common mistakes)

**Multi-line expressions need `()`:**
```
# ✗ WRONG — newline terminates the statement
result = [1,2,3]
  |> map(double)

# ✓ CORRECT
result = ([1,2,3]
  |> map(double))
```

**`if` always needs `then` and `else` (both styles work):**
```
# ✗ WRONG — missing else
if x > 0 then "positive"

# ✓ CORRECT — single line
if x > 0 then "positive" else "non-positive"

# ✓ CORRECT — multi-line (newlines between keywords are fine)
if x > 0
  then "positive"
  else "non-positive"
```

**Class methods can't call each other:**
```
class Foo(x),
  double = || x * 2,
  # quad = || double()    # ✗ WRONG: double is not a variable
  quad   = || x * 4       # ✓ inline the logic
```

**Constructor args need getter methods to be read externally:**
```
class Item(name, price),
  get_name  = || name,    # exposes name for callers
  get_price = || price

item = Item("apple", 1.5)
# item["name"]    ✗ KeyError — name is not a map key
item["get_name"]()  # ✓ => "apple"
```

**String interpolation can't call methods:**
```
class Foo(x),
  double = || x * 2,
  # msg = || "double is {double()}"  ✗ double not in scope
  msg    = || "double is " + str(x * 2)  # ✓ inline it
```

---

## 40. Operators

| Category   | Operators                                |
|------------|------------------------------------------|
| Arithmetic | `+` `-` `*` `/` `%` `**` (power)        |
| Comparison | `==` `!=` `<` `>` `<=` `>=`             |
| Boolean    | `and` `or` `not`                         |
| Pipe       | `\|>`                                    |
| Null-safe  | `??`                                     |
| Range      | `..` (exclusive) `..=` (inclusive)       |
| String/List concat | `+`                              |
