# Types

Normative contract for what a type annotation means, where one may appear, and
what is checked when.

Every rule here was derived by probing the running implementation. Serez Code is
dynamically typed with **optional annotations that are enforced at runtime**.
The static checker exists but reaches very little; where the two disagree, this
document says which one you are actually relying on.

## The type keywords

Seven, and only these are keywords:

| Keyword | Values |
| --- | --- |
| `int` | 64-bit signed integers |
| `decimal` | binary floating point |
| `dec` | exact base-10, literal suffix `m` |
| `string` | UTF-8 text |
| `bool` | `true` / `false` |
| `void` | a function that returns nothing; matches `null` |
| `any` | everything, including `null` |

A **class**, **interface** or **enum** name is also usable as a type wherever an
annotation is accepted. So is `T?`, which additionally accepts `null`.

`array`, `dict`, `function` and `DateTime` are not keywords but *are* recognized
as type names by the runtime matcher and by `is`.

## Where an annotation may appear

| Position | Form | Example |
| --- | --- | --- |
| Function parameter | any type name, `T?` | `fn int f(string s, int? n)` |
| Function return | any type name, `T?` | `fn int? maybe()` |
| Constructor parameter | any type name | `public Point(int x)` |
| Array literal / `let` | `[T]`, T a **keyword** | `let a [int] = [1, 2];` |
| Dict literal / `let` | `<K, V>`, both **keywords** | `let d <string, int> = ({"k", 1});` |
| Class field declaration | `name: T = default;` | `timeout: int = 30;` |
| Interface field | `name: T;` | `interface P { n: int; }` |

Two limits that surprise people:

- **A scalar annotation on `let` does not parse.** `let x int = 5;` and
  `let n int? = null;` are parse errors. Only the `[T]` and `<K, V>` container
  forms are accepted on a binding.
- **`[T]` and `<K, V>` accept only the seven keywords.** `let arr [Base] = [];`
  is a parse error — there is no typed array of a class.

## What a declared type does

At a **call**, each supplied argument is matched against its parameter's
declared type before the body runs; a mismatch is a catchable `TypeError` /
`SZ4002` naming the parameter, the expected type and the received one. The same
happens for the value a function returns, and for a constructor's arguments.

Matching is by this table, and by nothing else:

| Declared | Accepts |
| --- | --- |
| `int` | an `int` (and a `DateField`, which behaves as one) |
| `decimal` | a `decimal` — **not** an `int`, **not** a `dec` |
| `dec` | a `dec` |
| `string`, `bool` | that type |
| `void` | `null` |
| `any` | anything |
| `T?` | a `T`, or `null` |
| `[T]` | **any array**, whatever its elements |
| a class name | an instance of **exactly** that class |
| an interface name | an instance of **exactly** that interface |
| an enum name | a variant of that enum |

### No widening

`int` does not widen to `decimal` at a parameter, and neither does `dec`:

```serez
fn decimal half(decimal d) { return d / 2.0; }
half(1);        // TypeError / SZ4002 — expected 'decimal' but received 'int'
half(1m);       // TypeError / SZ4002 — expected 'decimal' but received 'dec'
half(1.0);      // fine
```

This is the opposite of what the operators do — `1 + 1.5` is a `decimal`
without complaint. Arithmetic mixes numeric types; parameter binding does not.
Declaring `decimal` on a parameter that callers will reasonably pass integers
to is a trap; `any` is the honest annotation until this is reconciled.

### No subtyping

A declared class name matches that class and nothing else. Inheritance drives
method dispatch and field layout, but the type system does not see it:

```serez
class Base    { public Base() { this.v = 1; } }
class Derived : Base { public Derived() { this.v = 2; } }

new Derived() is Base;                       // false

fn any take(Base b) { return b.v; }
take(new Derived());                         // TypeError / SZ4002
```

Two different interfaces with identical fields are likewise incompatible. A
function meant to work across a hierarchy has to take `any`; that is what the
existing polymorphism tests do.

This is recorded as an inconsistency, not defended. Making a declared class type
accept its subclasses is a compatibility-affecting change and needs the process
in `compatibility.md`. It is pinned by
`a_declared_type_matches_exactly_and_never_a_subclass` in
`tests/runtime_outcome.rs` so it cannot move in either direction by accident.

### An unknown type name matches nothing

An annotation is any identifier followed by an optional `?`. Nothing checks that
the name exists:

```serez
fn any f(Frobnicate x) { return "reached"; }
f(1);   // TypeError / SZ4002 — expected 'Frobnicate' but received 'int'
```

The function parses, loads and is callable — and rejects every value that
reaches it. A misspelled type turns a function into one that can never succeed,
and nothing says so until a call happens.

## Where enforcement stops

Annotations are checked at the boundaries above. They are **not** checked here:

| Situation | What happens |
| --- | --- |
| `let x = 5; x = "s";` | accepted — a binding has no type to violate |
| `[int]` passed to a `[int]` parameter | accepted, and so is `[string]` |
| A class field's declared type, on assignment | accepted — `c.timeout = "str"` works |
| An interface field's type, on assignment | accepted — checked only at construction |
| A new field added by assignment | accepted — `c.brandNew = 1` creates it |

A declared class field (`timeout: int = 30;`) supplies a **default value**. Its
type is not enforced after construction, from inside the class or outside it.
Interface fields are checked when the instance is built and never again. See
`classes.md`.

## `type_of` and `is`

`type_of(x)` returns a string; `x is T` returns a boolean. They agree.

| Value | `type_of` |
| --- | --- |
| `1` | `int` |
| `1.5` | `decimal` |
| `1m` | `dec` |
| `"s"` | `string` |
| `true` | `bool` |
| `null` | `null` |
| `[1]` | `array` |
| a dict | `dict` |
| a set | `Set` |
| a lambda or function | `function` |
| an instance of `C` | `C` |
| a variant of `enum P` | `P` |

`is` uses the same matching table as a parameter: `1 is decimal` is `false`,
`1m is decimal` is `false`, everything `is any`, and a subclass `is` only its
own class.

## The static checker

`sz` runs a type checker over the parsed program before evaluating it. It is
**advisory**: findings print to stderr as `❌ TYPE ERROR [SZ3000]` and change
neither the exit code nor whether the program runs. `sz --check` prints them and
still exits 0.

It performs exactly four checks:

1. an array literal's elements against its declared element type;
2. a call's argument **count** against the function's parameters;
3. a call's argument **types** against the declared parameter types;
4. a `return` expression's type against the function's declared return type.

And it can only do them when it can infer the argument's type, which it can for:
a literal, an identifier bound by a **top-level** `let`, a call to a declared
function with a declared return type, and an array literal with a declared
element type. Everything else infers nothing, and the check is silently skipped.

The practical consequence:

```serez
let s = "x";
fn int f(int n) { return n; }
f(s);                    // caught statically, then again at runtime

fn void caller() {
    let s = "x";         // a local: the checker has no type for it
    f(s);                // runtime only
}
```

Not checked at all, statically: assignments, class and interface members, dicts,
sets, method calls, constructor calls, lambdas, and any expression whose type
the four inference rules do not cover. `SZ3000` is the only code it emits.

Treat the checker as a linter that occasionally catches a top-level mistake
early. **The runtime is what enforces the contract.**

## Known gaps

These are limitations, not guarantees:

- no widening between numeric types at a parameter, unlike in arithmetic;
- no subtyping — a declared class or interface name is an exact match;
- an unknown type name is accepted and matches nothing;
- `[T]` on a parameter accepts any array;
- declared field types are defaults, not constraints, after construction;
- no scalar type annotation on a `let`;
- no generics, no union types, no type aliases, no inference beyond the four
  rules above;
- the static checker never fails a build, and `--check` exits 0 on type errors.

See `values.md` for what a value *is*, `operators.md` for the operand types each
operator accepts, `classes.md` for construction and dispatch, and `errors.md`
for the diagnostic model.
