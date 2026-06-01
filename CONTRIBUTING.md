# Contributing to Serez-Code

Thank you for wanting to contribute. This document explains how to report issues, propose changes, and submit pull requests so that the process is clear to everyone.

---

## Reporting an Issue

Before opening an issue, please check if a similar one is already open or closed.

### Issue Title

The title should be clear and concise. Follow this format according to the type:

| Type | Format | Example |
|---|---|---|
| Bug | `[BUG] Short description of the problem` | `[BUG] flat(n) only flattens one level` |
| Feature request | `[FEATURE] What you want to add` | `[FEATURE] Negative modulo operator` |
| Documentation | `[DOCS] What is missing or wrong` | `[DOCS] Incorrect Set.union example` |
| Question | `[QUESTION] Your question` | `[QUESTION] How does Flash Scope work` |

### Issue Body

**For bugs**, include:
- What you did (minimal `.sz` code that reproduces the problem)
- What you expected to happen
- What actually happened (error message or incorrect output)
- `sz` version (`sz --version`)
- Operating system

**For features**, include:
- What you want to add and what it is useful for
- Example of how the syntax or behavior would look
- If you already have an idea of how to implement it, mention it

---

## Proposing a Change (before writing code)

For large changes — new syntax, changes to the evaluator, modifications to the memory model — open an issue first and describe what you want to do. This prevents you from working on something that won't be merged.

For small changes (fixing a typo in docs, correcting a simple bug), you can go straight to a PR.

---

## Workflow

### 1. Fork and Clone

```bash
git clone https://github.com/<your-username>/serez-code
cd serez-code
cargo build
```

### 2. Branch Naming

The branch should exactly describe what it implements. Use the issue number if it exists:

```
# With associated issue:
feature/123-do-while-loop
fix/87-flat-depth-parameter
docs/45-update-set-examples

# Without issue (direct contribution):
feature/string-repeat-method
fix/parser-error-recovery-semicolon
docs/add-closures-example
```

**Valid prefixes:**

| Prefix | When to use it |
|---|---|
| `feature/` | New language feature or tooling |
| `fix/` | Bug fix |
| `docs/` | Documentation only |
| `refactor/` | Internal change without behavioral change |
| `test/` | Adding or fixing tests |
| `ci/` | Changes to the CI/CD pipeline |

Avoid generic names like `my-branch`, `changes`, `fix`, `patch`.

### 3. Making Changes

- One commit per logical change
- The commit message must explain the **why**, not just the what
- If the change closes an issue, include `Closes #123` in the commit body

```bash
git commit -m "fix: flat(n) now flattens n levels recursively

Previously only flat() with depth 1 was supported. Now flat(n) flattens
up to n levels using a recursive function.

Closes #54"
```

### 4. Running Tests

Before submitting the PR, make sure all tests pass:

```powershell
# Windows
.\run_tests.ps1

# Linux / macOS
./run_tests.sh
```

If you add a new feature, include at least one `.sz` test that exercises it in `tests/`.

### 5. Opening the Pull Request

**PR Title:** same as the main commit, clear and descriptive.

```
fix: fix flat(n) for depth greater than 1
feature: add String.repeat(n) method
docs: document behavior of for-in with copies
```

**PR Description**, include:

- **What this PR does** — one or two sentences
- **Why** — what problem it solves or what improvement it brings
- **How to test it** — steps to verify the change
- **Related issue** — `Closes #123` if applicable

**Example:**

```markdown
## What it does
Fixes `flat(n)` so that it recursively flattens n levels instead of just 1.

## Why
`[1, [2, [3]]].flat(2)` returned `[1, 2, [3]]` instead of `[1, 2, 3]`.

## How to test it
sz tests/unit_array_methods_edge.sz

## Related issue
Closes #54
```

---

## Technical Conventions

- **Zero `unsafe`** — the memory model is maintained without unsafe blocks
- **No external runtime dependencies** — `[dependencies]` in `Cargo.toml` only for essential tooling
- **Errors go to `stderr`** — `eprintln!` for errors, `println!` only for program output and the REPL
- **New syntax follows the full pipeline** — `token.rs` → `lexer.rs` → `ast.rs` → `parser.rs` → `evaluator/`
- **New infix operator requires registration in two places** in `parser.rs`: `token_precedence()` and the `is_infix` match
- **Every new `{ }` block must push/pop** scope in all code paths, including error paths

---

## Any questions?

Open an issue with the prefix `[QUESTION]` or comment on the corresponding issue/PR.
