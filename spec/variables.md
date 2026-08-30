# Variables and destructuring

This document freezes declaration destructuring. The rest of what it once listed
as pending has since been written elsewhere: type annotations in `types.md`,
shadowing, block scoping, closure capture and `const` rejection in `scopes.md`,
and assignment, copying and receiver writeback in `values.md`.

## Declarations

`let` creates a mutable binding and `const` creates a binding that cannot be
reassigned. Both forms use the current lexical scope. Destructuring is a
declaration form, not a general assignment target, and its right-hand expression
is evaluated exactly once.

## Array destructuring

```text
let [first, _, third, ...rest] = array
const [head, ...tail] = array
```

- The right-hand value must be an array. Otherwise evaluation raises catchable
  `TypeError` (`SZ4002`) and declares none of the pattern bindings.
- Slots bind by zero-based position. A slot beyond the source length receives
  `null`; extra source elements are ignored when no rest binding exists.
- `_` is a hole and does not create a binding.
- `...rest` must be last and receives a new array with the remaining value
  copies. It is empty when nothing remains.
- Every binding created by a `const` pattern is immutable.

Nested patterns are not supported by the current grammar.

## Object destructuring

```text
let {name, age: years} = value
```

Accepted right-hand values are:

- a Dict, matched by string keys;
- a class instance, matched by field name;
- a `DateTime`, whose calendar fields are exposed as integer entries.

Shorthand `{name}` binds a local named `name`; `{name: alias}` binds `alias`.
A missing property receives `null`. An unsupported right-hand value raises
catchable `TypeError` (`SZ4002`) and declares none of the pattern bindings.
Object rest and nested object patterns are not supported.

An error or user `throw` raised while evaluating either destructuring source is
propagated unchanged.

## Conformance evidence

- `tests/unit_destructuring.sz`: holes, rest, short arrays, aliases, missing
  properties, Dicts, class instances and invalid operands.
- `tests/unit_datetime.sz`: `DateTime` destructuring and missing fields.
- `tests/runtime_outcome.rs`: structured payload, catchability and nested user
  `throw` propagation.

## Coverage boundary

Every rule above was re-probed against the binary in the current cycle: binding
by position, ignored extras, a slot beyond the source, rest and its copies, the
single evaluation of the source, dicts, class instances, `DateTime`, the alias
form, a missing property, `const` binding and rejection, and the four shapes the
grammar refuses. All held.

What this document once listed as pending is now covered: typed declarations in
`types.md`; `const` write attempts, shadowing and capture in `scopes.md`;
ordinary assignment, copying and receiver writeback in `values.md`. Nothing
about destructuring remains unaudited.
