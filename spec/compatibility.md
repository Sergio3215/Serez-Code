# Compatibility and versioning

This document states what a Serez Code release promises, what it does not, and
how a change that breaks something is allowed to ship. It describes the process
actually in force, including where past practice departed from strict SemVer.

Other spec documents refer to "the deprecation policy". This is it.

## Two version numbers

**Runtime version** — the number `sz --version` prints and `Cargo.toml`
declares. It is currently `11.0.0`. A release is a git tag `vX.Y.Z`; the release
workflow refuses to publish when the tag and the Cargo version disagree, so the
printed version is always the version that was built.

**Language specification version** — the contract in `spec/`. It is not a
separate number today. Every document in `spec/` describes the behavior of one
runtime version and says so where it matters. Introducing a second number is
only worth doing once the runtime can ship a release that changes no documented
behavior; until then a second number would be noise that has to be kept in sync
by hand.

What this means in practice: **the runtime version is the compatibility
number.** A package that needs behavior frozen in a spec document requires the
runtime version that froze it.

## Classes of change

| Class | Definition | Version effect |
| --- | --- | --- |
| **Bugfix** | The implementation did not do what the spec, the tests or the documentation said. Fixing it makes them agree. | Patch. |
| **Backward-compatible change** | New capability, or a previously invalid program becoming valid. Every program that worked still works and still means the same thing. | Minor. |
| **Breaking change** | A program that worked stops working, or keeps working and means something different. | See below. |
| **Deprecation** | Something still works, is documented as going away, and warns where a warning is possible. | Minor. |
| **Experimental** | Marked as such in `spec/`. May change or be removed in any release. | No guarantee. |
| **Unstable** | Behavior that exists but is not specified. Not a promise; relying on it is at the caller's risk. | No guarantee. |

The second-worst outcome is a program that breaks. The worst is a program that
keeps running and quietly means something else, so a silent semantic change is
treated as breaking even when nothing fails.

### What has actually happened

Strict SemVer would put every breaking change in a major release. That is not
what this project has done, and pretending otherwise would make this document
useless:

- `7.0.0` reserved the system namespace names, so `class Task` stopped parsing.
  That shipped as a major release with an explicit BREAKING heading.
- `9.4.0` changed `obj.methodWithNoArgs` from calling the method to producing a
  reference to it. That is a breaking change, and it shipped in a **minor**
  release — but only after a sweep of every official package found zero
  occurrences of the affected form, which the changelog entry records by name.

- **Unreleased** gave the three unbounded reads a 64 MiB ceiling — `fetch`'s
  response body, an HTTP `import`'s module text, and an `OS.spawn` child's
  stderr. Over it is a fatal `ResourceError` (`SZ6002`), like every other entry
  in `limits.md`. Breaking for any program that reads more than that, which no
  corpus fixture and no official package does; the exposure is to deployed
  programs this repository cannot see, and `limits.md` records that rather than
  claiming a sweep proved something it could not. See `limits.md`.
- **Unreleased** made a name that does not resolve lexically a fatal `SZ8000`.
  Dynamic resolution through the caller's scope is gone: a function reading a
  name it does not declare is rejected before the program runs, where it used to
  pick up whatever the caller happened to hold. Also breaking for the three
  shapes that used to be *catchable* `ReferenceError`s — reading an undeclared
  name, assigning to one, and `new` on an undeclared class — which are now
  refused before evaluation and cannot be caught. The sweep: 29 unaccounted uses
  across the 491-file conclusive corpus, of which 17 are fixtures that already
  exited 1, five are `_`-prefixed scratch neither runner globs, and seven were
  tests asserting the catchability this removes. Ecosystem 8/8 — a file with any
  `import` is not analysed, and every cross-file reference in an official package
  is reached through one. See `scopes.md`.
- **Unreleased** closed `fetch` under lockdown. `sz --eval` and any embedder
  using `RunOpts::sandboxed()` now refuse an outbound request with fatal
  `PermissionError` (`SZ6001`) unless the host is on an explicit allowlist, and
  every redirect hop is checked against the same list. **Breaking for `--eval`
  and the playground; not for `sz file.sz`**, which is unaffected. The
  conformance test that pinned the old behaviour — `eval/lockdown: fetch is NOT
  gated` — inverted by design, in both runners. No official package runs under
  lockdown, so the ecosystem sweep is vacuous rather than reassuring, and it is
  recorded that way. See `security.md`.
- **Unreleased** made `private` private to the declaring class. A subclass
  method reaching an inherited private member now raises catchable `TypeError`
  (`SZ4002`) where it used to succeed. The sweep found **27** private
  declarations across the corpus and **0** in the ecosystem — the one match in an
  official package is a string literal in serez-ui's translator — and no file
  reaches a parent's private from a subclass. See `classes.md`.
- **Unreleased** made an off-type write to a declared field a catchable
  `TypeError` where it used to be accepted. The sweep found **2** affected sites
  across 1,070 corpus and ecosystem files, and both were the conformance
  fixtures that documented the old behaviour; no package declares a typed field
  at all. See `types.md`.
- **Unreleased** made two previously-accepted programs fatal: a name declared
  twice in one scope, and a class declaring a parent that cannot be resolved.
  Both were silent before — the duplicate ran with the later definition, and the
  unresolvable parent ran to completion as long as nothing constructed it. The
  sweep the `9.4.0` rule requires was run first and is recorded in the changelog:
  **0** duplicate declarations and **0** unresolvable top-level parents across
  1,070 corpus and ecosystem files, the single exception being
  `tests/err_parent_missing.sz`, the fixture that documents the defect and
  already exited `1`. See `classes.md`.

The rule in force is therefore narrower than "breaking changes need a major",
and it is the rule the `9.4.0` entry demonstrates:

> A breaking change may ship in a minor release only when a sweep of the
> official packages and the conformance suite finds no affected code, and the
> changelog entry names the sweep and its result. Otherwise it needs a major
> release and a migration note.

A breaking change may never ship in a patch release.

## What a release promises

Within a major version, unless a changelog entry says otherwise:

- Programs that ran keep running and keep meaning the same thing.
- Documented value semantics do not change: assignment and argument passing copy
  arrays, dicts and sets; closures capture as they do today; receiver writeback
  behaves as it does today.
- Diagnostic **codes** (`SZ1xxx`–`SZ7xxx`) and **kinds** are stable. A failure
  keeps its code and kind, or the change is treated as breaking. This is now
  enforced rather than promised: `tests/diagnostic_codes.rs` drives the built
  binary and pins the code every documented construct produces, and checks the
  registry in `errors.md` against the suite in both directions, so a code cannot
  be changed, dropped or added without a test saying so. The 63 `err_*` and 85
  `sec_*` conformance programs assert only that the exit was non-zero and a `❌`
  appeared, so before this a failure could move from `SZ4003` to `SZ4009` and all
  148 would still pass.
- A recoverable failure stays recoverable. Making a catchable error fatal is
  breaking; making a fatal error catchable is not, because no program could have
  caught it before.
- Exit codes keep their meaning: `0` success, non-zero failure.

Explicitly **not** promised:

- Diagnostic **message wording**. Tooling must match on `code`, never on prose.
- Anything marked experimental in `spec/`, which today includes the AOT
  compiler pipeline and the native renderer variants.
- Behavior that no spec document describes. If it is not written down, it is
  unstable — see `spec/` for what is currently frozen.
- Performance characteristics, unless a limit in `spec/limits.md` states one.

## Deprecation

A capability is removed in three steps, never fewer:

1. **Announce.** A changelog entry marks it deprecated, names the replacement,
   and the spec document that describes it says so. It keeps working unchanged.
2. **Warn.** Where a diagnostic is possible without breaking the program, using
   it emits one. This step may coincide with the announcement.
3. **Remove.** In a major release at the earliest, and only after an ecosystem
   sweep shows no official package still uses it.

Steps 1 and 2 ship in minor releases. Step 3 does not.

## Minimum runtime for a package

A package declares the oldest runtime it supports with the reserved
`serez-code` key in `dependencies`:

```json
{
  "name": "serez-ui",
  "version": "4.36.0",
  "dependencies": { "serez-code": ">= 9.17.0" }
}
```

This key is **not** a package to install — there is no package by that name, it
is the interpreter reading the manifest. `sz install` checks it against the
running runtime and fails with an actionable message when it is not satisfied.

**`sz install` is the only thing that checks it.** Running a program does not:
a project declaring `"serez-code": ">= 99.0.0"` runs on 11.0.0 without a word.
`sz update` skips the key, and installing a *dependency* does not verify that
dependency's own floor — only the manifest in the current directory is read. So
the declaration protects the author who types `sz install`, not the user who
runs the program. Making it a run-time gate would refuse programs that work
today, so it is written down here rather than changed silently; the CLI suite
pins both halves. See `ecosystem.md`.

Accepted forms are `">= X.Y.Z"`, `"> X.Y.Z"`, `"= X.Y.Z"` and a bare `"X.Y.Z"`,
which means the same as `>=`. Whitespace around the operator is optional, and a
`-suffix` or `+suffix` is ignored for comparison. Caret, tilde and union ranges
are **rejected**, not reinterpreted: package versions elsewhere in the manifest
are identifiers rather than ranges (see `packages.md`), and accepting `^9.17.0`
here would imply a resolver that does not exist.

A package should declare the runtime version that froze the behavior it relies
on, which is the version whose spec documents describe it.

## Diagnostic output is a compatibility surface

Stderr is not covered by SemVer the way program behaviour is, but tools read it,
so a change to *how* a diagnostic is reported is recorded here rather than made
silently.

**10.0.x — the reserved-name rule moved phase.** Declaring a `class`, `interface`
or `enum` named after a reserved namespace was reported as
`❌ PARSER ERROR [SZ2000]`. It is now `❌ SEMANTIC ERROR [SZ8000]`, from the
semantic phase (`errors.md`).

Classified **behavioural**, not breaking: the set of accepted programs is
unchanged, the exit code is unchanged at `1`, and the message text is unchanged.
What moved is the phase word, the code and — because the declaration is now
parsed before being rejected — the caret, which points at the declaration rather
than at the name.

**Unreleased — the private-access message names the declaring class.**
`Method 'm' is private and cannot be called externally` becomes `Method 'm' is
private to 'Base' and cannot be called from here`, and the bound-reference
variant changes the same way. The old wording said "externally", which is no
longer what the rule refuses: an access from inside the hierarchy but outside the
declaring class is refused too, and it is not external. The code (`SZ4002`), the
kind (`TypeError`), the catchability and the exit code are unchanged. Three
conformance fixtures pin the text and were regenerated.

It also **removes** diagnostics: a rejected class with a body used to produce two
spurious `Unexpected token '}'` errors, and now produces none.

Anything matching on `PARSER ERROR` or on `SZ2000` for this case is affected. No
consumer that does so is known: the corpus has one affected file, the eight
official packages have none, and `vscode-serez` matches no diagnostic code at all.
That is a measurement of *this* repository and its ecosystem, not of every user.

## Known gaps

These are stated so nobody mistakes silence for a guarantee.

- **No filesystem confinement.** `File` reaches anywhere the invoking user can
  read or write: `..` walks out of the working directory and an absolute path
  goes wherever it points. A relative `File` path is measured from the process
  working directory, while a relative `import` is measured from the file. See
  `security.md`; both are pinned by `tests/filesystem_reach.rs`.
- **No lockfile.** Installing twice can resolve differently when the registry
  changes, because `sz install name` picks the greatest available version.
- **No integrity or signature check** on downloaded packages. See `packages.md`.
- **Installation is not atomic.** An interrupted install can leave a partially
  written package directory.
- **The type checker is advisory**, not sound. `sz --check` can accept a program
  that fails at runtime and can describe a program differently from how it runs.
  Its diagnostics are not a compatibility surface.
- **Only one official package declares a runtime floor.** The mechanism now
  works and is tested; the other packages have not adopted it yet, so their
  manifests state no minimum.
- **The spec is incomplete.** Behavior with no document is unstable by the rule
  above. This entry used to name syntax, the type system, operators, scopes and
  modules; `syntax.md`, `types.md`, `operators.md`, `scopes.md` and `modules.md`
  have since been written, so all five are frozen and the entry was understating
  what a release promises. `regex.md` and the control-flow expansion followed. What remains undocumented is genuinely unstable, and
  the honest way to find it is to look for behaviour no file in `spec/`
  describes rather than to trust a list here.
