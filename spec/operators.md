# Operators

Normative contract for the operators and the types they accept.

Every combination below was checked against the running implementation.

## Arithmetic

| Left | Op | Right | Result |
| --- | --- | --- | --- |
| `int` | `+ - * / % **` | `int` | `int` |
| `int` / `decimal` | `+ - * / % **` | `decimal` / `int` | `decimal` |
| `dec` | `+ - * / %` | `dec` / `int` | `dec` (exact, up to 28 significant digits) |
| `dec` | `**` | `dec` / `int` with an **integer** value | `dec` |
| `dec` | `+` | `string` | `string` (concatenation) |
| `string` | `+` | `string` / `int` / `decimal` | `string` (concatenation) |
| `int` / `decimal` | `+` | `string` | `string` |
| `string` | `*` | `int` | `string` (repetition) |

Any other combination is a catchable `TypeError` / `SZ4002`. In particular
`[1] + [2]`, `true + 1` and `"a" - 1` all fail: there is no array
concatenation operator and no boolean arithmetic. `null` on either side of an
arithmetic operator is also `TypeError`.

`int` division truncates toward zero (`-7 / 2` is `-3`) and `%` takes the sign
of the dividend (`-7 % 2` is `-1`).

`**` is exponentiation and is **right-associative**: `2 ** 3 ** 2` is 512. It
binds tighter than unary minus, so `-2 ** 2` is `4`.

Repeating a string by a negative count is a catchable `TypeError`; by zero it
yields the empty string. The result has a length ceiling — see `limits.md`.

## `dec` does not mix with `decimal`

`dec` is exact base-10; `decimal` is binary floating point. Mixing them is
refused rather than silently rounded, and the refusal covers **comparison and
equality as well as arithmetic**:

```serez
// runtime-error-example: mixing dec and decimal is refused
1m + 1        // 2m  — int mixes in exactly
1m + 0.5      // TypeError / SZ4002
1m < 0.5      // TypeError / SZ4002
1m == 1.0     // TypeError / SZ4002 — `==` throws here
```

`==` is otherwise total: every other operand pair returns `true` or `false`
rather than failing. This is the one exception, and it is deliberate — an exact
value and an approximate one have no honest answer. A fractional exponent
(`2m ** 0.5m`) is refused for the same reason.

`unit_dec.sz` pins this rule.

## Failure modes

| Situation | Diagnostic |
| --- | --- |
| `/` or `%` with a zero right operand | catchable `DivisionByZero` / `SZ4004` |
| `int` arithmetic outside the `i64` range | catchable `Overflow` / `SZ4000` |
| Any unsupported operand pair | catchable `TypeError` / `SZ4002` |
| An operand that is `null` | catchable `TypeError` / `SZ4002` |

Integer overflow is detected, not wrapped. Exact-decimal division by zero,
invalid exponent types and overflow use the same channel as their integer
equivalents.

## Comparison

`< <= > >=` compare numbers numerically across `int`, `decimal` and `dec`, and
strings by scalar-value order. `"10" < "9"` is `true` because it is a string
comparison, not a numeric one.

`==` and `!=` compare scalars by value and containers by identity:

- `1 == 1.0` is `true` — int and decimal compare numerically. `dec` is the
  exception: comparing it with a `decimal` throws, see above.
- `1 == "1"` is `false` — no cross-type coercion.
- `null == null` is `true`; `null == 0` and `null == false` are `false`.
- `[1, 2] == [1, 2]` is `false`, and so is any comparison of two containers or
  two instances.

See `values.md`, which records the container case as a known inconsistency
rather than a design statement.

## Logical

`&&` and `||` **return one of their operands, not a boolean**:

- `a && b` yields `a` when `a` is falsy, otherwise `b`. `1 && 0` is `0`.
- `a || b` yields `a` when `a` is truthy, otherwise `b`. `0 || "x"` is `"x"`.

Both short-circuit. `&&` binds tighter than `||`. Truthiness is defined in
`values.md`.

`!` yields a boolean, unless the operand is an instance whose class defines
`op_not`, in which case the result is whatever that method returns — it is
not coerced.

## Bitwise and shifts

`& | ^ << >>` operate on `int`. `6 & 3` is `2`, `6 | 3` is `7`, `6 ^ 3` is `5`,
`1 << 3` is `8`, `8 >> 2` is `2`. An invalid shift amount is a catchable
`TypeError` / `SZ4002`.

## Other operators

| Operator | Meaning |
| --- | --- |
| `??` | Left operand unless it is `null`, otherwise the right one. |
| `? :` | Conditional expression; the condition uses the truthiness rules. |
| `++` / `--` | Increment/decrement in place, prefix and postfix. |
| `\|>` | Pipe: `x \|> f` calls `f(x)`. |
| `&` / `*` | Address-of and dereference. Writing through `*` requires `unsafe { }`; see `security.md`. |
| `sizeof(T)` | Size in bytes of a **type**'s in-memory slot, as a static `int`. It takes a type keyword, not a value — `sizeof(x)` does not parse. |

### `sizeof`

`sizeof` is an operator over a **type keyword**, not an expression. `sizeof(5)`
and `sizeof x` do not parse.

| Expression | Result |
| --- | ---: |
| `sizeof(int)` | 8 |
| `sizeof(decimal)` | 8 |
| `sizeof(string)` | 8 |
| `sizeof(bool)` | 1 |
| `sizeof(null)` | 0 |

`string` reports 8 because that is the size of the slot that holds it, not the
size of its contents.

`Memory.sizeof("int")` answers the same question with a string argument and is
the form the runtime API documents; it accepts `"int"`, `"decimal"` and
`"bool"`. Both are available. See `limits.md` for what these numbers do and do
not tell you about a program's memory use.

## Precedence

Tightest to loosest:

1. unary `!`, unary `-`, `&`, `*`, `++`/`--`
2. `**` (right-associative)
3. `* / %`
4. `+ -`
5. `<< >>`
6. `< <= > >=`
7. `== !=`
8. `&`, `^`, `|`
9. `&&`
10. `||`
11. `??`
12. `? :`
13. `|>`

Binary operators at the same level are left-associative: `10 - 3 - 2` is `5`.

## Operator overloading

A class may define these methods, and the matching operator dispatches to them:

| Method | Operator |
| --- | --- |
| `op_add` `op_sub` `op_mul` `op_div` `op_mod` | `+` `-` `*` `/` `%` |
| `op_eq` `op_ne` | `==` `!=` |
| `op_lt` `op_le` `op_gt` `op_ge` | `<` `<=` `>` `>=` |
| `op_neg` `op_not` | unary `-` and `!` |
| `op_str` | string conversion in `out` and interpolation |

When no overload matches the operand pair, the failure is the ordinary
`TypeError` / `SZ4002` above, naming both types. Calling an operator method
that does not exist on the class is `ReferenceError` / `SZ4001`.

See `classes.md`.
