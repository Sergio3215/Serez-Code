# Limits

Normative ceilings a Serez program runs under. Every one of them exists for the
same reason: without it, ordinary input reaches a native failure the language
cannot describe — a stack the process cannot grow, an allocation the host
refuses, a loop that does not end. A limit turns that into something with a
message, a position and an exit code.

A limit is part of the language contract. Raising one is compatible. Lowering
one is compatibility-impacting and normally follows the deprecation policy; an
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

The ceiling is far above real code. Across the 999 `.sz`/`.szx` files in the
official ecosystem, the deepest nesting is 19 levels and the longest operator
chain is 25 operators. Source that genuinely needs more should build the
structure at runtime rather than spell it out.

## Runtime limits

| Limit | Value | Behavior on breach |
| --- | --- | --- |
| Call depth | 512 frames | Fatal `ResourceError` (`SZ6002`). |
| String repetition | 10,000,000 repetitions | Fatal `ResourceError` (`SZ6002`). |
| Padded string result | 10,000,000 Unicode scalar values | Fatal `ResourceError` (`SZ6002`) before result allocation. |
| Tensor element count | 10,000,000 `f64` elements | Fatal `ResourceError` (`SZ6002`). |
| Regex execution steps, per match | 1,000,000 | Match fails. |
| Regex backtracking depth | 8,000 | Match fails. |
| `Memory.alloc` size | 256 MiB | Larger requests are fatal `SZ6002`; requires `unsafe`. |
| One GPU buffer | 256 MiB = 33,554,432 `f64` elements | Larger creation/upload/matmul results are fatal `SZ6002`. |
| `File.read` / `File.read_asBinary` | 256 MiB | Larger files are rejected before contents are read, fatal `SZ6002`. |
| WebSocket frame payload | 16 MiB | Frame rejected. |
| Concurrent Task workers, per runtime | 32 | New worker creation is fatal `SZ6002`. |
| Task argument, reply or stored worker error | 1 MiB | Larger messages become `SZ6002`; worker error text is bounded before retention. |
| Task worker source | 16 MiB | Worker enters failed state before parsing. |
| Retained Task records, per runtime | 256 | Oldest terminal record is evicted; active workers remain. |

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

The two regex limits bound the matcher rather than the pattern: a pattern with
catastrophic backtracking fails to match instead of running until the process is
killed. This means a match can report "no match" for a string it would have
matched given unbounded time.

## What is not limited

These are known gaps, not guarantees:

- **Total memory.** The individual caps above are not a process-wide budget.
  There is no ceiling on aggregate GPU buffers, arena growth, array length,
  string length or dictionary size. A program can exhaust host memory; the
  language has no by-design guard against it. Serez does not run a garbage
  collector, so memory is reclaimed by scope and arena lifetime rather than by
  collection.
- **Wall-clock time.** There is no execution timeout or Task cancellation. A
  `while (true)`—including inside a worker—runs until the process is stopped.
- **File and socket count.** Open handles are bounded by the host, not by the
  language.
- **Imported module count and total source size.** Bounded by memory only.

None of these is a security boundary. A program that must be constrained in
these dimensions has to be constrained by the operating system — see the
trusted/untrusted execution contract, not this document.
