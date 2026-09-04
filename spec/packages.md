# Package and manifest contract

This document records the package behavior implemented by Serez Code 9.17.0.
It is intentionally narrower than a complete package-system design. Statements
using **must** describe behavior on which projects may rely; the final section
lists guarantees the current implementation does not provide.

## `serez.json`

`serez.json` must be UTF-8, valid JSON, no larger than 1 MiB, with a JSON object
at the top level. Parsing consumes the complete document: a valid prefix followed
by malformed input is an error.

The public fields have these types:

| Field | Required | Contract |
| --- | --- | --- |
| `name` | yes | Non-empty string. Package operations apply the identifier rules below. |
| `version` | yes | Non-empty string. Publishing and installation apply the version rules below. |
| `description` | no | String; defaults to `""`. |
| `author` | no | String; defaults to `""`. |
| `dependencies` | no | Object whose values are strings; defaults to `{}`. The key `serez-code` is reserved: it declares the minimum runtime and is checked, never fetched. See `compatibility.md`. |
| `permissions` | no | Array of strings; defaults to `[]`. See `security.md`. |
| `scripts` | no | Object mapping command names to shell-command strings; defaults to `{}`. |
| `bin` | no | Object mapping command names to portable package-relative `.sz` paths; defaults to `{}`. |

Unknown fields are accepted for forward compatibility. Wrong types in known
fields are errors; they are not silently replaced with defaults. `sz init`
serializes user input as JSON, including quotes, backslashes and line breaks,
instead of interpolating it into the document.

## Identifiers and versions

Package names accepted by install, update, uninstall, info and registry operations:

- are 1–128 bytes;
- contain only ASCII letters, ASCII digits, `.`, `-` and `_`;
- are neither `.` nor `..`.

Resolved and explicit versions use the same rule and may additionally contain
`+`. These checks happen before a name or version is joined to a filesystem path
or inserted into a registry URL. Version strings are currently identifiers, not
SemVer ranges; dependency resolution does not implement caret, tilde or inequality
constraints.

The one exception is the reserved `serez-code` key, which is not resolved as a
package at all. It states the minimum runtime, accepts `">= X.Y.Z"`, `"> X.Y.Z"`,
`"= X.Y.Z"` and a bare `"X.Y.Z"`, and is compared against the running runtime by
`sz install`. Caret and tilde ranges are rejected there too. `sz install
serez-code` is refused with a message explaining why. See `compatibility.md`.

## Resolution and installation

For `sz install name@version`, the explicit version is used. For `sz install
name`:

1. if the local registry contains the package, the lexicographically greatest
   local version directory is selected;
2. otherwise the HTTP registry's `latest` endpoint selects the version.

After resolving a version, Serez copies the exact local-registry directory when
present and otherwise downloads the matching ZIP from the HTTP registry. Local
installs go to `<project>/packages/<name>`; global installs use
`$SEREZ_PACKAGES/<name>` or the platform user's `~/.serez/packages/<name>`.

### One install is one transaction

The logical unit of installation is three things, not one:

    the package tree  +  its serez.json line  +  its serez.lock line

They represent a single confirmed state. An install either establishes all three
or leaves the previous state, and an interruption between them is repaired
**deterministically on the next run**.

Concretely, an install never leaves:

- a new package tree beside old metadata;
- a manifest updated against a lockfile that was not;
- a `serez.lock` line for a package that was not installed.

#### What is guaranteed, and what is not

**Not atomic**, and not described as such: three paths change and no filesystem
commits three paths at once. Between the first rename and the last there is a
window in which the tree is new and the metadata is not.

What removes that window is **recovery, not atomicity**. Before the first
mutation, an install writes `.serez-install.journal` beside `serez.json`
containing every byte of the target state — the destination, the staging
directory, and the full new text of both metadata files — and flushes it to
disk. The next `sz install` finds the journal and applies whatever remains, from
the recorded bytes, however many times it is run.

A journal cut short by a crash *while it was being written* has no matching
terminator and is discarded. That is safe because it is written before anything
is mutated: a torn record means nothing happened.

#### Order

1. the package tree, staged beside the destination and swapped in;
2. `serez.lock`, written beside itself and renamed over;
3. `serez.json`, the same way.

The tree first because it is the step that can still fail on its own; the
lockfile next because it is what the next install verifies against; the manifest
last because it is the only one a person edits by hand.

#### Failures happen before anything moves

Everything that can be decided is decided while the project is untouched: the
version, the staged tree, its digest, the integrity check against the lockfile,
and the manifest's new text. A `serez.json` that does not parse fails there —
with the previously installed package still in place — rather than after the
swap.

Until 10.0.0 the destination was removed *before* the new version was fetched, so
a failed download left the project with no package at all. Until this release the
manifest and lockfile were written *after* the install and a failure was a
warning.

#### `install_all`

`sz install` with no argument installs each dependency as its own transaction.
A failure at the *n*-th leaves the first *n*-1 fully committed — package,
manifest and lockfile agreeing — and the rest untouched. It is not one
transaction across every dependency, and does not claim to be.

### `serez.lock`

`serez.lock` sits beside `serez.json` and records the resolved graph, one line
per package, sorted by name:

```
<name>	<version>	<integrity>
```

`integrity` is `sha256-<hex>` over the installed tree: every file's relative path,
its length and its contents, in path order. Paths use `/` and the file is written
with `
` endings, so the same resolved graph produces the same bytes on every
platform and a diff of the lockfile shows what changed rather than how it was
written.

A missing lockfile is empty rather than an error — the first install in a project
is the one that creates it. `sz uninstall` removes the package's line.

### Integrity is checked before the install is committed

When `serez.lock` already has a line for the package **at the version being
installed**, the staged tree is hashed and compared to it before the rename. A
mismatch aborts the install with the project untouched and reports both digests.

A package with no line yet, or a line at a different version, is being resolved
rather than reproduced: it is recorded rather than refused. A lockfile that
rejected everything it had not already seen could never be created.

The digest covers the installed tree rather than the downloaded archive, because a
local-registry install has no archive — it copies a directory. For a remote
install the two are the same statement, since extraction is a pure function of
the archive, and hashing the tree has the advantage of checking what actually
landed.

`sz update <name>` differs from versionless install: it always asks the HTTP
registry for `latest`, avoiding a stale local-registry version.

## Commands

`sz run <command>` resolves in this order:

1. a project `scripts` entry;
2. a `bin` entry from `<project>/packages`;
3. a `bin` entry from the global package store.

A local package folder shadows a global folder with the same name. Two different
packages exporting the same command cause an ambiguity error; `package:command`
selects one explicitly.

Project scripts are passed to `cmd /C` on Windows or `sh -c` on Unix. They are
arbitrary trusted shell commands. Extra arguments are appended to that command;
the current API does not provide shell escaping as a security boundary.

A `bin` entry must end in `.sz`, must resolve to a regular file inside its own
package directory, and cannot escape through `..`, an absolute path, a Windows
prefix or a symbolic link. Portable paths use `/` and normal relative components,
with a maximum length of 4096 bytes.

## Archive extraction

Remote package archives have these hard limits:

| Dimension | Limit |
| --- | ---: |
| Downloaded ZIP | 64 MiB |
| ZIP entries | 10,000 |
| Total declared/extracted content | 256 MiB |

Every archive path must satisfy the same portable relative-path rule as `bin`.
Extraction refuses path traversal, absolute/platform-prefixed paths, a package
destination that is itself a symbolic link, symbolic-link directory components,
and overwriting a symbolic link. A single common top-level wrapper directory is
stripped to preserve the package layout used by the registry.

## Trust and missing guarantees

Installation does not run lifecycle code, but imported package source and
`scripts` run with the `sz` process's rights. Packages are trusted code; the
permission manifest is not a sandbox. See `security.md` for the full execution
contract.

The following are known gaps, not implied guarantees:

- no **signature or publisher trust policy**. `serez.lock` says a package is the
  same one that was installed before; it says nothing about who published it, and
  it cannot detect a substitution made before the first install. That is a
  separate decision and nothing in the lockfile format anticipates it;
- no transitive dependency solver or cycle contract. The lockfile records the
  graph that was resolved, and today that graph is flat;
- no declared minimum runtime or language-spec version enforced for packages;
- no **atomic** commit across package files, `serez.json` and `serez.lock`. The
  three are one *recoverable* transaction — journalled before the first mutation
  and completed by the next run — which is what a filesystem can actually
  provide. A process killed mid-commit leaves a window until the next
  `sz install`;
- no cross-process lock. Two installs of the same package at once still race,
  and neither the journal nor the staging directory makes that safe;
- no normative yank/cache/offline policy.

These gaps are compatibility and supply-chain work, not reasons to silently
change current resolution behavior.
