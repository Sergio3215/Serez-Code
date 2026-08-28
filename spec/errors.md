# Errors and diagnostics

This document defines the public diagnostic namespace and the runtime error value exposed to Serez programs.

## Diagnostic code ranges

- `SZ1xxx`: lexer diagnostics
- `SZ2xxx`: parser diagnostics
- `SZ3xxx`: semantic and type diagnostics
- `SZ4xxx`: evaluator and runtime diagnostics
- `SZ5xxx`: module and import diagnostics
- `SZ6xxx`: permission, resource, and security diagnostics
- `SZ7xxx`: compiler diagnostics

Codes are stable identifiers. Tooling must not parse the human-readable message to classify an error.

## Coverage today

A code is a promise of stability, so ranges are populated deliberately rather
than all at once. What is emitted now:

| Range | Status |
| --- | --- |
| `SZ1xxx` | Emitted. The lexer reports unexpected characters, incomplete strings/comments and invalid base-prefixed integers through the shared frontend channel. |
| `SZ2xxx` | Emitted. `SZ2000` is the generic syntax error every parser message falls back to; `SZ2001` is the AST depth ceiling. |
| `SZ3xxx` | Emitted. `SZ3000` is the generic semantic/type diagnostic. |
| `SZ4xxx`–`SZ6xxx` | Emitted for structured runtime failures, by `kind`. See below. |
| `SZ7xxx` | Emitted by the experimental compiler. |

Individual messages move from a generic code to a narrower one only once a test
pins what the narrower code means. Until then the generic code is the honest
answer: it says "this is a syntax error" without also claiming a classification
nothing verifies.

### Frontend diagnostics

| Code | Meaning |
| --- | --- |
| `SZ1001` | Unexpected source character, including embedded NUL. |
| `SZ1002` | Unterminated normal, single-quoted or raw string. |
| `SZ1003` | Unterminated block comment. |
| `SZ1004` | Empty, malformed or overflowing binary/hex integer. |
| `SZ2000` | Syntax error. |
| `SZ2001` | Source describes an AST deeper than `MAX_PARSE_DEPTH` (512). Nesting costs one level per level; an operator chain costs one level per operator, because it builds a tree that deep for the type checker, the evaluator and the AST's drop glue to walk. Rejecting this is what keeps such source from exhausting the native stack and killing the process without a diagnostic. |
| `SZ3000` | Semantic or type diagnostic. Advisory: the checker is partial and runtime checks remain authoritative, so `sz file.sz` reports these and still runs. |

They are reported on stderr as `❌ LEXER ERROR [SZ1001] [file line:col]: …`,
`❌ PARSER ERROR [SZ2000] [file line:col]: …` and `❌ TYPE ERROR [SZ3000]
[line line:col]: …`, and are published to the LSP with the code in the standard
`code` field of each diagnostic. See `lexical-grammar.md` for the lexical rules.

The experimental compiler currently uses `SZ7001` for unsupported statements
and `SZ7002` for unsupported expressions. Lowering is atomic: if either code is
reported, no partial HIR program is returned. Unsupported syntax must never be
silently replaced with `null` or omitted.

## Caught runtime errors

A recoverable runtime error caught by `catch (e)` is an `Error` instance with these public fields:

| Field | Type | Contract |
| --- | --- | --- |
| `code` | `string` | Stable diagnostic identifier. |
| `kind` | `string` | Compatibility category such as `TypeError`. |
| `message` | `string` | Human-readable explanation; wording is not a stable API. |
| `span` | `string?` | Best available `line:column`, or `null` when unavailable. |
| `stack` | `[string]` | Innermost-first runtime frames. |
| `notes` | `[string]` | Optional additional explanations. |

Existing `kind` and `message` fields are retained for compatibility.

Current runtime mappings are:

| Code | `kind` |
| --- | --- |
| `SZ4000` | Generic runtime category, including `Overflow` and `RangeError` until narrower codes are specified. |
| `SZ4001` | `ReferenceError` |
| `SZ4002` | `TypeError` |
| `SZ4003` | `IndexOutOfBounds` |
| `SZ4004` | `DivisionByZero` (division and modulo) |
| `SZ4005` | `IOError` |
| `SZ5001` | `ModuleNotFound` |
| `SZ6001` | `PermissionError` |
| `SZ6002` | `ResourceError` |
| `SZ6003` | `UnsafeError` |
| `SZ6004` | `SecurityError` |

Exact-decimal (`dec`) division/modulo by zero, invalid exponent types and
arithmetic overflow use this structured channel and are catchable just like
their integer equivalents. Their current categories are respectively
`DivisionByZero`, `TypeError` and `Overflow`.

Operator type errors, invalid shifts and integer exponent overflow use the same
recoverable channel. `try/catch` may consume these ordinary programming errors.
Both `[...value]` and `fn(...value)` require `value` to be an array. A different
operand yields catchable `TypeError` / `SZ4002`; evaluating the operand may still
propagate a user `throw` unchanged.
`for-in` requires an array, string or dict, and an array pattern used by
`for-in` requires every visited item to be an array. Declaration array/object
destructuring likewise validates its source. These four mismatches are
catchable `TypeError` / `SZ4002`; errors and user `throw` from source evaluation
propagate unchanged.

A default-parameter expression is an ordinary call-time evaluation. A user
`throw` propagates as that same user exception; a runtime failure preserves its
structured payload and recoverability classification. No call form may replace
either outcome with `null`. See `functions.md`.

Class/interface construction validation is recoverable. An unknown construction
target is `ReferenceError` / `SZ4001`; an invalid interface shape or field,
abstract-class instantiation, class field-form construction, and constructor
arity are `TypeError` / `SZ4002`. See `classes.md`.

Recoverable `super` validation uses the same categories. Invalid context,
missing parent, impossible implicit construction, arguments for a parent without
a constructor, and constructor/method arity are `TypeError` / `SZ4002`. A
missing method in the parent chain is `ReferenceError` / `SZ4001`.

Ordinary instance/static dispatch validation is also recoverable. A missing
instance member or static method is `ReferenceError` / `SZ4001`; method arity,
external private-method call/reference and declared return mismatch are
`TypeError` / `SZ4002`. A caught privacy error does not expose the member.

Property validation follows the same recoverable channel. Getter-only writes,
field writes on non-instances, private accessor use, accessor arity and declared
getter return mismatches are `TypeError` / `SZ4002`. Errors and user `throw`
raised inside a getter/setter preserve their original payload.

Inheritance cycles and attempts to extend a sealed class are `TypeError` /
`SZ4002`. A declared but still unresolved parent is `ReferenceError` / `SZ4001`
when the hierarchy is used. Forward parent declarations remain valid until use;
the later parent declaration resolves them.

DateTime/DateField arity and type mismatches are `TypeError` / `SZ4002`;
invalid calendar/epoch ranges are `RangeError` / `SZ4000`; field arithmetic
overflow is `Overflow` / `SZ4000`; and unknown members are `ReferenceError` /
`SZ4001`. All are recoverable. Invalid arity is rejected before evaluating
argument expressions, while errors and user `throw` from valid arguments
propagate unchanged. See `datetime.md`.

Task arity/type mismatches are recoverable `TypeError` / `SZ4002`; unknown or
evicted task IDs and unknown Task members are recoverable `ReferenceError` /
`SZ4001`. Nested argument failures propagate unchanged. Task concurrency,
message and native-thread ceilings use fatal `ResourceError` / `SZ6002`. A
worker's own runtime failure remains a compatibility `ERROR: [code] kind:
message` string returned by `poll`, not an exception in the parent. See
`tasks.md`.

Random arity/type mismatches are recoverable `TypeError` / `SZ4002`; invalid
bounds, deviations, probabilities, empty choices and invalid shape dimensions
are recoverable `RangeError` / `SZ4000`; unknown Random members are recoverable
`ReferenceError` / `SZ4001`. Tensor allocation ceilings remain fatal
`ResourceError` / `SZ6002`. Valid argument failures propagate unchanged. See
`random.md`.

String method arity/type mismatches are recoverable `TypeError` / `SZ4002`; a
negative padding target is recoverable `RangeError` / `SZ4000`; unknown members
are recoverable `ReferenceError` / `SZ4001`. Padding result/allocation ceilings
are fatal `ResourceError` / `SZ6002`. Valid argument failures propagate
unchanged. See `strings.md`.

Array method arity/type mismatches, values rejected by a declared element type,
non-function callbacks and a comparator returning a non-number are recoverable
`TypeError` / `SZ4002`; a `sort` order string other than `"asc"`/`"desc"` is
recoverable `RangeError` / `SZ4000`; `pop`/`shift` on an empty array and an
out-of-range `remove` index are recoverable `IndexOutOfBounds` / `SZ4003`;
unknown members are recoverable `ReferenceError` / `SZ4001`. Arity is rejected
before arguments are evaluated, callbacks are validated before iteration, and a
comparator that fails leaves the receiver unsorted. Valid argument failures
propagate unchanged. See `arrays.md`.

The shared argument helpers used by Array, `Crypto.randomBytes` and `Regex.*`
return the original outcome when evaluation does not yield a value, so a user
`throw` inside an argument is no longer collapsed into a generic error.

Dict and Set follow the same shape. Wrong arity on any method, an `Add`
argument that is not an entry literal, a key or value rejected by a declared
dict type, a non-array `new Set(values)` initialiser and a non-Set argument to
`union`/`intersection` are recoverable `TypeError` / `SZ4002`; an unknown member
is recoverable `ReferenceError` / `SZ4001`. Arity is rejected before arguments
are evaluated. See `dicts.md` and `sets.md`.

An unknown `Set` member previously reported `TypeError`, which made Set the only
collection whose `kind` could not separate "no such member" from "called
wrongly". It now matches every other type.

Core expression diagnostics use the same channel. A typed parameter or declared
return type that a value does not satisfy, an array or dict literal whose
element violates its own declared type, an entry literal or object patch used
outside the position it is valid in, a dot call on a type that has no methods,
`&` applied to something that is not a named variable and dereferencing a
non-pointer are recoverable `TypeError` / `SZ4002`. An unknown enum variant, a
method on an enum variant that does not exist, taking the address of an
undeclared variable and a dangling pointer are recoverable `ReferenceError` /
`SZ4001`.

The call stack that a typed-parameter failure used to print as a side effect now
travels in the payload's `stack`, so an embedder reads frames instead of
scraping stderr. `break`/`continue` escaping a call is a recoverable generic
`RuntimeError` / `SZ4000`.

## Recoverable and fatal runtime errors

Structured does not imply catchable. Runtime failures carry the same public
payload in both cases, but the evaluator records an internal recoverability bit:

- **Recoverable:** ordinary programming faults such as `TypeError`, bounds
  errors, division by zero and arithmetic overflow. A matching `catch` receives
  the public `Error` value above.
- **Fatal:** security gates and resource ceilings. They cross `try/catch`
  unchanged and abort the complete program, so user code cannot retry an
  allocation limit in a loop or convert a denied operation into normal flow.

The following fatal producers are migrated and covered by regressions:

- string repetition, call depth, Tensor element counts, `Memory.alloc`, GPU
  buffers/matrix output and file-read ceilings use `ResourceError` / `SZ6002`;
- every native namespace permission guard uses `PermissionError` / `SZ6001`;
- every lexical `unsafe { }` gate uses `UnsafeError` / `SZ6003`;
- the `OS.exec`/`OS.spawn` protected-target policy uses `SecurityError` /
  `SZ6004`.

None can be caught. Ordinary size/type mistakes below a resource ceiling remain
recoverable; for example, `Memory.alloc(0)` is `TypeError` / `SZ4002`. Legacy
producers elsewhere are still being migrated incrementally and may reach
`UnstructuredError` while preserving their previous fatal behavior.

## Current implementation boundary

Evaluation of a complete program has a structured boundary. Embedders can use
`Evaluator::eval_program_outcome`, whose result distinguishes:

- successful value;
- structured runtime error;
- uncaught user exception (`throw`);
- invalid top-level `return`, `break` or `continue`;
- a legacy unstructured runtime failure.

These outcomes are mutually exclusive. In particular, a user `throw` is not a
runtime error, and control flow is not inferred from printed text. A reused
evaluator must not attach an earlier runtime payload to a later failure.

`run_source_detailed` exposes the same distinction as `RunFailure`, including
the ordered `ParseError` values for a frontend failure. The existing
`run_source` API and its exit-only `Outcome` remain available for compatibility.
Both forms keep the current exit-code contract: `0` for success and `1` for a
frontend or evaluation failure.

Human diagnostics are rendered once at the pipeline boundary. The compatibility
`eval_program` adapter retains its historical `Some(value)` / `None` result and
human output for older embedders.

The evaluator still propagates failures *internally* with an
`EvalResult::Error` sentinel and temporarily stores structured payload in
`last_error` until a matching `catch` or program boundary consumes it. A
generation counter prevents stale payload reuse, but the side channel remains
transitional implementation debt, not a public semantic guarantee. Producers
that have not migrated to `rt_err_kind` are surfaced explicitly as
`UnstructuredError`; class `super`/dispatch paths and other native subsystems
still contain examples under active audit.
A future internal refactor may carry the payload directly, provided the public
fields and control-flow behavior remain compatible.
