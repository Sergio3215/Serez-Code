<div align="center">

# ![](./img/sz-icon.svg) Serez-Code

**A hand-crafted interpreted programming language.**

No garbage collector. No runtime to install. Instant memory cleanup via **Flash Scopes**.

[![Release](https://img.shields.io/badge/release-v9.11.0-blue?style=flat-square)](https://github.com/Sergio3215/serez-code/releases/latest)
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
   - [Random](#random)
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
   - [Modules (`import` / `export`)](#modules-import--export)
   - [Package Manager](#package-manager)
   - [Classes & Interfaces](#classes--interfaces)
   - [Type Conversions](#type-conversions)
   - [Output](#output)
   - [Comments](#comments)
4. [Type System](#type-system)
5. [Runtime Safety](#runtime-safety)
6. [Flash Scopes](#flash-scopes)
7. [Static Profiler](#static-profiler-check-mode)
8. [Error Reference](#error-reference)
9. [Known Gotchas](#known-gotchas)
10. [Contributing](#contributing)
11. [License](#license)

---

## Why Serez-Code?

Most interpreted languages manage object lifetimes with a garbage collector or with reference counting. Serez-Code takes a fundamentally different approach: **region-based arena allocation** with watermark-based cleanup.

| Trait | Traditional interpreters | Serez-Code |
|---|---|---|
| Memory management | GC pauses / reference counting | Bump allocator + watermark truncation |
| Scope cleanup | Non-deterministic (GC) or proportional to the whole live heap | Deterministic — bounded by the exiting scope's own data |
| When memory is freed | Whenever the collector decides | At the closing `}`, always |
| Type safety | Fully dynamic or fully static | Optional annotations, enforced at every call site |
| Integer safety | Silent overflow or panic | Checked arithmetic — overflow is a runtime error |
| Startup | Runtime or VM to install first | A single self-contained binary |

On top of that model sits a feature you drive yourself: an inner `{ ... }` block inside a function — a **Flash Scope** — bounds the lifetime of everything declared in it. Build a large structure in the braces, keep only the part you need in a variable declared before them, and the rest is released at the closing brace. See [Flash Scopes](#flash-scopes).

---

## Getting Started

### Install

Every published binary is self-contained — there is nothing else to install and
no separate runtime to set up. `sz --version` is the authority for the installed
version; download links below always resolve the latest published release.

**Linux / macOS** — installs `sz` into `~/.local/bin`:
```sh
curl -fsSL https://raw.githubusercontent.com/Sergio3215/serez-code/main/install.sh | sh
```

**Windows (PowerShell)** — installs `sz.exe` into `%LOCALAPPDATA%\SerezCode\bin` and adds it to your user `PATH`:
```powershell
irm https://raw.githubusercontent.com/Sergio3215/serez-code/main/install.ps1 | iex
```

Both scripts resolve the latest published release automatically. Open a new terminal and confirm:

```bash
sz --version
```

#### Direct downloads

Prefer to grab the file yourself? These links always point at the newest release:

| Platform | Asset | Download |
|---|---|---|
| Windows x64 | `sz-windows-x64-setup.msi` | [Installer](https://github.com/Sergio3215/serez-code/releases/latest/download/sz-windows-x64-setup.msi) — adds `sz` to `PATH` for you |
| Windows x64 | `sz-windows-x64.exe` | [Standalone .exe](https://github.com/Sergio3215/serez-code/releases/latest/download/sz-windows-x64.exe) — no installer, put it anywhere |
| macOS — Apple Silicon | `sz-macos-arm64` | [Download](https://github.com/Sergio3215/serez-code/releases/latest/download/sz-macos-arm64) |
| macOS — Intel | `sz-macos-x64` | [Download](https://github.com/Sergio3215/serez-code/releases/latest/download/sz-macos-x64) |
| Linux x64 | `sz-linux-x64` | [Download](https://github.com/Sergio3215/serez-code/releases/latest/download/sz-linux-x64) — static build, runs on any distro |

On macOS and Linux the downloaded file needs the executable bit before first use:

```sh
chmod +x sz-macos-arm64
sudo mv sz-macos-arm64 /usr/local/bin/sz
```

Verify what you downloaded against [`checksums.txt`](https://github.com/Sergio3215/serez-code/releases/latest/download/checksums.txt). Older versions and full release notes live on the [Releases page](https://github.com/Sergio3215/serez-code/releases).

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

### Run a snippet without a file

```bash
sz --eval 'out 2 + 3;'
echo 'let x = 10; out x * x;' | sz --eval -
```

`--eval` (short: `-e`) runs source handed in as a string. Use `-` to read it from
stdin, which saves you from escaping quotes and newlines through the shell.

Because there is no file, there is no `serez.json` to read permissions from, and
snippets run under **lockdown**: `use permissions { … }` is refused instead of
granting itself the permissions it asks for, and `File`, `import` and Autodiff's
weight files — which are otherwise reachable with nothing declared — are denied.
All of them as catchable `PermissionError`s. Running `sz script.sz` is unaffected;
declaring permissions inline in your own file still works.

Lockdown is not a sandbox against a hostile program: `fetch` still reaches the
network from wherever `sz` is running. Don't feed `--eval` source you don't trust
without real isolation around the process.

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

### Editor support (LSP)

`sz-lsp` is a Language Server Protocol implementation for `.sz` files, so any LSP-capable editor can give you live errors and completion:

Capabilities:

| Feature | Detail |
|---|---|
| Live diagnostics | Parser errors (as errors) + static type checker findings (as warnings), on every keystroke |
| Completion | Keywords, the 21 native namespaces with their real methods (extracted from the evaluator), builtin functions, and the document's own functions/classes/variables; `File.` → `read`, `write`, … |
| Hover | Signatures of user functions/classes (`fn int suma(int a, int b)`), namespace summaries, builtin signatures |
| Go to definition | Functions, classes, enums, variables in the file; `import "…" ` lines jump to the imported file |
| Document symbols | Outline with classes and their nested methods/fields |

The VS Code extension (≥ 1.7.0) starts it automatically for `.sz` files: it looks for `sz-lsp` on your `PATH` (point it elsewhere with the `serez.lsp.path` setting, or turn it off with `serez.lsp.enabled: false`). Any other LSP-capable editor — Neovim, Zed, JetBrains — can launch `sz-lsp` as a stdio JSON-RPC server.

> **Heads-up:** the release assets currently ship `sz` only, so `sz-lsp` still has to be built from source — see [DEVELOPMENT.md](DEVELOPMENT.md). Editing works fine without it; you just lose the live diagnostics.

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

#### Destructuring declarations

Array and object patterns are available on `let` and `const` declarations:

```serez
let [head, _, ...tail] = [1, 2, 3, 4];
let {name, age: years} = ({"name", "Ana"}, {"age", 30});
```

Array holes create no binding, missing positions become `null`, and the final
`...rest` receives a new array. Object patterns accept dicts, class instances
and `DateTime`; missing properties become `null`. A wrong source type is a
catchable `TypeError` (`SZ4002`), and a `throw` while evaluating the source is
preserved. The normative contract is in [`spec/variables.md`](spec/variables.md).

---

### Types

Serez-Code has five primitive types and three compound types:

| Type | Literal / annotation examples | Runtime representation |
|---|---|---|
| `int` | `0`, `42`, `-7` | 64-bit signed integer (`i64`) |
| `decimal` | `3.14`, `0.5`, `2.0` | 64-bit floating-point (`f64`) |
| `dec` | `12.50m`, `5m`, `1e-7m` | **Exact** base-10 decimal, 28–29 significant digits |
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

`&&` and `||` **return one of their operands**, not a boolean, and
**short-circuit**: `&&` stops at the first falsy value, `||` at the first truthy
one.

```serez
a && b   //  a if a is falsy, otherwise b
a || b   //  a if a is truthy, otherwise b
```

```serez
out true && true;     // → true
out true && false;    // → false
out false && true;    // → false  (right side not evaluated)
out false || true;    // → true
out true || false;    // → true   (right side not evaluated)

// Combine with comparison operators:
out (1 < 2) && (3 > 0);    // → true
out (1 > 2) || (3 == 3);   // → true

// Returning an operand is what makes these useful beyond conditions:
let name = given || "anonymous";     // default value
out items && render(items);          // only build the row if there is something
```

##### One rule of truthiness

These are the falsy values — everything else is truthy:

`false` · `null` · `0` · `0.0` · `""` · an **empty** array, dict or set

The same rule drives `&&`, `||`, the `!` prefix, the ternary, `match` guards and
the `filter`/`some`/`every` callbacks.

**Empty collections being falsy is a deliberate departure from JavaScript**,
where `[]` is truthy and `items && render(items)` fires on an empty list — the
classic bug whose usual workaround (`items.length && …`) prints a stray `0`.
Here the simple form already means "if there is anything".

`!` negates that same rule and always yields a boolean:

```serez
out !true;      // → false
out !0;         // → true
out !"";        // → true
out ![];        // → true   (empty collection)
out ![1, 2];    // → false
out !!"text";   // → true   (double negation normalises to a boolean)
```

A class can override `!` for its instances by defining `op_not`; that overload
takes precedence over the general rule.

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

#### Pipe operator (`|>`)

`expr |> f` feeds the left-hand value into `f` as its single argument — it is exactly `f(expr)`, resolved at parse time. It turns nested calls into a left-to-right reading order:

```serez
fn int double(int n) { return n * 2; }
fn int plus1(int n)  { return n + 1; }

out 5 |> double;             // → 10   (same as double(5))
out 5 |> double |> plus1;    // → 11   (same as plus1(double(5)))
```

The right-hand side is any expression that evaluates to a function, so a lambda held in a variable works too:

```serez
let inc = int (int n) => { return n + 1; };
out 5 |> inc;    // → 6
```

`|>` has the **lowest precedence of every operator** — everything else binds tighter. That makes the left-hand side work as expected, but means the right-hand side swallows any operator that follows it:

```serez
out 2 + 3 |> double;     // → 10  — left side groups first: double(2 + 3)
out 5 |> double + 1;     // ❌ ERROR: '+' between 'function' and 'int'
out (5 |> double) + 1;   // → 11  — parenthesize when mixing
```

#### `sizeof`

`sizeof(T)` returns the size in bytes of a **type's** in-memory slot, as a static `int`:

```serez
out sizeof(int);       // → 8
out sizeof(decimal);   // → 8
out sizeof(dec);       // → 8
out sizeof(bool);      // → 1
out sizeof(string);    // → 8
out sizeof(any);       // → 8
out sizeof(null);      // → 0
out sizeof(void);      // → 0
```

`sizeof(string)` is 8 because it measures the pointer-sized handle, **not** the length of the text. For the number of characters use `.length`.

> **Types only.** `sizeof` accepts a type keyword and nothing else. Passing a value or a variable — `sizeof(5)`, `sizeof(x)`, `sizeof("hi")` — fails with `❌ PARSE ERROR: expected ')' to close sizeof`.

#### Operator precedence

From lowest to highest:

| Level | Operators |
|---|---|
| `Lowest` | — |
| `Pipe` | `\|>` |
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

Parameters can have default values. If the caller omits the argument, the default is used. Default parameters must come after required ones; a required parameter after a default is a parser error (`SZ2000`). A final `...rest` parameter may still follow defaults.

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

Default values are arbitrary expressions evaluated at call time. They are
evaluated in declaration order, so a default can read an earlier parameter; an
explicit argument skips its default. A user `throw` or runtime error raised by a
default propagates unchanged rather than becoming `null`:

```serez
fn int compute(int n = 2 + 3) {
    return n * 2;
}

out compute();    // → 10  (default: 5 * 2)
out compute(7);   // → 14
```

The normative parameter and call-time contract is in
[`spec/functions.md`](spec/functions.md).

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

#### Generators (`fn*` / `yield`)

A function declared with `fn*` is a **generator**: instead of returning a single value, every `yield` inside it appends to a result the call produces at the end.

```serez
fn* gen() {
    yield 1;
    yield 2;
    yield 3;
}

out gen();            // → [1, 2, 3]
out type_of(gen());   // → array
```

`yield` works anywhere in the body, including inside loops:

```serez
fn* evens(int n) {
    for (let i = 0; i < n; i = i + 1) {
        yield i * 2;
    }
}

out evens(4);   // → [0, 2, 4, 6]
```

> **Generators are eager, not lazy.** The call runs the whole body to completion and hands back a plain `array` of everything yielded — there is no iterator, no `.next()`, and no pausing. `fn*` is shorthand for "build a list by yielding into it", not a coroutine. An infinite generator never returns.

`yield` is only valid inside a `fn*`. Using it in a normal function is an error:

```serez
fn any wrong() { yield 1; }
// ❌ ERROR: 'yield' used outside of a generator function (fn*)
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

Iterates over every element of an array, every Unicode scalar value of a string,
or every key of a dict. The loop variable is declared with `let` and is scoped
to the loop body. Values/keys are snapshotted before the first iteration and
bound as copies.

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

Dict iteration yields keys in insertion order. An array pattern can destructure
array rows directly:

```serez
for (let [key, value] in [["a", 1], ["b", 2]]) {
    out key + "=" + value;
}
```

A non-array/string/dict iterable, or a non-array row used with that pattern,
raises catchable `TypeError` (`SZ4002`). See the normative rules in
[`spec/control-flow.md`](spec/control-flow.md).

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

#### `match` expression

Where `switch` is a statement that runs blocks, `match` is an **expression**: it evaluates to a value, so it can be returned or assigned directly. Each arm is `pattern => body`, and arms are separated by commas.

```serez
let x = 2;
let r = match (x) {
    1 => "one",
    2 => "two",
    _ => "other"
};
out r;   // → two
```

`_` is the wildcard arm. Patterns can be integer, decimal, `dec`, string, `true`/`false` and `null` literals, or an `Enum.Variant`.

**Alternatives** are joined with `|`, and an arm can carry a **guard** with `if`. A bare identifier binds the subject to that name, which is what makes guards useful:

```serez
fn string classify(int n) {
    return match (n) {
        0            => "zero",
        1 | 2 | 3    => "small",
        x if x > 100 => "huge",
        _            => "normal"
    };
}

out classify(0);     // → zero
out classify(2);     // → small
out classify(500);   // → huge
out classify(50);    // → normal
```

An arm body can also be a block, in which case the block's last expression is its value:

```serez
let cmd = "add";
let r = match (cmd) {
    "add" => { let a = 2; a + 3 },
    "del" => 0,
    _     => -1
};
out r;   // → 5
```

Arms are tried top to bottom and the first match wins, so put `_` last.

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
| `ReferenceError` | Missing variables, classes or migrated namespace/object members |
| `TypeError` | Type mismatches and wrong argument counts/types |
| `RangeError` | A value outside a supported semantic range, such as an invalid calendar date |
| `InvalidAssignTarget` | Assign into a temporary — one not reachable from a variable (`get()[i] = x`) |
| `Overflow` | Arithmetic outside a supported numeric or calendar range |
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
| `PermissionError` | Missing native namespace permission (`SZ6001`) |
| `ResourceError` | A recursion, allocation or input-size ceiling (`SZ6002`) |
| `UnsafeError` | An operation used outside its required `unsafe {}` block (`SZ6003`) |
| `SecurityError` | A protected native target refused by policy (`SZ6004`) |

Permission, resource, unsafe and security gates are fatal and normally bypass
`try/catch`; lockdown denials retain their documented recoverable compatibility
behavior. Stable fields/codes and the exact ceilings are specified in
[`spec/errors.md`](spec/errors.md) and [`spec/limits.md`](spec/limits.md).

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

Any `{ ... }` is a **Flash Scope** — the language's tool for bounding how long temporary data stays in memory. See [Flash Scopes](#flash-scopes) for what it is good for:

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

**Nested** targets work: the write is routed down the path from the root
variable, so `m[i][j] = x` and `obj.a.b = x` land where you wrote them.

```serez
let m = [[1, 2], [3, 4]];
m[0][1] = 99;
out m[0][1];                  // → 99

let grid = [[0, 0], [0, 0]];
grid[1][0] = 1;               // through a field works too: this.rows[i][j] = 1
```

What is still rejected — loudly, with `InvalidAssignTarget`, never a silent
no-op — is writing into a **temporary**: reading anything that is not reachable
from a variable yields a copy (value semantics), and there is nowhere for the
write to go back to.

```serez
fn any get() { return [1, 2]; }
// get()[0] = 99;             // ❌ ERROR: InvalidAssignTarget (the result is a copy)
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
| `.replace(from, to)` | Returns a new string with the **first** occurrence of `from` replaced by `to`. |
| `.replaceAll(from, to)` | Returns a new string with every occurrence replaced. |
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
| `.padStart(n[, text])` | Pads the start with `text` (default: space) to exactly `n` Unicode characters. `n` must be non-negative. |
| `.padEnd(n[, text])` | Pads the end with `text` (default: space) to exactly `n` Unicode characters. `n` must be non-negative. |

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

// replace changes the first occurrence; replaceAll changes every occurrence
let r = "one two one two one";
out r.replace("one", "X");        // → X two one two one
out r.replaceAll("one", "X");     // → X two X two X

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

Indexes and lengths count Unicode scalar values rather than UTF-8 bytes.
`substring` clamps negative indexes to zero; `slice` interprets negative bounds
relative to the end. Reversed bounds return an empty string. Wrong arity/types
are catchable `TypeError` (`SZ4002`), a negative padding target is catchable
`RangeError` (`SZ4000`), and unknown members are catchable `ReferenceError`
(`SZ4001`). Padding above 10,000,000 characters is a fatal resource error. See
the normative [String contract](spec/strings.md).

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

Scalar Math arguments accept `int` and `decimal`; exact `dec` values require an
explicit conversion. A value of any other type raises catchable `TypeError`
`SZ4002`. Arguments are evaluated before the operation, and a user `throw` or
runtime error produced by an argument propagates unchanged.

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

### Random

`Random` is a deterministic, seedable pseudo-random namespace. The same seed
and ordered calls reproduce the same stream; it shares generator state with
`Math.random()`. It requires no permission.

| Call | Result and constraints |
| --- | --- |
| `Random.seed(n)` | Reset from an `int`; negative seeds are valid. |
| `Random.decimal()` | `decimal` in `[0, 1)`. |
| `Random.int(min, max)` | Inclusive `int` in `[min, max]`, including the complete 64-bit integer domain. |
| `Random.uniform(lo, hi)` | Finite `decimal` in `[lo, hi)`; `lo < hi`. |
| `Random.normal(mean, std)` | Normal draw with finite parameters and `std >= 0`. |
| `Random.normalTensor(shape, mean, std)` | Tensor of normal draws. |
| `Random.uniformTensor(shape, lo, hi)` | Tensor of uniform draws. |
| `Random.shuffle(array)` | Shuffled copy; does not mutate the input. |
| `Random.choice(array)` | Element from a non-empty array. |
| `Random.bernoulli(p)` | Boolean with finite `p` in `[0, 1]`. |

Wrong types/arity raise catchable `TypeError` (`SZ4002`), invalid domains raise
catchable `RangeError` (`SZ4000`), and unknown members raise catchable
`ReferenceError` (`SZ4001`). Tensor allocation ceilings remain fatal. See the
normative [Random contract](spec/random.md).

> ⚠️ `Random` is a predictable LCG intended for games, simulations and
> reproducible tests. Use `Crypto.randomBytes` for secrets.

---

### File

`File` is a built-in namespace for file I/O operations.

| Function | Description |
|---|---|
| `File.exists(path)` | Returns `true` if the file at `path` exists. |
| `File.read(path)` | Returns the full file contents as a `string`. Runtime error if it cannot be read; files above 256 MiB are refused with fatal `SZ6002`. |
| `File.write(path, content)` | Writes `content` (converted to string) to `path`. Creates the file if it does not exist; overwrites if it does. Returns `null`. |
| `File.create(path)` | Creates an empty file at `path` if it does not already exist (touch). No-op if the file exists. Returns `null`. |
| `File.read_asBinary(path)` | Returns raw bytes as `[int]` (0–255); files above 256 MiB are refused with fatal `SZ6002`. |
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

Raw TCP client/server sockets over `std::net`, plus RFC 6455 WebSocket text frames. These are the low-level networking primitives — for a full HTTP/WebSocket server with routing, use the `serez-http` package. **Requires `use permissions { Socket }`** (or a project-level `"permissions": ["Socket"]` in `serez.json`).

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

### Modules (`import` / `export`)

A `.sz` file is a module. `export` marks what other files may use; `import` pulls another module's exports into the current scope.

```serez
// src/math.sz
export fn int double(int n) { return n * 2; }
export fn int quadruple(int n) { return double(double(n)); }
```

```serez
// index.sz
import "src/math";

out double(5);      // → 10
out quadruple(5);   // → 20
```

Imported names land directly in the importing scope — there is no namespace object and no `as` alias. The extension is omitted: `import "src/math"` loads `src/math.sz`.

**Paths are relative to the directory of the file doing the importing**, not to the project root. This trips people up in a package with `index.sz` at the root and modules under `src/`:

```serez
// index.sz          → import "src/parser";   ✅
// src/lexer.sz      → import "parser";       ✅  (sibling, simple name)
// src/lexer.sz      → import "src/parser";   ❌  looks for src/src/parser.sz
```

A bare package name resolves through several roots in order — the app's directory, the CWD, `<cwd>/packages`, `SEREZ_HOME`, the directory of the `sz` executable, and `~/.serez/packages` — trying `<root>/<pkg>/index.sz` at each:

```serez
import "serez-ui";
```

> **Export every function reachable from another file, including helpers.** A non-exported function is invisible when an exported function that calls it is invoked from a different module — even though both live in the same file. The failure surfaces at the call site, not at import:
>
> ```serez
> // src/math.sz
> fn int helper(int n) { return n + 1; }             // not exported
> export fn int useHelper(int n) { return helper(n); }
> ```
> ```serez
> // index.sz
> import "src/math";
> out useHelper(5);
> // ❌ ERROR: Variable not found: helper
> //     called from 'useHelper'
> ```
>
> The fix is `export fn int helper(...)`. This applies transitively: if `a` calls `b` calls `c`, all three need `export`.

Classes are visible globally once their module is imported. Functions are bound per importer, so import order matters when two modules export the same name — the last import wins.

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

CPU-backed compute buffers with a GPU-shaped API. Buffers are flat `decimal` arrays; the create / upload / dispatch / readback / free pattern mirrors real GPU compute so a future backend can swap the CPU implementation for actual GPU calls. Buffers are **not** garbage-collected — free them with `GPU.freeBuffer` when done. No permission declaration is required. Every buffer is capped at 256 MiB (33,554,432 `decimal`/`f64` elements), including uploads and `matmul` outputs; dimension overflow or a larger result is fatal `SZ6002`.

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
permission declaration required. Random bytes come from the operating system's
entropy source, and signatures use a vetted, audited implementation rather than
a hand-rolled one.

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

The protected-path refusal used by `OS.exec`/`OS.spawn` is fatal `SZ6004`, but
it is only a case-sensitive substring heuristic. It is not path canonicalization,
an allowlist or a sandbox; use OS isolation for untrusted process execution.

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

Every member enforces its documented arity before evaluating argument
expressions. Wrong arity/type is catchable `TypeError` (`SZ4002`), an invalid
calendar or epoch is catchable `RangeError` (`SZ4000`), field arithmetic
overflow is catchable `Overflow` (`SZ4000`), and an unknown member is catchable
`ReferenceError` (`SZ4001`). Nested runtime errors and user `throw` values
propagate unchanged. The normative contract is in
[`spec/datetime.md`](spec/datetime.md).

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

Putting those pieces together gives you a full graphical form — a text field plus a clickable button — in a single file.

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
| `Gui.nodeRect(x, y, w, h, color)` | Creates a node; returns its `int` id. Also `nodeRoundRect`, `nodeRoundRectOutline`, `nodeRectAlpha`, `nodeRectOutline`, `nodeCircle`, `nodeLine`, `nodePolyline`, `nodePolygon`, `nodeText`, `nodeTextPx`, `nodeImage`, `nodeClipPush`/`nodeClipPop`. |
| `Gui.nodeRoundRectOutline(x, y, w, h, radius, color)` | Outline (1 px, antialiased corners) of a rounded rect — the stroked counterpart of `nodeRoundRect`. |
| `Gui.nodeImage(x, y, imageId[, w, h[, alpha[, radius]]])` | Image node, additive arities: native size, scaled (`w`/`h`), with global alpha (0–255), and with AA-masked rounded corners (`radius`). |
| `Gui.nodeTextPx(x, y, text, px, color)` | Text node at a **literal pixel size** (any value, not only multiples of 8). `Gui.measureTextPx(text, px)` returns `[width_px, px]`. |
| `Gui.nodeTransform(id, rotDeg, scaleXmille, scaleYmille, origX, origY)` | Optional affine transform on a node: rotation in degrees and scale in thousandths (`1000` = 1.0), around an origin in canvas px. Identity `(0, 1000, 1000)` clears it. |
| `Gui.nodeSet(id, prop, value)` | Updates a property: `x, y, w, h, r, x2, y2, color, z, visible, text, scale, px, font, style, spacing, radius, alpha, width, points`. |
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
`(flag)` for truthiness) — last match wins.

Conditions **compose** with `and` / `or` / `not` (aliases `&&`, `||`, `!`).
Precedence is the usual one — `not` binds tighter than `and`, and `and` tighter
than `or`, so `a or b and c` reads as `a or (b and c)`; there are no grouping
parentheses. Connectors only count as whole words, so a name like `android` or a
quoted `"a or b"` is not split.

To apply **one condition to several rules**, wrap them in a `@when` block instead
of repeating it per selector. `@else` complements the previous branch and
`@else (cond)` chains else-if; branches are mutually exclusive (top to bottom,
first match wins), so ranges need no manual negation. `@when` blocks nest — the
conditions AND together — and a rule inside may still carry its own `(cond)`.
Unknown at-rules (`@media`, …) are discarded whole rather than corrupting the parse.

```css
@when (width < 300 and darkMode) {
    body  { color: #fff }
    .card { padding: 8 }
    #main { gap: 4 }
}
@else (width < 600) { body { color: #ccc } }
@else               { body { color: #333 } }
``` `img` takes a PNG/JPG **file path**
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

Serez-Code uses three complementary safety mechanisms: a permission manifest,
operation-level `unsafe` gates, and lockdown for selected capabilities. These
mechanisms reduce accidental access; they are **not a security sandbox** and do not
make hostile code safe to execute. In particular, `fetch` can access the network
under lockdown. Run untrusted programs inside an OS/container boundary with its own
filesystem, process and network restrictions.

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

Calling one outside the block aborts with structured fatal `UnsafeError`
(`SZ6003`); it is visible to CLI/tooling and deliberately bypasses `try/catch`.

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
| `Memory.*` and raw pointers | Access raw evaluator-managed memory |

`Memory.alloc(n)` accepts 1 through 256 MiB. Zero is a catchable argument
`TypeError`; a larger allocation is a fatal `ResourceError` (`SZ6002`).

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

Without a declared permission, every guarded namespace call fails immediately
with a structured fatal `PermissionError` (`SZ6001`) pointing to how to grant it.
The error is visible to CLI/tooling but deliberately bypasses `try/catch`.

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

// Record the response; it is published when the worker exits successfully
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
| `Task.reply(result: string) -> void` | (Worker only) Records a provisional result. The script continues; the result is published only if it exits successfully. |
| `Task.isDone(taskId: int) -> bool` | Returns `true` if the task completed successfully or failed with an error. |
| `Task.poll(taskId: int) -> string` | Retrieves the result of the task. If the task failed or panicked, it returns a string starting with `"ERROR: "`. |

Workers use isolated evaluators but native threads in the same process. Task IDs
and replies are shared only with the parent/descendant runtime tree, not with
unrelated embedders. A worker inherits lockdown when its parent is restricted;
outside lockdown it can use inline/manifest permissions like trusted source.

The runtime allows 32 concurrent workers, 1 MiB message/error payloads, 16 MiB worker
sources and 256 retained task records. Starting over the concurrent/message
ceilings is fatal `ResourceError` (`SZ6002`); old terminal records are evicted
before active workers. Unknown/evicted IDs are catchable `ReferenceError`
(`SZ4001`), and API arity/type mistakes are catchable `TypeError` (`SZ4002`).
Worker failures remain returned as `ERROR: [code] kind: message` strings for
compatibility. There is no cancellation or timeout. See the normative contract
in [`spec/tasks.md`](spec/tasks.md).

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

All field names and types from the interface declaration must be supplied.
Positional construction, missing or extra fields, and wrong field types are
catchable `TypeError` (`SZ4002`).

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

A `class` bundles data and behaviour. It may have a constructor (same name as
the class, prefixed with `public`) and any number of `public` or `private`
methods. A class without a constructor accepts zero construction arguments.

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

An unknown class/interface is a catchable `ReferenceError` (`SZ4001`). Invalid
class/interface construction shape, abstract instantiation and constructor
arity are catchable `TypeError` (`SZ4002`). The audited construction subset is
normative in [`spec/classes.md`](spec/classes.md).

**Field assignment:**

Fields set inside the constructor via `this.field = value` are created automatically. Any method can read or write them with the same syntax:

```serez
perro.energia = 50;   // direct field mutation from outside
```

**Methods** are called with dot syntax and parentheses, just like built-in methods:

```serez
out perro.getNombre();   // → Rex
```

Method lookup walks the runtime class and then its parents. A missing member is
catchable `ReferenceError` (`SZ4001`); invalid instance/static arity and a value
that violates the method's declared return type are catchable `TypeError`
(`SZ4002`). A missing `ClassName.staticMethod()` reports the class/member rather
than masquerading as an undeclared class variable. See
[`spec/classes.md`](spec/classes.md) for the audited dispatch contract.

---

#### Inheritance

Use `: ParentClass` to inherit from another class. `super(args...)` executes the
parent constructor body against the same `this` object. When the parent can be
called with no arguments, Serez inserts that call before the child constructor
body if the child omitted it.

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

Inheritance is single — a class can have at most one parent. A parent may be
declared later, but the child cannot be instantiated until it exists; attempting
to use the unresolved hierarchy raises catchable `ReferenceError` (`SZ4001`). A
self/indirect cycle is rejected at declaration with catchable `TypeError`
(`SZ4002`) instead of entering an unbounded ancestor lookup. The normative graph
contract is in [`spec/classes.md`](spec/classes.md).

**Method resolution** walks the chain from the most-derived class upward until the method is found:

```
Perro.describir()    → found in Perro — use it
Perro.hacer_sonido() → not in Perro → found in Animal — use it
```

**`super()` semantics:**

`super(args...)` runs the parent constructor's body against the same `this` that
the child constructor received. Only variables assigned to `this` become fields
visible to the child. If the parent requires arguments, a child constructor that
omits `super(...)` remains allowed for compatibility and must initialize the
needed fields itself; a child with no constructor instead gets catchable
`TypeError` (`SZ4002`) because the runtime cannot supply those arguments.

The implicit-call check is syntactic and conservative: a `super(...)` occurrence
in any branch suppresses insertion even if that branch does not run. An empty
`super()` to a parent without a constructor is a no-op, while supplying arguments
is `SZ4002`. Grand-parent constructors are not synthesized by an explicit
`super()` call; each required level must chain itself.

`super.method(...)` begins lookup at the direct parent, bypasses the current
override and keeps the same `this`. Invalid context/parent/arity is catchable
`TypeError` (`SZ4002`); a missing parent method is catchable `ReferenceError`
(`SZ4001`). The normative audited subset is in
[`spec/classes.md`](spec/classes.md).

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

External calls or bound references to `private` methods remain refused with
catchable `TypeError` (`SZ4002`); catching the error does not grant access. The
runtime currently treats subclass execution as internal for inherited private
members. This compatibility caveat is documented in `spec/classes.md` and is not
a host-security boundary.

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

An `abstract` class cannot be instantiated directly. It is designed to be
subclassed. Attempting to call `new` on it raises catchable `TypeError`
(`SZ4002`).

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

A `sealed` class cannot be inherited from. Attempting to extend it raises
catchable `TypeError` (`SZ4002`); the rejected child is not registered.

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

A property with only a getter and no setter is read-only. Assigning to it, using
a private accessor externally, malformed accessor arity, or assigning a field on
a non-instance raises catchable `TypeError` (`SZ4002`). A `throw` from inside an
accessor propagates unchanged. See the normative audited subset and known dynamic
field compatibility debt in [`spec/classes.md`](spec/classes.md).

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

Every arithmetic operation is checked. Overflow raises an error instead of silently wrapping around:

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

## Flash Scopes

A **Flash Scope** is an inner `{ ... }` block you write inside a function or method. Not the function's own braces — a block *within* the body, opened by you, on purpose:

```serez
fn int sumar(int a, int b) {
    let res = 0;      // declared OUTSIDE the braces — it survives them

    {                 // ← this is the Flash Scope
        res = a + b;
    }

    return res;       // → 5
}
```

Everything declared between those inner braces is gone at the closing brace. The only way to keep a result is to put it in a variable declared *before* the block, assign into it from inside, and use it *after* the block — exactly like `res` above. The `return` belongs after the Flash Scope, not inside it.

It works the same way outside a function. At the top level of a script the shape is identical — declare before, compute inside, use after:

```serez
let a   = 1;
let b   = 2;
let res = 0;      // declared before the block

{
    res = a + b;  // computed inside it
}

out res;          // → 3
```

So the rule does not depend on being in a function: **what is declared inside the braces is temporary, and what you want to keep is declared outside them.**

### What Flash Scopes are for

They solve a specific problem: a computation that needs a lot of RAM but keeps only a fraction of it. Put the bulky part inside the braces, keep the piece you actually need in the outer variable, and everything else is released at `}` — not eventually, not when some collector gets around to it. At the brace.

```serez
use permissions { File }

fn [string] topThree(string path) {
    let top = [];                                    // the small result

    {
        let raw    = File.read(path);                // the whole file
        let rows   = raw.split("\n");                // + one string per row
        let parsed = rows.map(r => r.split(","));    // + every field of every row
        top = parsed.slice(0, 3);                    // ← only this is worth keeping
    }

    return top;
}
```

Three copies of the dataset existed inside those braces. At the closing brace `raw`, `rows` and `parsed` are released, and the function returns holding only the three rows it was asked for. Without the inner block all three would stay alive until `topThree` itself returned.

That is the idiom: **build big, keep small, and mark the boundary with braces.** It applies to any structure you only need a slice of — a parsed file, a query result, an intermediate index you use once and discard.

Blocks nest, so you can peel in stages: an inner block releases its own temporaries while the outer one keeps working.

### How memory works underneath

Serez-Code has no garbage collector and nothing to free by hand. Memory is **region-based**: values live in arenas, and leaving a scope releases that scope's region in one step. Deterministic, no pauses, nothing to tune. Flash Scopes are the language-level handle on that machinery — the way you decide, in your own code, where a region begins and ends.

Two consequences of the model are visible from `.sz`, and they explain behavior that is otherwise surprising.

### Values are copied, not shared

Reading a variable, passing an argument, returning a value or reading a field gives you a **copy**. Two names never point at the same data, so nothing you hold can be invalidated by a scope you don't control:

```serez
let a = [1, 2, 3];
let b = a;        // b is an independent copy
b.push(4);

out a.length;     // → 3 — a is untouched
out b.length;     // → 4
```

The same rule explains a classic surprise: mutating an object you read out of a field mutates *your copy*, not the field. Write it back if you want the change to stick:

```serez
let cfg = this.config;   // copy
cfg.retries = 5;
this.config = cfg;       // ← without this line the change is lost
```

### Copying big values costs

Because copies are real copies, size matters. Returning a 100k-element array out of a function copies 100k elements, and writing a single element of a large array (`a[i] = x`) is proportional to the array's length, not constant time. Building a large array element by element in a loop therefore gets quadratic.

For heavy numeric work reach for `Tensor` (see [Autodiff & Tensors](#autodiff--tensors)) — it stores its numbers flat and is designed for bulk operations.

### Closures are the one exception: they share

A variable captured by a closure is *not* copied. The closure and the surrounding code share one cell, so a mutation inside the closure is visible outside, and vice versa:

```serez
fn counter() {
    let n = 0;
    return () => { n = n + 1; return n; };
}

let next = counter();
out next();   // → 1
out next();   // → 2 — n outlived the function that declared it
```

That is what makes counters, accumulators and event handlers work. The cost: a captured variable lives until the program ends, so creating closures in a hot loop — one per frame, one per iteration — keeps accumulating memory. Create them once outside the loop when you can.

Top-level `out` statements are free of charge: whatever they allocate just to print is released as soon as the statement finishes, no matter how many times you call them.

> Want the machinery — arenas, watermarks, the promote-before-pop protocol? It is documented for contributors in [DEVELOPMENT.md](DEVELOPMENT.md).

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

## Known Gotchas

None of these are bugs — they are correct semantics — but they surprise almost everyone the first time.

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

### System namespace names cannot name a class, interface or enum

`Task`, `File`, `OS`, `Gui`, `Env`, `Time`, `Socket`, `System`, `Terminal` … are
reserved. Declaring a type with one of those names is a **parse error**, not a
warning:

```serez
class Task { … }       // ❌ 'Task' is a reserved system namespace
class TaskItem { … }   // ✅ rename it
```

The rule exists because otherwise a user class would shadow the native namespace
of the same name. It arrived with the `Task` namespace in **v7.0.0**, so code
written before that may need a rename. Only the exact name collides: `UrgentTask`
or `TaskList` are fine.

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

Want to work on the language itself — the interpreter, the standard namespaces, the tooling? That is a different job from writing `.sz` programs, and it has its own guide: **[DEVELOPMENT.md](DEVELOPMENT.md)** covers the repository layout, the interpreter architecture, the test suite and the release pipeline. Issue and pull-request conventions are in [CONTRIBUTING.md](CONTRIBUTING.md).

Found a bug in the language while writing your program? Open an issue — a minimal `.sz` file that reproduces it is the most useful thing you can attach.

---

## License

See [LICENSE](LICENSE) for details.

---

<div align="center">

Built with ❤️ — no GC required.

</div>
