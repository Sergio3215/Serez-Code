# Execution contract

What Serez Code does and does not promise about running a program. The short
version, because getting this wrong is expensive:

> **Serez Code has no sandbox.** Nothing in the language makes hostile code safe
> to run. The mechanisms below reduce *accidental* access by a program you
> trust. Untrusted code needs an operating-system boundary — a container, a VM,
> a separate user account with its own filesystem, process and network limits.

Four different mechanisms are often confused with one another. They are not
interchangeable and only the last one is a security boundary.

| Mechanism | What it is | What it is not |
| --- | --- | --- |
| Permission manifest | A declaration in `serez.json` (or inline) of which native namespaces a program intends to use. | Not a boundary: outside lockdown a program can grant itself any permission at runtime. |
| `unsafe { }` | A syntactic gate on individually destructive operations. | Not a permission and not isolation: any program can write `unsafe`. |
| Lockdown | A restricted profile for source you did not write, closing self-granting, the capabilities that reach disk with nothing declared, and the network. | Not a sandbox: the process still runs with the invoking user's full rights, and an allowed host is still reached from the machine's network position. |
| OS isolation | A container, VM or restricted account around the `sz` process. | The only actual security boundary. Provided by the operating system, not by Serez. |

## Permission manifest

Permissions are **additive declarations**, checked at the point a native
operation runs. These namespaces are enforced:

`Env`, `Gui`, `Media`, `OS`, `Socket`, `System`, `Task`, `Terminal`, `Time`

A program without the matching namespace permission gets a structured fatal
`PermissionError` (`SZ6001`) instead of the operation. It crosses `try/catch`
unchanged: the public payload is available to CLI/tooling, but user code cannot
turn the denial into ordinary control flow. `sec_notcatch_permission.sz` and the
Rust program-outcome regressions pin both properties across every guarded
namespace.

Two properties make this a manifest and not a boundary:

- **It is self-grantable.** Outside lockdown, `use permissions { … }` inserts
  into the running program's permission set. A program that wants a permission
  can simply take it. This is intentional: someone running their own file is
  supposed to be able to declare permissions inline.
- **It is not exhaustive.** `File`, `import` and Autodiff's weight files reach
  the disk with **no permission declared at all**. The permission set covers the
  namespaces listed above, not the whole native surface.

Read the manifest as documentation of intent — useful for review, for tooling,
and for catching a dependency doing something unexpected. Do not read it as a
list of what a program is *able* to do.

### Names that are accepted and do nothing

The nine above are the whole enforced vocabulary. Three other things a manifest
may contain are accepted and gate nothing, which matters because all three look
like they work:

- **`File`.** It is the second-most-declared capability across the official
  packages — twenty-three `use permissions` blocks and four manifests — and it
  is inert. `File.read` succeeds with no permissions declared at all, and
  declaring `File` changes nothing. It is accepted because removing it would
  break those declarations for no security gain, and it deliberately produces
  **no warning**: the author followed the documented convention, and it is the
  runtime that does not gate file access.
- **A dotted name** such as `OS.exec` or `File.delete`. The grammar accepts it —
  the parser's own comment advertises the form — and nothing ever checks it. It
  does **not** imply its prefix, so `use permissions { OS.exec }` leaves `OS`
  denied and the program fails at its first `OS` call. No official package
  writes one.
- **Any other name.** A misspelling is inserted and never looked at. Before this
  was diagnosed, `use permissions { Termnal }` granted nothing and the program
  then failed at its first `Terminal` call telling the author to declare a
  permission they believed they had declared, one character away in the same
  file.

The last two now produce a warning at the point of the grant, naming the likely
intended permission where one is within edit distance 2 and unambiguous. A
warning, not a refusal: rejecting an unrecognised name would break any program
that declares one today. `src/permissions.rs` holds the vocabulary, and
`enforced_permissions_match_the_evaluator` keeps it equal to what
`require_permission` actually checks.

### How far a path reaches, and what it is relative to

Neither is restricted, and both are worth stating because a reader could
reasonably assume otherwise.

- **`..` and absolute paths are not blocked.** `File.read("../x")` walks out of
  the working directory, and an absolute path goes wherever it points. There is
  no confinement of any kind — consistent with `File` being inert, but not
  obvious from it. Two conformance programs used to imply the opposite:
  `sec_path_traversal.sz` said traversal "must be rejected" and named escaping
  "the sandbox", and `sec_path_traversal_abs.sz` said the same for absolute
  paths. Both passed on all three CI platforms only because the paths they named
  do not exist there. They now say what they actually prove — that a read of a
  path that is not there fails with a structured `SZ4005` — and the real
  behaviour is pinned in `tests/filesystem_reach.rs`.
- **A relative `File` path is measured from the process working directory, not
  from the script.** `import "./lib"` is measured from the *file*, so an import
  resolves the same wherever `sz` was invoked. `File.read("./data.txt")` does
  not: the same program reads its own data file when run from its folder and
  fails with `SZ4005` when run from one level up. Two features, both spelled
  `./`, measured from different places. Changing either is a breaking change.

Making `File` — or per-operation dotted permissions — genuinely enforced is a
capability decision, not a diagnostics one. It would break existing programs
and needs the process in `compatibility.md`.

## `unsafe { }`

An operation-level gate on things that are destructive rather than merely
capable. It requires the call to appear lexically inside an `unsafe { }` block:

- `File.delete`, `File.rename`
- Raw memory: allocation, pointer reads and writes, `*ptr = value`
- OS operations that modify OS state

`unsafe` is a speed bump against accident, not a privilege check. Any program
can open an `unsafe` block. Its value is that a reviewer can grep for it, and
that no destructive operation happens without the author having typed the word.
Calling a gated operation outside such a block yields structured fatal
`UnsafeError` (`SZ6003`). It crosses `try/catch` unchanged; the CLI and tooling
receive the diagnostic payload, while user code cannot swallow the gate.

## Protected process-target heuristic

`OS.exec` and `OS.spawn` refuse command strings containing a short list of
protected system-path fragments, returning fatal `SecurityError` (`SZ6004`).
This is a compatibility defense against accidental direct execution from paths
such as `C:\Windows\System32` or `/etc/`; it is **not** a sandbox or a complete
process policy. The check is a case-sensitive substring test, not canonical path
resolution, and process indirection, alternate spellings, symlinks or platform
aliases can evade it. Host isolation and an OS allowlist are required when
process execution must be contained.

## Lockdown

The profile for **source you did not write**. `sz --eval` runs under it
automatically, because there is no file and therefore no `serez.json` to read
intent from.

Under lockdown:

- `File`, `import` and Autodiff weight I/O are refused — the three capabilities
  that otherwise reach the disk with nothing declared. These three are
  **catchable** `PermissionError` (`SZ6001`): catching records the denial, and
  the program continues without the capability.
- `use permissions { … }` is refused instead of granting, and this one is
  **fatal**: `SecurityError` (`SZ6004`), which `try/catch` cannot consume. The
  other three refuse an *action*; this one refuses to hand out *capability*, so
  it is not something a program may catch and route around.
- Lockdown starts with an empty permission set and does not grant anything, so
  a guarded namespace is still refused the ordinary fatal way (`SZ6001`).
- A Task worker inherits lockdown and the parent's granted permission set; it
  cannot load extra permissions from its manifest or inline source. Workers
  remain native threads in the same process, not an isolation boundary.

The split between catchable and fatal is deliberate and pinned by
`lockdown_denials_split_into_catchable_and_fatal` in `tests/runtime_outcome.rs`.
Unifying it in either direction would be a semantic change.

### `fetch` under lockdown

**Closed by default.** A `fetch` under lockdown is refused with fatal
`PermissionError` (`SZ6001`) before any request leaves the process, and
`try/catch` cannot consume it — a security refusal that a program can turn back
into control flow is advice.

It is opened only by an **allowlist of hostnames**, which the *embedder* sets and
the program cannot:

| Surface | How |
| --- | --- |
| CLI | `sz --eval "…" --allow-fetch a.example,b.example` (repeatable) |
| Library | `run::RunOpts::fetch_allowlist` |
| Embedder API | `Evaluator::allow_fetch_hosts` |

Matching is on the **hostname**, case-insensitively and exactly. There are no
wildcards, no suffix matching and no port matching: `sub.allowed.test` is not
`allowed.test`, and neither is `allowed.test.evil.test`. Userinfo does not
disguise a host — `http://allowed.test@evil.test/` is `evil.test`. A URL whose
host cannot be read is refused rather than guessed at.

**Redirects are checked at every hop.** A response that redirects from an allowed
host to one that is not on the list is refused, and the refusal names the host it
stopped at:

```text
allowed.example  ->  302  ->  forbidden.internal      refused
allowed.example  ->  302  ->  allowed.example/next    followed
```

A `Location` this runtime will not resolve — protocol-relative, path-relative, or
a different scheme — is refused rather than followed. The redirect ceiling is 5,
the same as the default outside lockdown.

**Outside lockdown nothing changed.** `sz file.sz` reaches any host and follows
redirects exactly as before; the allowlist is not consulted at all. Until 10.0.0
lockdown behaved that way too, and `spec/compatibility.md` records the change.

This closes the SSRF shape lockdown used to have — a snippet reaching loopback,
link-local and metadata addresses from the machine's own network position. It
does not make an *allowed* request safe: the host you allow is still reached from
wherever `sz` is running.

Lockdown narrows the blast radius of a careless snippet. It does not contain a
hostile one.

## Trusted vs untrusted execution

**Trusted execution** — `sz file.sz` on code you or your team wrote. The
manifest documents intent, `unsafe` marks the destructive parts, and the process
runs with your full rights. This is the normal case and it is fine.

**Untrusted execution** — code from anywhere else. The language cannot make this
safe. The requirement is an OS boundary with its own filesystem, process and
network restrictions. Lockdown is defense in depth *inside* that boundary, never
a replacement for it.

Concretely, running someone else's Serez program without OS isolation gives it:
network access, whatever the host lets the invoking user do through any
capability the program grants itself, and unbounded memory and CPU (see
`limits.md` — there is no execution timeout and no memory ceiling).

## Supply chain

Installing a package runs no code at install time, but an installed package is
ordinary source that runs with the importing program's rights, including
whatever permissions that program grants itself. There is currently no lockfile,
integrity hash or signature policy. Treat a dependency as code you are choosing
to trust. `packages.md` defines the manifest, path and archive limits that reduce
malformed-package risk; those checks do not establish publisher trust or isolate
the code after installation.
