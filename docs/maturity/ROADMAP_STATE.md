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
| **Current milestone** | **M3 — Diagnostics Unified. IN PROGRESS.** |
| Goals done in M3 | **M3.0** audit (§9D.0) · **M3.1** the rendering net (§9D.1) |
| Goals done in M2 | **M2.0** audit · **M2.1** measurement + decision · **M2.2** the `Span` type · **M2.3.1** lexer offsets · **M2.3.2** expression extents · **M2.3.3** the remaining sites · **M2.4** declarations · **M2.5** statements · **M2.6a** expressions · **M2.6b** identifiers · **M2.6d** literals · **M2.6e** wrappers · **M2.7** components · **M2.8** audit + Token collapse |
| Last completed milestone | **M2 — AST + Spans Stable** (M0, M1 before it) |
| Next molecule | **M3.2** — introduce the common `Diagnostic` model in its own leaf module |
| Branch | `improve` |
| Baseline commit | `d8662c2` (= tag `v10.0.0`, on `origin`) |
| Runtime version | 10.0.0 |
| Last state update | 2026-09-02, M2 milestone audit |

Milestone ledger:

| Milestone | Status |
|---|---|
| M0 — Baseline Frozen | **COMPLETE** (2026-09-01) |
| M1 — Parser Molecular | **COMPLETE** (2026-09-01) — mod.rs 3,936 -> 422 (-89%), 1 file -> 14 |
| M2 — AST + Spans Stable | **COMPLETE** (2026-09-02) — all 28 `Expression` variants and 39 of 40 structs carry a span |
| M3 — Diagnostics Unified | **IN PROGRESS** |
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

§5.1–5.9 are M0; §5.10–5.16 are M1.0; §5.17–5.18 are M1.1; §5.19 is M1.14; §5.20 is M1.15; §5.21 is M2.0; §5.22–5.24 are M2.3.3; §5.25 is M2.7; §5.26 is M3.5; §5.27 is M3.6. §5.4 was corrected by M2.0 — see the note in it.

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
| **M3.7** | **§5.17** — nine parser errors that reach nobody | **behaviour change, its own commit** |
| **M3.8** | **§5.12** — diagnostic ordering, grouped-by-producer vs by-position | **decision, then possibly a behaviour change** |
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
correction:** the M3.4/M3.5 commit message (`aeeebf2`) says the compiler's type
is the one "which M3.6 takes". That was written ahead of the evidence and is
withdrawn here; M3.6 does not take it.

---

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
| M3.4-M3.5 | `aeeebf2` | `the checker and the runtime move onto the one model` |
| **M1 checkpoint** | the commit that created `src/parser/expressions.rs` — `git log --diff-filter=A -1 --format=%h -- src/parser/expressions.rs` | `the last two grammar areas move out, and M1 closes` — assignment, expressions, and the milestone audit. A commit cannot name its own hash, so this row resolves it. |

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
