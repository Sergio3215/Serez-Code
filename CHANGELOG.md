# Serez-Code — Changelog

Technical record of all changes to the language, stdlib, and tooling.  
Order: most recent to oldest.

---

## [Unreleased] — maturity hardening

### `sz install` no longer fails on a declared minimum runtime

- `serez-ui` declares `"serez-code": ">= 9.17.0"` in `dependencies` to state the
  oldest runtime it supports. `sz install` treated that like any other package
  and pushed the value through the package-version identifier rules, which
  reject spaces and `>`. Running `sz install` in a `serez-ui` project therefore
  failed outright — the one official package that followed the documented
  advice was the one whose install was broken.
- `serez-code` is now a reserved key in `dependencies`: it declares the minimum
  runtime, is compared against the running version, and is never fetched. There
  is no package by that name — it is the interpreter reading the manifest.
- Accepted forms are `">= X.Y.Z"`, `"> X.Y.Z"`, `"= X.Y.Z"` and a bare `"X.Y.Z"`.
  Caret and tilde ranges are rejected rather than silently narrowed: package
  versions elsewhere are identifiers, not ranges, and accepting `^9.17.0` would
  imply a resolver that does not exist.
- `sz install serez-code` is refused with a message explaining what to write
  instead. An unsatisfied requirement names both versions and what to do.
- `spec/compatibility.md` is new: the versioning and deprecation policy that
  `spec/limits.md` and `spec/random.md` already referenced but that did not
  exist. It records the rule actually in force — including that `9.4.0` shipped
  a breaking change in a minor release after an ecosystem sweep found no
  affected code — rather than a stricter SemVer the history contradicts.

### Dict and Set stop accepting calls they cannot honour

- `new Set(5)` silently produced an empty set: a non-array initialiser was
  dropped on the floor. It is now a catchable `TypeError` (`SZ4002`).
- Every dict reader (`keys`, `values`, `toList`, `toArray`, `length`,
  `toString`) and the zero-argument Set methods (`size`, `toArray`, `clear`,
  `toString`) ignored extra arguments. They now reject them, before evaluating
  the arguments they are rejecting.
- All remaining dict diagnostics moved off stderr prints onto the structured
  channel, so `d.Add()` and a dict type mismatch are catchable and carry a code.
- An unknown `Set` member reported `TypeError` while arrays, strings, Random,
  DateTime and Task all reported `ReferenceError`. Set was the outlier; it now
  reports `ReferenceError` (`SZ4001`) like the rest, so `e.kind` can be used to
  tell "no such member" from "called wrongly".
- A dict or Set method whose receiver is not a dict or Set now says so instead
  of answering with empty data, which is how a dispatcher bug used to look.
- Unchanged: insertion order, missing-key reads returning null, `Remove` of an
  absent key staying a no-op, Set deduplication and compound-element behavior,
  `add` returning the receiver, `delete` returning a bool, and both value
  semantics. `spec/dicts.md` and `spec/sets.md` freeze the contracts.

### Array failures are structured, catchable and no longer silently ignored

- Array was the last large public surface still reporting failures by printing
  to stderr and returning an untyped sentinel. Those failures could not be
  caught: `try { a.push() } catch (e)` aborted the process instead of running
  the handler. All 21 methods now use the structured channel, so `e.code` and
  `e.kind` classify the failure without matching on wording.
- Three methods silently did something the program had not asked for.
  `slice("x")` used index 0, `flat("x")` used depth 1 and `sort("ascending")`
  sorted ascending. All three are now errors.
- Arity is validated before arguments are evaluated, so `a.pop(f())`,
  `a.reverse(f())` and `a.sort(cmp, f())` reject the call without running `f`.
- Callbacks are validated before iteration, so `[].find(1)` is a type error
  instead of a silent `null`.
- A comparator that fails leaves the receiver untouched; a failed sort never
  publishes a half-ordered array.
- `eval_str_arg` / `eval_int_arg` returned `Option`, collapsing a user `throw`
  and a nested runtime error into the same `None`. They now propagate the
  original outcome, which also fixes the same latent defect in
  `Crypto.randomBytes` and `Regex.*`; their type errors now name the call that
  rejected the argument.
- Unchanged: `remove` on an empty array still yields null, `reduce(initial,
  callback)` keeps its argument order, `sort` and `reverse` still return the
  receiver, negative `slice` indices still count from the end, negative `flat`
  depth still clamps to 0, and every valid result is byte-identical.
  `spec/arrays.md` freezes the contract.

### String padding can no longer grow without a bound

- `padStart(-1, "x")` and `padEnd(-1, "x")` cast the negative target to
  `usize` and entered effectively unbounded, quadratic growth. Padding now
  rejects negative targets with catchable `RangeError` (`SZ4000`), builds in
  linear time, uses fallible capacity reservation and applies a fatal
  10,000,000-character ceiling (`SZ6002`) before allocating the result.
- Valid Unicode and multi-character padding results remain compatible,
  including the historical `padStart` truncation direction.
- Every String method now validates arity and types structurally (`SZ4002`), an
  unknown member is `ReferenceError` (`SZ4001`), and nested runtime failures or
  user `throw` propagate unchanged. Invalid padding/types are no longer silently
  converted to zero, space or omitted bounds.
- README incorrectly claimed `.replace` changed every occurrence despite the
  implementation and regression suite defining first-only replacement. The
  guide now matches `.replace`/`.replaceAll` reality; `spec/strings.md` freezes
  the Unicode/index/error contract.

### Random no longer crashes on the complete integer domain

- `Random.int(i64::MIN, i64::MAX)` previously overflowed while calculating
  `max - min + 1`, panicking the debug host (exit 101) and reaching modulo by
  zero in wrapping builds. Width arithmetic now occurs outside `i64`, and wide
  ranges combine enough deterministic LCG output for every integer to be
  reachable.
- Seeded sequences for established ranges of width at most 2³¹ remain byte-for-
  byte compatible. Wider ranges use rejection sampling rather than the previous
  truncated 31-bit result.
- Every Random validation path is now structured and catchable: arity/types use
  `TypeError` (`SZ4002`), invalid domains use `RangeError` (`SZ4000`), and an
  unknown member uses `ReferenceError` (`SZ4001`). Nested errors and user
  `throw` propagate unchanged; arity failures do not evaluate arguments.
- The shared Tensor shape parser now reports structured type/range diagnostics,
  while product overflow and the element ceiling remain fatal resource errors.

### Task workers have an isolated, bounded runtime contract

- Task state moved from one process-global registry into an evaluator-owned
  runtime shared only with descendant workers. Independent embedders can no
  longer observe or poll each other's predictable task IDs; nested tasks remain
  compatible.
- Workers inherit parent lockdown and its granted permissions. Previously a
  restricted evaluator with Task permission spawned an unrestricted child that
  could read files and load manifest permissions.
- `Task.reply` is provisional until successful worker exit. Runtime failure or
  panic after a reply now wins instead of exposing a premature success. Worker
  runtime diagnostics retain `[code] kind: message` in the compatible `ERROR: `
  polling result.
- All Task API validation is structured and catchable (`SZ4001`/`SZ4002`), while
  nested errors/throws propagate unchanged. Registry poison no longer panics the
  host.
- Per-runtime limits are now explicit: 32 live workers, 256 retained records,
  1 MiB arguments/replies and 16 MiB worker source. Concurrency/message/thread
  creation ceilings are fatal `ResourceError` (`SZ6002`).

### Test runners no longer accept aborted unit files

- The Windows and Unix runners now require exit code 0, a `Results:` summary and
  no `[FAIL]` output for every framework-based unit file. Previously a parse or
  runtime abort before `summary()` produced no `[FAIL]` line and was reported as
  PASS—the exact defect recorded in the 9.16.0 changelog.
- Error/security fixtures now require both a non-zero exit and an error
  diagnostic. E2E output cannot be accepted or regenerated after a failed
  process.
- Every invocation runs a deliberately aborting fixture and proves the runner
  rejects it, so this quality-gate regression cannot silently return.
- The first honest run exposed 24 prior false positives: 16 legacy golden files
  were being treated as framework suites, three suites omitted `summary()`, and
  five fixtures did not parse. They are now correctly classified or repaired;
  the complete runner now passes 459/459.

### DateTime failures have a stable contract

- DateTime/DateField wrong arity and types now raise catchable `TypeError`
  (`SZ4002`); invalid calendars/epochs raise `RangeError` (`SZ4000`); field
  overflow raises `Overflow` (`SZ4000`); unknown members raise `ReferenceError`
  (`SZ4001`). The previous paths stopped with an unstructured sentinel.
- All members validate arity before evaluating argument expressions. This fixes
  zero-argument operations that previously accepted and silently skipped extra
  arguments and their side effects.
- Valid arguments preserve nested runtime errors and user `throw` unchanged.
  Rust, language and CLI regressions cover classification, capture and recovery.

### Cyclic inheritance can no longer hang the runtime

- Self and indirect cycles are rejected atomically at class declaration with
  catchable `TypeError` (`SZ4002`). Before this change, missing method/getter/
  setter lookup on a cycle looped indefinitely.
- All three ancestor lookup helpers are bounded defensively, including against a
  corrupt legacy/internal registry.
- Forward parent references remain compatible. Using the child before its parent
  is declared now raises catchable `ReferenceError` (`SZ4001`) instead of
  constructing a partial object or treating a missing parent as constructorless.
- Extending a sealed class now preserves the rejection but reports structured
  catchable `TypeError` (`SZ4002`).

### Property-write failures are structured

- Assignment to a getter-only property and field assignment on a non-instance
  now raise catchable `TypeError` (`SZ4002`) instead of an unstructured fatal
  sentinel. The write is still refused.
- Private accessors, malformed accessor arity and getter return mismatches share
  the same structured method-dispatch contract; accessor `throw`/runtime errors
  propagate unchanged.
- The specification now records, without changing, three broader compatibility
  debts: typed class fields are not enforced on later writes, interfaces can be
  extended after exact construction, and inherited private access is keyed to
  runtime rather than declaring class.

### Member-dispatch failures have stable diagnostics

- Missing instance members and missing static methods now raise catchable
  `ReferenceError` (`SZ4001`). A missing static method identifies the actual
  class/member instead of falling through to “Variable not found: Class”.
- Instance/static arity, external private-method calls/references and declared
  return mismatches now raise catchable `TypeError` (`SZ4002`). Resolution,
  argument evaluation, privacy enforcement and successful dispatch are
  unchanged.
- Rust, language and CLI regressions pin all seven paths and verify that caught
  failures do not corrupt later valid instance/static calls.

### `super` validation is structured and no longer ignores arguments

- Invalid `super()`/`super.method()` context, missing parents, impossible
  implicit chaining and constructor/method arity now raise catchable
  `TypeError` (`SZ4002`). A missing method in the parent chain raises catchable
  `ReferenceError` (`SZ4001`).
- `super(args...)` against a parent with no constructor no longer succeeds while
  discarding the arguments. Empty `super()` remains the compatible no-op; a
  non-empty call is `SZ4002`.
- The README now matches the implemented implicit-super contract instead of
  claiming every child constructor must call it explicitly. The conservative
  branch scan and the compatible manual-initialization exception remain visible
  rather than being changed silently.
- Rust, language and CLI regressions cover all nine error paths and verify that
  caught failures do not corrupt a later valid parent-method dispatch.

### Construction validation now has stable errors

- An unknown `new` target now raises catchable `ReferenceError` (`SZ4001`).
- Invalid interface construction, abstract-class instantiation, class
  field-form construction, constructor arity and arguments passed to a class
  without a constructor now raise catchable `TypeError` (`SZ4002`). These nine
  paths previously stopped with an unstructured, non-catchable sentinel.
- Human messages retain their identifying text, while Rust and Serez
  regressions pin `code`, `kind`, catchability and evaluator reuse. Successful
  class/interface construction and the eight official package suites remain
  compatibility gates.

### Default argument failures no longer become `null`

- User `throw` and runtime failures from a default expression now propagate
  unchanged through ordinary functions, native callbacks, constructors,
  `super()` constructors, `super.method()` and instance methods. Previously all
  six paths silently bound `null` and continued execution.
- Default evaluation now uses one cleanup-aware internal result contract, with
  regressions for the structured error payload and every call path.
- The parser now enforces the already documented ordering rule: a required
  parameter cannot follow a default parameter (`SZ2000`). A final `...rest`
  parameter remains valid after defaults; a non-final rest now also reports a
  syntax error instead of disappearing without a diagnostic. The official
  ecosystem has no signatures that depend on either invalid form.

### Resource and security failures are structured

- Call-depth checks now cover functions, methods, `super`, native callbacks and
  operator overloads through one fatal `ResourceError` (`SZ6002`) path. The
  ceiling is 512 frames: the former nominal 1000-frame contract allowed the
  Windows debug CLI to overflow its native stack around 800 callback frames
  before it could report an error.
- Tensor shapes and GPU matrix dimensions use checked multiplication before
  allocation. Tensors retain the 10,000,000-element ceiling. GPU buffers now
  enforce the documented 256 MiB **byte** ceiling (33,554,432 `f64` values) on
  creation, upload and matmul output; the old comparison admitted about 2 GiB.
- `Memory.alloc` above 256 MiB and `File.read`/`read_asBinary` above 256 MiB now
  report fatal structured `SZ6002`. `Memory.alloc(0)` remains a catchable
  invalid-argument `SZ4002`.
- Protected targets in `OS.exec` and `OS.spawn` now report fatal
  `SecurityError` (`SZ6004`). The existing substring guard remains explicitly a
  defense-in-depth heuristic, not a sandbox or canonical path policy.
- Array spread now matches call-argument spread: a non-array operand reports a
  catchable `TypeError` (`SZ4002`) instead of an unstructured fatal sentinel;
  user `throw` from the operand still propagates unchanged.
- Invalid `for-in` iterables, non-array rows in destructured `for-in`, and
  invalid array/object declaration destructuring now report catchable
  `TypeError` (`SZ4002`). Loop scope cleanup and nested user `throw` propagation
  are pinned by regressions.
- Rust runtime-outcome regressions pin each fatal payload and verify that
  `try/catch` cannot swallow it.

## [9.17.0] — 2026-08-16

### A subclass now chains to its parent's constructor on its own

```
class App:Window {
    public render() { ... }      // no constructor — normal coming from React
}
new App().mount()                // ❌ 'App' has no field or method named 'effects'
```

A subclass that never called `super()` got an instance with **none of the parent's
fields**, and the failure surfaced far away, naming an internal field of a class
the author never wrote. Java, C# and JavaScript all chain implicitly; now so does
this. Same for a constructor that simply forgets the call — the chain runs
**before** its body, so the subclass can still overwrite what it inherits.

The chain only happens when the parent constructor takes **no required
arguments**. When it does need them:

- the subclass **has** a constructor → nothing happens, silently, exactly as
  before. Initialising the parent's fields by hand instead of calling `super()`
  is a style the language allowed and there is code doing it (`tests/30_integral_e2e`
  has `Perro:Animal` doing precisely that). Turning it into an error broke three
  suites, so it stays legal.
- the subclass has **no** constructor → nothing can initialise the object, so it
  reports it, naming both classes and how many arguments are missing.

Whether the body already calls `super(...)` is a static walk of the constructor,
cached per class (`super_cache`), so it is paid once per class, not per `new`.
The walk is conservative: a `super(` anywhere — including inside an `if` — counts
as explicit and suppresses the implicit call.

`tests/unit_implicit_super.sz` (9 cases). Suite: 433, 0 failures.

## [9.16.0] — 2026-08-15

Four gaps that all showed up writing UI with serez-ui, fixed together because
three of them are the same thing seen from different angles: a value read out of
a container is a **copy**, and the language only knew how to write that copy back
in a handful of hardcoded shapes.

### A method of your own on a nested receiver kept its mutations

```
lista.push(new Celda())
lista[0].correr()      // mutated a copy
out lista[0].veces     // 0 — nothing happened
```

Reading `lista[0]`, `o.campo` or `this.celdas[i]` plants a copy, and until now
only the built-in mutators (`push`, `add`, `Add`, `clear`…, a fixed list of
names in `expr.rs`) were written back — and only one level deep. A method you
wrote yourself mutated the copy and dropped it.

This is what broke `useEffect` in serez-ui: `Window.runEffects()` calls
`this.effects[i].run()`, so `ran`, `prevDeps` and `cleanup` never persisted —
`deps []` re-ran on **every** update and the cleanup was never stored, so
`unmount()` cleaned nothing. **No library change was needed**; the two apps that
had the broken behaviour pinned in `apps40_test.sz` now assert the real thing.

- It was never just "an array element": `b.campo.metodo()` lost the mutation the
  same way. Any receiver that is not a plain variable.
- New `src/evaluator/lvalue.rs`: `resolve_lvalue_path` walks the receiver into a
  root variable plus a chain of `.field` / `[key]` hops, and `store_path` writes
  through it with a single `get_mut` on the root slot — no rebuilding of the
  intermediate containers.
- The writeback costs a copy back into the container, so it is gated twice, in
  this order: the receiver has to be a nested path (a syntactic check, free),
  and the method has to be able to write to `this` (a static walk of its body,
  cached per class+method). A read-only method pays nothing.
- The static analysis is deliberately coarse — any call rooted at `this` counts
  as a write. Refining it would need a whitelist of every read-only built-in,
  and being wrong there loses a mutation, which is the bug being closed.

### `a.b.c = x` and `a[i][j] = x` are writable

The first was a **parse error** (`Unexpected token '='`), the second a runtime
one (`Cannot assign to an index of a temporary value`). The workaround was to
rebuild and reassign the whole intermediate value.

```
o.inn.v = 9                     // was: parse error
this.filas[i][j] = 1            // was: InvalidAssignTarget
d["a"]["nueva"] = 5             // inserts, like the direct path does
t.o.inn.v += 8                  // compound forms too
```

Both now resolve the same writable path as the writeback above. New AST node
`NestedFieldAssign` rather than generalising `FieldAssignStatement`, whose
`object` is a bare name: `obj.campo = v` is the massive case and has no reason
to pay for path resolution.

- A setter halfway down the path still runs — the assignment goes through the
  same checks as the direct path (setter, getter-only property, element type,
  bounds), only the destination changed.
- Writing into a real **temporary** (`get()[i] = x`) is still a loud
  `InvalidAssignTarget`: there is nowhere to write back to.
- The AOT/LLVM backend (behind the `llvm` feature, unused) does not lower the
  new node — its HIR only has single-hop lvalues. Noted alongside the `&&`/`||`
  divergence from 9.14.

### A closure created in the constructor captured another `this`

```
class W { public W() { this.n = 0; this.f = () => { this.n = this.n + 1 } } }
let w = new W(); let h = w.f; h()
out w.n     // 0
```

The same closure written in a normal method worked. Registering effects or
callbacks in the constructor — the natural thing coming from React — was silently
mute.

Two copies stood between the closure and the finished object:

1. `eval_new_class` ended with `extract` + `plant`, returning a **different slot**
   than the one the constructor body used. Now it returns the live `this` slot
   (read from the binding, since a closure capture may have promoted it to the
   global arena). That also removes a deep copy of the instance on every `new`.
2. `let x = new C(...)` then copied it again. `Statement::Let` copies so a
   variable never aliases its source (`let x = arr[0]`), but a `new` produces an
   object nobody else can be holding, so the copy protected nothing and broke the
   identity. That single case now binds directly.

Value semantics are unchanged everywhere else, and that is the boundary: a
closure keeps a **cell** to the object it was created in, so if that object is
later copied into an array, a field or a return value, the copy is a different
object and the closure still points at the original. Construct-then-use, including
passing the variable to a function, now behaves.

### `!` follows the one truthiness rule

`&&`, `||`, the ternary and `match` guards moved to a single falsy rule in 9.14
(`false` · `null` · `0` · `0.0` · `""` · **empty** array/dict/set) but `!` was
left behind, so the idiom was split down the middle: `items && <Fila/>` compiled
and `!items` died with `Prefix '!' only applies to booleans`. You had to write
`items.length() == 0`.

`!` now negates `is_truthy` and always yields a boolean. With booleans the result
is identical to before, so this only turns former errors into values. A class
that defines `op_not` still wins — an explicit overload beats the general rule.

- `tests/err_bang_nonbool.sz` was removed: the condition it pinned is no longer
  an error. Its coverage moved to `unit_logical_operators.sz`, which also
  documents the rule from both sides.
- Still inconsistent, and left alone on purpose: `0m` (exact decimal) is truthy,
  while `0` and `0.0` are falsy. `is_truthy` has no `Dec` arm. Changing it would
  move `&&`/`||`/ternary/`match` too, so it is a call to make, not a slip to fix
  in passing.

### Tests

432, 0 failures. New: `unit_nested_receiver_writeback` (12),
`unit_nested_assignment` (14). `unit_logical_operators` grew to 19.
`unit_exceptions_advanced` now asserts that `m[i][j] = x` writes through, and
that a temporary and an out-of-bounds inner index still throw.

Also found and **not** fixed: the test runner reports `[PASS]` for a unit test
whose file fails to parse (PASS is "no `[FAIL]` line in stdout", and a parse
error produces neither).

## [9.15.0] — 2026-08-10

### A module with `export` erased the classes it imported itself

Composing a component out of components that live in **separate files** was
impossible, and the symptom pointed nowhere:

```
let c = new Card()   // fine
c.render()           // Unknown class or interface 'Badge'
```

`Card.szx` imports `Badge` and uses it inside `render()`. Construction worked;
the call died. The only workaround was to import every transitive dependency
*again* from the top file — the opposite of what an isolated component is for.

The cause is the visibility barrier in `import` (`stmt.rs`). When a module that
uses `export` finished loading, everything registered during its load that was
not in **its** export list got dropped:

```rust
self.class_registry.retain(|k, _| before.contains(k) || exports.contains(k));
```

"Everything registered during its load" includes what the module's *own* imports
brought in. `Card.szx` imports `Badge` (registered), exports only `Card` — so on
the way out, `Badge` was erased. It survived long enough for `new Card()` to
resolve because that only needs `Card`; `Badge` was looked up later, from inside
`render()`, when it was already gone.

Now the only names eligible for removal are the ones the module **declares** at
its own top level, via the existing `declaration_name`; whatever a nested import
registered stays. The barrier still does its job: what a module defines and does
not export remains hidden from its importer.

- Not specific to `.szx` — plain `.sz` modules had it identically.
- The resolver already handled `.szx`: it tries `<base>/<path>.sz`, then `.szx`,
  then `index.sz` / `index.szx`.
- Verified both ways (all `.sz` and all `.szx`, `Panel → Card → Badge`, one file
  each) and across a three-hop chain. Suite: 431, 0 failures.

## [9.14.0] — 2026-08-10

### `&&` and `||` return an operand, not a boolean

They used to demand a boolean on **both** sides and reject anything else with
`'&&' operator requires boolean operands`. Now they behave the way this operator
behaves in every language that has it:

```
a && b   // a if a is falsy, otherwise b
a || b   // a if a is truthy, otherwise b
```

With booleans on both sides the result is **identical to before** — `false && x`
is still `false`, `true && b` is still `b` — which is what makes this safe for
existing code, and the right-hand side is still not evaluated when the left one
already decides. What it opens up is the one-line conditional:

```
let name = input || "anonymous"        // fallback
let row  = items && buildRow(items)    // only when there is something
```

The second line is what prompted the change: building a UI, `items && <Row/>` is
how you say "render this when there is something to render", and it was a hard
error.

### One rule for what counts as falsy

`false`, `null`, `0`, `0.0`, `""` and an **empty** array, dict or set. Everything
else is truthy. The same rule now backs `&&` / `||`, the ternary, `match` guards
and the `filter` / `some` / `every` callbacks — previously only `false` and
`null` were falsy there, so `0` and `""` passed as true.

Empty collections being falsy is a **deliberate departure from JavaScript**,
where `[]` is truthy: there, `items && render(items)` fires on an empty list, and
the workaround (`items.length && …`) is itself the well-known bug that prints a
stray `0`. Here the plain form already means "if there is anything".

- Suite: 431 (new `unit_logical_operators`, 14 assertions), 0 failures.
- Not touched: the AOT/LLVM path (`compiler/llvm_emit.rs`) still lowers `&&`/`||`
  as bitwise ops on `i1`, so it only agrees with the interpreter for boolean
  operands. Worth reconciling before that path is used for anything real.

## [9.13.0] — 2026-08-09

### Dispatch stops copying the receiver

`.` and `[ ]` cloned the **whole receiver** before operating on it. That is not
what value semantics asks for: `ANALISIS_MEMORIA_RENDIMIENTO.md` picked P1
(Embedding), where reading copies **the element** — the code copied **the
container**. The copy was then mutated and written back over the same slot, so
it protected nobody: it was work that existed only to be thrown away one line
later.

Every method now runs against the arena slot, the way `d[k] = v` and `Set` have
since 7.3.0 and 9.12.0. Measured on release builds, best of three:

| | before | after |
|---|---|---|
| `a[i]` read, 10 000 elements | 7138 ms | 30 ms |
| `length()` × 10 000 | 2233 ms | 32 ms |
| `obj.method()`, instance holding 1000 elements | 956 ms | 297 ms |
| `obj.field`, same instance | 275 ms | 82 ms |
| `d.Add(…)` × 4000 | 8411 ms | 6 ms |
| `d.Remove(…)` × 200 over 3000 entries | 505 ms | 66 ms |

- **`a[i]`** on an array or a string reads the element out of the slot. The
  index expression was already evaluated before the clone, so evaluation order
  is untouched.
- **Instance dispatch** no longer copies every field. The copy was only ever
  read — mutation always went through `obj_ref` — so a new `field_value` helper
  pulls out the one field a call actually needs.
- **`length()`** on arrays and dicts reads the slot. It is O(1) and was paying
  an O(N) clone, in the single most common call in an indexed `for` header.
- **Dict methods** move to a new `methods_dict.rs` (`eval_dict_method_slot`),
  built on the `methods_set.rs` template; the dict arm of the generic match is
  gone. `Add` also stops scanning linearly for a duplicate key: it probes the
  slot-resident hash index, which the old whole-slot rewrite used to discard on
  every insert. Building a dict with `Add` was quadratic twice over.
  The indexed probe is validated against the legacy comparator and falls back to
  the scan when they disagree, so `Decimal` and compound keys keep the exact
  behavior they had; a miss cannot disagree, which is what makes each insert
  O(1).

Array methods that are inherently O(N) (`indexOf`, `join`, `map`…) deliberately
stay on the snapshot path: the clone does not change their complexity, and
moving them would change when their arguments observe the receiver.

### Mutations through a field or a dict slot no longer vanish

Reading `instance.field` or `d["k"]` plants a **copy**, so a mutation on the
result is dropped unless it is written back. Three shapes had no writeback at
all and failed silently — no error, just a change that never happened:

| Shape | Was | Now |
|---|---|---|
| `h.tags.add(x)` — a `Set` in an instance field | mutation dropped | persists |
| `d["k"].add(x)` — a `Set` in a dict slot | mutation dropped | persists |
| `outer["in"].Add({k, v})` — a dict in a dict slot | mutation dropped | persists |

The first was a missing entry in the list that triggers the field writeback: it
named `Add` (the dict method) and the aliases `remove`/`clear`, but never `add`
or `delete`. A user-defined `add`/`remove` method reached across a field hop
(`o.c.add(5)`) was losing its mutation for the same reason.

The other two were the writeback machinery living in the wrong place. It is now
two helpers — `dict_slot_ctx` (recognizes the `dict["k"].mutator()` shape) and
`apply_dict_writeback` (returns the mutated value to the slot) — shared by every
dispatch path instead of being inlined in the Array arm, which is precisely why
it only ever worked for arrays. The Array arm drops from 22 inline lines to 3.
The context is taken before the method runs and after `obj_ref` is evaluated —
the order the generic path always used, so nothing changes about when the key is
evaluated. A read-only method does not take it at all, so `outer["in"].keys()`
no longer copies itself back over itself.

### Also

- **README**: five features that had worked for a long time and were documented
  nowhere — `|>` (plus the missing `Pipe` row in the precedence table; it is the
  lowest precedence of all), `sizeof` (type keywords only — `sizeof(5)` is a
  parse error), `fn*`/`yield` (generators are **eager**: they return an array of
  everything yielded, not a lazy iterator), `match` as an expression with `|`
  alternatives, guards and subject binding, and a new Modules section (paths are
  relative to the importing file's directory, and every function reached from
  another file has to be exported, private helpers included).
- **360 coverage battery**, aimed by measurement rather than by eye: crossing the
  generated inventory in `src/lsp/builtins_gen.rs` against every test file found
  100 methods with zero coverage. Namespace gaps went 80 → 64, value-method gaps
  20 → 1; the remainder is structural (62 Gui methods need a real window and are
  verified by screenshot, 2 Terminal methods would corrupt the runner's output).
  New: `unit_360_random`, `unit_360_tensor_gaps`, `unit_360_namespace_gaps`,
  `unit_360_documented_gotchas` (the README's seven Known Gotchas, promised as
  guarantees and tested by nobody — all seven hold), `73_language_360_e2e` and
  `err_enum_string_concat`.
- **Pinned, not changed**: `argmax` and `argmin` break ties in opposite
  directions — `argmax` returns the LAST maximum, `argmin` the FIRST minimum
  (out of Rust's `max_by`/`min_by`). `argmax` also disagrees with NumPy and
  PyTorch, which return the first, and it is what picks the predicted class in
  classification. Asserted as-is so any change to it is a visible decision.
- **New test suites for the dispatch change**: `unit_slot_receiver_semantics`
  (value semantics, evaluation order, both writebacks), `unit_slot_collections_surface`
  (every array/string/dict/set method, each one standalone, through an instance
  field and through a dict slot), `unit_dict_methods_slot`, plus the
  `72_receiver_360_e2e` and `74_dict_slot_e2e` programs. The last one carries a
  cost guard that fails if the quadratic build ever comes back, without
  hardcoding a machine-specific number.
- **New benchmark** `16_dict_build`: dict construction and teardown through the
  method surface (bench 08 covers the subscript surface). 6384 ms against the
  previous binary, 347 ms now.
- Suite: 430 (39 new), 0 failures.

## [9.12.0] — 2026-07-30

### `sz --eval "<code>"` — run a snippet with no file

The interpreter lived entirely inside the `sz` binary, so the only way to run code
was to hand the CLI a path. It is a library now (`src/lib.rs`, crate `serez_code`),
and the binary is a thin shell over it, with two doors onto one pipeline:

| Door | Entry point |
|---|---|
| `sz file.sz` | `run::run_file` — reads disk, permissions from `serez.json` |
| `sz --eval "…"` | `run::run_eval` — source as a string, no permissions |

- **`run::run_source(src, name, opts)`** is the single pipeline (lex → parse →
  type-check → eval). A `.sz` file was only ever a string that came from disk: past
  the lexer nothing downstream can tell the difference, and the path survived only
  to label errors and locate `serez.json`. `RunOpts` carries those explicitly now,
  and `run_file` just reads the bytes and delegates.
- **`sz --eval "<code>"`** (also `-e`) takes the source as an argument — no temp
  file to write, keep clean and delete. **`sz --eval -`** reads it from stdin, which
  avoids fighting the shell over quotes and newlines in a multi-line snippet.
- The `.szx` (serez-ui JSX) plumbing moved out of `main.rs` into `src/szx.rs`.

### Lockdown mode — for source you did not write

The permission set is a **manifest, not a sandbox**. Any program can hand itself
everything with `use permissions { … }`, and three more capabilities reach the disk
with no permission declared at all — unlike OS/Socket/Task/Gui/Media/Time:

| Closed under lockdown | Why it needs closing |
|---|---|
| `use permissions { … }` | Inserts straight into the evaluator's permission set at runtime |
| `File` | Reads, writes, deletes and renames with nothing declared |
| `import` | Reads an arbitrary path off disk and **executes** it |
| `Autodiff.saveWeights` / `loadWeights` | The only methods in that namespace that touch disk |

All four come back as catchable `PermissionError`s. On for `--eval`
(`RunOpts::sandboxed()`), off for `sz file.sz` — declaring permissions inline in
your own file is unaffected.

**`fetch` is deliberately NOT part of lockdown.** It stays reachable, so on the
`--eval` path the request leaves from the host's network position: the usual SSRF
shape (cloud metadata endpoints, services on localhost, the host as an open relay).
Running untrusted source through `--eval` still needs real isolation around the
process, or a permission of its own for `fetch`.

### Also

- `fetch`'s transport is split out of `eval_fetch` into `fetch_transport`, with a
  shared `FetchResponse`; parsing, validation and the response shape no longer sit
  in the same function as the HTTP call.
- The three hardcoded `1000`s guarding recursion depth are one `MAX_CALL_DEPTH`
  const, and the error reports the actual limit.
- Suite: 419 (13 new `--eval`/lockdown CLI tests), 0 failures.

## [9.11.0] — 2026-07-27

### GUI: per-node affine transform — `Gui.nodeTransform` (rotate/scale)

- New scene primitive: **`Gui.nodeTransform(id, rotDeg, scaleXmille, scaleYmille, origX, origY)`**
  assigns an OPTIONAL affine transform to the retained node (rotation in degrees, scale in
  thousandths —1000 = 1.0—, origin in canvas px). The identity `(0,1000,1000)` clears it.
- The painter (`draw_node_transformed`) rasterizes transformed nodes by
  **inverse-mapping** with 2×2 supersampling (edge AA): fills (Rect/RectAlpha/
  RoundRect), **text** (local glyph coverage) and **images** (bitmap sampling)
  are mapped pixel by pixel; **outlines/lines** transform their vertices and are
  drawn straight; the circle scales its radius. `SceneNode` carries a `tr: Option` field.
- Enables `transform: rotate()/scale()/scaleX/scaleY` in serez-ui (the element's
  subtree is transformed around its top-left = `transform-origin: 0 0`).

## [9.10.0] — 2026-07-27

### GUI: text in PIXELS — `Gui.nodeTextPx` / `Gui.measureTextPx` (real font-size)

- The glyph engine (cosmic-text) now rasterizes by **pixel size** instead of by an
  integer scale of the 8×8 grid. Internally `ensure_glyph`/`measure`/
  `text_width`/`char_width`/`advances`/`draw_text` take `px` (the real size); the
  monospaced grid advances `px` per character instead of `8*scale`. The glyph cache
  is keyed by px.
- **The scale-based API is untouched**: `Gui.nodeText`, `Gui.measureText`, `Gui.drawText` and
  `Gui.textAdvances` map `px = 8*scale` at their boundary → **zero behavior change**
  for existing code (including the scene's `Text` node and the native primitive
  renderer).
- New primitives: **`Gui.nodeTextPx(x, y, text, px, color)`** (scene node at a
  literal pixel size) and **`Gui.measureTextPx(text, px)`** → `[width_px, px]`.
  They enable real `font-size: Npx` in serez-ui (14/20/27/34…px, not only multiples
  of 8) in the INTERPRETED renderer. `nodeSet` accepts `"px"` in addition to `"scale"`.

## [9.9.0] — 2026-07-26

### GUI: `Gui.nodeImage` with `radius` — rounded image clipping

- `Gui.nodeImage` accepts an optional 7th argument `radius`:
  `(x, y, imageId, w, h, alpha, radius)`. The blit (native and scaled) masks the
  corners with the round-rect's AA coverage (new `round_cov` helper, same distance
  as `fill_round_rect`), rounding the image's **pixels**. This enables real
  `Image { border-radius }` in serez-ui — previously the border was rounded but the
  image inside stayed rectangular.

### GUI: `Gui.nodeRoundRectOutline` — rounded outline (retained node)

- New scene primitive: **`Gui.nodeRoundRectOutline(x, y, w, h, radius, color)`**
  draws the **outline** (1px, antialiased corners) of a rounded rect. Previously there
  was only `nodeRoundRect` (filled) and `nodeRectOutline` (straight), so a `border` with
  `border-radius` ended up with square corners. It reuses the AA distance from
  `fill_round_rect`, painting only the ring band.
- Enables `border` + `border-radius` together in serez-ui: containers (`div`/`.card`),
  Image and Modal draw a rounded border instead of a square one.

## [9.7.0] — 2026-07-26

### GUI: `Gui.nodeImage` scales and applies alpha (retained node)

- The **retained** image node (`Gui.nodeImage`) goes from native size only to
  accepting, **additively**, size and opacity:
  `Gui.nodeImage(x, y, imageId)` (native), `(x, y, imageId, w, h)` (scaled) and
  `(x, y, imageId, w, h, alpha)` (scaled + global alpha 0–255). It reuses the same
  `draw_image_scaled` that `Gui.drawImage` already used, but in the scene (with dirty-skip),
  so it serves serez-ui's **retained** renderer — previously only the immediate
  `Gui.drawImage` scaled, and `renderScene` covered it up.
- Enables serez-ui's image CSS: `Image { width / height / opacity }` now
  works (scaling via the retained node did not exist).

## [9.6.0] — 2026-07-24

### `.szs`: `@when` / `@else` blocks — one condition grouping several elements

- New at-rule in the CSS engine: **`@when (cond) { … }`** wraps several rules
  (tags, `.classes`, `#ids`) under **a single logical condition**, so the condition
  does not have to be repeated selector by selector. The "query" is not a media query:
  it is the same `.szs` logic the rules use (a state variable, or `width`/`height`, with
  `and`/`or`/`not`).

  ```css
  @when (width < 300 and darkMode) {
      body   { color: #fff }
      .card  { padding: 8 }
      #main  { gap: 4 }
  }
  ```
- **`@else`** is the complement of the preceding `@when`, and **`@else (cond)`** chains
  else-if. The branches are **mutually exclusive** (evaluated top to bottom, the first
  match wins), so ranges do not have to be negated by hand:

  ```css
  @when (w < 200) { body { color: #100 } }
  @else (w < 400) { body { color: #200 } }
  @else           { body { color: #300 } }
  ```
- They can be **nested** (`@when` inside `@when`: the conditions are AND-ed) and a rule
  inside can carry **its own** `(cond)`, which is combined with the block's. `@else`
  negates the **whole** condition of the previous branch (`¬(a or b)` is
  `!eval(a or b)`, no De Morgan), so compounds like `(a or b)` complement correctly.
- **Unknown** at-rules (`@media`, …) are **discarded whole** instead of polluting
  the parse.
- Implementation: a rule's condition went from a single DNF to an **AND of negatable
  terms** (`CondTerm`); the parser is recursive per block with an inherited condition.
  Covered by 9 new Rust tests in `namespaces_gui::css` (18 in total), including the
  negation of a compound condition. serez-ui's interpreted engine gets the same grammar
  (new `when_test`, suite 22/22) so parity is not broken.

## [9.5.0] — 2026-07-21

### `.szs`: compound conditions with `and` / `or` / `not`

- A rule's condition in the native CSS engine is no longer a single comparison:
  it accepts several joined by **`and`** and **`or`**, plus negation with **`not`**,
  in the style of CSS media queries. `&&`, `||` and `!` are accepted as aliases.

  ```css
  body  (width > 600 and flag == true) { background-color: #c12; }
  .item (selected or hovered)          { border-color: #3b82f6; }
  .row  (not hidden)                   { display: flex; }
  ```
- Usual precedence: `not` binds tighter than `and`, and `and` tighter than `or`, so
  `a or b and c` is `a or (b and c)`. There are **no** grouping parentheses: the
  stylesheet scanner closes the condition at the first `)`.
- The connectors only count as whole words and respect quotes, so a name like
  `android`/`notify` or a value `"a or b"` does not split anything.
- This used to fail **silently**: the parser cut at the first comparison operator,
  left a non-existent variable (`width > 600 && flag`) and the rule never applied,
  with no error at all.
- An empty `()` now means "no condition" instead of a condition that never passes.
- Covered by the `namespaces_gui::css` Rust tests (9 cases), now included in
  `run_tests.ps1`. serez-ui's interpreted engine gets the same grammar so parity is
  not broken.

## [9.4.0] — 2026-07-21

### GUI: forgiving colors — `#rgba` / `#rrggbbaa` hex from color pickers

- The color parser accepts the alpha forms that color pickers emit
  (`#rgba`, `#rrggbbaa`): the alpha tints the background, like `rgba()` in CSS.
- Primitive-engine documentation brought up to date in the README.

### BUG: `obj.method` without parentheses ran the method instead of referencing it

- **Reading a method now yields the function bound to the object, not its execution.**
  `let ref = obj.method` (no parentheses, no arguments) returns a function you can
  invoke later; previously it fell through to method dispatch and ran it with zero
  arguments, returning its return value.
- This broke the pattern of **passing a handler as data** (`onClick={this.handler}`,
  handlers in arrays/dicts, callbacks between components): the method fired on
  EVERY read, so a state-mutating method did so on every render (a boolean flipped
  by itself, frame after frame), and what got stored was its return value —
  `null` for a `void` — so the callback never ran when invoked
  ("Attempt to call a non-function").
- If the method declared parameters, the zero-argument auto-invocation killed the
  program on the spot: `Method 'pick' expects 1 argument(s), got 0`.
- The bound reference **keeps its class context**: its body still sees its own
  private members, and referencing a private method from outside is rejected just
  like calling it.
- The `get prop()` mechanism is **unchanged**: explicit getters still run when read,
  and `obj.field` is still a field read. Resolution is field → getter → method
  reference.
- **Breaking**: code that wrote `obj.methodWithNoArgs` expecting it to run now gets
  the function without calling it. Ecosystem sweep
  (`Serez-code`, `serez-ui`, `serez-http`, `serez-ai`, `serez-graph`,
  `serez-pack`, `serez-dotenv`, `serez-cobol`, `serez-strike`, `serez-apipack`):
  zero occurrences of that form.
- Regression covered by `tests/unit_method_ref.sz` (10 cases).

## [9.3.8] — 2026-07-20

> First version published after 9.2.7: the local tag `v9.2.8` (a reverted bump)
> has no release, so its changes are listed here.

### Editor extension: `.szx` and `.szs` formatting (vscode-serez 1.9.0)

- The formatter covers all three languages: besides `.sz`, now `.szx` (JSX braces
  and depth) and `.szs` (blocks and `/* */` comments).

### GUI: native engine parity with the interpreted renderer

- `:font` recognized in `loadStylesheet`, bare boolean conditions (no comparator)
  in `.szs` rules, `font-scale` inheritance, `white-space: nowrap`,
  shrink-wrap of `absolute` elements without `width`, alpha on text nodes and the
  `:active-focus` alias.

### Multi-user `sz publish`: log in with a registry account, no hand-made tokens

- **`sz publish` / `sz unpublish` no longer require `SEREZ_API_KEY`**: the first time
  they ask for the username and password of a registry account (created at
  `packages.serezcode.org/register`), exchange the credentials for a long-lived token
  via `POST /api/login` and store it in `~/.serez/credentials.json`.
  From then on it is just `sz publish`.
- The password is read without echo (raw mode via crossterm) on a real TTY; with
  piped stdin (scripts/tests) it falls back to plain reading.
- If the stored token was revoked (401), the credential is deleted, login is
  requested again and the operation is retried once automatically.
- Registry 403 errors (someone else's package) arrive with the server's message;
  409 still reports "version already exists".
- **Compat**: if `SEREZ_API_KEY` is set it is used as before (legacy `x-api-key`
  header) and no login is requested. `SEREZ_REGISTRY_URL` still works for pointing
  at your own registry; the stored credential is per registry (if the URL changes,
  it asks for login again).
- **New `sz logout`**: deletes the stored credential; the next `sz publish` asks for
  username/password again (useful for switching accounts). With no active session it
  says so and exits successfully.

## [9.2.7] — 2026-07-14

### `throw` propagation fixes + visible `.szx` translation errors

- **`throw` inside `out f()` keeps its message**: rewinding the `out` statement's
  scratch mark freed the thrown payload BEFORE rendering it — the uncaught error
  showed "Referencia inválida" instead of the real message. Now it renders first
  and rewinds afterwards.
- **A `throw` while evaluating a nested argument no longer dies silently**: in
  `f(g())` with `g` throwing (also spread `f(...g())`), the throw degraded into a
  bare Error — exit 1 with no message at all and no chance to `catch` it.
  It now propagates as a Throw, re-planting the payload across the call's frame
  (catchable with try/catch, and visible as UNCAUGHT if nobody catches it).
- **`.szx` translator errors reach the console**: the translator's child process
  runs with `CREATE_NO_WINDOW` and its stderr was lost; now
  `sz app.szx` and `import` of `.szx` modules capture and reprint it as
  `TRANSLATE ERROR` before the generic message. (This complements serez-ui's new
  translator validation: two adjacent JSX roots in a `return()` abort with the real
  `.szx` line and the `<>…</>` fragment suggestion.)
- Tests: `unit_throw_propagation` (3 catchable cases), `err_throw_out_stmt`,
  `err_throw_nested_arg` + 2 CLI tests that verify the exact message content on stderr.

## [9.2.6] — 2026-07-14

### Primitive engine: real background translucency (rgba/hsla)

- **`background`/`background-color` with `rgba()`/`hsla()` respects the alpha
  channel**: translucency applies ONLY to the node's background (and is multiplied
  with the subtree's accumulated `opacity`) instead of being ignored. This fixes the
  Modal backdrop: `.modal-backdrop { opacity: 0.6 }` washed out the child box too;
  with `background-color: rgba(0,0,0,0.6)` (serez-ui UA sheet ≥ 4.3.6) the veil is
  translucent and the modal stays opaque.

## [9.2.5] — 2026-07-14

### Primitive engine: structural CSS gaps

- **Descendant selectors `.a .b`** (the last simple selector is the subject, the
  earlier ones match ancestors; `>` is treated as descendant), **compound classes
  `.a.b`** (previously only the last one was kept) and **groups
  `h1, h2 { }`** (one rule per selector). Focus rings like
  `Switch.focused .switch-track` stop being inert.
- **Pseudo-classes `:focus`/`:hover`/`:active`/`:disabled`**: they match the node's
  state attributes (the engine is stateless; the framework marks the state in the
  tree, the same contract as `.focused`).
- **`height` in `%` resolves against the PARENT** (the nearest ancestor with an
  explicit height; without one it falls back to the window, compatible with the
  previous behavior).
- **`opacity` propagates to the whole subtree, text included** (accumulated alpha
  ancestors × own; glyphs multiply their coverage).
- **`linear-gradient(...)`** in `background`/`background-image`
  (`to right/left/top/bottom` and `Ndeg`; with a border it paints an inset frame).
- **`box-shadow`** `[ox oy [blur [spread]]] color` with soft falloff
  (inset/spread are ignored).
- **`transform: translate/translateX/translateY`** (px): visual offset without
  touching the flow (like relative).
- **Basic `display: grid`**: `grid-template-columns` with px/%/fr/repeat(),
  `gap`/`column-gap`/`row-gap`, children in row-major order.
- serez-ui adoption (4.3.5): **continuous Slider dragging** with the mouse
  (previously click-to-set + keyboard only).

## [9.2.4] — 2026-07-12

### `.szx` module imports + modular refactor of the primitive engine + more CSS

- **`sz app.szx` runs directly** and **`import "x"` resolves `.szx` modules**
  (JSX) with on-the-fly translation, delegated to serez-ui's translator
  (`tools/translate.sz`; requires serez-ui installed). If `.sz` and `.szx`
  coexist, `.sz` wins. This replaces the szx.ps1/szx.sh wrappers.
- **Modular refactor**: the primitive engine moved out of `namespaces_gui.rs`
  (5290 → 4037 lines) into the submodules `namespaces_gui/css.rs` (selectors +
  prop resolution) and `namespaces_gui/render.rs` (layout + scene emission),
  without exposing internals (a child submodule sees its parent's privates).
- **CSS**: `rgb()/rgba()/hsl()/hsla()` colors and more CSS names; `font-size`
  in px (takes priority over font-scale); **`border` displaces the content**
  (content starts at `max(padding, border)`); **`color` inheritance** from
  ancestor to children; **`flex-shrink`** (a row of fixed items that do not fit
  shrinks proportionally instead of overflowing).
- **Build with no warnings of our own** (cleanup of unused/deprecated in crypto,
  autodiff, svg and Cargo.toml).

## [9.2.2] — 2026-07-11

### Primitive engine: web-like flex + readable refactor + text fixes

- **Shrink-to-fit text in flex rows**: spans/labels without `flex` or `width`
  measure their content instead of growing to fill — `justify-content` finally
  acts on rows of text (this fixes the Dropdown arrow stuck against the edge,
  `.modal-header` and the checkbox/fileinput centering). Bare strings in a row
  measure the same as a span.
- **Surgical CSS batch**: `width` in px/`%` on flex children (the `%` is of the
  container and is not re-applied over the slot), values with a `px` suffix in
  numeric props, `gap` only BETWEEN children (not after the last one),
  `position:relative` with left/top (right/bottom = negatives) without altering
  the flow.
- **textbox**: real `line-height`, an explicit `height` overrides the computed one,
  and caret/selection at glyph height.
- **Readable refactor of the engine** (so it can be modified by hand):
  the monolithic `prim_render` (~400 lines) → a ~70-line dispatcher + typed pieces
  `PrimCtx`/`PrimFrame`/`PrimStyle`/`PrimBox`, leaves (`prim_draw_*`)
  and containers (`prim_layout_*`), commented in Spanish with a code map at the top.
- serez-ui adoption: **caret proportional to the click** in Input/Textarea
  (`Gui.textAdvances`, nearest character boundary) with drag selection.

## [9.2.1] — 2026-07-10

### Primitive engine: adoption in real apps (serez-strike)

- **`img` accepts an image PATH**: if `src` is not a numeric handle from
  `Gui.loadSvg`, it is treated as a path to a PNG/JPG — read from disk, decoded
  (the `image` crate), scaled (preserving aspect ratio if only one dimension is
  given) and cached by path+dimensions. Web-like `<img src="…">` behavior.
- **`textbox` at 16px by default** (`font-scale: 2`, like serez-ui's interpreted
  path); the stylesheet can override it. Previously it fell back to 8px and native
  Inputs came out with tiny text.

## [9.2.0] — 2026-07-10 (work from 2026-07-07 to 2026-07-10)

### Native render primitive engine: layout + CSS + paint in the core

The bottleneck in large UIs was not rasterizing pixels (~1 ms) but the layout walk
+ CSS matching running interpreted (51–103 ms/frame on a real app tree). The core
gains a browser-style engine: it takes a tree of generic HTML-like primitives + a
CSS sheet, resolves styles, lays out and emits the scene in Rust. Measured:
**~0.04–0.08 ms/frame** of layout+CSS+emit (~1000× vs interpreted); a full frame
≈ 3.6 ms. The core stays generic (it does not know serez-ui's widgets): the
framework *lowers* its components to these primitives.

- **New API**: `Gui.loadStylesheet(src) -> handle` (`.szs` sheet),
  `Gui.loadSvg(srcOrPath) -> handle`, and
  `Gui.renderTree(root, sheet, w, h[, ctx]) -> regions`. The tree is a nested array
  `[tag, [[prop, val]…], [child|text…]]`; `renderTree` rebuilds the retained scene
  and `Gui.renderScene(bg)` rasterizes it (dirty-skip intact).
- **Primitives**: `div`, `row`, `p`, `h1`–`h6`, `span`, `b`/`strong`, `i`/`em`,
  `hr`, `img`, `svg`, `circle`, `line`, `polyline`, `polygon` and an editable
  `textbox` (caret + selection painted by the core; virtualization — it only lays
  out the visible lines, so a 10 KB text stops being expensive).
- **Web-like CSS**: selectors by `tag`, `*`, `.class`, `#id` and compounds
  (`tag.class#id`) + reactive conditions `(var op val)` evaluated against the
  `ctx`; "last one wins" resolution. Full box model (padding/margin per side
  + 1–4 value shorthands), `border` (including the `1px solid #333` shorthand) and
  `border-radius`, `width`/`height` in px/`%`/`auto`, `display:none`,
  `text-align`, `line-height`, `letter-spacing`, numeric `font-weight`,
  `text-decoration` (underline/line-through), per-node `font-family`/`font-scale`.
- **Flexbox**: `row`/`display:flex` (+ `flex-direction:column`), `flex` weights,
  `justify-content` (all 6 modes), `align-items`, `gap`; `position:absolute`
  children are out of the flow.
- **Overlays**: `position:absolute` with `left`/`top`/`bottom`/`right` (containing
  block = positioned ancestor) and `z-index` — the basis for Dropdown/Modal/Tooltip/
  Toast.
- **Real proportional text**: measurement by glyph advance (bold/italic aware),
  true word-wrap (breaking on spaces), scrolling with per-node clipping
  (scrolled backgrounds are cut cleanly).
- **Vector SVG**: our own parser for a subset of SVG (paths
  M/L/H/V/C/S/Q/T/A/Z abs+rel, shapes, `<g transform>`, fill/stroke inheritance,
  `viewBox`, colors) rasterized with **tiny-skia** (antialiasing), cached by
  handle+dimensions. New core dependency: `tiny-skia`.
- **Hit-testing**: regions come back in PRE-order as
  `[tag, x, y, w, h, onClick|null]`; the function value embedded in `onClick`
  survives the round-trip and `.sz` routes the click with `region[5]()`.
- **fix(evaluator)**: when promoting an object captured by a closure
  (Scoped→Global), ALL of the scope's aliases are now rebound, not just the
  innermost frame — previously the object forked and mutations made after creating
  a lambda were lost.
- Tests: `tests/unit_gui_primitives.sz` (headless engine checks); the real render
  is verified with demos + screenshots. Suite **399/0**.
- **Adoption**: serez-ui lowers its components to these primitives behind the
  `useNativeRenderer` flag (Phase 3 complete: every widget verified natively)
  and serez-strike runs on the native renderer.

## [8.2.0] — 2026-07-03 (work from 2026-07-02 to 2026-07-03)

### "Technical debt" batch: strict parser, closure semantics, multi-window, retained-mode, audio

- **Parser: no more silent recoveries.** `let x = ;`, `let = 5;`,
  `let x;`, `return a +`, invalid numeric literals and the rest of the holes where
  the parser discarded statements without reporting now emit a `PARSER ERROR` with
  position and caret. A program with parse errors **no longer runs halfway**
  (it aborts with exit 1), and `import`s of modules with parse errors abort instead
  of evaluating the partial module. A bare `;` is a legal empty statement.
  This uncovered and fixed 2 latent bugs in serez-ui (renderer.sz: an unescaped
  `"{·"` triggered the interpolation parser and the whole `out` was silently
  discarded — the TUI Select never printed). New `err_parse_*` tests.
- **Parser errors name the FILE** (`PARSER ERROR [path line:col]`),
  including imported modules and interpolated expressions.
- **Lexer: uniform `token.column`.** Every token carries the position of its
  FIRST character (before: identifiers pointed one position past the last char).
  The LSP dropped its `ident_start_col` correction.
- **Semantics: closures with a SHARED CELL.** A lambda and its enclosing scope
  share the captured variable at any nesting level: mutations inside the closure
  escape (`makeCounter` counters work) and later writes are visible inside. A `for`
  counter is fresh per iteration (like JS `let`): the loop's closures keep the value
  of their iteration (10,20,30 — not 40,40,40). A counter declared outside a
  `while` is a single shared cell (333). Semantics tests updated.
- **Semantics: a non-callable parameter no longer hides a same-named function in
  CALLS** (the `h` parameter case that broke serez-ui's render): `name(...)`
  falls back to the nearest callable binding; reads still see the shadow.
- **Multi-window Gui:** `Gui.openWindow(title,w,h) -> id`, `Gui.selectWindow(id)`
  (all existing drawing/input moves to that window), `Gui.currentWindow()`,
  `Gui.closeWindow(id)`. The classic `Gui.open` window is id 0 and its protocol
  does not change (serez-ui untouched). Each extra window has its own canvas and
  input (mouse/keyboard/scroll/focus). Verified with a 2-window demo
  (~2,600 combined presents/s).
- **Retained-mode Gui (scene graph):** persistent nodes the core redraws in Rust —
  `nodeRect/nodeCircle/nodeLine/nodeText/nodeImage -> id`,
  `nodeSet(id, prop, value)` (x, y, w, h, r, x2, y2, color, z, visible, text,
  scale, image), `nodeDelete`, `sceneClear`, `nodeCount` and
  `Gui.renderScene(bg) -> bool`, which redraws ONLY if the scene is dirty (if not,
  it re-presents and returns false). This removes re-running the interpreted draw
  tree every frame.
- **New `Media` namespace (audio, `Media` permission):** `playSound(path) -> id`
  (wav/mp3/flac/vorbis via rodio, asynchronous), `stop/stopAll/pause/resume`,
  `setVolume(id, 0..200)`, `isPlaying(id)`, `playingCount()`. Catchable errors:
  `IOError` (file) and `MediaError` (format/device); a permission denial is still
  fatal (`sec_media_no_permission`). Video is out of scope: decoding it requires
  ffmpeg (design decision pending).
- **LSP:** multi-file analysis (symbols/definition/completion follow transitive
  `import`s, cached by mtime), `.szx` support (symbols/outline without
  diagnostics: the parser does not speak JSX), `rename`, `references` and
  `signatureHelp` (user functions, builtins and namespace methods).
  Extension **1.8.0**: client for `.szx`, partial Restricted Mode support
  (`untrustedWorkspaces: limited` — highlighting/formatter yes; sz-lsp only in
  trusted workspaces, and it starts as soon as trust is granted).
- Suite: **398/0** (+9 new tests); fuzzing 300 cases with no panics; LSP 27 tests +
  smoke 9/9; whole ecosystem green (ui 17/17, http/ai/pack/apipack/agentai/
  graph 3/0, dotenv 2/0, cobol 23+22, strike 53/0).

### Ecosystem adoption (same date): full scene parity + retained serez-ui

- **The retained scene reached parity with EVERY primitive serez-ui uses**:
  `nodeRoundRect`, `nodeRectAlpha`, `nodeRectOutline`, `nodePolygon`,
  `nodePolyline`, `nodeClipPush`/`nodeClipPop` (clipping as markers in the
  draw order) and text with per-node font/style/spacing (`nodeSet`:
  `font`, `style`, `spacing`, `radius`, `alpha`, `width`, `points`).
- **The scene is PER WINDOW** (each window has its own scene graph; `node*` and
  `renderScene` operate on the selected window) — two retained windows no longer
  collide.
- **Click by EVENT in extra windows**: `Gui.mousePressed()` on a secondary window
  counts presses as events in its accumulator — a short click between two presents
  is no longer lost (it used to be level-triggered).
- **An unreadable `serez.json` no longer fails silently**: if it exists but does not
  parse (e.g. without `"version"`), a WARNING is emitted instead of running with no
  permissions.
- **serez-ui (2.3.0)**: the GUI renderer migrated to retained-mode (`sw*` methods with
  positional node reuse; `Gui.renderScene` instead of `clear+draw+present`;
  pixel-perfect visual parity verified against the previous engine) and gained
  **secondary windows**: `openPanel/closePanel/panelCount` + `renderPanel(id)`
  on the component itself (Button/Link clicks routed per panel; verified with a real
  demo: click on a panel → app state → re-render of the main one).
- **serez-pack**: compatibility verified end-to-end (an app with the `Media`
  permission packaged and executed); it now validates at packaging time that
  `serez.json` has a `"version"` (without it, the installed app would run with no
  permissions).

### New: `sz-lsp` — Language Server Protocol for editor support

- **New binary `sz-lsp`** (`src/lsp_main.rs` + `src/lsp/`): an LSP server over
  stdio JSON-RPC that closes the last open roadmap checkbox. It reuses the
  interpreter's lexer/parser/type-checker directly (second `[[bin]]` target;
  the `sz` binary and the runtime are untouched). No async runtime: a
  synchronous framed loop over `serde_json` (the only new dependency).
- **Capabilities:** live diagnostics on every keystroke (parser errors as
  errors + static type checker findings as warnings, with real ranges),
  completion (keywords, the 21 native namespaces with their real methods,
  builtin functions, and the document's own symbols — `File.` lists `read`,
  `write`, …), hover (user signatures `fn int suma(int a, int b)`, namespace
  summaries), go-to-definition (functions/classes/enums/variables +
  `import "…"` jumps to the file) and hierarchical document symbols.
- **Symbol index works on broken files:** it scans tokens (which carry
  line/column) instead of the AST, so completion/outline keep working while
  the user is mid-keystroke — the normal state in an editor.
- **Parser/type checker now *collect* their errors** (`Parser::take_errors`,
  `TypeChecker::take_errors`, with 1-based positions) in addition to printing
  them; CLI output is unchanged (suite 389/0).
- **Namespace/method catalog is generated** from the evaluator sources by
  `tools/gen_lsp_builtins.py` (21 namespaces, 227 methods + value methods of
  array/string/set/dec/tensor) into `src/lsp/builtins_gen.rs` — re-run it when
  a namespace gains methods.
- **VS Code extension 1.7.0** (`vscode-serez/`): starts `sz-lsp` automatically
  for `.sz` (new settings `serez.lsp.enabled`, `serez.lsp.path`; uses `PATH`
  by default). Formatter and highlighting are unchanged and keep working if
  the binary is missing.
- **Tests:** 22 Rust tests (`cargo test --bin sz-lsp`) covering analysis,
  symbol scanning and the full protocol handshake, plus
  `tools/lsp_smoke.py` — a real LSP session against the compiled binary
  (initialize → didOpen broken file → diagnostics → completion → hover →
  definition → shutdown), 9/9.

---

## [7.3.0] — 2026-07-02

### New: language-level errors are catchable (third pass) + collections are O(1)

- **Third pass of catchable errors — the language core itself.** `Variable not
  found` and undeclared-variable assignments now raise a catchable
  **`ReferenceError`**; calling a non-function, argument-count/spread mismatches,
  `const` reassignment and all `builtins` failures (`parseInt`, `fetch`, `env`,
  …) raise a catchable `TypeError`/`RuntimeError` instead of aborting. The call
  path unwinds cleanly (scope / call-depth / call-stack restored) so a
  `try/catch` in a loop can absorb thousands of these without corrupting the
  evaluator. **Stack overflow and resource limits stay fatal** (a catchable
  overflow would let infinite recursion retry forever).
- **Array `push`/`pop` are O(1)** (run against the arena slot instead of cloning
  the whole array per call): building an array with `a.push(x)` in a loop went
  from O(N²) to O(N) — 20 000 pushes dropped from 8 824 ms to 11 ms. The
  `dict["k"].push(x)` and `instance.field.push(x)` write-back patterns are
  preserved.
- **Lambda capture no longer leaks.** A lambda captured *every* visible local
  into the global arena on each creation (a permanent slot per unused local per
  lambda); it now snapshots only the identifiers the body actually references.
  A lambda created per loop iteration dropped from linear arena growth to flat.

### Security

- **Fixed a string-escape bug that could dodge the `OS.exec` System32 block.**
  An unknown escape (`"C:\Windows"`, `"a\d"`) duplicated the character after the
  backslash (`C:\Windows` → `C:\WWindows`), which both corrupted the path and
  made it slip past the blocked-path substring check. Escapes now keep both
  characters verbatim without duplication.
- New systematic non-catchable security tests (`sec_notcatch_*.sz`): permission
  denials, `unsafe {}` gates, the System32 exec block, tensor size limits and
  stack overflow are each verified to stay fatal inside a `try/catch`.
- Added `fuzz_parser.py`: feeds garbage and mutated corpus to the lexer/parser
  and asserts no Rust panic (0 crashes over 1 000 cases across two seeds).

### Build

- The AOT/LLVM backend (`src/compiler/`, ~3 000 lines) is now behind the
  `llvm` Cargo feature (off by default): it is Phase-1 and not wired to any CLI
  verb, so the default build skips it and the `inkwell`/LLVM-17 dependency.

### New: I/O and namespace errors are catchable (second pass, ~530 sites)

- All runtime failures across **File, JSON, Math, OS, Terminal, Env, Time,
  System, Socket, Gui, Tensor, Autodiff, GPU, Memory, Binary and Crypto** are
  now catchable with `try/catch`, binding the structured `Error` object. New
  `.kind` categories: **`IOError`**, **`JsonError`**, **`OSError`**,
  **`SocketError`**, **`GuiError`**, **`TensorError`**, **`AutodiffError`**,
  **`GpuError`**, **`MemoryError`**, **`BinaryError`**. Invalid arguments
  (wrong count/type, unknown method) raise `TypeError`. A missing file, a
  refused socket or a tensor shape mismatch no longer kill the process.
- **Unchanged and still fatal**: permission denials, `unsafe {}` gates, the
  System32 exec block, and resource limits (256 MB file reads, 10M-element
  tensors, GPU buffer caps). The sandbox invariant is untouched
  (`sec_runtime_not_catchable` still passes).
- Compat notes: `OS.spawn` still returns `-1` when the process fails to start,
  and `Socket.recvWsFrame` still returns `null` on protocol errors (APIs relied
  upon by serez-ui/serez-http).
- New suite file `unit_catchable_io.sz` (11 assertions) pins the behavior.

### Perf: dict lookups are O(1) on large dicts (~860× on 20k keys)

- `d[key]` reads no longer clone the whole dict — the lookup runs directly
  against the arena slot.
- Dicts now carry a lazy hash index (canonical key → position) built on first
  lookup once the dict has ≥ 16 entries, kept warm incrementally on
  `d[key] = value` inserts, and validated on every hit (a stale cache can only
  fall back to the linear scan, never return a wrong entry). Insertion order is
  preserved; small dicts keep the plain linear scan. Benchmark: 20 000 inserts +
  20 000 reads went from 39 689 ms to 46 ms.

### Perf: Set membership is O(1) on large sets (~1 500× on 20k elements)

- `has`/`contains`/`add` run directly against the arena slot (no O(N) clone of
  the whole set per call) and use a lazy hash index over TYPE-TAGGED element
  fingerprints — faithful to `obj_data_eq`: `5` and `"5"` stay distinct
  elements, `1.50m` equals `1.5m` (scale-insensitive `dec`), `-0.0` equals
  `0.0`, NaN never equals itself, and compound values keep the authoritative
  linear scan. `new Set([...])` deduplicates via hash in O(N) instead of the
  old O(N²) pairwise scan. Benchmark: 20 000 adds + 20 000 `has` went from
  71 383 ms to 48 ms. Small sets (< 16) keep the plain linear scan. New suite
  file `unit_set_index.sz` pins the equality semantics.
- ALL Set methods now run against the arena slot: the generic dot-call path
  used to clone the entire element vector before entering any method — even
  `.size()` paid an O(N) copy — and mutations rewrote the whole slot.
  `delete` finds its target through the index and removes in place (insertion
  order preserved); `clear` truncates in place.
- `union`/`intersection` went from O(N×M) pairwise comparisons to O(N+M) via
  fingerprint sets: two 5 000-element sets took 2 546 ms / 1 563 ms, now 2 ms
  each. Set argument/arity errors are now catchable (`TypeError`), matching
  the rest of the runtime.

### Fixed: top-level loops no longer grow the global arena

- A `while` / `do-while` at top level leaked one global-arena slot per
  iteration (the condition's temporary, allocated with no scope active). Loops
  now run inside an ephemeral frame so those temporaries land in the scoped
  arena and are reclaimed per iteration; `do-while` additionally gained the
  per-iteration condition cleanup it was missing at any depth. A 2 000-iteration
  top-level loop now leaves the arena at baseline (~261 slots, was ~2 262).

---

## [7.3.0] — 2026-07-01 (earlier work in this release)

### New: catchable runtime errors + structured `Error` object

- `try/catch` now catches ordinary **programming** errors (index out of range,
  division/modulo by zero, type mismatches, invalid assignment targets), not just
  values raised with `throw`. Inside `catch` they bind an **`Error`** object with
  `.message` and `.kind` (`IndexOutOfBounds`, `DivisionByZero`, `TypeError`,
  `InvalidAssignTarget`, `Overflow`, `RuntimeError`). `throw "x"` still binds the
  raw value.
- **Security and resource-limit errors stay fatal and non-catchable** (permission
  denials, `unsafe`-required gates, stack overflow / resource guards) — a
  `try/catch` cannot silently swallow them, preserving the sandbox and DoS
  protections.
- String concatenation with an instance (`"x" + e`) now renders it (the `Error`
  object → its `.message`), while still honouring a user-defined `op_str`/`op_add`.

### New: `Regex` namespace (dependency-free engine)

- `Regex.test / match / findAll / split / replace`. Backtracking engine compiled
  to bytecode, bounded step budget (no hangs). Supports `. \d \w \s`, classes
  `[a-z]` `[^…]`, anchors `^ $`, groups `( )` / `(?: )`, alternation `|`, and
  quantifiers `* + ? {n,m}` (greedy or lazy). Patterns use raw strings `r"…"`.
  No permission required. Invalid patterns raise a catchable error.

### Changed: `arr[i] = x` is now O(1); nested index-assign is loud

- Array index assignment mutates the element in place (`Arena::get_mut`) instead of
  copying the whole array — a fill loop goes from O(N²) to O(N). Value semantics
  are unchanged (verified: `let b = a; a[0]=x` does not affect `b`).
- Assigning into a **temporary** target (`m[i][j] = x` where `m[i]` is a copy,
  `getArr()[i] = x`, …) previously did nothing silently; it now raises a catchable
  `InvalidAssignTarget` error. Reassign the whole element instead (`m[i] = inner`).

### Fixed: `x is function`

- `is function` now returns `true` for named functions and lambdas (previously
  always `false`, though `type_of` already reported `"function"`).

### Perf: parallel, cache-friendly matmul

- `Tensor.matmul` rewritten from a naive `ijk` triple loop to a cache-friendly
  `ikj` order and parallelized across output rows with `std` scoped threads (no
  external dependency; small matrices stay single-threaded). The autodiff backward
  pass reuses the same kernel, so training also benefits. Results are bit-identical.

---

## [7.2.0] — 2026-06-30

### GUI: vector drawing, full input and window control

- **Vector primitives**: thick lines, polylines, polygons and an antialiased
  circle.
- **Text**: underline / strikethrough and `letter-spacing` in `drawText`.
- **Images**: loading from bytes (in addition to a path), image clipboard
  (get/set) and a custom-image mouse cursor.
- **Window and screen**: window position, monitor enumeration and the rest of
  the window operations.
- **Input**: the missing `winit` events — focus, cursor, file drop,
  IME preedit and side mouse buttons — plus touch, hover and pinch.
- **Horizontal scrolling** in the predictive compositing.

---

## [7.1.0] — 2026-06-29

### GUI: asynchronous predictive scrolling (threaded compositing)

- Scroll compositing moves to a separate thread and anticipates the displacement,
  so the window does not wait for the repaint to respond.

---

## [7.0.0] — 2026-06-28

### `Task` namespace — isolated concurrency on native threads

- New **`Task`** namespace: asynchronous execution of isolated subprocesses on
  Rust threads, *share-nothing* (each worker with its own arena, communicating
  via JSON). Requires the `Task` permission.
- Covered by stress tests, nested workers and protection against panics inside
  a subprocess.

### BREAKING: system namespace names are reserved

- **A class, interface or enum can no longer be named after a system namespace**
  (`Task`, `File`, `OS`, `Gui`, `Env`, `Time`, `Socket`, …). The parser rejects it
  with an explicit message.
- This is the counterpart of adding `Task`: without the rule, a user `class Task`
  would shadow the native namespace. Existing code using one of those names must
  rename the class — as was done with the `apps/01_task_manager.sz` example
  (`Task` → `TaskItem`).

### OS: non-blocking `OS.spawn`

- `OS.spawn` stops blocking and is harvested by *polling* with **`OS.tick()`**
  (no callbacks: callbacks would open a use-after-free with the region model).

### GUI: ~0 CPU when idle

- The loop becomes *event-driven*: with no activity, CPU usage drops to ~0.
  Window, dialog, image and text APIs are added, and reflow on resize is fixed.

### Fixes

- Two interpreter panics on invalid input.
- Overflow in the iterative fibonacci benchmark; concurrency and decimal
  benchmarks are added.
- Editor extension: Serez Dark theme and up-to-date grammar (1.6.0), `"{var}"`
  interpolation colored as braces + variable (1.6.1).

---

## [6.3.0] — branch `improve` (2026-06-20)

### New: `DateTime` namespace (calendar date/time)

- Immutable date/time built on `chrono`. Construction: `DateTime.now()` /
  `utcNow()` (require the `Time` permission), `DateTime.from(y,m,d[,h,mi,s,ms])`
  and `DateTime.fromEpoch(ms)` (no permission — pure, and reject invalid dates).
- Fields `.year/.month/.day/.hour/.minute/.second/.ms` return a **DateField**
  that acts as an `int` under operators yet carries immutable
  `.add(n)/.reduce(n)/.remove(n)` returning a new `DateTime`. Day/time units shift
  the instant; month/year adjust field-wise and clamp the day to month end.
  Read-only `.weekday/.dayOfYear/.daysInMonth`, plus `.isLeapYear()/.isUtc()`.
- `.format(pattern)` (moment.js-style, `[literal]` escaping), `.toString()/.iso()`,
  `.timestamp()/.toEpoch()/.epochMillis()`. Object-destructuring exposes the
  calendar fields as ints: `const {day, month, year} = DateTime.now()`.

### New: exact decimal type `dec`

- Base-10 exact decimal (crate `rust_decimal`, 28–29 digits) alongside the
  untouched `decimal` (f64). Literal suffix `m`: `12.50m`, `5m`, `1e-7m`.
- `int` mixes in exactly; mixing `dec` with f64 `decimal` is a type error
  (convert via `toDecimal()` / `Dec.parse`). Comparison by value; checked
  arithmetic; `/` rounds to 28 digits half-even; `**` requires an int exponent.
- Methods `round/setScale/truncate` (modes half-even default, half-up, down, up,
  floor, ceil), `scale/abs/floor/ceil/isZero/sign/min/max/toInt/toDecimal/
  toString`; namespace `Dec.parse/fromInt/MAX/MIN/MAX_SCALE`. JSON serializes a
  `dec` as an exact number literal. Works in switch, sort, includes and dicts.

### New: raw string literals `r"…"`

- `r"…"` disables interpolation **and** escape processing — `{ }` and backslashes
  are literal (great for literal braces, Windows paths, regexes). Cannot contain a
  `"`. Default `"…"` interpolation is unchanged (zero impact on existing code).

### Bug fixes

- **B-77** — `op_str` is now honored in string `+` concatenation (both operand
  orders), consistent with interpolation/array display.
- **B-78** — escaped closing brace `\}` in a string literal no longer leaks the
  backslash (symmetric with `\{`); inline literal JSON now works.
- **B-79** — the power operator `**` is now **right-associative**
  (`2 ** 3 ** 2 == 512`), matching math/Python.

### Tests

- New: `unit_datetime`, `unit_dec`, plus mixed/integration suites
  `unit_mixed_features`, `unit_stdlib_mixed`, `unit_systems_mixed`,
  `unit_net_gui_mixed` and e2e `63`–`71` (datetime, dec, raw/op_str, deep
  cross-feature, stdlib, systems/crypto/GPU/autodiff, networking+GUI), plus
  security tests for the new namespaces. Full suite: **369 passing, 0 failing.**

---

## [Unreleased] — branch `improve` (2026-06-11)

### Memory — loop-body value retention fixed (leak #1 residual)

- **`eval_block_discard`**: loop bodies (`for`, `while`, `do-while`, `foreach`) no
  longer deep-extract and re-plant the value of the body's **last statement** into
  the loop's frame. Every loop caller discards that value, but the copy lived until
  the loop exited — so any loop whose last statement produced a compound
  (`arr = arr.map(...)`, `arr.reverse()`, …) retained one full copy **per
  iteration**. Measured: 300 iterations over a 20k-element array went from
  **~430 MB peak RSS to ~17 MB**. `return`/`throw` escaping the body keep the
  exact same extract+plant semantics as before.
- Probes refreshed (`mem_probe/`): the historic big leak (push-promotion in
  helpers, probes `f`/`h`) was already killed by the element-embedding refactor
  (`Array/Dict/Set` store `OwnedValue`, like `Instance`); global arena stays at
  baseline (~262 slots). Known minor residual: one small global slot per lambda
  **created** inside a loop (capture snapshot, ~24 bytes each).
- New regression test `unit_loop_body_value` (7 asserts): compound reassign /
  mutating method as last statement, return/throw from body, break/continue,
  do-while and foreach intact.

### Crypto — real signatures and CSPRNG (vetted crates)

- **`Crypto.randomBytes(n)`** — cryptographically secure random bytes from the OS
  entropy source (`getrandom` crate). Returns `[int]` (0..255). `n` capped at
  1 MB (throws beyond it; throws on `n < 1`). Unlike `Random.*` (seedable LCG,
  predictable — fine for games, never for secrets), this is safe for tokens,
  salts and keys.
- **`Crypto.ed25519Keypair()`** — generates an Ed25519 keypair
  (`ed25519-dalek` crate); returns `{ private, public }` as 64-char hex strings.
- **`Crypto.ed25519Sign(privateHex, message)`** — returns the 128-char hex
  signature. Deterministic (Ed25519 by design). Malformed/short keys throw.
- **`Crypto.ed25519Verify(publicHex, message, signatureHex)`** — `true`/`false`
  via strict verification (rejects non-canonical signatures). Malformed hex or
  wrong lengths throw; well-formed but invalid signatures return `false`.
- New tests: `unit_crypto_ed25519` (7) and `sec_crypto_ed25519` (8 — caps,
  malformed inputs, corrupted-signature behavior).

### Lexer

- New regression suite `unit_sci_notation` (7 asserts) cementing scientific
  notation (`1e-7`, `2.5e3`, `1E+10`, bare `e` still an identifier) — the
  feature itself shipped in 4.6.2.

---

## [5.0.0]

### GUI

- **`Gui.time()`**, **`Gui.drawRect(x, y, w, h, color)`**,
  **`Gui.fillCircle(cx, cy, r, color)`**, **`Gui.setImePosition(x, y)`** — drawing
  and IME surface for serez-ui (cursor blink timing, outlines, radio buttons,
  IME composition placement).

---

## [4.9.0]

### GUI

- **Font loading and selection**: `Gui.loadFont(path)` + proportional text
  rendering with real font metrics (replaces fixed-advance text).
- **`Gui.fillRoundRect(x, y, w, h, radius, color)`**.
- Error + security test coverage for the new Gui surface
  (`err_gui_*`, `sec_gui_no_permission`).

---

## [4.8.0]

### GUI — backend migration

- Backend migrated **minifb → winit + softbuffer + cosmic-text**: proper window
  lifecycle, IME support, real text shaping/rasterization, and the event model
  serez-ui's self-driven loop (`app.runGui`) builds on.

---

## [4.7.0]

### CLI

- **Run `.szx` (serez-ui JSX) files directly**: `sz app.szx` transpiles and runs
  without a separate step.

---

## [4.6.2]

### Lexer

- **Scientific notation in number literals**: `1e-7`, `2.5e3`, `1E+10`. The `e`
  is only consumed when followed by `[+-]?digit`, so identifiers like `e` keep
  lexing as before. (Unblocked BCE-style epsilon constants in serez-ai guides.)

### CLI

- **`sz run <name>` resolves package bin commands**: if `<name>` is not a script
  in `serez.json`, it resolves the entry of an installed package and forwards
  the remaining args (e.g. `sz run apipack build`).
- **Non-zero exit codes** on parse errors, runtime errors, and subcommand
  failures (CI-friendly).

---

## [4.6.0] — branch `improve`

### Package manager — dependency write-back

- **`sz install <pkg>`** now records the resolved dependency in `serez.json` (insert or update), so installing by command keeps the manifest in sync — matching the behavior of `npm install <pkg>` / `cargo add`. Previously the manifest was read-only and only `sz install` (no args) consumed it.
- **`sz uninstall <pkg>`** now removes the dependency from `serez.json` as well.
- The manifest edit is **surgical**: only the `dependencies` object is rewritten (canonical 2-space layout); `name`, `version`, `scripts`, `permissions` and the rest of the file's formatting are preserved verbatim. Brace matching honors `{`/`}` inside string values.
- `sz install` (no args, installs from the manifest) does **not** rewrite `serez.json`, so hand-written version specs are never clobbered.
- Manifest write failures are non-fatal: the package is already on disk, so the install/uninstall reports a warning instead of failing. With no `serez.json` present, `sz install` hints to run `sz init`.

### Tests

- 7 new Rust unit tests in `package_manager` (upsert into empty deps, append, update-in-place, insert missing `dependencies` key, preserve `scripts` block, brace-in-string handling, remove round-trip). Module suite: 14/14 pass.

---

## [4.5.0] — branch `core-websocket` → merged to `improve`

### WebSocket support (RFC 6455)

- **`Crypto.sha1(s)`** — SHA-1 hash, returns 40-char lowercase hex. Pure-Rust implementation, no external crates. Validated against RFC 3174 test vectors.
- **`Crypto.sha1base64(s)`** — SHA-1 followed by base64 encode of the raw digest. Used for the WebSocket handshake `Sec-WebSocket-Accept` key. Validated against the RFC 6455 §1.3 vector.
- **`Socket.recvWsFrame(conn_id)`** → `string | null` — decodes one WebSocket frame (RFC 6455): parses header, extended length, unmasks payload. Returns `null` on close frame.
- **`Socket.sendWsFrame(conn_id, data)`** → `null` — encodes `data` as an unmasked text frame (server → client) with correct 1-byte / 2-byte / 8-byte length encoding.
- **`Socket.listen(port)`** — now binds to `0.0.0.0` instead of `127.0.0.1`, allowing external connections (e.g. inside Docker via serez-apipack).

### WebSocket protocol hardening (5 bugs fixed)

- **DoS — unbounded payload** — a frame claiming `payload_len = 2^63` would allocate `vec![0; huge]` and crash. Now capped at `WS_MAX_PAYLOAD` (16 MiB), enforced on both the 1-byte and 8-byte extended-length paths before allocation.
- **Ping not answered** — `opcode=9` (ping) was returned as data. Real browsers close the connection on missing pong. Now auto-replies with `opcode=10` (pong) carrying the same payload, then loops to read the next data frame. Loop (not recursion) avoids stack overflow on repeated pings.
- **Close frame stream desync** — `opcode=8` returned before reading the close code + reason, leaving bytes in the TCP buffer that corrupted the next read. Now the payload is fully consumed before returning `null`.
- **RSV bits not validated** — RFC 6455 §5.2 requires RSV1/2/3 = 0 without a negotiated extension. Now rejects frames with any RSV bit set.
- **Invalid UTF-8 silently mangled** — text frames used `from_utf8_lossy` (replacing bad bytes with U+FFFD). RFC 6455 §5.7 requires an error. Now returns an error on invalid UTF-8. Control frames with payload > 125 bytes are also rejected (§5.5).

### Tests

- `unit_websocket` (13), `unit_sec_websocket` (13), `sec_websocket`, `54_websocket_e2e`, `55_websocket_integral`, `62_websocket_full_integral` (33 assertions), plus 8 Rust `ws_frame_tests`. Full suite: 327 `.sz` tests, 0 failures.

---

## [4.3.2] — branch `ai-deep` → merged to `improve`

### AI / Autodiff — Phase 1: Core training infrastructure

- **Optimizers** — `Autodiff.adamStep`, `adamwStep`, `sgdStep`, `rmspropStep`. All are pure functions that take current params + state and return `[new_param, new_state...]`. No tape side-effects.
- **Loss functions** — `Autodiff.mseLoss`, `maeLoss`, `bceLoss`, `crossEntropyLoss`. All tracked on the tape with correct backward passes.
- **Weight initialization** — `Autodiff.xavierUniform`, `xavierNormal`, `heUniform`, `heNormal`. Fan-in/fan-out computed automatically from shape (2D: `[out, in]`; 4D conv: `[cout, cin, kH, kW]`).
- **Gradient clipping** — `Autodiff.clipGrad(grad, max_norm)` per-tensor; `clipGradNorm(grads_array, max_norm)` global norm across a list of tensors.

### AI / Autodiff — Phase 2: Regularization & modern layers

- **BatchNorm** — `Autodiff.batchNorm(x, gamma, beta, training, [eps])`. Full backward: per-feature gradient for `gamma`, `beta`, and input. Input must be `[N, C]`.
- **Dropout** — `Autodiff.dropout(x, p, [training])`. Inverted dropout (divides by keep_prob in forward). Mask saved for backward. `training=false` → no-op.
- **Embedding** — `Autodiff.embedding(indices, weight)`. Gathers rows from `[vocab, emb_dim]` weight. Backward scatters gradients back to touched rows. `vocab_size` stored in `TapeOp` to avoid inference issues.
- **New activations (all tracked):**
  - `t.elu([alpha])` — ELU with correct `alpha * exp(x)` backward
  - `t.swish()` / `t.silu()` — swish with `(sigmoid + x*sigmoid*(1-sigmoid))` backward; stores both `x` and `sigmoid(x)`
  - `t.mish()` — mish with `tanh(sp) + x*sech²(sp)*sigmoid(x)` backward
  - `t.gelu()` — GELU now tracked with full `d/dx` backward (was untracked before)
  - `t.leaky_relu(alpha)` — now tracked (was untracked before)
- **AvgPool2d** — `t.avg_pool2d(kernel, stride)`. Uniform gradient distribution in backward.
- **Tensor utilities** — `.variance()`, `.std()`, `.cumsum()`, `.softplus()`, `.hardsigmoid()`, `.hardswish()`

### AI / Autodiff — Phase 3: N-D operations & performance

- **Shape manipulation** — `t.unsqueeze(dim)`, `t.squeeze()`, `t.squeeze(dim)`, `t.permute([axes])` (full N-D generalized transpose)
- **N-D broadcasting** — `t.broadcastTo([shape])`, `t.broadcastAddNd(other)`, `t.broadcastMulNd(other)`. Full numpy semantics for arbitrary dimensions.
- **Batch matmul** — `t.bmm(other)`: `[B,N,M] @ [B,M,K] → [B,N,K]`
- **N-D reduce** — `t.reduceSum(axis)`, `t.reduceMean(axis)`, `t.reduceMax(axis)` for any tensor dimension
- **Element-wise ops** — `t.sign()`, `t.reciprocal()`, `t.sin()`, `t.cos()`, `t.round()`, `t.floor()`, `t.ceil()`, `t.maximum(other)`, `t.minimum(other)`
- **stopGrad / detach** — `t.stopGrad()`, `t.detach()`, `Autodiff.stopGrad(tensor)` — returns a copy disconnected from the tape

### AI / Autodiff — Weight persistence

- **`Autodiff.saveWeights(path, tensors)`** — saves an array of tensors to a `.szw` binary file (magic `SZWT` + version + count + per-tensor: ndim, shape, data as f64 LE)
- **`Autodiff.loadWeights(path)`** — reads `.szw` and returns `Array` of tensors in the same order. Full round-trip precision (float64).

### Autodiff bug fixes

- **`TapeOp::BroadcastMul` backward** — was incomplete (only accumulated gradient to `mat_id`, skipped `rhs_id`). Now saves both `mat_data` and `rhs_data` in forward, computes `d_mat` and `d_rhs` correctly.
- **`TapeOp::Swish` backward** — was reconstructing `x` from `sigmoid(x)` via logit (numerically unstable). Now stores `cached_input` alongside `cached_sigmoid`.
- **`TapeOp::Gelu`** — GELU was not tracked at all. Added `TapeOp::Gelu` with correct backward.
- **`leaky_relu`** — was not recorded on the tape. Now records `TapeOp::LeakyRelu`.
- **`TapeOp::Embedding`** — backward was inferring vocab size heuristically. Now stores `vocab_size` explicitly in the op.
- **`TapeOp::Swish` shape** — added `cached_input: Vec<f64>` field to the variant.

### Dict bug fix (B-31 complete)

- **Typed dict missing-key access** — `d["missing"]` on a `<string, int>` dict was still throwing `❌ ERROR: Key not found in typed dict` instead of returning `null`. The B-31 fix was only applied to `value_type == "any"` dicts. Now all dicts return `null` for missing keys regardless of type annotation.
- **`dict["key"].push(val)` writeback** — calling mutating array methods on a value retrieved from a dict (`grupos["pares"].push(n)`) now writes the modified array back to the dict automatically. Previously the modification was silently discarded.
- **`plant` → `plant_global`** for dict value access — prevents dangling refs when the dict lives in an outer scope.

### Package manager

- **`sz init`** — creates a `serez.json` interactively in the current directory. Prompts for name (default: folder name), version, description, author.
- **`sz init --y`** — non-interactive: uses folder name as project name, all defaults, no prompts.
- **`sz run <script>`** — reads `serez.json` and executes the named script entry (e.g. `sz run dev` → runs `sz index.sz`). Reports error with available scripts if name not found.
- **`scripts` field in `serez.json`** — new manifest field, parsed alongside `dependencies` and `permissions`.

### stdout flush fix

- **`stdout` buffer** — `run_file()` now explicitly flushes `stdout` before returning. On Windows, large output from the spawned interpreter thread could appear after the shell prompt due to unflushed buffered writes. Regression test: `49_stdout_flush` (200 output lines).

### Test count

- **321 passing** (0 failing) across E2E, unit, error, security, AI, CLI, and package manager tests.
- New test files: `ai_phase1_training.sz`, `ai_phase2_layers.sz`, `ai_phase3_ops.sz`, `ai_weights_persistence.sz`, `49_stdout_flush.sz`.

---

## [4.1.2] — branch `improve`

### Package manager

- `sz init` / `sz run` / `scripts` field (see v4.3.2 above — backfilled from ai-deep merge)

---

## [4.0.1] — branch `improve`

### Networking / stdlib

- **Default `User-Agent`** — `fetch` now sends `User-Agent: Serez-Code/<version>` unless the caller sets one in `headers`. Without it, ureq sends `ureq/x.y`, which some CDNs/WAFs answer with `503`; an identifiable UA avoids those spurious failures. A caller-provided `User-Agent` always wins. (`src/evaluator/builtins.rs`, `eval_fetch`.)

### JSON

- **`JSON.pretty(value, [indent])`** — pretty-prints values as indented JSON (default **2** spaces per level; `0` falls back to compact). When given a raw JSON string — such as a `fetch` response body — it parses it first and re-indents, so `JSON.pretty(fetch(url))` prints formatted output directly; non-JSON strings are kept as-is. `JSON.stringify` is unchanged (still compact, single-line). Implemented in `src/evaluator/mod.rs` (`json_pretty_owned` / `json_pretty_inner`) + `src/evaluator/namespaces.rs`.

### Docs

- Documented the `fetch` HTTP client (signature, default headers incl. the new `User-Agent`, options dict, `full`/`binary` modes, throw-on-4xx/5xx) and `JSON.pretty` in `README.md`.

### Fixes

- **`unit_native_fns.sz` parsing** — the POST test embedded a JSON body with an unescaped `{`, which serez treats as string-interpolation start. That silently aborted parsing of the rest of the file, so the POST test (and any added after it) never ran while the runner still reported the file as passing (parser errors go to stderr; the runner only greps stdout for `[FAIL]`). Escaped as `\{` so the whole file parses and executes.
- **`43_fetch_full_e2e` flakiness** — the test hit httpbin.org, which intermittently returns 503; since `full` mode does not throw on HTTP status, a 503 left `status="unknown"` and the test failed. Switched the endpoint to PokeAPI (`/api/v2/pokemon/ditto`) — a stable, CDN-backed service that consistently returns 200 — and tightened the assertions to check the *real* response (`status == 200`, `ok == true`, `statusText`/`headers` present, body contains `ditto`), so it actually exercises status-line/header/body parsing. Still degrades gracefully (`network_error`) on a genuine outage.

### Test count

- 310 passing (0 failing) — added `unit_json_pretty` (10 `JSON.pretty` cases) and two `fetch` User-Agent tests in `unit_native_fns`.

---

## [4.0.0] — branch `improve`

### Networking / stdlib

- **`fetch` is now a complete general-purpose HTTP client.** Previously `fetch(url, [method], [body])` always sent a hardcoded `Content-Type: application/json`, had a fixed 10 s timeout, threw on any status ≥ 400 (discarding the response body), only supported GET/POST/PUT/PATCH/DELETE, and corrupted binary responses via `from_utf8_lossy`. It now accepts an optional **options dict** after the url — `fetch(url, [method], [body], options)` — where `options` is a serez dict (e.g. `({"full", true})`):
  - `headers` — a `<string, string>` dict of request headers (enables `Authorization`, `Accept`, cookies, custom headers, …). Names/values containing control chars (`\n` `\r` `\0`) are rejected to prevent CRLF / header injection. A user-set `Content-Type` overrides the default (which is now only applied when a body is sent and the user didn't set one).
  - `timeout` — request timeout in seconds (default **60**, was 10; connect capped at 30).
  - `full` — when `true`, returns a `<string, any>` dict `{ status, ok, statusText, headers, body }` and does **not** throw on HTTP status, so 4xx/5xx (404, 429, 529, …) can be inspected. `headers` is a `<string, any>` dict keyed by lowercased name; a missing key reads as `null`.
  - `binary` — when `true`, the body is returned as a byte array `[int]` (0-255) instead of a UTF-8 string, so images / zips / PDFs download intact. Decode with `Binary.toUtf8` / `Binary.toHex`.
  - Default (no options) behaviour is unchanged: returns the body string and throws on status ≥ 400 — now with the response body embedded in the thrown message instead of just the status code.
  - Any HTTP method is accepted (incl. HEAD/OPTIONS) via `Agent::request`. Arguments are sniffed by type: the first string after the url is the method, the second is the body, and a dict is the options — so `fetch(url, opts)`, `fetch(url, "POST", opts)` and `fetch(url, "POST", body, opts)` all work. 100% backward compatible; `native fn` declarations are unaffected.
  - Implemented in `src/evaluator/builtins.rs` (`eval_fetch` + `fetch_make_value`).

### Test count

- 309 passing (0 failing) — added `43_fetch_full_e2e`, `44_fetch_binary_e2e`, `sec_fetch_header_injection`.

---

## [3.8.4] — branch `improve`

### Tooling / diagnostics

- **Arena stats** — `Evaluator::arena_stats()` returns the current object-slot counts of the two arenas `(global, scoped)`. When the program is run with the environment variable `SEREZ_ARENA_STATS` set, a line `[arena] global=N scoped=M` is printed to stderr at exit. Read-only diagnostic for measuring memory behaviour of the Region-Based Memory (e.g. confirming that scoped loops stay flat and which patterns promote to the never-freed global arena). **Not a GC and not an optimization** — zero runtime overhead unless the env var is set (a single `env::var` lookup at exit). Used to characterize the closure/escaping-container promotion-to-global behaviour (documented; the GUI memory discipline belongs to serez-ui, not the core).

---

## [3.8.3] — branch `improve`

### Bug fixes

- **B-84** — Parenthesized single-parameter arrow lambda failed to parse. `(x => body)` raised `Expected ')' in grouped expression`, even though `(x) => body`, bare `x => body`, and `(a, b) => body` all parsed. After consuming `(` and a leading identifier, the parser matched `,` (multi-param), `)` (`(a)`/`(a) => …`) and a catch-all that assumed a grouped expression — so a following `=>` (Arrow) was never recognized. Added an explicit `Arrow` arm that parses `( ident => body )` as a parenthesized single-param lambda. This unblocks common forms like `5 |> (x => x * 2)`, `((x => x + 1))(5)`, and `let f = (x => …)`. New regression: `unit_paren_lambda` (6 cases). Found while fuzzing pipe/lambda syntax.

### Test count

- 306 passing (0 failing) — added `unit_paren_lambda`.

---

## [3.8.2] — branch `improve`

### Bug fixes

- **B-83** — Inconsistent lambda capture: scope-dependent snapshot vs. live reference. Lambdas snapshot scoped locals (`capture_env` extracts + plants them to the global arena at creation), but variables referenced from a lambda that live in the **global** arena (top-level `let`s) were resolved *live* at call time. So the exact same lambda captured locals by value but globals by reference, depending only on where it was written: `let x=10; let f=()=>x; x=20; f()` gave `20` at top level but `10` inside a function; `while (i<3){ fns.push(()=>i); i=i+1 }` gave `3 3 3` at top level but `0 1 2` inside a function. Fixed with `capture_lambda_env`: in addition to the existing local snapshot, a best-effort free-identifier walk of the lambda body now also snapshots referenced **global data variables** at creation. Global **functions** are intentionally skipped (kept live) so recursion and late binding keep working. The walk only ever *adds* snapshots — an unhandled construct simply degrades to the previous live-lookup behavior, so it cannot break a valid closure (the whole suite is unchanged). New regression: `45_closure_capture_e2e`.

### Test count

- 305 passing (0 failing) — added `45_closure_capture_e2e`.

---

## [3.8.1] — branch `improve`

### Bug fixes

- **B-82** — Nested arrays corrupted when reassigning an outer-scope variable from inside a nested block. The shared scoped arena is a single stack rewound on block exit (`pop` → `reset_to`). A plain variable assignment (`x = value`) stored a *shallow* clone of the value's `ObjectData`: for an array/dict/set it copied the inner `ObjectRef`s, which could point into a deeper block's region. When that inner block popped, the inner refs dangled — the container's `.length()` stayed correct but indexing an element read a truncated/reused slot (symptom: `is array` == false, "Index operator not supported"). `push`/index-assign/dict-value-assign already promoted to the global arena at `depth > 1`, but plain variable assignment was missed. Fixed by `promote_container_for_assign`: when assigning a heap container (Array/Dict/Set) to a variable from inside a nested scope, the value is deep-promoted to the global arena so its elements outlive inner-block pops. Scalars and instances (fields are `OwnedValue`) are untouched — no effect on loop counters like `i = i + 1`. Found while building serez-ui's `.szs` CSS parser. New regression: `unit_nested_array_assign` (4 cases).

### Test count

- 303 passing (0 failing) — added `unit_nested_array_assign`.

---

## [2.1.0] — branch `improve`

### New features

**Fase 1 — Memory namespace: raw byte heap**

- `Memory` namespace: `sizeof`, `alloc`, `free`, `size`, `read`, `write`, `copy`, `fill`, `offsetOf`.
- `Memory.sizeof(type)` — returns byte-size of a primitive type name (`"int"`, `"bool"`, `"float32"`, etc.).
- `Memory.alloc(n)` → int handle — allocates `n` bytes of zeroed memory in a `HashMap<i64, Vec<u8>>` heap stored on the evaluator; requires `unsafe {}` block.
- `Memory.read(handle, offset, type)` / `Memory.write(handle, offset, type, value)` — typed read/write at a byte offset; require `unsafe {}`.
- `Memory.copy(src, dst, n)` — copies `n` bytes between two allocations; requires `unsafe {}`.
- `Memory.fill(handle, byte)` — fills an entire allocation with a byte value; requires `unsafe {}`.
- `Memory.offsetOf(class_name, field_name)` — returns word-aligned field offset (8-byte stride) by looking up the class registry.
- New evaluator fields: `memory_heap: HashMap<i64, Vec<u8>>`, `memory_heap_next_id: i64`.
- New source file: `src/evaluator/namespaces_memory.rs`.

**Fase 1.5 — unsafe as expression + new built-in globals**

- `unsafe { ... }` can now be used as an expression, enabling patterns like `let h = unsafe { Memory.alloc(64) }`. AST: `Expression::UnsafeBlock(BlockStatement)`. Parser: expression-level dispatch in `parse_expression`. Evaluator: delegates to `eval_unsafe_block`.
- `time()` built-in — returns current Unix timestamp in milliseconds as `int`.
- `env(name)` built-in — reads an environment variable by name; returns empty string if not set.
- `exit(code)` built-in — terminates the process with the given exit code (`std::process::exit`).
- `native fn` dispatch: when a declared native function is called but has no Rust implementation registered, a clear error is now printed.

**Fase 2 — Extended Tensor math**

- **Activation functions** (element-wise, return new Tensor): `relu`, `sigmoid`, `tanh`, `softmax`.
- **Element-wise math**: `abs`, `sqrt`, `exp`, `log`, `pow(exp)`.
- **Norms**: `norm()` (L2, default) / `norm(1)` (L1) — returns a Decimal.
- **Clamp**: `clamp(min, max)` — clips all elements to `[min, max]`.
- **Broadcast add**: `broadcastAdd(bias)` — adds a 1D tensor to each row of a 2D tensor `(m, n) + (n,)`.

### Bug fixes

- **B-75** — Keyword token as method name rejected by class parser: methods named `get`, `set`, or `static` (lexed as `KwGet`/`KwSet`/`KwStatic`) were unconditionally rejected by the `Ident`-only check in `parse_class_declaration`. Fixed by extracting `token_type_is_name()` helper and using `current_token_is_name()` at the method-name check point.
- **B-76** — `Tensor.sum()` on empty tensor returned `-0.0`: Rust's `Iterator::sum` initialises the accumulator with `0.0_f64` and produces negative zero on empty input. Fixed by adding an `is_empty()` early-return guard matching the pattern already used by `Tensor.mean()`.
- **B-65 assertion corrected** — `Math.round(-4.5)` returns `-5` (Rust "half away from zero"), not `-4`. Test expectation updated.
- **`unit_class_arch` assertion corrected** — `pts.find(p => p.sum() > 6)` returns the first match (x=3), not the last (x=5). Test expectation updated.

### New parser feature

- **Enum.Variant in match patterns** — `match dir { case Direction.North => ... }` now works. The parser detects `Ident.Ident` in match position and creates a `MatchPattern::Literal(DotCall)`, evaluated at runtime by the existing literal-pattern path.

### Test count

- 274 passing (0 failing) — added: `unit_memory`, `unit_native`, `unit_tensor_math`, `56_memory_e2e`, `57_tensor_math_e2e`, `unit_match_enum`, `unit_bug_b64_b74`, `unit_math_trig`, `unit_memory_offsetof`, `unit_tensor_ops`, `unit_set_ops` (extended), `unit_bug_b75_b76`, `unit_class_arch` (extended), `sec_memory_requires_unsafe`, `sec_memory_write_requires_unsafe`, `sec_memory_read_requires_unsafe`, `sec_memory_free_requires_unsafe`, `sec_json_invalid`, `59_integral2_e2e`.

---

## [2.0.2] — branch `improve`

### New features

**Fase 2.5 — serez-sec: Socket and Binary namespaces**

- `Socket` namespace: `connect`, `send`, `recv`, `close`, `listen`, `accept` — raw TCP over `std::net::TcpStream` / `TcpListener`. Socket IDs (int) stored in the evaluator's registry; usable from Serez code as `Socket.connect("host", port)`.
- `Binary` namespace: byte-array utilities — `fromHex`, `toHex`, `fromUtf8`, `toUtf8`, `packInt32Le`, `packInt32Be`, `unpackInt32Le`, `unpackInt32Be`, `packInt64Le`, `unpackInt64Le`, `concat`. All operate on Serez integer arrays (values 0–255).
- Tests: `tests/53_socket_e2e.sz`, `tests/unit_binary.sz`, `tests/unit_socket.sz` (42 new test cases).

**Fase 4 — GPU compute (CPU-backed)**

- `GPU` namespace: `createBuffer`, `createBufferFromArray`, `readBuffer`, `freeBuffer`, `fill`, `size`, `map`, `reduce`, `dot`, `axpy`, `matmul`. Buffers are flat `Vec<f64>` stored in the evaluator. API mirrors GPU compute patterns (create/upload/dispatch/readback/free) so a future backend can swap to real GPU calls with no language changes.
- Tests: `tests/54_gpu_e2e.sz`, `tests/unit_gpu.sz` (13 new test cases).

**Fase 6 — Package manager**

- `src/package_manager.rs`: `SerezManifest` JSON parser (hand-rolled, no external crate), `install_package(spec)`, `install_all()`, `packages_dir()` / `registry_dir()` (support `SEREZ_PACKAGES` / `SEREZ_REGISTRY` env vars for testing).
- `sz install [pkg@version]` CLI subcommand: without argument reads `serez.json` and installs all dependencies; with argument installs a specific package from the registry.
- Import resolution now searches `packages_dir()` (and falls back to `~/.serez/packages/`) after all existing search paths. Also supports `<pkg>/index.sz` layout so `import "pkg-name"` resolves to `packages/pkg-name/index.sz`.
- `run_tests.ps1` / `run_tests.sh`: set `SEREZ_PACKAGES=tests/packages` so package tests run correctly against local test packages.
- Tests: `tests/55_packages_e2e.sz`, `tests/unit_packages.sz` (13 new test cases). Test packages: `tests/packages/math-helpers/`, `tests/packages/string-tools/`.
- Rust unit tests in `package_manager.rs` verify manifest parsing and pkg-spec parsing.

### Test count

- 214 → 256 passing (0 failing).

---

## [2.0.1] — branch `improve`

### Bug fixes

**B-64 — `abs(i64::MIN)` overflow** (`src/evaluator/builtins.rs`)
- Before: called `.abs()` on `i64::MIN` — overflows in release mode (|i64::MIN| > i64::MAX).
- Now: uses `i64::checked_abs()` — returns an error for `i64::MIN`.

**B-65 — `floor` / `ceil` / `round` / `trunc` UB on non-finite f64** (`src/evaluator/builtins.rs`)
- Before: casting `f64::INFINITY`, `f64::NEG_INFINITY`, or `f64::NAN` to `i64` via `as i64` is undefined behavior in Rust.
- Now: each function validates `!v.is_nan() && !v.is_infinite()` before casting.

**B-66 — `Math.random()` only produced values in `[0, ~0.5)`** (`src/evaluator/namespaces.rs`)
- Before: LCG state shifted right 33 bits (31-bit range `[0, 2³¹)`) divided by `u32::MAX` (2³²−1) — maximum ≈ 0.5.
- Now: divides by `1u64 << 31` to produce the documented `[0, 1.0)` range.

**B-67 — `asin` / `acos` accepted out-of-domain arguments** (`src/evaluator/builtins.rs`)
- Before: any `f64` was accepted — inputs outside `[-1, 1]` silently produced `NaN`.
- Now: validates `v >= -1.0 && v <= 1.0` before calling the intrinsic.

**B-68 — `JSON.stringify` emitted invalid JSON for `NaN` / `Infinity`** (`src/evaluator/mod.rs`)
- Before: non-finite `f64` values were formatted with Rust's `Display`, producing `"inf"`, `"-inf"`, or `"NaN"`.
- Now: `if !d.is_finite() { return "null".to_string(); }` per the JSON specification.

**B-69 — `call_function` (map / filter / sort callbacks) rejected default and rest parameters** (`src/evaluator/mod.rs`)
- Before: arity checked as `arg_count != params.len()` and parameters bound via `args[i]` direct indexing.
- Now: computes `required_count`, checks `arg_count >= required` with upper bound for non-rest, binds defaults and collects rest parameter into an array.

**B-70 — `min_params` formula wrong for functions with default + rest parameters** (`src/evaluator/expr.rs`)
- Before: `if has_rest { params.len() - 1 } else { required_count }` — gives wrong count when both rest and defaults are present.
- Now: `let min_params = required_count` in all cases.

**B-71 — `super()` constructor call rejected default and rest parameters** (`src/evaluator/classes.rs`)
- Before: `eval_super_call` used strict arity and `args[i]` direct indexing.
- Now: same default/rest parameter handling as `call_function`.

**B-72 — `new ClassName()` constructor call rejected default and rest parameters** (`src/evaluator/classes.rs`)
- Before: `eval_new_class` used strict arity and direct indexing for constructor binding.
- Now: same default/rest parameter handling.

**B-73 — `super.method()` call rejected default and rest parameters** (`src/evaluator/classes.rs`)
- Before: `eval_super_method_call` used strict arity.
- Now: same default/rest parameter handling.

**B-74 — `invoke_method` rest parameter not collected** (`src/evaluator/classes.rs`)
- Before: parameter binding loop did not handle rest parameters — extra arguments beyond the last named param were silently discarded.
- Now: rest parameter is collected from `args[i..]` into an `Array` and declared in scope.

### Version

- `Cargo.toml`: `2.0.0` → `2.0.1`

---

## [2.0.0] — branch `improve`

### Breaking changes

**`pop()` on empty array is now a runtime error (Bug 1)**
- Before: returned `null` silently
- Now: `❌ ERROR: pop() called on an empty array`
- Rationale: silent null masked logic bugs where callers expected a real value

**`shift()` on empty array is now a runtime error (Bug 2)**
- Before: returned `null` silently
- Now: `❌ ERROR: shift() called on an empty array`
- Rationale: same as pop() — silent null was undetectable

**`2 ** 63` and exponent overflow are now runtime errors (Bug 3)**
- Before: f64 precision caused `2 ** 63` to silently return `i64::MAX` instead of detecting overflow
- Now: uses `i64::checked_pow` — exact overflow detection with no floating-point rounding
- Now: `❌ ERROR: Integer overflow in exponentiation`
- Base 0, 1, -1 at any exponent are still handled correctly (no overflow possible)
- Decimal exponent path (`2 ** 63.0`) is unchanged — goes through `f64::powf`

**Typed dict missing key is now a runtime error (Bug 4)**
- Before: `d["missing"]` on a `<K, V>` dict (V ≠ `any`) silently returned `null`
- Now: `❌ ERROR: Key 'missing' not found in typed dict <_, V>`
- Untyped dicts (`<K, any>`) still return `null` for missing keys — no change

### Distribution

- **Release pipeline**: GitHub Actions workflow builds binaries for Windows x64, Linux x64 (static musl), macOS ARM64, macOS x64 on every version tag and publishes them to GitHub Releases
- **`install.sh`**: one-line installer for Linux and macOS — auto-detects OS and arch, installs to `~/.local/bin/sz`
- **`install.ps1`**: one-line installer for Windows — downloads to `%LOCALAPPDATA%\SerezCode\bin\sz.exe` and adds to user PATH
- **CI workflow** (`ci.yml`): builds on `main` and `integration` on every push and pull request

### Tests (214 total, 0 failures)

- `41_bug_fixes_e2e.sz` — E2E integration test covering all 4 bug fixes (Queue, SafeStack, safePow2, Registry, game loop)
- `unit_bug_fixes.sz` — 21 unit tests for positive regression across all 4 fixes
- `sec_pop_empty_array.sz`, `sec_shift_empty_array.sz`, `sec_typed_dict_miss_key.sz`, `sec_power_2_63.sz` — security tests verifying each fix produces the correct error
- `unit_sec_pentest_bugs.sz` — 16 penetration tests with boundary exhaustion, alternating cycles, power edge cases, dict key patterns
- `run_tests.ps1` — new `-cli` flag runs 12 tests covering CLI flags (`--version`, unknown flags, non-.sz), REPL behavior (arithmetic, variable persistence, function definition, error recovery), and `--check` mode output

### Native backend (foundation — not yet connected to runtime)

- `src/compiler/types.rs` — compile-time type system (`SzType`) mapping Serez types to LLVM types
- `src/compiler/hir.rs` + `hir_lower.rs` — AST → HIR lowering with full desugar pass
- `src/compiler/mir.rs` + `mir_lower.rs` — HIR → MIR three-address code with basic blocks
- `src/compiler/llvm_emit.rs` — MIR → LLVM IR text emission (74 tests passing)

---

## [1.0.0] — VS Code formatter and CI

### VS Code — Formatter (`vscode-serez` v0.2.0)

**`extension.js`** — new `DocumentFormattingEditProvider`:
- Auto-indentation with 4 spaces per level, based on `{` and `}` counting
- Ignores braces inside string literals and line comments (`//`)
- `} else {` handled correctly: dedent before printing, indent after
- Collapses consecutive blank lines into one
- Removes trailing whitespace from all lines
- File always ends with exactly one `\n`

**`package.json`** — version `0.2.0`:
- `"main": "./extension.js"` and `"activationEvents": ["onLanguage:serez"]`
- `Formatters` category added
- `configurationDefaults` for `.sz`: `editor.defaultFormatter` and `editor.formatOnSave: true` enabled automatically

**Usage:** `Shift+Alt+F` to format manually, or save the file (formatOnSave).  
**Rebuild:** `vsce package` in `vscode-serez/` generates `serez-code-0.2.0.vsix`.

---

### CI / Tooling
- `release.yml`: permissions scoped per job — only `host` has `contents: write`; others have `contents: read`
- `.github/dependabot.yml`: automatic weekly updates for GitHub Actions and Cargo dependencies
- `run_tests.sh`: Bash script equivalent to `run_tests.ps1`, with `--filter`, `--generate`, `--unit`, `--e2e`, `--security` flags; ANSI colors; CRLF normalization; unique temp files per process
- Evaluator refactored from a single `evaluator.rs` (5300+ lines) to 12 submodules:

| Module | Responsibility |
|---|---|
| `mod.rs` | Main entry, Flash Scope protocol, StoredMethod cache, static profiler |
| `stmt.rs` | Statement evaluation (let, assign, for, while, return, …) |
| `expr.rs` | Expression evaluation (calls, index, dot, ternary, …) |
| `ops.rs` | Infix and prefix operators |
| `check.rs` | Type-check helpers (parameters, return, typed arrays) |
| `builtins.rs` | Global functions (parseInt, parseDecimal, readLine, …) |
| `classes.rs` | Instantiation, method dispatch, inheritance, super |
| `methods_array.rs` | Array methods (push, pop, map, filter, reduce, sort, …) |
| `methods_string.rs` | String methods (split, replace, trim, padStart, …) |
| `methods_set.rs` | Set methods (add, has, delete, toArray, union, …) |
| `namespaces.rs` | Built-in namespaces (Math, File, JSON) |
| `control.rs` | Control flow helpers (break, continue, labeled loops, do-while) |

### Demo apps
- `apps/01_task_manager.sz` — enum, inheritance, static methods, switch, HOF, try/catch
- `apps/02_statistics.sz` — typed arrays, Math, map/filter/reduce, Pearson correlation
- `apps/03_text_analyzer.sz` — string methods, dicts, Caesar cipher, File I/O
- `apps/04_bank_system.sz` — abstract class, sealed, interface, const, getters, optional chaining
- `apps/05_data_pipeline.sz` — JSON, File, Set, bitwise/power ops, pipeline HOF

---

## [0.1.0] — Language history

### Phase 5 — Bug fixes and semantics (B-62 to B-63)

**`reverse()` — in-place mutation with return (B-62)**
- Before: `reverse()` returned void, was not chainable
- Now: mutates the array in-place AND returns the same array — allows `let sorted = arr.reverse()`

**`trimLeft` / `trimRight` as aliases (B-63)**
- Added as aliases for `trimStart` / `trimEnd` for compatibility

---

### Phase 4 — Critical bug fixes (B-54 to B-61)

**`is` operator — full fix (B-61)**
- Bug: `is` was tokenized as an identifier, never worked as an infix operator
- Fix: `KwIs` token added; registered in `token_precedence()` and in the parser's `is_infix` match; `eval_infix` handler added in the evaluator
- `null is null` also fixed: missing case `("null", ObjectData::Null)` in `type_matches`

**Named function capture semantics (B-58)**
- Before: `fn` declarations captured the value at definition time (snapshot)
- Now: `fn` declarations use reference semantics — rebind of the shared global slot
- Lambdas maintain snapshot semantics (no changes)
- `ScopeStack::rebind()` added for selective rebinding of outer scope

**Dict mutation from nested scope (B-57)**
- Bug: arena lifetime — a new entry in a dict mutated from inside a function stayed in the local scope and was destroyed on exit
- Fix: `plant_global` used when `depth > 1`

**`padStart` / `padEnd` — incorrect early return (B-56)**
- Bug: if the string already had the target length, it returned empty instead of returning the original string
- Fix: early return corrected

**Shift validation (B-55)**
- `1 << 64` and `8 >> -1` were silently incorrect
- Now they are runtime errors: negative or ≥ 64 shift throws an error

**`flat(n)` — depth parameter (B-54)**
- Before: only supported `flat()` with depth 1
- Now: `flat(n)` recursively flattens `n` levels; `flat()` is equivalent to `flat(1)`

**Getter-only — write error (B-53)**
- Attempting to assign to a property that only has `get` (without `set`) is now a runtime error

---

### Phase 3 — New language features

#### Operators

**Power operator `**`**
- `2 ** 10` → `1024`; works with `int` and `decimal`
- Higher precedence than `*` / `/` / `%`
- `0 ** 0` → `1` (mathematical convention)

**Bitwise operators**
- `&` AND, `|` OR, `^` XOR, `~` NOT (prefix), `<<` left shift, `>>` arithmetic right shift
- Only for `int` (64-bit signed, two's complement)
- Negative or ≥ 64 shift is a runtime error
- Binary (`0b1010`) and hexadecimal (`0xFF`) literals supported
- Numeric separators: `1_000_000`, `0xFF_FF`

**Optional chaining `?.`**
- `obj?.method()` / `obj?.field` — if `obj` is `null`, returns `null` without error
- Chainable: `a?.getNext()?.getValue() ?? 0`
- Combinable with `??` for fallback

#### Control flow

**`do-while`**
- The body executes at least once
- `break` and `continue` work the same as in `while`/`for`

#### Classes

**Static methods**
- `public static T method(args)` in classes
- Called as `ClassName.method(args)` — no instance required
- No access to `this`

**Parameters with default values**
- `fn int add(int a, int b = 10)` — if the caller omits the argument, the default is used
- The default is an arbitrary expression evaluated at call time
- The type checker handles variable arity (skip if there are defaults)

**Abstract classes**
- `abstract class Foo` — not directly instantiable; runtime error on `new`
- Methods without a body declared for override in subclasses

**Sealed classes**
- `sealed class Foo` — not inheritable; attempting to extend it is a runtime error

**Getters and setters**
- `public get T prop()` — called automatically when reading `obj.prop` (without parentheses)
- `public set prop(T val)` — called automatically when assigning `obj.prop = val`
- Property with only getter is read-only; writing to it is a runtime error

**Class fields with default values**
- `field: type = value` in the class body

#### Arrays — new methods

| Method | Description |
|---|---|
| `.find(cb)` | First element where `cb` returns `true`, or `null` |
| `.findIndex(cb)` | Index of the first element matching the predicate, or `-1` |
| `.every(cb)` | `true` if `cb` is `true` for all elements |
| `.some(cb)` | `true` if `cb` is `true` for at least one |
| `.slice(start, end)` | New array from `start` (inclusive) to `end` (exclusive) |
| `.flat(n?)` | Flattens `n` nesting levels (default 1) |
| `.reverse()` | Reverses in-place, returns the same array |
| `.indexOf(val)` | Index of the first occurrence, or `-1` |
| `.includes(val)` | `true` if the array contains the value |
| `.remove(idx)` | Removes and returns the element at `idx` |

#### Strings — new methods

| Method | Description |
|---|---|
| `.padStart(n, ch?)` | Pads the start with `ch` (default space) up to length `n` |
| `.padEnd(n, ch?)` | Pads the end with `ch` (default space) up to length `n` |
| `.slice(start, end?)` | Substring with negative index support |
| `.trimStart()` / `.trimLeft()` | Removes leading whitespace |
| `.trimEnd()` / `.trimRight()` | Removes trailing whitespace |
| `.toUpperCase()` / `.upper()` | Uppercase copy |
| `.toLowerCase()` / `.lower()` | Lowercase copy |
| `.startsWith(prefix)` | `true` if the string starts with `prefix` |
| `.endsWith(suffix)` | `true` if the string ends with `suffix` |
| `.charAt(i)` | Character at position `i`, or `""` if out of range |
| `.indexOf(sub)` | Index of first occurrence of `sub`, or `-1` |
| `.replace(from, to)` | Replaces **all** occurrences (previously only the first) |

---

### Phase 2 — Stdlib and compound types

#### `const`
- `const PI = 3.14159` — immutable; any reassignment is a runtime error
- Same scoping as `let` — invisible outside its block

#### `enum`
- `enum Color { Red, Green, Blue }` — variants accessed as `Color.Red`
- Variants are their own type (not `string`) — do not annotate enum parameters as `string`
- Comparable with `==` and usable in `switch case`
- Displayed as `"Color.Red"` (fully qualified name)

#### Labeled loops
- `outer: for (...)` + `break outer` / `continue outer`
- Works with `while`, `for`, `for-in`, `do-while`

#### Spread and rest
- Spread in array literals: `[...arr, 1, 2]`
- Spread in calls: `fn(...args)`
- Rest params: `fn void log(...args)` — `args` is an array with all extra arguments
- The type checker skips arity checks for functions with rest params

#### Namespace `Math`

| Function/Constant | Description |
|---|---|
| `Math.PI`, `Math.E` | Mathematical constants |
| `Math.abs(x)` | Absolute value |
| `Math.floor(x)`, `Math.ceil(x)`, `Math.round(x)`, `Math.trunc(x)` | Rounding (return `int`) |
| `Math.sqrt(x)` | Square root |
| `Math.pow(base, exp)` | Power |
| `Math.exp(x)`, `Math.log(x)`, `Math.log2(x)`, `Math.log10(x)` | Exponential and logarithms |
| `Math.sin(x)`, `Math.cos(x)`, `Math.tan(x)` | Trigonometric (radians) |
| `Math.asin(x)`, `Math.acos(x)`, `Math.atan(x)`, `Math.atan2(y, x)` | Inverse trigonometric |
| `Math.min(a, b, ...)`, `Math.max(a, b, ...)` | Variadic min/max |
| `Math.clamp(x, min, max)` | Clamp to range `[min, max]` |
| `Math.sign(x)` | Returns `1`, `0`, or `-1` |
| `Math.random()` | Pseudo-random decimal in `[0, 1)` (LCG) |

#### Namespace `File`

| Function | Description |
|---|---|
| `File.exists(path)` | `true` if the file exists |
| `File.read(path)` | File contents as `string` |
| `File.write(path, content)` | Writes/overwrites the file |
| `File.create(path)` | Creates empty file if not exists (touch, idempotent) |
| `File.read_asBinary(path)` | File bytes as `[int]` (0–255 each) |
| `File.write_asBinary(path, bytes)` | Writes byte array to file |

#### Namespace `JSON`

| Function | Description |
|---|---|
| `JSON.stringify(value)` | Serializes any value to a JSON string |
| `JSON.parse(string)` | Parses a JSON string; runtime error if invalid |

#### `Set` type

| Method/property | Description |
|---|---|
| `new Set()`, `new Set([...])` | Creates empty set or initialized from array (no duplicates) |
| `.size` | Element count (property, without parentheses) |
| `.add(val)` | Inserts `val` if not present (mutates in-place) |
| `.has(val)` / `.contains(val)` | `true` if the set contains `val` |
| `.delete(val)` / `.remove(val)` | Removes `val`, returns `true` if it existed |
| `.clear()` | Removes all elements |
| `.toArray()` | Returns all elements as an array |
| `.union(other)` | New set with all elements from both |
| `.intersection(other)` | New set with only elements present in both |

---

### Phase 1 — Language core

#### Variables and types
- `let x = value` — declaration; `x = value` — reassignment (without `let`)
- Primitive types: `int` (i64), `decimal` (f64), `bool`, `string`, `void`, `any`, `null`
- Compound types: array `[T]`, dict `<K,V>`, function, interface, class instance
- Nullable types: `int?`, `string?` — accept the base type or `null`
- Typed arrays: `let nums [int] = [1, 2, 3]` — type enforced on push, unshift, index-assign
- Type inference: `let x = add(1, 2)` infers `x: int` in the static checker

#### Operators
- Arithmetic: `+`, `-`, `*`, `/` (integer, truncates), `%`
- Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Logical: `&&`, `||`, `!` (short-circuit)
- Ternary: `cond ? then : else` (lazy, right-associative)
- Null coalescing: `a ?? b`
- `is`: `expr is TypeName` — `true`/`false` at runtime
- Compound assignment: `+=`, `-=`, `*=`, `/=`, `%=`
- Increment/decrement: `++`, `--` (prefix and postfix, as statements only)
- String repetition: `"ha" * 3` → `"hahaha"`
- Concatenation: `"x" + 42` → `"x42"`

#### Runtime safety
- Integer overflow: `checked_*` — error instead of silent wrap
- Division/modulus by zero: runtime error
- Out-of-range index: runtime error
- Undeclared variable: runtime error
- `return` outside a function: runtime error
- Stack overflow: runtime error (not catchable via try/catch)

#### Functions
- Declared: `fn returnType name(type param) { ... }`
- Arrow: `let f = returnType (type param) => { ... }`
- Anonymous: `let f = fn void () { ... }`
- First-class: assignable to variables, passable as arguments
- Recursive: supported with call stack in errors
- Lexical closures: capture variables from the scope where they are defined
- `fn` declarations: reference semantics (rebind of global slot)
- Lambdas (`x => expr`): snapshot semantics (capture by value)

#### Control flow
- `if` / `else if` / `else` — condition in parentheses, braces required
- `while` — condition in parentheses
- `for` — `for (let i = 0; i < n; i++)` — update accepts `i++`, `i--`, `i+=n`, etc.
- `for-in` — `for (let x in arr)` iterates array or string; `x` is a copy of the element
- `break` / `continue` — in all loops
- `switch` — no fall-through; `case a, b:` for multiple values; `default:`
- `try` / `catch(e)` / `finally` — `finally` always runs; `throw` accepts any value
- Standalone blocks `{ ... }` — create new Flash Scope

#### Arrays
- Literals: `[1, 2, 3]`, `[]`
- Index access: `arr[i]` (0-based)
- Index mutation: `arr[i] = val`
- Global mutation from function: `data[i] = val` persists; `this.arr[i] = val` persists
- **Limitation**: `for-in` creates a copy — mutating the loop variable does not affect the original array
- Mutation methods: `.push`, `.pop`, `.shift`, `.unshift`, `.reverse`, `.sort`, `.sort("desc")`, `.sort((a,b) => ...)`
- Query methods: `.length`, `.join`, `.map`, `.filter`, `.reduce`

#### Strings
- Interpolation: `"Hello {name}!"` — supports complex expressions inside `{}`
- `\{` for literal brace; `\"` inside `{...}` breaks the parser (use a variable)
- Escape sequences: `\n`, `\t`, `\r`, `\\`, `\"`, `\{`
- Methods: `.length`, `.substring`, `.split`, `.replace`, `.includes`, `.trim`, `.toString()`

#### Dictionaries
- `let d <string,int> = ({"a",1},{"b",2})`
- Access: `d["key"]` — returns `null` if the key does not exist (no error)
- Write: `d["key"] = val` or `d.Add({"key",val})`
- Methods: `.Add`, `.Remove`, `.RemoveAll`, `.clear`, `.toList`, `.toArray`

#### Classes and interfaces
- `interface Point { x: decimal, y: decimal }` — typed field record, no methods
- `class Foo { public Foo(args) { ... } }` — constructor + fields + methods
- Single inheritance: `class Bar : Foo { ... }`, `super(args)` in constructor
- `public` / `private` — `private` only accessible from methods of the same class
- Instance: `let obj = new Foo(args)`
- Field mutation: `obj.field = val`
- **Limitation**: `this.field[i].method()` inside a class method creates a copy — the result does not persist; use `this.field[i] = newValue` instead

#### Conversions and I/O
- `parseInt(val)` — converts to `int` (string, decimal, int)
- `parseDecimal(val)` — converts to `decimal` (string, int, decimal)
- `readLine(prompt?)` — reads a line from stdin
- `out expr` — prints to stdout with newline; statement, not function

#### Memory — Flash Scopes
- Two arenas: global (entire program) and scoped (local per block)
- Each `{ }` records a watermark on entry and truncates on exit — O(k) per scope
- Return values extracted as `OwnedValue` before the pop and replanted in the parent scope
- `Rc<BlockStatement>` for function bodies — cloning a function is O(1)
- `StoredMethod` in classes — O(1) dispatch without cloning the method body

#### Tooling
- `sz script.sz` — execute file
- `sz` — REPL
- `sz --check script.sz` — static profiler (byte estimation per function)
- `sz --watch script.sz` — automatic rerun on save
- `sz --version` — version
- Span errors: line + column + caret `^` in source
- VS Code extension: syntax highlighting for `.sz`
