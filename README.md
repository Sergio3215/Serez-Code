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
10. [Contributing](#contributing)
11. [Roadmap](#roadmap)
12. [License](#license)
13. [Bugs Fixed List](bugs.md)

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
sz script.sz
```

Errors go to `stderr`. You can separate program output from errors:

```bash
sz script.sz > output.txt    # captures only out statements
sz script.sz 2> errors.txt   # captures only runtime errors
```

### Start the REPL

```bash
sz
>> let x = 10;
>> out x * 3;
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
| `bool` | `true`, `false` | Boolean |
| `string` | `"hello"`, `"foo bar"` | UTF-8 string |
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

`catch` is optional. `finally` is optional. Both together are also valid:

```serez
try {
    riskyOperation();
} finally {
    cleanup();   // runs even if riskyOperation throws
}
```

Unhandled exceptions (no enclosing `try`) terminate the program with a runtime error message. Runtime errors (stack overflow, out-of-bounds, etc.) are **not** catchable via `try/catch`.

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
| `.pop()` | Removes and returns the last element (returns `null` on empty array). |
| `.shift()` | Removes and returns the first element (returns `null` on empty array). |
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

If the key does not exist, `null` is returned. Use `??` to provide a default value: `d["missing"] ?? 0`.

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
| `JSON.stringify(value)` | Converts any value (int, decimal, bool, string, array, dict, null) to a JSON string. |
| `JSON.parse(string)` | Parses a JSON string and returns the equivalent Serez-Code value. Runtime error on invalid JSON. |

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
| (dict returns `null` for missing key) | Accessing a dict key that doesn't exist returns `null`; use `??` to provide a default |
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
├── main.rs          — CLI entry point: file execution, --check mode, REPL
├── token.rs         — Token enum and keyword-to-token lookup table
├── lexer.rs         — Hand-rolled character scanner; byte-indexed over the source String
├── ast.rs           — AST node definitions (Statement, Expression, BlockStatement, …)
├── parser.rs        — Pratt (TDOP) parser with 8-level precedence + error recovery
├── type_checker.rs  — Static pre-run type checker with literal and variable inference
├── region.rs        — Arena allocator (with_capacity), ObjectRef, ObjectData/OwnedValue with Rc<BlockStatement>
├── scope.rs         — ScopeStack — push/pop/lookup with watermark cleanup and all_bindings dedup
├── evaluator.rs     — Tree-walking interpreter, Flash Scope protocol, StoredMethod cache, static profiler
└── repl.rs          — Read-eval-print loop
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

#### `StoredMethod` — O(1) method dispatch

Class methods are stored as `Vec<StoredMethod>` inside `StoredClass`. `StoredMethod` holds a `body: Rc<BlockStatement>`, so each `find_method()` clone is O(1) regardless of how large the method body is. Previously, every method call cloned the entire `ast::ClassMethod` including its body.

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

## Contributing

All contributions are welcome — bug fixes, new language features, documentation, or test cases.

### Build and test

```bash
cargo build
cargo test              # runs the lexer unit test
sz test.sz              # run the integration test script
sz test_arrays.sz       # run the array test script
sz test_complex.sz      # run the advanced/edge-case test suite
```

### Project conventions

- **No `unsafe`** — the memory model is intentionally built without unsafe blocks. Keep it that way.
- **No external runtime dependencies** — `[dependencies]` stays empty. Dev dependencies are fine.
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
- [x] JSON namespace — `stringify`, `parse`
- [x] Power operator — `**` for integer and decimal exponentiation
- [x] Bitwise operators — `&`, `|`, `^`, `~`, `<<`, `>>` (64-bit signed integers); binary (`0b`) and hex (`0x`) literals; numeric separators (`1_000_000`)
- [x] `is` type-check operator — `expr is TypeName` returns `bool` at runtime
- [x] Default parameters — `fn int f(int x = 10)` with fallback when argument is omitted
- [x] Security test suite — 17 error tests (`sec_*.sz`) + 6 unit test files (`unit_sec_*.sz`) covering arithmetic, null safety, type safety, error isolation, injection, and resource limits

### Type system
- [x] Typed arrays — `[int]`, `[string]`, `[decimal]`, `[T?]` with element-level enforcement on `push`, `unshift`, index-assign, and construction
- [x] Type inference for function call results — `let x = add(1, 2)` infers `x: int` in the static checker
- [x] Optional / nullable types — `int?`, `string?`, `fn int? search()`, `null` literal, null equality (`== null`, `!= null`)

### Tooling
- [x] Security test runner — `-security` flag on `run_tests.ps1` runs all security test files
- [x] Span-aware error diagnostics — parser and runtime errors show the source line with a `^` caret
- [x] Watch mode — `sz --watch file.sz` re-runs on every save
- [x] VS Code extension — syntax highlighting for `.sz` files (`vscode-serez/`)
- [ ] `.sz` file formatter
- [ ] LSP server for editor support

---

## License

See [LICENSE](LICENSE) for details.

---

<div align="center">

Built with ❤️ and Rust — no GC required.

</div>
