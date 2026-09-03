//! The boundary between *what the language did* and *what failed*.
//!
//! # Why this file exists
//!
//! `MATURITY_AUDIT.md` carried, under M6: *"`EvalResult` mixes values, control
//! flow, throw and an untyped `Error` sentinel"*. It was one enum with eight
//! variants — seven legal states of a running program and one fault — and every
//! `match` over it had to decide per arm which of the two it was looking at.
//! `EvalResult::Throw(v)` and `EvalResult::Error` sat one line apart in the
//! declaration and meant entirely different things about whether the program was
//! still running.
//!
//! It is now `Result<ExecutionFlow, RuntimeFailure>`: `Ok` is the language doing
//! something, `Err` is the runtime failing.
//!
//! # What this pins
//!
//! Not the type — the compiler does that. What a type cannot check is that the
//! *split runs along the right line*: that a `throw` is still control flow after
//! the change and a division by zero is still a failure, and that the two
//! behave as differently as they did before. That is the property a mechanical
//! rewrite of 1,600 call sites could break silently, and it is what these
//! assertions are for.
//!
//! `tests/runtime_outcome.rs` pins the payloads at the same boundary; this pins
//! which side of it each thing lands on.

use serez_code::run::{RunFailure, RunOpts, run_source_detailed};

fn outcome(src: &str) -> (i32, Option<RunFailure>) {
    let result = run_source_detailed(
        src.to_string(),
        "<execution-flow>",
        RunOpts {
            permissions: vec![],
            ..RunOpts::default()
        },
    );
    (result.exit_code, result.failure)
}

/// A `throw` that nothing catches is an *uncaught exception*, not a runtime
/// failure. It is `ExecutionFlow::Throw` all the way up, and only becomes a
/// program-level failure when it runs out of program to propagate through.
#[test]
fn an_uncaught_throw_is_an_exception_and_not_a_runtime_failure() {
    let (code, failure) = outcome("throw \"boom\";\n");
    assert_eq!(code, 1);
    assert!(
        matches!(failure, Some(RunFailure::UncaughtException { .. })),
        "a throw became {failure:?}"
    );
}

/// The other side of the same line: a division by zero is a runtime failure. It
/// never was control flow and must not have become it.
#[test]
fn a_division_by_zero_is_a_runtime_failure_and_not_control_flow() {
    let (code, failure) = outcome("out (1 / 0);\n");
    assert_eq!(code, 1);
    assert!(
        matches!(failure, Some(RunFailure::Runtime(_))),
        "a division by zero became {failure:?}"
    );
}

/// A caught `throw` leaves no failure at all — the program completes.
///
/// This is the assertion that would fail if `Throw` had been moved to the error
/// side of the `Result`: propagating it would stop looking like control flow,
/// and `try/catch` would have nothing to catch.
#[test]
fn a_caught_throw_leaves_the_program_running() {
    let (code, failure) = outcome("try { throw \"boom\"; } catch (e) { out \"caught\"; }\n");
    assert_eq!(code, 0, "a caught throw must not fail the program");
    assert!(failure.is_none(), "a caught throw left {failure:?}");
}

/// A caught *runtime* failure also leaves the program running — the two look
/// identical from the outside once caught, which is exactly why the distinction
/// has to live in the type rather than in the observed outcome.
#[test]
fn a_caught_runtime_failure_leaves_the_program_running() {
    let (code, failure) = outcome("try { out (1 / 0); } catch (e) { out \"caught\"; }\n");
    assert_eq!(code, 0);
    assert!(failure.is_none(), "a caught runtime error left {failure:?}");
}

/// A failure the language forbids `try/catch` from consuming stays a failure.
///
/// Security gates and resource ceilings raise through the same `Err` channel and
/// carry `catchable: false`. If the split had collapsed that flag, this program
/// would exit 0.
#[test]
fn a_fatal_failure_is_not_catchable_after_the_split() {
    let (code, failure) = outcome(
        "try { let p = Memory.alloc(0); } catch (e) { out \"caught\"; }\n\
         out \"after\";\n",
    );
    assert_ne!(
        code, 0,
        "a security/ceiling failure must not be turned into ordinary control flow"
    );
    assert!(
        matches!(failure, Some(RunFailure::Runtime(_))),
        "expected a runtime failure, got {failure:?}"
    );
}

/// `return`, `break` and `continue` in their legal places are `Ok`, and produce
/// no failure.
#[test]
fn ordinary_control_flow_produces_no_failure() {
    for src in [
        "fn int f() { return 1; }\nout f();\n",
        "for (let i = 0; i < 3; i = i + 1) { if (i == 1) { continue; } out i; }\n",
        "let i = 0;\nwhile (true) { i = i + 1; if (i > 2) { break; } }\nout i;\n",
        "outer: for (let i = 0; i < 2; i = i + 1) { for (let j = 0; j < 2; j = j + 1) { break outer; } }\nout \"done\";\n",
    ] {
        let (code, failure) = outcome(src);
        assert_eq!(code, 0, "program failed: {src}");
        assert!(failure.is_none(), "{src} produced {failure:?}");
    }
}

/// Control flow with nowhere legal to go is *still not* a runtime failure — it
/// has its own classification, and the split must not have folded it into one.
#[test]
fn control_flow_with_no_consumer_keeps_its_own_classification() {
    let (code, failure) = outcome("return 1;\n");
    assert_eq!(code, 1);
    assert!(
        matches!(failure, Some(RunFailure::InvalidControlFlow(_))),
        "a top-level return became {failure:?}"
    );
}
