# Memory

Normative contract for `Memory` — raw byte buffers, reached through handles.

Every rule here was derived by probing the running implementation. Rules carry a
**normative identifier** (`MEM-nnn`); see `conformance.md` for what an identifier
means and how it is verified.

`Memory` is the language's escape hatch to untyped storage. It does not exist to
be convenient — it exists so that a program that must speak to a binary layout can
do so without leaving Serez, and every operation that can corrupt state is behind
`unsafe`.

## Handles, not pointers

`Memory.alloc` returns an **integer handle**, not an address. Nothing in the
language can turn a handle into a machine address, and a handle is meaningless to
any process but the one that issued it.

**[MEM-001]** A handle is an `int`. Its value is an implementation detail: a
program may compare handles for equality and must not depend on their magnitude,
ordering or density.

**[MEM-002]** A freed handle's value is **never reissued**. `alloc` after `free`
returns a handle distinct from the freed one, for the lifetime of the evaluator.

**[MEM-003]** Every operation naming a handle that is not currently allocated —
because it was freed, or was never issued — raises a **catchable** error. This
covers `free`, `size`, `read`, `write`, `copy` and `fill` alike.

MEM-002 and MEM-003 exist together and neither is sufficient alone. Reissuing
would make a stale handle silently address a different buffer; accepting an
unknown handle would make a stale handle silently address nothing. Together they
make use-after-free a reported error rather than a corruption.

## The `unsafe` boundary

**[MEM-004]** `alloc`, `free`, `read`, `write`, `copy` and `fill` require lexical
`unsafe { }`. Outside it the call is refused, and the refusal is **not catchable**
— a `try` around it does not consume the denial. See `security.md`.

**[MEM-005]** `sizeof`, `size` and `offsetOf` do **not** require `unsafe`. They
report facts about a layout or an allocation and cannot modify anything.

## Allocation

**[MEM-006]** `Memory.alloc(n)` allocates `n` zeroed bytes. `n` must be at least
`1`; `alloc(0)` raises a catchable `TypeError`.

**[MEM-007]** `n` must not exceed **256 MiB**. Above it the call raises a
**fatal** `ResourceError` / `SZ6002` naming the maximum — a `try` around it does
not consume the failure, and the program stops. The ceiling is per allocation,
not per program: a program may hold many allocations.

The asymmetry with MEM-006 is deliberate and worth stating, because the two look
alike and are not. `alloc(0)` is a **caller mistake** — catchable `TypeError`,
like any bad argument. Exceeding the ceiling is a **resource limit** — fatal, like
every other ceiling in `limits.md`. A program can be written to handle the first
and cannot be written to handle the second.

**[MEM-008]** `Memory.free(handle)` releases the buffer. Freeing twice raises
under MEM-003 — the second `free` names a handle that is no longer allocated.

**[MEM-009]** `Memory.size(handle)` returns the byte length passed to `alloc`.

## Reading and writing

**[MEM-010]** `Memory.read(handle, offset, type)` takes exactly 3 arguments;
`Memory.write(handle, offset, type, value)` takes exactly 4. Wrong arity raises a
catchable `TypeError` and is checked **after** the `unsafe` gate, so a call made
outside `unsafe` is refused for that reason first.

**[MEM-011]** `type` is a string naming a layout from the `sizeof` table below.

**[MEM-012]** An access that would touch a byte outside `[0, size)` raises a
catchable error and writes nothing. A partially-completed write is not a state
this contract permits.

**[MEM-013]** `Memory.copy` and `Memory.fill` are bounds-checked by the same rule.

## Layout

**[MEM-014]** `Memory.sizeof(type)` returns the byte width of a layout name:

| Name | Bytes | | Name | Bytes |
| --- | --- | --- | --- | --- |
| `bool` | 1 | | `uint8` | 1 |
| `byte` | 1 | | `uint16` | 2 |
| `int8` | 1 | | `uint32` | 4 |
| `int16` | 2 | | `uint64` | 8 |
| `int32` | 4 | | `float32` | 4 |
| `int64` | 8 | | `float64` | 8 |
| `int` | 8 | | `decimal` | 8 |
| `ptr` | 8 | | `str` | 8 |

`int` is `int64` and `decimal` is `float64`, matching `values.md`. `ptr` and `str`
are 8 because a handle and a string reference are both machine words; neither is
the size of what they refer to.

**[MEM-015]** `Memory.offsetOf(...)` takes exactly 2 arguments. Wrong arity raises
a catchable `TypeError`.

## What this contract does not promise

- **No alignment guarantee.** A buffer is a byte array. Reading a `float64` at
  offset 1 is permitted by MEM-012 and its result is whatever those eight bytes
  hold.
- **No initialisation guarantee beyond `alloc`.** MEM-006 zeroes at allocation;
  nothing re-zeroes on `free`, and MEM-002 plus MEM-003 are what make that safe
  rather than observable.
- **No cross-process meaning.** A handle is valid inside one evaluator. Passing
  one to a task worker is not covered here; see `tasks.md`.

## Related

`security.md` for the `unsafe` gate and why its refusal is not catchable.
`limits.md` for resource ceilings elsewhere in the runtime. `values.md` for what
`int` and `decimal` are. `errors.md` for the diagnostic model.
