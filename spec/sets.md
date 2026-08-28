# Sets

Normative contract for `Set` values and their built-in methods.

Every rule here was derived from the current implementation and is pinned by a
test in `tests/36_stdlib_e2e.sz`, `tests/73_language_360_e2e.sz`,
`tests/unit_dict_set_errors.sz` or `tests/runtime_outcome.rs`.

## Construction and value semantics

```serez
let s = new Set();            // empty
let t = new Set([3, 1, 2, 1]); // deduplicated, insertion order kept: [3, 1, 2]
```

The initialiser, when given, must be an array. Any other value is rejected; it
is not silently treated as an empty set.

A Set is a value, not a shared handle: assigning one to another name copies it,
and so does passing it to a function. See `arrays.md` for the same rule.

## Membership

Deduplication and `has` use the same value equality as `==`. Scalar elements
(int, decimal, dec, string, bool) have a fingerprint and compare by value.
Compound elements do not: they never compare equal to anything, so they are
always admitted and `has` on a compound is always false. That is long-standing
behavior, not a new limitation, and it is what the original pairwise scan
concluded for them.

Insertion order is observable through `toArray()` and survives `delete`.

## Methods

| Method | Contract |
| --- | --- |
| `size()` | Element count. |
| `has(value)` / `contains(value)` | Membership test. |
| `add(value)` | Inserts when absent. Returns the receiver, so calls chain. |
| `delete(value)` / `remove(value)` | Returns true when something was removed, false otherwise. |
| `clear()` | Empties the set. Returns null. |
| `union(other)` | New Set: this set's elements, then `other`'s that are absent. |
| `intersection(other)` | New Set of the elements present in both. |
| `toArray()` | Array of the elements in insertion order. |
| `toString()` | Display form of the set. |

`size`, `clear`, `toArray` and `toString` take zero arguments. `union` and
`intersection` build a new Set and leave both operands unchanged.

## Evaluation and errors

Arity is checked before any argument is evaluated, so a call with the wrong
number of arguments runs none of their side effects. A user `throw`, a runtime
failure or a control-flow result raised while evaluating an argument reaches the
caller unchanged.

| Failure | Diagnostic |
| --- | --- |
| Wrong arity on any method | catchable `TypeError` / `SZ4002` |
| `new Set(values)` with a non-array initialiser | catchable `TypeError` / `SZ4002` |
| `union`/`intersection` given something other than a Set | catchable `TypeError` / `SZ4002` |
| Unknown Set member | catchable `ReferenceError` / `SZ4001` |

An unknown member was previously reported as a `TypeError`, which made Set the
only collection whose `kind` could not be used to tell "you called something
that does not exist" from "you called it wrongly". It is now `ReferenceError`,
matching arrays, strings, Random, DateTime and Task.
