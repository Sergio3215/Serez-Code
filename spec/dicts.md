# Dicts

Normative contract for dict values and their built-in methods.

Every rule here was derived from the current implementation and is pinned by a
test in `tests/07_dicts.sz`, `tests/74_dict_slot_e2e.sz`,
`tests/unit_dict_set_errors.sz`, `tests/unit_dict_dot_access.sz`,
`tests/unit_dict_dot_assign.sz`, `tests/fetch_dot_access.rs` or
`tests/runtime_outcome.rs`.

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
// runtime-error-example: every line here is one of the failing shapes
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
// fragment: continues the `ages` dict declared above
out ages["nobody"];   // null
```

Use `??` to supply a default. This is deliberate and load-bearing for the
official packages; it is not an oversight.

## Reading a key

A key is read in either of two forms, and they are two ways of writing one
operation rather than two operations:

```serez
let dic <string, any> = ({"name", "Sergio"});
out dic["name"];   // Sergio
out dic.name;      // Sergio
```

`dic.name` reads the key named by the identifier **exactly as written**. There
is no case folding and no separator translation: `dic.firstName` reads the key
`"firstName"` and nothing else.

Both forms answer `null` for a key the dict does not hold, and both leave the
dict unchanged — reading a missing key does not insert it.

The two forms mix freely in one expression, which is what makes nested data
readable:

```serez
let inner <string, any> = ({"name", "Inner"});
let dic <string, any> = ({"user", inner});
out dic["user"]["name"];
out dic.user.name;
out dic["user"].name;
out dic.user["name"];
```

Optional chaining reaches the same lookup, so `d?.key` and `d?.a?.b` read keys
and answer `null` rather than failing.

**Brackets are not redundant.** `d[expr]` takes any expression, so a computed
key, a key held in a variable, and a key that is not a valid identifier are
reachable only that way. A header name like `"x-probe"` is the ordinary case:
`d.x-probe` is a subtraction, not a key.

**No key name is reserved.** Resolution is driven by the receiver, and the
receiver here is a dict, so `d.k` is the key `"k"` whatever `k` is called — the
method table is not consulted for a property access at all:

```serez
let dic <string, any> = ({"keys", "valor"});
out dic.keys;      // valor — the key
out dic["keys"];   // valor — the same key
out dic.keys();    // [keys] — the method
```

`d.k` and `d.k()` are told apart by whether the call was written with
parentheses, which the parser records on the node. Nothing resolves by asking
whether a name happens to be a method, so data arriving from elsewhere cannot
change which code a program runs. This is **DEC-M12-001**.

An unknown *call* is still an error: `d.notAMethod()` reports an unknown method
rather than answering `null`, so a mistyped call does not pass silently.

Structured data from the network is read the same way and by the same code.
`fetch` returns the body as a string; `JSON.parse` turns it into an ordinary
dict, so `response.name` and `response["name"]` are the rules above and not a
separate feature. Pinned by `tests/fetch_dot_access.rs`.

## Writing

`d[key] = value` and `d.Add({key, value})` perform the same insert: they replace
the value when the key is present and append otherwise. `Add` returns null.

`Remove(key)` drops every entry under that key and returns null. Removing a key
that is not present is a no-op, not an error.

`RemoveAll()` and `clear()` are the same operation and empty the dict.

**`d.key = value` is `d["key"] = value`.** Both spellings reach the same
writer, so everything stated above about writing holds for either — the
declared value type is enforced, and a key that is not present is created:

```serez
let dic <string, any> = ({"name", "Sergio"});
dic.name = "Jonathan";
dic.newKey = 42;
out dic["name"];     // Jonathan
out dic["newKey"];   // 42
```

The same holds through a nested path, where all four spellings write the key the
bracket form writes:

```serez
let inner <string, any> = ({"name", "Old"});
let dic <string, any> = ({"user", inner});
dic.user.name = "New";
out dic["user"]["name"];   // New
```

Whether a write is permitted depends on the mutability of the binding and on the
declared types — never on which spelling was used. This is **DEC-M12-002**.

`const` currently rejects **rebinding** the name and does not freeze the value
it binds, uniformly for arrays, dicts and mutating methods: `const d; d["k"] = v`
and `const d; d.k = v` both write, and both would stop together if that changed.
Whether `const` should freeze the value is **DEC-M12-003**, open, and is not a
dictionary question.

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
| Unknown dict method *call* — `d.notAMethod()` | catchable `ReferenceError` / `SZ4001` |
| Unknown dict *property* — `d.notAKey` | not an error: `null`, as for `d["notAKey"]` |
| `d.key = value` rejected by the declared value type | catchable `TypeError` / `SZ4002`, the same as `d[key] = value` |

A reader that takes no arguments no longer ignores extra ones, and a reader
whose receiver is not a dict reports it instead of answering with empty data.
