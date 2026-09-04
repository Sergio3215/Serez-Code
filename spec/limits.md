# Limits

Normative ceilings a Serez program runs under. Every one of them exists for the
same reason: without it, ordinary input reaches a native failure the language
cannot describe — a stack the process cannot grow, an allocation the host
refuses, a loop that does not end. A limit turns that into something with a
message, a position and an exit code.

A limit is part of the language contract. Raising one is compatible. Lowering
one is compatibility-impacting and follows the deprecation policy in
`compatibility.md`; an
exception is a crash-prevention correction when the advertised value cannot be
enforced safely. Such a correction must have a regression and a changelog entry.

## Source limits

| Limit | Value | Enforced by |
| --- | --- | --- |
| AST depth described by one source file | 512 | `MAX_PARSE_DEPTH`, reported as `SZ2001` |

Depth is counted over the *tree*, not over the parser's own recursion, because
the tree is what every later stage walks. Two shapes of source reach it:

- **Nesting** — `(((…)))`, `[[[…]]]`, `f(f(f(…)))`, nested blocks. One level of
  source is one level of tree. Constructs do not map one-to-one: `- x` costs two
  levels because the prefix re-enters expression parsing, and `f(x)` costs one
  for the call plus one for the argument.
- **Operator chains** — `a + b + c + …`. These parse in a flat loop, so they
  cost the parser nothing, but they build a left-leaning tree one level deeper
  per operator. Each operator is therefore charged one level.

Exceeding the limit is a parse error: the file does not run, exactly as with any
other syntax error.

The ceiling is far above real code, and that is checked rather than asserted:
parse-checking every `.sz` file across the official packages and this
repository, the **only** two that reach it are
`tests/err_parse_depth_chain.sz` and `tests/err_parse_depth_nesting.sz` — the
two fixtures written to test the ceiling. Source that genuinely needs more
should build the structure at runtime rather than spell it out.

The measurement is stated that way on purpose. An earlier version of this
document named a file count and a deepest-nesting figure, and both went stale
as the ecosystem grew; "nothing real reaches it" stays true or fails loudly,
and re-checking it is a `--check` sweep for `SZ2001`.

## Runtime limits

Eight of these were re-probed against the binary in the current cycle, at the
boundary rather than near it: string repetition (`"a" * 10000000` succeeds,
`10000001` is fatal `SZ6002`), padded string result, tensor element count,
`Memory.alloc`, call depth, value nesting depth, `Crypto.randomBytes` (1,048,576
succeeds, 1,048,577 and 0 are refused with the plain-string throw described
below) and the `sz-lsp` message body. All eight matched. `File.read` was probed
earlier in the cycle. The rest — the GPU buffer, the WebSocket frame, the four
weights-file ceilings and the Task limits — were **not** re-probed: reaching
them needs a GPU, a live socket, a `.szw` file or thirty-three concurrently
spinning workers. They are stated on the strength of their own tests, not of a
measurement made here.


| Limit | Value | Behavior on breach |
| --- | --- | --- |
| Call depth | 512 frames | Fatal `ResourceError` (`SZ6002`). |
| Value nesting depth | 500 levels | Fatal `ResourceError` (`SZ6002`). |
| String repetition (`"a" * n`) | 10,000,000 repetitions | Fatal `ResourceError` (`SZ6002`). |
| Padded string result | 10,000,000 Unicode scalar values | Fatal `ResourceError` (`SZ6002`) before result allocation. |
| Tensor element count | 10,000,000 `f64` elements | Fatal `ResourceError` (`SZ6002`). |
| Regex execution steps, per match | 1,000,000 | Match fails. |
| Regex backtracking depth | 8,000 | Match fails. |
| `Memory.alloc` size | 256 MiB | Larger requests are fatal `SZ6002`; requires `unsafe`. |
| One GPU buffer | 256 MiB = 33,554,432 `f64` elements | Larger creation/upload/matmul results are fatal `SZ6002`. |
| `File.read` / `File.read_asBinary` | 256 MiB | Larger files are rejected before contents are read, fatal `SZ6002`. |
| `fetch` response body | 64 MiB | Fatal `ResourceError` (`SZ6002`). |
| HTTP `import` module text | 64 MiB | Fatal `ResourceError` (`SZ6002`), before the module is cached to disk. |
| `OS.spawn` child stderr | 64 MiB | Fatal `ResourceError` (`SZ6002`), raised when `OS.tick` harvests the job. |
| `OS.exec` child stdout/stderr | 64 MiB | Fatal `ResourceError`. **Waived inside `unsafe { }`**, which is the only context `OS.exec` runs in — see `security.md`. |
| Values one generator call accumulates | 1,000,000 (host-configurable) | Fatal `ResourceError` (`SZ6002`). |
| WebSocket frame payload | 16 MiB | Frame rejected. |
| Concurrent Task workers, per runtime | 32 | New worker creation is fatal `SZ6002`. |
| Task argument, reply or stored worker error | 1 MiB | Larger messages become `SZ6002`; worker error text is bounded before retention. |
| Task worker source | 16 MiB | Worker enters failed state before parsing. |
| Retained Task records, per runtime | 256 | Oldest terminal record is evicted; active workers remain. |
| `Crypto.randomBytes` request | 1 MiB (1,048,576 bytes) | Rejected — see the note below on its shape. |
| `sz-lsp` message body | 64 MiB | The message is refused and the server exits. |
| Autodiff weights file (`.szw`) | 256 MiB | Rejected before the file is read. |
| Tensors in one weights file | 100,000 | Load fails. |
| Tensor rank in a weights file | 64 | Load fails. |
| Total values in a weights file | 10,000,000 | Load fails. |

Value nesting bounds how deeply *data* may nest, which a program reaches by
nesting containers rather than by nesting code — `v = [v]` in a loop. It is
separate from AST depth and from call depth. Exceeding it used to replace the
subtree with null, print one line per truncated site and let the program finish
with exit 0, so the caller received a corrupted value and no failure. It is now
one fatal diagnostic at the next statement boundary.

Call depth bounds recursion in the program being run: 512 nested Serez frames,
including ordinary functions, methods, `super`, native collection callbacks and
operator overloads. It is deliberately **not** catchable — a `try`/`catch`
around runaway recursion would itself run inside the exhausted stack. The old
nominal ceiling of 1000 was lowered after the Windows debug CLI reproducibly
overflowed its native stack near 800 callback frames, before Serez could report
an error. The current regression exercises all five invocation paths.

Tensor/GPU shape products use checked multiplication before allocating. A
dimension product that does not fit the platform size is the same fatal resource
failure as exceeding the element cap. `Memory.alloc(0)` is instead an ordinary,
catchable invalid-argument `TypeError`: it consumes no resource. GPU limits are
per buffer, not an aggregate quota. The previous GPU check compared an element
count with a byte constant and therefore admitted roughly 2 GiB buffers; the
implementation now enforces the documented 256 MiB contract on every creation
path and matrix output.

String padding validates negative targets before conversion to a native size,
uses linear construction and reserves capacity fallibly. This prevents the old
`padStart(-1, "x")` path from converting `-1` to a platform-sized maximum and
growing until the host killed the process. The repetition and padding ceilings
are separate because repetition bounds the multiplier while padding bounds the
result character count.

Task workers use fixed 16 MiB native stacks in addition to their isolated
evaluator arenas. The concurrency ceiling therefore also bounds reserved worker
stack space. Terminal replies remain repeat-pollable within a 256-record window;
the oldest completed/failed record is evicted before a new task is registered.
See `tasks.md` for the lifecycle contract.

`Crypto.randomBytes` bounds a single request at 1 MiB and also rejects a
count below 1. Unlike every other limit in this table it does not report a
structured error: it throws a **plain string**, `"Crypto.randomBytes: n must
be between 1 and 1048576"`, with no `kind` and no `code`. It is catchable —
the same shape as a missing module — so a caller can recover from it but
cannot classify it without matching English. Recorded as a diagnostic gap in
`errors.md`, not as a separate contract.

The `sz-lsp` ceiling bounds the `Content-Length` header of a JSON-RPC
message. It was the one input-sized allocation in the project with no
ceiling: the body was allocated at exactly the advertised length, so a
header reading `Content-Length: 9999999999999` aborted the process with
`memory allocation of 9999999999999 bytes failed` — an allocator message
rather than a diagnostic, and the editor's language server simply
disappeared. An over-limit header now prints why and exits, which is the
same outcome the framing already had for a malformed header. 64 MiB is
generous on purpose: the largest legitimate message carries a whole
document in a `didOpen`, and a source file near that size is far past the
AST ceiling already.

The weights-file ceilings guard `.szw` loading specifically: the file size is
checked from its metadata before any bytes are read, and the tensor count,
rank and total value count are checked while parsing the header, so a
malformed or hostile file cannot make the loader allocate first and validate
afterwards.

The two regex limits bound the matcher rather than the pattern: a pattern with
catastrophic backtracking fails to match instead of running until the process is
killed. This means a match can report "no match" for a string it would have
matched given unbounded time.

### The three unbounded reads

`fetch`, an HTTP `import` and an `OS.spawn` child's stderr each read an amount the
program does not control, from a source it does not control, into memory. They
share **one** ceiling rather than one each: this page is a single policy, and a
second kind of ceiling would make it two.

64 MiB rather than the 256 MiB `File.read` and `Memory.alloc` use, because those
read something the user chose from their own disk while these read whatever a
remote host or a child process decides to send.

The reader takes one byte *past* the ceiling before deciding, so a body of
exactly 64 MiB is accepted and one of 64 MiB + 1 is refused.
`tests/read_ceiling.rs` asserts both, the far-over case, that the refusal is not
catchable, and that an ordinary small body is unaffected.

`OS.spawn`'s is the one that cannot be raised where it is detected: the stderr
pipe is drained by a background thread that has no evaluator. That thread records
the ceiling, and `OS.tick` raises it when it harvests the job — so the failure
still reaches the program as a fatal `ResourceError`, just at the next harvest
rather than at the moment the byte arrived.

Until 10.0.0 none of the three had a ceiling at all.

## What is not limited

These are known gaps, not guarantees:

- **Total memory.** The individual caps above are not a process-wide budget.
  There is no ceiling on aggregate GPU buffers, arena growth, array length,
  string length or dictionary size. A program can exhaust host memory; the
  language has no by-design guard against it. Serez does not run a garbage
  collector, so memory is reclaimed by scope and arena lifetime rather than by
  collection.
- **A generator's laziness.** `fn*` still is not lazy: calling one runs the body
  to completion and returns an ordinary array of everything it yielded
  (`control-flow.md`). What *is* limited now is how much it may accumulate — see
  the ceiling above, added in 10.0.0 — so an unbounded generator stops with a
  fatal `ResourceError` instead of growing until the host runs out of memory. The
  measurement that shaped the number: about 160 bytes per value, linear —
  100,000 yielded integers cost 20 MB, 400,000 cost 71 MB and 1,600,000 cost
  254 MB. One million is therefore roughly 160 MB, in the same range as the
  256 MiB ceilings above, and ten thousand times the largest generator in the
  conformance suite. The host may set a different one; a running program cannot.
  A lazy or streaming redesign remains a separate architectural change, and this
  ceiling does not prejudge it.
- **`OS.exec` output, inside `unsafe`.** The ceiling below is **waived** there,
  by design: see `security.md`'s waivable-guarantee table. The child's stdout and
  stderr are captured whole and returned as two strings on `ExecResult`. The size
  is the child's choice and the cost is roughly **5×** it — measured against a
  release build, a child emitting 200 MiB took the interpreter to a peak working
  set of **1,009.6 MiB** and succeeded; 16 MiB cost 56.3 MiB.

  `OS.exec` requires `unsafe`, so this is what every call does today. It is not
  reachable from untrusted source — the `OS` permission is needed too, and under
  lockdown `use permissions` is refused. A program that needs a large amount of
  output from a child should write it to a file and read that, which is bounded
  at 256 MiB. **DEC-M9-003** is closed by this.
- **Wall-clock time.** There is no execution timeout or Task cancellation. A
  `while (true)`—including inside a worker—runs until the process is stopped.
- **File and socket count.** Open handles are bounded by the host, not by the
  language.
- **Imported module count and total source size.** Bounded by memory only.

None of these is a security boundary. A program that must be constrained in
these dimensions has to be constrained by the operating system — see the
trusted/untrusted execution contract, not this document.
