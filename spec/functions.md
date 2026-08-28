# Functions and parameters

This revision freezes parameter ordering, positional arity, default evaluation
and rest collection. Closure capture, receiver writeback and the complete type
contract remain under **Coverage boundary** rather than being implied here.
Normative words such as "must" describe compatibility requirements.

## Parameter order

A parameter list has this order:

```text
required parameters, default parameters, optional final ...rest parameter
```

- A default parameter has the form `name = expression` (with an optional type
  annotation before `name`).
- A required parameter must not follow a default parameter. The parser rejects
  that signature with `SZ2000`; it is not a call-time arity rule.
- At most one rest parameter is accepted. It must be last, has no default, and
  may follow default parameters.

For example:

```serez
fn int total(int first, int offset = first + 1, ...rest) {
    return offset + rest.length();
}
```

## Positional arity

Without a rest parameter, a call accepts from the number of required parameters
through the total number of parameters. With a rest parameter, it accepts at
least the required count and has no language-level positional maximum.

Too few arguments, or too many arguments when there is no rest parameter,
reject the call before the function body runs. The exact error migration for
every class-related call path is not frozen by this document yet.

The final rest binding is always a new array containing every supplied argument
not consumed by an earlier parameter, in call order. It is an empty array when
there are none.

## Evaluation order and defaults

Supplied argument expressions are evaluated left-to-right in the caller. An
explicit argument always wins and its corresponding default expression is not
evaluated.

After the call environment is entered, parameters are bound in declaration
order. Each omitted default is evaluated at that point, on every call. It can
therefore read an earlier parameter, captured lexical bindings and the method
receiver when that call form provides one. A later parameter is not yet bound.
Default values are not precomputed when the function is declared.

If a default expression executes `throw`, the same user value propagates to the
caller. If it raises a runtime error, its structured payload and recoverability
classification propagate unchanged. Neither case is converted to `null`, and
the function or constructor body does not run.

These rules apply consistently to:

- ordinary function and function-value calls;
- native higher-order callbacks, including bound method references;
- instance methods and constructors;
- `super.method(...)` and `super(...)` constructor delegation.

## Conformance evidence

- `tests/unit_default_params.sz`: omission/override, call-time expressions,
  left-to-right dependency and failure propagation through every call form
  above.
- `tests/unit_bug_b64_b74.sz`: default plus final rest arity and collection.
- `tests/frontend_robustness.rs`: invalid ordering across named, anonymous,
  typed-arrow, constructor and method signatures, plus the valid rest exception.
- `tests/err_parse_required_after_default.sz`: CLI/parser error path.
- `tests/runtime_outcome.rs`: user throws and structured runtime errors survive
  default evaluation at the embedding boundary.

## Coverage boundary

The full contracts for parameter/return type enforcement, concise lambdas,
closure capture and mutation, function identity, generators, receiver
writeback, method visibility and overload dispatch still require their own
implementation and ecosystem audits. Existing conformance tests remain the
compatibility source for those behaviors.
