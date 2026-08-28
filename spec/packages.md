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

With dependency recording enabled, the resolved exact version is written to the
project manifest after installation. Failure to update the manifest is reported
as a warning because package files are already present.

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

- no lockfile or reproducible dependency graph;
- no archive integrity hash, signature or publisher trust policy;
- no transitive dependency solver or cycle contract;
- no declared minimum runtime or language-spec version enforced for packages;
- no atomic install/update: an existing package is removed before replacement,
  so a failed copy/download can leave it absent or partially installed;
- no transaction spanning package files and `serez.json`;
- no normative yank/cache/offline policy.

These gaps are compatibility and supply-chain work, not reasons to silently
change current resolution behavior.
