# The Serez-Code formatter

What it does, what it refuses to do, and what is still undecided.

`formatter.js` is three pure functions — `formatSz`, `formatSzx`, `formatSzs` —
with no dependency on VS Code. `extension.js` registers each as a
`DocumentFormattingEditProvider` for its language. Run the suite with:

```bash
cd vscode-serez && npm test
```

## What it is

**An indenter, not a pretty-printer.** It decides the indentation of each line
and removes trailing whitespace. It does not insert or remove spaces inside a
line, does not wrap, does not reorder, and does not rewrite declarations.

That boundary is deliberate. Everything it *does* change is recoverable by
looking at one line; everything it leaves alone is the author's.

## The contract

| | Rule |
|---|---|
| **Indentation** | One level per open `{`. The unit comes from the editor's `tabSize` / `insertSpaces`, defaulting to four spaces. |
| **A line starting with `}`** | Dedents before it is printed, so `}` and `} else {` land on the enclosing level. |
| **Continuation lines** | Left exactly as the author indented them. See below. |
| **Literal lines** | A line that *begins* inside a multi-line string, raw string or block comment is emitted byte-for-byte, trailing whitespace included. |
| **Trailing whitespace** | Removed everywhere else, including on continuation lines. |
| **Blank lines** | Runs collapse to one. Leading and trailing blank lines are removed. |
| **Final newline** | Exactly one, always. |
| **Line endings** | The document's dominant ending is preserved — CRLF stays CRLF. |
| **Spacing inside a line** | Never touched. `-x`, `obj.field`, `f(x)`, `a[i]`, `1+2` come back as written. |

### What counts as a continuation

A line is the middle of an expression, and keeps its own indentation, when
either:

1. a `(` or `[` opened on an earlier line is still open; or
2. it begins with a token that **cannot start a statement**: `.` `,` `?` `:`
   `*` `/` `%` `&&` `||` `??` `|>` `==` `!=` `<=` `>=` `=>` `::`.

`+`, `-`, `!`, `<` and `>` are **excluded** on purpose: they are also prefix
operators (or, in `.szx`, tag syntax), so `-x;` is a statement in its own right
and the formatter cannot tell the two apart without a parser. The cost is that a
continuation line starting with `+` is still re-indented; the alternative was
leaving real statements wherever they happened to sit.

### Lexical safety

The scanner tracks, with state carried across lines, exactly what
`src/lexer.rs` defines:

| Construct | Handled |
|---|---|
| `// line comment` | to end of line |
| `/* block comment */` | spans lines, does **not** nest |
| `"string"` | spans lines, escapes, and `{…}` interpolation whose blocks may contain further `"…"` strings |
| `r"raw string"` | spans lines, **no** escapes — the first `"` closes it |

Braces inside any of those are not braces. This is the difference between a
formatter that changes how code looks and one that changes what it does: before
the scanner existed, a `/* } */` dedented the block around it, and a multi-line
string came back with the formatter's indentation inserted *into the literal*.

## Invalid or incomplete documents

The formatter runs on every keystroke-adjacent save, so it is written to survive
half-typed code. It never throws, never deletes a line, and never reorders
tokens.

- **unclosed `{`** — the lines after it are indented as if the block were open;
- **unclosed `(` or `[`** — everything after is a continuation, held verbatim;
- **unterminated string or block comment** — everything after is literal, held
  verbatim;
- **stray `}`** — indentation clamps at zero rather than going negative.

If the formatter does throw despite this, `extension.js` catches it and returns
no edits, leaving the document untouched.

## `.szx`

`.szx` is formatted by tag depth on top of the same scanner: `<Tag>` and `<>`
indent, `</Tag>` and `/>` dedent, and `<` is only a tag when the preceding
character is not alphanumeric — so `a < b` and the dict annotation
`<string, any>` are left alone.

There is **no JSX parser** here, and the tag counting is a heuristic. It is
measured rather than assumed: across 347 real `.szx` files in this repository
and the ecosystem packages, the formatter now changes 15. Before the
continuation rule it changed 327 of them, every one by pulling the component's
JSX one level left.

## Open decisions

### DEC-FMT-001 — should continuation lines be re-indented?

**Problem.** A multi-line expression — a method chain, an argument list, an
array literal, a JSX tree inside `return (` — has no `{`, so the brace indenter
has no opinion about it.

**Current behaviour.** Left exactly as written.

**Evidence.** Actively re-indenting them to brace depth is what the formatter
used to do, and it degraded 40 of the 493 corpus `.sz` files and 327 of the 347
real `.szx` files. Leaving them alone brings those to 6 and 15 respectively.

**A — leave them (current).** Cannot damage correct code. Gives no help to
someone who has pasted a badly-indented chain.

**B — indent one level from the line that opened the expression.** Helpful, and
it is a genuine layout policy: it will move code that is already correct but
indented differently.

**C — align to the opening delimiter's column.** Common in other formatters,
much more sensitive to line length, and it cannot be undone by re-running.

**Trade-offs.** A is the only one that is inert on correct input. B and C both
have to decide what to do with chains whose opening line is itself a
continuation, and both make the formatter's output depend on line width.

**Recommendation — a recommendation, not a decision.** **A** until there is a
demand for B, and B before C if there is.

**Blocks:** nothing.

### DEC-FMT-002 — what should a document that cannot parse be formatted as?

**Problem.** The formatter has no parser, so it cannot tell "still being typed"
from "broken". It formats what it can either way.

**Current behaviour.** Partial: balanced regions are indented, unbalanced ones
are held verbatim. Measured over the malformed suite — unclosed brace, paren,
bracket, string, block comment, stray `}`, half-written declaration — nothing is
lost, nothing is reordered, and no exception escapes.

**A — partial formatting (current).** Format-on-save keeps working while typing.
A file with a missing brace gets indentation that reflects the missing brace.

**B — return the document unchanged unless delimiters balance.** Never surprises
anyone with indentation derived from a typo, at the cost of format-on-save
silently doing nothing — which reads as a broken extension.

**Trade-offs.** A's failure mode is visible and self-correcting: type the brace
and the next save fixes it. B's is invisible.

**Recommendation — a recommendation, not a decision.** **A**, and it is what the
malformed suite pins.

**Blocks:** nothing.

## Known limits

- No range formatting: `registerDocumentFormattingEditProvider` only, so
  "Format Selection" is not offered.
- No `.szx` parser; tag depth is a heuristic, measured above.
- A continuation line starting with `+`, `-` or `!` is re-indented, because
  those tokens are ambiguous without a parser.
- `.szs` is indentation only; declarations and one-line rules are preserved
  verbatim by design.
