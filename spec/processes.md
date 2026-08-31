# Processes and the environment

Normative contract for `OS`, `Env` and `System`: starting external processes,
reading and setting environment variables, and asking the host about itself.

Every rule here was derived by probing the running implementation.

## Permissions and gates

Each namespace requires its own permission — `OS`, `Env`, `System` — declared in
`serez.json` or with `use permissions { … }`. Without it the call is a **fatal**
`PermissionError` / `SZ6001` that `try/catch` cannot consume.

Four operations additionally require an `unsafe { }` block, and outside one are
a **fatal** `UnsafeError` / `SZ6003`:

| Operation | Reason given by the gate |
|---|---|
| `OS.exec` | *it executes an external process* |
| `OS.spawn` | *it starts an external process* |
| `OS.kill` | *it terminates an OS process* |
| `Env.set` | *it modifies process environment state* |

`OS.exec` and `OS.spawn` are additionally refused when the command string
contains one of five protected system-path fragments, as a fatal
`SecurityError` / `SZ6004`. That check is a case-sensitive substring test, not
a security boundary — `security.md` states exactly what it does and does not
stop, and it should be read before relying on it.

## API

```text
OS.platform()            -> string   // "windows", "linux", "macos", …
OS.pid()                 -> int
OS.exec(cmd, args?)      -> ExecResult      // unsafe; blocks
OS.spawn(cmd, args?)     -> int             // unsafe; pid, or -1
OS.tick()                -> [[int, int, string]]
OS.kill(pid)             -> null            // unsafe

Env.get(name)            -> string | null
Env.set(name, value)     -> null            // unsafe
Env.args()               -> [string]

System.cpuCount()        -> int
System.totalMemory()     -> int     // bytes
System.freeMemory()      -> int     // bytes
System.hostname()        -> string
System.uptime()          -> int     // seconds
```

`OS.platform`, `OS.pid`, `OS.tick`, `Env.args` and all five `System` readers
take **no arguments** and refuse any you pass, with `TypeError` / `SZ4002`.

## Arguments to a process

`args` is optional. When given it must be an **array of strings**, and every
element must be a string. A non-array, or an element that is not a string, is a
`TypeError` / `SZ4002` raised **before the process is started**.

This is worth stating because it did not use to be true. Both methods collected
the vector while ignoring anything that did not match, so:

```text
OS.exec("cmd", "/c echo hi")       // launched a bare interactive shell, code 0
OS.exec("cmd", ["/c", "echo", 42]) // ran echo with no operand
```

The command still ran, it ran as something else, and the result reported
success. An argument that disappears can be the one that made a command safe.

An **omitted** `args` and an **empty array** both mean "no arguments". No
shell is involved: `cmd` is the executable, resolved by the host's normal
lookup, and the arguments are passed as a vector. There is no quoting or word
splitting to reason about, and no shell metacharacter has any meaning. To use a
shell, invoke one explicitly.

## `OS.exec` — run and wait

Returns a value of the built-in type `ExecResult`:

```text
ExecResult{ stdout: string, stderr: string, code: int }
```

`code` is the child's exit code, or `-1` when the host reports none (killed by
a signal, typically). `stdout` and `stderr` are the **complete** captured
streams, decoded as UTF-8 with invalid bytes replaced by `U+FFFD` — a child
that emits binary output produces replacement characters rather than an error.

`OS.exec` **blocks until the child exits.** There is no timeout, no way to
cancel and no way to write to the child's stdin, so a child that waits for
input waits forever and takes the interpreter with it. `OS.spawn` plus
`OS.tick` is the non-blocking alternative.

A command that cannot be started at all — not found, not executable — is a
catchable `OSError` / `SZ4000`.

## `OS.spawn` and `OS.tick` — start and poll

`OS.spawn` starts the process and returns its pid immediately. **The child's
stdout is discarded**; its stderr is captured and delivered later. Nothing is
inherited from the interpreter's own streams, and on Windows no console window
is created.

`OS.tick()` harvests every child that has finished since the last call and
returns them as data — never a callback:

```text
[[pid, code, stderrText], …]
```

Each finished child is reported **exactly once**; a second `tick` returns an
empty array. `tick` never blocks, requires no `unsafe` block (it starts
nothing), and returns an empty array when nothing has finished.

**`OS.spawn` reports a failure to start differently from `OS.exec`.** Where
`exec` raises a catchable `OSError`, `spawn` returns **`-1`** and prints a
warning to stderr. A caller cannot catch it and must compare the return value.
This asymmetry is known debt: two sibling methods reporting the same failure
two different ways. It is documented rather than changed because `-1` is the
return value the poll-based API was designed around, and changing it is a
public breaking change that should be made deliberately, not as a side effect.
Until then, **check the return value of `OS.spawn`.**

## `OS.kill`

Terminates a process by pid. It does not signal — it invokes the host's own
utility: `taskkill /PID <pid> /F` on Windows, `kill <pid>` on Unix. The
platforms therefore differ in a way the API hides: Windows force-terminates,
Unix sends the default `TERM` and the target may handle or ignore it.

**A kill that does not succeed is a catchable `OSError` / `SZ4000`**, carrying
the helper's own message. This includes killing a process that has already
exited, which matters for the obvious pattern: a child spawned earlier may
finish on its own before you get to it, so a caller that kills a possibly
finished child must catch the failure. That is the deliberate trade-off — the
alternative was the previous behaviour, where every failure, including a denial,
returned the success value while the helper's error text leaked into the
program's stderr from a process the caller never launched.

`OS.kill` returns `null`. It does not report whether the target was your child.

## `Env`

`Env.get(name)` returns the value, or **`null`** when the variable is not set —
not an empty string, and not an error. `type_of` on the result is `"null"`.

`Env.set(name, value)` changes the environment of the **running interpreter**,
which means child processes started afterwards inherit it. It does not reach
the parent shell: nothing survives the process exiting. It requires `unsafe`.

`Env.args()` returns the **whole process command line**, not the script's
arguments:

```text
sz args.sz foo bar
→ ["…/sz.exe", "args.sz", "foo", "bar"]
```

Index 0 is the interpreter's own path and index 1 is the script, so a program's
own arguments begin at index 2. Nothing strips them for you.

## `System`

Five readers, all taking no arguments. `totalMemory` and `freeMemory` are in
**bytes**; `uptime` is in **seconds**. `cpuCount` is the parallelism the host
reports, falling back to `1` when it cannot say. `hostname` is whatever the host
returns and carries no guarantee of being resolvable as a network name.

These are snapshots. Nothing caches them and nothing guarantees two consecutive
calls agree.

## Errors

| Situation | Code | Kind | Catchable |
|---|---|---|---|
| Wrong argument count, wrong argument type | `SZ4002` | `TypeError` | yes |
| `args` not an array, or an element not a string | `SZ4002` | `TypeError` | yes |
| Command cannot be started (`exec`), failed kill | `SZ4000` | `OSError` | yes |
| Unknown method name | `SZ4001` | `ReferenceError` | yes |
| Missing permission | `SZ6001` | `PermissionError` | **no** |
| `exec` / `spawn` / `kill` / `Env.set` outside `unsafe` | `SZ6003` | `UnsafeError` | **no** |
| Protected system-path fragment in the command | `SZ6004` | `SecurityError` | **no** |
| `spawn` cannot start the process | — | — | not an error: returns `-1` |

`OSError` shares the generic `SZ4000` bucket with fourteen other kinds, so
match on `kind` as well as `code` when you need to tell them apart — see
`errors.md`.

## Not specified

- **stdin.** No method writes to a child's standard input.
- **Working directory and environment per child.** A child inherits the
  interpreter's; neither can be set per call.
- **Signals.** There is no signal API. `OS.kill` is the only way to stop a
  process, and what it means differs by platform.
- **Process groups, exit-on-parent-death, orphan reaping.** A process started
  with `OS.spawn` and never harvested outlives nothing in particular; the host
  decides.
- **Quoting.** Because no shell is involved there is nothing to quote, but that
  also means a command string containing arguments — `"git status"` as one
  string — is looked up as a single executable name and will not be found.

## Conformance evidence

Pinned by `unit_os_spawn.sz`, `sec_os_spawn_requires_unsafe.sz`,
`sec_os_spawn_system_path.sz`, `sec_notcatch_system_path.sz`, and in
`tests/runtime_outcome.rs` by `process_arguments_are_never_silently_dropped`,
`a_failed_kill_is_reported_instead_of_leaking`, the `OS`/`Env` rows of
`unsafe_gates_are_structured_but_not_catchable` and
`zero_argument_native_methods_reject_arguments`, and the `OS`, `Env` and
`Terminal` rows of `user_throw_survives_native_argument_evaluation`.
