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
| `Equals` | `==` `!=` |
| `LessGreater` | `<` `>` `<=` `>=` |
| `Sum` | `+` `-` |
| `Product` | `*` `/` `%` |
| `Prefix` | `-x` `!x` |
| `Call` | `f(x)` `.method(args)` |
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

C-style for loop. The initializer must be a `let` declaration; the update must be an assignment.

```
for (<let init>; <condition>; <update>) { <body> }
```

```serez
for (let i = 0; i < 5; i = i + 1) {
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
| `.pop()` | Removes and returns the last element. |
| `.shift()` | Removes and returns the first element. |
| `.unshift(val)` | Prepends `val` to the beginning (mutates in-place). |
| `.sort()` | Sorts in ascending order (mutates in-place, returns the same array). |
| `.sort("desc")` | Sorts in descending order (mutates in-place, returns the same array). |
| `.sort((a, b) => expr)` | Sorts with a custom comparator lambda. Positive result = swap (like JS). |

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

#### Array properties

`.length` is a property (no parentheses) that returns the number of elements:

```serez
let a = [10, 20, 30, 40];
out a.length;   // → 4
out [].length;  // → 0
```

---

### String Methods

All string methods are called with dot syntax. `.length` is a property; all others are method calls.

| Method / property | Description |
|---|---|
| `.length` | Number of Unicode characters (UTF-8 aware). |
| `.toString()` | Returns the string itself (identity for strings; works on `int`, `decimal`, `bool` too). |
| `.substring(start, end)` | Returns characters from index `start` (inclusive) to `end` (exclusive). |
| `.split(sep)` | Splits the string by `sep` and returns an array of substrings. Empty `sep` splits into individual characters. |
| `.replace(from, to)` | Returns a new string with the **first** occurrence of `from` replaced by `to`. |
| `.replaceAll(from, to)` | Returns a new string with **all** occurrences of `from` replaced by `to`. |
| `.includes(sub)` / `.contains(sub)` | Returns `true` if the string contains `sub`, `false` otherwise. |

```serez
let s = "hello world";

out s.length;                     // → 11
out s.substring(0, 5);            // → hello
out s.split(" ");                 // → [hello, world]
out s.includes("world");          // → true
out s.includes("xyz");            // → false
out "abc".split("");              // → [a, b, c]

// replace vs replaceAll
let r = "one two one two one";
out r.replace("one", "X");        // → "X two one two one"   (first only)
out r.replaceAll("one", "X");     // → "X two X two X"       (all)
```

`.toString()` also works on `int`, `decimal`, and `bool` values, returning their string representation:

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

If the key does not exist, a runtime error is raised: `❌ ERROR: Key 'x' not found in dict`.

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
| `❌ ERROR: Key 'x' not found in dict` | Dict key lookup on a key that does not exist |
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
├── region.rs        — Arena allocator, ObjectRef, ObjectData, OwnedValue definitions
├── scope.rs         — ScopeStack — manages push/pop/lookup over the scoped arena
├── evaluator.rs     — Tree-walking interpreter, Flash Scope protocol, static profiler
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
- [x] `for` loop — `for (let i = 0; i < n; i = i + 1) { ... }`, nested loops, 1D/2D array traversal
- [x] Array mutation via index — `arr[i] = expr`, works in loops and from inside functions
- [x] String interpolation — `"Hello, {name}!"`, supports nested quotes inside `{…}` (e.g. `{dict["key"]}`)
- [x] Lexical closures — functions that capture variables from their defining scope
- [x] Native higher-order functions — `map`, `filter`, `reduce` with lambda syntax `x => expr` / `(x, i) => expr`
- [x] Array methods — `.push`, `.pop`, `.shift`, `.unshift`, `.sort("asc"/"desc")`, `.sort((a,b) => ...)`, `.length`
- [x] String methods — `.length`, `.substring(s,e)`, `.split(sep)`, `.replace(a,b)`, `.replaceAll(a,b)`, `.includes(sub)`, `.toString()`
- [x] Dict methods — `.toList()` (keys array), `.toArray()` (2D entries array)
- [x] `decimal` type — f64 literals (`3.14`), mixed arithmetic with `int`
- [x] Global conversions — `parseInt(val)`, `parseDecimal(val)`
- [x] Console input — `readLine(prompt?)`
- [x] Interfaces — typed record schemas: `interface Point { x: decimal, y: decimal }`, `new Point({ x:1.0, y:2.0 })`, field read/write, object patch `p = { x: 5.0 }`
- [x] Classes — C#-style OOP: `public class Foo`, constructor `public Foo(args)`, `this.field`, `public`/`private` methods, field assignment `obj.field = val`
- [x] Single inheritance — `public class Bar : Foo`, `super(args)` constructor delegation, method override, inherited method lookup
- [x] `break` / `continue` — loop control flow inside `while` and `for`
- [x] Null coalescing — `a ?? b` returns `a` if non-null, else evaluates `b`
- [x] Escape sequences — `\n`, `\t`, `\r`, `\\`, `\"`, `\{` inside string literals
- [x] Math built-ins — `abs`, `sqrt`, `floor`, `ceil`, `round`, `min`, `max`, `pow`, `log`, `log2`, `log10`

### Type system
- [x] Typed arrays — `[int]`, `[string]`, `[decimal]`, `[T?]` with element-level enforcement on `push`, `unshift`, index-assign, and construction
- [x] Type inference for function call results — `let x = add(1, 2)` infers `x: int` in the static checker
- [x] Optional / nullable types — `int?`, `string?`, `fn int? search()`, `null` literal, null equality (`== null`, `!= null`)

### Tooling
- [ ] Span-aware error diagnostics with source line preview
- [ ] Standard library (string formatting, file I/O)
- [ ] `.sz` file formatter
- [ ] LSP server for editor support

---

## License

See [LICENSE](LICENSE) for details.

---

<div align="center">

Built with ❤️ and Rust — no GC required.

</div>
