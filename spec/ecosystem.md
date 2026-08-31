# Ecosystem layers and stability tiers

This document answers two questions that `compatibility.md` deliberately does
not. That document says what a *change* promises — bugfix, breaking,
deprecation, experimental. This one says what a *thing* is: which layer it
belongs to, what tier of promise it carries, and what a tier requires of it.

Both are needed. "Experimental" as a class of change means nothing until
something is named experimental, and a package cannot be held to a promise that
was never written down.

## The layers

Layers are named by what they can be replaced by, not by directory.

**They describe intent, and the code does not yet honour them.** The evaluator's
language half dispatches directly into the native namespace modules — `expr.rs`
alone names twenty-one of them — so Core reaches into Native capabilities today.
That coupling is the P2 work in `MATURITY_AUDIT.md`, being unpicked one boundary
at a time (`modules.rs` and `permissions.rs` are the two taken so far). This
document names the layers so the direction of travel is legible; it does not
claim the arrows below are already one-way.

```text
Serez Core          the language itself
      ↓
Serez Runtime       what running a program requires
      ↓
Native capabilities primitives that cannot be written in Serez
      ↓
Official libraries  Serez code, maintained here
      ↓
Official frameworks Serez code built on those libraries
      ↓
Developer tooling   things that read or run Serez without being it
      ↓
Community packages  everything else
```

**Serez Core** — `lexer.rs`, `parser.rs`, `ast.rs`, `token.rs`,
`type_checker.rs`, `scope.rs`, `region.rs` and the evaluator's language half
(`expr.rs`, `stmt.rs`, `control.rs`, `ops.rs`, `classes.rs`, `lvalue.rs`,
`builtins.rs`). This is what `spec/` specifies. It has no knowledge of files,
sockets, windows or processes.

**Serez Runtime** — `run.rs`, `modules.rs`, `permissions.rs`, `repl.rs`,
`szx.rs`, `package_manager.rs`. What it takes to *run* a program: resolving an
import, holding the permission set, loading a package, driving the REPL.

**Native capabilities** — the `namespaces_*.rs` files and `methods_tensor.rs`.
Primitives that cannot be written in Serez without losing the capability
entirely: the filesystem, sockets, processes, the clock, entropy, raw memory,
threads, native windows, GPU and tensor kernels. The rule that keeps this layer
small is stated in `README.md` and enforced by review, not by the compiler: **if
it can reasonably be written in Serez, it is a library, not a builtin.**

**Official libraries and frameworks** — separate repositories, each with its own
manifest and its own suite. They are consumers of the language, and are treated
as its compatibility suite: `run_ecosystem.sh` / `run_ecosystem.ps1` run them
against a freshly built core.

**Developer tooling** — the `sz-lsp` binary and `lsp/`, `test_run.rs`, the
benchmark runners. These read or run Serez without being part of it.

## Tiers

A tier is a promise about change, so each one is defined by what
`compatibility.md` allows to happen to it.

### Stable

Breaking changes follow the full deprecation path: announce, warn, remove — and
removal only in a major release, only after an ecosystem sweep shows no official
package still relies on it.

To be Stable, a surface must have **a document in `spec/` describing its
behaviour, and conformance tests the document points at.** That is the whole
requirement, and it is deliberately mechanical: a promise nobody can check is
not a promise. Behaviour with no spec document is *unstable* by definition —
`compatibility.md` says so — regardless of how long it has worked.

### Official

Maintained here, released alongside the core, and covered by the compatibility
suite. An Official package promises that it works against the runtime it
declares, and that a break is a bug rather than an expected cost of upgrading.

To be Official, a package must:

1. declare a **minimum runtime** with the reserved `serez-code` key;
2. carry a **suite of its own** that runs against a freshly built core;
3. be **listed in the shared ecosystem runner**, so a core change that breaks it
   is seen before release rather than after.

Official is not a claim about API stability *within* the package — that is the
package's own version number, under the same rules as the core.

### Experimental / Labs

May change or be removed in any release, with no deprecation path. What
Experimental must **not** mean is silent degradation: an experimental component
that cannot do something has to say so with a diagnostic. The AOT compiler is
the model — it refuses an unsupported construct with `SZ7001`/`SZ7002` rather
than lowering it to `null`.

Experimental status has to be visible where the thing is used, not only in a
changelog.

## Where each surface sits today

**Core and runtime.** Every area with a document in `spec/` is Stable; `ls spec/`
is the inventory, and no count is given here because it goes stale.

| Surface | Tier | Why |
|---|---|---|
| Language, values, types, scopes, modules, errors | Stable | Specified and pinned |
| Native namespaces with a `spec/` document | Stable | Specified and pinned |
| Native namespaces without one | Unstable | Not a tier — see `compatibility.md`; behaviour nobody wrote down |
| AOT compiler pipeline (`compiler/`, `llvm` feature) | Experimental | `spec/compiler.md`; not compiled into a default build, not reachable from the CLI |
| `Media` (`audio` feature) | Experimental | Absent from a build made with `--no-default-features`, where every call is a catchable error |
| `sz-lsp` | Experimental | No spec document; the protocol surface is not frozen |

**Official packages.** Every one of these passes against the current core —
the eight in the shared runner, plus `serez-cobol` (23/23) and `serez-strike`
(113/113) run separately. What the table records is whether each meets the
three Official requirements.

| Package | Declares a runtime floor | Own suite | In the shared runner |
|---|---|---|---|
| `serez-ui` | **yes** (`>= 9.17.0`) | yes | yes |
| `serez-http` | no | yes | yes |
| `serez-ai` | no | yes | yes |
| `serez-agentai` | no | yes | yes |
| `serez-pack` | no | yes | yes |
| `serez-apipack` | no | yes | yes |
| `serez-dotenv` | no | yes | yes |
| `serez-graph` | no | yes | yes |
| `serez-cobol` | no | yes | **no** |
| `serez-strike` | no | yes | **no** |

`serez-ui` is the canary and is treated as such: it exercises classes,
inheritance, constructors, closures, method references, modules, mutation,
receiver semantics, GUI and the `.szx`/`.szs` dialects, so a core change that
breaks the language usually breaks it first.

## What the tiers require and the ecosystem does not yet meet

Stated rather than quietly carried, per the rule that debt is documented:

- **Nine of ten official packages declare no minimum runtime.** The mechanism is
  implemented and specified (`compatibility.md`), and the one package that used
  it is the reason the key was reserved. Until the rest adopt it, "works with
  this runtime" is an assumption every user re-derives.

- **The floor is checked by `sz install`, and by nothing else.** Running a
  program does not check it: a manifest declaring `"serez-code": ">= 99.0.0"`
  runs on 9.17.0 without a word. `sz update` filters the key out, and installing
  a *dependency* does not verify that dependency's own floor. So the declaration
  protects the author who types `sz install`, not the user who runs the program.
  Making it a run-time check is a behaviour change that would refuse programs
  that run today, so it is written down here rather than made silently.

- **`serez-cobol` and `serez-strike` are not in the shared runner.** Their
  suites pass, but they are run by hand, which means a core change is not
  measured against them before it ships.

- **The ecosystem suite is not in CI.** It runs locally. Automating it needs a
  trust policy for executing external code at commit-pinned revisions, which has
  not been decided. Until then, "the ecosystem passes" is a claim about the
  machine that ran it.

- **No tier is declared in a manifest.** A package's tier lives in this document
  and nowhere the tooling can read. A `tier` field would let `sz install` warn
  when an Experimental package is pulled into a project that expects Stable;
  adding one is only worth doing with something that acts on it.

## Not specified

- **Community packages.** The registry does not distinguish them from official
  ones, and this document makes no promise about them. `packages.md` describes
  what installing any package does and does not verify.
- **Support windows.** No policy states how long an older runtime keeps
  receiving fixes. Do not infer one.
- **Deprecation of a whole package.** The three-step path in
  `compatibility.md` is written for a capability inside the runtime. Retiring an
  entire official package has not happened and has no stated process.
