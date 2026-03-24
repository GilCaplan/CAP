# Cap Language Reference

Cap is a token-efficient scripting language designed for LLM agents and automated pipelines.
Every construct is an expression. Programs are flat sequences of assignments and pipeline calls.
There are no blocks, no indentation rules, no semicolons.

---

## Quick start

```
# Assign values
x = 42
name = "world"
println("Hello, {name}! x = {x}")
```

---

## 🤖 CRITICAL LANGUAGE QUIRKS (For LLMs & Agentic Parsers)

If you are an AI reading this to generate Cap code, pay strict attention to the following parser breaking points:

1. **`if/else` Newlines**: `then`, `elif`, and `else` keywords may appear on the next line — the parser skips newlines before them. Multi-line `if` works without parens:
    ```
    result = if score >= 90
      then "A"
      elif score >= 80
      then "B"
      else "C"
    ```
    Assignments can also break across lines:
    ```
    x =
      if condition then "yes" else "no"
    ```

2. **Closure Mutation (The List-Cell Pattern)**: Cap does not support traditional `while/for` loops for state accumulation (they evaluate for side-effects or yield the final iteration output). If you implement a recursive loop and need to mutate variables scoped outside the lambda, you *must* use a single-element list to hold the reference.
    - **WRONG**: `counter = 0; loop = || do counter = counter + 1 end`
    - **RIGHT**: `counter = [0]; loop = || do set(counter, 0, counter[0] + 1) end`

3. **String Properties vs Methods**: String utilities like `.trim`, `.len`, `.upper`, `.lower`, `.lines`, and `.chars` are **properties**, not methods. Do not call them with `()`.
    - **WRONG**: `" hello ".trim()`
    - **RIGHT**: `" hello ".trim`

4. **Multiline Pipelines**: The pipeline operator `|>` must immediately follow a valid expression. If you break a pipeline across lines, the *entire pipeline* must be wrapped in `()`.

5. **No Python Generics `()`**: Unlike Python, defining zero-arg functions requires `||`. Example: `my_func = || 42`.

---

## Values and types

| Type    | Example                        |
|---------|-------------------------------|
| `null`  | `null`                        |
| `bool`  | `true`, `false`               |
| `int`   | `42`, `-7`, `1_000_000`       |
| `float` | `3.14`, `-0.5`               |
| `str`   | `"hello"`, `"Hi {name}!"`    |
| `list`  | `[1, 2, 3]`                  |
| `map`   | `{"key": "value", "n": 42}`  |
| `tuple` | `(1, "a", true)`             |
| `fn`    | `|x| x * 2`                  |

---

## Strings

Strings use `"double quotes"`. Interpolation with `{expr}` — any expression works:

```
name = "Alice"
age  = 30
msg  = "Name: {name}, Age: {age}, Next: {age + 1}"
```

Escape sequences: `\"`, `\\`, `\n`, `\t`, `\r`. Literal braces: `{{` and `}}`.

---

## Functions (lambdas)

Functions are lambdas assigned to variables. There is no `fn`/`def` keyword.

```
double   = |x| x * 2
add      = |a, b| a + b
greet    = |name| "Hello, {name}!"
constant = || 42           # zero-arg lambda
```

Functions are first-class values, closures capture their enclosing scope:

```
make_adder = |n| |x| x + n
add5 = make_adder(5)
add5(3)    # => 8
```

---

## Pipelines

`|>` passes the left value as the **first argument** of the right call.

```
result = [1, 2, 3] |> map(|x| x * 2) |> filter(|x| x > 3)
# equivalent to: filter(map([1,2,3], |x| x*2), |x| x>3)
```

Pipelines chain naturally:

```
"alice,bob,charlie"
  |> split(",")
  |> map(upper)
  |> sort
  |> join(", ")
# => "ALICE, BOB, CHARLIE"
```

Wrap long pipelines in `()` for multi-line:

```
result = (
  users
  |> filter(|u| u["active"])
  |> map(|u| u["name"])
  |> sort
)
```

---

## Conditionals

`if` is an expression, not a statement. It requires `then` and `else`.
Both single-line and multi-line styles are valid.

```
abs_val = if x >= 0 then x else -x
label   = if score >= 90 then "A" elif score >= 80 then "B" else "C"

# Multi-line (newlines are allowed between all keywords)
grade = if score >= 90
  then "A"
  elif score >= 80
  then "B"
  elif score >= 70
  then "C"
  else "F"
```

---

## Loops

### `while`

```
i = 0
while i < 5 do
  println(i)
  i = i + 1
end
```

`while cond do...end` evaluates the body as long as the condition is truthy.
The result of the last iteration is returned (or `null` if the body never runs).

### `for`

```
# Iterate over a list
for x in [1, 2, 3] do
  println(x)
end

# Iterate over a range
for i in 1..=5 do
  println(i)
end

# Collect results with append
result = []
for x in range(5) do
  append(result, x * x)
end
# result => [0, 1, 4, 9, 16]
```

`for var in iter do...end` binds each element of the iterable to `var` and
evaluates the body. Works with lists, tuples, ranges, and strings.

---

## Match

`match` is an inline expression. Arms are comma-separated `pattern -> value`.
The last arm does not need a trailing comma. `_` is the wildcard.

```
msg = match status, 200 -> "ok", 404 -> "not found", _ -> "error"

# Multi-arm over multiple lines (wrap in parens):
result = match code,
  0 -> "success",
  1 -> "warning",
  _ -> "failure"
```

### Pattern guards

Append `if <condition>` to any pattern to add a guard. The arm only matches
when both the pattern matches **and** the guard is truthy. Pattern-bound
variables are in scope inside the guard.

```
classify = |n| match n,
  x if x < 0  -> "negative",
  0            -> "zero",
  x if x < 10 -> "small",
  _            -> "large"

classify(-3)   # => "negative"
classify(7)    # => "small"
```

### OR patterns

Separate alternatives with `|`:

```
match code,
  200 | 201 | 204 -> "success",
  400 | 422       -> "client error",
  _               -> "other"
```

---

## Null coalescing

`??` returns the left value unless it is `null`, then returns the right:

```
port = config["port"] ?? 8080
name = input("Name: ") ?? "anonymous"
```

---

## Operators

| Category        | Operators                              |
|-----------------|---------------------------------------|
| Arithmetic      | `+`, `-`, `*`, `/`, `%`, `**` (power) |
| Comparison      | `==`, `!=`, `<`, `>`, `<=`, `>=`      |
| Boolean         | `and`, `or`, `not`                    |
| Pipe            | `\|>`                                  |
| Composition     | `>>`  (`f >> g` = `\|x\| g(f(x))`)   |
| Null-safe       | `??`                                  |
| Optional chain  | `?.`, `?[`                            |
| Range           | `..` (exclusive), `..=` (inclusive)   |
| Concat          | `+` on strings and lists              |

---

## Collections

### Lists

```
nums  = [1, 2, 3, 4, 5]
names = ["alice", "bob"]
empty = []

first_item = nums[0]
last_item  = nums[-1]      # negative indexing
length     = nums.len      # property shorthand
```

### Maps

```
user = {"name": "Alice", "age": 30}
name = user["name"]        # index access
age  = user.age            # dot access (shorthand for map["age"])

# Update (returns same map reference):
user["email"] = "alice@example.com"
```

### Tuples

Immutable fixed-length sequences. Destructure with index:

```
point = (3, 4)
x = point[0]
y = point[1]
```

---

## Destructuring assignment

Extract fields from maps or elements from lists/tuples directly into variables.

### Map destructure

```
user = {"name": "Alice", "age": 30, "city": "NYC"}
{name, age} = user
# name => "Alice", age => 30
```

Saves ~5 tokens per 3 fields compared to `name = user["name"]`.

### Tuple / list destructure

```
a, b, c = (1, 2, 3)
x, y    = [10, 20, 30]   # extra elements ignored
```

---

## Optional chaining (`?.`, `?[`)

Safe navigation: if the left side is `null`, the whole expression returns `null` instead of erroring.

```
user = {"address": {"city": "Paris"}}

# Field access
city = user?.address?.city      # => "Paris"
city = null?.address?.city      # => null

# Index access
first = user?.items?[0]         # => null if user or user["items"] is null

# Method call
name = user?.getName?()         # => null if user is null
```

Combines naturally with `??`:
```
city = user?.address?.city ?? "unknown"
```

---

## Function composition (`>>`)

`f >> g` builds a new function that calls `f` then passes its result to `g`.

```
double = |x| x * 2
inc    = |x| x + 1

double_then_inc = double >> inc
double_then_inc(5)   # => 11  (double(5)=10, inc(10)=11)

# Chain multiple:
process = trim >> lower >> |s| replace(s, " ", "_")
process("  Hello World  ")   # => "hello_world"

# Works with |> pipes:
result = 5 |> (double >> inc)   # => 11
```

Point-free pipelines with composition:
```
clean_name = trim >> lower
users |> map(|u| {u["id"]: clean_name(u["name"])})
```

### Ranges

```
r1 = 1..5     # [1, 2, 3, 4]      exclusive
r2 = 1..=5    # [1, 2, 3, 4, 5]   inclusive
```

---

## Standard library

### Core

| Function                  | Description                                         |
|---------------------------|-----------------------------------------------------|
| `print(x, ...)`           | Print without newline                               |
| `println(x, ...)`         | Print with newline                                  |
| `str(x)`                  | Convert to string                                   |
| `int(x)`                  | Convert to int                                      |
| `float(x)`                | Convert to float                                    |
| `bool(x)`                 | Convert to bool (truthy check)                      |
| `len(x)`                  | Length of str/list/map/tuple                        |
| `type(x)`                 | Returns type name as string                         |
| `repr(x)`                 | Debug representation (strings quoted)               |
| `range(n)`                | `[0, 1, ..., n-1]`                                  |
| `range(a, b)`             | `[a, a+1, ..., b-1]`                                |
| `range(a, b, s)`          | With step `s` (non-zero; negative step goes down)   |
| `error(msg)`              | Raise a runtime error and stop execution            |
| `try(fn)`                 | Call `fn`, return `{ok, value}` or `{ok, error}`    |
| `keys(map)`               | List of map keys                                    |
| `values(map)`             | List of map values                                  |
| `items(map)`              | List of `(key, value)` tuples                       |
| `set(col, key, val)`      | Mutate map/list in-place, return `val`              |
| `from_pairs(list)`        | Build map from `[(k, v), ...]`                      |
| `merge(base, overlay)`    | Merge two maps; overlay keys win                    |

### Lists

| Function                  | Description                                    |
|---------------------------|------------------------------------------------|
| `map(list, fn)`           | Apply `fn` to each item, return new list       |
| `filter(list, fn)`        | Keep items where `fn(item)` is truthy          |
| `reduce(list, fn)`        | Fold left; errors on empty list without init   |
| `reduce(list, fn, init)`  | Fold with explicit initial value               |
| `each(list, fn)`          | Call `fn` on each item for side effects        |
| `tap(list, fn)`           | Call `fn(list)`, return list unchanged         |
| `sort(list)`              | Sort by natural order, returns new list        |
| `sort_by(list, fn)`       | Sort by key function                           |
| `reverse(list)`           | Reverse, returns new list                      |
| `zip(a, b)`               | List of `(a[i], b[i])` tuples                  |
| `flatten(list)`           | Flatten one level of nested lists              |
| `first(list)`             | First item or `null`                           |
| `last(list)`              | Last item or `null`                            |
| `find(list, fn)`          | First item where `fn(item)` is truthy or `null`|
| `any(list, fn)`           | True if any item passes `fn`                   |
| `all(list, fn)`           | True if all items pass `fn`                    |
| `enumerate(list)`         | List of `(index, item)` tuples                 |
| `append(list, item)`      | Add item to end (mutates in place)             |
| `extend(list, other)`     | Add all items from `other` (mutates in place)  |
| `sum(list)`               | Sum of numeric items                           |
| `min(list)`               | Minimum value                                  |
| `max(list)`               | Maximum value                                  |

### Strings

All string functions also work as methods: `s.split(",")` = `split(s, ",")`.

| Function                        | Description                         |
|---------------------------------|-------------------------------------|
| `split(s, sep)`                 | Split into list                     |
| `join(list, sep)`               | Join list into string               |
| `trim(s)`                       | Strip leading/trailing whitespace   |
| `trim_start(s)` / `trim_end(s)` | One-sided trim                      |
| `upper(s)` / `lower(s)`         | Case conversion                     |
| `replace(s, from, to)`          | Replace all occurrences             |
| `contains(s, sub)`              | True if `sub` is in `s`             |
| `starts_with(s, pre)`           | Prefix check                        |
| `ends_with(s, suf)`             | Suffix check                        |
| `lines(s)`                      | Split on newlines                   |
| `chars(s)`                      | Split into individual characters    |

String property shorthands (no call needed):

```
s.len      # character count
s.upper    # uppercase copy
s.lower    # lowercase copy
s.trim     # trimmed copy
s.lines    # list of lines
s.chars    # list of chars
```

### File I/O

| Function               | Description                                 |
|------------------------|---------------------------------------------|
| `read(path)`           | Read file as string                         |
| `write(path, content)` | Write string to file (overwrites)           |
| `file_append(path, content)`| Append string to file                  |
| `exists(path)`         | True if file/directory exists               |
| `ls(path?)`            | List directory contents                     |
| `input(prompt?)`       | Read a line from stdin                      |

### JSON

```
json_parse(str)        # parse JSON string → cap value (map/list/int/str/...)
json_stringify(value)  # cap value → JSON string
```

### HTTP / Networking

All functions return `{status: int, body: str, headers: map}`.

| Function                            | Description              |
|-------------------------------------|--------------------------|
| `http_get(url)`                     | GET request              |
| `http_get(url, headers_map)`        | GET with custom headers  |
| `http_post(url, body)`              | POST with string body    |
| `http_post(url, body, headers_map)` | POST with headers        |
| `http_put(url, body)`               | PUT request              |
| `http_delete(url)`                  | DELETE request           |
| `http_request(method, url, body, headers)` | Generic request   |

```
r = http_get("https://api.example.com/data")
if r["status"] == 200 then json_parse(r["body"]) else error(r["body"])
```

### System / Shell / Regex

| Function                          | Description                                       |
|-----------------------------------|---------------------------------------------------|
| `shell(cmd)`                      | Run shell command → `{status, stdout, stderr}`    |
| `shell_lines(cmd)`                | Run command → list of non-empty stdout lines      |
| `env(name)`                       | Get environment variable or `null`                |
| `env_all()`                       | Map of all environment variables                  |
| `python(code)`                    | Run Python 3 code → stdout string                 |
| `python(code, stdin)`             | Run Python with stdin input                       |
| `pyval(code)`                     | Run Python, return structured value (see below)   |
| `regex_match(pattern, text)`      | True if pattern matches anywhere in text          |
| `regex_find(pattern, text)`       | First match string or `null`                      |
| `regex_find_all(pattern, text)`   | List of all match strings                         |
| `regex_replace(pattern, repl, text)` | Replace all matches                            |

#### `pyval` — Python JSON bridge

`pyval(code)` injects a `cap_return(value)` helper into the code. Call it to
return any JSON-serializable Python value back to cap:

```
pi     = pyval("import math; cap_return(math.pi)")
primes = pyval("cap_return([x for x in range(2,50) if all(x%i for i in range(2,x))])")
stats  = pyval("""
import statistics
data = [2, 4, 4, 4, 5, 5, 7, 9]
cap_return({'mean': statistics.mean(data), 'stdev': statistics.stdev(data)})
""")
```

### CSV

Native Rust parsing — no Python required. Types (int, float, str) are
inferred automatically.

| Function                   | Description                                       |
|----------------------------|---------------------------------------------------|
| `csv_read(path)`           | Read CSV file → list of maps (header = keys)      |
| `csv_read(path, sep)`      | Custom separator (e.g. `"\t"` for TSV)            |
| `csv_read_raw(path)`       | Read CSV → list of lists (no header interpretation)|
| `csv_write(path, data)`    | Write list of maps to CSV file                    |
| `csv_parse(str)`           | Parse CSV string → list of maps                   |
| `csv_parse(str, sep)`      | Parse with custom separator                       |

```
rows = csv_read("data.csv")
rows[0]["name"]           # => "Alice"

csv_write("out.csv", rows)

# TSV
rows = csv_read("data.tsv", "\t")

# From string
rows = csv_parse("a,b\n1,2\n3,4")
```

### DataFrame (`df_*`)

Pandas-backed operations. Cap represents DataFrames as **list of maps**
(each map is one row). Requires Python 3 and pandas installed.

| Function                         | Description                                         |
|----------------------------------|-----------------------------------------------------|
| `df_read(path)`                  | Read CSV/JSON/Excel → list of maps (auto-detected)  |
| `df_write(data, path)`           | Write to CSV/JSON/Excel (by extension)              |
| `df_head(data, n=5)`             | First `n` rows                                      |
| `df_tail(data, n=5)`             | Last `n` rows                                       |
| `df_shape(data)`                 | `{rows: int, cols: int}`                            |
| `df_columns(data)`               | List of column name strings                         |
| `df_describe(data)`              | Descriptive statistics map per column               |
| `df_select(data, cols)`          | Keep only the listed columns                        |
| `df_drop(data, cols)`            | Remove the listed columns                           |
| `df_filter(data, query)`         | Filter rows using pandas query syntax               |
| `df_sort(data, col)`             | Sort ascending by column                            |
| `df_sort(data, col, true)`       | Sort descending                                     |
| `df_groupby(data, col, agg)`     | Group by column; agg: `"sum"`, `"mean"`, `"count"`, `"min"`, `"max"` |
| `df_join(left, right, on)`       | Inner join on column                                |
| `df_join(left, right, on, how)`  | Join with `"inner"`, `"left"`, `"right"`, `"outer"` |
| `df_rename(data, map)`           | Rename columns: `{"old": "new"}`                    |
| `df_fillna(data, value)`         | Fill `null`/NaN with value                          |
| `df_apply(data, col, lambda_str)`| Apply a Python lambda string to a column            |

```
df = df_read("sales.csv")
df_shape(df)                              # {rows: 1000, cols: 8}
top = df_filter(df, "revenue > 10000")
by_region = df_groupby(top, "region", "sum")
df_write(by_region, "summary.xlsx")
```

### Plots (`plt_*`)

Matplotlib / seaborn wrappers. All functions save to a PNG and return its
path. Requires Python 3 and matplotlib installed; seaborn optional.

All accept an optional last argument `opts` (a map):

| Key        | Effect                                          |
|------------|-------------------------------------------------|
| `"title"`  | Chart title                                     |
| `"xlabel"` | X-axis label                                    |
| `"ylabel"` | Y-axis label                                    |
| `"color"`  | Line/bar/scatter color (any matplotlib color)   |
| `"label"`  | Legend label (adds a legend)                    |
| `"save"`   | Output file path (default: `/tmp/cap_plot_<ts>.png`) |
| `"figsize"`| `[width, height]` in inches                     |
| `"bins"`   | Number of histogram bins (default 20)           |
| `"annot"`  | Show values in heatmap cells (true/false)       |
| `"labels"` | Tick labels for boxplot                         |

| Function                     | Description                              |
|------------------------------|------------------------------------------|
| `plt_line(x, y)`             | Line chart                               |
| `plt_line(x, y, opts)`       | With options                             |
| `plt_bar(labels, values)`    | Bar chart                                |
| `plt_scatter(x, y)`          | Scatter plot                             |
| `plt_hist(data)`             | Histogram                                |
| `plt_hist(data, opts)`       | With custom `"bins"`                     |
| `plt_boxplot(data)`          | Box-and-whisker plot                     |
| `plt_heatmap(matrix)`        | Heatmap (seaborn if available)           |
| `plt_pie(values, labels)`    | Pie chart                                |
| `plt_show(path)`             | Open saved image in system viewer        |

```
path = plt_line(
  range(10),
  range(10) |> map(|x| x * x),
  {"title": "y = x²", "xlabel": "x", "ylabel": "y", "color": "steelblue"}
)
plt_show(path)

# Save to specific file:
plt_bar(["Q1","Q2","Q3","Q4"], [120,95,140,160], {"save": "revenue.png"})
```

### PyTorch (`torch_*`)

Neural network utilities. Requires Python 3 and PyTorch installed.
Models are serialized as base64 strings for portability.

| Function                             | Description                                   |
|--------------------------------------|-----------------------------------------------|
| `torch_device()`                     | `"cuda"`, `"mps"`, or `"cpu"`                 |
| `torch_tensor(list)`                 | Create tensor → `{shape, dtype, data}`        |
| `torch_zeros(shape)`                 | Zero tensor → `{shape, data}`                 |
| `torch_ones(shape)`                  | Ones tensor → `{shape, data}`                 |
| `torch_train_linear(X, y)`           | Train a simple MLP, return `{losses, model_state}` |
| `torch_train_linear(X, y, opts)`     | opts: `{lr, epochs, hidden}`                  |
| `torch_predict(model_state, X)`      | Run inference → list of predictions           |
| `torch_predict(model_state, X, hidden)` | Specify hidden layer size               |
| `torch_save(model_state, path)`      | Save model to file                            |
| `torch_load(path)`                   | Load model from file → base64 state string    |

```
# Train
X = [[1.0],[2.0],[3.0],[4.0]]
y = [2.0, 4.0, 6.0, 8.0]
result = torch_train_linear(X, y, {"lr": 0.01, "epochs": 200})
println("final loss: " + str(result["losses"][-1]["loss"]))

# Predict
preds = torch_predict(result["model_state"], [[5.0],[6.0]])
println("predictions: " + str(preds))

# Persist
torch_save(result["model_state"], "model.pt")
state = torch_load("model.pt")
```

### YAML & TOML

```
yaml_parse(str)         # parse YAML string → value
yaml_stringify(value)   # value → YAML string
toml_parse(str)         # parse TOML string → value
toml_stringify(value)   # value → TOML string
```

### File System (`fs_*` / `os_*`)

Native Rust — no Python required.

| Function | Description |
|---|---|
| `fs_read(path)` | Read file → string |
| `fs_write(path, str)` | Overwrite file |
| `fs_append(path, str)` | Append to file |
| `fs_delete(path)` | Delete file |
| `fs_exists(path)` | → bool |
| `fs_is_file(path)` / `fs_is_dir(path)` | Type check |
| `fs_mkdir(path)` / `fs_mkdir_all(path)` | Create directory (with parents) |
| `fs_rmdir(path)` | Remove directory tree |
| `fs_copy(src, dst)` / `fs_move(src, dst)` | Copy / rename |
| `fs_ls(path?)` | List directory |
| `fs_stat(path)` | `{size, is_file, is_dir, modified, readonly}` |
| `os_cwd()` / `os_chdir(path)` | Working directory |
| `os_home()` / `os_pid()` / `os_hostname()` / `os_username()` | System info |
| `os_path_join(...)` / `os_path_basename(p)` / `os_path_dirname(p)` / `os_path_ext(p)` / `os_abs(p)` | Path utilities |

### Time (`time_*`)

Native Rust via `chrono` — no Python required.

```
now  = time_now()                       # {unix: int, tz: "local"}
utc  = time_now_utc()                   # {unix: int, tz: "utc"}
t    = time_unix(1700000000)            # from unix timestamp
t2   = time_parse("2024-01-15", "%Y-%m-%d")
s    = time_format(now, "%Y-%m-%d %H:%M:%S")
t3   = time_add(now, 3600)             # add seconds
diff = time_diff(t3, now)              # => 3600 (seconds)
time_year(now) / time_month(now) / time_day(now)
time_hour(now) / time_minute(now) / time_second(now)
time_weekday(now)                      # => "Thursday"
time_sleep(1.5)                        # sleep N seconds
```

### SQLite (`sql_*`)

Native Rust via `rusqlite` — no Python required. SQLite is bundled.

```
sql_open("mydb.sqlite")  # or ":memory:"
sql_exec("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)")
sql_exec("INSERT INTO t (name) VALUES (?)", ["Alice"])
rows = sql_query("SELECT * FROM t WHERE id > ?", [0])
row  = sql_query_one("SELECT * FROM t WHERE id = ?", [1])
sql_begin() / sql_commit() / sql_rollback()
sql_tables()          # => ["t", ...]
sql_schema("t")       # => [{name, type, notnull, pk}, ...]
sql_close()
```

### Streaming I/O & Gzip (`stream_*` / `gz_*`)

Native Rust via `flate2` — no Python required.

```
lines = stream_lines("large.log")   # list of lines
bytes = stream_bytes("data.bin")    # list of byte ints
stream_write("out.txt", content)
stream_append("log.txt", content)

gz_write("data.gz", content)        # compress and write
content = gz_read("data.gz")        # decompress and read
b64  = gz_compress("hello!")        # → base64 string
orig = gz_decompress(b64)
```

### LLM (`ollama_*`, `openai_*`, `anthropic_*`, `gemini_*`, `llm_*`)

Uses Python's built-in `urllib` — no pip install required for basic calls.

```
# Message builders
msgs = [llm_user("What is 2+2?")]
msgs = [llm_system("You are helpful"), llm_user("Hi")]
# llm_assistant(text) builds an assistant turn

# Ollama (local — no API key)
reply = ollama_chat("llama3.2", msgs)
reply["content"]           # => "4"
text  = ollama_complete("llama3.2", "Once upon a time")
emb   = ollama_embed("nomic-embed-text", "hello world")
ollama_list()              # list available models
ollama_pull("llama3.2")    # pull a model

# Generic aliases (delegate to Ollama)
reply = llm_chat("llama3.2", msgs)
text  = llm_complete("llama3.2", "Once upon a time")
emb   = llm_embed("nomic-embed-text", "hello world")

# OpenAI-compatible (reads OPENAI_API_KEY)
reply = openai_chat("gpt-4o-mini", msgs)
emb   = openai_embed("text-embedding-3-small", "hello")

# Anthropic (reads ANTHROPIC_API_KEY)
reply = anthropic_chat("claude-haiku-4-5-20251001", msgs)

# Google Gemini (reads GEMINI_API_KEY)
reply = gemini_chat("gemini-1.5-flash", msgs)

# All reply maps: {content: str, model: str, tokens: {in: int, out: int}}
```

### Crypto (`hash_*`, `hmac_*`, `b64_*`, `uuid_*`, `rand_*`, `pbkdf2_*`)

Uses Python's built-in `hashlib`, `hmac`, `secrets` — no pip install required.

```
hash_sha256("hello")          # hex string
hash_md5("data")
hash_sha512("data")
hash_file("big.bin")          # SHA-256 of file

hmac_sha256("secret", "msg")  # hex HMAC

b64_encode("hello")           # base64 string
b64_decode("aGVsbG8=")        # => "hello"
hex_encode("hi")
hex_decode("6869")

uuid_v4()                     # random UUID string
rand_int(1, 100)              # random integer in [1, 100)
rand_float()                  # random float in [0, 1)
rand_bytes(16)                # random bytes as hex string
rand_choice([1,2,3])          # random element
rand_shuffle([1,2,3,4,5])     # shuffled copy

h = pbkdf2_hash("password")                         # {hash: str, salt: str}
pbkdf2_verify("password", h["hash"], h["salt"])     # => bool
```

### Vector DBs (`chroma_*`, `pinecone_*`, `vec_*`)

```
# Requires: pip install chromadb  (for chroma_*)
# Requires: pip install pinecone-client  (for pinecone_*)

# ChromaDB
chroma_add("my_index", ["id1","id2"], [emb1,emb2], ["doc1","doc2"],
           {"path": "./chroma_db"})
results = chroma_query("my_index", query_emb, 3, {"path": "./chroma_db"})
results[0]["document"]      # => "doc1"

# Pinecone
pinecone_init("my-index", {"api_key": env("PINECONE_API_KEY")})
pinecone_upsert("my-index", [{"id":"v1", "values": emb}], opts)
matches = pinecone_query("my-index", query_emb, 5, opts)

# Native Rust vector math (no Python needed)
vec_cosine_sim([1.0, 0.0], [0.0, 1.0])  # => 0.0
vec_dot([1.0, 2.0], [3.0, 4.0])         # => 11.0
vec_norm([3.0, 4.0])                     # => 5.0
```

### HTTP Server (`server_*`)

```
# Requires: pip install flask

# Receive one request then stop
req = server_serve_once(8080, 30)   # port, timeout_seconds
req["method"]  # "POST"
req["body"]    # raw body string
req["headers"] # map

# Background server
server_start(8080)
req = server_poll(30)
server_stop()

# Mock — respond immediately with a canned response
resp = server_mock(8080, {"status": 200, "body": "ok"})
```

### Image Processing (`img_*`)

Requires `pip install Pillow`. All functions work with **file paths**.
Transform functions write a new file and return the output path.

**Metadata** (single path → value):

| Function | Returns |
|---|---|
| `img_open(path)` | `{path, width, height, mode, format}` |
| `img_info(path)` | `{path, width, height, mode, format, size_bytes}` |
| `img_size(path)` | `{width: int, height: int}` |
| `img_mode(path)` | Mode string: `"RGB"`, `"RGBA"`, `"L"`, … |
| `img_pixels(path)` | List of `[r, g, b]` per pixel |
| `img_to_array(path)` | Nested list H×W×C (requires numpy) |
| `img_show(path)` | Open in system viewer |

**Transforms** — `img_*(src, dst, ...) → dst_path`:

```
img_save("in.jpg", "copy.png")
img_resize("in.jpg", "out.jpg", 320, 240)
img_crop("in.jpg", "out.jpg", 0, 0, 100, 100)    # left top right bottom
img_rotate("in.jpg", "out.jpg", 90)
img_flip("in.jpg", "out.jpg", "horizontal")        # or "vertical"
img_grayscale("in.jpg", "out.jpg")
img_blur("in.jpg", "out.jpg", 2.0)                # Gaussian radius
img_sharpen("in.jpg", "out.jpg")
img_brightness("in.jpg", "out.jpg", 1.5)          # 1.0 = unchanged
img_contrast("in.jpg", "out.jpg", 1.2)
img_thumbnail("in.jpg", "out.jpg", 128)            # max dimension, keeps aspect ratio
img_convert("in.jpg", "out.jpg", "RGB")           # "RGBA", "L", "P", etc.
img_draw_text("in.jpg", "out.jpg", "Hello", 10, 10, {"font_size": 24, "color": "white"})
img_paste("base.jpg", "overlay.png", "out.jpg", 50, 50)   # paste at (x,y)
img_from_array(pixels, "out.png", "RGB")          # create from H×W×C list (needs numpy)
```

### Scikit-Learn (`sklearn_*`)

```
# Requires: pip install scikit-learn

# Train
X = [[1.0],[2.0],[3.0],[4.0]]
y = [0, 0, 1, 1]
# Model names: "linear", "logistic", "ridge", "lasso", "svm", "svr",
#              "decision_tree", "random_forest", "gradient_boosting", "knn"
model = sklearn_train(X, y, {"model": "random_forest", "n_estimators": 100})

preds = sklearn_predict(model, [[1.5],[3.5]])
score = sklearn_score(model, X_test, y_test)

# Evaluation
metrics = sklearn_metrics(y_true, y_pred)          # {accuracy, precision, recall, f1, ...}
fi      = sklearn_feature_importance(model)         # [{feature, importance}, ...]

# Preprocessing
X_scaled = sklearn_scale(X)
X_norm   = sklearn_normalize(X)
y_enc    = sklearn_encode_labels(["cat","dog","cat"])

# Splits — returns {X_train, X_test, y_train, y_test}
split = sklearn_train_test_split(X, y, 0.2)   # 3rd arg: test_size float (default 0.2)

# Clustering
labels = sklearn_kmeans(X, 3)
labels = sklearn_dbscan(X, {"eps": 0.5, "min_samples": 5})

# Model persistence
sklearn_save(model, "model.pkl")
model = sklearn_load("model.pkl")

# Tuning
cv   = sklearn_cross_val({"model": "svm"}, X, y, {"cv": 5})
best = sklearn_grid_search(param_grid, X, y)
```

### PDF (`pdf_*`)

```
# Requires: pip install pdfplumber pypdf reportlab

pdf_pages("doc.pdf")                # page count
pdf_text("doc.pdf")                 # full text
pdf_page_text("doc.pdf", 0)         # text of page 0
pdf_tables("doc.pdf", 0)            # tables on page 0 (list of lists)
pdf_metadata("doc.pdf")             # {title, author, ...}
pdf_images("doc.pdf", 0)            # images on page 0

# Create a PDF from text
pdf_create("out.pdf", ["Page one content", "Page two content"])

# Merge PDFs
pdf_merge(["a.pdf", "b.pdf"], "combined.pdf")
```

### ZIP / TAR Archives (`zip_*` / `tar_*`)

```
# No pip required — uses Python's built-in zipfile/tarfile

zip_list("archive.zip")                     # [{name, size, compressed_size, is_dir}, ...]
zip_extract("archive.zip", "file.txt", "/tmp")  # extract one file
zip_extract_all("archive.zip", "/tmp")      # extract all
zip_create("new.zip", ["/path/to/a", "/path/to/b"])
zip_add("new.zip", "/path/to/file")
zip_read_entry("archive.zip", "readme.txt") # read without extracting

tar_list("archive.tar.gz")                  # [{name, size}, ...]
tar_extract("archive.tar.gz", "file.txt", "/tmp")
tar_extract_all("archive.tar.gz", "/tmp")
tar_create("new.tar.gz", ["/path/a", "/path/b"])
```

### Apache Arrow (`arrow_*`)

```
# Requires: pip install pyarrow pandas

rows = [{"name": "Alice", "age": 30}]
tbl  = arrow_from_list(rows)
arrow_schema(tbl)                            # {"name": "string", "age": "int64"}
arrow_to_list(tbl)                           # list of maps
arrow_from_parquet("data.parquet")
arrow_to_parquet(tbl, "out.parquet")
arrow_filter(tbl, "age > 26")
arrow_select(tbl, ["name"])
arrow_sort(tbl, "age", true)                 # desc=true
arrow_aggregate(tbl, "age", "sum")
```

---

## Imports / Module System

`import(path)` evaluates a Cap file in an isolated scope and returns all its
top-level bindings as a map.

```
# utils.cap
double = |x| x * 2
PI     = 3.14159

# main.cap
u = import("utils.cap")
u.double(5)    # => 10
u["PI"]        # => 3.14159

# Destructure immediately:
mod = import("utils.cap")
double = mod["double"]
PI     = mod["PI"]
```

Only top-level assignments are exported — intermediate values computed inside
`do...end` blocks are not visible.

---

## Sequential blocks (`do...end`)

A `do...end` block evaluates multiple statements and returns the value of
the last one. Use it anywhere an expression is expected.

```
result = do
  x = expensive_a()
  y = expensive_b()
  x + y
end

# Inside a lambda:
process = |data| do
  cleaned = data |> filter(|x| x != null)
  total   = cleaned |> sum
  avg     = total / cleaned.len
  avg
end
```

---

## Error handling (`try`)

`try(fn)` calls `fn` and catches any runtime error. Returns a result map:

```
r = try(|| read("missing.txt"))
if r["ok"] then r["value"] else "default"

# Inline:
name = try(|| data["name"]).value ?? "unknown"
```

Result shape:
- Success: `{ok: true, value: <result>}`
- Failure: `{ok: false, error: "<TypePrefix: message>"}` — e.g. `"RuntimeError: oops"`, `"IOError: ..."`

---

## Map utilities

```
# Mutation (works inside closures — maps are shared by reference):
set(map, "key", value)         # set map["key"] = value, returns value
set(list, 2, value)            # set list[2] = value

# Build a map from a list of (key, value) pairs:
m = from_pairs([("a", 1), ("b", 2)])   # => {"a": 1, "b": 2}

# Merge two maps (second wins on key conflicts):
merged = merge(base, overlay)
```

---

## OOP (classes)

The `class` keyword defines a constructor that returns a map of methods.
Constructor arguments are captured in all method closures — no `self` needed.

```
class Point(x, y),
  dist = || (x*x + y*y) ** 0.5,
  add  = |other| Point(x + other["x"], y + other["y"]),
  str  = || "({x}, {y})"

p1 = Point(3, 4)
p2 = Point(1, 2)
println(p1["str"]())               # => (3, 4)
println(p1["dist"]())              # => 5.0
println(p1["add"](p2)["str"]())    # => (4, 6)
```

Methods are accessed with `obj["method"](args)` or dot syntax `obj.method(args)`. The `class` keyword is purely
syntactic sugar — it desugars to a lambda returning a map:

```
# class Point(x, y), dist = || expr
# is exactly the same as:
Point = |x, y| {"dist": || expr, ...}
```

### Important: scope rules for class methods

Constructor args are **directly accessible** by name in all method bodies. However,
**other method names are NOT in scope** — methods are stored in a map, not as local variables.

```
class Circle(r),
  area      = || 3.14159 * r * r,      # r is in scope ✓
  diameter  = || r * 2,                 # r is in scope ✓
  # perimeter = || 2 * area()           # ERROR: area is not a variable
  perimeter = || 2 * 3.14159 * r        # inline the logic instead ✓
```

To expose constructor args externally, add getter methods:

```
class Item(name, price),
  get_name  = || name,     # exposes name for callers
  get_price = || price,    # exposes price for callers
  discount  = |pct| price * (1.0 - pct)
```

Objects are immutable by default. Mutations return new instances:

```
class Counter(n),
  inc   = || Counter(n + 1),
  value = || n

c = Counter(0)["inc"]()["inc"]()
println(c["value"]())   # => 2
```

### Calling methods

```
obj["method"]()          # call no-arg method
obj["method"](arg)       # call with argument
obj.method()             # dot syntax — same as above
obj.method(arg)          # dot syntax with argument
obj["field"]             # access a value stored in the map
obj.field                # dot shorthand for map field access
```

### Inheritance (`extends`)

`extends Parent(args)` merges the parent's map into the child's, with
child methods overriding parent methods:

```
class Animal(name),
  speak = || "{name} says ...",
  kind  = || "animal"

class Dog(name) extends Animal(name),
  speak = || "{name} says woof",
  fetch = || "{name} fetches the ball"

d = Dog("Rex")
d.speak()    # => "Rex says woof"
d.kind()     # => "animal"   (inherited)
d.fetch()    # => "Rex fetches the ball"
```

### Multi-line method bodies

Methods that need more than one expression should use `()` to allow newlines:

```
class Shape(w, h),
  area    = || w * h,
  scale   = |factor| Shape(w * factor, h * factor),
  summary = || "Shape " + str(w) + "x" + str(h) + " area=" + str(w * h)
```

---

## Patterns and idioms

### Process a file

```
read("data.txt")
  |> lines
  |> filter(|l| l != "")
  |> map(trim)
  |> each(println)
```

### Transform JSON-like data

```
users = [{"name": "Alice", "score": 95}, {"name": "Bob", "score": 72}]
top = users |> filter(|u| u["score"] >= 90) |> map(|u| u["name"]) |> sort
```

### Safe map lookup

```
value = data["key"] ?? "default"
```

### Compose functions

```
pipe = |f, g| |x| g(f(x))
process = pipe(trim, lower)
clean_names = names |> map(process)
```

### Fibonacci (recursive lambda)

```
fib = |n| if n <= 1 then n else fib(n-1) + fib(n-2)
```

---

## Error messages

Errors include a source pointer:

```
  --> script.cap:3:8
   |
   3 | result = foo(x y)
   |         ^
SyntaxError: unexpected `y`, expected `)` or `,`
```

---

## Running cap

```bash
cap <file.cap>          # run a script
cap check <file.cap>    # syntax check only
cap ast <file.cap>      # print AST
cap repl                # interactive REPL
```

---

## Token efficiency vs Python

| Python                              | Cap                              | Tokens saved |
|-------------------------------------|-----------------------------------|-------------|
| `def double(x): return x * 2`      | `double = \|x\| x * 2`            | ~40%        |
| `lambda x: x * 2`                  | `\|x\| x * 2`                     | ~30%        |
| `list(filter(lambda x: ..., xs))`  | `xs \|> filter(\|x\| ...)`        | ~35%        |
| `f"hello {name}"`                   | `"hello {name}"`                  | ~10%        |
| `if cond:\n  a\nelse:\n  b`        | `if cond then a else b`           | ~50%        |
| `[x for x in xs if x > 0]`         | `xs \|> filter(\|x\| x > 0)`      | ~20%        |
