# Cap Language — Bug Tracker

**Workflow:** The finder agent adds bugs here (status: `open`). The fixer agent marks them `fixed` with a note on the fix.

---

## BUG-1: Match expression consumes trailing newline
- **Status:** fixed
- **File:** tests/functions.cap (line 29–37), tests/pipelines.cap (line 21–28)
- **Expected:** A multi-arm `match` followed by a new statement on the next line parses correctly
- **Actual:** `SyntaxError: unexpected <next-statement>, expected newline or EOF`
- **Fix:** `src/parser/mod.rs` — save/restore `self.pos` before `skip_newlines()` in `parse_match_expr` when no comma follows (same pattern the class parser already used)
- **Minimal repro:**
  ```cap
  label = match x, 1 -> "one", _ -> "other"
  println(label)
  ```

---

## BUG-2: Recursive functions fail with NameError
- **Status:** fixed
- **File:** any file with a recursive lambda
- **Expected:** `fib(10)` returns `55`
- **Actual:** `NameError: fib is not defined` — closure snapshot taken before variable is assigned
- **Fix:** `src/interpreter/mod.rs` — (a) give lambdas their variable name on assignment in `eval_stmt`; (b) inject function's own name into call env at start of `call_function`
- **Minimal repro:**
  ```cap
  fib = |n| if n <= 1 then n else fib(n - 1) + fib(n - 2)
  println(fib(10))
  ```

---

## BUG-3: String interpolation fails when `{expr}` contains escaped quotes
- **Status:** fixed
- **File:** programs/automation.cap (line 23)
- **Expected:** `"{p[\"name\"]}"` interpolates the map key `"name"` correctly
- **Actual:** `RuntimeError: interpolation error: SyntaxError: unexpected character '\'`
- **Fix:** `src/lexer/mod.rs` — unescape `\"` → `"` and `\\` → `\` inside the interpolation content scanner
- **Minimal repro:**
  ```cap
  p = {"name": "alice"}
  println("hello {p[\"name\"]}")
  ```

---

## BUG-4: Class constructor not accessible from within methods
- **Status:** fixed (covered by BUG-2 fix)
- **File:** tests/oop.cap
- **Expected:** `Point(x+…, y+…)` inside a method creates a new Point
- **Actual:** `NameError: Point is not defined`
- **Fix:** Same as BUG-2 — self-injection at call time makes the constructor visible inside its own body
- **Minimal repro:**
  ```cap
  class Point(x, y), double = || Point(x * 2, y * 2)
  p = Point(1, 2)
  ```

---

## BUG-5: Missing map key throws KeyError instead of returning null
- **Status:** fixed
- **File:** tests/pipelines.cap (line 36)
- **Expected:** `config["port"] ?? 8080` returns `8080` when `"port"` is not in the map
- **Actual:** `KeyError: key 'port' not found` — crashes before `??` can activate
- **Fix:** `src/interpreter/mod.rs` `eval_index` — map `[]` access now returns `Value::Null` for missing keys instead of `KeyError`, consistent with `??` null-coalescing usage. Field access via `.` still errors for missing keys (intentional).
- **Minimal repro:**
  ```cap
  m = {"a": 1}
  println(m["b"] ?? 99)
  ```

---

## BUG-6: Unclosed `{` in string silently swallows adjacent string literals
- **Status:** fixed
- **File:** tests/test_braces.cap (line 3)
- **Expected:** `"escaped {"` should be a `SyntaxError` (unterminated interpolation), since `{` opens an interpolation that has no closing `}`
- **Actual:** The interpolation scanner consumed the `"` end-of-string delimiter plus the entire next string literal (`+ "name}"`) before finding a `}` — silent wrong result
- **Fix:** `src/lexer/mod.rs` — interpolation content scanner now treats an unescaped `"` as an unterminated-string error. Use `\"` inside `{...}` to embed string literals.
- **Minimal repro:**
  ```cap
  s = "escaped {" + "name}" # SyntaxError: unterminated string
  ```

---

## BUG-7: `append` function for file I/O throws TypeError expecting a list
- **Status:** fixed
- **Fix:** `src/interpreter/mod.rs` — `call_builtin_str` intercepts `"append"` before list dispatch; if first arg is `Str` → routes to `io::file_append`; otherwise → list append.
- **File:** tests/test_stdlib_io.cap (line 15)
- **Expected:** `append("tests/temp_test_io.txt", "world\n")` should append string content to the file.
- **Actual:** `append` throws `TypeError: expected list, got str` because it assumes the first argument is a list and does not support file paths.
- **Minimal repro:**
  ```cap
  append("test.txt", "hello")
  ```


## TOKEN EFFICIENCY FLAG: Sequential Computations
- **Issue:** Without sequential statement blocks, computing intermediate variables and returning a result requires chaining lambdas or deep `if/else` nesting, which is far LESS readable and MORE verbose than Python.
- **Python Comparison:**
  ```python
  def diff(x, y):
      a = expensive(x)
      b = expensive(y)
      return a - b
  ```
- **Cap Equivalent:**
  ```cap
  diff = |x, y| (|a| (|b| a - b)(expensive(y)))(expensive(x))
  ```
- **Status:** fixed — `do...end` blocks added. Write multi-statement lambdas as:
  ```cap
  diff = |x, y| do
    a = expensive(x)
    b = expensive(y)
    a - b
  end
  ```

---

## TOKEN EFFICIENCY FLAG: String Character Access
- **Issue:** Accessing a character by index requires creating a list of chars first.
- **Python:** `s[0]` (3 tokens)
- **Cap:** `s.chars[0]` (5 tokens)
- **Status:** fixed — `s[0]` now works directly (integer indexing on strings in `eval_index`).

## BUG-8: Inconsistency in `.len` property for tuples
- **Status:** fixed (already handled in interpreter — `eval_field_access` returns `len` for tuples)
- **File:** tests/test_tuple_single.cap (line 9)
- **Expected:** `(1,).len` should return 1, identical to lists and strings.
- **Actual:** `RuntimeError: cannot access field len on tuple`
- **Minimal repro:**
  ```cap
  t = (1, 2)
  println(t.len)
  ```

## BUG-9: Binary operator `+` type error reports left operand type instead of right operand type
- **Status:** fixed (Add error message already shows both types: `got int and null`)
- **File:** tests/test_type_error.cap (line 1)
- **Expected:** `1 + null` should report that it expected a number or string but got `null`.
- **Actual:** `TypeError: expected number or str, got int` - incorrectly prints `got int` (which is the valid left operand).
- **Minimal repro:**
  ```cap
  1 + null
  ```

## BUG-10: Function calls with missing arguments silently assign `null` instead of throwing an arity error
- **Status:** fixed
- **Fix:** `src/interpreter/mod.rs` `call_function` — counts required params and raises `TooFewArgs` if `args.len() < required`.
- **File:** tests/test_args_count.cap
- **Expected:** Calling a function requiring 2 arguments with only 1 argument should throw an `ArgumentCountError` or similar (`Expected 2 arguments, got 1`).
- **Actual:** The missing argument is passed as `null`, leading to confusing downstream errors like `TypeError: expected number or str, got int and null` when applying standard operations.
- **Minimal repro:**
  ```cap
  add = |a, b| a + b
  add(1)
  ```

## BUG-11: Dynamic population of maps/lists inside loops is syntactically impossible
- **Status:** fixed
- **Fix:** Added `set(collection, key, val)`, `from_pairs([(k,v),...])`, and `merge(base, overlay)` stdlib functions. Use `set(m, k, v)` inside closures instead of `m[k] = v`.
- **File:** tests/test_map_build.cap
- **Expected:** The language should provide a way to build a map from a list of key-value pairs (e.g. `items |> each(|t| m[t[0]] = t[1])` or a `from_pairs()` stdlib func).
- **Actual:** Since assignment `m[k] = v` is strictly a top-level statement and not an expression, it causes `SyntaxError` inside closures. With no `from_pairs` stdlib function, there is no way to dynamically build a map with arbitrary keys at runtime inside an iterator.
- **Minimal repro:**
  ```cap
  m = {}
  [("a", 1)] |> each(|t| m[t[0]] = t[1]) # SyntaxError: unexpected =
  ```

## TOKEN EFFICIENCY FLAG: List Slicing
- **Issue:** The language does not support slicing lists using range syntax (e.g. `list[1..5]`). Because ranges evaluate to lists natively and the indexing operator purely expects integers, writing `list[1..5]` throws a `TypeError`.
- **Python:** `items[1:]` (4 tokens)
- **Cap:** `1..items.len |> map(|i| items[i])` (15+ tokens)
- **Status:** fixed — `list[1..5]` and `str[1..5]` now work natively via range-based indexing in `eval_index`.

## BUG-12: Interpolation parsing silently drops unconsumed tokens inside `{}` blocks
- **Status:** fixed
- **Fix:** `src/interpreter/mod.rs` `eval_interp_str` — after parsing the expression, calls `parser.is_at_eof()`; errors if tokens remain.
- **File:** tests/test_brace_parsing.cap
- **Expected:** In `s = "{" + "xyz" + "}"`, if `{` is evaluated as an interpolation block, the parser should process everything until `}`. If it finds `" + "xyz" + "` surrounded by `{}`, the parser should process `" + "` (string), then fail on `"xyz"` as consecutive string literals with no operator, or complain about leftover tokens.
- **Actual:** `parse_expr_impl` inside interpolations stops after parsing the first valid sub-expression (a string literal here: `" + "`) and silently drops the rest (`xyz" + "`). Thus `s` evaluates merely to the string `" + "` without any syntax error.
- **Minimal repro:**
  ```cap
  s = "{" + "dropped" + "}"
  println(s) # Outputs " + "
  ```

## BUG-13: Missing `try` or any error recovery mechanism
- **Status:** fixed
- **Fix:** `src/interpreter/mod.rs` `call_builtin_str` — intercepts `"try"`, calls the lambda, wraps result in `{ok: true, value: ...}` or `{ok: false, error: ...}`.
- **File:** tests/test_try.cap
- **Expected:** As documented in `RESEARCH.md`, the language should support `try(fn)` to capture runtime errors (such as `read`ing a missing file) and return an `{ok, value/error}` tuple. 
- **Actual:** `try` is not defined in the standard library. As a result, any runtime error (like `IOError: No such file or directory`) unrecoverably panics the script with no way to catch or handle it programmatically.
- **Minimal repro:**
  ```cap
  try(|| read("missing.txt")) # NameError: `try` is not defined
  ```

## BUG-14: No support for class inheritance (`extends`)
- **Status:** fixed
- **Fix:** `src/parser/mod.rs` `parse_class_def` — recognizes `extends ClassName(args)` after params; desugars to `merge(Base(args), {methods})`. Added `merge(base, overlay)` stdlib function.
- **File:** tests/test_class_inheritance.cap
- **Expected:** A token-efficient scripting language should typically provide an `extends` keyword or some prototype delegation mechanism to avoid duplicating methods across classes.
- **Actual:** The `class` keyword purely desugars to a map-returning lambda. Attempting to use `class Derived(y) extends Base(y)` results in a `SyntaxError: unexpected extends`. 
- **Minimal repro:**
  ```cap
  class Derived(y) extends Base(y), str = || "derived"
  ```

## BUG-15: Fatal Stack Overflow on deep mutual recursion (Brainfuck stress test)
- **Status:** fixed
- **Fix:** Two-part fix: (1) `src/main.rs` — spawns main logic on a 64 MB stack thread; (2) `tests/test_brainfuck.cap` — corrected `find_open`'s off-by-one: `run_step(temp_ip + 1, ...)` instead of `run_step(temp_ip, ...)` so the `]` handler lands on the matching `[` (not one before it), preventing accidental infinite recursion.
- **File:** tests/test_brainfuck.cap
- **Expected:** A language that forces standard loops (`while`/`for`) to be written as pure recursion should have tail-call optimization (TCO) or a drastically increased stack limit, otherwise flat iterative algorithms like interpreting an instruction tape will crash the host.
- **Actual:** Evaluating a 63-step string of Brainfuck instructions (`+++++[>++++++++<-]>.`) using a mutually recursive `run_step` matches function caused a host crash: `thread 'main' has overflowed its stack / fatal runtime error: stack overflow, aborting`.
- **Minimal repro:**
  See `tests/test_brainfuck.cap`.

## BUG-16: Tuples cannot be used as map keys
- **Status:** fixed
- **Fix:** `src/interpreter/value.rs` — added `MapKey::Tuple(Vec<MapKey>)` variant and `Value::Tuple` arm in `to_map_key()` that recursively converts each element; also added `Display` for `MapKey::Tuple`.
- **File:** tests/test_map_keys_types.cap
- **Expected:** Since tuples are completely immutable, they should be valid keys in maps (similar to Python) to allow coordinates or pair lookups like `m[(1, 2)]`.
- **Actual:** `TypeError: tuple cannot be used as a map key`. Only int, str, and bool seem to be accepted.
- **Minimal repro:**
  ```cap
  m = {(1, 2): "nested"}
  ```

## BUG-17: Modulo by zero causes fatal Rust panic
- **Status:** fixed
- **Fix:** `src/interpreter/mod.rs` `BinOp::Mod` — added `(Value::Int(_), Value::Int(0))` guard that returns `CapError::Runtime { message: "modulo by zero" }` before the `a % b` operation.
- **File:** tests/test_math_edges.cap
- **Expected:** `1 % 0` should throw a handled Cap `RuntimeError` (like `1 / 0` correctly does: `RuntimeError: division by zero`).
- **Actual:** The binary crashes with a Rust thread panic: `thread 'main' panicked at src/interpreter/mod.rs:552:65: attempt to calculate the remainder with a divisor of zero`.
- **Minimal repro:**
  ```cap
  1 % 0
  ```


## BUG-18: Pattern guards parsed but never evaluated
- **Status:** fixed
- **Fix:** (1) `src/parser/mod.rs` `parse_pattern()` — added `if <expr>` guard parsing after pattern; (2) `src/interpreter/mod.rs` `ExprKind::Match` — after binding pattern variables, evaluates the guard in that scope; if falsy, pops scope and continues to next arm.
- **Expected:** `match n, x if x < 0 -> "negative", _ -> "other"` evaluates the guard.
- **Actual:** Guards were defined in the AST but the parser never emitted them (syntax error on `if`) and the interpreter ignored them.
- **Minimal repro:**
  ```cap
  classify = |n| match n, x if x < 0 -> "negative", _ -> "non-negative"
  println(classify(-1))  # negative
  ```

## BUG-19: Float modulo not supported
- **Status:** fixed
- **Fix:** `src/interpreter/mod.rs` `BinOp::Mod` — added float/float, int/float, float/int arms with zero-guard.
- **Expected:** `3.5 % 2.0` returns `1.5`.
- **Actual:** `TypeError: expected int` on any float operand.

## BUG-25: `range(a, b, 0)` silently returns empty list instead of erroring
- **Status:** fixed
- **Fix:** `src/interpreter/stdlib/core.rs` `"range"` — added `if *step == 0 { return Err(...) }` before the loop.
- **Expected:** `range(0, 10, 0)` throws `RuntimeError: range() step cannot be zero`.
- **Actual:** Returned `[]` silently.

## BUG-32: `reduce()` on empty list without initial value returns null
- **Status:** fixed
- **Fix:** `src/interpreter/stdlib/list.rs` `"reduce"` — changed `unwrap_or(Value::Null)` to `ok_or_else(|| CapError::Runtime { ... })?`.
- **Expected:** `[].reduce(|a, b| a + b)` throws `RuntimeError: reduce() on empty list requires an initial value`.
- **Actual:** Returned `null` silently.

---

## BUG-33: Match OR pattern only accepts `|`, not the `or` keyword
- **Status:** fixed
- **Fix:** `src/parser/mod.rs` `parse_pattern()` — OR separator check now accepts both `TokenKind::Pipe` and `TokenKind::Or`, so `"a" or "b" -> x` and `"a" | "b" -> x` are both valid.
- **Expected:** `match x, "apple" or "pear" -> "pome", _ -> "other"` works
- **Actual:** `SyntaxError: unexpected 'or', expected '->'`
