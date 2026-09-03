//! What one generator call may accumulate.
//!
//! # The gap
//!
//! `fn*` is not lazy. Calling one runs the body to completion and returns an
//! ordinary array of everything it yielded (`spec/control-flow.md`), and the
//! collector was an unbounded vector — so an unbounded generator never returned
//! and grew until the host ran out of memory. `spec/limits.md` measured the
//! shape (about 160 bytes per value, linear) and recorded that a ceiling had
//! been considered and deliberately not added, because no official package uses
//! `fn*` and the largest generator in the conformance suite yields 100.
//!
//! # The decision
//!
//! A ceiling the **host** sets, with a safe default, and no way for the running
//! program to raise its own. Over it is a defined, testable resource error.
//!
//! Deliberately **not** a lazy/streaming redesign of generators: that is a
//! separate architectural change, and this ceiling does not prejudge it.
//!
//! # Why the tests set a small limit
//!
//! The default is a million values — chosen to be invisible to real code — and
//! reaching it in a test would cost about 160 MB and a lot of seconds. The
//! property under test is the *guard*, not the number, so the host sets a small
//! limit and the boundary is exercised there. That the default is what
//! `spec/limits.md` says is asserted separately, from the constant itself.

use serez_code::evaluator::DEFAULT_GENERATOR_YIELD_LIMIT;
use serez_code::run::{RunFailure, RunOpts, run_source_detailed};

/// Run a program whose generator yields `count` values, under a host limit.
fn yielding(count: usize, limit: usize) -> (i32, Option<RunFailure>) {
    let source = format!(
        "fn* int upTo(int n) {{\n\
         \x20   for (let i = 0; i < n; i = i + 1) {{ yield i; }}\n\
         }}\n\
         let got = upTo({count});\n\
         out got.length();\n"
    );
    let outcome = run_source_detailed(
        source,
        "<generator-ceiling>",
        RunOpts {
            generator_yield_limit: Some(limit),
            ..RunOpts::default()
        },
    );
    (outcome.exit_code, outcome.failure)
}

fn is_resource_error(failure: &Option<RunFailure>) -> bool {
    matches!(failure, Some(RunFailure::Runtime(e)) if e.kind.as_deref() == Some("ResourceError"))
}

#[test]
fn a_generator_just_under_the_limit_completes() {
    let (code, failure) = yielding(9, 10);
    assert_eq!(code, 0, "9 of 10 must complete: {failure:?}");
}

#[test]
fn a_generator_exactly_at_the_limit_completes() {
    // The boundary. A guard written `>` instead of `>=`, or checked after the
    // push instead of before it, moves this case.
    let (code, failure) = yielding(10, 10);
    assert_eq!(code, 0, "exactly the limit must complete: {failure:?}");
}

#[test]
fn a_generator_one_value_over_the_limit_is_stopped() {
    let (code, failure) = yielding(11, 10);
    assert_ne!(code, 0, "one over the limit must fail");
    assert!(
        is_resource_error(&failure),
        "expected a ResourceError, got {failure:?}"
    );
}

#[test]
fn a_clearly_runaway_generator_is_stopped_the_same_way() {
    // The case the ceiling exists for, at a size that used to grow until the
    // host gave out. It stops in milliseconds now.
    let (code, failure) = yielding(5_000_000, 1_000);
    assert_ne!(code, 0);
    assert!(
        is_resource_error(&failure),
        "expected a ResourceError, got {failure:?}"
    );
}

#[test]
fn the_message_names_the_limit_that_was_hit() {
    // A resource error a user cannot act on is a crash with better manners.
    let (_, failure) = yielding(11, 10);
    let Some(RunFailure::Runtime(error)) = failure else {
        panic!("expected a runtime failure");
    };
    assert!(
        error.message.contains("10"),
        "the message must name the limit: {}",
        error.message
    );
    assert_eq!(error.code, "SZ6002", "{error:?}");
}

#[test]
fn the_ceiling_is_not_catchable() {
    // Like every other entry in `spec/limits.md`. A `try` around a
    // memory-exhaustion guard would let a program keep going past one.
    let outcome = run_source_detailed(
        "fn* int forever() { for (let i = 0; i < 1000; i = i + 1) { yield i; } }\n\
         try { let got = forever(); out got.length(); } catch (e) { out \"caught\"; }\n\
         out \"after\";\n"
            .to_string(),
        "<generator-ceiling>",
        RunOpts {
            generator_yield_limit: Some(5),
            ..RunOpts::default()
        },
    );
    assert_ne!(
        outcome.exit_code, 0,
        "a resource ceiling must not be catchable"
    );
}

#[test]
fn an_ordinary_generator_is_unaffected_by_the_default() {
    // The positive control. Without it, a guard that fired on every `yield`
    // would satisfy every "must be stopped" case above — and the largest
    // generator in the conformance suite yields 100, so this is the shape real
    // code has.
    let outcome = run_source_detailed(
        "fn* int upTo(int n) { for (let i = 0; i < n; i = i + 1) { yield i; } }\n\
         let got = upTo(100);\n\
         assert(got.length() == 100, \"a 100-value generator must be unaffected\");\n"
            .to_string(),
        "<generator-ceiling>",
        RunOpts::default(),
    );
    assert_eq!(
        outcome.exit_code, 0,
        "the default must be invisible to real code: {:?}",
        outcome.failure
    );
}

#[test]
fn the_default_is_the_one_the_specification_names() {
    // `spec/limits.md` promises a million. Asserted from the constant so the
    // document and the build cannot drift apart silently.
    assert_eq!(DEFAULT_GENERATOR_YIELD_LIMIT, 1_000_000);
}
