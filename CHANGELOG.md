# Serez-Code — Changelog

Technical record of all changes to the language, stdlib, and tooling.  
Order: most recent to oldest.

---

## [Unreleased] — branch `improve`

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
