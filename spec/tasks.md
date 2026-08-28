# Tasks and worker runtime

Status: normative for the default interpreter in Serez Code 9.17.

## Runtime ownership and isolation

Each top-level `Evaluator` owns one task runtime. Workers spawned from it receive
fresh evaluators and arenas but share that task runtime with their parent, so
nested workers can poll their children. Unrelated top-level evaluators do not
share IDs, states or replies. A task ID is meaningful only inside the runtime
tree that created it.

Workers share the host process and use native threads. They are memory-isolated
at the Serez arena/value level, not process-isolated: a panic is contained and
reported as a failed task, but OS resources and process-wide effects still belong
to the same host. Native handles cannot be transferred through Task messages.

## Permissions and lockdown

`Task.run`, `message`, `reply`, `poll` and `isDone` require the `Task` permission.
Outside lockdown, a worker starts with `Task` permission and may add permissions
through its source or the `serez.json` beside its script, matching ordinary
trusted execution.

A worker spawned by a locked-down evaluator inherits that lockdown and the
parent's already granted permission set. It cannot grant itself more permissions
and does not load manifest permissions. This inheritance is mandatory: granting
`Task` to a restricted embedder must not reopen `File`, imports or weight I/O in
a child evaluator. Lockdown remains defense in depth, not a sandbox; workers
still share the process and `fetch` remains available.

## API and lifecycle

```text
Task.run(scriptPath: string, argument: string) -> int
Task.message()                             -> string
Task.reply(result: string)                 -> null
Task.isDone(taskId: int)                   -> bool
Task.poll(taskId: int)                     -> string | null
```

`run` validates both arguments left to right and starts a detached native
worker. The path is an OS path resolved from the host process working directory;
it does not use module resolution or establish path containment. Imports inside
the worker resolve relative to the worker file.

The state machine is:

```text
Running (optional provisional reply)
        ├── successful script exit -> Finished(reply or "")
        └── parse/runtime/panic     -> Failed(error)
```

`reply` records a provisional result; it does not terminate evaluation. The
result becomes observable only after the script exits successfully. A later
runtime failure or panic wins over an earlier reply, so callers cannot observe a
premature success. Multiple replies before successful exit use the last value.

`isDone` is false only for `Running`, and true for `Finished` or `Failed`.
`poll` returns `null` while running, the reply on success, or a compatibility
string beginning with `ERROR: ` on failure. Structured worker runtime failures
retain `[code] kind: message` inside that string. Returning failure as a string
rather than throwing in the parent is existing public behavior.

An unknown or evicted ID is `ReferenceError` (`SZ4001`) for both `poll` and
`isDone`. Wrong arity/type is recoverable `TypeError` (`SZ4002`). Argument
runtime errors and user `throw` propagate unchanged. Resource ceilings are fatal
`ResourceError` (`SZ6002`).

For compatibility, `Task.message()` outside a worker returns an empty string and
`Task.reply(value)` outside a worker warns and does nothing. These permissive
fallbacks can hide context bugs and remain documented debt rather than being
changed silently.

## Resource ceilings and retention

| Resource | Limit | Behavior |
| --- | --- | --- |
| concurrent workers per runtime | 32 | New `run` is fatal `SZ6002`. |
| source read by one worker | 16 MiB | Worker finishes as failed before parsing. |
| argument, reply or stored worker-error text | 1 MiB | Oversized argument/reply is fatal `SZ6002`; oversized worker error is replaced by bounded `SZ6002` text. |
| retained task records per runtime | 256 | Oldest terminal record is evicted before a new task; active workers are never evicted. |
| native stack per worker | 16 MiB | Fixed thread-builder allocation. |

Terminal records remain repeat-pollable while retained. There is currently no
explicit cancellation, join, timeout or user-facing `forget`; eviction is the
only lifecycle cleanup. A non-terminating worker consumes one concurrency slot
until the host process ends. Host/OS limits remain necessary for untrusted code.

## Conformance evidence

- `tests/unit_task_errors.sz`: API diagnostics, propagation, worker failure,
  provisional reply and recovery.
- `tests/unit_task*.sz`: simple, failed, concurrent and nested workers.
- `tests/runtime_outcome.rs`: lockdown inheritance and structured boundary.
- `evaluator::namespaces_task::tests`: runtime isolation, poison recovery,
  record eviction and fatal concurrency ceiling.
