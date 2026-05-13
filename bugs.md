# Serez-Code — Bug Log

> All bugs found and fixed during interpreter development.  
> Each entry describes the symptom, root cause, fix, and affected files.

---

## Index

| ID | Short description | Severity | Status |
|----|-------------------|----------|--------|
| [B-01](#b-01--dangling-refs-when-returning-arrays-from-functions) | Dangling refs when returning arrays from functions | 🔴 Critical | ✅ |
| [B-02](#b-02--modulo-without-division-by-zero-check) | Modulo without division by zero check | 🔴 Critical | ✅ |
| [B-03](#b-03--silent-overflow-in-integer-arithmetic) | Silent overflow in integer arithmetic | 🔴 Critical | ✅ |
| [B-04](#b-04--while-condition-accumulates-temporaries-in-parent-scope) | `while` condition accumulates temporaries in parent scope | 🟡 High | ✅ |
| [B-05](#b-05--statementblock-duplicated-eval_block-with-incorrect-semantics) | `Statement::Block` duplicated `eval_block` with incorrect semantics | 🟡 High | ✅ |
| [B-06](#b-06--assign-ignored-the-regionid-of-objectref) | `assign` ignored the `RegionId` of `ObjectRef` | 🟡 High | ✅ |
| [B-07](#b-07--all-errors-went-to-stdout-instead-of-stderr) | All errors went to `stdout` instead of `stderr` | 🟡 High | ✅ |
| [B-08](#b-08--call-stack-leak-on-call-error-paths) | Call stack leak on `Call` error paths | 🟡 High | ✅ |
| [B-09](#b-09--token_precedence-incomplete-for-lteq-and-gteq) | `token_precedence` incomplete for `LtEq` and `GtEq` | 🔴 Critical | ✅ |
| [B-10](#b-10--while-propagated-body-value-as-function-return) | `while` propagated body value as function return | 🔴 Critical | ✅ |
| [B-11](#b-11--scratch-watermark-destroyed-global-side-effects) | Scratch watermark destroyed global side-effects | 🔴 Critical | ✅ |
| [B-12](#b-12--return-inside-for-produced-a-dangling-ref) | `return` inside `for` produced a dangling ref | 🔴 Critical | ✅ |
| [B-13](#b-13--typechecker-only-verified-literals-not-variables) | TypeChecker only verified literals, not variables | 🟠 Medium | ✅ |
| [B-14](#b-14--parser-without-error-recovery--cascade-of-false-positives) | Parser without error recovery — cascade of false positives | 🟠 Medium | ✅ |
| [B-15](#b-15--global-arena-grew-unboundedly-in-long-programs) | Global arena grew unboundedly in long programs | 🟠 Medium | ✅ |
| [B-16](#b-16--lexer-duplicated-input-memory-with-vecchar) | Lexer duplicated input memory with `Vec<char>` | 🟢 Low | ✅ |
| [B-17](#b-17---operator-not-lexed--fell-through-to-illegal) | `%` operator not lexed — fell through to `Illegal` | 🔴 Critical | ✅ |
| [B-18](#b-18--let-x--arri-aliased-the-array-element-slot) | `let x = arr[i]` aliased the array element slot | 🔴 Critical | ✅ |

---

## B-01 — Dangling refs when returning arrays from functions

**Date:** 2026-05-12  
**Files:** `src/evaluator.rs`  
**Severity:** 🔴 Critical

### Symptom

Any function returning an array produced corrupted or silently incorrect data when reading elements of the returned array:

```serez
fn make_arr() {
    return [7, 8, 9];
}
let result = make_arr();
out result[0];  // → random value / 0 instead of 7
```

### Root cause

`ObjectData::Array(Vec<ObjectRef>)` stores elements as references (`ObjectRef`) to arena slots. When returning the array from the function, the evaluator copied the `Vec<ObjectRef>`, but the refs pointed to slots in the function's **scoped arena**. When `scopes.pop()` destroyed that scope, `arena.reset_to(watermark)` freed those slots. The refs became **dangling**: they were still valid numeric indices, but now pointed to another variable's data or simply garbage.

The problem did not appear with primitives (`Integer`, `Boolean`, `Str`) because those are returned by value; the slot simply stops existing but the data was already copied. For arrays, the child refs were never promoted.

### Fix

The type `OwnedValue` was introduced — a value tree representation completely independent of any arena:

```rust
enum OwnedValue {
    Integer(i64),
    Boolean(bool),
    Str(String),
    Array(Vec<OwnedValue>),
    Dict { key_type: String, value_type: String, entries: Vec<(OwnedValue, OwnedValue)> },
    Null,
}
```

And two methods on the evaluator:

- **`extract(obj_ref) → OwnedValue`** — materializes the full tree **before** the `pop()`, recursively following all refs
- **`plant(owned) → ObjectRef`** — re-allocates the tree in the parent scope **after** the `pop()`

The "promote before pop" pattern is applied at all scope exit points: `eval_block`, `eval_call`, and `Statement::For`.

---

## B-02 — Modulo without division by zero check

**Date:** 2026-05-12  
**Files:** `src/evaluator.rs`  
**Severity:** 🔴 Critical

### Symptom

`n % 0` caused a **panic** in Rust (`attempt to calculate the remainder with a divisor of zero`), stopping the entire process instead of producing a controlled language error.

### Root cause

The original implementation was:

```rust
"%" => ObjectData::Integer(l % r),
```

Rust panics on `i64 % 0` in debug mode; in release the behavior is undefined.

### Fix

```rust
"%" => {
    if r == 0 {
        eprintln!("❌ ERROR: Modulus operator by zero");
        return EvalResult::Error;
    }
    match l.checked_rem(r) {
        Some(v) => ObjectData::Integer(v),
        None => {
            eprintln!("❌ ERROR: Integer overflow");
            return EvalResult::Error;
        }
    }
}
```

---

## B-03 — Silent overflow in integer arithmetic

**Date:** 2026-05-12  
**Files:** `src/evaluator.rs`  
**Severity:** 🔴 Critical

### Symptom

Operations like `i64::MAX + 1` silently produced the minimum negative value (wrapping) in release, or a panic in debug. The program continued with incorrect data.

### Root cause

All arithmetic operators used Rust's native operators (`+`, `-`, `*`, `/`) which wrap in release mode.

### Fix

All arithmetic operators migrated to their `checked_*` versions:

```rust
"+" => match l.checked_add(r) {
    Some(v) => ObjectData::Integer(v),
    None => { eprintln!("❌ ERROR: Integer overflow"); return EvalResult::Error; }
},
// same for -, *, /, %
```

---

## B-04 — `while` condition accumulates temporaries in parent scope

**Date:** 2026-05-12  
**Files:** `src/evaluator.rs`  
**Severity:** 🟡 High

### Symptom

A `while` with N iterations allocated N booleans in the active scope (global arena or scoped arena) without freeing them. In long loops this caused unnecessary linear growth of the arena.

### Root cause

Evaluating the condition (`eval_expression(&while_stmt.condition)`) allocated the resulting `Boolean` in the parent scope without a watermark protection. It was not freed between iterations.

### Fix

A watermark is taken **before** evaluating the condition, the data is extracted to the Rust stack, and the watermark is immediately restored — freeing the temporary before entering the body:

```rust
let cond_mark = if !self.scopes.is_empty() {
    Some(self.scopes.arena.watermark())
} else { None };

let cond_ref = match self.eval_expression(&while_stmt.condition) { ... };
let condition_data = self.resolve(cond_ref).unwrap().clone();

if let Some(mark) = cond_mark {
    self.scopes.arena.reset_to(mark);
}
```

---

## B-05 — `Statement::Block` duplicated `eval_block` with incorrect semantics

**Date:** 2026-05-12  
**Files:** `src/evaluator.rs`  
**Severity:** 🟡 High

### Symptom

A `return` inside a nested `{ }` block within a function did not propagate the return correctly — the function continued executing statements after the block.

### Root cause

`Statement::Block` contained a ~35-line copy of `eval_block`'s code, but treated `EvalResult::Return` as an error instead of propagating it. This meant any `return` captured by a nested block was silently swallowed.

### Fix

`Statement::Block` delegates directly to `eval_block`:

```rust
Statement::Block(block) => self.eval_block(block),
```

`eval_block` already handles `Return` propagation correctly.

---

## B-06 — `assign` ignored the `RegionId` of `ObjectRef`

**Date:** 2026-05-12  
**Files:** `src/scope.rs`  
**Severity:** 🟡 High

### Symptom

`scopes.assign("x", new_data)` always updated `self.arena` (the scoped arena), without checking whether the found `ObjectRef` had `region = RegionId::Global`. This could cause writes to the wrong offset if a scoped and a global binding happened to share the same index.

### Root cause

The method looked up the binding in the frames, got the ref, and called `self.arena.update(r.index, new_data)` without checking `r.region`.

### Fix

A `debug_assert` was added to catch the inconsistency in development builds before it produces a silent bug in production:

```rust
debug_assert_eq!(
    r.region, RegionId::Scoped,
    "assign() found a Global ref in a scope frame — arena mismatch"
);
```

---

## B-07 — All errors went to `stdout` instead of `stderr`

**Date:** 2026-05-12  
**Files:** `src/evaluator.rs`, `src/parser.rs`, `src/main.rs`  
**Severity:** 🟡 High

### Symptom

```bash
sz script.sz > output.txt
# output.txt contained both program output and error messages
```

The `❌ ERROR: ...` messages mixed with normal program output when redirecting `stdout`.

### Root cause

~30 occurrences of `println!("❌ ...")` instead of `eprintln!`.

### Fix

Mechanical conversion of all error messages to `eprintln!`. Legitimate program output (`out` statement, `check_program`) remains on `stdout`.

---

## B-08 — Call stack leak on `Call` error paths

**Date:** 2026-05-12  
**Files:** `src/evaluator.rs`  
**Severity:** 🟡 High

### Symptom

In the REPL, after any error in a function call (wrong argument type, wrong arity, etc.), subsequent calls showed error messages with the previous call's stack, producing incorrect traces and accumulating frames indefinitely.

### Root cause

Five `return EvalResult::Error` paths inside the `Expression::Call` evaluator did not call `call_stack.pop()`, and some also skipped `scopes.pop()`:

| Path | `scopes.pop()` | `call_stack.pop()` |
|------|---------------|-------------------|
| Call on non-function | ❌ missing | ❌ missing |
| Error evaluating argument | ❌ missing | ❌ missing |
| Wrong arity | ❌ missing | ❌ missing |
| Wrong parameter type | ❌ missing | ❌ missing |
| Error in body | ✅ present | ❌ missing |

### Fix

Added `self.scopes.pop()` and `self.call_stack.pop()` on each error path before `return EvalResult::Error`.

---

## B-09 — `token_precedence` incomplete for `LtEq` and `GtEq`

**Date:** 2026-05-12  
**Files:** `src/parser.rs`  
**Severity:** 🔴 Critical

### Symptom

```serez
fn bool isAdult(int age) {
    return age >= 18;  // ← caused TYPE ERROR
}
out isAdult(20);
// ❌ TYPE ERROR: Function expected to return 'bool' but returned another type.
```

### Root cause

`LtEq` and `GtEq` were added to the parser's `is_infix` list but **not** to `token_precedence`. Having `Lowest` precedence (the default `_`), the Pratt algorithm never entered the infix loop:

```
while precedence < self.peek_precedence()
      Lowest   <   Lowest                 → false → loop does not execute
```

The parser returned only the left-hand identifier (`age`), the expression `>= 18` was left unparsed, and the function returned `Null` instead of `Boolean`. The type checker detected that `Null ≠ bool`.

### Fix

```rust
// Before
TokenType::Lt | TokenType::Gt => Precedence::LessGreater,

// After
TokenType::Lt | TokenType::Gt | TokenType::LtEq | TokenType::GtEq => Precedence::LessGreater,
```

**Lesson:** In a Pratt parser there are **two places** where an infix operator must be registered — `token_precedence` (so the loop knows when to continue) and the `is_infix` list (so the parser accepts it as infix). Missing either one produces silently incorrect behavior.

---

## B-10 — `while` propagated body value as function return

**Date:** 2026-05-12  
**Files:** `src/evaluator.rs`  
**Severity:** 🔴 Critical

### Symptom

Any `void` function whose **last statement** was a `while` raised:

```
❌ TYPE ERROR: Function expected to return 'void' but returned another type.
```

even though the function had no explicit `return`.

```serez
fn void count(int n) {
    while (n > 0) {
        n = n - 1;
    }
}
count(5);  // ← TYPE ERROR
```

### Root cause

`Statement::While` saved and returned the last value produced by the body on each iteration:

```rust
let mut result = EvalResult::Value(self.null_ref);
// ...
EvalResult::Value(v) => result = EvalResult::Value(v),
// ...
result  // ← returned Integer(N) if the body ended with an assignment
```

When the function reached end-of-body without a `return`, the `Call` evaluator used that `Integer` as the function's return value. The type checker compared it to `"void"` and failed.

### Fix

`Statement::While` discards the body value and always returns `null_ref`, just like `Statement::For`:

```rust
EvalResult::Value(_) => {}  // discarded — while is a statement, not an expression
// ...
EvalResult::Value(self.null_ref)  // always returns null
```

---

## B-11 — Scratch watermark destroyed global side-effects

**Date:** 2026-05-12  
**Files:** `src/evaluator.rs`  
**Severity:** 🔴 Critical

### Symptom

```serez
let data = [10, 20, 30];
fn void doubleAt(int idx) {
    data[idx] = data[idx] * 2;
}
doubleAt(1);
out data[1];  // → 20 instead of 40
```

Mutation of a global array from within a function was silently reverted.

### Root cause

`eval_program` applied a scratch watermark to both `Statement::Out` and `Statement::Expression` to free global temporaries:

```rust
let scratch_mark = match statement {
    Statement::Out(_) | Statement::Expression(_) => Some(self.global_arena.watermark()),
    _ => None,
};
```

The watermark was taken **before** evaluating the statement. During the call to `doubleAt`, `Statement::IndexAssign` used `plant_global(Integer(40))` → allocated at `global_arena[slot_N]`. After the expression statement finished, `global_arena.reset_to(mark)` freed that slot. The next read `out data[1]` found garbage or a different value at that index.

### Fix

The scratch watermark is applied only to `Statement::Out`, which by definition produces no persistent side-effects in the arena:

```rust
let scratch_mark = match statement {
    Statement::Out(_) => Some(self.global_arena.watermark()),
    _ => None,
};
```

`Statement::Expression` (function calls with side-effects) is excluded from the reset.

---

## B-12 — `return` inside `for` produced a dangling ref

**Date:** 2026-05-12  
**Files:** `src/evaluator.rs`  
**Severity:** 🔴 Critical

### Symptom

```serez
fn int search(int target) {
    for (let i = 0; i < 5; i = i + 1) {
        if (arr[i] == target) { return i; }
    }
    return -1;
}
// ❌ TYPE ERROR: Function expected to return 'int' but returned another type.
```

### Root cause

The `for` loop calls `scopes.push()` to create its own scope for the `init` variable. When `eval_block` of the body returns `EvalResult::Return(v)`, `v` is a ref allocated in the for-scope (promoted to the for-scope by `eval_block`). The original code propagated that ref directly and then called `scopes.pop()` — destroying the for-scope and turning `v` into a dangling ref. The type checker read garbage and reported an incorrect type.

### Fix

The same "promote before pop" pattern from `eval_block`: extract the value **before** the pop, pop, and re-plant in the parent scope:

```rust
EvalResult::Return(v) => {
    loop_return = Some(self.extract(v));  // extract while for-scope is still alive
    break;
}
// ...
self.scopes.pop();
if let Some(owned) = loop_return {
    return EvalResult::Return(self.plant(owned));  // plant in parent scope
}
```

---

## B-13 — TypeChecker only verified literals, not variables

**Date:** 2026-05-12  
**Files:** `src/type_checker.rs`  
**Severity:** 🟠 Medium

### Symptom

```serez
let age = 20;
fn bool adult(int a) { return a >= 18; }
adult(age);  // TypeChecker didn't detect that age is int → no validation
```

The static TypeChecker could only verify the type of literal arguments (`adult(20)`). When the argument was an identifier, it accepted it without checking.

### Root cause

`check_call` only handled `Expression::Integer`, `Expression::String`, `Expression::Boolean` to determine the argument type. `Expression::Identifier` fell to the default without verification.

### Fix

`var_types: HashMap<String, String>` was added to the TypeChecker struct. During analysis of `Statement::Let`, if the RHS is a literal, the variable's type is inferred and recorded. In `check_call`, when the argument is an `Identifier`, its inferred type is looked up in `var_types`.

---

## B-14 — Parser without error recovery — cascade of false positives

**Date:** 2026-05-12  
**Files:** `src/parser.rs`  
**Severity:** 🟠 Medium

### Symptom

A syntax error on line 5 of a program produced dozens of false errors on lines 6–50, because the parser became desynchronized reading tokens in incorrect contexts.

### Root cause

When `parse_statement` returned `None` (parse error), `parse_program` advanced only one token (`self.next_token()`) and tried to parse the next statement from an unknown state.

### Fix

`synchronize()` was implemented — advances to the next synchronization token without consuming it:

```rust
fn synchronize(&mut self) {
    while self.current_token.token_type != TokenType::Eof {
        match self.current_token.token_type {
            TokenType::Semicolon | TokenType::RBrace => return,
            TokenType::Let | TokenType::Return | TokenType::Out
            | TokenType::Function | TokenType::While | TokenType::For => return,
            _ => self.next_token(),
        }
    }
}
```

`parse_program` calls `synchronize()` instead of `next_token()` when `parse_statement` fails.

---

## B-15 — Global arena grew unboundedly in long programs

**Date:** 2026-05-12  
**Files:** `src/evaluator.rs`  
**Severity:** 🟠 Medium

### Symptom

Programs with many `out` statements at global level accumulated booleans/integers/strings in the global arena indefinitely, even though none of those values were referenced after being printed.

### Root cause

`out expr` allocated the result of `expr` in the global arena and printed it. That slot was never freed because the global arena has no GC mechanism.

### Fix

`Out` statements in `eval_program` use a scratch watermark: the mark is taken before evaluating the expression, the value is printed, and the mark is immediately restored:

```rust
let scratch_mark = match statement {
    Statement::Out(_) => Some(self.global_arena.watermark()),
    _ => None,
};
// ... eval and print ...
if let Some(mark) = scratch_mark {
    self.global_arena.reset_to(mark);
}
```

---

## B-16 — Lexer duplicated input memory with `Vec<char>`

**Date:** 2026-05-12  
**Files:** `src/lexer.rs`  
**Severity:** 🟢 Low

### Symptom

The lexer consumed twice the source memory: the original `String` plus a copy as `Vec<char>`. For large files this was unnecessary.

### Root cause

```rust
pub struct Lexer {
    input: Vec<char>,  // ← O(n) copy of the input as individual chars
    ...
}
```

### Fix

Lexer rewritten to operate directly on the original `String` with byte offsets:

| Before | After |
|--------|-------|
| `input: Vec<char>` | `input: String` |
| char indices | byte offsets |
| `self.input[i]` | `self.input[i..].chars().next()` |
| `self.input[a..b].iter().collect()` | `self.input[a..b].to_string()` |

`read_char` advances `read_position` by `c.len_utf8()` bytes instead of 1, correctly handling multibyte Unicode.

---

## B-17 — `%` operator not lexed — fell through to `Illegal`

**Date:** 2026-05-12  
**Files:** `src/token.rs`, `src/lexer.rs`, `src/parser.rs`  
**Severity:** 🔴 Critical

### Symptom

```serez
fn bool is_even(int n) { return n % 2 == 0; }
out is_even(4);
// ❌ TYPE ERROR: Function expected to return 'bool' but returned another type.
```

The `%` operator was completely ignored. `n % 2 == 0` was parsed as just `n`.

### Root cause

Three simultaneous omissions in the pipeline:

1. **`token.rs`** — no `Percent` variant existed in `TokenType`
2. **`lexer.rs`** — no `'%'` case in `next_token()`, so `%` fell to the `_` arm and was emitted as `TokenType::Illegal` with literal `"%"`
3. **`parser.rs`** — `TokenType::Percent` (which didn't exist) was not in the `is_infix` list, so even if the token had arrived with the correct precedence, the parser would have ignored the infix loop

Result: the parser saw `n` (Identifier), then `%` (Illegal, Lowest precedence). `Lowest < Lowest = false` → the infix loop did not execute. The return statement returned only `n` (Integer), which did not match the return type `"bool"`.

### Fix

Three coordinated changes:

```rust
// token.rs — add variant
Percent, // %

// lexer.rs — add case
'%' => Token::new(TokenType::Percent, self.ch.to_string(), self.line, self.column),

// parser.rs — token_precedence
TokenType::Slash | TokenType::Asterisk | TokenType::Percent => Precedence::Product,

// parser.rs — is_infix
| TokenType::Percent
```

**Lesson reinforced from B-09:** Infix operators must be registered in **four places**: `TokenType` enum, `lexer next_token`, `token_precedence`, and `is_infix`. Missing any one of them causes the operator to be silently discarded.

---

## B-18 — `let x = arr[i]` aliased the array element slot

**Date:** 2026-05-12  
**Files:** `src/evaluator.rs`  
**Severity:** 🔴 Critical

### Symptom

```serez
let grades = [85, 92, 78, 65, 90, 55, 88, 95, 72, 83];
let max_grade = grades[0];   // initialize to first element
let min_grade = grades[0];   // same

for (let i = 0; i < 10; i = i + 1) {
    max_grade = max(max_grade, grades[i]);
    min_grade = min(min_grade, grades[i]);
}

out max_grade;  // → 83 (wrong, should be 95)
out min_grade;  // → 83 (wrong, should be 55)
```

Both variables ended up with the value of the last array element, regardless of which was the greatest or smallest.

### Root cause

`eval_statement(Let)` stored the `ObjectRef` returned by `eval_expression` directly into the bindings, without creating a new slot:

```rust
let val_ref = match self.eval_expression(&let_stmt.value) { ... };
// val_ref points to the SAME slot as grades[0] in global_arena
self.global_bindings.insert("max_grade", val_ref);
```

For `let max_grade = grades[0]`:
- `eval_expression(Index(grades, 0))` returns the `ObjectRef` of array element 0 — e.g., `ObjectRef { region: Global, index: 5 }`
- That same `ObjectRef{Global, 5}` ends up in `global_bindings["max_grade"]` **and** inside the `Vec<ObjectRef>` of the `grades` array

Both variables (`max_grade`, `min_grade`) and `grades[0]` pointed to exactly the same slot of the global arena: `global_arena[5]`.

When `max_grade = max(max_grade, grades[i])` executed the `Statement::Assign`:

```rust
self.global_arena.update(existing_ref.index, new_data);
// existing_ref.index == 5 → mutated global_arena[5]
// → grades[0] also saw the new value from that point on
```

After the last iteration (i=9, `grades[9]=83`):
- `max_grade = max(95, 83)` → should be 95, was → `global_arena[5] = 95`
- `min_grade = min(95, 83)` → but `min_grade` is also `global_arena[5]`, now 95 → `min(95, 83) = 83` → `global_arena[5] = 83`

Result: both ended at 83, the value of the last write to the shared slot.

### Fix

`eval_statement(Let)` now always allocates a **fresh slot** for the variable, regardless of where the value came from:

```rust
Statement::Let(let_stmt) => {
    let val_ref = match self.eval_expression(&let_stmt.value) { ... };

    // Fresh slot to prevent aliasing (e.g. `let x = arr[i]`)
    let fresh_data = self.resolve(val_ref).unwrap().clone();
    let val_ref = self.alloc(fresh_data);

    if self.scopes.is_empty() {
        self.global_bindings.insert(let_stmt.name.clone(), val_ref);
    } else {
        self.scopes.declare(let_stmt.name.clone(), val_ref);
    }
    ...
}
```

`self.resolve(val_ref).unwrap().clone()` copies the `ObjectData` to the Rust stack. `self.alloc(fresh_data)` allocates it in a new, separate slot. The variable never shares an arena index with any other source.

**Note:** For `ObjectData::Array`, `.clone()` copies the `Vec<ObjectRef>` but not the referenced elements (shallow clone). This is intentional: an array assigned to a new variable still references the same elements, but the container itself (the Vec) is independent — which is sufficient to prevent aliasing between the variable and the source array.

---

*Last updated: 2026-05-12 — 18 bugs documented, all fixed.*
