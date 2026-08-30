# Modules and imports

Normative contract for `import` and `export`.

Every rule here was checked against the running implementation. Where a behavior
is a consequence of the design rather than a decision, that is said so.

## What `import` does

```serez
import "./lib/math.sz";
import "lib/greet";
import "mypkg";
```

`import` takes a **string literal** and nothing else. It resolves the string to
a file, parses it, and **executes it** — the module's top-level statements run
at the point of import — then merges the names it declared into the importing
program's namespace.

There is no namespace object, no aliasing and no selective import. `import x
from y`, `import { a, b }` and `import * as m` do not parse. Everything a module
makes visible arrives as a bare global name.

Executing an arbitrary file is the widest capability in the language, and it is
gated by **no permission entry**. See "Lockdown" below and `security.md`.

## Path resolution

The string is tried against these bases, in order, and the first hit wins:

1. the directory of the **importing file**;
2. the process working directory;
3. `<cwd>/packages/` — where `sz install` puts local packages;
4. `$SEREZ_HOME`, if set;
5. the directory of the `sz` executable;
6. `$SEREZ_PACKAGES`, or `~/.serez/packages/` when it is unset.

Within each base, four names are tried in this order:

| Candidate | For `import "pref"` |
| --- | --- |
| `<base>/<path>.sz` | `pref.sz` |
| `<base>/<stem>.szx` | `pref.szx` |
| `<base>/<stem>/index.sz` | `pref/index.sz` |
| `<base>/<stem>/index.szx` | `pref/index.szx` |

So `.sz` wins over `.szx` in the same directory, a file wins over a directory,
and a *closer base* beats a better-matching name in a farther one. The `.sz`
extension is optional in the import string and is added when absent.

`.szx` is serez-ui's JSX dialect. It is translated to `.sz` source before
parsing, and its own relative imports resolve against the `.szx` file's
directory. A `.szx` the translator cannot handle is an `ImportError`, below.

The resolved path is **canonicalized** — symlinks and `..` resolved — before it
is recorded, so two spellings of the same file are the same module.

## Export visibility

Visibility is decided per module, and it is **all-or-nothing**:

- A module that uses `export` **at least once** exposes only what it exported.
  Everything else it declared at its own top level is removed when it finishes
  loading.
- A module that uses `export` **nowhere** exposes everything it declared. This
  is not an oversight to route around; it is the compatibility path for modules
  written before `export` existed.

`export` wraps a declaration — `let`, `fn`, `class`, `interface`, `enum` or a
native declaration — and all of them behave the same way:

```serez
// lib.sz
export let VALUE = "a";
export fn int visible() { return 1; }
export class Visible { public Visible() { this.x = 1; } }

fn int hidden() { return 99; }        // removed after loading
let secret = "hidden";                // removed after loading
class Secret { public Secret() { } }  // removed after loading
```

After `import "./lib.sz"`, `VALUE`, `visible()` and `new Visible()` all work,
and `hidden`, `secret` and `Secret` are `ReferenceError` / `SZ4001`.

### Only the module's own declarations are candidates for removal

Cleanup looks at the top-level statements of *that module's* parse tree.
Anything a **nested** import registered is left alone. That is deliberate, and
it exists because the naive version was broken: a component file that imported a
sibling and exported only its own class was deleting the sibling on the way out,
which made composing components across files impossible unless the entry file
also imported every transitive dependency.

The consequence is that **exports leak transitively**:

```serez
// middle.sz
import "./lib.sz";
export fn string viaMiddle() { return visible(); }
```

```serez
import "./middle.sz";
viaMiddle();   // works, as intended
visible();     // ALSO works — lib.sz's export is visible here
VALUE;         // so is this
hidden();      // still hidden: it was never exported by anyone
```

A module cannot keep its dependencies' exports to itself. Importing one module
can bring in the public surface of the whole reachable graph. Treat the exported
names of everything you depend on as part of your own namespace.

## One namespace, and collisions overwrite

There is a single flat global namespace. A module that declares a name the
importing program already has **overwrites** it:

```serez
let collide = "from-main";
import "./collide.sz";     // the module has `export let collide = "from-module"`
collide;                   // "from-module"
```

The import reports every name it replaced:

```
⚠️  WARNING: importing '.../collide.sz' replaced 'collide', which this file
    already defined. The module's definition wins from here on — rename one of
    them, or move the import above your own declaration if the replacement is
    what you meant.
```

The rule itself is unchanged: the namespace is still flat, the module's
definition still wins, and the exit code is unaffected. Until this cycle the
overwrite was **silent**, which is what made it the main practical hazard of the
module system. The warning was measured before being kept — across the 483-test
core suite and all eight official packages the collision count is **zero**, with
a probe verified to fire on a real collision — so it can only speak when
something genuinely collided, and a clean import stays quiet.

Nothing warns in the other direction: a name your own file declares *after* an
import shadows the module's, and that is ordinary shadowing rather than a
surprise. Together with transitive leaking this remains a hazard to design
around: prefix names that are meant to be public, and assume any name you did
not choose deliberately can be taken by a dependency.

## A module runs once

The canonicalized path is recorded the first time it loads. A second `import` of
the same file is a **no-op** — it is not re-read, re-parsed or re-executed:

```serez
// counter.sz
export let runs = 0;
runs = runs + 1;
export fn int getRuns() { return runs; }
```

```serez
import "./counter.sz";   getRuns();   // 1
import "./counter.sz";   getRuns();   // 1, not 2
```

`tests/unit_sec_import.sz` pins this.

Because the path is recorded *before* the module body runs, **import cycles
terminate**: `a.sz` importing `b.sz` importing `a.sz` loads each once, and both
sets of exports end up visible. What a cycle does not give you is ordering. The
second module runs while the first is only half-executed, so a top-level
statement that reads a value the other module has not assigned yet sees the
earlier one:

```serez
// a.sz
export let aVal = "unset";
import "./b.sz";
aVal = "set-by-A";
```

```serez
// b.sz
import "./a.sz";
export let seenFromB = aVal;   // "unset", not "set-by-A"
```

Cycles resolve; they are not made safe. Keep top-level side effects out of a
module that participates in one.

## `import` belongs at the top level

`import` parses anywhere a statement does, but only a top-level import applies
completely. Inside a function or a block, the module's `let` bindings and
functions land in that frame and disappear when it ends; only classes,
interfaces and enums — which live in registries rather than on the scope stack —
survive:

```serez
fn void load() { import "./lib.sz"; }
load();
visible();          // ReferenceError — the function went with the frame
new Visible();      // works — the class did not
```

This is a consequence of where each kind of declaration is stored, not a
feature. **Import at the top level.** A conditional or lazy import is not
supported, and writing one produces the half-loaded state above rather than an
error.

## Failure modes

| Situation | Diagnostic | Catchable |
| --- | --- | --- |
| No candidate path exists | a thrown **string** beginning `ModuleNotFound:` | yes |
| Found, but does not parse | `ImportError` / `SZ5002` | yes |
| Found, but cannot be read | `ImportError` / `SZ5002` | yes |
| A `.szx` the translator cannot handle | `ImportError` / `SZ5002` | yes |
| `import` under lockdown | `PermissionError` / `SZ6001` | yes |

The two module failures are told apart on purpose:

```serez
try { import "./nope.sz"; }
catch (e) { e; }              // "ModuleNotFound: Cannot find module './nope.sz'"

try { import "./broken.sz"; }
catch (e) { e.kind; e.code; } // "ImportError", "SZ5002"
```

**Not found** is an unstructured string throw, not a `RuntimeError`. It keeps
that shape because `tests/unit_sec_import.sz` has pinned the message form since
before structured errors existed, and changing it would break every consumer
matching on the text. It is the one module failure with no `kind` and no `code`
to read.

`SZ5001` is reserved for `ModuleNotFound` in the code table and **nothing emits
it today**. It exists so the string throw has a number to migrate to once the
message form can change; see `compatibility.md`.

A parse error inside an imported module prints the parser's own diagnostic *and*
the `SZ5002` line, and the import does not partially apply — nothing the module
declared becomes visible. A missing import is reported exactly **once**, at the
program boundary.

## Lockdown

Under lockdown — `sz --eval`, and anywhere else there is no entry file to be
relative to — `import` is refused outright:

```
❌ ERROR [SZ6001]: import is not available here — this code runs as a single
   file, with no packages and no filesystem access.
```

This is a catchable `PermissionError`. It is the only gate on `import`; in a
normal script run there is none. See `security.md`.

## URL imports

An import string starting with `http://` or `https://` is fetched over the
network with a 15-second timeout and cached at
`~/.serez/packages/<hash-of-url>.sz`. The cache file's path is the module
identity, so the same URL loads once per process and is not re-fetched on later
runs. A fetch or read failure is a `ModuleNotFound:` string throw, like a
missing local module.

Three things this does **not** do, stated plainly because the shape invites the
opposite assumption:

- there is no integrity hash, signature or pinning — the URL is the only
  identity, and what it serves can change between runs;
- the cache never expires and is never revalidated, so a URL import is
  reproducible only by accident;
- the downloaded source runs with the full rights of the `sz` process.

A URL import is strictly weaker than `sz install`, which at least records a
version. `security.md` and `packages.md` carry the same warning for installed
packages.

## Known gaps

These are limitations, not guarantees:

- no selective import, aliasing or namespacing — one flat global namespace;
- exports leak transitively; a collision overwrites, and is reported as a
  warning rather than prevented;
- no re-export declaration; the leak above is what gets used instead;
- an import inside a function or block half-applies rather than failing;
- no lockfile, integrity check or version constraint on a URL import;
- `sz --check` does not resolve imports, so nothing validates the module graph
  before the program runs.

See `packages.md` for the manifest and installation contract, `errors.md` for
the diagnostic model, and `scopes.md` for how the merged names resolve.
