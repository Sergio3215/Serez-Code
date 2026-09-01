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
| **Current milestone** | **M1 — Parser Molecular. IN PROGRESS.** |
| Goals done in M1 | **M1.0** (skeleton + net) and **M1.1** (parser core) — 2 of 17 |
| Last completed milestone | M0 — Baseline Frozen (**COMPLETE**) |
| Next molecule | **M1.2.1** — extract type syntax (`parse_type_string`, `is_type_keyword`, `parse_dict_type_annotation`) into `parser/types.rs` |
| Branch | `improve` |
| Baseline commit | `d8662c2` (= tag `v10.0.0`, on `origin`) |
| Runtime version | 10.0.0 |
| Last state update | 2026-09-01, end of M1.1 |

Milestone ledger:

| Milestone | Status |
|---|---|
| M0 — Baseline Frozen | **COMPLETE** (2026-09-01) |
| M1 — Parser Molecular | **IN PROGRESS** — M1.0 and M1.1 done, 15 goals left |
| M2 — AST + Spans Stable | NOT STARTED |
| M3 — Diagnostics Unified | NOT STARTED |
| M4 — Semantic Layer Established | NOT STARTED |
| M5 — Type System Stable | NOT STARTED |
| M6 — Runtime Molecular | NOT STARTED (partially pre-empted; see §6) |
| M7 — Semantics Frozen | NOT STARTED |
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

---

## 2. Codebase size

63 Rust files, 54,555 lines. Largest:

| File | Lines |
|---|---|
| `src/evaluator/namespaces_gui.rs` | 6,264 |
| `src/parser/mod.rs` | 3,673 (was 3,936 before M1.1) |
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
  → parser::Parser        (4,026 lines, 4 files) ── ParseError { code SZ2xxx, line, column, message }
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

### 3.2 Diagnostics as they actually are

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

§5.1–5.9 are M0; §5.10–5.16 are M1.0; §5.17–5.18 are M1.1.

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

### 5.4 Four of five positional AST fields have no consumer — *architectural debt*, medium (M2 input)

Only 5 of 48 AST types carry `line`/`column`: `EnumDeclaration`, `ClassField`,
`MethodCallExpression`, `CallExpression`, `InfixExpression`. The first four are
marked `#[allow(dead_code)]` and nothing reads them. Only `InfixExpression`'s
position is consumed (`evaluator/expr.rs:1429`).

Two consequences for M2:
- The AST is not merely inconsistent about spans, it is 90% *absent* of them.
- `Token` carries `line`/`column` but **no byte offsets**, and `Lexer` never
  computes one. A byte-offset `Span` therefore requires a lexer change, not just
  an AST change. Plan M2 around that.

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

### 5.17 Nine parser errors never reach the error list — **confirmed bug**, high (found in M1.1.1)

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

### 9.3 What the parser looks like now

| File | Lines | Responsibility |
|---|---|---|
| `parser/mod.rs` | 3,673 | `Parser` state, `new`, `parse_program` + `synchronize`, and **all grammar** |
| `parser/diagnostics.rs` | 135 | `ParseError`, `SZ2000`, reporting, rendering, source labels, the error readers |
| `parser/depth.rs` | 111 | `MAX_PARSE_DEPTH`, `SZ2001`, `DepthGuard`, the charge/release accounting |
| `parser/cursor.rs` | 107 | advancing the stream, precedence at the cursor, name classification |

The infrastructure is out. Goals M1.2–M1.16 are the grammar itself, which is the
3,673 lines that remain.

---

## 10. Commits and checkpoints

| Milestone | Commit | Note |
|---|---|---|
| M0 baseline | `d8662c2` | pre-existing HEAD, = tag `v10.0.0`; the frozen reference point |
| M0 checkpoint | `5b34f65` | `docs(maturity): freeze the M0 baseline, and re-measure an audit that had drifted`. Documentation only; no behavior change. |
| M1.0 checkpoint | the commit that created `tests/parser_snapshot.rs` — `git log --diff-filter=A -1 --format=%h -- tests/parser_snapshot.rs` | `refactor(parser): give the parser a module of its own, behind a net that can see it`. A commit cannot name its own hash, so this row resolves it instead of quoting it. |

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
- Do not start M(n+1) because M(n) looks close enough. Each milestone needs
  implementation, tests, documentation, self-audit and full gates.
- Update this file at every milestone boundary, and update §0 whenever the
  "next authorized molecule" changes.
