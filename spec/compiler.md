# Experimental compiler contract

The AOT compiler is an experimental implementation component. It is not a
second definition of Serez semantics and is not currently exposed by the `sz`
CLI. The interpreter remains the authoritative runtime.

HIR and MIR validation are compiled and tested in normal builds. Native LLVM
emission remains behind the Cargo feature `llvm`.

## Atomic lowering

AST-to-HIR lowering returns either a complete `HirProgram` or one or more
compiler diagnostics. It must never return a partial program in which an
unsupported source construct was replaced with `null` or omitted.

- `SZ7001 UnsupportedStatement`: a statement has no semantics in the backend.
- `SZ7002 UnsupportedExpression`: an expression has no semantics in the backend.

Diagnostics are accumulated so users can fix multiple unsupported constructs in
one pass. Their codes and kinds are stable; message wording is informational.

## Currently accepted source subset

The checked lowering currently accepts scalar `int`, binary `decimal`, `bool`,
`string` and `null` values; variables and constants; supported unary and binary
operators; direct named function calls; functions; `if`, `switch`, `while`,
`do while`, C-style `for`; unlabeled `break`/`continue`; `return`; `out`; and
ternary/null-coalescing expressions.

Everything else must produce an `SZ7001` or `SZ7002` diagnostic until all later
pipeline stages implement equivalent behavior. In particular, exact `dec`,
exceptions, closures, collections, objects/classes, field/index operations,
imports, native capabilities, unsafe/pointers, generators, destructuring,
interpolation, `foreach`, `match`, and labeled control flow are not accepted.

Being represented by an HIR or MIR enum does not by itself mean a source feature
is supported. Those variants are scaffolding for incremental backend work.
