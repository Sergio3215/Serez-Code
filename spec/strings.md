# Strings

Normative contract for `string` values and their built-in methods.

## Character model

A Serez `string` stores valid UTF-8. Public lengths and indexes are measured in
Unicode scalar values (`char`), not UTF-8 bytes and not user-perceived grapheme
clusters. Thus one scalar outside ASCII still has length 1, while a grapheme
formed from multiple combining scalars has a larger length.

String methods return new values. They do not mutate the receiver.

`.length` returns the scalar-value count. The historical `.length()` call form
is also accepted with zero arguments. `charAt(i)` returns the scalar at `i`, or
the empty string when `i` is negative or outside the string.

## Search, split and replacement

| Method | Contract |
| --- | --- |
| `startsWith(prefix)` / `endsWith(suffix)` | Exact scalar sequence prefix/suffix test. |
| `includes(sub)` / `contains(sub)` | Exact substring membership. |
| `indexOf(sub)` | First character index, `0` for an empty needle, or `-1`. |
| `split(separator)` | Array of strings; an empty separator yields one string per scalar value. |
| `replace(from, to)` | Replace the first occurrence only. An empty `from` leaves the receiver unchanged. |
| `replaceAll(from, to)` | Replace every occurrence. An empty `from` leaves the receiver unchanged. |

Search is case-sensitive and performs no Unicode normalization.

## Substrings and slices

`substring(start[, end])` accepts one or two integer indexes. Both are clamped
to `[0, length]`; omitted `end` is `length`. If the clamped start exceeds the
end, the result is empty. Unlike JavaScript's similarly named method, Serez does
not swap reversed indexes.

`slice([start[, end]])` accepts zero to two integer indexes. Omitted bounds are
`0` and `length`. A negative bound counts backward from the end and then clamps
at zero. Positive bounds clamp at `length`. Reversed bounds return the empty
string. The complete `int` domain is safe: `i64::MIN` clamps instead of
overflowing or indexing native memory.

## Case and whitespace

`toUpperCase()`/`upper()` and `toLowerCase()`/`lower()` use Unicode case
conversion. Conversion can change the scalar count. `trim()` removes Unicode
whitespace at both ends; `trimStart()`/`trimLeft()` affect only the start and
`trimEnd()`/`trimRight()` only the end.

`toString()` on a string is identity. These methods take zero arguments.

## Padding

`padStart(targetLength[, padString])` and
`padEnd(targetLength[, padString])` require a non-negative integer target and an
optional string, defaulting to one space. They return the receiver unchanged if
it already meets the target or the pad string is empty.

Padding counts scalar values and constructs the result in linear time. A
multi-scalar pad repeats and truncates to the exact target. Historical ordering
is preserved: `"x".padStart(4, "ab") == "babx"` and
`"x".padEnd(4, "ab") == "xaba"`.

Padding that would create more than 10,000,000 scalar values stops with fatal
`ResourceError` / `SZ6002` before allocating the result. Allocation/capacity
failure is the same fatal category. A negative target is instead a catchable
`RangeError` / `SZ4000`.

## Evaluation and errors

Every method validates arity before evaluating arguments. With valid arity,
arguments are evaluated left-to-right. A runtime failure, control-flow result or
user `throw` from an argument propagates unchanged.

| Failure | Diagnostic |
| --- | --- |
| Wrong arity or argument type | catchable `TypeError` / `SZ4002` |
| Negative padding target | catchable `RangeError` / `SZ4000` |
| Unknown string member | catchable `ReferenceError` / `SZ4001` |
| Padding result/allocation ceiling | fatal `ResourceError` / `SZ6002` |

Invalid types are never silently converted to index `0`, default padding or an
omitted bound.
