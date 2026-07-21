<div align="center">

# ![](./img/sz-icon.svg) Serez-Code

**A hand-crafted interpreted programming language — written from scratch in Rust.**

No garbage collector. No heavy dependencies. Instant memory cleanup via **Flash Scopes**.

[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![No GC](https://img.shields.io/badge/memory-no%20GC-green?style=flat-square)]()

</div>

---

```serez
fn int fibonacci(int n) {
    if (n <= 1) {
        return n;
    }
    return fibonacci(n - 1) + fibonacci(n - 2);
}

out fibonacci(10);   // → 55
```

---

## Table of Contents

1. [Why Serez-Code?](#why-serez-code)
2. [Getting Started](#getting-started)
3. [Language Reference](#language-reference)
   - [Variables](#variables)
   - [Types](#types)
   - [Operators](#operators)
   - [Functions](#functions)
   - [Control Flow](#control-flow)
   - [Arrays](#arrays)
   - [String Methods](#string-methods)
   - [Dictionaries](#dictionaries)
   - [Higher-Order Functions](#higher-order-functions)
   - [Enums](#enums)
   - [Set](#set)
   - [Math](#math)
   - [File](#file)
   - [JSON](#json)
   - [Networking (fetch)](#networking-fetch)
   - [Socket (TCP & WebSocket)](#socket-tcp--websocket)
   - [GPU](#gpu)
   - [Crypto](#crypto)
   - [Autodiff & Tensors](#autodiff--tensors)
   - [Terminal](#terminal)
   - [OS](#os)
   - [Env](#env)
   - [Time](#time)
   - [DateTime](#datetime)
   - [System](#system)
   - [Permissions](#permissions)
   - [Tasks (Multithreading)](#tasks-multithreading)
   - [Package Manager](#package-manager)
   - [Classes & Interfaces](#classes--interfaces)
   - [Type Conversions](#type-conversions)
   - [Output](#output)
   - [Comments](#comments)
4. [Type System](#type-system)
5. [Runtime Safety](#runtime-safety)
6. [Flash Scopes — Memory Model](#flash-scopes--memory-model)
7. [Static Profiler](#static-profiler-check-mode)
8. [Error Reference](#error-reference)
9. [Architecture Overview](#architecture-overview)
10. [Demo Apps](#demo-apps)
11. [Known Gotchas](#known-gotchas)
12. [Contributing](#contributing)
13. [Roadmap](#roadmap)
14. [License](#license)
15. [Bugs Fixed List](bugs.md)

---

## Why Serez-Code?

Most interpreted languages manage object lifetimes with a garbage collector or Rust's `Rc<RefCell<T>>`. Serez-Code takes a fundamentally different approach: **region-based arena allocation** with watermark-based cleanup.

| Trait | Traditional interpreters | Serez-Code |
|---|---|---|
| Memory management | GC pauses / reference counting | Bump allocator + watermark truncation |
| Scope cleanup | Non-deterministic (GC) or O(n) | Deterministic, `O(k)` drops per scope exit |
| Object references | `Box` / `Rc` / raw pointers | `ObjectRef` — a safe `(RegionId, usize)` index pair |
| Type safety | Fully dynamic or fully static | Optional annotations, enforced at every call site |
| Integer safety | Silent overflow or panic | `checked_*` arithmetic — overflow is a runtime error |
| `unsafe` code | Often required for performance | **Zero `unsafe` blocks** |

Every `{ ... }` block is a **Flash Scope**. When the interpreter exits it, all block-local memory is freed via a single `Vec::truncate()` call — no reference counting, no GC pause.

---

## Getting Started

### Install

**Linux / macOS:**
```sh
curl -fsSL https://raw.githubusercontent.com/Sergio3215/serez-code/main/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/Sergio3215/serez-code/main/install.ps1 | iex
```

Or download a binary directly from the [GitHub Releases](https://github.com/Sergio3215/serez-code/releases) page.

### Build from source

Requires [Rust](https://rustup.rs/) stable, edition 2024:

```bash
git clone https://github.com/Sergio3215/serez-code
cd serez-code
cargo install --path . --force
```

### Run a script

```bash
sz script.sz
```

Errors go to `stderr`. You can separate program output from errors:

```bash
sz script.sz > output.txt    # captures only out statements
sz script.sz 2> errors.txt   # captures only runtime errors
```

### Run a JSX component file (`.szx`)

`.szx` files (the JSX dialect used by [serez-ui](https://github.com/Sergio3215/serez-ui)) run directly — the runtime translates them to `.sz` on the fly and executes the result:

```bash
sz app.szx
```

`import "component.szx";` works the same way inside `.sz` and `.szx` modules (translation is transparent and cached per run). Requirements and behavior:

- The translator ships with serez-ui (`tools/translate.sz`); install it with `sz install serez-ui`. Without it, `sz app.szx` fails with an explicit message.
- Translation errors are printed to the console as `TRANSLATE ERROR:` with the real `.szx` line — e.g. two adjacent JSX roots in a `return ()` suggest wrapping them in a fragment `<>…</>`.
- `sz --check app.szx` analyzes the translated output.

### Start the REPL

```bash
sz
>> let x = 10;
>> out x * 3;
30
```

### Watch mode (auto-rerun on save)

```bash
sz --watch script.sz
```

The script reruns automatically every time the file changes on disk.

### Analyze memory usage

```bash
sz --check script.sz
```

### Check version

```bash
sz --version
```

> **Note:** Serez-Code does not auto-print expression results when running files. Use `out` to send values to stdout.

### Pre-built binaries

Pre-built binaries for Windows x64, Linux x64 (static musl), macOS ARM64, and macOS x64 are published automatically on every tagged release via GitHub Actions. No Rust installation required to run them.

### Editor support (LSP)

`sz-lsp` is a Language Server Protocol implementation for `.sz` files (stdio JSON-RPC, built from the same crate):

```bash
cargo build --release --bin sz-lsp
```

Capabilities:

| Feature | Detail |
|---|---|
| Live diagnostics | Parser errors (as errors) + static type checker findings (as warnings), on every keystroke |
| Completion | Keywords, the 21 native namespaces with their real methods (extracted from the evaluator), builtin functions, and the document's own functions/classes/variables; `File.` → `read`, `write`, … |
| Hover | Signatures of user functions/classes (`fn int suma(int a, int b)`), namespace summaries, builtin signatures |
| Go to definition | Functions, classes, enums, variables in the file; `import "…" ` lines jump to the imported file |
| Document symbols | Outline with classes and their nested methods/fields |

The VS Code extension (`vscode-serez/`, ≥ 1.7.0) starts it automatically for `.sz` files: it looks for `sz-lsp` on the `PATH` (or use the `serez.lsp.path` setting; disable with `serez.lsp.enabled: false`). Any other LSP-capable editor (Neovim, Zed, JetBrains, …) can launch `sz-lsp` as a stdio server.

Regenerate the namespace/method catalog after adding native methods, and smoke-test the server end-to-end, with:

```bash
python tools/gen_lsp_builtins.py   # rebuilds src/lsp/builtins_gen.rs
python tools/lsp_smoke.py          # drives a real LSP session over stdio
```

---

## Language Reference

### Variables

Variables are declared with `let`. Reassignment uses bare `=` — no `let` again.

```serez
let name   = "Sergio";
let count  = 20;
let active = true;

count = count + 1;   // reassignment — variable must already exist
```

Variables declared inside a block `{ ... }` are invisible outside it. Variables from outer scopes can be mutated from inside:

```serez
let total = 0;

{
    let local = 42;   // only lives in this block
    total = local;    // outer variable mutated — allowed
}

out total;    // → 42
// out local; // ❌ ERROR: Variable not found: local
```

Attempting to use or reassign an undeclared variable is a runtime error:

```serez
x = 5;    // ❌ ERROR: Undeclared variable: x
out y;    // ❌ ERROR: Variable not found: y
```

#### `const`

`const` declares an immutable variable. Any attempt to reassign it is a runtime error.

```serez
const PI = 3.14159;
const MAX = 100;

PI = 3.0;   // ❌ ERROR: Cannot reassign const 'PI'
```

`const` follows the same scoping rules as `let` — it is invisible outside its block.

---

### Types

Serez-Code has five primitive types and three compound types:

| Type | Literal / annotation examples | Runtime representation |
|---|---|---|
| `int` | `0`, `42`, `-7` | 64-bit signed integer (`i64`) |
| `decimal` | `3.14`, `0.5`, `2.0` | 64-bit floating-point (`f64`) |
| `dec` | `12.50m`, `5m`, `1e-7m` | **Exact** base-10 decimal (`rust_decimal`, 28–29 digits) |
| `bool` | `true`, `false` | Boolean |
| `string` | `"hello"`, `r"raw {x}"` | UTF-8 string (interpolated, or raw with `r"…"`) |
| `void` | — | Signals absence of a return value |
| `any` | — | Wildcard: skips type validation |
| `null` | `null` | Absence of a value; used with nullable types |
| Array | `[1, 2, "x"]` or `[int]`, `[string]` | Typed or untyped, 0-indexed |
| Dict | `let d <string,int> = (...)` | Typed key-value store, ordered insertion |
| Function | `fn int add(...)` | First-class value |
| Interface | `new Punto({ x: 0.0, y: 0.0 })` | Record of typed fields; no methods |
| Class instance | `new Rectangulo("Box", 5.0, 3.0)` | Object with constructor, fields, and methods |

Types are **dynamic by default**. Annotations are optional on parameters and return values. When provided, they are enforced at every call site — see [Type System](#type-system) for details.

The `any` keyword suppresses type checking for that slot. It is useful for dict values of mixed type and for function parameters that accept any value.

#### Nullable types

Append `?` to any type to make it nullable. A nullable type accepts either the base type or `null`:

```serez
fn int? findIndex(string target) {
    // returns int if found, null if not
    let i = 0;
    while (i < names.length()) {
        if (names[i] == target) { return i; }
        i = i + 1;
    }
    return null;
}

let idx = findIndex("Ana");
if (idx != null) {
    out "Found at index {idx}";
} else {
    out "Not found";
}
```

Nullable annotations work on parameters, return types, and array element types: `int?`, `string?`, `[int?]`. The `null` literal produces a null value that is compatible with any nullable type.

#### Exact decimals (`dec`)

`decimal` is `f64` — fast, but binary, so `0.1 + 0.2 != 0.3`. For money and any
domain that cannot tolerate rounding drift, use **`dec`**: an exact base-10
decimal written with the **`m` suffix** (`12.50m`, `5m`, `1e-7m`).

```serez
out 0.1 + 0.2 == 0.3        // false  (f64)
out 0.1m + 0.2m == 0.3m     // true   (exact)

let price = 12.50m          // type inferred as dec; scale is preserved → "12.50"
let total = price * (1m + 0.21m)
out total                    // 15.1250
```

- `int` mixes in exactly (`1 + 1m → 2m`); mixing `dec` with `decimal` (f64) is a
  **type error** — convert explicitly with `d.toDecimal()` / `Dec.parse`.
- Comparison is by value (`1.50m == 1.5m` → `true`); arithmetic is checked
  (overflow → error), `/` rounds to 28 digits, `**` needs an integer exponent.
- Rounding is explicit: `d.round(n[, mode])` / `d.setScale(n[, mode])` where mode
  is `half-even` (default), `half-up`, `down`, `up`, `floor` or `ceil`.
- Methods: `round setScale truncate scale abs floor ceil isZero sign min max
  toInt toDecimal toString`. Namespace: `Dec.parse(s)`, `Dec.fromInt(v, scale)`,
  `Dec.MAX`, `Dec.MIN`, `Dec.MAX_SCALE` (28).

```serez
let iva = (1000.00m * 0.21m).setScale(2, "half-up")   // 210.00 (COBOL ROUNDED)
out Dec.fromInt(1250, 2)                               // 12.50
```

#### Raw strings (`r"…"`)

By default a `"…"` string is **interpolated**: `{expr}` is evaluated and `\{`/`\}`
escape literal braces. A **raw** string `r"…"` disables interpolation *and* escape
processing — `{ }` and backslashes are literal — which is ideal for literal
braces, Windows paths and regexes:

```serez
let x = 5
out "value is {x}"     // value is 5     (interpolated)
out r"value is {x}"    // value is {x}   (raw)
out r"C:\temp\new"     // C:\temp\new    (no escapes)
out r"\d+\.\d{2}"      // \d+\.\d{2}     (regex literal)
```

A raw string cannot contain a `"` (the first quote closes it) — use a normal
string with `\"` for that.

---

### Operators

#### Arithmetic

Integer arithmetic operates on `int` values. Integer division truncates toward zero.

```serez
out 10 + 3;    // → 13
out 10 - 3;    // → 7
out 10 * 3;    // → 30
out 10 / 3;    // → 3   (integer division, truncates)
out 10 % 3;    // → 1   (modulo)
out -5;        // → -5  (negation — prefix)
```

All integer arithmetic operations are overflow-safe. If the result would overflow `i64`, a runtime error is raised instead of wrapping silently. Division and modulo by zero are runtime errors.

#### Decimal arithmetic

The `decimal` type (`f64`) supports the same arithmetic operators as `int`. Mixing `int` and `decimal` in the same expression is allowed — the `int` is automatically promoted:

```serez
let pi = 3.14159;
let r  = 2.0;

out pi * r * r;       // → 12.56636
out 1 + 0.5;          // → 1.5   (int + decimal → decimal)
out 10.0 / 4;         // → 2.5
out -3.14;            // → -3.14 (prefix negation)
```

Decimal literals always require a digit on both sides of the dot: `3.14`, `0.5`, `2.0`. The display trims trailing zeros but always shows at least one decimal place for integer-valued results (`5.0`, not `5`).

Functions can be annotated with `decimal` for parameter and return types:

```serez
fn decimal area(decimal r) {
    return r * r * 3.14159;
}

out area(5.0);   // → 78.53975
```

#### Comparison

Comparison operators produce `bool` values:

```serez
out 5 > 3;     // → true
out 5 < 3;     // → false
out 5 >= 5;    // → true
out 5 <= 4;    // → false
out 5 == 5;    // → true
out 5 != 3;    // → true
```

#### Logical

```serez
out !true;     // → false
out !false;    // → true
```

The `!` prefix applies only to booleans. Applying it to any other type is a runtime error.

`&&` and `||` are infix logical operators. Both require boolean operands and use **short-circuit evaluation**: `&&` stops at the first `false`, `||` stops at the first `true`.

```serez
out true && true;     // → true
out true && false;    // → false
out false && true;    // → false  (right side not evaluated)
out false || true;    // → true
out false || false;   // → false
out true || false;    // → true   (right side not evaluated)

// Combine with comparison operators:
out (1 < 2) && (3 > 0);    // → true
out (1 > 2) || (3 == 3);   // → true
```

Applying `&&` or `||` to non-boolean operands is a runtime error.

#### Power operator

`**` raises a number to an exponent. Works for both `int` and `decimal`. Applies tighter than `*`:

```serez
out 2 ** 10;       // → 1024
out 3 ** 3;        // → 27
out 2.0 ** 32.0;   // → 4294967296.0
out 0 ** 0;        // → 1   (mathematical convention)
out (-2) ** 3;     // → -8
```

#### Bitwise operators

Integer-only. All operate on 64-bit signed integers (two's complement).

| Operator | Name | Example |
|---|---|---|
| `&` | Bitwise AND | `0b1010 & 0b1100 == 8` |
| `\|` | Bitwise OR | `0b1010 \| 0b0101 == 15` |
| `^` | Bitwise XOR | `0b1111 ^ 0b1010 == 5` |
| `~` | Bitwise NOT (prefix) | `~0 == -1` |
| `<<` | Left shift | `1 << 4 == 16` |
| `>>` | Right shift (arithmetic) | `16 >> 2 == 4` |

Binary (`0b`) and hexadecimal (`0x`) literals are supported:

```serez
out 0b1010;    // → 10
out 0xFF;      // → 255
out 0b1010 & 0b1100;   // → 8
out ~9223372036854775807;  // → -9223372036854775808  (i64::MIN)
```

Shifting by a negative amount or by ≥ 64 is a runtime error.

#### `is` type-check operator

`expr is TypeName` returns `true` if the expression has the given type at runtime:

```serez
out 42 is int;        // → true
out "hi" is int;      // → false
out 3.14 is decimal;  // → true
out null is null;     // → true
out [1,2] is array;   // → true

let f = (x) => x + 1;
out f is function;    // → true (named functions and lambdas both match)

fn string dispatch(any v) {
    if (v is int)     { return "int:" + v; }
    if (v is string)  { return "str:" + v; }
    if (v is decimal) { return "dec:" + v; }
    return "unknown";
}
out dispatch(42);     // → int:42
out dispatch("hi");   // → str:hi
```

Type names: `int`, `decimal`, `string`, `bool`, `null`, `array`, `dict`, `function`, or a class name.

#### Numeric separators

Underscores can be inserted anywhere in a numeric literal for readability. They are ignored by the parser:

```serez
let million = 1_000_000;
let mask    = 0xFF_FF_FF_FF;
let bits    = 0b1111_0000;
```

#### String operations

Strings support concatenation with `+` and repetition with `*`:

```serez
out "hello" + " world";    // → hello world
out "ha" * 3;              // → hahaha
out "a" == "a";            // → true
out "a" != "b";            // → true
```

`*` requires a non-negative integer on the right. Negative repeat is a runtime error.

String and integer concatenation requires explicit conversion via concatenation with another string:

```serez
let age = 23;
out "Sergio con " + age + " años";   // → Sergio con 23 años
```

#### Compound assignment

`+=`, `-=`, `*=`, `/=`, and `%=` are shorthand for reading, computing, and writing back in one step:

```serez
let n = 10;
n += 5;    // n = 15
n -= 3;    // n = 12
n *= 2;    // n = 24
n /= 4;    // n = 6
n %= 4;    // n = 2
```

#### Increment / decrement

`++` and `--` increment or decrement a variable by 1. Both postfix and prefix forms are supported and produce the same effect (the value is not returned — they are pure statements):

```serez
let i = 0;
i++;     // i = 1   (postfix)
++i;     // i = 2   (prefix)
i--;     // i = 1
--i;     // i = 0
```

Typical use inside loops:

```serez
let count = 0;
while (count < 5) {
    out count;
    count++;
}
// → 0, 1, 2, 3, 4
```

#### Ternary operator

The `? :` operator evaluates a condition and returns one of two expressions. Only the chosen branch is evaluated (lazy):

```serez
let x = 10;
let label = x > 5 ? "big" : "small";
out label;   // → big

out true ? 1 : 2;    // → 1
out false ? 1 : 2;   // → 2
```

Ternary is right-associative — chained ternaries read naturally:

```serez
let n = 2;
let name = n == 1 ? "one" : n == 2 ? "two" : "other";
out name;   // → two
```

#### Operator precedence

From lowest to highest:

| Level | Operators |
|---|---|
| `Lowest` | — |
| `Ternary` | `? :` |
| `NullCoalesce` | `??` |
| `LogicalOr` | `\|\|` |
| `LogicalAnd` | `&&` |
| `BitOr` | `\|` |
| `BitXor` | `^` |
| `BitAnd` | `&` |
| `Equals` | `==` `!=` |
| `LessGreater` | `<` `>` `<=` `>=` `is` |
| `Shift` | `<<` `>>` |
| `Sum` | `+` `-` |
| `Product` | `*` `/` `%` |
| `Power` | `**` |
| `Prefix` | `-x` `!x` `~x` |
| `Call` | `f(x)` `.method(args)` `?.method(args)` |
| `Index` | `a[i]` |

Parentheses can override precedence:

```serez
out 2 + 3 * 4;     // → 14  (Product before Sum)
out (2 + 3) * 4;   // → 20
```

---

### Functions

Serez-Code supports three function syntaxes. All functions are first-class values.

#### Named declarations

```
fn <return_type> <name>(<params>) { <body> }
```

The return type and parameter types are optional. Names are required for declarations.

```serez
fn int add(int a, int b) {
    return a + b;
}

fn void greet(string name) {
    out "Hello, ";
    out name;
}

fn bool isAdult(int age) {
    return age >= 18;
}

fn string repeat(string s, int n) {
    return s * n;
}
```

#### Arrow functions

```
let <name> = <return_type> (<params>) => { <body> }
```

The return type goes **before** the parentheses. Braces are always required.

```serez
let double = int (int n) => {
    return n * 2;
}

let greet = void (string s) => {
    out s;
}

let isEven = bool (int n) => {
    return n == 0;
}
```

#### Anonymous functions

Functions without a name can be assigned to variables and passed around:

```serez
let run = fn void () {
    out "running anonymous logic";
};

run();
```

#### Mixed / untyped parameters

Type annotations are per-parameter. Typed and untyped can be mixed freely in the same signature:

```serez
fn int mixta(x, int y, string z) {
    out z;
    return y + 100;
}

out mixta(1, 50, "processing...");   // → 150
```

When a parameter has no type annotation, the function accepts any value for it.

#### Default parameters

Parameters can have default values. If the caller omits the argument, the default is used. Default parameters must come after required ones.

```serez
fn string greet(string name = "World") {
    return "Hello, " + name + "!";
}

out greet();          // → Hello, World!
out greet("Sergio");  // → Hello, Sergio!
```

Multiple defaults, with required parameters first:

```serez
fn int add(int a, int b = 10) {
    return a + b;
}

out add(5);      // → 15   (b defaults to 10)
out add(5, 3);   // → 8    (b supplied)
```

Default values are arbitrary expressions evaluated at call time:

```serez
fn int compute(int n = 2 + 3) {
    return n * 2;
}

out compute();    // → 10  (default: 5 * 2)
out compute(7);   // → 14
```

#### Calling functions

```serez
out add(3, 7);          // → 10
out isAdult(18);        // → true
out repeat("ab", 3);   // → ababab
```

Arguments are evaluated left-to-right before the call.

#### Recursive functions

```serez
fn int factorial(int n) {
    if (n <= 1) {
        return 1;
    }
    return n * factorial(n - 1);
}

out factorial(6);   // → 720
```

The call stack is tracked and printed on error, so deeply nested recursion will display a readable trace.

#### Functions as values

```serez
fn int double(int n) {
    return n * 2;
}

let op = double;    // functions are values
out op(21);         // → 42
```

---

### Control Flow

#### `if` / `else`

Parentheses around the condition are required. Braces around each branch are required.

```serez
if (x > 0) {
    out "positive";
} else {
    out "non-positive";
}
```

`if` is an expression — it produces a value that can be returned or assigned:

```serez
fn string classify(int n) {
    if (n > 0) {
        return "positive";
    } else if (n < 0) {
        return "negative";
    } else {
        return "zero";
    }
}
```

#### `else if` chaining

```serez
if (score >= 90) {
    out "A";
} else if (score >= 75) {
    out "B";
} else if (score >= 60) {
    out "C";
} else {
    out "F";
}
```

#### `while`

```serez
let i = 0;
while (i < 5) {
    out i;
    i = i + 1;
}
// → 0, 1, 2, 3, 4
```

`return` inside a `while` propagates through the loop and exits the enclosing function immediately:

```serez
fn int findFirst(int target) {
    let i = 0;
    while (i < 10) {
        if (i == target) {
            return i;
        }
        i = i + 1;
    }
    return -1;
}

out findFirst(7);   // → 7
out findFirst(99);  // → -1
```

The while condition is evaluated freshly each iteration and its temporary memory is released before entering the body, so loops do not accumulate condition allocations.

#### `for`

C-style for loop. The initializer must be a `let` declaration. The update accepts `i = expr`, `i++`, `i--`, `i += n`, `i -= n`, `i *= n`, `i /= n`, or `i %= n`.

```
for (<let init>; <condition>; <update>) { <body> }
```

```serez
for (let i = 0; i < 5; i++) {
    out i;
}
// → 0, 1, 2, 3, 4
```

The loop variable is scoped to the loop — it is not accessible after the closing `}`. Iterating over an array by index:

```serez
let nums = [10, 20, 30, 40, 50];
let sum = 0;

for (let i = 0; i < 5; i = i + 1) {
    sum = sum + nums[i];
}
out sum;   // → 150
```

Nested `for` loops work naturally and each loop variable is scoped independently:

```serez
let matrix = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];

for (let i = 0; i < 3; i = i + 1) {
    for (let j = 0; j < 3; j = j + 1) {
        out matrix[i][j];
    }
}
// → 1, 2, 3, 4, 5, 6, 7, 8, 9
```

`return` inside a `for` propagates through the loop and exits the enclosing function immediately:

```serez
fn int firstOver(int limit) {
    for (let k = 0; k < 100; k = k + 1) {
        if (k > limit) {
            return k;
        }
    }
    return -1;
}

out firstOver(7);    // → 8
out firstOver(200);  // → -1
```

Like `while`, the condition and update temporaries are freed each iteration — loops do not accumulate allocations.

---

#### `for-in`

Iterates over every element of an array or every character of a string. The loop variable is declared with `let` and is scoped to the loop body.

```
for (let <var> in <iterable>) { <body> }
```

```serez
let nums [int] = [10, 20, 30];
let sum = 0;

for (let n in nums) {
    sum += n;
}
out sum;   // → 60
```

Iterating over a string visits each character:

```serez
let result = "";
for (let c in "abc") {
    result = result + c + "-";
}
out result;   // → a-b-c-
```

`break` and `continue` work the same as in `while`/`for`:

```serez
let nums [int] = [1, 2, 3, 4, 5];
let sum = 0;
for (let n in nums) {
    if (n == 3) { continue; }   // skip 3
    sum += n;
}
out sum;   // → 1 + 2 + 4 + 5 = 12
```

Nested `for-in` loops each have their own independent variable:

```serez
let outer [int] = [1, 2, 3];
let inner [int] = [10, 20];
let total = 0;

for (let a in outer) {
    for (let b in inner) {
        total += a * b;
    }
}
out total;   // → 180
```

`return` inside a `for-in` propagates immediately and exits the enclosing function.

---

#### `do-while`

`do-while` guarantees the body runs **at least once**. The condition is checked after each iteration.

```serez
let i = 0;
do {
    out i;
    i++;
} while (i < 3);
// → 0, 1, 2
```

Even when the condition starts false, the body executes once:

```serez
let x = 100;
do {
    out "ran once";
} while (x < 0);
// → ran once
```

`break` and `continue` work the same as in `while`/`for`:

```serez
let n = 0;
do {
    n++;
    if (n == 5) { break; }
} while (n < 100);
out n;   // → 5
```

---

#### Labeled loops

A label can be placed before any loop. `break label` exits the labeled loop from any depth; `continue label` restarts the labeled loop's next iteration.

```serez
outer: for (let i = 0; i < 3; i++) {
    for (let j = 0; j < 3; j++) {
        if (j == 1) { continue outer; }   // skip inner, go to next i
        out "{i},{j}";
    }
}
// → 0,0   1,0   2,0   (j=1 is always skipped)
```

```serez
outer: while (true) {
    let x = 0;
    while (x < 10) {
        if (x == 3) { break outer; }   // exit the outer while entirely
        x++;
    }
}
```

Labels work with `while`, `for`, `for-in`, and `do-while`.

---

#### `switch`

`switch` matches an expression against one or more `case` values. Each case body is a block. An optional `default` block runs when no case matches.

```serez
let day = 3;

switch (day) {
    case 1: { out "Monday"; }
    case 2: { out "Tuesday"; }
    case 3: { out "Wednesday"; }
    default: { out "Other"; }
}
// → Wednesday
```

A single `case` can match multiple values separated by commas:

```serez
switch (day) {
    case 1, 2, 3, 4, 5: { out "Weekday"; }
    case 6, 7:           { out "Weekend"; }
}
```

`switch` does **not** fall through — only the matched case runs. `break` is not needed.

---

#### Exceptions (`try` / `catch` / `finally` / `throw`)

`throw` raises an exception with any value. `try/catch` intercepts it. `finally` always runs, whether or not an exception was thrown.

```serez
fn int divide(int a, int b) {
    if (b == 0) { throw "Division by zero"; }
    return a / b;
}

try {
    let result = divide(10, 0);
    out result;
} catch (e) {
    out "Caught: {e}";   // → Caught: Division by zero
} finally {
    out "Always runs";
}
```

Any value can be thrown — strings, numbers, objects:

```serez
throw 42;
throw { code: 404, msg: "Not found" };
```

**Catchable runtime errors.** Ordinary programming errors — index out of range,
division by zero, type mismatches, invalid assignment targets — are catchable
too. Inside `catch` they bind a structured **`Error`** object with `.message`
(human-readable) and `.kind` (a category). Concatenating the error with a string
uses its message:

```serez
let a = [1, 2, 3];
try {
    let x = a[99];
} catch (e) {
    out e.kind;         // → IndexOutOfBounds
    out e.message;      // → Index out of bounds: 99 (length 3)
    out "boom: " + e;   // → boom: Index out of bounds: 99 (length 3)
}
```

A thrown value keeps its original type (`throw "x"` binds the string `"x"`); only
errors raised by the runtime bind an `Error` object.

**I/O and namespace errors are catchable too.** A missing file, a refused socket
connection, an invalid JSON body or a tensor shape mismatch no longer abort the
program — they raise an `Error` your code can handle:

```serez
use permissions { File }

try {
    let config = File.read("config.json");
} catch (e) {
    out e.kind;      // → IOError
    out e.message;   // → File error reading 'config.json': ... (os error 2)
}
```

**`Error.kind` reference:**

| Kind | Raised by |
|---|---|
| `IndexOutOfBounds` | Array/string access outside `[0, len-1]` |
| `DivisionByZero` | `/` or `%` with zero on the right |
| `TypeError` | Type mismatches, wrong argument counts/types, unknown methods |
| `InvalidAssignTarget` | Index-assign into a nested/temporary target (`m[i][j] = x`) |
| `Overflow` | Integer arithmetic outside the `i64` range |
| `IOError` | `File.*` failures (missing file, permissions), `Terminal.*` I/O |
| `JsonError` | `JSON.parse` on invalid JSON |
| `OSError` | `OS.exec` / `OS.kill` process failures |
| `SocketError` | `Socket.*` network failures (refused, reset, invalid id) |
| `GuiError` | `Gui.*` runtime failures (no window open, no GUI host) |
| `TensorError` | `Tensor` shape/value errors (matmul mismatch, bad reshape) |
| `AutodiffError` | `Autodiff.*` runtime failures |
| `GpuError` | `GPU.*` buffer errors (invalid handle, size mismatch) |
| `MemoryError` | `Memory.*` runtime failures inside `unsafe {}` (bad handle, OOB) |
| `BinaryError` | `Binary.*` decode/encode failures |
| `RuntimeError` | Anything else raised by the runtime (e.g. invalid `Regex` patterns) |

`catch` is optional. `finally` is optional. Both together are also valid:

```serez
try {
    riskyOperation();
} finally {
    cleanup();   // runs even if riskyOperation throws
}
```

Unhandled exceptions (no enclosing `try`) terminate the program with a runtime
error message.

**Not catchable — fatal by design.** Security and resource-limit violations stay
fatal and bypass `try/catch`: permission denials, operations that require an
`unsafe {}` block, stack overflow and other resource guards always abort. This
guarantees a script can never silently swallow a security or denial-of-service
condition.

---

#### Optional chaining (`?.`)

`?.` calls a method or accesses a field only when the receiver is non-null. If the receiver is `null`, the whole expression evaluates to `null` without throwing.

```serez
let s = null;
let upper = s?.toUpperCase();   // s is null → upper = null (no error)

class Node {
    public Node(int v) { this.value = v; this.next = null; }
    public int getValue() { return this.value; }
}

let n = new Node(42);
out n?.getValue();       // → 42
out null?.getValue();    // → null  (no crash)
```

`?.` chains: each link stops at `null` and the remainder is never evaluated:

```serez
let result = a?.getNext()?.getValue() ?? 0;
// if a is null                → null ?? 0 → 0
// if a.getNext() returns null → null ?? 0 → 0
// otherwise                  → the value
```

Combine with `??` to provide a safe fallback for the whole chain.

---

#### Standalone blocks

Any `{ ... }` creates a new Flash Scope. This is useful to limit the lifetime of temporary variables:

```serez
let y = 1;

out y;   // → 1

{
    let x = 10;   // x is local to this block
    y = 100;      // y lives outside — mutation propagates
}

out y;   // → 100
// out x;   // ❌ ERROR: Variable not found: x
```

---

### Arrays

Arrays are heterogeneous (can mix types) and 0-indexed. They are created with bracket literals.

```serez
let nums  = [1, 2, 3, 4, 5];
let mixed = [42, "hello", true];
let empty = [];
```

#### Typed arrays

Place a type keyword between the name and `=` to constrain every element to that type. The interpreter enforces the type on construction, `push`, `unshift`, and index-assignment:

```serez
let nums    [int]     = [1, 2, 3];
let prices  [decimal] = [9.99, 14.50, 3.0];
let labels  [string]  = ["a", "b", "c"];
let maybes  [int?]    = [1, null, 3];   // nullable element type

nums.push(4);        // ✅
nums.push("hello");  // ❌ TYPE ERROR: Cannot push 'string' into [int] array
```

Functions can also declare typed array parameters and return types:

```serez
fn decimal sumAll([decimal] values) {
    return values.reduce(0.0, (acc, v) => acc + v);
}

fn [string] namesAbove([decimal] scores, decimal threshold) {
    // returns a typed [string] array
    let result [string] = [];
    let i = 0;
    while (i < scores.length()) {
        if (scores[i] > threshold) { result.push(names[i]); }
        i = i + 1;
    }
    return result;
}
```

Untyped arrays (e.g. `let arr = [1, "x", true]`) remain valid and accept mixed element types.

#### Index access

```serez
out nums[0];    // → 1
out nums[4];    // → 5
out mixed[1];   // → hello
```

Indexing with a negative number or an index beyond the last element is a runtime error:

```serez
out nums[10];   // ❌ ERROR: Index out of bounds
```

#### Index mutation

Array elements can be reassigned by index. The array must already be declared with `let`.

```serez
let nums = [10, 20, 30];
nums[1] = 99;
out nums[1];   // → 99
```

Mutation works inside loops:

```serez
let squares = [0, 0, 0, 0, 0];
for (let i = 0; i < 5; i = i + 1) {
    squares[i] = i * i;
}
out squares[3];   // → 9
```

Mutation of a global array from inside a function also works:

```serez
let data = [10, 20, 30];

fn void doubleAt(int idx) {
    data[idx] = data[idx] * 2;
}

doubleAt(1);
out data[1];   // → 40
```

Index must be a non-negative integer within bounds — out-of-range mutations are runtime errors:

```serez
let a = [1, 2, 3];
a[5] = 0;   // ❌ ERROR: Index out of bounds
```

Only assignments to a **variable** (`a[i] = x`) or an **object field**
(`obj.field[i] = x`) persist, because reading anything else yields a copy
(value semantics). A **nested** target like `m[i][j] = x` — where `m[i]` is a
copy — is rejected loudly with an `InvalidAssignTarget` error (never a silent
no-op). To update a nested element, rebuild and reassign the whole inner value:

```serez
let m = [[1, 2], [3, 4]];
// m[0][1] = 99;              // ❌ ERROR: InvalidAssignTarget (m[0] is a copy)
let row = m[0];
row[1] = 99;
m[0] = row;                   // ✅ reassign the whole element
out m[0][1];                  // → 99
```

#### Arrays from functions

Functions can build and return arrays. The returned array is safely promoted out of the function's scope before cleanup:

```serez
fn make_arr() {
    return [7, 8, 9];
}

let result = make_arr();
out result[0];   // → 7
out result[1];   // → 8
out result[2];   // → 9
```

Passing values into arrays works the same way:

```serez
fn wrap(a, b) {
    return [a, b];
}

let pair = wrap(42, 99);
out pair[0];   // → 42
out pair[1];   // → 99
```

#### Array mutation methods

| Method | Effect |
|---|---|
| `.push(val)` | Appends `val` to the end of the array (mutates in-place). |
| `.pop()` | Removes and returns the last element. **Runtime error if called on an empty array.** |
| `.shift()` | Removes and returns the first element. **Runtime error if called on an empty array.** |
| `.unshift(val)` | Prepends `val` to the beginning (mutates in-place). |
| `.remove(idx)` | Removes the element at index `idx` and returns it. |
| `.reverse()` | Reverses the array in-place (mutates, returns the same array). |
| `.sort()` | Sorts in ascending order (mutates in-place, returns the same array). |
| `.sort("desc")` | Sorts in descending order (mutates in-place, returns the same array). |
| `.sort((a, b) => expr)` | Sorts with a custom comparator lambda. Positive result = swap (like JS). |

#### Array query methods

| Method | Returns | Description |
|---|---|---|
| `.length` | `int` | Number of elements (property, no parentheses). |
| `.indexOf(val)` | `int` | Index of first element equal to `val`, or `-1` if not found. |
| `.includes(val)` / `.contains(val)` | `bool` | `true` if the array contains `val`. |
| `.find(cb)` | element or `null` | First element for which `cb(element)` returns `true`, or `null`. |
| `.findIndex(cb)` | `int` | Index of first element matching the predicate, or `-1`. |
| `.every(cb)` | `bool` | `true` if `cb` returns `true` for **every** element (vacuously `true` for empty). |
| `.some(cb)` | `bool` | `true` if `cb` returns `true` for **at least one** element (vacuously `false` for empty). |
| `.slice(start, end)` | array | New array with elements from `start` (inclusive) to `end` (exclusive). Negative `start` counts from the end. |
| `.flat()` | array | New flattened array — one level of nesting removed. |
| `.join(sep?)` | `string` | Joins all elements into a string separated by `sep` (default: `","`). |

```serez
let nums = [1, 2, 3, 4, 5];

out nums.find(x => x > 3);        // → 4
out nums.findIndex(x => x > 3);   // → 3
out nums.indexOf(3);              // → 2
out nums.includes(99);            // → false
out nums.every(x => x > 0);       // → true
out nums.some(x => x > 4);        // → true
out nums.slice(1, 4);             // → [2, 3, 4]

let nested = [[1, 2], [3, 4]];
out nested.flat();                 // → [1, 2, 3, 4]

nums.reverse();
out nums;                          // → [5, 4, 3, 2, 1]
```

```serez
let stack = [1, 2, 3, 4, 5];
let top   = stack.pop();       // removes 5
out top;                       // → 5
out stack;                     // → [1, 2, 3, 4]

stack.push(99);
out stack;                     // → [1, 2, 3, 4, 99]

let first = stack.shift();     // removes 1
out first;                     // → 1

stack.unshift(0);
out stack;                     // → [0, 2, 3, 4, 99]

let nums = [5, 2, 8, 1, 4];
nums.sort();
out nums;                      // → [1, 2, 4, 5, 8]

nums.sort("desc");
out nums;                      // → [8, 5, 4, 2, 1]

// Custom comparator — descending by absolute value:
let vals = [3, -7, 1, -2, 8];
let sorted = vals.sort((a, b) => b - a);
out sorted;                    // → [8, 3, 1, -2, -7]
```

`.sort` without a comparator requires a homogeneous array (all `int`, all `decimal`, or all `string`). Mixed-type arrays cannot be sorted — this is a runtime error. `.sort` with a comparator lambda uses bubble sort internally and works for any numeric array.

`.sort` mutates the array in-place **and** returns the same array reference, allowing assignment: `let sorted = arr.sort((a, b) => b - a)`.

---

### String Methods

All string methods are called with dot syntax. `.length` is a property; all others are method calls.

#### Core methods

| Method / property | Description |
|---|---|
| `.length` | Number of Unicode characters (UTF-8 aware). |
| `.toString()` | Returns the string itself (identity for strings; works on `int`, `decimal`, `bool` too). |
| `.substring(start[, end])` | Characters from `start` (inclusive) to `end` (exclusive). Omitting `end` goes to end of string. |
| `.slice(start[, end])` | Like `substring`; negative `start` counts from the end. |
| `.split(sep)` | Splits by `sep`, returns an array. Empty `sep` splits into individual characters. |
| `.replace(from, to)` | Returns a new string with **all** occurrences of `from` replaced by `to`. |
| `.replaceAll(from, to)` | Alias for `.replace`. |
| `.includes(sub)` / `.contains(sub)` | `true` if the string contains `sub`. |
| `.indexOf(sub)` | Index of first occurrence of `sub`, or `-1`. |
| `.startsWith(prefix)` | `true` if the string starts with `prefix`. |
| `.endsWith(suffix)` | `true` if the string ends with `suffix`. |
| `.charAt(i)` | Single character at position `i`, or `""` if out of bounds. |

#### Case and whitespace

| Method | Description |
|---|---|
| `.toUpperCase()` / `.upper()` | Returns an uppercase copy. |
| `.toLowerCase()` / `.lower()` | Returns a lowercase copy. |
| `.trim()` | Removes leading and trailing whitespace. |
| `.trimStart()` / `.trimLeft()` | Removes leading whitespace only. |
| `.trimEnd()` / `.trimRight()` | Removes trailing whitespace only. |

#### Padding

| Method | Description |
|---|---|
| `.padStart(n[, ch])` | Pads the start with `ch` (default: space) until the string is at least `n` characters. |
| `.padEnd(n[, ch])` | Pads the end with `ch` (default: space) until the string is at least `n` characters. |

```serez
let s = "hello world";

out s.length;                     // → 11
out s.substring(0, 5);            // → hello
out s.slice(-5, 11);              // → world
out s.split(" ");                 // → [hello, world]
out s.includes("world");          // → true
out s.indexOf("world");           // → 6
out s.startsWith("hel");          // → true
out s.endsWith("ld");             // → true
out "abc".split("");              // → [a, b, c]

// replace replaces all occurrences
let r = "one two one two one";
out r.replace("one", "X");        // → X two X two X

// case and whitespace
out "hello".toUpperCase();        // → HELLO
out "  hello  ".trim();           // → hello
out "  hello  ".trimStart();      // → hello  (trailing preserved)

// padding
out "42".padStart(5, "0");        // → 00042
out "hi".padEnd(5, "-");          // → hi---
```

`.toString()` works on `int`, `decimal`, and `bool` values too:

```serez
out 42.toString();     // → 42
out 3.14.toString();   // → 3.14
out true.toString();   // → true
```

---

### Dictionaries

Dictionaries are typed key-value stores. The type annotation `<key_type, value_type>` is mandatory. Use `any` for values of mixed or unknown type.

```serez
let dicc    <string,string> = ({"hola","1"},{"chau","1"},{"gracias","1"});
let precios <string,int>    = ({"jamon",12},{"Shen",2});
let mixto   <string,any>    = ({"jamon",2},{"Shen",true});
let empty   <string,int>    = ();
```

#### Reading

```serez
out dicc["hola"];      // → 1
out precios["jamon"];  // → 12
out mixto["Shen"];     // → true
```

Accessing a missing key returns `null` (typed and untyped dicts alike). Use `??` to provide a default: `d["missing"] ?? 0`. Writing a **value of the wrong type** into a typed dict (`<K, V>`) is a runtime error.

#### Printing the whole dict

```serez
out dicc;   // → {hola: 1, chau: 1, gracias: 1}
```

#### Methods

| Method | Syntax | Effect |
|---|---|---|
| `Add` | `d.Add({"key","val"})` | Insert a new entry. If the key already exists, replace its value (upsert). |
| `Remove` | `d.Remove("key")` | Delete the entry with the given key. No-op if the key is absent. |
| `RemoveAll` | `d.RemoveAll()` | Delete all entries. |
| `clear` | `d.clear()` | Alias for `RemoveAll`. |
| `toList()` | `d.toList()` | Returns an array of all keys in insertion order. |
| `toArray()` | `d.toArray()` | Returns a 2D array of `[[key, val], [key, val], ...]` pairs. |

```serez
let scores <string,int> = ({"Alice",90},{"Bob",75},{"Carol",88});

let names = scores.toList();
out names;   // → [Alice, Bob, Carol]

let pairs = scores.toArray();
out pairs;   // → [[Alice, 90], [Bob, 75], [Carol, 88]]

// toArray() is useful with filter / map:
let top = pairs.filter(pair => pair[1] >= 85);
out top;     // → [[Alice, 90], [Carol, 88]]
```

```serez
dicc.Add({"cantar","true"});
out dicc["cantar"];    // → true

dicc.Add({"hola","2"});   // overwrite existing key
out dicc["hola"];          // → 2

dicc.Remove("cantar");
out dicc;              // → {hola: 2, chau: 1, gracias: 1}

dicc.RemoveAll();
out dicc;              // → {}
```

#### Writing via index

As an alternative to `Add`, a key can be written directly with index-assignment syntax:

```serez
precios["queso"] = 8;    // inserts "queso" → 8
precios["jamon"] = 15;   // replaces existing value
out precios["jamon"];    // → 15
```

#### Type enforcement

The type annotation is enforced on both `Add` and the dict literal. Using `any` for either type skips enforcement for that slot:

```serez
let typed <string,int> = ({"a",1});
typed.Add({"b","wrong"});   // ❌ TYPE ERROR: Dict value type mismatch on Add (expected 'int')

let flexible <string,any> = ({"a",1},{"b",true},{"c","mixed"});   // all valid
```

#### Mutating a global dict from a function

Mutations of global dicts from inside functions use the same `plant_global` mechanism as arrays — the new values are allocated in the global arena so they outlive the function scope:

```serez
let counters <string,int> = ({"hits",0});

fn void inc() {
    counters.Add({"hits", counters["hits"] + 1});
}

inc();
inc();
out counters["hits"];   // → 2
```

---

### Higher-Order Functions

Arrays support three built-in higher-order functions: `.map`, `.filter`, and `.reduce`. Each takes a **lambda** (anonymous inline function) as its callback.

#### Lambda syntax

Lambdas use JS-style arrow syntax:

```
// Single parameter — no parentheses needed
x => expression
x => { statements; return value; }

// Two parameters (value + index)
(item, index) => expression
(item, index) => { statements; return value; }

// Accumulator pattern (for reduce)
(acc, item) => expression
```

#### `.map(callback)`

Transforms each element. Returns a new array.

```serez
let nums = [1, 2, 3, 4, 5];

let doubled = nums.map(x => x * 2);
out doubled;   // → [2, 4, 6, 8, 10]

// With index:
let indexed = nums.map((x, i) => i);
out indexed;   // → [0, 1, 2, 3, 4]

// Multi-line lambda body:
let results = nums.map(x => {
    let doubled = x * 2;
    return doubled + 1;
});
out results;   // → [3, 5, 7, 9, 11]

// toString on each element:
let strs = [1, 2, 3].map(x => x.toString());
out strs;      // → [1, 2, 3]
```

#### `.filter(callback)`

Keeps only elements for which the callback returns `true`. Returns a new array.

```serez
let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

let evens = data.filter(x => x % 2 == 0);
out evens;   // → [2, 4, 6, 8, 10]

// Capturing an outer variable (closure):
let threshold = 5;
let big = [1, 3, 5, 7, 9, 11].filter(x => x > threshold);
out big;     // → [7, 9, 11]
```

#### `.reduce(initial, callback)`

Folds the array into a single value. The callback receives `(accumulator, currentValue)`. The first argument is the initial accumulator value.

```serez
let nums = [1, 2, 3, 4, 5];

let sum = nums.reduce(0, (acc, x) => acc + x);
out sum;   // → 15

// String accumulator:
let words = ["hello", " ", "world", "!"];
let sentence = words.reduce("", (acc, w) => acc + w);
out sentence;   // → hello world!

// Chaining filter + reduce:
let sum_evens = [1,2,3,4,5,6,7,8,9,10].filter(x => x % 2 == 0).reduce(0, (a,x) => a+x);
out sum_evens;   // → 30
```

#### Lambdas capture their enclosing scope

Lambdas close over variables from the scope where they are defined:

```serez
let multiplier = 3;
let tripled = [1, 2, 3, 4].map(x => x * multiplier);
out tripled;   // → [3, 6, 9, 12]
```

A closure and its enclosing scope **share** each captured variable (cell semantics, like JavaScript's `let`): mutating it inside the closure is visible outside, and later outer writes are visible inside. A `for` counter is captured **fresh per iteration** (each closure keeps its own iteration's value); a variable declared outside a `while` is a single shared cell across iterations.

```serez
fn any makeCounter() {
    let n = 0;
    return () => { n = n + 1; return n; };   // mutation persists across calls
}
let c = makeCounter();
out c();   // → 1
out c();   // → 2
```

---

### Enums

`enum` declares a named set of variants. Variants are accessed as `EnumName.VariantName` and are stored as strings internally.

```serez
enum Direction { North, South, East, West }
enum Color     { Red, Green, Blue }

let d = Direction.North;
let c = Color.Green;

out d;   // → North
out c;   // → Green

if (d == Direction.North) {
    out "Heading north!";
}
```

Enum variants can be used anywhere a value is expected — in arrays, dicts, function arguments, and switch cases:

```serez
enum Status { Ok, Error, Pending }

fn string describe(any s) {
    switch (s) {
        case Status.Ok:      { return "All good"; }
        case Status.Error:   { return "Something failed"; }
        case Status.Pending: { return "Still waiting"; }
        default:             { return "Unknown"; }
    }
}

out describe(Status.Ok);      // → All good
out describe(Status.Error);   // → Something failed
```

---

### Set

`Set` is an unordered collection of unique values. Duplicate elements are silently ignored on insertion.

#### Creating a Set

```serez
let s = new Set();                    // empty set
let s2 = new Set([1, 2, 3, 2, 1]);   // initialized from array — duplicates removed
out s2;   // → Set{1, 2, 3}
```

#### Methods

| Method | Returns | Description |
|---|---|---|
| `.size` | `int` | Number of elements (property, no parentheses). |
| `.add(val)` | `Set` | Inserts `val` if not already present (mutates in-place). |
| `.has(val)` / `.contains(val)` | `bool` | `true` if the set contains `val`. |
| `.delete(val)` / `.remove(val)` | `bool` | Removes `val`, returns `true` if it was present. |
| `.clear()` | `null` | Removes all elements. |
| `.toArray()` | array | Returns all elements as an array (unordered). |
| `.union(other)` | `Set` | New set with all elements from both sets. |
| `.intersection(other)` | `Set` | New set with only elements present in both. |

```serez
let a = new Set([1, 2, 3, 4]);
let b = new Set([3, 4, 5, 6]);

out a.size;              // → 4
out a.has(2);            // → true
out a.has(99);           // → false

a.add(5);
out a.size;              // → 5

a.delete(1);
out a.toArray();         // → [2, 3, 4, 5]  (order may vary)

out a.union(b);          // → Set{2, 3, 4, 5, 6}
out a.intersection(b);   // → Set{3, 4, 5}
```

---

### Math

`Math` is a built-in namespace for mathematical functions. All functions are called as `Math.functionName(args)`.

#### Constants

| Constant | Value |
|---|---|
| `Math.PI` | `3.141592653589793` |
| `Math.E`  | `2.718281828459045` |

#### Basic functions

| Function | Description |
|---|---|
| `Math.abs(x)` | Absolute value. |
| `Math.floor(x)` | Rounds down to nearest integer (returns `int`). |
| `Math.ceil(x)` | Rounds up to nearest integer (returns `int`). |
| `Math.round(x)` | Rounds to nearest integer (returns `int`). |
| `Math.trunc(x)` | Truncates toward zero (returns `int`). |
| `Math.sqrt(x)` | Square root (returns `decimal`). |
| `Math.pow(base, exp)` | `base` raised to `exp` (returns `decimal`). |
| `Math.exp(x)` | `e` raised to `x` (returns `decimal`). |
| `Math.log(x)` | Natural logarithm (base *e*). |
| `Math.log2(x)` | Logarithm base 2. |
| `Math.log10(x)` | Logarithm base 10. |
| `Math.sign(x)` | Returns `1`, `0`, or `-1`. |
| `Math.clamp(x, min, max)` | Clamps `x` to the `[min, max]` range. |
| `Math.min(a, b, ...)` | Smallest of one or more arguments. |
| `Math.max(a, b, ...)` | Largest of one or more arguments. |
| `Math.random()` | Pseudo-random `decimal` in `[0, 1)` (LCG generator). |

#### Trigonometric functions

All accept and return `decimal`. Angles are in radians.

| Function | Description |
|---|---|
| `Math.sin(x)` | Sine. |
| `Math.cos(x)` | Cosine. |
| `Math.tan(x)` | Tangent. |
| `Math.asin(x)` | Arc sine. Returns value in `[-π/2, π/2]`. |
| `Math.acos(x)` | Arc cosine. Returns value in `[0, π]`. |
| `Math.atan(x)` | Arc tangent. Returns value in `[-π/2, π/2]`. |
| `Math.atan2(y, x)` | Two-argument arc tangent. Returns angle in `(-π, π]`. |

```serez
out Math.PI;                    // → 3.141592653589793
out Math.sqrt(16.0);            // → 4.0
out Math.pow(2.0, 10.0);        // → 1024.0
out Math.abs(-7);               // → 7
out Math.floor(3.9);            // → 3
out Math.ceil(3.1);             // → 4
out Math.trunc(-3.9);           // → -3
out Math.clamp(15, 0, 10);      // → 10
out Math.min(3, 1, 4, 1, 5);   // → 1
out Math.max(3, 1, 4, 1, 5);   // → 5

out Math.sin(Math.PI / 2.0);   // → 1.0
out Math.cos(0.0);              // → 1.0
out Math.atan2(1.0, 1.0);      // → 0.7853981633974483  (π/4)
```

---

### File

`File` is a built-in namespace for file I/O operations.

| Function | Description |
|---|---|
| `File.exists(path)` | Returns `true` if the file at `path` exists. |
| `File.read(path)` | Returns the full file contents as a `string`. Runtime error if the file cannot be read. |
| `File.write(path, content)` | Writes `content` (converted to string) to `path`. Creates the file if it does not exist; overwrites if it does. Returns `null`. |
| `File.create(path)` | Creates an empty file at `path` if it does not already exist (touch). No-op if the file exists. Returns `null`. |
| `File.read_asBinary(path)` | Returns the raw bytes of the file as a `[int]` array (each byte as an integer 0–255). |
| `File.write_asBinary(path, bytes)` | Writes a `[int]` array of bytes to `path`. |
| `File.listDir(path)` | Returns a `[string]` array with the names of entries in the directory at `path`. |
| `File.mkdir(path)` | Creates a directory (and all intermediate directories) at `path`. |
| `File.stat(path)` | Returns an object `{ size: int, modified: int, isDir: bool }` with file metadata. `modified` is a Unix timestamp in ms. |
| `File.delete(path)` ⚠️ | Deletes a file or directory recursively. **Requires `unsafe {}` block.** |
| `File.rename(from, to)` ⚠️ | Moves/renames a file or directory. **Requires `unsafe {}` block.** |

```serez
File.write("hello.txt", "Hello, world!");
out File.exists("hello.txt");         // → true
out File.read("hello.txt");           // → Hello, world!

let bytes = File.read_asBinary("hello.txt");
out bytes.length;                     // → 13

File.create("empty.txt");
out File.exists("empty.txt");         // → true
```

---

### JSON

`JSON` is a built-in namespace for serializing and deserializing data.

| Function | Description |
|---|---|
| `JSON.stringify(value)` | Converts any value (int, decimal, bool, string, array, dict, null) to a compact, single-line JSON string. |
| `JSON.parse(string)` | Parses a JSON string and returns the equivalent Serez-Code value. Runtime error on invalid JSON. |
| `JSON.pretty(value, [indent])` | Like `stringify`, but indented for readability. `indent` is the number of spaces per level (default `2`; `0` falls back to compact). If `value` is a raw JSON string (e.g. a `fetch` body), it is parsed first and then re-indented. |

```serez
let data <string,any> = ({"name","Sergio"},{"age",30},{"active",true});

let json = JSON.stringify(data);
out json;   // → {"name":"Sergio","age":30,"active":true}

let parsed = JSON.parse(json);
out parsed["name"];   // → Sergio
out parsed["age"];    // → 30

let arr = JSON.stringify([1, 2, 3]);
out arr;              // → [1,2,3]
```

**Pretty-printing JSON** — handy when inspecting a `fetch` response in the console:

```serez
native fn string fetch(string url);

let body = fetch("https://api.example.com/data");
out JSON.pretty(body);       // parses the raw body and prints it indented (2 spaces)
out JSON.pretty(body, 4);    // 4-space indent

// Also works on structured values directly:
out JSON.pretty(data);
// → {
//     "name": "Sergio",
//     "age": 30,
//     "active": true
//   }
```

---

### Networking (fetch)

`fetch` is a general-purpose HTTP client. Declare it once as a `native fn`, then call it. Only `http://` and `https://` URLs are allowed; URLs/headers with control characters are rejected (CRLF / header-injection safe).

```serez
native fn string fetch(string url);

let body = fetch("https://pokeapi.co/api/v2/pokemon/ditto");
out JSON.pretty(body);
```

**Signature:** `fetch(url, [method], [body], [options])`. Arguments after the url are sniffed by type — the first string is the **method**, the second is the **body**, and a dict is the **options** — so `fetch(url, opts)`, `fetch(url, "POST", body)` and `fetch(url, "POST", body, opts)` all work.

```serez
// POST with a JSON body (Content-Type defaults to application/json when a body is sent)
let res = fetch("https://example.com/api", "POST", "\{\"name\":\"serez\"}");
```

**Default headers sent automatically:**

| Header | Value | When |
|---|---|---|
| `User-Agent` | `Serez-Code/<version>` | Always, unless you set your own. Without it some CDNs/WAFs reply `503`. |
| `Content-Type` | `application/json` | Only when a body is sent and you didn't set one. |

Both are overridable via the `headers` option (a caller-provided value always wins).

**Options dict** (passed as a `<string, any>` dict, e.g. `({"full", true})`):

| Key | Type | Effect |
|---|---|---|
| `headers` | `<string, string>` | Extra request headers (`Authorization`, `Accept`, cookies, a custom `User-Agent`, …). |
| `timeout` | `int` | Request timeout in seconds (default **60**; connect capped at 30). |
| `full` | `bool` | Return a dict `{ status, ok, statusText, headers, body }` and **do not throw** on HTTP status — so 4xx/5xx can be inspected. `headers` is keyed by lowercased name; a missing key reads as `null`. |
| `binary` | `bool` | Return the body as a byte array `[int]` (0–255) instead of a UTF-8 string, so images/zips/PDFs download intact. Decode with `Binary.toUtf8` / `Binary.toHex`. |

```serez
native fn any fetch(string url, any options);

let auth <string, string> = ({"Authorization", "Bearer TOKEN"});
let opts <string, any> = ({"headers", auth}, {"full", true}, {"timeout", 10});

let r = fetch("https://pokeapi.co/api/v2/pokemon/ditto", opts);
if (r["ok"] == true) {
    out "status " + r["status"];          // 200
    out JSON.pretty(r["body"]);           // pretty-print the JSON body
}
```

**Default mode** (no `full`) returns the body string and **throws on status ≥ 400**, embedding the response body in the thrown message — so wrap network calls in `try / catch`:

```serez
try {
    let body = fetch("https://pokeapi.co/api/v2/pokemon/ditto");
    out body.length();
} catch (e) {
    out "request failed: " + e;
}
```

---

### Socket (TCP & WebSocket)

Raw TCP client/server sockets over `std::net`, plus RFC 6455 WebSocket text frames. These are the low-level networking primitives — for a full HTTP/WebSocket server with routing, use the `serez-http` package. No permission declaration is required.

```serez
// TCP client
let sock = Socket.connect("example.com", 80);   // → socket id (int)
Socket.send(sock, "GET / HTTP/1.0\r\nHost: example.com\r\n\r\n");
let reply = Socket.recv(sock, 4096);            // read up to 4096 bytes → string
Socket.close(sock);

// TCP server
let server = Socket.listen(8080);   // → listener id
let conn   = Socket.accept(server); // blocks until a client connects → socket id
let msg    = Socket.recv(conn, 1024);
Socket.send(conn, "echo: " + msg);
Socket.close(conn);
Socket.close(server);

// WebSocket text frames (after a connection is established)
Socket.sendWsFrame(conn, "ping");        // encode + send a text frame → null
let frame = Socket.recvWsFrame(conn);    // → text payload, or null on close
```

| Method | Returns | Description |
|--------|---------|-------------|
| `Socket.connect(host, port)` | `int` | Open a TCP connection → socket id |
| `Socket.send(id, data)` | `int` | Send a string → bytes written |
| `Socket.recv(id, max_bytes)` | `string` | Read up to `max_bytes` |
| `Socket.listen(port)` | `int` | Bind + listen → listener id |
| `Socket.accept(listener_id)` | `int` | Accept a connection (blocks) → socket id |
| `Socket.close(id)` | `null` | Close a socket or listener |
| `Socket.sendWsFrame(id, data)` | `null` | Send a WebSocket text frame |
| `Socket.recvWsFrame(id)` | `string \| null` | Read one WebSocket text frame (null on close); frames > 16 MiB are rejected |

---

### Autodiff & Tensors

Serez-Code has a built-in reverse-mode automatic differentiation engine and multi-dimensional tensor type. No imports needed.

```serez
// Weight initialization
let w = Autodiff.heNormal([128, 64])
let b = Tensor.zeros([1, 64])
let m = Tensor.zeros([128, 64])
let v = Tensor.zeros([128, 64])

// Training loop
let step = 0
while step < 1000 {
    step++
    Autodiff.tape()
    let out = x.matmul(w).broadcastAdd(b).relu()
    let loss = Autodiff.crossEntropyLoss(out, targets)
    Autodiff.backward(loss)

    let grad_w = Autodiff.gradient(w)
    let grad_b = Autodiff.gradient(b)

    // Adam optimizer
    let rw = Autodiff.adamStep(w, grad_w, m, v, step, 0.001)
    w = rw[0]; m = rw[1]; v = rw[2]
}

// Save trained weights
Autodiff.saveWeights("model.szw", [w, b])

// Load later
let weights = Autodiff.loadWeights("model.szw")
```

**Optimizers:** `adamStep`, `adamwStep`, `sgdStep`, `rmspropStep`

**Loss functions:** `mseLoss`, `maeLoss`, `bceLoss`, `crossEntropyLoss`

**Weight init:** `xavierUniform`, `xavierNormal`, `heUniform`, `heNormal`

**Layers:** `batchNorm`, `dropout`, `embedding`

**Gradient utils:** `clipGrad`, `clipGradNorm`, `stopGrad`

**Tensor activations (all tracked):** `relu`, `sigmoid`, `tanh`, `softmax`, `gelu`, `leaky_relu`, `elu`, `swish`, `silu`, `mish`

**Tensor N-D ops:** `permute`, `unsqueeze`, `squeeze`, `broadcastTo`, `broadcastAddNd`, `broadcastMulNd`, `bmm`, `reduceSum`, `reduceMean`, `reduceMax`

---

### Package Manager

```bash
sz init          # create serez.json interactively
sz init --y      # create serez.json using folder name (no prompts)
sz install pkg   # install package into ./packages/
sz uninstall pkg # remove package
sz run dev       # execute script from serez.json
sz run build     # execute build script
```

`serez.json` supports a `scripts` field:

```json
{
  "name": "my-app",
  "version": "1.0.0",
  "scripts": {
    "dev": "sz index.sz",
    "build": "sz apipack build"
  },
  "dependencies": {
    "serez-ai": "1.0.0"
  }
}
```

---

### GPU

CPU-backed compute buffers with a GPU-shaped API. Buffers are flat `decimal` arrays; the create / upload / dispatch / readback / free pattern mirrors real GPU compute so a future backend can swap the CPU implementation for actual GPU calls. Buffers are **not** garbage-collected — free them with `GPU.freeBuffer` when done. No permission declaration is required.

```serez
let src     = GPU.createBufferFromArray([1.0, 2.0, 3.0, 4.0]);  // → buffer id
let doubled = GPU.map(src, x => x * 2.0);                 // element-wise → new buffer
let sum     = GPU.reduce(src, (acc, x) => acc + x, 0.0);  // → 10.0
let product = GPU.reduce(src, (acc, x) => acc * x, 1.0);  // → 24.0

let d = GPU.dot(src, doubled);          // dot product → decimal
let r = GPU.axpy(2.0, src, doubled);    // 2*src + doubled → new buffer

// Matrix multiply: [2×2] @ [2×2]
let I = GPU.createBufferFromArray([1.0, 0.0, 0.0, 1.0]);
let M = GPU.createBufferFromArray([5.0, 6.0, 7.0, 8.0]);
let C = GPU.matmul(I, 2, 2, M, 2, 2);
out GPU.readBuffer(C);   // → [5.0, 6.0, 7.0, 8.0]

GPU.freeBuffer(src);
GPU.freeBuffer(doubled);
```

| Method | Returns | Description |
|--------|---------|-------------|
| `GPU.createBuffer(size)` | `int` | Allocate a zero-filled buffer → id |
| `GPU.createBufferFromArray(arr)` | `int` | Allocate from a Serez array → id |
| `GPU.readBuffer(id)` | `[decimal]` | Copy a buffer back to a Serez array |
| `GPU.freeBuffer(id)` | `null` | Release a buffer |
| `GPU.fill(id, value)` | `null` | Set every element to `value` |
| `GPU.size(id)` | `int` | Number of elements |
| `GPU.map(id, fn)` | `int` | Element-wise `fn` → new buffer |
| `GPU.reduce(id, fn, initial)` | `decimal` | Fold over the buffer → scalar |
| `GPU.dot(id_a, id_b)` | `decimal` | Dot product of two buffers |
| `GPU.axpy(alpha, id_x, id_y)` | `int` | `alpha*x + y` → new buffer |
| `GPU.matmul(id_a, ra, ca, id_b, rb, cb)` | `int` | Matrix multiply → new buffer |

---

### Crypto

Hashing, encodings, a real CSPRNG, and Ed25519 signatures. Pure compute — no
permission declaration required. Hashes and encodings are implemented in pure
Rust; the security-critical primitives use vetted crates (`getrandom` for OS
entropy, `ed25519-dalek` for signatures).

> ⚠️ **`Random.*` is a seedable LCG — predictable.** Fine for games and
> simulations; never use it for tokens, salts, or keys. Use
> `Crypto.randomBytes` for anything secret.

```serez
// Hashing & encodings
out Crypto.sha256("hola");                  // hex digest
out Crypto.hmacSha256("clave", "mensaje");  // HMAC hex
out Crypto.base64encode("serez");           // "c2VyZXo="
out Crypto.hexEncode([222, 173, 190, 239]); // "deadbeef"

// CSPRNG: token de sesión de 32 bytes
let token = Crypto.hexEncode(Crypto.randomBytes(32));

// Firmas Ed25519
let kp  = Crypto.ed25519Keypair();          // { private, public } en hex
let sig = Crypto.ed25519Sign(kp["private"], "payload");
out Crypto.ed25519Verify(kp["public"], "payload", sig);    // true
out Crypto.ed25519Verify(kp["public"], "alterado", sig);   // false
```

| Method | Returns | Description |
|--------|---------|-------------|
| `Crypto.sha256(s)` | `string` | SHA-256 hex digest |
| `Crypto.sha1(s)` | `string` | SHA-1 hex digest (legacy interop, e.g. WebSocket handshake) |
| `Crypto.sha1base64(s)` | `string` | SHA-1 + base64 (WebSocket `Sec-WebSocket-Accept`) |
| `Crypto.md5(s)` | `string` | MD5 hex digest (legacy interop only — not for security) |
| `Crypto.hmacSha256(key, data)` | `string` | HMAC-SHA256 hex |
| `Crypto.base64encode(s)` / `base64decode(s)` | `string` | Base64; decode throws on invalid input |
| `Crypto.hexEncode(bytes)` / `hexDecode(hex)` | `string` / `[int]` | Bytes ↔ hex; decode throws on invalid input |
| `Crypto.randomBytes(n)` | `[int]` | **CSPRNG** — n bytes (0..255) from OS entropy. 1 ≤ n ≤ 1 MB, throws outside the range |
| `Crypto.ed25519Keypair()` | `dict` | `{ private, public }` — 64-char hex strings (32 bytes each) |
| `Crypto.ed25519Sign(privHex, msg)` | `string` | 128-char hex signature; deterministic. Throws on malformed key |
| `Crypto.ed25519Verify(pubHex, msg, sigHex)` | `bool` | Strict verification. Throws on malformed hex/lengths; `false` on invalid signature |

---

### Terminal

`Terminal` interacts with the terminal emulator directly. **Requires `use permissions { Terminal }` or a project-level `"permissions": ["Terminal"]` in `serez.json`.**

| Function | Description |
|---|---|
| `Terminal.getSize()` | Returns `[cols, rows]` — current terminal dimensions in characters. |
| `Terminal.clear()` | Clears the screen. |
| `Terminal.setCursor(row, col)` | Moves the cursor to the given position (0-indexed). |
| `Terminal.writeByte(int)` | Writes a single byte to stdout. Useful for ANSI escape sequences. |
| `Terminal.setRawMode(bool)` ⚠️ | Enables or disables raw mode (no line buffering, no echo). **Requires `unsafe {}`**. |
| `Terminal.readByte()` → `int` ⚠️ | Reads one raw byte from stdin. **Requires `unsafe {}`**. |
| `Terminal.enableMouse(bool)` ⚠️ | Enables or disables mouse event reporting. **Requires `unsafe {}`**. |
| `Terminal.readEvent()` ⚠️ | Reads the next terminal event (key or mouse). Returns a `KeyEvent`, `MouseEvent`, or `ResizeEvent`. **Requires `unsafe {}`**. |

**Event objects returned by `Terminal.readEvent()`:**

```serez
// Key event
{ type: "key", code: "a", modifiers: ["ctrl"] }

// Mouse event
{ type: "mouse", kind: "down", button: "left", col: 10, row: 5, modifiers: [] }

// Resize event
{ type: "resize", cols: 120, rows: 40 }
```

`code` can be a character (`"a"`, `"A"`) or a named key (`"Enter"`, `"Esc"`, `"Up"`, `"Down"`, `"Left"`, `"Right"`, `"Tab"`, `"Backspace"`, `"Delete"`, `"F1"`–`"F12"`, etc.).
`kind` for mouse: `"down"`, `"up"`, `"drag"`, `"move"`, `"scrollDown"`, `"scrollUp"`.
`button`: `"left"`, `"right"`, `"middle"`, `"none"`.

```serez
use permissions { Terminal }

let size = Terminal.getSize()
out "Columns: {size[0]}, Rows: {size[1]}"

unsafe {
    Terminal.setRawMode(true)
    Terminal.enableMouse(true)

    let evt = Terminal.readEvent()
    if (evt.type == "key") {
        out "Key pressed: {evt.code}"
    } else if (evt.type == "mouse") {
        out "Mouse {evt.kind} at col={evt.col} row={evt.row}"
    }

    Terminal.enableMouse(false)
    Terminal.setRawMode(false)
}
```

---

### OS

`OS` provides access to operating system process information. **Requires `use permissions { OS }`.**

| Function | Description |
|---|---|
| `OS.platform()` | Returns `"windows"`, `"linux"`, or `"macos"`. |
| `OS.pid()` | Returns the current process ID as `int`. |
| `OS.exec(cmd, args)` ⚠️ | Executes an external command. Returns `{ stdout: string, stderr: string, code: int }`. **Requires `unsafe {}`**. Blocked for system paths (`C:\Windows\System32`, `/etc/`, etc.). |
| `OS.kill(pid)` ⚠️ | Terminates a process by PID. **Requires `unsafe {}`**. |

```serez
use permissions { OS }

out OS.platform()   // → windows
out OS.pid()        // → 12345

let result = null
unsafe {
    result = OS.exec("git", ["status"])
}
out result.code     // → 0
out result.stdout   // → On branch improve...
```

---

### Env

`Env` reads and writes environment variables and program arguments. **Requires `use permissions { Env }`.**

| Function | Description |
|---|---|
| `Env.get(key)` | Returns the value of environment variable `key`, or `null` if not set. |
| `Env.args()` | Returns a `[string]` array of command-line arguments (including the program name). |
| `Env.set(key, value)` ⚠️ | Sets an environment variable. **Requires `unsafe {}`**. |

```serez
use permissions { Env }

let path = Env.get("PATH")
out path

let args = Env.args()
out args.length   // → number of CLI arguments

unsafe {
    Env.set("MY_VAR", "hello")
}
out Env.get("MY_VAR")   // → hello
```

---

### Time

`Time` provides time and sleep utilities. **Requires `use permissions { Time }`.**

| Function | Description |
|---|---|
| `Time.now()` | Returns the current Unix timestamp in **milliseconds** as `int`. |
| `Time.sleep(ms)` | Pauses execution for `ms` milliseconds. |

```serez
use permissions { Time }

let t1 = Time.now()
Time.sleep(100)
let t2 = Time.now()
out t2 - t1   // → ~100 (ms elapsed)
```

---

### DateTime

`DateTime` is a calendar date/time built on `chrono`. It is **immutable**: every
operation returns a *new* `DateTime`. Reading the wall clock (`now`, `utcNow`)
**requires `use permissions { Time }`**; pure construction (`from`, `fromEpoch`)
and any operation on an existing value need **no permission**.

**Construction**

| Function | Description |
|---|---|
| `DateTime.now()` | Current **local** date/time. Requires `Time`. |
| `DateTime.utcNow()` | Current **UTC** date/time. Requires `Time`. |
| `DateTime.from(y, m, d, [h, mi, s, ms])` | Build from fields (3–7 ints). Rejects invalid dates (e.g. `Feb 30`). |
| `DateTime.fromEpoch(ms)` | Build from a millisecond Unix timestamp. |

**Fields** — each returns a `DateField` that behaves as an `int` under operators
but carries `.add(n)` / `.reduce(n)` / `.remove(n)` returning a **new** `DateTime`:

| Field | Meaning |
|---|---|
| `.year .month .day .hour .minute .second .ms` | Calendar components (month is 1-indexed). |
| `.weekday` | 1 = Monday … 7 = Sunday (`int`). |
| `.dayOfYear` | 1–366 (`int`). |
| `.daysInMonth` | Days in the current month (`int`). |

**Immutable arithmetic** — day/hour/minute/second/ms shift the instant; month/year
adjust field-wise and **clamp the day** to the end of the resulting month:

```serez
let d = DateTime.from(2026, 1, 31, 9, 30, 0)
out d.day.add(5)            // 2026-02-05T09:30:00
out d.month.add(1)          // 2026-02-28T09:30:00  (31 clamped to 28)
out d.month.reduce(1)       // 2025-12-31T09:30:00
out d.day + 5               // 36   (DateField acts as int)
```

**Methods & formatting**

| Member | Description |
|---|---|
| `.format(pattern)` | moment.js-style: `YYYY YY MM M DD D HH H hh h mm m ss s SSS A`; `[text]` is literal. |
| `.toString()` / `.iso()` | ISO 8601 (`Z` suffix when UTC). |
| `.timestamp()` / `.toEpoch()` / `.epochMillis()` | Millisecond epoch (`int`). |
| `.isLeapYear()` / `.isUtc()` | `bool`. |
| `.add/.reduce/.remove(n)` *(on a field)* | Immutable add/subtract; `remove` == `reduce`. |

```serez
let d = DateTime.from(2026, 6, 20, 14, 30, 0)
out d.format("YYYY-MM-DD HH:mm")   // 2026-06-20 14:30
out d.format("D/M/YYYY h:mm A")    // 20/6/2026 2:30 PM
out d.weekday                       // 6  (Saturday)

// Object-destructuring exposes the calendar fields as plain ints
const {day, month, year} = DateTime.from(2026, 6, 20)
out year + "-" + month + "-" + day  // 2026-6-20
```

Two `DateTime`s compare by instant (`<`, `>`, `==`, …); arithmetic between two
dates is not allowed — operate through their fields.

---

### Regex

`Regex` is a dependency-free regular-expression engine. Write patterns as **raw
strings** (`r"…"`) so backslashes reach the engine verbatim. No permission is
required (it is pure computation).

```serez
Regex.test(r"\d+", "abc123");                 // → true   (matches anywhere)
Regex.test(r"^\d+$", "12345");                // → true   (anchored)
Regex.match(r"(\w+)@(\w+)\.(\w+)", "joe@x.com"); // → [joe@x.com, joe, x, com]  (or null)
Regex.findAll(r"\d+", "a1b22c333");           // → [1, 22, 333]
Regex.split(r",\s*", "a, b,c");               // → [a, b, c]
Regex.replace(r"\d+", "a1b22", "#");          // → a#b#
Regex.replace(r"(\w+)@(\w+)", "joe@corp", "$2.$1"); // → corp.joe
```

| Method | Returns |
|--------|---------|
| `Regex.test(pattern, text)` | `bool` — does it match anywhere |
| `Regex.match(pattern, text)` | `[whole, group1, …]` of the first match, or `null` (absent optional groups are `null`) |
| `Regex.findAll(pattern, text)` | array of all non-overlapping matches |
| `Regex.split(pattern, text)` | array split on matches |
| `Regex.replace(pattern, text, repl)` | string, replacing all matches (`$0`/`$&` = whole match, `$1`…`$9` = groups, `$$` = literal `$`) |

**Supported syntax:** literals, `.` (any char except newline), `\d \D \w \W \s \S`
and escapes (`\. \\ \n \t \r`), character classes `[abc]` `[a-z]` `[^…]`, anchors
`^` `$`, groups `( … )` and non-capturing `(?: … )`, alternation `|`, and
quantifiers `* + ?` and `{n}` `{n,}` `{n,m}` — each optionally lazy (`*?`, `+?`).
The engine is bounded (step budget) so a pathological pattern returns "no match"
instead of hanging. An invalid pattern raises a catchable error.

---

### System

`System` provides read-only system information. **Requires `use permissions { System }`.**

| Function | Description |
|---|---|
| `System.cpuCount()` | Number of logical CPU cores available. |
| `System.totalMemory()` | Total physical RAM in bytes. |
| `System.freeMemory()` | Available physical RAM in bytes. |
| `System.hostname()` | The machine hostname as `string`. |
| `System.uptime()` | Seconds since system boot as `int`. |

```serez
use permissions { System }

out System.cpuCount()      // → 15
out System.totalMemory()   // → 34279034880  (bytes)
out System.hostname()      // → DESKTOP-XYZ
out System.uptime()        // → 168517  (seconds)
```

---

### Gui

`Gui` opens a native OS window and draws on a CPU pixel framebuffer (`0xRRGGBB`). It is a **real graphical interface** (not the terminal): pixels, mouse, and keyboard. Backed by `winit` (windowing), `softbuffer` (presentation) and `cosmic-text` (real glyph rasterization — accents, `ñ`, Unicode). **Requires `use permissions { Gui }`.** No `unsafe` needed.

The model is poll/present: each frame you `clear`, draw, `present`, then read input. Call these in a loop driven by `Gui.isOpen()`.

| Function | Description |
|---|---|
| `Gui.open(title, w, h)` | Opens a resizable window with a `w`×`h` framebuffer. |
| `Gui.isOpen()` | Returns `bool` — `false` once the window is closed. |
| `Gui.close()` | Closes the window and frees its state. |
| `Gui.size()` | Returns `[w, h]` — current framebuffer size (tracks resizes). |
| `Gui.present()` | Pushes the framebuffer to the window and pumps input events. |
| `Gui.setTitle(title)` | Changes the window title. |
| `Gui.setCursor(name)` | Sets the mouse cursor (`"default"`, `"text"`, `"hand"`, `"crosshair"`, `"wait"`, `"not-allowed"`). |
| `Gui.clear(color)` | Fills the whole buffer with `color`; reallocates on window resize. |
| `Gui.fillRect(x, y, w, h, color)` | Fills a rectangle (clipped to the buffer). |
| `Gui.fillRectAlpha(x, y, w, h, color, alpha)` | Alpha-blended rectangle (`alpha` 0–255). |
| `Gui.fillRoundRect(x, y, w, h, radius, color)` | Filled rectangle with antialiased rounded corners. |
| `Gui.setPixel(x, y, color)` | Sets a single pixel. |
| `Gui.drawLine(x0, y0, x1, y1, color)` | Draws a line (Bresenham). |
| `Gui.drawText(x, y, text, scale, color)` | Draws text with the current font (see fonts below). |
| `Gui.measureText(text, scale)` | Returns `[w, h]` in pixels for the given text with the current font. |
| `Gui.loadFont(path)` | Loads a `.ttf`/`.otf` file and returns its **family name**. Works before `open`. |
| `Gui.setFont(family)` | Selects a font family (loaded or system-installed). `""`/`"default"`/`"monospace"` resets. Returns `bool` (family found). |
| `Gui.font()` | Returns the current family name (`""` = default). |
| `Gui.pushClip(x, y, w, h)` / `Gui.popClip()` | Nestable clip rectangles for drawing. |
| `Gui.loadImage(path)` | Loads a PNG/JPG; returns an `int` handle. |
| `Gui.drawImage(x, y, handle)` | Blits a loaded image (alpha-blended). |
| `Gui.imageSize(handle)` | Returns `[w, h]` of a loaded image. |
| `Gui.mouse()` | Returns `[x, y]` — mouse position (clamped to the window). |
| `Gui.mouseDown()` / `Gui.mouseRightDown()` / `Gui.mouseMiddleDown()` | `bool` — button held. |
| `Gui.mousePressed()` | Returns `bool` — left button **clicked this frame** (edge). |
| `Gui.scroll()` | Returns `[dx, dy]` — scroll wheel delta this frame. |
| `Gui.keyDown(name)` | `bool` — named key or modifier (`"Shift"`, `"Ctrl"`, `"Alt"`) currently held. |
| `Gui.keysPressed()` / `Gui.keysRepeated()` / `Gui.keysReleased()` | `[name]` — key edges this frame (with auto-repeat in `keysRepeated`). |
| `Gui.charsTyped()` | Returns the `string` of characters typed this frame — native OS keyboard layout and IME (accents work). |
| `Gui.clipboardGet()` / `Gui.clipboardSet(text)` | Read / write the system clipboard. |

`color` is an `int` in `0xRRGGBB` form. Key names match `Terminal`: characters (`"a"`), digits, and `"Enter"`, `"Esc"`, `"Space"`, `"Backspace"`, `"Tab"`, `"Delete"`, `"Left"`/`"Right"`/`"Up"`/`"Down"`, `"Home"`, `"End"`.

> **Fonts:** the default font draws on a fixed monospace grid of `8 × scale` px per character (`measureText` = chars × 8 × scale — stable for layout math). After `Gui.setFont(family)` with a loaded (`Gui.loadFont`) or system-installed family, `drawText`/`measureText` switch to **real proportional rendering** with per-glyph advances. Reset with `Gui.setFont("")`.

```serez
use permissions { Gui }

Gui.open("Mi App", 480, 320)

let name = ""

while (Gui.isOpen()) {
    if (Gui.keyDown("Esc")) { break }

    // Input
    name = name + Gui.charsTyped()
    let keys = Gui.keysPressed()
    let i = 0
    while (i < keys.length()) {
        if (keys[i] == "Backspace" && name.length() > 0) {
            name = name.substring(0, name.length() - 1)
        }
        i = i + 1
    }

    // Draw
    Gui.clear(0x0f172a)
    Gui.fillRect(20, 20, 200, 48, 0x3b82f6)
    Gui.drawText(36, 36, "Hola", 2, 0xffffff)
    Gui.drawText(20, 100, name + "_", 2, 0xe2e8f0)

    let m = Gui.mouse()
    if (Gui.mousePressed()) {
        out "click en {m[0]},{m[1]}"
    }

    Gui.present()
}

Gui.close()
```

See `apps/09_gui_window.sz` for a full graphical form (text field + clickable button).

#### Multiple windows

Open extra windows alongside the main one. The window from `Gui.open` is id `0`; `Gui.openWindow` returns a new id. `Gui.selectWindow(id)` makes all subsequent drawing and input calls apply to that window — each has its own canvas and input.

| Function | Description |
|---|---|
| `Gui.openWindow(title, w, h)` | Opens a secondary window; returns its `int` id (≥ 1). |
| `Gui.selectWindow(id)` | Directs drawing/input to window `id` (`0` = the main window). |
| `Gui.currentWindow()` | Returns the id of the selected window. |
| `Gui.closeWindow(id)` | Closes a secondary window. |

#### Retained-mode (scene graph)

Instead of clearing and redrawing every frame (immediate mode), declare **persistent nodes** once and mutate their properties; the core redraws them natively. `Gui.renderScene(bg)` repaints **only if the scene changed** (dirty-skip) and returns `bool` (`true` if it repainted). The scene is per-window.

| Function | Description |
|---|---|
| `Gui.nodeRect(x, y, w, h, color)` | Creates a node; returns its `int` id. Also `nodeRoundRect`, `nodeRectAlpha`, `nodeRectOutline`, `nodeCircle`, `nodeLine`, `nodePolyline`, `nodePolygon`, `nodeText`, `nodeImage`, `nodeClipPush`/`nodeClipPop`. |
| `Gui.nodeSet(id, prop, value)` | Updates a property: `x, y, w, h, r, x2, y2, color, z, visible, text, scale, font, style, spacing, radius, alpha, width, points`. |
| `Gui.nodeDelete(id)` / `Gui.sceneClear()` | Remove one node / all nodes. |
| `Gui.nodeCount()` | Number of active nodes. |
| `Gui.renderScene(bg)` | Repaints the scene if dirty and presents; returns `bool` (repainted?). |

```serez
use permissions { Gui }

Gui.open("Scene", 640, 480)
let box = Gui.nodeRect(100, 100, 200, 150, 0x3b82f6)
Gui.nodeText(100, 300, "Persistent", 2, 0xffffff)

while (Gui.isOpen()) {
    Gui.nodeSet(box, "x", 100 + Gui.time() / 20)   // animate by mutating
    Gui.renderScene(0x0f172a)                       // redraws only if changed
    Gui.idleWait(16)
}
Gui.close()
```

#### Primitives engine (HTML/CSS-like)

Instead of drawing rectangles yourself, hand the core a **tree of HTML-like
primitives plus a CSS stylesheet** and let it do style resolution, layout and
painting natively — the browser model. One call lays out the tree and rebuilds
the retained scene; `Gui.renderScene(bg)` paints it. Layout + CSS for a real
app-sized tree runs in ~0.05 ms, roughly **1000× faster** than doing the same
walk in interpreted code. This is the engine behind `serez-ui`'s native
renderer, and it is generic: the core knows tags, not widgets.

| Function | Description |
|---|---|
| `Gui.loadStylesheet(src)` | Parses CSS text; returns an `int` handle. |
| `Gui.loadSvg(srcOrPath)` | Parses SVG markup (or reads an `.svg` file); returns an `int` handle usable as `src` of `svg`/`img` nodes. |
| `Gui.renderTree(root, sheet, w, h[, ctx])` | Resolves CSS, lays out, rebuilds the scene and returns the clickable **regions** `[[tag, x, y, w, h, onClick], …]` in pre-order. `ctx` is a dict evaluated by reactive CSS conditions. |

Nodes are plain arrays — `[tag, [[prop, value], …], [children…]]` where children
are nodes or plain strings. Supported tags: `div`, `row`, `p`, `h1`–`h6`,
`span`, `b`, `i`, `hr`, `img`, `svg`, `circle`, `line`, `polyline`, `polygon`
and `textbox` (editable: caret, selection and line virtualization handled
natively). The CSS covers the familiar web subset: full box model (per-side
padding/margin and 1–4 value shorthands), `border` / `border-radius`, flexbox
(`justify-content`, `align-items`, `gap`, `flex` weights,
`flex-direction: column`), `position: absolute` (+ `left`/`top`/`bottom`/`right`
and `z-index` for overlays — an absolute node without `width` shrink-wraps to
its text, so `right:`-anchored badges just work), `width`/`height` in px / `%` /
`auto`, `overflow: scroll`, `opacity` (multiplicative down the subtree, text
included), `text-align`, `line-height`, `letter-spacing`,
`white-space: nowrap`, `font-weight`, `text-decoration`, `font-family` — with
custom fonts declared in `:font { alias: "path.ttf" }` blocks of the sheet and
resolved per node — and `display: none`. `color` and `font-size`/`font-scale`
**inherit** down the tree like on the web. Selectors: `tag`, `*`, `.class`,
`#id`, compounds (`tag.class#id`), descendant chains (`section span`), groups
(`h2, h3 { … }`), pseudo-states (`:hover`, `:focus`, `:active`, `:disabled` —
matched against same-named boolean attrs the framework marks on nodes;
`:active-focus` is an alias of `:focus`) and reactive conditions evaluated
against `ctx` (`(var == val)` with `==`/`!=`/`<`/`<=`/`>`/`>=`, or a bare
`(flag)` for truthiness) — last match wins. `img` takes a PNG/JPG **file path**
(auto-sized, aspect-preserving, cached) or a `Gui.loadSvg` handle.

```serez
use permissions { Gui }

Gui.open("Primitives", 480, 220)
// Raw string (r"…") because CSS braces would otherwise trigger string interpolation
let sheet = Gui.loadStylesheet(r".card { background: #1e293b; padding: 14; border-radius: 10 } h2 { color: #f1c40f } .btn { background: #3b82f6; color: #ffffff; padding: 10; border-radius: 6; width: 130 }")

let clicks = 0
fn void onBtn() { clicks = clicks + 1 }

while (Gui.isOpen()) {
    let tree = ["div", [["class", "card"]], [
        ["h2", [], ["Native engine"]],
        ["p", [], ["Clicks: {clicks}"]],
        ["div", [["class", "btn"], ["onClick", onBtn]], ["Click me"]]
    ]]
    let regions = Gui.renderTree(tree, sheet, 480, 220)
    Gui.renderScene(0x0f172a)

    // Hit-test: route the click to the region under the mouse
    if (Gui.mousePressed()) {
        let m = Gui.mouse()
        let i = 0
        while (i < regions.length()) {
            let r = regions[i]
            if (r[5] != null && m[0] >= r[1] && m[0] <= r[1] + r[3] && m[1] >= r[2] && m[1] <= r[2] + r[4]) {
                r[5]()
            }
            i = i + 1
        }
    }
    Gui.idleWait(16)
}
Gui.close()
```

---

### Media

`Media` plays audio files (WAV, MP3, FLAC, Vorbis) asynchronously. Each `playSound` returns an `int` id you use to control that sound. **Requires `use permissions { Media }`.**

| Function | Returns | Description |
|---|---|---|
| `Media.playSound(path)` | `int` | Starts a sound asynchronously; returns its id. |
| `Media.stop(id)` / `Media.stopAll()` | `bool` / — | Stop one sound / all sounds. |
| `Media.pause(id)` / `Media.resume(id)` | `bool` | Pause / resume a sound. |
| `Media.setVolume(id, volume)` | `bool` | Volume `0`–`200` (100 = normal). |
| `Media.isPlaying(id)` | `bool` | Is that sound currently playing? |
| `Media.playingCount()` | `int` | Number of sounds playing. |

```serez
use permissions { Media, Time }

let id = Media.playSound("chime.mp3")
Media.setVolume(id, 150)
while (Media.isPlaying(id)) { Time.sleep(50) }
```

A missing file throws a catchable `IOError`; an unsupported format or no audio device throws a catchable `MediaError`.

---

### Permissions

Serez-Code uses a **three-level permission model** to control access to OS, hardware, and destructive operations. Programs run in a sandbox by default — no OS access without an explicit opt-in.

#### Level 1 — Project-wide (`serez.json`)

Grants namespaces to every file in the project:

```json
{
  "name": "my-app",
  "version": "1.0.0",
  "permissions": ["Terminal", "OS", "Env", "Time", "System", "Gui"]
}
```

#### Level 2 — File-level (`use permissions {}`)

Grants additional namespaces for the current file only. Additive — cannot revoke project-level permissions.

```serez
use permissions { OS, File }
```

> `DateTime.now()` / `DateTime.utcNow()` read the clock and so reuse the **`Time`**
> permission. `DateTime.from()` / `.fromEpoch()` and all field/arithmetic/format
> operations are pure and need no permission.

#### Level 3 — Operation-level (`unsafe {}`)

Certain destructive or OS-modifying operations require an `unsafe {}` block even when the namespace is permitted:

| Operation | Why unsafe |
|---|---|
| `Terminal.setRawMode` | Modifies OS terminal state |
| `Terminal.readByte` | Reads raw input |
| `Terminal.enableMouse` | Modifies OS input mode |
| `Terminal.readEvent` | Reads raw input events |
| `OS.exec` | Executes external processes |
| `OS.kill` | Terminates processes |
| `Env.set` | Modifies environment (thread-unsafe) |
| `File.delete` | Permanently removes files |
| `File.rename` | Modifies the filesystem |

```serez
use permissions { OS, Env }

// Safe operations — no unsafe needed
out OS.platform()
out Env.get("HOME")

// Dangerous operations — unsafe required
unsafe {
    let result = OS.exec("echo", ["hello"])
    Env.set("BUILD", "release")
}
```

Without a declared permission, every namespace call fails immediately with a clear error pointing to how to grant it.

---

### Tasks (Multithreading)

By default, Serez-Code programs run sequentially. If you need to perform a slow or blocking operation—such as sending HTTP requests, reading large files, or running heavy calculations—without freezing your main application (which is critical to keep GUI apps running smoothly at 500 FPS), you can use the `Task` namespace to run scripts in the background.

A background task runs independently and communicates with your main script using text messages (typically formatted in JSON).

#### Required Permissions
You must declare `Task` permissions in `serez.json` or in your script using:
```serez
use permissions { Task, Time }
```

#### Step 1: Write the Worker Script
Create a separate script that will execute in the background (e.g., `worker.sz`). Use `Task.message()` to get the input argument, and `Task.reply()` to return the result:

```serez
// worker.sz
use permissions { Task }

// Retrieve the argument passed from the main thread
let input = Task.message()

// Do some calculations or IO...
let result = "Hello, " + input + "! This runs in parallel."

// Send the response back and exit the worker
Task.reply(result)
```

#### Step 2: Run and Poll from the Main Thread
In your main script, start the task with `Task.run()`. It will immediately return a task ID. You can check its status using `Task.isDone(id)` and retrieve the result with `Task.poll(id)`:

```serez
// main.sz
use permissions { Task, Time }

// 1. Spawns the worker in background
let taskId = Task.run("worker.sz", "Serez Developer")
out "Worker started with ID: {taskId}"

// 2. Do non-blocking polling
while (!Task.isDone(taskId)) {
    out "Waiting for worker..."
    Time.sleep(10) // Sleep 10ms to release CPU
}

// 3. Retrieve the result
let response = Task.poll(taskId)
out "Result: " + response
```

#### API Reference

| Method | Description |
|---|---|
| `Task.run(scriptPath: string, arg: string) -> int` | Spawns a background thread running the specified script. Returns the `taskId`. |
| `Task.message() -> string` | (Worker only) Retrieves the argument passed to the worker. |
| `Task.reply(result: string) -> void` | (Worker only) Sends the result back to the main thread and terminates the task. |
| `Task.isDone(taskId: int) -> bool` | Returns `true` if the task completed successfully or failed with an error. |
| `Task.poll(taskId: int) -> string` | Retrieves the result of the task. If the task failed or panicked, it returns a string starting with `"ERROR: "`. |

---

### Classes & Interfaces

Serez-Code supports C#-style object-oriented programming with interfaces, classes, single inheritance, and `super()` constructor delegation.

---

#### Interfaces

An `interface` defines a named record with typed fields. It is purely a data container — no methods. Create instances with `new`:

```serez
interface Punto {
    x: decimal,
    y: decimal,
}

let origen = new Punto({ x: 0.0, y: 0.0 });
let p      = new Punto({ x: 3.0, y: 4.0 });

out "{origen.x}, {origen.y}";   // → 0.0, 0.0
out "{p.x}, {p.y}";             // → 3.0, 4.0
```

All field names and types from the interface declaration must be supplied. Extra fields are a runtime error.

**Reading fields:**

```serez
out p.x;   // → 3.0
```

**Mutating fields:**

```serez
p.x = 10.0;
out p.x;   // → 10.0
```

**Partial object patch** — reassign selected fields at once without `let`:

```serez
p = { x: 5.0, y: 12.0 };   // overwrites only named fields; others unchanged
out "{p.x}, {p.y}";         // → 5.0, 12.0
```

The patch only overwrites the listed fields. Fields not listed keep their previous values.

---

#### Classes

A `class` bundles data and behaviour. Each class has a constructor (same name as the class, prefixed with `public`) and any number of `public` or `private` methods.

```serez
public class Animal {
    public Animal(string nombre, string sonido) {
        this.nombre  = nombre;
        this.sonido  = sonido;
        this.energia = 100;
    }

    public string getNombre() {
        return this.nombre;
    }

    public void hacer_sonido() {
        out "{this.nombre} dice: {this.sonido}";
    }

    public void comer(int cantidad) {
        this.energia = this.energia + cantidad;
    }

    public string describir() {
        return "{this.nombre} (energía: {this.energia})";
    }
}

let perro = new Animal("Rex", "Guau");
perro.hacer_sonido();          // → Rex dice: Guau
perro.comer(20);
out perro.describir();         // → Rex (energía: 120)
```

**Field assignment:**

Fields set inside the constructor via `this.field = value` are created automatically. Any method can read or write them with the same syntax:

```serez
perro.energia = 50;   // direct field mutation from outside
```

**Methods** are called with dot syntax and parentheses, just like built-in methods:

```serez
out perro.getNombre();   // → Rex
```

---

#### Inheritance

Use `: ParentClass` to inherit from another class. The child's constructor **must** call `super(args...)` before doing anything else — this executes the parent constructor body against the same `this` object.

```serez
public class Perro : Animal {
    public Perro(string nombre, string raza) {
        super(nombre, "Guau");   // runs Animal's constructor with this
        this.raza = raza;
    }

    public string getRaza() {
        return this.raza;
    }

    // Override the parent method:
    public string describir() {
        return "{this.nombre} [{this.raza}] (energía: {this.energia})";
    }
}

let fido = new Perro("Fido", "Labrador");
fido.hacer_sonido();        // → Fido dice: Guau  (inherited from Animal)
out fido.describir();       // → Fido [Labrador] (energía: 100)
out fido.getNombre();       // → Fido  (inherited)
out fido.getRaza();         // → Labrador
```

Inheritance is single — a class can have at most one parent.

**Method resolution** walks the chain from the most-derived class upward until the method is found:

```
Perro.describir()    → found in Perro — use it
Perro.hacer_sonido() → not in Perro → found in Animal — use it
```

**`super()` semantics:**

`super(args...)` runs the parent constructor's body against the same `this` that the child constructor received. Only the variables that the parent explicitly assigns to `this` inside its body are visible in the child. Grand-parent constructors are not automatically called by `super()` — each level must call `super()` explicitly if the chain needs to be continued.

Multi-level inheritance example:

```serez
public class Figura {
    public Figura(string nombre) {
        this.nombre = nombre;
        this.color  = "blanco";
    }
    public void setColor(string c) { this.color = c; }
}

public class Rectangulo : Figura {
    public Rectangulo(string nombre, decimal ancho, decimal alto) {
        super(nombre);          // → runs Figura's constructor
        this.ancho = ancho;
        this.alto  = alto;
    }
    public decimal area() { return this.ancho * this.alto; }
}

public class Cuadrado : Rectangulo {
    public Cuadrado(string nombre, decimal lado) {
        super(nombre, lado, lado);   // → runs Rectangulo's constructor
        this.lado = lado;
    }
}

let c = new Cuadrado("Tile", 4.0);
c.setColor("azul");
out c.area();     // → 16.0
out c.color;      // → azul
out c.nombre;     // → Tile
```

---

#### `public` and `private` methods

```serez
public class Contador {
    public Contador(int inicio) {
        this.valor = inicio;
    }

    private void incrementar() {
        this.valor = this.valor + 1;
    }

    public int siguiente() {
        this.incrementar();
        return this.valor;
    }
}

let c = new Contador(0);
out c.siguiente();   // → 1
out c.siguiente();   // → 2
```

`private` methods can only be called by other methods of the same class. Calling a private method from outside the instance is a runtime error.

> **Note:** The `public` keyword is required on class and constructor declarations. Omitting it is a parse error.

---

#### Static methods

`static` methods belong to the class itself, not to any instance. Call them with `ClassName.method(args)` — no instance needed.

```serez
class MathUtils {
    public static int square(int n) { return n * n; }
    public static int max(int a, int b) {
        if (a > b) { return a; }
        return b;
    }
}

out MathUtils.square(5);      // → 25
out MathUtils.max(7, 3);      // → 7
```

Static methods do not have access to `this` — they cannot read or write instance fields.

```serez
class Counter {
    public static int zero() { return 0; }
    public static string label() { return "Counter"; }
}

out Counter.zero();    // → 0
out Counter.label();   // → Counter
```

---

#### Abstract classes

An `abstract` class cannot be instantiated directly. It is designed to be subclassed. Attempting to call `new` on it is a runtime error.

```serez
abstract class Shape {
    public Shape(string name) {
        this.name = name;
    }
    public abstract decimal area();   // abstract method — no body required
    public string describe() {
        return "{this.name}: area={this.area()}";
    }
}

public class Circle : Shape {
    public Circle(decimal r) {
        super("Circle");
        this.r = r;
    }
    public decimal area() { return 3.14159 * this.r * this.r; }
}

let c = new Circle(5.0);
out c.describe();   // → Circle: area=78.53975
// new Shape("x");  // ❌ ERROR: Cannot instantiate abstract class 'Shape'
```

---

#### Sealed classes

A `sealed` class cannot be inherited from. Attempting to extend it is a runtime error.

```serez
sealed class Token {
    public Token(string kind, string value) {
        this.kind  = kind;
        this.value = value;
    }
}

// public class MyToken : Token { ... }   // ❌ ERROR: Cannot inherit from sealed class 'Token'
```

---

#### Getters and setters

`get` and `set` mark computed properties on a class. A getter is called with no arguments when the property is read; a setter is called with one argument when the property is written.

```serez
public class Temperature {
    public Temperature(decimal celsius) {
        this.celsius = celsius;
    }

    public get decimal fahrenheit() {
        return this.celsius * 9.0 / 5.0 + 32.0;
    }

    public set fahrenheit(decimal f) {
        this.celsius = (f - 32.0) * 5.0 / 9.0;
    }
}

let t = new Temperature(0.0);
out t.fahrenheit;         // → 32.0   (getter called, no parentheses)
t.fahrenheit = 212.0;     // setter called
out t.celsius;            // → 100.0
```

A property with only a getter and no setter is read-only — assigning to it is a runtime error.

---

#### Method references

Writing `obj.method` **without parentheses** yields the method as a value, bound to that object — it does not call it. Invoking it later still mutates the object it came from, so a method can be passed around as data: stored in an array or dictionary, handed to another function, or given to a UI component as a callback prop.

```serez
public class Counter {
    public Counter() { this.n = 0; }
    public void incr() { this.n = this.n + 1; }
    public void add(int k) { this.n = this.n + k; }
}

let c = new Counter();

let bump = c.incr;        // no parentheses → a reference, NOT a call
out c.n;                  // → 0    (nothing ran)
bump();
bump();
out c.n;                  // → 2    (mutates the original object)

let handlers = [c.incr, c.add];
handlers[1](10);
out c.n;                  // → 12

out type_of(c.incr);      // → "function"
```

Resolution for a parenthesis-less `obj.name` is **field → getter → method reference**: a field wins if one exists, then a `get name()` getter (which *does* run on read), and only then the method itself as a value.

A bound reference keeps its class context, so its body still reaches the class's own private members. Referencing a private method from outside is rejected exactly like calling it would be:

```serez
let f = obj.privateHelper;   // ❌ ERROR: Method 'privateHelper' is private and cannot be referenced externally
```

This is what makes the parent→child callback pattern work in `serez-ui`: the parent passes the method, the child invokes it.

```serez
<TaskRow onPick={this.pick} />                    // parent: a reference
<Button onClick={this.props.onPick}>Pick</Button> // child: invokes it
```

---

### Type Conversions

Two global functions convert between `string`, `int`, and `decimal`:

#### `parseInt(val)`

Converts a value to `int`:
- `string` → parses the string as a decimal integer. Runtime error if the string is not a valid integer.
- `decimal` → truncates toward zero (same as casting).
- `int` → returns the value unchanged.

```serez
out parseInt("42");     // → 42
out parseInt("  7 ");   // → 7    (whitespace trimmed)
out parseInt(3.99);     // → 3    (truncated)
out parseInt(10);       // → 10
```

#### `parseDecimal(val)`

Converts a value to `decimal`:
- `string` → parses the string as a floating-point number.
- `int` → promotes to `decimal`.
- `decimal` → returns the value unchanged.

```serez
out parseDecimal("3.14");   // → 3.14
out parseDecimal(5);        // → 5.0
out parseDecimal(2.71);     // → 2.71
```

---

#### `readLine(prompt?)`

Reads a line from stdin and returns it as a `string`. Strips the trailing newline.
- Called with no arguments: blocks and waits for input silently.
- Called with a `string` argument: prints the prompt first (no newline), then reads.

```serez
let name: string = readLine("What is your name? ");
out "Hello, {name}!";

let raw: string = readLine();
let n: int = parseInt(raw);
```

---

### Output

`out` prints any value to stdout followed by a newline. It accepts any expression:

```serez
out "hello";             // → hello
out 42;                  // → 42
out true;                // → true
out [1, 2, 3];           // → [1, 2, 3]
out "x = " + 10;        // → x = 10
out fibonacci(8);        // → 21
```

`out` is a statement, not a function — it cannot be nested inside an expression.

---

### Comments

Single-line comments with `//`. Everything from `//` to end of line is ignored.

```serez
// Full-line comment
let x = 5;   // Inline comment

// Commented-out code:
// out x * 2;
```

Multi-line block comments with `/* ... */`. Everything between the delimiters is ignored, including newlines.

```serez
/* This is a
   multi-line comment */

let y = /* inline block */ 42;
```

---

## Type System

### Overview

Serez-Code uses a **hybrid type system**: the language is dynamically typed by default, but you can add optional annotations that are enforced at runtime and partially checked statically before the program runs.

```
                 ┌──────────────────────────────────┐
                 │          Type Annotations        │
                 │                                  │
  fn int add(int a, int b) { ... }                  │
       ^^^        ^^^   ^^^                         │
       │          │     └─ parameter type           │
       │          └─ parameter type                 │
       └─ return type                               │
                 └──────────────────────────────────┘
                        ↓ checked at two points ↓
                  Static Checker          Runtime
                  (before run)          (on call)
```

### Type annotations

Annotations use the keywords `int`, `decimal`, `string`, `bool`, `void`, `any`, and typed array forms `[int]`, `[string]`, `[decimal]`. Append `?` to make a type nullable:

```serez
fn int strictAdd(int a, int b) {
    return a + b;
}

fn void log(string msg) {
    out msg;
}

fn bool check(int n) {
    return n > 0;
}

fn int? search(string name) {    // nullable return — may return null
    // ...
    return null;
}

fn [int] getIndices([int] arr, int threshold) {   // typed array param and return
    let result [int] = [];
    // ...
    return result;
}
```

They are fully optional. Parameters and return types without annotations accept any value:

```serez
fn multiply(a, b) {     // untyped: accepts any value for a and b
    return a * b;
}
```

### Static type checker

Before the program runs, the interpreter performs a static analysis pass over the AST. It infers types for top-level variables and checks call sites against declared signatures:

**Catches literal mismatches:**
```serez
fn int double(int n) {
    return n * 2;
}

double("hello");
// ❌ TYPE ERROR [line 5:7]: Parameter 'n' of 'double' expected 'int' but received 'string'.
```

**Catches variable mismatches** when the variable was declared with a literal or inferred from a call result:
```serez
let name = "Sergio";   // inferred as string
double(name);
// ❌ TYPE ERROR [line 2:8]: Parameter 'n' of 'double' expected 'int' but received 'string'.

fn int add(int a, int b) { return a + b; }
let x = add(1, 2);   // x inferred as int
double(x);            // ✅ int → int, no error
```

**Catches return type violations** when the returned expression type is known statically:
```serez
fn bool isPositive(int n) {
    return 42;   // ❌ TYPE ERROR: Function declares return 'bool' but 'return' expression has type 'int'.
}
```

**Catches arity errors:**
```serez
fn int add(int a, int b) { return a + b; }
add(1);
// ❌ TYPE ERROR: 'add' expects 2 arguments but got 1.
```

Expressions too complex to analyze statically (nested calls, array elements, etc.) are skipped — they fall through to the runtime checker. The static checker never halts execution; it only prints to `stderr`.

**Nullable awareness:** The static checker understands nullable types. A variable assigned `null` is inferred as type `"null"`. A nullable parameter (`int?`) accepts both `int` and `null` arguments without a static error.

### Runtime type enforcement

At every call site, typed parameters and return values are checked against the actual runtime values:

```serez
fn int double(int n) {
    return n * 2;
}

let x = 5;
double(x);           // ✅ x is int → passes
double(true);        // ❌ TYPE ERROR: Parameter 'n' expected 'int' but received another type.
```

Return type violations:

```serez
fn int alwaysNull() {
    // returns null implicitly — violates 'int' return annotation
}

alwaysNull();
// ❌ TYPE ERROR: Function expected to return 'int' but returned another type.
```

### Call stack in errors

When a type or runtime error occurs inside a nested call chain, the full call stack is printed:

```serez
fn int inner(int n) { return n * 2; }
fn void outer() { inner("bad"); }

outer();
// ❌ TYPE ERROR: Parameter 'n' expected 'int' but received another type.
//     called from 'outer' [line 2:22]
//     called from '<top>' [line 4:1]
```

---

## Runtime Safety

Serez-Code enforces several runtime invariants that would otherwise cause panics or silent corruption in a naive interpreter.

### Integer overflow

All arithmetic operations use Rust's `checked_*` variants. Overflow raises an error instead of wrapping:

```serez
let max = 9223372036854775807;   // i64::MAX
out max + 1;
// ❌ ERROR: Integer overflow
```

### Division and modulo by zero

```serez
out 10 / 0;   // ❌ ERROR: Division by zero
out 10 % 0;   // ❌ ERROR: Modulus operator by zero
```

### Array bounds

```serez
let a = [1, 2, 3];
out a[-1];    // ❌ ERROR: Index out of bounds
out a[3];     // ❌ ERROR: Index out of bounds
```

### Undeclared variables

```serez
out x;        // ❌ ERROR: Variable not found: x
y = 10;       // ❌ ERROR: Undeclared variable: y
```

### Non-function calls

```serez
let n = 42;
n();          // ❌ ERROR: Attempt to call a non-function
```

### Type mismatch in operators

```serez
out true + 1;        // ❌ ERROR: Type mismatch — operator '+' cannot be applied between 'bool' and 'int'
out "hello" - 1;     // ❌ ERROR: Type mismatch — ...
```

### `return` outside a function

```serez
return 5;   // ❌ FLASH SCOPE ERROR: 'return' cannot be used outside of a function
```

---

## Flash Scopes — Memory Model

Flash Scopes are the core of Serez-Code's runtime. They replace garbage collection with a deterministic, arena-based memory model that is predictable, fast, and requires zero `unsafe` Rust.

### Two memory regions

The runtime maintains two separate arenas:

```
┌──────────────────────────────────────────────────┐
│                  Global Arena                    │
│  [null | x=42 | greet=Fn | result=Array | ...]  │
│                                                  │
│  Top-level variables and function declarations   │
│  persist for the entire program lifetime.        │
│  Temporary allocations from 'out' and bare       │
│  expression statements are reclaimed immediately │
│  via a scratch watermark after each statement.   │
└──────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────┐
│                  Scoped Arena                    │
│  [...frame0... | ...frame1... | ...frame2... ]   │
│                ^mark0          ^mark1            │
│                                                  │
│  Local variables, function arguments, and        │
│  block-level temporaries. One shared arena       │
│  with a stack of watermarks — each scope exit    │
│  truncates back to its entry mark instantly.     │
└──────────────────────────────────────────────────┘
```

### ObjectRef — the safe pointer

No raw pointers are used anywhere. Every value reference is an `ObjectRef`:

```
ObjectRef { region: RegionId, index: usize }
                │                  │
                │                  └── slot index within the arena Vec
                └──── Global or Scoped — determines which arena to read
```

An `ObjectRef` cannot dangle: if the arena is reset, the index becomes unreachable, not invalid memory. The interpreter never hands out refs that cross the reset boundary.

### How scope entry and exit work

Every `{ ... }` block — function body, `if` branch, `while` body, or standalone block — follows this protocol:

```
1. Record watermark = arena.len()
2. Execute statements (new allocs append to arena)
3. Extract the return value as an arena-independent OwnedValue (deep clone)
4. arena.truncate(watermark) — all block-local data is freed
5. Re-allocate the extracted value in the parent scope (plant)
```

Step 3–5 is the **"promote before pop" invariant**. It ensures the returned value is never a dangling reference even when it is an array whose elements live inside the now-freed scope.

```serez
fn make_pair(int a, int b) {
    return [a, b];          // array lives in the function's scoped frame
}

let p = make_pair(10, 20); // extracted before pop, planted in global arena
out p[0];                  // → 10 — safe, lives in global arena now
out p[1];                  // → 20
```

### Why scope cleanup is O(k), not O(n)

`Vec::truncate(k)` runs the Rust `Drop` implementation for each removed element — that is `O(k)` where `k` is the number of objects in the scope that was exited. A garbage collector would traverse the entire live heap to identify unreachable objects — `O(n)` over the full heap.

For a function with 5 local variables, scope cleanup costs exactly 5 destructor calls, regardless of how large the rest of the program's memory is.

### Scratch watermark for top-level temporaries

At the top level, `out` statements create temporary values (e.g., the result of `fibonacci(10)` used only for printing). These are freed immediately after the statement via a scratch watermark on the global arena — they do not accumulate for the lifetime of the script.

```serez
out fibonacci(10);   // temporary result allocated, printed, freed
out fibonacci(20);   // same — no accumulation between statements
```

Bare expression statements (e.g., function calls used as statements) are **not** subject to the scratch reset, because they may have persistent side-effects — for example, a function that mutates a global array via index assignment allocates the new element value in the global arena as a side-effect. Resetting the watermark would destroy that allocation.

Global variable bindings from `let` are always kept; only display-only temporaries from `out` are released.

---

## Static Profiler (`--check` mode)

Run `sz --check script.sz` to analyze your program's memory footprint before executing it. The profiler walks the AST and estimates the byte cost of each function using heuristic rules:

| AST node | Estimated cost |
|---|---|
| `int` literal | 8 bytes |
| `decimal` literal | 8 bytes |
| `bool` literal | 1 byte |
| `string` literal | 24 + length bytes |
| Lambda expression | 32 bytes |
| Identifier lookup | 8 bytes |
| Prefix expression | 8 + operand bytes |
| Infix expression | 8 + left + right bytes |
| Function call | 8 + sum of arguments bytes |
| Array literal | 24 + sum of elements bytes |
| Dict literal | 24 + sum of (key + value) bytes per entry |
| Dot call (method) | 8 + sum of arguments bytes |
| `if` expression | condition + max(consequence, alternative) bytes |

Each function is classified by criticality:

```
🚀 Starting static analysis (Flash Scope Criticality)...
⚠️  NOTE: Cost in bytes is an estimated value based on AST heuristics.

Function 'fibonacci': ~312 estimated bytes
  Criticality: ██  🟢 < 1KB (Safe)

Function 'processData': ~11840 estimated bytes
  Criticality: ██████████  🔴 > 10KB (Critical)

📊 Estimated Global Memory: 12152 bytes
```

Criticality levels:

| Color | Range | Meaning |
|---|---|---|
| 🟢 Green | < 1 KB | Safe — well within typical stack budget |
| 🟡 Yellow | 1–10 KB | Warning — review loops and allocations |
| 🔴 Red | > 10 KB | Critical — likely a hot path; optimize |

> These are AST-level heuristic estimates, not exact runtime measurements. Use them to identify relative hotspots, not as absolute byte counts.

---

## Error Reference

All error messages go to `stderr`. Program output (`out` statements) goes to `stdout`. This lets you pipe them independently:

```bash
sz script.sz > output.txt 2> errors.txt
```

### Common errors

| Error message | Cause |
|---|---|
| `❌ ERROR: Variable not found: x` | Reading an undeclared variable |
| `❌ ERROR: Undeclared variable: x` | Assigning to a variable that was never `let`-declared |
| `❌ ERROR: Attempt to call a non-function` | Calling a value that is not a function |
| `❌ ERROR: Function expected N arguments, got M` | Arity mismatch at call site |
| `❌ ERROR: Index out of bounds` | Array access outside `[0, len-1]` |
| (dict: missing key → `null`) | Accessing a missing key in a dict returns `null`; use `??` for a default |
| `❌ ERROR: Unknown dict method 'x'` | Calling an undefined method on a dict |
| `❌ TYPE ERROR: Dict key/value type mismatch` | Adding an entry whose types violate the dict's annotation |
| `❌ ERROR: Division by zero` | `/` with zero on the right |
| `❌ ERROR: Modulus operator by zero` | `%` with zero on the right |
| `❌ ERROR: Integer overflow` | Arithmetic result exceeds `i64` range |
| `❌ TYPE ERROR: Parameter 'p' expected 'T'` | Runtime type mismatch on a typed parameter |
| `❌ TYPE ERROR: Function expected to return 'T'` | Return value type does not match declared return type |
| `❌ TYPE ERROR [line L:C]: ...` | Static checker caught a type error before execution |
| `❌ FLASH SCOPE ERROR: 'return' outside function` | `return` used at the top level |
| `❌ PARSER ERROR: Expected ...` | Syntax error — the parser describes the missing token |

### Understanding parser errors

The parser recovers from errors and continues parsing remaining statements. This means multiple errors can be reported in one run, each pointing to a different line:

```serez
let x = ;       // ← parse error here
let y = 10;     // this line still parses correctly
out y;          // and this executes
```

Parser errors always include the expected token or construct, making them actionable without needing a language specification.

---

## Architecture Overview

```
src/
├── main.rs           — CLI entry point: file execution, --check mode, REPL
├── token.rs          — Token enum and keyword-to-token lookup table
├── lexer.rs          — Hand-rolled character scanner; byte-indexed over the source String
├── ast.rs            — AST node definitions (Statement, Expression, BlockStatement, …)
├── parser.rs         — Pratt (TDOP) parser with 8-level precedence + error recovery
├── type_checker.rs   — Static pre-run type checker with literal and variable inference
├── region.rs         — Arena allocator (with_capacity), ObjectRef, ObjectData/OwnedValue with Rc<BlockStatement>
├── scope.rs          — ScopeStack — push/pop/lookup with watermark cleanup and all_bindings dedup
├── repl.rs           — Read-eval-print loop
├── compiler/         — Native backend pipeline (2.0.0 — work in progress)
│   ├── types.rs          — Compile-time type system (SzType) mapping Serez types to LLVM types
│   ├── hir.rs            — High-level IR: desugared AST nodes (HirStmt, HirExpr, HirBinOp)
│   ├── hir_lower.rs      — AST → HIR lowering pass (resolves syntax sugar)
│   ├── mir.rs            — Mid-level IR: three-address code with basic blocks and terminators
│   ├── mir_lower.rs      — HIR → MIR flattening (SSA-like temporaries, explicit control flow)
│   ├── llvm_emit.rs      — MIR → LLVM IR text emission
│   └── mod.rs            — Module glue
└── evaluator/        — Tree-walking interpreter (split into focused submodules)
    ├── mod.rs            — Core entry points, Flash Scope protocol, StoredClass dispatch, static profiler
    ├── stmt.rs           — Statement evaluation (let, assign, for, while, return, …)
    ├── expr.rs           — Expression evaluation (calls, index, dot, ternary, …)
    ├── ops.rs            — Infix and prefix operator evaluation
    ├── check.rs          — Type-check helpers (parameter, return, typed array)
    ├── builtins.rs       — Global built-in functions (parseInt, parseDecimal, readLine, …)
    ├── classes.rs        — Class instantiation, method dispatch, inheritance, super
    ├── methods_array.rs  — Array method dispatch (push, pop, map, filter, reduce, sort, …)
    ├── methods_string.rs — String method dispatch (split, replace, trim, padStart, …)
    ├── methods_set.rs    — Set method dispatch (add, has, delete, toArray, union, …)
    ├── namespaces.rs     — Built-in namespace dispatch (Math, File, JSON)
    ├── namespaces_os.rs  — OS/hardware namespaces (Terminal, OS, Env, Time, System)
    └── control.rs        — Control flow helpers (break, continue, labeled loops, do-while)
```

### Data flow

```
Source file (.sz) or REPL line
        │
        ▼
    Lexer
    — Byte-indexed scan over the source String (no intermediate Vec<char> copy)
    — 1-character lookahead for two-char tokens (==, !=, <=, >=, =>)
    — Emits a stream of Token { type, literal, line, column }
        │
        ▼
    Parser (Pratt TDOP)
    — parse_program() → Program { Vec<Statement> }
    — Prefix handlers: literals, identifiers, if, fn, arrays, entry literals {k,v}, ( )
    — Infix handlers: +, -, *, /, %, ==, !=, <, >, <=, >=, &&, ||, f(args), a[i], obj.method(args)
    — Error recovery: synchronize() skips to ; or } or keyword on failure
        │
        ▼
    TypeChecker (static pass)
    — Collects all FunctionDeclarations into a name → signature map
    — Infers types for let-bound variables with literal RHS
    — Checks call sites against declared parameter and return types
    — Reports errors to stderr; does not halt execution
        │
        ▼
    Evaluator (tree-walking)
    — eval_program() iterates top-level statements
    — eval_statement() dispatches Let, Assign, While, For, Out, Block, …
    — eval_expression() dispatches Infix, Prefix, Call, If, Index, …
    — Flash Scope protocol on every { } block: push → eval → extract → pop → plant
    — Scratch watermark reclaims top-level Out temporaries (Expression excluded — may have persistent side-effects)
        │
        ├──► stdout  (out statements, REPL results)
        └──► stderr  (type errors, runtime errors, parser errors)
```

### Lexer — byte-indexed scanning

The lexer operates directly on the source `String` using byte offsets (`position`, `read_position`). It does not copy the input into a `Vec<char>`. Multi-byte UTF-8 characters in identifiers are handled correctly because `read_char` advances by `c.len_utf8()` bytes, and string slicing uses `&str[start..end]` which is byte-range indexed.

### Parser — Pratt TDOP

The parser implements Top-Down Operator Precedence (Pratt parsing). Every infix operator must be registered in **two places**:

1. `token_precedence()` — returns the operator's binding power (precedence level)
2. `is_infix` match in `parse_expression()` — gates entry into the infix loop

Registering in only one place produces subtly wrong behavior: the parser either ignores the operator or silently discards the expression around it.

### Evaluator — Flash Scope protocol

The core memory invariant enforced by the evaluator:

```rust
// Every block follows this sequence in ALL code paths, including errors:
scopes.push();
// ... evaluate block statements ...
let owned = extract(result_ref);   // deep clone before pop
scopes.pop();                      // free all block-local memory
let promoted = plant(owned);       // re-allocate in parent scope
```

`extract` materializes the full object tree (including nested arrays) into an arena-independent `OwnedValue`. `plant` re-allocates it wherever `alloc()` currently points — the parent scope or global arena.

### Performance internals

Several optimizations reduce redundant allocations and clones during hot paths.

#### `Rc<BlockStatement>` — O(1) function cloning

Every function value stores its AST body as `Rc<BlockStatement>` rather than an owned `BlockStatement`. Looking up a function from the arena, passing it as a callback, or returning it from `find_method` increments a reference count instead of deep-cloning the body. This applies to both `OwnedValue::Function` and `ObjectData::Function` in `region.rs`.

#### `StoredClass` — O(1) method dispatch

Class methods are stored in `StoredClass` using four separate `HashMap`s: `methods`, `static_methods`, `getters`, and `setters`. Each lookup is O(1) by name. Method values (`StoredMethod`) hold a `body: Rc<BlockStatement>`, so each clone is O(1) regardless of how large the method body is. Previously, every method call cloned the entire `ast::ClassMethod` including its body.

#### Arena and HashMap pre-sizing

Pre-sized collections avoid repeated growth reallocations during typical program execution:

| Allocation | Initial capacity |
|---|---|
| Global arena | 256 objects |
| Scoped arena (`Arena::new()`) | 64 objects |
| Scope frame bindings | 4 entries |
| `global_bindings` | 32 entries |
| Interface / class registries | 8 entries each |

#### `all_bindings()` deduplication

`ScopeStack::all_bindings()` traverses frames inner-to-outer and skips names already seen. When a closure captures its environment, shadowed outer variables are not extracted and re-allocated — each name appears at most once in the captured environment.

#### Structural helpers

Three helpers in `evaluator.rs` centralize patterns that previously appeared 6–11 times each:

| Helper | Replaces |
|---|---|
| `leave_call()` | `scopes.pop(); call_depth -= 1; call_stack.pop()` — 11 call-exit sites |
| `print_call_stack()` | 3-line call-chain printer loop — 6 error sites |
| `plant_for_target(value, ref)` | Region-aware arena selection for dict `IndexAssign` — 3 sites |

---

## Demo Apps

The `apps/` directory contains five console programs that together exercise every language feature. Run any of them with `sz apps/<name>.sz`.

| File | What it exercises |
|---|---|
| `apps/01_task_manager.sz` | `enum`, class inheritance (`UrgentTask : Task`), static methods with `switch`, HOF (filter/map/reduce), `try/catch/throw` |
| `apps/02_statistics.sz` | Typed `[decimal]` arrays, `Math` namespace, map/filter/reduce for mean/stddev/median/percentile, histogram, Pearson correlation |
| `apps/03_text_analyzer.sz` | String methods (split, replace, trim, indexOf, charAt, padEnd, substring), dicts for word frequency, Caesar cipher, `File` I/O |
| `apps/04_bank_system.sz` | `abstract` class, `sealed` class, `interface`, `const`, getters (`get`), `try/catch/throw`, optional chaining `?.`, null coalescing `??` |
| `apps/05_data_pipeline.sz` | `JSON` (stringify/parse), `File` (write/read), `Set` (deduplication), bitwise ops (`&`, `\|`, `^`), power ops (`**`, `>>`), HOF pipeline |

---

## Known Gotchas

These behaviors were discovered writing the demo apps. None are bugs — they are correct semantics — but they can surprise first-time users.

### `for-in` loop variable is a copy

`for (let x in arr)` binds a **value copy** of each element. Mutating `x` does not affect the original array.

```serez
let items = [1, 2, 3];
for (let x in items) {
    x = x * 10;   // ⚠️ mutates the copy only — items is unchanged
}
out items;   // → [1, 2, 3]
```

To mutate elements, use an index loop: `for (let i = 0; i < items.length; i++) { items[i] = ...; }`.

### `this.field[i].method()` inside a class method does not persist

Accessing `this.field` inside a method returns a copy of the stored value. Calling a mutating method on that copy does not write back to the instance.

```serez
// ⚠️ Does NOT work — arr is a copy of this.items
fn void broken() {
    this.items[0] = 99;   // index-assign on this.items DOES work
    // but: this.items.push(4) — push on this.items DOES work
    // ⚠️: this.items[0].someMethod() — calls method on a copy, not persisted
}
```

Index-assign (`this.items[i] = value`) and direct method calls (`this.items.push(v)`) on `this.field` do persist. The limitation only applies to chained method calls on elements retrieved from `this.field`.

### `{` inside a string literal triggers interpolation

Any `{` starts an interpolation expression. Use `\{`/`\}` for literal braces, or a
**raw string** `r"…"` to disable interpolation entirely:

```serez
out "Score: {score}";      // ✅ interpolation
out "Empty dict: \{\}";    // ✅ literal braces → Empty dict: {}
out r"Empty dict: {}";     // ✅ raw string → Empty dict: {}
out "Block: {";            // ❌ parse error — unclosed interpolation
```

### `\"` inside `{…}` interpolation breaks the parser

Escape sequences inside `{…}` expressions are not supported. Extract the value to a variable instead:

```serez
// ⚠️ This breaks the parser:
out "Names: {arr.join(\", \")}";

// ✅ Use a variable:
let sep = ", ";
out "Names: {arr.join(sep)}";
```

### Enum parameters must not be annotated as `string`

Enum variants have their own type. Annotating a parameter as `string` when passing an enum value causes a type error:

```serez
enum Priority { Low, High }

fn add(string p) { ... }   // ⚠️ type error when called with Priority.High
fn add(p) { ... }          // ✅ untyped parameter accepts enum values
```

### `public abstract TYPE method()` is not valid syntax

Abstract method *declarations* (no body) are not supported. Provide a default throwing body instead:

```serez
// ⚠️ Not supported:
public abstract decimal area();

// ✅ Use a default implementation that throws:
public decimal area() {
    throw "area() not implemented in " + this.name;
    return 0.0;
}
```

---

## Contributing

All contributions are welcome — bug fixes, new language features, documentation, or test cases.

### Build and test

```powershell
cargo build
cargo test                         # Rust unit tests (lexer, etc.)

# Windows (PowerShell):
.\run_tests.ps1                    # full suite (E2E + unit + error + security)
.\run_tests.ps1 -unit              # unit tests only (framework-based)
.\run_tests.ps1 -e2e               # E2E golden-file tests + error tests
.\run_tests.ps1 -security          # security/error tests only
.\run_tests.ps1 -filter "switch"   # run tests matching a name pattern
.\run_tests.ps1 -generate          # regenerate .expected files after language changes
```

```bash
# Linux / macOS (Bash):
./run_tests.sh                     # full suite
./run_tests.sh --unit              # unit tests only
./run_tests.sh --e2e               # E2E golden-file tests + error tests
./run_tests.sh --security          # security/error tests only
./run_tests.sh --filter "switch"   # run tests matching a name pattern
./run_tests.sh --generate          # regenerate .expected files after language changes
```

### Project conventions

- **No `unsafe` in the interpreter core** — the arena memory model is intentionally built without Rust unsafe blocks. New language features must maintain this invariant. (`namespaces_os.rs` uses `unsafe` only for platform FFI calls such as `GlobalMemoryStatusEx`.)
- **Minimal external runtime dependencies** — adding a new crate requires a strong reason. Current runtime deps: `notify`, `ureq`, `zip`, `crossterm`.
- **Errors go to `stderr`** — use `eprintln!` for all error output; `println!` only for program output (`out` statements) and the REPL.
- **Flash Scope invariant** — any new block-level construct must call `scopes.push()` before evaluating its body and `scopes.pop()` after, in **all** code paths including error paths. Forgetting a pop on an error path leaks the call stack in the REPL.
- **All new syntax flows through the full pipeline** — `token.rs` → `lexer.rs` → `ast.rs` → `parser.rs` → `evaluator.rs`. Never add to the evaluator without a corresponding AST node.

### Adding a new infix operator

Infix operators require registration in **two** places in `parser.rs`, or the parser will silently misbehave:

```rust
// 1. token_precedence() — gives the operator its binding power
TokenType::MyOp => Precedence::Sum,

// 2. is_infix match — allows parse_expression to enter the infix loop
TokenType::MyOp => true,
```

Then add evaluation in `eval_infix()` in `evaluator.rs`.

### Adding a new statement

1. Add a `TokenType` variant in `token.rs`. If keyword-based, wire it in `lookup_ident()`.
2. Add the AST node(s) in `ast.rs`.
3. Add a parse handler in `parser.rs` inside `parse_statement()`.
4. Add an eval handler in `evaluator.rs` inside `eval_statement()`.
5. Add a test `.sz` file demonstrating the feature.

### Open a PR

- One logical change per commit.
- Describe **why** a change was made, not just what.
- PRs that add language features must include at least one `.sz` example file.

---

## Roadmap

### Language features
- [x] `&&` and `||` — logical AND and OR operators with short-circuit evaluation
- [x] `for` loop — `for (let i = 0; i < n; i++)`, nested loops, 1D/2D array traversal; update accepts `i++`, `i--`, `i += n`
- [x] Array mutation via index — `arr[i] = expr`, works in loops and from inside functions
- [x] String interpolation — `"Hello, {name}!"`, supports nested quotes inside `{…}` (e.g. `{dict["key"]}`)
- [x] Lexical closures — functions that capture variables from their defining scope
- [x] Native higher-order functions — `map`, `filter`, `reduce` with lambda syntax `x => expr` / `(x, i) => expr`
- [x] Array methods — `.push`, `.pop`, `.shift`, `.unshift`, `.remove`, `.reverse`, `.sort`, `.find`, `.findIndex`, `.indexOf`, `.includes`, `.every`, `.some`, `.slice`, `.flat`, `.join`
- [x] String methods — `.length`, `.substring`, `.slice`, `.split`, `.replace`, `.includes`, `.indexOf`, `.startsWith`, `.endsWith`, `.charAt`, `.trim`, `.trimStart` / `.trimLeft`, `.trimEnd` / `.trimRight`, `.toUpperCase`, `.toLowerCase`, `.padStart`, `.padEnd`, `.toString()`
- [x] Dict methods — `.toList()` (keys array), `.toArray()` (2D entries array); missing key returns `null`
- [x] `decimal` type — f64 literals (`3.14`), mixed arithmetic with `int`
- [x] Global conversions — `parseInt(val)`, `parseDecimal(val)`
- [x] Console input — `readLine(prompt?)`
- [x] Interfaces — typed record schemas: `interface Point { x: decimal, y: decimal }`, `new Point({ x:1.0, y:2.0 })`, field read/write, object patch `p = { x: 5.0 }`
- [x] Classes — C#-style OOP: `public class Foo`, constructor `public Foo(args)`, `this.field`, `public`/`private` methods, field assignment `obj.field = val`
- [x] Single inheritance — `public class Bar : Foo`, `super(args)` constructor delegation, `super.method()`, method override, inherited method lookup
- [x] Static methods — `public static T method(...)` on classes, called as `ClassName.method(args)`
- [x] Abstract classes — `abstract class Foo` cannot be instantiated; abstract methods have no body
- [x] Sealed classes — `sealed class Foo` cannot be subclassed
- [x] Getters / setters — `public get T prop()` / `public set prop(T val)` computed properties on class instances
- [x] `break` / `continue` — loop control flow inside `while`, `for`, `for-in`, and `do-while`
- [x] Labeled `break` / `continue` — `label: for ...` with `break label` / `continue label` for nested loop control
- [x] `do-while` loop — body executes at least once; `break`/`continue` supported
- [x] `switch` — `switch(expr) { case val: {} case a, b: {} default: {} }` — no fall-through
- [x] Exceptions — `try {} catch (e) {} finally {}` and `throw expr`; any value can be thrown
- [x] `const` — immutable variable declarations enforced at runtime
- [x] `enum` — `enum Color { Red, Green, Blue }` with `Color.Red` variant access
- [x] `Set` type — `new Set([...])`, methods: `add`, `has`, `delete`, `clear`, `size`, `toArray`, `union`, `intersection`
- [x] Null coalescing — `a ?? b` returns `a` if non-null, else evaluates `b`
- [x] Optional chaining — `a?.method()` / `a?.field` returns `null` without error when `a` is `null`; chains with `??`
- [x] Ternary operator — `cond ? then : else` with lazy evaluation and right-associativity
- [x] Escape sequences — `\n`, `\t`, `\r`, `\\`, `\"`, `\{` inside string literals
- [x] Block comments — `/* ... */` multi-line comments
- [x] Math namespace — `abs`, `sqrt`, `floor`, `ceil`, `round`, `trunc`, `min`, `max`, `pow`, `exp`, `log`, `log2`, `log10`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `clamp`, `sign`, `random`, `PI`, `E`
- [x] File namespace — `read`, `write`, `create`, `exists`, `read_asBinary`, `write_asBinary`
- [x] JSON namespace — `stringify`, `parse`, `pretty`
- [x] Power operator — `**` for integer and decimal exponentiation
- [x] Bitwise operators — `&`, `|`, `^`, `~`, `<<`, `>>` (64-bit signed integers); binary (`0b`) and hex (`0x`) literals; numeric separators (`1_000_000`)
- [x] `is` type-check operator — `expr is TypeName` returns `bool` at runtime
- [x] Default parameters — `fn int f(int x = 10)` with fallback when argument is omitted
- [x] Security test suite — 17 error tests (`sec_*.sz`) + 6 unit test files (`unit_sec_*.sz`) covering arithmetic, null safety, type safety, error isolation, injection, and resource limits
- [x] OS/hardware namespaces — `Terminal` (raw mode, keyboard, mouse, cursor), `OS` (platform, pid, exec, kill), `Env` (get, set, args), `Time` (now, sleep), `System` (cpuCount, totalMemory, freeMemory, hostname, uptime)
- [x] Socket namespace — TCP client/server (`connect`, `send`, `recv`, `listen`, `accept`, `close`) + RFC 6455 WebSocket text frames (`sendWsFrame`, `recvWsFrame`)
- [x] GPU namespace — CPU-backed compute buffers (`createBuffer`, `createBufferFromArray`, `map`, `reduce`, `dot`, `axpy`, `matmul`, `fill`, `readBuffer`, `freeBuffer`)
- [x] File extended — `listDir`, `mkdir`, `stat`, `delete`, `rename`
- [x] Permission system — three-level model: `serez.json` (project-wide) → `use permissions {}` (file-level) → `unsafe {}` (operation-level)
- [x] `use permissions {}` keyword — grants namespace access at file scope

### Type system
- [x] Typed arrays — `[int]`, `[string]`, `[decimal]`, `[T?]` with element-level enforcement on `push`, `unshift`, index-assign, and construction
- [x] Type inference for function call results — `let x = add(1, 2)` infers `x: int` in the static checker
- [x] Optional / nullable types — `int?`, `string?`, `fn int? search()`, `null` literal, null equality (`== null`, `!= null`)

### Tooling
- [x] Security test runner — `-security` / `--security` flag on `run_tests.ps1` / `run_tests.sh` runs all security test files
- [x] Cross-platform test runner — `run_tests.sh` (Bash) mirrors all flags of `run_tests.ps1` (PowerShell)
- [x] Span-aware error diagnostics — parser and runtime errors show the source line with a `^` caret
- [x] Watch mode — `sz --watch file.sz` re-runs on every save
- [x] VS Code extension — syntax highlighting and formatter for `.sz` files (`vscode-serez/`)
- [x] Demo apps — five `apps/*.sz` programs that exercise every language feature end-to-end
- [x] `.sz` file formatter — `DocumentFormattingEditProvider` integrado en la extensión VS Code; `formatOnSave` activado automáticamente para `.sz`
- [x] LSP server for editor support — `sz-lsp` binary (stdio JSON-RPC): live diagnostics (parser + type checker), completion (keywords, native namespaces + their methods, document symbols), hover, go-to-definition and document symbols; wired into the VS Code extension (`serez.lsp.enabled` / `serez.lsp.path`)

---

## License

See [LICENSE](LICENSE) for details.

---

<div align="center">

Built with ❤️ and Rust — no GC required.

</div>
