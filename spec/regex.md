# Regex

Normative contract for the `Regex` namespace and the pattern dialect it accepts.

Every rule here was derived by probing the running implementation. The engine is
hand-written and small: it is **not** PCRE, not RE2 and not JavaScript's. The
section on what it does *not* support is the most important part of this
document, because the unsupported constructs do not fail — they parse as
ordinary characters and match something else.

Patterns are ordinary strings, so a raw string (`r"..."`) is usually what you
want: a normal string treats `\` as an escape and `{` as the opening of an
interpolation.

## Methods

```text
Regex.test(pattern, text)                   -> bool
Regex.match(pattern, text)                  -> [string] | null
Regex.findAll(pattern, text)                -> [string]
Regex.replace(pattern, text, replacement)   -> string
Regex.split(pattern, text)                  -> [string]
```

Note the argument order of `replace`: the **replacement comes last**, after the
text, not after the pattern.

| Method | Contract |
| --- | --- |
| `test` | True when the pattern matches anywhere in the text. |
| `match` | The first match as an array: element `0` is the whole match, then one element per capturing group in order. `null` when nothing matches — not an empty array. |
| `findAll` | Every whole match, in order, as an array of strings. Empty matches are produced, so `r"a*"` against `"bab"` yields `["", "a", "", ""]`. |
| `replace` | Replaces **every** occurrence, not just the first. The replacement string has its own substitution syntax — see below. |
| `split` | Splits on every match. An empty pattern does not split per character: it returns the text as a single element. Splitting the empty string yields an empty array. Adjacent separators produce an empty element between them. |

### The replacement string

`replace` expands a small substitution syntax in its third argument:

| Form | Expands to |
| --- | --- |
| `$&` | the whole match |
| `$1` … `$9` | that capturing group, or nothing when the group did not participate |
| `$$` | a literal `$` |

Everything else is literal, including a trailing `$` and a `$` before any other
character. Two consequences follow from the group form reading exactly **one**
digit, and both are measured:

- `$10` is group 1 followed by a literal `0`, not group 10. With one group,
  replacing `(a)` in `"a"` by `"$10"` yields `"a0"`.
- A `$n` naming a group the pattern does not have stays literal. With two
  groups, `"$5"` yields `"$5"`.

```serez
Regex.replace(r"(\w+)@(\w+)", "joe@corp", "$2.$1");   // "corp.joe"
Regex.replace(r"\d", "x1y", "[$&]");                  // "x[1]y"
Regex.replace(r"a", "a", "$$");                      // "$"
```

```serez
Regex.test(r"a+", "aaa");                  // true
Regex.match(r"(a)(b)", "xabz");            // ["ab", "a", "b"]
Regex.match(r"z", "abc");                  // null
Regex.findAll(r"a", "banana");             // ["a", "a", "a"]
Regex.replace(r"a", "banana", "X");        // "bXnXnX"
Regex.split(r",", "a,,b");                 // ["a", "", "b"]
```

## The dialect

| Form | Meaning |
| --- | --- |
| literal characters | Match themselves. |
| `.` | Any character **except** a newline. |
| `^` / `$` | Start / end of the text. Usable inside an alternation. |
| `\|` | Alternation. |
| `( … )` | Group and capture. |
| `(?: … )` | Group without capturing. |
| `*` `+` `?` | Zero or more, one or more, zero or one. |
| `{n}` `{n,}` `{n,m}` | Exact, minimum, range. A `{` that does not open a valid form is a literal `{`. |
| a trailing `?` on any quantifier | Makes it lazy: `r"a.*b"` on `"axbxb"` matches `axbxb`, `r"a.*?b"` matches `axb`. |
| `[abc]` `[a-c]` `[^a]` | Character set, range, negated set. A `]` immediately after `[` or `[^` is a literal. |
| `\d` `\D` `\w` `\W` `\s` `\S` | Digit, non-digit, word, non-word, whitespace, non-whitespace. |
| `\n` `\t` `\r` `\0` | The control characters. |
| `\` before anything else | That character, literally: `\.` `\\` `\(` and so on. |

Matching is **case-sensitive** and there are no flags. There is no way to ask
for case-insensitive, multiline or dot-matches-newline behaviour.

## What is not supported — and what it does instead

This is the part that bites. None of the following is rejected. Each one parses
as ordinary characters and matches something the author did not intend:

| Written | What the engine actually sees | Measured |
| --- | --- | --- |
| `(?=b)` lookahead | a capturing group of the three literals `?`, `=`, `b` | `r"a(?=b)"` does **not** match `"ab"`; it matches `"a?=b"` |
| `(?!b)` negative lookahead | a group of the literals `?`, `!`, `b` | `r"a(?!b)"` does not match `"ac"`; it matches `"a?!b"` |
| `(?<n>a)` named group | a group of the literals `?`, `<`, `n`, `>`, `a` | matches the text `"?<n>a"` |
| `\1` backreference | the literal character `1` | `r"(a)\1"` matches `"a1"`, not `"aa"` |
| `\b` word boundary | the literal character `b` | `r"a\bc"` matches `"abc"` |

The rule behind all five: `\` before an unrecognised character yields that
character, and `(?` followed by anything but `:` is an ordinary capturing group.
A pattern using them compiles, runs, and quietly answers the wrong question.

Lookbehind, atomic groups, possessive quantifiers, inline flags, Unicode
property classes and POSIX bracket expressions are absent in the same way.

## Errors

| Failure | Diagnostic |
| --- | --- |
| Wrong arity | catchable `TypeError` / `SZ4002` |
| An argument that is not a string | catchable `TypeError` / `SZ4002` |
| Unknown `Regex` member | catchable `ReferenceError` / `SZ4001` |
| A malformed pattern | catchable `RuntimeError` / `SZ4000` |

A malformed pattern is one the parser cannot finish: an unbalanced `(`, a stray
`)`, a trailing `\`. It keeps the generic `RuntimeError` kind because there is
no `SyntaxError` kind to move it to and inventing one would be a new code rather
than a reclassification — recorded in `errors.md`.

Until this cycle, wrong arity also reported the generic `RuntimeError` /
`SZ4000`, which made `Regex` the only namespace whose `kind` could not separate
"called wrongly" from any other runtime failure. See `errors.md`.

## Limits

Two ceilings bound the matcher rather than the pattern, both in `limits.md`:
1,000,000 execution steps per match and 8,000 frames of backtracking depth.
Exceeding either makes the **match fail** — it is not an error, so a pattern
that is too expensive answers "no match" rather than saying it gave up.

## Conformance evidence

- `tests/unit_regex.sz`: the five methods, the dialect, the error contract and
  the catchable malformed pattern.
