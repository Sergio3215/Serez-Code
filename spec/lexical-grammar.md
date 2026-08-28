# Lexical grammar

This document defines how Serez Code 9.17.0 turns source text into tokens. It is
normative for accepted token forms and diagnostics; syntax built from those
tokens belongs in `syntax.md`.

## Source and positions

Source files are read as UTF-8. Diagnostic lines and columns are one-based;
columns count Unicode scalar values, not UTF-8 bytes. A literal NUL character in
source is invalid (`SZ1001`) and does not terminate lexing early.

The lexer ignores ASCII space, tab, carriage return and line feed between tokens.

## Comments

- `//` starts a line comment and consumes through the next line ending or EOF.
- `/*` starts a block comment and the next `*/` ends it.
- Block comments do not nest.
- EOF before `*/` is `SZ1003`; it is not silently treated as a closed comment.

Comments are consumed iteratively. Their number does not consume the native call
stack.

## Identifiers and keywords

An identifier begins with `_` or a Unicode alphabetic character. Following
characters may additionally be Unicode numeric characters. Keyword recognition
is exact and case-sensitive; an identifier matching a keyword becomes that
keyword token.

This is the implementation's Unicode contract, not ASCII-only normalization.
Serez does not currently normalize canonically equivalent identifiers, so tools
must preserve the spelling in source.

## Numeric literals

### Decimal integers and floating point

A decimal integer starts with a numeric character. A `.` is part of the number
only when followed by another numeric character. An exponent has the form
`e[+|-]?digits` or `E[+|-]?digits`; without the required exponent digit, `e`/`E`
starts the next token instead.

Examples:

```text
42        integer
3.5       decimal
1e6       decimal
2.5E-3    decimal
12.50m    exact `dec`
5m        exact `dec`
```

The suffix `m` selects the exact base-10 `dec` literal when it is not followed by
an identifier/numeric character.

`_` is removed while scanning numeric literals. The current implementation also
accepts repeated or trailing underscores; this is existing behavior, not a
recommendation for new code, and tightening it requires a compatibility review.

Decimal integers must fit signed 64-bit range; an otherwise lexical integer that
does not fit is currently a parser diagnostic (`SZ2000`). Floating-point and
exact-decimal range rules are defined with values/types rather than here.

### Base-prefixed integers

`0b`/`0B` accepts ASCII binary digits and `0x`/`0X` accepts ASCII hexadecimal
digits. At least one digit is required, suffix characters are not accepted, and
the resulting value must fit signed 64-bit range. Empty, malformed or overflowing
base-prefixed integers are `SZ1004`; they never silently become integer zero.

## Strings

- `"..."` is a normal string. It supports `\n`, `\t`, `\r`, `\\`, `\"`, escaped
  braces, and `{expression}` interpolation.
- `'...'` is a single-quoted string. Its contents are literal until the next
  single quote.
- `r"..."` is a raw string. Backslashes and braces are literal; the next `"`
  closes it.

Unknown escapes in a normal string retain the backslash and following character,
preserving existing Windows-path and regular-expression behavior. EOF before the
matching delimiter is `SZ1002`, and the incomplete token is invalid; it cannot
be evaluated as if the source had supplied a closing quote.

## Punctuation and operators

The lexer recognizes the punctuation and operator spellings used by the syntax,
including the multi-character forms:

```text
== != <= >= && || => ?? ?. |> ++ -- += -= *= /= %= ** << >> ...
```

Longest valid forms take precedence. The historical `..` spelling is not an
operator: it is tokenized as two dots, while `...` is spread/rest.

Any other character produces an `Illegal` token and `SZ1001`. The parser may
synchronize and continue collecting diagnostics, but a source containing any
lexical diagnostic must not be evaluated.

## Stable diagnostics

| Code | Meaning |
| --- | --- |
| `SZ1001` | Unexpected character, including an embedded NUL. |
| `SZ1002` | Unterminated normal, single-quoted or raw string. |
| `SZ1003` | Unterminated block comment. |
| `SZ1004` | Empty, malformed or overflowing `0b`/`0x` integer. |

These diagnostics are reported on stderr by the CLI and forwarded through the
same structured frontend channel used by the LSP. Messages may improve; codes
and meanings are the stable contract.
