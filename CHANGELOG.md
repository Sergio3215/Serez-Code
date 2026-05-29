# Serez-Code — Changelog

Technical record of all changes to the language, stdlib, and tooling.  
Order: most recent to oldest.

---

## [3.8.3] — branch `improve`

### Bug fixes

- **B-84** — Parenthesized single-parameter arrow lambda failed to parse. `(x => body)` raised `Expected ')' in grouped expression`, even though `(x) => body`, bare `x => body`, and `(a, b) => body` all parsed. After consuming `(` and a leading identifier, the parser matched `,` (multi-param), `)` (`(a)`/`(a) => …`) and a catch-all that assumed a grouped expression — so a following `=>` (Arrow) was never recognized. Added an explicit `Arrow` arm that parses `( ident => body )` as a parenthesized single-param lambda. This unblocks common forms like `5 |> (x => x * 2)`, `((x => x + 1))(5)`, and `let f = (x => …)`. New regression: `unit_paren_lambda` (6 cases). Found while fuzzing pipe/lambda syntax.

### Test count

- 306 passing (0 failing) — added `unit_paren_lambda`.

---

## [3.8.2] — branch `improve`

### Bug fixes

- **B-83** — Inconsistent lambda capture: scope-dependent snapshot vs. live reference. Lambdas snapshot scoped locals (`capture_env` extracts + plants them to the global arena at creation), but variables referenced from a lambda that live in the **global** arena (top-level `let`s) were resolved *live* at call time. So the exact same lambda captured locals by value but globals by reference, depending only on where it was written: `let x=10; let f=()=>x; x=20; f()` gave `20` at top level but `10` inside a function; `while (i<3){ fns.push(()=>i); i=i+1 }` gave `3 3 3` at top level but `0 1 2` inside a function. Fixed with `capture_lambda_env`: in addition to the existing local snapshot, a best-effort free-identifier walk of the lambda body now also snapshots referenced **global data variables** at creation. Global **functions** are intentionally skipped (kept live) so recursion and late binding keep working. The walk only ever *adds* snapshots — an unhandled construct simply degrades to the previous live-lookup behavior, so it cannot break a valid closure (the whole suite is unchanged). New regression: `45_closure_capture_e2e`.

### Test count

- 305 passing (0 failing) — added `45_closure_capture_e2e`.

---

## [3.8.1] — branch `improve`

### Bug fixes

- **B-82** — Nested arrays corrupted when reassigning an outer-scope variable from inside a nested block. The shared scoped arena is a single stack rewound on block exit (`pop` → `reset_to`). A plain variable assignment (`x = value`) stored a *shallow* clone of the value's `ObjectData`: for an array/dict/set it copied the inner `ObjectRef`s, which could point into a deeper block's region. When that inner block popped, the inner refs dangled — the container's `.length()` stayed correct but indexing an element read a truncated/reused slot (symptom: `is array` == false, "Index operator not supported"). `push`/index-assign/dict-value-assign already promoted to the global arena at `depth > 1`, but plain variable assignment was missed. Fixed by `promote_container_for_assign`: when assigning a heap container (Array/Dict/Set) to a variable from inside a nested scope, the value is deep-promoted to the global arena so its elements outlive inner-block pops. Scalars and instances (fields are `OwnedValue`) are untouched — no effect on loop counters like `i = i + 1`. Found while building serez-ui's `.szs` CSS parser. New regression: `unit_nested_array_assign` (4 cases).

### Test count

- 303 passing (0 failing) — added `unit_nested_array_assign`.

---

## [2.1.0] — branch `improve`

### New features

**Fase 1 — Memory namespace: raw byte heap**

- `Memory` namespace: `sizeof`, `alloc`, `free`, `size`, `read`, `write`, `copy`, `fill`, `offsetOf`.
- `Memory.sizeof(type)` — returns byte-size of a primitive type name (`"int"`, `"bool"`, `"float32"`, etc.).
- `Memory.alloc(n)` → int handle — allocates `n` bytes of zeroed memory in a `HashMap<i64, Vec<u8>>` heap stored on the evaluator; requires `unsafe {}` block.
- `Memory.read(handle, offset, type)` / `Memory.write(handle, offset, type, value)` — typed read/write at a byte offset; require `unsafe {}`.
- `Memory.copy(src, dst, n)` — copies `n` bytes between two allocations; requires `unsafe {}`.
- `Memory.fill(handle, byte)` — fills an entire allocation with a byte value; requires `unsafe {}`.
- `Memory.offsetOf(class_name, field_name)` — returns word-aligned field offset (8-byte stride) by looking up the class registry.
- New evaluator fields: `memory_heap: HashMap<i64, Vec<u8>>`, `memory_heap_next_id: i64`.
- New source file: `src/evaluator/namespaces_memory.rs`.

**Fase 1.5 — unsafe as expression + new built-in globals**

- `unsafe { ... }` can now be used as an expression, enabling patterns like `let h = unsafe { Memory.alloc(64) }`. AST: `Expression::UnsafeBlock(BlockStatement)`. Parser: expression-level dispatch in `parse_expression`. Evaluator: delegates to `eval_unsafe_block`.
- `time()` built-in — returns current Unix timestamp in milliseconds as `int`.
- `env(name)` built-in — reads an environment variable by name; returns empty string if not set.
- `exit(code)` built-in — terminates the process with the given exit code (`std::process::exit`).
- `native fn` dispatch: when a declared native function is called but has no Rust implementation registered, a clear error is now printed.

**Fase 2 — Extended Tensor math**

- **Activation functions** (element-wise, return new Tensor): `relu`, `sigmoid`, `tanh`, `softmax`.
- **Element-wise math**: `abs`, `sqrt`, `exp`, `log`, `pow(exp)`.
- **Norms**: `norm()` (L2, default) / `norm(1)` (L1) — returns a Decimal.
- **Clamp**: `clamp(min, max)` — clips all elements to `[min, max]`.
- **Broadcast add**: `broadcastAdd(bias)` — adds a 1D tensor to each row of a 2D tensor `(m, n) + (n,)`.

### Bug fixes

- **B-75** — Keyword token as method name rejected by class parser: methods named `get`, `set`, or `static` (lexed as `KwGet`/`KwSet`/`KwStatic`) were unconditionally rejected by the `Ident`-only check in `parse_class_declaration`. Fixed by extracting `token_type_is_name()` helper and using `current_token_is_name()` at the method-name check point.
- **B-76** — `Tensor.sum()` on empty tensor returned `-0.0`: Rust's `Iterator::sum` initialises the accumulator with `0.0_f64` and produces negative zero on empty input. Fixed by adding an `is_empty()` early-return guard matching the pattern already used by `Tensor.mean()`.
- **B-65 assertion corrected** — `Math.round(-4.5)` returns `-5` (Rust "half away from zero"), not `-4`. Test expectation updated.
- **`unit_class_arch` assertion corrected** — `pts.find(p => p.sum() > 6)` returns the first match (x=3), not the last (x=5). Test expectation updated.

### New parser feature

- **Enum.Variant in match patterns** — `match dir { case Direction.North => ... }` now works. The parser detects `Ident.Ident` in match position and creates a `MatchPattern::Literal(DotCall)`, evaluated at runtime by the existing literal-pattern path.

### Test count

- 274 passing (0 failing) — added: `unit_memory`, `unit_native`, `unit_tensor_math`, `56_memory_e2e`, `57_tensor_math_e2e`, `unit_match_enum`, `unit_bug_b64_b74`, `unit_math_trig`, `unit_memory_offsetof`, `unit_tensor_ops`, `unit_set_ops` (extended), `unit_bug_b75_b76`, `unit_class_arch` (extended), `sec_memory_requires_unsafe`, `sec_memory_write_requires_unsafe`, `sec_memory_read_requires_unsafe`, `sec_memory_free_requires_unsafe`, `sec_json_invalid`, `59_integral2_e2e`.

---

## [2.0.2] — branch `improve`

### New features

**Fase 2.5 — serez-sec: Socket and Binary namespaces**

- `Socket` namespace: `connect`, `send`, `recv`, `close`, `listen`, `accept` — raw TCP over `std::net::TcpStream` / `TcpListener`. Socket IDs (int) stored in the evaluator's registry; usable from Serez code as `Socket.connect("host", port)`.
- `Binary` namespace: byte-array utilities — `fromHex`, `toHex`, `fromUtf8`, `toUtf8`, `packInt32Le`, `packInt32Be`, `unpackInt32Le`, `unpackInt32Be`, `packInt64Le`, `unpackInt64Le`, `concat`. All operate on Serez integer arrays (values 0–255).
- Tests: `tests/53_socket_e2e.sz`, `tests/unit_binary.sz`, `tests/unit_socket.sz` (42 new test cases).

**Fase 4 — GPU compute (CPU-backed)**

- `GPU` namespace: `createBuffer`, `createBufferFromArray`, `readBuffer`, `freeBuffer`, `fill`, `size`, `map`, `reduce`, `dot`, `axpy`, `matmul`. Buffers are flat `Vec<f64>` stored in the evaluator. API mirrors GPU compute patterns (create/upload/dispatch/readback/free) so a future backend can swap to real GPU calls with no language changes.
- Tests: `tests/54_gpu_e2e.sz`, `tests/unit_gpu.sz` (13 new test cases).

**Fase 6 — Package manager**

- `src/package_manager.rs`: `SerezManifest` JSON parser (hand-rolled, no external crate), `install_package(spec)`, `install_all()`, `packages_dir()` / `registry_dir()` (support `SEREZ_PACKAGES` / `SEREZ_REGISTRY` env vars for testing).
- `sz install [pkg@version]` CLI subcommand: without argument reads `serez.json` and installs all dependencies; with argument installs a specific package from the registry.
- Import resolution now searches `packages_dir()` (and falls back to `~/.serez/packages/`) after all existing search paths. Also supports `<pkg>/index.sz` layout so `import "pkg-name"` resolves to `packages/pkg-name/index.sz`.
- `run_tests.ps1` / `run_tests.sh`: set `SEREZ_PACKAGES=tests/packages` so package tests run correctly against local test packages.
- Tests: `tests/55_packages_e2e.sz`, `tests/unit_packages.sz` (13 new test cases). Test packages: `tests/packages/math-helpers/`, `tests/packages/string-tools/`.
- Rust unit tests in `package_manager.rs` verify manifest parsing and pkg-spec parsing.

### Test count

- 214 → 256 passing (0 failing).

---

## [2.0.1] — branch `improve`

### Bug fixes

**B-64 — `abs(i64::MIN)` overflow** (`src/evaluator/builtins.rs`)
- Before: called `.abs()` on `i64::MIN` — overflows in release mode (|i64::MIN| > i64::MAX).
- Now: uses `i64::checked_abs()` — returns an error for `i64::MIN`.

**B-65 — `floor` / `ceil` / `round` / `trunc` UB on non-finite f64** (`src/evaluator/builtins.rs`)
- Before: casting `f64::INFINITY`, `f64::NEG_INFINITY`, or `f64::NAN` to `i64` via `as i64` is undefined behavior in Rust.
- Now: each function validates `!v.is_nan() && !v.is_infinite()` before casting.

**B-66 — `Math.random()` only produced values in `[0, ~0.5)`** (`src/evaluator/namespaces.rs`)
- Before: LCG state shifted right 33 bits (31-bit range `[0, 2³¹)`) divided by `u32::MAX` (2³²−1) — maximum ≈ 0.5.
- Now: divides by `1u64 << 31` to produce the documented `[0, 1.0)` range.

**B-67 — `asin` / `acos` accepted out-of-domain arguments** (`src/evaluator/builtins.rs`)
- Before: any `f64` was accepted — inputs outside `[-1, 1]` silently produced `NaN`.
- Now: validates `v >= -1.0 && v <= 1.0` before calling the intrinsic.

**B-68 — `JSON.stringify` emitted invalid JSON for `NaN` / `Infinity`** (`src/evaluator/mod.rs`)
- Before: non-finite `f64` values were formatted with Rust's `Display`, producing `"inf"`, `"-inf"`, or `"NaN"`.
- Now: `if !d.is_finite() { return "null".to_string(); }` per the JSON specification.

**B-69 — `call_function` (map / filter / sort callbacks) rejected default and rest parameters** (`src/evaluator/mod.rs`)
- Before: arity checked as `arg_count != params.len()` and parameters bound via `args[i]` direct indexing.
- Now: computes `required_count`, checks `arg_count >= required` with upper bound for non-rest, binds defaults and collects rest parameter into an array.

**B-70 — `min_params` formula wrong for functions with default + rest parameters** (`src/evaluator/expr.rs`)
- Before: `if has_rest { params.len() - 1 } else { required_count }` — gives wrong count when both rest and defaults are present.
- Now: `let min_params = required_count` in all cases.

**B-71 — `super()` constructor call rejected default and rest parameters** (`src/evaluator/classes.rs`)
- Before: `eval_super_call` used strict arity and `args[i]` direct indexing.
- Now: same default/rest parameter handling as `call_function`.

**B-72 — `new ClassName()` constructor call rejected default and rest parameters** (`src/evaluator/classes.rs`)
- Before: `eval_new_class` used strict arity and direct indexing for constructor binding.
- Now: same default/rest parameter handling.

**B-73 — `super.method()` call rejected default and rest parameters** (`src/evaluator/classes.rs`)
- Before: `eval_super_method_call` used strict arity.
- Now: same default/rest parameter handling.

**B-74 — `invoke_method` rest parameter not collected** (`src/evaluator/classes.rs`)
- Before: parameter binding loop did not handle rest parameters — extra arguments beyond the last named param were silently discarded.
- Now: rest parameter is collected from `args[i..]` into an `Array` and declared in scope.

### Version

- `Cargo.toml`: `2.0.0` → `2.0.1`

---

## [2.0.0] — branch `improve`

### Breaking changes

**`pop()` on empty array is now a runtime error (Bug 1)**
- Before: returned `null` silently
- Now: `❌ ERROR: pop() called on an empty array`
- Rationale: silent null masked logic bugs where callers expected a real value

**`shift()` on empty array is now a runtime error (Bug 2)**
- Before: returned `null` silently
- Now: `❌ ERROR: shift() called on an empty array`
- Rationale: same as pop() — silent null was undetectable

**`2 ** 63` and exponent overflow are now runtime errors (Bug 3)**
- Before: f64 precision caused `2 ** 63` to silently return `i64::MAX` instead of detecting overflow
- Now: uses `i64::checked_pow` — exact overflow detection with no floating-point rounding
- Now: `❌ ERROR: Integer overflow in exponentiation`
- Base 0, 1, -1 at any exponent are still handled correctly (no overflow possible)
- Decimal exponent path (`2 ** 63.0`) is unchanged — goes through `f64::powf`

**Typed dict missing key is now a runtime error (Bug 4)**
- Before: `d["missing"]` on a `<K, V>` dict (V ≠ `any`) silently returned `null`
- Now: `❌ ERROR: Key 'missing' not found in typed dict <_, V>`
- Untyped dicts (`<K, any>`) still return `null` for missing keys — no change

### Distribution

- **Release pipeline**: GitHub Actions workflow builds binaries for Windows x64, Linux x64 (static musl), macOS ARM64, macOS x64 on every version tag and publishes them to GitHub Releases
- **`install.sh`**: one-line installer for Linux and macOS — auto-detects OS and arch, installs to `~/.local/bin/sz`
- **`install.ps1`**: one-line installer for Windows — downloads to `%LOCALAPPDATA%\SerezCode\bin\sz.exe` and adds to user PATH
- **CI workflow** (`ci.yml`): builds on `main` and `integration` on every push and pull request

### Tests (214 total, 0 failures)

- `41_bug_fixes_e2e.sz` — E2E integration test covering all 4 bug fixes (Queue, SafeStack, safePow2, Registry, game loop)
- `unit_bug_fixes.sz` — 21 unit tests for positive regression across all 4 fixes
- `sec_pop_empty_array.sz`, `sec_shift_empty_array.sz`, `sec_typed_dict_miss_key.sz`, `sec_power_2_63.sz` — security tests verifying each fix produces the correct error
- `unit_sec_pentest_bugs.sz` — 16 penetration tests with boundary exhaustion, alternating cycles, power edge cases, dict key patterns
- `run_tests.ps1` — new `-cli` flag runs 12 tests covering CLI flags (`--version`, unknown flags, non-.sz), REPL behavior (arithmetic, variable persistence, function definition, error recovery), and `--check` mode output

### Native backend (foundation — not yet connected to runtime)

- `src/compiler/types.rs` — compile-time type system (`SzType`) mapping Serez types to LLVM types
- `src/compiler/hir.rs` + `hir_lower.rs` — AST → HIR lowering with full desugar pass
- `src/compiler/mir.rs` + `mir_lower.rs` — HIR → MIR three-address code with basic blocks
- `src/compiler/llvm_emit.rs` — MIR → LLVM IR text emission (74 tests passing)

---

## [1.0.0] — VS Code formatter and CI

### VS Code — Formatter (`vscode-serez` v0.2.0)

**`extension.js`** — new `DocumentFormattingEditProvider`:
- Auto-indentation with 4 spaces per level, based on `{` and `}` counting
- Ignores braces inside string literals and line comments (`//`)
- `} else {` handled correctly: dedent before printing, indent after
- Collapses consecutive blank lines into one
- Removes trailing whitespace from all lines
- File always ends with exactly one `\n`

**`package.json`** — version `0.2.0`:
- `"main": "./extension.js"` and `"activationEvents": ["onLanguage:serez"]`
- `Formatters` category added
- `configurationDefaults` for `.sz`: `editor.defaultFormatter` and `editor.formatOnSave: true` enabled automatically

**Usage:** `Shift+Alt+F` to format manually, or save the file (formatOnSave).  
**Rebuild:** `vsce package` in `vscode-serez/` generates `serez-code-0.2.0.vsix`.

---

### CI / Tooling
- `release.yml`: permissions scoped per job — only `host` has `contents: write`; others have `contents: read`
- `.github/dependabot.yml`: automatic weekly updates for GitHub Actions and Cargo dependencies
- `run_tests.sh`: Bash script equivalent to `run_tests.ps1`, with `--filter`, `--generate`, `--unit`, `--e2e`, `--security` flags; ANSI colors; CRLF normalization; unique temp files per process
- Evaluator refactored from a single `evaluator.rs` (5300+ lines) to 12 submodules:

| Module | Responsibility |
|---|---|
| `mod.rs` | Main entry, Flash Scope protocol, StoredMethod cache, static profiler |
| `stmt.rs` | Statement evaluation (let, assign, for, while, return, …) |
| `expr.rs` | Expression evaluation (calls, index, dot, ternary, …) |
| `ops.rs` | Infix and prefix operators |
| `check.rs` | Type-check helpers (parameters, return, typed arrays) |
| `builtins.rs` | Global functions (parseInt, parseDecimal, readLine, …) |
| `classes.rs` | Instantiation, method dispatch, inheritance, super |
| `methods_array.rs` | Array methods (push, pop, map, filter, reduce, sort, …) |
| `methods_string.rs` | String methods (split, replace, trim, padStart, …) |
| `methods_set.rs` | Set methods (add, has, delete, toArray, union, …) |
| `namespaces.rs` | Built-in namespaces (Math, File, JSON) |
| `control.rs` | Control flow helpers (break, continue, labeled loops, do-while) |

### Demo apps
- `apps/01_task_manager.sz` — enum, inheritance, static methods, switch, HOF, try/catch
- `apps/02_statistics.sz` — typed arrays, Math, map/filter/reduce, Pearson correlation
- `apps/03_text_analyzer.sz` — string methods, dicts, Caesar cipher, File I/O
- `apps/04_bank_system.sz` — abstract class, sealed, interface, const, getters, optional chaining
- `apps/05_data_pipeline.sz` — JSON, File, Set, bitwise/power ops, pipeline HOF

---

## [0.1.0] — Language history

### Phase 5 — Bug fixes and semantics (B-62 to B-63)

**`reverse()` — in-place mutation with return (B-62)**
- Before: `reverse()` returned void, was not chainable
- Now: mutates the array in-place AND returns the same array — allows `let sorted = arr.reverse()`

**`trimLeft` / `trimRight` as aliases (B-63)**
- Added as aliases for `trimStart` / `trimEnd` for compatibility

---

### Phase 4 — Critical bug fixes (B-54 to B-61)

**`is` operator — full fix (B-61)**
- Bug: `is` was tokenized as an identifier, never worked as an infix operator
- Fix: `KwIs` token added; registered in `token_precedence()` and in the parser's `is_infix` match; `eval_infix` handler added in the evaluator
- `null is null` also fixed: missing case `("null", ObjectData::Null)` in `type_matches`

**Named function capture semantics (B-58)**
- Before: `fn` declarations captured the value at definition time (snapshot)
- Now: `fn` declarations use reference semantics — rebind of the shared global slot
- Lambdas maintain snapshot semantics (no changes)
- `ScopeStack::rebind()` added for selective rebinding of outer scope

**Dict mutation from nested scope (B-57)**
- Bug: arena lifetime — a new entry in a dict mutated from inside a function stayed in the local scope and was destroyed on exit
- Fix: `plant_global` used when `depth > 1`

**`padStart` / `padEnd` — incorrect early return (B-56)**
- Bug: if the string already had the target length, it returned empty instead of returning the original string
- Fix: early return corrected

**Shift validation (B-55)**
- `1 << 64` and `8 >> -1` were silently incorrect
- Now they are runtime errors: negative or ≥ 64 shift throws an error

**`flat(n)` — depth parameter (B-54)**
- Before: only supported `flat()` with depth 1
- Now: `flat(n)` recursively flattens `n` levels; `flat()` is equivalent to `flat(1)`

**Getter-only — write error (B-53)**
- Attempting to assign to a property that only has `get` (without `set`) is now a runtime error

---

### Phase 3 — New language features

#### Operators

**Power operator `**`**
- `2 ** 10` → `1024`; works with `int` and `decimal`
- Higher precedence than `*` / `/` / `%`
- `0 ** 0` → `1` (mathematical convention)

**Bitwise operators**
- `&` AND, `|` OR, `^` XOR, `~` NOT (prefix), `<<` left shift, `>>` arithmetic right shift
- Only for `int` (64-bit signed, two's complement)
- Negative or ≥ 64 shift is a runtime error
- Binary (`0b1010`) and hexadecimal (`0xFF`) literals supported
- Numeric separators: `1_000_000`, `0xFF_FF`

**Optional chaining `?.`**
- `obj?.method()` / `obj?.field` — if `obj` is `null`, returns `null` without error
- Chainable: `a?.getNext()?.getValue() ?? 0`
- Combinable with `??` for fallback

#### Control flow

**`do-while`**
- The body executes at least once
- `break` and `continue` work the same as in `while`/`for`

#### Classes

**Static methods**
- `public static T method(args)` in classes
- Called as `ClassName.method(args)` — no instance required
- No access to `this`

**Parameters with default values**
- `fn int add(int a, int b = 10)` — if the caller omits the argument, the default is used
- The default is an arbitrary expression evaluated at call time
- The type checker handles variable arity (skip if there are defaults)

**Abstract classes**
- `abstract class Foo` — not directly instantiable; runtime error on `new`
- Methods without a body declared for override in subclasses

**Sealed classes**
- `sealed class Foo` — not inheritable; attempting to extend it is a runtime error

**Getters and setters**
- `public get T prop()` — called automatically when reading `obj.prop` (without parentheses)
- `public set prop(T val)` — called automatically when assigning `obj.prop = val`
- Property with only getter is read-only; writing to it is a runtime error

**Class fields with default values**
- `field: type = value` in the class body

#### Arrays — new methods

| Method | Description |
|---|---|
| `.find(cb)` | First element where `cb` returns `true`, or `null` |
| `.findIndex(cb)` | Index of the first element matching the predicate, or `-1` |
| `.every(cb)` | `true` if `cb` is `true` for all elements |
| `.some(cb)` | `true` if `cb` is `true` for at least one |
| `.slice(start, end)` | New array from `start` (inclusive) to `end` (exclusive) |
| `.flat(n?)` | Flattens `n` nesting levels (default 1) |
| `.reverse()` | Reverses in-place, returns the same array |
| `.indexOf(val)` | Index of the first occurrence, or `-1` |
| `.includes(val)` | `true` if the array contains the value |
| `.remove(idx)` | Removes and returns the element at `idx` |

#### Strings — new methods

| Method | Description |
|---|---|
| `.padStart(n, ch?)` | Pads the start with `ch` (default space) up to length `n` |
| `.padEnd(n, ch?)` | Pads the end with `ch` (default space) up to length `n` |
| `.slice(start, end?)` | Substring with negative index support |
| `.trimStart()` / `.trimLeft()` | Removes leading whitespace |
| `.trimEnd()` / `.trimRight()` | Removes trailing whitespace |
| `.toUpperCase()` / `.upper()` | Uppercase copy |
| `.toLowerCase()` / `.lower()` | Lowercase copy |
| `.startsWith(prefix)` | `true` if the string starts with `prefix` |
| `.endsWith(suffix)` | `true` if the string ends with `suffix` |
| `.charAt(i)` | Character at position `i`, or `""` if out of range |
| `.indexOf(sub)` | Index of first occurrence of `sub`, or `-1` |
| `.replace(from, to)` | Replaces **all** occurrences (previously only the first) |

---

### Phase 2 — Stdlib and compound types

#### `const`
- `const PI = 3.14159` — immutable; any reassignment is a runtime error
- Same scoping as `let` — invisible outside its block

#### `enum`
- `enum Color { Red, Green, Blue }` — variants accessed as `Color.Red`
- Variants are their own type (not `string`) — do not annotate enum parameters as `string`
- Comparable with `==` and usable in `switch case`
- Displayed as `"Color.Red"` (fully qualified name)

#### Labeled loops
- `outer: for (...)` + `break outer` / `continue outer`
- Works with `while`, `for`, `for-in`, `do-while`

#### Spread and rest
- Spread in array literals: `[...arr, 1, 2]`
- Spread in calls: `fn(...args)`
- Rest params: `fn void log(...args)` — `args` is an array with all extra arguments
- The type checker skips arity checks for functions with rest params

#### Namespace `Math`

| Function/Constant | Description |
|---|---|
| `Math.PI`, `Math.E` | Mathematical constants |
| `Math.abs(x)` | Absolute value |
| `Math.floor(x)`, `Math.ceil(x)`, `Math.round(x)`, `Math.trunc(x)` | Rounding (return `int`) |
| `Math.sqrt(x)` | Square root |
| `Math.pow(base, exp)` | Power |
| `Math.exp(x)`, `Math.log(x)`, `Math.log2(x)`, `Math.log10(x)` | Exponential and logarithms |
| `Math.sin(x)`, `Math.cos(x)`, `Math.tan(x)` | Trigonometric (radians) |
| `Math.asin(x)`, `Math.acos(x)`, `Math.atan(x)`, `Math.atan2(y, x)` | Inverse trigonometric |
| `Math.min(a, b, ...)`, `Math.max(a, b, ...)` | Variadic min/max |
| `Math.clamp(x, min, max)` | Clamp to range `[min, max]` |
| `Math.sign(x)` | Returns `1`, `0`, or `-1` |
| `Math.random()` | Pseudo-random decimal in `[0, 1)` (LCG) |

#### Namespace `File`

| Function | Description |
|---|---|
| `File.exists(path)` | `true` if the file exists |
| `File.read(path)` | File contents as `string` |
| `File.write(path, content)` | Writes/overwrites the file |
| `File.create(path)` | Creates empty file if not exists (touch, idempotent) |
| `File.read_asBinary(path)` | File bytes as `[int]` (0–255 each) |
| `File.write_asBinary(path, bytes)` | Writes byte array to file |

#### Namespace `JSON`

| Function | Description |
|---|---|
| `JSON.stringify(value)` | Serializes any value to a JSON string |
| `JSON.parse(string)` | Parses a JSON string; runtime error if invalid |

#### `Set` type

| Method/property | Description |
|---|---|
| `new Set()`, `new Set([...])` | Creates empty set or initialized from array (no duplicates) |
| `.size` | Element count (property, without parentheses) |
| `.add(val)` | Inserts `val` if not present (mutates in-place) |
| `.has(val)` / `.contains(val)` | `true` if the set contains `val` |
| `.delete(val)` / `.remove(val)` | Removes `val`, returns `true` if it existed |
| `.clear()` | Removes all elements |
| `.toArray()` | Returns all elements as an array |
| `.union(other)` | New set with all elements from both |
| `.intersection(other)` | New set with only elements present in both |

---

### Phase 1 — Language core

#### Variables and types
- `let x = value` — declaration; `x = value` — reassignment (without `let`)
- Primitive types: `int` (i64), `decimal` (f64), `bool`, `string`, `void`, `any`, `null`
- Compound types: array `[T]`, dict `<K,V>`, function, interface, class instance
- Nullable types: `int?`, `string?` — accept the base type or `null`
- Typed arrays: `let nums [int] = [1, 2, 3]` — type enforced on push, unshift, index-assign
- Type inference: `let x = add(1, 2)` infers `x: int` in the static checker

#### Operators
- Arithmetic: `+`, `-`, `*`, `/` (integer, truncates), `%`
- Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Logical: `&&`, `||`, `!` (short-circuit)
- Ternary: `cond ? then : else` (lazy, right-associative)
- Null coalescing: `a ?? b`
- `is`: `expr is TypeName` — `true`/`false` at runtime
- Compound assignment: `+=`, `-=`, `*=`, `/=`, `%=`
- Increment/decrement: `++`, `--` (prefix and postfix, as statements only)
- String repetition: `"ha" * 3` → `"hahaha"`
- Concatenation: `"x" + 42` → `"x42"`

#### Runtime safety
- Integer overflow: `checked_*` — error instead of silent wrap
- Division/modulus by zero: runtime error
- Out-of-range index: runtime error
- Undeclared variable: runtime error
- `return` outside a function: runtime error
- Stack overflow: runtime error (not catchable via try/catch)

#### Functions
- Declared: `fn returnType name(type param) { ... }`
- Arrow: `let f = returnType (type param) => { ... }`
- Anonymous: `let f = fn void () { ... }`
- First-class: assignable to variables, passable as arguments
- Recursive: supported with call stack in errors
- Lexical closures: capture variables from the scope where they are defined
- `fn` declarations: reference semantics (rebind of global slot)
- Lambdas (`x => expr`): snapshot semantics (capture by value)

#### Control flow
- `if` / `else if` / `else` — condition in parentheses, braces required
- `while` — condition in parentheses
- `for` — `for (let i = 0; i < n; i++)` — update accepts `i++`, `i--`, `i+=n`, etc.
- `for-in` — `for (let x in arr)` iterates array or string; `x` is a copy of the element
- `break` / `continue` — in all loops
- `switch` — no fall-through; `case a, b:` for multiple values; `default:`
- `try` / `catch(e)` / `finally` — `finally` always runs; `throw` accepts any value
- Standalone blocks `{ ... }` — create new Flash Scope

#### Arrays
- Literals: `[1, 2, 3]`, `[]`
- Index access: `arr[i]` (0-based)
- Index mutation: `arr[i] = val`
- Global mutation from function: `data[i] = val` persists; `this.arr[i] = val` persists
- **Limitation**: `for-in` creates a copy — mutating the loop variable does not affect the original array
- Mutation methods: `.push`, `.pop`, `.shift`, `.unshift`, `.reverse`, `.sort`, `.sort("desc")`, `.sort((a,b) => ...)`
- Query methods: `.length`, `.join`, `.map`, `.filter`, `.reduce`

#### Strings
- Interpolation: `"Hello {name}!"` — supports complex expressions inside `{}`
- `\{` for literal brace; `\"` inside `{...}` breaks the parser (use a variable)
- Escape sequences: `\n`, `\t`, `\r`, `\\`, `\"`, `\{`
- Methods: `.length`, `.substring`, `.split`, `.replace`, `.includes`, `.trim`, `.toString()`

#### Dictionaries
- `let d <string,int> = ({"a",1},{"b",2})`
- Access: `d["key"]` — returns `null` if the key does not exist (no error)
- Write: `d["key"] = val` or `d.Add({"key",val})`
- Methods: `.Add`, `.Remove`, `.RemoveAll`, `.clear`, `.toList`, `.toArray`

#### Classes and interfaces
- `interface Point { x: decimal, y: decimal }` — typed field record, no methods
- `class Foo { public Foo(args) { ... } }` — constructor + fields + methods
- Single inheritance: `class Bar : Foo { ... }`, `super(args)` in constructor
- `public` / `private` — `private` only accessible from methods of the same class
- Instance: `let obj = new Foo(args)`
- Field mutation: `obj.field = val`
- **Limitation**: `this.field[i].method()` inside a class method creates a copy — the result does not persist; use `this.field[i] = newValue` instead

#### Conversions and I/O
- `parseInt(val)` — converts to `int` (string, decimal, int)
- `parseDecimal(val)` — converts to `decimal` (string, int, decimal)
- `readLine(prompt?)` — reads a line from stdin
- `out expr` — prints to stdout with newline; statement, not function

#### Memory — Flash Scopes
- Two arenas: global (entire program) and scoped (local per block)
- Each `{ }` records a watermark on entry and truncates on exit — O(k) per scope
- Return values extracted as `OwnedValue` before the pop and replanted in the parent scope
- `Rc<BlockStatement>` for function bodies — cloning a function is O(1)
- `StoredMethod` in classes — O(1) dispatch without cloning the method body

#### Tooling
- `sz script.sz` — execute file
- `sz` — REPL
- `sz --check script.sz` — static profiler (byte estimation per function)
- `sz --watch script.sz` — automatic rerun on save
- `sz --version` — version
- Span errors: line + column + caret `^` in source
- VS Code extension: syntax highlighting for `.sz`
