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
| **Where to start** | **§0A — the M0→M10 audit.** Milestone statuses, every decision, what blocks what, and the order to answer them in. Written at the end of the autonomous run; the decision-resolution phase updates it as answers land. |
| **Current milestone** | The autonomous run reached M10. Four COMPLETE, seven PARTIAL, none BLOCKED. |
| Goals done in M4 | **M4.0** audit (§9F.0) · **M4.1** the divergence, measured (§9F.2) · **M4.2–M4.3** the symbol layer, corpus-validated (§9F.3) · **M4.7.1–M4.7.2** the scope model and the measurement (§9F.6) · **audit** (§9G) |
| Goals done in M3 | **M3.0** audit · **M3.1** the rendering net · **M3.2–M3.3** the model, and the frontend onto it · **M3.4–M3.5** checker and runtime · **M3.6** one renderer (D5) · **M3.7** the nine silent errors (**behaviour change**) · **M3.8** ordering (D6) |
| Last completed milestone | **M5 — Type System Stable** (§9I). M4 and M6 are **PARTIAL** by decision, not by omission |
| Goals done in M10 | **M10.0** the audit (§9R.0) · **M10.1** the DAG as a gate, finding §5.38 (§9R.1) · **audit** (§9S) · **final M0→M10 audit** (§0A) |
| Goals done in M9 | **M9.0** the audit (§9P.0) · **M9.1** the `OS.spawn` deadlock, verified and fixed (§9P.1) · **M9.2** frontend property testing (§9P.2) · **audit** (§9Q) |
| Goals done in M8 | **M8.0** the audit (§9N.0) · **M8.1** scheme + checker (§9N.1) · **M8.2** `spec/memory.md`, 15 rules proved (§9N.2) · **audit** (§9O) |
| Goals done in M7 | **M7.0** the spec sweep (§9L.0) · **M7.1** six decisions registered · **M7.2** `frozen_semantics.rs` (§9L.2) · **audit** (§9M) |
| Goals done in M6 | **M6.0** the 48-field audit (§9J.0) · **M6.1** autodiff · **M6.2** modules · **M6.3** security, task, caches · **M6.4** service operations · **audit** (§9K) |
| Goals done in M5 | **M5.0** audit (§9H.0) · **M5.1** the agreement net (§9H.1) · **M5.2** three false positives (§9H.2) · **M5.3** `export`, closing §5.29 (§9H.3) · **M5.4** positions and tooling parity (§9H.4) · **audit** (§9I) |
| **Autonomy protocol** | Milestones proceed without per-milestone authorization. A decision with several defensible answers is **registered in §7A, not taken**, and blocks only what genuinely depends on it. Nothing is marked COMPLETE whose Definition of Done is unmet. See §12. |
| **Open decisions** | **17 OPEN, 11 DECIDED.** All in §7A. Decided this cycle: **DEC-M4-002** (a name must resolve lexically), **-004** (the outline comes from the tree), **DEC-M6-001** (a narrow trait), **DEC-M9-001** (one 64 MiB ceiling), **DEC-M10-001** (the canary as a pinned gate plus a daily run), **-002** (a clippy baseline). Earlier: **DEC-M4-001**, **-005**, **DEC-M7-002**, **-006**. **Nothing open blocks queued work.** Four new and open: **DEC-M4-006**, **-007**, **-008**, **-009**. Added 2026-09-03 by the Core-defects pass: **DEC-M9-002** (is an install one transaction), **DEC-M10-003** (does any phase timing block a build). **DEC-M9-003** is now decided. New 2026-09-04: **DEC-M11-001** (is the `unsafe` context lexical or dynamic) |
| Branch | `improve` |
| HEAD | `807a0a5` |
| M0 baseline commit | `d8662c2` (= tag `v10.0.0`, on `origin`) |
| Runtime version | 10.0.0 |
| Last state update | 2026-09-04 — **the Core-defects pass** (§5.43–§5.55). Eleven demonstrated defects, one commit each: the lockfile the ordinary install never wrote, a crash that lost a package, a local registry that could name any host file, a clippy gate debt could move through and that release never ran, a file whose names went unchecked because it imported, top-level bindings the runtime does not create, an outline that discarded its own nesting, timings that measured the sampling order, and a test that asserted only that nothing crashed. Three findings needed a decision and got a registered one instead of a guess |

Milestone ledger:

| Milestone | Status |
|---|---|
| M0 — Baseline Frozen | **COMPLETE** (2026-09-01) |
| M1 — Parser Molecular | **COMPLETE** (2026-09-01) — mod.rs 3,936 -> 422 (-89%), 1 file -> 14 |
| M2 — AST + Spans Stable | **COMPLETE** (2026-09-02) — all 28 `Expression` variants and 39 of 40 structs carry a span |
| M3 — Diagnostics Unified | **COMPLETE** (2026-09-02) — 5 diagnostic types -> 1, 4 rendered formats -> 1 renderer, §5.17 fixed |
| M4 — Semantic Layer Established | **PARTIAL** (2026-09-04) — the phase gained a fifth rule (§5.51) and an import resolver that is built and switched off (§5.50). **Three things the DoD asks for are still missing**: DEC-M4-003 is unanswered so the reserved-name guard still covers 7 of 22 namespaces (M4.6.1); the phase does not reach the editor at all, now across all five rules (DEC-M4-006); and DEC-M4-007 decides whether it may resolve through `import`. Four rules more than it had is not the definition of done |
| M5 — Type System Stable | **COMPLETE** (2026-09-03, §9I) — 4 checker/runtime divergences fixed, §5.29 closed, 5 decisions registered |
| M6 — Runtime Molecular | **PARTIAL** (2026-09-03, §9K) — unchanged by the Core-defects pass, deliberately: **1 of 16** namespaces is across the `ValueSink` boundary and 15 are not. The charter asks for dispatch off the evaluator. No molecule here required crossing one, so none was crossed |
| M7 — Semantics Frozen | **PARTIAL** (2026-09-03, §9M) — everything settled is specified; 6 decisions open and pinned |
| M8 — Conformance Complete | **PARTIAL** (2026-09-03, §9O) — scheme + checker complete and enforced; **1 area of 30** covered. Unchanged: the Core-defects pass added no conformance identifiers, because inflating the percentage without doing an area's work is the one thing §9O says not to do |
| M9 — Robustness & Security Hardened | **PARTIAL** (2026-09-04, §9Q) — package installation is now genuinely what it claimed: the lockfile is written by the install everyone runs, a crash mid-swap is recoverable, and a local registry is held to the archive path's limits (§5.43–§5.46). **Still not done**: the runtime is not property-tested (the DoD's ask, and M9.2 covered the frontend only), `OS.exec`'s output has no ceiling (DEC-M9-003), and concurrent installs have no lock (§5.45) |
| M10 — Stable Language Platform | **PARTIAL** (2026-09-04, §9S) — the release gates are now real rather than nominal: the clippy gate identifies a warning by what it is instead of counting per file, and **release runs it**, which it did not (§5.49). Phase timings measure the code rather than the sampling order (§5.54). **Still not done**: LLVM parity is measured and unverified — 12 of 27 features reach the backend, none has demonstrated parity — and DEC-M10-003 decides whether any timing blocks a build |

---

## 0A. FINAL AUDIT — M0 through M10

Written at the end of the autonomous run, 2026-09-03. **This is the section to
read first.** It says where every milestone stands, what every open decision is,
which of them block what, and the order they should be answered in.

Nothing below resolves a decision. The roadmap's rule is that a choice with
several defensible answers is registered rather than taken, and reaching a green
tick by answering one quietly is the single failure this whole structure exists
to prevent.

### A. Milestone status

| Milestone | Status | Why it is not COMPLETE |
|---|---|---|
| **M0** Baseline Frozen | **COMPLETE** | — re-verified at `9ca4d22` (§1.5) |
| **M1** Parser Molecular | **COMPLETE** | — |
| **M2** AST + Spans Stable | **COMPLETE** | — |
| **M3** Diagnostics Unified | **COMPLETE** | — |
| **M4** Semantic Layer | **PARTIAL** | five rules now, and none of them reaches the editor. M4.6.1 waits on DEC-M4-003; the editor on DEC-M4-006; imports on DEC-M4-007 |
| **M5** Type System Stable | **COMPLETE** | — every consumer agrees; the 5 open decisions are about what the rules *should be*, not whether the implementation is coherent about them |
| **M6** Runtime Molecular | **PARTIAL** | 48 fields → 38; the boundary exists and 1 of 16 dispatches is across it. Not blocked any more — 15 namespaces of work |
| **M7** Semantics Frozen | **PARTIAL** | everything settled is specified; freezing the unsettled *is* deciding it — 6 decisions |
| **M8** Conformance Complete | **PARTIAL** | machinery complete and enforced; **1 area of 30** carries identifiers. Blocked by nothing — this one closes with work |
| **M9** Robustness & Security | **PARTIAL** | package installation is done and now proven rather than asserted; the **runtime and JSON boundaries** are still not property-tested, and `OS.exec` is the fourth unbounded read (DEC-M9-003) |
| **M10** Stable Platform | **PARTIAL** | the release gates are real rather than nominal — release runs the clippy gate now, and it identifies a warning by what it is. The LLVM backend is *measured* and unproven; 15 of 27 features do not reach it |

**Four COMPLETE, seven PARTIAL, none BLOCKED.** Nothing is stopped: every PARTIAL
either waits on an answer or waits on work, and both are named.

### B. Every open decision, by identifier

19 decisions. **What each blocks** is the column that matters — most block
nothing, and treating them as one undifferentiated backlog would hide the four
that actually gate a milestone.

| ID | Question | Blocks | Evidence available | Recommendation |
|---|---|---|---|---|
| ~~**DEC-M4-001**~~ | Where the reserved-name check runs | — | **DECIDED 2026-09-03: A.** See §7A for the rationale and §5.39 for the measurement that settled it | *resolved* |
| **DEC-M4-002** | Is an unresolved free variable a diagnostic | M4.7.3+; the M7 scope entry | **0 of 486 corpus files rely on dynamic resolution**; a resolver found 6 real defects in `serez-ui` on its first run (§5.35) | **Advisory first**, never fatal without it |
| **DEC-M4-003** | Reserved-name guard: 7 names or 22 | M4.6.1; ordered after -001 | breaking with **0 measured victims**, corpus and all 8 packages | **Extend to 22** |
| **DEC-M4-004** | What the editor's outline shows | the LSP's migration onto `semantic` | 95 of 483 files over-report, 0 under-report | **All declarations, correctly nested** |
| **DEC-M5-001** | Nullable value at a non-nullable parameter | nothing | 0 corpus occurrences | **Keep reporting**, add narrowing later |
| **DEC-M5-002** | Numeric widening at a parameter | nothing | 15 `decimal` params in corpus, **0 in the ecosystem** | **Widen `int`→`decimal`**, in a major |
| **DEC-M5-003** | Diagnose an unknown type name | nothing | exactly **1** in corpus — the fixture documenting the behaviour — and 0 in the ecosystem | **Advisory now**; gives `semantic` its first product consumer |
| **DEC-M5-004** | Field type: constraint or default | nothing | 68 typed fields in corpus, **0 in the ecosystem** | **Enforce**, in a major; measure off-type assignment first |
| **DEC-M5-005** | Does a class type accept a subclass | nothing | not measurable; the `is` half changes working programs **silently** | **Accept subclasses**, in a major, with `is` called out separately |
| **DEC-M6-001** | How a service raises and allocates | **the rest of M6** | 16 dispatches, 12,000+ lines | **A narrow trait** — and a differential runtime harness *first* |
| **DEC-M7-001** | `remove` on an empty array | nothing | `remove` in 12 files | **Add `tryRemove`, then make `remove` raise** |
| ~~**DEC-M7-002**~~ | Subclass reaching an inherited private | — | **DECIDED 2026-09-03: A.** Measured first — 27 private declarations in the corpus, 0 in the ecosystem, no file reaching a parent's private from a child | *resolved* |
| **DEC-M7-003** | `match` with no matching arm | nothing | 107 matches, **50** without a catch-all; runtime error affects 0 today, static exhaustiveness affects all 50 | **Warn now, raise in a major**, never a hard static requirement |
| **DEC-M7-004** | Structural container equality | nothing | 0 direct comparisons found, and the search is weak | **Structural for containers**, with DEC-M5-005, in one release |
| **DEC-M7-005** | A pattern that fails to evaluate | nothing | not measurable by construction — it produces no signal | **Propagate the error**, and **before** DEC-M7-003 |
| ~~**DEC-M7-006**~~ | `fetch` under lockdown | — | **DECIDED 2026-09-03: C.** Gated by default, opened by an embedder-set allowlist, redirects validated per hop. DEC-M9-001 deliberately left open | *resolved* |
| **DEC-M9-001** | Ceiling for three unbounded reads | the ceiling | not measured against real usage, and said so | **One fixed fatal ceiling** for all three |
| **DEC-M10-001** | Ecosystem canary in CI | the canary's place in the release gate | 8 packages, 56 tests, 8/8 every run of M0–M10 | **Scheduled daily**, not per-commit |
| **DEC-M10-002** | Clippy as a gate | nothing | per-site list moved **twice in eleven milestones**, both caught by hand | **Gate new sites** against a committed baseline |

### C. What blocks what

**Nothing.** The three that gated a milestone — DEC-M4-002, -004 and DEC-M6-001 —
were answered and implemented on 2026-09-03, so every remaining decision changes
the language or the pipeline and blocks no queued work.

That is a change in kind rather than in degree. For eleven milestones this
section existed to say which work could not start; it now says that what remains
is work rather than answers.

The four decisions added while implementing the others — DEC-M4-006, -007, -008,
-009 — block nothing either. Each is pinned by a test, so none can be drifted
into: the pin fails the day the behaviour moves, which forces the decision to be
taken rather than absorbed.

### D. Recommended order

Ordered by *what unblocks the most* and *what is cheapest to get wrong*, not by
identifier.

**1. DEC-M7-005** — a pattern that fails to evaluate should raise. Cheapest to
implement, hardest current behaviour to defend (documented nowhere), and it must
precede DEC-M7-003: a `match` that raises when nothing matches is far less useful
while a misspelled arm silently is not the thing that matched.

**2. DEC-M4-001** — the semantic phase. Unblocks six molecules, costs two
manifest rows on the guard's own fixture, and deletes §5.32 as a side effect. It
is also the *slot* every later semantic validation needs, so deciding it late
means deciding it under pressure.

**3. DEC-M4-003** — extend the guard to 22, immediately after, so the rule changes
once and in its final home.

**4. DEC-M4-002 (advisory) and DEC-M5-003** together — both make the checker
consume `semantic`, both are stderr-only, and both have measured exposure near
zero. This is where M4's layer finally acquires a product consumer.

**5. DEC-M6-001**, and **build the differential runtime harness before acting on
it**. 12,000 lines of behaviour-preserving change against a suite that asserts
what programs print is the highest-risk work in the roadmap.

**6. DEC-M10-002 and DEC-M10-001** — cheap, independent, and they convert two
disciplines this roadmap performed by hand into gates.

**7. The major-version cluster: DEC-M5-002, -004, -005, DEC-M7-001, -003, -004,
-002.** Every one is breaking; several change working programs *silently*
(subtyping's effect on `is`, structural equality). They belong in one deliberate
major with `spec/compatibility.md` staging, not spread across releases where users
learn the same lesson repeatedly.

**8. DEC-M9-001 and DEC-M7-006** — the two `fetch`-adjacent questions, decided
together since they are one capability.

**9. DEC-M5-001 and DEC-M4-004** — real, and neither blocks nor breaks anything.

### E. What the run produced

| | Before (`9ca4d22`) | After the run | After the findings pass (`649ba49`) |
|---|---|---|---|
| Rust tests | 398 | **447** | **463** |
| Serez conformance | 499 | **501**, identical in both runners | **508**, identical in both runners |
| Ecosystem | 8/8 | **8/8** | **8/8** |
| Clippy sites (§5.26) | 181 | **180** | **180** |
| `Evaluator` fields | 48 | **38** | **38** |
| Spec documents | 30 | **32** | **32** |
| Normative rules | 0 | **15**, all proved | **15**, all proved |
| Registered decisions | 0 | **19** | **22**, 4 decided |
| Dependency cycles | 2 | **2**, both on record | **0** |
| Frontend crate roots | 2 | **2** | **1** |

The findings-pass column counts one thing downward on purpose: the Rust total is
463 rather than 505 because §5.18 stopped compiling 42 tests twice. No test was
lost, and that was verified by comparing the two lists by name rather than by
trusting the count.

**New test files, each closing a gap nothing else could see:** `type_agreement`
(checker vs runtime), `frozen_semantics` (undecided behaviour), `scope_resolution`
(dynamic name resolution), `conformance_map` (spec ↔ test), `frontend_properties`
(generated input), `architecture` (the DAG).

**Behaviour changed four times, each declared:** three checker false positives
removed, `export` no longer hides declarations from the checker, two diagnostics
gained positions, and a child filling the stderr pipe now completes instead of
hanging.

### G. The findings pass — 2026-09-03, `9d91f3c` -> `649ba49`

Thirteen commits closing findings the run had recorded and not fixed. Not a
milestone: a pass over §5 and §6, taking the ones whose decision had been made.

| Roadmap ID | Outcome | Commit |
|---|---|---|
| §5.2 v10.0.0 has no changelog heading | **FIXED** | `a1e23b3` |
| §5.3 `src/test_run.rs` dead weight | **FIXED** | `c05fe71` |
| §5.13 `has_errors()` hides a lexical error | **FIXED** | `e8ba7ee` |
| §5.14 `tests/~tmp_test.sz` runner residue | **FIXED** | `8625195` |
| §5.16 peer session in the working tree | **POLICY SET** | `866825c` |
| §5.18 the frontend compiled twice | **FIXED** | `d0b31b3` |
| §5.6 `run <-> szx` cycle | **FIXED** (with §5.38) | `7ec8fc9` |
| §5.38 `evaluator -> szx -> run -> evaluator` | **FIXED** | `7ec8fc9` |
| §6/M6 `EvalResult` mixes four things | **FIXED** | `5af868c` |
| §5.39 duplicate declaration | **FIXED** | `0385afd`, `d20b64d` |
| §5.39 unknown parent class | **FIXED** (top level; DEC-M4-007 bounds the rest) | `0385afd`, `d20b64d` |
| §6/M5 property schemas | **FIXED** (runtime; the checker's half is its own work) | `89395b3` |
| §6/M7 DEC-M7-002 private access | **DECIDED + FIXED** | `5f78f4e` |
| §6/M7-M9 DEC-M7-006 `fetch` under lockdown | **DECIDED + FIXED** | `649ba49` |
| §5.35 `serez-ui` calls `Int.parse` | **FIXED (external)** — 2 of 6 sites covered, 4 blocked by defects (3) and (4) | `3fb554d` in serez-ui |

**Gates, start and end.**

| | `9d91f3c` | `649ba49` |
|---|---|---|
| `cargo fmt --check` | clean | clean |
| `cargo clippy --all-targets` | 180 sites | **180** |
| `cargo test --all-targets` | 457 | **463** |
| `run_tests.ps1` / `run_tests.sh` | 501 / 0 / 0 | **508 / 0 / 0**, identical |
| ecosystem canary | 8/8 | **8/8** |
| dependency cycles | 2 | **0** |

**Three things this pass got wrong first, all caught by the suite rather than by
care.** They are recorded because the pattern in §F is that a verification which
agrees with its author is worth nothing, and here the verifications disagreed.

  * The unresolvable-parent rule was written without a reach restriction and
    reported `unit_inheritance_errors.sz`, where a lambda legitimately writes
    `class A : B` before declaring `B`. A class is not a variable: it registers
    globally when its declaration runs, so the forward reference recovers. The
    rule now stops at the top level.
  * The private-access fix searched only `StoredClass::methods`, leaving private
    getters and setters keyed to the receiver's class — the exact members
    `spec/classes.md`'s caveat had named. Found by reading the caveat back
    against the diff.
  * §5.13's own worked example turned out to be on the wrong side of the
    boundary it described: `let s = "unterminated;` is not known at construction,
    because two tokens of lookahead do not reach the fourth token.

**Two pins fired and neither was edited to pass.** `frozen_semantics`'s DEC-M7-002
pin and the conformance suite's `eval/lockdown: fetch is NOT gated` both existed
to fail the day their behaviour moved. Each was replaced by an assertion of the
decided rule, with the decision named in the commit — and `frozen_semantics`
gained a `refuses` helper, because a file that could only assert "this program
completes" had no way to state a rule that says "this one must not".

**Where a positive control was added because the negative one went quiet.**
`KNOWN_CYCLES` is empty now, so `the_only_cycles_are_the_ones_on_record` can no
longer distinguish a clean graph from a broken search — which is precisely §5.38's
own failure mode. `the_cycle_finder_finds_cycles` runs the detector over
hand-built graphs: a mutual pair, a three-cycle, a four-cycle at `MAX_CYCLE`, and
a diamond that is not a cycle.

### H. The approved-decisions cycle — 2026-09-03, `6bec7eb` -> `2d4302f`

Eleven answers arrived already decided. §0B is the reconnaissance written before
any of them was implemented; this is what came of it.

| Decision | Outcome | Commit |
|---|---|---|
| *(prerequisite)* nested-`fn` resolution | **FIXED** — a false positive that would have rejected working code | `7b98d63` |
| **DEC-M4-002** free variables | **A, fatal** | `1513b43` |
| **DEC-M4-004** the editor's outline | **B, all declarations correctly nested** | `ec3cd81` |
| **DEC-M9-001** unbounded reads | **A, one 64 MiB fatal ceiling, three sites** | `41e8d5a` |
| generator accumulation | host-set ceiling, default 1,000,000 | `2df302c` |
| **DEC-M6-001** service dispatch | a narrow trait, first service across | `5707779` |
| **DEC-M10-002** clippy | a per-lint/file baseline gate | `1d61f5d` |
| **DEC-M10-001** + daily canary | a pinned blocking gate *and* a daily unpinned run | `1d61f5d` |
| package installation | atomic, lockfile, integrity | `d87a134` |
| LLVM | a measured feature matrix; parity claimed nowhere | `cac34a4` |
| performance | phase baseline and budgets, advisory | `2d4302f` |

**Gates, start and end.**

| | `8b8bbeb` | `2d4302f` |
|---|---|---|
| `cargo fmt --check` | clean | clean |
| clippy | 180 sites, ungated | **180 sites, gated** at `current <= baseline` |
| `cargo test --all-targets` | 463 | **517** |
| both Serez runners | 508 / 0 / 0 | **508 / 0 / 0**, identical per category |
| ecosystem canary | 8/8, local only | **8/8, a blocking CI gate** |
| dependency cycles | 0 | **0** |

#### No milestone became COMPLETE, and that is the finding

Six decisions were answered and every one of them was implemented, which for
three milestones removed the *only* thing the ledger said was blocking them. None
of the three is finished, because a decision was never the whole Definition of
Done:

  * **M4** — DEC-M4-002 and -004 are implemented, and the semantic layer is now
    *adopted* rather than merely built: it rejects programs and the LSP reads it.
    **M4.6.1 is still open**, waiting on DEC-M4-003, and the reserved-name guard
    still covers 7 of 22 namespaces.
  * **M6** — the boundary exists and `Binary` is across it. **1 of 16**
    namespaces. The charter asks for dispatch off the evaluator; 15 remain, and
    they are now work rather than a blocked question.
  * **M10** — every release gate exists, and LLVM is *measured* instead of
    unproven. 12 of 27 features reach the backend and **none has demonstrated
    parity**, so it stays experimental.

**M9** gained resource ceilings and package hardening and is still PARTIAL for
the reason it already was: the runtime is not property-tested. **M8** was not
touched.

#### What the work found that the decisions did not anticipate

Four things, each recorded as its own `DEC-*` rather than decided in passing, and
each pinned by a test so it cannot be drifted into.

**A prerequisite the decision did not mention.** DEC-M4-002 could not be
implemented as written. `semantic::scopes` modelled a nested `fn` as
position-dependent, so two mutually recursive nested functions — legitimate,
working, and in `unit_functions_adv.sz` — were reported as unresolvable. Making
that fatal would have rejected a correct program, which is the one failure mode a
checker must not have. Measured against the release binary first: mutual
recursion works, a forward *call* does not, and a lexical walk cannot tell the
second from the third case. The model now resolves toward "bound", which is the
only acceptable side to be wrong on for a fatal rule.

**A case DEC-M4-004 did not cover.** An AST-derived outline can only show what
parsed, and while a user is typing the file usually does not. The token scan is
kept as a fallback for exactly that, and **DEC-M4-009** asks whether it should
be.

**The letters and the text disagreed twice**, recorded in §0B.B rather than
resolved silently: DEC-M6-001's letter names "pass `&mut Evaluator`" while its
text asks for a narrow trait, and DEC-M10-002's letter names 180 `#[allow]`
attributes while its text forbids a mass cleanup. Both were implemented to the
text.

**Two gates caught mistakes in the work that followed them**, which is the only
way to know a gate works:

  * `tests/architecture.rs` refused
    `evaluator -> package_manager -> package_install -> evaluator`, introduced by
    reusing SHA-256 from the evaluator. The fix was to move the hash **down**
    into a leaf, not to add a line to `KNOWN_CYCLES`.
  * the clippy baseline, committed one commit earlier, failed on a
    `print_literal` in the LLVM matrix before it could be committed.

And one measurement justified a decision that would otherwise have read as
caution: the first run of the phase-timing harness found `semantic.validate`
varying by **2.73×** between the fastest and slowest of seven consecutive runs on
an idle machine. "Warning first, gate later" is not a hedge — it is what that
number requires.

### I. The Core-defects pass — 2026-09-04, `c3a84bf` -> `807a0a5`

Eleven demonstrated Core defects, one commit each, each against a measurement
taken before anything was changed.

| | `c3a84bf` | `807a0a5` |
|---|---|---|
| `cargo fmt --check` | clean | clean |
| clippy | 180 sites, keyed `(lint, file)` | **180 sites, keyed per warning** |
| release runs the clippy gate | **no** | **yes** |
| `cargo test --all-targets` | 517 in 25 suites | **586 in 30 suites** |
| both Serez runners | 508 / 0 / 0 | **508 / 0 / 0**, identical |
| ecosystem canary | 8/8 | **8/8** |
| open decisions | 14 | **17** |

#### Three decisions were registered rather than taken, and that is the result

The instruction was explicit: a fix that requires choosing between legitimate
policies is not a fix. Three did.

  * **DEC-M9-003** — `OS.exec` captures a child's output whole. Every available
    answer changes a public contract, so the routes were enumerated, the reach
    was measured (unreachable from untrusted source), the risk went into
    `spec/limits.md`, and the current behaviour is pinned so the change has to be
    deliberate. Nothing about `OS.exec` moved.
  * **DEC-M4-007** — resolving imports is implementable and implemented, and it
    is **off**. §7A had asked for "an explicit answer for lockdown and a measured
    cost" before A could be taken; both now exist in the entry, including the one
    ecosystem package it rejects and why that package is wrong.
  * **DEC-M10-003** — the phase timings had to be fixed before the gating
    question could even be asked honestly, and then the answer was a table of
    variances, not a gate.

#### Two claims in this repository were weaker than they read

Worth separating from the fixes, because both were things the roadmap already
said were done.

**"Package installation is atomic."** It was atomic against controlled errors.
Against a crash between the two renames it left the project with no package and
the only copy parked under a name nothing ever read. And the lockfile — the
integrity record the whole design turns on — was never written by the install a
fresh clone runs, so the check had nothing to verify against. Both fixed;
`commit()` now says where its window is instead of implying it has none.

**"Clippy is a gate."** It was, on CI, keyed on counts per file — so fixing one
warning and adding a different one of the same lint passed. And
`release.yml` ran bare `cargo clippy`, which fails nothing, so a change could be
blocked on a pull request and shipped by a tag.

#### The gates caught four mistakes in the work that followed them

Which is the only way to know a gate works.

  * The **clippy gate** failed on tabs in a doc comment, then on a
    `type_complexity` pair from the new timing harness. Both fixed at the cause.
  * The **clippy gate caught its own weakness**: adding a field to `RunOpts`
    re-fingerprinted a years-old `derivable_impls` warning, because that lint
    spans a whole `impl`. The fingerprint stops at the span's first line now, and
    a sixth self-test step pins it.
  * The **lib tests** caught a real bug in a new incremental SHA-256 — `update`
    discarded everything buffered when a call did not complete a block. The
    published vectors could not see it, because one-shot `sha256` calls `update`
    exactly once.
  * The **corpus** caught two mistakes in the declaration-order rule: excluding
    `UseKind::Type` on the strength of its name, and an empty `declared_so_far`
    that made every builtin an out-of-order use — 52 files.

#### No milestone became COMPLETE, and none was close

Five milestones were re-read against their Definition of Done. M4 gained a rule
and still does not reach the editor. M6 was not touched, because no molecule here
needed to cross the boundary. M8 gained nothing, deliberately. M9's package work
is genuinely done and its runtime is still not property-tested. M10's gates are
real now and LLVM parity is still unverified.

### F. The pattern worth carrying forward

Three times, in three different milestones, a verification agreed with its author:

  * **§5.34** — three security fixtures asserted nothing and passed, because the
    error they produced satisfied the contract for the wrong reason;
  * **§9N.2** — a pin proved *arity* rather than use-after-free, and would have
    passed unchanged if the protection were removed;
  * **§5.38** — a cycle detector that could only see mutual pairs reported the
    graph clean, while a three-module cycle existed.

The first was someone else's. The second and third were this run's own, committed
*after* finding the first. The discipline that catches all three is the same and
it is not attention: **a negative assertion needs a positive control, and a
checker needs to be run against the failure it claims to detect.** Where that was
done — the perturbed pin in `frozen_semantics`, the deliberately-bad fixture for
the `sec_*` guard, the before/after deadlock measurement — the test is worth
something. Where it was skipped, three tests were worth nothing and looked fine.

The second pattern, from M5 and M7 both: **this project's documentation is ahead
of its decisions.** `spec/` repeatedly says "this is recorded as an inconsistency,
not defended", and is right to. A roadmap that assumes it will find undocumented
chaos will keep rediscovering that the audit was already done, and that what is
missing is someone choosing.

---

## 0B. The approved-decisions cycle — reconnaissance, 2026-09-03

Eleven decisions arrived answered. This section is the reconnaissance done before
any of them was implemented: what the repository actually contains, what the
answers cost against it, and the order the work has to happen in. It is written
before the work rather than after it, so that a plan contradicted by the code is
visible as a contradiction rather than quietly reshaped.

### A. Two documents the instructions assume, which do not exist

`docs/maturity/ROADMAP.md` and `docs/maturity/DECISIONS.md` were named as required
reading. Neither exists, and neither ever has. `docs/maturity/ROADMAP_STATE.md` is
the only file in that directory, and it carries both roles: the progress ledger
(§0–§6, §9) and the decision register (§7A). `MATURITY_AUDIT.md` is the finding
register. Recorded so the next reader does not go looking.

### B. Two decisions where the option letter and the instruction text disagree

The answers arrived as a letter plus a paragraph. For nine of the eleven the two
agree. For two they do not, and the discrepancy is recorded here rather than
resolved silently, because picking either one without saying so would be exactly
the substitution the protocol forbids.

| Decision | Letter given | What that letter is in §7A | What the instruction text describes | Implemented |
|---|---|---|---|---|
| **DEC-M6-001** | A | "Pass `&mut Evaluator` to service methods … the same coupling, written down" | *"interfaz/trait estrecha … los servicios deben recibir únicamente las capacidades que necesitan"* — and explicitly **not** a god object moved | **the text** (= §7A's option **B**) |
| **DEC-M10-002** | B | "baseline them with `#[allow]` at each site" — 180 attributes in source | *"Formalizar Clippy mediante baseline … no ejecutar una limpieza masiva … identificación estable por lint/sitio"* | **the text** (= §7A's option **C**) |

In both cases the paragraph is specific, self-consistent, and describes one
option exactly; the letter names a different one. The paragraph is the decision —
a letter is a label, and the text says what to build. Both are implemented to the
text, and both are flagged here so the owner can correct the record if the letter
was the intent.

`DEC-M10-001` needed reconciling rather than correcting: the answer makes the
canary a **blocking gate** (§7A's option B, a vendored hermetic snapshot), and a
separate answer adds a **daily scheduled run**. §7A recommended C (scheduled
only). The two are complementary, not alternatives, and the instructions say so:
the schedule is additional coverage, not a replacement for the gate. Both are
built; §9T records why the vendored gate and the live daily run answer different
questions.

### C. What the code actually contains, measured

| Decision | Prior infrastructure that is reusable | State |
|---|---|---|
| DEC-M4-002 free variables | `semantic::scopes` — a complete lexical walker, validated over the corpus, **with no product consumer** | analysis done, reporting absent |
| DEC-M4-004 LSP outline | `semantic::top_level` exists; `lsp::analysis` still calls `scan_symbols`, a second lex | both derivations present, the wrong one wired |
| DEC-M6-001 service dispatch | `tools/runtime_diff.sh` — the differential harness the decision requires **already exists** | harness done, extraction absent |
| Package hardening | `MAX_PACKAGE_ARCHIVE_BYTES` (64 MiB) already bounds the download | size bounded; atomicity, lockfile and integrity absent |
| LLVM parity | `hir_lower` reports `SZ7001`/`SZ7002` for every unsupported form — the feature matrix is **derivable from the code** | lowering complete enough to measure; no matrix, no harness |
| Performance budget | 17 benchmarks + two runners exist | benchmarks exist; no baseline, no budget |
| DEC-M10-001 canary | `run_ecosystem.{sh,ps1}` exist and need eight sibling checkouts | runs locally; CI has none |
| Generators | `yield_collector: Option<Vec<..>>` in `evaluator/expr.rs` | unbounded |
| DEC-M9-001 reads | the three paths are named in the decision, not guessed | unbounded |
| DEC-M10-002 clippy | §5.26's per-site comparison, done by hand at each milestone | discipline exists, gate absent |

### D. What DEC-M4-002 costs, measured before implementing

The register's number was 4 uses inside a function across 486 files. Re-run at
this commit over 491 conclusively-analysed files: **30 unaccounted uses in 21
files.** Every one was read, and they are not one population:

| Group | Count | What happens under a fatal rule |
|---|---|---|
| `err_*` / `sec_*` fixtures written to hold an undefined name | 17 | already exit 1; the *phase* and *code* change, not the outcome |
| `_`-prefixed scratch (`_cobol_poc.sz`, `_dbg_ai.sz`) | 5 | not globbed by either runner — no effect |
| `unit_catchable_core.sz` | 3 | **breaks**: asserts undefined names are *catchable* `ReferenceError`s |
| `unit_inheritance_errors.sz` | 4 | **breaks**: asserts a missing parent is a catchable `SZ4001` |
| `unit_functions_adv.sz` | 1 | **a false positive in the resolver, not a program defect** — see below |

**The last row is the one that changes the plan.** `unit_functions_adv.sz` declares
two mutually recursive nested functions:

```serez
fn bool isEven(int n) { if (n == 0) { return true; }  return isOdd(n - 1); }
fn bool isOdd(int n)  { if (n == 0) { return false; } return isEven(n - 1); }
```

This is legitimate, working, tested code. `semantic::scopes` reports `isOdd` as
free because it models a nested `fn` as position-dependent — true for a *read* at
the point of declaration, false for a *call inside a body that runs later*. The
model conflates "used before declared textually" with "evaluated before declared",
and its own header claims every ambiguity resolves toward "bound".

So the resolver has to be corrected **before** its findings can be made fatal.
That is a prerequisite molecule, not part of DEC-M4-002, and it is first in the
order below. Shipping the fatal rule on today's model would reject a correct
program — the one failure mode a checker must not have.

The three fixtures that genuinely break are testing a behaviour the decision
removes. They are rewritten to assert the new contract rather than deleted; the
break is declared in `spec/compatibility.md`.

### E. The order, derived from the code rather than assumed

The instructions give a conceptual order. The real dependencies are narrower:
most of these decisions do not touch each other at all, and the DAG has more
parallelism than the sketch suggests. What genuinely constrains order:

```
  resolver correctness  ──> DEC-M4-002 (fatal free variables)
                       └──> DEC-M4-004 (outline from the AST)

  tools/runtime_diff.sh ──> DEC-M6-001 (narrow trait)      [harness already exists]

  everything else       ──> independent

  performance baseline  ──> last, so it measures the code this cycle ships
```

1. **resolver correctness** — the nested-`fn` false positive. Prerequisite.
2. **DEC-M4-002** — free variables become a fatal semantic error.
3. **DEC-M4-004** — `.sz` outline from the AST; the token scan stays for `.szx`.
4. **DEC-M9-001** — one ceiling, three named call sites.
5. **generators** — a host-set ceiling with a safe default.
6. **DEC-M6-001** — the narrow trait, behind the existing differential harness.
7. **package hardening** — atomic install, lockfile, integrity.
8. **LLVM** — the feature matrix and the differential harness.
9. **DEC-M10-002** — the clippy baseline gate.
10. **DEC-M10-001 + the daily run** — the canary as a gate, and as a schedule.
11. **performance budget** — baseline and budgets, warning-only, measured last.

### F. One constraint the environment puts on step 8

`cargo check --features llvm` **fails on this machine**: `llvm-sys` cannot find an
LLVM 17 installation. The differential harness and the feature matrix can be built
and the matrix can be *derived from `hir_lower`* without linking LLVM, but the
differential itself cannot be executed here. It is written to skip, loudly, when
the feature is off, and §9U records that its parity column is therefore
**unverified on this machine** rather than green.

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
| Serez runner (PowerShell) | `.
un_tests.ps1 -json <f>` | 490 / 0 / 0 | **499 / 0 / 0** |
| Serez runner (bash) | `./run_tests.sh --json <f>` | 490 / 0 / 0 | **499 / 0 / 0** |
| Runner parity | per-category, both reports | identical | **identical** |
| Ecosystem canary | `.
un_ecosystem.ps1 -SkipBuild` | 8 / 8, 56 tests | **8 / 8, 56 tests** |

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

### 5.2 `v10.0.0` is released but has no changelog heading — **FIXED 2026-09-03**, low

> **Status: FIXED.** commit `a1e23b3` · tests: none needed (no source touched);
> `grep` confirmed no runner, script or workflow reads the heading · 2026-09-03.
>
> `## [Unreleased] — maturity hardening` became `## [10.0.0] — 2026-08-31 —
> maturity hardening`, and a new `## [Unreleased]` opened above it. The boundary
> was measured, not remembered: `CHANGELOG.md` at `d8662c2` hashes to `3908649c`
> and so did the working copy, and `git log d8662c2..HEAD -- CHANGELOG.md` was
> empty, so the whole section was the release and it was renamed rather than
> split. The new `[Unreleased]` carries only what a user can observe out of the
> 61 commits since — 45 of them declare `CONTRACT: documentation only` / `no
> observable behaviour` / `tests only`, and the four that survive that filter
> each name their commit.
>
> The description below is the state before the fix.

`Cargo.toml` says `10.0.0`, the tag `v10.0.0` exists on `origin` and points at
HEAD (`d8662c2`, 0 commits after it). `CHANGELOG.md` still files all ~1,800 lines
of work since 9.17.0 under `## [Unreleased] — maturity hardening`. A released
version therefore has no section of its own. Recorded in the audit's Versioning
row. **Not fixed** — closing a changelog section is a release decision, not a
refactor.

### 5.3 `src/test_run.rs` is tracked dead weight — **FIXED 2026-09-03**, low

> **Status: FIXED.** commit `c05fe71` · tests: `cargo check --all-targets` clean,
> `tests/architecture.rs` 3/3 including the `files.len() > 40` guard · 2026-09-03.
>
> Verified dead before deleting: no `mod test_run;` in `lib.rs`, `main.rs` or
> `lsp_main.rs`; no `[[bin]]`, `[[test]]` or path entry in `Cargo.toml`; no
> reference from any runner, workflow, tool or manifest. Three references to its
> *existence* went with it — the tree listing in `DEVELOPMENT.md`, the
> developer-tooling sentence in `spec/ecosystem.md`, and the comment in
> `tests/architecture.rs` explaining why the `src/` walk skips an unreadable
> file. The skip itself stays.
>
> The description below is the state before the fix.

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

### 5.6 The `run` ↔ `szx` module cycle — **FIXED 2026-09-03**, low (M10 input)

> **Status: FIXED**, together with §5.38 and by the same change. commit
> `7ec8fc9` · tests: `tests/architecture.rs` 5/5; `KNOWN_CYCLES` is empty and its
> staleness check named both cycles the moment they were gone · 2026-09-03.
>
> `szx.rs` owned two jobs: translating a `.szx` file and *running* one. Only the
> second reached `run`. `run_szx_file` moved to `run.rs` — which door a file
> extension goes through is entry-point work — and `szx` kept the translation
> half as `translate_szx_beside_source`. See §5.38 for the rest.
>
> The description below is the state before the fix.

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

### 5.13 `has_errors()` is false on a parser whose source is already broken — **FIXED 2026-09-03**, low

> **Status: FIXED.** commit `e8ba7ee` · tests:
> `a_parser_whose_source_is_already_broken_says_so_before_parsing`,
> `a_parser_over_clean_source_still_reports_no_errors`,
> `a_lexical_error_the_parser_has_not_reached_yet_is_not_yet_known` in
> `tests/parser_facade.rs` · 2026-09-03.
>
> `has_errors()` reads the lexical queue as well as the flag. `take_errors()`
> still returns only what has been flushed (§5.11) and the flush still groups
> every SZ2xxx before every SZ1xxx (§5.12, decision D6); only the yes/no question
> moved.
>
> **The entry's own example was on the wrong side of the boundary**, and the
> tests now say where the boundary is. `let s = "unterminated;` is *not* known at
> construction: two tokens of lookahead reach `let` and `s`, and the bad string is
> the fourth token. `0x;` is, because the malformed token is the first one.
> `has_errors()` answers "does the parser know of an error", not "does this source
> contain one" — the second cannot be answered without lexing the whole file
> eagerly.
>
> No observable behaviour for any program: all six product call sites read
> `has_errors()` after `parse_program` has returned, where the flush has already
> set the flag. What changes is the answer an early caller gets.

`Parser::new` pulls two tokens, so a lexical failure on line 1 exists inside the
parser before `parse_program` is called. It sits in a separate queue until the
flush, so `has_errors()` answers `false` until then. No caller checks early.
Pinned by `lexical_diagnostics_become_visible_only_once_parsing_has_run`.

### 5.14 `tests/~tmp_test.sz` is committed runner residue — **FIXED 2026-09-03**, low

> **Status: FIXED.** commit `8625195` · tests: `tests/parser_snapshot.rs` 4/4
> including `the_corpus_the_snapshot_walks_is_the_one_it_claims_to_walk`; both
> Serez runners 501/0/0 · 2026-09-03.
>
> Coverage was checked before deletion and the answer is exact: concatenating
> `framework.sz` with `unit_dict_advanced.sz` and diffing against the file yields
> **one** difference, a blank line at line 34. Nothing to migrate, so no fixture
> was created.
>
> `.gitignore` is the part that stops it recurring: it ignored
> `tests/~unit_temp*.sz`, the exact name the runners write, which is why a scratch
> file under a different name could be committed and twice "restored". It now
> ignores `tests/~*.sz`. `git ls-files '*.sz'` and the snapshot corpus agree.
>
> The description below is the state before the fix.

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

### 5.16 A peer session writes into this working tree — **POLICY SET 2026-09-03**, low

> **Status: ADDRESSED (policy, not code).** commit `866825c` · 2026-09-03.
>
> `DEVELOPMENT.md` §13 now states the flow — `improve -> integration -> main`,
> with the version bump on promotion to `main` — and that **no auxiliary branches
> or worktrees are created for automated sessions**. The reason given is this
> finding: parallel writers into one tree make "what is the state of the
> repository" unanswerable, and every milestone that treats a clean `git status`
> as a precondition depends on being able to answer it.
>
> Deliberately a convention with no enforcement. A hook or wrapper blocking `git
> worktree` would exist on one machine, enforce nothing elsewhere, and hide the
> convention from the people who need to read it.
>
> One historical exception is named there so nobody tidies it away:
> `.claude/worktrees/agent-ae7aff06bbe3f1d73`, branch
> `worktree-agent-ae7aff06bbe3f1d73`, commit `6e62276`. It predates the rule,
> holds work awaiting its own audit, and is not to be deleted, modified, merged,
> cherry-picked or built on. `.claude/` is gitignored, which is exactly why the
> fact of it needed writing down somewhere that is not.
>
> The `audit/` file described below is still present and still uncommitted.

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

### 5.18 The frontend is compiled twice, from the same source — **FIXED 2026-09-03**, medium (M10 input)

> **Status: FIXED.** commit `d0b31b3` · tests:
> `architecture::the_lsp_binary_declares_no_modules_of_its_own` (a structural
> gate) and the new `tests/lsp_shared_frontend.rs`, 5 tests, **a file that could
> not have compiled before** because `lsp` was a private module of a binary crate
> · 2026-09-03.
>
> **The two routes, demonstrated before anything moved.** `sz` reaches the
> frontend through `serez_code::`. `sz-lsp` declared ten modules of its own, and
> Cargo passes `--extern serez_code` to both binaries — so the library was
> available and simply not used.
>
> **The size of it, measured.** `cargo test --bin sz-lsp` ran **73** tests and
> `cargo test --lib` ran **223**; comparing the two lists by name, **42 were the
> same tests** — `lexer::`, `render::`, `span::`, `diagnostic::`, `semantic::` —
> compiled twice, run twice, over two builds of one file. Only 31 were the LSP's
> own, and those were invisible to the library.
>
> **Differences, looked for and reported.** Behaviour: none, same files and same
> flags. Reachability: real — every `pub`/`pub(crate)` in the frontend had to be
> correct under two module roots at once, and `parser/mod.rs` carried three
> `#[allow(unused_imports)]` because of it. All three are gone. Scope: the binary
> compiled the frontend only, so it linked less; that reverses, below.
>
> **The shared API is `serez_code::lsp`** — the whole of `lsp/` promoted from
> binary-private to a public library module. Nothing inside it changed: it already
> referenced only `crate::lexer`, `parser`, `token`, `type_checker` and `semantic`.
>
> **No test was lost, verified by name.** Library 223 → 254 (exactly the 31 LSP
> tests), `--bin sz-lsp` 73 → 0, 0 dropped and 0 renamed. The suite total falls
> because 42 duplicates now run once.
>
> **One observable cost, declared.** `sz-lsp.exe` release grows 887,808 →
> 1,579,008 bytes: it links a library containing the evaluator and host services
> rather than a frontend-only crate. Nothing in it is reachable from
> `lsp::server::run()`; removing it means splitting the library into frontend and
> runtime crates, which is a workspace change and a different decision.
>
> The description below is the state before the fix.

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

### 5.35 — `serez-ui` calls `Int.parse`, and `Int` does not exist — **FIXED (external) 2026-09-03**, medium (found in M4.7.2)

> **Status: FIXED in serez-ui**, commit `3fb554d` in that repository, not in this
> one · tests: `apps/layout_test.sz` goes from 0 assertions to 16; serez-ui 36/36;
> ecosystem canary 8/8 · 2026-09-03.
>
> All six sites are `parseInt` now — the builtin the language has, and the right
> one for the data: serez-ui's own stylesheets write unitless numbers (`gap: 8`),
> which is exactly what it accepts.
>
> **Writing the coverage found three more defects, and together they say
> `src/layout.sz` is dead code.** `computeLayout` has **no caller anywhere in
> serez-ui**, which is why none of it was ever noticed:
>
> 1. the six `Int.parse` calls — fixed;
> 2. `computeLayout` was the module's only entry and its helpers were private, so
>    `export` removed them when the module finished loading and any caller got
>    `SZ4001: Variable not found: measureNode`. Every one of serez-ui's other
>    seventeen modules exports everything it declares — **0 private functions
>    across all of them** — so this was the only module breaking the convention
>    and the only one that did not work. Fixed the same way;
> 3. `getLayoutStyle` returns an object literal, which the runtime refuses with
>    `SZ4002: Object patch '{field: val}' is only valid in an assignment context`.
>    **Not fixed**;
> 4. `measureNode` reads `vnode.type` and `vnode.content`; `VNode` has `tag` and
>    plain-string children. The module is written against a virtual-DOM shape the
>    project no longer has. **Not fixed**.
>
> (3) and (4) are a rewrite against the current VNode rather than a fix, so they
> are reported and left to the ecosystem owner. **Coverage is therefore 2 of the
> 6 parse sites** — `parseSize`'s absolute and percentage paths, with a control
> proving `auto`, `""` and `null` never reach a parse. The other four are named in
> the test file with the defect that blocks each, so the gap is known rather than
> invisible.
>
> **Seven vendored copies still carry `Int.parse`** — `serez-pack/dist/` ×2,
> `Project Serez Code/*/packages/` ×4, `serez-strike/packages/` ×1. They are
> installed copies and build output and refresh from the package rather than being
> patched. §5.35 said the defect shipped three times; it is seven.
>
> The description below is the state before the fix.

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

### 5.38 — the evaluator depends on the entry point that drives it — **FIXED 2026-09-03**, medium (found in M10.1)

> **Status: FIXED**, together with §5.6. commit `7ec8fc9` · tests:
> `tests/architecture.rs` 5/5 with `KNOWN_CYCLES` empty, plus four new
> `modules::` loader tests · 2026-09-03.
>
> **The edges, traced before anything moved:** `evaluator/stmt.rs:1609` called
> `szx::translate_szx_to_string`; `szx.rs:126` called `run::run_file`;
> `run.rs:152` constructs an `Evaluator`; `run.rs:220` dispatched `.szx` to
> `szx::run_szx_file` (§5.6's other half).
>
> **The loader.** `crate::modules::load_source(&canonical)` answers one question —
> what Serez source does this resolved module contain — with `.sz` read from disk
> and `.szx` translated first. Owned by neither `run` nor `Evaluator`:
>
> ```text
> run       -----\
>                  ---> modules ---> szx
> evaluator -----/
> ```
>
> **Deliberately a function, not a `ModuleLoader` object.** There is no state for
> one to hold: resolution is a function of its arguments, and the loaded-set
> already lives in `LoadedModules` inside `ModuleContext`, saved and restored by
> the evaluator as part of the import push/pop. A struct would be a namespace with
> a `new()`, and the next thing it acquired would be the evaluator's arenas.
>
> **Entry-point dispatch** moved to `run.rs`; `szx` keeps
> `translate_szx_beside_source`.
>
> **Preserved and checked, not assumed:** imports, `.sz`, `.szx`, both
> `ImportError` texts (moved to `modules::LoadError` unchanged), cycle termination
> and once-only caching, the translated file's name and position beside the
> source, the "diagnostics refer to the translated form" note, cleanup on both
> paths, every exit code. Live checks against the release binary for the paths no
> suite covers: `sz hello.szx` runs and leaves no `.szx.sz` behind, a failing
> `.szx` prints the note and exits 1, a missing `.szx` reports the same message as
> before.
>
> **The cycle test now has nothing to find**, which is the failure mode §5.38 was
> itself an instance of — so the detector got a positive control,
> `the_cycle_finder_finds_cycles`, over hand-built graphs: a mutual pair, a
> three-cycle, a four-cycle at `MAX_CYCLE`, and a diamond that is not a cycle.
>
> The description below is the state before the fix.

`evaluator -> szx -> run -> evaluator`. A three-module cycle, and §3.1 names all
three edges without naming the cycle they form.

The mechanism is `import`: `evaluator::stmt` re-enters the frontend to load a
module, which means reaching `szx` for `.szx` files, which reaches `run` for
dispatch, which constructs an `Evaluator`. The entry point and the thing it
enters are mutually dependent.

**Found by writing the checker wrong first.** `tests/architecture.rs` was written
to detect *mutual pairs*, which found `run <-> szx` (§5.6, already on record) and
reported the graph otherwise clean. A separate scan for longer cycles found this
one. The pair-only version would have licensed exactly the claim M10 exists to
test — that the dependency graph is understood.

**The lesson generalises past this cycle.** A checker that can only see the shape
you expected is a checker that confirms what you expected. It is the same failure
as §5.34's fixtures passing for the wrong reason and §9N.2's pin proving arity
instead of liveness: three instances now, in three different milestones, of a
verification that agreed with its author. The checker now searches any cycle up
to four modules.

**Not fixed.** Breaking it means deciding where module loading belongs — a
loader that neither `run` nor `evaluator` owns — which is an architectural change
with no forced answer. Recorded, and both cycles are now in `KNOWN_CYCLES` where a
third one would fail the build.

### 5.39 — three more semantic gaps, all silent — **TWO FIXED 2026-09-03**, medium (measured 2026-09-03, presenting DEC-M4-001)

> **Status: duplicate declaration FIXED · unknown parent FIXED · the third gap
> untouched.** commits `0385afd` (preparation) and `d20b64d` · tests: 12 new
> cases in `semantic::validate`, three new conformance fixtures
> (`err_duplicate_class`, `err_duplicate_fn`,
> `err_parent_missing_uninstantiated`) · 2026-09-03.
>
> **Measured before implemented**, because both are breaking and
> `compatibility.md`'s rule is that a breaking change ships in a minor only when
> a sweep finds no affected code. Across **1,070** files — 515 corpus and 555
> ecosystem, all eight packages:
>
> | | count |
> |---|---|
> | duplicate same-kind top-level declarations | **0** |
> | cross-kind collisions (`class X` + `interface X`) | **0** |
> | top-level `class X : Y`, `Y` unresolvable, no `import` in the file | **1** — `tests/err_parent_missing.sz`, the fixture documenting the defect, which already exited 1 |
> | ecosystem `class X : Y` sites | **369**, every one in a file that imports, so every one stays silent |
>
> **A duplicate declaration** is a fatal `SZ8000` naming the first declaration's
> line; the *second* is reported, because it is the edit that broke the program,
> and every collision in a file is reported. Shadowing across scopes is untouched
> (top level only, the reserved-name rule's reach), cross-kind is **DEC-M4-008**,
> cross-file is a different rule with its own reporting, and no overload rule was
> invented.
>
> **An unresolvable parent** is a fatal `SZ8000`. "Not declared in this file" is
> not "does not exist", so it defers to `semantic::scopes`, whose stated bias is
> that every ambiguity resolves toward *treat it as bound*: a file containing
> `import` is never reported against, and builtin construction targets are bound.
> An `import` that in fact fails to supply the parent is **not** caught —
> **DEC-M4-007**, registered and pinned.
>
> **The first version of the parent rule was wrong and the suite caught it.**
> Written without a reach restriction it reported `unit_inheritance_errors.sz`,
> where a `test(…)` lambda writes `class InheritanceCycleA : InheritanceCycleB`
> before declaring `InheritanceCycleB` six lines later and asserts "forward
> reference can recover". `scopes` models position exactly inside a body — right
> for a variable, wrong for a class, which registers globally when its
> declaration executes. The rule stops at the top level, and
> `a_forward_parent_reference_inside_a_body_is_not_reported` pins the case.
>
> **The third gap in this entry is untouched** and stays open.
>
> One `diagnostic_render` row moved: `err_parent_missing.sz`, from a runtime
> `SZ4001` at construction to a fatal `SZ8000` at declaration, exit 1 either way.
> `parser_ast` is purely additive. Specs travel with it: `errors.md` one rule →
> three, `classes.md` states both with the table of what is and is not reported,
> `compatibility.md` records the change with its sweep.
>
> The description below is the state before the fix.

Probed to answer one question: would a semantic phase have more than a single
tenant, or would it be the speculative abstraction rule 9 forbids? Three gaps,
none of which any phase currently reports:

**A duplicate declaration is accepted silently, and the last one wins.**

```serez
class A { public A() { this.x = 1; } }
class A { public A() { this.x = 2; } }
out new A().x;                            // 2. No diagnostic, exit 0.
```

The same holds for `fn int f()` declared twice. Nothing reports the collision at
any phase, so a program that accidentally defines a name twice silently gets the
second definition — including across a file that grew past the point where a
reader can see both.

**An unknown parent class is caught only at instantiation.**

```serez
class Child : Missing { public Child() {} }
out 1;                                    // 1. Runs clean, exit 0.
```

`tests/err_parent_missing.sz` reports `SZ4001` — but only because it calls
`new MissingParentChild()`. A program that *declares* the class and never
constructs it runs to completion. So `--check` cannot tell you that you inherit
from something that does not exist, because the error does not exist until the
object does.

**Why this is recorded rather than fixed.** All three are candidate tenants of the
semantic phase DEC-M4-001 created, and each is its own contract question: what a
duplicate should do (reject? warn? keep last-wins and say so?) is not settled by
deciding *where* validation lives. They are the evidence that the phase has real
work, and they become their own decisions when someone gets to them.

---

### 5.41 — the semantic phase reaches `sz` and not `sz-lsp` — *tooling gap*, medium (found in §5.18, 2026-09-03)

`sz` rejects `class Task { … }` with a fatal `SZ8000` and exit 1. The editor
publishes nothing. Verified against both release binaries over real JSON-RPC, not
only in process:

```
$ sz task.sz
❌ SEMANTIC ERROR [SZ8000] [task.sz 1:1]: 'Task' is a reserved system namespace…
$ sz-lsp   (didOpen with the same source)
"diagnostics": []
```

`lsp::analysis::analyze` runs the lexer, the parser and the type checker. It does
not run `semantic::validate`, because `9d91f3c` wired the phase into
`run::run_source_detailed` and nothing else. So the two rules §5.39 just added —
a duplicate declaration and an unresolvable parent — are also invisible in an
editor, and the gap widens with every rule the phase acquires.

**Pre-existing, and deliberately not fixed in §5.18**, whose contract was that
consolidating the two builds changes no observable behaviour. Making the editor
start reporting a fatal phase is a behaviour change with more than one defensible
shape, so it is **DEC-M4-006**. Pinned by
`the_semantic_phase_does_not_yet_reach_the_editor` in
`tests/lsp_shared_frontend.rs`, with a positive control proving the rule really
does fire in the same process through the same library — so the pin cannot pass
because nothing was rejected.

### 5.42 — seven vendored copies of `serez-ui/src/layout.sz` carry the fixed defect — *ecosystem hygiene*, low (found in §5.35, 2026-09-03)

§5.35 said the `Int.parse` defect "ships three times in the measured tree". It is
seven, and they are three different kinds of copy:

| Location | Count | What it is |
|---|---|---|
| `serez-pack/dist/…/serez-ui/src/layout.sz` | 2 | build output |
| `Project Serez Code/*/packages/serez-ui/src/layout.sz` | 4 | installed package copies |
| `serez-strike/packages/serez-ui/src/layout.sz` | 1 | installed package copy |

Not patched, deliberately: they refresh from the package rather than being edited,
and editing a `dist/` artefact would make the next build silently undo it.
Recorded so that "fixed in serez-ui" is not read as "gone from the tree". The
copies matter only when something actually calls `computeLayout`, and §5.35's
finding (2)–(4) is that nothing does.

---

### 5.43 — the lockfile was never written by the install everyone runs — **FIXED 2026-09-03**, high (found in the Core-defects pass, 2026-09-03)

`sz install <pkg>` wrote `serez.lock`. A bare `sz install` did not — and a bare
`sz install` is what a fresh clone runs, what CI runs, and what every project
whose dependencies are already in `serez.json` runs. The integrity check was
built, tested and unreachable through the ordinary path.

**Measured before touching anything**, in a project whose one dependency came
from the manifest:

```text
$ sz install
✅ Installed test-pkg@1.0.0 → ./packages/test-pkg
$ ls serez.lock
ls: cannot access 'serez.lock': No such file or directory
```

**The cause, not the symptom.** `install_package` took a `record: bool` that
gated two different records at once: the dependency line in `serez.json` and the
integrity line in `serez.lock`. `install_all` reads its dependencies *from* the
manifest, so it correctly did not want to write them back — and passing
`record: false` silently took the lockfile with it. One flag, two questions, and
the answer to one of them was wrong for the other.

The fix is to stop conflating them. `ManifestPolicy::{Record, Keep}` names the
manifest question only; the lockfile is written on every successful local
install, because it records *what was installed* rather than what was asked for.

**Why this was safe to fix while DEC-M9-002 is open.** Writing the lockfile is
correct under both of that decision's alternatives — under a single transaction
it is part of it, and under an authoritative package store it is the derived
record being kept in step. What DEC-M9-002 still decides is the *failure* path,
and that is untouched here.

**Pinned by** `tests/package_lockfile.rs`, five tests over the real binary. Three
of them fail against the old behaviour and two do not, which is the control that
matters: `install_all_leaves_the_manifest_alone` and
`installing_a_named_package_records_it_in_both` describe behaviour the fix must
*not* change, and they passed before and after.

The negative control is the load-bearing one. A lockfile that is written but
never consulted would satisfy a test that only stats the path, so
`a_recorded_digest_refuses_a_changed_package` tampers with the registry after a
successful install and requires the second install to exit non-zero **and** to
leave the already-correct copy undamaged. Measured:

```text
❌ ERROR: integrity check failed for 'test-pkg':
  expected sha256-ef1b614f…
  got      sha256-f1e28ac6…
$ cat packages/test-pkg/index.sz
out "v1";
```

---

### 5.44 — a crash between the two renames lost the package, and the surviving copy was invisible — **FIXED 2026-09-03**, high (found in the Core-defects pass, 2026-09-03)

`package_install` describes itself as atomic. It is — **against controlled
errors**. Against a crash it has a window, and the module said nothing about it.

`Transaction::commit` swaps in three renames, because `fs::rename` will not
overwrite a directory on Windows:

```text
1. destination      -> <name>.replaced-<pid>
2. staging          -> destination
3. remove <name>.replaced-<pid>
```

Between 1 and 2 the destination **does not exist**. Either rename failing is
handled: the old version is renamed back before the error returns. A panic
unwinds through `Drop` and the staging directory goes. A *crash* runs neither.

**Measured** by reconstructing each window exactly and running the next install:

| Window | State a crash leaves | What the next install did |
|---|---|---|
| before rename 1 | destination = old, staging orphan | installed; **staging orphan kept** |
| **between 1 and 2** | **destination ABSENT**, old parked | re-installed from the registry; **parked copy kept** |
| between 2 and 3 | destination = new, old parked | re-installed; **parked copy kept** |

The middle row looks repaired, and that is the trap: it was repaired *because the
registry was still reachable and still had that version*. An upgrade that crashes
while the source is offline, or against a version since withdrawn, lost a working
install permanently — with an intact copy sitting one directory away under a name
that nothing ever read again. Every crashed install also left that copy behind
for good; they accumulate.

**The fix is recovery, not a stronger claim.** The three-rename swap cannot be
made atomic on Windows, so nothing here is called crash-safe. What changed is
that the state a crash leaves is now *recognisable*, and `Transaction::begin`
recognises it before staging anything: if the destination is missing and a parked
copy exists, the parked copy is restored; whatever is still parked afterwards is
superseded and removed.

Recovery runs **before** the install rather than after it, which is the load-
bearing ordering: if the install that follows then fails for any reason, the
project still has the package it had.
`an_install_that_fails_after_a_crash_still_leaves_the_old_version` is that test.

**A second defect found on the way.** The parked name was built with
`Path::with_extension`, which *replaces* an extension rather than appending one.
A package named `my.pkg` parked as `my.replaced-<pid>` — a name whose prefix no
longer matched the package, so it could be neither restored nor swept. Built with
`with_file_name` now, and `a_dotted_package_name_parks_where_recovery_looks`
pins the two halves together.

**Pinned by** nine tests in `src/package_install.rs`. Five fail against a
no-op `recover`; the other four are controls that must pass either way —
the window reproduction itself, a clean tree, a longer-named neighbour whose
parked copy must be neither installed nor deleted, and
`dropping_a_transaction_is_not_a_crash_guarantee`, which uses `mem::forget` to
show that a destructor is cleanup and not a crash guarantee.

### 5.45 — two installs of one package race, with no lock anywhere — *open*, medium (found in §5.44, 2026-09-03)

Nothing in the install path takes a lock. Two `sz install` processes working on
the same package both move the same destination aside, and the second one's
"previous version" is the first one's parked copy. The outcome depends on
interleaving, and one of them can lose its package.

This predates §5.44 and is not made worse by it — recovery reads and writes the
same directories the racing installs already fight over. It is *bounded* by it:
the state a lost race leaves is now the same recognisable state a crash leaves,
so the next install repairs it.

Not fixed here, deliberately. A cross-process lock is a design question with real
alternatives — a lockfile in the project, a lock per package, an advisory
directory lock — and it interacts with DEC-M9-002's answer about what a
transaction covers. Recorded so that "installs are atomic" is not read as
"concurrent installs are safe". No measured occurrence: no test, tool or
documented workflow runs two installs at once.

---

### 5.46 — a local-registry package could name any file on the host and have it installed — **FIXED 2026-09-03**, high (found in the Core-defects pass, 2026-09-03)

A **remote** install validates every archive path, refuses more than 10,000
entries, stops at 256 MiB extracted, and refuses to traverse a symbolic link. A
**local-registry** install called `copy_dir_recursive`, which did none of that —
and `Path::is_dir` follows links, so a link was walked through rather than seen.

**Measured** against the release binary, with a registry holding a package that
links out of itself:

| The package contained | What happened |
|---|---|
| `leak.txt -> ../../../outside/id_rsa` | exit 0, and `packages/pkg/leak.txt` held `PRIVATE KEY MATERIAL` |
| `out -> ../../../outside` (directory symlink) | exit 0, the whole outside subtree copied in |
| `junc -> …\outside` (junction) | exit 0, the same — **and a junction needs no privilege on Windows** |
| a directory link back to an ancestor | `Access is denied. (os error 5)` |
| 20,000 files | exit 0, 20,001 entries installed |

The first three are a read primitive: a registry entry names a path on the host
and its contents land inside the project, silently. The junction row is the one
that matters most in practice — a symbolic link on Windows needs Developer Mode
or elevation, and a junction needs neither.

The fourth did not loop forever only because the OS gave up at some depth. That
is the platform refusing, not the code, and it produced a message that said
nothing about what the package had done. The fifth is the limits simply not
existing on this path.

**The fix is one policy rather than two.** `copy_registry_tree` walks with
`symlink_metadata`, which does not follow, and refuses any reparse point;
validates every path with `validate_package_relative_path`, the archive path's
own validator; and charges entries and bytes against
`MAX_PACKAGE_ARCHIVE_ENTRIES` and `MAX_PACKAGE_EXTRACTED_BYTES`, the archive
path's own ceilings. Nothing new was invented — the local door now asks what the
remote door already asked.

Links are **refused**, not resolved. Canonicalising each target and checking it
stays under the registry root is racy — the target can change between the check
and the copy — and it would still admit something the archive path would never
have accepted. `create_package_directories` already refuses symlink traversal
remotely, so this makes the two consistent.

**Two related fixes came with it.** `package_install::collect` walked with
`is_dir`, which follows, so a digest could describe a tree the package does not
contain and differ between two machines whose link targets differ; it now refuses
a link outright. And `tree_digest` concatenated every file in a package into one
`Vec<u8>` before hashing — at the 256 MiB ceiling that is that much resident, and
`sha256` copied its input, so twice that. It now streams in 64 KiB chunks through
an incremental `hash::Sha256`.

**The digest is unchanged**, which is the compatibility requirement: every
existing `serez.lock` records digests from the old path.
`the_digest_is_the_one_existing_lockfiles_recorded` pins the exact value
captured from the release binary before the change.

**Pinned by** `tests/registry_containment.rs`, 8 tests over the real binary, and
5 new unit tests. Five of the eight fail against the old copy; the two controls —
an ordinary nested package installing as itself, and the entry ceiling *accepting*
10,000 — pass either way, which is what stops "refuse everything" from passing
the suite. The ceilings are tested at the boundary, 10,000 and 10,001, not at 10×.

### 5.47 — the incremental hasher discarded every update that did not fill a block — *caught by tests, never shipped*, 2026-09-03

Recorded because of how it was found rather than because it reached anyone.

`Sha256::update` filled a partial block and then fell through to code that
reset `pending_len` from the remainder of an already-consumed slice, throwing
away everything buffered before it. One-shot `sha256` calls `update` exactly
once, so **`tests/unit_crypto.sz`'s published vectors could not see it** — all
six passed. `tree_digest` calls it four times per file, and its own two tests
failed on the next run:

```text
assertion `left != right` failed: content ignored
  left:  sha256-807effff00d801ad698fbb1ae55e742e3c7e0febf762c9369f8241b454201cff
  right: sha256-807effff00d801ad698fbb1ae55e742e3c7e0febf762c9369f8241b454201cff
```

Two files with different contents hashing identically — the integrity check
silently accepting anything.

The lesson is about coverage shape, not about the bug: a vector suite tests one
call pattern, and the API had grown a second. `a_split_stream_hashes_like_a_whole_one`
now feeds the same bytes at eight different split sizes around the 64-byte block
and requires one digest, which is the property vectors cannot express.

---

### 5.48 — the fourth unbounded read, and the only one DEC-M9-001 did not cover — *open under DEC-M9-003*, high (found in the Core-defects pass, 2026-09-03)

`OS.exec` uses `Command::output()`, which reads both of a child's streams to EOF
with no ceiling. The size is the child's choice; the command is the author's.

**Measured**, peak working set of the release `sz` against a child emitting a
fixed number of bytes on stdout:

| Child output | `OS.exec` | `OS.spawn` stderr, bounded since DEC-M9-001 |
|---|---|---|
| a few bytes | 9.3 MiB | — |
| 16 MiB | 56.3 MiB | — |
| **200 MiB** | **1,009.6 MiB**, exit 0, 6.4 s | **9.4 MiB** |

About 5× the child's output, resident, and it *succeeds* —
`r.stdout.length()` returned 209,715,200. The multiplier is the raw `Vec`, plus
`from_utf8_lossy(..).to_string()`'s copy, plus the `ObjectData::Str`, on top of
`read_to_end`'s doubling.

**Bounded in reach, measured rather than assumed.** `OS.exec` needs the `OS`
permission *and* an `unsafe` block, and under lockdown `use permissions` is
refused outright (`SZ6004`). A locked-down `Task` worker inherits its parent's
grants, but a locked-down parent could not have granted `OS`; only a host calling
`set_permissions` on a locked-down evaluator produces that pairing. So this is a
resource risk to a program its author ran deliberately, not the untrusted-input
class DEC-M9-001 closed.

**Not fixed here, deliberately.** Every fix changes `OS.exec`'s public contract,
and which change is right is **DEC-M9-003** — a fatal ceiling, bounded capture
with a truncation flag, or a host-set default. What was done instead is
everything that does not require the answer: the routes were enumerated (below),
the reach was measured, the risk is written into `spec/limits.md` under *what is
not limited*, and the current contract is pinned by
`os_exec_output_is_currently_unbounded`, so whoever implements the decision has
to change that test on purpose.

**Every route that captures a child's output**, since the finding asked for all
of them:

| Route | stdout | stderr | Bounded |
|---|---|---|---|
| `OS.exec` | captured whole | captured whole | **no** — this finding |
| `OS.spawn` | `Stdio::null()` | `read_bounded`, drained in a thread | yes |
| `OS.kill` (`taskkill`/`kill`) | `.output()` | `.output()` | no, but the child is a fixed system utility whose output is a line |
| `szx` translate, two sites in `src/szx.rs` | `Stdio::null()` | `.output()` piped | **no** — the child is `sz` running a translator, so the output is a Serez program's stderr |

The `szx` pair is recorded rather than fixed: the child is this same binary
running a known translator, the parent is the CLI rather than a Serez program,
and changing it would pre-empt whatever DEC-M9-003 decides about the shape of a
bounded two-pipe read. It is listed so "every route" means every route.

**The infrastructure any answer needs, and why it is not written yet.**
`read_bounded` and `OVER_THE_READ_CEILING` exist and are the right primitive for
a single stream. `OS.exec` needs a *two-pipe* bounded read: `Command::output()`
drains both concurrently, and reading one to EOF before the other deadlocks when
the second pipe fills — the bug already fixed once for `OS.spawn`. Writing that
drainer is the whole of implementing option A or C, so it waits for the decision
rather than being staged as unused code.

---

### 5.49 — the clippy gate let debt move, and release did not run it at all — **FIXED 2026-09-03**, high (found in the Core-defects pass, 2026-09-03)

Two defects in the gate DEC-M10-002 built, one in what it measured and one in
where it ran.

**The identity hole.** The baseline keyed on `(lint, file)` and a **count**. That
is strictly better than a total and it still let debt move: fix one warning of a
lint and introduce a different one of the same lint in the same file, and the
count does not change. Measured on this tree with `clippy::needless_return` in
`src/evaluator/ops.rs`, which has 59 sites:

| Step | Old gate | Required |
|---|---|---|
| 1. baseline, untouched | PASS | PASS |
| 2. one new warning added | FAIL, `59 -> 60` | FAIL |
| **3. one fixed AND a different one added** | **PASS** | **FAIL** |
| 4. reverted | PASS | PASS |

Step 3 is a genuinely new `needless_return` entering the tree while the gate
prints `180 sites in 61 pairs`, unchanged.

**The fix is to identify a warning by what it is.** The key is now the lint, the
file, and a 12-character fingerprint over the **offending source text** — the
highlighted span, not the whole line — together with the normalised message. A
new warning has a fingerprint the baseline does not contain, whatever was fixed
beside it. Two identical sites share a fingerprint and are counted, so a third
copy still fails.

Line numbers stay out of the key, for the reason they were left out originally:
a baseline that needed refreshing after every unrelated edit would be ignored
within a week. Verified rather than assumed — 40 lines inserted at the top of
`ops.rs`, moving every warning in the file:

```text
Clippy debt within baseline: 180 sites, 137 distinct.
exit=0
```

Re-run against the new gate, the same four steps give **PASS / FAIL / FAIL /
PASS**, and the failure names the text: `return n + 1`.

**The migration hid nothing.** The baseline was regenerated in the new format and
the per-`(lint, file)` totals compared against the old one line by line: 180
sites both ways, every pair identical, now resolved into 137 distinct warnings.

**Release ran a clippy that failed nothing.** `.github/workflows/release.yml`
ran bare `cargo clippy --all-targets`, and clippy exits 0 on warnings. CI has
gated on the baseline since DEC-M10-002 and release did not — the worse way
round, since a change could be blocked on a pull request and then shipped by a
tag. Both now run `tools/clippy_baseline.py --check`.

**The gate tests itself.** `--self-test` runs the four steps against synthetic
clippy JSON in seconds instead of minutes, plus a fifth: warnings that merely
*moved* must pass, or the gate would be noise and would be switched off. It runs
on every push. The real end-to-end control is slow and was run by hand, which is
what the table above records — CI checks the logic, and the logic is where the
hole was.

---

### 5.50 — one `import` switched off name checking for a whole file — *implemented, off, under DEC-M4-007*, high (found in the Core-defects pass, 2026-09-03)

`check_names` returns early whenever `ScopeReport::has_imports`, so a file with
any `import` had **no** name checked. Measured, the same undefined name in two
files:

```text
$ sz no_import.sz
❌ SEMANTIC ERROR [SZ8000] [no_import.sz 1:5]: 'totallyUndefined' is not declared
   in this scope or any enclosing one

$ sz with_import.sz
4
❌ ERROR [SZ4001]: Variable not found: totallyUndefined
```

Rejected before execution in one; in the other, only after `out helper(2)` had
already printed `4`. The escape is not a heuristic that fires too rarely — it is
the whole check turning off for the file.

**This is resolvable, not a guess.** Serez has no named imports, no aliases and
no namespaces: `import "path"` executes the module in the same evaluator and its
top-level declarations land in the importer's global scope. `eval_import`'s rule
is exact, and `semantic::imports` mirrors it — exports win where a module has
any, everything is visible where it has none, and what a module's *own* imports
brought in survives either way, because `eval_import` only removes names the
module itself declared. Measured against the runtime:

```text
$ sz vis.sz
1
❌ ERROR [SZ4001]: Variable not found: notExported
```

So a name is now **local**, **imported**, or **unresolved**, and only the third
excuses it — per file, for a stated reason, rather than for every file
containing the word `import`. Five kinds of unreadable module (URL, missing,
unreadable, unparseable, `.szx`) report *less* confidence and never a
diagnostic: a module that does not resolve is already a runtime
`ModuleNotFound`, and a second diagnostic for it would be a new rule.

**It is switched off, because it is DEC-M4-007.** That entry has been open since
this cycle began, its option A is exactly this, and §7A recommended B "until an
explicit answer for lockdown and a measured cost". Both now exist and are
recorded there. `RunOpts::resolve_imports` is `false` at every call site;
`sz file.sz` behaves exactly as before.

**Found while building it, and worth keeping separately.** The first version
walked only top-level statements. That cost three corpus failures and one
ecosystem failure, because `import` is an ordinary statement and
`tests/unit_export.sz` writes one **inside a lambda**:
`test("...", () => { import "lib/greet_noexport"; ... })`. It still lands in the
global scope when it runs. The resolver now takes its list from
`ScopeReport::import_specs`, filled by the walker that already visits every
statement and every expression — one rule, one traversal, no second walker to
drift.

**Also found:** an interpolated expression's diagnostic carries the *statement's*
span rather than the expression's, so `let s = "{ name: inner }";` reports at
column 1. Cosmetic, pre-existing, and not touched here.

---

### 5.51 — the phase pre-seeded top-level bindings the runtime does not create — **FIXED 2026-09-03**, medium (found in the Core-defects pass, 2026-09-03)

`seed_globals` binds every top-level declaration into frame 0 before the walk
starts, so a use before its declaration was invisible at top level and reported
correctly everywhere else. The phase disagreed with itself depending on nesting,
and with the runtime at top level.

**Measured first**, against the release binary. Serez hoists nothing:

| Written | Runtime | Phase, before |
|---|---|---|
| `out x; let x = 1;` | `SZ4001 Variable not found: x` | silent |
| `out f(); fn int f() {…}` | `SZ4001 Variable not found: f` | silent |
| `let c = new C(); class C {…}` | `SZ4001 Unknown class 'C'` | silent |
| `out E.A; enum E { A, B }` | `SZ4001 Variable not found: E` | silent |
| `out a; let [a, b] = [1, 2];` | `SZ4001 Variable not found: a` | silent |
| `out n; native fn int n();` | `SZ4001 Variable not found: n` | silent |
| `{ out z; let z = 1; }` | `SZ4001` | **`SZ8000`** — correct |

**No new hoisting rule was invented**, which the finding was explicit about.
Frame 0 still holds every top-level declaration from the start, because five
forward references legitimately work and none of them is now reported:

```text
class Child : Parent {…}  class Parent {…}     parents resolve at instantiation
fn a() { return b(); }    fn b() {…}  a();     mutual recursion
let f = () => later;      let later = 1; f();  the lambda runs afterwards
fn g() { return later; }  let later = 7; g();  same
fn h(Later p) {}          class Later {…}      an annotation evaluates nothing
```

So the rule is not "declare before use". It is: **a use evaluated when its own
statement runs needs the declaration to have run** — which is exactly
`frames.len() == 1`, because every deferred context (a function body, a block, a
lambda) goes through `scoped` and is deeper.

**Two things the first attempt got wrong**, both caught by the suites rather
than by reading:

  * `UseKind::Type` sounds like an annotation and was excluded on that reading.
    It means a **construction site**, `new Name(...)`, evaluated where it stands
    — and excluding it missed `let c = new C();` above `class C`, which the
    runtime rejects. Annotations are not walked at all, so they never needed an
    exception.
  * `declared_so_far` started empty, so `out abs(5);` on line 1 read as a use
    before a declaration. **52 corpus files failed**, every one of them on a
    builtin. Builtins, namespaces and builtin classes exist before the first
    statement runs and are seeded from the root frame.

**Reported as its own finding, not folded into the free-name rule.** "Declared
nowhere" and "not declared yet" are different facts and want different advice,
and `a_name_declared_nowhere_keeps_its_own_message` pins that a name in one
category never gets the other's wording.

**Pinned by** `tests/declaration_order.rs`, 18 tests split evenly: 9 that must be
reported and 9 that must not. Against a disabled rule exactly the first 9 fail
and exactly the second 9 pass, which is the control that says the rule is doing
what its name claims rather than rejecting forward references in general.
`spec/scopes.md`'s "No hoisting" section is rewritten from one sentence about
functions to the measured rule for every form, with the five legitimate cases.

---

### 5.52 — a `stopGrad` test that asserted only that nothing crashed — **FIXED 2026-09-03**, low (found in the Core-defects pass, 2026-09-03)

`tests/ai_phase3_ops.sz` built a detached tensor, ran a backward pass, and
finished with:

```serez
    // Just verify it doesn't crash
    assert(true, "stopGrad should not crash")
```

The comment above it explained what *should* happen and nothing checked it. The
same assertion passes against a `stopGrad` that returns its argument untouched,
which is the one behaviour worth testing for.

**Semantics unchanged.** This is a test-quality fix: `stopGrad` is not touched.

**What replaced it**, four tests where there was one, measured against the
release binary first:

  * **the gradient is there when it should be** — `Autodiff.gradient(w).norm()`
    is `2.7386` for a live tensor. The positive control, because every other
    assertion is about a gradient being *absent* and all of them would pass if
    gradients never flowed.
  * **the detached source is off the tape** — `Autodiff.gradient(w)` *throws*
    `SZ4000, tensor was not recorded during tape`. Not a small gradient: none.
  * **the quantitative control** — the same tensor in three graphs: one live
    branch `2.7386`, a live branch plus a **detached** branch `2.7386`
    (unchanged), two live branches `5.4772` (exactly double). A `stopGrad` that
    zeroed the whole tape would satisfy the first two tests and fail this one.
  * **values survive detaching** — without it, returning a tensor of zeros would
    pass everything above.

**The control that says it works.** With `stopGrad` reduced to `return t_ref`,
exactly the two detachment tests fail and the other two pass — correctly, since
a passthrough does preserve values and does let gradients flow. `ai_phase3_ops`
goes from 19 assertions passing to 22.

---

### 5.53 — the `.szx` outline threw away nesting the scanner had already computed — **PARTLY FIXED 2026-09-03**, medium (found in the Core-defects pass, 2026-09-03)

DEC-M4-004 moved the `.sz` outline onto `semantic::declarations`. `.szx` stayed
on `scan_symbols`, a token walk, because this frontend does not parse JSX —
`modules::load_source` translates it by running the serez-ui translator as a
subprocess, which an editor cannot do per keystroke.

**There was no `.szx` corpus at all**, so what the scan produced had never been
measured. Four fixtures now exist in `tests/szx/`, shaped like the ecosystem's
real files (`serez-ui/apps/counter.szx`, `serez-strike/app.szx`, the
`proyecto03` demos): a component class, four levels of nesting, Serez
expressions inside JSX braces, and one of each top-level declaration form.

**Measured against them, before:**

```text
  depth=0 Function outer          container=None
  depth=0 Variable insideFn       container=None
  depth=0 Function inner          container=None      <- nested inside outer
  depth=0 Variable insideInner    container=None      <- nested two deep
  depth=0 Variable insideMethod   container=Panel     <- the class, not render
```

Three defects, all recoverable from the tokens the scan already had:

  1. **`depth` was hard-coded `0`**, under a comment saying the token scan could
     not see nesting — two lines below the counter that was tracking it.
     `analysis` uses `depth == 0` to decide a symbol is top-level, so a `fn`
     nested two levels down was offered as a top-level name.
  2. **`container` was only ever a class.** A `fn` inside a `fn` had none, and a
     method-local was attributed to the **class**, which puts it in the outline
     beside the method rather than inside it.
  3. **No class field appeared at all** — `count`, `label`, `name`, `agree`,
     `id`, every field in the four fixtures.

**Fixed within the scan, without a new frontend.** The brace counter feeds
`depth`; a second stack tracks named callables so `container` is the innermost
enclosing scope rather than the nearest class; and an `Ident Assign` at exactly
a class's body depth is a field — safe to read from tokens because at that depth
there is no statement position, so it cannot be an assignment.

The kind and the container stay on separate stacks deliberately: a `fn` nested
inside another is a **function**, not a method of whatever class happens to be
open, and merging the two would have lost that.

**What is still debt, and is not improvised here.** The scan has no idea it is
inside JSX. It works on these fixtures because JSX expression braces balance —
`{c}`, `onChange={(v) => { … }}` — and `jsx_expression_braces_leave_the_depth_balanced`
pins that a top-level `let` after a render() full of them is still at depth 0.
A JSX construct that does not balance braces, or one that puts a declaration
somewhere the token order does not reflect, is out of reach. Closing that needs a
**structural `.szx` frontend**, which is a real piece of work and a decision
about where translation happens; the finding was explicit that it should be
recorded rather than attempted inside this molecule.

**Pinned by** `tests/szx_outline.rs`, 9 tests. Against a `depth` forced back to
`0`, exactly the two depth tests fail and the seven others pass — the container,
kind, field and survivability properties are independent of it, which is what
says the suite is measuring more than one thing.

---

### 5.54 — the phase timings measured the sampling order as much as the code — **FIXED 2026-09-04**, medium (found in the Core-defects pass, 2026-09-03)

`tests/perf_budget.rs` ran each phase 15 times in a row before starting the
next, with no warmup. Two consequences, both visible in the numbers:

  * whatever else the machine did during a phase's block of runs landed on
    **that phase**, entirely — which is how one phase acquires a 3× spread while
    its neighbours look clean, and why the phases were not comparable to each
    other;
  * a phase's first run pays for cold caches and a heap that has not reached a
    steady size. The minimum is immune to that; `max` is not, and `max` is what
    the spread column reported.

**Measured before**, three consecutive release runs:

```text
semantic.validate    136    190   1.40x   3.17x
semantic.validate    136    192   1.41x   2.83x
semantic.validate    136    200   1.47x   3.60x
```

A ratio of 1.40× with a spread of 3.17× is not evidence of anything, which is
precisely what the finding said: observed above baseline, variance too high to
call it a regression.

**Fixed by changing how it samples**, not what it measures. Three warmup rounds
discarded, then fifteen rounds running **every** phase once each. A slow stretch
of wall-clock now touches every phase's sample for that round instead of one
phase's whole distribution. The report gained the **median** and reports
`max/median` beside `max/min`: a scheduling hiccup moves `max` and leaves the
median alone, while `max/min` is also moved by an unusually *fast* run, which is
evidence of nothing.

| | before (`max/min`, consecutive) | after (`max/median`, interleaved) |
|---|---|---|
| spread across phases | 1.8× – 3.6× | **1.1× – 1.6×** |

**The change invalidated the baseline, and that is recorded rather than
presented as an improvement.** A phase measured after a *different* phase sees a
colder cache than one measured after itself, so the minimum rises — most for the
sub-100 µs phases. Isolated before concluding anything: with this cycle's new
semantic work disabled, `semantic.validate` read **2.69×**, *higher* than with it
enabled (2.40×). The jump was the harness, not the code. The baseline was
re-recorded under the new regime, the file says so, and the two sets of numbers
are not comparable.

Against the new baseline, three consecutive runs put every phase between 0.91×
and 1.13× — well inside the 1.5× budget.

**The baseline now says which machine made it.** `# recorded on: windows/x86_64`,
and a comparison against a different OS/arch prints a note saying the difference
is between two machines as much as between two revisions. That was the half
missing from "numbers are machine-specific" being merely written in a comment.

**What this does not decide.** Whether any phase becomes a blocking gate is
**DEC-M10-003**, registered with the ratio-stability table this produced:
`runtime.execute` swings 0.02 across three runs, the two sub-100 µs phases swing
0.20 and 0.21. Nothing is promoted here.

**Found by the gate one commit later:** the interleaved harness needed a boxed
closure per phase, and `clippy::type_complexity` fired twice on the tuple. Fixed
with a `type Phase<'a>` alias rather than recorded as accepted debt.

---

### 5.55 — the editor's silence about the semantic phase now spans five rules — *open under DEC-M4-006*, medium (widened from §5.41, 2026-09-04)

§5.41 recorded that `sz` rejects a program the editor says nothing about, and
DEC-M4-006 owns what to do. The Core-defects pass asked whether that decision was
still necessary once the resolver's false positives and negatives were closed.

**They were closed, and it is still necessary** — the re-evaluation is written
into DEC-M4-006 itself. In short: the reason severity was fraught was not knowing
how often the phase is wrong, and that is now measured — §5.51's rule has zero
false positives across the corpus and all eight ecosystem packages, and §5.50's
import resolution is off by default, so the LSP sees exactly what the CLI sees.
What remains is the A/B/C product judgement about whether an editor should also
suppress type diagnostics, which no measurement settles.

**What did change is the size of the gap.** `analyze` does not call
`semantic::validate` at all, so the silence grows with every rule the phase
gains. One rule when §5.41 was written, five now:

| The phase rejects | The editor says |
|---|---|
| a reserved namespace name | nothing |
| a duplicate declaration | nothing |
| an unresolvable parent | nothing |
| a name declared nowhere | nothing |
| a name used before it is declared | nothing |

`the_semantic_phase_does_not_yet_reach_the_editor` covers all five now, each with
a positive control asserting the rule really fires in the same process through
the same library — without which the pin would pass against a phase that had
stopped working. A sixth test asserts the editor is not silent in general, which
is the control the five absences need.

---

### 5.56 — `unsafe` was a bool with no stated contract — **FIXED 2026-09-04**, medium (owner decision, 2026-09-04)

The owner's decision: Serez is **safe by default**, there is no `safe` keyword
and there will not be one, and `unsafe { }` is the author accepting *specific,
named* relaxations — not the defences going off.

**What was there.** `Evaluator::in_unsafe_block: bool`, set on entry to a block
and read at exactly one site, `require_unsafe`. Nothing said which guarantees
`unsafe` may relax, because nothing relaxed any: the gate was binary and the
question "may this limit be waived here" had no representation at all.

**What is there now.** `execution::ExecutionContext`, with the question a guard
should ask:

```rust
ctx.waives(Guarantee::ProcessOutputCeiling)
```

`Guarantee` is the contract, enumerated in one place. A limit not listed is not
waivable, and adding a variant is a language decision rather than a refactor —
which is the point of an enum over `if function_name == "OS.exec"`. Today the
list has one entry.

**Measured before changing anything.** The permission/`unsafe` separation was
already correct and is now pinned in both directions:

| Program | Outcome |
|---|---|
| `unsafe { OS.exec(…) }` | `SZ6001` — no permission |
| `use permissions { OS }` then `OS.exec(…)` | `SZ6003` — no unsafe |
| both | runs |

And what `unsafe` does *not* relax, each now a test rather than a sentence:
lockdown, argument validation, the protected-path heuristic, type safety, and an
unlisted limit — the generator ceiling, chosen because it is host-set and
deliberately absent from `Guarantee`.

**A spec/implementation divergence, found by measuring.** `spec/security.md`
said a gated call must "appear **lexically** inside an `unsafe { }` block". It
need not: the context is **dynamic**, and a function called from inside a block
runs with it in force. Registered as **DEC-M11-001** and *not* changed; the spec
now describes the runtime.

The evidence turned out to cut against the obvious recommendation. Every one of
the **20** gated calls across the eight ecosystem packages is already lexical,
and **145 of 159** in the corpus — the other 14 being the `sec_*_requires_unsafe`
fixtures that call outside a block on purpose. So the usual argument for keeping
dynamic, that changing it would break working code, is not supported by anything
measurable from here. That is written into the entry rather than left as a
recommendation the numbers do not carry.

**Pinned by** `tests/unsafe_contract.rs` (20 tests) and `src/execution.rs`'s own
four, which walk `Guarantee::ALL` rather than naming a variant, so one added
later cannot default to waived in ordinary code without failing.

### 5.57 — `OS.exec`'s output ceiling, decided — **FIXED 2026-09-04**, high (closes DEC-M9-003)

§5.48 measured the fourth unbounded read and registered the decision. The owner
answered it, and not with any of the A/B/C the entry offered: **the ceiling is a
guarantee `unsafe` waives.** Limits are mandatory while the runtime's guarantees
are in force; `unsafe` is where an author accepts named relaxations; this is one.

So the ceiling exists — 64 MiB, the same number the other three reads use — and
`unsafe` waives it. `OS.exec` requires `unsafe`, so every call takes the waived
path. The observable behaviour is what it was; what changed is that it is now a
contract in `spec/security.md`'s waivable-guarantee table instead of a gap in
`spec/limits.md`'s "what is not limited".

**The unwaived path is real, not decorative.** `evaluator::run_child_bounded`
drains both pipes concurrently — reading one to EOF before the other deadlocks
when the child fills the second, which is the bug already fixed once for
`OS.spawn` — and is tested at its boundary: exactly 64 MiB captured, 64 MiB + 1
refused, on stdout and stderr **separately**, because they take different code
paths and a ceiling applied only to the first would pass a stdout-only test. A
fourth test writes 4 MiB to both streams at once and fails by hanging if the
drain is ever serialised.

It has to be tested there rather than through `OS.exec`, because `OS.exec` never
runs with the guarantee in force. That is stated in the test file rather than
left for a reader to notice.

**Fixture note.** The child fixtures copy a file rather than generating bytes in
the shell: a `for /L` loop emitting 64 MiB takes minutes on Windows and timed the
suite out. Two more Windows-specific traps cost a cycle each and are recorded in
the code — a `ThreadId(5)` in a filename, whose parentheses `cmd` treats as
grouping, and `cmd /c "type \"…\""`, which Rust escapes for the MSVCRT parser
and `cmd` re-parses differently. Both showed up as a stdout of 0 bytes.

---

## 6. Carried-forward debt from `MATURITY_AUDIT.md`

`MATURITY_AUDIT.md` remains the register; this is the roadmap-facing digest of
what is still **open** there.

| Item | Severity | Milestone that owns it |
|---|---|---|
| Free variables resolve dynamically; undocumented in README; `--check` does not flag them | **critical, open** | M4 / M7 — needs an explicit product decision under `spec/compatibility.md` |
| ~~Property schemas not enforced after construction~~ | ~~high~~ | **FIXED** 2026-09-03, `89395b3` — see below |
| ~~Private access compares against the runtime receiver class~~ | ~~high~~ | **FIXED** 2026-09-03, `5f78f4e` — see below |
| ~~`EvalResult` mixes values, control flow, throw and an untyped `Error` sentinel~~ | ~~medium~~ | **FIXED** 2026-09-03, `5af868c` — see below |
| ~~`fetch` remains reachable under lockdown~~ | ~~high~~ | **FIXED** 2026-09-03, `649ba49` (DEC-M7-006) — see below |
| Non-atomic package installation; no lockfile, integrity or signature policy | high | M9 |
| LLVM backend parity unproven, feature-gated, absent from the CLI | high | M10 |
| No benchmark regression budget in CI, no stored baseline | medium | M10 |
| CI does not run the ecosystem canary | high | M10 |
| Generators accumulate into an unbounded vector (ceiling measured, deliberately not added) | medium | M9 |

### The four items closed on 2026-09-03

Each was closed with a measurement first, because all four are breaking and
`spec/compatibility.md`'s rule is that a breaking change ships in a minor only
when a sweep of the official packages and the conformance suite finds no affected
code, and the entry names the sweep and its result.

**Property schemas** — `89395b3`. A declared field type is a constraint for the
object's whole life, checked on every write from inside the class and outside it,
through `evaluator::type_matches` — the same function every parameter, return
type and `is` already uses, so nullable, `[T]`, `any` and enum variants behave at
a field exactly as they do at a parameter. It is not a comparison of type names,
and it deliberately does **not** make a declared class type accept a subclass,
because that is DEC-M5-005 and still open. Inherited fields and interface fields
are covered. Not constrained: a field with a default and no annotation, and a
field created by assignment — DEC-M5-004 recommended option A, explicitly not C.
DEC-M5-004 asked for the off-type-assignment count before implementing; running
the suites against the enforcement answers it exactly: **2**, both of them the
fixtures documenting the old behaviour.
*Checker:* `type_checker` does not inspect `FieldAssign` and never did, so this is
the runtime's rule alone — a gap in reach rather than a disagreement, recorded in
three `tests/type_agreement.rs` rows and stated in `spec/types.md`. Closing it
needs the checker to carry each class's field schema and would meet DEC-M5-005
the moment it typed a class-valued variable, so it is its own work.

**Private access** — `5f78f4e`, and DEC-M7-002 decided. `private` is private to
the class that declares the member. The declaring class is the key in both
directions: it is what the check compares against, and it is what
`executing_class` becomes while the body runs — so a subclass reaching an
inherited private is refused, and a `Base` method reaching `Base`'s own private
through a `Derived` receiver still works. Getters and setters are covered too;
they live in their own maps, and the first version of the fix missed them, which
would have left half the decision unimplemented on exactly the members
`spec/classes.md`'s caveat named. Sweep: **27** private declarations in the
corpus, **0** in the ecosystem — the one match is a string literal in serez-ui's
translator — and no file reaches a parent's private from a child. The message
changed, because "cannot be called externally" is no longer what the rule
refuses; `spec/compatibility.md`'s diagnostic-surface section records it.

**`EvalResult`** — `5af868c`. `Result<ExecutionFlow, RuntimeFailure>`: `Ok` is the
language doing something, `Err` is the runtime failing. No variant invented and
none dropped; `Throw` stays on the flow side, because a `throw` is a program
doing what programs may do. `RuntimeFailure` is a unit struct on purpose — the
payload already lives on the evaluator as `last_error` with a generation counter,
and a second copy would be a second source of truth. 1,608 sites rewritten by
script with balanced-paren matching, compiler-checked, zero warnings.
*The differential harness DEC-M6-001 asked for was built first* and committed as
`tools/runtime_diff.sh`: exit code, stdout and stderr verbatim for **222**
self-contained fixtures, captured before and after, `diff` empty.
*What the type found:* seven `self.rt_err(...)` calls whose result was discarded,
invisible under the old enum and `unused_must_use` under `Result`. All seven are
legitimate — they raise for the recording side effect and carry the decision in a
local flag — and now say `let _ =` so the intent is written down. No behaviour
changed at any of them.
*Not conflated:* DEC-M6-001 itself is untouched; namespace dispatch is still
`impl Evaluator`.

**`fetch` under lockdown** — `649ba49`, and DEC-M7-006 decided as option C.
Blocked by default, opened only by an explicit allowlist that the **embedder**
sets — `--allow-fetch`, `RunOpts::fetch_allowlist`, `Evaluator::allow_fetch_hosts`
— and that a program cannot extend, for the same reason `use permissions { }`
stops granting under lockdown. Hostname matching, case-insensitive and exact: no
wildcards, no suffix matching, no port matching, and userinfo cannot disguise a
host. A URL whose host cannot be read is refused rather than guessed at.
*Redirects are checked at every hop*, which is the half that would have made the
rest theatre: `ureq` follows redirects itself, so `allowed → 302 → forbidden`
would have arrived with nothing asking. Under an allowlist the agent is built
with `redirects(0)` and the hops are followed and validated one at a time, to the
same ceiling of 5. Outside lockdown none of this is in the path — verified
against the release binary that a cross-host redirect still follows.
*The conformance test that pinned the old behaviour inverted by design*, in both
runners, and became four. `tests/lockdown_fetch.rs` adds 14 more, seven against a
real HTTP server, with a positive control for the redirect case so a `fetch` that
simply always failed could not pass.
*Not closed:* **DEC-M9-001**. The response-size ceiling and the unbounded
`read_to_end` are separate problems and stay open; the only overlap taken here is
the redirect chain, without which the allowlist is bypassable.

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
| **DEC-M11-001** | Whether the `unsafe` context is lexical or dynamic | **OPEN** | nothing; the measured behaviour is pinned and the spec now describes it |
| **DEC-M4-001** | Where the reserved-name check runs | **DECIDED** 2026-09-03 — **A, a new fatal semantic phase** | unblocked M4.5.*; DEC-M4-003's landing site |
| **DEC-M4-002** | Whether an unresolved free variable is a diagnostic | **DECIDED** 2026-09-03 — **A, fatal**; implemented in `1513b43` | — |
| **DEC-M4-003** | Whether the reserved-name guard covers all 22 namespaces | **OPEN** | M4.6.1 — and is ordered after DEC-M4-001 |
| **DEC-M4-004** | What the editor's outline should show | **DECIDED** 2026-09-03 — **B, all declarations correctly nested**; implemented in `ec3cd81` | — |
| **DEC-M4-005** | The semantic phase's code and label | **DECIDED** 2026-09-03 — **A, `SZ8xxx` + `SEMANTIC`** | unblocked M4.5.4–M4.5.6 |
| **DEC-M4-006** | Whether the LSP reports the fatal semantic phase | **OPEN** | nothing; §5.41 is the finding |
| **DEC-M4-007** | Whether the semantic phase resolves through `import` | **OPEN** — option A is now implemented, measured and switched off; see the update in its entry | §5.39's parent rule, and §5.50's free-name rule |
| **DEC-M4-008** | Whether a `class` and an `interface` may share a name | **OPEN** | nothing; 0 measured occurrences |
| **DEC-M4-009** | Whether the `.sz` outline falls back to a token scan on a file that does not parse | **OPEN** | nothing; the fallback preserves today's behaviour |
| **DEC-M5-001** | Whether a nullable value at a non-nullable parameter is reported | **OPEN** | nothing — a question to answer, not a gate |
| **DEC-M5-002** | Whether a numeric type widens at a parameter | **OPEN** | nothing |
| **DEC-M5-003** | Whether an unknown type name is diagnosed | **OPEN** | nothing; option B depends on DEC-M4-001 |
| **DEC-M5-004** | Whether a declared field type is a constraint or a default | **OPEN** | nothing |
| **DEC-M5-005** | Whether a declared class type accepts a subclass | **OPEN** | nothing |
| **DEC-M6-001** | How a runtime service raises an error and allocates a value | **DECIDED** 2026-09-03 — **a narrow trait** (§7A's B; the letter given was A — see §0B.B); `ValueSink` and the first service in `5707779` | the remaining 15 namespaces, now mechanical |
| **DEC-M7-001** | Whether `remove` on an empty array is an error | **OPEN** | nothing |
| **DEC-M7-002** | Whether a subclass reaches an inherited private member | **DECIDED** 2026-09-03 — **A, keyed to the declaring class**; implemented in `5f78f4e` | — |
| **DEC-M7-003** | Whether a `match` with no matching arm is an error | **OPEN** | nothing; interacts with DEC-M7-005 |
| **DEC-M7-004** | Whether `==` compares containers structurally | **OPEN** | nothing; ships with DEC-M5-005 |
| **DEC-M7-005** | What a `match` pattern that fails to evaluate does | **OPEN** | nothing; should precede DEC-M7-003 |
| **DEC-M7-006** | Whether `fetch` is reachable under lockdown | **DECIDED** 2026-09-03 — **C, gated with an explicit allowlist**; implemented in `649ba49` | — (DEC-M9-001 remains open) |
| **DEC-M9-001** | What ceiling an unbounded read has, and what happens at it | **DECIDED** 2026-09-03 — **A, 64 MiB fatal**; implemented in `41e8d5a` | — |
| **DEC-M9-002** | Whether package, manifest and lockfile are one recoverable transaction | **OPEN** | nothing; the lockfile is written under either answer |
| **DEC-M9-003** | What ceiling `OS.exec` output has, and what happens at it | **DECIDED** 2026-09-04 — **the ceiling is a guarantee `unsafe` waives**; 64 MiB when in force, waived inside `unsafe { }`, which is the only context `OS.exec` runs in | — |
| **DEC-M10-001** | Whether CI runs the ecosystem canary | **DECIDED** 2026-09-03 — **a pinned blocking gate, plus a daily unpinned run**; implemented in `1d61f5d` | — |
| **DEC-M10-003** | Whether any phase timing blocks a build | **OPEN** | nothing; the timings are advisory and the stability evidence is collected on every run |
| **DEC-M10-002** | Whether clippy is a gate | **DECIDED** 2026-09-03 — **a per-lint/file baseline** (§7A's C; the letter given was B — see §0B.B); implemented in `1d61f5d` | — |

---

### DEC-M11-001 — Is the `unsafe` context lexical or dynamic?

**Problem.** `spec/security.md` said an `unsafe`-gated call must "appear
**lexically** inside an `unsafe { }` block". The runtime does not work that way,
and never has: the context is **dynamic**, so a function called from inside a
block runs with it in force wherever its body happens to be.

**Current behaviour.** Measured against the release binary:

```text
fn void helper() { OS.exec("cmd", ["/c","echo","hi"]); }
unsafe { helper(); }        runs
helper();                   SZ6003 — requires an `unsafe { }` block
```

`Evaluator` holds one context, set on entry to a block and restored on the way
out — including when the block is left by `throw` or by `return` — so the
propagation is a property of *execution*, not of source position.

**Measured evidence.** The divergence is between the spec and the
implementation, not within either — and **nothing measured depends on the
dynamic reading**:

| Where | Gated calls | Lexically inside `unsafe` | Not |
|---|---|---|---|
| the 508-file corpus | 159 | 145 | **14** |
| eight ecosystem packages | 20 | **20** | 0 |

All 14 corpus exceptions are the `sec_*_requires_unsafe.sz` fixtures, which call
the operation outside a block on purpose to assert that it is refused. Not one
program in either set gates a destructive call through a helper.

That cuts against the recommendation below rather than for it, and is recorded
that way: option B's compatibility cost, which is the main argument against it,
is **zero across everything currently measurable**. What B would still cost is
paid by code that is not in these two sets.

**Option A — dynamic, and correct the spec.** What the runtime does. A block
says "everything that happens while this runs may relax the listed guarantees",
which is coherent, and it is what every existing program was written against.
It also means a function's own source cannot tell you whether it is running
under a relaxation.

**Option B — lexical, and correct the runtime.** What the spec said, and what
Rust does. A reader of a function can see whether it is inside `unsafe` without
knowing its callers, which is the property that makes `unsafe` greppable. It is
a **breaking change**: `unsafe { helper() }` stops working, and every program
that gates through a helper has to move the block.

**Trade-offs.** A is free and already true; its cost is that "grep for unsafe"
tells you where relaxations are *authorised*, not where they *happen*. B is the
stronger review property, and its usual objection — that it breaks working
programs — is not supported by the measurement above: nothing in the corpus or
the ecosystem would move. B does still need a rule for what a lambda called from
inside a block does, which A never has to answer, and it is breaking for code
outside these two sets in a way that cannot be measured from here.

**Architectural impact.** B needs the context on the call frame rather than the
evaluator, and a decision about closures. **Semantic impact.** B rejects
programs that run today. **Compatibility.** B is breaking; `compatibility.md`'s
process applies. **Impact by area.** `execution`, `evaluator::stmt`,
`spec/security.md`, and any ecosystem package that wraps a gated call.

**Recommendation — a recommendation, not a decision.** Weaker than it would
have been before the measurement. **A** for now, because it is what the runtime
does and the spec has been corrected to describe it rather than to describe an
intention. But the usual reason to prefer A — that B breaks working code — did
not survive being checked: 20 of 20 ecosystem calls and 145 of 145 real corpus
calls are already lexical. If the owner wants the stronger review property, B is
cheaper than it looks, and the open question is the closure rule rather than
migration.

**Blocked by this decision:** nothing. `unsafe_propagates_dynamically_into_calls`
pins the measured behaviour and says in its failure message that changing it is
this decision.

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
  * The rejection emits **three diagnostics for one problem** (§5.32) *when the
    class has a body*: the real error plus two spurious `Unexpected token '}'`
    inventions, because `parse_class_declaration` returns `None` mid-declaration
    and the unconsumed body is re-parsed as top-level expressions.
    **Correction, 2026-09-03:** this record previously said the corpus fixture's
    222 bytes were large "for that reason". They are not. `err_task_reserved_class.sz`
    declares `class Task { }` with an **empty** body and emits exactly **one**
    diagnostic; the 222 bytes are the caret rendering plus the abort line. The
    cascade is real and the fixture does not exhibit it. Isolated by a positive
    control: the identical file with an unguarded name parses cleanly.

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

## RESOLUTION — **DECIDED 2026-09-03: option A, a new fatal semantic phase.**

Decided by Sergio, presented with the evidence above plus the additional
measurements below.

### Why A was chosen

The property it buys is **static analysability**, and the argument is about what
the pipeline can express rather than about tidiness.

Serez had exactly two modes of rejection: **syntactic**, fatal, in the parser; and
**type**, advisory, which by contract in `spec/types.md` does not reject anything.
There was no third. So any rule about *meaning* that needs to reject a program had
to disguise itself as a rule about *structure* — which is precisely what
`is_reserved_name` did, and precisely why it produces invented errors: the parser
was aborting a half-built structure in order to express something that is not
about structure.

Three consequences, each measurable:

1. **Diagnostic predictability.** Three diagnostics become one for the
   class-with-a-body case. One problem, one error.
2. **Cohesion.** The parser is left with a single reason to change — the grammar.
   `src/semantic.rs`, built in M4.2 with no consumers, acquires the first one M4
   built it for.
3. **Reach of `--check`.** Semantic gaps that are invisible before running become
   reportable.

### The measurement that settled it

The presentation added evidence the original record did not have: **the phase
would not have a single tenant.** Three further semantic gaps were probed on
2026-09-03, all silent today:

| Program | Today | Recorded as |
|---|---|---|
| `class A {}` declared twice | **accepted silently**, last wins | §5.39 |
| `fn int f()` declared twice | **accepted silently**, last wins | §5.39 |
| `class Child : Missing {}`, never instantiated | **runs clean**; fails only at `new`, with `SZ4001` | §5.39 |

With the three already registered as decisions — DEC-M5-003 (unknown type name),
DEC-M4-002 (free variables), DEC-M7-003 (`match` exhaustiveness) — the phase has
**six** candidate tenants rather than one. That is what distinguishes a real
boundary from the speculative abstraction rule 9 forbids, and it is the single
strongest argument for A over B.

### Alternatives rejected, and why

  * **B — leave it in the parser.** Rejected not because it is wrong but because
    it does not avoid the decision, it **defers and multiplies** it: each of the
    five remaining candidates re-asks "where does this live?", and answering that
    five times separately is how a language ends up with five answers. B also
    leaves `DEC-M4-003` with no good landing site and §5.32 permanently.
  * **C — move it to the type checker.** Not viable. `spec/types.md` makes checker
    findings advisory, so `class Task {}` would begin to **run**. `BREAKING`, in
    the worst direction — accepting programs currently rejected.
  * **D — an advisory semantic phase.** Added during the presentation because it
    was not in the original record and is defensible. Rejected for C's reason: it
    makes `class Task {}` run, and it leaves the language still without any stage
    able to reject on meaning, which is the gap A exists to close.

### Classification and contract

**BEHAVIORAL.** The set of accepted programs is **unchanged** — A moves *when* the
error is reported, not *which* programs are rejected. What changes is stderr: a new
phase label and a new diagnostic code. The exit code for an affected program stays
`1`.

`spec/compatibility.md` governs the stderr change and needs a note, because "0
affected files in the corpus and 0 in the ecosystem" measures *internal* users
only. Something external may parse `PARSER ERROR`.

### What this unblocks

  * **M4.5.1 – M4.5.6** in full (§9F.5).
  * **DEC-M4-003** acquires its landing site, and is ordered strictly after M4.5.5
    so the name list changes once, in its final home.
  * **DEC-M5-003 option B** becomes available.
  * **DEC-M4-002 option A** and **DEC-M7-003 option B** become *possible*; neither
    becomes decided by this.

### What it does not authorise

The phase's **public surface** — the rendered label and the diagnostic code — is
its own choice, hard to revert once released, and is **not** settled by this
decision. It is M4.5.1, and it is registered separately as **DEC-M4-005**.

M4.5.2 and M4.5.3 do not need it: a phase that reports nothing needs no label, so
that work proceeds first.

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

### DEC-M4-005 — What code and label does a semantic error carry?

**Problem.** The phase DEC-M4-001 created will print something. Two blanks have
to be filled:

```
❌ ??????? ERROR [SZ????] [file 2:7]: 'Task' is a reserved system namespace…
```

Both are public API. `spec/errors.md` states it directly: *"A code is a promise of
stability."* Once released, changing either is a breaking change with its own
deprecation process.

**Current behaviour.** Four phases produce diagnostics, each with a word and a
range: `LEXER`/`SZ1xxx`, `PARSER`/`SZ2xxx`, `TYPE`/`SZ3xxx`, the runtime with no
word at all and `SZ4xxx`–`SZ6xxx`, and `COMPILER`/`SZ7xxx`. `SZ8xxx`, `SZ9xxx` and
`SZ0xxx` are unused.

The line that decides this: **`spec/errors.md:45`** documents `SZ3000` as
*"Semantic or type diagnostic. **Advisory:** … `sz file.sz` reports these and
**still runs**."*

So the range is already *named* "semantic", and its one documented code carries an
explicit promise that the program keeps running.

**Measured evidence.**

  * 21 codes exist across `SZ1`–`SZ7`; `SZ8`, `SZ9`, `SZ0` are free.
  * The runtime range **already** mixes fatal and recoverable
    (`spec/errors.md` §"Recoverable and fatal runtime errors"), so
    "one fatality per range" is **not** a rule the language currently keeps. That
    weakens the tidiest argument for a new range and is recorded rather than
    omitted.
  * `src/lsp/analysis.rs:146,159` sets severity by **which producer it asked**
    (`1` for the parser, `2` for the checker) and never reads
    `Diagnostic::severity`. Every alternative requires touching it.
  * **No external consumer keys on codes:** `vscode-serez/` contains no `SZ`
    string at all. In-repo, only tests do.
  * `spec/cli.md` lists "a lexer or parser diagnostic" among the causes of exit
    `1`; every alternative but C needs "semantic" added.

**Alternatives.**

| # | Option | Consequence |
|---|---|---|
| A | **`SZ8xxx` + `SEMANTIC`** | Range, phase and fatality align. `SZ3xxx` keeps its advisory promise intact. Costs a table row and a new word, and leaves `SZ3xxx`'s "semantic or type" name needing correction to "type" |
| B | **`SZ3001` + `SEMANTIC`** | Uses the range already called semantic. The range then mixes advisory and fatal — the property a caller cares about most — and needs a sub-rule where it has none today |
| C | **Reuse `PARSER`/`SZ2000`** | Zero new surface. Also makes the phase invisible and mislabels its findings: duplicate declarations and missing parents would all report as syntax errors |
| D | **`SZ8xxx`, no label** | One fewer piece of surface. Inconsistent with the other three frontend phases, and drops the cheapest useful fact in the line — which stage failed |

**Trade-offs.** The real tension is B against A. B is not wrong on the letter —
the advisory promise is written per *code*, not per range — but `spec/errors.md`
spent a sentence making sure nobody reads `SZ3000` as fatal, and putting a fatal
code beside it invites exactly that misreading. C is the minimum-cost option and
was rejected on diagnostic quality rather than on cost.

**Architectural impact.** None on the pipeline; DEC-M4-001 already settled that.
This decides only what the phase looks like from outside.

**Semantic impact.** None. Nothing about which programs run changes.

**Compatibility.** `ADDITIVE` to the code space; `BEHAVIORAL` for the affected
program's stderr.

**Impact by area.** Tests: manifest rows for the one affected fixture, at M4.5.4.
Specs: `spec/errors.md` gains a range row and needs `SZ3xxx` renamed to "type";
`spec/cli.md`'s exit-code sentence; `spec/compatibility.md` gets the behavioural
note. LSP: a new block in `analyze` with `severity: 1`. Runtime: none.

**Recommendation — a recommendation, not a decision.** **A.** The property it buys
is a consumer's ability to classify without reading prose: a free range carries no
prior promise to contradict, while `SZ3000`'s first documented fact is that the
program keeps running.

---

## RESOLUTION — **DECIDED 2026-09-03: option A, `SZ8xxx` with the label `SEMANTIC`.**

Decided by Sergio.

### Why A

Not because ranges should be homogeneous — they are not, and the runtime range
proves it. The argument is narrower and stronger: **`SZ3000` carries a written
promise that the program still runs**, and that promise is the first thing anyone
learns about `SZ3xxx`. B leaves it technically intact and practically
unrecognisable; a tool filtering `SZ3` as warnings would start swallowing fatal
errors. A free range has no prior promise to contradict.

C was rejected on diagnostic quality, not cost. The phase's future tenants —
duplicate declarations, a missing parent class, an unknown type name (§5.39) — are
none of them syntax errors, and labelling them `PARSER ERROR` sends a user looking
for a typo where the problem is meaning. Improving that diagnostic is why
DEC-M4-001 chose a phase at all.

### The specific code, and why it is `SZ8000`

Within the chosen range, the number follows the convention `spec/errors.md`
already documents:

> *"Individual messages move from a generic code to a narrower one only once a
> test pins what the narrower code means. Until then the generic code is the
> honest answer."*

`SZ2000` and `SZ3000` are the generic codes of their ranges. **`SZ8000` is the
generic semantic diagnostic**, and the reserved-name rule uses it. Narrower
`SZ8001`+ codes get allocated when a test pins what each means — not in advance.

### Alternatives rejected

  * **B** — `SZ3001` in the existing range. Mixes advisory and fatal in the range
    whose documented headline is "still runs".
  * **C** — reuse `PARSER`/`SZ2000`. Makes the phase invisible and mislabels every
    future tenant.
  * **D** — new range, no label. Drops the cheapest useful fact in the line.

### Classification

`ADDITIVE` to the code space. `BEHAVIORAL` for the one affected program's stderr.
No semantic change.

### What this unblocks

**M4.5.4** — moving `is_reserved_name` into the phase, which is the first real
diagnostic it emits and could not be written without this. Then **M4.5.5** (delete
the parser's path) and **M4.5.6** (specs). **DEC-M4-003** lands after M4.5.5.

### Known follow-up this creates

`spec/errors.md` describes `SZ3xxx` as *"Semantic or type diagnostic"*. With fatal
semantics in `SZ8xxx`, that row is misleading and is corrected to "type" in
M4.5.6. Recorded here so the correction reads as a consequence of this decision
rather than as an unrelated edit.

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

### DEC-M7-001 — Should `remove` on an empty array be an error?

**Problem.** `remove` on an empty array returns `null`. Every other out-of-range
index on an array raises `IndexOutOfBounds`. One method disagrees with the rest of
the container surface about what an impossible index means.

**Current behaviour.** As above. `spec/arrays.md` "Known inconsistency" documents
it, and says aligning it "would be a breaking change with no migration path for
callers that rely on the null, so it is recorded here rather than fixed silently".

**Measured evidence.** `remove` appears in **12** tracked corpus files. How many
call it on a possibly-empty array is not mechanically decidable and was not
guessed at.

**Alternatives.** A: **raise `IndexOutOfBounds`**, matching every other index
operation — breaking, and silent for anyone who tests the result for null. B:
**keep the null** and state it as intentional rather than as an inconsistency.
C: **add a separate `tryRemove`** returning null, and make `remove` raise — no
break, at the cost of two methods for one operation.

**Trade-offs.** A's danger is its silence: code doing `if (a.remove(0) == null)`
does not stop compiling, it starts throwing. B is free and keeps a rule nobody can
derive. C is the compatible path and adds surface.

**Architectural impact.** None; one method body.
**Semantic impact.** A and C change which programs run. **Compatibility.** A is
breaking, C additive. **Impact by area.** Tests: fixtures asserting the null would
invert under A. Specs: `spec/arrays.md` loses its "Known inconsistency" section.
Runtime: one method. LSP, ecosystem: none.

**Recommendation — a recommendation, not a decision.** **C**, then **A** in the
following major. It is the only path that gives callers a migration, and
`spec/compatibility.md` exists to stage exactly this.

**Blocked by this decision:** nothing.

---

### DEC-M7-002 — Should a subclass reach an inherited private member?

**Problem.** Privacy is keyed to the **receiver's runtime class**, not to the
member's declaring class. A subclass method can therefore call an inherited
private method, while the same call from outside is refused.

**Current behaviour.** Probed against 10.0.0:

```serez
public class Base    { public Base() { this.secret = 42; }
                       private int hidden() { return this.secret; } }
public class Derived : Base { public Derived() { super(); }
                              public int reach() { return this.hidden(); } }
out new Derived().reach();     // 42, exit 0
```

`spec/classes.md` records it under caveats — "recorded below rather than silently
described as stronger than the implementation" — and `MATURITY_AUDIT.md` carries
it as **high, open**, assigned to M7.

**Measured evidence.** Not measured, and the reason is worth stating: the pattern
is only visible by resolving each `this.m()` inside a subclass against the
declaring class of `m`, which needs the class hierarchy *and* member visibility —
the semantic layer M4 built but has not been given a consumer for. **This is a
concrete second use for DEC-M4-002's resolver**, and it is recorded as such rather
than estimated.

**Alternatives.** A: **key privacy to the declaring class** — a subclass can no
longer reach an inherited private. Breaking, and silent: working code starts
raising. B: **keep it** and rename the guarantee in the docs, since what the
language enforces is "not reachable from outside the hierarchy", not "private".
C: **add `protected`** and leave `private` as it is — additive, and makes the
existing behaviour the honest meaning of a different keyword.

**Trade-offs.** A is what "private" means in every language that has it, and is
the most likely to break real code silently. B costs nothing and permanently
weakens a word users already understand. C adds a keyword to a language that does
not have one.

**Architectural impact.** A needs the member's declaring class at the call site;
the evaluator has the hierarchy, so this is reachable without new infrastructure.
**Semantic impact.** A and C change which programs run. **Compatibility.** A is
breaking and silent — the worst combination, and it needs the staging in
`spec/compatibility.md`. **Impact by area.** Specs: `spec/classes.md` caveats.
Runtime: the privacy check. Tests: unknown until measured. Ecosystem: unmeasured.

**Recommendation — a recommendation, not a decision.** **A**, in a major, **after
the measurement** — and the measurement should be the first thing built on M4's
resolver, because it is the same walk. Shipping A blind is the one option here
that can break working code with no diagnostic.

**Blocked by this decision:** nothing today. **Wants:** a resolver, per above.

## RESOLUTION — **DECIDED 2026-09-03: option A, keyed to the declaring class.**

Implemented in `5f78f4e`. `private` is private to the class that declares the
member.

| Access | Before | Now |
|---|---|---|
| a `Base` method uses `Base`'s private | allowed | allowed |
| ...through a `Derived` receiver | allowed | **allowed** |
| a `Derived` method uses `Base`'s private | allowed | **refused** |
| another class | refused | refused |
| outside any class | refused | refused |

**Row two is what made this more than a one-line change.** Inheritance must not
widen `private`, and it must not narrow it either: a subclass calling an
accessible parent method that internally uses the private member keeps working,
and that was never the thing being forbidden. So the declaring class is the key
in both directions — the check compares against it, and `executing_class` is set
to it while the body runs. Keying `executing_class` to the receiver's class, as
before, would have refused a `Base` method its own private members whenever it
was reached through a `Derived`.

**The measurement this entry recorded itself as lacking:** **27** `private`
declarations across the 515-file corpus and **0** across all eight ecosystem
packages — the single ecosystem match is a string literal in serez-ui's
translator. Five corpus files declare a private member *and* a subclass; none
reaches a parent's private from the child.

**Getters and setters were the trap.** They live in `StoredClass::getters` /
`setters`, not `methods`, so an owner lookup that searched only `methods` fell
back to the receiver's class and left half the decision unimplemented — on
exactly the members `spec/classes.md`'s caveat named ("method/getter/setter").
`declaring_class_of` picks the map from the member's own `is_getter` /
`is_setter`.

**The pin did its job.** `frozen_semantics::a_subclass_reaches_an_inherited_private_method`
existed to fail the day this moved, and it failed. It is replaced, not deleted,
by `private_is_keyed_to_the_declaring_class_and_not_the_receiver`, which asserts
both halves; the file gained a `refuses` helper, because a suite that could only
assert "this program completes" had no way to state a rule that says "this one
must not".

**Message text changed**, and that is recorded in `spec/compatibility.md`'s
diagnostic-surface section: `Method 'm' is private and cannot be called
externally` becomes `Method 'm' is private to 'Base' and cannot be called from
here`. The old wording is now false — an access from inside the hierarchy but
outside the declaring class is refused, and it is not external. Code, kind,
catchability and exit code are unchanged.

**Tests:** `err_private_inherited.sz`, which prints the permitted access before
making the refused one so it fails in either direction; three existing fixtures
regenerated for the wording; four `runtime_outcome.rs` assertions updated to name
the class rather than a wording that no longer describes the rule.

---

### DEC-M7-003 — Should a `match` with no matching arm be an error?

**Problem.** There is no exhaustiveness check, and no arm matching yields `null`
silently. `spec/control-flow.md` states it and calls it "a hazard, not a design
statement", noting that the `null` "is indistinguishable from an arm that
legitimately returned null".

**Current behaviour.** `let r = match 99 { 1 => "one", 2 => "two" };` sets `r` to
`null`, prints nothing, exits 0. Probed.

**Measured evidence.** **107** `match` expressions in the tracked corpus; **50**
of them have neither a wildcard `_` nor a binding arm. All 499 tests pass, so
none of the 50 currently falls through — but each is a site where a value outside
the listed arms would produce a silent `null`.

The two options below have very different exposure, and conflating them is how
this decision gets taken by accident:

| Option | What it affects |
|---|---|
| A runtime error when no arm matches | only programs where an arm actually fails to match — **0 in the corpus today** |
| A static exhaustiveness requirement | all **50** sites without a catch-all, whether or not they can fall through |

**Alternatives.** A: **raise at run time** when no arm matches. B: **require
exhaustiveness statically** — needs the checker to know an enum's full variant
set, which `semantic::declarations` already collects. C: **keep the null** and
state it as intentional. D: **warn**, advisory, on a `match` with no catch-all.

**Trade-offs.** A's corpus exposure is zero and its real exposure is every program
that relies on the null as a default — a use the spec explicitly says exists. B is
the strongest guarantee and touches 50 sites in this repository alone. D is free
and is the one option that produces information before anyone commits.

**Architectural impact.** B gives the checker a second dependency on the semantic
layer, alongside DEC-M5-003's option A. **Semantic impact.** A and B change which
programs run. **Compatibility.** A and B are breaking. **Impact by area.** Tests:
50 sites under B, none under A. Specs: `spec/control-flow.md`. LSP: B is a
genuinely useful squiggle. Ecosystem: unmeasured for B.

**Recommendation — a recommendation, not a decision.** **D now, A in a major, B
never as a hard error** — enum exhaustiveness is worth a warning, and a language
where `match` is an expression over arbitrary values cannot require it in general.
Note that A is much less useful while **DEC-M7-005** stands: a misspelled pattern
silently becomes a non-match, so A would raise at a `match` whose real defect is
one arm above.

**Blocked by this decision:** nothing. **Interacts with:** DEC-M7-005.

---

### DEC-M7-004 — Should `==` compare containers structurally?

**Problem.** `==` compares scalars by value and containers by identity, and
assignment copies — so **no two array values are ever equal, including an array
and its own copy**. `spec/values.md` calls it "a documented inconsistency, not a
design statement".

**Current behaviour.** `[1, 2] == [1, 2]` is `false`. Probed. Set membership and
`indexOf` do *not* use `==`; they use a fingerprint that compares scalars by value
and never matches a compound — so the language already has two different notions
of sameness, and neither is structural equality of containers.

**Measured evidence.** **Zero** direct `[…] == […]` comparisons in the tracked
corpus. That number is weak evidence and is labelled as such: the pattern that
matters is `a == b` where both are arrays, which a text search cannot see.

**Alternatives.** A: **structural equality** for arrays and dicts. B: **keep
identity** and say so as a decision. C: **structural for containers, identity for
class instances** — matching most languages that separate value and reference
types.

**Trade-offs.** A is what nearly every user expects and is breaking in the
direction that turns `false` into `true` — which flips branches silently, the same
hazard DEC-M5-005 carries for `is`. B is free and keeps a rule that surprises
everyone once. C is the most defensible and the most to specify.

**Architectural impact.** Structural comparison needs a depth bound, which
`MAX_VALUE_DEPTH` already provides for `extract` — so the ceiling exists and the
answer for exceeding it would have to be chosen. **Semantic impact.** A and C
change what working programs compute. **Compatibility.** Breaking and **silent**.
**Impact by area.** Runtime: the comparison path plus a cycle/depth rule. Specs:
`spec/values.md`, `spec/operators.md`, `spec/sets.md`. Tests: unknown.

**Recommendation — a recommendation, not a decision.** **C**, in a major, and only
alongside DEC-M5-005 — both change what `==`-shaped questions answer for compound
values, and shipping them in different releases means users learn the same lesson
twice. This is the highest-risk decision in the register precisely because
nothing fails loudly when it lands.

**Blocked by this decision:** nothing.

---

### DEC-M7-005 — What should a `match` pattern that fails to evaluate do?

**Problem.** `evaluator/expr.rs:1592` discards **any** error raised while
evaluating a literal pattern and reports "did not match" to the `match`:

```rust
let lit_ref = match self.eval_expression(lit_expr) {
    EvalResult::Value(v) => v,
    _ => return false,          // every failure becomes "did not match"
};
```

An undefined name, a bad member access, a thrown exception — all become a silent
fall-through to the next arm. §5.24 has the reproduction: a pattern naming a
non-existent enum falls through to `_` with no error, no warning and exit 0.

**Current behaviour.** As above, and it is **not** in any spec: `spec/control-flow.md`
documents the exhaustiveness hazard and not this one. Undocumented and
undecided is the worst of the four states a behaviour can be in.

**Measured evidence.** Not measurable by construction — the whole defect is that
it produces no signal. Its interaction with DEC-M7-003 is the real exposure: a
typo in a pattern falls to `_`, and if there is no `_`, to `null`. **Two silent
failures compose into a third.**

**Alternatives.** A: **propagate the error** — a pattern that cannot be evaluated
raises. B: **keep the fall-through** and document it. C: **propagate for
resolution failures** (undefined name, bad member) and keep the fall-through for
thrown exceptions, on the grounds that a `throw` inside a pattern might be
intentional.

**Trade-offs.** A is the M3 position — "zero silent failures" — applied here, and
it is breaking for any program with an unreachable pattern that currently costs
nothing. B leaves a typo undetectable. C is a rule with two halves and needs a
reason a user can remember.

**Architectural impact.** Small: one `_ => return false` becomes a propagation.
**Semantic impact.** A and C change which programs run. **Compatibility.**
Breaking, but only for programs whose patterns are already broken. **Impact by
area.** Runtime: `expr.rs:1592`. Specs: `spec/control-flow.md` gains a rule it does
not have. M3: this is a surviving instance of the "zero silent failures" goal.

**Recommendation — a recommendation, not a decision.** **A**, and **before**
DEC-M7-003, not after. A `match` that raises when nothing matches is far less
useful while a misspelled arm silently is not the thing that matched. This is also
the cheapest decision in the register to implement and the one whose current
behaviour is hardest to defend: nothing anywhere documents it, and no user could
discover it except by losing an afternoon.

**Blocked by this decision:** nothing. **Should precede:** DEC-M7-003.

---

### DEC-M7-006 — Should `fetch` be reachable under lockdown?

**Problem.** Lockdown exists to close the paths the permission manifest cannot,
for source that arrived from somewhere else — `--eval`, the playground. It closes
File, `import`, URL import and Autodiff's weight files. It does **not** close
`fetch`, so untrusted source can still make outbound HTTP requests.

**Current behaviour.** Deliberate and tested: `eval/lockdown: fetch is NOT gated`
is a conformance test, so the behaviour is pinned in both runners.
`MATURITY_AUDIT.md` records it as **high, open**, assigned to M7/M9, and the
external audit in `audit/` raises the same path as an SSRF and unbounded-memory
concern independently.

**Measured evidence.** One conformance test asserts the current behaviour, in
both runners. The audit report records that `fetch` reads the whole response with
`read_to_end` and no ceiling, so the exposure is not only *where* it can reach but
*how much* it can pull.

**Alternatives.** A: **gate `fetch` under lockdown**, like every other host reach.
B: **keep it open** and document the boundary precisely — lockdown covers the
filesystem, not the network. C: **gate it and add an explicit opt-in**, so a
playground can choose.

**Trade-offs.** B is the current state and its problem is that "lockdown" reads as
a sandbox while leaving the network open. A is the least surprising and removes a
capability an embedder may be relying on. C is the honest version of A.

**Architectural impact.** Small — one gate. It sits alongside the response-size
ceiling the audit recommends, which is **M9's** work and should not be conflated
with this one.

**Semantic impact.** A and C change which programs run under `--eval`.
**Compatibility.** Breaking for `--eval` and the playground; not for
`sz file.sz`. **Impact by area.** Tests: the pinned lockdown test inverts by
design under A or C. Specs: `spec/security.md` and `run::RunOpts::sandboxed`.
Runtime: one gate. Ecosystem: none — packages run outside lockdown.

**Recommendation — a recommendation, not a decision.** **C**. A capability
described as untrusted-source mode that leaves outbound HTTP open is a name doing
more work than the code, and an opt-in keeps the playground working. **This is the
one decision in the register with a security consequence**, which is why it is
recorded with its evidence rather than folded into M9's hardening pass.

**Blocked by this decision:** M9's treatment of `fetch`, in part — the size
ceiling is independent and can proceed either way.

## RESOLUTION — **DECIDED 2026-09-03: option C, gated with an explicit allowlist.**

Implemented in `649ba49`. Under lockdown `fetch` is refused with fatal
`PermissionError` (`SZ6001`) before any request leaves the process, and
`try/catch` cannot consume it — a security refusal a program can turn into
control flow is advice.

**The allowlist belongs to the embedder**, which is what makes it a gate. There
is deliberately no way for a running program to add to it, for the same reason
`use permissions { }` stops granting under lockdown: a list untrusted source can
extend is not a list. Three surfaces, all outside the program — `--allow-fetch`,
`RunOpts::fetch_allowlist`, `Evaluator::allow_fetch_hosts`.

**Matching is hostname, case-insensitive and exact.** No wildcards, no suffix
matching, no port matching: each is a policy question, and inventing an answer to
one inside a security gate is how a gate acquires a hole. Userinfo cannot
disguise a host — `http://allowed.test@evil.test/` is `evil.test` — and a URL
whose host cannot be read is refused rather than guessed at.

**Redirects are checked at every hop**, and this is the half that would have made
the rest theatre. `ureq` follows redirects itself, so gating only the URL the
program wrote would let `allowed.example → 302 → forbidden.internal` arrive with
nothing asking. Under an allowlist the agent is built with `redirects(0)` and the
hops are followed and validated one at a time, to ureq's own ceiling of 5. A
`Location` this runtime will not resolve — protocol-relative, path-relative, a
different scheme — is refused rather than followed.

**Outside lockdown none of this is in the path.** `allows_fetch` is always true
there, so `sz file.sz` goes straight to the original transport with ureq's own
redirect handling; verified against the release binary that a cross-host redirect
still follows.

**The pinned test inverted by design**, in both runners, and became four:
gated with no list, still gated for a host not on the list, an allowed host
reaching the builtin, and the refusal surviving `try/catch`. None needs the
network. `tests/lockdown_fetch.rs` adds 14 more, seven of them against a real
HTTP server that plays `localhost` and `127.0.0.1` as two names for one socket —
with a positive control (allowing *both* names lets the same redirect complete)
so a `fetch` that simply always failed could not pass, and a same-host relative
redirect asserted to still work so the gate is not over-blocking.

**Ecosystem sweep is vacuous, and says so:** no official package runs under
lockdown, so 8/8 is not evidence here. Breaking for `--eval` and the playground;
not for `sz file.sz`.

**DEC-M9-001 is deliberately NOT closed.** The response-size ceiling, the
unbounded `read_to_end` and M9's wider hardening remain separate and open. The
only overlap taken here is the redirect chain, which is not a size question and
without which the allowlist is bypassable.

---

### DEC-M9-001 — What ceiling should an unbounded read have, and what happens at it?

**Problem.** Three paths read an unbounded amount from a source the program does
not control, into memory:

| Path | Where | What it reads |
|---|---|---|
| `fetch` | `evaluator/builtins.rs` | the whole HTTP response body, via `read_to_end` |
| HTTP `import` | `evaluator/stmt.rs` | the whole remote module, via `into_string`, then caches it to disk |
| `OS.spawn` stderr | `evaluator/namespaces_os.rs` | everything the child writes, now into a drained buffer |

A server, a module host or a child process can therefore exhaust the interpreter's
memory. The independent audit in `audit/2026-09-01_14-52-03.md` raises the first
two as **high** and recommends a configurable limit read through
`take(limit + 1)`.

**Current behaviour.** No ceiling on any of the three. `OS.spawn`'s case changed
shape in M9.1 — it used to *deadlock* instead of growing, which is worse and is
now fixed — so all three are now genuinely unbounded rather than two unbounded
and one stuck.

**Measured evidence.** Not measured against real usage, and that is the honest
state: nothing in the corpus or the ecosystem fetches a large body, so a
measurement here would only confirm that the test suite does not do the dangerous
thing. The exposure is to *deployed* programs, which this repository cannot see.

**Alternatives.**

| # | Option | Consequence |
|---|---|---|
| A | **A fixed ceiling, fatal** — like `Memory.alloc`'s 256 MiB / `SZ6002` | Consistent with every other resource limit in `limits.md`. A program legitimately fetching something large stops working, with no way to ask for more |
| B | **A fixed ceiling, catchable** | The program can react — retry smaller, stream instead — but a `try` around a memory-exhaustion guard is a strange shape, and it differs from every other ceiling |
| C | **Configurable, with a default** | What the audit recommends. Needs a place to configure it: `serez.json`, an environment variable, or a runtime API, and each is a different public surface |
| D | **Streaming APIs** instead of a ceiling | Solves the real problem rather than capping it, and is a language feature, not a limit |

**Trade-offs.** A is the smallest change and matches the existing convention,
which is worth a lot — `limits.md` currently reads as one policy, and adding a
second kind of ceiling makes it two. C is more flexible and introduces a
configuration surface that does not exist today. D is correct and out of scope for
a hardening milestone.

**Architectural impact.** A and B are three call sites. C adds configuration
plumbing to the evaluator. **Semantic impact.** All of A-C make some currently-
working programs fail. **Compatibility.** Breaking for any program that reads more
than the ceiling; `spec/compatibility.md` governs, and `spec/limits.md` must state
whichever is chosen.

**Impact by area.** Tests: new fixtures per path; the ceiling itself needs a
fixture at the boundary, as `MEM-007` has. Specs: `spec/limits.md`,
`spec/security.md`. Runtime: three call sites. Ecosystem: `serez-http` is the
package most likely to notice, and it is not measured.

**Recommendation — a recommendation, not a decision.** **A**, at a generous
ceiling (64 MiB is the audit's upper suggestion), *fatal*, and stated in
`spec/limits.md` beside the others. Consistency with the existing resource policy
is worth more here than flexibility nobody has asked for, and C can follow if
anyone does. Do **not** pick a different answer per path: one policy, three call
sites.

**Blocked by this decision:** the ceiling itself. **Not blocked, and done:** the
`OS.spawn` deadlock (§9P.1), which was a bug rather than a policy question.

---

### DEC-M10-001 — Should CI run the ecosystem canary?

**Problem.** The ecosystem canary is the strongest compatibility signal this
repository has: eight official packages, 56 tests, run against a freshly built
`sz`. It runs **only locally**, because it needs the eight packages as sibling
checkouts, and CI has none. `MATURITY_AUDIT.md` records this as **high, open**.

Every milestone in this roadmap ran it by hand at each boundary. That worked and
does not scale: it depends on whoever is working remembering, and on their machine
having eight checkouts in the right place.

**Current behaviour.** `.github/workflows/ci.yml` runs `fmt`, `check`, `clippy`
and both Serez runners on `ubuntu-latest`, `windows-latest` and `macos-latest`.
Cross-platform parity is genuinely covered. The canary is absent.

**Measured evidence.** 8 packages, 56 tests, and the canary caught nothing during
M0-M10 — every run was 8/8. That is weak evidence for its value and strong
evidence that it is cheap: it is a signal that has not yet fired, not one that
fires noisily.

**Alternatives.** A: **clone the eight repositories in CI**, pinned to a ref —
straightforward, and CI now depends on eight external repositories being
reachable and green. B: **vendor a snapshot** of each package's tests into this
repository — hermetic, and a snapshot drifts from what the packages actually ship.
C: **a separate scheduled workflow** rather than per-commit — catches drift within
a day without making every PR depend on eight repositories. D: **leave it local**
and write down that it is a release gate rather than a CI gate.

**Trade-offs.** A makes every PR's result depend on repositories this one does not
control, which is how a compatibility signal turns into flakiness. C keeps the
signal and moves the failure out of the PR path. D is honest and keeps the
dependency on a person remembering.

**Architectural impact.** None on the language. It is a question about what the
release gate is, which is M10's subject.

**Semantic impact.** None. **Compatibility.** None.

**Impact by area.** CI: a new job or workflow. Ecosystem: the eight packages
acquire a contract with this repository's CI — under A that contract is
per-commit, and their breakage becomes this repository's red build.

**Recommendation — a recommendation, not a decision.** **C**, scheduled daily,
plus **D**'s documentation: the canary is a release gate and a daily signal, not a
per-commit one. A compatibility signal that can be broken by someone else's
repository should not be able to block a PR that did not touch the language.

**Blocked by this decision:** the canary's place in the release pipeline.

---

### DEC-M10-002 — Should clippy be a gate?

**Problem.** CI runs `cargo clippy --all-targets` **without** `-D warnings`, so
its 180 warnings fail nothing. A new warning introduced by a change is invisible
to CI. This roadmap worked around it by comparing the per-site list at every
milestone boundary (§5.26) — a discipline that exists because the gate does not.

**Current behaviour.** 180 distinct warning sites, stable across all of M0-M10;
59 in `evaluator/ops.rs`, 26 in `namespaces_gui.rs`, 13 in `render.rs`. Every one
is pre-existing.

**Measured evidence.** The per-site list moved **twice** in eleven milestones:
down one when M5 replaced a manual suffix strip, and up one when M9 added a hex
literal with uneven digit groups — caught by the manual comparison, fixed, and
back to 180. So the number of times a real regression was caught by the manual
discipline is one, and it would have been caught by a gate instead.

**Alternatives.** A: **`-D warnings` now** — fails until all 180 are fixed, which
is a large mechanical change touching files no milestone has otherwise needed to
open. B: **baseline them** with `#[allow]` at each site and turn the gate on —
the gate works immediately and 180 `allow`s are their own debt. C: **`-D warnings`
for new code only**, by comparing the per-site list in CI as this roadmap did by
hand — no cleanup needed, and it needs a committed baseline file. D: **leave it**
and keep the manual comparison.

**Trade-offs.** A is the clean end state and the largest immediate cost. C is what
this roadmap actually did, and automating it turns a discipline into a gate, which
is the whole difference. B trades one kind of noise for another.

**Architectural impact.** None. **Semantic impact.** None.

**Impact by area.** CI: one step. Source: none under C or D, 180 sites under A.

**Recommendation — a recommendation, not a decision.** **C**, then **A** when
someone wants to spend the afternoon. C is the option that makes the property
already being enforced by hand enforced by the build, which is exactly the
transition M10 is about — and §5.26's snapshot command is already written down.

**Blocked by this decision:** nothing; the manual comparison works and is
documented.

---

### DEC-M4-006 — Should the LSP report the fatal semantic phase?

**Problem.** `sz` rejects `class Task { … }` with a fatal `SZ8000` and exit 1.
`sz-lsp` publishes nothing. `lsp::analysis::analyze` runs the lexer, the parser
and the type checker, and not `semantic::validate` — `9d91f3c` wired the phase
into `run::run_source_detailed` and nothing else.

**Current behaviour.** As above, verified against both release binaries over real
JSON-RPC. §5.41 has the measurement. It widens with every rule the phase gains:
§5.39's duplicate-declaration and unresolvable-parent rules are invisible in an
editor too.

**Measured evidence.** Every program the semantic phase rejects — today the
reserved-name rule plus §5.39's two — is a program the editor stays silent about.
Not a sample: the phase has no LSP consumer at all.

**Alternatives.** A: **run the phase in `analyze` and publish its findings** as
severity 1, alongside parser and checker diagnostics. B: run it and *also* skip
the type checker when it reports, mirroring `run.rs`, so the editor shows what
the compiler shows and nothing that would be noise. C: leave it, and say in
`spec/` that the editor reports syntax and types only.

**Trade-offs.** A is the smallest change and makes the editor agree with the
compiler about which programs are rejected. B is more faithful — `run.rs` skips
the checker precisely so a rejected program does not reach a stage whose findings
may be ignored — but an editor that silently drops type diagnostics when a
semantic one appears is a different UX question. C is honest and permanently
weaker.

**Architectural impact.** None: `semantic::validate` is in the library and
`analyze` already holds the parsed program. **Semantic impact.** None — the
editor reports, it does not run. **Compatibility.** Editor-only; no program's
behaviour changes. **Impact by area.** LSP: `analysis::analyze` and its tests.
Specs: `errors.md`'s phase table. Runtime: none.

**Recommendation — a recommendation, not a decision.** **B**, because the
editor's job is to show what the compiler will do, and `run.rs` already decided
that a semantically-rejected program's type findings are noise. Note the
interaction with DEC-M4-004, which owns what the outline shows: this one is about
diagnostics and they can be decided independently.

**Blocked by this decision:** nothing. Pinned by
`the_semantic_phase_does_not_yet_reach_the_editor`.

#### Re-evaluated 2026-09-04, and still required

The Core-defects pass asked whether this decision was still necessary once the
resolver's false positives and negatives were addressed. They were — §5.50 and
§5.51 — and it is.

**What changed.** The reason severity was fraught was that nobody knew how often
the phase was wrong. Now:

  * §5.51's declaration-order rule was measured against the whole 508-file corpus
    and all eight ecosystem packages with **zero** false positives, and carries
    nine negative controls for the forward references that legitimately work.
  * §5.50's import resolution — the one remaining source of new rejections — is
    **off by default**, so `lsp::analysis::analyze`, which passes no directory,
    sees exactly what `sz file.sz` sees. Neither more nor less.

So "would the editor report things the compiler does not" is answered: no.

**What has not changed**, and is the whole decision: **A, B or C**. B silently
drops type diagnostics when a semantic one appears, mirroring `run.rs`. That is a
product judgement about what an editor should show, and no measurement settles
it. Severity is the smaller half of it.

**What widened.** The gap is not a property of one rule — `analyze` never calls
`semantic::validate` at all, so it grows with every rule the phase gains. It had
one rule when this entry was written and has five now. The pin was widened to
cover all of them, each with a positive control proving the rule fires in the
same process through the same library, plus a control proving the editor is not
simply silent.

| The phase rejects | The editor says |
|---|---|
| a reserved namespace name | nothing |
| a duplicate declaration | nothing |
| an unresolvable parent | nothing |
| a name declared nowhere | nothing |
| a name used before it is declared | nothing |

**Explicitly blocked**, as the finding asked: this stays open, and nothing about
what the editor publishes changes until it is answered.

---

### DEC-M4-007 — Should the semantic phase resolve through `import`?

**Problem.** §5.39's unresolvable-parent rule reports nothing for a file that
contains any `import`, because the phase resolves one file at a time and an
imported module may legitimately declare the parent. So `import "./real"; class
Child : NeverDeclared {}` is not caught, even when the import supplies nothing of
the sort.

**Current behaviour.** Conservative and silent, following
`semantic::scopes`'s own stated bias — every ambiguity resolves toward *treat it
as bound*, and a file with imports is `is_conclusive() == false`.

**Measured evidence.** **369** of the ecosystem's `class X : Y` sites are in files
that import, so the suppression is doing almost all the work there; in the
515-file corpus it costs nothing, because 58 of 59 top-level parents resolve
locally.

**Alternatives.** A: **resolve imports at check time** — `modules::resolve` +
`load_source` + parse, transitively, collecting each module's top-level
declarations. B: **leave it**, and document that the rule is per file. C: resolve
only when the file's imports are all local `.sz` paths, refusing to judge when a
URL import or a `.szx` translation is involved.

**Trade-offs.** A is the only option that catches the case, and it makes the
semantic phase read the filesystem — during `--check`, on every run, transitively
— and forces an answer for `--eval` **lockdown**, where `import` is refused at run
time and reading modules at check time would be a capability leak. It also needs
a cycle guard and a cost budget. B keeps the phase cheap, pure and lockdown-safe,
and leaves a real class of defect uncaught. C is A's cost with a narrower reach.

**Architectural impact.** A gives `semantic` a dependency on `modules` and, for
`.szx`, on spawning the translator — from a phase that is currently a pure
function of one AST. **Semantic impact.** A rejects more programs.
**Compatibility.** A is breaking for programs whose imports do not in fact supply
a declared parent. **Impact by area.** Runtime: none. Specs: `classes.md`'s table
of what is and is not reported. Tests:
`an_import_that_does_not_supply_the_parent_is_still_not_reported` inverts under A.

**Recommendation — a recommendation, not a decision.** **B for now**, and A only
with an explicit answer for lockdown and a measured cost. The phase being a pure
function of one tree is worth more than the marginal reach, and the uncaught case
still fails at instantiation the way it always did.

**Blocked by this decision:** the reach of §5.39's parent rule, and the free-name
rule's reach into files that import — see §5.50.

#### Update, 2026-09-03 — A is implemented, measured, and switched off

The two things this entry said A needed before it could be taken now exist. The
decision is still open; what has changed is that taking it is one flag.

**The lockdown answer.** The phase reads **nothing** under lockdown, whatever the
caller asks. `import` is refused there at run time, so reading module files to
analyse a locked-down program would be exactly the capability leak this entry
named. `lockdown_never_reads_modules` pins it.

**The measured cost.** Turning A on rejects **one** corpus fixture and **one**
pinned ecosystem package, and neither is a false positive:

| What | Why it is rejected | Is it right? |
|---|---|---|
| `tests/unit_modules.sz` | names `mod_exports_secret` on purpose, to observe the `SZ4001` a non-exported name raises | yes — the name genuinely is not declared |
| `serez-apipack` `sec_apipack.sz` | writes `"{ name: inner }"`, which Serez reads as **string interpolation** of an undeclared `name` | yes — and it is a real bug in that package |

The second is worth reading twice. The author meant a literal JSON-ish string;
Serez interpolated it, and `name` resolved *dynamically* to the enclosing
`test(string name, ...)` parameter, so the test passed for a reason that has
nothing to do with what it asserts. Measured directly:

```text
$ sz interp.sz          # let s = "{ name: inner }";
❌ ERROR [SZ4001]: Variable not found: name
```

That is `MATURITY_AUDIT.md`'s standing **critical** entry — "free variables
resolve dynamically" — showing up in shipped code. A widens the reach of a rule
that already rejects this pattern in any file without an `import`; it does not
invent one.

**The scope this does not settle.** Because the canary is pinned, taking A means
either fixing `serez-apipack` and moving its pin in its own commit — which the
pins file requires — or accepting a red gate. That sequencing is part of the
decision and is not started here.

**What exists now.** `src/semantic/imports.rs` resolves an import graph the way
`eval_import` does: exports win where there are any, everything is visible where
there are none, nested imports survive either way, cycles terminate on a
mark-before-read, and five kinds of unreadable module (URL, missing, unreadable,
unparseable, `.szx`) make the file *unresolved* rather than producing a
diagnostic. 13 tests in `tests/semantic_imports.rs`, and
`RunOpts::resolve_imports` — `false` at every call site — is the switch.

---

### DEC-M4-008 — Should a `class` and an `interface` share a name?

**Problem.** §5.39's duplicate rule is per *kind*: two classes collide, two
interfaces collide, a class and an interface do not. They live in separate
registries and both declarations are accepted.

**Current behaviour.** Both are accepted, and `new Name(...)` consults the
**interface** regardless of which was declared last, so the class becomes
unreachable. `spec/classes.md` already documents this under "A class and an
interface cannot share a name" as a hazard.

**Measured evidence.** **0** cross-kind collisions across 1,070 corpus and
ecosystem files. Nothing depends on the answer in either direction.

**Alternatives.** A: **make it an error**, folding it into the duplicate rule. B:
**keep it**, and keep documenting the shadowing hazard. C: keep it but **warn**,
which needs the advisory channel DEC-M5-003 is about.

**Trade-offs.** A is what the reader of `class Shape` / `interface Shape` almost
certainly wants, and it is free today. But the two are genuinely different
namespaces at run time, and making them one is a statement about the language's
name model rather than a bug fix — the same statement DEC-M4-003 is about for
namespaces, and it should probably be made once for both.

**Architectural impact.** None; the check is already there and keyed on
`DeclKind`. **Semantic impact.** A rejects programs. **Compatibility.** Breaking
in principle, with 0 measured victims. **Impact by area.** Specs:
`classes.md`'s existing hazard paragraph becomes a rule. Tests:
`two_kinds_of_declaration_sharing_a_name_are_not_reported` inverts under A.

**Recommendation — a recommendation, not a decision.** **A**, decided together
with DEC-M4-003, so the language answers "what shares a namespace with what" once
rather than twice.

**Blocked by this decision:** nothing. Pinned by
`two_kinds_of_declaration_sharing_a_name_are_not_reported`.

---

### DEC-M4-009 — Should the outline fall back to a token scan when the file does not parse?

**Problem.** DEC-M4-004 made the `.sz` outline come from the parse tree, which can
only show what parsed. While a user is typing, the file usually does not.

**Current behaviour.** The tree is the source of truth whenever there is one, and
`scan_symbols` — kept for `.szx` — is used when the parser reported an error. On a
file that parses, which is every corpus file and every file a user is not
mid-keystroke in, it never runs.

**Measured evidence.** `fn int suma(…) { … }` followed by `if (true {` and then
`let z = 1;` recovers far enough to keep `suma` in the tree and loses `z`
entirely; the token scan keeps both. `lsp::analysis::tests::symbols_survive_parse_errors`
is that program, and it has been in the suite since before M4.

**Alternatives.** A: **keep the fallback**, as now — nothing is lost, and the
tree is authoritative whenever it exists. B: **remove it**, so the outline is
always the tree and a broken file has a shrinking outline until it parses again.
C: **improve parser recovery** so the tree keeps more of a broken file, which
would narrow the gap for both.

**Trade-offs.** A keeps two derivations in the file, which is what DEC-M4-004
set out to remove — though only on input where the first has nothing to say. B is
the clean end state and takes something away from users on the input where an
outline is most useful. C is the real answer and is a parser project.

**Architectural impact.** A leaves `scan_symbols` reachable from `.sz`. **Semantic
impact.** None; this is tooling. **Compatibility.** Editor behaviour only.
**Impact by area.** LSP only.

**Recommendation — a recommendation, not a decision.** **A** until C is done. The
guarantee DEC-M4-004 asked for — that a `.sz` outline agrees with the compiler —
holds for every file that compiles, and `semantic_divergence` asserts it over 466
of them. Extending that to files that do not compile is a promise the tree cannot
keep.

**Blocked by this decision:** nothing. Pinned by `symbols_survive_parse_errors`.

---

### DEC-M10-003 — Should any phase timing block a build?

*Also referred to as **DEC-PENDING-PERFORMANCE-GATE** in the request that raised
it.*

**Problem.** `tests/perf_budget.rs` measures five phases against a committed
baseline and never fails on a timing. That was the right call while nobody knew
what the runners did; the question it deferred is whether it stays that way.

**Current behaviour.** Advisory. A phase more than `BUDGET` (1.5×) its baseline
is printed loudly and the test passes. It fails on exactly two things, neither a
timing: a missing baseline and a malformed one.

**Measured evidence.** The measurement itself had to be fixed before the
question could be asked honestly — see §5.54. Each phase used to run 15 times in
a row with no warmup, so whatever else the machine did landed on one phase
entirely. With three warmup rounds discarded and the phases **interleaved**, one
round running each once:

| | before (`max/min`, consecutive) | after (`max/median`, interleaved) |
|---|---|---|
| spread across phases | 1.8× – 3.6× | **1.1× – 1.6×** |

And the number that decides gateability — how much a phase's *ratio to baseline*
moves between three consecutive runs on an idle `windows/x86_64` machine:

| Phase | ratios over three runs | swing | `max/median` |
|---|---|---|---|
| `runtime.execute` | 1.08, 1.10, 1.09 | **0.02** | 1.11 – 1.40 |
| `types.check` | 0.99, 1.06, 1.00 | 0.07 | 1.41 – 1.90 |
| `frontend.parse` | 1.02, 1.02, 1.07 | 0.05 | 1.27 – 1.65 |
| `semantic.validate` | 0.93, 1.02, 1.13 | 0.20 | 1.50 – 3.12 |
| `semantic.declarations` | 0.91, 1.12, 1.03 | 0.21 | 1.39 – 1.74 |

`runtime.execute` is both the largest absolute number and by far the steadiest:
a 2% swing against a 50% budget. The two sub-100 µs phases swing 20%, which is
where a 1.5× budget starts to look like a coin toss on a busy runner.

This is one machine. CI runs three operating systems and the same table is
printed on each, which is the point of collecting it on every run.

**Option A — keep every phase advisory.** One rule, no flakiness, and the
numbers stay a prompt to look rather than a verdict. It also means a real 40%
regression in `runtime.execute` merges with a warning nobody reads.

**Option B — block on the phases whose variance supports it, per runner.** On
this evidence that is `runtime.execute` first, and possibly `frontend.parse`,
with a per-OS baseline so a Linux runner is not compared against a Windows
recording. The two sub-100 µs phases stay advisory. It costs a baseline file per
runner, a decision about who refreshes them, and the first flaky failure will
still be argued about.

**Trade-offs.** A is what exists and asks nothing of anyone. B turns the one
measurement that is stable enough to mean something into a gate, and its cost is
entirely in the per-runner baselines — three files that must be refreshed
together, or a gate that fires on whichever platform drifted first.

**Architectural impact.** B needs the baseline keyed by machine; the file already
records which one produced it, which is the half that was missing.
**Semantic impact.** None. **Compatibility.** None; this is CI.
**Impact by area.** `perf-baseline.txt`, `.github/workflows/ci.yml`.

**Recommendation — a recommendation, not a decision.** **B, for
`runtime.execute` only**, at a budget no tighter than 1.3×, with a baseline per
runner OS. It is the phase a user actually experiences, it is the steadiest by an
order of magnitude, and gating one phase that means something beats gating five
that half do. Everything else stays advisory until its own numbers say otherwise.

**Blocked by this decision:** nothing. The timings run on every CI push and print
the evidence either way.

---

### DEC-M9-002 — Are package, manifest and lockfile one recoverable transaction?

*Also referred to as **DEC-PENDING-PACKAGE-TRANSACTION** in the request that
raised it.*

**Problem.** Installing a dependency writes three things: the package tree under
`packages/`, a dependency line in `serez.json`, and an integrity line in
`serez.lock`. Only the first is transactional. The other two are written
afterwards, individually, and a failure between them is reported as a warning on
an install that has already said it succeeded. `install_all` compounds this: it
loops over dependencies and writes the lockfile once per package, so a failure at
package *N* leaves entries for 1..*N*-1 committed and the rest absent.

Whether that is a defect or the intended design cannot be settled by reading the
code, because two coherent designs disagree about it.

**Current behaviour.** `install_package` (`src/package_manager.rs`) drives
`package_install::Transaction`, which is atomic **for the package tree**: staging
directory, digest, verify against the lockfile if an entry exists, then commit by
rename. After `commit()` returns, and outside any transaction:

```rust
if !global {
    if let Some(dir) = &project {
        if manifest == ManifestPolicy::Record {
            if let Err(e) = record_dependency(dir, &pkg_name, &version) { eprintln!("⚠ ...") }
        }
        lock.upsert(...);
        if let Err(e) = lock.write(dir) { eprintln!("⚠ ...") }
    }
}
```

Both failures print `⚠ Installed, but could not update …` and the process still
exits 0.

**Measured evidence.**

  * A bare `sz install` in a project whose one dependency comes from
    `serez.json` produced the package and **no lockfile at all** — the defect
    fixed in this cycle, and the reason the transaction question surfaced.
  * The package tree survives a refused install intact:
    `a_recorded_digest_refuses_a_changed_package` tampers with the registry after
    a successful install, and asserts both that the second install exits non-zero
    and that `packages/test-pkg/index.sz` is still the original.
  * The three-write sequence is **not** covered by that: no test asserts what
    `serez.json` and `serez.lock` contain after a failure between them, because
    the current design has no defined answer.

**Option A — one recoverable transaction.** Package, manifest and lockfile commit
together or not at all. An install that cannot write the lockfile rolls the
package back and exits non-zero; `install_all` commits one lockfile at the end
rather than one per package.

**Option B — the package store is authoritative.** `packages/` is the truth, and
the manifest and lockfile are derived records that may lag. A failed write is a
warning, the install stands, and a later command reconstructs the records from
what is on disk.

**Trade-offs.** A gives a project that is either fully installed or untouched,
which is what a CI cache and a reproducible build want; it costs a rollback path
for the manifest and a second failure mode (rollback itself failing), and it
turns a warning into a hard failure, which is a public behaviour change. B is
what the code does today and never destroys a working install to satisfy
bookkeeping; it costs the guarantee that the lockfile describes what is on disk,
which is the guarantee the lockfile exists to provide — and B is only honest if
the reconstruction command exists, which it does not.

**Architectural impact.** A extends `package_install::Transaction` to cover files
outside the package tree, which is a widening of its contract. B needs a new
`sz install --repair` or equivalent. **Semantic impact.** None on the language.
**Compatibility.** A changes exit codes on a path that currently exits 0.
**Impact by area.** Package manager, CI, the `55_packages_e2e` corpus.

**Recommendation — a recommendation, not a decision.** **A**, scoped to
`install_all` first: one lockfile write after the loop, rather than one per
package. The per-package write is the part with no defender — it produces a
lockfile that describes a partial install, which is worse than either whole
answer. The manifest rollback is the expensive half of A and can follow.

### DEC-M9-003 — What ceiling does `OS.exec` output have, and what happens at it?

*Also referred to as **DEC-PENDING-OS-EXEC-LIMIT** in the request that raised it.*

**Problem.** `OS.exec` uses `Command::output()`, which reads the child's stdout
and stderr to EOF into two `Vec<u8>` with no ceiling. The size is chosen by the
child, and a Serez program calling `git log`, a build tool or anything fed from
the network does not know it in advance. DEC-M9-001 gave the other three
unbounded reads a 64 MiB fatal ceiling; this one was not in its scope and still
has none.

**Current behaviour.** The whole of both streams is captured and returned as two
Serez strings on an `ExecResult`. There is no truncation, no error and no
warning at any size.

**Measured evidence.** Peak working set of `sz` against a child emitting a fixed
number of bytes on stdout, release build:

| Child output | `OS.exec` peak | `OS.spawn` peak (stderr, bounded) |
|---|---|---|
| a few bytes | 9.3 MiB | — |
| 16 MiB | 56.3 MiB | — |
| **200 MiB** | **1,009.6 MiB**, exit 0, 6.4 s | **9.4 MiB** |

About **5× the child's output**, resident, and it succeeds: `r.stdout.length()`
returned 209,715,200. The amplification is the raw `Vec` plus
`String::from_utf8_lossy(..).to_string()`'s copy plus the `ObjectData::Str`, on
top of `read_to_end`'s doubling. The last column is the same 200 MiB through the
path DEC-M9-001 already bounded, for contrast.

**Reachability, measured rather than assumed.** `OS.exec` needs both the `OS`
permission and an `unsafe` block. Under lockdown `use permissions` is refused
outright:

```text
❌ ERROR [SZ6004]: `use permissions` is not available here — this code is running
without permissions and cannot grant itself any.
```

so untrusted source cannot reach it. A locked-down `Task` worker inherits its
parent's grants, but a locked-down parent cannot have granted `OS` in the first
place; only a **host** that calls `set_permissions` on a locked-down evaluator
can produce that combination. This is therefore a resource risk to a program the
user ran deliberately, not an untrusted-input vector like the three reads
DEC-M9-001 closed. It is still real: the *size* is the child's choice even when
the *command* is the author's.

**Option A — one fixed fatal ceiling.** 64 MiB, like DEC-M9-001, reusing
`read_bounded` and `OVER_THE_READ_CEILING`. One policy across four reads instead
of two. A child that legitimately emits more becomes unusable through `OS.exec`.

**Option B — bounded capture with a defined outcome short of fatal.** Keep the
first *N* bytes, mark the result truncated, and let the program decide — an extra
field on `ExecResult`, or spooling the overflow to a file. Nothing is lost that
the program cares about, at the cost of a wider public type and a second notion
of "the output" for callers that ignore the flag.

**Option C — host-configurable with a safe default.** The shape generator
accumulation already uses: a default the host may change through `RunOpts`, which
a running program cannot raise. Fits the existing precedent and adds another
knob.

**Trade-offs.** A is the smallest change and the most consistent, and it is the
one that breaks a working program. B keeps every current use working and changes
a public type, which is the thing this cycle is otherwise avoiding. C postpones
the argument by making it configuration, which is honest for a host and useless
for a program that hits the default.

**Architectural impact.** A and C need a bounded *two-pipe* reader, not
`read_bounded`: `Command::output()` drains both streams concurrently, and reading
one at a time deadlocks when the other's pipe fills — the bug already fixed once
for `OS.spawn`. That is the only new machinery any option needs, and it is not
written here, because writing it would be implementing the decision.

**Semantic impact.** A makes a currently-successful call fatal. B changes the
shape of `ExecResult`. C changes nothing by default and everything for a host.
**Compatibility.** A and B are both observable changes to a public API.
**Impact by area.** `OS.exec` only; `spec/limits.md`; the `unit_os_namespaces`
corpus.

**Recommendation — a recommendation, not a decision.** **A**, at 64 MiB. Four
unbounded reads with one answer is worth more than a fourth answer tuned to this
one, and B's truncation flag is a field every existing caller will ignore, which
makes silent truncation the common case — worse than a loud failure. A program
that genuinely needs 200 MiB from a child wants a file or `OS.spawn`, not a
string.

**Blocked by this decision:** nothing. The risk is documented in
`spec/limits.md` and the current behaviour is pinned by
`os_exec_output_is_currently_unbounded` in `tests/read_ceiling.rs`, which fails
the day the contract changes — so the change has to be deliberate.

## RESOLUTION — **DECIDED 2026-09-04: the ceiling is a guarantee `unsafe` waives.**

Not A, B or C as written. The owner's answer reframes the question: a limit the
runtime enforces is mandatory *while the runtime's guarantees are in force*, and
`unsafe { }` is where an author accepts named relaxations. The process output
ceiling is one of those, and the only one.

So the ceiling **exists**, at 64 MiB, and `unsafe` waives it. Because `OS.exec`
requires `unsafe`, every call today takes the waived path — which is the same
observable behaviour as before and, for the first time, a stated contract rather
than an omission.

Implemented in `execution::Guarantee::ProcessOutputCeiling` and
`evaluator::run_child_bounded`, a bounded two-pipe drain: reading one stream to
EOF before the other deadlocks when the child fills the second, which is the bug
already fixed once for `OS.spawn`. The unwaived path is tested at its boundary —
exactly 64 MiB captured, 64 MiB + 1 refused, on both streams — in
`evaluator::child_output_tests`, because there is no safe route to a child
process to test it through.

---

**Blocked by this decision:** nothing. Writing the lockfile on every successful
local install is correct under both: under A it is part of the transaction, and
under B it is the derived record being kept in step. That fix is implemented and
tested. What waits is the failure-path behaviour, which has no test because it
has no defined answer.

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

### §9F.7 — M4.5.2-M4.5.3: the phase exists, and reports nothing

`src/semantic/validate.rs`, wired into `run::run_source_detailed` between the
parser and the type checker. It has **no rules**, and that is the molecule.

**Introducing the stage and introducing a rule are separate changes**, because
only the first can be proved invisible. A phase that arrived with its first rule
attached would have moved manifests for two reasons at once, and no one could
have said which.

**Where it sits, and why each side of it is forced:**

  * **After the parser**, because it needs a *complete tree*. That is the entire
    point: the reserved-name cascade (§5.32) exists because the parser abandons a
    half-built declaration, and a phase that sees finished nodes cannot.
  * **Before the type checker**, because this phase is **fatal** and the checker
    is not. `spec/types.md` makes checker findings advisory, so a program rejected
    on meaning must not reach a stage whose findings may be ignored.
  * **Only on a tree the parser accepted.** Validating a broken tree reports
    consequences of the syntax error rather than problems of its own.

**Findings are returned, not printed.** `validate` is a pure function over the
tree; `run.rs` renders. That is M3's data/rendering split, and it differs from the
parser, which prints eagerly at the producer. The consequence for ordering is
benign: semantic findings print after every parser diagnostic, which is the phase
order, and D6 (§9D.7) governs nothing beyond the lexer/parser interleaving it was
about.

**M4.5.3 — proving the net sees it.** `validate` was perturbed to report once,
unconditionally:

```
135 of 156 error fixtures now produce different output
❌ PARSER ERROR [SZ2000] [tests/unit_classes.sz 1:1]: perturbation…
❌ Aborted: fix the errors above before running.
```

So the wiring reaches rendering, the abort path works, and `diagnostic_render`
sees it. Reverted immediately. This is the M1.0.2 method, and it matters more than
usual here: **a no-op is indistinguishable from a component that is not connected
at all**, and every gate stays green in both cases.

**Behaviour: unchanged, measured.** No manifest row moved. 447 Rust tests,
501/0/0 in both runners, ecosystem 8/8, clippy per-site list 180.

**What is deliberately still missing:** the phase's public surface — the rendered
label and the diagnostic code. The perturbation borrowed `PARSER`/`SZ2000` because
it was temporary. Choosing the real ones is **DEC-M4-005**, and M4.5.4 cannot
proceed without it.

### §9F.8 — M4.5.4-M4.5.6: the rule moves, and M4.5 closes

`is_reserved_name` is gone from the parser. `semantic::validate` rejects a
`class`, `interface` or `enum` named after one of the seven reserved namespaces,
as `SZ8000` / `SEMANTIC` (DEC-M4-005).

M4.5.4 and M4.5.5 landed together because a half-state is meaningless: while the
parser still rejects, the phase never runs, so adding the rule without removing
the old one produces a dead rule and no observable change.

**Measured, and exactly what DEC-M4-001 predicted: two manifest rows, one
fixture.**

| Manifest | Before | After | What the delta says |
|---|---|---|---|
| `diagnostic_render` | `1 · 222 bytes` | `1 · 212 bytes` | exit code unchanged at `1`; the rendered text changed |
| `parser_ast` | `26 · 1 diagnostic` | `245 · 0 diagnostics` | **the class now enters the AST** — tree 26 -> 245 — and the parser reports nothing |

That second row is the change made visible: the parser no longer has an opinion
about this program, and the tree it produces is a real class declaration instead
of the wreckage of an abandoned one.

**§5.32 is fixed.** A rejected class *with a body* produced three diagnostics and
now produces one:

```
before:  ❌ PARSER ERROR … reserved system namespace
         ❌ PARSER ERROR … Unexpected token '}'
         ❌ PARSER ERROR … Unexpected token '}'
after:   ❌ SEMANTIC ERROR [SZ8000] … reserved system namespace
```

**A wiring mistake found and fixed inside the molecule.** The first version ran
the type checker before the semantic gate, so a program rejected on meaning would
still have been type-checked. No observable difference today — the checker does
not inspect classes — but it contradicts DEC-M4-001's own rule that such a program
"must not reach a stage whose findings may be ignored", and it would have grown
into misleading noise as the phase acquired rules. The checker is now skipped when
the semantic phase reports. It is deliberately **not** skipped when the *parser*
failed: that is pre-existing behaviour and a separate question.

**A gate caught something this molecule had not thought about.**
`tests/diagnostic_codes.rs` cross-checks `spec/errors.md`'s registry against codes
the binary actually emits, and failed with *"spec/errors.md lists codes this suite
does not pin: SZ8000"*. A case was added. Worth recording because the gate did the
thing gates are for — it noticed a promise made in a document and not yet kept by
anything executable.

### §5.40 — the caret moved from the name to the declaration — *diagnostic quality*, low (M4.5.4)

A cost of the move, recorded rather than absorbed:

```
before:  [2:7]   class Task {     after:  [2:1]   class Task {
                       ^                          ^
```

The parser pointed at the *name*, because it reported from the token it had just
read. The phase points at the *declaration*, because `ClassDeclaration` carries a
span for itself and none for its name.

Fixing it means adding a `name_span` to `ClassDeclaration`, `InterfaceDeclaration`
and `EnumDeclaration` — an AST change that would move `parser_ast.manifest` for
**every** class, interface and enum in the corpus, for one caret column. Out of
scope for a molecule whose contract was to move a rule, and recorded as a
candidate rather than done quietly.

Arguable in both directions: the message is about the declaration, and pointing at
the item is what several languages do. It is listed as a cost because it is a
change from what users saw, not because the new position is wrong.

### M4.5 — status

| Molecule | State |
|---|---|
| **M4.5.1** public surface | **done** — DEC-M4-005 |
| **M4.5.2** the phase, no-op | **done** — §9F.7 |
| **M4.5.3** prove the net sees it | **done** — §9F.7 |
| **M4.5.4** move the rule | **done** |
| **M4.5.5** delete the parser's copy | **done** |
| **M4.5.6** specs | **done** — `errors.md` (the phase, `SZ8xxx`, and `SZ3xxx` corrected to "type"), `classes.md` (the rule, stated for the first time), `cli.md` (the exit-code sentence), `compatibility.md` (the behavioural note) |

**M4.5 is complete.** The parser has no semantic rules left, and
`src/parser/classes.rs` says so at the top so the next one does not land there.

**Next in M4:** DEC-M4-003 now has its landing site — the name list lives in
`semantic::validate::RESERVED_NAMESPACES`, pinned at seven by a unit test that
names DEC-M4-003 as the only thing that may change it.

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

## 9L. M7 - Semantics Frozen

Charter: *"El comportamiento observable importante existe porque fue decidido, no
porque la implementacion casualmente lo hace."*

### 9L.0 The audit: the specs already decline to decide, in writing

M7's premise is that observable behaviour may be accidental. Searching `spec/` for
the language it uses when it is *not* making a design statement turns up the
complete list, and the specs are better than the premise assumed - every one of
these is documented, and documented **as undecided**:

| Where | Behaviour | The spec's own words |
|---|---|---|
| `spec/arrays.md` | `remove` on an empty array returns null | "Known inconsistency" |
| `spec/classes.md` | a subclass reaches an inherited private member | "recorded below rather than silently described as stronger than the implementation" |
| `spec/control-flow.md` | no exhaustiveness; no matching arm yields null | "recorded here as a hazard, not as a design statement" |
| `spec/operators.md`, `spec/values.md` | `==` compares containers by identity | "a documented inconsistency, not a design statement" |
| `spec/types.md` | no subtyping, no numeric widening | "recorded as an inconsistency, not defended" |

So M7 is not a discovery milestone. **The work was already done honestly; what is
missing is the decision.** That reframing is the audit's main result, and it is
the same shape M5 found: good documentation, absent decisions.

**The rest of the charter's list is specified and settled**, checked against
`spec/`: generator semantics (`control-flow.md`, `functions.md`, `limits.md`),
overflow (`operators.md`, `limits.md`, `errors.md`), module cycles
(`modules.md`), task cancellation (`tasks.md`), process semantics
(`processes.md`), filesystem and platform differences (`files.md`,
`processes.md`, `limits.md`), null (`values.md`, `types.md`).

**One gap the sweep found: handle/free has no specification at all.** There is no
`spec/memory.md`. Probed, the behaviour is *good* - a freed handle's id is never
reissued, and use-after-free is refused and catchable - and it is deliberate,
since `handles.rs` states the never-reissued rule. It is simply written nowhere a
user can read. **Classified as an M8 documentation item, not an M7 decision**,
because nothing about it needs deciding.

### 9L.1 - M7.1: six decisions registered

**DEC-M7-001** through **DEC-M7-006**, each with the field set §7A requires. Two
carry measurements taken here:

  * **DEC-M7-003** - **107** `match` expressions in the corpus, **50** with
    neither a wildcard nor a binding arm. The two ways to close this have very
    different exposure, and separating them is the point: a *runtime* error when
    no arm matches would affect **0** corpus sites today, while a *static*
    exhaustiveness requirement would touch all **50**. Conflating them is how the
    decision gets taken by accident.
  * **DEC-M7-001** - `remove` appears in **12** files; how many can be called on
    an empty array is not mechanically decidable and was not guessed at.

**DEC-M7-005 has no measurement, by construction:** the defect is that a pattern
which fails to evaluate produces no signal at all. Its real exposure is its
interaction with DEC-M7-003 - a typo falls to `_`, and with no `_`, to `null`.
**Two silent failures compose into a third**, which is why the recommendation puts
DEC-M7-005 first.

### 9L.2 - M7.2: `tests/frozen_semantics.rs`, so nothing moves before it is decided

The independent work M7 *can* do without deciding anything: pin the current
behaviour, so that when a decision lands, the diff shows it.

Every one of these is unpinned today in the way that matters. **The conformance
suite asserts what a program prints, and each of these is either silent or
produces a value no passing test looks at.** A refactor could flip any of them and
all 499 conformance tests would still pass. That is not hypothetical - it is the
same structural gap M5 found for checker findings on succeeding programs, and it
is why both needed a purpose-built net rather than more fixtures.

Seven pins: the five open M7 decisions, the numeric-equality triple that ties
DEC-M7-004 to DEC-M5-002, and the memory-handle safety property that has no spec.

They assert through the **language's own `assert`** rather than captured output,
because `out` writes with `println!` and there is no capture hook. A failed
assertion raises, the outcome stops being `ProgramOutcome::Value`, and the test
fails - which also makes each fixture a readable statement of what is pinned, in
Serez, rather than a claim only an evaluator author can check.

Verified load-bearing before being trusted: one pin was perturbed, the harness
failed and named the decision it belongs to, and the perturbation was reverted.
The failure message says *do not edit this test to match* and points at §7A,
because the failure mode this file exists to prevent is a decision being taken by
someone making a red test green.

---

## 9N. M8 - Conformance Complete

Charter: *"Poder demostrar automaticamente que la implementacion cumple la
specification."*

### 9N.0 The audit: two good things, unconnected

30 specification documents, 4,672 lines, and **zero normative identifiers** -
§5.8 predicted this and it held. The documents reference tests in prose
(`classes.md` names 21, `modules.md` 12, `values.md` and `syntax.md` none), but a
prose mention is not a mapping. Nothing in the repository could answer *"which
test proves this sentence?"* or, more usefully, *"which sentences does nothing
prove?"*

So M8's problem is not that the spec is bad or that tests are missing. It is that
`spec/` and `tests/` are two good artefacts with no edge between them.

### 9N.1 - M8.1: the scheme, and the checker that makes it real

`spec/conformance.md` defines it. A normative rule carries an identifier at its
definition site - `**[MEM-002]**` - and a test declares coverage with a
`conformance: MEM-002` marker in a comment, in either language.

The marker was chosen over a mapping table on purpose: it needs no build step,
works identically in `.sz` and `.rs`, and puts the claim **next to the assertion**
rather than in a file someone has to remember to update.

`tests/conformance_map.rs` enforces three properties, each failing for a different
reason:

1. **defined exactly once** - a duplicate is a numbering mistake, caught when it
   is made rather than the first time someone follows the wrong one;
2. **no claim to a rule that does not exist** - a stale marker *reads as coverage*,
   which is worse than no marker;
3. **every rule has at least one test** - the property worth having, because it
   makes it impossible to add a rule and forget to prove it.

Property 3 is also why identifiers are assigned **as an area is covered** rather
than all at once. An identifier is a commitment that something verifies the rule;
numbering an unproved rule would record the gap in a second place instead of
closing it. It is the difference between a coverage scheme and a coverage report.

**The checker caught itself on its first run.** `spec/conformance.md` illustrates
the scheme by *showing* a definition inside a code fence, and that example was
read as a real definition and reported as a duplicate of the rule it was
illustrating. Fixed by stripping fenced blocks - a specification that cannot show
its own syntax would be the wrong trade - and pinned by a unit test.

### 9N.2 - M8.2: `spec/memory.md`, the first area and the worked example

M7 handed over one documentation gap: memory handles had **no specification at
all**, while having a real safety property. Fifteen rules, `MEM-001` to `MEM-015`,
every one derived by probing the running binary rather than by reading source.

**Two things the probing found that source-reading had got wrong**, and both are
the argument for the method:

  * **MEM-007 was written as "catchable" and is not.** Exceeding the 256 MiB
    ceiling raises a **fatal** `ResourceError` / `SZ6002`; a `try` does not consume
    it and the program stops. The corrected rule now also states the asymmetry
    that makes the pair confusing: `alloc(0)` is a *caller mistake* and catchable,
    while exceeding the ceiling is a *resource limit* and fatal. A program can be
    written to handle the first and cannot be written to handle the second.
  * **§9L.2's memory pin was proving the wrong thing.** It used
    `Memory.write(handle, 0, 65)` - three arguments, where `write` takes four - so
    the "use-after-free is refused" assertion was actually observing an **arity**
    rejection. A change that removed use-after-free protection would not have
    failed it. Corrected to the 4-argument form, with a live-handle control
    alongside so it cannot pass for the wrong reason again.

That second one is worth naming plainly: it is **exactly the failure mode §5.34
found in three security fixtures**, committed by this roadmap, one milestone after
finding it in someone else's work. The lesson generalises past both instances - a
negative assertion needs a positive control, or it proves only that *something*
refused.

### 9N.3 What is covered, and what is not

| | |
|---|---|
| Specification documents | **32** (30 + `memory.md` + `conformance.md`) |
| Carrying identifiers | **1** |
| Normative rules defined | **15** |
| Rules proved | **15** - asserted, not reported |
| Test files claiming rules | 6 |

**One area of thirty.** That is the honest figure and it is the point of stating
it: the machinery is complete and the coverage is a beginning. Four of the six
claiming files are pre-existing `sec_memory_*` fixtures that already proved
MEM-004 and needed only a marker - reuse over duplication, as
`spec/conformance.md` prescribes.

**Suggested order for the rest**, by value rather than by size: `errors.md` and
`limits.md` first, because every other document refers to them and their rules are
already heavily tested; then `types.md`, `values.md` and `operators.md`, the
semantic core; then the namespaces. `syntax.md` and `lexical-grammar.md` last -
`parser_snapshot` already pins the grammar far more tightly than identifiers would.

---

## 9P. M9 - Robustness & Security Hardened

Charter: *hostile or unexpected input must not be able to destroy the language's
guarantees.*

### 9P.0 The audit: the ceilings exist, the generators do not

§6 already recorded the shape and it held: depth ceilings, string/crypto/tensor/
allocation caps, ZIP traversal checks, JSON-RPC body bounds and a panic-site
classification **all exist**. `spec/limits.md` is 187 lines of them.
`tests/frontend_robustness.rs` covers malformed input in 16 tests.

What does not exist is **fuzzing or property testing of any kind**. Everything
above is a list of cases somebody thought of, which is the right instrument for
predictable shapes and the wrong one for the shapes nobody predicted - which is
where crashes live.

There is also an **independent external audit** in
`audit/2026-09-01_14-52-03.md`, written by another session against 10.0.0 and
untouched by this roadmap until now. It raises four findings. M9 treats it as
evidence to verify rather than as conclusions to accept.

### 9P.1 - M9.1: the `OS.spawn` deadlock, confirmed and fixed

The audit's second finding, **verified**:

```
~600 KB written to stderr : never harvested, 200 polls over 10 seconds
3 lines written to stderr : harvested on the 2nd poll
```

Same command, same code path, only the volume differs - which isolates the pipe
buffer as the cause exactly as the audit predicted. The mechanism, confirmed by
reading `tick`: `try_wait()` is called *first*, and stderr is only read after the
child has exited. A child that fills the pipe blocks on the write, so it never
exits, so `try_wait` never reports it, so nothing ever drains the pipe. A program
polling `OS.tick()` for completion loops forever.

**Fixed** by draining stderr concurrently: a reader thread per child, started at
spawn, writing into an `Arc<Mutex<String>>` that `tick` reads after harvest. A
reader thread is the portable answer; non-blocking pipe reads are
platform-specific in three different ways. The thread ends when the pipe closes,
which is when the child exits.

**After the fix the same program harvests in 10 rounds.** The regression is
`unit_os_spawn.sz`, and the before/after measurement above is what makes it
load-bearing - a test that would have failed by *hanging* is not one that could
have been written first.

**This was a bug, not a policy question**, which is why it was fixed rather than
registered. What the child's stderr should be *capped* at is a policy question,
and that is **DEC-M9-001**.

### 9P.2 - M9.2: property testing, which did not exist

`tests/frontend_properties.rs`. Three properties, 1,021 generated inputs:

  * **P1** - the frontend never panics. A panic is not a diagnostic: no line
    number, no chosen exit code, nothing for the LSP to underline.
  * **P2** - every diagnostic points inside its own source. M2 spent a milestone
    giving nodes real spans; this keeps them honest on input M2 never saw.
  * **P3** - parsing is deterministic. Otherwise the frontend depends on something
    it should not, and every manifest in `tests/snapshots/` is a coin flip.

**No new dependency.** `proptest` and `arbitrary` are declined for the reason
`parser_snapshot` declines `DefaultHasher`: this repository keeps its test
infrastructure free of crates whose behaviour it does not control. The PRNG is
xorshift64* in ten lines, seeded by a constant, so **a failure reproduces by
re-running the test** rather than by copying a seed out of a log - the only
reproduction instruction anyone reliably follows.

**The generators that matter start from real source and damage it.** Random bytes
find shallow bugs quickly and then stop, because almost nothing random is
nearly-valid. A truncation is what a half-saved file looks like; a
single-character mutation is what a typo looks like. Both reach deep parser paths
that soup does not, and both are what the corpus makes possible.

**A fuzzer that rejects nothing tests nothing**, so that is asserted too: of 1,021
inputs, **688 are rejected**, producing **8,157 diagnostics**. Without that check,
P1-P3 could hold over a generator that emitted only valid programs and the test
would report success for having tried nothing.

**Result: all three properties hold.** No panic, no span outside its source, no
non-determinism. That is a real negative result rather than a vacuous one,
because the rejection count shows the error paths were exercised.

### 9P.3 What M9 did not do

  * **The audit's other three findings are not fixed.** `fetch`'s unbounded read
    and SSRF reach, HTTP `import`'s unbounded read, and the GUI's image-decode
    limits. The first two are **DEC-M9-001**; `fetch`'s reachability under
    lockdown is already **DEC-M7-006**; the GUI's `checked_mul` arithmetic is
    genuine and is the one item left neither fixed nor registered, because it
    needs a probe against a real decode path this session did not build.
  * **Property testing covers the frontend only.** The evaluator, the package
    manager's ZIP extraction, and the JSON and binary boundaries have none. The
    frontend went first because it is the only surface that takes arbitrary input
    by design.
  * **No resource-boundary sweep.** The charter asks for a per-resource table -
    can it grow without limit, should it, what error, fatal or recoverable,
    specified, tested. `spec/limits.md` answers much of it already; a
    reconciliation of that document against the code was not done.

---

## 9R. M10 - Stable Language Platform

Charter: *close the architecture — a comprehensible DAG, tooling that reuses real
components, and a compatibility and release story.*

### 9R.0 The audit: the description was right and unenforced

§3.1 describes the module graph in prose: *"mostly a clean DAG"*, a table of the
edges worth naming, and a sentence listing the inversions that are absent — no
`parser -> evaluator`, no `ast -> gui`, no `lexer -> package_manager`.

Checked: **the sentence is true.** None of those edges exists.

It is also unenforced. An architecture description a compiler never reads
describes the architecture someone *intended*, and it drifts one convenient `use
crate::` at a time. That is M10's actual problem: not that the shape is wrong, but
that nothing holds it.

### 9R.1 - M10.1: the DAG becomes a gate

`tests/architecture.rs` reads `src/` and asserts what §3.1 claims:

  * **A1** — no forbidden edge. Ten inversions, each with *what it would mean*
    written beside it, because a rule whose reason is not recorded gets deleted by
    whoever first finds it inconvenient.
  * **A2** — no cycle except the ones on record, with `KNOWN_CYCLES` checked for
    staleness in both directions, so a fixed cycle cannot stay recorded as
    permanent.

**Writing it wrong first is what made it useful.** The first version looked for
mutual pairs, found `run <-> szx` (§5.6, on record) and reported the graph
otherwise clean. A separate scan for longer cycles found
`evaluator -> szx -> run -> evaluator` — three edges §3.1 names individually
without naming the cycle they form. That is **§5.38**, and the pair-only checker
would have licensed precisely the claim this milestone exists to test.

Both cycles are now on record and a third would fail the build.

### 9R.2 What M10 checked and did not change

**Cross-platform parity: covered.** `.github/workflows/ci.yml` runs `fmt`,
`check`, `clippy` and **both** Serez runners on `ubuntu-latest`,
`windows-latest` and `macos-latest`. §6's concern here is met.

**Two CI gaps confirmed, both registered rather than closed:** the ecosystem
canary does not run in CI (**DEC-M10-001**) and clippy is not a gate
(**DEC-M10-002**). Both are decisions about what a release gate *is*, and both
have a recommendation with the measurement behind it — including the honest one
for clippy, that the manual per-site discipline caught exactly one real regression
in eleven milestones and a gate would have caught it instead.

**Tooling reuse: already true, and measured in earlier milestones.** M5.4 found
the LSP constructs the same `TypeChecker` the CLI does, so type findings cannot
diverge. M4 found the opposite for symbols — the LSP re-lexes and discards the
parse tree — which is **DEC-M4-004**. So the charter's *"LSP shares frontend"* is
half true, and which half is known.

**Not addressed:** the LLVM backend's parity is still unproven and it is still
absent from the CLI (`MATURITY_AUDIT.md`, high); there is still no benchmark
regression budget; §5.18's double compilation of the frontend stands; and §5.35's
`serez-ui` defect is unfixed in a repository this roadmap does not own.

---

## 9S. M10 MILESTONE AUDIT

Charter: *close the architecture — a comprehensible DAG, tooling reusing real
components, compatibility and release policy.*

| Item | Status | Evidence |
|---|---|---|
| The dependency DAG is comprehensible | **met** | §3.1's claims checked; all true |
| It is enforced, not described | **met** | `tests/architecture.rs`, and it found §5.38 |
| Cross-platform CI | **met** | 3 platforms, both runners, already in place |
| Tooling reuses real components | **partially met** | the LSP shares the type checker (M5.4) and re-derives symbols (M4) — DEC-M4-004 |
| Compiler and interpreter share semantics | **NOT met** | the LLVM backend is feature-gated, unwired, parity unproven |
| Release gates settled | **NOT met** | DEC-M10-001, DEC-M10-002 |

**Three of six. M10 is PARTIAL.**

The milestone's most useful output is not the gate but what the gate found: a
three-module cycle §3.1 described edge by edge without ever naming (§5.38), found
only because the first version of the checker was written to look for the shape
that was expected. That is recorded in §0A.F alongside its two siblings, because
the pattern outlives all three instances.

Gates at close: fmt **PASS** · check **PASS** · clippy **PASS**, per-site **180**
unchanged across the whole run · `cargo test --all-targets` **447 / 0** · both
Serez runners **501 / 0 / 0**, categories identical · ecosystem **8 / 8**.

---

## MILESTONE STATUS: **PARTIAL**

---

## 9Q. M9 MILESTONE AUDIT

Charter: *hostile or unexpected input must not be able to destroy the language's
guarantees.*

### Definition of Done, item by item

The plan's DoD: *untrusted Serez input should not produce a panic, crash or
corruption outside conditions explicitly classified as fatal host failure.*

| Item | Status | Evidence |
|---|---|---|
| Fuzzing / property testing exists | **met, for the frontend** | 1,021 generated inputs, 3 properties, 688 rejections |
| The frontend cannot be crashed by input | **met, as far as measured** | P1 holds across every generator |
| Diagnostics stay honest under hostile input | **met** | P2, which is what M2's spans are worth on input M2 never saw |
| The runtime cannot be crashed by input | **NOT met** | no property testing beyond the frontend |
| Resource boundaries audited per resource | **NOT met** | `spec/limits.md` answers much of it; not reconciled against the code |
| Known security findings addressed | **partially** | 1 of 4 fixed, 2 registered, 1 neither |

**Three and a half of six. M9 is PARTIAL.**

### 1. The one thing M9 proves, and its limit

It proves the **frontend** holds three properties on input nobody wrote — and the
rejection count is what makes that a real result rather than a vacuous one. 688 of
1,021 inputs were rejected and 8,157 diagnostics were produced, so the error paths
the properties are about were genuinely exercised.

What it does not prove is anything about the evaluator, the package manager's ZIP
extraction, or the JSON and binary boundaries. The frontend went first because it
is the only surface that takes arbitrary input **by design**; the others take it
by accident, which is a weaker reason to start there and not a reason to stop.

### 2. What the external audit was worth

`audit/2026-09-01_14-52-03.md` was written by another session and sat untouched
until M9. Treating it as evidence to verify rather than conclusions to accept was
the right call in both directions:

  * its `OS.spawn` finding was **correct**, and the verification produced the
    before/after measurement that makes the fix's regression test meaningful;
  * its remaining findings are real but are **policy**, not defects — and folding
    them into a hardening pass would have taken three decisions silently.

That split — one bug fixed, two registered, one left explicitly unaddressed — is
M9's actual output, and it is more honest than a milestone that "addressed the
audit".

### 3. The one item neither fixed nor registered

The GUI's image-decode path (`namespaces_gui.rs`, `render.rs`) does `i32`
arithmetic such as `req_w * nh` before reserving, which can overflow. It is
recorded here rather than registered because a decision record with no measured
evidence is a worse artefact than an honest gap: verifying it needs a probe
against a real decode path, and this session did not build one. **Assigned to
whoever continues M9**, with the audit entry as the starting point.

### 4. Semantic drift

One deliberate observable change, declared: a program whose child fills the stderr
pipe used to hang and now completes. Every other path is unchanged — the harvest
shape, the `[pid, code, errMsg]` triple, and the message content for children that
do not fill the pipe. No manifest row moved except for the new test fixture.

### 5. Gates at close

fmt **PASS** · check **PASS** · clippy **PASS**, per-site **180** unchanged ·
`cargo test --all-targets` **444 / 0** · both Serez runners **501 / 0 / 0**,
categories identical · ecosystem **8 / 8**.

### 6. What M9 hands forward

| To | What |
|---|---|
| Whoever continues M9 | property testing beyond the frontend; the GUI overflow probe; a per-resource reconciliation of `spec/limits.md` against the code |
| The decision owner | **DEC-M9-001** — one ceiling policy for three unbounded reads. **DEC-M7-006** is the other half of the `fetch` question |
| M8 | `spec/limits.md` is the document most worth identifiers next, and M9 is why |

### 7. Commits

`30e02e4`: `a child that fills the stderr pipe no longer hangs forever, and
the frontend gets property tests`.

---

## MILESTONE STATUS: **PARTIAL**

The frontend is measurably robust and the runtime is untested against generated
input. One real bug was found, verified and fixed; two policy questions were
registered rather than answered; one gap is named rather than papered over.

---

## 9O. M8 MILESTONE AUDIT

Charter: *demonstrate automatically that the implementation satisfies the
specification.*

### Definition of Done, item by item

The plan's DoD: *important stable rules have normative text, an identifier, and
automated evidence.*

| Item | Status | Evidence |
|---|---|---|
| A scheme for normative identifiers exists | **met** | `spec/conformance.md` |
| It is enforced, not conventional | **met** | `tests/conformance_map.rs`, three asserted properties |
| Reuse is preferred over duplication | **met** | 4 of 6 claiming files are pre-existing fixtures that needed only a marker |
| Important stable rules carry identifiers | **NOT met** | 1 area of 30 |

**Three of four. M8 is PARTIAL**, and the unmet item is unmet by *quantity*, not
by obstruction: nothing blocks covering the next area, it simply has not been
done. That is a different kind of PARTIAL from M4, M6 and M7, where a decision
stands in the way, and the distinction is worth keeping — this one closes with
work, those close with answers.

### 1. What the milestone actually built

The scheme, the checker, and one area proved end to end. The order matters: an
identifier scheme without enforcement is a naming convention, and a naming
convention decays the first time someone is in a hurry. Property 3 — *every rule
has a test* — is what makes it a gate rather than a habit.

### 2. What it found

Two defects, both in this roadmap's own recent work, and both of the same class:

  * **MEM-007 written from source-reading was wrong.** The 256 MiB ceiling raises
    a *fatal* error, not a catchable one. Caught because the conformance fixture
    was run rather than reasoned about.
  * **§9L.2's memory pin proved arity, not use-after-free** — a three-argument
    call to a four-argument function. It would have passed unchanged if
    use-after-free protection were removed.

The second is the same failure mode §5.34 found in three security fixtures, one
milestone earlier, in someone else's work. Committing it here — with the finding
still fresh — is the useful evidence: **the discipline that catches this is a
positive control, not attention.** A negative assertion with no positive control
proves that *something* refused, and nothing about what.

### 3. Semantic drift

None. No source file touched. Two spec documents, three test files, four markers
added to existing fixtures, and manifest rows for exactly the new fixtures.

### 4. Gates at close

fmt **PASS** · check **PASS** · clippy **PASS**, per-site **180** unchanged ·
`cargo test --all-targets` **442 / 0** · both Serez runners **501 / 0 / 0**,
categories identical · ecosystem **8 / 8**.

### 5. What M8 hands forward

| To | What |
|---|---|
| Whoever continues M8 | the coverage order in §9N.3: `errors.md` and `limits.md` first, the semantic core next, namespaces after, grammar last because `parser_snapshot` already pins it harder |
| M7 | its memory pin is corrected; the handle/free gap it handed over is now `spec/memory.md` |
| M10 | 29 areas without identifiers is a documentation-architecture fact, not only a testing one |

### 6. Commits

`1535a6e`: `normative identifiers, a checker that enforces them, and the
first area`.

---

## MILESTONE STATUS: **PARTIAL**

The machinery is complete and enforced; one area of thirty is covered. Unlike M4,
M6 and M7, nothing blocks the rest — it is work, not a decision, and §9N.3 says
what order to do it in.

---

## 9M. M7 MILESTONE AUDIT

Charter: *important observable behaviour should exist because it was decided, not
because the implementation happens to do it.*

### Definition of Done, item by item

| Item | Status | Evidence |
|---|---|---|
| Every behaviour on the charter's list probed and classified | **met** | §9L.0 - the sweep covers all 14 categories the plan names |
| Behaviour that is settled, is specified | **met** | generators, overflow, module cycles, task cancellation, process semantics, platform differences and null are each covered by a `spec/` document |
| Behaviour that is not settled, is decided | **NOT met** | six open decisions, DEC-M7-001 through -006 |
| Nothing important is undocumented | **nearly met** | one gap found: handle/free has no spec. Behaviour is correct and deliberate; assigned to M8 |
| Undecided behaviour cannot drift | **met** | `tests/frozen_semantics.rs`, 7 pins, verified load-bearing |

**Four of five. M7 is PARTIAL**, and the unmet item is the milestone's whole
subject: freezing semantics *is* deciding them.

### 1. What M7 found, and why it changes the milestone's shape

M7 expected to find accidental behaviour and to have to distinguish it from
intended behaviour. It found something better and more awkward: **`spec/` already
makes that distinction, explicitly, in its own words.** Five separate documents
say a version of "this is recorded as an inconsistency, not defended".

So there was no accidental semantics to discover. There is decided semantics,
which is specified, and undecided semantics, which is specified **as undecided**.
The milestone's remaining work is not engineering; it is six answers.

That is the same result M5 reached from a different direction, and the pattern is
worth naming for M8 through M10: **this project's documentation is ahead of its
decisions.** A roadmap that assumes the reverse will keep discovering that the
audit has already been done.

### 2. What M7 changed

No source file. One test file, seven pins, and six decision records. That is the
correct output for a milestone whose charter is to decide and whose decisions are
not its to take - and it is worth stating plainly rather than padding.

### 3. The one gap that is not a decision

Memory handles have no `spec/` document. Probed: a freed handle's id is never
reissued, and use-after-free is refused and catchable. Correct, deliberate
(`handles.rs` states the rule), and unwritten. **M8 owns writing it down**;
M7 pinned it so it cannot be optimised away by someone who does not know it is a
guarantee.

### 4. Semantic drift

None possible - no source changed. The point of the milestone is the opposite: the
seven pins mean that from here, drift in any of these six behaviours fails a test
that names the decision it belongs to.

### 5. Gates at close

fmt **PASS** · check **PASS** · clippy **PASS**, per-site list **180**, unchanged ·
`cargo test --all-targets` **440 / 0** · `run_tests.sh` **499 / 0 / 0** ·
`run_tests.ps1` **499 / 0 / 0** · ecosystem **8 / 8**.

### 6. What M7 hands forward

| To | What |
|---|---|
| The decision owner | DEC-M7-001…006, and the ordering constraint that DEC-M7-005 should precede DEC-M7-003, and DEC-M7-004 should ship with DEC-M5-005 |
| M8 | `spec/memory.md` does not exist; handle/free is unwritten |
| M9 | DEC-M7-006 (`fetch` under lockdown) is half of the `fetch` question; the response-size ceiling is the other half and is independent |
| Whoever answers DEC-M7-002 | the measurement wants a resolver, which is the same walk DEC-M4-002 needs - two decisions, one piece of infrastructure |

### 7. Commits

`2cc1d46` M7.1-M7.2 (six decisions registered, seven pins).

---

## MILESTONE STATUS: **PARTIAL**

Everything settled is specified; everything unsettled is registered, measured
where measurement was possible, and pinned so it cannot move before it is
answered. The milestone cannot close further without six answers, and taking them
to reach COMPLETE is exactly what the protocol forbids.

---

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
| **M6 checkpoint** | `c946d36` | `M6 closes PARTIAL — the state moved, the behaviour did not` |
| **M7 checkpoint** | `acd6054` | `M7 closes PARTIAL — the documentation was ahead of the decisions` |
| **M8 checkpoint** | `b1387ad` | `M8 closes PARTIAL — the machinery is complete, the coverage is one area` |
| M9.1-M9.2 | `30e02e4` | `a child that fills the stderr pipe no longer hangs forever, and the frontend gets property tests` — **behaviour change**, a hang becomes a completion |
| M8.1-M8.2 | `1535a6e` | `normative identifiers, a checker that enforces them, and the first area` |
| M7.1-M7.2 | `2cc1d46` | `pin six undecided behaviours so none of them moves by accident` |
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
