# Serez Code — maturity roadmap state

**This file is the source of truth for roadmap progress.** It exists so that a
session with no conversational memory can pick the work up exactly where the
previous one left it. Conversation history is never sufficient; this file is.

Read before starting any milestone, in this order:

1. this file
2. `MATURITY_AUDIT.md`
3. the `spec/` document relevant to what you are about to touch
4. `git log` for the subsystem in question

---

## 0. Where we are

| | |
|---|---|
| **Current milestone** | **M7 — Semantics Frozen.** M6 closed PARTIAL (§9K), M5 COMPLETE (§9I), M4 PARTIAL (§9G). |
| Goals done in M4 | **M4.0** audit (§9F.0) · **M4.1** the divergence, measured (§9F.2) · **M4.2–M4.3** the symbol layer, corpus-validated (§9F.3) · **M4.7.1–M4.7.2** the scope model and the measurement (§9F.6) · **audit** (§9G) |
| Goals done in M3 | **M3.0** audit · **M3.1** the rendering net · **M3.2–M3.3** the model, and the frontend onto it · **M3.4–M3.5** checker and runtime · **M3.6** one renderer (D5) · **M3.7** the nine silent errors (**behaviour change**) · **M3.8** ordering (D6) |
| Last completed milestone | **M5 — Type System Stable** (§9I). M4 and M6 are **PARTIAL** by decision, not by omission |
| Goals done in M6 | **M6.0** the 48-field audit (§9J.0) · **M6.1** autodiff · **M6.2** modules · **M6.3** security, task, caches · **M6.4** service operations · **audit** (§9K) |
| Goals done in M5 | **M5.0** audit (§9H.0) · **M5.1** the agreement net (§9H.1) · **M5.2** three false positives (§9H.2) · **M5.3** `export`, closing §5.29 (§9H.3) · **M5.4** positions and tooling parity (§9H.4) · **audit** (§9I) |
| **Autonomy protocol** | Milestones proceed without per-milestone authorization. A decision with several defensible answers is **registered in §7A, not taken**, and blocks only what genuinely depends on it. Nothing is marked COMPLETE whose Definition of Done is unmet. See §12. |
| **Open decisions** | 10 OPEN — **DEC-M4-001**…**-004**, **DEC-M5-001**…**-005**, **DEC-M6-001**. All in §7A with measured evidence and a marked recommendation |
| Branch | `improve` |
| HEAD | `9ca4d22` |
| M0 baseline commit | `d8662c2` (= tag `v10.0.0`, on `origin`) |
| Runtime version | 10.0.0 |
| Last state update | 2026-09-03 — M6 closed PARTIAL (§9K) |

Milestone ledger:

| Milestone | Status |
|---|---|
| M0 — Baseline Frozen | **COMPLETE** (2026-09-01) |
| M1 — Parser Molecular | **COMPLETE** (2026-09-01) — mod.rs 3,936 -> 422 (-89%), 1 file -> 14 |
| M2 — AST + Spans Stable | **COMPLETE** (2026-09-02) — all 28 `Expression` variants and 39 of 40 structs carry a span |
| M3 — Diagnostics Unified | **COMPLETE** (2026-09-02) — 5 diagnostic types -> 1, 4 rendered formats -> 1 renderer, §5.17 fixed |
| M4 — Semantic Layer Established | **PARTIAL** (2026-09-02, §9G) — 3 of 6 DoD items met. Layer established, validated, **unadopted**; adoption held by DEC-M4-001/002/004 |
| M5 — Type System Stable | **COMPLETE** (2026-09-03, §9I) — 4 checker/runtime divergences fixed, §5.29 closed, 5 decisions registered |
| M6 — Runtime Molecular | **PARTIAL** (2026-09-03, §9K) — `Evaluator` 48 fields -> 38; dispatch still on the evaluator, held by DEC-M6-001 |
| M7 — Semantics Frozen | **IN PROGRESS** |
| M8 — Conformance Complete | NOT STARTED |
| M9 — Robustness & Security Hardened | NOT STARTED (partially pre-empted; see §6) |
| M10 — Stable Language Platform | NOT STARTED |

---

## 1. M0 — Baseline Frozen

### 1.1 Definition of Done

| Criterion | Status |
|---|---|
| Baseline reproducible | met — §1.2 lists every command and its measured result |
| Gates green | met — 6 of 6 gates PASS, §1.2 |
| Debt visible | met — §5 and §6 |
| Current architecture understood | met — §3 and §4 |
| Clean checkpoint | met — working tree content-identical to HEAD before and after |

**M0 STATUS: COMPLETE.**

### 1.2 Measured baseline — v10.0.0 / `d8662c2` / Windows 11, 2026-09-01

Every row was executed in this session. Reproduce from the repository root.

| Gate | Command | Result |
|---|---|---|
| Format | `cargo fmt --check` | **PASS**, exit 0 |
| Check | `cargo check --all-targets` | **PASS**, no warnings |
| Clippy | `cargo clippy --all-targets` | **PASS**, 0 errors, 186 warnings / 39 distinct lints |
| Rust tests | `cargo test --all-targets` | **PASS**, 318 / 0 failed |
| Serez runner (PowerShell) | `.\run_tests.ps1 -json <f>` | **PASS**, 490 / 0 failed / 0 skipped |
| Serez runner (bash) | `./run_tests.sh --json <f>` | **PASS**, 490 / 0 failed / 0 skipped |
| Ecosystem canary | `.\run_ecosystem.ps1 -SkipBuild` | **PASS**, 8 / 8 packages, 56 tests |

Rust test breakdown (318): 180 library · 36 `sz-lsp` binary · 3
`tests/diagnostic_codes.rs` · 4 `tests/filesystem_reach.rs` · 16
`tests/frontend_robustness.rs` · 79 `tests/runtime_outcome.rs`.

Serez runner breakdown (490), **identical in both runners** — parity is verified
per category, not only on the total:

```
ai 5 · check 3 · cli 14 · e2e 91 · error 63 · eval 13 · import 4
package-manager 15 · repl 11 · runner-integrity 1 · rust 2 · security 104 · unit 164
```

Both runners emit a `serez-conformance/1` JSON report. The bash run was executed
under MINGW64 (Git Bash on this Windows host), which proves *runner* parity, not
*platform* parity; real Linux/macOS parity comes from CI (`.github/workflows/ci.yml`
runs the full set on `ubuntu-latest`, `windows-latest` and `macos-latest`).

Ecosystem canary: `serez-ui` 36, `serez-http` 3, `serez-ai` 3, `serez-agentai` 3,
`serez-pack` 3, `serez-apipack` 3, `serez-dotenv` 2, `serez-graph` 3.

Clippy's 186 warnings are pre-existing and concentrated: 59 in
`evaluator/ops.rs`, 26 in `namespaces_gui.rs`, 13 in `namespaces_gui/render.rs`,
11 in `methods_tensor.rs`, 3 in `parser.rs`. CI does not pass `-D warnings`, so
none of them is a gate today.

### 1.3 Working-tree state

`git status` reports ` M README.md`, and this is **not** a content change.
`core.autocrlf=input` with no `.gitattributes`: the working file carries CRLF,
the blob carries LF, and `git diff --numstat` reports zero changed lines. The
tree was content-identical to `d8662c2` before M0 and after it, apart from the
two files M0 deliberately wrote (§1.4).

### 1.4 What M0 changed

M0 changed no behavior. It wrote documentation only:

- `docs/maturity/ROADMAP_STATE.md` — new; this file.
- `MATURITY_AUDIT.md` — evidence rows re-measured where they had drifted
  (see §5.1 for the drift itself).

### 1.5 Baseline re-verified at `9ca4d22` — 2026-09-02

M0's claim is that the baseline is **reproducible**, not merely that it was once
measured. A session opening after M1–M3 landed has to re-establish it against the
current HEAD before trusting anything else in this file, because a green figure
recorded at `d8662c2` says nothing about `9ca4d22`. Every row below was executed
in this session, on the same host, from the repository root.

| Gate | Command | At `d8662c2` (M0) | At `9ca4d22` (now) |
|---|---|---|---|
| Format | `cargo fmt --check` | PASS | **PASS**, exit 0 |
| Check | `cargo check --all-targets` | PASS, no warnings | **PASS**, no warnings |
| Clippy | `cargo clippy --all-targets` | PASS, 0 errors | **PASS**, 0 errors |
| Clippy per-site list (§5.26) | the `--message-format short` pipeline | 181 lines | **181 lines — unchanged** |
| Rust tests | `cargo test --all-targets` | 318 / 0 failed | **398 / 0 failed** |
| Serez runner (PowerShell) | `.un_tests.ps1 -json <f>` | 490 / 0 / 0 | **499 / 0 / 0** |
| Serez runner (bash) | `./run_tests.sh --json <f>` | 490 / 0 / 0 | **499 / 0 / 0** |
| Runner parity | per-category, both reports | identical | **identical** |
| Ecosystem canary | `.un_ecosystem.ps1 -SkipBuild` | 8 / 8, 56 tests | **8 / 8, 56 tests** |

Seven gates plus the canary: **all green.**

Rust tests at 398 (from 318): 199 library · 56 + 10 `sz-lsp` binary · 79
`tests/runtime_outcome.rs` · 22 · 16 `tests/frontend_robustness.rs` · 4 + 4 + 4 ·
3 · 1. The growth is the nets M1–M4 added — the parser snapshot, the diagnostic
render manifest, the semantic unit tests and the corpus divergence measurement.

Serez runner at 499 (from 490), **identical per category in both runners**:

```
ai 5 · check 3 · cli 14 · e2e 91 · error 72 · eval 13 · import 4
package-manager 15 · repl 11 · runner-integrity 1 · rust 2 · security 104 · unit 164
```

The whole of the +9 is `error` 63 → 72: the nine fixtures M3.7 added for the nine
parser errors that used to reach nobody (§9D.6). Every other category is
unmoved, which is the shape a milestone chain that changed one behaviour on
purpose should produce.

**The clippy per-site list is the row that matters most.** §5.26 established it
as the real gate because the raw count is cache-sensitive. It reads **181 lines,
byte-identical to the figure recorded there** — so M1, M2, M3 and M4's delivered
half introduced no new lint site anywhere in the crate, across four milestones.
The raw count reads 174 today against 186 at M0; that difference is the cache
artefact §5.26 documents, not an improvement, and it is why the count is not the
gate.

**Working tree.** `git status --porcelain` reports one entry, `?? audit/` — an
untracked report written by the peer session of §5.16, not by this work. The
` M README.md` of §1.3 is gone: it was the CRLF/LF artefact described there, and
it has since normalised. No tracked file differs from `9ca4d22`.

---

## 2. Codebase size

> **Measured at M0 (`d8662c2`), before M1–M4.** Kept as the frozen reference
> point. Current figures, re-measured at `9ca4d22`: **80 Rust files, 56,787
> lines**; the parser is 14 files totalling 4,738, `mod.rs` 468,
> `expressions.rs` 829; `src/semantic.rs` (360) and `src/span.rs` and
> `src/diagnostic.rs` are new since. `namespaces_gui.rs` is still the largest
> file at 6,264 and has not been touched.

63 Rust files, 54,555 lines. Largest:

| File | Lines |
|---|---|
| `src/evaluator/namespaces_gui.rs` | 6,264 |
| `src/parser/expressions.rs` | 754 (the parser is now 14 files; `mod.rs` is 422) |
| `src/evaluator/methods_tensor.rs` | 3,672 |
| `src/evaluator/namespaces_autodiff.rs` | 2,935 |
| `src/evaluator/mod.rs` | 2,653 |
| `src/package_manager.rs` | 2,649 |
| `src/evaluator/namespaces_gui/render.rs` | 2,530 |

Serez corpus, git-tracked: **491** `.sz` files — 460 under `tests/`, 17
benchmarks, 9 `apps/`, 5 `std/`. No `.szx` is tracked, so the parser harness
needs no `.szx` translation step. Tracked rather than on-disk is the number that
matters: `tests/` also accumulates gitignored `_*.sz` and `~unit_temp_*.sz`
residue that differs between checkouts.

`tests/parser_snapshot.rs` walks **490** of the 491. The one it skips is
`tests/~tmp_test.sz`, which is committed runner residue rather than a test — see
§5.14.

---

## 3. Pipeline as it actually is

```
source (String)
  → lexer::Lexer          (828 lines, 20 fns) ── LexError  { code SZ1xxx, line, column, message }
  → parser::Parser        (4,403 lines, 14 files) ── ParseError { code SZ2xxx, line, column, message }
  → ast::Program                              ── 48 types, all derive Debug, no HashMap
  → type_checker::TypeChecker (410 lines)     ── TypeError  { code SZ3xxx, line, column, message }
  → evaluator::Evaluator  (37,187 lines, 48 fields) ── RuntimeError { code, kind, message, span, stack, notes }
```

There is **no resolver and no semantic layer**. `run::run_source_detailed`
(`src/run.rs`) is the single door; `sz file.sz`, `sz --eval`, the REPL, `import`,
`.szx` and task workers all reach it or re-run the same stages.

Type-check findings are **advisory**: `checker.check()` prints, and its result is
discarded for exit-code purposes. Verified: a program with `SZ3000` still runs to
completion and only fails later at runtime.

The optional AOT compiler (`src/compiler/`, HIR → MIR → LLVM) consumes the same
AST, is feature-gated behind `llvm`, and is not wired to any CLI verb.

### 3.1 Module dependency graph

Mostly a clean DAG. The edges worth naming:

| Edge | Nature |
|---|---|
| `run` ↔ `szx` | **cycle** — `run.rs:174` calls `szx::run_szx_file`, `szx.rs:126` calls `run::run_file` |
| `region` → `ast` | runtime values embed AST nodes (`BlockStatement`, `Parameter`) — inherent to a tree-walking interpreter, but it couples the value representation to syntax |
| `evaluator::stmt` → `lexer`, `parser`, `szx` | `import` re-enters the frontend |
| `evaluator::namespaces_task` → `lexer`, `parser`, `type_checker`, `package_manager` | a worker re-runs the whole pipeline |
| `compiler::hir_lower` → `parser`, `lexer` | **test-only** (`#[cfg(test)]`), not a production edge |

No `parser → evaluator`, no `ast → gui`, no `lexer → package_manager`. The
inversions M10 warns about are, with the exceptions above, absent already.

### 3.2 Diagnostics as they actually were, at M0

> **Superseded by M3.** This section describes `d8662c2`, and M3 undid every
> problem it names: the five diagnostic types became one model (`src/diagnostic.rs`),
> the four rendered formats became one renderer, and data is now separated from
> rendering. It is kept because it is the *before* half of M3's evidence — see
> §9D.0 for the audit that replaced it and §9E for what actually changed. Do not
> read the table below as current.

Four unrelated error types, one per phase, each printing to stderr at the point
of production:

| Type | Where | Shape |
|---|---|---|
| `LexError` | `lexer.rs` | code, line, column, message |
| `ParseError` | `parser.rs` | code, line, column, message |
| `TypeError` | `type_checker.rs` | code, line, column, message |
| `RuntimeError` | `evaluator/mod.rs` | code, kind, message, `Option<RuntimeErrorSpan>`, `Vec<RuntimeErrorFrame>`, notes |

68 `eprintln!` sites across the crate (16 in `evaluator/mod.rs`, 14 in
`parser.rs`, 12 in `main.rs`). Data and rendering are not separated anywhere.
Codes *are* stable and are already consumed by the LSP — that part holds.

Of the parser's 139 error sites, **137 use the generic `SZ2000`** and 2 use a
narrower code. This is deliberate (a code is a stability promise) and is
documented at `SZ_PARSE_ERROR`.

---

## 4. The parser — the M1 map

Measured at `d8662c2`, when the file was `src/parser.rs`. M1.0 moved it to
`src/parser/mod.rs` without changing a line of it, so every figure below still
holds.

3,936 lines, one `impl Parser` block, ~95 methods, 5 free functions
(`is_writable_chain`, `token_precedence`, `is_type_keyword`, `parse_dec_literal`,
`parse_interpolated_string`).

### 4.1 Public surface (the façade contract M1 must preserve)

Small and fully enumerated — every external use across `src/` and `tests/`:

```rust
Parser::new(Lexer)
parser.set_source(Vec<String>)
parser.set_source_name(&str)
parser.parse_program() -> Program
parser.has_errors() -> bool
parser.take_errors() -> Vec<ParseError>

pub struct ParseError
pub const SZ_PARSE_ERROR, SZ_PARSE_DEPTH_EXCEEDED
pub const MAX_PARSE_DEPTH            // used by tests/frontend_robustness.rs
pub enum Precedence                  // pub, no external consumer
pub fn token_precedence              // pub, no external consumer
```

Consumers: `run.rs`, `lsp/analysis.rs`, `evaluator/stmt.rs` (×2, for `import`),
`evaluator/namespaces_task.rs` (×2, for workers), `tests/frontend_robustness.rs`,
`tests/diagnostic_codes.rs`.

Since M1.0 this surface is pinned by `tests/parser_facade.rs`, which also
records three things about it that were not written down anywhere:
`take_errors` does not drain (§5.11), diagnostics are grouped lexer-last rather
than ordered by position (§5.12), and `has_errors()` answers `false` on a
parser whose source is already lexically broken (§5.13).

### 4.2 Shared core (high fan-in — belongs in `parser/core`)

| Method | Fan-in |
|---|---|
| `next_token` | 55 |
| `parser_error` | 30 |
| `parse_expression` | 30 |
| `parse_block_statement` | 10 |
| `parse_type_string` | 7 |
| `parse_statement` | 5 |
| `parse_function_parameters` | 5 |
| `is_compound_assign` | 5 |
| `parse_inner_block` | 3 |
| `is_reserved_name` | 3 |

Plus the infrastructure nobody else calls through: `enter_depth` / `charge_depth`
/ `DepthGuard`, `parser_error_code`, `print_frontend_error`, `flush_lexer_errors`,
`synchronize`, `peek_precedence` / `current_precedence`, the `*_is_name` helpers.

### 4.3 Largest methods

| Method | Lines |
|---|---|
| `parse_expression` (prefix/primary dispatcher) | 288 |
| `parse_class_declaration` | 250 |
| `parse_infix_chain` | 217 |
| `parse_for_inner` | 184 |
| `parse_statement` (dispatcher) | 161 |
| `parse_expression_statement` | 157 |
| `parse_let_statement` | 149 |
| `parse_interface_declaration` | 106 |
| `parse_function_parameters` | 101 |
| `parse_function_statement` | 99 |

### 4.4 Extraction mechanism (decided, low risk)

`src/parser.rs` → `src/parser/mod.rs` plus sibling modules that each open
`impl super::Parser { ... }`. Rust privacy is module-based and descendants see an
ancestor's private items, so `Parser`'s private fields stay reachable with no
visibility changes.

**This is not speculative: it is exactly what `src/evaluator/` already does.**
28 files under `src/evaluator/` extend `Evaluator` through
`impl super::Evaluator { pub(super) fn … }`. M1 reuses the established
convention rather than inventing one.

### 4.5 Precedence — audit before touching

`Precedence` + `token_precedence` + `parse_expression(precedence)` +
`parse_infix_chain` already form a precedence-climbing parser, not naïve
recursive descent. Per the roadmap, **M1 must not change the algorithm.** Any
move toward Pratt parsing is a separate project requiring differential testing,
because it can silently alter precedence or associativity.

### 4.6 Parser core inventory (M1.1.1 — measurement, no code change)

Every access to every `Parser` field, attributed to the method that makes it.
The question M1.1 has to answer is not "what looks like infrastructure" but
"where does infrastructure already leak into grammar", because those leaks are
what a file split would otherwise carry along with it.

| Field | Kind | Owned by | Grammar leaks |
|---|---|---|---|
| `lexer` | cursor | `new`, `next_token` | none |
| `current_token`, `peek_token` | cursor | grammar, 212 + 253 reads | **by design** — this is recursive descent; hiding these behind accessors would be abstraction for its own sake |
| `depth` | depth accounting | `enter_depth`, `charge_depth` | **1** — `parse_infix_chain:2176` constructs a `DepthGuard` directly, deliberately and with a comment: it charges one level per operator so the ceiling bounds the *tree*, not the recursion |
| `errors` | diagnostics | `take_errors`, `parser_error_code`, `flush_lexer_errors` | none |
| `lexer_errors` | diagnostics | `next_token`, `flush_lexer_errors` | none |
| `had_error` | diagnostics | `has_errors`, `parser_error_code`, `flush_lexer_errors` | **9** — and every one of them is the bug in §5.17 |
| `source_lines` | rendering | `set_source`, `print_frontend_error` | none |
| `source_name` | rendering | `set_source_name`, `print_frontend_error` | **1** — `parse_expression:2442` hands it to the free function `parse_interpolated_string`, which needs a label for a message it prints itself |

**The boundary is already almost clean.** Eight of the nine fields are confined
to the methods that own them; the token pair is grammar's by nature. Only three
places cross the line, and two of them are the same defect:

1. The nine `had_error` sites (§5.17). They are a diagnostics bug, not a
   layering problem — routing them through `parser_error_code` fixes both at
   once. **M1.1.4 must move them as they are** and leave the fix to its own
   commit.
2. `parse_infix_chain`'s direct `DepthGuard`. Intentional and documented; it
   moves with the depth module in M1.1.3 and needs `DepthGuard` to stay
   reachable from grammar.
3. `parse_interpolated_string`'s `source_name`. A free function that reports for
   itself, which is why its message is the tenth uncoded one.

Consequence for the plan: **M1.1 is smaller than it looked.** The extraction is
mostly mechanical, and the one genuinely entangled thing — error reporting — is
entangled because of a bug rather than because of the architecture.

---

## 5. Discoveries

§5.1–5.9 are M0; §5.10–5.16 are M1.0; §5.17–5.18 are M1.1; §5.19 is M1.14; §5.20 is M1.15; §5.21 is M2.0; §5.22–5.24 are M2.3.3; §5.25 is M2.7; §5.26 is M3.5; §5.27 is M3.6; §5.28 is M3.7; §5.29 is M4.4. §5.4 was corrected by M2.0 — see the note in it.

All are **pre-existing**. Neither milestone changed any behavior; none was fixed.

### 5.1 `MATURITY_AUDIT.md` evidence had drifted — *documentation mismatch* (fixed in M0, docs only)

The audit was written 2026-08-25 and last re-measured 2026-08-28; 76 commits
landed after that. Corrected in M0:

| Claim | Was | Is |
|---|---|---|
| Runtime version | 9.17.0 | 10.0.0 |
| Rust LOC / files | ~50,860 / 60 | 54,555 / 63 |
| Clippy warnings | 190 | 186 |
| `diagnostic_codes.rs` tests | 2 | 3 |
| Largest-file table | stale figures | re-measured |

### 5.2 `v10.0.0` is released but has no changelog heading — *documentation mismatch*, low

`Cargo.toml` says `10.0.0`, the tag `v10.0.0` exists on `origin` and points at
HEAD (`d8662c2`, 0 commits after it). `CHANGELOG.md` still files all ~1,800 lines
of work since 9.17.0 under `## [Unreleased] — maturity hardening`. A released
version therefore has no section of its own. Recorded in the audit's Versioning
row. **Not fixed** — closing a changelog section is a release decision, not a
refactor.

### 5.3 `src/test_run.rs` is tracked dead weight — *architectural debt*, low

Two lines, UTF-16 encoded, unchanged since 2026-05-07. No `mod test_run;` exists
in `lib.rs`, `main.rs` or `Cargo.toml`, so it is not compiled, not formatted, not
linted. It reads `test.serez` — an extension the language no longer uses.
**Not fixed**; removal is an independent task.

### 5.4 Only 5 of 48 AST types carry a position, and 2 of those 5 are dead — *architectural debt*, medium (M2 input)

> **Corrected 2026-09-02 by the M2.0 audit.** This entry originally said four of
> the five positional fields had no consumer. That was wrong: it was written by
> reading the `#[allow(dead_code)]` attributes in `ast.rs` rather than by tracing
> who reads the fields. Three of the five are load-bearing. The error mattered —
> it would have told M2 it could normalise the positions away freely, when in
> fact three of them feed runtime and type-checker diagnostics.

Five of 48 AST types carry `line`/`column`. Traced to their consumers:

| Node | Position consumed by | Live? |
|---|---|---|
| `CallExpression` | `evaluator/expr.rs:330`, `type_checker.rs:355`, `:383` | **yes** |
| `DotCallExpression` | `evaluator/classes.rs:692`, `:876`, `:921` | **yes** |
| `InfixExpression` | `evaluator/expr.rs:1429` | **yes** |
| `ClassField` | — | no |
| `EnumDeclaration` | — | no |

Consequences for M2:

- **Coverage is 10%.** The other 43 node types carry nothing, so a diagnostic
  about a `let`, an `if`, a class, a loop or a literal has no position to report
  — which is why `spec/errors.md` types the caught `Error.span` as `string?`,
  nullable, "best available".
- **There is no `Span` type.** Position is an inlined `line: usize, column:
  usize` pair, written out five times in `ast.rs` and populated at 29 sites in
  the parser.
- **`Token` carries no byte offset.** The lexer tracks `position` and
  `read_position` internally but never puts either on a `Token`, so a range span
  (`start..end`) cannot be built in the AST alone — M2 reaches into the lexer.
- **Two normative constraints bound the design.**
  `spec/lexical-grammar.md`: columns count *Unicode scalar values, not UTF-8
  bytes*. `spec/errors.md`: the caught `Error.span` is a `"line:column"` string
  or `null`. A byte-offset span is therefore an internal representation only,
  and must render identically.

### 5.21 The AST already satisfies every "must not" M2 asks of it — *audit result*, no action (M2.0)

M2's charter says the AST must not consume tokens, resolve symbols, check types,
execute, access the runtime, or render diagnostics. Measured: **`src/ast.rs` has
zero `use` statements and zero functions.** It is 48 plain data types and nothing
else. Every one of those six prohibitions already holds.

So M2 is not a purification project. Its whole content is spans: making them
uniform, giving them a type, and deciding how far coverage should reach.

*Updated by M2.2:* `ast.rs` now has exactly one `use` — `crate::span::Span`.
That is not a regression of the property measured above. `span` is a leaf: it
depends on nothing, so `ast -> span` adds no coupling to any subsystem and the
six prohibitions still hold. It is noted because the sentence "zero use
statements" would otherwise read as stale the next time someone checks it.

One small exception, recorded rather than fixed: `evaluator/stmt.rs:475`
constructs an `ast::IndexAssignStatement` — not to synthesise source, but as an
ad-hoc tuple to re-bundle three already-cloned values, with a comment saying so.
The runtime reusing a syntax type as a data carrier is untidy; it is not a
layering violation, since the dependency runs runtime → AST, which is the
correct direction.

### 5.5 Crate-level `#![allow(dead_code)]` — *test/quality deficiency*, low

`src/lib.rs` line 14. It suppresses dead-code detection across all 54,555 lines,
which is how §5.3 survived. It also makes the per-field `#[allow(dead_code)]`
attributes in `ast.rs` decorative. Removing it is likely to surface real findings
and should be treated as its own task with its own noise budget.

### 5.6 The `run` ↔ `szx` module cycle — *architectural debt*, low (M10 input)

`run.rs:174` → `szx::run_szx_file`; `szx.rs:126` → `run::run_file`. Legal in
Rust, but it is a genuine cycle in the dependency graph M10 has to certify.

### 5.7 Ten of twenty runtime namespaces have no specification — *documentation gap*, medium (M7/M8 input)

Specified: `File`, `Socket`, `Task`, `Random`, `Regex`, `DateTime`/`Time`,
`OS`/`System`/`Terminal`.
**Unspecified:** `Gui`, `Media`, `Tensor`, `Autodiff`, `GPU`, `Memory`,
`Binary`, `Crypto`, `JSON`, `Math`.

`Gui` alone is 6,264 lines — the largest file in the repository and the thinnest
specified.

### 5.8 No normative rule identifiers exist yet — M8 input

The 30 `spec/` documents contain zero `LEX-001`-style identifiers. M8's
spec-rule ↔ conformance-test mapping starts from nothing.

### 5.9 Re-verified open behaviors (unchanged at 10.0.0)

| Behavior | Probe result |
|---|---|
| Free variables resolve **dynamically** | `callee()` reading a free `secret` returns `from-a` under one caller and `from-b` under another. The audit's one remaining **critical open** finding still holds. |
| Type errors are advisory | `SZ3000` printed, `out "before"` still executed, failure came later as runtime `SZ4002`. |
| Parse-depth ceiling | 600 nested parens → `SZ2001`, "deeper than the 512 level limit". |

---

### 5.10 The existing gate cannot see an AST change or a reworded diagnostic — *test deficiency*, high (measured in M1.0.2)

Not an opinion — an experiment. Two changes were made to `parser.rs`, both of
the exact shape a careless extraction produces:

1. `CallExpression.column` set to `0`. That field is `#[allow(dead_code)]` and
   has **no consumer anywhere in the crate** (§5.4), so this is the most
   invisible tree change available.
2. One parser message reworded, `"Unexpected token"` → `"PERTURBED token"`.

Then the whole quality gate was run against the perturbed parser:

| Gate | Result with both perturbations in place |
|---|---|
| `cargo test` (318 tests, snapshot excluded) | **all pass** |
| `run_tests.ps1` (490 files/groups) | **490 passed, 0 failed** |
| `tests/parser_snapshot.rs` | **fails — 287 of 490 files, and 4 diagnostic lists** |

So 808 tests were blind to both, and the harness caught both. This is the
justification for M1.0 existing as its own goal, and the reason no later M1
molecule is allowed to proceed without it green.

### 5.11 `Parser::take_errors` clones rather than drains — *API hazard*, low

Despite the name, the body is `self.errors.borrow().clone()`. Both callers
(`run.rs`, `lsp/analysis.rs`) call it exactly once, so nothing depends on the
difference — which is exactly why a refactor could "fix" it into a real drain
and break nothing visibly until a second caller appeared. Pinned by
`take_errors_reads_the_list_rather_than_draining_it`.

### 5.12 Diagnostics are grouped by producer, not ordered by position — *unspecified behavior*, medium (M3 input)

`flush_lexer_errors` runs at the *end* of `parse_program`, so every `SZ2xxx`
precedes every `SZ1xxx` regardless of where they occurred: a malformed literal
on line 1 is reported after a syntax error on line 3. `spec/errors.md` documents
the codes and the stderr shape and says nothing about order, so neither
behaviour is currently wrong — but a caller sorting or trusting the order has
nothing to rely on. Pinned as observed by
`lexical_diagnostics_arrive_after_syntactic_ones`; M3 has to decide it.

### 5.13 `has_errors()` is false on a parser whose source is already broken — *hazard*, low

`Parser::new` pulls two tokens, so a lexical failure on line 1 exists inside the
parser before `parse_program` is called. It sits in a separate queue until the
flush, so `has_errors()` answers `false` until then. No caller checks early.
Pinned by `lexical_diagnostics_become_visible_only_once_parsing_has_run`.

### 5.14 `tests/~tmp_test.sz` is committed runner residue — *test deficiency*, low

`framework.sz` with the body of what is now `unit_dict_advanced.sz` appended:
a captured temp file from a 2026-06-19 run. No runner, script or document
references it. Two commits have "restored" it after glob cleanups removed it, on
the assumption it was needed — `chore: restore tests/~tmp_test.sz (caught again
by glob cleanup)`. It is why the snapshot corpus is 490 files and
`git ls-files '*.sz'` reports 491. **Not fixed**; deletion is an independent
task.

### 5.15 A frontend test cannot walk a 512-deep AST on a default test thread — *note, not a defect*

The snapshot harness died with `STATUS_STACK_OVERFLOW` on first run. Cause: the
corpus contains the two depth-ceiling fixtures, `cargo test` gives its threads
2 MiB, and a debug-build `parse_expression` frame is ~8 KiB. This is already
documented at `tests/frontend_robustness.rs:31`, which answers it with an
explicit 16 MiB thread; the snapshot uses 32 MiB because it walks each tree
three times (parse, `Debug`, drop) rather than twice. **No product behavior is
involved** — the release binary parses both fixtures in every conformance run.
Recorded so the next frontend test does not rediscover it.

### 5.16 A peer session writes into this working tree — *working-tree hazard*, low

`audit/2026-09-01_14-52-03.md` appeared untracked, mid-session, at 14:52. It was
written by a separate Claude Code session (`Auditoría de módulos y permisos
nativos`). It changed no tracked file — verified against `git status` and by
diffing the moved parser against `HEAD:src/parser.rs` — so the M0 baseline and
the M1.0 evidence are intact. It is left in place and uncommitted. Worth knowing
before a future milestone treats a clean `git status` as a precondition.

### 5.17 Nine parser errors never reach the error list — **FIXED in M3.7**, see §9D.6 (found in M1.1.1)

> **Status: fixed.** All nine now go through `parser_error`, so they carry
> `SZ2000`, a file, a line, a column and a caret, and they reach `take_errors()`.
> A tenth site was found while fixing them. The description below is the state
> before M3.7 and is kept because §9D.6 refers to it.

Nine sites in `parser/mod.rs` report by hand instead of calling
`parser_error`: `had_error.set(true)` followed by a bare `eprintln!`. Lines
687, 702, 717 (`parse_native_declaration`), 935, 971
(`parse_sizeof_expression`), 980 (`parse_unsafe_statement`), 2642
(`parse_expression`'s `unsafe` arm), 3179 (`parse_class_declaration`), 3371
(`parse_visibility_statement`); plus 3907 in the free function
`parse_interpolated_string`, which cannot reach the parser's state at all.

Nothing is pushed into `errors`, so `take_errors()` returns **empty** for a
program the parser has just rejected. Measured, not inferred:

```
$ sz p_sizeof.sz
❌ PARSE ERROR: expected '(' after 'sizeof'          # no code, no file, no line, no caret
$ sz p_normal.sz
❌ PARSER ERROR [SZ2000] [p_normal.sz 2:1]: Expected variable name after 'let'
  2 | let = 2;
      ^
```

Consequences:

- **The LSP publishes nothing.** `lsp/analysis.rs` builds its diagnostics from
  `take_errors()`, so an editor underlines nothing for these nine errors.
- **`run.rs` builds `RunFailure::Frontend(vec![])`** — a frontend failure
  carrying no reason for an embedder to read.
- **`spec/errors.md` is contradicted.** It states every parser diagnostic is
  `SZ2000`/`SZ2001`, rendered `❌ PARSER ERROR [SZ2000] [file line:col]`. These
  are uncoded, unpositioned, and even use a different prefix — `PARSE ERROR`
  against `PARSER ERROR`.

Why nothing caught it: `has_errors()` is still set, so the CLI still exits 1 and
still prints a `❌`, and that is the whole of what the 63 error tests assert.

**Classification: confirmed bug + documentation mismatch.** **Not fixed** — the
refactor preserves it, per the bug-discovery protocol. Pinned by
`some_syntax_errors_never_reach_the_error_list_at_all` so it cannot change
silently in either direction, and so that whoever fixes it is forced to update
this entry, `spec/errors.md` and the snapshot manifest together.

The natural owner is **M3 (Diagnostics Unified)**, whose whole premise —
"cero silent failures", one diagnostic model per conceptual error — this
violates. It may be worth fixing earlier as its own commit, since it is a
user-visible defect rather than architectural debt.

### 5.18 The frontend is compiled twice, from the same source — *architectural debt*, medium (M10 input)

`src/lsp_main.rs` opens with its own `mod ast; mod lexer; mod lsp; mod parser;
mod token; mod type_checker;`. The `sz-lsp` binary therefore does **not** depend
on the `serez_code` library — it recompiles five frontend modules into a second,
independent crate.

Found because M1.1.3's `pub use depth::MAX_PARSE_DEPTH` warned as an unused
import: the library needs the re-export (tests name
`serez_code::parser::MAX_PARSE_DEPTH`), and in the binary `parser` is a private
module of a binary crate, where the same re-export reaches nobody. One
`#[allow(unused_imports)]` with a comment holds it for now.

M10 asks that "LSP shares frontend". It shares the *source text*, not the
*crate* — which means every `pub`/`pub(crate)` decision in the frontend has to
be correct under two different module roots at once, and nothing enforces that
except the build. Making `sz-lsp` depend on the library the way `sz` does would
remove the whole class of problem.

**Not fixed** — changing a binary's crate structure is not a parser refactor.

### 5.19 `spec/operators.md` states the wrong bitwise precedence — **documentation mismatch**, medium (found in M1.14)

The normative table lists thirteen levels tightest-to-loosest, and level 8 is
`` `&`, `^`, `|` `` — one level, three operators. Line 156 then states that
operators at the same level are left-associative. Read literally, that makes
`1 | 2 ^ 3` mean `(1 | 2) ^ 3`, which is `3 ^ 3` = `0`.

The implementation ranks them as three separate levels, C-style. Measured
against the 10.0.0 binary:

```
sz --eval 'out (1 | 2 ^ 3);'   ->  1     # = 1 | (2^3);  (1|2)^3 would be 0
sz --eval 'out (1 ^ 2 & 2);'   ->  3     # = 1 ^ (2&2);  (1^2)&2 would be 2
```

So `&` binds tighter than `^`, which binds tighter than `|`, and
`Precedence::{BitAnd, BitXor, BitOr}` are three distinct variants.

**The contradiction is registered rather than resolved**, per the roadmap's
source-of-truth rule: implementation, tests and specification disagree, and
choosing between them silently is exactly what that rule forbids. What the
evidence says, for whoever decides:

- The implementation matches every C-family language, and `spec/operators.md`
  was written recently (`docs(spec): freeze the operator contract`, 2026-08-28)
  against an implementation that already behaved this way.
- Changing the *implementation* to match the document would be a breaking
  change to any program using `|` and `^` in one expression.
- So the likely correct fix is to the document, splitting level 8 into three.
  That is a documentation change and safe — but it is still a decision about
  what the language promises, so it belongs to M7, not to a parser refactor.

Two further gaps in the same table, found by the same audit:

- **`is` is absent.** `token_precedence` maps `KwIs` to `LessGreater`, so
  `a is string == b` parses as `(a is string) == b`. The table says nothing.
- **Member access, call and index are absent.** `.`, `?.`, `(` and `[` are the
  two tightest levels in the implementation (`Call`, `Index`), above unary. A
  precedence table that omits them cannot be used to predict how
  `-a.b[c]()` groups.

**Not fixed.** No code changed in M1.14 — it was an audit, and D2 forbids
touching the algorithm during M1.

### 5.20 The parser performs one semantic check, and it covers 7 of 20 namespaces — *semantic debt*, medium (found in M1.15)

M1's Definition of Done asks for no semantic validation in the parser. There is
exactly one, and the milestone audit is what surfaced it: `is_reserved_name`
rejects a `class`, `interface` or `enum` named `Task`, `Time`, `DateTime`,
`System`, `Gui`, `Dec` or `Media`.

It is semantic, not syntactic. Those names are not keywords — the lexer returns
them as `Ident`, and `class Task {}` is well-formed source. It is rejected
because the name collides with the runtime's namespace table, which is a fact
about the program's meaning.

And the list is arbitrary. The evaluator exposes twenty namespaces; seven appear
here. Measured against the 10.0.0 binary:

```
class Task {}    -> SZ2000, "'Task' is a reserved system namespace"
class Gui {}     -> SZ2000
class Math {}    -> accepted
class File {}    -> accepted
class Socket {}  -> accepted
class Crypto {}  -> accepted
```

And shadowing an unguarded one demonstrably works:

```
class Math { public int hi() { return 7; } }
let m = new Math();
out m.hi();            ->  7
out Math.floor(3.7);   ->  3     # the namespace still resolves
```

So the guard is not preventing a collision the language cannot survive. Which
seven names it covers looks accidental rather than designed.

**Not fixed.** Deleting the check is a compatibility question — a program that
today gets a clear message would start failing later and less clearly — and
extending it to all twenty is a breaking change for anyone with a `class File`.
Either is M4's or M7's decision. M1.15 moved the function from the façade to
`classes.rs`, where its only three callers are, and wrote the finding at the
site so it cannot be mistaken for settled design.

### 5.22 An enum match pattern had no position at all — **fixed in M2.3.3**, low

`parse_single_match_pattern` built the `Enum.Variant` node with
`Span::point(0, 0)` — the "unknown" span — although the variant's real position
was in `self.current_token` two lines above. Every other `DotCall` in the
language carries its position, and `evaluator/classes.rs` reads exactly that
field when a dispatch fails, so a failure on an enum pattern reported `0:0`.

Unlike §5.17 and §5.19 this one was fixed rather than recorded, because fixing
it *is* M2's charter — a node that has a position and does not carry it is the
inconsistency the milestone exists to remove — and because the fix is confined
to one construct.

**Correction: no user-visible diagnostic changes, and the first draft of this
entry said one would.** The claim was that a failure on an enum pattern would
stop reporting `0:0`. Checked rather than assumed, and it is wrong — see §5.24.
`match_pattern` discards the error before any diagnostic is built, so the
position was never surfaced and still is not. The fix is real but latent: the
node now carries what it always should have, and it will matter when §5.24 is
addressed.

Precisely scoped, and the snapshot measured that: **4 of 490 corpus files**
changed, all four containing an `Enum.Variant` pattern, with every diagnostic
hash identical.

### 5.24 A match pattern that fails to evaluate is silently treated as "no match" — *semantic debt / silent failure*, medium (found in M2.3.3)

`evaluator/expr.rs:1592`, evaluating a literal match pattern:

```rust
let lit_ref = match self.eval_expression(lit_expr) {
    EvalResult::Value(v) => v,
    _ => return false,          // every failure becomes "did not match"
};
```

Any error raised while evaluating a pattern — an undefined name, a bad member,
a thrown exception — is discarded and reported to the `match` as a
non-match. Measured:

```
enum Direction { North, South }
let d = Direction.North;
let r = match d {
    Nonexistent.Nope => "x",    # `Nonexistent` does not exist
    _ => "other",
};
out r;                          # -> "other".  No error, no warning, nothing.
```

A typo in a pattern silently falls through to the next arm. Combined with the
fact that `match` is not checked for exhaustiveness (`spec/control-flow.md`), a
misspelled variant can send a program down a `_` arm with no signal at all.

Found while trying to *demonstrate* the §5.22 fix and failing to make any
diagnostic appear — the fix was correct, and the reason nothing changed is this.

**Not fixed.** Deciding what a failing pattern should do is a semantics
question: raise, or treat as non-match and say so. It touches `match`'s contract,
so it belongs to **M7**, with the diagnostic half belonging to **M3**'s "zero
silent failures".

### 5.23 Three kinds of span site, and the rule for telling them apart (M2.3.3)

Working through the 24 construction sites turned up a distinction the charter
does not make, and it is worth writing down because the wrong choice at any one
of them is invisible:

| Kind | Example | What it gets |
|---|---|---|
| **Parsed** | a call, an infix expression, a dot call | `span_to_here(open)` — a real extent, opening token to cursor |
| **Copied** | the `a.b` rebuilt when desugaring `a.b += x` | the original node's span, inherited whole |
| **Synthetic** | the `+ 1` in `i++`, which has no `+` in the source | `Span::point(…)` — a position with no extent, because there is no source text to point at |

The synthetic sites are the interesting ones: five of the 24 (four in the façade
for `++`/`--`, one in `loops.rs` for a `for` step) build nodes the programmer
never wrote. Giving them an extent would be a lie — `span_to_here` would hand
them the text of the statement they were desugared *from*. A point is the honest
answer, and `Span::point`'s documentation says so.

### 5.25 M2.4 and M2.5 both missed two statement forms — *method deficiency*, low (found in M2.7)

`let [a, b] = xs;` and `let {k} = d;` are statements —
`Statement::LetDestructureArray` and `Statement::LetDestructureDict` — and
neither M2.4 (declarations) nor M2.5 (statements) gave them a span.

The cause is how those molecules enumerated their work: by listing AST types
whose *names* contain `Statement` or `Declaration`. These two are named
`LetDestructureArray` and `LetDestructureDict`. They fell through the gap
between two passes that each assumed the other had them.

Found only in M2.7, while triaging what looked like a list of *components* and
noticing two entries that were nothing of the kind. Fixed there.

**The lesson is about method, not about these two types.** An enumeration by
name is a guess about naming discipline. The reliable enumeration was available
the whole time — the variants of `enum Statement` — and would have caught it.
Later milestones that sweep a category should enumerate from the type system,
not from a naming convention.

---

### 5.30 — §0 of this file had drifted three milestones behind its own ledger — *documentation mismatch*, **medium** (found on re-entry, 2026-09-02)

The file's job is to let a session with no conversational memory resume the work.
Its §0 said **"Current milestone: M3 — Diagnostics Unified. IN PROGRESS"** and
**"Next molecule: M3.2"**, while the milestone ledger eight lines below it, in
the same section, said M3 was **COMPLETE** and M4 **BLOCKED**, and §9F.4 gave the
three reasons why.

The mismatch is not cosmetic, and the failure mode is specific: §0 is the part a
resuming session reads first and trusts most, so a session following it would
have re-implemented M3.2 — the `Diagnostic` model — on top of the one that
already exists, and would have discovered the collision only after writing the
code. The rows disagreeing were the *authoritative* row and the *summary* row of
the same table.

**Cause.** §12's last line already prescribes the fix — *"update §0 whenever the
next authorized molecule changes"* — and M3.2 through M4.3 each updated the
ledger, the milestone body and the commit table without touching the header. The
rule was written; keeping the header current simply lost to keeping the evidence
current, seven molecules running.

**Corrected here**, along with two sections that had gone stale the same way but
carry no resumption risk, because they are dated evidence rather than
instructions: §2 (codebase size, an M0 measurement) and §3.2 (diagnostics, which
describes exactly the world M3 dismantled). Both are now marked as M0-era and
point at the current figures rather than being rewritten — a frozen baseline is
worth more than a tidy one.

**The structural lesson, for whoever closes M5.** A summary that can disagree
with its own detail will. §0 is derived information: every row in it is stated
authoritatively somewhere else in this file. The cheap guard is to make updating
§0 the *last* step of a milestone audit rather than an independent chore — the
audit already re-reads everything §0 summarises.

---

### 5.31 — a class and a namespace of the same name both resolve, in the same program — *compatibility hazard*, **medium** (probed 2026-09-02, pre-existing)

§9F.0 asserted this. It is now run:

```
public class Math {
    public Math(int v) { this.v = v; }
    public int give() { return this.v; }
}
let m = new Math(42);
out m.give();        // 42
out Math.floor(3.7); // 3
```

**Exit 0. Both lines print.** One program holds two unrelated things called
`Math`, and which one a site gets is decided by the *shape* of the site — `new`
and instance dispatch reach the class, static dispatch reaches the builtin. The
same file with `Task` in place of `Math` is rejected at parse time. Nothing about
the language distinguishes the two cases; only the seven-name list does.

This is the concrete cost of the 7-of-22 rule, and it is the evidence for §9F.4(c).

### 5.32 — the reserved-name rejection emits two spurious follow-on errors — *diagnostic quality*, **low** (found 2026-09-02, pre-existing)

The rejection is fatal *mid-declaration*: `parse_class_declaration` returns
`None` with the class body still unconsumed, and the body is then re-parsed as
top-level expressions. So this three-line file:

```
class Task {
    public Task(int v) { this.v = v; }
}
```

produces the real diagnostic **and two inventions** — `Unexpected token '}':
expected an expression`, twice, pointing at the two closing braces. A reader is
told about three problems when there is one. The identical file with an
unguarded name (`Math`) parses cleanly, which isolates the guard as the cause.

**This matters to §9F.4(a) as an argument, not just an annoyance.** The reason
the cascade exists is precisely the reason the check is in the wrong place: a
parser that rejects a name has to abandon a structure it was midway through
building. A post-parse phase would parse the class normally, report one error
against a complete node, and emit nothing else. Option (a) does not merely move
the check — **it deletes this finding as a side effect.**

### 5.33 — how much code relies on dynamic name resolution, measured — **the number DEC-M4-002 needed**

`src/semantic/scopes.rs` (M4.7.1) models lexical scope without reporting
anything; `tests/scope_resolution.rs` (M4.7.2) runs it over the corpus. Both are
leaves: no product consumer, no diagnostic, nothing rejected.

The model matches the runtime where it matters, and each rule was probed against
the 10.0.0 binary rather than assumed:

| Probe | Runtime | Model |
|---|---|---|
| Forward call to a function declared later | resolves | top-level names are position-independent |
| Local read before its own `let` | `SZ4001` | not bound |
| Nested `fn` called before its declaration | `SZ4001` | not bound |
| Closure reading the enclosing function's local | resolves | bound |
| **Function reading its caller's local** | **resolves — prints 42** | **free** |

**Results.** In-repo corpus, 486 files conclusively analysed (28 excluded for
containing `import`, since a name may legitimately come from another module):

| | In-repo | With the 8 ecosystem packages |
|---|---|---|
| Files conclusively analysed | 486 | 583 |
| Files with an unaccounted use | 24 (4.9%) | 31 (5.3%) |
| Uses **inside a function or method** | **4** | **38** |
| Uses at a file's top level | 52 | 52 |

**The four in-repo uses inside a function are all deliberate.** They are the
fixtures written to contain an undefined name — `ghost_variable`, `Fantasma`,
`PhantomClass`, `CatchMissing`, `MissingParent`, `InheritanceCycleB`. The model
finding exactly the names those fixtures exist to hold, and nothing else, is the
validation: 486 real files produced no unexplained result.

**So the corpus does not rely on dynamic resolution at all.** That is the answer
DEC-M4-002 was waiting for, and it points strongly at option B — advisory
reporting would be silent on every legitimate file in this repository.

**The ecosystem's 34 additional uses split into two kinds**, and the split
matters more than the total:

  * **Cross-file references inside one package.** `serez-ui/src/renderer_gui.sz`
    calls `cssIndexOf`, declared `export fn` in `css.sz`, without importing it;
    the file is consumed by one that does. A *per-file* resolver flags these; a
    module-aware one would not. This is a constraint on how a resolver must be
    built, not evidence of reliance — recorded so it is not double-counted.
  * **A name that does not exist anywhere.** See §5.35.

**Why the figure is a floor.** Files with `import` are excluded rather than
guessed at, and every remaining ambiguity in the model resolves toward "bound".
Under-reporting is the safe direction for a measurement whose conclusion is that
a change is affordable.

### 5.34 — three security fixtures assert nothing, and pass — *test deficiency*, **high** (found in M4.7.2)

`tests/sec_crypto.sz`, `tests/sec_crypto_ed25519.sz` and `tests/sec_tensor.sz`
are written in framework style — `test("…", () => { … assert(…) })` — but the
runner **does not prepend `framework.sz` to `sec_*.sz`**. It prepends it to
`unit_*.sz` and `ai_*.sz` only (`run_tests.ps1:388, 633-637, 664, 676`).

So each of the three fails on its first line:

```
$ sz tests/sec_crypto.sz
❌ ERROR [SZ4001]: Variable not found: test
```

These are registered as **error tests**, whose contract is "exit non-zero and
print a `❌` line". A missing `test` function satisfies both. All three have
passed every run, for the wrong reason, and **23 assertions across them have
never executed**:

| Fixture | `test` blocks | `assert` calls | What it claims to cover |
|---|---|---|---|
| `sec_crypto.sz` | 7 | 7 | sha256/md5/base64/hex/hmac argument validation |
| `sec_crypto_ed25519.sz` | 8 | 8 | malformed key and signature handling |
| `sec_tensor.sz` | 8 | 8 | index arity, non-uniform and non-numeric input |

**This is the same failure mode the runner's own integrity guard exists to catch**
— a suite passing for a reason unrelated to what it tests — and `MATURITY_AUDIT.md`
already records one instance of it (the bash runner accepting any non-zero exit).
It caught that one and not this one, because the guard checks the *runner*, not
whether a fixture's category matches the way it is written.

**Not a security regression.** Run correctly, with the framework prepended, all
23 assertions **pass**. Nothing was broken and hidden; the coverage was simply
fictional. There is no `unit_sec_crypto` or `unit_sec_tensor`, so this coverage
exists nowhere else in the suite.

Found by M4.7.2, whose corpus walk reported `test` as an unaccounted name in
exactly these three files after the framework composition was modelled correctly
for the categories that do receive it. **Fixed in §5.37**, with a guard.

### 5.37 — §5.34 fixed, and a guard so it cannot recur

The three fixtures were **renamed** rather than rewritten:
`sec_crypto.sz` -> `unit_sec_crypto.sz`, `sec_crypto_ed25519.sz` ->
`unit_sec_crypto_ed25519.sz`, `sec_tensor.sz` -> `unit_sec_tensor.sz`. The
`unit_sec_*` category already exists for exactly this — framework-based safety
tests — and both runners route it through the framework. The files' contents were
not touched, so what now runs is what was written and never executed.

**Nothing about the suite's shape moved.** Both runners report **499 passed / 0
failed / 0 skipped**, and every per-category total is identical in both reports —
`security` is still **104**. The rename moves three files between two globs that
feed the same category, which is why the count is unchanged while the coverage is
not: 23 assertions that never ran now run, and pass.

Manifests: `parser_ast.manifest` re-keyed three rows with **identical hashes** —
same content, new name. `diagnostic_render.manifest` **lost** three rows, which is
the substantive part: those fixtures no longer produce a diagnostic, because they
no longer fail. The three deleted rows read `1 / 46 / d97fdc04bee4b1a9` —
identical exit, identical byte count, identical hash. The manifest had been
recording the same "Variable not found: test" three times over, and that was
legible to anyone who looked.

**The guard.** `tests/diagnostic_render.rs::no_error_fixture_is_written_in_the_frameworks_style`
asserts that no `err_*.sz` or `sec_*.sz` fixture contains a line beginning
`test(`. The fix repairs three files; the guard covers every file added after
today. It was verified to fail: a probe fixture in framework style was added, the
test named it, and the probe was removed — the M1.0.2 method, because a net
nobody has seen fail is not evidence.

### 5.35 — `serez-ui` calls `Int.parse`, and `Int` does not exist — *confirmed bug, ecosystem*, medium (found in M4.7.2)

`serez-ui/src/layout.sz` calls `Int.parse(...)` at six sites (lines 65, 70, 85,
96, 99 and one more), and `Int` is not a runtime namespace, not declared in the
package, and not imported:

```
$ sz -e 'out Int.parse("42");'
❌ ERROR [SZ4001]: Variable not found: Int
```

The namespace is `Math`; there are 22 and `Int` is not among them. Every one of
those six call sites raises `SZ4001` when reached.

**serez-ui's 36 tests pass**, and the ecosystem canary reports the package green,
because no test exercises those paths. The two copies bundled under
`serez-pack/dist/` carry the same code, so the same defect ships three times in
the measured tree.

**This is the single most direct piece of evidence for DEC-M4-002 in the whole
register.** A static resolver — even an advisory one, option B, breaking nothing
— would have printed six diagnostics naming these lines. Thirty-six passing tests
and a green canary did not. The finding was produced by a resolver written for a
completely different purpose, on its first run.

**Not fixed here.** It is a defect in a separate repository, outside this
roadmap's tree, and fixing another project's source from a language-milestone
molecule is exactly the scope creep the protocol forbids. Recorded for the
ecosystem owner, and assigned to **M10**, which owns ecosystem compatibility.

### 5.36 — the measurement got two things wrong before it got them right — *method note*

Recorded because both errors are the kind a later measurement will repeat, and
both were caught only by reading the output rather than trusting it.

**A unit test file is not a program.** The first run reported `test` as an
unaccounted name **2,003 times** and `summary` 162 times — 39.5% of the corpus
"affected". Every one was an artefact: the runner composes `framework.sz` with
each `unit_*.sz` and `ai_*.sz` before running it, so those files are fragments,
and analysing a fragment as a whole program measures the analysis rather than the
code. Modelling the composition dropped the figure from 39.5% to 4.9%.

**`x is int` is not a read of `int`.** The parser lowers the `is` operator to
`Infix("is", x, Identifier("type_name"))` (`parser/expressions.rs:555`), so a
naive expression walk sees `int`, `string`, `bool`, `decimal`, `array`, `dec`,
`any` and `null` as free names. Both fixes carry a regression test.

**The lesson is the ratio.** The uncorrected measurement said a fifth to two
fifths of the corpus depended on dynamic resolution; the corrected one says
effectively none of it does. Those two numbers argue for opposite decisions on
DEC-M4-002. §9F.4 refused to produce this number from a half-correct model on the
grounds that a confident wrong number is worse than none — and the first two runs
of the correct model are what that warning looks like in practice.

**Also:** the measurement rediscovered §5.15's stack ceiling on its first run,
`STATUS_STACK_OVERFLOW` on the corpus depth fixtures. §5.15 was written so the
next frontend test would not have to. It did anyway — a note in a roadmap does not
reach the person opening a new file, so the constant now carries the explanation
at its definition site, where a reader of that file will meet it.

---

## 6. Carried-forward debt from `MATURITY_AUDIT.md`

`MATURITY_AUDIT.md` remains the register; this is the roadmap-facing digest of
what is still **open** there.

| Item | Severity | Milestone that owns it |
|---|---|---|
| Free variables resolve dynamically; undocumented in README; `--check` does not flag them | **critical, open** | M4 / M7 — needs an explicit product decision under `spec/compatibility.md` |
| Property schemas not enforced after construction (typed fields accept other types later) | high | M5 |
| Private access compares against the runtime receiver class, so a subclass can reach a parent's private members | high | M7 |
| `EvalResult` mixes values, control flow, throw and an untyped `Error` sentinel | medium | M6 |
| `fetch` remains reachable under lockdown (deliberate, breaking to change) | high | M7 / M9 |
| Non-atomic package installation; no lockfile, integrity or signature policy | high | M9 |
| LLVM backend parity unproven, feature-gated, absent from the CLI | high | M10 |
| No benchmark regression budget in CI, no stored baseline | medium | M10 |
| CI does not run the ecosystem canary | high | M10 |
| Generators accumulate into an unbounded vector (ceiling measured, deliberately not added) | medium | M9 |

Two milestones are **partially pre-empted** by work already landed, and their
audits must account for it rather than redo it:

- **M6** — `handles.rs` (socket/GPU/memory id registries), `GuiRuntime` and
  `modules.rs` were already extracted out of `Evaluator` in the last ten commits.
  `Evaluator` is nevertheless still **48 fields** wide, spanning language state,
  execution state, caches, and seven host services.
- **M9** — depth ceilings, string/crypto/tensor/allocation caps, ZIP traversal
  checks, JSON-RPC body bounds and a panic-site classification all exist. What
  does not exist is **fuzzing or property testing of any kind**.

---

## 7. Architectural decisions taken so far

| # | Decision | Rationale | Taken at |
|---|---|---|---|
| D1 | M1 extracts via `src/parser/mod.rs` + siblings opening `impl super::Parser` | Rust module privacy keeps private fields reachable with no visibility edits; `src/evaluator/` already proves the pattern in this repo | M0 |
| D2 | M1 does **not** change the expression-parsing algorithm | Precedence climbing is already in place; changing it risks silent precedence/associativity drift and belongs to its own project with differential testing | M0 (restating the roadmap) |
| D3 | M1's first goal builds a differential AST harness before moving any code | All 48 AST types derive `Debug` and none contains a `HashMap`, so `{:?}` is deterministic and diffable across 632 in-repo `.sz` files | M0 |
| D4 | `MATURITY_AUDIT.md` stays the finding register; this file stays the progress ledger | Two documents with one job each rather than one with two | M0 |

Nothing here was chosen by the project owner; these are the working assumptions
M0 proposes. D1–D4 are open to reversal.

---

## 7A. Pending decisions register

A **pending decision** is a choice with several defensible answers whose
consequences differ — architecture, language design, semantics, compatibility,
public behaviour, syntax, or security policy. This roadmap does not take them.
It records them so they can be taken deliberately, later, against the evidence.

**The operating rule.** A pending decision may leave a molecule, a goal or a
whole milestone `PARTIAL` or `BLOCKED`. It must not stop work that does not
depend on it. Where the current behaviour can be preserved while independent work
continues, it is preserved. **No provisional implementation may quietly turn a
recommendation into a decision** — if the work cannot proceed without choosing,
it is blocked and says so.

Every entry carries a stable identifier (`DEC-<milestone>-<nnn>`) that never
changes and is never reused, and the same fields: problem · current behaviour ·
measured evidence · alternatives · trade-offs · architectural impact · semantic
impact · compatibility · impact on tests, specs, LSP, runtime and ecosystem ·
**recommendation, marked as a recommendation** · exactly what it blocks.

| ID | Subject | Status | Blocks |
|---|---|---|---|
| **DEC-M4-001** | Where the reserved-name check runs | **OPEN** | M4.5.* (the semantic phase); the landing site for DEC-M4-003 |
| **DEC-M4-002** | Whether an unresolved free variable is a diagnostic | **OPEN** | M4.7.3+ (resolver reporting); the M7 entry for scope semantics |
| **DEC-M4-003** | Whether the reserved-name guard covers all 22 namespaces | **OPEN** | M4.6.1 — and is ordered after DEC-M4-001 |
| **DEC-M4-004** | What the editor's outline should show | **OPEN** | the LSP's migration onto `semantic::declarations` |
| **DEC-M5-001** | Whether a nullable value at a non-nullable parameter is reported | **OPEN** | nothing — a question to answer, not a gate |
| **DEC-M5-002** | Whether a numeric type widens at a parameter | **OPEN** | nothing |
| **DEC-M5-003** | Whether an unknown type name is diagnosed | **OPEN** | nothing; option B depends on DEC-M4-001 |
| **DEC-M5-004** | Whether a declared field type is a constraint or a default | **OPEN** | nothing |
| **DEC-M5-005** | Whether a declared class type accepts a subclass | **OPEN** | nothing |
| **DEC-M6-001** | How a runtime service raises an error and allocates a value | **OPEN** | the rest of M6 — moving namespace dispatch off `Evaluator` |

---

### DEC-M4-001 — Where does the reserved-name check run?

**Problem.** The parser performs exactly one semantic validation. M4's charter is
to move semantic work out of the parser, and there is no behaviour-preserving
route for this particular move: every available destination changes something
observable.

**Current behaviour.** `Parser::is_reserved_name` (`src/parser/classes.rs:55`)
rejects 7 names at three sites — `parse_class_declaration:80`,
`parse_interface_declaration:337`, `parse_enum_declaration:449`. The error is
**fatal at parse time** and abandons the declaration: the function returns `None`
and the class never enters the AST.

**Measured evidence.**

  * Exactly **one** file in the 491-file tracked corpus is rejected by this
    guard: `tests/err_task_reserved_class.sz`, the fixture that asserts it.
  * That fixture occupies **one row** of `diagnostic_render.manifest` (exit 1,
    222 bytes) and one of `parser_ast.manifest`. Those two rows are the entire
    measured blast radius of moving the check.
  * The rejection currently emits **three diagnostics for one problem** (§5.32):
    the real error plus two spurious `Unexpected token '}'` inventions, because
    the class body is left unconsumed and re-parsed as top-level expressions.
    The 222 bytes are large for that reason.

**Alternatives.**

| # | Option | Consequence |
|---|---|---|
| A | **New fatal phase** between parser and type checker | Class parses normally; one error against a complete node; §5.32 disappears. New public surface: a phase label in rendered output, a diagnostic code range, a position in the D6 order |
| B | **Leave it in the parser** | Nothing observable changes. The parser keeps its only semantic rule; M4's charter is not met for this item; §5.32 stays |
| C | **Move it to the type checker** | **Not viable.** `spec/types.md` makes checker findings advisory, so `class Task {}` would begin to *run*. Breaks a documented contract |

**Trade-offs.** A is the only option that satisfies the charter, and also the only
one that costs public surface. B is free and leaves the milestone honestly
incomplete. C is listed to be ruled out explicitly rather than rediscovered.

**Architectural impact.** A introduces the pipeline's first post-parse fatal
stage — the slot every later semantic validation would use, which is why the
choice matters beyond this one check. B leaves the pipeline as it is.

**Semantic impact.** Under A, *when* the error is reported moves; *which*
programs are rejected does not. The set of accepted programs is identical under A
and B. Only C changes it, by making the error non-fatal.

**Compatibility.** A is not breaking for program acceptance. It **is** a visible
change to diagnostic output, which `spec/compatibility.md` governs.

**Impact by area.**

| Area | Under A |
|---|---|
| Tests | 2 manifest rows on 1 fixture; both improve (3 diagnostics → 1) |
| Specs | New phase documented in `spec/errors.md`; the reserved-name rule stated in `spec/classes.md`, which does not mention it today |
| LSP | Must surface the new phase's diagnostics; it already consumes codes |
| Runtime | None — the phase runs before evaluation |
| Ecosystem | None measured; 0 of 8 packages declare a colliding name |

**Recommendation — this is a recommendation, not a decision.** Option **A**. It
is the only choice that meets the charter, the measured cost is two rows on the
guard's own fixture, and it deletes §5.32 as a side effect. The reason to decide
it deliberately is the new public surface, not the risk.

**Blocked by this decision:** M4.5.1 through M4.5.6 (§9F.5) in full, and the
landing site for DEC-M4-003 — the name list should change once, in its final
home, not twice.

---

### DEC-M4-002 — Should an unresolved free variable be a diagnostic?

**Problem.** Serez has no static name resolution. `name -> declaration` is
answered once, at run time. A function body may read a name it does not declare
and pick up whatever the *caller's* scope happens to hold.

**Current behaviour.** `ScopeStack::lookup` (`src/scope.rs:135`) resolves
dynamically at evaluation time. `--check` cannot flag an unresolved name because
nothing resolves names before running. `MATURITY_AUDIT.md` records this as
**critical, open**, undocumented in the README, and needing an explicit product
decision under `spec/compatibility.md`.

**Measured evidence — §5.33, and it is decisive.** Of 486 in-repo files
conclusively analysed, **four** uses inside a function are unaccounted for, and
all four are the fixtures written to hold an undefined name. The corpus does not
rely on dynamic resolution at all. And §5.35: the resolver's first run found six
call sites in `serez-ui` to a namespace that does not exist, which 36 passing
tests and a green ecosystem canary had not. Producing that number required a model handling closures, `this`,
class bodies, `for`-in bindings, `catch` bindings, destructuring and generators;
a half-correct model yields a *confident wrong number*, which is worse input than
none. Building and measuring commit to nothing, so both were **independent of
this decision** and were done.

**Alternatives.**

| # | Option | Consequence |
|---|---|---|
| A | Unresolved name is a **fatal** diagnostic | Strongest guarantee; rejects programs that run today; largest break |
| B | Unresolved name is an **advisory** diagnostic | `--check` reports; exit code unmoved; no program stops working |
| C | **Keep dynamic resolution**, document it as intentional | No break; the hazard becomes a declared contract and moves to M7 |

**Trade-offs.** B is the only option that informs users without breaking any of
them, and it can precede A in a later major. C is honest but permanently accepts
a footgun that `--check` exists to catch.

**Architectural impact.** A and B both require a real resolver — the substance of
M4's charter. C means the semantic layer is established but never adopted for
resolution.

**Semantic impact.** A changes which programs run. B and C do not.

**Compatibility.** A is breaking and needs a major plus a `spec/compatibility.md`
entry. B changes stderr only. C changes nothing but requires a spec statement.

**Impact by area.**

| Area | Under A or B |
|---|---|
| Tests | See §5.33 for the measured corpus exposure |
| Specs | `spec/scopes.md` and `spec/compatibility.md` both need the rule stated |
| LSP | Gains real go-to-definition and unresolved-name reporting — the largest user-visible win |
| Runtime | Unchanged under B; under A the resolver must agree with `ScopeStack` exactly, or the checker and the runtime disagree about the same program |
| Ecosystem | See §5.33 |

**Recommendation — this is a recommendation, not a decision.** **B first**, and
the measurement strengthens rather than merely permits it. Advisory reporting
would be silent on every legitimate file in the corpus, so the "it would break
things" objection has a number against it now: zero. §5.35 is the other half —
the value is not hypothetical, since the first run of a resolver written for
another purpose found a real defect in an official package. A belongs to a major,
if at all, and only once B has shipped long enough to show what it finds.

One constraint the measurement puts on *any* implementation: a resolver must be
**module-aware**, not per-file. `serez-ui` legitimately calls across files within
a package without importing, and a per-file resolver flags those (§5.33).

**Blocked by this decision:** M4.7.3 and everything downstream — resolver
reporting, `--check` behaviour, and the M7 entry for scope semantics. **Not
blocked, and done:** M4.7.1 and M4.7.2.

---

### DEC-M4-003 — Should the reserved-name guard cover all 22 namespaces?

**Problem.** The guard covers 7 of 22 runtime namespaces. Membership looks
accidental, and the 15 unguarded names produce a real collision.

**Current behaviour.** Guarded: `Task`, `Time`, `DateTime`, `System`, `Gui`,
`Dec`, `Media`. Unguarded: `Autodiff`, `Binary`, `Crypto`, `Env`, `File`, `GPU`,
`JSON`, `Math`, `Memory`, `OS`, `Random`, `Regex`, `Socket`, `Tensor`,
`Terminal`. The generated table the LSP uses (`lsp/builtins_gen.rs`, produced
from the evaluator) lists all 22 — so the parser and the editor already disagree
about what a namespace is.

**Measured evidence.**

  * §5.31, **run**: a program declaring `class Math`, calling `new Math(42)`, and
    calling `Math.floor(3.7)` prints `42` then `3` and **exits 0**. Two unrelated
    things of the same name coexist, told apart only by the shape of the call
    site.
  * Collisions across every tracked `.sz`/`.szx` in the corpus (including `std/`,
    `apps/`, `benchmarks/`) and every source file in all **8 official ecosystem
    packages**: **1** — `tests/err_task_reserved_class.sz`, on a name already
    among the 7. **Zero in the ecosystem.**

**Alternatives.**

| # | Option | Consequence |
|---|---|---|
| A | **Extend to 22** | Closes §5.31. Breaking under SemVer, **0 measured victims**. 15 names become illegal for `class`/`interface`/`enum` |
| B | **Leave at 7** | No break. §5.31 stays; the rule stays arbitrary; parser and LSP stay disagreed |
| C | **Remove the guard** | Breaking in the other direction: the 7 currently-rejected names begin to compile with a silent collision. Makes §5.31 the rule rather than the exception |

**Trade-offs.** A's break is real but has no measured victim *in code reachable
from this machine* — third-party code is not covered by that measurement, and
that limit is the substance of the decision. B is free and leaves a footgun the
language cannot detect for the user. C is coherent only if collisions are
declared acceptable, which contradicts having a guard at all.

**Architectural impact.** Under A the list should be **generated from the same
source as `lsp/builtins_gen.rs`** rather than hand-written, so the parser and the
editor cannot drift apart again. That is the durable part of A; the 15 extra
names are the visible part.

**Semantic impact.** A and C both change which programs are accepted. B does not.

**Compatibility.** A and C are breaking and need a major plus a
`spec/compatibility.md` entry. Note the guard covers only `class`, `interface`
and `enum`; variables and functions may already take these names, which this
decision does **not** settle — recorded here so it is not mistaken for settled.

**Impact by area.**

| Area | Under A |
|---|---|
| Tests | 15 new rejection fixtures; existing `err_task_reserved_class` unaffected |
| Specs | `spec/classes.md` must state the rule, which it does not today |
| LSP | Converges with the parser instead of diverging from it |
| Runtime | None |
| Ecosystem | 0 of 8 packages affected, measured |

**Recommendation — this is a recommendation, not a decision.** Option **A**, in
the same release as DEC-M4-001 option A, so the rule and its location change
once. If third-party adoption is wider than this machine can see, B is the
defensible hold — and that is the question the recommendation cannot answer.

**Blocked by this decision:** M4.6.1, ordered after DEC-M4-001.

---

### DEC-M4-004 — What should the editor's outline show?

**Problem.** `analyze` parses a `.sz` file, hands the `Program` to the type
checker, and then **throws the parse tree away**: it builds the outline with
`scan_symbols(text, &lines)`, a second lex of the same source
(`lsp/analysis.rs:95,163,243`). The outline, hover, go-to-definition, references
and rename a user sees therefore have no structural relationship to the tree the
compiler built. Migrating them onto `semantic::declarations` is the substance of
M4's charter — and it cannot be done without deciding what the outline is *for*,
because the two derivations do not merely differ in accuracy, they answer
different questions.

**Current behaviour.** The token scanner is not nesting-aware. A `fn` declared
inside a lambda — `test("…", () => { fn int double(int n) { … } })` — is reported
with `container: None`, as though it were a top-level symbol.

**Measured evidence (M4.1, §9F.2).** Over 483 corpus files, comparing the two
derivations on the four declaration kinds both report:

| Direction | Meaning | Count |
|---|---|---|
| `scan - tree` | outline shows a top-level symbol the tree does not have | **95 files (~20%)** |
| `tree - scan` | outline **omits** a declaration the tree has | **0** |

The asymmetry is the useful part. Nothing is currently hidden from the user —
go-to-definition does not fail on code that compiles — so this is not a
correctness emergency. What the outline does is *over-report*, in a fifth of the
corpus, always in the same direction.

**Why the scanner exists.** `.szx` documents contain JSX, which the parser does
not understand. Tolerating arbitrary broken regions means not relying on
structure, and the token scan is how `.szx` still gets an outline at all. That
justification is sound for `.szx` and does not transfer to `.sz`, where a correct
AST has already been built and discarded.

**Alternatives.**

| # | Option | Consequence |
|---|---|---|
| A | **Top-level only**, from the AST | Honest and simple. A user editing a unit-test file **loses** the nested `fn`s currently listed — 95 files' worth of outline entries disappear |
| B | **All declarations, correctly nested**, from the AST | Nothing is lost and the nesting lie is fixed. Requires deciding a `container` for a declaration inside an *anonymous* lambda, which has no name to be contained by |
| C | **Leave the token scan** | No change. The outline keeps disagreeing with the compiler by construction, and M4's central finding goes unaddressed |

**Trade-offs.** B is the only option that strictly improves on today, and it is
also the only one with an unsolved sub-question: LSP `documentSymbol` is a tree,
and an anonymous lambda is a node with no name. Synthesising one (`"(lambda)"`,
the enclosing call's callee, a line number) is a user-visible naming choice, not
an implementation detail. A is cheap and takes something away from users who have
it today.

**Architectural impact.** A or B removes the fourth independent derivation of
what a program declares — the one §9F.0 identified as the most surprising, since
it re-lexes source the compiler has already parsed. This is the largest single
step remaining in M4's charter.

**Semantic impact.** None. The language is unaffected; this is tooling.

**Compatibility.** Not governed by SemVer for the language. It **is** a visible
change to an editor experience, and `spec/compatibility.md` has no clause for
tooling behaviour — which is itself a gap worth noting for M10.

**Impact by area.**

| Area | Under A or B |
|---|---|
| Tests | `semantic_divergence` becomes trivially satisfied and should be replaced by an equivalence assertion rather than deleted |
| Specs | None today; `spec/` has no LSP document |
| LSP | `scan_symbols` stays for `.szx` and stops being used for `.sz`. `SymbolInfo.detail` is a source slice the AST does not carry and must be derived from spans |
| Runtime | None |
| Ecosystem | None |

**Recommendation — this is a recommendation, not a decision.** Option **B**, with
a lambda's container synthesised from the enclosing call when there is one
(`test("name", …)` -> `test`) and omitted otherwise. It removes the divergence
without taking anything away from users. **A** is the acceptable cheaper answer if
the naming question is not worth settling now. **C** should be rejected explicitly
rather than by default — leaving it is a choice, and after M4.1 measured it, an
informed one.

**Blocked by this decision:** the LSP's migration onto `semantic::declarations`,
which is M4's last unblocked-in-principle goal. **Not blocked:** nothing else —
`semantic::declarations` and `semantic::scopes` are complete and validated
without it.

---

### DEC-M5-001 — Should the checker report a nullable value at a non-nullable parameter?

**Problem.** The checker infers `int?` for a value returned by `fn int? maybe()`.
Passed to a parameter declared `int`, it reports. The runtime accepts, because
the value at run time is an `int`. Whether that report is a defect or the correct
behaviour of a null-aware checker is a design question, and unlike the other four
divergences in §9H.0, **`spec/types.md` takes no position on it**.

**Current behaviour.**

```serez
fn int? maybe() { return 5; }
fn int wants(int n) { return n; }
let m = maybe();
out wants(m);      // ❌ TYPE ERROR [SZ3000] … expected 'int' but received 'int?'
                   // prints 5, exit 0
```

**Measured evidence.** Zero occurrences in the corpus. The whole tracked corpus
emits **3** `SZ3000` findings and all three are ordinary type mismatches; nothing
triggers this path today. So the decision is about what the language should do
for code not yet written, not about existing code — which is the cheapest moment
to take it.

**Alternatives.**

| # | Option | Consequence |
|---|---|---|
| A | **Keep reporting** — a `T?` may be null, so passing it where `T` is required is a real risk | Strict-null behaviour. Without flow analysis it fires on *every* such call, including where the value is provably non-null, so users would learn to ignore it — the exact failure §9H.0 argues against |
| B | **Stop reporting** — match the runtime, which accepts when the value is not null | Removes the false-positive risk; loses the only null-safety signal the checker has |
| C | **Keep reporting, add narrowing** — report only where the value is not provably non-null | The correct answer in a mature checker, and much the largest: it needs flow analysis the checker has no other use for today |

**Trade-offs.** A and C differ only in precision, and the precision is the whole
question: a warning that cannot be silenced by writing correct code is noise
wearing a useful label. B is honest about what this checker is — `spec/types.md`
calls it "a linter that occasionally catches a top-level mistake early" — and
gives up a genuine class of bug.

**Architectural impact.** C introduces flow-sensitivity, which nothing in the
checker has today and which would be its largest structural change. A and B are
each a line.

**Semantic impact.** None under any option. Findings are advisory; the set of
accepted programs does not move.

**Compatibility.** stderr only, under every option.

**Impact by area.**

| Area | Note |
|---|---|
| Tests | `tests/type_agreement.rs` holds the case; under B its `KNOWN_DIVERGENCES` entry is deleted, and the staleness check enforces that |
| Specs | `spec/types.md` must state the rule under any option — it is silent today, which is the underlying defect |
| LSP | Consumes checker findings directly; whatever is decided is what an editor underlines |
| Runtime | Unchanged |
| Ecosystem | No occurrences measured |

**Recommendation — this is a recommendation, not a decision.** **A now, C later,
never B.** The signal is real — a null reaching a non-nullable parameter is a
genuine bug class, and this is the only place the checker can see it. Keeping it
costs nothing today, since nothing in the corpus triggers it. C is where it
should end up, and it should be scheduled as its own project rather than folded
into a milestone about consistency. **Whatever is chosen, `spec/types.md` must
say so** — the real defect is that the document is silent, not that the checker
reports.

**Blocked by this decision:** nothing. The case is recorded in the net with this
identifier, so the behaviour is pinned and documented either way. This is the
first decision in the register that blocks **no** work — it is a question the
roadmap should answer, not a gate.

---

### DEC-M5-002 — Should a numeric type widen at a parameter?

**Problem.** Arithmetic mixes numeric types freely; parameter binding does not.
`1 + 1.5` is a `decimal` without complaint, and `half(1)` against
`fn decimal half(decimal d)` is a `TypeError`. The same two values, the same two
types, opposite answers.

**Current behaviour.** No widening, at a parameter, a return or a constructor.
`int` does not widen to `decimal`; neither does `dec`. Both the checker and the
runtime enforce this, and they agree — this is not a consistency defect, it is a
design question. `spec/types.md` states it and calls declaring `decimal` on a
parameter callers will pass integers to "a trap", recommending `any` "until this
is reconciled".

**Measured evidence.** **15** `decimal`-typed parameters across the tracked
corpus; **0** across all eight ecosystem packages. So the trap is real but
narrow, and no official package would be affected by changing it in either
direction.

**Alternatives.**

| # | Option | Consequence |
|---|---|---|
| A | **Keep exact matching** | No break. The asymmetry with arithmetic stays, and `any` remains the honest annotation for numeric parameters |
| B | **Widen `int` -> `decimal`** (and `int`/`dec` -> `dec`) at a parameter | Matches arithmetic. **Accepts programs rejected today** — additive, so no existing program stops working, but `TypeError` stops firing where a caller relied on it as a guard |
| C | Widen, and warn where precision is lost | The safest version of B and the largest |

**Trade-offs.** B is additive for *acceptance* and subtractive for *rejection*: a
program relying on `half("…")`-style rejection is unaffected, but one relying on
`half(1)` being rejected is not. That is a narrow but real class.

**Architectural impact.** One table in `evaluator::type_matches`, mirrored in
`type_checker::types_compatible`, with `tests/type_agreement.rs` holding the two
to each other.

**Semantic impact.** B and C change which programs run.

**Compatibility.** B and C are breaking in the "accepts more" direction, which
`spec/compatibility.md` still governs.

**Impact by area.** Tests: `spec/types.md`'s widening example and its pinned
runtime test would both invert. Specs: `spec/types.md` "No widening" and "Known
gaps" both rewritten. LSP: inherits the checker. Runtime: the matcher. Ecosystem:
zero measured exposure.

**Recommendation — this is a recommendation, not a decision.** **B**, in a major,
narrowly: `int` -> `decimal` and `int` -> `dec` only, never a lossy direction
(`decimal` -> `int`, or `decimal` -> `dec`). The asymmetry with arithmetic is the
kind of rule users cannot derive and must memorise, and `spec/types.md` already
declines to defend it. C's precision warning is a separate, later question.

**Blocked by this decision:** nothing today. Recorded because M5's charter asks
for each rule to be decided rather than inherited.

---

### DEC-M5-003 — Should an unknown type name be diagnosed?

**Problem.** An annotation is any identifier plus an optional `?`, and nothing
checks that the name exists. `fn any f(Frobnicate x)` parses, loads, is callable —
and rejects every value that reaches it. A misspelled type silently produces a
function that can never succeed.

**Current behaviour.** Accepted, matches nothing, no diagnostic at any phase.
`spec/types.md` documents it under "Known gaps" and shows the example.

**Measured evidence.** Every `fn` annotation across the tracked corpus and all
eight ecosystem packages was checked against the type keywords, the runtime-
recognised names, and every `class`/`interface`/`enum` declared anywhere in the
same tree:

| Scope | Annotations naming an undeclared type |
|---|---|
| Language repo, tracked corpus | **1** — `Frobnicate` in `tests/unit_types.sz`, the fixture that exists to document this behaviour |
| All 8 ecosystem packages | **0** |

So a diagnostic would fire, today, on exactly one file: the one written to prove
the behaviour it would report.

**Alternatives.**

| # | Option | Consequence |
|---|---|---|
| A | **Advisory diagnostic** from the checker | No program stops working; findings are advisory. Needs the set of known type names, which the checker does not collect today (`semantic::declarations` does) |
| B | **Fatal**, in the semantic phase DEC-M4-001 would create | Strongest, and rejects programs that run today — including ones whose bad annotation is never reached |
| C | **Leave it** | The documented gap stays |

**Trade-offs.** A is nearly free and catches a whole bug class at its cheapest
moment. B is more valuable and needs DEC-M4-001 to exist first. C keeps a footgun
the language can detect for the user but does not.

**Architectural impact.** A gives the checker its **first dependency on the
semantic layer** — it needs `declarations` to know which class names exist. That
is a real architectural step and a good one: it is the first consumer M4 built
the layer for.

**Semantic impact.** None under A. B changes which programs run.

**Compatibility.** A is stderr only. B is breaking.

**Impact by area.** Tests: one fixture gains a finding. Specs: `spec/types.md`
"Known gaps" loses an entry. LSP: inherits, and gains a genuinely useful squiggle.
Runtime: unchanged under A. Ecosystem: zero exposure.

**Recommendation — this is a recommendation, not a decision.** **A now, B when
DEC-M4-001 lands.** The measurement makes A close to free, and it is the first
piece of work that would give M4's semantic layer a product consumer — which is
worth something beyond the diagnostic itself.

**Blocked by this decision:** nothing. **Depends on:** DEC-M4-001, for option B
only.

---

### DEC-M5-004 — Is a declared field type a constraint or a default?

**Problem.** `timeout: int = 30;` on a class looks like a type declaration. It is
a **default value** with a type annotation that is never enforced after
construction: `c.timeout = "str"` is accepted, from inside the class and outside
it. Interface fields are checked when the instance is built and never again. A
new field can also be created by assignment — `c.brandNew = 1`.

**Current behaviour.** As above; `spec/types.md` and `spec/classes.md` both state
it. `MATURITY_AUDIT.md` records "property schemas not enforced after
construction" as **high, open**, assigned to M5.

**Measured evidence.** **68** typed field declarations across the tracked corpus;
**0** across all eight ecosystem packages. No official package declares a typed
field at all, so enforcement could not break one.

**Alternatives.**

| # | Option | Consequence |
|---|---|---|
| A | **Enforce on assignment** — the annotation becomes a constraint | Closes the gap. Breaking: any program assigning off-type to a declared field starts failing at run time, and today nothing warns |
| B | **Keep as a default**, and say so louder in the docs | No break. The syntax keeps looking like a guarantee it does not give |
| C | **Enforce, and forbid new fields by assignment** | The full schema story, and much the largest break — `c.brandNew = 1` is a documented, working idiom |

**Trade-offs.** The syntax is the problem: `name: T = v` reads as a typed field in
every language that has it, and here it is a default. B accepts a permanent
mismatch between what the code looks like and what it means. A is where users
already believe they are. C changes an idiom the corpus uses.

**Architectural impact.** A needs a per-class field schema consulted on every
field write — the runtime knows the declaration but does not carry it to the
assignment path.

**Semantic impact.** A and C change which programs run.

**Compatibility.** A and C are breaking and need a major.

**Impact by area.** Tests: 68 declaration sites, unknown how many assign off-type
— that measurement is the natural first molecule if A is chosen. Specs:
`spec/types.md` "Where enforcement stops" and `spec/classes.md`. Runtime: the
field-assignment path. Ecosystem: zero measured exposure.

**Recommendation — this is a recommendation, not a decision.** **A**, in a major,
**not C**. Enforcing a declared field's type is what the syntax already promises,
and the ecosystem cannot be broken by it. Forbidding undeclared fields is a
separate and much larger question that should not ride along. Before implementing
A, measure how many of the 68 sites are ever assigned off-type — that is a
molecule, and it is the same shape as M4.7.2.

---

### DEC-M5-005 — Should a declared class type accept a subclass?

**Problem.** A declared class name matches that class and nothing else.
Inheritance drives dispatch and field layout; the type system does not see it.
`new Derived() is Base` is `false`, and a `Base` parameter rejects a `Derived`.
A function meant to work across a hierarchy must take `any`.

**Current behaviour.** Exact match, in the checker and the runtime alike — they
agree, so this is a design question rather than a consistency defect.
`spec/types.md` records it "as an inconsistency, not defended", and it is pinned
by `a_declared_type_matches_exactly_and_never_a_subclass` in
`tests/runtime_outcome.rs` "so it cannot move in either direction by accident".

**Measured evidence.** Zero programs can rely on subtyping working, since it does
not. The measurable proxy is how much code takes `any` where it means a base
class, and that is not mechanically distinguishable from code that means `any`.
Recorded as unmeasured rather than estimated.

**Alternatives.**

| # | Option | Consequence |
|---|---|---|
| A | **Accept subclasses** at a parameter, and make `is` follow | What every user expects from a language with `class X : Y`. Accepts programs rejected today; changes `is` from `false` to `true` for a subclass, which is a **silent behaviour change in working programs** |
| B | **Keep exact matching** | No break. Inheritance stays a dispatch mechanism that the type system cannot see |
| C | Accept subclasses at parameters, leave `is` exact | Splits the two, and creates a second rule to memorise |

**Trade-offs.** A's danger is not the parameter, it is `is`: a program branching
on `x is Base` today takes the `false` path for a `Derived` and would take the
`true` path afterwards. That is the one change in this whole register that can
alter a working program's output silently. C avoids it at the cost of coherence.

**Architectural impact.** The matcher needs the class hierarchy, which the
evaluator has (`class_registry`) and the checker does not — so under A the
checker needs the semantic layer, the same dependency DEC-M5-003 option A
introduces.

**Semantic impact.** A and C change which programs run; A additionally changes
what `is` returns.

**Compatibility.** Breaking, major, and the `is` half needs an explicit note in
`spec/compatibility.md` because it is silent.

**Impact by area.** Tests: the pinned exactness test inverts by design, and it was
written to make that deliberate. Specs: `spec/types.md` "No subtyping" and
`spec/classes.md`. LSP: inherits. Runtime: `type_matches` gains a hierarchy walk.
Ecosystem: no package declares a class-typed parameter today.

**Recommendation — this is a recommendation, not a decision.** **A**, in a major,
with the `is` change called out separately in the release notes as the silent one.
A language with `class X : Y` whose type system cannot see the `: Y` is teaching
users a rule that exists for no reason they can find. But this is the single
largest semantic change in the register and should be its own project, with
differential testing over the corpus, not a milestone item.

---

### DEC-M6-001 — How should a runtime service raise an error and allocate a value?

**Problem.** M6 gave five services their own state and two of them their own
operations. It could not give any of them their **dispatch** — the code that
answers `Autodiff.backward(...)` or `Socket.connect(...)` — because every one of
those needs three things that live on the evaluator: `alloc` to make a value,
`rt_err_kind` to raise one, and `null_ref` to return nothing. So
`eval_autodiff_namespace` and its fifteen siblings remain `impl super::Evaluator`.

This is the boundary between M6 as done and M6 as chartered, and crossing it needs
a choice about what a service is allowed to depend on.

**Current behaviour.** Every namespace's dispatch is a method on `Evaluator`,
taking `&mut self` and returning `EvalResult`. The service structs hold state; the
evaluator holds the behaviour.

**Measured evidence.** Sixteen namespace dispatch functions across
`evaluator/namespaces_*.rs`, 12,000+ lines. `eval_autodiff_namespace` alone is
~2,300 lines and touches `self.alloc`, `self.rt_err_kind`, `self.null_ref` and
`self.resolve` throughout. This is the largest single body of work left in the
runtime, which is why the choice of mechanism matters more than the mechanism.

**Alternatives.**

| # | Option | Consequence |
|---|---|---|
| A | **Pass `&mut Evaluator`** to service methods | Smallest change; the dependency becomes explicit in signatures instead of implicit in `self`. But a service that takes the whole evaluator is not decoupled from it — it is the same coupling, written down |
| B | **A narrow trait** — `ValueSink`, or similar, with `alloc`, `raise`, `null` | A real boundary: a service depends on three operations rather than on 38 fields, and becomes testable with a stub. Costs a trait object or a generic parameter on every dispatch, and a design pass on what the minimal surface actually is |
| C | **Services return plain Rust `Result`**, and the evaluator adapts at the edge | The cleanest and the largest. Services stop knowing about `EvalResult`, `ObjectRef` and the arena entirely; the evaluator translates. Every one of the 16 dispatches changes shape |

**Trade-offs.** A is cheap and buys little — the plan warns against introducing
abstractions that do not represent a real contract, and A is the opposite failure:
keeping a real dependency while calling the move an extraction. C is the right
architecture and is a project, not a milestone item. B is the middle, and its risk
is that the "minimal surface" is decided by what the first service happens to
need rather than by what services need.

**Architectural impact.** This decides whether Serez's runtime services are
*modules of the evaluator* (A) or *components the evaluator drives* (B, C). It is
the largest open architectural question in the register.

**Semantic impact.** None under any option, if done correctly — this is a
refactor. That is also its risk: 12,000 lines of behaviour-preserving change with
no semantic net beyond the conformance suite.

**Compatibility.** None. No public surface moves.

**Impact by area.** Tests: the existing suite is the only net, and it asserts what
programs print rather than how the runtime is structured — the same gap M1 met and
answered with a differential harness. Specs: none. LSP, runtime, ecosystem: none
if behaviour is preserved.

**Recommendation — this is a recommendation, not a decision.** **B**, and only
after a differential harness exists for runtime behaviour the way
`parser_snapshot` exists for the frontend. The order matters more than the option:
attempting any of these across 12,000 lines without a net that can see a changed
value or a changed error is the highest-risk work in the whole roadmap.

**Blocked by this decision:** the rest of M6 — moving namespace dispatch off
`Evaluator`. **Not blocked:** everything M6.1–M6.4 did, which is why the milestone
is PARTIAL rather than stopped.

---

## 8. Open risks going into M1

| Risk | Why it matters | Mitigation |
|---|---|---|
| A parser extraction silently changes an AST shape | The 490-test suite asserts *output*, not *parse trees*; a mis-nested node that still evaluates the same would pass | M1.0 builds the differential AST harness **first** (D3) |
| A parser extraction changes error text, order or position | 63 error tests assert only that a `❌` line appears, so most wording is unpinned | The same harness snapshots `take_errors()`, not just the AST |
| `parse_expression` and `parse_infix_chain` are mutually load-bearing (fan-in 30 and 2) | Splitting them apart is the highest-coupling move in M1 | Schedule them **last**, after every leaf family is out |
| `parse_visibility_statement` and `parse_abstract_or_sealed_class` are a shared modifier-prefix layer, not class-private | They dispatch into **both** `parse_class_declaration` and `parse_interface_declaration`, so extracting either declaration family first strands them | Extract the modifier-prefix dispatchers as their own molecule **before** interfaces and classes |
| The ecosystem canary needs sibling checkouts | It is the strongest compatibility signal and cannot run in CI today | Run `run_ecosystem.ps1` locally at every M1 goal boundary, not every molecule |
| CI clippy has no `-D warnings` | New warnings introduced by a refactor would not fail any gate | Compare the 186-warning count at each goal boundary |

---

## 9. M1 — Parser Molecular: discovered goal decomposition

Derived from the call graph in §4, ordered by coupling and risk: leaves first,
the shared expression core last.

| Goal | Scope | Risk |
|---|---|---|
| **M1.0** | Frontend skeleton + façade contract pinned. `parser.rs` → `parser/mod.rs` with **no code movement**, plus the differential AST/diagnostic harness over the 491-file corpus | LOW |
| **M1.1** | Parser core extracted: cursor, state, depth accounting, error emission, recovery | MEDIUM |
| **M1.2** | Type syntax: `parse_type_string`, `is_type_keyword`, `parse_dict_type_annotation` | LOW |
| **M1.3** | Module syntax: `import`, `export`, `use permissions` | LOW |
| **M1.4** | Declarations — enums, native | LOW |
| **M1.5** | Declarations — the modifier-prefix layer (`parse_visibility_statement`, `parse_abstract_or_sealed_class`), then interfaces | LOW |
| **M1.6** | Declarations — classes | MEDIUM |
| **M1.7** | Declarations — functions and parameters | MEDIUM |
| **M1.8** | Declarations — variables (`let`/`const` + array/dict destructuring patterns) | MEDIUM |
| **M1.9** | Statements — blocks and flow (`return`, `out`, `throw`, labels, `unsafe`) | LOW |
| **M1.10** | Statements — loops (`while`, `do-while`, `for`/`for-in`) | MEDIUM |
| **M1.11** | Statements — `switch`, `match`, `try` | MEDIUM |
| **M1.12** | Statements — assignment forms (`assign`, index-assign, compound, nested writeback, `try_build_*`) | HIGH — this is where `is_writable_chain` and receiver-writeback shape live |
| **M1.13** | Expressions — literals and collections (array, dict, entry, brace, object patch, interpolated string) | MEDIUM |
| **M1.14** | Expressions — calls, lambdas, arrow functions, `new`, if-expression, `sizeof` | MEDIUM |
| **M1.15** | Expressions — precedence **audit only**, no algorithm change (D2) | LOW |
| **M1.16** | Expressions — prefix/primary dispatcher and infix chain extracted last | HIGH |
| **M1.17** | Façade reduced to orchestration; duplicated paths deleted; M1 milestone audit | MEDIUM |

Definition of Done for M1: `parser/mod.rs` is principally façade and
orchestration; grammar is distributed by responsibility; no semantic validation
sits in the parser; the harness proves byte-identical ASTs and diagnostics; all
seven gates plus the ecosystem canary stay green.

### 9.1 M1.0 — skeleton and façade contract: **COMPLETE**

| Molecule | Action | Outcome |
|---|---|---|
| **M1.0.1** | `tests/parser_snapshot.rs` — walk the corpus, parse each file, hash the `Debug` tree and the diagnostic list into a committed manifest | **done**, 490 files, 34 KB manifest. Also proves the manifest is line-ending independent, since CI checks out CRLF on Windows and LF elsewhere |
| **M1.0.2** | Prove the harness fails: perturb the parser, confirm detection, revert | **done** — see §5.10, the most important result in M1.0 |
| **M1.0.3** | `tests/parser_facade.rs` — pin the public API of §4.1 | **done**, 10 tests. Writing them found §5.11, §5.12, §5.13 |
| **M1.0.4** | `git mv src/parser.rs src/parser/mod.rs`, no other edit | **done**, byte-identical to `HEAD:src/parser.rs` |
| **M1.0.5** | Full gates | **done** — fmt PASS, check PASS, clippy still **exactly 186**, `cargo test` **331/0** (318 + 13 new), `run_tests.ps1` 490/0/0, `run_tests.sh` 490/0/0 |
| **M1.0.6** | Ecosystem canary | **done**, 8/8 |
| **M1.0.7** | Diff review, self-audit, commit | **done** |

Behavior: **UNCHANGED.** The parser file moved and two test files were added;
no parser logic was edited. Verified three ways — the moved file is
byte-identical to its predecessor, the snapshot manifest regenerates identically,
and every pre-existing gate holds at its M0 number.

### 9.2 M1.1 — parser core: **COMPLETE**

| Molecule | Action | Outcome |
|---|---|---|
| **M1.1.1** | Inventory only, no code change | **done** — §4.6. It found §5.17 |
| **M1.1.2** | Token cursor → `parser/cursor.rs` | **done**, 107 lines |
| **M1.1.3** | Depth accounting → `parser/depth.rs` | **done**, 111 lines. It found §5.18 |
| **M1.1.4** | Diagnostic emission → `parser/diagnostics.rs` | **done**, 135 lines |
| **M1.1.5** | Recovery → `parser/recovery.rs` | **DROPPED — see below** |
| **M1.1.6** | Full gates + ecosystem; commit | **done** |

`mod.rs` went from 3,936 lines to 3,673. Behavior: **UNCHANGED** — the snapshot
manifest matched after every one of the three extractions.

**Why M1.1.5 was dropped.** `synchronize` is 28 lines with exactly one caller,
`parse_program`, eight lines above it. Extracting it would produce a file
smaller than its own module doc, called from the loop it belongs to, reducing no
coupling and clarifying no ownership. `parse_program` drives the statement loop
and `synchronize` is that loop's error branch: one responsibility, correctly
adjacent.

This is the roadmap's own rule — "molecular NO significa muchos archivos", "no
crear archivos diminutos sin frontera conceptual", "NO limitarse a dividir
parser.rs en archivos". A plan written before reading the code proposed it; the
code says no. Recovery gets a module when it acquires a *policy* worth naming,
which is M1.17's or M3's call, not a file move's.

### 9.3 M1.2 — type syntax: **COMPLETE**

`parse_type_string`, `parse_dict_type_annotation` and `is_type_keyword` →
`parser/types.rs` (93 lines).

The boundary is the one `spec/types.md` already draws, and it is the one worth
stating: **parsing a type belongs to the parser, deciding whether two types are
compatible does not.** Nothing in the file asks what a type means. A type
reaches the AST as the `String` that was written, which is why
`parse_type_string` is nine lines — the parser has no opinion to be wrong about.
When M5 gives types a real representation, the syntax half of that change lands
here.

Called from seven places (native declarations, `let`, function statements and
their parameters, arrow functions, interfaces, classes), which is what makes it
shared rather than part of any one of them.

Snapshot: identical.

### 9.4 M1.3 — directives: **COMPLETE**

`import`, `export` and `use permissions { … }` → `parser/directives.rs`
(107 lines).

Together because they answer one question — what a file needs from, and offers
to, the world outside it. All three are top-level and none produces a value,
which is also their shared reason to change: a decision about the module system
or the permission vocabulary lands here and nothing about expressions does.

`export` is 6 lines because it is a wrapper: it parses the declaration that
follows and returns it inside `Statement::Export`, so every rule about *what* may
be exported lives in the evaluator. `use permissions` parses dotted names
(`OS.exec`) and validates none of them — consistent with `spec/security.md`,
which records that permissions are additive declarations rather than an
isolation boundary.

Snapshot: identical.

### 9.5 M1.4–M1.6 — declarations: **COMPLETE**

The plan in §9 split declarations five ways — enums+native, interfaces,
classes, functions, variables. Reading the code first (the sizes and the call
graph) said three, and the three follow `spec/` rather than a taxonomy:

| Goal | Module | Lines | Contents |
|---|---|---|---|
| **M1.4** | `parser/functions.rs` | 343 | `fn`, `fn*`, parameters, `native fn`, arrow functions, lambda bodies |
| **M1.5** | `parser/variables.rs` | 290 | `let`, `const`, array and dict destructuring patterns |
| **M1.6** | `parser/classes.rs` | 523 | `class`, `interface`, `enum`, and the `public`/`private`/`abstract`/`sealed` prefixes |

Why these three and not five:

- **Functions.** Five spellings, one subject, and they *share*
  `parse_function_parameters` — a change to how a parameter list is written has
  to reach all of them at once. That shared dependency is the boundary, not the
  line count.
- **Variables.** `let` and `const` are one grammar with one flag, and both admit
  the same three left-hand sides. The pattern parsers stay `pub(super)` because
  `for (let x in xs)` uses them too.
- **Classes.** The call graph decided this one. `parse_visibility_statement`
  dispatches to **both** class and interface, and
  `parse_abstract_or_sealed_class` steps over a visibility keyword on its way to
  a class. The modifier prefixes are owned by neither declaration — they route
  between them — so splitting classes from interfaces would have stranded the
  dispatchers between two files or duplicated them. `enum` joins because it is
  the third way a nominal type is introduced by name, and `is_reserved_name`
  guards all three alike.

Snapshot: identical after each of the three.

### 9.6 A correction to the diff-review method

The check used through M1.3 — "every non-blank line removed from `mod.rs`
reappears in the new file" — produced a false alarm here: twelve lines of
`parse_if_expression`, a method that was never extracted, showed as removed.
The method was intact at its original 59 lines.

The cause is git, not the code. When several large adjacent regions are removed,
the diff re-anchors and reports some *surviving* lines as `-` at one position
and `+` at another. The check only collected `-` lines and compared them against
the new files, so re-anchored survivors looked unaccounted for.

**The correct invariant** — used from M1.4 onward — is that every removed line
must appear in the new files **or** among the lines added back to `mod.rs`:

```
comm -23  <(removed lines, sorted -u)  <(new files + re-added lines, sorted -u)
```

Under that check M1.4–M1.6 come out exactly right: 13 lines differ, and all 13
are the signatures that gained `pub(super)`.

### 9.7 M1.9–M1.11 — control flow and literals: **COMPLETE**

| Goal | Module | Lines | Contents |
|---|---|---|---|
| **M1.9** | `parser/loops.rs` | 325 | `while`, `do-while`, C-style `for`, `for-in`, `label:` |
| **M1.10** | `parser/branches.rs` | 400 | `if`, `switch`, `match` + patterns, `try`, `parse_inner_block` |
| **M1.11** | `parser/literals.rs` | 325 | array, dict, entry and brace forms, `dec` lexemes, string interpolation |

Three judgements worth recording:

- **Labels went with loops**, not with the general statement forms.
  `parse_labeled_statement` exists only to introduce a loop — it reads the name
  and then *requires* `while` or `for`. A label in front of anything else is a
  syntax error, so all of it is loop grammar despite looking like a general
  prefix.
- **`if` and `match` went with `switch` and `try`** even though the first two
  produce an `Expression` and the last two a `Statement`. The four are the
  language's ways of choosing a branch and three share `parse_inner_block`. The
  statement/expression split there is the language's shape, not a filing error,
  and `spec/control-flow.md` records it.
- **The brace forms are why `literals.rs` is one module.** A `{` in expression
  position can begin an entry literal, an object patch or a dict body;
  `parse_brace_expression` is the single place that decides, so the three forms
  it can produce belong beside it. The two lexeme parsers (`parse_dec_literal`,
  `parse_interpolated_string`) live here rather than in the lexer because both
  build an `Expression` and the lexer only produces tokens.

Snapshot: identical after each. Verbatim check under the §9.6 invariant: 22
lines differ, all 22 the signatures that gained `pub(super)`.

### 9.8 What the parser looks like now

| File | Lines | Responsibility |
|---|---|---|
| `parser/mod.rs` | 1,527 | `Parser` state, `new`, `parse_program` + `synchronize`, `parse_statement`, the simple statement forms, the assignment forms and the expression core |
| `parser/classes.rs` | 523 | `class`, `interface`, `enum`, modifier prefixes |
| `parser/branches.rs` | 400 | `if`, `switch`, `match`, `try` |
| `parser/functions.rs` | 343 | every way a callable is written |
| `parser/literals.rs` | 325 | arrays, dicts, brace forms, `dec`, interpolation |
| `parser/loops.rs` | 325 | `while`, `do-while`, `for`, `for-in`, labels |
| `parser/variables.rs` | 290 | `let`, `const`, destructuring |
| `parser/diagnostics.rs` | 135 | `ParseError`, `SZ2000`, reporting, rendering |
| `parser/depth.rs` | 111 | `MAX_PARSE_DEPTH`, `SZ2001`, `DepthGuard` |
| `parser/cursor.rs` | 107 | advancing, precedence at the cursor, name classification |
| `parser/directives.rs` | 107 | `import`, `export`, `use permissions` |
| `parser/types.rs` | 93 | type syntax, and nothing about type compatibility |

**`mod.rs` is down 2,409 lines from 3,936 — 61%.** Infrastructure, every
declaration form, all control flow and every literal are out.

Two substantial areas remain, and they are the two the plan always said to leave
until last because they are the HIGH-risk ones:

| Remaining goal | Scope | Lines | Risk |
|---|---|---|---|
| **M1.12** | assignment forms — `parse_expression_statement`, the three `try_build_*`, index and compound assignment, `is_writable_chain` | ~414 | **HIGH** — this is where receiver-writeback shape is decided, the behavior `serez-ui` leans on hardest |
| **M1.13** | expression core — `parse_expression`, `parse_infix_chain`, `Precedence`, `token_precedence`, call arguments, `new`, `sizeof` | ~715 | **HIGH** — fan-in 30, and precedence/associativity live here |
| **M1.14** | precedence audit; no code change (D2) | — | LOW |
| **M1.15** | façade reduction; M1 milestone audit | — | MEDIUM |

What is intended to remain in `mod.rs` at the end: the `Parser` struct, `new`,
`parse_program` + `synchronize`, the `parse_statement` dispatcher,
`is_reserved_name`, and the five one-liner statement forms (`return`, `out`,
`throw`, `unsafe`, `block`). That is the façade the Definition of Done asks for.

---

## 9A. M1 MILESTONE AUDIT

Run at the end of M1.15, per the roadmap's checkpoint procedure.

### Definition of Done

| Criterion | Status |
|---|---|
| Parser core small | **met** — `mod.rs` is 422 lines, of which **329 are code** and 66 are the module documentation. The roadmap's 300–400 signal is met on the measure that matters. |
| Grammar distributed by responsibility | **met** — 13 modules, each named for what changes it |
| Dependencies clear | **met** — see below |
| No accidental semantic validation | **one exception, documented** — see below |
| Behavior preserved | **met** — the snapshot manifest matched after every one of the 13 extractions |
| Suites and ecosystem green | **met** — 331 Rust tests, 490 conformance, 8/8 ecosystem |

### 1. Objective vs implementation

The charter said "eliminate the parser monolith as an architectural
concentration" and "do not merely split parser.rs into files". One 3,936-line
`impl Parser` became 14 files with a 422-line façade. Each module was placed by
reading the call graph, not by cutting the file into equal parts — which is why
the plan changed three times along the way (§9.2 dropped a molecule, §9.5
collapsed five goals into three, §9.7 merged statement families).

### 2. Responsibilities not actually extracted

One. `is_reserved_name` is a semantic check that remains in the parser (§5.20).
It is preserved deliberately: removing it changes when an error is reported, and
extending it to all twenty namespaces is a breaking change. **M4 owns it.**

### 3. Circular or new dependencies

None. Verified mechanically: no file under `src/parser/` references
`crate::evaluator`, `crate::run`, `crate::region`, `crate::scope`, `crate::szx`
or `crate::package_manager`. Every module depends only on `crate::{ast, token,
lexer}` and `super::`.

The horizontal edges form a DAG:

```
types     ← classes, functions, variables, expressions
literals  ← branches, expressions
```

`types` is the shared type-syntax helper; `literals` provides the two lexeme
parsers that build `Expression`s. Nothing else crosses.

### 4. Duplication introduced

None. No method name is defined twice across the 14 files, and every extraction
was verified line-by-line under the §9.6 invariant.

### 5. Semantic drift

None detected. The evidence is the snapshot manifest: 490 files, both the
`Debug` tree and the full diagnostic list hashed, identical before and after
every extraction — and demonstrated in M1.0.2 to catch changes that all 808
other tests miss.

### 6–7. Full gates and ecosystem

fmt PASS · check PASS, no warnings · clippy 0 errors and **exactly 186
warnings**, the M0 number, unmoved across all 13 extractions · `cargo test`
331/0 · `run_tests.ps1` 490/0/0 · `run_ecosystem.ps1` 8/8.

### 8–9. Documentation and MATURITY_AUDIT.md

Every module carries a doc comment stating what it owns and why, including the
hazards that live in it. `MATURITY_AUDIT.md`'s Parser row said "the large parser
now exposes coded positional errors through one CLI/LSP shape, but recovery
rules remain ad hoc"; the "large parser" half is now stale and is updated.

### 10. Commits

Six, one per goal group, each with the contract it preserved and the evidence
for it: `4edad8b`, `da82ffe`, `7775c2c`, `61080fc`, `ca1f24a`, and this one.

### Findings raised by M1 that other milestones now own

| § | Finding | Owner |
|---|---|---|
| 5.17 | Nine parser errors never reach the error list — the LSP shows nothing | **M3** |
| 5.12 | Diagnostics grouped by producer, not ordered by position | **M3** |
| 5.19 | `spec/operators.md` states the wrong bitwise precedence | **M7** |
| 5.20 | `is_reserved_name` is semantic, and covers 7 of 20 namespaces | **M4** |
| 5.18 | The frontend is compiled twice; `sz-lsp` does not use the library | **M10** |
| 5.11, 5.13 | `take_errors` does not take; `has_errors` is false before the flush | M3 |

## MILESTONE STATUS: **COMPLETE**

With one qualification stated plainly rather than buried: the "no semantic
validation" criterion has a single documented exception (§5.20) that M1 was
forbidden to remove, because doing so is a semantic change and the milestone's
governing rule is that a refactor is not one.

---

## 9B. M2 — AST + Spans Stable: discovered goal decomposition

M2.0's audit (§5.4, §5.21) changed what this milestone is. The charter's
pipeline-purity half is **already satisfied** — `ast.rs` has no imports and no
functions, so none of the six prohibitions is violated. What remains is spans,
and the audit says the shape of that work is:

| | |
|---|---|
| Coverage | 5 of 48 node types (10%); 3 of the 5 are load-bearing, 2 dead |
| Representation | no `Span` type — an inlined `line`/`column` pair, five times |
| Source of truth | `Token` has line/column but **no byte offset** |
| Constraint | columns are Unicode scalar values (`lexical-grammar.md`); `Error.span` is `"line:column"` or null (`errors.md`) |

### The question M2.1 has to answer first

**How far should coverage reach, and what should a span store?** These are not
independent, and getting them wrong in either direction is expensive:

- *Every node gets a full byte range.* Maximum fidelity, and what a mature
  frontend eventually wants. It also means touching all 48 types, changing
  `Token`, threading offsets through 29 parser sites, and growing every AST node
  — for a language whose diagnostics currently render one `line:column`.
- *Every node gets line/column only.* Cheaper, matches what is rendered today,
  and cannot ever underline a range in an editor. The LSP already wants ranges;
  `lsp/analysis.rs` reconstructs them by re-lexing.
- *Only the nodes that report errors get a span.* What exists now, arrived at by
  accident rather than decision. Its failure mode is exactly what
  `spec/errors.md` documents: `Error.span` is nullable because most nodes have
  nothing to give.

**This is a product decision with a real cost, and M2.1 is where it gets made
rather than drifted into.** It is the first genuine design choice in the roadmap
so far — M0 and M1 were measurement and relocation, where the right answer was
discoverable from the code. This one is not: all three options are defensible,
and the choice depends on what the LSP is meant to become.

### 9B.1 What M2.1.1 and M2.1.2 measured — and why M2 is not what the charter assumed

Two measurements, both of which say the same thing: **adding spans to AST nodes
would, by itself, improve nothing, because nothing consumes AST spans.**

**1. A runtime error takes its position from the call stack, not from the node
that failed.** `evaluator/mod.rs:record_runtime_error` builds the span as
`self.call_stack.last().map(|frame| …)`. All 1,029 `rt_err_kind` /
`fatal_err_kind` sites go through it. Measured:

```
sz --eval 'try { let a = [1]; out a[99]; } catch (e) { out "span=" + e.span; }'
  ->  span=null                     # the failure is on line 1; nothing is reported

sz --eval 'fn int boom() { let a = [1]; return a[99]; }
           try { out boom(); } catch (e) { out "span=" + e.span; }'
  ->  span=2:15                     # line 2 col 15 is the CALL SITE, not the failure
```

So the position a user sees is *where the innermost function was called from*.
An error outside any function has no position at all — which is the real reason
`spec/errors.md` types `Error.span` as nullable.

This also explains the 10% coverage. It is not an oversight: `CallExpression`,
`DotCallExpression` and `InfixExpression` carry positions because **those are the
nodes that create call frames**, and call frames are the only thing that carries
a position into a runtime error. The current design is coherent, just coarse.

**2. The LSP throws the AST away.** `grep 'program\.'` in `lsp/analysis.rs`
returns nothing. It parses, takes `parser.take_errors()` for diagnostics, and
discards the `Program`. Every symbol comes from `scan_symbols`, a hand-rolled
token scanner tracking `depth: i32` and a `class_stack`; references and rename
come from `occurrences`, which re-lexes and filters identifiers by name; import
paths come from `str::find('"')` on raw lines.

So the one consumer that actually wants ranges does not read the AST at all.

**What follows.** The charter's plan — migrate literals, then identifiers, then
expressions, then statements, then declarations — would add a field to 48 types
and change the lexer, to serve no existing reader. Making those spans matter
means changing *how a runtime error acquires its position*: from
`call_stack.last()` to the failing node. That is 1,029 call sites in the
evaluator, and it is **M3's charter** ("one diagnostic model per conceptual
error", "zero silent failures"), not M2's.

**This is a sequencing question, and it is the user's to answer.** The options
are set out in §9B.2; M2.1.3 is where it gets decided rather than drifted into.

### 9B.2 The decision M2.1.3 must record

| Option | What it costs | What it buys |
|---|---|---|
| **A. Re-scope M2 small, then M3** | ~1 day. Introduce `Span`, replace the 5 inlined pairs, delete or populate the 2 dead fields, stop. | Removes the duplication and gives M3 a type to build on. Changes no diagnostic. |
| **B. M3 before M2** | Reorders the roadmap. M3 fixes §5.17 (nine errors that reach nobody) and decides where a runtime error's position comes from; M2 then adds spans *to the nodes M3 proved it needs*. | Every span added has a consumer the day it lands. Fixes a user-visible defect first. |
| **C. M2 as chartered** | 48 types, the lexer, 29 parser sites, and a larger AST node. | Maximum fidelity, no user-visible improvement until M3 lands anyway. |

**DECIDED 2026-09-02: option C — M2 proceeds as chartered.** The recommendation below was option B; the measurements were presented and the choice was made for the full scope. Recorded so a later session sees the evidence *and* the decision, and does not re-open it.

The consequence to hold on to: spans will land before their consumers exist, so
M2 cannot be judged by "did a diagnostic improve" — none will, until M3 changes
where a runtime error gets its position. M2 is judged on the AST being uniform
and the behaviour being unchanged.

*(Original recommendation, retained as the evidence trail:)* **B, with A folded into it.** The evidence is that spans are not the
bottleneck — the bottleneck is that diagnostics do not know where they happened
and, for nine of them, do not exist at all. Doing M3 first means M2's work is
sized by measured need instead of guessed at, and it fixes something a user can
see. `Span` itself (option A's cheap part) can land first either way, since M3
will want the type.

### 9B.3 M2.2 — the `Span` type: **COMPLETE**

`src/span.rs` (128 lines). It lives in its own module rather than in `ast.rs`
for a reason that will matter in M2.3: `Token` is about to carry a span, and if
the type lived with the AST the **lexer would have to depend on the AST** to
produce a token. A span is neither syntax nor a value — it is a fact about text
— so it depends on nothing and everything may depend on it.

```rust
pub struct Span { line: usize, column: usize, start: usize, end: usize }
```

Both halves, deliberately. `line`/`column` are what gets rendered and are
normative — one-based, columns in Unicode scalar values, and `Error.span` is
`"line:column"` or null. `start`/`end` are byte offsets, the half a *range*
needs. Carrying both means no conversion in the path of any diagnostic the
language already renders correctly; deriving line/column from offsets instead
would put one there. 16 bytes is the cheap direction to be wrong in.

`start == end` is a point, which is what every node gets until M2.3 populates
offsets from tokens.

The five inlined pairs in `ast.rs` are gone, replaced by one `span: Span` field
each, and the `#[allow(dead_code)]` attributes with them.

**Behavior: UNCHANGED — and this one needed proving, because the snapshot
failed.** 445 of 490 files reported a different tree. That is correct and
expected: the `Debug` rendering went from flat `line: 7, column: 1` to nested
`span: Span { line: 7, column: 1, start: 0, end: 0 }`. The values are the same;
the text is not.

The proof that it is representation and not behaviour:

```
$ diff <(old manifest | cut path, diagnostic-count, diagnostic-hash)
       <(new manifest | cut path, diagnostic-count, diagnostic-hash)
IDENTICAL — every diagnostic count and hash unchanged across all 490 files
```

Diagnostics do not contain the AST, so if a position had moved, a parse error's
`line`/`column` would have moved with it. None did. The manifest was then
regenerated, which is what `parser_snapshot.rs` documents as the correct
response to a difference that has been read and understood.

A second-order observation: `cargo test` went from 331 to **337**, not 334, for
three new tests. They run twice — once in the library, once in the `sz-lsp`
binary, because `lsp_main.rs` now declares `mod span;` too. §5.18 made concrete.

### 9B.5 M2.3.1 — the lexer supplies offsets: **COMPLETE**

`Token` now carries a `span`, and the lexer fills in real byte offsets.

**The shape of the change.** `next_token` does not have one exit — identifiers,
numbers and strings return early with the cursor already one past the token,
while every other kind falls through to a final `read_char()` that leaves it in
the same place. So rather than thread an offset through 55 `Token::new` calls
that have no use for it, the body became `next_token_inner` and `next_token` is
a short wrapper that stamps the span. `self.position` on return is
one-past-the-token on every path, and the wrapper is the one place where that
holds regardless of which path ran. The start offset is recorded once per token
onto a new `Lexer::token_start` field, beside the existing `tok_line`/`tok_col`.

`Token` keeps `line`/`column` beside the span for now. Collapsing them is its
own migration, and doing it here would have mixed a mechanical rename into a
change that needed thinking about.

**`tests/lexer_spans.rs` (5 tests) — and it earned its keep immediately.**
Offsets were added ahead of any consumer, which is exactly the condition in
which one can be wrong for months unnoticed. So the tests assert the properties
that make an offset mean anything, over the whole 490-file corpus:

- every span is a valid, char-boundary-respecting slice of its source;
- an identifier's span **slices back to the identifier** — the decisive one,
  since literal and source text are the same string there, so an off-by-one or a
  stale offset fails it loudly;
- spans advance and never overlap;
- `span.line`/`column` still agree with the pair beside them, so the two cannot
  drift while both exist;
- a multi-byte identifier moves columns and offsets by *different* amounts —
  written out by hand because it is invisible in ASCII and the corpus might not
  contain it. `café` is four columns and five bytes.

**The first run found a real bug.** The EOF token came out inverted —
`Span { start: 15, end: 14 }` on a 14-byte source — because `read_char` runs
`position` one past the end at EOF and only `end` was clamped. Both ends clamp
now, which makes EOF the empty point at the end of the source, which is what it
is. Nothing consumed the offsets yet, so nothing was broken; it would have been
the day something did.

### 9B.6 M2.3.2 — nodes get real extents, and the manifest stops being portable

The five expression sites now build their span with `Parser::span_to_here`,
which runs from the node's opening token to wherever the cursor has reached.
Recursive descent captures the opening position *before* parsing a node's parts
and constructs the node *after*, so the node's real extent is available for
free.

**What a node's extent covers, stated precisely because it is narrower than it
sounds.** A call spans from its `(`, an infix expression from its operator, a
dot call from its `.` — not from the start of the callee or the left operand,
because those are `Identifier`s and do not carry spans until M2.6. So
`foo(1, 2)` gives an extent of `(1, 2)`. That is correct and incomplete;
`tests/ast_spans.rs` (6 tests) asserts what is true today rather than what is
wanted later, so that widening it in M2.6 has to be deliberate.

`line`/`column` are untouched by the widening — they stay the opening token's,
because they are what `spec/errors.md` promises a caught `Error.span` reports.

**Then this broke the manifest's portability, and the M1.0 test caught it.**
`the_manifest_does_not_depend_on_what_the_checkout_did_to_line_endings` failed
for 439 files. The cause is not a defect: `\r\n` is two bytes and `\n` is one,
so every offset past the first newline legitimately differs between a CRLF and
an LF checkout. It is arithmetic. But the *manifest* is committed and CI's
Windows runner checks out CRLF, so a hash containing offsets would fail on
exactly one of the three platforms.

Resolved by separating the two things that were conflated:

- **The snapshot is defined over LF-normalised source.** `read_normalised`
  strips `\r\n` before parsing, so the manifest is the same everywhere. It is a
  test artifact; defining it over a canonical form costs nothing.
- **The line-ending test now asserts what is genuinely invariant.** It compares
  diagnostics, and the tree with `start:`/`end:` masked — so it still checks
  every node's shape and that spans are present, while permitting the one
  difference that is arithmetic.

**And the fix had a bug of its own, worth recording because it was nearly
invisible.** The mask reached for `start:` preferentially:
`find("start: ").or_else(|| find("end: "))`. That looks equivalent to "find the
next marker" and is not — once a span's `start` is masked, the following
`start:` lies beyond that span's `end:`, so every `end:` was skipped and the
mask did half its job. It reported **342 files as differing when the true number
is zero**. Caught because 342 was implausible: the offsets that differ under
CRLF are *shifts*, and a shift affects `start` and `end` alike, so a mask that
worked could not leave a third of the corpus differing.

### 9B.7 M2.4 — the declaration nodes: **COMPLETE**

Five declarations gained a span and had it populated: `LetStatement`,
`FunctionDeclaration`, `ClassDeclaration`, `InterfaceDeclaration`,
`NativeFnDeclaration`. AST coverage goes from 5 of 48 node types to **10**.

**These extents are complete, unlike the expression ones.** A declaration starts
at its own first token — there is no callee or left operand in front of it — so
`let x = 1 + 2;` spans the whole statement, and a function spans its whole body.
Five new tests in `tests/ast_spans.rs` pin that, including two decisions worth
being explicit about:

- **A class spans from `class`, not from `public`.** The modifier prefix is
  consumed by `parse_visibility_statement` before `parse_class_declaration`
  runs, so the extent begins at the keyword that names the construct — the same
  rule every other declaration follows. `a_class_declaration_spans_from_class_not_from_its_modifier`
  asserts the column is 8 rather than 1, so the choice cannot drift unnoticed.
- **A `for` initializer needed its span captured early.** By the time the
  `LetStatement` is constructed the cursor has moved past the `;` onto the
  condition, so `span_to_here` there would have swallowed `i < 3`. It is
  captured right after the initializer expression parses instead, and
  `a_for_loop_initializer_stops_at_the_semicolon` is what proves the early
  capture was necessary rather than defensive.

The synthetic fixtures in `compiler/hir_lower.rs` take `Span::unknown()` — they
build ASTs that never came from source, which is the case §5.23 names.

Snapshot: **399 of 490** files changed, every diagnostic hash identical.

### 9B.8 M2.5 — the statement nodes: **COMPLETE**

Twelve statement nodes gained a span: `AssignStatement`, `BlockStatement`,
`ReturnStatement`, `WhileStatement`, `IndexAssignStatement`, `ForStatement`,
`ForEachStatement`, `OutStatement`, `FieldAssignStatement`,
`NestedFieldAssignStatement`, `SwitchStatement`, `TryStatement`.

**Coverage: 5 of 48 node types when M2 began, 10 after M2.4, now 22 — 46%.**

43 construction sites, and sorting them was the work rather than the edit.
§5.23's three kinds all appear here, and two of them needed handling that a
uniform rewrite would have got wrong:

- **The `try_build_*` helpers receive an already-parsed expression**, so they
  cannot see where the statement began. The opening span is now an explicit
  parameter rather than something recovered from the expression, because the
  statement starts before the expression does — `a.b[i] = x` begins at `a`, and
  the node that knows that is gone by the time the assignment is built.
- **Three blocks are synthetic and one is empty by construction**: the `else if`
  wrapper, a single-expression match arm, the shorthand-lambda `return`, and an
  abstract member's absent body. None has braces in the source. They take
  `Span::unknown()`.

Two new tests assert the classification rather than leaving it as prose:
`a_synthetic_node_gets_a_position_but_no_extent` (the `i++` assignment reports
line 2 column 1 and claims no text) and `an_else_if_wrapper_has_no_position_at_all`
(the wrapper has no position, because there is nothing to point at).

Snapshot: **416 of 490** files changed, every diagnostic hash identical.

### 9B.9 M2.6a — the struct-shaped expression nodes: **COMPLETE**

Nine gained a span: `ArrayLiteral`, `IndexExpression`, `TernaryExpression`,
`DictLiteral`, `NewExpression`, `LambdaExpression`, `IfExpression`,
`MatchExpression`, `FunctionLiteral`.

**Coverage: 22 of 48 → 31. Sixty-five percent.**

Snapshot: 351 of 490 files changed, every diagnostic hash identical.

### 9B.10 M2.6b — the scalar variants, and why they are their own molecule

Seventeen `Expression` variants remain, and they are a different shape of
problem. They are not structs:

```rust
Identifier(String),  Integer(i64),  Decimal(f64),  String(String),
Boolean(bool),  Null,  Prefix(String, Box<Expression>),  Spread(Box<Expression>),
AddressOf(Box<Expression>),  Deref(Box<Expression>),  …
```

Giving one a span means changing the variant's shape, and every `match` arm in
the crate that names it has to change with it. Measured:

| Variant | Match sites |
|---|---|
| `Expression::Identifier` | 38 |
| `Expression::Integer` | 34 |
| `Expression::Boolean` | 13 |
| `Expression::String` | 10 |
| `Expression::Null` | 8 |
| `Expression::Prefix` | 4 |

Roughly 110 sites for the six most common alone, spread across the evaluator's
hot paths, the type checker, the compiler and the LSP. That is not the
mechanical edit the last four molecules were, and it needs its own decision
about representation before any of it is written:

- **a tuple field** — `Identifier(String, Span)` — smallest diff per site, but
  every existing `Expression::Identifier(name)` pattern has to gain a `, _`, and
  a positional `Span` in a tuple is easy to misread at a construction site;
- **a struct variant** — `Identifier { name: String, span: Span }` — reads
  better and makes construction explicit, but rewrites every pattern;
- **a side table** keyed by node id — leaves every existing match arm alone,
  and introduces node identity to an AST that has none, which is a much larger
  idea than M2 is chartered for.

**DECIDED — struct variant.** A positional `Span` beside a `String` is correct
only by remembering which is which, which is the shape of defect
`MATURITY_AUDIT.md` records twice already (the GUI handle arithmetic, the
unchecked SVG index). `{ name, .. }` in a pattern also survives the next field
being added, where `(name, _)` would not. The side table was rejected outright:
it introduces node identity to an AST that has none, which is a larger idea than
M2 is chartered for.

Applied to `Identifier` first, alone, as M2.6b — it is the highest-value span in
the language and doing one variant proves the shape before sixteen more follow.

### 9B.11 M2.6b — `Identifier` carries its span: **COMPLETE**

```rust
Identifier { name: String, span: Span }
```

38 sites, and the compiler found every one. Three groups:

- **Patterns** became `{ name, .. }`, so the bodies below them did not change.
- **Real constructions** take the token's own span — `parse_expression`'s
  identifier arm, a dict key, the type name on the right of `is`, and the
  binding in a grouped expression.
- **Desugared identifiers** take the position of the construct they were
  rewritten from: the `i` rebuilt inside `i = i + 1` gets a point at the
  original `i`, and the `Enum` rebuilt for a match pattern gets the pattern's
  own span.

`an_identifier_spans_exactly_its_name` is the sharpest check in the whole span
suite: an identifier is the one node whose extent involves no judgement — it is
exactly the name — so an off-by-one has nowhere to hide. A second test uses
`café` to keep bytes and columns visibly distinct.

Snapshot: 448 of 490 files changed, every diagnostic hash identical.

**Coverage: 32 of 48.** Sixteen scalar variants remain (`Integer`, `Decimal`,
`String`, `Boolean`, `Null`, `Prefix`, `Spread`, `AddressOf`, `Deref`,
`EntryLiteral`, `InterpolatedString`, `ObjectPatch`, `SizeOf`, `UnsafeBlock`,
`DictLiteral`'s entries, `Match`) — M2.6d, now that the representation question
is settled and the shape is proven.

### 9B.12 M2.6c — widening the expression extents: **RECLASSIFIED OUT OF M2**

Planned as the next molecule, then examined before writing it, and it does not
belong in this milestone.

**Why it looked like M2's work.** Until M2.6b a call spanned from its `(`
because the callee had no span to reach back to. That reason is gone, so
widening looks like finishing the job.

**Why it is not.** Measured against the 10.0.0 binary:

```
fn int f(int n) { return n; }
out f(1, 2);

❌ TYPE ERROR [SZ3000] [line 2:6]: 'f' expects 1 argument(s) but got 2.
❌ ERROR  [SZ4002]: Function expected 1 argument(s), got 2
    called from 'f' [line 2:6]
```

Column 6 is the `(`. Widening moves both of those to column 5, the `f` — in the
type checker, in the runtime error, and in the call frame that appears in every
stack trace. That is a change to **where a diagnostic points**, which is exactly
what the milestone's governing rule forbids a representation change from doing.

**And half-widening is worse than either.** Moving `start` to the callee while
leaving `line`/`column` on the `(` would make the span internally incoherent:
`start` would no longer be the byte offset of the position `line`/`column`
names. A span that contradicts itself is a worse artifact than a narrow one.

**Owner: M3.** Deciding what a diagnostic points at is that milestone's subject
— it already has to decide where a runtime error's position comes from at all
(§9B.1), and this is the same question one level down. M7 owns it if it is
treated as a frozen contract instead.

`tests/ast_spans.rs` keeps asserting the narrow form, which is now a deliberate
pin rather than a placeholder.

**It is also what unblocks widening the M2.3.2 extents.** A call spans from its
`(` and not from its callee precisely because the callee is usually an
`Identifier`, which carries nothing. Once the scalar variants have spans, the
extents in `tests/ast_spans.rs` can widen — and those tests were written to
assert the narrow form so that widening has to be deliberate.

### 9B.13 M2.6d — the literal variants: **COMPLETE**

Six converted to struct variants under the M2.6b decision: `Integer`,
`Decimal`, `Dec`, `String`, `Boolean` and `Null`.

`Null` is the interesting one. It was a *unit* variant — no payload at all — and
becomes `Null { span: Span }`: the one expression with no value still has to say
where it was written. That is what "uniform spans" means when taken seriously
rather than only where it is convenient.

77 sites, and the same three kinds as before: real constructions take the
token's span, desugared ones take a point at the construct they came from (the
`1` inside `i = i + 1`), and two are genuinely unknown — the implicit `null` of
a bare `return;`, which the programmer never wrote, and the collapsed
single-literal case of `parse_interpolated_string`, a free function with no
cursor to ask.

`a_literal_spans_exactly_its_own_text` checks each of the five spellable kinds
separately, because each is built by a different arm of the prefix dispatcher
and one arm reading the wrong token is exactly the mistake nothing else would
catch. It also pins a decision worth stating: **a string literal's span includes
its quotes** while its `value` does not. That is right for a *span* — the quotes
are source the literal occupies — and it is why the test asserts against the
source text rather than against the value.

Snapshot: 459 of 490 files changed, every diagnostic hash identical.

**Coverage: 31 of 40 structs, and 7 of 28 `Expression` variants.** Eight
wrapper variants remain (`Prefix`, `Spread`, `AddressOf`, `Deref`,
`EntryLiteral`, `InterpolatedString`, `ObjectPatch`, `SizeOf`); `Match` and
`UnsafeBlock` already carry one through their payloads.

### 9B.14 M2.6e — the wrapper variants, and expressions are finished

Eight converted: `Prefix`, `Spread`, `AddressOf`, `Deref`, `EntryLiteral`,
`InterpolatedString`, `ObjectPatch`, `SizeOf`.

**Every one of the 28 `Expression` variants now carries a span** — 15 directly,
and 13 through a payload struct that has one. Verified rather than assumed: each
tuple variant's payload was checked for the field.

| | |
|---|---|
| `Expression` variants with a position | **28 of 28** |
| Structs in `ast.rs` with a span | 31 of 40 |

Two spans here are honestly unknown, and both for the same reason:
`parse_interpolated_string` is a free function with no cursor to ask. The
interpolated string itself and its collapsed single-literal case take
`Span::unknown()`. Its *parts* carry their own positions, because those are
built by a nested parser that does have one.

Snapshot: 191 of 490 files changed — much lower than the 459 of M2.6d, because
these variants are rarer in real source. Every diagnostic hash identical.

**On method.** The redundant-field-name warnings this produced were fixed with
`cargo fix --allow-dirty` rather than by hand-written regex. Two of my own
`perl -0pi` invocations had already gone wrong this session — one inserting a
field inside the wrong struct, one mangled by shell escaping — and a tool that
understands the syntax is the right instrument when one exists.

### 9B.15 What is left in M2

| Remaining | Status |
|---|---|
| The 9 structs without a span | `Program`, `SwitchCase`, `MatchArm`, `Parameter`, `InterfaceField`, `ClassMethod`, `ClassConstructor`, `LetDestructureArray`, `LetDestructureDict` — mostly *parts* of a node rather than nodes, which is why they were not swept in with their parents |
| M2 milestone audit | Not started |

The nine are a real question rather than a leftover: a `Parameter` or a
`SwitchCase` is a component, and whether a component needs its own position
depends on whether anything would ever point at one. `Parameter` plainly would —
a type error on an argument wants to underline the parameter it failed against.
`Program` plainly would not. That triage is the next molecule.

### 9B.16 M2.7 — the component structs, triaged: **COMPLETE**

Eight of the nine gained a span. **`Program` deliberately did not**: it is the
root, its extent is the whole file, and that is `0..source.len()` — derivable,
and nothing would point at it that is not simply "this file".

The other eight each have something that would point at them:

| Struct | What would point at it |
|---|---|
| `Parameter` | a type error on an argument, underlining the parameter it failed against |
| `SwitchCase`, `MatchArm` | an unreachable or duplicate case; a non-exhaustive `match` |
| `InterfaceField` | the extra/missing-field enforcement `MATURITY_AUDIT.md` still lists as open |
| `ClassMethod`, `ClassConstructor` | a duplicate member, a bad override, a wrong constructor arity |
| `LetDestructureArray`, `LetDestructureDict` | they are statements, not components — see §5.25 |

**The tests caught two capture-point errors, which is the whole reason they were
written before the gates ran.**

- `Parameter` spanned `a` rather than `int a`. The capture sat *after* the type
  annotation was consumed; the parameter opens at whichever token starts it —
  its type, its `...`, or its name — and that is now taken at the top of the
  loop before any of the three is read.
- `MatchArm` spanned `1 => "one",` — including the separator. A comma divides
  arms; it is not part of one. The span is now taken before it is consumed.

Both were mistakes in the code rather than in the expectation, and both would
have been invisible to every other gate.

**Coverage: 39 of 40 structs, and all 28 `Expression` variants.**

Snapshot: 184 of 490 files changed, every diagnostic hash identical.

### 9B.4 M2.3 onward — molecules (planned)

| Molecule | Action | Verification |
|---|---|---|
| **M2.3.1** | Give `Token` a `span`, populated by the lexer | **done** — see §9B.5. Snapshot unchanged, as predicted: tokens are not in the AST |
| **M2.3.2** | Populate `start`/`end` at the expression sites | **done** — §9B.6. Also made the manifest LF-normalised, since byte offsets are not portable across checkouts |
| **M2.4** | Migrate the declaration nodes | **done** — §9B.7. Coverage 5 of 48 -> 10 |
| **M2.5** | Migrate the statement nodes | **done** — §9B.8. Coverage 10 -> 22 of 48 |
| **M2.6a** | The struct-shaped expression nodes | **done** — §9B.9. Coverage 22 -> 31 of 48 |
| **M2.6b** | `Identifier` as a struct variant | **done** — §9B.11. Coverage 31 -> 32 |
| **M2.6c** | Widen the expression extents | **reclassified out of M2** — §9B.12. It moves where a diagnostic points; M3 owns it |
| **M2.6d** | The literal variants (`Integer`, `Decimal`, `Dec`, `String`, `Boolean`, `Null`) | **done** — §9B.13 |
| **M2.6e** | The 8 wrapper variants | **done** — §9B.14. All 28 `Expression` variants now carry a span |
| **M2.7** | Triage the 9 component structs | **done** — §9B.16. 8 got spans; `Program` deliberately did not |
| **M2.8** | Collapse `Token`'s duplicated position; M2 milestone audit | **done** — §9C |
| **M2.7** | Resolve the two dead fields — `ClassField` and `EnumDeclaration` now have spans nothing reads; either give them a consumer or state why they stay | a decision recorded, not a silent deletion |
| **M2.8** | M2 milestone audit | full gates + ecosystem |

Each of M2.4–M2.6 will fail the snapshot the same way M2.2 did, for the same
reason, and each needs the same proof before regenerating: **the diagnostic
columns must be identical.** That check is the milestone's safety rail, and it
is cheap — two `cut`s and a `diff`.

### M2.1 — molecules (planned)

| Molecule | Action | Verification |
|---|---|---|
| **M2.1.1** | Measure the cost surface of each option | **done** — 48 AST types, 56 `Token` construction sites, 29 parser position sites, 17 of them reading a token position directly |
| **M2.1.2** | Measure what consumes AST spans today | **done** — §9B.1. Nothing does: runtime errors read the call stack, the LSP discards the `Program` |
| **M2.1.3** | Record the decision §9B.2 sets out, then write the contract into `spec/` | **awaiting the decision** — the measurements are in; the choice is the user's |

Only after M2.1.3 do M2.2+ (introduce the type, migrate node families, remove
the two dead fields) become writable. Decomposing them now would presume the
answer.

---

## 9C. M2 MILESTONE AUDIT

Run at the end of M2.8.

### Definition of Done

| Criterion | Status |
|---|---|
| Spans uniform | **met** — 39 of 40 structs and all 28 `Expression` variants. `Program` excluded deliberately (§9B.16) |
| AST source-oriented | **met** — `ast.rs` has one `use` (`crate::span::Span`, a leaf) and zero functions |
| Responsibilities clear | **met** — the AST consumes no tokens, resolves no symbols, checks no types, executes nothing, reaches no runtime, renders no diagnostics |
| Legacy positional state removed once unconsumed | **met in M2.8** — see below |
| Behaviour preserved | **met** — ten snapshot regenerations, each with diagnostic columns identical across all 490 files |
| Gates green | **met** |

### 1. Objective vs implementation

The charter asked for an AST that is a clean representation of the source, with
uniform spans, and for legacy positional state to go once it had no consumers.

The first half was **already true** when M2 began — M2.0 measured it and the
milestone was re-scoped accordingly (§5.21). The work was spans, and it is done:
every node that can be pointed at can now say where it is.

### 2. Legacy positional state — the last DoD item, closed in M2.8

`Token` carried `line`/`column` *and* `span`, added that way in M2.3.1 because
collapsing them was a separate migration from producing them. M2.8 did the
collapse: 26 reader sites across the parser, the LSP and the lexer now go
through the span, and the pair is gone.

One test was **deleted rather than updated**:
`the_span_agrees_with_the_line_and_column_it_was_built_from` compared the two
representations. With one representation left, the invariant it checked cannot
be violated. A test retired because the type system took over its job is the
only kind that should go without a replacement, and the file says so where it
stood.

### 3. Circular or new dependencies

None. `span` is a leaf — zero `use` statements — which is why it could be given
to both the lexer and the AST without either depending on the other. Had it
lived in `ast.rs`, the lexer would have had to depend on the AST to produce a
token.

### 4. Duplication

None. No node carries two positions; the one case that did (`Token`) was the
subject of M2.8.

### 5. Semantic drift

None, and this is the milestone where it was most at risk — every one of the
fourteen molecules changed the `Debug` rendering of the tree, so the snapshot
failed every time and could not itself distinguish representation from
behaviour. The discriminator was the manifest's diagnostic columns, diffed
before every regeneration:

```
diff <(old manifest: path, diagnostic-count, diagnostic-hash)
     <(new manifest: path, diagnostic-count, diagnostic-hash)
```

Ten regenerations, ten times identical across all 490 files.

**One planned molecule was cancelled for this reason.** M2.6c would have widened
a call's extent to its callee, which moves `f(1, 2)`'s reported column from 6 to
5 in the type checker, the runtime error and every stack frame. That is a
diagnostic change, not a representation one; reclassified to M3 (§9B.12).

### 6–7. Full gates and ecosystem

fmt PASS · check PASS, no warnings · clippy 0 errors and **exactly 186
warnings**, the M0 number, unmoved across all 25 commits · `cargo test` 364/0 ·
`run_tests.ps1` 490/0/0 · `run_ecosystem.ps1` 8/8.

### 8–9. Documentation and MATURITY_AUDIT.md

`src/span.rs` states what a span stores and why both halves. `tests/ast_spans.rs`
documents what each *kind* of extent covers and why they differ.
`MATURITY_AUDIT.md`'s AST row is updated.

### 10. Findings raised by M2

| § | Finding | Owner |
|---|---|---|
| 5.24 | A match pattern that fails to evaluate is silently a non-match | **M7** / M3 |
| 9B.12 | Widening expression extents moves diagnostics | **M3** |
| 5.25 | M2.4 and M2.5 both missed two statement forms; enumeration by name is a guess | method, recorded |
| 5.4 | Corrected: 3 of 5 original positional fields were live, not 1 | closed |
| 5.21 | The AST already satisfied every "must not" | closed |

## MILESTONE STATUS: **COMPLETE**

---

## 9D. M3 — Diagnostics Unified

### 9D.0 The audit: five types, six renderers, one spec sentence

**The five diagnostic types**, and what each carries:

| Type | Where | Fields |
|---|---|---|
| `LexError` | `lexer.rs` | code, line, column, message |
| `ParseError` | `parser/diagnostics.rs` | code, line, column, message |
| `TypeError` | `type_checker.rs` | code, line, column, message |
| `RuntimeError` | `evaluator/mod.rs` | code, **kind**, message, **span**, **stack**, **notes** |
| `CompilerDiagnostic` | `compiler/hir_lower.rs` | code, kind, message |

Three are the same four fields written out three times. `RuntimeError` is the
only one with a kind, a stack or notes. `CompilerDiagnostic` is the only one
that is *returned* rather than printed — it has a `Display` impl and reaches the
caller as `Err`, which is the shape the other four should have.

**The rendering is scattered across producers.** `LexError` has no renderer of
its own: the *parser* renders it, through `print_frontend_error("LEXER", …)` —
so the lexer already produces data only, which is the state M3 wants for all of
them. The type checker prints inside `type_error_code`. The evaluator has
sixteen `eprintln!` calls.

**What M3 must not move**, all normative: the `SZ1xxx`–`SZ7xxx` codes
(`spec/errors.md`), the exit codes (`spec/cli.md`), the rendered shape
`❌ PARSER ERROR [SZ2000] [file line:col]: …`, the catchable/fatal split, the
`Error.span` string-or-null contract, and stack frames.

### 9D.1 The net: `tests/diagnostic_render.rs`

M3 changes the code that produces what a user reads, and the conformance
suite's **149 error fixtures assert only that some `❌` appeared and the exit
code was non-zero**. A reworded message, a moved column, a dropped note, a
diagnostic that stops printing entirely — all pass today.

So the harness runs the real `sz` binary against every `err_*.sz` and `sec_*.sz`
and hashes the **complete stderr plus the exit code** into a committed manifest.
149 fixtures. It is the M3 analogue of `parser_snapshot.rs`: between them the
data and the rendering are both pinned.

**Proved it catches things, as M1.0.2 did.** A single trailing space added to
one runtime-error format string: **129 of 149 fixtures** flagged.

### 9D.2 What the proof itself found — *architectural debt*, medium

The first perturbation changed **nothing**, because it hit the wrong renderer.
There are two:

| Site | When |
|---|---|
| `record_runtime_error` (`mod.rs:605`) | as the error is raised |
| `report_program_outcome` (`mod.rs:1726`) | at the pipeline boundary |

Only the second fires. `eval_program_outcome` raises
`diagnostic_capture_depth` for the whole evaluation, which suppresses the first
— deliberately, and `spec/errors.md` says so: *"Human diagnostics are rendered
once at the pipeline boundary."*

**The first renderer is unreachable from anywhere in the crate.** Every caller —
`run.rs`, `repl.rs`, `namespaces_task.rs`, `stmt.rs` and the tests — goes
through `eval_program_outcome` or the legacy `eval_program`, and both raise the
depth. It can only fire for an external embedder driving `Evaluator` directly.

Recorded rather than removed: deleting it is a behaviour change for such an
embedder, and `Evaluator` is `pub`. M3's own molecule for the data/rendering
split decides it, explicitly.

### 9D.3 M3 — molecules

| Molecule | Action | Kind |
|---|---|---|
| **M3.0** | Audit the five types, their renderers and consumers | done — §9D.0 |
| **M3.1** | `tests/diagnostic_render.rs`: pin stderr + exit code for 149 fixtures | done — §9D.1 |
| **M3.2** | Introduce the common `Diagnostic` model in its own leaf module | refactor |
| **M3.3** | Migrate `LexError` and `ParseError` onto it | refactor |
| **M3.4** | Migrate `TypeError` | done — §9D.3 |
| **M3.5** | Migrate `RuntimeError`, preserving kind/stack/notes and the catchability bit | done — §9D.4 |
| **M3.6** | One renderer; decide the unreachable second (§9D.2) | done — §9D.5, decision **D5** |
| **M3.7** | **§5.17** — nine parser errors that reach nobody | done — §9D.6, **behaviour change** |
| **M3.8** | **§5.12** — diagnostic ordering | done — §9D.7, decision **D6: no change**, one proposal left for Sergio |
| **M3.9** | M3 milestone audit | — |

M3.7 and M3.8 are marked because they are *not* refactors. Every other molecule
must leave `diagnostic_render.manifest` untouched; those two will change it, and
each says so in its own commit rather than arriving mixed into a migration.

### §9D.3 — M3.4: `TypeError` (refactor)

`pub type TypeError = Diagnostic;`. `type_error_code` keeps its `eprintln!`
byte-for-byte — including the `if line > 0` conditional that omits the position
entirely rather than printing `line 0:0` — and now pushes
`Diagnostic::frontend(code, Phase::Type, Span::point(line, column), message)`.

`Phase::Type` maps to `Severity::Advisory`, which is the contract, not a default:
`spec/types.md` states the checker is deliberately partial and that `sz file.sz`
reports its findings **and still runs**. The exit code did not move.

The checker's "unknown position" spelling and the span model's already agreed:
the checker used `0`, and `Span::point(0, 0)` *is* `Span::unknown()`.

One consumer moved: `src/lsp/analysis.rs:156-157`, `e.line`/`e.column` →
`e.span.line`/`e.span.column`.

Evidence: both manifests passed **without regeneration**.

### §9D.4 — M3.5: `RuntimeError` (refactor)

`pub type RuntimeError = Diagnostic;` and
`pub type RuntimeErrorFrame = crate::diagnostic::Frame;`. `RuntimeErrorSpan` is
gone entirely — `grep -rn RuntimeErrorSpan src/ tests/` returns nothing.

There is exactly one producer, `record_runtime_error`, so the migration had one
construction site to get right. `kind: String` became `kind: Some(String)`;
`Option<RuntimeErrorSpan>` became a `Span`.

**The one real hazard, and why it is not a behaviour change.** The caught
`Error.span` is typed by `spec/errors.md` as `"line:column"` **or null**. The
obvious replacement for the old `Option::is_none()` test is
`Span::is_known()` — and it is *not the same question*. `is_known()` asks whether
the line is non-zero; the old test asked whether there was a frame at all. A
frame genuinely sitting at line 0 would have rendered `"0:0"` before and would
start rendering `null` after. Whether such a frame is reachable is beside the
point: a refactor may not depend on the answer.

So the render site in `src/evaluator/control.rs` tests `error.stack.first()`
instead. `span` and `stack` are built from the same `self.call_stack` inside
`record_runtime_error` — the stack is empty exactly when there was no frame — so
this is the same predicate as the `Option` it replaced, **for every input**, with
no dependence on line values. The reasoning is written at the site.

The latent question — *should* a frame at line 0 report `"0:0"`? — is a behaviour
decision, and belongs with M3.7/M3.8, not here.

**Evidence.** Both manifests passed without regeneration, but neither covers the
caught `Error` object: it is a *language value* on stdout, not rendered stderr.
So it was measured directly. A probe exercising both branches —

```
A code=SZ4003 kind=IndexOutOfBounds span=2:19
A frame=inner at 2:19
A frame=outer at 3:12
B code=SZ4003 kind=IndexOutOfBounds
B span-is-null=true
B stacklen=0
```

— was run against a binary built from `c4657e3` (via `git stash`) and against the
migrated binary. `diff` reported no difference. `A` is the nested case (span
present, two frames); `B` is the top-level case (empty call stack → **null**,
zero frames). Both `spec/errors.md` branches are covered.

The catchability bit stayed on `PendingRuntimeError`, untouched, exactly as
`src/diagnostic.rs` says it must.

### §5.26 — the clippy gate as recorded is cache-sensitive

Recorded as "exactly 186 warnings", counted with `cargo clippy --all-targets`
piped to `grep -c "^warning: "`. During M3.5 that count read 187, which looked
like a regression I had introduced.

It was not. The same command run against `c4657e3` — the pre-migration commit,
restored with `git stash` — **also** reads 187 once `src/lib.rs`, `src/main.rs`
and `src/lsp_main.rs` are touched to force a full rebuild. The figure depends on
how much of the crate clippy actually recompiles, because the per-target
"generated N warnings (M duplicates)" summary lines are counted along with the
warnings themselves.

**The gate is now the unique per-site list, not the count.** Snapshot it with:

```sh
touch src/lib.rs src/main.rs src/lsp_main.rs
cargo clippy --all-targets --message-format short 2>&1   | grep -E '^[^ ].*: warning: '   | sed -E 's/:[0-9]+:[0-9]+: warning: / :: /' | sed 's/: help:.*//' | sort
```

That yields **181 lines** and is stable across cache states. M3.4+M3.5 changed it
by zero lines (`comm -13` and `comm -23` both empty against the `c4657e3`
baseline). Prefer this to any count: it says *which* warning appeared, and a
count that moves by one tells you nothing about which one.

### §9D.5 — M3.6: one renderer (refactor + decision D5)

`src/render.rs`, a leaf module over `diagnostic` and `span`. It takes a
`Diagnostic` and a `Context` and returns a `String`; it does not print. Returning
a string rather than writing to stderr is the point — the five unit tests in it
pin the exact byte layout of every phase without running a binary or capturing a
stream.

Four producers now call it, where before each had its own format:

| Producer | Was | Now |
|---|---|---|
| `parser/diagnostics.rs::print_frontend_error` | its own `match` on `source_name`, two `eprintln!` for the caret | builds a `Context` and calls `render` |
| the same, for lexical errors | passed six loose arguments | passes the `Diagnostic` |
| `type_checker.rs::type_error_code` | its own `eprintln!` with an inline `if line > 0` | `render` with a default `Context` |
| `evaluator/mod.rs::report_program_outcome` | its own `eprintln!` | `render` with a default `Context` |
| `evaluator/mod.rs::record_runtime_error` | its own `eprintln!` — **the unreachable one** | `render`; see D5 |

**Three format differences were preserved rather than tidied**, and only one of
them needed a rule:

  * Runtime failures print no position bracket even though they have a span,
    because the frames underneath carry it. That is the one genuine phase rule,
    `Phase::shows_position()`.
  * The file name appears only when the caller knows it. The parser is told;
    the type checker never is, so its diagnostics still say `[line L:C]` even
    when `sz` was given a path. **This is an inconsistency** — recorded, not
    fixed, because fixing it changes what a user reads.
  * The snippet and caret appear only when the caller supplies the source, which
    again only the parser does.

The last two need no phase rule at all: they follow from what the caller puts in
the `Context`. Only the first is a decision, so only the first is written down.

### §5.27 — the rule that made one renderer possible, and the gate under it

The type checker has always omitted the position bracket when `line == 0`,
printing a bare code rather than `[line 0:0]`. Making that uniform — *omit the
bracket whenever the span is unknown* — is what let four formats collapse into
one.

It is byte-identical for the lexer and the parser **only if their diagnostics
never carry an unknown span**. One that did would have printed `[file 0:0]`
before and would print nothing after: a silent, user-visible change hiding inside
a refactor.

So it is not assumed. The new
`tests/parser_snapshot.rs::every_frontend_diagnostic_carries_a_real_position`
parses all 490 corpus files and asserts every collected diagnostic has
`span.line > 0`. It passes. The assumption is now a gate, and if it ever fails
the message says what was lost.

### D5 — the unreachable runtime renderer stays, routed through `render`

**The finding (§9D.2), now proved stronger than it was stated.** The audit said
`record_runtime_error`'s `eprintln!` was unreachable from inside the crate and
could still fire for an external embedder driving `Evaluator` directly. That
second half is **wrong**. `Evaluator` exposes exactly two public methods that
evaluate anything — `eval_program_outcome` and `eval_program`, and the latter
delegates to the former — and `eval_program_outcome` raises
`diagnostic_capture_depth` for the whole evaluation. `record_runtime_error` is
private. So the guard is false for **every** caller, inside the crate or outside
it. There is no embedder for whom deleting it would change anything.

**The decision: keep it, and route it through the single renderer.**

  * M3.6's goal is one *renderer*, not one *call site*. Both sites calling
    `render::render` achieves it completely; the formats can no longer diverge.
  * Deleting it is the only irreversible option and buys nothing structural.
  * It is a real fallback. If a future path ever evaluates without raising the
    depth, the alternative to this branch is a non-zero exit with **no output at
    all** — the least actionable thing the runtime can do, and exactly the
    failure `unstructured_outcome_diagnostic` already exists to prevent.
  * It was a trap, and routing it fixes the trap. This is the site that
    swallowed the first M3.1 perturbation without a trace. It can no longer
    hold a second format, and the comment at the site now says it is
    unreachable and why.

**Reclassified out of M3: `CompilerDiagnostic`.** M3.0 counted five diagnostic
types; four are now one. The fifth is not, and forcing it in would be wrong:

  * it has **no consumer outside `src/compiler/`**, so it has no user-visible
    surface to preserve or change;
  * its rendered form comes from a `Display` impl with no marker, no phase word
    and no position — nothing in common with the other four. Migrating it either
    changes that text or makes the renderer carry a second format for a type
    nobody reads;
  * it carries no span at all, so it cannot exercise anything the model adds.

It belongs to whichever milestone owns the experimental AOT compiler. **A
correction:** the M3.4/M3.5 commit message (`361c2cb`) says the compiler's type
is the one "which M3.6 takes". That was written ahead of the evidence and is
withdrawn here; M3.6 does not take it.

### §9D.6 — M3.7: the nine errors that reached nobody (**behaviour change**)

The one molecule in M3 that is not a refactor, and it has its own commit.

**What was wrong.** Nine sites in the grammar reported by hand:
`had_error.set(true)` plus a bare `eprintln!`. Nothing was pushed into `errors`,
so `take_errors()` came back **empty** for a program the parser had just
rejected. Everything downstream was blind: the LSP published no diagnostic and
underlined nothing; `run.rs` built `RunFailure::Frontend(vec![])`, a failure with
no reason attached. The printed line carried no `SZ` code, no file, no line and
no column, and said `❌ PARSE ERROR:` — a prefix that appears nowhere in
`spec/errors.md`.

**The spec did not change, because the spec was already right.**
`spec/errors.md` says syntax errors are `SZ2000`, rendered as
`❌ PARSER ERROR [SZ2000] [file line:col]: …`. These nine sites were violating
it. M3.7 brings the code into compliance rather than the document into line with
the code.

**A tenth site.** `literals.rs::parse_interpolated_string` printed
`❌ PARSER ERROR: Unclosed '{' in string interpolation` from a free function with
no cursor and no error list — the same defect in miniature. It now returns an
`InterpolationFailure` and the caller reports it through `parser_error`, so it
gets a code and a position like the rest.

**What changed for a user**, exactly: nine constructs that were rejected with an
uncoded, unpositioned line are now rejected with a coded, positioned one plus a
caret. **The exit code did not move** — `has_errors()` was already true, so all
nine already exited 1.

**The evidence, and what it revealed.** Both manifests passed *unchanged* after
the fix. That is not the fix being invisible; it is proof that **none of the 490
corpus files and none of the 149 error fixtures exercised any of these nine
paths**. Zero coverage is why the defect survived to be found by reading the
code. So nine fixtures were added, one per site, and the manifests regenerated:

| Manifest | Before | After | Rows changed |
|---|---|---|---|
| `diagnostic_render.manifest` | 149 fixtures | 158 | **0 modified, 9 added** |
| `parser_ast.manifest` | 490 files | 499 | **0 modified, 9 added** |

Purely additive in both. No pre-existing row moved, which is the strongest
available statement that the change reaches these nine constructs and nothing
else.

`tests/parser_facade.rs::some_syntax_errors_never_reach_the_error_list_at_all`
was written in M1.0.3 to pin the defect and to fail the day it was fixed. It
failed. It is now
`every_rejected_program_says_why_in_the_error_list`, asserting the corrected
contract over all nine constructs: non-empty `take_errors()`, code `SZ2000`, a
1-based line and column, a non-empty message.

### §5.28 — the interpolation sub-parser's diagnostics are discarded — **open**, medium

Found while fixing §5.17, and **not fixed there**, because fixing it is a design
decision rather than a routing change.

`parse_interpolated_string` re-parses the inside of `"a {b + c} d"` with a
*fresh* `Parser` over just the fragment. That sub-parser prints its own
diagnostic — so the user does see a message — but its error list is dropped when
the sub-parser goes out of scope. Nothing reaches the outer `take_errors()`, so
the LSP still shows nothing for an error inside an interpolated string.

Routing it is not mechanical: the sub-parser's positions are relative to the
fragment, not to the file, so the fix requires **deciding how to map a span from
an interpolated fragment back to the enclosing source** — including that the
fragment has had its `\{` escapes replaced by a sentinel, so offsets do not
correspond one-to-one.

That is a span-mapping decision, which is M4's subject matter, not a diagnostics
routing change. Recorded and assigned there rather than forced into M3. The
`InterpolationFailure::Expression` variant marks the exact site.

### §9D.7 — M3.8: diagnostic ordering (decision **D6** — no behaviour change)

**The wart.** `flush_lexer_errors` runs at the end of `parse_program`, so the
list is grouped by producer rather than sorted by position. Reproduced:

```
$ sz probe.sz          # let a = 0x; / let b = 2; / let = 3;
❌ PARSER ERROR [SZ2000] [probe.sz 3:1]: Expected variable name after 'let'
❌ PARSER ERROR [SZ2000] [probe.sz 3:5]: Unexpected token '=': expected an expression
❌ LEXER ERROR [SZ1004] [probe.sz 1:9]: Invalid hexadecimal integer literal '0x'
```

Line 3 before line 1. `spec/errors.md` documents the codes and the rendered
shape and says **nothing** about order, so nothing is currently violated.

**Why the flush is at the end at all.** `Parser::new` pulls two tokens before
`set_source` and `set_source_name` are called (§5.13), so a lexical error from
construction cannot be rendered when it is detected — there is no file name and
no source to draw a caret from yet. The deferral is not arbitrary.

**Measured blast radius: zero.** Across all 499 corpus files, only **three**
produce more than one diagnostic, and all three are pure `SZ2000`. **No file in
the repository produces both a lexical and a syntactic diagnostic**, so the
ordering is unobservable in the entire corpus; the reproduction above had to be
constructed by hand, and had to use `0x` rather than an unterminated string,
because an unterminated string reads to EOF and swallows the syntax error.

**Decision D6: keep the current order. M3 changes nothing here.**

The reasoning is about *who decides*, not about which order is nicer:

  * The spec permits either. Nothing is broken, and no evidence of user harm
    exists — the case is unreachable in 499 files.
  * Changing it is a **user-facing UX decision**, and the roadmap chartered M3.8
    to *decide*, not to adopt a particular outcome.
  * The comment in `tests/parser_facade.rs` calling position-order "an
    improvement" is **my own note from M1.0.3**, not Sergio's decision. Treating
    it as authorization would be citing myself. It is a proposal, and it stays a
    proposal until Sergio picks it.

**The proposal, costed, for whenever Sergio wants it.** Three options were
considered:

| | Approach | Cost | Public consequence |
|---|---|---|---|
| **A** *(chosen)* | Leave it. | none | none |
| **B** | Sort the list in `take_errors()`. | small | **Worse than either.** The printed order comes from eager printing at the producers, so the list and the output would disagree. Rejected outright. |
| **C** | Flush the queue at the *start* of `parse_program` (labels are set by then) and again inside `next_token`, instead of at the end. | ~5 lines | Diagnostics print in the order the lexer and parser encounter them, i.e. source order. Because the lexer runs one token ahead, a lexical error can still be flushed at most one token early — "source order" is exact in practice, not by construction. |

**C is the recommendation** if the order is ever to change: it is small,
reversible, needs no move of rendering to the pipeline boundary, and the corpus
measurement says **no manifest row would move**. It flips exactly one test,
`lexical_diagnostics_arrive_after_syntactic_ones`, which was written to be
flipped by this decision.

What C is *not*: it is not "render once at the boundary". The producers still
print eagerly. Fulfilling `spec/errors.md`'s boundary sentence literally would
mean buffering every frontend diagnostic until `run.rs` — which touches imports,
the REPL, `--watch` and the interpolation sub-parsers, and is a genuinely
different architecture. Not proposed here.

## 9E. M3 MILESTONE AUDIT

Run at the end of M3.8.

### Definition of Done

| Criterion | Status |
|---|---|
| One diagnostic model | **met** — `LexError`, `ParseError`, `TypeError`, `RuntimeError` are all `pub type … = Diagnostic`. `CompilerDiagnostic` reclassified out with evidence (§9D.5) |
| Data separated from rendering | **met** — `src/diagnostic.rs` does not print; `src/render.rs` returns a `String` and does not decide |
| One renderer | **met** — five print sites, one `render::render` |
| Codes, exit codes, catchability, `Error.span` unchanged | **met** — measured, not assumed; see below |
| The §5.17 defect fixed | **met** — M3.7, its own commit, nine fixtures added |
| Ordering decided | **met** — D6, no change, alternative costed (§9D.7) |
| Gates green | **met** |

### 1. What M3 was for, and whether it happened

Five types, four rendered formats, and producers that printed. Now: one type,
one renderer, and producers that hand it data. `src/diagnostic.rs` (199 lines)
and `src/render.rs` (214) are both leaves — they depend on `span` and on each
other, and on nothing else, which is what let the lexer, the parser, the checker
and the evaluator all adopt them without any of them depending on another.

### 2. The line M3 was told not to cross

> *"No cambies semántica del lenguaje como parte de una migración de
> diagnostics."*

Six of the seven molecules are refactors, and every one of them left **both**
manifests untouched — `parser_ast.manifest` (499 files, structured diagnostics)
and `diagnostic_render.manifest` (158 fixtures, complete stderr plus exit code).
No regeneration, so nothing a user reads moved.

Three hazards were found where the obvious refactor *would* have changed
behaviour, and each was measured rather than reasoned about:

| Hazard | The tempting move | Why it was wrong | What was done instead |
|---|---|---|---|
| Caught `Error.span` (§9D.4) | `Span::is_known()` in place of `Option::is_none()` | asks whether the line is non-zero, not whether there was a frame — a frame at line 0 would flip `"0:0"` to `null` | test `stack.first()`, provably the same predicate for every input |
| The position bracket (§5.27) | omit it whenever the span is unknown | byte-identical only if lexer/parser diagnostics never carry line 0 | asserted over all 499 corpus files, as a permanent test |
| Advisory severity (§9D.3) | let `Phase::Type` default like the rest | `spec/types.md` makes the checker's findings non-fatal; the exit code depends on it | `Diagnostic::frontend` maps `Phase::Type` to `Advisory` explicitly |

Catchability was never modelled. It stays on the evaluator's private
`PendingRuntimeError`, so no diagnostic change can make a fatal error catchable.

### 3. The one behaviour change, and its evidence

M3.7 only. Nine parser sites that rejected a program without telling anyone why
now report through `parser_error`. The exit code did not move — `has_errors()`
was already true.

The evidence is unusually clean: **both manifests regenerated purely additively**
— 0 rows modified, 9 added in each. A behaviour change that touches exactly its
nine constructs and provably nothing else.

It also exposed why the defect survived: those nine paths had **zero coverage**
in 490 files and 149 fixtures. They have nine fixtures now.

### 4. What the nets caught that the pre-existing suite could not

The 808-test gate asserts an exit code and the presence of a `❌`. Both new nets
were perturbation-tested before being trusted:

  * `parser_snapshot` (M1.0.1) — the structured data.
  * `diagnostic_render` (M3.1) — the bytes. One trailing space in a format
    string flagged **129 of 149** fixtures.

The second perturbation is also what found §9D.2: the first attempt changed
nothing because it hit a renderer that cannot fire. That became decision D5.

### 5. Corrections made during M3

Recorded because the protocol says a milestone reports what it got wrong, not
only what it built.

  * **§9D.2 was too weak.** It said the unreachable renderer could still fire
    for an external embedder. It cannot: both public entry points raise the
    capture depth and the method is private. Corrected in D5.
  * **The clippy gate was cache-sensitive** (§5.26). "Exactly 186" reads 187 on
    a forced rebuild *at the baseline commit too*, because per-target summary
    lines get counted. Replaced with the unique per-site list — 181 lines, and
    unchanged by every M3 molecule.
  * **`git add -A` swept an untracked audit report** into `361c2cb`. Amended out;
    the repository is back to the state it was in, plus the molecule.
  * **The M3.4/M3.5 commit message over-claimed**, saying M3.6 would take
    `CompilerDiagnostic`. It does not, and the reasoning is in §9D.5.

### 6. What M3 leaves open, and where it goes

| Item | Where | Why not here |
|---|---|---|
| §5.28 — the interpolation sub-parser's diagnostics are discarded | **M4** | needs a decision on mapping a span from a fragment back to the source, including that `\{` escapes shift offsets. Span work, not routing |
| `CompilerDiagnostic` | whichever milestone owns the AOT compiler | no consumer outside `src/compiler/`, no span, unrelated rendered form |
| The type checker never learns the file name | open, low | its diagnostics say `[line L:C]` where the parser says `[file L:C]`. Fixing it changes what a user reads |
| Ordering (D6) | Sergio's call | costed in §9D.7; option C is ~5 lines and moves no manifest row |

### 7. Gates at close

| Gate | Result |
|---|---|
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets` | **181 unique sites, identical to the M3 baseline** (`comm` both directions empty) |
| `cargo test` | **381 passed, 0 failed** |
| `run_tests.ps1` | **499 passed, 0 failed, 0 skipped** |
| `parser_ast.manifest` | 499 files; regenerated once in M3, additively |
| `diagnostic_render.manifest` | 158 fixtures; regenerated once in M3, additively |

## MILESTONE STATUS: **COMPLETE**

## 9F. M4 — Semantic Layer Established

Charter, from the master plan: *"Resolver, symbols, scopes y validaciones
semánticas fuera del parser."*

### 9F.0 The audit: there is no semantic layer, and four things do its job

The charter's phrasing — *outside the parser* — implies the parser is where this
work currently lives. **It is not.** The parser holds exactly one semantic
validation. The real finding is the opposite shape: the responsibility is not
misplaced, it is **absent and quadruply improvised**.

**Four independent derivations of what a program declares and means:**

| # | Owner | Input | State | Authority |
|---|---|---|---|---|
| 1 | Evaluator | the AST, at run time | `class_registry`, `interface_registry`, `enum_registry`, `global_bindings`, `ScopeStack` (`mod.rs:189-203`) | **authoritative** — this is what the program does |
| 2 | Type checker | the AST, before running | `functions`, `var_types` (`type_checker.rs`) | advisory; `spec/types.md` records how partial |
| 3 | LSP | **the token stream** — it re-lexes | `scan_symbols` -> `Vec<SymbolInfo>` (`lsp/analysis.rs:243`) | what the editor shows |
| 4 | Parser | the token stream, while parsing | `is_reserved_name` (`parser/classes.rs:55`) | rejects 3 declaration forms |

**The LSP throws the parse tree away.** `analysis.rs::analyze` parses, hands the
`Program` to the type checker for warnings, and then calls
`scan_symbols(text, &lines)` — `text`, not `program`. Its signature is
`fn scan_symbols(_text: &str, lines: &[String])`, and it works off
`collect_tokens(text)`, a second lex of the same source. So the outline, hover,
go-to-definition, references and rename a user sees are derived from a token
scan that **has no structural relationship to the AST the compiler built**. They
can disagree, by construction, and nothing tells anyone when they do.

**Nothing resolves a name statically.** There is no resolver. `name -> declaration`
is answered exactly once, at run time, by `ScopeStack::lookup` (`scope.rs:135`).
That is why free variables resolve dynamically and `--check` cannot flag them —
already recorded as **critical, open** in §6.

**The LSP's analysis is unreachable from the test suite.** `mod lsp` is declared
in `src/lsp_main.rs`, not in `src/lib.rs`, so it is binary-local. No integration
test in `tests/` can call `analyze` or `scan_symbols`. Whatever covers it is
inside the binary's own unit tests. Any M4 net for the LSP's symbol view has to
deal with this first.

**The one semantic check in the parser is the smallest of the four problems.**
`is_reserved_name` covers **7 names**; the generated table the LSP uses
(`lsp/builtins_gen.rs`, produced by `tools/gen_lsp_builtins.py` *from the
evaluator*) lists **22**. So the parser rejects `class Task {}` and `class Gui {}`
and accepts `class Math {}`, `class File {}`, `class Socket {}` and
`class Crypto {}` — and a program may then define `class Math`, call
`new Math()`, and still call `Math.floor(3.7)`, both resolving. The guard does
not prevent a collision the language cannot survive.

> **Correction to §5.20:** it says "twenty namespaces". The measured count in
> `builtins_gen.rs` today is **22** — Autodiff, Binary, Crypto, DateTime, Dec,
> Env, File, GPU, Gui, JSON, Math, Media, Memory, OS, Random, Regex, Socket,
> System, Task, Tensor, Terminal, Time. All 7 the parser guards are among them.
> The ratio is 7 of 22, not 7 of 20.

### 9F.1 What this means for M4's shape

M4 cannot be an extraction, because there is nothing coherent to extract. It has
to *establish* the layer, and then move consumers onto it one at a time —
Strangler Pattern, as the plan requires. The order that follows from the audit:

1. A net first. Two of the four derivations (evaluator, type checker) are
   reachable from `tests/`; the LSP's is not, and that gap is itself a molecule.
2. A symbol/scope model in its own leaf module, with **no consumers**, the way
   `src/span.rs` and `src/diagnostic.rs` were introduced.
3. Consumers migrated one at a time, each verified against the net.
4. `is_reserved_name` moves **last**, and it is a **behaviour change** — moving
   it changes *when* the error is reported, and any change to *which* names it
   covers is breaking. Both need to be declared, not absorbed.

**Two questions M4 cannot answer on its own** — both are product decisions, and
both are flagged here rather than decided:

  * **Should free variables resolve statically?** Making `--check` flag them is
    what a resolver is for, and it would reject programs that run today. §6 marks
    it critical and open and assigns it to M4/M7 "needs an explicit product
    decision under `spec/compatibility.md`". It still does.
  * **Should `is_reserved_name` cover all 22?** Extending it is breaking; §5.20
    already says so. Leaving 7 keeps an arbitrary rule. Neither is mine to pick.

Work that does **not** depend on either answer comes first.

### 9F.2 — M4.1: the divergence, measured

§9F.0 said the editor's outline and the parse tree "can disagree, by
construction". That was an assertion. It is now a number.

`src/lsp/analysis.rs::semantic_divergence` walks all 483 in-repo `.sz` files and
asks both derivations the same question — *which names does this file declare at
the top level?* — restricted to the four forms both report, so a difference is a
real disagreement rather than two tools answering different questions.

**Result: 95 of 483 files (~20%) disagree. Every one of them disagrees in the
same direction.**

| Direction | Meaning | Count |
|---|---|---|
| `scan - tree` | the outline shows a top-level symbol the tree does not have | **95** |
| `tree - scan` | the outline **omits** a top-level declaration the tree has | **0** |

The asymmetry is the useful part, and it identifies the mechanism. The token
scanner is **not nesting-aware**: `tests/unit_unsafe_block.sz:51` declares
`fn int double(int n)` *inside* a `test(…)` lambda, and `scan_symbols` reports it
with `container: None` — as a top-level symbol. The corpus is full of unit-test
files shaped that way, which is why it is a fifth of them.

**This is a limitation of the design, not a coding error.** The scanner exists so
that `.szx` documents — JSX, which the parser does not understand — still get an
outline; the module says so. Tolerating arbitrary broken regions means not
relying on structure. Applying that to plain `.sz`, where a correct AST has
already been built and thrown away, is the waste M4 can remove.

**What the test asserts, and what it only reports.** The two directions are not
equally serious, so they are not treated alike:

  * `tree - scan` is **asserted to be empty**. The editor omitting a symbol that
    exists means go-to-definition failing on code that compiles — a correctness
    property, and it holds today.
  * `scan - tree` is **printed, not asserted**. It is the size of the gap M4 is
    closing, and the count moves whenever a fixture is added, so pinning it would
    produce noise rather than signal.

It lives in `src/lsp/analysis.rs` rather than `tests/` because `mod lsp` is
declared in `src/lsp_main.rs` and not in `src/lib.rs` — §9F.0's finding, met in
practice. Exposing `lsp` from the library would let it move to `tests/`, but the
LSP binary carries `#![allow(dead_code)]`, so compiling it into the library would
move the clippy baseline. Not worth it for a test's address.

### §5.29 — the type checker cannot see through `export` — **FIXED in M5.3**, see §9H.3 (found in M4.4)

`TypeChecker::check` has three passes. **Pass 3 unwraps `Statement::Export`
(`type_checker.rs:226`); passes 1 and 2 do not.** So the body of an exported
function is checked, but the declaration itself never enters `self.functions`,
and an exported `let` never enters `self.var_types`.

Reproduced against the 10.0.0 build, twice:

```
$ sz a.sz     # fn int f(int a) {...}   let x = f("hello");
❌ TYPE ERROR [SZ3000] [line 2:14]: Parameter 'a' of 'f' expected 'int' but received 'string'.
❌ ERROR [SZ4002]: Parameter 'a' expected 'int' but received 'string'

$ sz b.sz     # export fn int f(int a) {...}   let x = f("hello");
❌ ERROR [SZ4002]: Parameter 'a' expected 'int' but received 'string'
```

The same mistake is caught statically for a plain declaration and only at run
time for an exported one. The same holds for `export let`: its inferred type is
missing, so calls using it are unchecked.

**Why it is an oversight rather than a design choice:** the inconsistency is
*inside one function*. Pass 3 handles `Export` and passes 1 and 2 forget to.
`spec/types.md` never mentions `export`, so nothing documents the difference.
`semantic::declarations` already unwraps `Export` correctly, which is how the
discrepancy surfaced.

**Not fixed here.** M4.4 found it while surveying consumers for migration, and
the bug protocol says a bug found mid-milestone is documented and assigned, not
fixed inside other work. Fixing it makes the checker report advisory `SZ3000`
findings it does not report today — the exit code cannot move, since
`spec/types.md` makes type findings advisory, but stderr does, so
`diagnostic_render.manifest` would move for any affected fixture.

**Assigned to M5**, whose charter is exactly this: *"Reglas de tipos coherentes,
normativas y consistentes entre checker/runtime/tooling."* A checker that
disagrees with the runtime about the same program is the milestone's subject.

> **Fixed in M5.3** (§9H.3). Passes 1 and 2 now unwrap through `export` via one
> helper. `tests/type_agreement.rs` pins both symptoms — the exported function and
> the exported `let` — against a control case that is identical but for the
> keyword, which is what makes the gap a defect rather than the checker's
> documented partiality. No manifest row moved, as this section predicted.

**Corpus exposure is small**: 3 of 483 files use `export`, and none combine it
with a type error, which is why no manifest row would move today and why no test
caught it.

### §9F.3 — M4.2-M4.3: the symbol layer, delivered and corpus-validated

`src/semantic.rs` — a leaf over `ast` and `span`, 8 unit tests, no consumers in
the product. `declarations(&Program)` returns every declared name with its kind,
its span, its container and, crucially, its **depth**. `top_level` filters on
depth `0`.

Depth is the fact a token scan cannot recover, and it is the whole of §9F.2's
95-file gap.

M4.3 then replaced the divergence test's hand-rolled walk with
`semantic::top_level` itself. The numbers did not move — still 95 over-reporting,
still 0 hiding — so the module reproduces the ad hoc walk on **all 483 corpus
files**, not merely on its own unit tests. That is the validation; a second
hand-written walk would only have proved the test agreed with itself.

### §9F.4 — **M4 IS BLOCKED.** What is left, and why none of it is mine to decide

Three items remain in M4's charter. Each one hits a stop-trigger, and the
evidence for each is below so that the decision can be made quickly.

#### (a) Move `is_reserved_name` out of the parser — **not behaviour-preserving by any route**

I looked for a way to move it that changes nothing. There is none:

  * The error is currently **fatal at parse time** and aborts the declaration —
    `parse_class_declaration` returns `None`, so the class never enters the AST.
    Moving the check later means the class *is* parsed, which changes
    `parser_ast.manifest` by construction.
  * The only pre-run pass that exists is the **type checker, which is advisory**.
    `spec/types.md` makes its findings non-fatal. Moving a fatal error there
    would make `class Task {}` **run**. That breaks a documented contract.
  * The remaining option is a **new fatal phase** between parser and checker.
    That is a pipeline change with public consequences: a new phase label in the
    rendered diagnostic, a code range to choose, and a different position in the
    diagnostic order.

**Measured 2026-09-02 — the change is one manifest row wide.** "Changes
`parser_ast.manifest` by construction" was left as a worst case. It is now
counted: exactly **one** file in the whole tracked corpus is rejected by this
guard, `tests/err_task_reserved_class.sz`, and it is the fixture written to
assert the guard. `diagnostic_render.manifest` pins it at one row (`exit 1`, 222
bytes); `parser_ast.manifest` pins the failed parse. So the blast radius of
moving the check is **two manifest rows on one fixture that exists to test the
thing being moved** — not a corpus-wide re-baselining.

**And it pays for itself immediately:** the 222 bytes are large because two of
the three diagnostics in them are spurious (§5.32). A post-parse phase reports
one error instead of three. The row does not merely move — it gets better, and
that improvement is reviewable in a single diff.

> **Decision needed:** accept a new fatal semantic phase, or leave the check in
> the parser? *(Recommendation: **a new phase.** It is the right architecture,
> it fixes §5.32 for free, and the measured cost is two manifest rows on the
> guard's own fixture. The reason to still decide it deliberately is the public
> surface — a new phase label in rendered output and a code range to choose —
> not the risk of the change.)*

#### (b) A static resolver — gated on a decision §6 already flagged

`name -> declaration` is answered once, at run time, by `ScopeStack::lookup`.
A resolver that answers it earlier is easy to write and useless until something
consumes it; a resolver that **reports** an unresolved name changes which
programs `--check` accepts. §6 already records this as *critical, open, needs an
explicit product decision under `spec/compatibility.md`*, and it still does.

> **Decision needed:** should a free variable be a diagnostic? *(Recommendation:
> before deciding, measure how many real programs rely on it. That measurement
> is the one piece of independent work here — see below.)*

#### (c) `is_reserved_name` coverage — 7 of 22

§5.20 already says extending it is breaking. Leaving it at 7 keeps a rule whose
membership looks accidental: `class Gui {}` is rejected and `class Math {}` is
accepted, and a program may define `class Math`, call `new Math()`, and still
call `Math.floor(3.7)`.

**Measured 2026-09-02 — extending to 22 breaks nothing that exists.** Every
tracked `.sz`/`.szx` in this repository (the 491-file corpus, including `std/`,
`apps/` and `benchmarks/`) and every source file in all **eight official
ecosystem packages** was searched for a `class`, `interface` or `enum`
declaration named after any of the 22 namespaces:

| Scope | Collisions |
|---|---|
| Language repo, tracked corpus | **1** — `tests/err_task_reserved_class.sz`, the fixture that asserts the guard, on a name already among the 7 |
| serez-ui, -http, -ai, -agentai, -pack, -apipack, -dotenv, -graph | **0** |

So extending 7 → 22 is a **breaking change under SemVer with no measured
victim**: it rejects programs that are legal today, and no program that exists
today is one of them. That does not make it free — an unmeasured user program is
still a user program, and `spec/compatibility.md` governs — but it moves the
question from "how much do we break?" to "do we accept a theoretical break to
close §5.31?"

The guard covers three declaration forms (`class`, `interface`, `enum` —
`parser/classes.rs:80,337,449`) and nothing else. Variables and functions may
already take these names unguarded, which is a separate question this decision
does not settle.

> **Decision needed:** extend to 22 (breaking, **0 measured victims**), leave at
> 7 (arbitrary, keeps §5.31), or remove the guard entirely (breaking in the other
> direction, and makes §5.31 the rule rather than the exception). *(Recommendation:
> **extend to 22**, in the same release as (a), so the rule and its location change
> once rather than twice.)*

#### What I did not do, and why

The one genuinely independent piece of work left is **measuring free-variable use
across the corpus**, which would quantify (b) for whoever decides it. I did not
do it, and the reason is worth recording rather than hiding: a correct scope
model has to handle closures, `this`, class bodies, `for`-in bindings, `catch`
bindings, destructuring and generators. A half-correct one produces a *confident
wrong number*, and a wrong number is worse input to a product decision than no
number. It should be built deliberately, as its own molecule, once (a) is
settled — because whether a semantic phase exists determines where it lives.

**Repository state: green.** Gates below. Nothing is half-migrated: `semantic`
has no product consumers, so there is no partial state to unwind.

> **Correction, 2026-09-02.** The paragraph above concluded that the free-variable
> measurement was "the one genuinely independent piece of work left", and that
> everything else waited on a decision. That was too strong, and the re-entry
> session found two pieces of independent work it had missed — both of them
> *inputs to the decisions themselves* rather than steps past them: the blast
> radius of (a) and (c), now measured above, and the probes behind §5.31 and
> §5.32. The reasoning that excluded them is worth naming, because it is a
> reusable mistake: "I am blocked on a decision" was read as "I can do nothing",
> when the useful move while blocked is almost always **to make the decision
> cheaper**. The free-variable judgement itself still stands — a half-correct
> scope model produces a confident wrong number, and that is worse than none.

### §9F.6 — M4.7.1-M4.7.2: the scope model, and the number it produced

Both molecules are **COMPLETE**, and both were independent of every open
decision — building a model and measuring with it commits to nothing.

**M4.7.1 — `src/semantic/scopes.rs`.** A lexical scope model over the AST, 12
unit tests, **no product consumers**, no diagnostics, nothing rejected: the shape
`span`, `diagnostic` and `semantic::declarations` were each introduced in. It
answers one question — *walking only lexical structure, which identifier uses
cannot be accounted for?* Every rule in it was probed against the 10.0.0 binary
first, and the probe table is in §5.33; the model is deliberately biased toward
"bound", so its output is a floor rather than an estimate.

**M4.7.2 — `tests/scope_resolution.rs`.** Runs it over 486 conclusively
analysable corpus files, and optionally over out-of-repo roots through
`SEREZ_SCOPE_EXTRA_ROOTS`, which is how the 8 ecosystem packages were included
without hard-coding one machine's layout. Following M4.1's method, it **asserts**
the properties that are correctness — no panic on any file including the 23 that
do not parse, every reported span inside its own file, the corpus not silently
collapsing — and **reports** the measurement, which moves whenever a fixture is
added.

**The result, in one line: the corpus does not rely on dynamic name resolution.**
Four unaccounted uses inside a function across 486 files, and all four are the
fixtures written to contain an undefined name. §5.33 has the tables.

**Three findings came out of it**, none of which the molecule went looking for:
§5.34, three security fixtures that assert nothing and pass; §5.35, six calls in
`serez-ui` to a namespace that does not exist; §5.36, the two method errors the
measurement made before it was trustworthy.

**What this does not do.** It does not answer DEC-M4-002 — it supplies the
evidence the decision was missing, and sharpens the recommendation from "measure
first" to "B, and here is the number". M4.7.3 remains blocked, because reporting
is the part that changes which programs the language accepts.

### §9F.5 — M4's remaining molecules, conditioned on the decisions

Nothing here is authorized. This is the decomposition each answer unlocks, so
that the decision can be taken against the work it implies rather than in the
abstract.

**If (a) = "a new fatal semantic phase"** — the largest branch, and the one that
also settles where (b) would live:

| Molecule | Action | Risk |
|---|---|---|
| M4.5.1 | Choose the phase's public surface: label in rendered output, code range, position in the diagnostic order (D6 governs). Decision record, no code | LOW |
| M4.5.2 | Introduce the phase as a no-op between parser and checker — runs, reports nothing, wired into `run::run_source_detailed`. Manifests must not move | LOW |
| M4.5.3 | Prove the net sees it: make the empty phase report once, confirm the manifest moves, revert (the M1.0.2 method — the only step that proves the harness is load-bearing) | LOW |
| M4.5.4 | Move the reserved-name check into it, unchanged at 7 names. **Behaviour change**: two manifest rows on one fixture; §5.32's cascade disappears | MEDIUM |
| M4.5.5 | Delete the parser's copy and its three call sites; `parser/classes.rs` loses its only semantic rule | LOW |
| M4.5.6 | Spec: record the phase in `spec/errors.md` and the reserved-name rule in `spec/classes.md`, which does not state it today | LOW |

**If (c) = "extend to 22"** — one molecule, and it must land *after* M4.5.5 so
the rule changes in its new home, not twice:

| Molecule | Action | Risk |
|---|---|---|
| M4.6.1 | Extend the list to 22, generated from the same source as `lsp/builtins_gen.rs` rather than hand-written, so the parser and the editor cannot drift apart again. Fixtures for the 15 newly-rejected names. **Breaking, 0 measured victims** | MEDIUM |

**If (b) = "a free variable should be a diagnostic"** — the largest and least
determined branch; it needs its own goal decomposition after (a), because where
the resolver reports determines what it may report:

| Molecule | Action | Risk |
|---|---|---|
| M4.7.1 | Build the scope model *without reporting*: closures, `this`, class bodies, `for`-in, `catch`, destructuring, generators. No consumers, the `semantic.rs` pattern | MEDIUM |
| M4.7.2 | Measure free-variable use across the 491-file corpus and all 8 ecosystem packages. **This is the number (b) actually needs**, and M4.7.1 is what makes it trustworthy | LOW |
| M4.7.3 | *Then* decide severity — and only then does the rest of (b) decompose | — |

**If all three are "leave it as it is"**, M4 closes as **INCOMPLETE by decision**
rather than by omission: `src/semantic.rs` stays a validated leaf, §5.31 and the
free-variable resolution stay recorded as accepted debt, and the milestone audit
records that the layer was established but not adopted. That is a legitimate
outcome and should be written down as one, not left ambiguous.

---

## 9H. M5 — Type System Stable

Charter: *"Reglas de tipos coherentes, normativas y consistentes entre
checker/runtime/tooling."*

### 9H.0 The audit: the spec is good, and the checker does not implement it

M5's premise is that type rules might be incoherent. `spec/types.md` turns out to
be the strongest document in `spec/` — 230 lines, every rule derived by probing
the running implementation, with a "Known gaps" section that names its own
limitations rather than hiding them. It is not the problem.

**The problem is that there are two implementations of its matching table.**

| | Implements | Size | Authority |
|---|---|---|---|
| `evaluator::type_matches` (`mod.rs:2229`) | values -> declared type | 25 arms | **authoritative** — enforces the contract |
| `type_checker::types_compatible` | type *names* -> declared type | **8 lines** | advisory — prints and does not stop anything |

The checker's version was not a subset of the runtime's. It disagreed, and every
disagreement found is one where **`spec/types.md` sides with the runtime**, which
is what makes these fixes ordinary work rather than product decisions.

**Four divergences, all probed against the 10.0.0 binary:**

| # | Program | Runtime | Checker | Spec says |
|---|---|---|---|---|
| 1 | `fn void nothing() { return null; }` | accepts | **reports** | "`void` \| `null`" — the checker is wrong |
| 2 | `[string]` passed to a `[int]` parameter | accepts | **reports** | "`[T]` \| **any array**, whatever its elements" — the checker is wrong |
| 3 | `[int]` passed to an `array` parameter | accepts | **reports** | "`array` … recognized as a type name by the runtime matcher" — the checker is wrong |
| 4 | an `int?`-typed value passed to an `int` parameter | accepts | **reports** | nothing — **DEC-M5-001** |
| 5 | `export fn int f(int a)` called with a string | rejects | **silent** | nothing makes `export` change what is checked — §5.29 |

The first three are **false positives**: the checker printing an error over a
program that runs correctly. That is the serious direction for an advisory tool.
Its findings change neither the exit code nor whether the program runs, so a
finding on correct code is pure noise, and noise on correct code is how a linter
teaches people to ignore it. `fn void f() { return null; }` is the most ordinary
way to write a void function.

The fifth is a **false negative** and is §5.29, inherited from M4.

### 9H.1 — M5.1: the net

`tests/type_agreement.rs` runs each case through **both** halves — the real
`TypeChecker` and the real `Evaluator` — and holds them to each other:

  * **asserted**: the checker reports nothing about a program the runtime
    accepts, unless the case is in `KNOWN_DIVERGENCES` *with a stated reason*;
  * **asserted**: a case marked `checker_must_catch` is caught. That flag is only
    legitimate where the checker demonstrably handles the same shape written
    differently — so the pair `a_plain_function_is_checked` /
    `an_exported_function_is_checked_like_any_other` is what makes §5.29 a defect
    rather than the documented partiality;
  * **reported**: every other miss, because `spec/types.md` says the checker is
    deliberately partial and reaching further is an improvement, not a contract.

`KNOWN_DIVERGENCES` is also checked for **staleness**: an entry that no longer
diverges fails the test. A list of intended divergences that outlives the
divergence becomes a place where a fixed defect is recorded as intended, and this
one cannot.

Verified load-bearing before being trusted: the net was run against the
unmodified checker and reported exactly the three false positives and both
symptoms of §5.29, naming each.

### 9H.2 — M5.2: the checker's matcher, corrected

`types_compatible` now implements the name-level half of `type_matches`:
`void` accepts `null`, and an array annotation in either spelling (`[T]` or
`array`) accepts an array in either spelling. What it still cannot express is not
a divergence: the arms of `type_matches` that inspect a *value* — a class
instance's name, an enum variant's enum, a `DateField` behaving as an `int` —
have no name-level counterpart, and `infer_type` never produces those names, so
the two never meet on them.

**Measured impact on the corpus: zero, and that is the finding.** The committed
binary and the fixed one both emit exactly **3** `SZ3000` findings across every
tracked `.sz` file — `err_arity.sz`, `err_type_param.sz`, `sec_type_violation.sz`
— and all three are legitimate. So no true positive was lost, and no corpus file
was suffering a false positive.

The corpus is silent on this for a reason worth recording: **nothing pins stderr
for a program that succeeds.** `diagnostic_render.manifest` covers `err_*` and
`sec_*` — failing programs. The e2e fixtures compare **stdout**. So a spurious
`TYPE ERROR` printed over a correct program is invisible to every existing gate,
which is precisely why these three survived. `tests/type_agreement.rs` is now the
gate that sees them.

### 9H.3 — M5.3: the checker sees through `export` (§5.29 fixed)

`TypeChecker::check` has three passes. Pass 3 unwrapped `Statement::Export`;
passes 1 and 2 did not. So an exported function never entered `self.functions`
and an exported `let` never entered `self.var_types` — the declaration was
invisible to every check that needs to look one up, while its *body* was checked
normally.

Both passes now unwrap through `export`, via one helper that says why. The
inconsistency was inside a single function, `spec/types.md` never mentions
`export`, and `semantic::declarations` had unwrapped it correctly since M4.2 —
which is how the discrepancy surfaced.

**This makes the checker report findings it did not report before**, which is a
change to stderr. The exit code cannot move: `spec/types.md` makes type findings
advisory. Corpus exposure was measured in M4 at 3 of 483 files using `export`,
none combining it with a type error, and the manifests confirm it: no row moved.

### 9H.4 — M5.4: the tooling leg, and two diagnostics that pointed nowhere

The charter's third consumer is tooling, and it turns out to be satisfied
**structurally rather than by coincidence**: `lsp/analysis.rs:151` constructs the
same `TypeChecker` the CLI does and maps its findings to LSP diagnostics with
`severity: 2`. There is no second implementation to keep in step, so M5.2's and
M5.3's fixes reached the editor with no further work. Worth recording as a
positive finding — §9F.0 found the opposite shape for symbols, where the LSP
re-derives everything.

**But the LSP inherits the checker's positions, and two of the four checks had
none.** `type_error(0, 0, …)` was used for the return-type mismatch and the
array-literal element mismatch. A position of `0` is the checker's "unknown", and
it is not merely untidy:

  * the CLI's renderer drops the `[line L:C]` bracket entirely;
  * `lsp/analysis.rs` documents `0` as "mapped to the start of the file", so an
    **editor underlines line 1** for a mistake anywhere in the program.

Both now carry the span of the node they are about — `ret.span` and `arr.span`.
M2 gave every AST node a span for exactly this, and §5.10 recorded at the time
that nothing consumed them; the checker is now among the first that does.

**Measured impact: none on the corpus.** No tracked file produces either
diagnostic — the whole corpus emits 3 `SZ3000` findings and all three already
carried a call span — so no manifest row moves. The improvement is entirely for
code not in the repository, which is the same shape as M5.2.

`every_finding_the_checker_emits_points_somewhere` asserts the property for all
four checks rather than the two that were broken, so a fifth check cannot be
added without one.

**Left undone, deliberately:** the array-literal diagnostic points at the literal
rather than at the offending *element*, which would be better. Doing that needs an
`Expression::span()` accessor, and `ast.rs` has none — M2 gave all 28 variants a
span field but no generic way to ask for one. That accessor is worth adding on its
own terms, and is recorded here rather than smuggled into a diagnostic fix.

---

## 9G. M4 MILESTONE AUDIT

Charter: *"Resolver, symbols, scopes y validaciones semánticas fuera del parser."*

### Definition of Done, item by item

The plan states M4's DoD as: *the parser answers "is this syntactically valid?"
and the semantic layer answers "what does this reference mean inside the
program?"*

| Item | Status | Evidence |
|---|---|---|
| A semantic layer exists, separate from the parser | **met** | `src/semantic.rs` (declarations) + `src/semantic/scopes.rs` (lexical scope), both leaves over `ast`/`span` |
| Symbols extracted | **met** | `declarations` / `top_level`, validated on all 483 corpus files (§9F.3) |
| Scopes extracted | **met as a model** | §9F.6; 12 unit tests, every rule probed against the 10.0.0 binary |
| Name resolution moved out of run time | **NOT met** | held by **DEC-M4-002**. A resolver that reports changes which programs are accepted |
| Semantic validation moved out of the parser | **NOT met** | held by **DEC-M4-001**. No behaviour-preserving route exists |
| Consumers migrated onto the layer | **NOT met** | held by **DEC-M4-004**. The LSP still re-lexes and discards the parse tree |

**Three of six met. M4 is PARTIAL, and the three unmet items are unmet because
they are held by registered decisions, not because they were skipped.**

### 1. What M4 was for, and what actually happened

M4 was chartered as an extraction — move semantic work *out of the parser*. The
M4.0 audit found the premise wrong: the parser holds exactly **one** semantic
check, and the responsibility was not misplaced but **absent and quadruply
improvised** — evaluator, type checker, LSP and parser each deriving what a
program declares, from three different inputs (§9F.0).

So M4 became an establishment rather than an extraction, and what it established
is real: two modules, no product consumers, validated against the whole corpus.
What it could not do is *adopt* them, and every one of those adoptions turns out
to change something a user can see.

### 2. Responsibilities not actually extracted

Named honestly, since "PARTIAL" is only useful if it says which part:

  * `is_reserved_name` is **still in the parser** (`parser/classes.rs:55`), and
    still the only semantic rule there. DEC-M4-001.
  * `ScopeStack::lookup` is **still the only name resolution in the language**.
    `semantic::scopes` models the same question statically and reports nothing.
    DEC-M4-002.
  * The LSP **still re-lexes** and discards the parse tree. DEC-M4-004.

### 3. Circular or new dependencies

None. `semantic` depends on `ast` and `span` and is depended on by nothing in the
product — `crate::semantic::top_level` appears once outside the module, inside a
`#[cfg(test)]` block in `lsp/analysis.rs`. `src/semantic/scopes.rs` is reached
only from `tests/scope_resolution.rs`. Both are leaves by construction, which is
what makes each future adoption a separately reviewable step.

### 4. Duplication introduced

One deliberate instance. `semantic::scopes` lists the 21 builtin globals and 22
namespaces that `evaluator/expr.rs` and `lsp/builtins_gen.rs` also know. Both
lists are transcribed with their source cited at the definition site. This is
duplication and is recorded as such: generating them from one source is the right
answer and is part of DEC-M4-003's option A, which is where it belongs rather
than in a measurement module.

### 5. Semantic drift

**None.** No product source file was modified in M4. `git diff` across the
milestone touches `src/semantic.rs` (one `pub mod` line), the two new files, test
fixtures, manifests and documentation. The parser, evaluator, type checker and
LSP are byte-identical to where M3 left them.

The one behaviour change in the milestone's commits is in **tests**: three
security fixtures now execute their assertions (§5.34, §5.37). No program's
behaviour moved.

### 6. Gates, at close

| Gate | Result |
|---|---|
| `cargo fmt --check` | **PASS** |
| `cargo check --all-targets` | **PASS**, no warnings |
| `cargo clippy --all-targets` | **PASS**, 0 errors |
| Clippy per-site list (§5.26) | **181 lines — unchanged across all of M4** |
| `cargo test --all-targets` | **PASS**, 424 / 0 failed |
| `run_tests.ps1` | **PASS**, 499 / 0 / 0 |
| `run_tests.sh` | **PASS**, 499 / 0 / 0 |
| Runner parity | **identical per category** |
| Ecosystem canary | **PASS**, 8 / 8 |

### 7. What M4 found that nothing else would have

Worth separating from what it built, because the findings came from measurement
rather than from construction, and three of the five were not what the molecule
was looking for:

  * **§5.33** — the corpus does not rely on dynamic name resolution. Four
    unaccounted uses inside a function across 486 files, all four in fixtures
    written to hold an undefined name. This is the number DEC-M4-002 was
    registered without.
  * **§5.35** — `serez-ui` calls `Int.parse` at six sites and `Int` does not
    exist. Thirty-six passing tests and a green ecosystem canary never saw it; a
    resolver written for a different purpose found it on its first run. The
    strongest single argument in the register for DEC-M4-002.
  * **§5.34 / §5.37** — three security fixtures asserting nothing and passing,
    23 assertions that had never executed. Fixed, with a guard that was verified
    to fail before being trusted.
  * **§5.36** — the measurement's own two method errors, which moved the headline
    figure from 39.5% to 4.9%. Recorded because those two numbers argue for
    opposite decisions, and because §9F.4's refusal to guess is what that warning
    looks like when it comes true.
  * **§5.31 / §5.32** — `class Math` coexisting with the `Math` namespace, and
    the reserved-name guard's two spurious cascade errors.

### 8. Documentation and `MATURITY_AUDIT.md`

`ROADMAP_STATE.md` carries §7A (the decisions register), §9F.6 and §§5.30–5.37.
`MATURITY_AUDIT.md`'s "free variables resolve dynamically" row is still open and
still correct; §5.33 now supplies its missing measurement, and §6 points at it.

### 9. Commits

`53518e7` M4.0 · `6a32ade` M4.1 · `795d94f` M4.2 · `c2775bd` M4.3 · `de485e9`
re-entry checkpoint · `75d5f69` decision measurements · `6ae8b38` the register ·
`69f9553` M4.7.1-M4.7.2 · `8e67150` the security fixtures.

### 10. What M4 hands forward

| To | What |
|---|---|
| The decision owner | DEC-M4-001, -002, -003, -004, all with evidence and a marked recommendation |
| M5 | §5.29 — the type checker cannot see through `export` |
| M7 | scope semantics, once DEC-M4-002 is answered |
| M10 | §5.35 (an ecosystem defect) and the absence of any compatibility clause for tooling behaviour, noted under DEC-M4-004 |

---

## MILESTONE STATUS: **PARTIAL**

Three of six DoD items met. The layer is established, validated and unadopted.
Every unmet item is held by a registered decision — DEC-M4-001, DEC-M4-002,
DEC-M4-004 — and none of them is unmet through omission. The repository is green
and nothing is half-migrated: both modules are leaves, so there is no partial
state to unwind if a decision goes the other way.

## 9J. M6 — Runtime Molecular

Charter: *the evaluator should evaluate the language, not simultaneously be a
filesystem, a process manager, a socket manager, a GPU manager, a GUI runtime, a
media runtime, a task scheduler, a memory manager and an autodiff runtime.*

### 9J.0 The audit: 48 fields, and what each one is

`Evaluator` had **48 fields**. §6 already records that `handles.rs`, `GuiRuntime`
and `modules.rs` were extracted before this roadmap began, so M6 is partly
pre-empted; the audit's job is to say what is left and in what order it can move.

Classified by the taxonomy the plan asks for:

| Class | Fields | Count |
|---|---|---|
| **Language state** | `global_arena`, `global_bindings`, `scopes`, `null_ref`, `true_ref`, `false_ref`, `class_registry`, `interface_registry`, `enum_registry`, `sealed_classes`, `const_names`, `native_fns` | 12 |
| **Execution state** | `call_stack`, `call_depth`, `constructing_class`, `executing_class`, `in_unsafe_block`, `yield_collector`, `try_depth`, `diagnostic_capture_depth`, `value_depth_exceeded`, `last_error`, `error_generation`, `source_lines` | 12 |
| **Module state** | `imported_files`, `current_dir`, `current_module_exports` | 3 |
| **Cache** | `int_cache`, `mutator_cache`, `super_cache` | 3 |
| **Security policy** | `permissions`, `lockdown` | 2 |
| **Host service** | `sockets`, `gpu`, `memory`, `gui`, `spawned`, `media`, `task_runtime`, `task_id`, `task_arg`, `lcg_state`, `tensor_id_counter`, and the 5 autodiff fields | 16 |

**Language state and execution state are 24 of the 48 and are not extraction
targets.** They are what an evaluator *is*. The milestone's subject is the other
24, and specifically the 16 host-service fields, because those are the ones that
make the evaluator something other than an evaluator.

**Extraction order, derived from measured coupling** rather than from how large
each cluster looks. Reference counts and the number of files touching each field:

| Cluster | Fields | Refs | Files | Order |
|---|---|---|---|---|
| Autodiff tape | 5 | 120 | 2 | **1st** — most fields, and 4 of 5 confined to one file |
| Module state | 3 | 25 | 2 | 2nd |
| Task context | 3 | 9 | 2 | 3rd |
| Security policy | 2 | 12 | 4 | 4th — fewest fields, widest spread |
| Method caches | 2 | 4 | 1 | 5th |

Already single-field and therefore already extracted in the sense that matters:
`sockets`, `gpu`, `memory` (all `handles.rs`), `gui` (`GuiRuntime`), `spawned`,
`media`.

**Not a target: `lcg_state` and `tensor_id_counter`.** One field each, and neither
has a second field to be cohesive *with*. Wrapping a single `u64` in a struct
produces a file, not a boundary — rule 3 of the molecular architecture rules.
They are recorded here so that a later reader knows they were considered and
declined, rather than missed.

### 9J.1 — M6.1: the autodiff tape

Five fields become one: `namespaces_autodiff::AutodiffTape`, owning `recording`,
`tape`, `grads`, `next_id` and `tensor_ids`, held by `Evaluator` as `autodiff`.

The evidence that they are one thing rather than five that happen to share a
prefix is behavioural: **`Autodiff.record()` and `Autodiff.stop()` each reset
three of them together** (`namespaces_autodiff.rs:333-335, 345-347`). `grads` is
only ever populated by `backward()` walking `tape`; `tensor_ids` only maps
identities that exist inside one recording session. Apart, none of them means
anything.

**`tensor_id_counter` was deliberately left behind**, and this is the part of the
molecule worth arguing. It has `tensor` in its name and sits in the same region of
the struct, so folding it in would look tidy. But it issues stable identity for
*every* tensor, whether or not anything is recording — `Evaluator::alloc` uses it
on the allocation path. It belongs to a tensor service that does not exist yet.
Moving it here would put a field used by all tensor allocation inside a type named
after differentiation, which is exactly the tidy-looking mistake the milestone is
meant to avoid.

**Behaviour: unchanged, and mechanically so.** 120 call sites renamed
`self.ad_x` -> `self.autodiff.x`, one of them written across two lines and caught
by the compiler rather than by the first regex — which is the argument for a
rename the type system checks rather than a textual one. No logic, no ordering and
no initialisation moved: `next_id` still starts at 1, expressed as
`AutodiffTape::new()` because `Default` cannot say so.

`Evaluator`: **48 fields -> 44.**

### 9J.2 — M6.2: the module context

Three fields become one: `modules::ModuleContext`, owning `loaded`,
`current_dir` and `exports`, held by `Evaluator` as `modules`.

The evidence that they belong together is again in the code that uses them, not
in their names: `eval_import` **saves `current_dir` and `exports`, runs the
module, and restores both**. It is a push and a pop of one context, spelled as two
independent fields on a 44-field struct — which is precisely the shape that makes
it possible to save one and forget the other.

**`loaded` is in the struct but is *not* saved and restored, and that asymmetry is
deliberate.** A module runs once per *program*, not once per importing scope, so
the set has to outlive the context switch the other two take part in. That is the
mechanism that makes import cycles terminate: the path is marked before the body
runs, so a second import of a file already on the stack is a no-op. Grouping the
three without saying this would leave the next reader to assume all three are
scoped, and to "fix" the asymmetry.

Behaviour unchanged: 25 sites renamed, no logic moved, and the import path is
covered end-to-end by the `import` and `package-manager` categories plus the
ecosystem canary — all green.

`Evaluator`: **44 fields -> 42.**

### 9J.3 — M6.3: the last three clusters

Seven fields become three. Grouped in one molecule because they are the same
mechanical change under the same contract, and because each is too small to carry
a commit of its own without obscuring rather than clarifying — the same judgement
rule 3 applies to files.

| New type | Absorbs | Where it lives |
|---|---|---|
| `permissions::SecurityPolicy` | `permissions`, `lockdown` | beside the grant classification that already lived there |
| `namespaces_task::TaskContext` | `task_runtime`, `task_id`, `task_arg` | beside `TaskRuntime` |
| `DispatchCaches` (private) | `mutator_cache`, `super_cache` | `evaluator/mod.rs`, private because nothing outside needs it |

Each groups on a stated invariant rather than on a shared name prefix:

  * **`SecurityPolicy`** — every gate in the runtime asks *"is this granted, and
    are we in lockdown?"* as one question. The type also carries the distinction
    that makes the pair counter-intuitive: `granted` is a **manifest, not a
    sandbox** — a program can hand itself the lot with `use permissions { .. }` —
    and `lockdown` exists to close the paths the manifest cannot. That was a
    comment on one field before; it is now attached to both, where it is true.
  * **`TaskContext`** — `id` and `arg` are `None` in a parent and `Some` in a
    worker, so together they answer "am I a worker, and what was I given?" That is
    one question, and it was asked in three places.
  * **`DispatchCaches`** — the grouping states the invariant that makes caching
    *sound* here: both answer a question about a **declaration**, and a
    declaration cannot change while a program runs. A cache keyed on anything
    mutable would be a bug, and saying so once is worth more than the two lines it
    saves.

**A `self.`-only rename missed four sites** — `first.task_runtime` and
`evaluator.task_runtime` inside `#[cfg(test)]` code — and the compiler named all
four. Worth recording as the same lesson M6.1 produced from a different angle: a
textual rename is a proposal, and the type system is what checks it.

`Evaluator`: **42 fields -> 38**, and 48 -> 38 across M6 so far.

### 9J.4 — M6.4: the services answer questions instead of exposing fields

M6.1-M6.3 grouped 15 fields into 5. That is ownership, not yet a *service*: the
types were data, and every operation on them was still spelled out at the call
site. The charter also asks for services that are **independently testable**, and
a plain data struct is not one. This molecule gives two of them the operations
that were already implicit in their callers.

**`AutodiffTape`.** `Autodiff.tape()` and `Autodiff.clear()` were **five
identical lines each**, differing only in `recording = true` versus `false`. They
become `begin()` and `discard()`. The third site — the tail of
`Autodiff.backward()` — was **one** line, `recording = false`, and it is now
`stop_recording()`.

That third method is the point of the molecule. `backward` stops recording and
**keeps `grads`**, because the gradients are its result and a caller is about to
read them. `discard` throws them away. At the call sites those two were one line
and five lines of the same shape, so telling them apart meant counting lines and
inferring intent. Now the difference has a name, and the name says which one is
which.

**`SecurityPolicy`** gains `allows()` and `grant()`. The point is not the two
lines saved: it is that a gate now *asks the policy* rather than inspecting its
set. If the answer ever stops being a set lookup — a wildcard, a scope, an
inherited grant — it changes in one place instead of at every call site.

**Seven unit tests**, and they test the services rather than the evaluator, which
is what "independently testable" means. Three of them pin things that were
previously only true by inspection: node ids restart at 1 with each tape, a
repeated grant reports itself as not-new, and `granted` and `lockdown` are
independent — the counter-intuitive pair whose distinction M6.3 wrote onto the
type.

**What M6 did not do, stated plainly.** The *behaviour* of each namespace still
lives in `impl super::Evaluator` in the same files. `eval_autodiff_namespace`
still needs `&mut Evaluator` for `alloc`, `rt_err_kind` and `null_ref`, so the
services own their state and a little of their logic, not their dispatch. Moving
that is a much larger change than M6 has done here, and calling it done would be
the false COMPLETE §12 warns about.

## 9K. M6 MILESTONE AUDIT

Charter: *"Evaluator debe evaluar lenguaje"* — not simultaneously be a filesystem,
a process manager, a socket manager, a GPU manager, a GUI runtime, a media
runtime, a task scheduler, a memory manager and an autodiff runtime.

### Definition of Done, item by item

| Item | Status | Evidence |
|---|---|---|
| Ownership explicit | **met** | 5 new types, each grouping on a stated invariant rather than a name prefix; the invariant is written at the type |
| `Evaluator` reduced | **met** | **48 fields -> 38**, -21%. The 24 language- and execution-state fields are not targets — they are what an evaluator is |
| Services independently testable | **partially met** | `AutodiffTape` and `SecurityPolicy` have operations and 7 unit tests. `ModuleContext`, `TaskContext` and `DispatchCaches` are still data |
| No extraction changes behaviour | **met** | four molecules, full gates green after each, no manifest row moved anywhere |

**Three of four met. M6 is PARTIAL**, and the unmet item is unmet because of
**DEC-M6-001**, not because it was skipped.

### 1. What M6 could and could not reach

M6 reduced the evaluator's *state* and left its *behaviour* where it was. Every
namespace's dispatch is still `impl super::Evaluator`, because each needs `alloc`,
`rt_err_kind` and `null_ref`. The services own their data and, in two cases, the
operations over that data — they do not own their dispatch.

That is a real limit and it is stated rather than dressed up: an evaluator that
holds an `AutodiffTape` instead of five autodiff fields is better structured, but
it is still the thing that differentiates. The charter's phrase — *the evaluator
should evaluate the language* — is not yet true.

### 2. Field regrouping has reached its floor, and here is why

38 fields, and the remaining ones were each considered:

  * **24 are language state and execution state.** Arenas, bindings, scopes, the
    class and enum registries, the call stack, the error slot. Not targets.
  * **`sockets`, `gpu`, `memory`** are three `HandleRegistry`s. Grouping them
    would group by *implementation type* — three things that share a generic — and
    not by responsibility. A socket table and a raw-memory table answer unrelated
    questions. Declined.
  * **`lcg_state` and `tensor_id_counter`** are one `u64` each with no second
    field to be cohesive with. Wrapping either produces a file, not a boundary
    (rule 3). Declined.
  * **`gui`, `spawned`, `media`** are already single fields holding their own
    types.

Recorded so a later reader knows these were weighed and declined, rather than
missed. Further reduction needs DEC-M6-001, because it means moving behaviour.

### 3. Semantic drift

**None.** Four molecules, and every gate green after each: 433 Rust tests, both
Serez runners at 499/0/0 with identical per-category totals, the ecosystem canary
at 8/8, and no row moved in either manifest. The changes are field regroupings and
three call-site collapses that were byte-identical to the methods replacing them.

Two rename slips were caught, both by the compiler and neither by a test: a
multi-line `self\n.ad_grads` in M6.1, and four non-`self` accesses inside
`#[cfg(test)]` code in M6.3. Worth recording together, because they make the same
point from two angles — **a textual rename is a proposal; the type system is what
checks it** — and because a language without that check would have shipped both.

### 4. Duplication introduced, and removed

None introduced. Removed: `Autodiff.tape()` and `Autodiff.clear()` were five
identical lines each, and are now two calls.

### 5. Circular or new dependencies

None. `permissions` and `modules` gained types and no dependencies;
`namespaces_task` and `namespaces_autodiff` already lived under `evaluator`.
`DispatchCaches` is private to `evaluator/mod.rs`, because nothing outside needs
it — the smallest visibility that works, rather than the most convenient.

### 6. Gates at close

| Gate | Result |
|---|---|
| `cargo fmt --check` | **PASS** |
| `cargo check --all-targets` | **PASS**, no warnings |
| `cargo clippy --all-targets` | **PASS**, 0 errors; per-site list **180**, unchanged across all of M6 |
| `cargo test --all-targets` | **PASS**, 433 / 0 failed |
| `run_tests.ps1` / `run_tests.sh` | **PASS**, 499 / 0 / 0 each, identical per category |
| Ecosystem canary | **PASS**, 8 / 8 |

### 7. Commits

`b84393c` M6.1 autodiff · `503d5f6` M6.2 modules · `be7fb96` M6.3 security, task,
caches · `139911d` M6.4 service operations.

---

## MILESTONE STATUS: **PARTIAL**

The evaluator is 21% narrower and every field it still holds has a reason that is
written down. What it does *not* have is services that own their dispatch, and
that is one decision — **DEC-M6-001** — plus, on the recommendation recorded
there, a differential runtime harness before anyone attempts 12,000 lines of
behaviour-preserving change.

---

## 9I. M5 MILESTONE AUDIT

Charter: *"Reglas de tipos coherentes, normativas y consistentes entre
checker/runtime/tooling."*

### Definition of Done, item by item

The plan's DoD: *a type rule means the same thing to the specification, the
checker, the runtime, the LSP and the compiler.*

| Consumer | Status | Evidence |
|---|---|---|
| Specification | **met** | `spec/types.md` is normative, complete for the rules it covers, and was the arbiter in every disagreement M5 found |
| Runtime | **met** | `evaluator::type_matches` implements the spec's table; nothing in M5 changed it |
| Checker | **met** | four divergences from the spec+runtime, all fixed (§9H.2, §9H.3); held by `tests/type_agreement.rs` |
| LSP | **met, structurally** | `lsp/analysis.rs:151` constructs the same `TypeChecker`; there is no second implementation to drift (§9H.4) |
| Compiler | **not applicable** | `src/compiler/` is feature-gated behind `llvm` and wired to no CLI verb. Recorded as an M10 item, not an M5 gap |

**All five consumers that exist are consistent. M5's stated DoD is met.**

### What "COMPLETE" does and does not claim here

It claims: **no consumer disagrees with another about the same program**, and a
gate now exists that fails if one starts to.

It does not claim the type system is good. `spec/types.md`'s "Known gaps" section
lists eight limitations, and M5 fixed none of them — it made every consumer agree
about them. Those eight are design questions, and five are now registered as
**DEC-M5-001** through **DEC-M5-005** with measured exposure and a marked
recommendation. That distinction is the milestone: *coherent* was the charter,
not *complete*.

### 1. What M5 found

The premise was that the rules might be incoherent. They were not — the rules are
fine and `spec/types.md` states them well. **What was incoherent was that the
matching table had two implementations**, 25 arms and 8 lines, and the small one
was not a subset of the large one:

| Divergence | Direction | Fixed |
|---|---|---|
| `void` did not accept `null` | false positive | M5.2 |
| `[int]` rejected `[string]` | false positive | M5.2 |
| `array` rejected `[int]` | false positive | M5.2 |
| `export` hid a declaration from passes 1 and 2 | false negative | M5.3 (§5.29) |
| Two of four checks carried no position | tooling defect | M5.4 |
| `int?` at an `int` parameter | undecided | **DEC-M5-001** |

### 2. Why no existing gate could have caught any of them

The three false positives are the important case, and the reason is structural:
**nothing in the repository pins stderr for a program that succeeds.**
`diagnostic_render.manifest` covers `err_*` and `sec_*` — failing programs. The
e2e fixtures compare stdout. So a spurious `TYPE ERROR` printed over a correct
program was invisible to all 499 conformance tests and all 426 Rust tests
simultaneously.

`tests/type_agreement.rs` is the gate that closes it, and it closes it by
construction rather than by enumeration: it runs each case through the real
checker *and* the real evaluator and compares them, so it does not need to know
what the right answer is — only that the two halves give the same one.

### 3. Semantic drift

**None.** No program's behaviour changed. Every M5 change is confined to stderr,
and only for programs that were being reported wrongly or not at all:

  * M5.2 removes findings from programs that run correctly;
  * M5.3 adds findings to exported declarations that were skipped;
  * M5.4 adds a position to two findings.

Exit codes cannot move — `spec/types.md` makes type findings advisory — and the
measurement confirms nothing observable did: **the corpus emits exactly 3
`SZ3000` findings, in the same 3 files, before M5 and after it**, and no manifest
row moved across the milestone.

### 4. Duplication introduced

None. M5 *reduced* duplication in the only place it could: `types_compatible` is
now the name-level half of `type_matches` and says so. The two remain separate
functions because one reasons about values and the other about type names, and
the arms of `type_matches` that inspect a value have no name-level counterpart —
recorded at the definition site rather than left to be rediscovered.

### 5. Circular or new dependencies

None. `type_checker` still depends on `ast`, `diagnostic`, `render` and `span`.
Note that **DEC-M5-003 option A would add a dependency on `semantic`** — the
checker needs to know which class names exist — which would make it the first
product consumer of the layer M4 built. Recorded because it is an architectural
consequence of a decision, not of a change.

### 6. Gates, at close

| Gate | Result |
|---|---|
| `cargo fmt --check` | **PASS** |
| `cargo check --all-targets` | **PASS**, no warnings |
| `cargo clippy --all-targets` | **PASS**, 0 errors |
| Clippy per-site list (§5.26) | **180 lines** — one *fewer* than the 181 baseline; `manual_strip` in `types_compatible`, removed by M5.2 |
| `cargo test --all-targets` | **PASS**, 426 / 0 failed |
| `run_tests.ps1` / `run_tests.sh` | **PASS**, 499 / 0 / 0 each, identical per category |
| Ecosystem canary | **PASS**, 8 / 8 |
| Corpus `SZ3000` findings | **3**, in the same 3 files as before M5 |

### 7. What M5 hands forward

| To | What |
|---|---|
| The decision owner | DEC-M5-001…005, each with measured exposure: 15 `decimal` parameters and 0 in the ecosystem; 1 unknown type name, in the fixture that documents it; 68 typed fields and 0 in the ecosystem |
| M4 | DEC-M5-003 option A would give `semantic` its first product consumer |
| M7 | DEC-M5-004 and DEC-M5-005 are semantics to freeze, and both are breaking |
| M10 | the compiler's type story is unproven because the compiler is unwired |
| Whoever adds a check | `Expression::span()` does not exist; the array-literal diagnostic points at the literal instead of the element because of it (§9H.4) |

### 8. Commits

`83dfef5` M5.0–M5.2 (audit, net, three false positives) · `b17cedd` M5.3
(`export`, closing §5.29) · `df6a0b8` M5.4 (positions, tooling parity).

---

## MILESTONE STATUS: **COMPLETE**

Every consumer of a type rule that exists today agrees with every other, the
specification is the arbiter, and `tests/type_agreement.rs` fails if that stops
being true. The five open decisions are about what the rules *should be*, which
is a different question from whether the implementation is coherent about them —
and each is registered with evidence rather than absorbed.

## 10. Commits and checkpoints

| Milestone | Commit | Note |
|---|---|---|
| M0 baseline | `d8662c2` | pre-existing HEAD, = tag `v10.0.0`; the frozen reference point |
| M0 checkpoint | `5b34f65` | `docs(maturity): freeze the M0 baseline, and re-measure an audit that had drifted`. Documentation only; no behavior change. |
| M1.0 | `4edad8b` | `give the parser a module of its own, behind a net that can see it` — the snapshot + façade tests, and the file move |
| M1.1 | `da82ffe` | `move the infrastructure the grammar sits on out of the grammar` — cursor, depth, diagnostics |
| M1.2-M1.3 | `7775c2c` | `move type syntax and the file-level directives out of the grammar` |
| M1.4-M1.6 | `61080fc` | `move every declaration form out of the grammar` — functions, variables, classes |
| M1.9-M1.11 | `ca1f24a` | `move control flow and the literal forms out of the grammar` — loops, branches, literals |
| M3.0-M3.1 | `e10bfd4` | `pin what every failing program prints before touching how it is produced` |
| M3.2-M3.3 | `c4657e3` | `one model, and the frontend moves onto it` |
| M3.4-M3.5 | `361c2cb` | `the checker and the runtime move onto the one model`. Amended once, to drop an untracked audit report that `git add -A` had swept into it; `33e6211`'s message still names the pre-amend hash `aeeebf2`. |
| M3.6 | `33e6211` | `four formats become one renderer` |
| M3.7 | `e77aec0` | `nine rejected programs stop failing without saying why` — **behaviour change** |
| M3.8 | `6028fe3` | `decide the ordering question, and change nothing` |
| M4.0 | `53518e7` | `the semantic layer is absent, not misplaced` |
| M4.1 | `6a32ade` | `measure how far the editor's outline is from the parse tree` |
| M4.2 | `795d94f` | `the missing layer, as a leaf with no consumers` |
| M4.3 | `c2775bd` | `validate the new module against the whole corpus` |
| **M3 checkpoint** | `78202b5` | `M3 closes — the audit, and two rows that were out of date` |
| **M1 checkpoint** | the commit that created `src/parser/expressions.rs` — `git log --diff-filter=A -1 --format=%h -- src/parser/expressions.rs` | `the last two grammar areas move out, and M1 closes` — assignment, expressions, and the milestone audit. A commit cannot name its own hash, so this row resolves it. |
| M4.7.1-M4.7.2 | `69f9553` | `a scope model with no consumers, and the number DEC-M4-002 was missing` |
| §5.34 fix | `8e67150` | `three fixtures that asserted nothing now assert, and a guard so it cannot recur` |
| **M4 checkpoint** | `a622e84` | `M4 closes PARTIAL — three of six, and the reason for each of the other three` |
| M5.0-M5.2 | `83dfef5` | `the checker stops reporting three programs the runtime accepts` — **behaviour change**, stderr |
| M5.3 | `b17cedd` | `the checker sees through export, and §5.29 closes` — **behaviour change**, stderr |
| **M5 checkpoint** | `38850d0` | `M5 closes COMPLETE — the rules were fine, the table had two implementations` |
| M6.1 | `b84393c` | `the autodiff tape stops being five fields of the evaluator` |
| M6.2 | `503d5f6` | `the module context stops being three fields of the evaluator` |
| M6.3 | `be7fb96` | `security, task context and dispatch caches stop being seven fields` |
| M6.4 | `139911d` | `two services stop being data and start answering questions` |
| M5.4 | `df6a0b8` | `two checker findings that pointed at nothing now point at the code` |
| **Re-entry checkpoint** | the commit that added §1.5 — `git log -1 --format=%h -S'1.5 Baseline re-verified' -- docs/maturity/ROADMAP_STATE.md` | `re-verify the baseline, and correct a header three milestones out of date`. Documentation only; no behaviour change. Seven gates plus the canary re-measured green at `9ca4d22`; §0 corrected; §5.30 recorded. Same self-naming problem as the M1 row, resolved the same way. |

---

## 11. Semantic contracts decided in this roadmap

None yet. M0 decided no semantics.

The contracts inherited as already-frozen live in `MATURITY_AUDIT.md` §"Confirmed
public contracts that must not change silently" and in `spec/`. The one that most
constrains M1–M2 work:

> A declared type matches **exactly**: no numeric widening at a parameter and no
> subtyping. Both are recorded as inconsistencies in `spec/types.md`; changing
> either is breaking.

---

## 12. Protocol reminders for the next session

- A refactor must not change observable behavior. If you find a bug mid-refactor:
  reproduce it, confirm it predates your change, record it in §5 here, preserve
  the behavior, finish the molecule, then fix it as its own task.
- Never make a test green by editing it, unless you can show the contract it
  asserted was wrong.
- If a gate fails, stop. Do not start another molecule.
- Do not mark a milestone COMPLETE because it looks close enough. Each milestone
  needs implementation, tests, documentation, self-audit and full gates. A
  milestone whose Definition of Done is genuinely unmet is **PARTIAL**, and the
  audit says which part and why. `PARTIAL` is an honest outcome; a false
  `COMPLETE` is not.
- **Autonomy, as of 2026-09-02.** Milestones proceed without per-milestone
  authorization, through M10. Work does not stop at a milestone boundary; it
  stops only for the reasons listed below.
- **A decision is registered, never taken to unblock yourself.** When a choice
  has several defensible answers with different consequences — architecture,
  language design, semantics, compatibility, public behaviour, syntax, security
  policy — give it a `DEC-<milestone>-<nnn>` identifier and write it into §7A
  with the full field set. Then preserve the current behaviour and continue with
  everything that does not depend on it. **Never ship a provisional
  implementation that silently converts a recommendation into a decision.**
- **Do not fake independence.** If a later milestone genuinely needs an open
  decision to be correct, mark exactly that part blocked and continue with the
  rest. Skipping a real dependency to reach a green tick is the one failure this
  protocol exists to prevent.
- **Stop only when:** no independent work remains; a gate is red and fixing it
  needs a product decision; there is serious risk of data corruption, security
  compromise, information loss or destructive behaviour; the repository can no
  longer be held at a green checkpoint; or an architectural dependency makes
  continuing technically invalid. An ordinary architectural decision is **not** a
  reason to stop — register it and continue.
- Update this file at every milestone boundary, and update §0 whenever the
  "next authorized molecule" changes. **Make §0 the last step of the milestone
  audit, not a separate chore** — it drifted three milestones behind its own
  ledger by being treated as one (§5.30). Every row in §0 is stated
  authoritatively elsewhere in this file; the audit already re-reads all of it.
- **Re-verify the baseline on re-entry**, before trusting any figure here. A
  green gate recorded at one commit says nothing about the next. §1.5 is the
  worked example; the clippy *per-site list* (§5.26), not the count, is the row
  that actually detects drift.
