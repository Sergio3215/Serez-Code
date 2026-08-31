# Serez Code maturity audit

Status: initial repository-wide audit, 2026-08-25.

This document is a living engineering record. It separates observed behavior from
the desired architecture and keeps unresolved debt visible. A finding is not
considered fixed until implementation, tests and public documentation agree.

## Evidence and baseline

The audit inspected the Rust frontend, evaluator/runtime, optional compiler, CLI,
REPL, LSP, package manager, native namespaces, CI/release workflows, documentation,
benchmarks and the local official-package repositories.

Current verified baseline on Windows (re-measured 2026-08-28):

| Gate | Result |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo check` | PASS, no warnings |
| `cargo clippy --all-targets` | PASS, 190 historical library warnings, no errors |
| `cargo test --all-targets` | PASS, 305 tests (170 library, 36 LSP binary, 16 frontend robustness, 76 runtime outcome, 2 diagnostic codes, 4 filesystem reach) |
| Serez test runner | PASS on **both** platforms, 488 files/groups each; 0 failed; 0 skipped. `run_tests.sh` had been executing 306 of them — see the parity row below. |
| Official ecosystem (`run_ecosystem.ps1`) | PASS, 8/8 packages: UI 36/36, HTTP 3/3, AI 3/3, AgentAI 3/3, pack 3/3, apipack 3/3, dotenv 2/2, graph 3/3 |

Platform parity is now measured rather than assumed. `run_tests.sh` reported
306 passed / 168 failed against `run_tests.ps1`'s 474 / 0 — not a behavioral
difference in the language, but every unit test on the bash side failing to
start. Each unit file runs from a temp file named `~unit_temp_$$.sz`, and
MSYS2 declines to rewrite a POSIX path into a Windows path when the final
component begins with `~`, so `sz` received a literal `/e/...` it could not
open. Two things kept it invisible: the bash runner printed stderr for a
failing E2E test but not for a failing unit test, so the whole report was
"process exited with code 1"; and the runner's own integrity guard — the one
check whose job is to catch a suite passing for the wrong reason — accepted any
non-zero exit with no summary, which a file-read error satisfies exactly. All
three are fixed: paths are converted with `cygpath` explicitly, the unit branch
prints stderr like the E2E branch always did, and both guards now require the
fixture's own `SZ4004` diagnostic, so they can only pass by actually running the
program. Verified in both directions — with the conversion reverted the guard
fails and names the cause.

The ecosystem row is the compatibility evidence for the frontend depth ceiling
introduced below: every official package still parses, type-checks and runs
unchanged against it.

The Rust codebase contains approximately 50,860 lines across 60 files. The largest
files are `namespaces_gui.rs` (6,036), `parser.rs` (3,880),
`methods_tensor.rs` (3,653), `namespaces_autodiff.rs` (2,929),
`namespaces_gui/render.rs` (2,527), `package_manager.rs` (2,311) and
`evaluator/mod.rs` (2,215).

## Severity model

- **critical**: plausible crash, corruption, privilege/boundary failure or silent
  miscompilation affecting supported use.
- **high**: major correctness, compatibility, security-contract or quality-gate
  risk.
- **medium**: maintainability, consistency, coverage or developer-experience debt
  likely to cause future defects.
- **low**: localized cleanup or presentation issue with limited operational risk.

## Findings by subsystem

| Area | Severity | Classification | Evidence / impact | Required direction |
|---|---|---|---|---|
| Lexer | medium | bug/DX fixed, semantic contract | `spec/lexical-grammar.md` now freezes token/Unicode behavior and `SZ1001`–`SZ1004`. Regressions cover incomplete tokens, embedded NUL, base-integer corruption and comment-stack exhaustion; CLI and LSP consume the same coded shape. | Fuzz Unicode boundaries and decide identifier normalization plus numeric-separator strictness through an explicit compatibility process. |
| Parser | high | architectural debt, diagnostics | The large parser now exposes coded positional errors through one CLI/LSP shape, and recovery is bounded, but recovery rules remain ad hoc and many AST nodes still lack uniform spans/notes. | Introduce a common frontend `Diagnostic` type and uniform AST spans incrementally while preserving current text and codes. |
| Frontend robustness | critical | bug (fixed) | **Resolved.** Ordinary source killed the process with no diagnostic — no message, no span, no CLI-chosen exit code. Two shapes reached it: nesting (`((((…))))`, one parser stack frame per level, measured crash at 32k–50k levels) and operator chains (`1+1+1+…`, which parse in a flat loop but leave a tree one level deeper per operator for the type checker, the evaluator and the AST's drop glue to recurse over — crash at ~32k terms evaluated, ~1M type-checked). `MAX_PARSE_DEPTH` now bounds AST depth for both shapes, charging one level per operator in a chain. | Keep the ceiling covered by `tests/frontend_robustness.rs` and the two `err_parse_depth_*.sz` conformance tests. Making infix evaluation iterative would remove the limit rather than bound it, and remains the better long-term fix. |
| AST | medium | architectural debt | One broad AST is shared by interpreter, tooling and an incomplete compiler. Nodes carry inconsistent source positions. | Add spans uniformly before changing syntax; keep AST source-oriented and lower to separate semantic IR. |
| Type checker | high | semantic inconsistency; two bugs fixed, contract written | The checker is intentionally partial and runtime checks remain authoritative. CLI/LSP can therefore describe a program differently from execution. `spec/types.md` now states exactly how partial: it performs four checks (array-literal elements, call arity, call parameter types, `return` type), can only infer types for literals, top-level `let` bindings, calls to declared functions and annotated array literals, and is **advisory** — findings print `SZ3000` to stderr and change neither the exit code nor whether the program runs, `--check` included. The same mistake is caught statically for a top-level binding and only at runtime for a function-local one. **Resolved:** a constructor checked argument *arity* but never argument *types*, so `new Point("x")` bound a string into an `int` field and failed only wherever that field was later used as a number — the one entry point into an object where annotations were decorative. **Resolved:** `type_matches` had no arm for an enum variant, so a `Priority`-typed parameter rejected `Priority.Low` saying "expected 'Priority' but received 'Priority'", and `x is Priority` was false; enum-typed parameters were unusable. Both fixes measured against the ecosystem before keeping: 473/473, canary 8/8, serez-cobol 23/23, serez-strike 113/113. | Pinned by `tests/unit_types.sz` (13 cases) and four tests in `runtime_outcome.rs`. Still open, each recorded in `spec/types.md` rather than smoothed over: numeric types do not widen at a parameter although arithmetic mixes them; a declared class or interface name matches **exactly** and never a subclass, so a hierarchy can only cross a call as `any`; an unknown type name is accepted and then matches nothing; `[T]` on a parameter accepts any array; declared class-field types are defaults, not constraints. Differential tests between `--check` and execution, and a decision on whether `--check` should fail on type errors, remain to be done. |
| Evaluator | high | architectural problem; expression diagnostics fixed, first extraction done | `Evaluator` owns arenas, modules, permissions, sockets, GPU buffers, raw memory, autodiff, GUI, processes, audio and tasks in addition to language semantics. **Partly resolved:** `expr.rs` no longer prints any diagnostic — the fourteen core expression failures a normal program hits (typed parameter, declared return, array/dict literal element types, entry literal and object patch position, dot call on a method-less type, enum variant, `&` and `*`) are structured, coded and catchable, and a typed-parameter failure now carries its call stack in the payload instead of printing it as a side effect. | Extract service-owned state incrementally behind runtime interfaces; do not rewrite evaluation. `mod.rs`, `stmt.rs`, `classes.rs` and `methods_dec.rs` still hold unmigrated producers.  **"Unmigrated producers" measured, and it does not mean what the phrase suggests.** Zero sites originate an unstructured failure — established earlier in this cycle by classifying all 138 constructions of the sentinel. What remains is coarseness, not absence: `SZ4000` is carried by three kinds, `Overflow`, `RangeError` and `RuntimeError`, and the registry named only two. **One real inconsistency came out of it.** A wrong argument count on any `Regex.*` method reported the generic `RuntimeError` / `SZ4000`, so a caller matching on kind could not tell "called wrongly" from any other runtime failure — while `arrays.md`, `dicts.md`, `random.md`, `sets.md`, `strings.md` and `tasks.md` all state the `TypeError` / `SZ4002` rule normatively. Regex was the only holdout, exactly the shape `errors.md` records for `Set`'s unknown member, and fixed the same way. Safe to change rather than a breaking one: there is no `spec/regex.md`, so the behaviour was unspecified and therefore unstable by `compatibility.md`'s own rule, and no official package calls `Regex.` at all. The three failures still on the generic kind — a malformed pattern, `parseInt`/`parseDecimal` given non-numeric text — stay there deliberately: there is no `SyntaxError` kind to move a bad pattern to, and a string whose *content* is not numeric is a value problem rather than a type one. Both are now in `errors.md` as a table of what shares the bucket, and pinned by three cases in `unit_regex.sz`. |
| Control flow | medium | semantic debt | `EvalResult` mixes normal values, return/break/continue, user throw and an untyped `Error` sentinel. | Separate internal control flow from structured runtime failure without changing catch behavior.  **The coverage boundary is closed.** `control-flow.md` froze only `for-in` and listed `if`, `while`, C-style `for`, `do-while`, `switch`, `match`, generators, `try/catch/finally` and labelled flow as unaudited. All are now probed against the binary and frozen. Everything held except two things nothing had written down, both recorded as hazards rather than smoothed over. **`match` is not exhaustive-checked:** a subject matching no arm evaluates to `null` with no diagnostic and exit 0, indistinguishable from an arm that legitimately returned null. **`fn*` is not lazy:** calling a generator runs the body to completion and returns an ordinary array, so an unbounded generator never returns — measured, no result after 20 seconds — and nothing bounds the collected values, since the collector is an unbounded vector and `limits.md` has no entry for it. Also frozen, all measured: `finally` runs on every exit including `break`, `continue` and `return`; a `throw` from a `finally` replaces the failure in flight and a `return` in one overrides the try's; `catch` binds the thrown value for a user `throw` and an `Error` object for a runtime error, and never runs for a fatal failure; `switch` has no fallthrough and a `default` written first does not pre-empt a matching case below it. Pinned by `tests/unit_control_flow_contract.sz` (11 cases) — which covers what the running suite did not: switch fallthrough had only ever been checked in `tests/_bug_hunt10.sz`, a file the runner does not glob. |
| Runtime errors | high | bug risk, DX, architecture | Structured errors preserve `code`, `kind`, `message`, `span`, `stack`, and `notes`; an internal recoverability bit prevents structured security/resource failures from becoming catchable. Complete programs return `ProgramOutcome`, and `run_source_detailed` exposes the same information without breaking the exit-only API. Direct operators, exact decimals, scalar Math, spread/iteration/destructuring operands, default evaluation, construction/`super`/member/property/inheritance/DateTime/Random/String/Task validation, permission/unsafe gates and the audited resource/security ceilings now preserve stable structured diagnostics. Internally, 176 textual `EvalResult::Error` producer/propagation occurrences across 169 source lines and mutable pending-error/`try_depth` state remain across other subsystems. This is an inventory, not a monotonic quality metric: explicit propagation can legitimately add occurrences while silent fallbacks are removed. | Migrate producers and then the internal carrier in tested slices; preserve the boundary and fatal/catchable distinction while eliminating the side channel. See `spec/errors.md`.  **Measured, not assumed:** `errors.md` said class `super`/dispatch paths and other native subsystems still produced unstructured failures. They no longer do. Thirty-one language constructs — construction, method and accessor bodies, `super` both ways, operator overloads, native callbacks, generators, `match`, destructuring, nested writes, pipes, a failing default argument — plus 894 hostile calls across twelve native namespaces produced **zero** unstructured outcomes. The thirty-four remaining `return EvalResult::Error` sites all *propagate* a failure already recorded structured further in; none originates one. The variant stays in `ProgramOutcome` because embedders must still be able to receive it. Pinned by `no_reachable_construct_produces_an_unstructured_outcome`.  **The silent branch is gone.** `UnstructuredError` was the one outcome the boundary rendered no output for — a non-zero exit with nothing on stderr — justified by a comment saying legacy error producers had already emitted their own diagnostic. Measured: no such producer exists. Of the sixteen `eprintln!` calls in the evaluator, every one is `rt_err_kind`'s structured printer, the boundary reporter, stack-frame rendering, or a warning, except `OS.spawn` and `Socket.recvWsFrame` — and those return a **value** (`-1`, `null`) rather than the sentinel, deliberately, with their blast radius already recorded in `errors.md` (`serez-http` and `serez-strike` branch on the `null`; `serez-strike` calls `OS.spawn`), so neither is changed and neither can reach that arm. The silence had nothing behind it. It now prints `SZ4999` and names Serez-Code, not the program, as the defective party. Nothing reachable produces the outcome, so this is a net for a future regression — one that says something beats one that says nothing. Two tests: the text, and the reporter's arm, because a message the reporter never reaches is the same silence with extra steps; confirmed to fail when the arm is silenced. **The inventory was re-measured, not resampled:** of 138 constructions of the sentinel outside tests, 102 re-emit it right after `rt_err_kind` or `fatal_err_kind` recorded the payload, 34 propagate a sub-evaluation that already failed, and the two the scan could not place are a `match` pattern split across two lines and `rt_err_kind`'s own tail. Zero originate — the document's "thirty-four" was right. The `try_depth` / `last_error` side channel was probed where suppression has to stop: a recoverable error in a finally-only `try`, an error raised inside the catch handler, a later error after a caught one, and an inner catch followed by an outer failure. All four report their own code at the boundary; pinned by `stderr_suppression_inside_try_never_swallows_an_error_that_escapes_it`.  **A correction to this cycle's own work.** The `SZ4000` table added two stages ago said three kinds shared the bucket. That was a sample of what one probe happened to reach, not a reading of the source. `runtime_error_code` maps eleven kinds and sends **everything else** to `SZ4000`; the runtime raises **fifteen** that fall through, including `GuiError` with forty sites — the most common kind after `TypeError` — plus `SocketError`, `TensorError`, `OSError`, `MemoryError`, `BinaryError`, `MediaError`, `JsonError`, `GpuError`, `AutodiffError`, `InvalidAssignTarget` and the defensive `InternalError`. The table is now derived from the source and carries the practical consequence: for `SZ4000` the code alone is not enough, and a consumer must read `kind` too. `kind_to_code_map_covers_every_kind_raised` compares the two directly so it cannot drift again — and found the fifteenth, `InvalidAssignTarget`, on its first run, which a hand count had missed because it is formatted across lines. |
| Classes / properties | high | semantic inconsistency, compatibility risk | Construction and audited dispatch errors are now structured, but property schemas are not enforced after construction: typed class fields accept later values of another type and interface instances accept new/wrongly typed fields. Internal private access compares against the runtime receiver class, so subclasses can reach inherited private members. Official packages make extensive use of dynamic fields; none currently declares getters/setters, while core tests cover accessors and nested receiver writeback. | Do not tighten silently. Add declaring-owner metadata and a property-schema design, measure dynamic-field dependencies, then use an explicit compatibility/deprecation process. See `spec/classes.md`.  **Audited claim by claim against the binary.** Construction targets, interface exactness, abstract/sealed/cycle rejection, `super` validation, member dispatch and the property rules all held, including the recorded privacy caveat. One claim did not: implicit constructor chaining reaches **exactly one level**, not "rely on the compatibility rule" at every level — a constructor reached as a parent gets no implicit call, so a grandparent's fields are silently never initialized. Corrected and pinned by `implicit_constructor_chaining_reaches_exactly_one_level`. Also newly documented: a declared field beats a getter of the same name (the only way they can coexist), and a subclass getter named after a field the parent assigns breaks the parent's constructor. |
| Inheritance graph | critical | denial-of-service bug (fixed), semantic contract | **Resolved.** Forward declarations allowed cycles (`A:B`, `B:A`) and the method/getter/setter walkers had no visited/bounded condition; all three reproduced a process timeout, including `A:A`. Cycles are now rejected before registry insertion, lookup is bounded defensively, unresolved parents raise `SZ4001` on use, and sealed inheritance raises `SZ4002`. | Keep the declaration, corrupt-registry and deep-valid-inheritance regressions. Owner-aware private lookup and abstract-method completeness remain separate work. |
| Name resolution | critical | semantic inconsistency, undocumented | **Open — needs a product decision.** A free variable inside a function resolves *dynamically*: a call pushes its frame onto the caller's scope stack and lookup walks every frame, so `fn callee() { return secret; }` returns whichever caller's local is on the stack. Two different callers give two different answers. Nothing documents this, the README describes closures as lexical (they are — separate machinery), and `sz --check` does not flag free variables at all, so a misspelled name inside a function fails only when no frame up the stack happens to bind it. | Pinned by `free_variables_in_a_function_resolve_dynamically` so it cannot change by accident, and written up in `spec/scopes.md`. Making it lexical means a per-call scope stack plus explicit captured environments — a change to the core evaluation model that could alter any ecosystem program relying on it, knowingly or not. Requires the `compatibility.md` process and an explicit decision. |
| Scopes / closures | high | silent data-loss bug (fixed), compatibility risk | Scope, closure-cell and receiver-writeback behavior has a long regression history and is heavily used by `serez-ui`. **Resolved:** writeback covered only `obj.field.mutate()` and `dict["key"].mutate()` on a bare identifier, so `a[0].push(x)` and every deeper chain mutated a dropped copy — silently. `serez-agentai`'s `KVCache.store()` is one of those shapes and never accumulated, with its own suite green. Writeback now travels any assignable path via the existing `resolve_lvalue_path`/`store_path` machinery; the two optimized special cases keep their exact cost. | Treat existing regression tests and `serez-ui` as the compatibility contract before refactoring. `spec/values.md` freezes the copy/writeback/closure model. |
| Strings | critical | denial-of-service bug fixed, semantic/documentation contract | **Resolved.** Negative padding targets were cast to `usize` and grew in a quadratic loop toward the platform maximum; a one-second subprocess probe did not terminate. Padding now rejects negatives, constructs linearly with fallible reservations and has a fatal 10M-character ceiling. String validation is structured and preserves nested outcomes. README's claim that `replace` changed all occurrences contradicted implementation/tests and now correctly distinguishes `replaceAll`. | Keep Unicode scalar/index, first-only replacement and multi-character padding compatibility covered; aggregate string memory remains an explicit gap. See `spec/strings.md`. |
| Arrays | high | semantic inconsistency fixed, contract | **Resolved.** Array was the last large public surface reporting failures with stderr prints and an untyped sentinel, so no Array failure was catchable and tooling had to match on prose. Worse, `slice("x")` used index 0, `flat("x")` used depth 1 and `sort("ascending")` sorted ascending — silently doing what the program had not asked for. All 21 methods now validate structurally, arity is checked before arguments are evaluated, callbacks are validated before iteration so `[].find(1)` cannot hide, and a failed comparator leaves the receiver unsorted. The shared `eval_str_arg`/`eval_int_arg` helpers no longer collapse a nested `throw`, which also fixed the same latent defect in `Crypto.randomBytes` and `Regex.*`. | Keep `spec/arrays.md` and the Array/helper regressions. The `remove`-on-empty null remains a documented inconsistency awaiting an explicit compatibility process. |
| Dicts / Sets | medium | semantic inconsistency fixed, contract | **Resolved.** `new Set(5)` silently produced an empty set, every dict reader and the zero-argument Set methods ignored extra arguments, dict diagnostics were still stderr prints, and an unknown `Set` member reported `TypeError` while every other type reported `ReferenceError`. All are now structured and consistent, arity is rejected before arguments are evaluated, and a broken receiver is reported instead of answered with empty data. | Keep `spec/dicts.md` / `spec/sets.md` and the collection regressions. Compound Set elements never comparing equal is long-standing behavior, now documented rather than changed. |
| Value copying | critical | silent corruption bug (fixed) | **Resolved.** `extract` bounds its recursion at 500 levels; past that it replaced the subtree with null, printed one line per truncated site and let the program **exit 0**. A program nesting containers deeply received corrupted data and no failure. It is now one fatal `ResourceError` / `SZ6002` at the next statement boundary, and the limit is documented in `spec/limits.md` beside the AST and call ceilings. | The truncation itself is still how the recursion terminates; removing it needs `extract` to return a `Result` across 84 call sites. Bounded and reported is the improvement; refactoring the signature remains open. |
| Regions / arenas | high | correctness risk | Region promotion and scratch watermarks are central to value lifetime. Prior bugs included dangling refs and lost mutations. | Add invariants/property tests around promotion, nested containers, loop watermarks and returned closures.  **The generator ceiling was measured and deliberately not added.** `fn*` accumulates into an unbounded vector: 100,000 yielded integers cost 20 MB, 400,000 cost 71 MB, 1,600,000 cost 254 MB — linear, about 160 bytes per value against about 107 for the same count pushed onto a plain array, so roughly 1.6 GB for ten million. No official package uses `fn*` at all and the largest generator in the conformance suite yields 100 values, so any limit would have been invisible to every program that exists while still able to break one that does not. Put to the maintainer as a decision rather than taken: documenting the absence was chosen over a ceiling, and `limits.md` now records it under "What is not limited" with the measurements and the reasoning. **Eight runtime ceilings re-probed at the boundary**, not near it — string repetition, padded string, tensor elements, `Memory.alloc`, call depth, value nesting, `Crypto.randomBytes` and the `sz-lsp` body — all eight matched. The GPU, WebSocket, weights-file and Task rows were not re-probed and `limits.md` now says so rather than letting the section read as uniformly verified. One row also named a limit without naming the operation it bounds: reaching for `"x".repeat(n)` gets `Unknown string method` because repetition is the `*` operator, so the row names it now. |
| Modules / imports | high | security, compatibility; double-reporting fixed, contract written | Import state, current directory and export tracking live in `Evaluator`. Canonicalization prevents duplicate imports/cycles, while package and relative resolution have separate paths. **Partly resolved:** a missing import was reported twice (an `❌ ERROR:` from the import plus an `❌ UNCAUGHT EXCEPTION:` from the boundary); only the boundary reports it now. "Cannot find" and "found but cannot be loaded" are now different failures — the first keeps its historical catchable `ModuleNotFound:` exception, the second is `ImportError` / `SZ5002`. `spec/modules.md` now freezes resolution order, cache identity, cycle behavior and export visibility, and records four hazards that were undocumented: exports **leak transitively** (cleanup only considers a module's own declarations, deliberately, because the strict version deleted a sibling component's class), a module **silently overwrites** a name the importer already held, an `import` inside a function or block **half-applies** (classes survive in registries, functions and `let`s leave with the frame), and a URL import has no integrity hash, pinning or cache expiry. `SZ5001` is reserved for `ModuleNotFound` and nothing emits it. | Pinned by `tests/unit_modules.sz` (9 cases) plus the existing `tests/unit_sec_import.sz` and `runtime_outcome.rs` failure-mode tests. Still open: extracting a module loader interface out of `Evaluator`, selective import/aliasing to make the flat namespace survivable, and a decision on whether a half-applying nested import should be an error instead.  **One of the four hazards is no longer silent.** A module that declares a name the importing file already held replaces it and wins from there on; verified against the binary — a file defining `greet()`, printing `MINE`, importing a module that exports its own `greet()`, then printing `FROM THE MODULE`, with nothing on stderr in between. The import now names every binding it replaced. Nothing about the rule changed: the flat namespace, last-writer-wins and the exit code are untouched — only the silence. Measured before keeping, with a probe verified to fire on a real collision: **zero** collisions across the 483-test core suite and all eight official packages, so it can only speak when something genuinely collided. Both halves pinned by a new Import Tests section on both runners — the collision reports, and a clean import stays quiet — which needed a `run_file_test` helper that can assert stderr does **not** say something, since a warning that fires on healthy code is worse than none. The other three hazards are unchanged and still recorded in `modules.md`: transitive export leakage (deliberate), a half-applying nested import, and URL imports without integrity, pinning or expiry. `SZ5001` is still reserved and still unemitted: a missing module is thrown as the historical catchable `ModuleNotFound:` **string**, confirmed against the binary, so it carries no code and no span. No official package catches it, but changing a thrown string into a structured Error object is a public breaking change and belongs to `compatibility.md`'s process, not to this pass. |
| Optional compiler | high | correctness, compatibility | Checked AST-to-HIR lowering now returns atomically and reports `SZ7001`/`SZ7002` instead of mapping unsupported syntax to `Null` or no-op. Its 76 HIR/MIR/compiler tests now run in normal builds. LLVM emission remains experimental, feature-gated and absent from the CLI; parity is still unproven. | Keep the accepted subset in `spec/compiler.md`; require differential tests and explicit later-stage diagnostics before exposing a compile command. |
| Filesystem | high | security contract | `File` does not require a permission in normal execution; lockdown blocks it. Delete/rename additionally require `unsafe`. Reads above 256 MiB are rejected before loading with fatal `SZ6002`. | Document manifest vs lockdown precisely; centralize path policy and test symlink/junction traversal on every OS.  **Two security tests were passing for the wrong reason.** `sec_path_traversal.sz` said traversal via relative segments "must be rejected" and named escaping "the sandbox"; `sec_path_traversal_abs.sz` said the same for absolute paths. There is no such guard — measured directly, `File.read("../outside.txt")` reads a file one directory up and exits 0, and so does an absolute path to it. Both programs passed on all three CI platforms only because the paths they named (`../../etc/passwd`, `/etc/shadow`) do not exist there; on Linux the second would have passed because the file is unreadable, again not for the stated reason. Neither was deleted or weakened: they now state what they actually prove — a read of a path that is not there fails with a structured `SZ4005` — and the real behaviour is pinned by `tests/filesystem_reach.rs`. **A second, undocumented trap came out of the same probe:** a relative `File` path is measured from the process working directory while a relative `import` is measured from the file, so the same program reads its own data file when run from its folder and fails with `SZ4005` when run from one level up. Two features, both spelled `./`, measured from different places. Both facts are now in `security.md` and listed as gaps in `compatibility.md`. Confining `File`, or moving its base to the script's directory, are breaking changes and capability decisions — recorded, not taken. |
| Permissions | high | security contract | Permissions are additive declarations and source can self-grant outside lockdown. They are not an isolation boundary. All guarded namespaces now use one fatal structured `SZ6001` check. | Keep the centralized check; enumerate every native operation and close uncovered capabilities only through an explicit compatibility process.  **Three defects found and diagnosed.** A misspelled permission name was accepted silently and the later denial told the author to declare what they already had, one character away in the same file; `File` is declared 23 times across the official packages and in four manifests and gates nothing (`File.read` works with no permissions at all); and a dotted name such as `OS.exec` parses, is advertised in the parser's own comment, grants nothing, and specifically does not imply `OS`. The misspelling and the dotted form now warn at the point of the grant, with a suggestion when exactly one enforced name is within edit distance 2. `File` stays silent on purpose. `src/permissions.rs` holds the vocabulary and `enforced_permissions_match_the_evaluator` keeps it equal to what `require_permission` checks. Making `File` or dotted names genuinely enforced remains a capability decision under `compatibility.md`, not a diagnostics one. |
| Lockdown | high | security / documentation | Lockdown closes self-granting, File, import and Autodiff weight I/O, but intentionally permits `fetch`. The process still shares host resources. | Never call it a sandbox. Consider a future compatible `Network` permission/deprecation path; require external isolation for hostile code.  **Audited claim by claim against the binary:** the nine enforced namespaces (`Env`, `Gui`, `Media`, `OS`, `Socket`, `System`, `Task`, `Terminal`, `Time`) match the source exactly, self-granting outside lockdown works, the `unsafe` gate is fatal `SZ6003`, the protected-path heuristic is fatal `SZ6004`, and `fetch` really does reach the network under lockdown. One claim was wrong: `spec/security.md` said **all** lockdown refusals were catchable `SZ6001`, but `use permissions` is fatal `SecurityError` / `SZ6004` — the document was under-stating a gate's strength and contradicting `errors.md`. Corrected, and the catchable/fatal split is now pinned by `lockdown_denials_split_into_catchable_and_fatal`. |
| Network / sockets | high | security, robustness | `fetch` under lockdown has SSRF shape. Socket/WebSocket has frame-size and permission tests, but network egress is not a boundary. | Add loopback/link-local policy guidance, time/size limits and platform integration tests.  **`spec/socket.md` written from a loopback probe.** The eight signatures, the id space, the error contract and the fatal permission gate — all measured. Three facts nothing had written down. **`recv` is partial and blocking:** it returns at most `maxBytes` and leaves the rest queued, so sending `"abcdef"` then `recv(conn, 3)` yields `"abc"` and the next call `"def"`; a caller wanting a whole message must loop. It blocks with no timeout, no non-blocking mode and no poll, so a `recv` on a silent peer waits until the process is stopped. **`close` is asymmetric with the rest of the namespace:** closing an id that was never issued is a no-op, while `send`, `recv` and `accept` on an unknown id are all `SocketError` — deliberate for cleanup paths, but it means a typo in a `close` is silent. **The WebSocket helpers do not do the handshake:** `sendWsFrame`/`recvWsFrame` layer RFC 6455 framing over an already-established connection, and the HTTP upgrade is the caller's job. The documented `recvWsFrame` null fallback is restated where a reader will meet it. |
| Raw memory | high | security / correctness | Memory operations require `unsafe`, but live in evaluator-owned registries and are not process isolation. `Memory.alloc` now distinguishes a catchable zero-size `TypeError` from a fatal allocation above 256 MiB. | Keep explicitly trusted-only; test use-after-free, overflow, allocation caps and copy overlap. |
| Tasks / concurrency | high | security/architecture bugs partly fixed, remaining lifecycle debt | **Resolved:** the process-global registry leaked predictable IDs/results across embedders; workers dropped parent lockdown; `reply` published success before worker completion; poisoned locks could panic; validation was unstructured; resources were unbounded. Task state is now an evaluator-owned service shared only down the worker tree, lockdown/permissions propagate, reply is provisional, panics/poison are contained, and concurrency/source/message/record ceilings are explicit. **Remaining:** no cancellation/join/timeout, terminal retention is eviction-based, paths use the process working directory, and `message`/`reply` outside worker context retain permissive compatibility fallbacks. | Add cooperative cancellation and explicit cleanup through a compatibility design; keep OS isolation mandatory for hostile/non-terminating workers. See `spec/tasks.md`.  **Re-probed; the API held, and one undocumented channel came out of it.** Seventeen claims measured against the binary: `run` returning an int id, `message` delivering the argument, a reply becoming observable only after a successful exit, a later failure winning over an earlier reply with the structured `SZ4003` surviving inside the `ERROR:` string, a non-parsing worker reported as a failed task, the last of several replies winning, a terminal record staying repeat-pollable, `SZ4001` for an unknown id on both `poll` and `isDone`, `SZ4002` for wrong arity and type, and both permissive fallbacks outside a worker. All held. **What nothing documented:** a worker shares the host process's streams — its `out` lands on the parent's **stdout** and its diagnostics on the parent's **stderr**, interleaved with the parent's own and ordered only by which thread writes first. A program cannot tell its output from a worker's, and handling a failure through `poll` does not suppress the diagnostic the worker already printed. Now in `tasks.md` and pinned by two runner cases on both platforms. Path resolution was confirmed to use the host process working directory as documented — and this audit walked straight into what that costs: the same program run one directory up silently executed a **different file of the same name** and reported success. It looked like a defect until the leftover file was found, which is the third false finding this cycle caught by verifying before claiming. |
| GUI / media | medium | architecture, portability | GUI is the largest subsystem and shares evaluator state; audio is a default feature and needs platform packages. | Move state behind capability services; keep GUI behavior covered by `serez-ui` canary tests.  **Audited systematically for the first time.** 6,036 lines and 120 exposed methods against roughly 27 conformance tests — the thinnest coverage per unit of surface in the repository. Nothing crashes: all 120 methods called with no arguments and no window, then the 33 reachable without a window called with a string, an array, `null`, `i64::MIN`, a dict and eight arguments — 198 combinations, zero panics, zero unstructured errors. **Resolved:** 31 of those readers accepted any arguments and ignored them, while every other namespace rejects a wrong count with `TypeError`/`SZ4002`; `mouseDown(RIGHT)` is a natural thing to write given `mouseRightDown` exists separately, and it used to be met with silence. One guard before the dispatch, `close` and `font` excluded because they do read arguments. Swept clean across the official packages before enforcing and verified after: 8/8 plus serez-strike 113/113 and serez-cobol 23/23. | Pinned by `zero_argument_gui_methods_reject_arguments`. **Also resolved:** eleven call sites turned a supplied-but-wrong-typed argument into a silent default — `Gui.setTitle(5)` cleared the title, `Gui.renderTree(root, "800", w, h)` rendered at width 0 — the same shape as the Array defects fixed earlier in this cycle. They now raise `TypeError`/`SZ4002` naming method and parameter, while an *omitted* optional still takes its default; the two cases used to be indistinguishable. Measured rather than assumed because serez-ui calls `renderTree` constantly: 474/474, 8/8, strike 113/113, cobol 23/23. Still open: the subsystem shares evaluator state, and argument type validation is now complete only for the eleven sites that were silently defaulting — the rest of the 120-method surface validates through `match` arms that were already strict. |
| GPU / Tensor / Autodiff | high | resource robustness | Large native surfaces have explicit caps and broad tests, but share evaluator state; compiler parity is incomplete. Tensor construction now checks shape multiplication and the 10M-element cap. Every GPU creation path and matmul output enforces a real 256 MiB per-buffer ceiling with checked dimension products. Malformed `.szw` metadata is checked before allocation. | Add aggregate runtime budgets; move numeric services and the format contract out of evaluator internals. |
| Random | critical | crash bug fixed, semantic/security contract | **Resolved.** `Random.int(i64::MIN, i64::MAX)` overflowed its inclusive-width calculation and panicked the debug host; wide ranges were also truncated to 31 bits. Width arithmetic is now overflow-safe, wide draws cover the complete integer domain, established small-range seeded sequences remain compatible, and all Random/shape validation is structured. The LCG remains deliberately predictable and is not cryptographic entropy. | Keep seeded compatibility and full-domain regressions; use `Crypto.randomBytes` for secrets and treat any future generator replacement as compatibility-impacting. See `spec/random.md`. |
| CLI | medium | DX; missing `--help` fixed | **Partly resolved.** `sz --help` did not exist — it fell through to `Unknown flag` on stderr with exit 1, so the command surface was undiscoverable without reading `main.rs`. It now prints usage on stdout with exit 0, with `-h` and `sz help` as aliases, and the two dead-end usage errors point at it. `spec/cli.md` states the exit-code and stream contract, verified against the binary. | Machine-readable (`--json`) diagnostics and finer exit codes remain unspecified and are listed as such in `spec/cli.md`.  **One panic fixed.** `sz --watch <mistyped name>` ran the file, printed the correct `ERROR reading file` diagnostic, and *then* panicked with a raw Rust message and a backtrace note — the watcher setup used `.expect(...)` on a path that had just been shown not to exist. Both calls now report on stderr and exit 1, which is what this document and `cli.md` say every failure does. Covered on both runners; the case fails against the previous binary.  **Two more fixed, both trust defects rather than crashes.** `--help` listed "type error" among the things that exit `1`. It is not: the checker is advisory, so `sz file.sz` reports `SZ3000` and runs the program, and `sz --check` reports it and exits `0` — confirmed against the binary both ways. The help is the surface people actually read, and the test guarding that section only asserted the words "EXIT CODES" appeared, so it could not have caught this. Corrected, and now pinned as behaviour by `a_type_diagnostic_does_not_change_the_exit_code` rather than as prose. **`sz info <name nobody published>` printed a complete record and exited 0.** The registry does not `404` for an unknown package — verified with a direct request — it answers `200` with `{"total":0,"weekly":0,"monthly":0,"versions":[]}`, byte for byte what a real package with no downloads looks like, so the client rendered a fabricated manifest for a typo while `sz update` answered the same input with "not found" and exit 1. An empty version list is the reliable signal (publishing creates a version; a yanked one still appears with `yanked: 1`), so `info` now agrees with `update`. Two further defects fell out of the same function: the three download counters came from `extract_json_number(...).unwrap_or(0)`, so a body the client could not read printed as "0 downloads" instead of an error; and the yank marker was `search.contains("\"yanked\":1")` against the whole remaining body, so the first unpublished version marked every later version unpublished too — invisible only because nothing in the registry is yanked today, and `sz unpublish` exists. All four replaced by one `serde_json` parser with a pure, testable `parse_package_stats`, pinned by four unit tests that need no network. |
| REPL | medium | crash (fixed), semantic inconsistency (fixed), contract written | **Resolved.** The parity contract this row asked for now exists in `spec/cli.md`, and writing it found two defects the five existing REPL tests could not see, because all five assert containment and both defects are absences. **A line the parser rejected still ran:** `out "x"; let y = ;` printed `x` in the REPL while the identical line in a file printed `Aborted: fix the parse errors above before running.` and executed nothing — `run_source` states the rule ("a program with parse errors must not half-run") and the REPL simply never called `parser.has_errors()`. **A line that was not UTF-8 killed the session:** `read_line().unwrap()` turned `InvalidData` into `thread '<unnamed>' panicked at src/repl.rs:17`, a raw panic with a backtrace note on an interactive surface; one pasted Latin-1 character was enough. The REPL was the only entry point that did this — a file, an imported module and `--eval -` all answer with a diagnostic and exit 1. Also fixed: `flush().unwrap()` made a closed terminal a panic, and diagnostics had no source line or caret because `set_source` was never called on either the parser or the evaluator. `DEVELOPMENT.md` claimed the REPL "reuses the same pipeline per line", which was false in all four respects. | Pinned by 11 REPL cases on **both** runners (was 5), including three that can only be stated as an absence and so needed a new `run_repl_test` helper with a forbid-in-stdout assertion; all three fail against the previous REPL, verified. Three of the new cases cover the permission boundary, which had none: the REPL denies by default, honours an inline `use permissions`, carries the grant across lines, and opens nothing it was not asked for. One deliberate difference remains, recorded in `spec/cli.md`: the REPL does not run the type checker, because each line is parsed as an independent program and a per-line checker would call functions declared on earlier lines unknown. The checker is advisory everywhere, so nothing enforced is lost. |
| LSP | medium | tooling consistency | LSP reconstructs symbols from partial frontend information and duplicates builtin knowledge. Diagnostics now carry the frontend's stable `SZ2xxx`/`SZ3xxx` code in the standard LSP `code` field, so a client no longer has to match on wording. | Consume the same structured diagnostics and generated capability metadata as CLI/runtime.  **Audited; one crash fixed.** Only one panic site exists in production LSP code and it is guarded. The defect was elsewhere: `rpc.rs` allocated the message body at exactly the `Content-Length` the client advertised, with no ceiling, so `Content-Length: 9999999999999` aborted the process with `memory allocation of 9999999999999 bytes failed` — reproduced against the built `sz-lsp`. It was the only input-sized allocation in the project without a bound; `File.read`, package archives, task source and WebSocket frames all have one in `limits.md`. Now capped at 64 MiB with a diagnostic on stderr instead of an allocator abort, documented in `limits.md`, and covered by three framing tests including EOF, a non-numeric length, a negative length and a truncated body. |
| `.szx` translation | medium | **data loss (fixed)**, contract written | **Resolved.** Running a `.szx` destroyed a user file. The translated output went to `szx.with_extension("szx.sz")` — a fixed name derived from the source — so `sz app.szx` overwrote and then deleted an existing `app.szx.sz`, on the success path and on the failure path alike, with no prompt and no warning. Measured against the built binary: a file holding user text was gone afterwards, on both paths. Two concurrent runs of the same source also raced for that one path, and `--watch` re-runs on every save. The import path in the same 153-line file had always generated a unique name with the pid and a counter; the run path had not. The output name is now generated the same way and still sits beside the source, which it must — the translation carries the app's relative imports, so a temp directory would break every `import "comp/Chip"`. | Pinned by five unit tests on the naming function, which need no serez-ui to run: the name is never the one the user could own, it stays in the source's directory, a bare filename does not become an absolute-looking join, two calls never agree, and the `.szx.sz` tail survives so an existing ignore rule still matches. Verified end to end against the built binary with serez-ui installed, and against the canary: 8/8, serez-ui 36/36. `spec/cli.md` now states what running a `.szx` actually does, which its one table row ("Lex, parse, type-check, run") did not: it needs serez-ui, it spawns a second `sz`, it writes beside the source, and its diagnostics name the *translated* file with translated line numbers and a snippet the user never wrote — a note after the diagnostic now says so. **Known debt:** there is no way to keep the translated file and read it. Adding a flag for that is a product decision, not a fix. |
| Package manager | high | supply-chain/security | Strict 1 MiB JSON manifests, identifier/path containment, canonical `bin` targets, ZIP traversal/symlink checks and archive expansion limits now have Rust regressions. Installation is still non-atomic and packages have no lockfile, integrity/signature policy or minimum-runtime field. | Add staging plus atomic replacement, then specify integrity, yanks and runtime/spec constraints without changing resolution silently. |
| Tests | medium | organization; missing-fixture bug (fixed) | Coverage is unusually broad (473 passing) but categories remain filename conventions in one directory and some tests depend on output text. Legacy `unit_*.expected` pairs are now explicitly treated as golden tests rather than fake framework suites. **Resolved:** the suite depended on files that were not in the repository. `.gitignore` excludes `*.sz` and `*.json` repository-wide and un-ignores `!tests/*.sz`, which covers only the top level, so `tests/lib/`, `tests/packages/`, `tests/runner_fixtures/` and the whole Serez-source `std/` library were untracked. Verified by exporting HEAD to a clean directory: `unit_import`, `unit_export` and `unit_packages` aborted before `summary()` with `ModuleNotFound`, `unit_sec_import` failed one case, and the twelve `std/`-importing files had nothing to import. All four trees are now tracked (476 lines of library source included). | Both runners now refuse to start when a required fixture is missing, and `runner_fixtures_are_tracked_by_git` in `tests/frontend_robustness.rs` fails if one exists on disk but not in git — the state that produced this. **Also resolved:** both runners now take `-json`/`--json <path>` and write the run as a `serez-conformance/1` document — totals, per-category counts and one record per test with its status and reason. CI writes one per platform and uploads it as an artifact even when the suite fails. The recorder is checked against the counters at the end of every run, so a site that counts without recording fails the run instead of producing a report quietly missing tests. `-filter` now applies to the embedded Rust module suites and the local-`./packages/` test on both platforms; it did not on Windows, so a filtered run reported different totals. Still open: explicit suites without mass moves. |
| Test runner integrity | critical | quality-gate bug (fixed) | **Resolved.** Both platform runners defined unit PASS as absence of `[FAIL]` on stdout. A parse/runtime abort before `summary()` therefore passed—the repository had recorded this defect since 9.16.0. The first strict run exposed 24 false positives (428 pass/24 fail): 16 mislabeled golden tests, three missing summaries and five parser-invalid fixtures. After repair, the baseline was 452/452; the current suite is 459/459. Unit files now require exit 0, a summary and no failures; error tests require non-zero plus a diagnostic; E2E requires exit 0. A deliberately aborting fixture self-tests the runner on every invocation. | Keep Windows and Unix classification aligned and retain the runner-integrity probe in every quality gate. |
| Benchmarks | medium | performance governance; platform gap fixed | Seventeen workload files exist, but CI has no regression budget or stored baseline. **Resolved:** the suite was Windows-only — `run_benchmarks.ps1` had no counterpart, so nobody on Linux or macOS could run it at all, the same platform gap the conformance runners had. `run_benchmarks.sh` now exists with the same flags, and both emit the same `serez-benchmarks/1` document with the same field types, so a baseline recorded on one platform is readable by the other. Both take `--baseline` and report what crossed a threshold. The reported statistic is the **minimum** of N runs, not the mean: a process is only ever slowed by its neighbours, never sped up. | **A wall-clock budget is deliberately not wired into CI.** Measured on an idle desktop, `00_startup` ranged 35–69 ms across two consecutive runs — a factor of two — and shared CI runners are worse; a threshold wide enough not to fire on that would not catch a real regression either. This repository has already paid for two gates that reported invalid results, and a flaky third would teach people to re-run until green, which is the habit that let the first two survive. Reproducible baselines remain open work and need a dedicated runner, not a threshold bolted onto shared CI. |
| CI | high | quality gate, platform parity fixed, invalid-result bug (fixed) | CI previously ran only build/check. It now runs format, check, Clippy, Rust tests and the full Serez runner on Windows/Linux/macOS. **Resolved:** the two runners were not the same suite — `run_tests.sh` ran no CLI, `--eval`, REPL or `--check` tests, no AI files and one of two Rust module suites, and it built the debug binary while `run_tests.ps1` built release, so the platforms exercised different overflow semantics. Both now cover the same categories against the release binary, and each asserts that an embedded `cargo test <filter>` actually ran something. **Also resolved:** CI checks out the tracked tree, and that tree was missing `tests/lib/`, `tests/packages/`, `tests/runner_fixtures/` and `std/`, so every run of the conformance suite in CI was reporting on a checkout that could not pass — see the Tests row. UI/HTTP/AI/AgentAI pass locally but are not executed as floating external code in this workflow. | Add isolated, commit-pinned ecosystem canaries after reviewing their CI trust boundary; add artifact summaries. Releases still ship a bare binary with no `std/` beside it, so `import "std/..."` cannot resolve for an installed user unless SEREZ_HOME points at a checkout — packaging the library with the release assets is a separate, undecided change. |
| Releases | medium | release integrity | Tag builds are cross-platform, publish checksums and now depend on format/check/Clippy/Rust/Serez verification plus exact tag-to-Cargo-version matching. Ecosystem and changelog/spec compatibility are not yet automated. | Add isolated ecosystem evidence and validate changelog/spec compatibility before publication. |
| Documentation | high | trust / DX | Documentation is extensive but mixes normative contracts, implementation notes and historical behavior. One public section called permissions a default sandbox despite explicit caveats elsewhere. | Split normative `spec/` from guides and internals; test runnable examples.  **Resolved for the README's code:** all 198 `serez` examples are now parse-checked by `readme_serez_examples_parse` in `tests/frontend_robustness.rs`. Thirteen did not parse. Six are invalid on purpose and now say so with a `// parse-error-example:` first line — an explicit marker rather than an inferred one, because one broken block carried a ⚠️ for an unrelated reason and would have been skipped for the wrong one. Two were not Serez at all and were re-fenced (`text` for an event-shape illustration, `jsx` for `.szx`). **Five were genuine drift**: a dict literal without an annotated binding, `fn any get()` and `let out = …` (both keywords), `while cond {` without parentheses, `let name: string = …` (no scalar annotation on a binding), and `public abstract decimal area();` — which the README's own Known Gotchas section already documented as unsupported, so the document contradicted itself and a reader following the feature section wrote code that would not parse. `spec/lexical-grammar.md` gained the 50 reserved words it had described but never listed, kept in step with `lookup_ident` by `reserved_words_match_the_lexer`.  **`spec/` now has the guard the README already had.** Nothing checked the 46 `serez` blocks across the 25 normative documents. Five did not parse, all in `syntax.md`, all deliberately invalid — two already said so with a `parse-error-example` marker and three did not, which is exactly what the marker is for: being invalid has to be stated, not inferred. `spec_serez_examples_parse` now covers them, and was confirmed to fail on a deliberate break. **Three documents were re-probed claim by claim.** `values.md` held on every rule — the copy semantics for arrays, dicts, sets and instances, copying across a call and out of a container, writeback for all thirteen named mutators and for a method assigning to `this`, a getter not being a place, closure capture, the five equality rows, the eight truthiness rows and the operand-returning `&&`/`||` — except one example, which could not run at all: it built a dict literal in a field initialiser and used an undeclared `kv`. `variables.md` held on every destructuring rule including the four the grammar refuses, but its coverage boundary still listed as pending what `types.md`, `scopes.md` and `values.md` have since covered.  **Four more documents re-probed claim by claim; all four held.** `arrays.md` (56 claims, including all eleven diagnostic codes in its table and the element-type preservation rules — `filter` and `slice` keep the declared type, `map` and `flat` do not), `strings.md` (48), `sets.md` (14, including that a compound element is always admitted and `has` on one is always false) and `control-flow.md` (11, including the snapshot rule and per-iteration closure capture) produced zero drift. Recording that is the point: it distinguishes documents written from measurement and still true from the ones this cycle had to correct. `control-flow.md`'s coverage boundary is the one that is still honest and current — `if`, `while`, C-style `for`, `do-while`, `switch`, `match`, generators, `try/catch/finally` and labeled flow genuinely remain unaudited, and nothing else covers them. **The spec examples now have to run, not only parse.** `spec_serez_examples_run` drives the binary over every block. Of 41, 23 ran unaided; the other 18 needed a marker — eight deliberate runtime-error demonstrations and ten fragments, mostly multi-file module examples. None of the 18 was wrong: they had simply never been distinguished from the blocks a reader is meant to be able to paste. This closes the gap the parse checker recorded against itself one stage earlier, and was confirmed to fail on a deliberate break.  **`spec/regex.md` written from measurement.** Regex was the last namespace with real surface and no specification, which by `compatibility.md`'s own rule left the whole of it unstable. The document states the five signatures — including that `replace` takes the replacement **last**, after the text — the return shapes (`match` gives `null`, not an empty array), the dialect, the replacement substitution syntax, the error contract and the two matcher ceilings. **Its most useful section is what the engine does not support.** Five constructs other engines have are not rejected: a lookahead, a negative lookahead, a named group, a backreference and a word boundary all parse as ordinary characters and match something else. Measured: `r"a(?=b)"` does not match `"ab"` — it matches `"a?=b"`; `r"(a)\1"` matches `"a1"`, not `"aa"`; `r"a\bc"` matches `"abc"`. A pattern using any of them compiles, runs and quietly answers the wrong question. Writing the document also turned up an omission in its own first draft: `replace` supports `$&`, `$1`–`$9` and `$$` in the replacement, which the draft missed and the existing tests already covered — the group form reads exactly one digit, so `$10` is group 1 then a literal `0`, and a `$n` naming an absent group stays literal. `unit_regex.sz` goes from 17 cases to 28, eleven of them pinning the hazards so a change to the pattern parser cannot alter them in either direction unnoticed. |
| Versioning | high | bug (fixed), compatibility | Runtime is 9.17.0. **Resolved:** the documented core-floor declaration was not merely unenforced, it was actively broken — `sz install` in a `serez-ui` project failed because `"serez-code": ">= 9.17.0"` went through the package-version identifier rules. The key is now reserved, checked against the running runtime and never fetched, with Rust and CLI regressions on both platforms. `spec/compatibility.md` states the policy that `limits.md` and `random.md` already referenced. **Remaining:** the other seven official packages declare no minimum, and there is still no separate language-specification version. | Adopt the floor across the official manifests, then decide whether a separate spec version earns its keep. |

## Confirmed public contracts that must not change silently

- Truthiness and operand-returning `&&` / `||`.
- Value semantics, closure cells and receiver writeback.
- Runtime catch behavior for recoverable failures versus fatal permission,
  unsafe and resource-limit failures.
- Import/export visibility and import caching.
- A declared type matches **exactly**: no numeric widening at a parameter and no
  subtyping, so a class hierarchy crosses a call only as `any`. Both are recorded
  as inconsistencies in `spec/types.md`; changing either is breaking.
- `fetch` currently remains available under lockdown. Changing this is desirable
  to evaluate, but is breaking behavior and requires migration and ecosystem tests.
- `serez-ui >= 4.36.0` requires Serez Code `>= 9.17.0`, specifically relying
  on recent constructor, closure, truthiness and receiver fixes.

## Ecosystem inventory

| Package | Current declared status / risk |
|---|---|
| `serez-ui 4.36.0` | PASS 36/36 with core 9.17.0; explicit core floor `>= 9.17.0`. |
| `serez-http 1.0.6` | PASS 3/3; uses Socket and Time; no core floor. |
| `serez-ai 1.0.7` | PASS 3/3; Tensor/Autodiff consumer; no core floor. |
| `serez-agentai 1.0.4` | PASS 3/3; depends on `serez-ai`; no core floor. |
| `serez-pack 1.2.8` | Trusted tooling using OS/File/Env and embedding the runtime. |
| `serez-apipack 1.1.6` | Trusted deployment tooling using OS/File/Env. |
| `serez-dotenv 1.0.2` | Filesystem/environment consumer; manifest declares only Env. |
| `serez-graph 1.0.0` | Pure library plus persistence examples; has unit/security runner. |

The strict manifest parser accepts all ten local official manifests inspected
(`agentai`, `ai`, `apipack`, `cobol`, `dotenv`, `graph`, `http`, `pack`, `strike`,
`ui`). Additional package-owned suites passed against the current core: Cobol
23/23, Graph 122/122 and Strike 113/113. These are recorded separately from the
eight-package aggregate canary because their runners are not yet entries in the
shared ecosystem script.

Proposed public tiers:

- **Stable**: language specification, default interpreter pipeline, CLI execution,
  modules and documented core value semantics.
- **Official**: maintained libraries/frameworks with declared runtime/spec ranges
  and mandatory compatibility suites.
- **Experimental / Labs**: LLVM compiler, native renderer variants and any API
  that may reject unsupported language constructs. Experimental must mean explicit
  diagnostics, not silent semantic degradation.

## Maturity plan

### P0 — prevent false security and silent corruption

1. **Implemented:** checked lowering rejects unsupported statements/expressions
   atomically with `SZ7001`/`SZ7002`; HIR/MIR tests are part of default Rust tests.
   Native LLVM parity remains a release blocker if a compile command is exposed.
2. **Implemented:** source can no longer crash the process through unbounded
   recursion over the AST. `MAX_PARSE_DEPTH` (512) bounds tree depth for both
   nesting and operator chains; `tests/frontend_robustness.rs` holds the ceiling
   and asserts the frontend never panics on 43 shapes of malformed input, and
   `tests/err_parse_depth_{nesting,chain}.sz` cover it from the conformance
   runner. The ceiling clears real code by more than 20×: across the 999
   `.sz`/`.szx` files in the official ecosystem the deepest nesting is 19 levels
   and the longest operator chain is 25.
3. **Implemented:** package identifiers can no longer escape install/uninstall
   roots, `bin` targets and ZIP members are package-contained, remote archives
   have compressed/entry/expanded limits, and malformed manifests are rejected
   as complete typed JSON. `.szw` tensor metadata is likewise bounded and uses
   checked arithmetic before allocation. See `spec/packages.md` and the package/
   Autodiff Rust regressions.
4. **Implemented:** cyclic class graphs can no longer hang method/getter/setter
   lookup. Declaration rejects self/indirect cycles atomically, all walkers are
   bounded by registry size, and unresolved forward parents fail on use while
   remaining resolvable by a later declaration. Timeout reproductions and
   `unit_inheritance_errors` pin the boundary.
5. Audit the remaining runtime panic/unwrap reachable from user input; add
   regression tests before replacing each one. 290 `unwrap`/`expect`/`panic!`
   sites remain — a *higher* number than the previous 286, because five were
   removed from the user-input path in `expr.rs` while nine were added inside
   `#[cfg(test)]` assertions. The raw grep does not distinguish the two, which
   is precisely why it is an inventory and not a target, concentrated in `namespaces_gui.rs` (60), `llvm_emit.rs` (37),
   `package_manager.rs` (33), `hir_lower.rs` (29) and `lsp/server.rs` (24).
   Reachability from user input has not been established site by site.
6. Publish the exact trusted/untrusted execution contract. Treat permissions,
   `unsafe`, lockdown and OS isolation as different mechanisms.

### P1 — enforce quality and diagnostic contracts

1. Retain the format/Clippy/check/Rust/Serez gates and add a machine-readable
   conformance summary.
2. **Partly implemented:** the frontend now carries stable codes. Lexical
   failures emit `SZ1001`–`SZ1004`; `SZ2000`/`SZ3000` remain the generic parser/
   type fallbacks and `SZ2001` is the AST depth ceiling. All reach the editor
   through the LSP's standard `code` field instead of existing only as prose on
   stderr. Unterminated strings/comments, embedded NUL, invalid `0x`/`0b` values
   and 50,000 consecutive comments have regressions. Narrower `SZ2xxx`/`SZ3xxx`
   codes are split out one at a time, each with a test pinning its meaning,
   rather than freezing distinctions nothing checks. See `spec/errors.md`.
3. **Partly implemented:** complete-program evaluation now returns a structured
   `ProgramOutcome`; it separates success, recoverable runtime errors, uncaught
   user exceptions, invalid top-level control flow and legacy unstructured
   failures. `run_source_detailed` carries that distinction to embedders while
   the original exit-only API stays source-compatible. Error generations prevent
   a reused evaluator (notably the REPL) from attributing a stale pending error to
   a later failure. Pending payload now includes an internal `catchable` bit, so
   fatal security/resource errors can be structured without being swallowed by
   `try/catch`. The payload still needs to move into `EvalResult`: 176 textual
   producer/propagation occurrences across 169 source lines remain, so migration
   must proceed in tested slices rather than as one mass semantic edit. This
   crude count includes six
   newly explicit default-error propagations that replaced silent `null`
   fallbacks, so it must not be treated as a burndown metric. Migrated slices include
   exact-decimal arithmetic, all direct operator diagnostics, scalar Math
   argument resolution, every native permission/unsafe gate, all six default
   call paths plus the existing invocation depth checks, Tensor/GPU/Memory/File
   ceilings and the protected OS target
   policy. Ordinary operator faults remain catchable; resource and security
   ceilings are structured but fatal. Math type failures use `SZ4002`, and
   nested user exceptions survive `sin`/`min`/`max`/`pow` argument evaluation.
   Array/call spread, invalid `for-in` inputs and declaration destructuring now
   agree on catchable `TypeError` / `SZ4002` while preserving nested user
   `throw`. Unknown-target, eight class/interface construction validations and
   eight ordinary `super` producers, five member-dispatch producers, two
   property-write producers and sealed/invalid inheritance now use catchable
   `SZ4001`/`SZ4002`. DateTime/DateField validation now distinguishes catchable
   type, range, overflow and reference failures and preserves nested outcomes.
   Task validation now uses the same channel, while worker failures preserve
   structured payload text across the asynchronous polling boundary.
   Property-schema enforcement, declaring-owner privacy,
   static inheritance/references and the internal missing-`this` invariant
   remain in the legacy inventory.

   **The argument boundary is now swept rather than sampled.** A user `throw`
   raised while evaluating an argument to a native method was destroyed at 36
   sites across `File`, `JSON`, `Math`, `Env`, `OS`, `Terminal` and `Time`:
   they matched `EvalResult::Value` and folded everything else into an
   unstructured error, so `try/catch` never saw the exception and the program
   died reporting `SZ4999` — an internal defect of Serez-Code — for an
   exception the source had deliberately raised. The list came from passing a
   throwing call to every native method of every namespace at arities one to
   four (1,610 programs), because reading is not reliable here: the same file
   mixes correct and incorrect sites, and `Math.abs` propagated while
   `Math.sign`, two match arms below it, did not.
   `user_throw_survives_native_argument_evaluation` pins every call form,
   including second- and third-argument positions.
4. Add malformed-input and error-path tests for every migrated diagnostic.

### P2 — boundaries and compatibility

1. Extract module loading, permissions and native registries behind small runtime
   service interfaces while keeping a single executable/repository.
   **First slice done:** `src/modules.rs` owns the two questions about a module
   that do not need an evaluator — which file a path means (`search_dirs`,
   `candidates_in`, `resolve`) and whether it has already run (`LoadedModules`).
   Resolution was a `flat_map` inlined in a 200-line statement handler and could
   only be tested through the filesystem and a whole interpreter; it now has
   seven unit tests that pin the search order and candidate order directly.
   Execution stays in the evaluator, because running a module means evaluating
   against its arenas, registries and export tracking — that is the seam, and
   moving it would be the rewrite this section warns against.
   **Second slice done:** `src/permissions.rs` owns the permission vocabulary.
   The nine enforced names existed only as string literals at their
   `require_permission` call sites, and any other name a program declared was
   inserted into a `HashSet` and never looked at, which is how three defects
   went unnoticed: a misspelling granted nothing and the program then failed
   telling its author to declare what they believed they had; `File` is the
   second-most-declared capability in the ecosystem and gates nothing; and a
   dotted name like `OS.exec` parses, grants nothing and does not imply `OS`.
   The last two now warn at the grant. `File` deliberately does not — it is
   inert because the runtime does not gate files, not because the author erred,
   and a warning on every run of correct-by-convention code would drown the one
   that matters. Zero warnings across the ecosystem confirms the calibration.
   **Third slice measured and deliberately not taken.** The three declaration
   registries have 43 touchpoints across five files and are woven through
   construction, dispatch and the inheritance walk; moving them would be the
   rewrite this section warns against, and probing found no defect that owning
   the *storage* would fix. What it did find is a defect in the *policy*: a
   class and an interface may share a name, `new Name(...)` always resolves to
   the interface regardless of declaration order, and the class becomes
   unreachable with the failure appearing at the construction site as an
   argument-form `TypeError`. That is one rule at two registration sites, so it
   is a documented helper beside the registries rather than a module — creating
   one for a single predicate is the theatre this section forbids. `class`/`enum`
   and `interface`/`enum` were checked and coexist correctly, so the warning is
   scoped to the one pair that breaks.
2. **Partly implemented:** `run_ecosystem.ps1` / `run_ecosystem.sh` run every
   official package's own suite against the freshly built core and report one
   table, preferring each runner's tally over its exit code (a green exit with
   failures in the log is the worst outcome for a canary) and reporting absent
   checkouts as SKIP rather than failure. Still local-only: automating
   commit-pinned revisions in CI needs an explicit trust policy for running
   external code, which has not been decided.
3. Specify CLI exit codes, stdout/stderr and machine-readable diagnostics.
4. Add minimum runtime and language-spec fields to official package manifests.

### P3 — normative specification and documentation

`spec/` holds one document per area. No count is given here because it goes
stale; `ls spec/` is the inventory.

- `errors.md` — diagnostic code ranges, what is emitted today, and the public
  shape of a caught runtime error.
- `limits.md` — every ceiling that is now part of the language contract, and an
  explicit list of the dimensions that are **not** bounded (memory, wall-clock
  time, handle counts). Audited against the constants: every documented number
  matched, five enforced ceilings were missing (the four `.szw` weights-file
  limits and `Crypto.randomBytes`), and the "999 files, 19 levels deep"
  measurement had gone stale at 1,255 files — replaced with the durable claim
  that the only two files in the whole ecosystem reaching the AST ceiling are
  the two fixtures written to test it, which a `--check` sweep re-verifies.
- `security.md` — the trusted/untrusted execution contract, separating the
  permission manifest, `unsafe`, lockdown and OS isolation, and stating plainly
  that there is no sandbox.
- `compiler.md` — the accepted subset of the experimental AOT pipeline.
  **Audited: clean.** Its atomicity claim — never a partial program with an
  unsupported construct replaced by `null` — is what `unwrap_err()` in
  `unsupported_expression_returns_sz7002_instead_of_null_hir` actually proves,
  and the diagnostic-accumulation claim has its own test.
- `packages.md` — typed manifest fields, resolution order, package-contained
  paths, archive limits and the explicit supply-chain/atomicity gaps.
- `lexical-grammar.md` — token forms, Unicode behavior, comment/string rules and
  the stable `SZ1001`–`SZ1004` failure modes, plus the 50 reserved words.
  **Audited: clean**, including that an unknown escape keeps its backslash (so a
  Windows path survives), single-quoted strings are literal, `r"…"` is raw, and
  `..` is two dots while `...` is spread.
- `variables.md` — normative declaration destructuring, accepted sources,
  missing-value behavior and `SZ4002` failures. **Audited: clean on all twelve
  claims**, including that nested patterns really are a parse error and that a
  failed pattern declares none of its bindings.
- `control-flow.md` — the audited `for-in` subset: accepted iterables, snapshot/
  copy behavior, array-pattern iteration and propagation. **Audited: clean on
  all eleven claims** — array elements yielded as copies, strings by Unicode
  scalar, dict keys in insertion order, a non-iterable rejected as `SZ4002`,
  the iterable evaluated exactly once, mutation during the body not changing
  the traversal, a closure per iteration keeping its own value, the three
  array-pattern rules, and a throw from the iterable propagating unchanged.
- `functions.md` — parameter ordering and arity, call-time default evaluation,
  rest collection and failure propagation across every invocation route.
  Audited claim by claim and **held on every one** — the first document in this
  pass to do so, including its own worked example, defaults reading an earlier
  parameter and `this`, a later parameter being unbound, an explicit argument
  suppressing the default's evaluation, and a `throw` in a default propagating
  unchanged through functions, methods, constructors, `super()` and native
  callbacks. One gap added: a lambda declares neither a type nor a default, so
  a function value carries defaults only when it came from a named `fn`.
- `classes.md` — construction targets, exact interface shapes, abstract/no-
  constructor rules, implicit chaining and stable recoverable `super`
  diagnostics.
- `arrays.md` — value semantics, element types, mutating versus
  non-mutating methods, callback arities, evaluation order and the stable
  Array failure modes.
- `dicts.md` — declaration form, insertion order, missing-key reads, declared
  key/value types and the stable dict failure modes.
- `sets.md` — construction, value equality and compound elements, the method
  table, and the stable Set failure modes.
- `strings.md` — the character model and the built-in string methods. **Audited:
  clean on all twenty-seven claims**, including scalar-vs-grapheme length, the
  historical padding order (`"x".padStart(4, "ab")` is `"babx"`), `i64::MIN`
  clamping in `slice`, and the four-way failure classification.
- `datetime.md` — `DateTime`/`DateField` values, their permissions and formats.
  Audited: every construction, range, leap-year, clamping and failure claim held.
  Two facts it described around but never stated are now written down, both of
  them the ones a caller trips over: a `DateField` reports as an **`int`** (there
  is no `"DateField"` type name to test against), and `add`/`reduce`/`remove`
  return a **`DateTime`**, not another field.
- `random.md` — the deterministic generator, its reproducibility contract and
  the fact that it is not security-grade entropy. **Audited: clean on all
  eighteen claims**, including the full `int` domain, the state shared with
  `Math.random()`, `shuffle` not mutating its input, and every failure class.
- `tasks.md` — worker runtime ownership, isolation and messaging. **Audited:
  clean**, including that a later failure wins over an earlier provisional
  `reply` so a caller cannot observe a premature success, that the last of
  several replies wins, the `ERROR: ` prefix on a failed poll, and the two
  documented permissive fallbacks outside a worker.
- `cli.md` — what the `sz` executable accepts, what it writes to stdout versus
  stderr, and what it returns.
- `compatibility.md` — the two version numbers, the classes of change, what a
  release does and does not promise, the three-step deprecation path, the
  reserved `serez-code` minimum-runtime key, and the known gaps.
- `values.md` — assignment and argument passing copying for every type, the
  receiver-writeback rule and what counts as a place, closures capturing the
  variable rather than its value, and the equality and truthiness tables.
- `scopes.md` — where a name comes from, including the dynamic resolution of
  free variables in functions, which is recorded and pinned rather than changed.
- `operators.md` — the accepted operand types per operator, precedence and
  associativity, the fact that `&&`/`||` return an operand rather than a
  boolean, `sizeof` taking a type rather than a value, and the overload table.
- `modules.md` — what `import` executes, path resolution order, the
  all-or-nothing export rule, transitive export leaking, silent name
  collisions, run-once caching and cycle behavior, why imports belong at the
  top level, and the two module failure modes.
- `types.md` — the seven type keywords and where an annotation may appear, what a
  declared type accepts at a call, the absence of numeric widening and of subtyping,
  where enforcement stops (bindings, field assignment, `[T]` parameters), `type_of`
  and `is`, and exactly how far the static checker reaches.
- `syntax.md` — the statement and expression grammar: what parses, and the forms
  that read as valid and do not (brace-less bodies, `for (x in …)` without the
  `let`, a typed lambda parameter, a scalar annotation on a `let`, a JSON-style
  object literal, trailing commas in three of seven list forms, nested block
  comments).

Every planned document now exists. The remaining sections of variables and control
flow still need expansion, and every rule added from here has to be checked against
the implementation and pointed at a conformance test before it is written down —
publishing a rule the implementation does not follow is worse than publishing
nothing. Writing these documents has been the most productive bug-finding activity
in this pass: the dynamic-scoping surprise, the nested-receiver data loss, the
unenforced constructor types and the unusable enum parameters were all found by
probing a claim rather than by reading code.

#### The native-surface sweep

Every method of twelve native namespaces was called with no arguments, a string,
`i64::MIN`, an array, `null` and five arguments — 894 calls in total, plus the
198 already run against `Gui`.

- **Zero panics.**
- **Zero unstructured errors.** `errors.md` lists unstructured producers as
  active debt; in the namespaces reachable this way there are none left.
- **Zero silent defaults.** The `_arg(..).unwrap_or(..)` pattern that turned a
  wrong-typed argument into 0 or `""` is gone from every namespace.
- Five zero-argument methods outside `Gui` accepted extras and ignored them —
  `OS.pid`, `OS.platform`, `OS.tick`, `Media.playingCount`, `Media.stopAll` —
  and now reject them through a shared `reject_arguments` helper.

One flagged case was **not** a defect: `DateTime.from(1, 2, 3, 4, 5)` is inside
its documented three-to-seven arity. The sweep was blunt there, and the pinning
test records that so it is not "fixed" later.

**That sweep read results, and results were the wrong thing to read.** A reader
that ignores your argument returns the same value it would have returned
without it, so nothing in the output distinguishes "accepted and used" from
"accepted and discarded". Passing a *throwing* call as the argument makes the
difference visible: if the member never evaluates it, the exception never
happens. Re-run that way, seventeen more zero-argument members were still
accepting and dropping arguments — `Math.PI`, `Math.E`, `Math.random`,
`Dec.MAX`, `Dec.MIN`, `Dec.MAX_SCALE`, `Autodiff.clear`,
`Autodiff.isRecording`, `Env.args`, `Time.now`, `System.cpuCount`,
`System.totalMemory`, `System.freeMemory`, `System.hostname`, `System.uptime`,
`Terminal.getSize` and `Terminal.clear` — plus three in `Gui`.

The three `Gui` ones are a correction of this cycle's own work. `Gui.close` and
`Gui.font` were excluded from `GUI_ZERO_ARG_METHODS` with a comment asserting
they "do read arguments". They do not: `Gui.setFont` is the setter, and the
`"text" | "font"` arm that prompted the belief belongs to scene-node property
assignment, a different method entirely. `Gui.windowPosition` was simply
missed. The claim was written into the source as justification, which is how it
survived — a false comment is not caught by a test unless the test disagrees
with it, and the test had been written to match the comment.

All twenty now refuse arguments through the same `reject_arguments` helper,
swept first against 1,123 `.sz`/`.szx` files across the core tests, the
benchmarks and all ten official packages: not one passes an argument to any of
them, so nothing that works today stops working. The property spelling
(`Math.PI` with no parentheses) is unaffected and pinned.

#### The panic-site audit

The repository carries 311 `unwrap` / `expect` / `panic!` / `unreachable!`
sites. The count on its own says nothing, so every one was classified rather
than counted:

| Class | Sites | Why it cannot be reached from Serez source |
| --- | ---: | --- |
| test-only | 148 | Behind `#[cfg(test)]`; a panic there is a failing test, which is the point |
| lock poisoning | 39 | `.lock()`/`cv.wait()` — reachable only after another thread has already died |
| behind the `llvm` feature | 37 | Not compiled in a default build; the backend is experimental and unreachable from the CLI |
| arena invariant | 28 | `self.resolve(ref)` on a ref the evaluator itself just produced |
| exhaustive match | 8 | `unreachable!()` in a `match` the compiler cannot prove exhaustive |
| guarded a line or two above | the rest | An explicit length, `is_none()` or shape check immediately before |

Every candidate that looked genuinely reachable was checked against the running
binary rather than by reading:

- **`broadcast_data(...).unwrap()`** in tensor broadcasting. Its `None`
  conditions are exactly what the `broadcast_shape` check two lines above
  already guarantees. It does contain a real hazard — `(0..ndim - 1)` underflows
  when `ndim == 0` — but an empty tensor shape is rejected by all four
  construction paths (`new Tensor`, `Tensor.zeros`, `Random.normalTensor`,
  `reshape`) with a clean `RangeError`. Latent, not reachable; noted because the
  function's safety rests on validation two modules away.
- **`Gui.nodeText` / `nodeTextPx` / `nodeRoundRectOutline`**, whose
  `gui_state.as_mut().unwrap()` has no guard inside its own match arm. Called
  with no window open, all three return `GuiError` / `SZ4000`; the guard is
  upstream of the arms.
- **`Binary.unpackInt64Le`** — an explicit `bytes.len() < 8` check two lines
  above. **`sz run` command resolution** — inside a `match matches.len() { 1 => }`.
  **The lexer's `peek_char`** — guarded by `read_position >= input.len()`, and
  `read_position` only ever advances by `len_utf8`, so the slice stays on a
  character boundary.

**No reachable panic was found.** That is a reading, though, and reading code
and concluding "this looks guarded" is the reasoning that let two invalid
quality gates survive here. So the audit leaves behind
`hostile_arguments_to_native_methods_never_panic`: 55 pieces of source a user
can type — i64 extremes into string indexes, empty and negative tensor shapes,
truncated binary input, uncompilable regexes, malformed JSON, freeing a null
pointer, reversed random bounds — each run under `catch_unwind`, each required
to produce a diagnostic rather than a dead process.

#### What auditing the specification has produced

Every document with concrete, falsifiable claims has now been checked against the
running binary rather than re-read. The result is worth recording because it
changes where the remaining risk is:

| Document | Result |
| --- | --- |
| `security.md` | one claim wrong — a lockdown gate documented as catchable is fatal |
| `limits.md` | five enforced ceilings undocumented; one measurement stale |
| `classes.md` | one claim wrong — implicit chaining reaches one level, not the hierarchy |
| README code | five examples did not parse; one contradicted the same file |
| `datetime.md` | two facts undocumented — `DateField` introspection and what its arithmetic returns |
| `packages.md` | clean |
| `functions.md` | clean |
| `control-flow.md` | clean |
| `variables.md` | clean |
| `strings.md` | clean |
| `random.md` | clean |
| `lexical-grammar.md` | clean |
| `tasks.md` | clean |
| `compiler.md` | clean |

The defects clustered in documents making *many* claims with *little* test
backing. Where a document was already pinned by tests — `packages.md` has 30 unit
tests behind it — it held. That is the argument for the rule this section already
states: a rule goes in `spec/` only once it is pointed at a conformance test.

Every document making falsifiable behavioural claims has now been checked.
`cli.md` is pinned by the runner's 33 CLI/`--eval`/REPL/`--check` cases rather
than by a separate pass, and `compatibility.md` states process and policy — it
has nothing to probe, only to be followed, which the constructor-type-check
sweep in this cycle did.

Six documents were clean on first reading; four needed correction. The four
were also the four making the largest number of claims per line of test
backing.

### P3/P4 — performance and features

Establish reproducible benchmark baselines only after correctness gates are stable.
New syntax or builtins remain last priority; functionality implementable in Serez
belongs in official libraries.

## Definition of a releasable change

A core change is releasable only when:

1. formatting, check, Clippy and Rust tests pass;
2. language conformance, regression, error-path and security suites pass;
3. affected official-package canaries pass;
4. public semantics have a spec entry or an explicit experimental marker;
5. diagnostics and exit behavior remain compatible or follow a documented
   deprecation;
6. the release workflow consumes those same results rather than rebuilding from an
   unverified tag alone.
