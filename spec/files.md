# Files

Normative contract for the `File` namespace: reading, writing, listing and
removing paths on the host filesystem.

Every rule here was derived by probing the running implementation. Where a
behaviour is inherited from the host rather than chosen by Serez, this document
says so, because those are the rules that differ between machines.

## No permission gates this namespace

`File` is **not** behind a permission. Unlike `OS`, `Socket`, `Task`, `Gui`,
`Media` and `Time`, it reads, writes, renames and deletes with nothing
declared. Declaring `File` in `serez.json` or `use permissions { File }`
changes nothing — it is accepted and inert. `security.md` records why that is
the case and what it costs; the short version is that `File` is one of the
three capabilities the permission set does not cover.

Two operations are gated by `unsafe { }` instead:

- `File.delete` — *"it permanently removes files"*
- `File.rename` — *"it modifies the filesystem"*

Outside an `unsafe` block both are a **fatal** `UnsafeError` / `SZ6003`, which
`try/catch` cannot consume. Note what this does *not* cover: `File.write` on an
existing path replaces its contents with no `unsafe` block and no warning. The
gate is on removing and moving, not on destroying.

Under **lockdown** — `sz --eval`, and any source the runtime treats as
untrusted — every `File` method is refused with a `PermissionError` / `SZ6001`.
That one *is* catchable.

## API

```text
File.exists(path)                 -> bool
File.read(path)                   -> string
File.write(path, content)         -> null
File.create(path)                 -> null
File.read_asBinary(path)          -> [int]      // 0-255
File.write_asBinary(path, bytes)  -> null
File.listDir(path)                -> [string]   // names, not paths
File.mkdir(path)                  -> null
File.stat(path)                   -> FileStat
File.delete(path)                 -> null       // unsafe
File.rename(from, to)             -> null       // unsafe
```

Every path is a `string`. A relative path is resolved against the **process
working directory** — where `sz` was invoked — not the directory of the source
file. A script is not portable across the directory it is started from.

`..` and absolute paths are not restricted in any way. `File.read("../../x")`
walks out of the project, and `File.read("C:/Windows/win.ini")` is an ordinary
read. There is no confinement.

The two binary methods keep their historical `read_asBinary` /
`write_asBinary` spelling, which does not match the camelCase of the rest of
the language. It is kept because renaming it would break every caller.

## Reading

`File.read` returns the whole file as a `string` and **requires valid UTF-8**.
A file that is not valid UTF-8 is an `IOError`, not replacement characters:

```text
File.read("image.png")   // IOError: stream did not contain valid UTF-8
```

`File.read_asBinary` has no such requirement. It returns an `[int]` array with
one element per byte, each in `0..=255`. An empty file yields an empty array
from both methods.

Both refuse a file larger than **256 MiB** before reading a single byte, as a
fatal `ResourceError` / `SZ6002`. See `limits.md`.

Reading a **directory** is an `IOError`. The message comes from the host and
differs between platforms — *"Access is denied. (os error 5)"* on Windows,
*"Is a directory"* on Unix — so match on the code, never on the wording.

## Writing

`File.write(path, content)` replaces the file's contents, creating it if it
does not exist. It does **not** create missing parent directories: writing to
`nodir/x.txt` when `nodir` does not exist is an `IOError`.

**`content` is not type-checked.** Any value is accepted and written using the
same rendering as `out`:

```text
File.write("f", 42)        // writes "42"
File.write("f", true)      // writes "true"
File.write("f", [1, 2, 3]) // writes "[1, 2, 3]"
File.write("f", null)      // writes "null"
```

This is the one place in the namespace where a wrong-typed argument is
converted rather than refused, and it is worth stating plainly because of what
it does to a mistake: a variable that is unexpectedly `null` does not raise —
it writes the four characters `null` into the file, and the failure surfaces
later, somewhere else, as data. A caller that means to write text should
convert deliberately.

`File.write_asBinary(path, bytes)` is strict by contrast. `bytes` must be an
array and **every element must be an `int` in `0..=255`**; a decimal, a
negative value, `256` or a string is a `TypeError` / `SZ4002` and nothing is
written. An empty array writes an empty file.

`File.write` returns `null`, not the number of bytes written.

## Creating, listing and describing

`File.create(path)` is a touch: it creates an empty file if nothing is there,
and **does nothing at all if anything is** — including an existing directory.
It never truncates. Two consequences follow, and both have surprised callers:

- `File.create(p)` succeeding does **not** mean `p` is now a readable file. If
  `p` was a directory it still is, and the next `File.read(p)` fails.
- `File.create(p)` on an existing file leaves its contents untouched. It is not
  a way to empty a file; `File.write(p, "")` is.

`File.mkdir(path)` creates the directory **and every missing parent**. Calling
it on a directory that already exists succeeds and is a no-op. Calling it on a
path where a *file* exists is an `IOError` — unlike `create`, `mkdir` refuses
to be confused about what is already there. The empty path is an `IOError`
(the host call would otherwise report success while creating nothing).

`File.listDir(path)` returns the **entry names**, not paths: listing `src`
gives `["main.sz", "util.sz"]`, and a caller that wants to open one must join
it with the directory itself. Three rules:

- **No order is guaranteed.** The entries arrive in the order the host
  filesystem yields them. NTFS happens to return them sorted; other filesystems
  do not. Sort explicitly if order matters.
- **Entries that cannot be read are omitted**, not reported. A directory whose
  contents change during the listing can therefore return a *short* list with
  no error. There is no way for a caller to tell.
- A name that is not valid Unicode on the host is returned with the invalid
  parts replaced by `U+FFFD`, which means the returned name may no longer open
  the file it came from.

`File.stat(path)` returns a value of the built-in type `FileStat`:

```text
FileStat{ size: int, modified: int, isDir: bool }
```

`size` is bytes and is `0` for a directory. `modified` is milliseconds since
the Unix epoch, or `-1` when the host cannot supply it. `type_of` reports
`"FileStat"`, and reading any other field is a `ReferenceError` / `SZ4001`.
There is no way for a program to declare or construct this type itself.

## Removing and moving

Both require `unsafe { }`.

**`File.delete` on a directory removes it and everything inside it**, without a
recursive flag and without confirmation. On a file it removes the file. The
`unsafe` gate's own wording — *"it permanently removes files"* — understates
this: a mistyped path that happens to name a directory takes the whole tree.

**`File.rename(from, to)` silently replaces `to` if it exists.** Its contents
are gone, no error is raised, and there is no flag to prevent it. Check with
`File.exists` first if that matters. Renaming a path that does not exist is an
`IOError`.

## Errors

| Situation | Code | Kind | Catchable |
|---|---|---|---|
| Wrong argument count | `SZ4002` | `TypeError` | yes |
| Wrong argument type (path not a string, byte not `0..=255`) | `SZ4002` | `TypeError` | yes |
| Any host failure — missing, denied, not a directory, invalid UTF-8 | `SZ4005` | `IOError` | yes |
| Unknown method name | `SZ4001` | `ReferenceError` | yes |
| File over 256 MiB in `read` / `read_asBinary` | `SZ6002` | `ResourceError` | **no** |
| `delete` / `rename` outside `unsafe` | `SZ6003` | `UnsafeError` | **no** |
| Any method under lockdown | `SZ6001` | `PermissionError` | yes |

The arity check runs **before** the arguments are evaluated, so
`File.exists(f(), g())` reports the wrong argument count without calling either
function. Once the count is right, arguments are evaluated left to right, and a
`throw` from one of them propagates to the caller unchanged — it reaches the
program's own `try/catch` with its payload intact, and no filesystem work
happens.

`IOError` messages embed the host's own text. It differs between platforms and
between locales; the code and kind do not. Match on those.

## Not specified

- **Symbolic links.** Nothing in the namespace distinguishes a link from its
  target, and `FileStat` has no field for it. Whatever the host does when
  following one is what happens.
- **Concurrency.** There is no locking, and no method is atomic with respect to
  another process. `File.exists` followed by `File.write` is two separate
  questions to the filesystem.
- **Permissions and ownership.** They can be neither read nor set.
- **Open handles.** There is no file-handle API — every method opens, acts and
  closes. Streaming a file larger than memory is not possible.
- **Encoding.** `File.read` and `File.write` are UTF-8 only. There is no way to
  read another encoding as text; use `read_asBinary` and decode in Serez.

## Conformance evidence

The behaviour above is pinned by `unit_file.sz`, `unit_file_extended.sz`,
`unit_catchable_io.sz`, `unit_native_throw_propagation.sz`,
`sec_file_rename_requires_unsafe.sz`, the `File` rows of
`unsafe_gates_are_structured_but_not_catchable` and
`user_throw_survives_native_argument_evaluation` in `tests/runtime_outcome.rs`,
and the `tests/filesystem_reach.rs` suite.
