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
| `cargo check` | PASS, one Rust warning (`namespaces_gui.rs:851`, unused assignment) |
| `cargo clippy --all-targets` | PASS, 190 historical library warnings, no errors |
| `cargo test --all-targets` | PASS, 246 tests (148 library, 33 LSP binary, 9 frontend robustness, 56 runtime outcome) |
| Serez test runner | PASS, 472 files/groups; 0 failed; 0 skipped |
| Official ecosystem (`run_ecosystem.ps1`) | PASS, 8/8 packages: UI 36/36, HTTP 3/3, AI 3/3, AgentAI 3/3, pack 3/3, apipack 3/3, dotenv 2/2, graph 3/3 |

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
| Evaluator | high | architectural problem; expression diagnostics fixed, first extraction done | `Evaluator` owns arenas, modules, permissions, sockets, GPU buffers, raw memory, autodiff, GUI, processes, audio and tasks in addition to language semantics. **Partly resolved:** `expr.rs` no longer prints any diagnostic — the fourteen core expression failures a normal program hits (typed parameter, declared return, array/dict literal element types, entry literal and object patch position, dot call on a method-less type, enum variant, `&` and `*`) are structured, coded and catchable, and a typed-parameter failure now carries its call stack in the payload instead of printing it as a side effect. | Extract service-owned state incrementally behind runtime interfaces; do not rewrite evaluation. `mod.rs`, `stmt.rs`, `classes.rs` and `methods_dec.rs` still hold unmigrated producers. |
| Control flow | medium | semantic debt | `EvalResult` mixes normal values, return/break/continue, user throw and an untyped `Error` sentinel. | Separate internal control flow from structured runtime failure without changing catch behavior. |
| Runtime errors | high | bug risk, DX, architecture | Structured errors preserve `code`, `kind`, `message`, `span`, `stack`, and `notes`; an internal recoverability bit prevents structured security/resource failures from becoming catchable. Complete programs return `ProgramOutcome`, and `run_source_detailed` exposes the same information without breaking the exit-only API. Direct operators, exact decimals, scalar Math, spread/iteration/destructuring operands, default evaluation, construction/`super`/member/property/inheritance/DateTime/Random/String/Task validation, permission/unsafe gates and the audited resource/security ceilings now preserve stable structured diagnostics. Internally, 176 textual `EvalResult::Error` producer/propagation occurrences across 169 source lines and mutable pending-error/`try_depth` state remain across other subsystems. This is an inventory, not a monotonic quality metric: explicit propagation can legitimately add occurrences while silent fallbacks are removed. | Migrate producers and then the internal carrier in tested slices; preserve the boundary and fatal/catchable distinction while eliminating the side channel. See `spec/errors.md`. |
| Classes / properties | high | semantic inconsistency, compatibility risk | Construction and audited dispatch errors are now structured, but property schemas are not enforced after construction: typed class fields accept later values of another type and interface instances accept new/wrongly typed fields. Internal private access compares against the runtime receiver class, so subclasses can reach inherited private members. Official packages make extensive use of dynamic fields; none currently declares getters/setters, while core tests cover accessors and nested receiver writeback. | Do not tighten silently. Add declaring-owner metadata and a property-schema design, measure dynamic-field dependencies, then use an explicit compatibility/deprecation process. See `spec/classes.md`.  **Audited claim by claim against the binary.** Construction targets, interface exactness, abstract/sealed/cycle rejection, `super` validation, member dispatch and the property rules all held, including the recorded privacy caveat. One claim did not: implicit constructor chaining reaches **exactly one level**, not "rely on the compatibility rule" at every level — a constructor reached as a parent gets no implicit call, so a grandparent's fields are silently never initialized. Corrected and pinned by `implicit_constructor_chaining_reaches_exactly_one_level`. Also newly documented: a declared field beats a getter of the same name (the only way they can coexist), and a subclass getter named after a field the parent assigns breaks the parent's constructor. |
| Inheritance graph | critical | denial-of-service bug (fixed), semantic contract | **Resolved.** Forward declarations allowed cycles (`A:B`, `B:A`) and the method/getter/setter walkers had no visited/bounded condition; all three reproduced a process timeout, including `A:A`. Cycles are now rejected before registry insertion, lookup is bounded defensively, unresolved parents raise `SZ4001` on use, and sealed inheritance raises `SZ4002`. | Keep the declaration, corrupt-registry and deep-valid-inheritance regressions. Owner-aware private lookup and abstract-method completeness remain separate work. |
| Name resolution | critical | semantic inconsistency, undocumented | **Open — needs a product decision.** A free variable inside a function resolves *dynamically*: a call pushes its frame onto the caller's scope stack and lookup walks every frame, so `fn callee() { return secret; }` returns whichever caller's local is on the stack. Two different callers give two different answers. Nothing documents this, the README describes closures as lexical (they are — separate machinery), and `sz --check` does not flag free variables at all, so a misspelled name inside a function fails only when no frame up the stack happens to bind it. | Pinned by `free_variables_in_a_function_resolve_dynamically` so it cannot change by accident, and written up in `spec/scopes.md`. Making it lexical means a per-call scope stack plus explicit captured environments — a change to the core evaluation model that could alter any ecosystem program relying on it, knowingly or not. Requires the `compatibility.md` process and an explicit decision. |
| Scopes / closures | high | silent data-loss bug (fixed), compatibility risk | Scope, closure-cell and receiver-writeback behavior has a long regression history and is heavily used by `serez-ui`. **Resolved:** writeback covered only `obj.field.mutate()` and `dict["key"].mutate()` on a bare identifier, so `a[0].push(x)` and every deeper chain mutated a dropped copy — silently. `serez-agentai`'s `KVCache.store()` is one of those shapes and never accumulated, with its own suite green. Writeback now travels any assignable path via the existing `resolve_lvalue_path`/`store_path` machinery; the two optimized special cases keep their exact cost. | Treat existing regression tests and `serez-ui` as the compatibility contract before refactoring. `spec/values.md` freezes the copy/writeback/closure model. |
| Strings | critical | denial-of-service bug fixed, semantic/documentation contract | **Resolved.** Negative padding targets were cast to `usize` and grew in a quadratic loop toward the platform maximum; a one-second subprocess probe did not terminate. Padding now rejects negatives, constructs linearly with fallible reservations and has a fatal 10M-character ceiling. String validation is structured and preserves nested outcomes. README's claim that `replace` changed all occurrences contradicted implementation/tests and now correctly distinguishes `replaceAll`. | Keep Unicode scalar/index, first-only replacement and multi-character padding compatibility covered; aggregate string memory remains an explicit gap. See `spec/strings.md`. |
| Arrays | high | semantic inconsistency fixed, contract | **Resolved.** Array was the last large public surface reporting failures with stderr prints and an untyped sentinel, so no Array failure was catchable and tooling had to match on prose. Worse, `slice("x")` used index 0, `flat("x")` used depth 1 and `sort("ascending")` sorted ascending — silently doing what the program had not asked for. All 21 methods now validate structurally, arity is checked before arguments are evaluated, callbacks are validated before iteration so `[].find(1)` cannot hide, and a failed comparator leaves the receiver unsorted. The shared `eval_str_arg`/`eval_int_arg` helpers no longer collapse a nested `throw`, which also fixed the same latent defect in `Crypto.randomBytes` and `Regex.*`. | Keep `spec/arrays.md` and the Array/helper regressions. The `remove`-on-empty null remains a documented inconsistency awaiting an explicit compatibility process. |
| Dicts / Sets | medium | semantic inconsistency fixed, contract | **Resolved.** `new Set(5)` silently produced an empty set, every dict reader and the zero-argument Set methods ignored extra arguments, dict diagnostics were still stderr prints, and an unknown `Set` member reported `TypeError` while every other type reported `ReferenceError`. All are now structured and consistent, arity is rejected before arguments are evaluated, and a broken receiver is reported instead of answered with empty data. | Keep `spec/dicts.md` / `spec/sets.md` and the collection regressions. Compound Set elements never comparing equal is long-standing behavior, now documented rather than changed. |
| Value copying | critical | silent corruption bug (fixed) | **Resolved.** `extract` bounds its recursion at 500 levels; past that it replaced the subtree with null, printed one line per truncated site and let the program **exit 0**. A program nesting containers deeply received corrupted data and no failure. It is now one fatal `ResourceError` / `SZ6002` at the next statement boundary, and the limit is documented in `spec/limits.md` beside the AST and call ceilings. | The truncation itself is still how the recursion terminates; removing it needs `extract` to return a `Result` across 84 call sites. Bounded and reported is the improvement; refactoring the signature remains open. |
| Regions / arenas | high | correctness risk | Region promotion and scratch watermarks are central to value lifetime. Prior bugs included dangling refs and lost mutations. | Add invariants/property tests around promotion, nested containers, loop watermarks and returned closures. |
| Modules / imports | high | security, compatibility; double-reporting fixed, contract written | Import state, current directory and export tracking live in `Evaluator`. Canonicalization prevents duplicate imports/cycles, while package and relative resolution have separate paths. **Partly resolved:** a missing import was reported twice (an `❌ ERROR:` from the import plus an `❌ UNCAUGHT EXCEPTION:` from the boundary); only the boundary reports it now. "Cannot find" and "found but cannot be loaded" are now different failures — the first keeps its historical catchable `ModuleNotFound:` exception, the second is `ImportError` / `SZ5002`. `spec/modules.md` now freezes resolution order, cache identity, cycle behavior and export visibility, and records four hazards that were undocumented: exports **leak transitively** (cleanup only considers a module's own declarations, deliberately, because the strict version deleted a sibling component's class), a module **silently overwrites** a name the importer already held, an `import` inside a function or block **half-applies** (classes survive in registries, functions and `let`s leave with the frame), and a URL import has no integrity hash, pinning or cache expiry. `SZ5001` is reserved for `ModuleNotFound` and nothing emits it. | Pinned by `tests/unit_modules.sz` (9 cases) plus the existing `tests/unit_sec_import.sz` and `runtime_outcome.rs` failure-mode tests. Still open: extracting a module loader interface out of `Evaluator`, selective import/aliasing to make the flat namespace survivable, and a decision on whether a half-applying nested import should be an error instead. |
| Optional compiler | high | correctness, compatibility | Checked AST-to-HIR lowering now returns atomically and reports `SZ7001`/`SZ7002` instead of mapping unsupported syntax to `Null` or no-op. Its 76 HIR/MIR/compiler tests now run in normal builds. LLVM emission remains experimental, feature-gated and absent from the CLI; parity is still unproven. | Keep the accepted subset in `spec/compiler.md`; require differential tests and explicit later-stage diagnostics before exposing a compile command. |
| Filesystem | high | security contract | `File` does not require a permission in normal execution; lockdown blocks it. Delete/rename additionally require `unsafe`. Reads above 256 MiB are rejected before loading with fatal `SZ6002`. | Document manifest vs lockdown precisely; centralize path policy and test symlink/junction traversal on every OS. |
| Permissions | high | security contract | Permissions are additive declarations and source can self-grant outside lockdown. They are not an isolation boundary. All guarded namespaces now use one fatal structured `SZ6001` check. | Keep the centralized check; enumerate every native operation and close uncovered capabilities only through an explicit compatibility process. |
| Lockdown | high | security / documentation | Lockdown closes self-granting, File, import and Autodiff weight I/O, but intentionally permits `fetch`. The process still shares host resources. | Never call it a sandbox. Consider a future compatible `Network` permission/deprecation path; require external isolation for hostile code.  **Audited claim by claim against the binary:** the nine enforced namespaces (`Env`, `Gui`, `Media`, `OS`, `Socket`, `System`, `Task`, `Terminal`, `Time`) match the source exactly, self-granting outside lockdown works, the `unsafe` gate is fatal `SZ6003`, the protected-path heuristic is fatal `SZ6004`, and `fetch` really does reach the network under lockdown. One claim was wrong: `spec/security.md` said **all** lockdown refusals were catchable `SZ6001`, but `use permissions` is fatal `SecurityError` / `SZ6004` — the document was under-stating a gate's strength and contradicting `errors.md`. Corrected, and the catchable/fatal split is now pinned by `lockdown_denials_split_into_catchable_and_fatal`. |
| Network / sockets | high | security, robustness | `fetch` under lockdown has SSRF shape. Socket/WebSocket has frame-size and permission tests, but network egress is not a boundary. | Add loopback/link-local policy guidance, time/size limits and platform integration tests. |
| Raw memory | high | security / correctness | Memory operations require `unsafe`, but live in evaluator-owned registries and are not process isolation. `Memory.alloc` now distinguishes a catchable zero-size `TypeError` from a fatal allocation above 256 MiB. | Keep explicitly trusted-only; test use-after-free, overflow, allocation caps and copy overlap. |
| Tasks / concurrency | high | security/architecture bugs partly fixed, remaining lifecycle debt | **Resolved:** the process-global registry leaked predictable IDs/results across embedders; workers dropped parent lockdown; `reply` published success before worker completion; poisoned locks could panic; validation was unstructured; resources were unbounded. Task state is now an evaluator-owned service shared only down the worker tree, lockdown/permissions propagate, reply is provisional, panics/poison are contained, and concurrency/source/message/record ceilings are explicit. **Remaining:** no cancellation/join/timeout, terminal retention is eviction-based, paths use the process working directory, and `message`/`reply` outside worker context retain permissive compatibility fallbacks. | Add cooperative cancellation and explicit cleanup through a compatibility design; keep OS isolation mandatory for hostile/non-terminating workers. See `spec/tasks.md`. |
| GUI / media | medium | architecture, portability | GUI is the largest subsystem and shares evaluator state; audio is a default feature and needs platform packages. | Move state behind capability services; keep GUI behavior covered by `serez-ui` canary tests. |
| GPU / Tensor / Autodiff | high | resource robustness | Large native surfaces have explicit caps and broad tests, but share evaluator state; compiler parity is incomplete. Tensor construction now checks shape multiplication and the 10M-element cap. Every GPU creation path and matmul output enforces a real 256 MiB per-buffer ceiling with checked dimension products. Malformed `.szw` metadata is checked before allocation. | Add aggregate runtime budgets; move numeric services and the format contract out of evaluator internals. |
| Random | critical | crash bug fixed, semantic/security contract | **Resolved.** `Random.int(i64::MIN, i64::MAX)` overflowed its inclusive-width calculation and panicked the debug host; wide ranges were also truncated to 31 bits. Width arithmetic is now overflow-safe, wide draws cover the complete integer domain, established small-range seeded sequences remain compatible, and all Random/shape validation is structured. The LCG remains deliberately predictable and is not cryptographic entropy. | Keep seeded compatibility and full-domain regressions; use `Crypto.randomBytes` for secrets and treat any future generator replacement as compatibility-impacting. See `spec/random.md`. |
| CLI | medium | DX; missing `--help` fixed | **Partly resolved.** `sz --help` did not exist — it fell through to `Unknown flag` on stderr with exit 1, so the command surface was undiscoverable without reading `main.rs`. It now prints usage on stdout with exit 0, with `-h` and `sz help` as aliases, and the two dead-end usage errors point at it. `spec/cli.md` states the exit-code and stream contract, verified against the binary. | Machine-readable (`--json`) diagnostics and finer exit codes remain unspecified and are listed as such in `spec/cli.md`. |
| REPL | medium | semantic consistency | State and recovery tests pass, but REPL and file execution need a declared parity contract. | Add multiline/parser/error/exit behavior tests and document deliberate differences. |
| LSP | medium | tooling consistency | LSP reconstructs symbols from partial frontend information and duplicates builtin knowledge. Diagnostics now carry the frontend's stable `SZ2xxx`/`SZ3xxx` code in the standard LSP `code` field, so a client no longer has to match on wording. | Consume the same structured diagnostics and generated capability metadata as CLI/runtime. |
| Package manager | high | supply-chain/security | Strict 1 MiB JSON manifests, identifier/path containment, canonical `bin` targets, ZIP traversal/symlink checks and archive expansion limits now have Rust regressions. Installation is still non-atomic and packages have no lockfile, integrity/signature policy or minimum-runtime field. | Add staging plus atomic replacement, then specify integrity, yanks and runtime/spec constraints without changing resolution silently. |
| Tests | medium | organization; missing-fixture bug (fixed) | Coverage is unusually broad (473 passing) but categories remain filename conventions in one directory and some tests depend on output text. Legacy `unit_*.expected` pairs are now explicitly treated as golden tests rather than fake framework suites. **Resolved:** the suite depended on files that were not in the repository. `.gitignore` excludes `*.sz` and `*.json` repository-wide and un-ignores `!tests/*.sz`, which covers only the top level, so `tests/lib/`, `tests/packages/`, `tests/runner_fixtures/` and the whole Serez-source `std/` library were untracked. Verified by exporting HEAD to a clean directory: `unit_import`, `unit_export` and `unit_packages` aborted before `summary()` with `ModuleNotFound`, `unit_sec_import` failed one case, and the twelve `std/`-importing files had nothing to import. All four trees are now tracked (476 lines of library source included). | Both runners now refuse to start when a required fixture is missing, and `runner_fixtures_are_tracked_by_git` in `tests/frontend_robustness.rs` fails if one exists on disk but not in git — the state that produced this. **Also resolved:** both runners now take `-json`/`--json <path>` and write the run as a `serez-conformance/1` document — totals, per-category counts and one record per test with its status and reason. CI writes one per platform and uploads it as an artifact even when the suite fails. The recorder is checked against the counters at the end of every run, so a site that counts without recording fails the run instead of producing a report quietly missing tests. `-filter` now applies to the embedded Rust module suites and the local-`./packages/` test on both platforms; it did not on Windows, so a filtered run reported different totals. Still open: explicit suites without mass moves. |
| Test runner integrity | critical | quality-gate bug (fixed) | **Resolved.** Both platform runners defined unit PASS as absence of `[FAIL]` on stdout. A parse/runtime abort before `summary()` therefore passed—the repository had recorded this defect since 9.16.0. The first strict run exposed 24 false positives (428 pass/24 fail): 16 mislabeled golden tests, three missing summaries and five parser-invalid fixtures. After repair, the baseline was 452/452; the current suite is 459/459. Unit files now require exit 0, a summary and no failures; error tests require non-zero plus a diagnostic; E2E requires exit 0. A deliberately aborting fixture self-tests the runner on every invocation. | Keep Windows and Unix classification aligned and retain the runner-integrity probe in every quality gate. |
| Benchmarks | medium | performance governance | Sixteen workload files exist, but CI has no regression budget or stored baseline. | Separate smoke benchmarks from tracked performance runs; record platform/runtime/version. |
| CI | high | quality gate, platform parity fixed, invalid-result bug (fixed) | CI previously ran only build/check. It now runs format, check, Clippy, Rust tests and the full Serez runner on Windows/Linux/macOS. **Resolved:** the two runners were not the same suite — `run_tests.sh` ran no CLI, `--eval`, REPL or `--check` tests, no AI files and one of two Rust module suites, and it built the debug binary while `run_tests.ps1` built release, so the platforms exercised different overflow semantics. Both now cover the same categories against the release binary, and each asserts that an embedded `cargo test <filter>` actually ran something. **Also resolved:** CI checks out the tracked tree, and that tree was missing `tests/lib/`, `tests/packages/`, `tests/runner_fixtures/` and `std/`, so every run of the conformance suite in CI was reporting on a checkout that could not pass — see the Tests row. UI/HTTP/AI/AgentAI pass locally but are not executed as floating external code in this workflow. | Add isolated, commit-pinned ecosystem canaries after reviewing their CI trust boundary; add artifact summaries. Releases still ship a bare binary with no `std/` beside it, so `import "std/..."` cannot resolve for an installed user unless SEREZ_HOME points at a checkout — packaging the library with the release assets is a separate, undecided change. |
| Releases | medium | release integrity | Tag builds are cross-platform, publish checksums and now depend on format/check/Clippy/Rust/Serez verification plus exact tag-to-Cargo-version matching. Ecosystem and changelog/spec compatibility are not yet automated. | Add isolated ecosystem evidence and validate changelog/spec compatibility before publication. |
| Documentation | high | trust / DX | Documentation is extensive but mixes normative contracts, implementation notes and historical behavior. One public section called permissions a default sandbox despite explicit caveats elsewhere. | Split normative `spec/` from guides and internals; test runnable examples.  **Resolved for the README's code:** all 198 `serez` examples are now parse-checked by `readme_serez_examples_parse` in `tests/frontend_robustness.rs`. Thirteen did not parse. Six are invalid on purpose and now say so with a `// parse-error-example:` first line — an explicit marker rather than an inferred one, because one broken block carried a ⚠️ for an unrelated reason and would have been skipped for the wrong one. Two were not Serez at all and were re-fenced (`text` for an event-shape illustration, `jsx` for `.szx`). **Five were genuine drift**: a dict literal without an annotated binding, `fn any get()` and `let out = …` (both keywords), `while cond {` without parentheses, `let name: string = …` (no scalar annotation on a binding), and `public abstract decimal area();` — which the README's own Known Gotchas section already documented as unsupported, so the document contradicted itself and a reader following the feature section wrote code that would not parse. `spec/lexical-grammar.md` gained the 50 reserved words it had described but never listed, kept in step with `lookup_ident` by `reserved_words_match_the_lexer`. |
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
   moving it would be the rewrite this section warns against. Permissions and
   the native registries are still evaluator-owned.
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
- `packages.md` — typed manifest fields, resolution order, package-contained
  paths, archive limits and the explicit supply-chain/atomicity gaps.
- `lexical-grammar.md` — token forms, Unicode behavior, comment/string rules and
  the stable `SZ1001`–`SZ1004` failure modes.
- `variables.md` — normative declaration destructuring, accepted sources,
  missing-value behavior and `SZ4002` failures.
- `control-flow.md` — the audited `for-in` subset: accepted iterables, snapshot/
  copy behavior, array-pattern iteration and propagation.
- `functions.md` — parameter ordering and arity, call-time default evaluation,
  rest collection and failure propagation across every invocation route.
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
- `strings.md` — the character model and the built-in string methods.
- `datetime.md` — `DateTime`/`DateField` values, their permissions and formats.
- `random.md` — the deterministic generator, its reproducibility contract and
  the fact that it is not security-grade entropy.
- `tasks.md` — worker runtime ownership, isolation and messaging.
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
