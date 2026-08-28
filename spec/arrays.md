# Arrays

Normative contract for array values and their built-in methods.

Every rule here was derived from the current implementation and is pinned by a
test in `tests/unit_arrays.sz`, `tests/unit_array_errors.sz`,
`tests/err_array_unknown_method.sz` or `tests/runtime_outcome.rs`.

## Value semantics

An array is a value, not a shared handle. Assigning one array to another name
copies it, and passing one to a function copies it:

```serez
let a = [1, 2, 3];
let b = a;
b.push(4);        // a is still [1, 2, 3]

fn void mutate([int] arr) { arr.push(99); }
mutate(a);        // a is still [1, 2, 3]
```

A mutating method therefore changes only the binding it was called on. Writing
back into a field, an index or a captured variable is the receiver-writeback
machinery described in `variables.md`, not aliasing.

## Element types

An array is either untyped or carries a declared element type (`[int]`,
`[string]`, `[decimal]`, `[T?]`). A declared type is enforced on the mutators
that insert values — `push` and `unshift` — and a rejected insert leaves the
array unchanged. It is not re-checked on values that arrive by other routes.

`filter` and `slice` preserve the receiver's element type. `map` and `flat`
produce untyped arrays, because neither can know the callback's or the nested
arrays' result type.

## Length

`length` is available both as a property (`a.length`) and as a zero-argument
call (`a.length()`). Both return the element count as an `int`.

## Mutating methods

These change the receiver in place and are the only methods that do.

| Method | Contract |
| --- | --- |
| `push(value)` | Appends. Returns null. |
| `pop()` | Removes and returns the last element. Empty receiver is an error. |
| `shift()` | Removes and returns the first element. Empty receiver is an error. |
| `unshift(value)` | Inserts at index 0. Returns null. |
| `remove(index)` | Removes and returns the element at `index`. |
| `sort([order \| comparator])` | Sorts in place and returns the receiver. |
| `reverse()` | Reverses in place and returns the receiver. |

`remove` on an empty array returns null instead of failing. This predates the
bounds check and is deliberately preserved; on a non-empty array an index below
zero or at/above the length is an error. Ecosystem code and the conformance
suite depend on the empty case.

`sort` accepts nothing, the string `"asc"`, the string `"desc"`, or a comparator
function. Without a comparator the array must be homogeneous — all `int`, all
`decimal`, all `dec`, or all `string`. A comparator receives two elements and
must return a number; a positive result orders the first after the second.

A comparator that fails — by returning a non-number, by raising a runtime error,
or by throwing — leaves the receiver exactly as it was. A failed sort never
publishes a half-ordered array.

## Non-mutating methods

| Method | Contract |
| --- | --- |
| `map(callback)` | New untyped array of results. |
| `filter(callback)` | New array of elements the callback kept, same element type. |
| `reduce(callback)` / `reduce(initial, callback)` | Folds left to right. |
| `find(predicate)` | First matching element, or null. |
| `findIndex(predicate)` | Index of the first match, or `-1`. |
| `every(predicate)` | True on an empty array. Stops at the first false. |
| `some(predicate)` | False on an empty array. Stops at the first true. |
| `indexOf(value)` | First index by value equality, or `-1`. |
| `includes(value)` / `contains(value)` | Value-equality membership. |
| `join([separator])` | Separator defaults to `","`. |
| `slice([start[, end]])` | New array over the selected range. |
| `flat([depth])` | Depth defaults to 1. |
| `toString()` | Display form of the array. |

`reduce` takes the initial value **first**: `reduce(initial, callback)`, which is
the opposite of JavaScript's argument order. With one argument the first element
seeds the accumulator and an empty receiver is an error. The callback receives
`(accumulator, element)` and no index.

`map` and `filter` pass `(element, index)` when the callback declares two or
more parameters, and `(element)` otherwise. `find`, `findIndex`, `every` and
`some` always pass only `(element)`.

`slice` clamps both bounds to `[0, length]`; a negative bound counts backward
from the end and then clamps at zero. Reversed bounds yield an empty array.

`flat` clamps a negative depth to 0, which flattens nothing and returns a copy.
Only a non-integer depth is rejected.

## Evaluation order and errors

Three ordering rules are normative:

1. **Arity is checked before any argument is evaluated.** A call with the wrong
   number of arguments never runs their side effects: `a.pop(f())`,
   `a.reverse(f())` and `a.sort(cmp, f())` all reject without calling `f`.
2. **Callbacks are validated before iteration.** An empty receiver does not hide
   an invalid callback: `[].find(1)` is a type error, not a silent null.
3. **Nested outcomes propagate unchanged.** A user `throw`, a runtime failure or
   a control-flow result produced while evaluating an argument or running a
   callback reaches the caller as itself, keeping its own structured payload.

| Failure | Diagnostic |
| --- | --- |
| Wrong arity | catchable `TypeError` / `SZ4002` |
| Argument of the wrong type | catchable `TypeError` / `SZ4002` |
| Value rejected by a declared element type | catchable `TypeError` / `SZ4002` |
| Callback argument that is not a function | catchable `TypeError` / `SZ4002` |
| Comparator returning a non-number | catchable `TypeError` / `SZ4002` |
| `sort` without a comparator on a mixed array | catchable `TypeError` / `SZ4002` |
| `sort` order string other than `"asc"`/`"desc"` | catchable `RangeError` / `SZ4000` |
| `pop`/`shift` on an empty array | catchable `IndexOutOfBounds` / `SZ4003` |
| `remove` index out of range | catchable `IndexOutOfBounds` / `SZ4003` |
| Unknown array member | catchable `ReferenceError` / `SZ4001` |

Invalid arguments are never silently converted. `slice("x")`, `flat("x")` and
`sort("ascending")` fail; they do not fall back to index 0, depth 1 or ascending
order.

## Known inconsistency

`remove` on an empty array returns null while every other out-of-range index is
an `IndexOutOfBounds` error. Aligning them would be a breaking change with no
migration path for callers that rely on the null, so it is recorded here rather
than fixed silently. See `compatibility.md` for how such a change would have to
be staged.
