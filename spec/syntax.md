# Syntax

Normative contract for the shapes that parse.

`lexical-grammar.md` covers tokens — identifiers, numeric literals and their
bases, strings, comments and the `SZ1xxx` failures. This document covers what
those tokens may be arranged into. Every entry was checked by feeding the form
to the running binary and recording whether it parsed.

Where a form that reads as obviously valid does **not** parse, that is stated
here rather than left to be discovered.

## Statements

| Form | Notes |
| --- | --- |
| `let name = expr;` | `const` for a binding that rejects assignment |
| `let name [T] = expr;` | typed array; `T` is a **keyword** |
| `let name <K, V> = expr;` | typed dict; both **keywords** |
| `let [a, _, b] = expr;` | array destructuring, `_` skips, `...rest` collects |
| `let {x, y} = expr;` | dict destructuring by key name |
| `name = expr;` | assignment; also `a[i] = e`, `o.f = e`, `o.f[i] = e` |
| `name += expr;` | also `-= *= /= %=`, and `name++` / `name--` |
| `if (cond) { }` | `else if` chains, `else { }` |
| `while (cond) { }` | `do { } while (cond);` |
| `for (let i = 0; c; step) { }` | classic three-part header |
| `for (let x in iterable) { }` | iteration; the `let` is **required** |
| `label: for (…) { }` | then `break label;` / `continue label;` |
| `switch (v) { case a, b: { } default: { } }` | multiple values per case; `default` optional |
| `try { } catch (e) { } finally { }` | either `catch` or `finally` may be omitted |
| `throw expr;` | |
| `return expr;` · `break;` · `continue;` | |
| `out expr;` | writes to stdout |
| `fn T name(params) { }` | `fn* T name()` declares a generator, with `yield` |
| `class C { }` · `interface I { }` · `enum E { }` | see below |
| `native fn T name();` | declares a runtime-provided function |
| `import "path";` | top level only in practice — see `modules.md` |
| `export <declaration>` | wraps `let`, `fn`, `class`, `interface`, `enum`, `native` |
| `use permissions { A, B }` | see `security.md` |
| `unsafe { }` | see `security.md` |
| `{ }` | a bare block introduces a scope |

Semicolons are **optional**. A statement ends at a newline just as well.

### A keyword is never an identifier

The 50 reserved words are listed in `lexical-grammar.md`. None of them can
name a variable, function, parameter, class or field. Three read like
ordinary names and are reached for by accident — `out`, `get` and `set`:

```serez
// parse-error-example: keywords cannot be identifiers
let out = compute();          // PARSE ERROR — `out` is the output statement
fn any get() { return 1; }    // PARSE ERROR — `get` opens a getter
```

### Braces are mandatory

`if`, `else`, `while`, `for` and `do` take a **block**, never a single
statement:

```serez
if (ready) out 1;          // PARSE ERROR
if (ready) { out 1; }      // fine
if (a) { } else out 1;     // PARSE ERROR
```

There is no brace-less body anywhere in the language.

### `for-in` requires the `let`

```serez
for (item in items) { }        // PARSE ERROR
for (let item in items) { }    // fine
```

### Declarations nest

`fn`, `class`, `interface` and `enum` are all valid inside a function body or a
block. What that costs at run time is in `scopes.md` and `modules.md`: a
function or `let` declared inside a frame leaves with it, a class does not.

### `return` and `break` outside their context

Both are accepted by the parser and rejected when reached, as an
`InvalidControlFlow` outcome printed as `❌ FLASH SCOPE ERROR`. That message
carries no `SZ` code — one of the remaining unstructured diagnostics noted in
`errors.md`.

## Expressions

| Form | Notes |
| --- | --- |
| literals | see `lexical-grammar.md`; `1.5m` is a `dec` |
| `"text {expr}"` | interpolation; `\{` escapes a literal brace |
| `[1, 2]` | array literal, optionally typed by the binding |
| `({"k", v}, {"k2", v2})` | dict literal — see the restriction below |
| `new C(args)` · `new I({ f: v })` | class and interface construction |
| `new Set([…])` | |
| `f(args)` · `f(...arr)` | call; spread expands an array into arguments |
| `o.field` · `o.method(args)` · `o?.field` | member access, optional chaining |
| `a[i]` | index |
| `x => expr` · `(a, b) => expr` · `() => { }` | lambdas |
| `match subj { pat => expr, _ => expr }` | see below |
| `cond ? a : b` · `a ?? b` | |
| `a \|> f` | pipe: calls `f(a)` |
| `&x` · `*p` | address-of and dereference; writing needs `unsafe` |
| `sizeof(T)` | takes a **type keyword** — see `operators.md` |

Operator precedence, associativity and the accepted operand types are in
`operators.md`.

### A lambda parameter takes no type and no default

```serez
// parse-error-example: neither form is accepted on a lambda
let f = (int a) => { return a; };    // PARSE ERROR — no type annotation
let f = (a = 1) => { return a; };    // PARSE ERROR — no default
```

```serez
let f = (a) => { return a; };        // fine
let f = a => a * 2;                  // fine — one parameter needs no parens
```

Annotations and defaults exist on `fn` declarations, methods and constructors,
not on lambdas. See `types.md` and `functions.md`.

### A dict literal needs a dict context

`{k, v}` is an *entry* literal, and a parenthesised sequence of them is a dict.
It is only valid where a dict is expected — an annotated binding, or an argument
to a dict method:

```serez
let d <string, int> = ({"a", 1}, {"b", 2});    // fine
let d = ({"a", 1});                            // TypeError / SZ4002:
                                               // "Entry literal {k,v} is only
                                               // valid as an argument to a dict
                                               // method"
let d = {"a": 1};                              // PARSE ERROR — no such form
```

There is no JSON-style `{key: value}` object literal. See `dicts.md`.

### `match`

```serez
let label = match n {
    1 | 2 | 3      => "low",
    x if x > 100   => "huge",
    x              => "other: " + x,
    _              => "none",
};
```

Arms are `pattern => expression`, separated by commas, and a **trailing comma is
allowed**. Patterns may be literals, `|`-separated alternatives, a binding name,
or `_`. A binding may carry an `if` guard. The subject may be parenthesised or
not.

## Trailing commas

Allowed in exactly three places, and a parse error everywhere else:

| Allowed | Rejected |
| --- | --- |
| `match` arms | array literals — `[1, 2,]` |
| `enum` variants | call arguments — `f(1,)` |
| array destructuring — `let [a, b,]` | parameter lists — `fn f(int a,)` |
| | dict literals — `({"a", 1},)` |

## Classes, interfaces, enums

```serez
abstract class Shape { }
sealed class Final { }
public class Circle : Shape {
    radius: decimal = 0.0;               // declared field with a default

    public Circle(decimal r) { this.radius = r; }
    public decimal area() { return 3.14159 * this.radius * this.radius; }
    private decimal helper() { return 1.0; }
    public static string tag() { return "Circle"; }

    public get decimal diameter() { return this.radius * 2.0; }
    public set diameter(decimal d) { this.radius = d / 2.0; }
}

interface Point { x: int; y: int; }
let p = new Point({ x: 1, y: 2 });

enum Priority { Low, Medium, High, }
```

`:` introduces the parent class. A getter is `public get T name()`, a setter is
`public set name(T v)` — the type follows `get`, not `set`. Interface bodies
hold `name: T;` fields only, and an instance is built from a `{ }` map. Fields
declared with `name: T = default;` supply a default value; the type is not
enforced afterwards — see `types.md`.

Construction, dispatch, `super`, abstract and sealed rules are normative in
`classes.md`.

## Comments

`// line` and `/* block */`, per `lexical-grammar.md`. **Block comments do not
nest**: `/* a /* b */ c */` is a parse error, because the first `*/` closes it
and `c */` is then loose text.

## Depth

Nesting and operator chains are bounded — a program past the ceiling is a parse
error, not a crash. See `limits.md`.

## Known gaps

- No brace-less statement bodies.
- No JSON-style object literal, and dict literals need a dict context.
- No type annotation on a lambda parameter or on a scalar `let`.
- Trailing commas are accepted inconsistently across the four list forms.
- Block comments do not nest.
- `return`/`break` outside their context report without a diagnostic code.

See `lexical-grammar.md` for tokens, `operators.md` for precedence,
`types.md` for annotations, `control-flow.md` for `for-in` semantics and
`classes.md` for the class model.
