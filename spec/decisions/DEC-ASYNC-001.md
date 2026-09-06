# DEC-ASYNC-001: Dual Sync/Async Execution Architecture

- **Status**: Accepted (Normative)
- **Date**: 2026-09-05
- **Target Release**: Serez Code v11.1.0
- **Authors**: DeepMind Antigravity Pair Architecture Team
- **Supersedes**: N/A
- **Related Specs**: [`spec/syntax.md`](../syntax.md), [`spec/functions.md`](../functions.md), [`spec/tasks.md`](../tasks.md), [`spec/security.md`](../security.md), [`spec/errors.md`](../errors.md)

---

## 1. Context & Motivation

Serez Code v11.0.0 established a robust, deterministic, and sandboxed synchronous runtime alongside thread-isolated worker concurrency via the `Task` namespace. All I/O operations (such as HTTP `fetch`, file system access, and sockets) executed in a blocking synchronous manner within their respective host threads.

As Serez Code expands into network services, user interfaces, and high-throughput orchestration, blocking threads on network operations creates unnecessary thread exhaustion and latency. However, introducing traditional asynchronous models (such as JavaScript Promises, Python AsyncIO Futures, or Rust Futures) presents significant hazards:
1. **API Fracturing ("Coloring")**: Splitting the ecosystem into duplicate APIs (e.g. `fetch` vs `fetchAsync`, `read` vs `readAsync`).
2. **Type Leaks**: Leaking wrapper objects like `Promise<T>` or `Future<T>` into user space, introducing unhandled promise rejections, dangling wrappers, and type divergence.
3. **Task Confusion**: Conflating cooperative suspension/resumption with OS thread concurrency (`Task`).
4. **Breaking Compatibility**: Breaking existing v11.0.0 synchronous scripts and libraries.

To resolve these challenges cleanly, Serez Code v11.1.0 adopts a **Dual Sync/Async Execution Architecture**.

---

## 2. Core Architectural Principles

### 2.1. Dual-Path Execution Model
The exact same operation (such as `fetch(url)`) possesses both a synchronous execution path and an asynchronous execution path:
- **Synchronous Path**: `fetch(url)` executes synchronously, blocking until completion and returning the computed value directly.
- **Asynchronous Path**: `await fetch(url)` suspends the current evaluator frame until the operation completes, without blocking the underlying OS execution thread, and resumes with the exact same value.

### 2.2. No Public Promise or Future Types
Neither `Promise<T>` nor `Future<T>` exists as a user-visible type in Serez Code. A value is never wrapped in a promise object. 
The expression `type(fetch(url))` and `type(await fetch(url))` produce identical results:
```serez
let r1 = fetch("https://example.com");
let r2 = await fetch("https://example.com");

assert(type_of(r1) == type_of(r2)); // Identical types
```
The distinction is strictly an **execution discipline** (thread-blocking vs. frame-suspending), not a data type transformation.

### 2.3. Zero Duplicate APIs
No duplicate API variants are permitted. Serez standard libraries and builtins shall not define `fetchAsync()`, `fetchSync()`, `readFileAsync()`, or similar bifurcated functions. The single unified function name represents both execution paths.

### 2.4. Full Backward Compatibility with v11.0.0
All valid Serez Code v11.0.0 programs remain 100% valid and produce identical runtime behaviors, return values, and error classifications in v11.1.0. Existing synchronous code runs unaltered.

---

## 3. Syntax Specification

### 3.1. Keywords
The lexer reserves two new keywords:
- `async` (`TokenType::KwAsync`)
- `await` (`TokenType::KwAwait`)

### 3.2. Asynchronous Functions
Functions may be declared with the `async` modifier immediately preceding `fn`:

```serez
// Top-level / local function declaration
async fn string fetchProfile(string url) {
    let resp = await fetch(url);
    return resp;
}

// Function without explicit return type
async fn logResult(string url) {
    let data = await fetch(url);
    out data;
}

// Anonymous function literal
let handler = async fn(url) {
    return await fetch(url);
};
```

### 3.3. Class Methods
Methods within class definitions may be declared with the `async` modifier:

```serez
class ApiClient {
    public string endpoint;

    public ApiClient(string endpoint) {
        this.endpoint = endpoint;
    }

    public async string get(string path) {
        let fullUrl = this.endpoint + path;
        return await fetch(fullUrl);
    }
}
```

#### Constructors
Class constructors **MUST NOT** be declared `async`. Attempting `public async ClassName(...)` is a syntactic error (`SZ2000`). Constructors must remain strictly synchronous to preserve deterministic object initialization.

#### Generators
Generator functions (`async fn*`) are explicitly out of scope for v11.1.0 and rejected by the parser (`SZ2000`).

### 3.4. Await Expressions
`await` is a prefix unary expression:

```text
AwaitExpression := "await" Expression
```

Examples:
```serez
let res = await fetch("https://api.example.com/data");
let user = await client.get("/user");
```

---

## 4. AST Representation

The AST is extended cleanly in `src/ast.rs`:

1. **`FunctionLiteral`**:
   ```rust
   #[derive(Debug, Clone)]
   pub struct FunctionLiteral {
       pub return_type: Option<String>,
       pub parameters: Vec<Parameter>,
       pub body: BlockStatement,
       pub is_generator: bool,
       pub is_async: bool, // NEW in v11.1.0
       pub span: Span,
   }
   ```

2. **`ClassMethod`**:
   ```rust
   #[derive(Debug, Clone)]
   pub struct ClassMethod {
       pub name: String,
       pub is_public: bool,
       pub is_abstract: bool,
       pub is_getter: bool,
       pub is_setter: bool,
       pub is_static: bool,
       pub is_async: bool, // NEW in v11.1.0
       pub return_type: Option<String>,
       pub parameters: Vec<Parameter>,
       pub body: BlockStatement,
       pub span: Span,
   }
   ```

3. **`Expression::Await`**:
   ```rust
   pub enum Expression {
       ...
       Await {
           value: Box<Expression>,
           span: Span,
       },
       ...
   }
   ```

Spans are maintained across all new nodes, ensuring accurate diagnostic emission, LSP symbol navigation, and code formatting.

---

## 5. Precedence & Parsing Rules

1. `await` has **Prefix Precedence** (`Precedence::Prefix`), equal to unary prefix operators `-`, `!`, `~`, `sizeof`, and pointer dereference `*`.
2. Prefix binding associates to the right operand:
   - `await fetch(url).length()` parses as `(await (fetch(url))).length()`.
   - `await a + b` parses as `(await a) + b`.
3. To await the result of a binary or ternary expression, explicit parentheses are required:
   - `await (cond ? fetch(a) : fetch(b))`

---

## 6. Semantic Validation & Diagnostics

The semantic analysis layer (`src/semantic/validate.rs`) enforces strict consistency before execution:

### 6.1. Async Function Call Contract
Because Serez Code does not expose naked promise handles, calling a declared `async fn` **without** `await` at the call site is a static semantic error (`SZ8000` / `SZ8002`):
```serez
async fn doWork() { ... }

// OK:
await doWork();

// Compile-Time Error (SZ8000):
// "async function 'doWork' must be awaited at call site"
doWork();
```
This rule guarantees that asynchronous work is never silently initiated as a dangling background side effect without explicit synchronization.

### 6.2. Dual-Path Operations vs. Sync-Only Operations
1. **Dual-Path Operations**: Operations that provide an asynchronous engine path (e.g. `fetch`).
2. **Sync-Only Operations**: Pure computational, in-memory, or deterministic built-in functions (e.g. `JSON.parse`, `Math.sqrt`, `parseInt`, string methods).
   - Calling `await` on a known sync-only built-in is rejected during semantic validation:
     ```serez
     // Semantic Error (SZ8000):
     // "operation 'JSON.parse' is synchronous and does not support 'await'"
     let obj = await JSON.parse(str);
     ```

### 6.3. Top-Level Await
Top-level await is permitted in standalone `.sz` script execution. The main evaluation environment supports suspension and resumption at the statement level without requiring a wrapping `async fn main()` boilerplate.

---

## 7. Runtime Evaluation & Suspension Model

### 7.1. Shared Operation Contract & Dispatch Architecture
Operations supporting dual paths (starting with `fetch`) separate their execution into two tiers:
1. **Shared Contract Tier**:
   - Argument evaluation and validation.
   - Option dictionary parsing (`headers`, `timeout`, `full`, `binary`).
   - Security checks (lockdown enforcement, URL scheme, host allowlist).
   - 64 MiB payload limits and response validation.
   - Response value synthesis (`fetch_make_value`).
2. **Transport Dispatch Tier & AST-Driven Dispatch**:
   - **AST-Driven Dispatch (No Ambient State)**: Dispatching between synchronous and asynchronous execution is strictly driven by the AST callsite structure (`eval_call_internal(call_expr, is_await: bool)` and `eval_dot_call_internal(dot_call, is_await: bool)`).
   - Direct `Expression::Call` and `Expression::DotCall` nodes pass `is_await = false`.
   - `Expression::Await { value, .. }` unpacks its operand and executes the direct child call with `is_await = true`.
   - Function/method arguments are always evaluated via `eval_expression`, ensuring that inner expressions in calls like `await wrapper(fetch(url))` evaluate with `is_await = false`, completely preventing "await mode" leakage to nested expressions.
   - Calling an `async fn` or async class method with `is_await = false` raises a runtime `TypeError` (`SZ4002`), mirroring static semantic analysis.

### 7.2. Central Native Operation Capability Registry
To prevent semantic divergence between the semantic validation layer (`src/semantic/validate.rs`) and the runtime evaluator (`src/evaluator/expr.rs`), capabilities are defined centrally in `src/capabilities.rs`:
- `OperationCapability::SyncOnly`: Pure deterministic, in-memory, or synchronous builtins (e.g. `parseInt`, `Math.sqrt`, `JSON.parse`, and namespace operations like `OS.platform`, `File.read`). Awaiting these triggers diagnostic `SZ8000` at compile time and runtime.
- `OperationCapability::DualPath`: Operations supporting both synchronous blocking execution and asynchronous non-blocking transport dispatch (`fetch`).
Any future dual-path operation (e.g., `Socket.receive`) must register in `src/capabilities.rs` as the single authoritative source of truth.

### 7.3. Asynchronous Runtime Architecture & Worker Pool
The asynchronous runtime (`src/evaluator/async_runtime.rs`) provides a deterministic, resource-bounded environment:
- **Bounded Worker Pool (`ASYNC_IO_MAX_WORKERS = 4`)**: All asynchronous I/O operations execute across a fixed pool of dedicated worker threads. Tasks are queued in a shared `VecDeque` and woken via `Condvar`, preventing thread exhaustion regardless of how many concurrent awaits are initiated.
- **Notification-Driven Completion**: Completed operations notify waiting evaluators directly via condition variables (`Condvar::wait_timeout`), eliminating busy loops, polling intervals, and CPU churn (`0%` idle CPU usage).
- **Absolute Global Deadline Across Redirects**: Fetch operations establish an absolute deadline (`start_time + timeout_secs`). Redirect chains carry the remaining time calculated against this single deadline, preventing HTTP redirects from resetting the timeout or extending latency indefinitely.
- **Atomic Cancellation & Late Completion Safety**: Pending operations carry an atomic cancellation token (`Arc<AtomicBool>`). If an operation times out or is cancelled, its token is flipped, HTTP redirect loops abort immediately, and any late-arriving results from network workers are cleanly discarded without corrupting evaluator state.
- **Concurrency Ceilings (`DEFAULT_MAX_PENDING_ASYNC_OPERATIONS = 256`)**: The runtime enforces a global ceiling on concurrently allocated operations. Attempting to allocate beyond this ceiling returns a structured, non-catchable `ResourceError` (`SZ6001`).

### 7.4. True Logical Suspension & Resumption
The evaluator implements genuine logical suspension without blocking its OS thread:
- **Suspension Flow (`ExecutionFlow::Suspend(op_id)`)**: Encountering an asynchronous I/O operation unwinds the Rust evaluation call stack, saving continuation frames into the evaluator's `SuspendedContext`.
- **Top-Level Outcomes (`ProgramOutcome::Suspended(op_id)`)**: Schedulers or host loops receive control back immediately when an evaluator suspends (`eval_program_outcome_step`), allowing the host thread to drive multiple evaluators or service external events.
- **Continuation Frames (`ContinuationFrame`)**:
  - `Statement`: Resumes statement assignments (`let`, assign, expression, return, out).
  - `Block`: Preserves remaining statements within blocks without prematurely popping scope.
  - `Function`: Preserves return type checks, async flags, and call depth.
  - `Try`: Preserves `catch` and `finally` blocks across suspension points.
- **Resumption (`resume_suspended_outcome`)**: When the background I/O operation signals completion via `Condvar`, the evaluator replays the continuation frames in exact inverse order, promoting in-arena objects, executing remaining statements, and completing program evaluation.

### 7.5. Error Propagation & Exception Handling
1. **`try / catch / finally`**:
   - Structured exception handling works transparently across suspension boundaries.
   - An exception thrown by an asynchronous operation (e.g. network disconnect, 4xx/5xx status in non-full mode, request timeout) is caught by the surrounding `try / catch` block as if it were thrown synchronously.
   - `finally` blocks are guaranteed to execute upon function return, unhandled throw, or normal exit.
2. **Diagnostic Codes**:
   - Runtime errors retain their normative error codes (`SZ4001`, `SZ4002`, `SZ6001`, `SZ6002`, etc.) and payload messages across suspension points.

---

## 8. Security & Lockdown Parity

Security constraints defined in [`spec/security.md`](../security.md) apply with 100% equivalence to both sync and async paths:
1. **Lockdown Network Restrictions**:
   - Under `--lockdown`, `fetch` is closed by default.
   - Host permissions granted via `--allow-fetch <host>` or `evaluator.allow_fetch_hosts(...)` apply identically to `fetch(url)` and `await fetch(url)`.
   - Unauthorized access raises a non-catchable `PermissionError` in both execution paths.
2. **Resource Ceilings**:
   - The 64 MiB maximum response limit (`SZ6002`) applies identically.
3. **Lexical Unsafe Isolation**:
   - `unsafe` blocks cannot be used to bypass asynchronous security gates or break memory isolation during suspension.

---

## 9. Interplay with `Task` Concurrency

`Task` and `async/await` address orthogonal architectural concerns:
- **`Task`** represents native OS thread worker concurrency with isolated evaluators, arenas, and message queues ([`spec/tasks.md`](../tasks.md)). `Task.run(...)` spawns an independent thread.
- **`async/await`** represents cooperative frame suspension and resumption within a single evaluator.

Rules:
1. `Task` is **NOT** a Promise or Future type.
2. `Task.run(script, arg)` semantics, return types (`int` task ID), and lifecycle polling (`Task.poll`, `Task.isDone`) remain unchanged.
3. A worker running inside a `Task` thread operates its own isolated `Evaluator`. Therefore, worker code **CAN** utilize `async/await` internally without impacting the parent evaluator or sibling workers.
4. An `async fn` cannot be passed directly as a thread entry point to `Task.run`; `Task.run` continues to accept script file paths.

---

## 10. Ecosystem, Tooling & Formatter

1. **Formatter (`vscode-serez/formatter.js`)**:
   - Updated to indent `async fn` and format `await <expr>` expressions cleanly without drift.
2. **Syntax Highlighting (`serez.tmLanguage.json`)**:
   - `async` and `await` are highlighted as standard language control keywords.
3. **LSP Server (`sz-lsp`)**:
   - Document symbols recognize `async` methods and functions, reflecting their symbol kind accurately.

---

## 11. Normative Invariants Summary

| Item | Synchronous Path | Asynchronous Path |
|---|---|---|
| Invocation | `fetch(url)` | `await fetch(url)` |
| Execution Mechanism | Thread-blocking I/O | Evaluator suspension / non-blocking resumption |
| Exposed Return Type | Direct value (`string` / `Dict`) | Direct value (`string` / `Dict`) — identical type |
| Promise / Future Object | None | None |
| Lockdown Enforcement | Governed by `--allow-fetch` | Governed by `--allow-fetch` (Identical) |
| Error Catchability | Catchable via `try/catch` | Catchable via `try/catch` (Identical) |
| Fatal Limits | Enforces 64 MiB ceiling (`SZ6002`) | Enforces 64 MiB ceiling (`SZ6002`) (Identical) |
| Worker Thread Support | Supported inside `Task` workers | Supported inside `Task` workers |
