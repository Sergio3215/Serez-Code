# DateTime and DateField

Status: normative for the default interpreter in Serez Code 9.17.

## Values and permissions

`DateTime` is an immutable instant stored with millisecond precision. Operations
return a new value; they do not mutate the receiver. `DateField` is the immutable
view returned by calendar fields. It behaves as an `int` in existing operator
contexts and also retains its originating instant for field arithmetic.

`DateTime.now()` and `DateTime.utcNow()` read the host clock and require the
`Time` permission. Pure construction, inspection, formatting and arithmetic do
not require a permission. A missing clock permission is fatal `PermissionError`
(`SZ6001`) under the runtime security contract.

## Construction

```text
DateTime.now()                         -> local DateTime
DateTime.utcNow()                      -> UTC DateTime
DateTime.from(y, m, d [, h, mi, s, ms]) -> DateTime
DateTime.fromEpoch(milliseconds)       -> UTC DateTime
DateTime.fromTimestamp(milliseconds)   -> UTC DateTime (compatibility alias)
```

`from` requires three through seven integer arguments. Missing time fields are
zero. `DateField` arguments are accepted as their integer value. The accepted
field ranges are year 1–9999, month 1–12, day 1–31, hour 0–23, minute/second
0–59 and millisecond 0–999. A tuple inside those simple ranges must also be a
real calendar date; for example, 2026-02-30 is invalid.

`fromEpoch` and `fromTimestamp` require exactly one integer representable by the
runtime calendar implementation.

## Inspection and formatting

The calendar fields `year`, `month`, `day`, `hour`, `minute`, `second`, `ms` and
`millisecond` take no arguments and return a `DateField`. Derived members
`weekday`, `dayOfYear`, `daysInMonth`, `isLeapYear`, `isUtc`, `timestamp`,
`toEpoch`, `epochMillis`, `toString` and `iso` also take no arguments.

`format(pattern)` requires exactly one string. Its token grammar is documented
in the DateTime section of the README; formatting does not change the instant.

Every `DateField` supports `add(n)`, `reduce(n)` and `remove(n)` with exactly one
integer or `DateField` argument. `reduce` and `remove` subtract. Day and smaller
units shift the instant; month/year arithmetic is calendar-based and clamps the
day to the last valid day of the target month. `value`, `toInt` and `toString`
take no arguments.

**`add`/`reduce`/`remove` return a `DateTime`, not a `DateField`.** That is the
point of the type: a field is a number that remembers the instant it came from,
so arithmetic on it produces a new instant rather than a new number.

```serez
let m = DateTime.from(2026, 1, 31).month();
m.toString();        // "1"                    — the field reads as its value
m.add(1);            // 2026-02-28T00:00:00    — and adds as a calendar month
```

Note the clamp in that example: January 31 plus one month is February 28, not an
invalid February 31.

A `DateField` also reports itself as an **`int`**, not as a `DateField`:

| Expression | Result |
| --- | --- |
| `type_of(field)` | `"int"` |
| `field is int` | `true` |
| `field is DateTime` | `false` |
| `field * 2` | an ordinary `int` product |
| passing it to an `int` parameter | accepted |

There is no `"DateField"` type name to test against. Introspection sees an
integer; only the `add`/`reduce`/`remove` members reveal the retained instant.

## Calls and failures

Arity is validated before argument expressions are evaluated. A rejected call
therefore has no argument side effects. Valid arguments are evaluated left to
right. A nested user `throw` or runtime failure propagates unchanged.

All ordinary DateTime validation failures are recoverable:

| Condition | Diagnostic |
| --- | --- |
| wrong arity or argument type | `TypeError` / `SZ4002` |
| invalid field, calendar date or epoch | `RangeError` / `SZ4000` |
| field arithmetic outside the representable range | `Overflow` / `SZ4000` |
| unknown DateTime/DateField member | `ReferenceError` / `SZ4001` |

`SZ4000` is currently the stable generic-runtime code shared by `RangeError`
and `Overflow`; tooling must also inspect `kind` when it needs that distinction.

## Conformance evidence

- `tests/unit_datetime.sz`: successful construction, fields, formatting,
  arithmetic, comparisons, destructuring and DateField interoperability.
- `tests/unit_datetime_errors.sz`: classification, catchability, arity ordering,
  propagation and recovery.
- `tests/err_datetime_invalid.sz`: uncaught invalid date through the CLI path.
- `tests/runtime_outcome.rs`: structured program-boundary payloads and original
  nested error/throw propagation.

