# Serez-Code — Test Documentation

## How to run

```powershell
.\run_tests.ps1                    # full suite
.\run_tests.ps1 -unit              # unit tests only
.\run_tests.ps1 -e2e               # E2E + error tests only
.\run_tests.ps1 -filter "switch"   # filter by name
.\run_tests.ps1 -generate          # regenerate .expected (after language changes)
```

**Result:** 91 files · 162 unit cases · 0 failures

---

## E2E Tests (Golden File)

Each `tests/NN_*.sz` file is executed and its `stdout` is compared against `tests/NN_*.expected`.
A failure means the output changed relative to the saved baseline.

---

### `01_basic.sz` — Primitive types and basic operators
Verifies that all primitive types are evaluated and displayed correctly.

| # | What it checks |
|---|----------------|
| 1 | Integer arithmetic: `+`, `-`, `*`, `/`, `%` |
| 2 | Decimal arithmetic: addition, multiplication, division |
| 3 | Booleans: `true`, `false`, `!true`, `!false` |
| 4 | Strings: literal and concatenation with `+` |
| 5 | Comparisons: `==`, `!=`, `>`, `<`, `>=`, `<=` |
| 6 | `null` literal |

---

### `01_arithmetic.sz` — Basic arithmetic
Integer and decimal arithmetic, operator precedence, and null coalescing `??`.

| # | What it checks |
|---|----------------|
| 1 | Addition, subtraction, multiplication, integer division, modulo |
| 2 | Decimals: floating-point operations |
| 3 | Mixed `int + decimal` operations |
| 4 | Precedence: `*` before `+` |
| 5 | `null ?? default_value` |

---

### `01_variables.sz` — Variable declaration and types
Declaration with `let`, reassignment, and all data types.

| # | What it checks |
|---|----------------|
| 1 | `let` with each primitive type: `int`, `decimal`, `string`, `bool`, `null` |
| 2 | Variable reassignment |
| 3 | Long variable names |

---

### `02_arithmetic.sz` — Advanced arithmetic
Deeper look at arithmetic edge cases.

| # | What it checks |
|---|----------------|
| 1 | Integer division truncates (does not return decimal) |
| 2 | Unary negation on integers and decimals |
| 3 | Mixed `int` / `decimal` operations |
| 4 | Integer overflow detection |
| 5 | String repetition with `*` |
| 6 | Complex precedence with parentheses |

---

### `02_variables.sz` — Variables and types
Variables with different types, reassignment and conversion.

| # | What it checks |
|---|----------------|
| 1 | Declaration of all primitive types |
| 2 | Reassignment changes value correctly |
| 3 | `null` coalescing `??` over different types |

---

### `02_variables_scope.sz` — Variable scoping
Scope behavior in blocks and functions.

| # | What it checks |
|---|----------------|
| 1 | Block variable does not escape to outer scope |
| 2 | Function can modify outer variable |
| 3 | Variable shadowing inside function |
| 4 | `null` in nested scope |

---

### `03_control_flow.sz` — Basic control flow
`if/else`, `while`, `for`, `break`, `continue`.

| # | What it checks |
|---|----------------|
| 1 | Simple `if`/`else` |
| 2 | `while` with accumulator |
| 3 | `for` with index |
| 4 | `break` exits the loop |
| 5 | `continue` skips the iteration |

---

### `03_strings.sz` — String methods
All built-in string methods.

| # | What it checks |
|---|----------------|
| 1 | `.length()` |
| 2 | `.includes()` / `.contains()` |
| 3 | `.replace()` / `.replaceAll()` |
| 4 | `.split()` |
| 5 | `.substring()` |
| 6 | `.toString()` on numbers |
| 7 | Interpolation `"{expr}"` |

---

### `04_control_flow.sz` — Full control flow
More advanced control flow with compound conditions and nested loops.

| # | What it checks |
|---|----------------|
| 1 | `if`/`else if`/`else` chain |
| 2 | Compound conditions with `&&`, `\|\|` |
| 3 | `while` with `break` and `continue` |
| 4 | `for` with early `break` |
| 5 | Nested loops |
| 6 | `if` as expression (return value) |

---

### `04_functions.sz` — Basic functions and recursion
Declaration, return, parameters, recursion, and higher-order functions.

| # | What it checks |
|---|----------------|
| 1 | `fn` with return type and parameters |
| 2 | Recursion: factorial, fibonacci |
| 3 | Functions as values (assigned to variables) |
| 4 | Closures that capture the environment |
| 5 | Higher-order functions (`any f`) |

---

### `05_arrays.sz` — Basic arrays
Typed arrays and their fundamental operations.

| # | What it checks |
|---|----------------|
| 1 | Declaration `[int]`, `[string]` |
| 2 | Index access, mutation |
| 3 | `.push()`, `.pop()`, `.shift()`, `.unshift()` |
| 4 | `.sort()` ascending and descending |
| 5 | `.map()`, `.filter()`, `.reduce()` |
| 6 | Method chaining |

---

### `05_functions.sz` — Advanced functions
Explicit return types, lambdas, currying and nested functions.

| # | What it checks |
|---|----------------|
| 1 | Functions with full type signatures |
| 2 | Function literals (lambdas with `=>`) |
| 3 | Functions as arguments (`any`) |
| 4 | Currying and composition |
| 5 | Functions that return functions |

---

### `06_arrays.sz` — Advanced arrays
Mutation operations, sort with comparator, and strict typing.

| # | What it checks |
|---|----------------|
| 1 | Index mutation `arr[i] = v` |
| 2 | `.pop()` / `.shift()` return the removed value |
| 3 | `.sort()` with lambda comparator `(a, b) => a - b` |
| 4 | Strict typing rejects push of wrong type |
| 5 | Chaining `filter().map().reduce()` |

---

### `06_strings.sz` — Advanced strings
Property access, interpolation with complex expressions, and methods.

| # | What it checks |
|---|----------------|
| 1 | `.length` as property and `.length()` as method |
| 2 | Interpolation with expressions, function calls |
| 3 | Concatenation of different types |
| 4 | `.split()` and accessing the result |
| 5 | Empty string and its methods |

---

### `07_dicts.sz` — Dictionaries
Creation, access, mutation and methods of typed dictionaries.

| # | What it checks |
|---|----------------|
| 1 | `<string, int>` declaration with initial pairs |
| 2 | Key access `dict["key"]` |
| 3 | Modification of existing value |
| 4 | `.Add()` to insert new pairs |
| 5 | `.Remove()` to delete by key |
| 6 | `.toList()` / `.toArray()` |
| 7 | Dict with `any` values for mixed types |

---

### `08_classes.sz` — Classes and instances
Class definition, constructors, methods, inheritance and polymorphism.

| # | What it checks |
|---|----------------|
| 1 | Constructor `public Class(params)` |
| 2 | `this.field = value` in constructor |
| 3 | Instance method calls |
| 4 | Inheritance: `class B extends A` |
| 5 | `super(args)` in child constructor |
| 6 | Polymorphism: overridden method |
| 7 | Mathematical calculations inside methods |

---

### `09_interfaces.sz` — Interfaces
Interface definition, instantiation and object patching.

| # | What it checks |
|---|----------------|
| 1 | `interface I { type field; }` |
| 2 | `new I { field: value }` |
| 3 | Field access and modification |
| 4 | Full and partial patching with `{ field: new }` |
| 5 | Interface arrays with `.filter()` |

---

### `10_lambdas.sz` — Lambdas and higher-order functions
Lambda syntax, closures, `map`/`filter`/`reduce`, composition.

| # | What it checks |
|---|----------------|
| 1 | Single-parameter lambda: `x => x * x` |
| 2 | Two-parameter lambda: `(a, b) => a + b` |
| 3 | Lambda with block body: `(a, b) => { ... }` |
| 4 | `.map()`, `.filter()`, `.reduce()` with lambdas |
| 5 | `.sort()` with comparator |
| 6 | Closure captures environment variable |
| 7 | Custom HOF (`my_map`, `my_filter`) |
| 8 | Chaining `filter().map().filter()` |
| 9 | Lambda with index: `(item, i) => ...` |
| 10 | Composition: `compose(f, g)` |

---

### `11_nullables.sz` — Nullables and null coalescing
Handling `null`, nullable types `T?`, and the `??` operator.

| # | What it checks |
|---|----------------|
| 1 | `null == null`, `null != null` |
| 2 | `null ?? "default"` with different types |
| 3 | `??` chain: `a ?? b ?? c` |
| 4 | Function with `string?` return |
| 5 | `if (value == null)` in condition |
| 6 | Array with nulls filtered with `.filter(x => x != null)` |
| 7 | `null ??` with complex expression as fallback |

---

### `12_math.sz` — Math functions
All built-in `Math.*` functions.

| # | What it checks |
|---|----------------|
| 1 | `Math.abs()` on int and decimal |
| 2 | `Math.sqrt()` |
| 3 | `Math.floor()`, `Math.ceil()`, `Math.round()` |
| 4 | `Math.min()`, `Math.max()` with mixed int and decimal |
| 5 | `Math.pow()` |
| 6 | `Math.log()`, `Math.log2()`, `Math.log10()` |
| 7 | Fibonacci with Math for demonstration |

---

### `13_edge_cases.sz` — General edge cases
17 edge-case scenarios spanning multiple features.

| # | What it checks |
|---|----------------|
| 1 | Empty string: `""`, `.length()`, comparison |
| 2 | Single-element array: access, push |
| 3 | Function with no arguments |
| 4 | `return` in the middle of `for` |
| 5 | Closure make_adder with different values |
| 6 | Interpolation with function call |
| 7 | Recursion with accumulator (`sum_to`) |
| 8 | Class with constructor, getter and mutation |
| 9 | Cross-type comparison (`1==1`, `"a"=="a"`, `null==null`) |
| 10 | `??` over nullable function result |
| 11 | Function that receives and returns array |
| 12 | Chained string method calls |
| 13 | Maximum `i64` integer |
| 14 | Deep nested `if/else if` |
| 15 | Array of lambda functions |
| 16 | Boolean equality (fix B-xx) |
| 17 | Mixed modulo `int % decimal`, `decimal % int` |

---

### `14_arch_features.sz` — Architectural features
Features that affect the evaluator design.

| # | What it checks |
|---|----------------|
| 1 | `.length` as property (without parentheses) |
| 2 | Escape sequences in strings |
| 3 | Instance field mutation from external function |
| 4 | Interface object patching |
| 5 | 3-level inheritance (`A → B → C`) |
| 6 | `break` in nested loop (breaks the correct loop) |
| 7 | Short-circuit `&&` and `\|\|` |
| 8 | `return` from nested loop in function |
| 9 | Closures in loops capturing iteration variable |
| 10 | Global dict mutation from function |

---

### `15_arch_stress.sz` — Architectural stress tests
Cases that combine multiple features at once.

| # | What it checks |
|---|----------------|
| 1 | `.sort()` with numeric and string comparators |
| 2 | Typed array rejects push of wrong type |
| 3 | Class with array field, methods that manipulate it |
| 4 | Dict pipeline: `filter` + `map` + `reduce` |
| 5 | Inheritance + method override |
| 6 | Closure composition |
| 7 | Mutual recursion (two functions that call each other) |
| 8 | Interpolation with complex expressions |
| 9 | Function that returns array of instances |
| 10 | `continue` inside loop with complex logic |

---

### `16_error_paths.sz` — Controlled error paths
Behaviors that could previously fail silently.

| # | What it checks |
|---|----------------|
| 1 | String repetition with `*` |
| 2 | Mixed string concatenation with different types |
| 3 | `.unshift()` adds to the front |
| 4 | Direct assignment to dict key |
| 5 | Nullable array `[string?]` |
| 6 | Modification of global array from function |
| 7 | `.sort()` with direction flag |

---

### `17_function_syntax.sz` — Function syntax variants
All forms of defining and using functions.

| # | What it checks |
|---|----------------|
| 1 | Arrow function with explicit return type |
| 2 | Anonymous function assigned to variable |
| 3 | Function as value passed to another function |
| 4 | Composition and currying |
| 5 | Single-parameter lambda without parentheses |
| 6 | Multi-line lambda body |
| 7 | Array of functions |
| 8 | Untyped parameters (`any`) |

---

### `18_error_cases.sz` — Operator boundary behaviors
Edge cases that don't produce an error but do produce specific behavior.

| # | What it checks |
|---|----------------|
| 1 | `null ??` in type variants |
| 2 | Operator precedence |
| 3 | Short-circuit with side effects |
| 4 | `!` negation on comparison result |
| 5 | Cross-type comparisons |
| 6 | Chained string method calls |
| 7 | `parseInt()`, `parseDecimal()` |
| 8 | Array mutation by reference |
| 9 | `.pop()` / `.shift()` return the element |
| 10 | `.toString()` on primitives |

---

### `19_untested_docs.sz` — Documented but untested features
Features that existed in docs but had no tests.

| # | What it checks |
|---|----------------|
| 1 | `.reduce()` with string accumulator |
| 2 | Chained `filter` + `reduce` |
| 3 | `dict.toArray()` with filtering |
| 4 | `parseInt()` with whitespace |
| 5 | `replace()` vs `replaceAll()` (replaces first vs all) |
| 6 | `.split("")` with empty separator |
| 7 | `.sort()` with explicit direction flag |
| 8 | `.map()` with index parameter |
| 9 | Standalone block `{ ... }` with scoping |
| 10 | Closure capturing outer variables |
| 11 | `.toString()` on different types |
| 12 | `.contains()` as alias for `.includes()` |

---

### `20_more_edge_cases.sz` — More edge cases
Feature combinations in practical scenarios.

| # | What it checks |
|---|----------------|
| 1 | `arr.length` in interpolation |
| 2 | Method call inside interpolation |
| 3 | Assignment to dict key |
| 4 | Method chaining |
| 5 | Function passed as value |
| 6 | Nested `if` as expression |
| 7 | Early `return` in `for` |
| 8 | Array created inside function |
| 9 | Use of function return value |

---

### `21_string_interp_complex.sz` — Complex interpolation
`"{expr}"` interpolation with non-trivial expressions.

| # | What it checks |
|---|----------------|
| 1 | Dict access with quoted key inside `{}` |
| 2 | `arr[i]` inside interpolation |
| 3 | Method call inside interpolation |
| 4 | Arithmetic expression in interpolation |
| 5 | Class instance field in interpolation |
| 6 | `null ??` inside interpolation |

---

### `22_math_edge.sz` — Math edge cases
Specific behaviors of math functions and numeric conversion.

| # | What it checks |
|---|----------------|
| 1 | `Math.abs()` with positive, negative and zero |
| 2 | Exact and irrational `Math.sqrt()` |
| 3 | `Math.floor()`, `Math.ceil()`, `Math.round()` on midpoint values |
| 4 | `Math.min()` / `Math.max()` with mixed types |
| 5 | `Math.pow()` with integer and decimal base and exponent |
| 6 | Integer division truncates toward zero |
| 7 | Decimal display: trailing zeros and `d.0` |
| 8 | Modulo with negatives |

---

### `23_boundary_cases.sz` — Type and structure boundary cases
Array, string, and dict limits under extreme conditions.

| # | What it checks |
|---|----------------|
| 1 | String repetition with factor `0` → empty string |
| 2 | `.sort()` on empty array (no crash) |
| 3 | `.split("")` on empty string |
| 4 | `dict.Remove()` of non-existent key (no crash) |
| 5 | `??` chain when all are null |
| 6 | Boolean comparisons |
| 7 | Decimal precision with `0.1 + 0.2` |
| 8 | Negative decimals |
| 9 | `parseInt()` applied to decimal |
| 10 | `parseDecimal()` applied to integer |

---

### `24_chained_calls.sz` — Chained calls
Method chaining on arrays, strings, and classes.

| # | What it checks |
|---|----------------|
| 1 | `arr.sort().map()` chained |
| 2 | Chained string methods |
| 3 | Method result used directly in expression |
| 4 | Builder pattern in class (methods return `this` implicitly) |
| 5 | Function that returns a class instance |

---

### `26_complex_scenarios.sz` — Complex scenarios
Scenarios that integrate multiple language features.

| # | What it checks |
|---|----------------|
| 1 | 2D array: `arr[i][j]` access |
| 2 | 2D array traversal with nested loop |
| 3 | Global variable modified from nested function |
| 4 | `return` from `if` inside `while` |
| 5 | Dict with `any` values (mixed types) |
| 6 | Array of class instances |
| 7 | Multiple closures capturing different values |

---

### `27_escape_sequences.sz` — Escape sequences
Verification of all escape sequences in strings.

| Sequence | Checks |
|----------|--------|
| `\n` | Newline |
| `\t` | Tab |
| `\"` | Literal double quote |
| `\\` | Literal backslash |
| `\{` | Literal brace (no interpolation) |
| `\r` | Carriage return |

---

### `28_final_checks.sz` — Final checks
Additional behaviors of dicts, functions and classes.

| # | What it checks |
|---|----------------|
| 1 | Dict preserves insertion order |
| 2 | `.toList()` and `.toArray()` |
| 3 | Multiple `return` statements in different function branches |
| 4 | Nullable function returns `null` or value |
| 5 | Function that calls another function |
| 6 | Method chaining with string operations |

---

### `29_bug_regression.sz` — Bug regressions (B-30, B-31, B-35, B-36, B-39, B-41, B-42)
Tests added specifically for each fixed bug.

| Bug | What it checks |
|-----|----------------|
| B-35 | `for (let i = arr[0]; ...)` does not corrupt `arr[0]` |
| B-36 | Negation of negative: `-(-1)` = `1`; large values without overflow |
| B-39 | `"str" + decimal` uses same format as `out decimal` |
| B-41 | `.remove(idx)` returns the element and shortens the array |
| B-42 | `.trim()`, `.toUpperCase()`, `.toLowerCase()`, `.upper()`, `.lower()`, `.startsWith()`, `.endsWith()`, `.indexOf()`, `.charAt()` |
| B-30 | `.pop()` / `.shift()` on empty array return `null` |
| B-31 | `dict["nonExistentKey"]` returns `null` |
| B-03/36 | Normal arithmetic within range does not fail |

---

### `30_class_regression.sz` — Class bug regressions (B-28, B-29, B-32, B-34, B-40, B-41)
Tests verifying specific bug fixes in the class system.

| Bug | What it checks |
|-----|----------------|
| B-29 | Class method can return `[int]` (typed array) |
| B-28 | `this.field[idx] = value` works inside method |
| B-32 | `.sort()`, `.shift()`, `.unshift()` on instance fields |
| B-34 | Field that stores a function can be called: `this.fn()` |
| B-40 | Correct call stack tracking in methods (depth) |
| B-41 | `.remove()` on instance array field |

---

### `31_compound_assign.sz` — Compound assignment operators (E2E)
Basic E2E coverage of `+=`, `-=`, `*=`, `/=`, `%=`.

| # | What it checks |
|---|----------------|
| 1 | `+=` on integer |
| 2 | `-=` on integer |
| 3 | `*=` on integer |
| 4 | `/=` on integer |
| 5 | `%=` on integer |
| 6 | `+=` on string (concatenates) |
| 7 | `+=` on decimal |
| 8 | `+=` in loop accumulator |
| 9 | `+=` on array element |
| 10 | `*=` on array element |
| 11 | `+=` on instance field (via method) |

---

### `32_switch.sz` — Switch (E2E)
Basic E2E coverage of `switch`.

| # | What it checks |
|---|----------------|
| 1 | Exact integer match |
| 2 | Case with multiple values: `case 1, 2, 3:` |
| 3 | String match |
| 4 | `default` when no case matches |
| 5 | `switch` inside function with `return` |
| 6 | Switch with expression as value: `arr[i] / 10` |

---

### `33_try_catch.sz` — Try / Catch / Throw / Finally (E2E)
Full E2E coverage of exception handling.

| # | What it checks |
|---|----------------|
| 1 | `catch` captures thrown string |
| 2 | `throw` with integer |
| 3 | `finally` runs even with throw |
| 4 | `finally` runs without throw (normal path) |
| 5 | Exception thrown from function propagated to caller |
| 6 | Function without error: does not trigger catch |
| 7 | Nested try: inner catch, outer never sees the exception |
| 8 | `finally` inside function with `return` in catch |
| 9 | Exception from class method (`BankAccount.withdraw`) |
| 10 | Balance does not change if withdraw fails |

---

### `38_real_programs.sz` — Real E2E programs (8 complete programs)
Full language integration: 8 real programs exercising all implemented features.

| # | Program | What it checks |
|---|---------|----------------|
| 1 | Bank Account | Classes, getters, exceptions, optional chaining `?.`, `??` |
| 2 | Task Manager | Enums (`Priority`, `TaskStatus`), `Set` (deduplication), static methods, factory |
| 3 | Shape Hierarchy | `abstract`/`sealed` classes, inheritance, `Math.PI`, `Math.round` |
| 4 | Functional Pipeline | Closures, `compose`, spread `...`, rest `...params`, `map`/`filter`/`reduce` |
| 5 | JSON Config | `JSON.stringify` and `JSON.parse` with primitives, arrays and roundtrip |
| 6 | Algorithms | `factorial`, `fib`, bitwise `is_power_of_two`, `count_bits`, Newton `sqrt` |
| 7 | Error Handling | `AppError`/`NetworkError` hierarchy, `is` type dispatch, `finally` |
| 8 | String Processing | `padStart`, `trimLeft`/`trimRight`, `split`, `slice`, `toUpperCase` |

---

## Error Tests (`err_*`)

Each `tests/err_*.sz` file must produce at least one `❌` line on stderr.
If no error is produced, the test **fails** (the error condition was not detected).

| File | Error condition it checks |
|------|--------------------------|
| `err_arity.sz` | Function called with fewer arguments than declared |
| `err_bang_nonbool.sz` | `!` applied to integer (not boolean) |
| `err_bool_plus_int.sz` | `true + 1` — adding boolean and integer |
| `err_bounds.sz` | Array access out of bounds |
| `err_call_undefined.sz` | Calling a function that does not exist |
| `err_div_zero.sz` | Integer division by zero |
| `err_extra_iface_field.sz` | Interface instantiated with field not declared in it |
| `err_for_scope_leak.sz` | `for` variable accessed outside the loop |
| `err_modulo_zero.sz` | Modulo by zero |
| `err_not_function.sz` | Attempting to call a value that is not a function |
| `err_overflow.sz` | `i64` overflow in multiplication |
| `err_private.sz` | Calling a `private` method from outside the class |
| `err_return_toplevel.sz` | `return` outside a function |
| `err_return_type_mismatch.sz` | Function returns type different from declared |
| `err_sort_mixed.sz` | `.sort()` on array with incompatible mixed types |
| `err_type_param.sz` | Passing argument of wrong type to typed function |
| `err_typed_push.sz` | `.push()` of wrong type in typed array |
| `err_undeclared_assign.sz` | Assigning to undeclared variable |
| `err_undeclared_class.sz` | `new Class()` where the class does not exist |
| `err_undeclared.sz` | Using undeclared variable |
| `err_foreach_nonarray.sz` | `for (let x in 42)` — iterating over an integer (not iterable) |
| `err_foreach_dict.sz` | `for (let x in dict)` — iterating over a dictionary (not iterable) |

---

## Unit Tests (`unit_*`)

Unit tests use the `tests/framework.sz` framework.
Each case calls `test("name", () => { assert(...); })`.
A failure produces `[FAIL]` on stdout; the runner detects it.

---

### `unit_try_catch.sz` — Basic Try/Catch (12 tests)

| Test | What it checks |
|------|----------------|
| catch receives thrown string | `throw "oops"` → `e == "oops"` in catch |
| catch receives thrown int | `throw 42` → `e == 42` in catch |
| code after throw in try does not run | Statements after `throw` are skipped |
| finally runs on normal path | `finally` runs when there is no exception |
| finally runs on throw path | `finally` runs after `catch` |
| exception from function propagates to caller catch | `throw` inside `fn` propagates to caller |
| nested try — inner catch, outer never sees it | Inner catch handles: outer does not fire |
| nested try — inner re-throws, outer catches | Rethrow from inner catch reaches outer |
| catch with return in function | `return` inside `catch` returns the correct value |
| assert throws on false | `assert(false, msg)` throws `msg` |
| assert does NOT throw on true | `assert(true, msg)` does not throw |
| exception from class method propagates | `throw` inside class method propagates |

---

### `unit_try_catch_edge.sz` — Try/Catch edge cases (10 tests)

| Test | What it checks |
|------|----------------|
| return in try — return value preserved through finally | `return` in try body: value reaches caller even though `finally` runs |
| throw in finally overrides try return | `finally` throws: overrides the try `return` |
| throw in finally overrides normal try completion | `finally` throws: overrides normal try completion |
| throw inside for loop propagates to outer catch | `throw` inside `for` → reaches catch wrapping the for |
| throw inside while loop propagates to outer catch | `throw` inside `while` → reaches outer catch |
| try with only finally — local variable modified correctly | `try { } finally { }` without `catch` is valid and works |
| finally-only try propagates throw | `try { throw } finally { }` → throw propagates after finally |
| catch body throws — propagates to outer catch | Throwing from inside `catch` → outer catch receives it |
| three-level nested try/rethrow chain | Three levels of nested catch with chained rethrow |
| throw propagates through multiple function calls | `throw` through two function frames reaches the catch |

---

### `unit_switch.sz` — Basic Switch (8 tests)

| Test | What it checks |
|------|----------------|
| switch matches exact int | Exact case with integer |
| switch matches exact string | Exact case with string |
| switch default when no case matches | `default` executes if no case matches |
| switch with multiple values per case | `case 1, 2:` — multiple values in one case |
| switch no match no default — skips cleanly | No match and no default: executes nothing, no crash |
| switch with expression as value | `switch (arr[1] / 10)` — expression as discriminant |
| switch inside function returns correctly | `return` inside switch case returns from the function |
| switch with bool | `case true:` / `case false:` |

---

### `unit_switch_edge.sz` — Switch edge cases (9 tests)

| Test | What it checks |
|------|----------------|
| switch — no fall-through between cases | Only the matching case runs; the following ones do not |
| switch with decimal values | `switch (1.5)` with `case 1.5:` |
| switch with null value | `switch (null)` with `case null:` |
| switch inside for loop — accumulates correctly | Switch inside for: each iteration evaluates the switch |
| nested switch | Switch inside another switch |
| throw inside switch case propagates | `throw` inside case reaches outer catch |
| switch inside for loop — break exits the loop | `break` inside case breaks the `for`, not the switch |
| switch default runs exactly once | Default runs exactly once when there is no match |
| switch multiple values per case — middle value matches | Third value in `case 7, 8, 9:` matches correctly |

---

### `unit_compound_assign.sz` — Basic compound assignment (11 tests)

| Test | What it checks |
|------|----------------|
| += on int | `10 += 5 → 15` |
| -= on int | `10 -= 3 → 7` |
| *= on int | `4 *= 3 → 12` |
| /= on int | `20 /= 4 → 5` |
| %= on int | `17 %= 5 → 2` |
| += on string | Concatenates: `"hello" += " world"` |
| += on decimal | `1.5 += 0.5 → 2.0` |
| += accumulates in loop | Sums 1..10 with `sum += i` → 55 |
| += on array element | `arr[1] += 5` modifies the correct element |
| *= on array element | `arr[0] *= 3` modifies the correct element |
| += on instance field | `this.val += n` inside class method |

---

### `unit_compound_assign_edge.sz` — Compound assignment edge cases (12 tests)

| Test | What it checks |
|------|----------------|
| -= on decimal | `5.0 -= 1.5 → 3.5` |
| /= on decimal | `10.0 /= 4.0 → 2.5` |
| *= on decimal | `3.0 *= 2.5 → 7.5` |
| -= on array element | `arr[1] -= 5` with adjacent element verification |
| /= on array element | `arr[0] /= 4 → 25` |
| += on dict entry | `dict["alice"] += 5` modifies the dict entry |
| *= on dict entry | `dict["x"] *= 4` modifies the dict entry |
| -= on dict entry | `dict["n"] -= 37` modifies the dict entry |
| += on instance field directly | `c.val += 3` from outside the class |
| -= on instance field directly | `b.n -= 7` from outside the class |
| compound assign chain on same variable | `x += 5; x *= 2; x -= 6; x /= 4; x %= 4` → 2 |
| += accumulates across iterations with growing step | Accumulation with growing step |

---

### `unit_operators.sz` — Operators (15 tests)

| Test | What it checks |
|------|----------------|
| && short-circuits when left is false | `false && boom()` → boom is never called |
| \|\| short-circuits when left is true | `true \|\| boom()` → boom is never called |
| && evaluates right side when left is true | `true && true`, `true && false` |
| \|\| evaluates right side when left is false | `false \|\| true`, `false \|\| false` |
| ?? short-circuits when left is not null | `"value" ?? boom()` → boom is not called |
| ?? evaluates right when left is null | `null ?? "fallback"` → `"fallback"` |
| && evaluates right side — throw from right propagates | `true && fn_that_throws()` → throw reaches catch |
| operator precedence: * before + | `2 + 3 * 4 = 14`, `10 - 2 * 3 = 4` |
| operator precedence: comparison after arithmetic | `2 + 3 > 4`, `10 / 2 == 5`, `3 * 3 >= 9` |
| chained boolean operations | `true && true && true`, combinations with `\|\|` |
| unary negation on int and decimal | `-5 = 0-5`, `-(-3) = 3`, `-1.5` |
| ! operator | `!false = true`, `!true = false`, `!!true = true` |
| string equality and inequality | `"a" == "a"`, `"a" != "b"` |
| integer comparison operators | `>`, `<`, `>=`, `<=`, `!=` on integers |
| decimal comparison operators | `>`, `<`, `>=`, `==`, `!=` on decimals |

---

### `unit_closures_mutable.sz` — Closures with mutable state (7 tests)

Covers the closure pattern that modifies its captured state between calls: counters, accumulators, shared state.

| Test | What it checks |
|------|----------------|
| make_counter: each call increments the state | `make_counter()` returns closure; successive calls return 1, 2, 3 |
| two independent counters do not share state | Two closures from `make_counter` have separate counts |
| accumulator: sums values between calls | Closure that accumulates sum between calls: 10 → 15 → 40 → 30 |
| make_adder_from with parameterized initial state | `make_adder_from(10)` starts at 10 and accumulates; independent from `make_adder_from(0)` |
| closure captures for loop variable and retains it | `captured = i` inside loop captures correct value; `fns[2]() == 4` |
| toggle: alternates bool state between calls | `make_toggle(false)` → true → false → true |
| closure accumulates strings | Builder closure that concatenates strings between calls |

---

### `unit_closures_edge.sz` — Closures and HOF (9 tests)

| Test | What it checks |
|------|----------------|
| lambda captures value at creation — basic | `let f = x => x + base` uses captured `base` |
| lambda returned from function — make_adder | `make_adder(5)` returns closure; `add5(3) = 8` |
| lambda returned from function — make_multiplier | `make_mult(2)` returns closure; closure composition |
| higher-order composition: compose(f, g)(x) = f(g(x)) | `compose(inc, double)(5) = 11` |
| apply_twice: f(f(x)) | `apply_twice(double, 3) = 12`; `apply_twice(square, 2) = 16` |
| lambda as argument to user-defined HOF | `my_map([1..5], x => x * 2)` with custom HOF |
| lambda with block body and multiple returns | Multi-line lambda with several `return` branches |
| closures used in map — each closure independent | Array of closures `[adder(1), adder(2), adder(3)]` independent |
| lambda captures outer fn parameter — currying | `curry_add(3)` returns `inner` that adds 3 |

---

### `unit_forin_string.sz` — for-in over strings (10 tests)

Covers character-by-character string iteration with `for-in`.

| Test | What it checks |
|------|----------------|
| for-in string collects characters in order | Iterates `"hello"` and verifies order and length |
| for-in string counts characters | `n++` per char of `"serez"` → 5 |
| for-in empty string does not iterate | `""` → zero iterations |
| for-in string counts vowels | `"Hello World"` → 3 vowels (e, o, o) |
| for-in string rebuilds in uppercase | `"abc"` → `"ABC"` using `toUpperCase()` per char |
| for-in string: break when character found | Breaks on finding `"-"` in `"serez-code"`, verifies position |
| for-in string: continue skips spaces | Omits spaces in `"a b c"` → `"abc"` |
| for-in string in function: early return | `firstDigit("abc3def") == 3` with `return` inside for-in |
| for-in string: result of split | Iterates over `"one,two,three".split(",")` |
| for-in string of single character | `"x"` produces exactly one character |

---

### `unit_foreach_ternary_incr.sz` — ForEach, Ternary and ++/-- (22 tests)

| Test | What it checks |
|------|----------------|
| for-in sums array elements | `for (let n in nums)` sums all elements of a `[int]` |
| for-in iterates in order | Iteration order matches array order |
| for-in over empty array does nothing | Empty array does not execute the body |
| for-in over string iterates characters | Iterates over each character of a `string` |
| for-in break exits early | `break` inside the body stops iteration |
| for-in continue skips elements | `continue` skips the current element |
| for-in nested loops | Two nested `for-in` with independent variables |
| for-in with method on elements | `.length()` call on each string element |
| ternary selects true branch | `true ? 1 : 2` produces `1` |
| ternary selects false branch | `false ? 1 : 2` produces `2` |
| ternary with expression condition | `n > 5 ? "big" : "small"` with variable |
| ternary is lazy — only evaluates chosen branch | The unchosen branch is not evaluated (`called == 0`) |
| ternary chained (right-associative) | `n == 1 ? "one" : n == 2 ? "two" : "other"` → `"two"` |
| ternary in expression | `a > b ? a : b` computes the maximum |
| ternary with null check | `val == null ? "was null" : "not null"` |
| postfix i++ increments by 1 | `i++` leaves `i = i + 1` |
| postfix i-- decrements by 1 | `i--` leaves `i = i - 1` |
| prefix ++i increments by 1 | `++i` leaves `i = i + 1` |
| prefix --i decrements by 1 | `--i` leaves `i = i - 1` |
| ++ inside while loop | `count++` used as loop advance |
| -- countdown | `n--` in countdown, `sum = 3+2+1 = 6` |
| ++ and -- together | `a++` and `b--` operate independently |

---

---

### `unit_foreach_edge.sz` — ForEach, Ternary and ++/-- edge cases (18 tests)

| Test | What it checks |
|------|----------------|
| for-in return from function exits immediately | `return` inside `for-in` exits the entire function |
| for-in throw caught by enclosing try-catch | `throw` inside `for-in` is received by outer `catch` |
| for-in over expression (split result) | `for (let w in "a,b,c".split(","))` iterates method result |
| for-in does not mutate the source array | The source array is not modified during iteration |
| for-in closures capture each iteration independently | Closure created in each iteration captures its own `v` |
| for-in inside class method mutates this field | `for-in` inside class method can mutate `this.total` |
| for-in ternary in body selects sign | Ternary in the body selects `"+"` or `"-"` per iteration |
| for-in with ++ counter | `count++` inside `for-in` correctly counts iterations |
| ternary as function return value | Chained ternary as `return`: `n>0 ? "positive" : n<0 ? "negative" : "zero"` |
| ternary result in array literal | `[a > b ? a : b, a < b ? a : b]` — ternary as array element |
| ternary inside while condition | `while (i < (limit > 2 ? 5 : 3))` — ternary in while condition |
| ternary in string interpolation | `"x is {x > 0 ? "positive" : "negative"}"` — interpolated ternary |
| ternary with ?? — ?? binds tighter | `val ?? "default" ? "yes" : "no"` = `(val ?? "default") ? "yes" : "no"` |
| ternary lazy — false branch with throw not evaluated | False branch containing `throw` is not evaluated when condition is true |
| ++ on global variable works | `g++; g++; ++g` from global scope → `g == 3` |
| -- to zero and below | `n--` three times from 2 → `-1` |
| ++ inside for-in body | `evens++` inside `for-in` with condition: counts only even numbers |
| ++ and -- in nested while loops | `inner_total++` and `outer++`/`inner++` in nested while → `outer==3`, `inner_total==9` |

---

### `unit_super_method.sz` — super.method() in normal child class methods (10 tests)

| Test | What it checks |
|------|----------------|
| super.method() no args dispatches to parent | `super.label()` calls `Counter::label` literal "Counter", not the child's override |
| own overridden method not affected | The child's own `label()` returns its override |
| super.method() returns value using this fields | `super.doubled()` uses `this.value` from the child → correct |
| super.method() with argument | `super.add(10)` with argument — `3 + 10 = 13` |
| super.method() dispatches to parent override not own override | `super.describe()` calls `Counter::describe`, not `NamedCounter::describe` |
| super.method() result used in expression | `super.label() + " vs " + this.label()` in an expression |
| 3-level: super.label() dispatches to NamedCounter::label | `TaggedCounter.super.label()` calls `NamedCounter::label` (does not skip to `Counter`) |
| 3-level: own label() overrides all | `TaggedCounter`'s own `label()` returns its override |
| 3-level: chained super through NamedCounter::parentLabel to Counter::label | `grandparentLabel()` chains `super` → `NamedCounter::parentLabel` → `super.label()` → "Counter" |
| 3-level: this.value accessible via inherited super method | `parentDoubled()` through inheritance uses correct `this.value` |

### `unit_functions_adv.sz` — Advanced functions (9 tests)

Covers functional patterns not covered in `unit_functions.sz`: multiple defaults, mutual recursion, advanced HOF.

| Test | What it checks |
|------|----------------|
| multiple parameters with default value | `format(val, pre="[", suf="]")` with 0, 1 and 2 overrides |
| default override of first only | `sum(1)`, `sum(1,2)`, `sum(1,2,3)` with 2 defaults |
| mutual recursion: isEven / isOdd | `isEven`/`isOdd` call each other; correct for n=0..7 |
| tail recursion: sum 1..n with accumulator | `sumTo(n, acc=0)` tail-recursive; `sumTo(10) == 55` |
| function that returns function based on condition | `selector(true)` → double, `selector(false)` → +100 |
| function stored in variable and reassigned | Variable `op` points to `double` then to `triple` |
| pipeline of functions in array | Array of lambdas applied in sequence: `5 → 6 → 12 → 9` |
| recursive function: pow with negative exponent | `pow(2.0, 3) == 8.0`, `pow(2.0, -1) == 0.5` |
| function with any parameter: dispatch by is | `describe(42)` → `"integer: 42"`, `describe(null)` → `"other"` |

---

### `unit_class_patterns.sz` — Class patterns (8 tests)

Covers OOP design patterns: factory method, fluent builder, counter class, array fields with HOF, private method.

| Test | What it checks |
|------|----------------|
| factory method: method that returns new instance | `point.translate(3,4)` returns new `Point`; original does not mutate |
| class Counter with reset | `inc()`, `dec()`, `reset()` manage internal state |
| class with array field and methods on it | `Bag.add/remove/has()` operate on `this.items` |
| inheritance: child class extends with new method | `Circle` inherits `id()` and adds `area()` |
| private method used only internally | `Validator.classify()` uses private method `isEven()` internally |
| fluent builder pattern | `QueryBuilder.from().where().limit().build()` chained |
| array of instances with map and filter | `filter(p => p.price > 20)` and `reduce` on array of `Product` |
| class Registry: stores and retrieves by name | `register("pi", 3.14)` then `get("pi") == 3.14`; `get("nope") == null` |

---

### `unit_dict_advanced.sz` — Advanced dicts (9 tests)

Covers non-string key types, dynamic construction, pass-by-value semantics, and grouping patterns.

| Test | What it checks |
|------|----------------|
| dict with int key | `<int,string>` with keys 0, 1, 2; non-existent key = null |
| dict `<int,int>`: numeric operations | `squares[3] == 9`, sum of values |
| for-in over dict `<int,string>` | Iterates integer keys; `keys.includes(10)` |
| dict as parameter: pass-by-value semantics | Mutation in function does NOT persist in caller |
| dict built dynamically with while loop | `d[i] = i*i` inside while; `d[3] == 9` after loop (B-60 fix) |
| dict as frequency table | Counts occurrences with `freq[w] = (freq[w] ?? 0) + 1` |
| dict returned from function | Function returns `<string,any>` with different value types |
| dict of arrays: group by category | `groups["evens"]` and `groups["odds"]` accumulate with `push` |
| dict: keys() and values() in sync | `keys()` and `values()` have same length; `reduce` over values |

---

### `unit_reverse_writeback.sz` — `.reverse()` mutates in-place and returns array (8 tests)

Covers B-62: `.reverse()` must mutate the array and return a reference to the same array (same as `.sort()`).

| Test | What it checks |
|------|----------------|
| reverse mutates the array | `a.reverse(); a[0] == 5` — mutation persists |
| reverse returns the same array | `let b = a.reverse(); b[0] == 30` — returned value is the reversed array |
| return value is the same array | `a` and `b` after `b = a.reverse()` reflect the same state |
| reverse on empty array | `[].reverse()` without error, length remains 0 |
| reverse on single element | `[42].reverse()` changes nothing |
| double reverse restores order | `a.reverse(); a.reverse()` → original state |
| reverse and then iterate | Sum of elements is the same before and after reverse |
| reverse works on string array | `["a","b","c"].reverse()` → `["c","b","a"]` |

---

### `unit_trim_aliases.sz` — `trimLeft` / `trimRight` as aliases (8 tests)

Covers B-63: `trimLeft()` and `trimRight()` are aliases for `trimStart()` and `trimEnd()`.

| Test | What it checks |
|------|----------------|
| trimLeft removes leading whitespace | `"   hello".trimLeft() == "hello"` |
| trimRight removes trailing whitespace | `"hello   ".trimRight() == "hello"` |
| trimLeft and trimRight together | `s.trimLeft().trimRight()` is equivalent to `s.trim()` |
| trimLeft is identical to trimStart | Both produce the same result on the same string |
| trimRight is identical to trimEnd | Both produce the same result on the same string |
| trimLeft preserves trailing spaces | Only removes leading spaces, not trailing |
| trimRight preserves leading spaces | Only removes trailing spaces, not leading |
| all five trim variants consistent | `trim`, `trimStart`, `trimEnd`, `trimLeft`, `trimRight` in sync |

---

### `unit_comprehensive_new.sz` — Deep coverage of new features (33 tests)

Exhaustive unit tests of all added features: `const`, `enum`, labeled loops, `abstract`/`sealed` classes, getters/setters, static methods, default parameters, optional chaining, `do-while`, bitwise/power, spread/rest, `Math`, `JSON`, `Set`, and `is` operator.

| Test | Area |
|------|------|
| const prevents reassignment | `const` — immutability enforced at runtime |
| const in different type contexts | `const` for int, string, bool, decimal |
| enum variant access | `Color.Red == Color.Red`, different variants are not equal |
| enum in conditional | `if (prio == Priority.High)` |
| labeled break exits outer loop | `outer: for ... break outer` |
| labeled continue skips outer iteration | `outer: for ... continue outer` |
| abstract class cannot be instantiated | `new AbstractBase()` throws non-catchable error |
| sealed class cannot be inherited | Inheriting from sealed = type error |
| getter returns computed value | `get area()` computes on the fly |
| setter validates and stores | `set value(v)` validates input |
| static method called on class | `MathHelper.add(3, 4)` without instance |
| default parameter single | `greet("Bob")` uses `"Hello"` as default |
| default parameter override | `greet("Bob", "Hi")` uses the override |
| optional chaining short-circuits | `null?.method` returns null without error |
| optional chaining with ?? | `obj?.field ?? "default"` |
| do-while executes at least once | Body runs even if condition is initially false |
| do-while with break | `break` inside do-while |
| bitwise AND / OR / XOR | `5 & 3 == 1`, `5 \| 3 == 7`, `5 ^ 3 == 6` |
| bitwise NOT | `~0 == -1`, `~7 == -8` |
| shift operators | `1 << 3 == 8`, `8 >> 2 == 2` |
| power operator | `2 ** 10 == 1024`, `3 ** 3 == 27` |
| spread in array literal | `[...a, ...b]` concatenates |
| rest parameters | `fn sum(...nums)` accumulates variable arguments |
| Math namespace | `Math.abs`, `Math.floor`, `Math.ceil`, `Math.sqrt`, `Math.PI` |
| Math.min / Math.max variadic | `Math.min(3, 1, 4, 1, 5)`, `Math.max(...)` |
| JSON.stringify primitives | int, string, bool, null to JSON |
| JSON.stringify array | `[1,2,3]` → `"[1,2,3]"` |
| JSON.parse roundtrip | stringify → parse → same value |
| Set deduplication | `new Set(["a","b","a"])` → size 2 |
| Set operations | `add`, `has`, `delete`, `clear`, `toArray` |
| is type check on primitives | `42 is int`, `"x" is string`, `true is bool` |
| is type check on instances | `obj is ClassName` |
| is type check in catch | `e is NetworkError` dispatch in catch |

---

## Coverage Summary

| Area | E2E | Unit | Error | Total |
|------|-----|------|-------|-------|
| Primitive types and arithmetic | 01_basic, 01_arithmetic, 02_arithmetic, 22_math_edge | unit_operators (partial) | err_overflow, err_bool_plus_int | ~40 cases |
| Variables and scoping | 01_variables, 02_variables, 02_variables_scope | — | err_undeclared, err_undeclared_assign, err_for_scope_leak | ~15 cases |
| Control flow | 03_control_flow, 04_control_flow | — | — | ~12 cases |
| Functions and recursion | 04_functions, 05_functions, 17_function_syntax | unit_functions_adv (9) | err_arity, err_return_toplevel, err_return_type_mismatch, err_type_param | ~30 cases |
| Strings | 03_strings, 06_strings, 21_string_interp_complex, 27_escape_sequences | — | — | ~25 cases |
| Arrays | 05_arrays, 06_arrays, 23_boundary_cases | unit_compound_assign (partial) | err_bounds, err_typed_push, err_sort_mixed | ~30 cases |
| Dictionaries | 07_dicts | unit_dict_advanced (9) + unit_compound_assign_edge (partial) | — | ~22 cases |
| Classes and inheritance | 08_classes, 30_class_regression | unit_class_patterns (8) + unit_super_method (10) | err_private, err_undeclared_class | ~40 cases |
| Interfaces | 09_interfaces | — | err_extra_iface_field | ~8 cases |
| Lambdas and closures | 10_lambdas, 26_complex_scenarios | unit_closures_edge (9) + unit_closures_mutable (7) | — | ~35 cases |
| Nullables | 11_nullables | — | — | ~8 cases |
| Math | 12_math, 22_math_edge | — | err_div_zero, err_modulo_zero | ~12 cases |
| Try/Catch/Throw/Finally | 33_try_catch | unit_try_catch (12) + unit_try_catch_edge (10) | — | 32 cases |
| Switch | 32_switch | unit_switch (8) + unit_switch_edge (9) | — | 23 cases |
| Compound assign | 31_compound_assign | unit_compound_assign (11) + unit_compound_assign_edge (12) | — | 34 cases |
| Operators | 14_arch_features, 18_error_cases | unit_operators (15) | err_bang_nonbool | 20 cases |
| Regressions | 29_bug_regression | — | — | ~25 cases |
| Edge cases | 13_edge_cases, 15_arch_stress, 20_more_edge_cases, 23_boundary_cases, 28_final_checks | — | — | ~40 cases |
| ForEach / Ternary / ++-- | — | unit_foreach_ternary_incr (22) + unit_foreach_edge (18) + unit_forin_string (10) | err_foreach_nonarray, err_foreach_dict | 50 cases |
| const / enum | unit_comprehensive_new (partial) | — | — | ~8 cases |
| abstract / sealed / static / default params | unit_comprehensive_new (partial) | — | — | ~8 cases |
| optional chaining / do-while | unit_comprehensive_new (partial) | — | — | ~4 cases |
| Bitwise / power | unit_comprehensive_new (partial) + unit_bitwise_edge | err_negative_shift, err_excessive_shift | — | ~12 cases |
| Spread / rest | unit_comprehensive_new (partial) | — | — | ~4 cases |
| Math namespace | 12_math, 22_math_edge | unit_comprehensive_new (partial) | — | ~10 cases |
| JSON namespace | unit_comprehensive_new (partial) + 38_real_programs (prog 5) | — | — | ~8 cases |
| Set | unit_comprehensive_new (partial) + 38_real_programs (prog 2) | — | — | ~6 cases |
| is type check | unit_is_type_advanced + unit_comprehensive_new (partial) | — | — | ~10 cases |
| Advanced exceptions | 37_exceptions_e2e | unit_exceptions_advanced + unit_comprehensive_new (partial) | sec_runtime_not_catchable | ~20 cases |
| Real integrated programs | 38_real_programs (8 programs) | — | — | ~80 cases |
| `.reverse()` write-back | — | unit_reverse_writeback (8) | — | 8 cases |
| trimLeft / trimRight aliases | — | unit_trim_aliases (8) | — | 8 cases |
