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

The annotation is **required**, and this is more consequential than it looks: a
dict literal is not an expression. `let name <K, V> = ( ... )` is the only place
the grammar accepts one, so every other position fails with catchable
`TypeError` / `SZ4002` and the message "Entry literal {k,v} is only valid as an
argument to a dict method" — which names the entry rather than the literal and
so reads as a puzzle. Measured, all of these fail:

```serez
let d = ({"a", 1});                 // unannotated `let`
f(({"a", 1}));                      // an argument
return ({"a", 1});                  // a return value
let a = [({"a", 1})];               // an array element
class C { public C() { this.d = ({"a", 1}); } }   // a field initialiser
d = ({"b", 2});                     // reassigning, even an annotated binding
```

To put a dict anywhere else, build it in an annotated `let` and use that
binding:

```serez
class C {
    public C() {
        let seed <string, int> = ({"a", 1});
        this.d = seed;
    }
}
```

An array has no such restriction: `[1, 2]` is an ordinary expression and works
in every one of those positions.

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
