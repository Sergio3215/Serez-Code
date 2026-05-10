<div align="center">

# ![](./img/serez-icon.svg) Serez-Code
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
   - [Functions](#functions)
   - [Control Flow](#control-flow)
   - [Arrays](#arrays)
   - [Operators](#operators)
   - [Output](#output)
   - [Comments](#comments)
4. [Flash Scopes — Memory Model](#flash-scopes--memory-model)
5. [Static Profiler](#static-profiler-check-mode)
6. [Architecture Overview](#architecture-overview)
7. [Contributing](#contributing)
8. [Roadmap](#roadmap)
9. [License](#license)

---

## Why Serez-Code?

Most interpreted languages manage object lifetimes with a garbage collector or Rust's `Rc<RefCell<T>>`. Serez-Code takes a fundamentally different approach: **region-based arena allocation** with watermark-based cleanup.

| Trait | Traditional interpreters | Serez-Code |
|---|---|---|
| Memory management | GC pauses / reference counting | Bump allocator + watermark truncation |
| Scope cleanup | Non-deterministic (GC) or O(n) | Deterministic, `O(1)` per scope exit |
| Object references | `Box` / `Rc` / raw pointers | `ObjectRef` — a safe `(RegionId, usize)` index |
| Type safety | Fully dynamic or fully static | Optional annotations, enforced at every call site |
| `unsafe` code | Often required for performance | **Zero `unsafe` blocks** |

Every `{ ... }` block is a **Flash Scope**. When the interpreter exits it, all block-local memory disappears in a single `truncate()` call — no iteration, no reference counting, no GC pause.

---

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) stable, edition 2024

### Install

```bash
git clone https://github.com/your-org/serez-code
cd serez-code
cargo install --path . --force
```

This installs the `sz` binary globally.

### Run a script

```bash
sz hello.sz
```

### Start the REPL

```bash
sz
>> let x = 10;
>> x * 3
30
```

### Analyze memory usage

```bash
sz --check script.sz
```

### Check version

```bash
sz --version
```

> **Note:** Serez-Code does not auto-print expression results when running files. Use `out` to send values to stdout.

---

## Language Reference

### Variables

Variables are declared with `let`. Reassignment uses plain `=` — no `let` again.

```serez
let name   = "Jhon";
let count  = 20;
let active = true;

count = count + 1;   // reassignment — variable must already exist
```

Attempting to reassign an undeclared variable is a runtime error.

```serez
x = 5;   // ❌ ERROR: Undeclared variable: x
```

---

### Types

Serez-Code has four primitive types and two compound types:

| Type | Literal examples | Notes |
|---|---|---|
| `int` | `0`, `42`, `-7` | 64-bit signed integer (`i64`) |
| `bool` | `true`, `false` | |
| `string` | `"hello"`, `"foo bar"` | UTF-8, no escape sequences yet |
| `void` | — | Signals absence of a return value |
| Array | `[1, 2, "x"]` | Heterogeneous, 0-indexed |
| Function | `fn int add(...)` | First-class value |

Type annotations are **optional** on parameters and return types. When present, they are **enforced at every call site** at runtime — not at compile time.

```serez
fn int strictAdd(int a, int b) {
    return a + b;
}

strictAdd(1, "oops");   // ❌ TYPE ERROR: Parameter 'b' expected 'int'
```

---

### Functions

Serez-Code supports two syntaxes for defining functions: **named declarations** and **arrow functions**.

#### Named declarations

```
fn <return_type> <name>(<params>) { <body> }
```

```serez
fn void greet(string name) {
    out "Hello, ";
    out name;
}

fn int add(int a, int b) {
    return a + b;
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
let double  = int (int n)     => { return n * 2; }
let greet   = void (string s) => { out s; }
let isEven  = bool (int n)    => { return n == 0; }
```

#### Mixed / untyped parameters

Type annotations are per-parameter. You can mix typed and untyped in the same signature:

```serez
fn int mixta(x, int y, string z) {
    out z;
    return y + 100;
}

mixta(1, 50, "processing...");   // → 150
```

#### Anonymous functions as values

Functions are first-class. Assign them, pass them, store them in variables:

```serez
let run = fn void () {
    out "running anonymous logic";
};

run();
```

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

---

### Control Flow

#### `if` / `else`

```serez
if (x > 0) {
    out "positive";
} else {
    out "non-positive";
}
```

`if` is an expression — it can appear anywhere a value is expected.

#### `else if` chaining

```serez
if (score >= 90) {
    out "A";
} else if (score >= 75) {
    out "B";
} else {
    out "C";
}
```

#### `while`

```serez
let i = 0;
while (i < 5) {
    out i;
    i = i + 1;
}
```

`return` inside a `while` propagates up through the loop and exits the enclosing function:

```serez
fn int findFirst(int target) {
    let i = 0;
    while (i < 10) {
        if (i == target) {
            return i;   // exits the function immediately
        }
        i = i + 1;
    }
    return -1;
}
```

#### Standalone blocks

Any `{ ... }` creates a new Flash Scope. Variables declared inside are invisible outside:

```serez
let y = 1;

{
    let x = 10;   // x lives only in this block
    y = 100;      // y is in an outer scope — mutation is allowed
}

out y;   // → 100
// out x;   // ❌ ERROR: Variable not found: x
```

---

### Arrays

Arrays are heterogeneous and 0-indexed. Indexing with an out-of-bounds value is a runtime error.

```serez
let nums   = [1, 2, 3, 4, 5];
let mixed  = [1, "two", true];

out nums[0];     // → 1
out nums[2];     // → 3
out mixed[1];    // → two
```

---

### Operators

#### Arithmetic

```serez
out 10 + 3;    // → 13
out 10 - 3;    // → 7
out 10 * 3;    // → 30
out 10 / 3;    // → 3   (integer division)
out 10 % 3;    // → 1
```

Division by zero is a runtime error.

#### Comparison

```serez
out 5 > 3;     // → true
out 5 < 3;     // → false
out 5 == 5;    // → true
out 5 != 3;    // → true
```

#### Logical

```serez
out !true;     // → false
out !false;    // → true
```

#### String operations

```serez
out "hello" + " world";    // → hello world  (concatenation)
out "ha" * 3;              // → hahaha        (repetition)
out "a" == "a";            // → true
out "a" != "b";            // → true
```

String repetition requires a non-negative integer. Negative repeat is a runtime error.

---

### Output

`out` prints any value to stdout followed by a newline. It accepts any expression:

```serez
out "hello";              // → hello
out 42;                   // → 42
out true;                 // → true
out [1, 2, 3];            // → [1, 2, 3]
out "score: " + "100";    // → score: 100
out fibonacci(8);         // → 21
```

---

### Comments

Single-line comments with `//`. No block comments yet.

```serez
// This is a comment
let x = 5;   // inline comment
```

---

## Flash Scopes — Memory Model

Flash Scopes are the core innovation of Serez-Code's runtime. They replace garbage collection with a deterministic, arena-based memory model.

### How it works

The runtime maintains two separate memory regions:

```
┌──────────────────────────────────────┐
│            Global Arena              │
│  [Null, x=42, greet=Fn, result=...]  │  ← top-level vars and fn declarations
│  Never reset during a script run     │     persist for the lifetime of the program
└──────────────────────────────────────┘

┌──────────────────────────────────────┐
│            Scoped Arena              │
│  [frame0_data ... | frame1_data ...] │  ← local vars, function args, block temps
│        ^mark0            ^mark1      │     truncated on every scope exit
└──────────────────────────────────────┘
```

Every time the interpreter enters a `{ ... }` block (whether a function body, `if` branch, `while` body, or a standalone block), it:

1. Records the current **watermark** of the scoped arena.
2. Evaluates the block's statements, allocating into the arena normally.
3. On exit, calls `arena.truncate(watermark)` — instantly freeing every object allocated in that block.

### The "promote before pop" invariant

If a block produces a return value, the runtime clones the value's data **before** calling `pop()`, then re-allocates it in the parent scope. This guarantees the returned value is never a dangling reference.

```serez
let total = 0;

{
    let temp = 42 * 2;   // allocated in scoped arena at index N
    total = temp;        // temp's data is promoted to outer scope
}                        // ← truncate(watermark): temp is gone, O(1)

out total;   // → 84  (lives in the outer or global arena)
```

### Why it matters

| Property | Result |
|---|---|
| Deterministic | Memory is freed at an exact, predictable point in the code |
| No GC pauses | Cleanup is a single `Vec::truncate()` call |
| No `unsafe` | All references are `(RegionId, usize)` index pairs — they can't dangle |
| No `Rc` / `RefCell` | No shared ownership, no runtime borrow checking overhead |

---

## Static Profiler (`--check` mode)

Run `sz --check script.sz` to analyze your program's memory footprint **before executing it**. The profiler walks the AST and estimates the byte cost of each function using heuristic rules:

| Node | Estimated cost |
|---|---|
| `int` literal | 8 bytes |
| `bool` literal | 1 byte |
| `string` literal | 24 + string length bytes |
| Identifier lookup | 8 bytes |
| Infix expression | 8 + left + right bytes |
| Function call | 8 + sum of arguments bytes |
| Array literal | 24 + sum of elements bytes |
| `if` expression | condition + max(consequence, alternative) |

Each function is classified by criticality:

```
Function 'fibonacci': ~312 estimated bytes
  Criticality: ██  🟢 < 1KB (Safe)

Function 'processData': ~11840 estimated bytes
  Criticality: ██████████  🔴 > 10KB (Critical)
```

> **Note:** These are AST-level heuristic estimates, not exact runtime measurements. Use them as a relative guide, not an absolute profiler.

---

## Architecture Overview

```
src/
├── main.rs        — CLI entry point: file execution, --check, REPL
├── token.rs       — Token types and keyword lookup table
├── lexer.rs       — Hand-rolled character scanner with 1-char lookahead
├── ast.rs         — AST node definitions (Statement, Expression, etc.)
├── parser.rs      — Pratt (TDOP) parser, 8-level precedence table
├── region.rs      — Arena allocator (bump alloc) + ObjectRef + ObjectData
├── scope.rs       — ScopeStack with watermark-based Flash Scope cleanup
└── evaluator.rs   — Tree-walking interpreter + static memory profiler
```

### Data flow

```
Source (.sz file or REPL line)
        │
        ▼
    Lexer  ──────────────────────►  Token stream
        │
        ▼
    Parser (Pratt TDOP)  ────────►  AST (Program)
        │
        ▼
    Evaluator  ──────────────────►  ObjectRef
        │                                │
        ▼                               ▼
  ScopeStack                       stdout / return value
  (Flash Scopes)
```

### Parser — Pratt precedence table

The parser uses Top-Down Operator Precedence (Pratt parsing). Operator precedence from lowest to highest:

| Level | Operators |
|---|---|
| `Lowest` | — |
| `Equals` | `==` `!=` |
| `LessGreater` | `<` `>` |
| `Sum` | `+` `-` |
| `Product` | `*` `/` `%` |
| `Prefix` | `-x` `!x` |
| `Call` | `f(x)` |
| `Index` | `a[i]` |

---

## Contributing

All contributions are welcome — bug fixes, new language features, documentation improvements, or test cases.

### 1. Fork and clone

```bash
git clone https://github.com/your-org/serez-code
cd serez-code
```

### 2. Build and test

```bash
cargo build
cargo test
```

Run just the lexer suite:

```bash
cargo test test_next_token
```

### 3. Project conventions

- **No `unsafe`** — the memory model is intentionally built without unsafe blocks. Keep it that way.
- **No external runtime dependencies** — `[dependencies]` stays empty. Dev dependencies are fine.
- **Error messages use `❌` prefix** and go to stdout (current behavior).
- **All new syntax** must flow through the full pipeline: `token.rs` → `lexer.rs` → `ast.rs` → `parser.rs` → `evaluator.rs`. Never add to the evaluator without adding to the AST first.
- **Flash Scope invariant** — any new block-level construct must call `scopes.push()` before evaluating its body and `scopes.pop()` after, in **all** code paths including error paths.

### 4. Adding a new statement

1. Add a `TokenType` variant in `token.rs`. If keyword-based, wire it in `lookup_ident()`.
2. Add the AST node(s) in `ast.rs`.
3. Add a parse handler in `parser.rs` inside `parse_statement()`.
4. Add an eval handler in `evaluator.rs` inside `eval_statement()`.
5. Add a test script (`.sz` file) or inline test in the relevant module.

### 5. Open a PR

- One logical change per commit.
- Describe **why** a change was made, not just what.
- PRs that add language features must include at least one `.sz` example file.

---

## Roadmap

### Language features
- [ ] Lexical closures (captured scope variables)
- [ ] `for` loop
- [ ] Array mutation via index: `arr[i] = expr`
- [ ] String interpolation: `"Hello, {name}!"`
- [ ] Native higher-order functions: `map`, `filter`, `reduce`

### Type system
- [ ] Typed arrays: `[int]`, `[string]`
- [ ] Optional / nullable types

### Tooling
- [ ] Span-aware error diagnostics (line + column numbers)
- [ ] Standard library (math, string utilities)
- [ ] `.sz` file formatter
- [ ] LSP server for editor support

---

## License

See [LICENSE](LICENSE) for details.

---

<div align="center">

Built with ❤️ and Rust — no GC required.

</div>
