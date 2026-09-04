# The Serez-Code formatter

What it does, what it refuses to do, and where it stops.

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
| **Indentation** | One level per *line* that leaves a delimiter open — `{`, `(`, `[`, and a JSX tag in `.szx`. The unit comes from the editor's `tabSize` / `insertSpaces`, defaulting to four spaces. |
| **Lines starting with a closer** | `}`, `)`, `]` and `</Tag>` dedent before the line is printed, so `}`, `} else {` and `)` land on the enclosing level. |
| **Continuation lines** | **Re-indented**, one level past the line they continue. See below. |
| **Literal lines** | A line that *begins* inside a multi-line string, raw string or block comment is emitted byte-for-byte, trailing whitespace included. |
| **Trailing whitespace** | Removed everywhere else. |
| **Blank lines** | Runs collapse to one. Leading and trailing blank lines are removed. |
| **Final newline** | **Removed.** Formatted output never ends with a line break — `out 1;`, `out 1;\n` and `out 1;\r\n` all come back as `out 1;`. |
| **Line endings** | The document's dominant ending is preserved — CRLF stays CRLF. Independent of the final-newline rule. |
| **Spacing inside a line** | Never touched. `-x`, `obj.field`, `f(x)`, `a[i]`, `1+2` come back as written. |

### One level per line, not per delimiter

    foo({            <- opens two delimiters, one level
        a: 1         <- level 1, not 2
    })

The formatter records the delimiter depth at which each open line started; a
line prints at the number of those still standing once its own **leading**
closers have popped. Only leading ones: `, b);` closes its group at the end of
the line and still belongs to it, while `)` alone on a line does not.

### What counts as a continuation

A line continues the expression above it, and is indented one level past it,
when either:

1. a `(` or `[` opened on an earlier line is still open — in which case the open
   delimiter already supplies the level and no extra bonus is added; or
2. it begins with a token that **cannot start a statement** — `.` `,` `?` `:`
   `*` `%` `&&` `||` `??` `|>` `==` `!=` `<=` `>=` `=>` `::` — *and* the line
   above ended with something that finishes an expression; or
3. the line above ended with something that **cannot end a statement**: `=`, an
   operator, a comma.

A line that starts by closing something is never a continuation, whatever the
line above dangled — `return a +` followed by `}` leaves the brace where it
belongs.

**`+`, `-` and `/` are decided by context, not by the symbol.** They are prefix
and infix both, and Serez does not require semicolons — `let a = 1` then `out a`
is two statements — so the previous token is the only reliable signal. `let a = 1`
followed by `+ 2;` parses as `1 + 2` (measured: it prints `3`), so a leading `+`
after a token that ends an expression continues the line; after `;` or `}` it
starts a statement. `!` is prefix only and is never a continuation on its own
account, though it can be one when the line above ended open.

`<` and `>` are excluded from the "ends open" set: a line ending in `>` is
overwhelmingly a JSX tag, not a dangling comparison.

### The end of the document

Formatted output ends at the last character of the last line. Trailing blank
lines were already dropped; the last line break goes with them, so how a
document ends has one answer instead of depending on how it arrived:

    "out 1;"        ->  "out 1;"
    "out 1;\n"      ->  "out 1;"
    "out 1;\r\n"    ->  "out 1;"

This is about the **end** of the document and nothing else. Line endings
*inside* it are untouched: a CRLF document keeps CRLF between its lines, and
only the final one is removed. The two rules are independent, and confusing
them would turn every save of a CRLF file into a whole-file diff.

One document is exempt. Under DEC-FMT-002 a contradictory document is returned
byte for byte, final newline included — that path does not format at all, and
taking a byte off a document the formatter has just declined to read would be
the one edit it is not entitled to make.

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

The formatter draws a line between **incomplete** and **contradictory**.

### Incomplete — formatted normally, silently

Code that is simply unfinished is what the user is writing most of the time, and
its structure is still determinable:

| | |
|---|---|
| unclosed `{` | the lines after it are indented as if the block were open |
| unclosed `(` or `[` | the lines after it are indented one level, as arguments |
| unterminated string or block comment | everything after is literal, held verbatim |
| half-written declaration | left alone |

No warning is shown for any of these.

### Contradictory — left untouched, and reported

A closer that matches nothing, or the wrong opener — `foo(}`, `[1, 2)`, a stray
`}` — means the document contradicts itself. Its structure cannot be determined,
so the formatter **returns the document exactly as it came** and the editor says
so:

> Serez formatter: the structure of this document could not be determined, so it
> was left unchanged. There may be a syntax error (a `(` is closed by a `}`).

One message per run, not one per problem. The formatter never throws, never
deletes a line and never reorders tokens; if it did throw despite that,
`extension.js` catches it and returns no edits.

## `.szx`

`.szx` is formatted by tag depth on top of the same scanner, and tags share the
delimiter stack with `{`, `(` and `[` — so a JSX tree and a block indent by one
rule. `<Tag>` and `<>` push, `</Tag>` and `… />` pop.

`<` is only a tag when the preceding character is not alphanumeric, so `a < b`
and the dict annotation `<string, any>` are left alone. A tag head may span
lines: the `>` that ends it must appear at the delimiter depth the tag opened
at, which is why the `>` of an `=>` inside `onChange={(v) => …}` does not close
it.

There is **no JSX parser**, and the tag counting is a heuristic. Measured across
347 real `.szx` files in this repository and the ecosystem packages: 346 format
cleanly and idempotently, and **one** is reported as indeterminable and left
untouched under DEC-FMT-002.

### The one that cannot be read, and why

`serez-strike/app.szx` contains this inside a `<p>`:

    Solo ws:// (el core no tiene TLS) …

`ws://` is text, but the scanner sees `//` and treats the rest of the line as a
line comment — so the `</p>` that follows it is never seen, the `<p>` is never
popped, and the enclosing `)` ends up "closing" a tag. The document is preserved
and the user is told, which is the right outcome for something the formatter
genuinely cannot read.

Distinguishing JSX **text** from code needs the structure a parser has and a
character scanner does not. That is recorded as debt rather than approximated:
a heuristic for "is this `//` inside text" would be wrong in the other
direction on real comments.

## Decisions

### DEC-FMT-001 — should continuation lines be re-indented? **DECIDED: yes.**

Running the formatter formats. While typing, the indentation is whatever the
author wants; when they press Format Document, or save with format-on-save, the
indentation is normalised — including for multi-line calls, arrays,
parenthesised expressions, chains, binary expressions and assignments.

The formatter does not preserve incorrect indentation just because a human typed
it. What it does not do is *guess*: where the structure cannot be determined
safely, DEC-FMT-002 applies instead.

### DEC-FMT-002 — what should a document that cannot be read be formatted as? **DECIDED: not at all.**

If the structure cannot be determined safely, the formatter does not invent an
indentation. It preserves the document, does not corrupt it, and the editor
tells the user a syntax error is likely.

The line is drawn at *contradiction*, not incompleteness — see above. Warning on
merely unfinished code would fire on almost every keystroke and teach people to
ignore it.

## Known limits

- No range formatting: `registerDocumentFormattingEditProvider` only, so
  "Format Selection" is not offered.
- No `.szx` parser; tag depth is a heuristic, measured above. `//` inside JSX
  text reads as a line comment — one file in 347, reported rather than
  mangled.
- A continuation starting with `!` is not recognised on its own account; `!` is
  prefix-only, so only the line above can make it a continuation.
- Continuation lines are indented exactly one level. Aligning to the opening
  delimiter's column is not offered, and would not be idempotent-safe without
  tracking columns.
- A continuation line that **opens with `[`** is not recognised as one, so a
  chain broken as `dic` / `["user"]` / `.name` comes back half-indented: the
  `.name` moves in, the `["user"]` does not. `[` is ambiguous the way a leading
  `+` or `-` is — it can index the line above or open an array literal that
  starts a statement — and because Serez does not require semicolons, the
  previous token cannot settle it: `let v = dic` is already complete. Telling
  the two apart needs the parser. Pinned by a test, and left rather than
  guessed at, per DEC-FMT-002's principle.
- `.szs` is indentation only; declarations and one-line rules are preserved
  verbatim by design.
