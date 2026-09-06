//! Integration tests for DEC-ASYNC-001: Dual Sync/Async Execution Architecture.

use serez_code::ast::{Expression, Statement};
use serez_code::evaluator::{
    ACTIVE_WORKER_THREADS, ASYNC_IO_MAX_WORKERS, ASYNC_MAX_PENDING_OPERATIONS, Evaluator,
    ProgramOutcome, TIMED_OUT_OPERATIONS_COUNT, get_runtime, test_get_fetch_transport_counts,
    test_reset_async_metrics, test_reset_fetch_transport_counts,
};
use serez_code::lexer::Lexer;
use serez_code::parser::Parser;
use serez_code::run::{RunOpts, run_source_detailed};
use serez_code::semantic::validate::validate;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Mutex;

static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn parse_program(src: &str) -> (serez_code::ast::Program, Parser) {
    let lexer = Lexer::new(src.to_string());
    let mut parser = Parser::new(lexer);
    parser.set_source(src.lines().map(str::to_string).collect());
    let program = parser.parse_program();
    (program, parser)
}

// ── 1. Syntax & Parser Conformance ──────────────────────────────────────────

#[test]
fn async_fn_declaration_sets_is_async() {
    let (prog, parser) = parse_program("async fn int doWork(int a) { return a + 1; }\n");
    assert!(!parser.has_errors(), "errors: {:?}", parser.take_errors());
    assert_eq!(prog.statements.len(), 1);
    match &prog.statements[0] {
        Statement::FunctionDeclaration(f) => {
            assert_eq!(f.name, "doWork");
            assert!(f.function.is_async, "function must have is_async = true");
        }
        other => panic!("expected FunctionDeclaration, got {other:?}"),
    }
}

#[test]
fn async_fn_literal_sets_is_async() {
    let (prog, parser) = parse_program("let f = async fn() { return 42; };\n");
    assert!(!parser.has_errors(), "errors: {:?}", parser.take_errors());
    match &prog.statements[0] {
        Statement::Let(l) => match &l.value {
            Expression::FunctionLiteral(func) => {
                assert!(func.is_async, "literal must have is_async = true");
            }
            other => panic!("expected FunctionLiteral, got {other:?}"),
        },
        other => panic!("expected Let, got {other:?}"),
    }
}

#[test]
fn class_async_methods_set_is_async() {
    let src = r#"
    class Client {
        public Client() {}
        public async string get(string path) { return path; }
        public async int ping() { return 1; }
        public static async void warmup() {}
    }
    "#;
    let (prog, parser) = parse_program(src);
    assert!(!parser.has_errors(), "errors: {:?}", parser.take_errors());
    match &prog.statements[0] {
        Statement::ClassDeclaration(c) => {
            assert_eq!(c.methods.len(), 3);
            let get_m = c.methods.iter().find(|m| m.name == "get").unwrap();
            assert!(get_m.is_async);
            assert!(!get_m.is_static);

            let ping_m = c.methods.iter().find(|m| m.name == "ping").unwrap();
            assert!(ping_m.is_async);
            assert!(ping_m.is_public);

            let warmup_m = c.methods.iter().find(|m| m.name == "warmup").unwrap();
            assert!(warmup_m.is_async);
            assert!(warmup_m.is_static);
        }
        other => panic!("expected ClassDeclaration, got {other:?}"),
    }
}

#[test]
fn async_constructors_are_rejected() {
    let src = "class Bad { public async Bad() {} }\n";
    let (_, parser) = parse_program(src);
    assert!(parser.has_errors(), "async constructor must be rejected");
    let errors = parser.take_errors();
    let has_ctor_err = errors
        .iter()
        .any(|d| d.message.contains("constructors cannot be declared async"));
    assert!(
        has_ctor_err,
        "expected constructor error diagnostic, got {errors:?}"
    );
}

#[test]
fn async_generators_are_rejected() {
    let src = "async fn* gen() { yield 1; }\n";
    let (_, parser) = parse_program(src);
    assert!(parser.has_errors(), "async fn* must be rejected");
    let errors = parser.take_errors();
    let has_gen_err = errors
        .iter()
        .any(|d| d.message.to_lowercase().contains("async"));
    assert!(
        has_gen_err,
        "expected async generator rejection diagnostic, got {errors:?}"
    );
}

#[test]
fn await_expression_precedence() {
    let (prog, parser) = parse_program("let x = await fetch(url) + 1;\n");
    assert!(!parser.has_errors(), "errors: {:?}", parser.take_errors());
    match &prog.statements[0] {
        Statement::Let(l) => match &l.value {
            Expression::Infix(infix) => {
                assert_eq!(infix.operator, "+");
                assert!(matches!(infix.left.as_ref(), Expression::Await { .. }));
            }
            other => panic!("expected Infix, got {other:?}"),
        },
        other => panic!("expected Let, got {other:?}"),
    }
}

// ── 2. Semantic Analysis Conformance ────────────────────────────────────────

#[test]
fn calling_async_fn_without_await_is_semantic_error() {
    let (prog, _) = parse_program("async fn compute() { return 10; }\ncompute();\n");
    let findings = validate(&prog);
    assert_eq!(findings.len(), 1);
    assert!(
        findings[0]
            .message
            .contains("async function 'compute' must be awaited"),
        "unexpected: {}",
        findings[0].message
    );
}

#[test]
fn calling_async_fn_with_await_passes_semantic_validation() {
    let (prog, _) = parse_program("async fn compute() { return 10; }\nawait compute();\n");
    let findings = validate(&prog);
    assert!(findings.is_empty(), "unexpected findings: {:?}", findings);
}

#[test]
fn awaiting_sync_only_builtin_is_semantic_error() {
    for (src, name) in [
        ("await parseInt(\"123\");\n", "parseInt"),
        ("await parseDecimal(\"1.2\");\n", "parseDecimal"),
        ("await Math.sqrt(4.0);\n", "Math.sqrt"),
        ("await JSON.parse(\"{}\");\n", "JSON.parse"),
        ("await JSON.stringify(1);\n", "JSON.stringify"),
        ("await assert(true, \"ok\");\n", "assert"),
        ("await type_of(42);\n", "type_of"),
        ("await OS.platform();\n", "OS.platform"),
        ("await File.read(\"test.txt\");\n", "File.read"),
        ("await Crypto.sha256(\"data\");\n", "Crypto.sha256"),
    ] {
        let (prog, _) = parse_program(src);
        let findings = validate(&prog);
        assert_eq!(findings.len(), 1, "failed for {src}");
        assert!(
            findings[0].message.contains(&format!(
                "operation '{name}' is synchronous and does not support 'await'"
            )),
            "unexpected message for {src}: {}",
            findings[0].message
        );
    }
}

// ── 3. Runtime Execution ───────────────────────────────────────────────────

#[test]
fn await_evaluates_async_functions_correctly() {
    let src = r#"
    async fn int add(int a, int b) {
        return a + b;
    }
    let sum = await add(20, 22);
    assert(sum == 42, "sum must be 42");
    "#;
    let outcome = run_source_detailed(src.to_string(), "<test>", RunOpts::default());
    assert_eq!(outcome.exit_code, 0, "failure: {:?}", outcome.failure);
}

#[test]
fn await_class_methods_and_static_methods() {
    let src = r#"
    class Calculator {
        public Calculator(int m) {
            this.multiplier = m;
        }
        public async int calculate(int val) {
            return val * this.multiplier;
        }
        public static async string version() {
            return "v11.1.0";
        }
    }
    let calc = new Calculator(10);
    let res = await calc.calculate(5);
    assert(res == 50, "instance method must return 50");

    let ver = await Calculator.version();
    assert(ver == "v11.1.0", "static method must return version string");
    "#;
    let outcome = run_source_detailed(src.to_string(), "<test>", RunOpts::default());
    assert_eq!(outcome.exit_code, 0, "failure: {:?}", outcome.failure);
}

#[test]
fn try_catch_finally_across_async_await() {
    let src = r#"
    async fn failing() {
        throw "async boom";
    }
    let caught = false;
    let finalized = false;
    try {
        await failing();
    } catch (e) {
        caught = true;
        assert(e == "async boom", "caught message mismatch");
    } finally {
        finalized = true;
    }
    assert(caught == true, "error must be caught");
    assert(finalized == true, "finally must execute");
    "#;
    let outcome = run_source_detailed(src.to_string(), "<test>", RunOpts::default());
    assert_eq!(outcome.exit_code, 0, "failure: {:?}", outcome.failure);
}

#[test]
fn awaiting_sync_only_builtin_runtime_is_type_error() {
    let src = "await parseInt(\"123\");\n";
    let outcome = run_source_detailed(src.to_string(), "<test>", RunOpts::default());
    assert_ne!(outcome.exit_code, 0);
    let err = format!("{:?}", outcome.failure);
    assert!(
        err.contains("operation 'parseInt' is synchronous and does not support 'await'"),
        "expected sync-only error, got: {err}"
    );
}

#[test]
fn calling_async_method_without_await_is_runtime_error() {
    let src = r#"
    class Service {
        public Service() {}
        public async int ping() { return 1; }
    }
    let s = new Service();
    s.ping();
    "#;
    let outcome = run_source_detailed(src.to_string(), "<test>", RunOpts::default());
    assert_ne!(outcome.exit_code, 0);
    let err = format!("{:?}", outcome.failure);
    assert!(
        err.contains("async method 'ping' must be awaited with 'await'"),
        "expected async method await error, got: {err}"
    );
}

// ── 4. Local HTTP Server & Dual-Path Parity ─────────────────────────────────

const MOCK_BODY: &str = r#"{"status":"active","code":100,"items":["alpha","beta"]}"#;

fn spawn_test_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.split_whitespace().nth(1).unwrap_or("/");

            let resp = match path {
                "/data" => format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{MOCK_BODY}",
                    MOCK_BODY.len()
                ),
                "/error" => "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\nConnection: close\r\n\r\nerror".to_string(),
                _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
            };
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    port
}

#[test]
fn fetch_sync_and_async_return_identical_body() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let port = spawn_test_server();
    let src = format!(
        r#"
        let syncBody = fetch("http://127.0.0.1:{port}/data");
        let asyncBody = await fetch("http://127.0.0.1:{port}/data");
        assert(syncBody == asyncBody, "sync and async body must match exactly");
        assert(type_of(syncBody) == type_of(asyncBody), "type_of must be identical");
        "#
    );
    let outcome = run_source_detailed(src, "<test>", RunOpts::default());
    assert_eq!(outcome.exit_code, 0, "failure: {:?}", outcome.failure);
}

#[test]
fn fetch_full_mode_returns_identical_dict() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let port = spawn_test_server();
    let src = format!(
        r#"
        let opts <string, any> = ({{"full", true}});
        let syncResp = fetch("http://127.0.0.1:{port}/data", opts);
        let asyncResp = await fetch("http://127.0.0.1:{port}/data", opts);

        assert(syncResp.status == 200, "status must be 200");
        assert(asyncResp.status == 200, "async status must be 200");
        assert(syncResp.status == asyncResp.status, "status must match");
        assert(syncResp.ok == asyncResp.ok, "ok must match");
        assert(syncResp.body == asyncResp.body, "body must match");
        "#
    );
    let outcome = run_source_detailed(src, "<test>", RunOpts::default());
    assert_eq!(outcome.exit_code, 0, "failure: {:?}", outcome.failure);
}

#[test]
fn fetch_error_status_throws_identically() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let port = spawn_test_server();
    let src = format!(
        r#"
        let syncCaught = false;
        let asyncCaught = false;

        try {{
            fetch("http://127.0.0.1:{port}/error");
        }} catch (e) {{
            syncCaught = true;
        }}

        try {{
            await fetch("http://127.0.0.1:{port}/error");
        }} catch (e) {{
            asyncCaught = true;
        }}

        assert(syncCaught == true, "sync error must throw");
        assert(asyncCaught == true, "async error must throw");
        "#
    );
    let outcome = run_source_detailed(src, "<test>", RunOpts::default());
    assert_eq!(outcome.exit_code, 0, "failure: {:?}", outcome.failure);
}

#[test]
fn lockdown_parity_blocks_both_without_allow_fetch() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let port = spawn_test_server();

    // Sync path under lockdown
    let opts_sync = RunOpts {
        lockdown: true,
        ..RunOpts::default()
    };
    let sync_src = format!("fetch(\"http://127.0.0.1:{port}/data\");\n");
    let sync_res = run_source_detailed(sync_src, "<test>", opts_sync);
    assert_ne!(
        sync_res.exit_code, 0,
        "sync fetch without permission must fail"
    );

    // Async path under lockdown
    let opts_async = RunOpts {
        lockdown: true,
        ..RunOpts::default()
    };
    let async_src = format!("await fetch(\"http://127.0.0.1:{port}/data\");\n");
    let async_res = run_source_detailed(async_src, "<test>", opts_async);
    assert_ne!(
        async_res.exit_code, 0,
        "async fetch without permission must fail"
    );

    // Both succeed when host is allowed
    let opts_allowed = RunOpts {
        lockdown: true,
        fetch_allowlist: vec!["127.0.0.1".into()],
        ..RunOpts::default()
    };
    let allowed_src = format!(
        r#"
        let s = fetch("http://127.0.0.1:{port}/data");
        let a = await fetch("http://127.0.0.1:{port}/data");
        assert(s == a, "both must succeed with permission");
        "#
    );
    let allowed_res = run_source_detailed(allowed_src, "<test>", opts_allowed);
    assert_eq!(
        allowed_res.exit_code, 0,
        "allowed fetch failed: {:?}",
        allowed_res.failure
    );
}

// ── 5. Transport Counter Differentiation & AST Leak Prevention ──────────────

#[test]
fn await_fetch_takes_async_transport() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    test_reset_fetch_transport_counts();

    let port = spawn_test_server();
    let src = format!("await fetch(\"http://127.0.0.1:{port}/data\");\n");
    let outcome = run_source_detailed(src, "<test>", RunOpts::default());
    assert_eq!(outcome.exit_code, 0, "failure: {:?}", outcome.failure);

    let (sync_cnt, async_cnt) = test_get_fetch_transport_counts();
    assert_eq!(
        async_cnt, 1,
        "await fetch MUST take the async transport (got {async_cnt})"
    );
    assert_eq!(
        sync_cnt, 0,
        "await fetch must NOT increment sync counter (got {sync_cnt})"
    );
}

#[test]
fn fetch_without_await_takes_sync_transport() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    test_reset_fetch_transport_counts();

    let port = spawn_test_server();
    let src = format!("fetch(\"http://127.0.0.1:{port}/data\");\n");
    let outcome = run_source_detailed(src, "<test>", RunOpts::default());
    assert_eq!(outcome.exit_code, 0, "failure: {:?}", outcome.failure);

    let (sync_cnt, async_cnt) = test_get_fetch_transport_counts();
    assert_eq!(
        sync_cnt, 1,
        "fetch without await MUST take the sync transport (got {sync_cnt})"
    );
    assert_eq!(
        async_cnt, 0,
        "fetch without await must NOT increment async counter (got {async_cnt})"
    );
}

#[test]
fn await_wrapper_with_fetch_argument_takes_sync_path_for_argument() {
    // Crucial anti-regression test:
    // In `await wrapper(fetch(url))`, `wrapper` is awaited (async path),
    // but the argument `fetch(url)` MUST evaluate synchronously and NOT leak `is_await = true`!
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    test_reset_fetch_transport_counts();

    let port = spawn_test_server();
    let src = format!(
        r#"
        async fn string wrapper(string payload) {{
            return payload;
        }}
        let res = await wrapper(fetch("http://127.0.0.1:{port}/data"));
        assert(type_of(res) == "string");
        "#
    );
    let outcome = run_source_detailed(src, "<test>", RunOpts::default());
    assert_eq!(outcome.exit_code, 0, "failure: {:?}", outcome.failure);

    let (sync_cnt, async_cnt) = test_get_fetch_transport_counts();
    assert_eq!(
        sync_cnt, 1,
        "fetch in call argument MUST evaluate via sync transport (got {sync_cnt})"
    );
    assert_eq!(
        async_cnt, 0,
        "fetch in call argument MUST NOT inherit await mode from wrapper call (got {async_cnt})"
    );
}

#[test]
fn fetch_without_await_inside_async_fn_takes_sync_transport() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    test_reset_fetch_transport_counts();

    let port = spawn_test_server();
    let src = format!(
        r#"
        async fn string worker() {{
            let s = fetch("http://127.0.0.1:{port}/data");
            return s;
        }}
        let res = await worker();
        assert(type_of(res) == "string");
        "#
    );
    let outcome = run_source_detailed(src, "<test>", RunOpts::default());
    assert_eq!(outcome.exit_code, 0, "failure: {:?}", outcome.failure);

    let (sync_cnt, async_cnt) = test_get_fetch_transport_counts();
    assert_eq!(
        sync_cnt, 1,
        "unawaited fetch inside async fn MUST take sync transport (got {sync_cnt})"
    );
    assert_eq!(
        async_cnt, 0,
        "unawaited fetch inside async fn must NOT increment async counter (got {async_cnt})"
    );
}

// ── 6. Async Runtime Architecture & Suspension Conformance ─────────────────

#[test]
fn evaluator_logical_suspension_and_resume_preserves_thread() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let port = spawn_test_server();
    let src = format!(
        r#"
        let msg = "hello";
        let body = await fetch("http://127.0.0.1:{port}/data");
        let res = msg + ": " + body;
        res;
        "#
    );
    let (prog, parser) = parse_program(&src);
    assert!(
        !parser.has_errors(),
        "parser errors: {:?}",
        parser.take_errors()
    );

    let mut eval = Evaluator::new();
    let outcome = eval.eval_program_outcome_step(&prog);

    let op_id = match outcome {
        ProgramOutcome::Suspended(id) => id,
        other => panic!("expected Suspended outcome, got {other:?}"),
    };

    assert!(
        eval.is_suspended(),
        "evaluator must report is_suspended == true"
    );
    assert_eq!(eval.suspended_operation_id(), Some(op_id));

    // The evaluator yielded control while the async worker completes the HTTP request.
    // Calling resume_suspended_outcome drives it to completion.
    let resumed_outcome = eval.resume_suspended_outcome(op_id);
    match resumed_outcome {
        ProgramOutcome::Value(v) => {
            let owned = eval.extract(v);
            match owned {
                serez_code::region::OwnedValue::Str(s) => {
                    assert!(
                        s.contains("hello: {\"status\":\"active\""),
                        "unexpected value: {s}"
                    );
                }
                other => panic!("expected string value upon resume, got {other:?}"),
            }
        }
        other => panic!("expected Value outcome upon resume, got {other:?}"),
    }

    assert!(
        !eval.is_suspended(),
        "evaluator must not be suspended after resume"
    );
}

#[test]
fn thread_scaling_worker_pool_ceiling() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    test_reset_async_metrics();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 512];
                let _ = stream.read(&mut buf);
                std::thread::sleep(std::time::Duration::from_millis(60));
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                );
                let _ = stream.flush();
            });
        }
    });

    let mut handles = Vec::new();
    for _ in 0..16 {
        let port_copy = port;
        let handle = std::thread::spawn(move || {
            let src = format!("await fetch(\"http://127.0.0.1:{port_copy}/\");\n");
            let outcome = run_source_detailed(src, "<test>", RunOpts::default());
            assert_eq!(outcome.exit_code, 0, "failure: {:?}", outcome.failure);
        });
        handles.push(handle);
    }

    let mut peak_active = 0;
    for _ in 0..12 {
        let active = ACTIVE_WORKER_THREADS.load(std::sync::atomic::Ordering::SeqCst);
        if active > peak_active {
            peak_active = active;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert!(
        peak_active <= ASYNC_IO_MAX_WORKERS,
        "peak active workers ({peak_active}) must not exceed pool limit ({ASYNC_IO_MAX_WORKERS})"
    );
}

#[test]
fn async_timeout_triggers_cancellation_and_late_completion_is_safe() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    test_reset_async_metrics();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 512];
                let _ = stream.read(&mut buf);
                // Server sleeps 1200ms before responding
                std::thread::sleep(std::time::Duration::from_millis(1200));
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nlate",
                );
                let _ = stream.flush();
            });
        }
    });

    let src = format!(
        r#"
        let opts <string, any> = ({{"timeout", 1}});
        try {{
            await fetch("http://127.0.0.1:{port}/", opts);
            assert(false, "must have timed out");
        }} catch (err) {{
            assert(err.contains("timed out") || err.contains("timeout"), "actual err: " + err);
        }}
        "#
    );

    let outcome = run_source_detailed(src, "<test>", RunOpts::default());
    assert_eq!(outcome.exit_code, 0, "failure: {:?}", outcome.failure);

    let timed_out = TIMED_OUT_OPERATIONS_COUNT.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        timed_out >= 1,
        "timed out count should be >= 1, got {timed_out}"
    );

    // Wait for the background worker to finish and complete safely
    std::thread::sleep(std::time::Duration::from_millis(1300));
}

#[test]
fn async_redirect_shares_absolute_deadline_across_hops() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 512];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req.split_whitespace().nth(1).unwrap_or("/");

                match path {
                    "/hop1" => {
                        std::thread::sleep(std::time::Duration::from_millis(400));
                        let resp = format!(
                            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/hop2\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        );
                        let _ = stream.write_all(resp.as_bytes());
                    }
                    "/hop2" => {
                        std::thread::sleep(std::time::Duration::from_millis(400));
                        let resp = format!(
                            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/hop3\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        );
                        let _ = stream.write_all(resp.as_bytes());
                    }
                    "/hop3" => {
                        std::thread::sleep(std::time::Duration::from_millis(400));
                        let resp =
                            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndone";
                        let _ = stream.write_all(resp.as_bytes());
                    }
                    _ => {}
                }
                let _ = stream.flush();
            });
        }
    });

    let start = std::time::Instant::now();
    let src = format!(
        r#"
        let opts <string, any> = ({{"timeout", 1}});
        try {{
            await fetch("http://127.0.0.1:{port}/hop1", opts);
            assert(false, "redirect chain exceeding total timeout must fail");
        }} catch (err) {{
            assert(err.contains("timed out") || err.contains("timeout"), "actual err: " + err);
        }}
        "#
    );
    let outcome = run_source_detailed(src, "<test>", RunOpts::default());
    let elapsed = start.elapsed();
    assert_eq!(outcome.exit_code, 0, "failure: {:?}", outcome.failure);
    assert!(
        elapsed < std::time::Duration::from_millis(2200),
        "timeout must abort at global deadline (~1s), but took {elapsed:?}"
    );
}

#[test]
fn pending_operation_ceiling_enforces_limit() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let runtime = get_runtime();

    let mut allocated = Vec::new();
    // Allocate up to the ceiling
    for _ in 0..ASYNC_MAX_PENDING_OPERATIONS {
        let entry = runtime
            .allocate_operation(10)
            .expect("should allocate up to max pending operations");
        allocated.push(entry.id);
    }

    // 257th allocation must fail
    let err = runtime
        .allocate_operation(10)
        .err()
        .expect("exceeding max pending operations must fail");
    assert!(
        err.contains("maximum pending async operations"),
        "unexpected error message: {err}"
    );

    // Clean up allocated operations
    for id in allocated {
        runtime.complete(id, Err("cleanup".to_string()));
        runtime.remove_operation(id);
    }
}

#[test]
fn parallel_evaluators_with_async_fetch() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let port = spawn_test_server();
    let mut handles = Vec::new();

    for i in 0..4 {
        let port_copy = port;
        handles.push(std::thread::spawn(move || {
            let src = format!(
                r#"
                let a = await fetch("http://127.0.0.1:{port_copy}/data");
                let b = fetch("http://127.0.0.1:{port_copy}/data");
                assert(a == b, "worker {i} results must match");
                assert(a.length() > 0);
                "#
            );
            let outcome = run_source_detailed(src, "<test>", RunOpts::default());
            assert_eq!(
                outcome.exit_code, 0,
                "worker {i} failed: {:?}",
                outcome.failure
            );
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}
