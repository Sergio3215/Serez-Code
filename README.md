# ![](./img/serez-icon.svg) Serez-Code

> A hand-crafted, interpreted programming language written from scratch in Rust — no garbage collector, no heavy dependencies, and blazing-fast memory cleanup via **Flash Scopes**.

```serez
fn int fibonacci(int n) {
    if (n <= 1) {
        return n;
    }
    return fibonacci(n - 1) + fibonacci(n - 2);
}

out fibonacci(10);   // → Integer(55)
```

---

## ✨ What makes Serez-Code different?

Most interpreters lean on Rust's `Rc<RefCell<T>>` or a garbage collector to manage object lifetimes. Serez-Code takes a different path:

| Feature | Traditional approach | Serez-Code |
|---|---|---|
| Memory management | GC or `Rc<RefCell<T>>` | Region-based bump allocators |
| Scope cleanup | Reference counting / GC pause | `O(1)` watermark truncation |
| "Pointers" | Box / Rc | `ObjectRef` — a `(RegionId, usize)` tuple |
| Type checking | Usually static or fully dynamic | Optional annotations, checked at call sites |

### Flash Scopes

Every `{ ... }` block is a **Flash Scope**. When the interpreter exits the block, it records the arena watermark from entry time and calls `truncate(watermark)` — destroying all block-local objects instantly, with zero iteration.

```serez
let total = 0;

{
    let temp = expensive_computation();
    total = temp * 2;
}   // ← temp is gone. O(1). No GC pause.

out total;
```

---

## 🚀 Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (stable, edition 2024)

### Install

```bash
git clone https://github.com/your-org/serez-code
cd serez-code
cargo install --path . --force
```

This installs the `sz` binary globally.

### Run a script

Create `hello.sz`:

```serez
fn void greet(string name) {
    out "Hello, ";
    out name;
}

greet("World");
```

```bash
sz hello.sz
```

> **Note:** Serez-Code doesn't print values automatically. Use `out` to send output to stdout.

### Interactive REPL

```bash
sz
>> let x = 10;
>> x * 3
Integer(30)
```

### Static profiler

```bash
sz --check script.sz
```

Analyzes memory cost per function before executing, showing a color-coded criticality bar:

- 🟢 `< 1 KB` — Safe
- 🟡 `< 10 KB` — Warning
- 🔴 `> 10 KB` — Critical

---

## 📖 Language Reference

### Variables

```serez
let name = "Sergio";
let count = 42;
let active = true;

count = count + 1;   // reassignment — no `let`
```

### Functions

Two supported styles:

```serez
// Named function declarations — fn <type> <name>(<params>) { ... }
fn void greet() { out "hi"; }
fn string getName() { return "Sergio"; }
fn int add(int a, int b) { return a + b; }
fn bool isEven(int n) { return n == 0; }
```

```serez
// Anonymous arrow functions — <type> (<params>) => { ... }
// Type goes before the parens, braces are always required
let greet   = void ()   => { out "hi"; }
let getName = string () => { return "Sergio"; }
let double  = int (x)   => { return x * 2; }
let check   = bool (x)  => { return x == 0; }
```

Type annotations are **enforced at runtime** — parameter types and return types are checked at every call site.

### Control flow

```serez
if (x > 0) {
    out "positive";
} else {
    out "non-positive";
}

let i = 0;
while (i < 5) {
    out i;
    i = i + 1;
}
```

### Arrays

```serez
let nums = [1, 2, 3, 4, 5];
out nums[2];   // → Integer(3)
```

### Operators

| Category | Operators |
|---|---|
| Arithmetic | `+ - * / %` |
| Comparison | `< > == !=` |
| Logical | `!` (prefix) |
| String | `+` (concat), `*` (repeat) |

### Output

```serez
out "hello";       // → String("hello")
out 42;            // → Integer(42)
out true;          // → Boolean(true)
```

---

## 🏗️ Architecture Overview

```
src/
├── main.rs        — Entry point (file, --check, REPL)
├── token.rs       — Token types and keyword table
├── lexer.rs       — Hand-rolled character scanner
├── ast.rs         — AST node types (Statement, Expression)
├── parser.rs      — Pratt (TDOP) parser
├── region.rs      — Arena allocator + ObjectRef
├── scope.rs       — ScopeStack with watermark-based cleanup
└── evaluator.rs   — Tree-walking evaluator + static profiler
```

### Data flow

```
Source text
    │
    ▼
Lexer  ──→  Token stream
    │
    ▼
Parser  ──→  AST (Program)
    │
    ▼
Evaluator  ──→  ObjectRef  ──→  stdout / return value
```

### Memory regions

```
┌────────────────────────────────┐
│         Global Arena           │  ← top-level variables, fn declarations
│  [Null, x=42, greet=Fn, ...]  │  never reset during a script run
└────────────────────────────────┘

┌────────────────────────────────┐
│         Scoped Arena           │  ← local vars, function args, block temps
│  frame0_mark  frame1_mark ...  │  truncated on every scope exit (O(1))
└────────────────────────────────┘
```

---

## 🛠️ Contributing

All contributions are welcome — bug fixes, new language features, docs improvements, or test cases.

### 1. Fork and clone

```bash
git clone https://github.com/your-org/serez-code
cd serez-code
```

### 2. Build and run tests

```bash
cargo build
cargo test
```

The lexer includes a comprehensive token test suite in `lexer.rs`. Run just that with:

```bash
cargo test test_next_token
```

### 3. Project conventions

- **No `unsafe`** — the memory model is intentionally built without unsafe blocks.
- **No external runtime dependencies** — keep `[dependencies]` empty. Dev dependencies are fine.
- **Error messages go to stdout** (current behavior) with the `❌` prefix pattern.
- **All new syntax** must be reflected in: `token.rs` → `lexer.rs` → `ast.rs` → `parser.rs` → `evaluator.rs`. Don't add to the evaluator without adding to the AST first.
- **Flash Scope invariant**: any new block-level construct must call `scopes.push()` before evaluating its body and `scopes.pop()` after — in all code paths including error paths.

### 4. Adding a new statement

1. Add a token in `token.rs` and wire it in `lookup_ident()` if keyword-based.
2. Add the AST node(s) in `ast.rs`.
3. Add a parse handler in `parser.rs` under `parse_statement()`.
4. Add an eval handler in `evaluator.rs` under `eval_statement()`.
5. Add a test script under `tests/` or inline in the relevant module.

### 5. Open a PR

- Keep commits focused (one logical change per commit).
- Describe *why* a change was made, not just *what*.
- PRs that add language features should include at least one `.sz` example file.

---

## 🗺️ Roadmap

- [ ] Lexical closures (captured scope variables)
- [ ] `else if` chain
- [ ] Array mutation via index assignment (`arr[i] = expr`)
- [ ] String interpolation
- [ ] Native higher-order functions (`map`, `filter`)
- [ ] Span-aware error diagnostics
- [ ] Standard library (basic math, string utilities)

---

## 📄 License

See [LICENSE](LICENSE) for details.

---

<p align="center">Built with ❤️ and Rust — no GC required.</p>
