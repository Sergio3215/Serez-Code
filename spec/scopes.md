# Scopes and name resolution

Normative contract for where a name comes from.

Every rule here was derived by probing the running implementation. One of them —
the first — is a property nothing in the documentation mentions and that a
reader would very likely assume the other way round.

## A name must resolve lexically

A name is valid only if lexical structure accounts for it. It must come from:

- a **parameter** of the enclosing function or method;
- a declaration in the **same scope**;
- a declaration in an **enclosing lexical scope**, including through a closure;
- a **top-level** declaration of the file, which is visible everywhere in it,
  before or after the point of use;
- a **builtin** — `parseInt`, `assert`, `this`, `super` — or a runtime namespace.

A name that none of those accounts for is a **fatal** `SZ8000` from the semantic
phase (`errors.md`). The program does not run and `sz` exits `1`.

```serez
// runtime-error-example: the semantic phase rejects the program
fn int leaky() { return secret; }
fn int caller() { let secret = 42; return leaky(); }
out caller();
```

### There is no dynamic resolution

Until 10.0.0 the program above **printed 42**. A call pushes its frame onto the
same scope stack the caller is using and lookup walked every frame, so a function
saw the locals of whoever called it — and the same function was valid or invalid
depending on the caller:

```serez
// runtime-error-example: rejected now; this is what it used to print
fn string callee() { return secret; }
fn string first()  { let secret = "from-first";  return callee(); }
fn string second() { let secret = "from-second"; return callee(); }
first();   // was "from-first"
second();  // was "from-second"
```

Both are now rejected before anything runs.

### What this does not change

**The evaluator.** `ScopeStack::lookup` still walks the frame stack, and a call
still pushes onto it. The rule is enforced by the semantic phase, which runs
before evaluation and refuses to evaluate a program it rejects — so a program
that passes behaves exactly as it did. Nothing in the evaluation model moved, and
an embedder driving `Evaluator` directly, without the phase, gets the old
behaviour.

### Where the rule does not look

A file containing any `import` is **not** analysed for names. A name may
legitimately come from another module, and the phase sees one file at a time;
`serez-ui` calls across files inside its package without importing, and every one
of those files is reached through an entry point that imports. See
`classes.md` for the same rule applied to an unresolvable parent class.

The analysis also resolves every remaining ambiguity toward "declared", so it
under-reports rather than over-reports. One case is known and deliberate: a
nested function whose body names a sibling declared further down is accepted,
because a lexical walk cannot tell a legitimate mutual recursion from a call made
too early. That program still fails at run time with `SZ4001`, exactly as before.

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
