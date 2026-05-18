# Serez-Code — Bug Log

> All bugs found during interpreter development.  
> Each entry describes the symptom, root cause, fix (if applied), and affected files.  
> Status: ✅ Fixed · 🔲 Open

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
| [B-19](#b-19--splitproduced-empty-strings-at-boundaries) | `split("")` produced empty strings at boundaries | 🟡 High | ✅ |
| [B-20](#b-20--tostring-only-worked-on-string-type) | `.toString()` only worked on `string` type | 🟡 High | ✅ |
| [B-21](#b-21--decimal-display-imprecision-due-to-f64-binary-representation) | Decimal display imprecision due to `f64` binary representation | 🟡 High | ✅ |
| [B-22](#b-22--push-inside-while-body-loses-elements-on-scoped-array) | `push` inside `while` body loses elements on scoped array | 🔴 Critical | ✅ |
| [B-23](#b-23--sort-returned-null-instead-of-the-array) | `.sort` returned `null` instead of the array | 🟡 High | ✅ |
| [B-24](#b-24--infinite-recursion-crashes-the-process-with-no-controlled-error) | Infinite recursion crashes the process with no controlled error | 🔴 Critical | ✅ |
| [B-25](#b-25--duplicate-type-error-typechecker-and-evaluator-both-fire) | Duplicate type error — TypeChecker and Evaluator both fire | 🟠 Medium | ✅ |
| [B-26](#b-26--dict-kv-indexassign-does-not-validate-value-type) | Dict `<K,V>` `IndexAssign` does not validate value type | 🟡 High | ✅ |
| [B-27](#b-27--closures-capture-environment-by-copy--mutations-do-not-persist-across-calls) | Closures capture environment by copy — mutations do not persist across calls | 🟠 Medium | ✅ |
| [B-28](#b-28--this-fieldiidx--val-silently-does-nothing-when-target-is-not-a-plain-identifier) | `this.field[idx] = val` silently does nothing inside class methods | 🔴 Critical | ✅ |
| [B-29](#b-29--array-return-type-int-not-parsed-in-class-method-signatures) | Array return type `[int]` not parsed in class method signatures | 🔴 Critical | ✅ |
| [B-30](#b-30--pop-and-shift-on-empty-array-errors-instead-of-returning-null) | `.pop()` and `.shift()` on empty array error instead of returning `null` | 🟡 High | ✅ |
| [B-31](#b-31--dict-missing-key-access-errors-instead-of-returning-null) | Dict missing-key access errors instead of returning `null` | 🟡 High | ✅ |
| [B-32](#b-32--sort-shift-unshift-on-instance-field-array-do-not-write-back) | `.sort()`, `.shift()`, `.unshift()` on instance field array do not write back | 🔴 Critical | ✅ |
| [B-33](#b-33--interface-field-array-type-int-fails-to-parse) | Interface field with array type `[int]` fails to parse | 🔴 Critical | ✅ |
| [B-34](#b-34--thisnfield_holding_a_function-is-not-callable-with-args) | `this.fn_field(args)` — calling a function stored in an instance field fails | 🟡 High | ✅ |
| [B-35](#b-35--for-loop-init-does-not-allocate-a-fresh-slot-aliasing-risk) | `for` loop init does not allocate a fresh slot — aliasing risk | 🔴 Critical | ✅ |
| [B-36](#b-36--prefix---on-i64min-produces-overflow-panic) | Prefix `-` on `i64::MIN` produces overflow panic | 🔴 Critical | ✅ |
| [B-37](#b-37--sort-evaluates-its-argument-argument-up-to-3-times) | `sort` evaluates its argument up to 3 times — redundant allocations | 🟠 Medium | ✅ |
| [B-38](#b-38--eprint-instead-of-eprintln-in-type-mismatch-error-corrupts-output-format) | `eprint!` instead of `eprintln!` corrupts error output format | 🟢 Low | ✅ |
| [B-39](#b-39--str--decimal-concatenation-uses-inconsistent-float-formatting) | `Str + Decimal` concatenation inconsistent with `display()` | 🟢 Low | ✅ |
| [B-40](#b-40--method-calls-on-class-instances-omit-call_stack-frame) | Method calls on class instances omit `call_stack` frame — incomplete error traces | 🟡 High | ✅ |
| [B-41](#b-41--remove-listed-in-mutating-but-not-implemented-for-arrays) | `remove` listed in MUTATING but not implemented for arrays | 🟡 High | ✅ |
| [B-42](#b-42--seven-common-string-methods-missing) | Seven common string methods missing: `trim`, `toUpperCase`, `toLowerCase`, `startsWith`, `endsWith`, `indexOf`, `charAt` | 🟠 Medium | ✅ |
| [B-43](#b-43--ternary-operator-parses-chained-expressions-left-associatively) | Ternary operator parses chained `?:` right branch left-associatively | 🔴 Critical | ✅ |
| [B-44](#b-44--supermethodargs-fails-in-non-constructor-child-class-methods) | `super.method(args)` fails in non-constructor child class methods | 🔴 Critical | ✅ |

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

---

## B-19 — `split("")` produced empty strings at boundaries

**Date:** 2026-05-13  
**Files:** `src/evaluator.rs`  
**Severity:** 🟡 High

### Symptom

```serez
out "abc".split("");
// → [, a, b, c, ]   // 5 elements — first and last are empty strings
```

Splitting on an empty separator should produce one element per character: `[a, b, c]`.

### Root cause

The implementation called Rust's `str::split("")` directly. Rust's `split("")` inserts empty-string matches at the start and end of the input (before the first character and after the last), yielding `n + 1` results for a string of length `n`.

```rust
// Before — produces ["", "a", "b", "c", ""]
s.split(&sep[..]).map(|part| self.alloc(ObjectData::Str(part.to_string()))).collect()
```

### Fix

Special-case an empty separator to use `str::chars()` instead, which yields exactly one `char` per Unicode character with no boundary artifacts:

```rust
if sep.is_empty() {
    s.chars().map(|c| self.alloc(ObjectData::Str(c.to_string()))).collect()
} else {
    s.split(&sep[..]).map(|part| self.alloc(ObjectData::Str(part.to_string()))).collect()
}
```

---

## B-20 — `.toString()` only worked on `string` type

**Date:** 2026-05-13  
**Files:** `src/evaluator.rs`  
**Severity:** 🟡 High

### Symptom

```serez
out 42.toString();     // ❌ ERROR: Unknown method 'toString' on type 'int'
out 3.14.toString();   // ❌ ERROR: Unknown method 'toString' on type 'decimal'
out true.toString();   // ❌ ERROR: Unknown method 'toString' on type 'bool'
```

`.toString()` was documented as a universal conversion but only worked on `string` values (where it was an identity).

### Root cause

The `DotCall` evaluator dispatched array methods for `Array`, string methods for `Str`, and dict methods for `Dict`. All other types fell directly to the error arm:

```rust
_ => {
    eprintln!("❌ ERROR: Unknown method '{}' on type '{}'", dot_call.method, type_name);
    EvalResult::Error
}
```

There was no general fallback for `.toString()` on other types.

### Fix

A wildcard arm for `.toString()` was added before the error fallback. It calls `self.display(obj_ref)` — the same function already used by `out` — to produce the string representation:

```rust
_ if dot_call.method == "toString" => {
    let s = self.display(obj_ref);
    EvalResult::Value(self.alloc(ObjectData::Str(s)))
}
_ => {
    eprintln!("❌ ERROR: Unknown method '{}' on type '{}'", dot_call.method, ...);
    EvalResult::Error
}
```

---

## B-21 — Decimal display imprecision due to `f64` binary representation

**Date:** 2026-05-13  
**Files:** `src/evaluator.rs`  
**Severity:** 🟡 High

### Symptom

```serez
let prices = [9.99, 3.50, 14.99, 7.25];
prices.sort();
let total = prices.reduce(0.0, (acc, p) => acc + p);
out total;
// → 35.730000000000004   // expected: 35.73
```

Decimal values produced by arithmetic operations displayed with spurious low-order digits.

### Root cause

`f64` represents decimal fractions in binary, and most decimal fractions cannot be represented exactly. `3.5 + 7.25 + 9.99 + 14.99` accumulates representational error in the least-significant bits. Using Rust's default `{}` or `{:.2}` formatting exposes this error:

```rust
format!("{}", 35.73_f64 + f64::EPSILON)  // → "35.730000000000004"
```

### Fix

Format to 10 significant decimal places and then trim trailing zeros and the decimal point:

```rust
Some(ObjectData::Decimal(d)) => {
    if d.fract() == 0.0 {
        format!("{:.1}", d)   // "5.0" for integer-valued decimals
    } else {
        let s = format!("{:.10}", d);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
```

This rounds the display at 10 decimal places, which eliminates all practical f64 noise while preserving genuine precision up to 10 decimal digits. Values like `35.730000000000004` display as `35.73`; values like `3.14159` display as `3.14159`.

---

---

## B-22 — `push` inside `while` body loses elements on scoped array

**Date:** 2026-05-14  
**Files:** `src/evaluator.rs`, `src/scope.rs`  
**Severity:** 🔴 Critical

### Symptom

A function-local typed array built up with `push` inside a `while` loop would lose elements or crash with "Index out of bounds":

```serez
fn [int] lowStock(int threshold) {
    let indices [int] = [];
    let i = 0;
    while (i < stock.length()) {
        if (stock[i] <= threshold) {
            indices.push(i);   // only first push survives
        }
        i = i + 1;
    }
    return indices;
}

let alerts = lowStock(3);
out alerts.length();  // → 1 (expected 2)
// accessing alerts[1] → ❌ ERROR: Index out of bounds
```

### Root cause

`eval_block` calls `scopes.push()` at the start of every `{ ... }` block and `scopes.pop()` at the end. `scopes.pop()` calls `arena.reset_to(frame.watermark)`, which truncates the scoped arena back to the mark taken at the start of that block.

When `push` is called on a scoped array (one declared inside a function) from within the `while` body, the new element is allocated via `self.plant(val)`, which lands in the scoped arena **at the current watermark** — inside the while body's ephemeral range. When the while body's `eval_block` pops, that watermark is reset, freeing the newly allocated element ref. The array object still holds the now-freed index, causing dangling refs.

```
Scoped arena state after first push:
  [indices_array @ 0][element_0 @ 1]   ← watermark at start of while body was 1
While body pops → reset_to(1) → element_0 freed
Array still contains ObjectRef{Scoped, index: 1} → dangling
```

### Fix

Added `pub fn depth(&self) -> usize` to `ScopeStack` (returns `self.frames.len()`).

In `eval_array_method`, for `push` and `unshift`: when the array lives in the scoped arena but we're currently inside a nested scope (`scopes.depth() > 1`), allocate the new element via `plant_global` instead of `plant`. This ensures the element outlives the inner block's scope pop:

```rust
let new_ref = match arr_ref.region {
    RegionId::Global => self.plant_global(val),
    RegionId::Scoped if self.scopes.depth() > 1 => self.plant_global(val),  // ← fix
    RegionId::Scoped => self.plant(val),
};
```

The same fix was applied to `IndexAssign` for scoped arrays inside nested scopes.

**Trade-off:** Elements of scoped arrays that are mutated from nested scopes now live in the global arena. They are logically owned by the function scope but physically in global arena — a bounded "leak" for the duration of program execution. For an interpreter this is acceptable; these elements are not reachable after the function returns.

---

## B-23 — `.sort` returned `null` instead of the array

**Date:** 2026-05-14  
**Files:** `src/evaluator.rs`  
**Severity:** 🟡 High

### Symptom

Assigning the result of `.sort()` produced `null`, making chaining impossible:

```serez
let sorted = nums.sort((a, b) => b - a);
out sorted.length();  // ❌ ERROR: '.' method call not supported for type 'null'
```

### Root cause

The `sort` branch in `eval_array_method` ended with `EvalResult::Value(self.null_ref)`. While it correctly sorted the array in-place via `update_array`, it discarded the array ref.

### Fix

Changed the return to `EvalResult::Value(arr_ref)` — returns the same (now sorted) array reference, consistent with JavaScript's `Array.prototype.sort` semantics.

Also added comparator lambda support: when the argument to `.sort` is a function (detected via `resolve` before consuming the argument), a bubble-sort loop calls `call_function(cb, [a, b])` for each comparison. Positive result → swap.

```rust
// Before
self.update_array(arr_ref, element_type, new_refs);
EvalResult::Value(self.null_ref)

// After
self.update_array(arr_ref, element_type, new_refs);
EvalResult::Value(arr_ref)
```

---

---

## B-24 — Infinite recursion crashes the process with no controlled error

**Date:** 2026-05-16  
**Files:** `src/evaluator.rs`, `src/main.rs`  
**Severity:** 🔴 Critical  
**Status:** ✅ Fixed

### Symptom

A function that recurses without a base case causes the OS thread to overflow its stack. The process prints a low-level Rust diagnostic and terminates abruptly — no `❌` message is produced by the interpreter:

```sz
fn int inf(int n) { return inf(n + 1); }
inf(0);
// → thread '<unknown>' has overflowed its stack
// → (process exits with non-zero code, no user-facing error)
```

### Root cause

The evaluator uses native Rust call frames for each function invocation (`eval_expression` calls itself recursively for each `Call` expression). There is no interpreter-level depth counter. The 64 MB thread stack allocated in `main.rs` buys several hundred thousand frames before the OS kills the thread — but the termination is always abrupt.

```rust
// main.rs:77 — stack is large but has no depth limit in the interpreter
let builder = std::thread::Builder::new().stack_size(64 * 1024 * 1024);
```

### Fix

Add a `call_depth: usize` counter to `Evaluator`. Increment it on every function call entry and decrement on exit. If the counter exceeds a configurable limit (e.g. 1000) before allocating a new call frame, return a controlled error:

```rust
// evaluator.rs — Evaluator struct
call_depth: usize,

// In eval_expression, Expression::Call, before scopes.push():
if self.call_depth >= 1000 {
    eprintln!("❌ ERROR: Stack overflow — maximum call depth (1000) exceeded");
    return EvalResult::Error;
}
self.call_depth += 1;

// Before all return points in the Call arm:
self.call_depth -= 1;
```

The same counter must also be applied to method dispatch in `eval_dot_call` for class instances (the `find_method` / body-execution path).

---

## B-25 — Duplicate type error — TypeChecker and Evaluator both fire

**Date:** 2026-05-16  
**Files:** `src/evaluator.rs`, `src/type_checker.rs`  
**Severity:** 🟠 Medium  
**Status:** ✅ Fixed

### Symptom

When a function is called with an argument of the wrong type, **two** error messages are emitted for a single mistake — one from the TypeChecker pass (good, includes the actual type) and one from the runtime Evaluator (worse, says "another type"):

```sz
fn int suma(int a, int b) { return a + b; }
suma("hola", 2);
// ❌ TYPE ERROR [line 2:5]: Parameter 'a' of 'suma' expected 'int' but received 'string'.
// ❌ TYPE ERROR: Parameter 'a' expected 'int' but received another type.
//     called from 'suma' [line 2:5]
```

The first message is from the TypeChecker. The second, redundant and inferior, is from `eval_expression` at runtime. The user sees the same error twice in degraded form.

### Root cause

Both passes are independent and have no shared state. The TypeChecker runs first (producing message 1), but the Evaluator re-performs the same check unconditionally at call time (producing message 2). The runtime check also fails to include the actual received type:

```rust
// evaluator.rs:900-905
eprintln!(
    "❌ TYPE ERROR: Parameter '{}' expected '{}' but received another type.",
    //                                                      ^^^^^^^^^^^
    //                          actual type is available via actual_data.type_name()
    //                          but is not used
    param.name, expected_type
);
```

### Fix

Two independent improvements, either or both can be applied:

**Option A — Include the actual type in the runtime message:**

```rust
eprintln!(
    "❌ TYPE ERROR: Parameter '{}' expected '{}' but received '{}'.",
    param.name, expected_type, actual_data.type_name()
);
```

**Option B — Skip the runtime check when the TypeChecker already caught the error:**  
This requires the TypeChecker to record which call sites it already diagnosed, or the Evaluator to honour a `--skip-runtime-type-checks` flag when `--check` was run first. More invasive; Option A is sufficient.

---

## B-26 — Dict `<K,V>` `IndexAssign` does not validate value type

**Date:** 2026-05-16  
**Files:** `src/evaluator.rs`  
**Severity:** 🟡 High  
**Status:** ✅ Fixed

### Symptom

A value of the wrong type can be silently stored in a typed dictionary. No error is produced:

```sz
let d <string,int> = ({"a", 1}, {"b", 2});
d["a"] = "not_an_int";   // no ❌ ERROR
out d["a"];              // → not_an_int
```

The dict now holds a `string` where it declared `int`, violating the type contract invisibly.

### Root cause

The `IndexAssign` handler in `eval_statement` branches on the target's runtime type. The `Array` branch validates the element against `element_type` before writing. The `Dict` branch performs no equivalent check on `value_type`:

```rust
// evaluator.rs — IndexAssign, Dict branch (~line 599)
ObjectData::Dict { key_type, value_type, mut entries } => {
    let search_key = obj_data_to_key_str(&idx_data);
    let owned_val = self.extract(val_ref);
    // ← no type validation of owned_val against value_type
    ...
}
```

Compare with the Array branch, which correctly rejects mismatched types:

```rust
ObjectData::Array { element_type, mut elements } => {
    if let Some(ref et) = element_type {
        let val_data = self.resolve(val_ref).unwrap();
        if !type_matches(et, val_data) {
            eprintln!("❌ TYPE ERROR: Cannot assign '{}' to [{}] array element", ...);
            return EvalResult::Error;
        }
    }
    ...
}
```

### Fix

Add the same guard to the Dict branch, before any mutation:

```rust
ObjectData::Dict { key_type, value_type, mut entries } => {
    // Validate value type — mirrors the Array branch
    {
        let val_data = self.resolve(val_ref).unwrap();
        if !type_matches(&value_type, val_data) {
            eprintln!(
                "❌ TYPE ERROR: Cannot assign '{}' to <{},{}> dict value",
                val_data.type_name(), key_type, value_type
            );
            return EvalResult::Error;
        }
    }
    let search_key = obj_data_to_key_str(&idx_data);
    let owned_val = self.extract(val_ref);
    ...
}
```

---

## B-27 — Closures capture environment by copy — mutations do not persist across calls

**Date:** 2026-05-16  
**Files:** `src/evaluator.rs`, `src/region.rs`, `src/scope.rs`  
**Severity:** 🟠 Medium  
**Status:** ✅ Fixed

### Symptom

A closure that captures and mutates a local variable does not retain the updated value between invocations. The captured variable resets to its original value on each call:

```sz
fn any mkCounter() {
    let count = 0;
    return fn() {
        count = count + 1;
        return count;
    };
}
let c = mkCounter();
out c();   // → 1  (expected 1)
out c();   // → 1  (expected 2)
out c();   // → 1  (expected 3)
```

Any closure that relies on mutable captured state — counters, accumulators, generators — behaves incorrectly.

### Root cause

`capture_env()` serializes the current environment into `Vec<(String, OwnedValue)>` — a deep value copy completely detached from the live arena:

```rust
fn capture_env(&mut self) -> Vec<(String, OwnedValue)> {
    // iterates scopes + global bindings, calls self.extract() on each
    // extract() produces an OwnedValue — a full heap copy of the data
}
```

When the closure is invoked, `plant()` re-materializes the captured values into fresh arena slots for that call:

```rust
for (name, owned) in captured {
    let local_ref = self.plant(owned);   // new slot each time
    self.scopes.declare(name, local_ref);
}
```

Each call starts from the **snapshot taken at definition time**. The mutation `count = count + 1` modifies the freshly-planted slot for that invocation, but this local slot is destroyed when the call's scope pops. The next call plants a fresh copy of the original snapshot again.

### Fix

This is a **design-level issue** that conflicts with the Arena + Flash Scope memory model. A correct fix requires one of:

**Option A — Shared mutable cells for captured variables:**  
Use `Rc<RefCell<OwnedValue>>` (or equivalent) for variables that are both captured and assigned-to inside a closure. This introduces reference counting but only for the closure cells, not the whole runtime.

**Option B — Allocate captures in the global arena:**  
Instead of storing captures as `OwnedValue`, store them as `ObjectRef` pointing to the global arena. Since the global arena is never truncated, the refs remain valid across calls. Mutations inside the closure update the global slot directly. Requires distinguishing captured refs from regular refs to prevent unintended aliasing.

**Option C — Document as intentional value semantics:**  
Treat closure capture as value semantics by design (similar to Swift's capture of value types). Document the limitation and provide an explicit escape hatch (e.g. a mutable reference type or a dedicated `cell` type) for cases where shared mutable state is needed.

Option C has the lowest implementation cost and is consistent with the existing arena model. Options A and B produce the semantics most users expect from languages like JavaScript or Python.

---

---

## B-28 — `this.field[idx] = val` silently does nothing inside class methods

**Date:** 2026-05-17  
**Files:** `src/ast.rs`, `src/parser.rs`, `src/evaluator.rs`  
**Severity:** 🔴 Critical  
**Status:** ✅ Fixed

### Symptom

```serez
class Doubler {
    public Doubler([int] list) { this.list = list; }
    public void doubleAll() {
        let i = 0;
        while (i < this.list.length()) {
            this.list[i] = this.list[i] * 2;  // ← silently ignored
            i = i + 1;
        }
    }
}
let d = new Doubler([1, 2, 3]);
d.doubleAll();
out d.getList();   // [1, 2, 3] — expected [2, 4, 6]
```

### Root cause

Three interacting issues:

1. `IndexAssignStatement.target` was `String` — only simple identifier targets (`arr[i] = val`) were supported. `this.list[i] = val` starts with `this.` and was parsed by `parse_expression_statement`, which only checked for `obj.field = val` (FieldAssign), not `obj.field[i] = val` (IndexAssign).

2. `parse_expression_statement` returned `Statement::Expression(expr)` for `this.list[i]` — the index and value were never consumed. Silent no-op.

3. The evaluator's `Statement::IndexAssign` used `lookup_var(&stmt.target)` expecting a `String`, so any non-identifier target would fail to compile once the AST was fixed.

### Fix

- Changed `IndexAssignStatement.target: String` → `IndexAssignStatement.target: Expression`.
- Refactored `parse_index_assign_or_expr_statement` into `try_build_index_assign(expr)`.
- `parse_expression_statement` now checks: if `peek == Assign` and `expr` is `Index(...)`, calls `try_build_index_assign(expr)` — catches `this.list[i] = val`.
- Evaluator `Statement::IndexAssign` evaluates `target` as an `Expression`. For `DotCall` targets (field access), adds writeback: after mutating the array, re-extracts it as `OwnedValue` and stores it back in the instance's field list.

---

## B-29 — Array return type `[int]` not parsed in class method signatures

**Date:** 2026-05-17  
**Files:** `src/parser.rs`  
**Severity:** 🔴 Critical  
**Status:** ✅ Fixed

### Symptom

```serez
class Doubler {
    public [int] getList() { return this.list; }  // ← PARSER ERROR
}
// ❌ PARSER ERROR: Expected method name in class body
```

### Root cause

The class body member parser handled `void`, `int`, `decimal`, `string`, `bool` (via `is_type_keyword`) and `ClassName[?]` (via Ident+Ident pattern) as return types, but not `[type]` array return types. When it saw `[`, it fell through to the "Expected method name" error.

### Fix

Added a `LBracket` branch in the class member return-type parser that reads `[elem_type]` and produces `Some("[elem_type]")`.

---

## B-30 — `.pop()` and `.shift()` on empty array error instead of returning `null`

**Date:** 2026-05-17  
**Files:** `src/evaluator.rs`  
**Severity:** 🟡 High  
**Status:** ✅ Fixed

### Symptom

```serez
let a [int] = [];
let v = a.pop();   // ❌ ERROR: pop on empty array
```

Calling `.pop()` or `.shift()` on an empty array produced a fatal error and stopped execution. The expected behavior (consistent with nullable return semantics) is `null`.

### Root cause

Both methods had an early guard:
```rust
if elems.is_empty() {
    eprintln!("❌ ERROR: pop on empty array");
    return EvalResult::Error;
}
```

### Fix

Changed both guards to return `null_ref` instead of erroring:
```rust
if elems.is_empty() {
    return EvalResult::Value(self.null_ref);
}
```

---

## B-31 — Dict missing-key access errors instead of returning `null`

**Date:** 2026-05-17  
**Files:** `src/evaluator.rs`  
**Severity:** 🟡 High  
**Status:** ✅ Fixed

### Symptom

```serez
let d <string,int> = ({"a", 1});
let v = d["missing"];   // ❌ ERROR: Key 'missing' not found in dict
out v ?? 0;             // never reached
```

Accessing a dict key that does not exist caused a fatal runtime error.

### Root cause

The dict index handler had:
```rust
None => {
    eprintln!("❌ ERROR: Key '{}' not found in dict", search_key);
    EvalResult::Error
}
```

### Fix

Returns `null_ref` instead, allowing `??` to provide a default:
```rust
None => EvalResult::Value(self.null_ref),
```

---

## B-32 — `.sort()`, `.shift()`, `.unshift()` on instance field array do not write back

**Date:** 2026-05-17  
**Files:** `src/evaluator.rs`  
**Severity:** 🔴 Critical  
**Status:** ✅ Fixed

### Symptom

```serez
class MyList {
    public MyList([int] init) { this.data = init; }
    public void doSort() { this.data.sort(); }
    public [int] get() { return this.data; }
}
let ml = new MyList([3, 1, 2]);
ml.doSort();
out ml.get();   // [3, 1, 2] — expected [1, 2, 3]
```

Calling `.sort()`, `.shift()`, or `.unshift()` on an instance field array mutated a temporary copy but did not write the result back to the instance. The field remained unchanged.

### Root cause

The `writeback_ctx` mechanism detects chained calls of the form `this.field.method(args)` and writes the mutated collection back to the instance after the call. The detection list was:

```rust
const MUTATING: &[&str] = &["push", "pop", "remove", "Add", "Remove", "RemoveAll", "clear"];
```

`"sort"`, `"shift"`, and `"unshift"` were missing from this list, so their writeback was never triggered.

### Fix

Added the three missing methods:
```rust
const MUTATING: &[&str] = &["push", "pop", "shift", "unshift", "sort", "remove", "Add", "Remove", "RemoveAll", "clear"];
```

---

---

## B-33 — Interface field with array type `[int]` fails to parse

**Date:** 2026-05-17  
**Files:** `src/parser.rs`  
**Severity:** 🔴 Critical  
**Status:** ✅ Fixed

### Symptom

```serez
interface Playlist {
    name: string;
    songs: [string];   // ← PARSER ERROR
    count: int;
}
// ❌ PARSER ERROR: Expected type after ':' for field 'songs' in interface
```

### Root cause

The interface field type parser only accepted type-keyword tokens (`is_type_keyword`) — `void`, `int`, `decimal`, `string`, `bool`, `any`. When it encountered `[` (LBracket), it fell through to the error path. Similarly, class-name types (e.g., `fieldName: MyClass`) were not accepted.

### Fix

Extended the interface field type parser to support:
1. `[elemType]` — array field types (e.g., `songs: [string]`)
2. Plain `Ident` — class or interface type names (e.g., `owner: User`)

Applied the same pattern already used in function parameter type parsing and class method return type parsing.

---

## B-34 — `this.fn_field(args)` — calling a function stored in an instance field fails

**Date:** 2026-05-17  
**Files:** `src/evaluator.rs`  
**Severity:** 🟡 High  
**Status:** ✅ Fixed

### Symptom

```serez
class Transformer {
    public Transformer() {
        this.transform = fn(int x) { return x; };
    }
    public int apply(int x) {
        return this.transform(x);   // ← ERROR
    }
}
let t = new Transformer();
out t.apply(5);
// ❌ ERROR: 'Transformer' has no field or method named 'transform'
```

A function value stored in an instance field cannot be called with arguments via `this.field(args)`.

### Root cause

`eval_instance_dot` uses `dot_call.arguments.is_empty()` as the condition for field reads:
```rust
if dot_call.arguments.is_empty() {
    // field read path
}
```

When `this.transform(x)` is called, arguments are **not empty** (one arg: `x`). So the field-read path is skipped entirely, and the code falls through to `find_method` — which finds no method named `transform` — and returns an error.

Zero-arg calls work by accident (`this.field` with no parens goes through the field path). Non-zero-arg calls never reach the field path.

### Fix

Added a fallback in the `None` branch of the method dispatch (after `find_method` returns `None`): check if a field with the requested name exists, and if so, plant its value as an `ObjectRef` and call it as a function via `call_function`:

```rust
None => {
    if method_name == "toString" { /* ... */ }
    // Fallback: field holds a callable function (this.fn_field(args))
    if let Some((_, owned)) = fields.iter().find(|(n, _)| n == method_name) {
        let owned = owned.clone();
        let fn_ref = self.plant(owned);
        let mut arg_vals = Vec::new();
        for arg_expr in &dot_call.arguments {
            match self.eval_expression(arg_expr) {
                EvalResult::Value(r) => arg_vals.push(self.extract(r)),
                other => return other,
            }
        }
        return self.call_function(fn_ref, arg_vals);
    }
    eprintln!("❌ ERROR: ...");
    EvalResult::Error
}
```

---

*Last updated: 2026-05-18 — 44 bugs documented, 44 fixed, 0 open.*

---

## B-40 — Method calls on class instances omit `call_stack` frame

**Date:** 2026-05-17  
**Files:** `src/evaluator.rs`  
**Severity:** 🟡 High  
**Status:** ✅ Fixed

### Symptom

An error inside a class method showed only the call chain up to the method invocation, not the method itself:

```
❌ ERROR: Division by zero
    called from 'main' [line 10:5]
```

Expected:
```
❌ ERROR: Division by zero
    called from 'MyClass::divide' [line 5:12]
    called from 'main' [line 10:5]
```

### Root cause

`eval_instance_dot` increments `call_depth` and pushes a scope, but never called `self.call_stack.push(...)`. Compare with `Expression::Call` which always pushes a `CallFrame` before entering the function body. There was also no `call_stack.pop()` in the cleanup path.

### Fix

Added `call_stack.push(CallFrame { name: "ClassName::method", line, column })` before `scopes.push()`, and `call_stack.pop()` in the unified cleanup block (after `scopes.pop()`, before the error/success branch). This keeps `call_stack` synchronized on all paths including error returns.

---

## B-41 — `remove` listed in `MUTATING` but not implemented for arrays

**Date:** 2026-05-17  
**Files:** `src/evaluator.rs`  
**Severity:** 🟡 High  
**Status:** ✅ Fixed

### Symptom

```serez
let a [int] = [1, 2, 3];
a.remove(1);   // ❌ ERROR: Unknown array method 'remove'
```

Worse: `instance.field.remove(1)` set up writeback context but then failed with the same error, making the failure mode confusing.

### Root cause

`"remove"` was present in the `MUTATING` constant (used for writeback detection) but had no corresponding arm in `eval_array_method`. The `_` fallback caught it and errored.

### Fix

Added `"remove"` arm: validates the index, removes the element with `Vec::remove`, writes back the shortened array, and returns the extracted element (consistent with `pop`/`shift`):

```rust
"remove" => {
    let idx = self.eval_int_arg(&dot_call.arguments[0])?;
    if idx < 0 || idx as usize >= elems.len() { /* out of bounds error */ }
    let removed = e.remove(idx as usize);
    self.update_array(arr_ref, element_type, e);
    EvalResult::Value(self.plant(self.extract(removed)))
}
```

---

## B-42 — Seven common string methods missing

**Date:** 2026-05-17  
**Files:** `src/evaluator.rs`  
**Severity:** 🟠 Medium  
**Status:** ✅ Fixed

### Symptom

```serez
out "  hello  ".trim();          // ❌ ERROR: Unknown string method 'trim'
out "hello".toUpperCase();       // ❌ ERROR: Unknown string method 'toUpperCase'
out "HELLO".toLowerCase();       // ❌ ERROR: Unknown string method 'toLowerCase'
out "hello".startsWith("hel");   // ❌ ERROR: Unknown string method 'startsWith'
out "hello".endsWith("llo");     // ❌ ERROR: Unknown string method 'endsWith'
out "hello world".indexOf("wo"); // ❌ ERROR: Unknown string method 'indexOf'
out "hello".charAt(1);           // ❌ ERROR: Unknown string method 'charAt'
```

### Root cause

These seven methods were simply not implemented in `eval_string_method`.

### Fix

Added all seven arms. `indexOf` operates on character indices (not byte offsets) for Unicode correctness. `charAt` returns an empty string for out-of-bounds indices (JavaScript semantics). `toUpperCase`/`lower` also accept short aliases `upper`/`lower`.

---

## B-43 — Ternary operator parses chained expressions left-associatively

**Date:** 2026-05-18  
**Files:** `src/parser.rs`  
**Severity:** 🔴 Critical  
**Status:** ✅ Fixed

### Symptom

A chained ternary like `n > 0 ? "positive" : n < 0 ? "negative" : "zero"` returned the wrong branch. With `n = 5` it returned `"negative"` instead of `"positive"`:

```serez
fn string sign(int n) {
    return n > 0 ? "positive" : n < 0 ? "negative" : "zero";
}
out sign(5);   // → "negative"  (expected "positive")
out sign(-3);  // → "zero"      (expected "negative")
out sign(0);   // → "yes"       (expected "zero")
```

### Root cause

In `parse_infix_chain`, the `else_expr` of a ternary was parsed with `Precedence::Ternary`:

```rust
let else_expr = match self.parse_expression(Precedence::Ternary) { ... };
```

`Precedence::Ternary` has value `1`. `parse_expression` enters the Pratt infix loop only when `current_precedence < peek_precedence()`. For the inner `?`, `peek_precedence()` is also `Ternary (1)`. Since `1 < 1` is false, the inner ternary was **not** absorbed as the `else_expr` of the outer one.

The parser instead returned just `n < 0` as `else_expr` of the outer ternary. The outer Pratt loop then saw the remaining `?` with its left side being `(n > 0 ? "positive" : n < 0)` — a ternary used as a condition. For `n = 5`:
- The condition `n > 0 ? "positive" : n < 0` evaluates to `"positive"` (a truthy non-empty string).
- The second ternary then picks `"negative"` (the then-branch).

This is the exact opposite of what the programmer intended. The ternary was parsing as:

```
// Actual (left-associative — wrong):
(n > 0 ? "positive" : n < 0) ? "negative" : "zero"

// Expected (right-associative — correct):
n > 0 ? "positive" : (n < 0 ? "negative" : "zero")
```

### Fix

Parse `else_expr` with `Precedence::Lowest` instead of `Precedence::Ternary`:

```rust
// Before (wrong):
let else_expr = match self.parse_expression(Precedence::Ternary) { ... };

// After (correct):
let else_expr = match self.parse_expression(Precedence::Lowest) { ... };
```

With `Lowest (0)`, the condition `0 < 1 (Ternary)` is true, so the inner `?` IS absorbed into the `else_expr`, producing correct right-associative nesting.

**Location:** `src/parser.rs`, `parse_infix_chain`, the `TokenType::Question` branch.

**Lesson:** The `else_expr` of a ternary must always be parsed with `Precedence::Lowest` (or strictly below `Ternary`) to achieve right-associativity. Parsing it at `Ternary` precedence enforces left-associativity, which contradicts how every major language (C, JavaScript, Python, Rust) defines chained `?:`.

---

## B-35 — `for` loop init does not allocate a fresh slot (aliasing risk)

**Date:** 2026-05-17  
**Files:** `src/evaluator.rs`  
**Severity:** 🔴 Critical  
**Status:** ✅ Fixed

### Symptom

```serez
let arr = [10, 20, 30];
for (let i = arr[0]; i < 30; i = i + 1) { ... }
out arr[0];  // → updated instead of 10
```

The for-loop update `i = i + 1` corrupted `arr[0]` because `i` aliased the same arena slot.

### Root cause

`eval_statement(Let)` was fixed by B-18 to allocate a fresh slot. However, the analogous code inside `Statement::For` for the `init` variable was never updated:

```rust
let init_val = match self.eval_expression(&for_stmt.init.value) { ... };
self.scopes.declare(for_stmt.init.name.clone(), init_val); // ← no fresh slot
```

When `init.value` is `arr[0]`, `eval_expression` returns the `ObjectRef` of that element directly. Declaring `i` with that same ref means `i` shares the slot with `arr[0]`. On update, `scopes.assign("i", ...)` finds `i` is a Global ref and calls `global_arena.update(arr[0].index, new_data)`, corrupting the array.

### Fix

```rust
let init_data = self.resolve(init_val).unwrap().clone();
let fresh_init = self.alloc(init_data);
self.scopes.declare(for_stmt.init.name.clone(), fresh_init);
```

---

## B-36 — Prefix `-` on `i64::MIN` produces overflow panic

**Date:** 2026-05-17  
**Files:** `src/evaluator.rs`  
**Severity:** 🔴 Critical  
**Status:** ✅ Fixed

### Symptom

```serez
let x = -9223372036854775808;
out -x;  // → panic: attempt to negate with overflow (debug) / UB (release)
```

### Root cause

B-03 fixed overflow in binary arithmetic (`+`, `-`, `*`, `/`, `%`) with `checked_*` methods, but the unary prefix `-` operator was missed:

```rust
"-" => ObjectData::Integer(-i),  // panics for i64::MIN
```

### Fix

```rust
"-" => match i.checked_neg() {
    Some(v) => EvalResult::Value(self.alloc(ObjectData::Integer(v))),
    None => { eprintln!("❌ ERROR: Integer overflow in negation ..."); EvalResult::Error }
},
```

---

## B-37 — `sort` evaluates its argument argument up to 3 times

**Date:** 2026-05-17  
**Files:** `src/evaluator.rs`  
**Severity:** 🟠 Medium  
**Status:** ✅ Fixed

### Symptom

Calling `.sort(comparatorFn)` allocated two or three redundant arena slots for the same function reference.

### Root cause

The `sort` arm had three separate `eval_expression(&dot_call.arguments[0])` calls:

1. One to detect if the argument is a function (`use_comparator` check).
2. One inside `if use_comparator` to get `cb_ref`.
3. One in the non-comparator path to get the `"asc"/"desc"` string.

The first and second calls both allocated a ref to the same object without reusing the result.

### Fix

Evaluate the argument exactly once at the top of the `sort` arm, store the result in `arg_ref: Option<ObjectRef>`, and use it in all branches:

```rust
let arg_ref: Option<ObjectRef> = if dot_call.arguments.len() == 1 {
    match self.eval_expression(&dot_call.arguments[0]) {
        EvalResult::Value(r) => Some(r),
        _ => return EvalResult::Error,
    }
} else {
    None
};
let is_comparator = arg_ref.map_or(false, |r| {
    matches!(self.resolve(r), Some(ObjectData::Function { .. }))
});
```

---

## B-38 — `eprint!` instead of `eprintln!` in type-mismatch error corrupts output format

**Date:** 2026-05-17  
**Files:** `src/evaluator.rs`  
**Severity:** 🟢 Low  
**Status:** ✅ Fixed

### Symptom

When a type mismatch error was emitted (e.g. `1 + "x"`), the error line and the first call-stack frame appeared on the same line:

```
❌ ERROR: Type mismatch — ...    called from 'main' [line 2:5]
```

### Root cause

The type-mismatch branch in `eval_infix` used `eprint!` (no trailing newline) while every other error in the evaluator used `eprintln!`:

```rust
eprint!(   // ← missing newline
    "❌ ERROR: Type mismatch — operator '{}' ...",
    ...
);
for frame in self.call_stack.iter().rev() {
    eprintln!("    called from '{}' [...]", ...);  // appended to same line
}
```

### Fix

Changed `eprint!` to `eprintln!`.

---

## B-39 — `Str + Decimal` concatenation uses inconsistent float formatting

**Date:** 2026-05-17  
**Files:** `src/evaluator.rs`  
**Severity:** 🟢 Low  
**Status:** ✅ Fixed

### Symptom

```serez
let x = 3.14 + 0.01;
out x;             // → 3.15      (correct, via display())
out "val: " + x;  // → val: 3.1500000000000004  (wrong)
```

### Root cause

The `(ObjectData::Str, ObjectData::Decimal)` and `(ObjectData::Decimal, ObjectData::Str)` branches in `eval_infix` used an ad-hoc formatting expression:

```rust
let ds = if d == d.floor() && d.abs() < 1e15 {
    format!("{:.1}", d)
} else {
    format!("{}", d)  // ← exposes full f64 representation noise
};
```

This differed from `display()` which uses `format!("{:.10}", d)` trimmed, eliminating binary-representation noise.

### Fix

Extracted `format_decimal(d: f64) -> String` as a free function with the same logic as `display()`. Both `Str + Decimal` branches now call `format_decimal(d)`, making string concatenation consistent with `out`.

```rust
fn format_decimal(d: f64) -> String {
    if d.fract() == 0.0 { format!("{:.1}", d) }
    else {
        let s = format!("{:.10}", d);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
```

---

## B-44 — `super.method(args)` fails in non-constructor child class methods

**Date:** 2026-05-18  
**Files:** `src/evaluator.rs`  
**Severity:** 🔴 Critical  
**Status:** ✅ Fixed

### Symptom

Calling `super.method()` from a regular (non-constructor) child class method produced a runtime error:

```serez
class Animal {
    public Animal(string n) { this.name = n; }
    public string label() { return "Animal"; }
}

class Dog : Animal {
    public Dog(string n) { super(n); }
    public string parentLabel() {
        return super.label();  // ← fails
    }
}

let d = new Dog("Rex");
out d.parentLabel();
// → ❌ ERROR: Variable not found: super
```

### Root cause

The evaluator's `Expression::DotCall` handler called `eval_expression(&dot_call.object)` to resolve the receiver. When the object was `Expression::Identifier("super")`, this tried to look up `"super"` as a regular variable in scope — which doesn't exist. Only `super(args)` (the constructor-delegation path) was implemented, via `eval_super_call` which checks `self.constructing_class`. There was no equivalent path for `super.method(args)` from regular methods.

### Fix

Two changes in `src/evaluator.rs`:

**1. Early intercept in `Expression::DotCall`** (before the `eval_expression` call):

```rust
Expression::DotCall(dot_call) => {
    // super.method(args) — dispatch to parent class method
    if let Expression::Identifier(ref name) = *dot_call.object {
        if name == "super" {
            return self.eval_super_method_call(dot_call);
        }
    }
    // ... rest of DotCall handling
}
```

**2. New `eval_super_method_call` function** that:
- Reads `self.executing_class` (the currently executing class name)
- Looks up its parent via `class_registry`
- Calls `find_method(&parent_name, method_name)` to resolve the method starting from the parent (not the child), preserving correct dispatch in 3-level hierarchies
- Gets `this` from scope via `self.scopes.lookup("this")`
- Runs the method body with `this` bound to the current instance — so the parent's method can read/write the child's fields

```rust
fn eval_super_method_call(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
    let current_class = match &self.executing_class {
        Some(c) => c.clone(),
        None => { eprintln!("❌ ERROR: super.{}() called outside of a class method", ...); return EvalResult::Error; }
    };
    let parent_name = self.class_registry.get(&current_class)
        .and_then(|c| c.parent.clone())
        .ok_or_else(|| eprintln!("❌ ERROR: Class '{}' has no parent", current_class))?;
    let method = self.find_method(&parent_name, &dot_call.method)?;
    let this_ref = self.scopes.lookup("this")?;
    // ... eval args, push scope, declare this + params, run body, pop scope
}
```

### Lesson

`super.method()` requires a separate code path from regular `DotCall` dispatch because the receiver is not an object stored in a variable — it is a class reference resolved statically from `executing_class`. The fix mirrors how `eval_super_call` works for constructors but uses `executing_class` instead of `constructing_class` and dispatches the result to `find_method` starting from the parent class.
