# Command-line interface

Normative contract for the `sz` executable: what it accepts, what it writes
where, and what it returns.

Every statement here was checked against the running binary, and the exit-code
and stream rules are pinned by CLI tests in `run_tests.ps1` and `run_tests.sh`.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. The program ran to completion, the check passed, the subcommand succeeded, or an informational flag (`--version`, `--help`) was answered. |
| `1` | Any failure: usage error, unknown flag, missing or non-`.sz` file, lexer/parser/type diagnostic that aborts, runtime error, uncaught exception, or a failed package subcommand. |

There is deliberately no finer granularity today. A caller that needs to know
*why* something failed reads the diagnostic code on stderr, which is stable;
inventing distinct exit codes now would freeze a classification the runtime
does not yet make consistently. See `errors.md`.

An uncaught user `throw` and a runtime error both exit `1`. So does a
type-checker diagnostic that aborts — but note the checker is advisory: `sz
file.sz` reports `SZ3000` diagnostics and still runs the program.

## Streams

- **stdout** carries program output (`out`), `--check` reports, `--version`,
  `--help` and package-command progress.
- **stderr** carries diagnostics only.

Nothing a program prints goes to stderr, and no diagnostic goes to stdout. A
caller can therefore pipe stdout without filtering, and `sz --help | less`
works.

Diagnostics are rendered as:

```
❌ ERROR [SZ4002]: Parameter 'n' expected 'int' but received 'string'
    called from 'outer' [line 3:10]
    3 | out outer("hola");
                 ^
```

The code in brackets is stable. The wording is not: tooling must match on the
code. See `errors.md` for the ranges.

## Running programs

| Invocation | Behavior |
| --- | --- |
| `sz <file.sz>` | Lex, parse, type-check, run. |
| `sz <file.szx>` | Translate to `.sz` first, then the same. See below. |
| `sz --check <file>` | Same pipeline without evaluation; prints a report. |
| `sz --watch <file>` | Re-runs the file whenever it changes. |
| `sz --eval "<code>"` / `sz -e "<code>"` | Runs a snippet. |
| `sz --eval -` | Reads the snippet from stdin. |
| `sz` | Starts the REPL. |
| `sz --version` | Prints `Serez-Code vX.Y.Z`. |
| `sz --help` / `sz -h` / `sz help` | Prints usage on stdout, exit 0. |

A file argument must end in `.sz` or `.szx`. Anything else is a usage error
before the file is read.

### `--eval` runs under lockdown

A snippet has no manifest behind it, so it cannot declare permissions and
cannot grant itself any. `use permissions` inside `--eval` is refused with a
fatal `SecurityError` (`SZ6004`). `File`, `import` and Autodiff weight I/O are
closed. `fetch` is deliberately **not** closed — see `security.md`, which also
states plainly that lockdown is not a sandbox.

Running your own file is unaffected: `sz file.sz` still honours an inline
`use permissions` block.

### `.szx` is translated before it runs

A `.szx` file is serez-ui's JSX dialect, and the core does not parse it. Running
one is a two-step operation, and the differences from `sz file.sz` are visible
to the user:

- **It requires serez-ui.** The translator is serez-ui's own `tools/translate.sz`
  — written in Serez, not linked into the core — so `sz app.szx` spawns a second
  `sz` process to run it. Without serez-ui installed the command fails with an
  install hint. It is looked for under `packages/` in the working directory,
  then `packages/` beside the source, then the global store, then the
  executable's own directory.
- **It writes a file beside the source.** The translation has the app's relative
  imports in it, so it only resolves from the source's own directory; a temp
  directory would break every `import "comp/Chip"`. The name is generated and
  unique per process and per call — it is not derived from the source name, so
  it cannot collide with a file you own, and two concurrent runs cannot race for
  it. It is removed when the run ends.
- **Diagnostics refer to the translated source.** A parse or runtime error
  carries the translated file's name, line numbers and source snippet, not the
  `.szx` as written. A note after the diagnostic says so. There is currently no
  way to keep the translated file and read it.

`--check` and `--watch` accept `.szx` and translate it the same way.

### The REPL

`sz` with no arguments starts the REPL. Each line is lexed, parsed and
evaluated as a **complete program of its own**, against an evaluator that
persists across lines — so a `let` or `fn` on one line is visible on the next,
and the value of the line is echoed (a line whose value is nothing echoes
`null`).

The REPL is **not** under lockdown. It is the user typing on their own machine,
so it grants like `sz file.sz` and unlike `--eval`: an inline
`use permissions` block is honoured.

Two rules it shares with `sz file.sz`:

- **A line with parse errors does not run.** The diagnostic is printed with the
  offending source and a caret, followed by `Aborted: fix the parse errors
  above before running.`, and the session continues at the next prompt. Until
  this was fixed the REPL printed the diagnostic and then evaluated anyway, so
  `out "x"; let y = ;` printed `x` here while the identical line in a file
  aborted without running anything.
- **A runtime error is reported and the session continues.** Only the line that
  raised it is abandoned.

One deliberate difference: the REPL does **not** run the type checker. Each
line is parsed as an independent program, so a checker running per line would
see calls to functions declared on earlier lines as unknown and report
diagnostics that are simply wrong. The checker is advisory everywhere
(`types.md`), so nothing is enforced that would otherwise be caught — runtime
checks remain authoritative, and they run.

Input that is not valid UTF-8 is reported and the line is skipped; the session
survives it. `sz file.sz`, an imported module and `--eval -` all reject
non-UTF-8 input with a diagnostic and exit 1.

## Package commands

| Command | Behavior |
| --- | --- |
| `sz init [--y]` | Creates `serez.json`. |
| `sz install [<pkg>[@<ver>]]` | Installs one package, or every dependency in `serez.json`. |
| `sz uninstall <pkg>` | Removes a package. |
| `sz update [<pkg>]` | Updates one package, or all of them. |
| `sz info <pkg>` | Prints a package's manifest. |
| `sz run <script> [args...]` | Runs a script from `serez.json`. |
| `sz publish` / `sz unpublish <pkg>@<ver>` | Registry operations. |
| `sz logout` | Forgets stored registry credentials. |

`-g` / `--global` applies a package command to the global store instead of
`./packages`.

The `serez-code` key in `dependencies` is the minimum runtime, not a package:
`sz install` checks it and never fetches it. See `compatibility.md`.

## Not yet specified

- **Machine-readable diagnostics.** There is no `--json` output. A tool that
  wants structure today must embed the runtime and use `run_source_detailed`,
  which returns the same payload the CLI renders.
- **Finer exit codes.** See above.
- **Locale.** Diagnostics are English; a few historical messages are Spanish.
  Neither is a stable surface.
