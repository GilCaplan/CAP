# Cap Language — Implementation Progress

## Current status: production-ready core, all planned features shipped

---

## Phase 1 — Core language ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Lexer + Pratt parser | ✅ | Full operator precedence, right-assoc power |
| Literals: null, bool, int, float, str | ✅ | |
| String interpolation `"Hello {name}!"` | ✅ | Nested `{expr}` supported |
| Variables + assignments | ✅ | |
| Lambdas `\|x\| expr` | ✅ | Closures, zero-arg `\|\|` |
| Recursion | ✅ | Named functions inject self into call env |
| Pipelines `\|>` | ✅ | Desugars to `f(lhs, args...)` |
| Null coalescing `??` | ✅ | |
| Ranges `..` / `..=` | ✅ | Evaluate to lists |
| Collections: list, map, tuple | ✅ | |
| Negative / range indexing | ✅ | `list[-1]`, `list[1..3]` |
| `if / then / elif / else` | ✅ | Expression, not statement |
| `match` with patterns | ✅ | Literal, wildcard, bind, OR, guards |
| `while cond do…end` | ✅ | |
| `for var in iter do…end` | ✅ | Lists, tuples, ranges |
| `do…end` blocks | ✅ | Sequential statements, returns last |
| `class` desugaring | ✅ | Desugars to lambda returning map |
| `class extends` | ✅ | Uses `merge()` |
| `import("file.cap")` | ✅ | Returns exported bindings as map |
| `try(fn)` | ✅ | Returns `{ok, value}` or `{ok:false, error}` |
| Field/index assignment `obj.f = v`, `obj[k] = v` | ✅ | |
| Method call `obj.method(args)` | ✅ | Dot dispatch via method partials |

---

## Phase 2 — Bug fixes ✅

All 33 documented bugs resolved. Key fixes:

- BUG-1: match consuming trailing newline → pos save/restore
- BUG-2: recursive functions fail → inject self into call env
- BUG-3: string interpolation with escaped quotes
- BUG-5: missing map key throws KeyError → returns null
- BUG-7: `append` name collision → `file_append` for IO
- BUG-9: TypeError shows wrong operand → String type in error
- BUG-10: missing args silently null → TooFewArgs error
- BUG-17: modulo by zero panic → zero guard
- BUG-33: match OR pattern only accepts `|` → also accepts `or`

---

## Phase 3 — New language features ✅ (2025-03)

| Feature | Status | Details |
|---------|--------|---------|
| **Map destructuring** `{name, age} = map` | ✅ | Saves ~5 tokens per 3 fields |
| **Tuple/list destructuring** `a, b = tuple` | ✅ | Unpacks positional elements |
| **Newline assignment** `x =\n  expr` | ✅ | Skip newlines after `=` in all assign forms |
| **Optional chaining** `obj?.field`, `obj?[idx]`, `obj?.method()` | ✅ | Returns null on null receiver |
| **Function composition** `f >> g` | ✅ | Builds `\|x\| g(f(x))`, right-assoc chains |

---

## Phase 4 — Standard library ✅

28 modules bridged to Python via `pyval`:

| Module | Status |
|--------|--------|
| core (print, len, type, try, keys, merge…) | ✅ |
| list (map, filter, reduce, sort, zip…) | ✅ |
| string (split, join, trim, replace…) | ✅ |
| io (read, write, file_append) | ✅ |
| json (json_parse, json_str) | ✅ |
| csv (csv_read, csv_write) | ✅ |
| fs (ls, mkdir, rm, mv, cp, exists) | ✅ |
| sys (exec, env) | ✅ |
| net (fetch, get, post) | ✅ |
| time (time_now, time_fmt, sleep) | ✅ |
| sql (sql_open, sql_query, sql_exec) | ✅ |
| plot (plot_line, plot_bar, plot_scatter) | ✅ |
| df (df_read, df_write, df_filter…) | ✅ |
| torch (tensor ops) | ✅ |
| sklearn (fit, predict, score) | ✅ |
| llm (llm_complete, llm_embed) | ✅ |
| vector (vec_store, vec_search) | ✅ |
| stream (stream_read, stream_write) | ✅ |
| arrow (arrow_read, arrow_write) | ✅ |
| task (task_spawn, task_wait) | ✅ |
| ffi (ffi_call) | ✅ |
| cluster (cluster_map, cluster_run) | ✅ |
| wasm (wasm_load, wasm_call) | ✅ |
| server (serve, route) | ✅ |
| image (img_load, img_save, img_resize) | ✅ |
| crypto (hash, hmac, encrypt, decrypt) | ✅ |
| zip_archive (zip_read, zip_write) | ✅ |
| pdf (pdf_read, pdf_pages) | ✅ |

---

## Pending / Future work

| Item | Priority | Notes |
|------|----------|-------|
| `?` error propagation (Rust-style) | Medium | `risky()? ` returns null upward |
| Lazy `\|>` — suppress newlines after `\|>` | Medium | Multi-line pipes without parens |
| Tail-call optimization | Medium | Deep recursion hits 8000-frame limit |
| Spread operator `...list` | Low | Useful for variadic calls |
| `=> ` single-arg lambda sugar | Low | `x => x*2` vs `\|x\| x*2` |
| Async / concurrency | Low | Phase 5 |
