# Scopes and name resolution

Normative contract for where a name comes from.

Every rule here was derived by probing the running implementation. One of them —
the first — is a property nothing in the documentation mentions and that a
reader would very likely assume the other way round.

## Free variables in a function resolve dynamically

A call pushes its frame onto the same scope stack the caller is using, and
lookup walks every frame from innermost to outermost. A function therefore sees
the locals of whoever called it:

```serez
fn string callee() { return secret; }

fn string first()  { let secret = "from-first";  return callee(); }
fn string second() { let secret = "from-second"; return callee(); }

first();   // "from-first"
second();  // "from-second"
```

`callee` has no `secret` of its own and none is declared at the top level. It
resolves to whichever caller is on the stack. This is **dynamic scoping**.

With the name bound nowhere on the stack it is still an error — `ReferenceError`
/ `SZ4001` — so this is dynamic resolution, not an implicit global.

### What this costs

- A misspelled variable inside a function does not reliably fail. It fails only
  when no frame anywhere up the call stack happens to bind that name.
- Renaming a local in one function can silently change what a different
  function reads.
- Whether a call is correct depends on who calls it, so a function cannot be
  understood from its own text.
- `sz --check` does not flag free variables at all, so nothing catches it
  before the program runs.

### Status

This is recorded, pinned by `free_variables_in_a_function_resolve_dynamically`
in `tests/runtime_outcome.rs`, and **not** changed here. Making resolution
lexical means giving a call its own scope stack and giving functions an
explicit captured environment — a change to the core evaluation model, not a
local fix, and one that could change the behavior of any program in the
ecosystem that relies on it, deliberately or not. It needs the process in
`compatibility.md` and a decision that is not the implementer's to make alone.
The pin exists so the change cannot happen by accident in either direction.

## Closures are lexical

Closures are separate machinery and behave the way the documentation says. A
closure captures a **cell**, not a value: it sees writes made after it was
created, and its own writes persist.

```serez
fn any counter() {
    let n = 0;
    return () => { n = n + 1; return n; };
}
let c = counter();
c(); c(); c();          // 1, 2, 3

let captured = 10;
let read = () => { return captured; };
captured = 20;
read();                 // 20
```

A `for` counter is captured **fresh per iteration**: three closures made in a
three-iteration loop return 0, 1 and 2. A variable declared outside a `while` is
one shared cell across iterations.

A lambda inside a method sees `this`.

## Blocks

A block introduces a scope. Declarations inside it shadow outer ones and do not
escape:

```serez
let x = "global";
{
    let x = "block";    // shadows
}
// x is "global" again
```

The same holds for an `if` body and for a `for` initializer: neither `y`
declared in an `if` block nor `i` declared in a `for` header is visible after
the statement.

Re-declaring a name in the same scope is accepted and shadows the earlier
binding. `const` bindings reject assignment.

## Globals

A top-level `let` is a global. A function can read **and write** globals:

```serez
let counter = 0;
fn void bump() { counter = counter + 1; }
bump(); bump();         // counter is 2
```

## No hoisting

A function must be declared before the statement that calls it. Calling a
function declared later in the file is `ReferenceError` / `SZ4001`, the same as
any other unbound name.

## Memory

Each frame owns a watermark into the scope arena, and leaving the frame releases
everything allocated inside it — the Flash Scope model described in the README.
A value returned out of a frame is promoted before the watermark resets; a value
captured by a closure lives in a cell that outlives the frame. Copying a value
out bounds its recursion at 500 levels; see `limits.md` and `values.md`.
