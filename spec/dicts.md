# Dicts

Normative contract for dict values and their built-in methods.

Every rule here was derived from the current implementation and is pinned by a
test in `tests/07_dicts.sz`, `tests/74_dict_slot_e2e.sz`,
`tests/unit_dict_set_errors.sz` or `tests/runtime_outcome.rs`.

## Declaration and value semantics

A dict is declared with the key and value types after the name:

```serez
let ages <string, int> = ({"Ana", 25}, {"Bob", 30});
```

A dict is a value, not a shared handle: assigning one to another name copies it,
and so does passing it to a function. A mutating method changes only the binding
it was called on. See `arrays.md`, which states the same rule for arrays.

## Keys and ordering

Entries keep insertion order. `keys()`, `values()` and `toArray()` all report
that order, and `toArray()` pairs them as `[[k1, v1], [k2, v2], …]`.

Reading a missing key yields `null` rather than failing:

```serez
out ages["nobody"];   // null
```

Use `??` to supply a default. This is deliberate and load-bearing for the
official packages; it is not an oversight.

## Writing

`d[key] = value` and `d.Add({key, value})` perform the same insert: they replace
the value when the key is present and append otherwise. `Add` returns null.

`Remove(key)` drops every entry under that key and returns null. Removing a key
that is not present is a no-op, not an error.

`RemoveAll()` and `clear()` are the same operation and empty the dict.

## Declared types

When the key or value type is not `any`, `Add` checks the value against it and
rejects a mismatch without changing the dict. The declared types are read from
the receiver *after* the arguments are evaluated, so an argument's side effects
still happen before a type is rejected.

## Reading

| Method | Contract |
| --- | --- |
| `keys()` / `toList()` | Array of keys in insertion order. |
| `values()` | Array of values in insertion order. |
| `toArray()` | Array of `[key, value]` pairs. |
| `length()` | Entry count. |
| `toString()` | Display form of the dict. |

All five take zero arguments.

## Evaluation and errors

Arity is checked before any argument is evaluated, so a call with the wrong
number of arguments runs none of their side effects. A user `throw`, a runtime
failure or a control-flow result raised while evaluating an argument reaches the
caller unchanged.

| Failure | Diagnostic |
| --- | --- |
| Wrong arity on any method | catchable `TypeError` / `SZ4002` |
| `Add` argument that is not an entry literal | catchable `TypeError` / `SZ4002` |
| Key or value rejected by the declared type | catchable `TypeError` / `SZ4002` |
| Unknown dict member | catchable `ReferenceError` / `SZ4001` |

A reader that takes no arguments no longer ignores extra ones, and a reader
whose receiver is not a dict reports it instead of answering with empty data.
