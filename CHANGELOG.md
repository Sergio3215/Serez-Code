# Serez-Code — Changelog

Technical record of all changes to the language, stdlib, and tooling.  
Order: most recent to oldest.

---

## [Unreleased] — maturity hardening

### `spec/processes.md`, a kill that always claimed success, and a false error marker

- `OS`, `Env` and `System` are the process and environment surface: permission
  gated, `unsafe` gated, and until now unspecified. The document states the
  gates, the argument contract, `ExecResult`, the spawn/tick polling model and
  the environment readers, all measured.
- **`OS.kill` returned the success value whatever happened.** It ran `taskkill`
  / `kill` with `.status()` and matched `Ok(_)` — but `Ok` means *the helper
  ran*, not that the target died. `OS.kill(999999)` returned `null` as if the
  kill had worked, while `ERROR: The process "999999" not found.` appeared on
  the program's stderr, from a process the caller never launched. The helper's
  output is captured now and a failure is a catchable `OSError`. The trade-off
  is deliberate and documented: killing a child that has already exited raises,
  so a caller racing a child's exit must catch it. Nothing in the ecosystem or
  the test suite called `OS.kill`, so there was no installed behaviour to keep.
- **`OS.spawn` printed the fatal marker for a non-fatal failure.** A failure to
  start emitted `❌ ERROR: …` — the marker the CLI and the conformance runner
  both read as "this program failed" — and then returned `-1` and let the
  program continue to exit 0. It is a `⚠️  WARNING:` now, which is what it
  always was.
- **The asymmetry that remains is written down, not hidden.** `OS.exec` raises
  a catchable `OSError` when a command cannot be started; `OS.spawn` returns
  `-1`. Two sibling methods, the same failure, two different reporting models,
  and only one of them catchable. Resolving it is a public breaking change to
  the return value the polling API was designed around, so it is recorded as
  debt with the advice that follows from it — check what `OS.spawn` returns.
- Three facts the document states that nothing had: `OS.exec` blocks with no
  timeout and no access to the child's stdin, so a child waiting for input
  takes the interpreter with it; `OS.spawn` **discards the child's stdout**
  entirely and delivers only stderr, through `tick`; and `Env.args()` is the
  whole process command line, so a program's own arguments start at index 2.


### `OS.exec` and `OS.spawn` ran a different command than you wrote

- Both built their argument vector with `if let Some(Array) = .. { for elem { if
  let Str(s) = elem { .. } } }` and no `else` on either shape. A non-array
  `args` was ignored **entirely**; a non-string element was **dropped**. The
  process then started with a different argument list and reported success.
- Measured, not imagined:
  - `OS.exec("cmd", "/c echo HELLO")` — a string where an array was expected —
    launched a **bare interactive shell**, printed the Windows banner, and
    returned `code: 0`.
  - `OS.exec("cmd", ["/c", "echo", 42])` ran `echo` with no operand. The `42`
    was gone and the call still succeeded.
- For a process API this is the worst shape a defect can take: the command
  runs, it runs as something else, and nothing says so. An argument that
  disappears can be the one that made a command safe.
- Both now fail with `TypeError` / `SZ4002` **before anything is started** —
  `args must be an array of strings`, or `every argument must be a string`. The
  two identical call sites became one helper.
- Swept across the ecosystem before enforcing: every official call site in
  `serez-pack`, `serez-apipack` and `serez-cobol` already passes an array of
  strings. An omitted list and an empty list both still mean "no arguments".
- The earlier native sweep reported "zero silent defaults" because it grepped
  for the `_arg(..).unwrap_or(..)` shape. This defect is an `if let` with no
  `else`, which that search could not see.


### `spec/ecosystem.md` — the tiers existed only as a proposal, so nothing could fail them

- `compatibility.md` says what a *change* promises. Nothing said what a *thing*
  is: which layer it belongs to, what tier of promise it carries, or what a tier
  obliges it to do. The tiers were three bullets inside the internal audit,
  which meant "Official" was a label rather than a bar — no package could fail
  to meet it.
- The document defines the three tiers by what may happen to them, and gives
  each a mechanical entry requirement. **Stable** needs a `spec/` document and
  conformance tests the document points at; behaviour with no document is
  unstable by definition, however long it has worked. **Official** needs a
  declared minimum runtime, a suite of its own, and a place in the shared
  ecosystem runner. **Experimental** may change or vanish in any release, but
  must fail with a diagnostic rather than degrade in silence.
- **Measured, not asserted.** Nine of the ten official packages declare no
  minimum runtime; `serez-cobol` and `serez-strike` pass their own suites (23/23
  and 113/113) but are absent from the shared runner; the ecosystem suite is not
  in CI; and no manifest field carries a tier, so tooling cannot act on any of
  this. All four are written down as unmet requirements.
- **The gap that makes the first one matter:** the minimum-runtime floor is
  checked by `sz install` and by nothing else. A project declaring
  `"serez-code": ">= 99.0.0"` runs on 9.17.0 without a word; `sz update` skips
  the key; installing a dependency never verifies that dependency's own floor.
  The declaration protects the author who types `sz install`, not the user who
  runs the program. Making it a run-time gate would refuse programs that work
  today, so it is documented rather than changed, and both halves are now pinned
  in the CLI suite on both platforms.
- The layering claim was corrected before publishing. The first draft said a
  lower layer never imports a higher one; `expr.rs` names twenty-one namespace
  modules, so Core reaches into Native capabilities today. The document states
  the layers as direction of travel and names the coupling that remains.


### `spec/files.md`, and five things about the filesystem nobody had written down

- `File` is the namespace every program can reach and the only disk surface no
  permission gates. It had no specification: the answer to "what happens if I
  pass this?" was "read the Rust". It is written now, from measurement.
- The five that will cost a caller time:
  - **`write` converts instead of refusing.** `content` is not type-checked —
    any value is rendered exactly as `out` would render it. A variable that is
    unexpectedly `null` does not raise; it writes the four characters `null`
    into the file, and the failure surfaces later, as data. `write_asBinary`,
    two methods away, refuses a `1.5` outright.
  - **`create` on an existing directory succeeds and creates nothing.** It
    never truncates either, so `File.create(p)` returning cleanly does not mean
    `p` is now a readable file. `mkdir`, by contrast, refuses to be confused
    about what is already at the path.
  - **`delete` on a directory removes the whole tree.** There is no recursive
    flag and no confirmation. The `unsafe` gate's own wording — "it permanently
    removes files" — understates what a mistyped path can take.
  - **`rename` replaces an existing destination silently.** Its contents are
    gone, with no error and no flag to prevent it.
  - **`listDir` guarantees no order and omits what it cannot read.** A
    directory changing during the listing returns a *short* list with no error,
    and the caller cannot tell.
- The document also states what the `unsafe` gates do **not** cover:
  `File.write` destroys an existing file's contents with nothing declared. The
  gate is on removing and moving, not on destroying.
- **One fix came out of writing it.** `File.mkdir("")` reported success and
  created nothing — `create_dir_all("")` is `Ok(())` in Rust — while `write`,
  `create`, `listDir` and `stat` all reject the empty path. It is an `IOError`
  now.
- `unit_file_contract.sz` pins all fifteen, including that the argument count
  is checked before any argument runs.


### Twenty more zero-argument members accepted arguments and threw them away

- A previous stage fixed five of these outside `Gui` and thirty-one inside it.
  It found them by calling each method with a fixed set of argument shapes and
  reading the result — and **the result is the wrong thing to read.** A reader
  that ignores your argument returns exactly what it would have returned
  without it. Nothing in the output separates "used it" from "discarded it".
- Passing a *throwing* call as the argument separates them: if the member never
  evaluates the expression, the exception never happens. Re-run that way,
  twenty members were still accepting and dropping what you wrote —
  `Math.PI`, `Math.E`, `Math.random`, `Dec.MAX`, `Dec.MIN`, `Dec.MAX_SCALE`,
  `Autodiff.clear`, `Autodiff.isRecording`, `Env.args`, `Time.now`,
  `System.cpuCount`, `System.totalMemory`, `System.freeMemory`,
  `System.hostname`, `System.uptime`, `Terminal.getSize`, `Terminal.clear`,
  `Gui.close`, `Gui.font` and `Gui.windowPosition`.
- **Three of them are a correction of this cycle's own work.** `Gui.close` and
  `Gui.font` were excluded from the Gui list by a comment in the source stating
  they "do read arguments". They do not — `Gui.setFont` is the setter, and the
  `"text" | "font"` arm that prompted the belief is scene-node property
  assignment, a different method. `Gui.windowPosition` was simply missed. The
  test had been written to agree with the comment, so it confirmed the mistake
  instead of catching it; it now asserts the opposite.
- Swept against 1,123 `.sz`/`.szx` files — the core tests, the benchmarks and
  all ten official packages — before being enforced. Not one passes an argument
  to any of these, so nothing that works today stops working. `Math.PI` and
  `Dec.MAX_SCALE` without parentheses are unaffected, and pinned.


### A `throw` inside a native argument was destroyed and blamed on the interpreter

- The program raises an exception while computing an argument to a native
  method. That exception belongs to the program. It did not reach the program:
  thirty-six argument sites across `File`, `JSON`, `Math`, `Env`, `OS`,
  `Terminal` and `Time` matched only `EvalResult::Value` and collapsed every
  other result — `Throw` among them — into a bare unstructured error.
- The visible outcome was the worst one available. `try/catch` never saw the
  exception, and the program died printing `SZ4999`: *"the runtime reported a
  failure without recording a diagnostic for it. This is a defect in
  Serez-Code itself, not in the program that was run."* The interpreter
  accused itself of a defect for an exception the source had deliberately
  raised, and the author's own payload was gone.
- All thirty-six sites now carry the propagation arm the rest of the runtime
  already used. `File.exists(boom())` reaches the handler with `"BOOM"`
  intact, and uncaught it reports `UNCAUGHT EXCEPTION: BOOM` rather than an
  internal defect.
- **Found by sweeping, not by reading.** A throwing call was passed to every
  native method of every namespace at arities one to four — 1,610 programs.
  Reading the source would not have produced this list: the same file mixes
  correct and incorrect sites, `Math.abs` propagates while `Math.sign`, two
  arms below it, did not.
- Two regressions keep it: `user_throw_survives_native_argument_evaluation`
  pins all thirty-four call forms from Rust, including second and third
  argument positions — a gate on argument one proves nothing about argument
  three — and `unit_native_throw_propagation.sz` pins the visible contract,
  that the handler runs, the payload arrives intact and execution continues.


### `SZ4000` has fifteen occupants, not three — correcting this cycle's own table

- The table added two stages ago said three kinds shared the generic bucket.
  That was a sample of what one probe happened to reach, not a reading of the
  source.
- `runtime_error_code` maps eleven kinds and sends **everything else** to
  `SZ4000`. Fifteen fall through: `GuiError` (forty sites, the most common kind
  after `TypeError`), `RangeError`, `RuntimeError`, `Overflow`, `TensorError`,
  `SocketError`, `BinaryError`, `OSError`, `MemoryError`, `MediaError`,
  `JsonError`, `GpuError`, `AutodiffError`, `InvalidAssignTarget`, and the
  defensive `InternalError`.
- The practical consequence is now stated: **for `SZ4000` the code alone is not
  enough.** Telling a GUI failure from a socket failure from an integer overflow
  needs `kind` as well. `compatibility.md` promises both fields are stable, so
  the advice "match on the code, never the wording" just needed its other half.
- `kind_to_code_map_covers_every_kind_raised` compares the source against the
  document directly, so the count cannot drift again. It found the fifteenth —
  `InvalidAssignTarget` — on its first run, which a hand count had missed
  because it is formatted across lines.

### `spec/socket.md`

- Written from a loopback probe: the eight signatures, the id space, the error
  contract and the fatal permission gate, all measured.
- Three facts nothing had written down:
  - **`recv` is partial and blocking.** It returns at most `maxBytes` and leaves
    the remainder queued — sending `"abcdef"` then `recv(conn, 3)` yields
    `"abc"`, and the next call `"def"`. A caller wanting a whole message must
    loop. It blocks with no timeout, no non-blocking mode and no poll, so a
    `recv` on a peer that never writes waits until the process is stopped.
  - **`close` is asymmetric with the rest of the namespace.** Closing an id that
    was never issued is a no-op, while `send`, `recv` and `accept` on an unknown
    id are all `SocketError`. Deliberate for cleanup paths, but it means a typo
    in a `close` is silent.
  - **The WebSocket helpers do not perform the handshake.** `sendWsFrame` and
    `recvWsFrame` layer RFC 6455 framing over an already-established connection;
    the HTTP upgrade is the caller's job.
- The documented `recvWsFrame` null fallback — the same value for "no message
  yet" and "the read failed" — is restated where a reader will actually meet it.

### `spec/regex.md`, and five constructs that silently mean something else

- Regex was the last namespace with real surface and no specification, which by
  `compatibility.md`'s own rule left all of it unstable. The document is written
  from measurement: the five signatures, the return shapes, the dialect, the
  replacement syntax, the error contract and the two matcher ceilings.
- **The section that matters most is what the engine does not support.** Five
  constructs other engines have are not rejected — they parse as ordinary
  characters and match something the author did not intend:

  | Written | What it actually is | Measured |
  | --- | --- | --- |
  | `(?=b)` | a group of the literals `?`, `=`, `b` | does not match `"ab"`; matches `"a?=b"` |
  | `(?!b)` | a group of the literals `?`, `!`, `b` | does not match `"ac"`; matches `"a?!b"` |
  | `(?<n>a)` | a group of literal characters | matches the text `"?<n>a"` |
  | `\1` | the literal character `1` | `r"(a)\1"` matches `"a1"`, not `"aa"` |
  | `\b` | the literal character `b` | `r"a\bc"` matches `"abc"` |

  One rule explains all five: `\` before an unrecognised character yields that
  character, and `(?` followed by anything but `:` is an ordinary capturing
  group. Such a pattern compiles, runs, and quietly answers the wrong question.
- Two shapes worth knowing before reaching for the API: `replace` takes the
  replacement **last**, after the text, and `match` returns `null` when nothing
  matches rather than an empty array.
- Writing the document turned up an omission in its own first draft. `replace`
  supports `$&`, `$1`–`$9` and `$$` in the replacement — already covered by
  tests, missed by the draft. The group form reads exactly **one** digit, so
  `$10` is group 1 followed by a literal `0`, and a `$n` naming a group the
  pattern does not have stays literal.
- `unit_regex.sz` goes from 17 cases to 28, eleven of them pinning the hazards so
  a change to the pattern parser cannot alter them in either direction unnoticed.

### `Regex` was the last namespace that got arity wrong

- A wrong argument count on any `Regex.*` method reported the generic
  `RuntimeError` / `SZ4000`, so a caller matching on `kind` could not tell
  "called wrongly" from any other runtime failure. Six spec documents —
  `arrays.md`, `dicts.md`, `random.md`, `sets.md`, `strings.md`, `tasks.md` —
  state the `TypeError` / `SZ4002` rule normatively. Regex was the only holdout.
- Exactly the shape `errors.md` already records for `Set`'s unknown member, and
  fixed the same way. Safe rather than breaking: there is no `spec/regex.md`, so
  the behaviour was unspecified and therefore unstable by `compatibility.md`'s
  own rule, and no official package calls `Regex.` at all.
- Pinned by three cases in `unit_regex.sz`: arity is `SZ4002`, an unknown member
  stays `SZ4001`, and a malformed pattern stays the generic kind.

### What "unmigrated producers" actually meant

- The audit's Evaluator row said `mod.rs`, `stmt.rs`, `classes.rs` and
  `methods_dec.rs` "still hold unmigrated producers". Measured: **zero** sites
  originate an unstructured failure — that was settled earlier in this cycle.
  What remains is coarseness, not absence.
- `SZ4000` is carried by three kinds, and the registry named two.
  `errors.md` now lists all three and which failures reach each: `Overflow` for
  integer and exponent overflow; `RangeError` for a bad `sort` order string, a
  negative padding target, an invalid `Random` range and an out-of-range
  `DateTime`; `RuntimeError` for a malformed regex pattern, `parseInt` /
  `parseDecimal` given non-numeric text, and `break`/`continue` outside a loop
  reached through a class body.
- The `RuntimeError` row stays undifferentiated deliberately, and says so: there
  is no `SyntaxError` kind for a malformed pattern, and `parseInt("abc")` is a
  value problem rather than a type one — the argument is a string, which is the
  type the function wants. Narrowing either would change a documented code.

### A worker speaks on the parent's streams, and nothing said so

- `tasks.md` describes `poll` as how a worker's result reaches the caller. It is
  not the only channel. Measured: a worker's `out` lands on the **parent's
  stdout** and its diagnostics on the **parent's stderr**, interleaved with the
  parent's own output and ordered only by which thread writes first.
- Two consequences neither the API nor the document showed: a program cannot
  tell its own output from a worker's — there is no prefix, tag or separation —
  and handling a worker failure through `poll` does not suppress the diagnostic
  the worker already printed, so a caller treating failure as ordinary control
  flow still gets unrequested text on stderr.
- Documented in `tasks.md` and pinned by two runner cases on both platforms.

### `tasks.md` re-probed: seventeen claims, all held

- `run` returning an int id; `message` delivering the argument; a reply becoming
  observable only after a successful exit; a later failure winning over an
  earlier reply, with the structured `SZ4003` surviving inside the `ERROR:`
  string; a worker that does not parse reported as a failed task; the last of
  several replies winning; a terminal record staying repeat-pollable; `SZ4001`
  for an unknown id on both `poll` and `isDone`; `SZ4002` for wrong arity and
  wrong argument type; and both permissive fallbacks outside a worker, including
  the warning `reply` prints.
- Path resolution uses the host process working directory, as documented — and
  this audit walked straight into what that costs. The same program run one
  directory up silently executed a **different file of the same name** and
  reported success. It looked like a defect in `poll` until the leftover file
  turned up; `tasks.md` now says the failure it produces is a worker that runs
  the wrong script and reports success.
- Not re-probed, and said so in the document: lockdown inheritance and the
  resource ceilings, which need an embedder or thirty-three concurrent workers.

### The generator ceiling: measured, put to a decision, not added

- `fn*` accumulates into an unbounded vector. Measured on the current build:
  100,000 yielded integers cost 20 MB, 400,000 cost 71 MB, 1,600,000 cost
  254 MB — linear, about 160 bytes per value, against about 107 bytes for the
  same count pushed onto a plain array. Ten million values is roughly 1.6 GB.
- No official package uses `fn*` at all, and the largest generator in the
  conformance suite yields 100 values. A ceiling would therefore have been
  invisible to every program that exists while still being able to break one
  that does not, so it was put to the maintainer as a decision rather than
  taken. Documenting the absence was chosen.
- `limits.md` records it under "What is not limited", with the measurements, the
  reasoning, and an explicit note that behaviour at exhaustion was **not**
  measured — the project's one precedent is the `sz-lsp` allocation fixed
  earlier in this cycle, which aborted with an allocator message and no
  diagnostic.

### Eight runtime ceilings re-probed at the boundary

- Not near the boundary — at it. `"a" * 10000000` succeeds and `* 10000001` is
  fatal `SZ6002`; `Crypto.randomBytes(1048576)` succeeds while `1048577` and `0`
  are refused with the plain-string throw the document already describes as the
  one unstructured limit. Padded string result, tensor element count,
  `Memory.alloc`, call depth, value nesting depth and the `sz-lsp` message body
  matched too. Eight for eight.
- The GPU buffer, WebSocket frame, four weights-file and Task rows were **not**
  re-probed — reaching them needs a GPU, a live socket, a `.szw` file or
  thirty-three concurrently spinning workers. `limits.md` now says which rows
  were measured here and which rest on their own tests, rather than letting the
  section read as uniformly verified.
- One row named a limit without naming the operation it bounds. Reaching for
  `"x".repeat(n)` gets `Unknown string method 'repeat'`, because repetition is
  the `*` operator — the mistake this audit made first. The row names it now.

### Control flow is frozen; two hazards came out of freezing it

- `control-flow.md` froze only `for-in` and listed everything else — `if`,
  `while`, C-style `for`, `do-while`, `switch`, `match`, generators,
  `try/catch/finally` and labelled flow — as unaudited. All are now probed
  against the binary and written down. Almost everything held; two things did
  not have a written contract at all, and both are recorded as hazards rather
  than smoothed over.
- **`match` is not exhaustive-checked.** A subject that matches no arm evaluates
  to `null`, with no diagnostic and exit 0 — indistinguishable from an arm that
  legitimately returned null. Making it an error would break any code relying on
  that null, so it is documented and left as a decision.
- **`fn*` is not lazy.** Calling a generator runs the body to completion and
  returns an ordinary array of everything it yielded. The syntax is borrowed
  from languages where it means the opposite, so the consequence is worth
  stating plainly: an unbounded generator never returns. Measured — no result
  after 20 seconds — and nothing bounds what it accumulates, because the
  collector is an unbounded vector and `limits.md` has no entry for it.
- Frozen alongside them, all measured: `finally` runs on every exit from the
  `try`, including `break`, `continue` and `return`, and before the function
  actually returns; a `throw` from a `finally` **replaces** the failure in
  flight; a `return` in a `finally` **overrides** the try's; `catch (e)` binds
  the thrown value itself for a user `throw` and an `Error` object carrying
  `code`/`kind`/`message` for a runtime error, and never runs at all for a fatal
  failure; `switch` has no fallthrough, evaluates its subject once, and a
  `default` written first does not pre-empt a matching case below it; `match` is
  an expression whose arms are tried in order, whose subject is evaluated once,
  and whose pattern bindings do not leak out of their arm.
- Pinned by `tests/unit_control_flow_contract.sz`, 11 cases covering what the
  running suite did not. Switch fallthrough, for instance, had only ever been
  checked in `tests/_bug_hunt10.sz` — a file the runner does not glob, so it has
  never run.
- The two new spec checkers caught two of the new document's own examples on
  their first run, which is what they are for; both were made self-contained
  rather than marked.

### Spec examples must run, not only parse

- The parse checker added one stage earlier recorded its own blind spot: a block
  that parses and then fails at runtime, which is exactly the `values.md` defect
  that prompted it. `spec_serez_examples_run` closes it by driving the binary
  over every `serez` block in `spec/`.
- Of 41 blocks, **23 ran unaided**. The other 18 needed a marker, and none of
  them was wrong — eight are deliberate runtime-error demonstrations (the
  checker rejecting a widening, `dec` refusing to mix with `decimal`, a
  constructor chain stopping, the dict-literal shapes that fail) and ten are
  fragments, mostly multi-file module examples. They had simply never been
  distinguished from the blocks a reader is meant to be able to paste.
- Two markers join the existing `parse-error-example`:
  `runtime-error-example: why` when failing is the thing being documented, and
  `fragment: why` when a block continues another or needs files beside it.
- Confirmed to fail on a deliberate break.

### Four more spec documents re-probed; all four held

- `arrays.md` — 56 claims, including every one of the eleven diagnostic codes in
  its error table and the element-type rules (`filter` and `slice` preserve the
  declared type, `map` and `flat` produce untyped arrays), the arity-before-
  arguments ordering, `reduce`'s initial-value-first argument order, the
  callback arity rule, `slice` and `flat` clamping, and the deliberate
  inconsistency where `remove` on an empty array returns null.
- `strings.md` — 48 claims, including scalar-based length, `substring` not
  swapping reversed indexes where JavaScript would, and the historical padding
  order `"x".padStart(4, "ab") == "babx"`.
- `sets.md` — 14 claims, including that a compound element is always admitted
  and `has` on a compound is always false.
- `control-flow.md` — 11 claims, including the single evaluation of the
  iterable, the snapshot that stops a growing source adding visits, and
  per-iteration closure capture.
- Zero drift across all four. Worth recording rather than passing over: it
  separates the documents that were written from measurement and stayed true
  from the ones this cycle had to correct.

### A dict literal is not an expression, and nothing said so

- `parse_dict_literal` is reachable from exactly one place in the grammar: a
  `let` carrying `<K, V>`. Everywhere else `({"a", 1})` parses as a parenthesised
  entry literal and fails at runtime with `TypeError` / `SZ4002` and the message
  "Entry literal {k,v} is only valid as an argument to a dict method" — which
  names the entry rather than the literal, so it reads as a puzzle.
- Measured, all of these fail: an unannotated `let`, an argument, a return value,
  an array element, a field initialiser, and reassigning a binding that *was*
  annotated. An array has no such restriction, which is why the asymmetry
  surprises. The way around it is an annotated `let` assigned across.
- `dicts.md` — the document a reader goes to for dicts — never mentioned it.
  `syntax.md` did, but said "an annotated binding, or an argument to a dict
  method", which reads as allowing the reassignment that in fact fails. Both now
  state the rule and agree.

### `values.md` carried an example that could not run

- The receiver-writeback chain example built a dict literal inside a field
  initialiser and called `kv.c[0][0]["k"].push(x)` with `kv` never declared.
  Replaced with a runnable version, verified to print what the surrounding prose
  claims. Its stale "see `operators.md` when it exists" is also gone —
  `operators.md` exists.
- Everything else in the document was re-probed against the binary and held: the
  copy rules for arrays, dicts, sets and class instances; copying across a call
  and out of a container; writeback for all thirteen named mutators and for a
  method that assigns to `this`; a getter not being a place, so nothing is
  written; closure capture in both directions; the five equality rows; the eight
  truthiness rows; and `&&`/`||` returning an operand. The 500-level copy ceiling
  is reachable and fatal as documented — but only between 501 and 511 levels,
  because the 512-level parse ceiling stops a deeper literal first.

### `spec/` examples are now checked, like the README's

- Nothing verified the 46 `serez` blocks across 25 normative documents. Five did
  not parse — all in `syntax.md`, all deliberately invalid, and only two of them
  said so. The three unmarked ones now carry a `parse-error-example` marker,
  which is the point of the marker: being invalid is a claim, and a claim should
  be written down.
- `spec_serez_examples_parse` mirrors the README guard. It parses rather than
  runs, and it would not have caught the `values.md` defect above, which parses
  fine and fails at runtime — that half is still uncovered and said so in the
  test's own comment. Confirmed to fail on a deliberate break.

### `variables.md` was pointing at work that had already been done

- Its opening and its coverage boundary both listed type annotations, shadowing,
  closure capture, `const` write attempts and assignment as pending a dedicated
  audit. `types.md`, `scopes.md` and `values.md` have since covered all of them.
- Every destructuring rule it freezes was re-probed and held, including the four
  shapes the grammar refuses (nested array and object patterns, object rest, and
  a non-array or unsupported right-hand side declaring none of the bindings).

### The compatibility promise about diagnostic codes is now enforced

- `compatibility.md` says: "Diagnostic **codes** and **kinds** are stable. A
  failure keeps its code and kind, or the change is treated as breaking."
  Nothing enforced it. The 63 `err_*` and 85 `sec_*` conformance programs assert
  only that the exit was non-zero and a `❌` appeared somewhere — never which
  code. A failure could move from `SZ4003` to `SZ4009`, or lose its code
  entirely, and all 148 would still pass. Eight of the twenty documented codes
  were named by no Rust test at all.
- `tests/diagnostic_codes.rs` drives the built binary and pins the code every
  documented construct produces, so what is frozen is the code a user sees. A
  second test reads the registry table out of `errors.md` and compares it with
  the suite in both directions: a code the registry lists and nothing pins is a
  failure, and so is a code pinned here that the registry no longer lists.
- Every fixture was derived by running it, not by trusting the registry. Three
  first attempts were wrong — `"x".repeat(...)` is not a method and reported
  `SZ4001`, a traversal read reported `SZ4005` rather than a security code, and
  the import fixture had been deleted by its own setup.
- Confirmed not decorative: changing `IndexOutOfBounds` to `SZ4093` turns the
  suite red and names the change.
- Two codes are recorded as unemitted with their reasons, a list that may shrink
  and must not grow: `SZ5001`, because a missing module is thrown as the
  historical `ModuleNotFound:` string, and `SZ4999`, which is unreachable by
  design.

### Two security tests were passing for the wrong reason

- `sec_path_traversal.sz` said traversal via relative segments "must be
  rejected" and named escaping "the sandbox". `sec_path_traversal_abs.sz` said
  the same for absolute paths. There is no such guard: measured directly,
  `File.read("../outside.txt")` reads a file one directory up and exits 0, and
  so does an absolute path to it.
- Both passed on all three CI platforms only because the paths they named
  (`../../etc/passwd`, `/etc/shadow`) do not exist there. A security test that
  passes for an unrelated reason is worse than no test, because the suite then
  reads as proof.
- Neither was deleted or weakened. They now state what they actually prove — a
  read of a path that is not there fails with a structured `SZ4005` — and the
  real behaviour is pinned by `tests/filesystem_reach.rs`, so confining `File`
  later is a deliberate change with a test to update.
- `security.md` and `compatibility.md`'s gap list now say plainly that there is
  no filesystem confinement.

### A relative `File` path and a relative `import` are measured from different places

- Found while probing the above. `import "./lib"` resolves against the **file**,
  so it works wherever `sz` was invoked from. `File.read("./data.txt")` resolves
  against the **process working directory**, so the same program reads its own
  data file when run from its folder and fails with `SZ4005` when run from one
  level up — and that failure does not look like a working-directory problem
  when you hit it.
- Documented in `security.md`, pinned by
  `a_relative_file_path_is_measured_from_the_caller_but_an_import_is_not`.
  Changing the base is a breaking change and is left as a decision, not taken.

### `compatibility.md` was understating what a release promises

- Its "Known gaps" said the spec did not cover syntax, the type system,
  operators, scopes or modules. All five documents have since been written, so
  by the document's own rule ("if it is not written down, it is unstable") the
  entry was telling readers that frozen behaviour was not frozen. Corrected.

### `--help` stated an exit-code contract the binary does not honour

- It listed "type error" among the things that exit `1`. It is not one: the
  checker is advisory, so `sz file.sz` reports `SZ3000` and runs the program
  anyway, and `sz --check` reports it and still exits `0`. Confirmed against the
  binary both ways.
- `--help` is the surface people actually read, and the test guarding that
  section asserted only that the words "EXIT CODES" appeared — it could not have
  caught this. Corrected, and now pinned as *behaviour* by
  `a_type_diagnostic_does_not_change_the_exit_code`, so a future change to the
  exit code has to be made deliberately.

### `sz info` invented a package that does not exist

- `sz info <name nobody published>` printed the name, three zeroes and an empty
  version list, and exited `0` — a complete, plausible record for something that
  is not there. `sz update` answers the same input with "not found" and exit `1`.
- The cause is not local: the registry does not `404` for an unknown package.
  Verified with a direct request — it answers `200` with
  `{"total":0,"weekly":0,"monthly":0,"versions":[]}`, byte for byte what a real
  package with no downloads would look like. The client rendered that faithfully.
- The distinguishing signal is the version list: publishing is what creates a
  version, and a yanked version still appears with `yanked: 1`, so an empty list
  means nothing was ever published. `sz info` now reports not found and exits `1`,
  agreeing with `sz update`.
- Two more defects were in the same function and went with it:
  - the three download counters came from `extract_json_number(...).unwrap_or(0)`,
    so a redirect, an error page or a changed schema printed as "0 downloads"
    rather than as an error;
  - the yank marker was `search.contains("\"yanked\":1")` against the whole
    remaining body, so the first unpublished version marked every version after
    it as unpublished too. Invisible only because nothing in the registry is
    yanked today — and `sz unpublish` exists.
- All four hand-rolled searches are replaced by one `serde_json` parser behind a
  pure `parse_package_stats`, pinned by four unit tests that need no network.
- Verified against the live registry afterwards: `sz info serez-ui` still shows
  58 downloads and 54 versions in order.

### A module replacing one of your own names says so now

- `modules.md` recorded it as a hazard and it was exactly that. Verified against
  the binary: a file that defines `greet()`, prints `MINE`, imports a module
  exporting its own `greet()`, then prints `FROM THE MODULE` — with nothing on
  stderr in between. Your definition was gone and nothing said so.
- The import now names every binding it replaced, and says which one wins and
  how to stop it.
- **Nothing about the rule changed.** The flat namespace, last-writer-wins and
  the exit code are untouched; packages may depend on all three. Only the
  silence changed.
- Measured before keeping, with a probe verified to fire on a real collision:
  **zero** collisions across the 483-test core suite and all eight official
  packages. A warning that fires on healthy code is worse than none.
- Both halves are pinned by a new Import Tests section on both runners — the
  collision is reported, and a clean import stays quiet. The second needed a
  `run_file_test` / `Run-File-Test` helper that can assert stderr does **not**
  say something; the runners could only assert containment.
- The first attempt at the probe fired on nothing, twice: once because it was
  installed on the URL import path rather than the relative one, and once
  because `declaration_name` returns `None` for an `export`ed declaration. Worth
  recording, because a probe that measures zero and a probe that is simply dead
  look identical in the output.

### The one outcome that printed nothing now says something

- `ProgramOutcome::UnstructuredError` was the single outcome the boundary
  rendered no output for: a non-zero exit with nothing on stderr. The comment
  justifying it read "legacy error producers already emitted their own
  diagnostic."
- Measured: no such producer exists. Of the sixteen `eprintln!` calls in the
  evaluator, every one is `rt_err_kind`'s structured printer, the boundary
  reporter, stack-frame rendering, or a warning — except two, `OS.spawn` and
  `Socket.recvWsFrame`, and those return a **value** (`-1`, `null`) rather than
  the sentinel. Both are deliberate compatibility fallbacks already recorded in
  `errors.md` with their blast radius (`serez-http` and `serez-strike` branch on
  the `null`; `serez-strike` calls `OS.spawn`), so they are unchanged — and
  neither can reach that arm. The silence had nothing behind it.
- It now prints `SZ4999` and says the defect is in Serez-Code, not in the program
  that was run. Nothing reachable produces the outcome today, so this is a net
  for a future regression; a regression that says something is worth more than
  one that says nothing.
- Two tests: one on the text, one that reads the reporter's arm — a message the
  reporter never reaches is the same silence with extra steps. Confirmed to fail
  when the arm is silenced.

### The `EvalResult::Error` inventory, re-measured

- `errors.md` said thirty-four sites remain and every one propagates rather than
  originates. Verified independently by classifying all 138 constructions of the
  sentinel outside tests: **102** re-emit it immediately after `rt_err_kind` or
  `fatal_err_kind` recorded the payload, **34** propagate a sub-evaluation that
  had already failed, and the two the scan could not place are a `match` pattern
  split across two lines and `rt_err_kind`'s own tail. **Zero originate.** The
  document's number was right; it is now backed by a measurement rather than a
  sample.

### `--watch` on a mistyped filename panicked

- `sz --watch nosuchfile.sz` ran the file, printed the correct diagnostic, and
  then panicked:

      thread '<unnamed>' panicked at src/main.rs:266
      Failed to watch file: ... Input watch path is neither a file nor a directory.

  A raw Rust panic with a backtrace note, immediately after the actual problem
  had already been reported properly. Both `.expect(...)` calls in the watch
  setup now report and exit 1, which is what `cli.md` says every failure does.
- Covered on both runners; the case fails against the previous binary.

### Running a `.szx` destroyed a file next to it

- The translated output went to `szx.with_extension("szx.sz")` — a fixed name
  derived from the source. So `sz app.szx` overwrote an existing `app.szx.sz`
  and then deleted it, with no prompt and no warning. Measured against the built
  binary, with a file holding user text: gone, on the success path and on the
  failure path alike.
- Two concurrent runs of the same source raced for that one path, and `--watch`
  re-runs on every save.
- The import path in the same 153-line file had always generated a unique name
  from the pid and a counter, for exactly this reason. The run path now does the
  same, and still writes beside the source, which it must: the translation
  carries the app's relative imports, so a temp directory would break every
  `import "comp/Chip"`. Only the generated path is ever removed.
- Pinned by five unit tests on the naming function, which need no serez-ui
  installed to run. Verified end to end against the built binary with serez-ui
  present, and against the canary: 8/8, serez-ui 36/36.

### `spec/cli.md` says what running a `.szx` actually does

- Its one table row read "Lex, parse, type-check, run", the same as for a `.sz`.
  What actually happens is materially different and visible to the user: it
  requires serez-ui installed, it spawns a second `sz` process to run serez-ui's
  own translator, and it writes a file beside the source.
- Diagnostics from a `.szx` carry the **translated** file's name, line numbers
  and source snippet — a line the user never wrote, in a file that is removed
  before they can open it. A note after the diagnostic now says so. Keeping the
  translated file for inspection is recorded as known debt rather than invented
  here; it needs a flag, and that is a product decision.

### The REPL ran what the parser rejected, and died on a pasted character

- **A line with parse errors was evaluated anyway.** `out "x"; let y = ;` printed
  `x` at the prompt, while the identical line in a file printed
  `Aborted: fix the parse errors above before running.` and executed nothing.
  `run_source` states the rule in a comment — "a program with parse errors must
  not half-run: statements after the broken one would execute against missing
  definitions" — and the REPL simply never called `parser.has_errors()`. It does
  now, and prints the same abort line.
- **A line that was not UTF-8 killed the session.** `read_line().unwrap()` turned
  `InvalidData` into `thread '<unnamed>' panicked at src/repl.rs:17` with a
  backtrace note, on an interactive surface, from one pasted Latin-1 character.
  The REPL reads raw bytes now and reports the line it could not decode, in the
  same shape its siblings use (`ERROR reading file`, `ERROR reading stdin`), then
  carries on at the next prompt. Consuming the line before validating it also
  guarantees progress, so a bad line cannot be re-read forever. It was the only
  entry point that panicked here: a file, an imported module and `--eval -` all
  answer with a diagnostic and exit 1.
- **A closed terminal was a panic.** `flush().unwrap()`. Ending a session is not
  a failure to report.
- **Diagnostics had no source line and no caret**, because `set_source` was never
  called on the parser or the evaluator. They now match the file path's shape.
- `spec/cli.md` gains the REPL contract this had been missing: each line is a
  complete program against a persistent evaluator, a line that does not parse
  does not run, a runtime error abandons only that line, and the REPL is **not**
  under lockdown — it grants like `sz file.sz`, unlike `--eval`. One deliberate
  difference is recorded rather than smoothed over: the REPL does not run the
  type checker, because each line is parsed independently and a per-line checker
  would report functions declared on earlier lines as unknown. The checker is
  advisory everywhere, so nothing enforced is lost.
- `DEVELOPMENT.md` said the REPL "reuses the same pipeline per line". It did not,
  in four separate respects. Corrected.

### The runners can now assert that something did *not* happen

- Both defects above are absences — a line that must not run, a session that must
  not die — and all five existing REPL cases assert containment, so neither was
  visible to them. A new `run_repl_test` / `Run-Repl-Test` helper takes a
  forbid-in-stdout assertion and reads stdin from a fixture file, since one
  fixture is deliberately not valid UTF-8 and cannot survive being carried as a
  shell or PowerShell string.
- REPL coverage goes from 5 cases to 11 on **both** runners. All three new
  behavioural cases were confirmed to fail against the previous REPL.
- Three of the new cases cover the REPL's permission boundary, which had no
  coverage at all: it denies by default, honours an inline `use permissions`,
  carries the grant across lines, and opens nothing it was not asked for.

### A third of the suite never ran on one platform

- `run_tests.sh` reported 306 passed / 168 failed where `run_tests.ps1` reported
  474 / 0. Nothing in the language differed: every unit test on the bash side
  failed to start. A unit file is run from a temp file named `~unit_temp_$$.sz`,
  and MSYS2 refuses to rewrite a POSIX path into a Windows path when the last
  component begins with `~`, so `sz` was handed a literal
  `/e/01 Proyectos/.../tests/~unit_temp_792.sz` and answered
  `ERROR reading file ... (os error 3)`. Paths are now converted with `cygpath`
  explicitly; `cygpath` is absent on Linux and macOS, where the path is already
  native. Both runners: 474 / 0.
- Two things kept it invisible, and both are fixed:
  - The bash runner printed the first three stderr lines for a failing E2E test
    but not for a failing unit test, so the entire report for 168 failures was
    `process exited with code 1` and a blank line. The PowerShell runner had
    always shown it. It now does on both.
  - The runner's own integrity guard — the check whose only job is to catch a
    suite that passes for the wrong reason — accepted *any* non-zero exit with
    no `Results:` line. A file that cannot be opened satisfies that, so the
    guard reported PASS throughout. Both runners now also require the fixture's
    own `SZ4004` diagnostic, so the guard can only pass by actually running the
    program. Verified from both sides: with the conversion reverted it fails and
    names the cause.

### The build is warning-free

- `namespaces_gui.rs` initialized `tw` to `0i32` outside a `match` when the only
  read is in the same arm as the only write. It was the sole `rustc` warning in
  the project, and it was recorded in the audit as a known state rather than
  fixed. Removed — a build with zero warnings is the only baseline on which a
  new warning is visible. GUI tests unchanged: 13/13.

### A malformed `Content-Length` no longer kills the language server

- `sz-lsp` allocated a JSON-RPC message body at exactly the length the client
  advertised, with no ceiling. Reproduced against the built binary:

      $ printf 'Content-Length: 9999999999999

' | sz-lsp
      memory allocation of 9999999999999 bytes failed

  An allocator abort, not a diagnostic — no code, no context, and the editor's
  language server simply vanishes.
- It was the only input-sized allocation in the project without a bound.
  `File.read` (256 MiB), package archives (64 MiB), a task worker's source
  (16 MiB) and WebSocket frames (16 MiB) all have one, and all are in
  `limits.md`. This one is now 64 MiB and in `limits.md` too — generous on
  purpose, since the largest legitimate message carries a whole document in a
  `didOpen`, and a file near that size is far past the AST ceiling already.
- An over-limit header prints why and exits, rather than exiting silently: a
  silent exit is indistinguishable from the editor closing the pipe, which is
  the wrong thing to go looking for.
- Verified both ways: a 1 MiB `didOpen` is still accepted, and ordinary
  `initialize` traffic is unaffected. Three framing tests cover the ceiling, EOF,
  a non-numeric length, a negative length and a truncated body.
- The rest of the LSP audit found nothing: exactly one panic site in production
  code, and it is guarded by the `!new_name.is_empty()` check beside it.

### `errors.md` said unstructured failures remained; they do not

- The document stated that "class `super`/dispatch paths and other native
  subsystems still contain examples under active audit" of producers that had
  not migrated to structured errors. They were migrated, and the claim outlived
  the work.
- Retired by measurement rather than on faith. `ProgramOutcome::UnstructuredError`
  is the one outcome the boundary prints **nothing** for — a non-zero exit with
  no diagnostic, the least actionable thing the runtime can do — so it is worth
  knowing whether anything still reaches it. Thirty-one language constructs
  (construction, method and accessor bodies, `super` in both directions,
  operator overloads, native callbacks, generators, `match`, destructuring,
  nested writes, pipes, ternaries, spreads, a failing default argument) plus the
  894 hostile native calls from the previous sweep: **zero unstructured
  outcomes**.
- Thirty-four `return EvalResult::Error` sites remain in the evaluator, and the
  document now says what they are: every one *propagates* a failure already
  recorded structured further in, running cleanup and passing the sentinel
  outward. None originates an unstructured one. The variant stays in
  `ProgramOutcome` because an embedder must still be able to receive it and
  removing it would be a public API change.
- Pinned by `no_reachable_construct_produces_an_unstructured_outcome`.

### The whole native surface swept; five more zero-argument gaps closed

- Every method of twelve native namespaces called with no arguments, a string,
  `i64::MIN`, an array, `null` and five arguments — **894 calls**. Result: zero
  panics and **zero unstructured errors** anywhere in the native surface, which
  is worth recording because `errors.md` still lists unstructured producers as
  active debt. In the namespaces reachable this way, there are none left.
- The silent-default pattern (`_arg(..).unwrap_or(..)`) is now **gone from every
  namespace** — the only two matches left in the tree are inside the doc comment
  explaining why it was removed.
- Five more zero-argument methods accepted extras and ignored them, the same gap
  as Gui's thirty-one: `OS.pid`, `OS.platform`, `OS.tick`, `Media.playingCount`,
  `Media.stopAll`. They now raise `TypeError` / `SZ4002` through a shared
  `reject_arguments` helper. Swept first: no official package passes an argument
  to any of them.
- `DateTime.from(1, 2, 3, 4, 5)` was flagged by the sweep and is **not** a
  defect — three through seven arguments is its documented arity. The probe was
  blunt, not the implementation, and the pinning test says so explicitly so
  nobody "fixes" it later.
- Pinned by `zero_argument_native_methods_reject_arguments`, which also asserts
  the zero-argument forms still work and that `DateTime.from` keeps its
  five-argument shape.

### A wrong-typed Gui argument is an error, not a silent default

- Eleven `Gui` call sites wrote `gui_int_arg(..).unwrap_or(0)` or
  `gui_str_arg(..).unwrap_or_default()`. Omission was already handled separately
  above each of them, so those defaults fired only when the caller **did** pass
  an argument and it was the wrong type:

      Gui.setTitle(5)                          // silently cleared the title
      Gui.clipboardSet(42)                     // silently wrote an empty string
      Gui.setCursor(7)                         // silently selected no cursor
      Gui.renderTree(root, "800", 600, 400)    // silently rendered at width 0

  All four now raise `TypeError` / `SZ4002` naming the method and the parameter.
- This is the same shape as the Array defects fixed at the start of this cycle —
  `slice("x")` becoming index 0 — and it is the failure mode this whole audit
  exists to remove: a wrong type quietly becoming a plausible-looking value, so
  the program does something the author never asked for and nothing says so.
- `drawText`'s `style`/`letterSpacing` and the file dialog's three strings keep
  their defaults when **omitted**; only a supplied-and-wrong argument errors. The
  two cases used to be indistinguishable.
- `renderTree` was the risky one — serez-ui calls it constantly — so the change
  was measured rather than assumed: conformance 474/474, ecosystem 8/8 with
  serez-ui 36/36, serez-strike 113/113, serez-cobol 23/23.
- Pinned by `a_supplied_gui_argument_of_the_wrong_type_is_rejected_not_defaulted`,
  which also asserts correct types still pass and omitted optionals still
  default.

### Gui readers that take no arguments now say so

- `namespaces_gui.rs` is the largest subsystem — 6,036 lines, 120 exposed
  methods — and the thinnest-covered: about 27 conformance tests across the
  whole surface. Auditing it systematically found nothing that crashes: every
  one of the 120 methods was called with no arguments and no window, and then
  the 33 reachable without a window were called with a string, an array, `null`,
  `i64::MIN`, a dict and eight arguments — 198 combinations, zero panics, zero
  unstructured errors.
- What it did find: **31 of those readers accepted anything and ignored it**.
  `Gui.mouseDown("left")`, `Gui.size([])` and `Gui.isOpen(1, 2, 3)` all returned
  normally, while every other namespace in the language rejects a wrong argument
  count with `TypeError` / `SZ4002` — `strings.md`, `random.md` and
  `datetime.md` all state it normatively. It is the same silent-acceptance shape
  as the Array defects fixed at the start of this cycle, and a plausible mistake
  to make here in particular: `mouseRightDown` exists as a *separate* method, so
  writing `mouseDown(RIGHT)` is a natural guess and used to be met with silence.
- One guard before the dispatch rather than thirty-one edits in a 6,000-line
  file, with the list and its reasoning in `GUI_ZERO_ARG_METHODS`. `close` and
  `font` are excluded because they really do read arguments — that distinction
  came from checking each arm for `dot_call.arguments`, not from the names.
- **Breaking in principle, empty in practice.** Swept before enforcing: no
  official package passes an argument to any of the 31. Verified after:
  ecosystem 8/8 with serez-ui 36/36, serez-strike 113/113, serez-cobol 23/23.
- Pinned by `zero_argument_gui_methods_reject_arguments`, which also asserts the
  same calls still work with no arguments and that `close`/`font` are untouched.

### The panic sites are classified, and a test keeps them honest

- All 311 `unwrap`/`expect`/`panic!`/`unreachable!` sites classified rather than
  counted: 148 test-only, 39 lock poisoning (reachable only after another thread
  has already died), 37 behind the `llvm` feature and not in a default build, 28
  resolving an arena ref the evaluator itself just produced, 8 exhaustive-match
  `unreachable!`, and the remainder guarded a line or two above.
- Every candidate that looked genuinely reachable was checked against the
  binary. `Gui.nodeText`, `nodeTextPx` and `nodeRoundRectOutline` have no guard
  inside their own match arm but return `GuiError`/`SZ4000` with no window open,
  because the guard is upstream. `Binary.unpackInt64Le` has an explicit length
  check. `sz run` resolution unwraps inside `match matches.len() { 1 => }`.
- One latent hazard recorded rather than fixed: `broadcast_data` computes
  `(0..ndim - 1)`, which underflows when `ndim == 0`. An empty tensor shape is
  rejected by all four construction paths with a clean `RangeError`, so it is
  unreachable — but the function's safety rests on validation two modules away,
  which is worth knowing before someone adds a fifth construction path.
- **No reachable panic was found**, and that is only a reading. Reading code and
  concluding "this looks guarded" is the reasoning that let two invalid quality
  gates survive in this repository. So the audit leaves
  `hostile_arguments_to_native_methods_never_panic`: 55 pieces of source a user
  can type — i64 extremes into string indexes, empty and negative tensor shapes,
  truncated binary input, uncompilable regexes, malformed JSON, freeing a null
  pointer, reversed random bounds — each under `catch_unwind`, each required to
  produce a diagnostic rather than a dead process.
- It earned its place immediately by catching a bad fixture of mine: a bare
  `"{"` in Serez opens an interpolation, so `Json.parse("{")` is a *lexer* case,
  not a runtime one. The raw-string form is used instead, and the frontend stays
  covered by `malformed_input_never_panics`.

### The benchmark suite exists on Unix, and can record a baseline

- `run_benchmarks.sh` is new. The suite was Windows-only: `run_benchmarks.ps1`
  had no counterpart, so nobody on Linux or macOS could run it at all — the same
  platform gap the conformance runners had, where a missing half means nobody
  outside one OS can reproduce a number, let alone a regression.
- Both runners take `--json`/`-Json` and `--baseline`/`-Baseline`, emit the same
  `serez-benchmarks/1` document with the same field types, and read each other's
  output: a baseline recorded by the bash runner is compared correctly by the
  PowerShell one, verified in both directions including a deliberate regression
  and a deliberate improvement.
- **The reported statistic is the minimum of N runs, not the mean.** A process is
  only ever slowed down by its neighbours, never sped up, so the fastest run is
  the least-contaminated estimate of the work. Mean and max are recorded beside
  it because their spread says how much to trust the number.
- **A wall-clock budget is deliberately not wired into CI**, and TESTS.md says
  why with the measurement rather than an opinion: on an idle desktop with
  nothing else running, `00_startup` ranged 35 ms to 69 ms across two consecutive
  runs — a factor of two. Shared CI runners are worse. A threshold wide enough
  not to fire on that noise would not catch a real regression either. This
  repository has already paid for two gates that reported invalid results; a
  flaky third would teach people to re-run until green, which is the habit that
  let the first two survive.

### A class shadowed by an interface says so

- Classes and interfaces live in separate registries, so both declarations of a
  shared name are accepted — and `new Name(...)` resolves to the **interface**,
  regardless of which came last. The class becomes unreachable, and the failure
  arrives at the construction site as an argument-form `TypeError` with nothing
  pointing back at the declaration that shadowed it.
- It now warns at the second declaration, naming the consequence. A warning
  rather than a refusal: refusing would be breaking, and no official package has
  such a collision — the ecosystem canary reports zero warnings.
- Scoped to the one pair that actually breaks. `class`/`enum` and
  `interface`/`enum` were checked and coexist correctly, because `new Name(...)`
  and `Name.Variant` are different syntax reaching different registries. Both
  facts are pinned in `tests/runtime_outcome.rs`, along with the shadowing in
  both declaration orders, and written down in `spec/classes.md`.
- Redeclaring the *same* kind twice is unchanged: ordinary shadowing, later wins.

### A permission that does nothing now says so

- `src/permissions.rs` is new: the permission vocabulary in one place. The nine
  enforced names existed only as string literals at their `require_permission`
  call sites, and anything else a program declared went into a `HashSet` and was
  never looked at again. Three things were hiding in that silence.
- **A misspelling was accepted.** `use permissions { Termnal }` granted nothing,
  and the program then failed at its first `Terminal` call telling the author to
  declare a permission they believed they had declared — one character away in
  the same file. It now warns at the grant, and suggests the intended name when
  exactly one enforced name is within edit distance 2. A tie suggests nothing: a
  wrong guess in a security-adjacent message is worse than none.
- **A dotted name grants nothing and does not imply its prefix.** `OS.exec`
  parses — the parser's own comment advertises the form — and leaves `OS` denied.
  It warns, and names `OS` as what to declare instead. No official package writes
  one.
- **`File` gates nothing**, and it is the second-most-declared capability across
  the official packages: 23 `use permissions` blocks and four manifests.
  `File.read` succeeds with no permissions declared at all. This one deliberately
  **does not warn**: it is inert because the runtime does not gate file access,
  not because the author wrote anything wrong, and a warning on every run of
  correct-by-convention code is the noise that teaches people to ignore the typo
  warning two lines below. The fact lives in `spec/security.md` and in
  `classify`, where it stays testable, rather than on stderr.
- Warnings, never refusals — rejecting an unrecognised name would break any
  program that declares one today. Calibration confirmed against the ecosystem:
  **zero permission warnings** across all eight official packages, and 8/8 still
  green.
- `enforced_permissions_match_the_evaluator` reads the `require_permission` call
  sites out of `src/` and asserts they equal `permissions::ENFORCED`, so the list
  cannot drift into telling an author their correct declaration does nothing.

### The specification audit is complete

- `spec/tasks.md` and `spec/compiler.md` checked, both clean. tasks: a later
  failure wins over an earlier provisional `reply` so a caller cannot observe a
  premature success, the last of several replies wins, an unknown task id is
  `SZ4001` for both `poll` and `isDone`, the `ERROR: ` prefix survives, and the
  two documented permissive fallbacks outside a worker behave exactly as the
  document admits they do. compiler: its atomicity claim — never a partial
  program with an unsupported construct replaced by `null` — is precisely what
  `unwrap_err()` proves in the test named after it.
- **Every document making falsifiable behavioural claims has now been checked
  against the running binary.** Ten were clean; four needed correcting, and
  those four were also the ones making the most claims per line of test backing:
  `security.md`, `limits.md`, `classes.md` and the README's code.
- `cli.md` is pinned by the runner's 33 CLI/`--eval`/REPL/`--check` cases rather
  than by a separate pass. `compatibility.md` states process, not behaviour —
  there is nothing in it to probe, only something to follow, which the
  constructor type-check sweep in this cycle did.

### Four more contracts audited; `DateField` was described around, never stated

- `spec/strings.md` (27 claims), `spec/random.md` (18), `spec/control-flow.md`
  (11), `spec/variables.md` (12) and the string/operator half of
  `spec/lexical-grammar.md` all checked against the running binary. **All clean.**
  Among the things that held rather than merely being asserted: `"x".padStart(4,
  "ab")` is `"babx"`, `slice` clamps `i64::MIN` instead of indexing native
  memory, an unknown escape keeps its backslash so a Windows path survives,
  `Random.int` covers the full `i64` domain, `Random.shuffle` does not mutate its
  input, a `for-in` closure per iteration keeps its own value, and a failed
  destructuring pattern declares none of its bindings.
- **`spec/datetime.md` had two gaps**, both of them the facts a caller trips
  over. Every construction, range, leap-year, clamping and failure claim held,
  but the document never said what a `DateField` *is* to the program:

      let m = DateTime.from(2026, 1, 31).month();
      type_of(m)      // "int"                 — there is no "DateField" type name
      m.toString()    // "1"                   — a field reads as its value
      m.add(1)        // 2026-02-28T00:00:00   — and returns a DateTime, not a field

  `add`/`reduce`/`remove` returning a **`DateTime`** is the point of the type — a
  number that remembers the instant it came from — and it was the one thing the
  section describing them did not say. Both facts are now written down, with the
  January-31 clamp shown, and `spec/types.md`'s `type_of` table gained the two
  rows it was missing. Pinned by
  `a_datefield_reports_as_an_int_and_its_arithmetic_returns_a_datetime`.
- No behaviour changed.

### Constructor chaining reaches one level, not the whole hierarchy

- `spec/classes.md` said each constructor in a multi-level chain "must itself
  call `super(...)`, **or rely on the compatibility rule above when invoked
  through ordinary construction**". The second half is false past the first
  level. Implicit chaining happens only at the outermost `new`; a constructor
  reached *as a parent* — by an explicit `super()` or by the implicit call — gets
  no implicit call of its own. A middle class with no constructor at all stops
  the chain the same way.
- The failure mode is quiet: the grandparent's field initialization simply does
  not happen, and it surfaces as a `ReferenceError` wherever that field is first
  read, far from the constructor that should have set it. In a hierarchy deeper
  than two levels, write `super(...)` in every intermediate constructor.
- Pinned by `implicit_constructor_chaining_reaches_exactly_one_level`.
- Also recorded: a **declared** class field wins over a getter of the same name,
  and that is the only way the two can coexist, because the getter-only check
  fires on any write. The same check means a subclass getter named after a field
  the parent assigns breaks the parent's constructor — `new Child()` raises
  `TypeError` / `SZ4002` from the parent's own `this.v = …`.

### A deep stack trace is readable again

- Runaway recursion printed all 512 frames, three lines each — around fifteen
  hundred lines of stderr that buried the one-line error explaining them. The
  human rendering now shows the innermost ten and then `... N more frame(s) not
  shown`. A getter that recursed printed twenty thousand characters; it prints
  thirteen lines.
- A frame with no recorded position (`line 0`) used to index `saturating_sub(1)`
  back to line 1 and confidently underline the **first line of the file**. Those
  frames now print their name and no snippet, which is the honest output.
- Only the human rendering changed. `RuntimeError::stack` still carries every
  frame, so tooling reading the structured payload is unaffected, and both
  renderers share one function instead of two copies that had already drifted.

### Every README example is parse-checked, and five had drifted

- `readme_serez_examples_parse` extracts all 198 ```serez blocks from README.md
  and parses each one. The README is what people copy; nothing checked it.
- **Five examples did not parse.** A dict literal without an annotated binding;
  `fn any get()` and `let out = …`, both using reserved words as names;
  `while step < 1000 {` without parentheses; `let name: string = readLine(…)`,
  a scalar annotation on a binding, which the language has never accepted; and
  `public abstract decimal area();`, an abstract method declaration.
- The last one is the worst kind of documentation defect: the README's own
  **Known Gotchas** section already said "Abstract method *declarations* (no
  body) are not supported" and showed that exact line as the ⚠️ example. The
  feature section a thousand lines earlier presented it as working. All five are
  fixed, and the abstract-class example now uses the throwing default the gotcha
  recommends — verified by running it, output included.
- Two blocks were not Serez source and are re-fenced: a `Terminal.readEvent()`
  shape illustration (`text`) and a JSX snippet (`jsx`, it is `.szx`).
- Six blocks are invalid **on purpose** and now say so with a first-line
  `// parse-error-example: <why>` comment. The marker is explicit rather than
  inferred from the ⚠️/❌ the prose already uses, because one genuinely broken
  block carried a ❌ for an unrelated reason and would have been skipped for the
  wrong one. It also tells a reader which blocks are deliberately broken, which
  previously took reading the surrounding prose.

### The reserved words are written down

- `spec/lexical-grammar.md` said keyword recognition was "exact and
  case-sensitive" and then never listed the keywords. All 50 are now in a table.
  A keyword can never be an identifier — `let out = 1;` and `fn any get() { }`
  are parse errors, not shadowing — and three of them read like ordinary names
  and get reached for by accident: **`out`**, **`get`** and **`set`**. README.md
  used all three.
- `reserved_words_match_the_lexer` reads the table back out of the document and
  compares it with `lookup_ident`, including the count in the prose, so adding a
  keyword without documenting it fails a test. A documented list that drifts is
  worse than none, because it reads as authoritative.
- `spec/syntax.md` cross-references the rule where people meet it.

### The limits document is audited: five ceilings were missing, one figure was stale

- Every number in `spec/limits.md` checked against its constant in the source.
  All of them matched — AST and call depth 512, value nesting 500, string
  repetition and padding 10,000,000, tensor elements 10,000,000, regex 1,000,000
  steps and 8,000 backtracking levels, `Memory.alloc` and GPU buffers and
  `File.read` at 256 MiB, WebSocket frames at 16 MiB, and the five Task
  ceilings.
- **Five enforced ceilings were not documented at all:** the `.szw` weights file
  (256 MiB, 100,000 tensors, rank 64, 10,000,000 total values) and
  `Crypto.randomBytes` (1 MiB per request, and a count below 1 rejected). All
  five are now in the table, with a note on where the weights-file checks happen
  — file size from metadata before any bytes are read, the rest while parsing
  the header, so a malformed file cannot make the loader allocate first.
- `Crypto.randomBytes` is also the one ceiling that reports a **plain catchable
  string** rather than a structured error: no `kind`, no `code`, classifiable
  only by matching English. Recorded in `errors.md` beside `ModuleNotFound`,
  which has the same shape for the opposite reason — that one is pinned and
  deliberate, this one is a gap to close.
- The claim "across the 999 `.sz`/`.szx` files in the official ecosystem, the
  deepest nesting is 19 levels" had gone stale: there are 1,255 files now. A
  number that drifts every release is worse than no number, so it is replaced by
  the durable version, verified by parse-checking every `.sz` file across the
  official packages and this repository: **the only two that reach the AST
  ceiling are `err_parse_depth_chain.sz` and `err_parse_depth_nesting.sz`**, the
  fixtures written to test it. Re-checking it is a `--check` sweep for `SZ2001`.
- `the_string_and_crypto_ceilings_are_the_ones_the_document_names` pins the
  string repetition and padding ceilings as fatal `SZ6002`, and both the shape
  and the exact value of the `Crypto.randomBytes` cap. Those four were covered
  only by `err_*`/`sec_*` fixtures, which assert "non-zero exit and a ❌ line" —
  enough to catch a crash, not a limit changing its code or its catchability.
- No behaviour changed.

### The execution contract is audited, and one claim in it was wrong

- Every concrete claim in `spec/security.md` was checked against the running
  binary rather than re-read: the nine enforced namespaces match the source
  exactly, self-granting works outside lockdown, the `unsafe` gate is fatal
  `SZ6003`, the protected-path heuristic on `OS.exec`/`OS.spawn` is fatal
  `SZ6004`, and `fetch` really does reach the network under lockdown.
- **One claim was wrong, in the direction that matters.** The document said all
  lockdown refusals surface as *catchable* `PermissionError` / `SZ6001`. Three
  of the four do — `File`, `import` and Autodiff weight I/O. The fourth,
  `use permissions`, is **fatal** `SecurityError` / `SZ6004` and `try/catch`
  cannot consume it, which is what `errors.md` said all along. A security
  document that under-states a gate's strength is worse than one that says
  nothing about it.
- The split is now stated with its reason — the first three refuse an *action*,
  the fourth refuses to hand out *capability* — and pinned by
  `lockdown_denials_split_into_catchable_and_fatal` in
  `tests/runtime_outcome.rs`, so unifying it in either direction has to be a
  deliberate semantic change.
- No behaviour changed. The implementation was right; the document was not.

### The grammar is written down, including what does not parse

- `spec/syntax.md` is new: the statement and expression grammar, checked form by
  form against the running binary. `lexical-grammar.md` already covered tokens;
  this covers what they can be arranged into.
- Half of it is the forms that read as obviously valid and are **not**, so they
  stop being discovered one at a time: brace-less `if`/`while`/`for` bodies,
  `for (item in …)` without the `let`, a typed lambda parameter, a scalar or
  nullable annotation on a `let`, a class name as an array element type, a
  JSON-style `{key: value}` literal, trailing commas in array literals, call
  arguments, parameter lists and dict literals (but not in `match` arms, `enum`
  variants or array destructuring), and nested block comments.
- `the_documented_grammar_is_what_the_parser_accepts` in
  `tests/frontend_robustness.rs` pins both columns — sixteen rejected forms and
  thirty-two accepted ones — so a parser change cannot quietly move a case from
  one to the other.
- Also recorded: `return`/`break` outside their context report as
  `❌ FLASH SCOPE ERROR` with no `SZ` code, one of the unstructured diagnostics
  `errors.md` still lists.

### The type contract is written down, and two holes in it are closed

- `spec/types.md` is new: the seven type keywords, where an annotation may
  appear, what a declared type accepts at a call, `type_of`/`is`, and exactly
  how far the static checker reaches. Pinned by `tests/unit_types.sz` (13 cases)
  and four new tests in `tests/runtime_outcome.rs`.

- **BREAKING (see the sweep below): a constructor now enforces its declared
  parameter types.** It already checked arity; it never checked types. So
  `new Point("x")` against `public Point(int x)` bound a string into an `int`
  field and the program failed only wherever that field was later used as a
  number — or never, if it was just read back out. It is now the same catchable
  `TypeError` / `SZ4002` a function parameter raises, naming the constructor.

- **Fixed: an enum value did not satisfy its own enum's type.** `type_matches`
  had no arm for an enum variant, so `fn rank(Priority p)` rejected
  `Priority.Low` and said so in the one way nobody can act on — *"expected
  'Priority' but received 'Priority'"* — and `Priority.Low is Priority` was
  `false`. Enum-typed parameters were unusable. This is a previously-invalid
  program becoming valid; nothing that worked before changes.

  The two are related: the constructor hole is what hid the enum bug. Adding the
  constructor check turned `38_real_programs.sz` red with the contradictory
  message above, which is how the enum arm was found.

- **The sweep required by `spec/compatibility.md` for a breaking change in a
  minor release**, run after both fixes:

  | Suite | Result |
  | --- | --- |
  | Conformance runner | 474 passed, 0 failed (473 at sweep time, plus `unit_types.sz`) |
  | Ecosystem canary (8 packages) | 8/8, serez-ui 36/36 |
  | serez-cobol | 23/23 |
  | serez-strike | 113/113 |
  | `cargo test --all-targets` | 251 passed, 0 failed |

  No official package passes a mistyped argument to a typed constructor. The one
  affected file in the repository was `38_real_programs.sz`, and it was affected
  by the enum bug rather than by a genuine type violation.

- Recorded in `spec/types.md` and left unchanged, because each needs a decision
  rather than a fix: numeric types do not widen at a parameter although
  arithmetic mixes them (`half(1)` fails for `fn decimal half(decimal d)`); a
  declared class or interface name matches **exactly** and never a subclass, so
  a hierarchy can only cross a call as `any`; an unknown type name is accepted
  and then matches nothing; `[T]` on a parameter accepts any array; and declared
  class-field types are defaults, not constraints.

### Scope and name resolution are written down, including one surprise

- `spec/scopes.md` is new: block scoping and shadowing, globals being writable
  from a function, no hoisting, closures capturing a cell (with a `for` counter
  captured fresh per iteration), and the Flash Scope watermark model.
- It records a property nothing documented: **a free variable inside a function
  resolves dynamically.** A call pushes its frame onto the caller's scope stack
  and lookup walks every frame, so a function sees the locals of whoever called
  it — two callers give two different answers. It is not an implicit global: a
  name bound nowhere still fails with `SZ4001`.
- Nothing is changed. Making resolution lexical means a per-call scope stack and
  explicit captured environments, which is a change to the core evaluation
  model and could alter any ecosystem program that relies on it. It is pinned by
  a regression test so it cannot change by accident in either direction, and
  recorded in `MATURITY_AUDIT.md` as an open decision.

### A mutation through a nested receiver no longer disappears

- `a[0].push(x)` did nothing. So did any chain deeper than one level:
  `d["k"][0].push(x)`, `h.f[0].push(x)`, `this.cache[l][h]["k"].push(x)`. The
  read planted a copy, the method mutated the copy, and the copy was dropped —
  no error, no effect, no way to notice except by checking afterwards.
- Receiver writeback covered exactly two shapes: `obj.field.mutate(x)` and
  `dict["key"].mutate(x)` where the dict was a bare identifier. Everything else
  fell through.
- **This was a live bug in an official package.** `serez-agentai`'s
  `KVCache.store()` is `this.cache[layer][head]["k"].push(k_vec)`. The cache
  never accumulated and `seqLen()` always returned 0. Its own test suite passed,
  because nothing covered that path.
- Writeback now works through any assignable path — a variable, a field read,
  an index, or any chain of them — reusing the same `resolve_lvalue_path` /
  `store_path` machinery that nested assignment and `this`-mutating class
  methods already used.
- The two existing special cases are untouched and still take their optimized
  paths; the general one is consulted only when neither applies, so the hot
  `d["k"].push(x)` loop keeps its exact cost.
- Unchanged: reading out of a container still copies. `let x = a[0]; x.push(2)`
  mutates `x` alone. Writeback is about calling a mutator *on a place*, not
  about making containers shared.
- `spec/values.md` is new: assignment and argument passing copy for every type
  including class and interface instances, the writeback rule and what counts
  as a place, closures capturing the variable rather than its value, the
  equality table (`[1,2] == [1,2]` is false) and the truthiness table.

### `sz --help` exists, and the CLI contract is written down

- `sz --help` was not implemented. It fell through to `Unknown flag '--help'`
  on stderr with exit 1, so the only way to discover the command surface was to
  read `main.rs`. It now prints usage on **stdout** with exit **0** — asking for
  help is not an error, and a tool that answers on stderr cannot be piped into
  a pager or checked by a script. `-h` and `sz help` do the same.
- The two usage errors that used to be dead ends — an unknown flag and a
  missing file argument — now point at `sz --help`.
- `spec/cli.md` is new: exit codes, which stream carries what, every flag and
  subcommand, the lockdown rules for `--eval`, and an explicit list of what is
  *not* specified (no `--json` output, no finer exit codes, no locale promise).
  Everything in it was checked against the running binary.
- Seven CLI tests in both runners cover the help output, its aliases, and the
  hints on the usage errors.

### Native namespace validation is structured, and `kind` finally means one thing

- GPU, Binary, Memory, Tensor, Crypto and Gui validated their arguments by
  printing to stderr and returning an untyped sentinel. They now report
  catchable `TypeError` (`SZ4002`), with `RangeError` (`SZ4000`) for a negative
  Memory size or offset and `IndexOutOfBounds` (`SZ4003`) for a Tensor index
  outside its dimension.
- **Breaking, deliberate:** twelve native namespaces reported an unknown member
  as `TypeError` while arrays, strings, dicts, sets, dec, enums, Random,
  DateTime and Task all reported `ReferenceError`. `e.kind` therefore could not
  tell "there is no such member" from "you called it wrongly". Math, File,
  JSON, Media, Memory, Binary, GPU, Regex, Socket, OS, Env, Time, Terminal and
  System now all report `ReferenceError` (`SZ4001`).

  Ecosystem sweep: no official package inspects these kinds, and none uses
  Media at all. One core test, `tests/unit_media.sz`, pinned the old
  `TypeError` and was updated. Per `spec/compatibility.md` this qualifies as a
  breaking change that may ship in a minor release.
- Three failures are still delivered as sentinel values and are now documented
  in `spec/errors.md` rather than changed: `Socket.recvWsFrame` returns `null`
  (which `serez-http` and `serez-strike` both branch on), `OS.spawn` returns
  `-1`, and `Task.reply` outside a worker warns and continues. Each needs a
  compatibility decision first.

### Exact-decimal (`dec`) methods are structured, and the runner stops colliding

- Every `dec` method and the static `Dec` namespace validated by printing to
  stderr and returning an untyped sentinel: arity, scale range, rounding mode,
  argument types, `toInt`/`toDecimal` overflow and unknown members. All are now
  catchable and coded — `TypeError` (`SZ4002`) for arity and argument types,
  `RangeError` (`SZ4000`) for a scale outside 0..=28, an unknown rounding mode
  and an unparseable literal, `Overflow` for the two conversions, and
  `ReferenceError` (`SZ4001`) for an unknown member.
- The three `dec` argument helpers collapsed every non-value outcome into a bare
  error, so a user `throw` inside `d.round(boom())` was lost. They now propagate
  it and name the call that rejected a wrong type.
- With this, every built-in type's methods — string, array, dict, set and dec —
  report through the structured channel.
- `run_tests.ps1` wrote its scratch file to a fixed `tests/~unit_temp.sz`. A
  lingering handle from an earlier run aborted a whole suite mid-way with
  "used by another process" and no TOTAL line. It is now per-process, matching
  what `run_tests.sh` already did, and both names are gitignored so a crashed
  run cannot leave one behind in the repository.

### Callback, class-dispatch and object-patch diagnostics are structured

- Callback arity and shape (`Callback expected N argument(s)`, `Callback is not
  a function`), a missing operator overload, `super.m()` with no `this` in
  scope, an object patch on an undeclared variable or a non-interface value,
  and an interface field patched with the wrong type were all stderr prints
  returning an untyped sentinel. They are now structured and catchable —
  `TypeError` (`SZ4002`) or `ReferenceError` (`SZ4001`).
- `break`/`continue` escaping a class method, an operator method or a callback
  is a recoverable generic `RuntimeError` (`SZ4000`) instead of a bare print.
- A patch rejected on type leaves the instance unchanged; that is now covered.
- `classes.rs` and the evaluator's dispatch machinery no longer print any
  diagnostic. The prints remaining in `mod.rs` are the boundary renderers that
  turn a structured payload into terminal output, which is their job.

### A value too deep to copy no longer corrupts itself and reports success

- `extract` bounds its own recursion at 500 levels. Past that it replaced the
  subtree with `null`, printed `❌ ERROR: Maximum nesting depth (500) exceeded`
  once per truncated site, and let the program **run to completion and exit 0**.
  A program that nested containers deeply — `v = [v]` in a loop — therefore got
  silently corrupted data, a flooded stderr and a success exit code.
- It is now a single fatal `ResourceError` (`SZ6002`) raised at the next
  statement boundary, with a non-zero exit. Fatal rather than catchable: the
  value has already lost a subtree, so a handler that carried on would be
  working with corrupted data.
- The limit was also undocumented. `spec/limits.md` now lists it beside the AST
  and call-depth ceilings and explains how it differs from both.
- `extract` takes `&self` and is called from 84 sites, so it records the
  truncation through a flag rather than raising the error itself. That keeps the
  fix to two guards and one checkpoint instead of a signature change across the
  evaluator.

### Statement and import diagnostics are structured, and reported once

- A missing import was reported **twice**. The module paths printed the failure
  themselves and also threw it, so one bad import produced both
  `❌ ERROR: ModuleNotFound: ...` and `❌ UNCAUGHT EXCEPTION: ModuleNotFound: ...`.
  Only the program boundary reports it now.
- Module loading now distinguishes two failures. A module that cannot be
  *found* keeps its historical shape — a catchable user exception whose message
  begins `ModuleNotFound:`, which `tests/unit_sec_import.sz` pins and which was
  therefore deliberately left alone. A module that was found but cannot be
  *loaded* (does not parse, `.szx` the translator cannot handle, unreadable) is
  now `ImportError` (`SZ5002`), a new code in the previously near-empty `SZ5xxx`
  range.
- `yield` outside a generator, a non-pointer left side in `*ptr = val`, an
  unresolved `for-in` source and a missing pointer target are structured and
  catchable instead of stderr prints.
- `use permissions` under lockdown is now `SecurityError` (`SZ6004`) and remains
  **fatal**: it is a security gate, so `try/catch` must not be able to turn a
  denied self-grant into ordinary control flow. Verified with a catch attempt.

### Core expression diagnostics are structured and catchable

- The diagnostics an ordinary program actually hits — a wrong argument type, a
  wrong return type, an array or dict literal violating its own declared type,
  a dot call on a type with no methods, an unknown enum variant, a bad `&` or
  `*` — were the last common ones still printing to stderr and returning an
  untyped sentinel. None of them was catchable, and none carried a code.
- All fourteen now use the structured channel: `TypeError` (`SZ4002`) for the
  type failures, `ReferenceError` (`SZ4001`) for the ones that name something
  that does not exist.
- The call stack a typed-parameter failure printed as a side effect now travels
  in the error payload's `stack`, so an embedder reads frames instead of
  scraping stderr. Uncaught, the terminal output is unchanged: the same
  `called from …` lines with the source excerpt and caret.
- Five `resolve(...).unwrap()` calls on the user-input path were replaced with
  explicit handling while migrating them.

### `sz install` no longer fails on a declared minimum runtime

- `serez-ui` declares `"serez-code": ">= 9.17.0"` in `dependencies` to state the
  oldest runtime it supports. `sz install` treated that like any other package
  and pushed the value through the package-version identifier rules, which
  reject spaces and `>`. Running `sz install` in a `serez-ui` project therefore
  failed outright — the one official package that followed the documented
  advice was the one whose install was broken.
- `serez-code` is now a reserved key in `dependencies`: it declares the minimum
  runtime, is compared against the running version, and is never fetched. There
  is no package by that name — it is the interpreter reading the manifest.
- Accepted forms are `">= X.Y.Z"`, `"> X.Y.Z"`, `"= X.Y.Z"` and a bare `"X.Y.Z"`.
  Caret and tilde ranges are rejected rather than silently narrowed: package
  versions elsewhere are identifiers, not ranges, and accepting `^9.17.0` would
  imply a resolver that does not exist.
- `sz install serez-code` is refused with a message explaining what to write
  instead. An unsatisfied requirement names both versions and what to do.
- `spec/compatibility.md` is new: the versioning and deprecation policy that
  `spec/limits.md` and `spec/random.md` already referenced but that did not
  exist. It records the rule actually in force — including that `9.4.0` shipped
  a breaking change in a minor release after an ecosystem sweep found no
  affected code — rather than a stricter SemVer the history contradicts.

### Dict and Set stop accepting calls they cannot honour

- `new Set(5)` silently produced an empty set: a non-array initialiser was
  dropped on the floor. It is now a catchable `TypeError` (`SZ4002`).
- Every dict reader (`keys`, `values`, `toList`, `toArray`, `length`,
  `toString`) and the zero-argument Set methods (`size`, `toArray`, `clear`,
  `toString`) ignored extra arguments. They now reject them, before evaluating
  the arguments they are rejecting.
- All remaining dict diagnostics moved off stderr prints onto the structured
  channel, so `d.Add()` and a dict type mismatch are catchable and carry a code.
- An unknown `Set` member reported `TypeError` while arrays, strings, Random,
  DateTime and Task all reported `ReferenceError`. Set was the outlier; it now
  reports `ReferenceError` (`SZ4001`) like the rest, so `e.kind` can be used to
  tell "no such member" from "called wrongly".
- A dict or Set method whose receiver is not a dict or Set now says so instead
  of answering with empty data, which is how a dispatcher bug used to look.
- Unchanged: insertion order, missing-key reads returning null, `Remove` of an
  absent key staying a no-op, Set deduplication and compound-element behavior,
  `add` returning the receiver, `delete` returning a bool, and both value
  semantics. `spec/dicts.md` and `spec/sets.md` freeze the contracts.

### Array failures are structured, catchable and no longer silently ignored

- Array was the last large public surface still reporting failures by printing
  to stderr and returning an untyped sentinel. Those failures could not be
  caught: `try { a.push() } catch (e)` aborted the process instead of running
  the handler. All 21 methods now use the structured channel, so `e.code` and
  `e.kind` classify the failure without matching on wording.
- Three methods silently did something the program had not asked for.
  `slice("x")` used index 0, `flat("x")` used depth 1 and `sort("ascending")`
  sorted ascending. All three are now errors.
- Arity is validated before arguments are evaluated, so `a.pop(f())`,
  `a.reverse(f())` and `a.sort(cmp, f())` reject the call without running `f`.
- Callbacks are validated before iteration, so `[].find(1)` is a type error
  instead of a silent `null`.
- A comparator that fails leaves the receiver untouched; a failed sort never
  publishes a half-ordered array.
- `eval_str_arg` / `eval_int_arg` returned `Option`, collapsing a user `throw`
  and a nested runtime error into the same `None`. They now propagate the
  original outcome, which also fixes the same latent defect in
  `Crypto.randomBytes` and `Regex.*`; their type errors now name the call that
  rejected the argument.
- Unchanged: `remove` on an empty array still yields null, `reduce(initial,
  callback)` keeps its argument order, `sort` and `reverse` still return the
  receiver, negative `slice` indices still count from the end, negative `flat`
  depth still clamps to 0, and every valid result is byte-identical.
  `spec/arrays.md` freezes the contract.

### String padding can no longer grow without a bound

- `padStart(-1, "x")` and `padEnd(-1, "x")` cast the negative target to
  `usize` and entered effectively unbounded, quadratic growth. Padding now
  rejects negative targets with catchable `RangeError` (`SZ4000`), builds in
  linear time, uses fallible capacity reservation and applies a fatal
  10,000,000-character ceiling (`SZ6002`) before allocating the result.
- Valid Unicode and multi-character padding results remain compatible,
  including the historical `padStart` truncation direction.
- Every String method now validates arity and types structurally (`SZ4002`), an
  unknown member is `ReferenceError` (`SZ4001`), and nested runtime failures or
  user `throw` propagate unchanged. Invalid padding/types are no longer silently
  converted to zero, space or omitted bounds.
- README incorrectly claimed `.replace` changed every occurrence despite the
  implementation and regression suite defining first-only replacement. The
  guide now matches `.replace`/`.replaceAll` reality; `spec/strings.md` freezes
  the Unicode/index/error contract.

### Random no longer crashes on the complete integer domain

- `Random.int(i64::MIN, i64::MAX)` previously overflowed while calculating
  `max - min + 1`, panicking the debug host (exit 101) and reaching modulo by
  zero in wrapping builds. Width arithmetic now occurs outside `i64`, and wide
  ranges combine enough deterministic LCG output for every integer to be
  reachable.
- Seeded sequences for established ranges of width at most 2³¹ remain byte-for-
  byte compatible. Wider ranges use rejection sampling rather than the previous
  truncated 31-bit result.
- Every Random validation path is now structured and catchable: arity/types use
  `TypeError` (`SZ4002`), invalid domains use `RangeError` (`SZ4000`), and an
  unknown member uses `ReferenceError` (`SZ4001`). Nested errors and user
  `throw` propagate unchanged; arity failures do not evaluate arguments.
- The shared Tensor shape parser now reports structured type/range diagnostics,
  while product overflow and the element ceiling remain fatal resource errors.

### Task workers have an isolated, bounded runtime contract

- Task state moved from one process-global registry into an evaluator-owned
  runtime shared only with descendant workers. Independent embedders can no
  longer observe or poll each other's predictable task IDs; nested tasks remain
  compatible.
- Workers inherit parent lockdown and its granted permissions. Previously a
  restricted evaluator with Task permission spawned an unrestricted child that
  could read files and load manifest permissions.
- `Task.reply` is provisional until successful worker exit. Runtime failure or
  panic after a reply now wins instead of exposing a premature success. Worker
  runtime diagnostics retain `[code] kind: message` in the compatible `ERROR: `
  polling result.
- All Task API validation is structured and catchable (`SZ4001`/`SZ4002`), while
  nested errors/throws propagate unchanged. Registry poison no longer panics the
  host.
- Per-runtime limits are now explicit: 32 live workers, 256 retained records,
  1 MiB arguments/replies and 16 MiB worker source. Concurrency/message/thread
  creation ceilings are fatal `ResourceError` (`SZ6002`).

### Test runners no longer accept aborted unit files

- The Windows and Unix runners now require exit code 0, a `Results:` summary and
  no `[FAIL]` output for every framework-based unit file. Previously a parse or
  runtime abort before `summary()` produced no `[FAIL]` line and was reported as
  PASS—the exact defect recorded in the 9.16.0 changelog.
- Error/security fixtures now require both a non-zero exit and an error
  diagnostic. E2E output cannot be accepted or regenerated after a failed
  process.
- Every invocation runs a deliberately aborting fixture and proves the runner
  rejects it, so this quality-gate regression cannot silently return.
- The first honest run exposed 24 prior false positives: 16 legacy golden files
  were being treated as framework suites, three suites omitted `summary()`, and
  five fixtures did not parse. They are now correctly classified or repaired;
  the complete runner now passes 459/459.

### DateTime failures have a stable contract

- DateTime/DateField wrong arity and types now raise catchable `TypeError`
  (`SZ4002`); invalid calendars/epochs raise `RangeError` (`SZ4000`); field
  overflow raises `Overflow` (`SZ4000`); unknown members raise `ReferenceError`
  (`SZ4001`). The previous paths stopped with an unstructured sentinel.
- All members validate arity before evaluating argument expressions. This fixes
  zero-argument operations that previously accepted and silently skipped extra
  arguments and their side effects.
- Valid arguments preserve nested runtime errors and user `throw` unchanged.
  Rust, language and CLI regressions cover classification, capture and recovery.

### Cyclic inheritance can no longer hang the runtime

- Self and indirect cycles are rejected atomically at class declaration with
  catchable `TypeError` (`SZ4002`). Before this change, missing method/getter/
  setter lookup on a cycle looped indefinitely.
- All three ancestor lookup helpers are bounded defensively, including against a
  corrupt legacy/internal registry.
- Forward parent references remain compatible. Using the child before its parent
  is declared now raises catchable `ReferenceError` (`SZ4001`) instead of
  constructing a partial object or treating a missing parent as constructorless.
- Extending a sealed class now preserves the rejection but reports structured
  catchable `TypeError` (`SZ4002`).

### Property-write failures are structured

- Assignment to a getter-only property and field assignment on a non-instance
  now raise catchable `TypeError` (`SZ4002`) instead of an unstructured fatal
  sentinel. The write is still refused.
- Private accessors, malformed accessor arity and getter return mismatches share
  the same structured method-dispatch contract; accessor `throw`/runtime errors
  propagate unchanged.
- The specification now records, without changing, three broader compatibility
  debts: typed class fields are not enforced on later writes, interfaces can be
  extended after exact construction, and inherited private access is keyed to
  runtime rather than declaring class.

### Member-dispatch failures have stable diagnostics

- Missing instance members and missing static methods now raise catchable
  `ReferenceError` (`SZ4001`). A missing static method identifies the actual
  class/member instead of falling through to “Variable not found: Class”.
- Instance/static arity, external private-method calls/references and declared
  return mismatches now raise catchable `TypeError` (`SZ4002`). Resolution,
  argument evaluation, privacy enforcement and successful dispatch are
  unchanged.
- Rust, language and CLI regressions pin all seven paths and verify that caught
  failures do not corrupt later valid instance/static calls.

### `super` validation is structured and no longer ignores arguments

- Invalid `super()`/`super.method()` context, missing parents, impossible
  implicit chaining and constructor/method arity now raise catchable
  `TypeError` (`SZ4002`). A missing method in the parent chain raises catchable
  `ReferenceError` (`SZ4001`).
- `super(args...)` against a parent with no constructor no longer succeeds while
  discarding the arguments. Empty `super()` remains the compatible no-op; a
  non-empty call is `SZ4002`.
- The README now matches the implemented implicit-super contract instead of
  claiming every child constructor must call it explicitly. The conservative
  branch scan and the compatible manual-initialization exception remain visible
  rather than being changed silently.
- Rust, language and CLI regressions cover all nine error paths and verify that
  caught failures do not corrupt a later valid parent-method dispatch.

### Construction validation now has stable errors

- An unknown `new` target now raises catchable `ReferenceError` (`SZ4001`).
- Invalid interface construction, abstract-class instantiation, class
  field-form construction, constructor arity and arguments passed to a class
  without a constructor now raise catchable `TypeError` (`SZ4002`). These nine
  paths previously stopped with an unstructured, non-catchable sentinel.
- Human messages retain their identifying text, while Rust and Serez
  regressions pin `code`, `kind`, catchability and evaluator reuse. Successful
  class/interface construction and the eight official package suites remain
  compatibility gates.

### Default argument failures no longer become `null`

- User `throw` and runtime failures from a default expression now propagate
  unchanged through ordinary functions, native callbacks, constructors,
  `super()` constructors, `super.method()` and instance methods. Previously all
  six paths silently bound `null` and continued execution.
- Default evaluation now uses one cleanup-aware internal result contract, with
  regressions for the structured error payload and every call path.
- The parser now enforces the already documented ordering rule: a required
  parameter cannot follow a default parameter (`SZ2000`). A final `...rest`
  parameter remains valid after defaults; a non-final rest now also reports a
  syntax error instead of disappearing without a diagnostic. The official
  ecosystem has no signatures that depend on either invalid form.

### Resource and security failures are structured

- Call-depth checks now cover functions, methods, `super`, native callbacks and
  operator overloads through one fatal `ResourceError` (`SZ6002`) path. The
  ceiling is 512 frames: the former nominal 1000-frame contract allowed the
  Windows debug CLI to overflow its native stack around 800 callback frames
  before it could report an error.
- Tensor shapes and GPU matrix dimensions use checked multiplication before
  allocation. Tensors retain the 10,000,000-element ceiling. GPU buffers now
  enforce the documented 256 MiB **byte** ceiling (33,554,432 `f64` values) on
  creation, upload and matmul output; the old comparison admitted about 2 GiB.
- `Memory.alloc` above 256 MiB and `File.read`/`read_asBinary` above 256 MiB now
  report fatal structured `SZ6002`. `Memory.alloc(0)` remains a catchable
  invalid-argument `SZ4002`.
- Protected targets in `OS.exec` and `OS.spawn` now report fatal
  `SecurityError` (`SZ6004`). The existing substring guard remains explicitly a
  defense-in-depth heuristic, not a sandbox or canonical path policy.
- Array spread now matches call-argument spread: a non-array operand reports a
  catchable `TypeError` (`SZ4002`) instead of an unstructured fatal sentinel;
  user `throw` from the operand still propagates unchanged.
- Invalid `for-in` iterables, non-array rows in destructured `for-in`, and
  invalid array/object declaration destructuring now report catchable
  `TypeError` (`SZ4002`). Loop scope cleanup and nested user `throw` propagation
  are pinned by regressions.
- Rust runtime-outcome regressions pin each fatal payload and verify that
  `try/catch` cannot swallow it.

## [9.17.0] — 2026-08-16

### A subclass now chains to its parent's constructor on its own

```
class App:Window {
    public render() { ... }      // no constructor — normal coming from React
}
new App().mount()                // ❌ 'App' has no field or method named 'effects'
```

A subclass that never called `super()` got an instance with **none of the parent's
fields**, and the failure surfaced far away, naming an internal field of a class
the author never wrote. Java, C# and JavaScript all chain implicitly; now so does
this. Same for a constructor that simply forgets the call — the chain runs
**before** its body, so the subclass can still overwrite what it inherits.

The chain only happens when the parent constructor takes **no required
arguments**. When it does need them:

- the subclass **has** a constructor → nothing happens, silently, exactly as
  before. Initialising the parent's fields by hand instead of calling `super()`
  is a style the language allowed and there is code doing it (`tests/30_integral_e2e`
  has `Perro:Animal` doing precisely that). Turning it into an error broke three
  suites, so it stays legal.
- the subclass has **no** constructor → nothing can initialise the object, so it
  reports it, naming both classes and how many arguments are missing.

Whether the body already calls `super(...)` is a static walk of the constructor,
cached per class (`super_cache`), so it is paid once per class, not per `new`.
The walk is conservative: a `super(` anywhere — including inside an `if` — counts
as explicit and suppresses the implicit call.

`tests/unit_implicit_super.sz` (9 cases). Suite: 433, 0 failures.

## [9.16.0] — 2026-08-15

Four gaps that all showed up writing UI with serez-ui, fixed together because
three of them are the same thing seen from different angles: a value read out of
a container is a **copy**, and the language only knew how to write that copy back
in a handful of hardcoded shapes.

### A method of your own on a nested receiver kept its mutations

```
lista.push(new Celda())
lista[0].correr()      // mutated a copy
out lista[0].veces     // 0 — nothing happened
```

Reading `lista[0]`, `o.campo` or `this.celdas[i]` plants a copy, and until now
only the built-in mutators (`push`, `add`, `Add`, `clear`…, a fixed list of
names in `expr.rs`) were written back — and only one level deep. A method you
wrote yourself mutated the copy and dropped it.

This is what broke `useEffect` in serez-ui: `Window.runEffects()` calls
`this.effects[i].run()`, so `ran`, `prevDeps` and `cleanup` never persisted —
`deps []` re-ran on **every** update and the cleanup was never stored, so
`unmount()` cleaned nothing. **No library change was needed**; the two apps that
had the broken behaviour pinned in `apps40_test.sz` now assert the real thing.

- It was never just "an array element": `b.campo.metodo()` lost the mutation the
  same way. Any receiver that is not a plain variable.
- New `src/evaluator/lvalue.rs`: `resolve_lvalue_path` walks the receiver into a
  root variable plus a chain of `.field` / `[key]` hops, and `store_path` writes
  through it with a single `get_mut` on the root slot — no rebuilding of the
  intermediate containers.
- The writeback costs a copy back into the container, so it is gated twice, in
  this order: the receiver has to be a nested path (a syntactic check, free),
  and the method has to be able to write to `this` (a static walk of its body,
  cached per class+method). A read-only method pays nothing.
- The static analysis is deliberately coarse — any call rooted at `this` counts
  as a write. Refining it would need a whitelist of every read-only built-in,
  and being wrong there loses a mutation, which is the bug being closed.

### `a.b.c = x` and `a[i][j] = x` are writable

The first was a **parse error** (`Unexpected token '='`), the second a runtime
one (`Cannot assign to an index of a temporary value`). The workaround was to
rebuild and reassign the whole intermediate value.

```
o.inn.v = 9                     // was: parse error
this.filas[i][j] = 1            // was: InvalidAssignTarget
d["a"]["nueva"] = 5             // inserts, like the direct path does
t.o.inn.v += 8                  // compound forms too
```

Both now resolve the same writable path as the writeback above. New AST node
`NestedFieldAssign` rather than generalising `FieldAssignStatement`, whose
`object` is a bare name: `obj.campo = v` is the massive case and has no reason
to pay for path resolution.

- A setter halfway down the path still runs — the assignment goes through the
  same checks as the direct path (setter, getter-only property, element type,
  bounds), only the destination changed.
- Writing into a real **temporary** (`get()[i] = x`) is still a loud
  `InvalidAssignTarget`: there is nowhere to write back to.
- The AOT/LLVM backend (behind the `llvm` feature, unused) does not lower the
  new node — its HIR only has single-hop lvalues. Noted alongside the `&&`/`||`
  divergence from 9.14.

### A closure created in the constructor captured another `this`

```
class W { public W() { this.n = 0; this.f = () => { this.n = this.n + 1 } } }
let w = new W(); let h = w.f; h()
out w.n     // 0
```

The same closure written in a normal method worked. Registering effects or
callbacks in the constructor — the natural thing coming from React — was silently
mute.

Two copies stood between the closure and the finished object:

1. `eval_new_class` ended with `extract` + `plant`, returning a **different slot**
   than the one the constructor body used. Now it returns the live `this` slot
   (read from the binding, since a closure capture may have promoted it to the
   global arena). That also removes a deep copy of the instance on every `new`.
2. `let x = new C(...)` then copied it again. `Statement::Let` copies so a
   variable never aliases its source (`let x = arr[0]`), but a `new` produces an
   object nobody else can be holding, so the copy protected nothing and broke the
   identity. That single case now binds directly.

Value semantics are unchanged everywhere else, and that is the boundary: a
closure keeps a **cell** to the object it was created in, so if that object is
later copied into an array, a field or a return value, the copy is a different
object and the closure still points at the original. Construct-then-use, including
passing the variable to a function, now behaves.

### `!` follows the one truthiness rule

`&&`, `||`, the ternary and `match` guards moved to a single falsy rule in 9.14
(`false` · `null` · `0` · `0.0` · `""` · **empty** array/dict/set) but `!` was
left behind, so the idiom was split down the middle: `items && <Fila/>` compiled
and `!items` died with `Prefix '!' only applies to booleans`. You had to write
`items.length() == 0`.

`!` now negates `is_truthy` and always yields a boolean. With booleans the result
is identical to before, so this only turns former errors into values. A class
that defines `op_not` still wins — an explicit overload beats the general rule.

- `tests/err_bang_nonbool.sz` was removed: the condition it pinned is no longer
  an error. Its coverage moved to `unit_logical_operators.sz`, which also
  documents the rule from both sides.
- Still inconsistent, and left alone on purpose: `0m` (exact decimal) is truthy,
  while `0` and `0.0` are falsy. `is_truthy` has no `Dec` arm. Changing it would
  move `&&`/`||`/ternary/`match` too, so it is a call to make, not a slip to fix
  in passing.

### Tests

432, 0 failures. New: `unit_nested_receiver_writeback` (12),
`unit_nested_assignment` (14). `unit_logical_operators` grew to 19.
`unit_exceptions_advanced` now asserts that `m[i][j] = x` writes through, and
that a temporary and an out-of-bounds inner index still throw.

Also found and **not** fixed: the test runner reports `[PASS]` for a unit test
whose file fails to parse (PASS is "no `[FAIL]` line in stdout", and a parse
error produces neither).

## [9.15.0] — 2026-08-10

### A module with `export` erased the classes it imported itself

Composing a component out of components that live in **separate files** was
impossible, and the symptom pointed nowhere:

```
let c = new Card()   // fine
c.render()           // Unknown class or interface 'Badge'
```

`Card.szx` imports `Badge` and uses it inside `render()`. Construction worked;
the call died. The only workaround was to import every transitive dependency
*again* from the top file — the opposite of what an isolated component is for.

The cause is the visibility barrier in `import` (`stmt.rs`). When a module that
uses `export` finished loading, everything registered during its load that was
not in **its** export list got dropped:

```rust
self.class_registry.retain(|k, _| before.contains(k) || exports.contains(k));
```

"Everything registered during its load" includes what the module's *own* imports
brought in. `Card.szx` imports `Badge` (registered), exports only `Card` — so on
the way out, `Badge` was erased. It survived long enough for `new Card()` to
resolve because that only needs `Card`; `Badge` was looked up later, from inside
`render()`, when it was already gone.

Now the only names eligible for removal are the ones the module **declares** at
its own top level, via the existing `declaration_name`; whatever a nested import
registered stays. The barrier still does its job: what a module defines and does
not export remains hidden from its importer.

- Not specific to `.szx` — plain `.sz` modules had it identically.
- The resolver already handled `.szx`: it tries `<base>/<path>.sz`, then `.szx`,
  then `index.sz` / `index.szx`.
- Verified both ways (all `.sz` and all `.szx`, `Panel → Card → Badge`, one file
  each) and across a three-hop chain. Suite: 431, 0 failures.

## [9.14.0] — 2026-08-10

### `&&` and `||` return an operand, not a boolean

They used to demand a boolean on **both** sides and reject anything else with
`'&&' operator requires boolean operands`. Now they behave the way this operator
behaves in every language that has it:

```
a && b   // a if a is falsy, otherwise b
a || b   // a if a is truthy, otherwise b
```

With booleans on both sides the result is **identical to before** — `false && x`
is still `false`, `true && b` is still `b` — which is what makes this safe for
existing code, and the right-hand side is still not evaluated when the left one
already decides. What it opens up is the one-line conditional:

```
let name = input || "anonymous"        // fallback
let row  = items && buildRow(items)    // only when there is something
```

The second line is what prompted the change: building a UI, `items && <Row/>` is
how you say "render this when there is something to render", and it was a hard
error.

### One rule for what counts as falsy

`false`, `null`, `0`, `0.0`, `""` and an **empty** array, dict or set. Everything
else is truthy. The same rule now backs `&&` / `||`, the ternary, `match` guards
and the `filter` / `some` / `every` callbacks — previously only `false` and
`null` were falsy there, so `0` and `""` passed as true.

Empty collections being falsy is a **deliberate departure from JavaScript**,
where `[]` is truthy: there, `items && render(items)` fires on an empty list, and
the workaround (`items.length && …`) is itself the well-known bug that prints a
stray `0`. Here the plain form already means "if there is anything".

- Suite: 431 (new `unit_logical_operators`, 14 assertions), 0 failures.
- Not touched: the AOT/LLVM path (`compiler/llvm_emit.rs`) still lowers `&&`/`||`
  as bitwise ops on `i1`, so it only agrees with the interpreter for boolean
  operands. Worth reconciling before that path is used for anything real.

## [9.13.0] — 2026-08-09

### Dispatch stops copying the receiver

`.` and `[ ]` cloned the **whole receiver** before operating on it. That is not
what value semantics asks for: `ANALISIS_MEMORIA_RENDIMIENTO.md` picked P1
(Embedding), where reading copies **the element** — the code copied **the
container**. The copy was then mutated and written back over the same slot, so
it protected nobody: it was work that existed only to be thrown away one line
later.

Every method now runs against the arena slot, the way `d[k] = v` and `Set` have
since 7.3.0 and 9.12.0. Measured on release builds, best of three:

| | before | after |
|---|---|---|
| `a[i]` read, 10 000 elements | 7138 ms | 30 ms |
| `length()` × 10 000 | 2233 ms | 32 ms |
| `obj.method()`, instance holding 1000 elements | 956 ms | 297 ms |
| `obj.field`, same instance | 275 ms | 82 ms |
| `d.Add(…)` × 4000 | 8411 ms | 6 ms |
| `d.Remove(…)` × 200 over 3000 entries | 505 ms | 66 ms |

- **`a[i]`** on an array or a string reads the element out of the slot. The
  index expression was already evaluated before the clone, so evaluation order
  is untouched.
- **Instance dispatch** no longer copies every field. The copy was only ever
  read — mutation always went through `obj_ref` — so a new `field_value` helper
  pulls out the one field a call actually needs.
- **`length()`** on arrays and dicts reads the slot. It is O(1) and was paying
  an O(N) clone, in the single most common call in an indexed `for` header.
- **Dict methods** move to a new `methods_dict.rs` (`eval_dict_method_slot`),
  built on the `methods_set.rs` template; the dict arm of the generic match is
  gone. `Add` also stops scanning linearly for a duplicate key: it probes the
  slot-resident hash index, which the old whole-slot rewrite used to discard on
  every insert. Building a dict with `Add` was quadratic twice over.
  The indexed probe is validated against the legacy comparator and falls back to
  the scan when they disagree, so `Decimal` and compound keys keep the exact
  behavior they had; a miss cannot disagree, which is what makes each insert
  O(1).

Array methods that are inherently O(N) (`indexOf`, `join`, `map`…) deliberately
stay on the snapshot path: the clone does not change their complexity, and
moving them would change when their arguments observe the receiver.

### Mutations through a field or a dict slot no longer vanish

Reading `instance.field` or `d["k"]` plants a **copy**, so a mutation on the
result is dropped unless it is written back. Three shapes had no writeback at
all and failed silently — no error, just a change that never happened:

| Shape | Was | Now |
|---|---|---|
| `h.tags.add(x)` — a `Set` in an instance field | mutation dropped | persists |
| `d["k"].add(x)` — a `Set` in a dict slot | mutation dropped | persists |
| `outer["in"].Add({k, v})` — a dict in a dict slot | mutation dropped | persists |

The first was a missing entry in the list that triggers the field writeback: it
named `Add` (the dict method) and the aliases `remove`/`clear`, but never `add`
or `delete`. A user-defined `add`/`remove` method reached across a field hop
(`o.c.add(5)`) was losing its mutation for the same reason.

The other two were the writeback machinery living in the wrong place. It is now
two helpers — `dict_slot_ctx` (recognizes the `dict["k"].mutator()` shape) and
`apply_dict_writeback` (returns the mutated value to the slot) — shared by every
dispatch path instead of being inlined in the Array arm, which is precisely why
it only ever worked for arrays. The Array arm drops from 22 inline lines to 3.
The context is taken before the method runs and after `obj_ref` is evaluated —
the order the generic path always used, so nothing changes about when the key is
evaluated. A read-only method does not take it at all, so `outer["in"].keys()`
no longer copies itself back over itself.

### Also

- **README**: five features that had worked for a long time and were documented
  nowhere — `|>` (plus the missing `Pipe` row in the precedence table; it is the
  lowest precedence of all), `sizeof` (type keywords only — `sizeof(5)` is a
  parse error), `fn*`/`yield` (generators are **eager**: they return an array of
  everything yielded, not a lazy iterator), `match` as an expression with `|`
  alternatives, guards and subject binding, and a new Modules section (paths are
  relative to the importing file's directory, and every function reached from
  another file has to be exported, private helpers included).
- **360 coverage battery**, aimed by measurement rather than by eye: crossing the
  generated inventory in `src/lsp/builtins_gen.rs` against every test file found
  100 methods with zero coverage. Namespace gaps went 80 → 64, value-method gaps
  20 → 1; the remainder is structural (62 Gui methods need a real window and are
  verified by screenshot, 2 Terminal methods would corrupt the runner's output).
  New: `unit_360_random`, `unit_360_tensor_gaps`, `unit_360_namespace_gaps`,
  `unit_360_documented_gotchas` (the README's seven Known Gotchas, promised as
  guarantees and tested by nobody — all seven hold), `73_language_360_e2e` and
  `err_enum_string_concat`.
- **Pinned, not changed**: `argmax` and `argmin` break ties in opposite
  directions — `argmax` returns the LAST maximum, `argmin` the FIRST minimum
  (out of Rust's `max_by`/`min_by`). `argmax` also disagrees with NumPy and
  PyTorch, which return the first, and it is what picks the predicted class in
  classification. Asserted as-is so any change to it is a visible decision.
- **New test suites for the dispatch change**: `unit_slot_receiver_semantics`
  (value semantics, evaluation order, both writebacks), `unit_slot_collections_surface`
  (every array/string/dict/set method, each one standalone, through an instance
  field and through a dict slot), `unit_dict_methods_slot`, plus the
  `72_receiver_360_e2e` and `74_dict_slot_e2e` programs. The last one carries a
  cost guard that fails if the quadratic build ever comes back, without
  hardcoding a machine-specific number.
- **New benchmark** `16_dict_build`: dict construction and teardown through the
  method surface (bench 08 covers the subscript surface). 6384 ms against the
  previous binary, 347 ms now.
- Suite: 430 (39 new), 0 failures.

## [9.12.0] — 2026-07-30

### `sz --eval "<code>"` — run a snippet with no file

The interpreter lived entirely inside the `sz` binary, so the only way to run code
was to hand the CLI a path. It is a library now (`src/lib.rs`, crate `serez_code`),
and the binary is a thin shell over it, with two doors onto one pipeline:

| Door | Entry point |
|---|---|
| `sz file.sz` | `run::run_file` — reads disk, permissions from `serez.json` |
| `sz --eval "…"` | `run::run_eval` — source as a string, no permissions |

- **`run::run_source(src, name, opts)`** is the single pipeline (lex → parse →
  type-check → eval). A `.sz` file was only ever a string that came from disk: past
  the lexer nothing downstream can tell the difference, and the path survived only
  to label errors and locate `serez.json`. `RunOpts` carries those explicitly now,
  and `run_file` just reads the bytes and delegates.
- **`sz --eval "<code>"`** (also `-e`) takes the source as an argument — no temp
  file to write, keep clean and delete. **`sz --eval -`** reads it from stdin, which
  avoids fighting the shell over quotes and newlines in a multi-line snippet.
- The `.szx` (serez-ui JSX) plumbing moved out of `main.rs` into `src/szx.rs`.

### Lockdown mode — for source you did not write

The permission set is a **manifest, not a sandbox**. Any program can hand itself
everything with `use permissions { … }`, and three more capabilities reach the disk
with no permission declared at all — unlike OS/Socket/Task/Gui/Media/Time:

| Closed under lockdown | Why it needs closing |
|---|---|
| `use permissions { … }` | Inserts straight into the evaluator's permission set at runtime |
| `File` | Reads, writes, deletes and renames with nothing declared |
| `import` | Reads an arbitrary path off disk and **executes** it |
| `Autodiff.saveWeights` / `loadWeights` | The only methods in that namespace that touch disk |

All four come back as catchable `PermissionError`s. On for `--eval`
(`RunOpts::sandboxed()`), off for `sz file.sz` — declaring permissions inline in
your own file is unaffected.

**`fetch` is deliberately NOT part of lockdown.** It stays reachable, so on the
`--eval` path the request leaves from the host's network position: the usual SSRF
shape (cloud metadata endpoints, services on localhost, the host as an open relay).
Running untrusted source through `--eval` still needs real isolation around the
process, or a permission of its own for `fetch`.

### Also

- `fetch`'s transport is split out of `eval_fetch` into `fetch_transport`, with a
  shared `FetchResponse`; parsing, validation and the response shape no longer sit
  in the same function as the HTTP call.
- The three hardcoded `1000`s guarding recursion depth are one `MAX_CALL_DEPTH`
  const, and the error reports the actual limit.
- Suite: 419 (13 new `--eval`/lockdown CLI tests), 0 failures.

## [9.11.0] — 2026-07-27

### GUI: per-node affine transform — `Gui.nodeTransform` (rotate/scale)

- New scene primitive: **`Gui.nodeTransform(id, rotDeg, scaleXmille, scaleYmille, origX, origY)`**
  assigns an OPTIONAL affine transform to the retained node (rotation in degrees, scale in
  thousandths —1000 = 1.0—, origin in canvas px). The identity `(0,1000,1000)` clears it.
- The painter (`draw_node_transformed`) rasterizes transformed nodes by
  **inverse-mapping** with 2×2 supersampling (edge AA): fills (Rect/RectAlpha/
  RoundRect), **text** (local glyph coverage) and **images** (bitmap sampling)
  are mapped pixel by pixel; **outlines/lines** transform their vertices and are
  drawn straight; the circle scales its radius. `SceneNode` carries a `tr: Option` field.
- Enables `transform: rotate()/scale()/scaleX/scaleY` in serez-ui (the element's
  subtree is transformed around its top-left = `transform-origin: 0 0`).

## [9.10.0] — 2026-07-27

### GUI: text in PIXELS — `Gui.nodeTextPx` / `Gui.measureTextPx` (real font-size)

- The glyph engine (cosmic-text) now rasterizes by **pixel size** instead of by an
  integer scale of the 8×8 grid. Internally `ensure_glyph`/`measure`/
  `text_width`/`char_width`/`advances`/`draw_text` take `px` (the real size); the
  monospaced grid advances `px` per character instead of `8*scale`. The glyph cache
  is keyed by px.
- **The scale-based API is untouched**: `Gui.nodeText`, `Gui.measureText`, `Gui.drawText` and
  `Gui.textAdvances` map `px = 8*scale` at their boundary → **zero behavior change**
  for existing code (including the scene's `Text` node and the native primitive
  renderer).
- New primitives: **`Gui.nodeTextPx(x, y, text, px, color)`** (scene node at a
  literal pixel size) and **`Gui.measureTextPx(text, px)`** → `[width_px, px]`.
  They enable real `font-size: Npx` in serez-ui (14/20/27/34…px, not only multiples
  of 8) in the INTERPRETED renderer. `nodeSet` accepts `"px"` in addition to `"scale"`.

## [9.9.0] — 2026-07-26

### GUI: `Gui.nodeImage` with `radius` — rounded image clipping

- `Gui.nodeImage` accepts an optional 7th argument `radius`:
  `(x, y, imageId, w, h, alpha, radius)`. The blit (native and scaled) masks the
  corners with the round-rect's AA coverage (new `round_cov` helper, same distance
  as `fill_round_rect`), rounding the image's **pixels**. This enables real
  `Image { border-radius }` in serez-ui — previously the border was rounded but the
  image inside stayed rectangular.

### GUI: `Gui.nodeRoundRectOutline` — rounded outline (retained node)

- New scene primitive: **`Gui.nodeRoundRectOutline(x, y, w, h, radius, color)`**
  draws the **outline** (1px, antialiased corners) of a rounded rect. Previously there
  was only `nodeRoundRect` (filled) and `nodeRectOutline` (straight), so a `border` with
  `border-radius` ended up with square corners. It reuses the AA distance from
  `fill_round_rect`, painting only the ring band.
- Enables `border` + `border-radius` together in serez-ui: containers (`div`/`.card`),
  Image and Modal draw a rounded border instead of a square one.

## [9.7.0] — 2026-07-26

### GUI: `Gui.nodeImage` scales and applies alpha (retained node)

- The **retained** image node (`Gui.nodeImage`) goes from native size only to
  accepting, **additively**, size and opacity:
  `Gui.nodeImage(x, y, imageId)` (native), `(x, y, imageId, w, h)` (scaled) and
  `(x, y, imageId, w, h, alpha)` (scaled + global alpha 0–255). It reuses the same
  `draw_image_scaled` that `Gui.drawImage` already used, but in the scene (with dirty-skip),
  so it serves serez-ui's **retained** renderer — previously only the immediate
  `Gui.drawImage` scaled, and `renderScene` covered it up.
- Enables serez-ui's image CSS: `Image { width / height / opacity }` now
  works (scaling via the retained node did not exist).

## [9.6.0] — 2026-07-24

### `.szs`: `@when` / `@else` blocks — one condition grouping several elements

- New at-rule in the CSS engine: **`@when (cond) { … }`** wraps several rules
  (tags, `.classes`, `#ids`) under **a single logical condition**, so the condition
  does not have to be repeated selector by selector. The "query" is not a media query:
  it is the same `.szs` logic the rules use (a state variable, or `width`/`height`, with
  `and`/`or`/`not`).

  ```css
  @when (width < 300 and darkMode) {
      body   { color: #fff }
      .card  { padding: 8 }
      #main  { gap: 4 }
  }
  ```
- **`@else`** is the complement of the preceding `@when`, and **`@else (cond)`** chains
  else-if. The branches are **mutually exclusive** (evaluated top to bottom, the first
  match wins), so ranges do not have to be negated by hand:

  ```css
  @when (w < 200) { body { color: #100 } }
  @else (w < 400) { body { color: #200 } }
  @else           { body { color: #300 } }
  ```
- They can be **nested** (`@when` inside `@when`: the conditions are AND-ed) and a rule
  inside can carry **its own** `(cond)`, which is combined with the block's. `@else`
  negates the **whole** condition of the previous branch (`¬(a or b)` is
  `!eval(a or b)`, no De Morgan), so compounds like `(a or b)` complement correctly.
- **Unknown** at-rules (`@media`, …) are **discarded whole** instead of polluting
  the parse.
- Implementation: a rule's condition went from a single DNF to an **AND of negatable
  terms** (`CondTerm`); the parser is recursive per block with an inherited condition.
  Covered by 9 new Rust tests in `namespaces_gui::css` (18 in total), including the
  negation of a compound condition. serez-ui's interpreted engine gets the same grammar
  (new `when_test`, suite 22/22) so parity is not broken.

## [9.5.0] — 2026-07-21

### `.szs`: compound conditions with `and` / `or` / `not`

- A rule's condition in the native CSS engine is no longer a single comparison:
  it accepts several joined by **`and`** and **`or`**, plus negation with **`not`**,
  in the style of CSS media queries. `&&`, `||` and `!` are accepted as aliases.

  ```css
  body  (width > 600 and flag == true) { background-color: #c12; }
  .item (selected or hovered)          { border-color: #3b82f6; }
  .row  (not hidden)                   { display: flex; }
  ```
- Usual precedence: `not` binds tighter than `and`, and `and` tighter than `or`, so
  `a or b and c` is `a or (b and c)`. There are **no** grouping parentheses: the
  stylesheet scanner closes the condition at the first `)`.
- The connectors only count as whole words and respect quotes, so a name like
  `android`/`notify` or a value `"a or b"` does not split anything.
- This used to fail **silently**: the parser cut at the first comparison operator,
  left a non-existent variable (`width > 600 && flag`) and the rule never applied,
  with no error at all.
- An empty `()` now means "no condition" instead of a condition that never passes.
- Covered by the `namespaces_gui::css` Rust tests (9 cases), now included in
  `run_tests.ps1`. serez-ui's interpreted engine gets the same grammar so parity is
  not broken.

## [9.4.0] — 2026-07-21

### GUI: forgiving colors — `#rgba` / `#rrggbbaa` hex from color pickers

- The color parser accepts the alpha forms that color pickers emit
  (`#rgba`, `#rrggbbaa`): the alpha tints the background, like `rgba()` in CSS.
- Primitive-engine documentation brought up to date in the README.

### BUG: `obj.method` without parentheses ran the method instead of referencing it

- **Reading a method now yields the function bound to the object, not its execution.**
  `let ref = obj.method` (no parentheses, no arguments) returns a function you can
  invoke later; previously it fell through to method dispatch and ran it with zero
  arguments, returning its return value.
- This broke the pattern of **passing a handler as data** (`onClick={this.handler}`,
  handlers in arrays/dicts, callbacks between components): the method fired on
  EVERY read, so a state-mutating method did so on every render (a boolean flipped
  by itself, frame after frame), and what got stored was its return value —
  `null` for a `void` — so the callback never ran when invoked
  ("Attempt to call a non-function").
- If the method declared parameters, the zero-argument auto-invocation killed the
  program on the spot: `Method 'pick' expects 1 argument(s), got 0`.
- The bound reference **keeps its class context**: its body still sees its own
  private members, and referencing a private method from outside is rejected just
  like calling it.
- The `get prop()` mechanism is **unchanged**: explicit getters still run when read,
  and `obj.field` is still a field read. Resolution is field → getter → method
  reference.
- **Breaking**: code that wrote `obj.methodWithNoArgs` expecting it to run now gets
  the function without calling it. Ecosystem sweep
  (`Serez-code`, `serez-ui`, `serez-http`, `serez-ai`, `serez-graph`,
  `serez-pack`, `serez-dotenv`, `serez-cobol`, `serez-strike`, `serez-apipack`):
  zero occurrences of that form.
- Regression covered by `tests/unit_method_ref.sz` (10 cases).

## [9.3.8] — 2026-07-20

> First version published after 9.2.7: the local tag `v9.2.8` (a reverted bump)
> has no release, so its changes are listed here.

### Editor extension: `.szx` and `.szs` formatting (vscode-serez 1.9.0)

- The formatter covers all three languages: besides `.sz`, now `.szx` (JSX braces
  and depth) and `.szs` (blocks and `/* */` comments).

### GUI: native engine parity with the interpreted renderer

- `:font` recognized in `loadStylesheet`, bare boolean conditions (no comparator)
  in `.szs` rules, `font-scale` inheritance, `white-space: nowrap`,
  shrink-wrap of `absolute` elements without `width`, alpha on text nodes and the
  `:active-focus` alias.

### Multi-user `sz publish`: log in with a registry account, no hand-made tokens

- **`sz publish` / `sz unpublish` no longer require `SEREZ_API_KEY`**: the first time
  they ask for the username and password of a registry account (created at
  `packages.serezcode.org/register`), exchange the credentials for a long-lived token
  via `POST /api/login` and store it in `~/.serez/credentials.json`.
  From then on it is just `sz publish`.
- The password is read without echo (raw mode via crossterm) on a real TTY; with
  piped stdin (scripts/tests) it falls back to plain reading.
- If the stored token was revoked (401), the credential is deleted, login is
  requested again and the operation is retried once automatically.
- Registry 403 errors (someone else's package) arrive with the server's message;
  409 still reports "version already exists".
- **Compat**: if `SEREZ_API_KEY` is set it is used as before (legacy `x-api-key`
  header) and no login is requested. `SEREZ_REGISTRY_URL` still works for pointing
  at your own registry; the stored credential is per registry (if the URL changes,
  it asks for login again).
- **New `sz logout`**: deletes the stored credential; the next `sz publish` asks for
  username/password again (useful for switching accounts). With no active session it
  says so and exits successfully.

## [9.2.7] — 2026-07-14

### `throw` propagation fixes + visible `.szx` translation errors

- **`throw` inside `out f()` keeps its message**: rewinding the `out` statement's
  scratch mark freed the thrown payload BEFORE rendering it — the uncaught error
  showed "Referencia inválida" instead of the real message. Now it renders first
  and rewinds afterwards.
- **A `throw` while evaluating a nested argument no longer dies silently**: in
  `f(g())` with `g` throwing (also spread `f(...g())`), the throw degraded into a
  bare Error — exit 1 with no message at all and no chance to `catch` it.
  It now propagates as a Throw, re-planting the payload across the call's frame
  (catchable with try/catch, and visible as UNCAUGHT if nobody catches it).
- **`.szx` translator errors reach the console**: the translator's child process
  runs with `CREATE_NO_WINDOW` and its stderr was lost; now
  `sz app.szx` and `import` of `.szx` modules capture and reprint it as
  `TRANSLATE ERROR` before the generic message. (This complements serez-ui's new
  translator validation: two adjacent JSX roots in a `return()` abort with the real
  `.szx` line and the `<>…</>` fragment suggestion.)
- Tests: `unit_throw_propagation` (3 catchable cases), `err_throw_out_stmt`,
  `err_throw_nested_arg` + 2 CLI tests that verify the exact message content on stderr.

## [9.2.6] — 2026-07-14

### Primitive engine: real background translucency (rgba/hsla)

- **`background`/`background-color` with `rgba()`/`hsla()` respects the alpha
  channel**: translucency applies ONLY to the node's background (and is multiplied
  with the subtree's accumulated `opacity`) instead of being ignored. This fixes the
  Modal backdrop: `.modal-backdrop { opacity: 0.6 }` washed out the child box too;
  with `background-color: rgba(0,0,0,0.6)` (serez-ui UA sheet ≥ 4.3.6) the veil is
  translucent and the modal stays opaque.

## [9.2.5] — 2026-07-14

### Primitive engine: structural CSS gaps

- **Descendant selectors `.a .b`** (the last simple selector is the subject, the
  earlier ones match ancestors; `>` is treated as descendant), **compound classes
  `.a.b`** (previously only the last one was kept) and **groups
  `h1, h2 { }`** (one rule per selector). Focus rings like
  `Switch.focused .switch-track` stop being inert.
- **Pseudo-classes `:focus`/`:hover`/`:active`/`:disabled`**: they match the node's
  state attributes (the engine is stateless; the framework marks the state in the
  tree, the same contract as `.focused`).
- **`height` in `%` resolves against the PARENT** (the nearest ancestor with an
  explicit height; without one it falls back to the window, compatible with the
  previous behavior).
- **`opacity` propagates to the whole subtree, text included** (accumulated alpha
  ancestors × own; glyphs multiply their coverage).
- **`linear-gradient(...)`** in `background`/`background-image`
  (`to right/left/top/bottom` and `Ndeg`; with a border it paints an inset frame).
- **`box-shadow`** `[ox oy [blur [spread]]] color` with soft falloff
  (inset/spread are ignored).
- **`transform: translate/translateX/translateY`** (px): visual offset without
  touching the flow (like relative).
- **Basic `display: grid`**: `grid-template-columns` with px/%/fr/repeat(),
  `gap`/`column-gap`/`row-gap`, children in row-major order.
- serez-ui adoption (4.3.5): **continuous Slider dragging** with the mouse
  (previously click-to-set + keyboard only).

## [9.2.4] — 2026-07-12

### `.szx` module imports + modular refactor of the primitive engine + more CSS

- **`sz app.szx` runs directly** and **`import "x"` resolves `.szx` modules**
  (JSX) with on-the-fly translation, delegated to serez-ui's translator
  (`tools/translate.sz`; requires serez-ui installed). If `.sz` and `.szx`
  coexist, `.sz` wins. This replaces the szx.ps1/szx.sh wrappers.
- **Modular refactor**: the primitive engine moved out of `namespaces_gui.rs`
  (5290 → 4037 lines) into the submodules `namespaces_gui/css.rs` (selectors +
  prop resolution) and `namespaces_gui/render.rs` (layout + scene emission),
  without exposing internals (a child submodule sees its parent's privates).
- **CSS**: `rgb()/rgba()/hsl()/hsla()` colors and more CSS names; `font-size`
  in px (takes priority over font-scale); **`border` displaces the content**
  (content starts at `max(padding, border)`); **`color` inheritance** from
  ancestor to children; **`flex-shrink`** (a row of fixed items that do not fit
  shrinks proportionally instead of overflowing).
- **Build with no warnings of our own** (cleanup of unused/deprecated in crypto,
  autodiff, svg and Cargo.toml).

## [9.2.2] — 2026-07-11

### Primitive engine: web-like flex + readable refactor + text fixes

- **Shrink-to-fit text in flex rows**: spans/labels without `flex` or `width`
  measure their content instead of growing to fill — `justify-content` finally
  acts on rows of text (this fixes the Dropdown arrow stuck against the edge,
  `.modal-header` and the checkbox/fileinput centering). Bare strings in a row
  measure the same as a span.
- **Surgical CSS batch**: `width` in px/`%` on flex children (the `%` is of the
  container and is not re-applied over the slot), values with a `px` suffix in
  numeric props, `gap` only BETWEEN children (not after the last one),
  `position:relative` with left/top (right/bottom = negatives) without altering
  the flow.
- **textbox**: real `line-height`, an explicit `height` overrides the computed one,
  and caret/selection at glyph height.
- **Readable refactor of the engine** (so it can be modified by hand):
  the monolithic `prim_render` (~400 lines) → a ~70-line dispatcher + typed pieces
  `PrimCtx`/`PrimFrame`/`PrimStyle`/`PrimBox`, leaves (`prim_draw_*`)
  and containers (`prim_layout_*`), commented in Spanish with a code map at the top.
- serez-ui adoption: **caret proportional to the click** in Input/Textarea
  (`Gui.textAdvances`, nearest character boundary) with drag selection.

## [9.2.1] — 2026-07-10

### Primitive engine: adoption in real apps (serez-strike)

- **`img` accepts an image PATH**: if `src` is not a numeric handle from
  `Gui.loadSvg`, it is treated as a path to a PNG/JPG — read from disk, decoded
  (the `image` crate), scaled (preserving aspect ratio if only one dimension is
  given) and cached by path+dimensions. Web-like `<img src="…">` behavior.
- **`textbox` at 16px by default** (`font-scale: 2`, like serez-ui's interpreted
  path); the stylesheet can override it. Previously it fell back to 8px and native
  Inputs came out with tiny text.

## [9.2.0] — 2026-07-10 (work from 2026-07-07 to 2026-07-10)

### Native render primitive engine: layout + CSS + paint in the core

The bottleneck in large UIs was not rasterizing pixels (~1 ms) but the layout walk
+ CSS matching running interpreted (51–103 ms/frame on a real app tree). The core
gains a browser-style engine: it takes a tree of generic HTML-like primitives + a
CSS sheet, resolves styles, lays out and emits the scene in Rust. Measured:
**~0.04–0.08 ms/frame** of layout+CSS+emit (~1000× vs interpreted); a full frame
≈ 3.6 ms. The core stays generic (it does not know serez-ui's widgets): the
framework *lowers* its components to these primitives.

- **New API**: `Gui.loadStylesheet(src) -> handle` (`.szs` sheet),
  `Gui.loadSvg(srcOrPath) -> handle`, and
  `Gui.renderTree(root, sheet, w, h[, ctx]) -> regions`. The tree is a nested array
  `[tag, [[prop, val]…], [child|text…]]`; `renderTree` rebuilds the retained scene
  and `Gui.renderScene(bg)` rasterizes it (dirty-skip intact).
- **Primitives**: `div`, `row`, `p`, `h1`–`h6`, `span`, `b`/`strong`, `i`/`em`,
  `hr`, `img`, `svg`, `circle`, `line`, `polyline`, `polygon` and an editable
  `textbox` (caret + selection painted by the core; virtualization — it only lays
  out the visible lines, so a 10 KB text stops being expensive).
- **Web-like CSS**: selectors by `tag`, `*`, `.class`, `#id` and compounds
  (`tag.class#id`) + reactive conditions `(var op val)` evaluated against the
  `ctx`; "last one wins" resolution. Full box model (padding/margin per side
  + 1–4 value shorthands), `border` (including the `1px solid #333` shorthand) and
  `border-radius`, `width`/`height` in px/`%`/`auto`, `display:none`,
  `text-align`, `line-height`, `letter-spacing`, numeric `font-weight`,
  `text-decoration` (underline/line-through), per-node `font-family`/`font-scale`.
- **Flexbox**: `row`/`display:flex` (+ `flex-direction:column`), `flex` weights,
  `justify-content` (all 6 modes), `align-items`, `gap`; `position:absolute`
  children are out of the flow.
- **Overlays**: `position:absolute` with `left`/`top`/`bottom`/`right` (containing
  block = positioned ancestor) and `z-index` — the basis for Dropdown/Modal/Tooltip/
  Toast.
- **Real proportional text**: measurement by glyph advance (bold/italic aware),
  true word-wrap (breaking on spaces), scrolling with per-node clipping
  (scrolled backgrounds are cut cleanly).
- **Vector SVG**: our own parser for a subset of SVG (paths
  M/L/H/V/C/S/Q/T/A/Z abs+rel, shapes, `<g transform>`, fill/stroke inheritance,
  `viewBox`, colors) rasterized with **tiny-skia** (antialiasing), cached by
  handle+dimensions. New core dependency: `tiny-skia`.
- **Hit-testing**: regions come back in PRE-order as
  `[tag, x, y, w, h, onClick|null]`; the function value embedded in `onClick`
  survives the round-trip and `.sz` routes the click with `region[5]()`.
- **fix(evaluator)**: when promoting an object captured by a closure
  (Scoped→Global), ALL of the scope's aliases are now rebound, not just the
  innermost frame — previously the object forked and mutations made after creating
  a lambda were lost.
- Tests: `tests/unit_gui_primitives.sz` (headless engine checks); the real render
  is verified with demos + screenshots. Suite **399/0**.
- **Adoption**: serez-ui lowers its components to these primitives behind the
  `useNativeRenderer` flag (Phase 3 complete: every widget verified natively)
  and serez-strike runs on the native renderer.

## [8.2.0] — 2026-07-03 (work from 2026-07-02 to 2026-07-03)

### "Technical debt" batch: strict parser, closure semantics, multi-window, retained-mode, audio

- **Parser: no more silent recoveries.** `let x = ;`, `let = 5;`,
  `let x;`, `return a +`, invalid numeric literals and the rest of the holes where
  the parser discarded statements without reporting now emit a `PARSER ERROR` with
  position and caret. A program with parse errors **no longer runs halfway**
  (it aborts with exit 1), and `import`s of modules with parse errors abort instead
  of evaluating the partial module. A bare `;` is a legal empty statement.
  This uncovered and fixed 2 latent bugs in serez-ui (renderer.sz: an unescaped
  `"{·"` triggered the interpolation parser and the whole `out` was silently
  discarded — the TUI Select never printed). New `err_parse_*` tests.
- **Parser errors name the FILE** (`PARSER ERROR [path line:col]`),
  including imported modules and interpolated expressions.
- **Lexer: uniform `token.column`.** Every token carries the position of its
  FIRST character (before: identifiers pointed one position past the last char).
  The LSP dropped its `ident_start_col` correction.
- **Semantics: closures with a SHARED CELL.** A lambda and its enclosing scope
  share the captured variable at any nesting level: mutations inside the closure
  escape (`makeCounter` counters work) and later writes are visible inside. A `for`
  counter is fresh per iteration (like JS `let`): the loop's closures keep the value
  of their iteration (10,20,30 — not 40,40,40). A counter declared outside a
  `while` is a single shared cell (333). Semantics tests updated.
- **Semantics: a non-callable parameter no longer hides a same-named function in
  CALLS** (the `h` parameter case that broke serez-ui's render): `name(...)`
  falls back to the nearest callable binding; reads still see the shadow.
- **Multi-window Gui:** `Gui.openWindow(title,w,h) -> id`, `Gui.selectWindow(id)`
  (all existing drawing/input moves to that window), `Gui.currentWindow()`,
  `Gui.closeWindow(id)`. The classic `Gui.open` window is id 0 and its protocol
  does not change (serez-ui untouched). Each extra window has its own canvas and
  input (mouse/keyboard/scroll/focus). Verified with a 2-window demo
  (~2,600 combined presents/s).
- **Retained-mode Gui (scene graph):** persistent nodes the core redraws in Rust —
  `nodeRect/nodeCircle/nodeLine/nodeText/nodeImage -> id`,
  `nodeSet(id, prop, value)` (x, y, w, h, r, x2, y2, color, z, visible, text,
  scale, image), `nodeDelete`, `sceneClear`, `nodeCount` and
  `Gui.renderScene(bg) -> bool`, which redraws ONLY if the scene is dirty (if not,
  it re-presents and returns false). This removes re-running the interpreted draw
  tree every frame.
- **New `Media` namespace (audio, `Media` permission):** `playSound(path) -> id`
  (wav/mp3/flac/vorbis via rodio, asynchronous), `stop/stopAll/pause/resume`,
  `setVolume(id, 0..200)`, `isPlaying(id)`, `playingCount()`. Catchable errors:
  `IOError` (file) and `MediaError` (format/device); a permission denial is still
  fatal (`sec_media_no_permission`). Video is out of scope: decoding it requires
  ffmpeg (design decision pending).
- **LSP:** multi-file analysis (symbols/definition/completion follow transitive
  `import`s, cached by mtime), `.szx` support (symbols/outline without
  diagnostics: the parser does not speak JSX), `rename`, `references` and
  `signatureHelp` (user functions, builtins and namespace methods).
  Extension **1.8.0**: client for `.szx`, partial Restricted Mode support
  (`untrustedWorkspaces: limited` — highlighting/formatter yes; sz-lsp only in
  trusted workspaces, and it starts as soon as trust is granted).
- Suite: **398/0** (+9 new tests); fuzzing 300 cases with no panics; LSP 27 tests +
  smoke 9/9; whole ecosystem green (ui 17/17, http/ai/pack/apipack/agentai/
  graph 3/0, dotenv 2/0, cobol 23+22, strike 53/0).

### Ecosystem adoption (same date): full scene parity + retained serez-ui

- **The retained scene reached parity with EVERY primitive serez-ui uses**:
  `nodeRoundRect`, `nodeRectAlpha`, `nodeRectOutline`, `nodePolygon`,
  `nodePolyline`, `nodeClipPush`/`nodeClipPop` (clipping as markers in the
  draw order) and text with per-node font/style/spacing (`nodeSet`:
  `font`, `style`, `spacing`, `radius`, `alpha`, `width`, `points`).
- **The scene is PER WINDOW** (each window has its own scene graph; `node*` and
  `renderScene` operate on the selected window) — two retained windows no longer
  collide.
- **Click by EVENT in extra windows**: `Gui.mousePressed()` on a secondary window
  counts presses as events in its accumulator — a short click between two presents
  is no longer lost (it used to be level-triggered).
- **An unreadable `serez.json` no longer fails silently**: if it exists but does not
  parse (e.g. without `"version"`), a WARNING is emitted instead of running with no
  permissions.
- **serez-ui (2.3.0)**: the GUI renderer migrated to retained-mode (`sw*` methods with
  positional node reuse; `Gui.renderScene` instead of `clear+draw+present`;
  pixel-perfect visual parity verified against the previous engine) and gained
  **secondary windows**: `openPanel/closePanel/panelCount` + `renderPanel(id)`
  on the component itself (Button/Link clicks routed per panel; verified with a real
  demo: click on a panel → app state → re-render of the main one).
- **serez-pack**: compatibility verified end-to-end (an app with the `Media`
  permission packaged and executed); it now validates at packaging time that
  `serez.json` has a `"version"` (without it, the installed app would run with no
  permissions).

### New: `sz-lsp` — Language Server Protocol for editor support

- **New binary `sz-lsp`** (`src/lsp_main.rs` + `src/lsp/`): an LSP server over
  stdio JSON-RPC that closes the last open roadmap checkbox. It reuses the
  interpreter's lexer/parser/type-checker directly (second `[[bin]]` target;
  the `sz` binary and the runtime are untouched). No async runtime: a
  synchronous framed loop over `serde_json` (the only new dependency).
- **Capabilities:** live diagnostics on every keystroke (parser errors as
  errors + static type checker findings as warnings, with real ranges),
  completion (keywords, the 21 native namespaces with their real methods,
  builtin functions, and the document's own symbols — `File.` lists `read`,
  `write`, …), hover (user signatures `fn int suma(int a, int b)`, namespace
  summaries), go-to-definition (functions/classes/enums/variables +
  `import "…"` jumps to the file) and hierarchical document symbols.
- **Symbol index works on broken files:** it scans tokens (which carry
  line/column) instead of the AST, so completion/outline keep working while
  the user is mid-keystroke — the normal state in an editor.
- **Parser/type checker now *collect* their errors** (`Parser::take_errors`,
  `TypeChecker::take_errors`, with 1-based positions) in addition to printing
  them; CLI output is unchanged (suite 389/0).
- **Namespace/method catalog is generated** from the evaluator sources by
  `tools/gen_lsp_builtins.py` (21 namespaces, 227 methods + value methods of
  array/string/set/dec/tensor) into `src/lsp/builtins_gen.rs` — re-run it when
  a namespace gains methods.
- **VS Code extension 1.7.0** (`vscode-serez/`): starts `sz-lsp` automatically
  for `.sz` (new settings `serez.lsp.enabled`, `serez.lsp.path`; uses `PATH`
  by default). Formatter and highlighting are unchanged and keep working if
  the binary is missing.
- **Tests:** 22 Rust tests (`cargo test --bin sz-lsp`) covering analysis,
  symbol scanning and the full protocol handshake, plus
  `tools/lsp_smoke.py` — a real LSP session against the compiled binary
  (initialize → didOpen broken file → diagnostics → completion → hover →
  definition → shutdown), 9/9.

---

## [7.3.0] — 2026-07-02

### New: language-level errors are catchable (third pass) + collections are O(1)

- **Third pass of catchable errors — the language core itself.** `Variable not
  found` and undeclared-variable assignments now raise a catchable
  **`ReferenceError`**; calling a non-function, argument-count/spread mismatches,
  `const` reassignment and all `builtins` failures (`parseInt`, `fetch`, `env`,
  …) raise a catchable `TypeError`/`RuntimeError` instead of aborting. The call
  path unwinds cleanly (scope / call-depth / call-stack restored) so a
  `try/catch` in a loop can absorb thousands of these without corrupting the
  evaluator. **Stack overflow and resource limits stay fatal** (a catchable
  overflow would let infinite recursion retry forever).
- **Array `push`/`pop` are O(1)** (run against the arena slot instead of cloning
  the whole array per call): building an array with `a.push(x)` in a loop went
  from O(N²) to O(N) — 20 000 pushes dropped from 8 824 ms to 11 ms. The
  `dict["k"].push(x)` and `instance.field.push(x)` write-back patterns are
  preserved.
- **Lambda capture no longer leaks.** A lambda captured *every* visible local
  into the global arena on each creation (a permanent slot per unused local per
  lambda); it now snapshots only the identifiers the body actually references.
  A lambda created per loop iteration dropped from linear arena growth to flat.

### Security

- **Fixed a string-escape bug that could dodge the `OS.exec` System32 block.**
  An unknown escape (`"C:\Windows"`, `"a\d"`) duplicated the character after the
  backslash (`C:\Windows` → `C:\WWindows`), which both corrupted the path and
  made it slip past the blocked-path substring check. Escapes now keep both
  characters verbatim without duplication.
- New systematic non-catchable security tests (`sec_notcatch_*.sz`): permission
  denials, `unsafe {}` gates, the System32 exec block, tensor size limits and
  stack overflow are each verified to stay fatal inside a `try/catch`.
- Added `fuzz_parser.py`: feeds garbage and mutated corpus to the lexer/parser
  and asserts no Rust panic (0 crashes over 1 000 cases across two seeds).

### Build

- The AOT/LLVM backend (`src/compiler/`, ~3 000 lines) is now behind the
  `llvm` Cargo feature (off by default): it is Phase-1 and not wired to any CLI
  verb, so the default build skips it and the `inkwell`/LLVM-17 dependency.

### New: I/O and namespace errors are catchable (second pass, ~530 sites)

- All runtime failures across **File, JSON, Math, OS, Terminal, Env, Time,
  System, Socket, Gui, Tensor, Autodiff, GPU, Memory, Binary and Crypto** are
  now catchable with `try/catch`, binding the structured `Error` object. New
  `.kind` categories: **`IOError`**, **`JsonError`**, **`OSError`**,
  **`SocketError`**, **`GuiError`**, **`TensorError`**, **`AutodiffError`**,
  **`GpuError`**, **`MemoryError`**, **`BinaryError`**. Invalid arguments
  (wrong count/type, unknown method) raise `TypeError`. A missing file, a
  refused socket or a tensor shape mismatch no longer kill the process.
- **Unchanged and still fatal**: permission denials, `unsafe {}` gates, the
  System32 exec block, and resource limits (256 MB file reads, 10M-element
  tensors, GPU buffer caps). The sandbox invariant is untouched
  (`sec_runtime_not_catchable` still passes).
- Compat notes: `OS.spawn` still returns `-1` when the process fails to start,
  and `Socket.recvWsFrame` still returns `null` on protocol errors (APIs relied
  upon by serez-ui/serez-http).
- New suite file `unit_catchable_io.sz` (11 assertions) pins the behavior.

### Perf: dict lookups are O(1) on large dicts (~860× on 20k keys)

- `d[key]` reads no longer clone the whole dict — the lookup runs directly
  against the arena slot.
- Dicts now carry a lazy hash index (canonical key → position) built on first
  lookup once the dict has ≥ 16 entries, kept warm incrementally on
  `d[key] = value` inserts, and validated on every hit (a stale cache can only
  fall back to the linear scan, never return a wrong entry). Insertion order is
  preserved; small dicts keep the plain linear scan. Benchmark: 20 000 inserts +
  20 000 reads went from 39 689 ms to 46 ms.

### Perf: Set membership is O(1) on large sets (~1 500× on 20k elements)

- `has`/`contains`/`add` run directly against the arena slot (no O(N) clone of
  the whole set per call) and use a lazy hash index over TYPE-TAGGED element
  fingerprints — faithful to `obj_data_eq`: `5` and `"5"` stay distinct
  elements, `1.50m` equals `1.5m` (scale-insensitive `dec`), `-0.0` equals
  `0.0`, NaN never equals itself, and compound values keep the authoritative
  linear scan. `new Set([...])` deduplicates via hash in O(N) instead of the
  old O(N²) pairwise scan. Benchmark: 20 000 adds + 20 000 `has` went from
  71 383 ms to 48 ms. Small sets (< 16) keep the plain linear scan. New suite
  file `unit_set_index.sz` pins the equality semantics.
- ALL Set methods now run against the arena slot: the generic dot-call path
  used to clone the entire element vector before entering any method — even
  `.size()` paid an O(N) copy — and mutations rewrote the whole slot.
  `delete` finds its target through the index and removes in place (insertion
  order preserved); `clear` truncates in place.
- `union`/`intersection` went from O(N×M) pairwise comparisons to O(N+M) via
  fingerprint sets: two 5 000-element sets took 2 546 ms / 1 563 ms, now 2 ms
  each. Set argument/arity errors are now catchable (`TypeError`), matching
  the rest of the runtime.

### Fixed: top-level loops no longer grow the global arena

- A `while` / `do-while` at top level leaked one global-arena slot per
  iteration (the condition's temporary, allocated with no scope active). Loops
  now run inside an ephemeral frame so those temporaries land in the scoped
  arena and are reclaimed per iteration; `do-while` additionally gained the
  per-iteration condition cleanup it was missing at any depth. A 2 000-iteration
  top-level loop now leaves the arena at baseline (~261 slots, was ~2 262).

---

## [7.3.0] — 2026-07-01 (earlier work in this release)

### New: catchable runtime errors + structured `Error` object

- `try/catch` now catches ordinary **programming** errors (index out of range,
  division/modulo by zero, type mismatches, invalid assignment targets), not just
  values raised with `throw`. Inside `catch` they bind an **`Error`** object with
  `.message` and `.kind` (`IndexOutOfBounds`, `DivisionByZero`, `TypeError`,
  `InvalidAssignTarget`, `Overflow`, `RuntimeError`). `throw "x"` still binds the
  raw value.
- **Security and resource-limit errors stay fatal and non-catchable** (permission
  denials, `unsafe`-required gates, stack overflow / resource guards) — a
  `try/catch` cannot silently swallow them, preserving the sandbox and DoS
  protections.
- String concatenation with an instance (`"x" + e`) now renders it (the `Error`
  object → its `.message`), while still honouring a user-defined `op_str`/`op_add`.

### New: `Regex` namespace (dependency-free engine)

- `Regex.test / match / findAll / split / replace`. Backtracking engine compiled
  to bytecode, bounded step budget (no hangs). Supports `. \d \w \s`, classes
  `[a-z]` `[^…]`, anchors `^ $`, groups `( )` / `(?: )`, alternation `|`, and
  quantifiers `* + ? {n,m}` (greedy or lazy). Patterns use raw strings `r"…"`.
  No permission required. Invalid patterns raise a catchable error.

### Changed: `arr[i] = x` is now O(1); nested index-assign is loud

- Array index assignment mutates the element in place (`Arena::get_mut`) instead of
  copying the whole array — a fill loop goes from O(N²) to O(N). Value semantics
  are unchanged (verified: `let b = a; a[0]=x` does not affect `b`).
- Assigning into a **temporary** target (`m[i][j] = x` where `m[i]` is a copy,
  `getArr()[i] = x`, …) previously did nothing silently; it now raises a catchable
  `InvalidAssignTarget` error. Reassign the whole element instead (`m[i] = inner`).

### Fixed: `x is function`

- `is function` now returns `true` for named functions and lambdas (previously
  always `false`, though `type_of` already reported `"function"`).

### Perf: parallel, cache-friendly matmul

- `Tensor.matmul` rewritten from a naive `ijk` triple loop to a cache-friendly
  `ikj` order and parallelized across output rows with `std` scoped threads (no
  external dependency; small matrices stay single-threaded). The autodiff backward
  pass reuses the same kernel, so training also benefits. Results are bit-identical.

---

## [7.2.0] — 2026-06-30

### GUI: vector drawing, full input and window control

- **Vector primitives**: thick lines, polylines, polygons and an antialiased
  circle.
- **Text**: underline / strikethrough and `letter-spacing` in `drawText`.
- **Images**: loading from bytes (in addition to a path), image clipboard
  (get/set) and a custom-image mouse cursor.
- **Window and screen**: window position, monitor enumeration and the rest of
  the window operations.
- **Input**: the missing `winit` events — focus, cursor, file drop,
  IME preedit and side mouse buttons — plus touch, hover and pinch.
- **Horizontal scrolling** in the predictive compositing.

---

## [7.1.0] — 2026-06-29

### GUI: asynchronous predictive scrolling (threaded compositing)

- Scroll compositing moves to a separate thread and anticipates the displacement,
  so the window does not wait for the repaint to respond.

---

## [7.0.0] — 2026-06-28

### `Task` namespace — isolated concurrency on native threads

- New **`Task`** namespace: asynchronous execution of isolated subprocesses on
  Rust threads, *share-nothing* (each worker with its own arena, communicating
  via JSON). Requires the `Task` permission.
- Covered by stress tests, nested workers and protection against panics inside
  a subprocess.

### BREAKING: system namespace names are reserved

- **A class, interface or enum can no longer be named after a system namespace**
  (`Task`, `File`, `OS`, `Gui`, `Env`, `Time`, `Socket`, …). The parser rejects it
  with an explicit message.
- This is the counterpart of adding `Task`: without the rule, a user `class Task`
  would shadow the native namespace. Existing code using one of those names must
  rename the class — as was done with the `apps/01_task_manager.sz` example
  (`Task` → `TaskItem`).

### OS: non-blocking `OS.spawn`

- `OS.spawn` stops blocking and is harvested by *polling* with **`OS.tick()`**
  (no callbacks: callbacks would open a use-after-free with the region model).

### GUI: ~0 CPU when idle

- The loop becomes *event-driven*: with no activity, CPU usage drops to ~0.
  Window, dialog, image and text APIs are added, and reflow on resize is fixed.

### Fixes

- Two interpreter panics on invalid input.
- Overflow in the iterative fibonacci benchmark; concurrency and decimal
  benchmarks are added.
- Editor extension: Serez Dark theme and up-to-date grammar (1.6.0), `"{var}"`
  interpolation colored as braces + variable (1.6.1).

---

## [6.3.0] — branch `improve` (2026-06-20)

### New: `DateTime` namespace (calendar date/time)

- Immutable date/time built on `chrono`. Construction: `DateTime.now()` /
  `utcNow()` (require the `Time` permission), `DateTime.from(y,m,d[,h,mi,s,ms])`
  and `DateTime.fromEpoch(ms)` (no permission — pure, and reject invalid dates).
- Fields `.year/.month/.day/.hour/.minute/.second/.ms` return a **DateField**
  that acts as an `int` under operators yet carries immutable
  `.add(n)/.reduce(n)/.remove(n)` returning a new `DateTime`. Day/time units shift
  the instant; month/year adjust field-wise and clamp the day to month end.
  Read-only `.weekday/.dayOfYear/.daysInMonth`, plus `.isLeapYear()/.isUtc()`.
- `.format(pattern)` (moment.js-style, `[literal]` escaping), `.toString()/.iso()`,
  `.timestamp()/.toEpoch()/.epochMillis()`. Object-destructuring exposes the
  calendar fields as ints: `const {day, month, year} = DateTime.now()`.

### New: exact decimal type `dec`

- Base-10 exact decimal (crate `rust_decimal`, 28–29 digits) alongside the
  untouched `decimal` (f64). Literal suffix `m`: `12.50m`, `5m`, `1e-7m`.
- `int` mixes in exactly; mixing `dec` with f64 `decimal` is a type error
  (convert via `toDecimal()` / `Dec.parse`). Comparison by value; checked
  arithmetic; `/` rounds to 28 digits half-even; `**` requires an int exponent.
- Methods `round/setScale/truncate` (modes half-even default, half-up, down, up,
  floor, ceil), `scale/abs/floor/ceil/isZero/sign/min/max/toInt/toDecimal/
  toString`; namespace `Dec.parse/fromInt/MAX/MIN/MAX_SCALE`. JSON serializes a
  `dec` as an exact number literal. Works in switch, sort, includes and dicts.

### New: raw string literals `r"…"`

- `r"…"` disables interpolation **and** escape processing — `{ }` and backslashes
  are literal (great for literal braces, Windows paths, regexes). Cannot contain a
  `"`. Default `"…"` interpolation is unchanged (zero impact on existing code).

### Bug fixes

- **B-77** — `op_str` is now honored in string `+` concatenation (both operand
  orders), consistent with interpolation/array display.
- **B-78** — escaped closing brace `\}` in a string literal no longer leaks the
  backslash (symmetric with `\{`); inline literal JSON now works.
- **B-79** — the power operator `**` is now **right-associative**
  (`2 ** 3 ** 2 == 512`), matching math/Python.

### Tests

- New: `unit_datetime`, `unit_dec`, plus mixed/integration suites
  `unit_mixed_features`, `unit_stdlib_mixed`, `unit_systems_mixed`,
  `unit_net_gui_mixed` and e2e `63`–`71` (datetime, dec, raw/op_str, deep
  cross-feature, stdlib, systems/crypto/GPU/autodiff, networking+GUI), plus
  security tests for the new namespaces. Full suite: **369 passing, 0 failing.**

---

## [Unreleased] — branch `improve` (2026-06-11)

### Memory — loop-body value retention fixed (leak #1 residual)

- **`eval_block_discard`**: loop bodies (`for`, `while`, `do-while`, `foreach`) no
  longer deep-extract and re-plant the value of the body's **last statement** into
  the loop's frame. Every loop caller discards that value, but the copy lived until
  the loop exited — so any loop whose last statement produced a compound
  (`arr = arr.map(...)`, `arr.reverse()`, …) retained one full copy **per
  iteration**. Measured: 300 iterations over a 20k-element array went from
  **~430 MB peak RSS to ~17 MB**. `return`/`throw` escaping the body keep the
  exact same extract+plant semantics as before.
- Probes refreshed (`mem_probe/`): the historic big leak (push-promotion in
  helpers, probes `f`/`h`) was already killed by the element-embedding refactor
  (`Array/Dict/Set` store `OwnedValue`, like `Instance`); global arena stays at
  baseline (~262 slots). Known minor residual: one small global slot per lambda
  **created** inside a loop (capture snapshot, ~24 bytes each).
- New regression test `unit_loop_body_value` (7 asserts): compound reassign /
  mutating method as last statement, return/throw from body, break/continue,
  do-while and foreach intact.

### Crypto — real signatures and CSPRNG (vetted crates)

- **`Crypto.randomBytes(n)`** — cryptographically secure random bytes from the OS
  entropy source (`getrandom` crate). Returns `[int]` (0..255). `n` capped at
  1 MB (throws beyond it; throws on `n < 1`). Unlike `Random.*` (seedable LCG,
  predictable — fine for games, never for secrets), this is safe for tokens,
  salts and keys.
- **`Crypto.ed25519Keypair()`** — generates an Ed25519 keypair
  (`ed25519-dalek` crate); returns `{ private, public }` as 64-char hex strings.
- **`Crypto.ed25519Sign(privateHex, message)`** — returns the 128-char hex
  signature. Deterministic (Ed25519 by design). Malformed/short keys throw.
- **`Crypto.ed25519Verify(publicHex, message, signatureHex)`** — `true`/`false`
  via strict verification (rejects non-canonical signatures). Malformed hex or
  wrong lengths throw; well-formed but invalid signatures return `false`.
- New tests: `unit_crypto_ed25519` (7) and `sec_crypto_ed25519` (8 — caps,
  malformed inputs, corrupted-signature behavior).

### Lexer

- New regression suite `unit_sci_notation` (7 asserts) cementing scientific
  notation (`1e-7`, `2.5e3`, `1E+10`, bare `e` still an identifier) — the
  feature itself shipped in 4.6.2.

---

## [5.0.0]

### GUI

- **`Gui.time()`**, **`Gui.drawRect(x, y, w, h, color)`**,
  **`Gui.fillCircle(cx, cy, r, color)`**, **`Gui.setImePosition(x, y)`** — drawing
  and IME surface for serez-ui (cursor blink timing, outlines, radio buttons,
  IME composition placement).

---

## [4.9.0]

### GUI

- **Font loading and selection**: `Gui.loadFont(path)` + proportional text
  rendering with real font metrics (replaces fixed-advance text).
- **`Gui.fillRoundRect(x, y, w, h, radius, color)`**.
- Error + security test coverage for the new Gui surface
  (`err_gui_*`, `sec_gui_no_permission`).

---

## [4.8.0]

### GUI — backend migration

- Backend migrated **minifb → winit + softbuffer + cosmic-text**: proper window
  lifecycle, IME support, real text shaping/rasterization, and the event model
  serez-ui's self-driven loop (`app.runGui`) builds on.

---

## [4.7.0]

### CLI

- **Run `.szx` (serez-ui JSX) files directly**: `sz app.szx` transpiles and runs
  without a separate step.

---

## [4.6.2]

### Lexer

- **Scientific notation in number literals**: `1e-7`, `2.5e3`, `1E+10`. The `e`
  is only consumed when followed by `[+-]?digit`, so identifiers like `e` keep
  lexing as before. (Unblocked BCE-style epsilon constants in serez-ai guides.)

### CLI

- **`sz run <name>` resolves package bin commands**: if `<name>` is not a script
  in `serez.json`, it resolves the entry of an installed package and forwards
  the remaining args (e.g. `sz run apipack build`).
- **Non-zero exit codes** on parse errors, runtime errors, and subcommand
  failures (CI-friendly).

---

## [4.6.0] — branch `improve`

### Package manager — dependency write-back

- **`sz install <pkg>`** now records the resolved dependency in `serez.json` (insert or update), so installing by command keeps the manifest in sync — matching the behavior of `npm install <pkg>` / `cargo add`. Previously the manifest was read-only and only `sz install` (no args) consumed it.
- **`sz uninstall <pkg>`** now removes the dependency from `serez.json` as well.
- The manifest edit is **surgical**: only the `dependencies` object is rewritten (canonical 2-space layout); `name`, `version`, `scripts`, `permissions` and the rest of the file's formatting are preserved verbatim. Brace matching honors `{`/`}` inside string values.
- `sz install` (no args, installs from the manifest) does **not** rewrite `serez.json`, so hand-written version specs are never clobbered.
- Manifest write failures are non-fatal: the package is already on disk, so the install/uninstall reports a warning instead of failing. With no `serez.json` present, `sz install` hints to run `sz init`.

### Tests

- 7 new Rust unit tests in `package_manager` (upsert into empty deps, append, update-in-place, insert missing `dependencies` key, preserve `scripts` block, brace-in-string handling, remove round-trip). Module suite: 14/14 pass.

---

## [4.5.0] — branch `core-websocket` → merged to `improve`

### WebSocket support (RFC 6455)

- **`Crypto.sha1(s)`** — SHA-1 hash, returns 40-char lowercase hex. Pure-Rust implementation, no external crates. Validated against RFC 3174 test vectors.
- **`Crypto.sha1base64(s)`** — SHA-1 followed by base64 encode of the raw digest. Used for the WebSocket handshake `Sec-WebSocket-Accept` key. Validated against the RFC 6455 §1.3 vector.
- **`Socket.recvWsFrame(conn_id)`** → `string | null` — decodes one WebSocket frame (RFC 6455): parses header, extended length, unmasks payload. Returns `null` on close frame.
- **`Socket.sendWsFrame(conn_id, data)`** → `null` — encodes `data` as an unmasked text frame (server → client) with correct 1-byte / 2-byte / 8-byte length encoding.
- **`Socket.listen(port)`** — now binds to `0.0.0.0` instead of `127.0.0.1`, allowing external connections (e.g. inside Docker via serez-apipack).

### WebSocket protocol hardening (5 bugs fixed)

- **DoS — unbounded payload** — a frame claiming `payload_len = 2^63` would allocate `vec![0; huge]` and crash. Now capped at `WS_MAX_PAYLOAD` (16 MiB), enforced on both the 1-byte and 8-byte extended-length paths before allocation.
- **Ping not answered** — `opcode=9` (ping) was returned as data. Real browsers close the connection on missing pong. Now auto-replies with `opcode=10` (pong) carrying the same payload, then loops to read the next data frame. Loop (not recursion) avoids stack overflow on repeated pings.
- **Close frame stream desync** — `opcode=8` returned before reading the close code + reason, leaving bytes in the TCP buffer that corrupted the next read. Now the payload is fully consumed before returning `null`.
- **RSV bits not validated** — RFC 6455 §5.2 requires RSV1/2/3 = 0 without a negotiated extension. Now rejects frames with any RSV bit set.
- **Invalid UTF-8 silently mangled** — text frames used `from_utf8_lossy` (replacing bad bytes with U+FFFD). RFC 6455 §5.7 requires an error. Now returns an error on invalid UTF-8. Control frames with payload > 125 bytes are also rejected (§5.5).

### Tests

- `unit_websocket` (13), `unit_sec_websocket` (13), `sec_websocket`, `54_websocket_e2e`, `55_websocket_integral`, `62_websocket_full_integral` (33 assertions), plus 8 Rust `ws_frame_tests`. Full suite: 327 `.sz` tests, 0 failures.

---

## [4.3.2] — branch `ai-deep` → merged to `improve`

### AI / Autodiff — Phase 1: Core training infrastructure

- **Optimizers** — `Autodiff.adamStep`, `adamwStep`, `sgdStep`, `rmspropStep`. All are pure functions that take current params + state and return `[new_param, new_state...]`. No tape side-effects.
- **Loss functions** — `Autodiff.mseLoss`, `maeLoss`, `bceLoss`, `crossEntropyLoss`. All tracked on the tape with correct backward passes.
- **Weight initialization** — `Autodiff.xavierUniform`, `xavierNormal`, `heUniform`, `heNormal`. Fan-in/fan-out computed automatically from shape (2D: `[out, in]`; 4D conv: `[cout, cin, kH, kW]`).
- **Gradient clipping** — `Autodiff.clipGrad(grad, max_norm)` per-tensor; `clipGradNorm(grads_array, max_norm)` global norm across a list of tensors.

### AI / Autodiff — Phase 2: Regularization & modern layers

- **BatchNorm** — `Autodiff.batchNorm(x, gamma, beta, training, [eps])`. Full backward: per-feature gradient for `gamma`, `beta`, and input. Input must be `[N, C]`.
- **Dropout** — `Autodiff.dropout(x, p, [training])`. Inverted dropout (divides by keep_prob in forward). Mask saved for backward. `training=false` → no-op.
- **Embedding** — `Autodiff.embedding(indices, weight)`. Gathers rows from `[vocab, emb_dim]` weight. Backward scatters gradients back to touched rows. `vocab_size` stored in `TapeOp` to avoid inference issues.
- **New activations (all tracked):**
  - `t.elu([alpha])` — ELU with correct `alpha * exp(x)` backward
  - `t.swish()` / `t.silu()` — swish with `(sigmoid + x*sigmoid*(1-sigmoid))` backward; stores both `x` and `sigmoid(x)`
  - `t.mish()` — mish with `tanh(sp) + x*sech²(sp)*sigmoid(x)` backward
  - `t.gelu()` — GELU now tracked with full `d/dx` backward (was untracked before)
  - `t.leaky_relu(alpha)` — now tracked (was untracked before)
- **AvgPool2d** — `t.avg_pool2d(kernel, stride)`. Uniform gradient distribution in backward.
- **Tensor utilities** — `.variance()`, `.std()`, `.cumsum()`, `.softplus()`, `.hardsigmoid()`, `.hardswish()`

### AI / Autodiff — Phase 3: N-D operations & performance

- **Shape manipulation** — `t.unsqueeze(dim)`, `t.squeeze()`, `t.squeeze(dim)`, `t.permute([axes])` (full N-D generalized transpose)
- **N-D broadcasting** — `t.broadcastTo([shape])`, `t.broadcastAddNd(other)`, `t.broadcastMulNd(other)`. Full numpy semantics for arbitrary dimensions.
- **Batch matmul** — `t.bmm(other)`: `[B,N,M] @ [B,M,K] → [B,N,K]`
- **N-D reduce** — `t.reduceSum(axis)`, `t.reduceMean(axis)`, `t.reduceMax(axis)` for any tensor dimension
- **Element-wise ops** — `t.sign()`, `t.reciprocal()`, `t.sin()`, `t.cos()`, `t.round()`, `t.floor()`, `t.ceil()`, `t.maximum(other)`, `t.minimum(other)`
- **stopGrad / detach** — `t.stopGrad()`, `t.detach()`, `Autodiff.stopGrad(tensor)` — returns a copy disconnected from the tape

### AI / Autodiff — Weight persistence

- **`Autodiff.saveWeights(path, tensors)`** — saves an array of tensors to a `.szw` binary file (magic `SZWT` + version + count + per-tensor: ndim, shape, data as f64 LE)
- **`Autodiff.loadWeights(path)`** — reads `.szw` and returns `Array` of tensors in the same order. Full round-trip precision (float64).

### Autodiff bug fixes

- **`TapeOp::BroadcastMul` backward** — was incomplete (only accumulated gradient to `mat_id`, skipped `rhs_id`). Now saves both `mat_data` and `rhs_data` in forward, computes `d_mat` and `d_rhs` correctly.
- **`TapeOp::Swish` backward** — was reconstructing `x` from `sigmoid(x)` via logit (numerically unstable). Now stores `cached_input` alongside `cached_sigmoid`.
- **`TapeOp::Gelu`** — GELU was not tracked at all. Added `TapeOp::Gelu` with correct backward.
- **`leaky_relu`** — was not recorded on the tape. Now records `TapeOp::LeakyRelu`.
- **`TapeOp::Embedding`** — backward was inferring vocab size heuristically. Now stores `vocab_size` explicitly in the op.
- **`TapeOp::Swish` shape** — added `cached_input: Vec<f64>` field to the variant.

### Dict bug fix (B-31 complete)

- **Typed dict missing-key access** — `d["missing"]` on a `<string, int>` dict was still throwing `❌ ERROR: Key not found in typed dict` instead of returning `null`. The B-31 fix was only applied to `value_type == "any"` dicts. Now all dicts return `null` for missing keys regardless of type annotation.
- **`dict["key"].push(val)` writeback** — calling mutating array methods on a value retrieved from a dict (`grupos["pares"].push(n)`) now writes the modified array back to the dict automatically. Previously the modification was silently discarded.
- **`plant` → `plant_global`** for dict value access — prevents dangling refs when the dict lives in an outer scope.

### Package manager

- **`sz init`** — creates a `serez.json` interactively in the current directory. Prompts for name (default: folder name), version, description, author.
- **`sz init --y`** — non-interactive: uses folder name as project name, all defaults, no prompts.
- **`sz run <script>`** — reads `serez.json` and executes the named script entry (e.g. `sz run dev` → runs `sz index.sz`). Reports error with available scripts if name not found.
- **`scripts` field in `serez.json`** — new manifest field, parsed alongside `dependencies` and `permissions`.

### stdout flush fix

- **`stdout` buffer** — `run_file()` now explicitly flushes `stdout` before returning. On Windows, large output from the spawned interpreter thread could appear after the shell prompt due to unflushed buffered writes. Regression test: `49_stdout_flush` (200 output lines).

### Test count

- **321 passing** (0 failing) across E2E, unit, error, security, AI, CLI, and package manager tests.
- New test files: `ai_phase1_training.sz`, `ai_phase2_layers.sz`, `ai_phase3_ops.sz`, `ai_weights_persistence.sz`, `49_stdout_flush.sz`.

---

## [4.1.2] — branch `improve`

### Package manager

- `sz init` / `sz run` / `scripts` field (see v4.3.2 above — backfilled from ai-deep merge)

---

## [4.0.1] — branch `improve`

### Networking / stdlib

- **Default `User-Agent`** — `fetch` now sends `User-Agent: Serez-Code/<version>` unless the caller sets one in `headers`. Without it, ureq sends `ureq/x.y`, which some CDNs/WAFs answer with `503`; an identifiable UA avoids those spurious failures. A caller-provided `User-Agent` always wins. (`src/evaluator/builtins.rs`, `eval_fetch`.)

### JSON

- **`JSON.pretty(value, [indent])`** — pretty-prints values as indented JSON (default **2** spaces per level; `0` falls back to compact). When given a raw JSON string — such as a `fetch` response body — it parses it first and re-indents, so `JSON.pretty(fetch(url))` prints formatted output directly; non-JSON strings are kept as-is. `JSON.stringify` is unchanged (still compact, single-line). Implemented in `src/evaluator/mod.rs` (`json_pretty_owned` / `json_pretty_inner`) + `src/evaluator/namespaces.rs`.

### Docs

- Documented the `fetch` HTTP client (signature, default headers incl. the new `User-Agent`, options dict, `full`/`binary` modes, throw-on-4xx/5xx) and `JSON.pretty` in `README.md`.

### Fixes

- **`unit_native_fns.sz` parsing** — the POST test embedded a JSON body with an unescaped `{`, which serez treats as string-interpolation start. That silently aborted parsing of the rest of the file, so the POST test (and any added after it) never ran while the runner still reported the file as passing (parser errors go to stderr; the runner only greps stdout for `[FAIL]`). Escaped as `\{` so the whole file parses and executes.
- **`43_fetch_full_e2e` flakiness** — the test hit httpbin.org, which intermittently returns 503; since `full` mode does not throw on HTTP status, a 503 left `status="unknown"` and the test failed. Switched the endpoint to PokeAPI (`/api/v2/pokemon/ditto`) — a stable, CDN-backed service that consistently returns 200 — and tightened the assertions to check the *real* response (`status == 200`, `ok == true`, `statusText`/`headers` present, body contains `ditto`), so it actually exercises status-line/header/body parsing. Still degrades gracefully (`network_error`) on a genuine outage.

### Test count

- 310 passing (0 failing) — added `unit_json_pretty` (10 `JSON.pretty` cases) and two `fetch` User-Agent tests in `unit_native_fns`.

---

## [4.0.0] — branch `improve`

### Networking / stdlib

- **`fetch` is now a complete general-purpose HTTP client.** Previously `fetch(url, [method], [body])` always sent a hardcoded `Content-Type: application/json`, had a fixed 10 s timeout, threw on any status ≥ 400 (discarding the response body), only supported GET/POST/PUT/PATCH/DELETE, and corrupted binary responses via `from_utf8_lossy`. It now accepts an optional **options dict** after the url — `fetch(url, [method], [body], options)` — where `options` is a serez dict (e.g. `({"full", true})`):
  - `headers` — a `<string, string>` dict of request headers (enables `Authorization`, `Accept`, cookies, custom headers, …). Names/values containing control chars (`\n` `\r` `\0`) are rejected to prevent CRLF / header injection. A user-set `Content-Type` overrides the default (which is now only applied when a body is sent and the user didn't set one).
  - `timeout` — request timeout in seconds (default **60**, was 10; connect capped at 30).
  - `full` — when `true`, returns a `<string, any>` dict `{ status, ok, statusText, headers, body }` and does **not** throw on HTTP status, so 4xx/5xx (404, 429, 529, …) can be inspected. `headers` is a `<string, any>` dict keyed by lowercased name; a missing key reads as `null`.
  - `binary` — when `true`, the body is returned as a byte array `[int]` (0-255) instead of a UTF-8 string, so images / zips / PDFs download intact. Decode with `Binary.toUtf8` / `Binary.toHex`.
  - Default (no options) behaviour is unchanged: returns the body string and throws on status ≥ 400 — now with the response body embedded in the thrown message instead of just the status code.
  - Any HTTP method is accepted (incl. HEAD/OPTIONS) via `Agent::request`. Arguments are sniffed by type: the first string after the url is the method, the second is the body, and a dict is the options — so `fetch(url, opts)`, `fetch(url, "POST", opts)` and `fetch(url, "POST", body, opts)` all work. 100% backward compatible; `native fn` declarations are unaffected.
  - Implemented in `src/evaluator/builtins.rs` (`eval_fetch` + `fetch_make_value`).

### Test count

- 309 passing (0 failing) — added `43_fetch_full_e2e`, `44_fetch_binary_e2e`, `sec_fetch_header_injection`.

---

## [3.8.4] — branch `improve`

### Tooling / diagnostics

- **Arena stats** — `Evaluator::arena_stats()` returns the current object-slot counts of the two arenas `(global, scoped)`. When the program is run with the environment variable `SEREZ_ARENA_STATS` set, a line `[arena] global=N scoped=M` is printed to stderr at exit. Read-only diagnostic for measuring memory behaviour of the Region-Based Memory (e.g. confirming that scoped loops stay flat and which patterns promote to the never-freed global arena). **Not a GC and not an optimization** — zero runtime overhead unless the env var is set (a single `env::var` lookup at exit). Used to characterize the closure/escaping-container promotion-to-global behaviour (documented; the GUI memory discipline belongs to serez-ui, not the core).

---

## [3.8.3] — branch `improve`

### Bug fixes

- **B-84** — Parenthesized single-parameter arrow lambda failed to parse. `(x => body)` raised `Expected ')' in grouped expression`, even though `(x) => body`, bare `x => body`, and `(a, b) => body` all parsed. After consuming `(` and a leading identifier, the parser matched `,` (multi-param), `)` (`(a)`/`(a) => …`) and a catch-all that assumed a grouped expression — so a following `=>` (Arrow) was never recognized. Added an explicit `Arrow` arm that parses `( ident => body )` as a parenthesized single-param lambda. This unblocks common forms like `5 |> (x => x * 2)`, `((x => x + 1))(5)`, and `let f = (x => …)`. New regression: `unit_paren_lambda` (6 cases). Found while fuzzing pipe/lambda syntax.

### Test count

- 306 passing (0 failing) — added `unit_paren_lambda`.

---

## [3.8.2] — branch `improve`

### Bug fixes

- **B-83** — Inconsistent lambda capture: scope-dependent snapshot vs. live reference. Lambdas snapshot scoped locals (`capture_env` extracts + plants them to the global arena at creation), but variables referenced from a lambda that live in the **global** arena (top-level `let`s) were resolved *live* at call time. So the exact same lambda captured locals by value but globals by reference, depending only on where it was written: `let x=10; let f=()=>x; x=20; f()` gave `20` at top level but `10` inside a function; `while (i<3){ fns.push(()=>i); i=i+1 }` gave `3 3 3` at top level but `0 1 2` inside a function. Fixed with `capture_lambda_env`: in addition to the existing local snapshot, a best-effort free-identifier walk of the lambda body now also snapshots referenced **global data variables** at creation. Global **functions** are intentionally skipped (kept live) so recursion and late binding keep working. The walk only ever *adds* snapshots — an unhandled construct simply degrades to the previous live-lookup behavior, so it cannot break a valid closure (the whole suite is unchanged). New regression: `45_closure_capture_e2e`.

### Test count

- 305 passing (0 failing) — added `45_closure_capture_e2e`.

---

## [3.8.1] — branch `improve`

### Bug fixes

- **B-82** — Nested arrays corrupted when reassigning an outer-scope variable from inside a nested block. The shared scoped arena is a single stack rewound on block exit (`pop` → `reset_to`). A plain variable assignment (`x = value`) stored a *shallow* clone of the value's `ObjectData`: for an array/dict/set it copied the inner `ObjectRef`s, which could point into a deeper block's region. When that inner block popped, the inner refs dangled — the container's `.length()` stayed correct but indexing an element read a truncated/reused slot (symptom: `is array` == false, "Index operator not supported"). `push`/index-assign/dict-value-assign already promoted to the global arena at `depth > 1`, but plain variable assignment was missed. Fixed by `promote_container_for_assign`: when assigning a heap container (Array/Dict/Set) to a variable from inside a nested scope, the value is deep-promoted to the global arena so its elements outlive inner-block pops. Scalars and instances (fields are `OwnedValue`) are untouched — no effect on loop counters like `i = i + 1`. Found while building serez-ui's `.szs` CSS parser. New regression: `unit_nested_array_assign` (4 cases).

### Test count

- 303 passing (0 failing) — added `unit_nested_array_assign`.

---

## [2.1.0] — branch `improve`

### New features

**Fase 1 — Memory namespace: raw byte heap**

- `Memory` namespace: `sizeof`, `alloc`, `free`, `size`, `read`, `write`, `copy`, `fill`, `offsetOf`.
- `Memory.sizeof(type)` — returns byte-size of a primitive type name (`"int"`, `"bool"`, `"float32"`, etc.).
- `Memory.alloc(n)` → int handle — allocates `n` bytes of zeroed memory in a `HashMap<i64, Vec<u8>>` heap stored on the evaluator; requires `unsafe {}` block.
- `Memory.read(handle, offset, type)` / `Memory.write(handle, offset, type, value)` — typed read/write at a byte offset; require `unsafe {}`.
- `Memory.copy(src, dst, n)` — copies `n` bytes between two allocations; requires `unsafe {}`.
- `Memory.fill(handle, byte)` — fills an entire allocation with a byte value; requires `unsafe {}`.
- `Memory.offsetOf(class_name, field_name)` — returns word-aligned field offset (8-byte stride) by looking up the class registry.
- New evaluator fields: `memory_heap: HashMap<i64, Vec<u8>>`, `memory_heap_next_id: i64`.
- New source file: `src/evaluator/namespaces_memory.rs`.

**Fase 1.5 — unsafe as expression + new built-in globals**

- `unsafe { ... }` can now be used as an expression, enabling patterns like `let h = unsafe { Memory.alloc(64) }`. AST: `Expression::UnsafeBlock(BlockStatement)`. Parser: expression-level dispatch in `parse_expression`. Evaluator: delegates to `eval_unsafe_block`.
- `time()` built-in — returns current Unix timestamp in milliseconds as `int`.
- `env(name)` built-in — reads an environment variable by name; returns empty string if not set.
- `exit(code)` built-in — terminates the process with the given exit code (`std::process::exit`).
- `native fn` dispatch: when a declared native function is called but has no Rust implementation registered, a clear error is now printed.

**Fase 2 — Extended Tensor math**

- **Activation functions** (element-wise, return new Tensor): `relu`, `sigmoid`, `tanh`, `softmax`.
- **Element-wise math**: `abs`, `sqrt`, `exp`, `log`, `pow(exp)`.
- **Norms**: `norm()` (L2, default) / `norm(1)` (L1) — returns a Decimal.
- **Clamp**: `clamp(min, max)` — clips all elements to `[min, max]`.
- **Broadcast add**: `broadcastAdd(bias)` — adds a 1D tensor to each row of a 2D tensor `(m, n) + (n,)`.

### Bug fixes

- **B-75** — Keyword token as method name rejected by class parser: methods named `get`, `set`, or `static` (lexed as `KwGet`/`KwSet`/`KwStatic`) were unconditionally rejected by the `Ident`-only check in `parse_class_declaration`. Fixed by extracting `token_type_is_name()` helper and using `current_token_is_name()` at the method-name check point.
- **B-76** — `Tensor.sum()` on empty tensor returned `-0.0`: Rust's `Iterator::sum` initialises the accumulator with `0.0_f64` and produces negative zero on empty input. Fixed by adding an `is_empty()` early-return guard matching the pattern already used by `Tensor.mean()`.
- **B-65 assertion corrected** — `Math.round(-4.5)` returns `-5` (Rust "half away from zero"), not `-4`. Test expectation updated.
- **`unit_class_arch` assertion corrected** — `pts.find(p => p.sum() > 6)` returns the first match (x=3), not the last (x=5). Test expectation updated.

### New parser feature

- **Enum.Variant in match patterns** — `match dir { case Direction.North => ... }` now works. The parser detects `Ident.Ident` in match position and creates a `MatchPattern::Literal(DotCall)`, evaluated at runtime by the existing literal-pattern path.

### Test count

- 274 passing (0 failing) — added: `unit_memory`, `unit_native`, `unit_tensor_math`, `56_memory_e2e`, `57_tensor_math_e2e`, `unit_match_enum`, `unit_bug_b64_b74`, `unit_math_trig`, `unit_memory_offsetof`, `unit_tensor_ops`, `unit_set_ops` (extended), `unit_bug_b75_b76`, `unit_class_arch` (extended), `sec_memory_requires_unsafe`, `sec_memory_write_requires_unsafe`, `sec_memory_read_requires_unsafe`, `sec_memory_free_requires_unsafe`, `sec_json_invalid`, `59_integral2_e2e`.

---

## [2.0.2] — branch `improve`

### New features

**Fase 2.5 — serez-sec: Socket and Binary namespaces**

- `Socket` namespace: `connect`, `send`, `recv`, `close`, `listen`, `accept` — raw TCP over `std::net::TcpStream` / `TcpListener`. Socket IDs (int) stored in the evaluator's registry; usable from Serez code as `Socket.connect("host", port)`.
- `Binary` namespace: byte-array utilities — `fromHex`, `toHex`, `fromUtf8`, `toUtf8`, `packInt32Le`, `packInt32Be`, `unpackInt32Le`, `unpackInt32Be`, `packInt64Le`, `unpackInt64Le`, `concat`. All operate on Serez integer arrays (values 0–255).
- Tests: `tests/53_socket_e2e.sz`, `tests/unit_binary.sz`, `tests/unit_socket.sz` (42 new test cases).

**Fase 4 — GPU compute (CPU-backed)**

- `GPU` namespace: `createBuffer`, `createBufferFromArray`, `readBuffer`, `freeBuffer`, `fill`, `size`, `map`, `reduce`, `dot`, `axpy`, `matmul`. Buffers are flat `Vec<f64>` stored in the evaluator. API mirrors GPU compute patterns (create/upload/dispatch/readback/free) so a future backend can swap to real GPU calls with no language changes.
- Tests: `tests/54_gpu_e2e.sz`, `tests/unit_gpu.sz` (13 new test cases).

**Fase 6 — Package manager**

- `src/package_manager.rs`: `SerezManifest` JSON parser (hand-rolled, no external crate), `install_package(spec)`, `install_all()`, `packages_dir()` / `registry_dir()` (support `SEREZ_PACKAGES` / `SEREZ_REGISTRY` env vars for testing).
- `sz install [pkg@version]` CLI subcommand: without argument reads `serez.json` and installs all dependencies; with argument installs a specific package from the registry.
- Import resolution now searches `packages_dir()` (and falls back to `~/.serez/packages/`) after all existing search paths. Also supports `<pkg>/index.sz` layout so `import "pkg-name"` resolves to `packages/pkg-name/index.sz`.
- `run_tests.ps1` / `run_tests.sh`: set `SEREZ_PACKAGES=tests/packages` so package tests run correctly against local test packages.
- Tests: `tests/55_packages_e2e.sz`, `tests/unit_packages.sz` (13 new test cases). Test packages: `tests/packages/math-helpers/`, `tests/packages/string-tools/`.
- Rust unit tests in `package_manager.rs` verify manifest parsing and pkg-spec parsing.

### Test count

- 214 → 256 passing (0 failing).

---

## [2.0.1] — branch `improve`

### Bug fixes

**B-64 — `abs(i64::MIN)` overflow** (`src/evaluator/builtins.rs`)
- Before: called `.abs()` on `i64::MIN` — overflows in release mode (|i64::MIN| > i64::MAX).
- Now: uses `i64::checked_abs()` — returns an error for `i64::MIN`.

**B-65 — `floor` / `ceil` / `round` / `trunc` UB on non-finite f64** (`src/evaluator/builtins.rs`)
- Before: casting `f64::INFINITY`, `f64::NEG_INFINITY`, or `f64::NAN` to `i64` via `as i64` is undefined behavior in Rust.
- Now: each function validates `!v.is_nan() && !v.is_infinite()` before casting.

**B-66 — `Math.random()` only produced values in `[0, ~0.5)`** (`src/evaluator/namespaces.rs`)
- Before: LCG state shifted right 33 bits (31-bit range `[0, 2³¹)`) divided by `u32::MAX` (2³²−1) — maximum ≈ 0.5.
- Now: divides by `1u64 << 31` to produce the documented `[0, 1.0)` range.

**B-67 — `asin` / `acos` accepted out-of-domain arguments** (`src/evaluator/builtins.rs`)
- Before: any `f64` was accepted — inputs outside `[-1, 1]` silently produced `NaN`.
- Now: validates `v >= -1.0 && v <= 1.0` before calling the intrinsic.

**B-68 — `JSON.stringify` emitted invalid JSON for `NaN` / `Infinity`** (`src/evaluator/mod.rs`)
- Before: non-finite `f64` values were formatted with Rust's `Display`, producing `"inf"`, `"-inf"`, or `"NaN"`.
- Now: `if !d.is_finite() { return "null".to_string(); }` per the JSON specification.

**B-69 — `call_function` (map / filter / sort callbacks) rejected default and rest parameters** (`src/evaluator/mod.rs`)
- Before: arity checked as `arg_count != params.len()` and parameters bound via `args[i]` direct indexing.
- Now: computes `required_count`, checks `arg_count >= required` with upper bound for non-rest, binds defaults and collects rest parameter into an array.

**B-70 — `min_params` formula wrong for functions with default + rest parameters** (`src/evaluator/expr.rs`)
- Before: `if has_rest { params.len() - 1 } else { required_count }` — gives wrong count when both rest and defaults are present.
- Now: `let min_params = required_count` in all cases.

**B-71 — `super()` constructor call rejected default and rest parameters** (`src/evaluator/classes.rs`)
- Before: `eval_super_call` used strict arity and `args[i]` direct indexing.
- Now: same default/rest parameter handling as `call_function`.

**B-72 — `new ClassName()` constructor call rejected default and rest parameters** (`src/evaluator/classes.rs`)
- Before: `eval_new_class` used strict arity and direct indexing for constructor binding.
- Now: same default/rest parameter handling.

**B-73 — `super.method()` call rejected default and rest parameters** (`src/evaluator/classes.rs`)
- Before: `eval_super_method_call` used strict arity.
- Now: same default/rest parameter handling.

**B-74 — `invoke_method` rest parameter not collected** (`src/evaluator/classes.rs`)
- Before: parameter binding loop did not handle rest parameters — extra arguments beyond the last named param were silently discarded.
- Now: rest parameter is collected from `args[i..]` into an `Array` and declared in scope.

### Version

- `Cargo.toml`: `2.0.0` → `2.0.1`

---

## [2.0.0] — branch `improve`

### Breaking changes

**`pop()` on empty array is now a runtime error (Bug 1)**
- Before: returned `null` silently
- Now: `❌ ERROR: pop() called on an empty array`
- Rationale: silent null masked logic bugs where callers expected a real value

**`shift()` on empty array is now a runtime error (Bug 2)**
- Before: returned `null` silently
- Now: `❌ ERROR: shift() called on an empty array`
- Rationale: same as pop() — silent null was undetectable

**`2 ** 63` and exponent overflow are now runtime errors (Bug 3)**
- Before: f64 precision caused `2 ** 63` to silently return `i64::MAX` instead of detecting overflow
- Now: uses `i64::checked_pow` — exact overflow detection with no floating-point rounding
- Now: `❌ ERROR: Integer overflow in exponentiation`
- Base 0, 1, -1 at any exponent are still handled correctly (no overflow possible)
- Decimal exponent path (`2 ** 63.0`) is unchanged — goes through `f64::powf`

**Typed dict missing key is now a runtime error (Bug 4)**
- Before: `d["missing"]` on a `<K, V>` dict (V ≠ `any`) silently returned `null`
- Now: `❌ ERROR: Key 'missing' not found in typed dict <_, V>`
- Untyped dicts (`<K, any>`) still return `null` for missing keys — no change

### Distribution

- **Release pipeline**: GitHub Actions workflow builds binaries for Windows x64, Linux x64 (static musl), macOS ARM64, macOS x64 on every version tag and publishes them to GitHub Releases
- **`install.sh`**: one-line installer for Linux and macOS — auto-detects OS and arch, installs to `~/.local/bin/sz`
- **`install.ps1`**: one-line installer for Windows — downloads to `%LOCALAPPDATA%\SerezCode\bin\sz.exe` and adds to user PATH
- **CI workflow** (`ci.yml`): builds on `main` and `integration` on every push and pull request

### Tests (214 total, 0 failures)

- `41_bug_fixes_e2e.sz` — E2E integration test covering all 4 bug fixes (Queue, SafeStack, safePow2, Registry, game loop)
- `unit_bug_fixes.sz` — 21 unit tests for positive regression across all 4 fixes
- `sec_pop_empty_array.sz`, `sec_shift_empty_array.sz`, `sec_typed_dict_miss_key.sz`, `sec_power_2_63.sz` — security tests verifying each fix produces the correct error
- `unit_sec_pentest_bugs.sz` — 16 penetration tests with boundary exhaustion, alternating cycles, power edge cases, dict key patterns
- `run_tests.ps1` — new `-cli` flag runs 12 tests covering CLI flags (`--version`, unknown flags, non-.sz), REPL behavior (arithmetic, variable persistence, function definition, error recovery), and `--check` mode output

### Native backend (foundation — not yet connected to runtime)

- `src/compiler/types.rs` — compile-time type system (`SzType`) mapping Serez types to LLVM types
- `src/compiler/hir.rs` + `hir_lower.rs` — AST → HIR lowering with full desugar pass
- `src/compiler/mir.rs` + `mir_lower.rs` — HIR → MIR three-address code with basic blocks
- `src/compiler/llvm_emit.rs` — MIR → LLVM IR text emission (74 tests passing)

---

## [1.0.0] — VS Code formatter and CI

### VS Code — Formatter (`vscode-serez` v0.2.0)

**`extension.js`** — new `DocumentFormattingEditProvider`:
- Auto-indentation with 4 spaces per level, based on `{` and `}` counting
- Ignores braces inside string literals and line comments (`//`)
- `} else {` handled correctly: dedent before printing, indent after
- Collapses consecutive blank lines into one
- Removes trailing whitespace from all lines
- File always ends with exactly one `\n`

**`package.json`** — version `0.2.0`:
- `"main": "./extension.js"` and `"activationEvents": ["onLanguage:serez"]`
- `Formatters` category added
- `configurationDefaults` for `.sz`: `editor.defaultFormatter` and `editor.formatOnSave: true` enabled automatically

**Usage:** `Shift+Alt+F` to format manually, or save the file (formatOnSave).  
**Rebuild:** `vsce package` in `vscode-serez/` generates `serez-code-0.2.0.vsix`.

---

### CI / Tooling
- `release.yml`: permissions scoped per job — only `host` has `contents: write`; others have `contents: read`
- `.github/dependabot.yml`: automatic weekly updates for GitHub Actions and Cargo dependencies
- `run_tests.sh`: Bash script equivalent to `run_tests.ps1`, with `--filter`, `--generate`, `--unit`, `--e2e`, `--security` flags; ANSI colors; CRLF normalization; unique temp files per process
- Evaluator refactored from a single `evaluator.rs` (5300+ lines) to 12 submodules:

| Module | Responsibility |
|---|---|
| `mod.rs` | Main entry, Flash Scope protocol, StoredMethod cache, static profiler |
| `stmt.rs` | Statement evaluation (let, assign, for, while, return, …) |
| `expr.rs` | Expression evaluation (calls, index, dot, ternary, …) |
| `ops.rs` | Infix and prefix operators |
| `check.rs` | Type-check helpers (parameters, return, typed arrays) |
| `builtins.rs` | Global functions (parseInt, parseDecimal, readLine, …) |
| `classes.rs` | Instantiation, method dispatch, inheritance, super |
| `methods_array.rs` | Array methods (push, pop, map, filter, reduce, sort, …) |
| `methods_string.rs` | String methods (split, replace, trim, padStart, …) |
| `methods_set.rs` | Set methods (add, has, delete, toArray, union, …) |
| `namespaces.rs` | Built-in namespaces (Math, File, JSON) |
| `control.rs` | Control flow helpers (break, continue, labeled loops, do-while) |

### Demo apps
- `apps/01_task_manager.sz` — enum, inheritance, static methods, switch, HOF, try/catch
- `apps/02_statistics.sz` — typed arrays, Math, map/filter/reduce, Pearson correlation
- `apps/03_text_analyzer.sz` — string methods, dicts, Caesar cipher, File I/O
- `apps/04_bank_system.sz` — abstract class, sealed, interface, const, getters, optional chaining
- `apps/05_data_pipeline.sz` — JSON, File, Set, bitwise/power ops, pipeline HOF

---

## [0.1.0] — Language history

### Phase 5 — Bug fixes and semantics (B-62 to B-63)

**`reverse()` — in-place mutation with return (B-62)**
- Before: `reverse()` returned void, was not chainable
- Now: mutates the array in-place AND returns the same array — allows `let sorted = arr.reverse()`

**`trimLeft` / `trimRight` as aliases (B-63)**
- Added as aliases for `trimStart` / `trimEnd` for compatibility

---

### Phase 4 — Critical bug fixes (B-54 to B-61)

**`is` operator — full fix (B-61)**
- Bug: `is` was tokenized as an identifier, never worked as an infix operator
- Fix: `KwIs` token added; registered in `token_precedence()` and in the parser's `is_infix` match; `eval_infix` handler added in the evaluator
- `null is null` also fixed: missing case `("null", ObjectData::Null)` in `type_matches`

**Named function capture semantics (B-58)**
- Before: `fn` declarations captured the value at definition time (snapshot)
- Now: `fn` declarations use reference semantics — rebind of the shared global slot
- Lambdas maintain snapshot semantics (no changes)
- `ScopeStack::rebind()` added for selective rebinding of outer scope

**Dict mutation from nested scope (B-57)**
- Bug: arena lifetime — a new entry in a dict mutated from inside a function stayed in the local scope and was destroyed on exit
- Fix: `plant_global` used when `depth > 1`

**`padStart` / `padEnd` — incorrect early return (B-56)**
- Bug: if the string already had the target length, it returned empty instead of returning the original string
- Fix: early return corrected

**Shift validation (B-55)**
- `1 << 64` and `8 >> -1` were silently incorrect
- Now they are runtime errors: negative or ≥ 64 shift throws an error

**`flat(n)` — depth parameter (B-54)**
- Before: only supported `flat()` with depth 1
- Now: `flat(n)` recursively flattens `n` levels; `flat()` is equivalent to `flat(1)`

**Getter-only — write error (B-53)**
- Attempting to assign to a property that only has `get` (without `set`) is now a runtime error

---

### Phase 3 — New language features

#### Operators

**Power operator `**`**
- `2 ** 10` → `1024`; works with `int` and `decimal`
- Higher precedence than `*` / `/` / `%`
- `0 ** 0` → `1` (mathematical convention)

**Bitwise operators**
- `&` AND, `|` OR, `^` XOR, `~` NOT (prefix), `<<` left shift, `>>` arithmetic right shift
- Only for `int` (64-bit signed, two's complement)
- Negative or ≥ 64 shift is a runtime error
- Binary (`0b1010`) and hexadecimal (`0xFF`) literals supported
- Numeric separators: `1_000_000`, `0xFF_FF`

**Optional chaining `?.`**
- `obj?.method()` / `obj?.field` — if `obj` is `null`, returns `null` without error
- Chainable: `a?.getNext()?.getValue() ?? 0`
- Combinable with `??` for fallback

#### Control flow

**`do-while`**
- The body executes at least once
- `break` and `continue` work the same as in `while`/`for`

#### Classes

**Static methods**
- `public static T method(args)` in classes
- Called as `ClassName.method(args)` — no instance required
- No access to `this`

**Parameters with default values**
- `fn int add(int a, int b = 10)` — if the caller omits the argument, the default is used
- The default is an arbitrary expression evaluated at call time
- The type checker handles variable arity (skip if there are defaults)

**Abstract classes**
- `abstract class Foo` — not directly instantiable; runtime error on `new`
- Methods without a body declared for override in subclasses

**Sealed classes**
- `sealed class Foo` — not inheritable; attempting to extend it is a runtime error

**Getters and setters**
- `public get T prop()` — called automatically when reading `obj.prop` (without parentheses)
- `public set prop(T val)` — called automatically when assigning `obj.prop = val`
- Property with only getter is read-only; writing to it is a runtime error

**Class fields with default values**
- `field: type = value` in the class body

#### Arrays — new methods

| Method | Description |
|---|---|
| `.find(cb)` | First element where `cb` returns `true`, or `null` |
| `.findIndex(cb)` | Index of the first element matching the predicate, or `-1` |
| `.every(cb)` | `true` if `cb` is `true` for all elements |
| `.some(cb)` | `true` if `cb` is `true` for at least one |
| `.slice(start, end)` | New array from `start` (inclusive) to `end` (exclusive) |
| `.flat(n?)` | Flattens `n` nesting levels (default 1) |
| `.reverse()` | Reverses in-place, returns the same array |
| `.indexOf(val)` | Index of the first occurrence, or `-1` |
| `.includes(val)` | `true` if the array contains the value |
| `.remove(idx)` | Removes and returns the element at `idx` |

#### Strings — new methods

| Method | Description |
|---|---|
| `.padStart(n, ch?)` | Pads the start with `ch` (default space) up to length `n` |
| `.padEnd(n, ch?)` | Pads the end with `ch` (default space) up to length `n` |
| `.slice(start, end?)` | Substring with negative index support |
| `.trimStart()` / `.trimLeft()` | Removes leading whitespace |
| `.trimEnd()` / `.trimRight()` | Removes trailing whitespace |
| `.toUpperCase()` / `.upper()` | Uppercase copy |
| `.toLowerCase()` / `.lower()` | Lowercase copy |
| `.startsWith(prefix)` | `true` if the string starts with `prefix` |
| `.endsWith(suffix)` | `true` if the string ends with `suffix` |
| `.charAt(i)` | Character at position `i`, or `""` if out of range |
| `.indexOf(sub)` | Index of first occurrence of `sub`, or `-1` |
| `.replace(from, to)` | Replaces **all** occurrences (previously only the first) |

---

### Phase 2 — Stdlib and compound types

#### `const`
- `const PI = 3.14159` — immutable; any reassignment is a runtime error
- Same scoping as `let` — invisible outside its block

#### `enum`
- `enum Color { Red, Green, Blue }` — variants accessed as `Color.Red`
- Variants are their own type (not `string`) — do not annotate enum parameters as `string`
- Comparable with `==` and usable in `switch case`
- Displayed as `"Color.Red"` (fully qualified name)

#### Labeled loops
- `outer: for (...)` + `break outer` / `continue outer`
- Works with `while`, `for`, `for-in`, `do-while`

#### Spread and rest
- Spread in array literals: `[...arr, 1, 2]`
- Spread in calls: `fn(...args)`
- Rest params: `fn void log(...args)` — `args` is an array with all extra arguments
- The type checker skips arity checks for functions with rest params

#### Namespace `Math`

| Function/Constant | Description |
|---|---|
| `Math.PI`, `Math.E` | Mathematical constants |
| `Math.abs(x)` | Absolute value |
| `Math.floor(x)`, `Math.ceil(x)`, `Math.round(x)`, `Math.trunc(x)` | Rounding (return `int`) |
| `Math.sqrt(x)` | Square root |
| `Math.pow(base, exp)` | Power |
| `Math.exp(x)`, `Math.log(x)`, `Math.log2(x)`, `Math.log10(x)` | Exponential and logarithms |
| `Math.sin(x)`, `Math.cos(x)`, `Math.tan(x)` | Trigonometric (radians) |
| `Math.asin(x)`, `Math.acos(x)`, `Math.atan(x)`, `Math.atan2(y, x)` | Inverse trigonometric |
| `Math.min(a, b, ...)`, `Math.max(a, b, ...)` | Variadic min/max |
| `Math.clamp(x, min, max)` | Clamp to range `[min, max]` |
| `Math.sign(x)` | Returns `1`, `0`, or `-1` |
| `Math.random()` | Pseudo-random decimal in `[0, 1)` (LCG) |

#### Namespace `File`

| Function | Description |
|---|---|
| `File.exists(path)` | `true` if the file exists |
| `File.read(path)` | File contents as `string` |
| `File.write(path, content)` | Writes/overwrites the file |
| `File.create(path)` | Creates empty file if not exists (touch, idempotent) |
| `File.read_asBinary(path)` | File bytes as `[int]` (0–255 each) |
| `File.write_asBinary(path, bytes)` | Writes byte array to file |

#### Namespace `JSON`

| Function | Description |
|---|---|
| `JSON.stringify(value)` | Serializes any value to a JSON string |
| `JSON.parse(string)` | Parses a JSON string; runtime error if invalid |

#### `Set` type

| Method/property | Description |
|---|---|
| `new Set()`, `new Set([...])` | Creates empty set or initialized from array (no duplicates) |
| `.size` | Element count (property, without parentheses) |
| `.add(val)` | Inserts `val` if not present (mutates in-place) |
| `.has(val)` / `.contains(val)` | `true` if the set contains `val` |
| `.delete(val)` / `.remove(val)` | Removes `val`, returns `true` if it existed |
| `.clear()` | Removes all elements |
| `.toArray()` | Returns all elements as an array |
| `.union(other)` | New set with all elements from both |
| `.intersection(other)` | New set with only elements present in both |

---

### Phase 1 — Language core

#### Variables and types
- `let x = value` — declaration; `x = value` — reassignment (without `let`)
- Primitive types: `int` (i64), `decimal` (f64), `bool`, `string`, `void`, `any`, `null`
- Compound types: array `[T]`, dict `<K,V>`, function, interface, class instance
- Nullable types: `int?`, `string?` — accept the base type or `null`
- Typed arrays: `let nums [int] = [1, 2, 3]` — type enforced on push, unshift, index-assign
- Type inference: `let x = add(1, 2)` infers `x: int` in the static checker

#### Operators
- Arithmetic: `+`, `-`, `*`, `/` (integer, truncates), `%`
- Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Logical: `&&`, `||`, `!` (short-circuit)
- Ternary: `cond ? then : else` (lazy, right-associative)
- Null coalescing: `a ?? b`
- `is`: `expr is TypeName` — `true`/`false` at runtime
- Compound assignment: `+=`, `-=`, `*=`, `/=`, `%=`
- Increment/decrement: `++`, `--` (prefix and postfix, as statements only)
- String repetition: `"ha" * 3` → `"hahaha"`
- Concatenation: `"x" + 42` → `"x42"`

#### Runtime safety
- Integer overflow: `checked_*` — error instead of silent wrap
- Division/modulus by zero: runtime error
- Out-of-range index: runtime error
- Undeclared variable: runtime error
- `return` outside a function: runtime error
- Stack overflow: runtime error (not catchable via try/catch)

#### Functions
- Declared: `fn returnType name(type param) { ... }`
- Arrow: `let f = returnType (type param) => { ... }`
- Anonymous: `let f = fn void () { ... }`
- First-class: assignable to variables, passable as arguments
- Recursive: supported with call stack in errors
- Lexical closures: capture variables from the scope where they are defined
- `fn` declarations: reference semantics (rebind of global slot)
- Lambdas (`x => expr`): snapshot semantics (capture by value)

#### Control flow
- `if` / `else if` / `else` — condition in parentheses, braces required
- `while` — condition in parentheses
- `for` — `for (let i = 0; i < n; i++)` — update accepts `i++`, `i--`, `i+=n`, etc.
- `for-in` — `for (let x in arr)` iterates array or string; `x` is a copy of the element
- `break` / `continue` — in all loops
- `switch` — no fall-through; `case a, b:` for multiple values; `default:`
- `try` / `catch(e)` / `finally` — `finally` always runs; `throw` accepts any value
- Standalone blocks `{ ... }` — create new Flash Scope

#### Arrays
- Literals: `[1, 2, 3]`, `[]`
- Index access: `arr[i]` (0-based)
- Index mutation: `arr[i] = val`
- Global mutation from function: `data[i] = val` persists; `this.arr[i] = val` persists
- **Limitation**: `for-in` creates a copy — mutating the loop variable does not affect the original array
- Mutation methods: `.push`, `.pop`, `.shift`, `.unshift`, `.reverse`, `.sort`, `.sort("desc")`, `.sort((a,b) => ...)`
- Query methods: `.length`, `.join`, `.map`, `.filter`, `.reduce`

#### Strings
- Interpolation: `"Hello {name}!"` — supports complex expressions inside `{}`
- `\{` for literal brace; `\"` inside `{...}` breaks the parser (use a variable)
- Escape sequences: `\n`, `\t`, `\r`, `\\`, `\"`, `\{`
- Methods: `.length`, `.substring`, `.split`, `.replace`, `.includes`, `.trim`, `.toString()`

#### Dictionaries
- `let d <string,int> = ({"a",1},{"b",2})`
- Access: `d["key"]` — returns `null` if the key does not exist (no error)
- Write: `d["key"] = val` or `d.Add({"key",val})`
- Methods: `.Add`, `.Remove`, `.RemoveAll`, `.clear`, `.toList`, `.toArray`

#### Classes and interfaces
- `interface Point { x: decimal, y: decimal }` — typed field record, no methods
- `class Foo { public Foo(args) { ... } }` — constructor + fields + methods
- Single inheritance: `class Bar : Foo { ... }`, `super(args)` in constructor
- `public` / `private` — `private` only accessible from methods of the same class
- Instance: `let obj = new Foo(args)`
- Field mutation: `obj.field = val`
- **Limitation**: `this.field[i].method()` inside a class method creates a copy — the result does not persist; use `this.field[i] = newValue` instead

#### Conversions and I/O
- `parseInt(val)` — converts to `int` (string, decimal, int)
- `parseDecimal(val)` — converts to `decimal` (string, int, decimal)
- `readLine(prompt?)` — reads a line from stdin
- `out expr` — prints to stdout with newline; statement, not function

#### Memory — Flash Scopes
- Two arenas: global (entire program) and scoped (local per block)
- Each `{ }` records a watermark on entry and truncates on exit — O(k) per scope
- Return values extracted as `OwnedValue` before the pop and replanted in the parent scope
- `Rc<BlockStatement>` for function bodies — cloning a function is O(1)
- `StoredMethod` in classes — O(1) dispatch without cloning the method body

#### Tooling
- `sz script.sz` — execute file
- `sz` — REPL
- `sz --check script.sz` — static profiler (byte estimation per function)
- `sz --watch script.sz` — automatic rerun on save
- `sz --version` — version
- Span errors: line + column + caret `^` in source
- VS Code extension: syntax highlighting for `.sz`
