# Random

Normative contract for the deterministic `Random` namespace. This namespace is
part of the runtime API, not a source of security-grade entropy.

## Generator and reproducibility

`Random` uses a seedable linear congruential generator (LCG). `Random.seed(n)`
sets the generator state from the complete bit pattern of an `int`; negative
seeds are valid. The state is shared with `Math.random()` in the same evaluator.

Given the same seed and the same ordered sequence of calls, an evaluator must
produce the same results. A change to that seeded sequence is
compatibility-impacting and requires the deprecation/versioning process in
`compatibility.md`.
In particular, the established sequence for integer ranges whose inclusive
width is at most 2³¹ is preserved.

Independent evaluators own independent generator states. Task workers create
their own evaluator and therefore do not implicitly consume their parent's
stream.

The LCG is predictable. `Random` must not be used for credentials, tokens,
nonces, salts, keys or any other secret. `Crypto.randomBytes` is the runtime API
for operating-system entropy.

## API

| Call | Result | Contract |
| --- | --- | --- |
| `Random.seed(n)` | `null` | `n` is an `int`; resets the deterministic stream. |
| `Random.decimal()` | `decimal` | A value in `[0, 1)`. |
| `Random.int(min, max)` | `int` | An inclusive draw in `[min, max]`; the complete `int` domain, including `i64::MIN..i64::MAX`, is supported. |
| `Random.uniform(lo, hi)` | `decimal` | A finite value in `[lo, hi)`; bounds must be finite numbers and `lo < hi`. |
| `Random.normal(mean, std)` | `decimal` | A Box–Muller draw; both parameters are finite numbers and `std >= 0`. |
| `Random.normalTensor(shape, mean, std)` | `Tensor` | One normal draw per element under the same parameter rules. |
| `Random.uniformTensor(shape, lo, hi)` | `Tensor` | One uniform draw per element under the same bound rules. |
| `Random.shuffle(array)` | array | Fisher–Yates shuffled copy; the input array is not mutated and its declared element type is preserved. |
| `Random.choice(array)` | value | One planted value from a non-empty array. |
| `Random.bernoulli(p)` | `bool` | `p` is a finite number in `[0, 1]`; `0` is always false and `1` is always true. |

`int` arguments do not coerce from `decimal`. Numeric distribution parameters
accept `int` or `decimal`. Tensor shapes use the shared Tensor contract: a
non-empty array of positive integers, checked multiplication, and the global
10,000,000-element ceiling from `limits.md`.

The interval names describe the returned bounds. This deterministic LCG is not
a certified statistical generator; callers needing stronger distribution
quality must use a purpose-built library outside the core.

## Evaluation and errors

Arity is validated before any argument expression is evaluated. After arity is
valid, arguments are evaluated from left to right. A runtime error or user
`throw` raised by an argument propagates unchanged.

| Failure | Diagnostic |
| --- | --- |
| Wrong arity or argument type | catchable `TypeError` / `SZ4002` |
| Reversed/non-finite bounds, negative/non-finite deviation, invalid probability, empty choice, empty/non-positive shape | catchable `RangeError` / `SZ4000` |
| Unknown Random member | catchable `ReferenceError` / `SZ4001` |
| Tensor shape product overflow or element ceiling | fatal `ResourceError` / `SZ6002` |

Invalid user input must never panic the host. In particular, calculating the
inclusive width of `Random.int` is performed outside `i64`, so the full-domain
call cannot overflow or perform modulo by zero.

`Random` requires no permission. That is a convenience/API statement, not a
sandbox guarantee and not evidence of unpredictability.
