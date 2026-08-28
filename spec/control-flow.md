# Control flow

This revision freezes the `for-in` subset of control flow. Other statements are
listed under **Coverage boundary** rather than being implied by this document.
Normative words such as "must" describe compatibility requirements.

## `for-in`

```text
for (let name in expression) { body }
for (let [slot, _, ...rest] in expression) { body }
```

The iterable expression is evaluated exactly once, before the loop scope is
entered. The accepted runtime values and yielded values are:

| Iterable | Value bound each iteration |
| --- | --- |
| Array | A value copy of each element, in index order. |
| String | A one-scalar string for each Unicode scalar value, in source order. This is not grapheme-cluster segmentation. |
| Dict | A value copy of each key, in the dict's insertion order. |

There is no user-defined iterator protocol. Any other value produces a
catchable `TypeError` (`SZ4002`) before the body runs.

The runtime snapshots the array elements/string scalars/dict keys before the
first iteration. Mutating the source during the body does not add or remove
visits from the active traversal. It may still mutate the source value itself.

The loop bindings live in the loop scope. Each iteration plants a fresh value,
so closures created by different iterations retain the value from their own
iteration. `break`, `continue`, labeled variants, `return`, user `throw` and a
runtime error from the body propagate according to their normal control-flow
meaning.

### Array-pattern iteration

With `for (let [a, _, ...rest] in rows)`, every yielded item must itself be an
array. A non-array item produces catchable `TypeError` (`SZ4002`) and the body is
not run for that item. Positional bindings past the item length receive `null`;
`_` is a hole and creates no binding; `rest` receives a new array containing the
remaining values.

An error or user `throw` produced while evaluating the iterable expression is
propagated unchanged; it is not replaced by the iterable type check.

## Conformance evidence

- `tests/unit_foreach_edge.sz`: value-copy, scope, closure, snapshot, control
  flow and invalid-iterable behavior.
- `tests/unit_destructuring.sz`: array-pattern iteration and invalid row types.
- `tests/runtime_outcome.rs`: stable `SZ4002` payload, catchability, scope
  cleanup and user-throw propagation.

## Coverage boundary

The exact contracts for `if`, `while`, C-style `for`, `do-while`, `switch`,
`match`, generators, `try/catch/finally` and labeled control flow remain to be
audited and added here. Their existing tests and runtime behavior remain the
compatibility source until then.
