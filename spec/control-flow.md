# Control flow

Normative contract for control flow. Normative words such as "must" describe
compatibility requirements.

Every rule here was derived by probing the running implementation. Two of them
are hazards rather than designs, and are marked as such rather than smoothed
over: a `match` that matches nothing, and what `fn*` actually does.

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

## `if`, `while`, `do-while` and C-style `for`

All four take a **block**; there is no brace-less body anywhere in the language
(`syntax.md`). `else if` chains. `while` tests before the first iteration, so a
false condition runs the body zero times; `do-while` tests after, so it always
runs the body at least once. A C-style `for` with a false condition runs zero
times.

## Labels

A loop may carry a label, and `break label` / `continue label` act on the
labelled loop rather than the innermost one:

```serez
outer: for (let i = 0; i < 3; i = i + 1) {
    for (let j = 0; j < 3; j = j + 1) {
        if (i == 1 && j == 1) { break outer; }
    }
}
```

`continue label` restarts the labelled loop's next iteration, abandoning the
rest of the inner one.

## `switch`

```serez
let subject = 2;
switch (subject) {
    case 1: { out "one"; }
    case 2: { out "two"; }
    default: { out "neither"; }
}
```

The subject is evaluated **once**. Case bodies are blocks. There is **no
fallthrough**: exactly one body runs, and `break` is not needed to stop the next
case from running. `default` runs only when no case matched, wherever in the
statement it is written — writing it first does not pre-empt a matching case
below it. When nothing matches and there is no `default`, the statement does
nothing and reports nothing.

## `match`

`match` is an **expression**, usable wherever a value is — a return, an array
element, an argument:

```serez
let n = 2;
let label = match n {
    0            => "zero",
    1 | 2 | 3    => "low",
    x if x < 0   => "negative",
    other        => "other: " + other,
};
```

The subject is evaluated once. Arms are tried **in order** and the first match
wins, so a later duplicate pattern is unreachable. The pattern forms are: a
literal; an OR of literals (`1 | 2 | 3`); an enum variant (`Color.Red`); a bare
identifier, which binds the subject and matches anything; and `_`, which matches
anything and binds nothing. Any arm may carry a guard (`x if x < 0 =>`); a guard
that evaluates falsy moves on to the next arm. A binding introduced by a pattern
is scoped to its own arm and does not leak.

### A `match` that matches nothing yields `null`

There is no exhaustiveness check. When no arm matches and there is no `_` or
binding pattern, the expression evaluates to `null` and nothing is reported:

```serez
// runtime-error-example: nothing is reported — that is the hazard
let r = match 99 { 1 => "a" };   // r is null, exit code 0
```

That `null` is indistinguishable from an arm that legitimately returned null.
It is recorded here as a hazard, not as a design statement. Making it an error
would be a breaking change for any code that relies on the null, and needs the
process in `compatibility.md`.

## `try` / `catch` / `finally`

`finally` runs on every exit from the `try`: normal completion, a caught throw,
a caught runtime error, `break`, `continue`, and `return` — in the `return` case
before the function actually returns. A `try` with only a `finally` and no
`catch` runs the `finally` and then propagates whatever was in flight.

What `catch (e)` binds depends on what failed:

| Failure | `e` is |
| --- | --- |
| A user `throw` | the thrown value itself, unchanged — a string stays a string |
| A recoverable runtime error | an `Error` instance carrying `code`, `kind`, `message`, `span`, `stack` and `notes` |

A **fatal** failure is not caught at all: a permission denial, a resource
ceiling and the other non-recoverable categories in `errors.md` abort the
program with the `catch` never running.

Two rules about `finally` decide precedence, and both follow the same principle
— the `finally` is last, so it wins:

- A `throw` from a `finally` **replaces** the failure that was in flight.
- A `return` in a `finally` **overrides** a `return` from the `try`.

## Generators

`fn*` declares a generator and `yield` adds a value to it. Calling one does
**not** produce a lazy iterator: the body runs to completion immediately and the
call returns an ordinary array of everything it yielded.

```serez
fn* count(int n) {
    let i = 0;
    while (i < n) { yield i; i = i + 1; }
}
count(4);            // [0, 1, 2, 3] — an array, already fully built
```

The result is an array like any other, so it iterates, indexes and copies like
one. A generator that yields nothing returns an empty array.

The consequence is worth stating plainly, because the `fn*`/`yield` syntax is
borrowed from languages where it means the opposite: **an unbounded generator
never returns.** `fn* forever() { while (true) { yield i; } }` runs until the
process is stopped, accumulating into memory the whole time. Measured: no
result after 20 seconds. There is no ceiling on the collected values, and that
absence is deliberate — `limits.md` records it under "What is not limited",
together with the measured cost of about 160 bytes per yielded value and the
reason a limit was considered and not added.

## Conformance evidence

- `tests/unit_foreach_edge.sz`: value-copy, scope, closure, snapshot, control
  flow and invalid-iterable behavior.
- `tests/unit_destructuring.sz`: array-pattern iteration and invalid row types.
- `tests/runtime_outcome.rs`: stable `SZ4002` payload, catchability, scope
  cleanup and user-throw propagation.

## Coverage boundary

Nothing in this document's scope remains unaudited. What is deliberately *not*
frozen here: the exact wording of any diagnostic (`compatibility.md` never
promises wording), and the interaction of `yield` with `try`/`finally`, which
has no test and was not probed.
