//! The `unsafe { }` contract, as behaviour rather than as prose.
//!
//! # Safe by default
//!
//! There is no `safe` keyword and there will not be one. Ordinary code runs
//! under the runtime's guarantees because that is the default, and `unsafe` is
//! the only explicit syntax — a deliberate step outside **named** guarantees,
//! not outside "safety".
//!
//! # Permissions are not `unsafe`, and neither substitutes for the other
//!
//! Measured against the release binary, three programs and three outcomes:
//!
//! ```text
//! unsafe { OS.exec(…) }                        SZ6001  no permission
//! use permissions { OS }  OS.exec(…)           SZ6003  no unsafe
//! use permissions { OS }  unsafe { OS.exec }   runs
//! ```
//!
//! A permission says the program is *authorised to reach a capability*. `unsafe`
//! says the author *accepts a named relaxation*. The first two rows are the
//! whole point and are asserted below in both directions.
//!
//! # What `unsafe` does not do
//!
//! It does not grant permissions, lift lockdown, skip argument validation, open
//! protected paths, or relax any limit that `execution::Guarantee` does not
//! list. Each of those is a test here rather than a sentence.
//!
//! # How it propagates — measured, not chosen
//!
//! Serez's `unsafe` is **dynamic**: a function *called* from inside a block runs
//! with the context in effect, though its body contains no `unsafe` keyword.
//! `spec/security.md` said "lexically", which is not what the runtime does; the
//! divergence is **DEC-M11-001** and this file pins the measured behaviour so
//! that answering the decision is a deliberate act.

use serez_code::run::{RunOpts, run_source_detailed};

/// Run a snippet the way `sz file.sz` does: permissions grantable, no lockdown.
fn run(source: &str) -> (i32, String) {
    outcome(source, RunOpts::default())
}

/// Run it the way `--eval` does: lockdown, nothing grantable.
fn run_locked(source: &str) -> (i32, String) {
    outcome(source, RunOpts::sandboxed())
}

fn outcome(source: &str, opts: RunOpts) -> (i32, String) {
    let result = run_source_detailed(source.to_string(), "<unsafe-contract>", opts);
    let message = match &result.failure {
        Some(failure) => format!("{:?}", failure),
        None => String::new(),
    };
    (result.exit_code, message)
}

/// A child that prints one short line, spelled for this platform.
fn echo() -> &'static str {
    if cfg!(windows) {
        "\"cmd\", [\"/c\", \"echo\", \"hi\"]"
    } else {
        "\"sh\", [\"-c\", \"echo hi\"]"
    }
}

// ── 1, 2: safe by default, and no `safe` keyword ────────────────────────────

/// Ordinary code needs no marker to be safe.
#[test]
fn ordinary_code_is_safe_without_saying_so() {
    let (code, message) = run("let x = 1;\nout x + 1;\n");
    assert_eq!(code, 0, "ordinary code did not run: {message}");
}

/// `safe` is not a keyword, and using it as an identifier proves it.
///
/// A reserved word cannot be a variable name. If `safe` were ever added to the
/// lexer this test fails, which is the intent: the decision is that it is not a
/// keyword, and a test is how that survives someone's good idea.
#[test]
fn there_is_no_safe_keyword() {
    let (code, message) = run("let safe = 1;\nout safe;\n");
    assert_eq!(
        code, 0,
        "`safe` is not a keyword and must remain usable as an identifier: {message}"
    );

    // And it is not a block form either.
    let (code, _) = run("safe { out 1; }\n");
    assert_ne!(code, 0, "`safe {{ }}` parsed as a block form");
}

// ── 3, 4, 5, 6: permissions and unsafe are independent ──────────────────────

/// `unsafe` does not grant the OS permission.
#[test]
fn unsafe_without_the_permission_is_refused() {
    let (code, message) = run(&format!("unsafe {{ OS.exec({}); }}\n", echo()));
    assert_ne!(code, 0, "unsafe granted a permission it does not own");
    assert!(
        message.contains("SZ6001") || message.contains("requires permission"),
        "refused, but not for want of the permission: {message}"
    );
}

/// And the OS permission does not authorise an operation that needs `unsafe`.
#[test]
fn the_permission_without_unsafe_is_refused() {
    let (code, message) = run(&format!("use permissions {{ OS }}\nOS.exec({});\n", echo()));
    assert_ne!(code, 0, "a permission stood in for unsafe");
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "refused, but not for want of unsafe: {message}"
    );
}

/// Both together run.
///
/// The positive control for the two above: without it, a build that refused
/// every `OS.exec` would satisfy both.
#[test]
fn the_permission_and_unsafe_together_run() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\nunsafe {{ let r = OS.exec({}); out r.code; }}\n",
        echo()
    ));
    assert_eq!(code, 0, "permission + unsafe was still refused: {message}");
}

/// The same separation holds for a second operation, so the rule is the model
/// and not a quirk of `OS.exec`.
#[test]
fn the_separation_is_not_specific_to_os_exec() {
    let (code, message) = run("use permissions { Terminal }\nTerminal.setRawMode(true);\n");
    assert_ne!(code, 0, "Terminal.setRawMode ran without unsafe");
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "refused for some other reason: {message}"
    );
}

// ── what unsafe never relaxes ───────────────────────────────────────────────

/// Lockdown is not lifted by `unsafe`.
#[test]
fn unsafe_does_not_lift_lockdown() {
    let (code, message) = run_locked(&format!(
        "use permissions {{ OS }}\nunsafe {{ OS.exec({}); }}\n",
        echo()
    ));
    assert_ne!(code, 0, "unsafe reached a capability under lockdown");
    assert!(
        message.contains("SZ6004") || message.contains("not available here"),
        "refused for some other reason: {message}"
    );
}

/// Argument validation still applies inside `unsafe`.
#[test]
fn unsafe_does_not_skip_argument_validation() {
    let (code, message) = run("use permissions { OS }\nunsafe { OS.exec(42); }\n");
    assert_ne!(code, 0, "a non-string command was accepted inside unsafe");
    assert!(
        message.contains("TypeError") || message.contains("must be a string"),
        "refused for some other reason: {message}"
    );
}

/// The protected-path guard still applies inside `unsafe`.
///
/// `sec_os_exec_system_path.sz` pins this at the corpus level; asserted here
/// too because it is one of the guarantees the contract promises never to
/// relax, and a reader of this file should be able to see that it holds.
#[test]
fn unsafe_does_not_open_a_protected_path() {
    let target = if cfg!(windows) {
        "C:\\\\Windows\\\\System32\\\\cmd.exe"
    } else {
        "/sbin/init"
    };
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\nunsafe {{ OS.exec(\"{target}\", []); }}\n"
    ));
    assert_ne!(code, 0, "unsafe opened a protected process path");
    assert!(
        !message.contains("SZ6003"),
        "refused for want of unsafe, which is not what this measures: {message}"
    );
}

/// Type safety is not suspended inside `unsafe`.
#[test]
fn unsafe_does_not_suspend_type_safety() {
    let (code, message) = run("unsafe { let n = 1; out n.noSuchMethod(); }\n");
    assert_ne!(code, 0, "an unknown method resolved inside unsafe");
    assert!(
        !message.contains("SZ6003"),
        "refused for want of unsafe: {message}"
    );
}

/// A limit that is not listed as waivable stays in force inside `unsafe`.
///
/// The generator ceiling is host-set and deliberately **not** in
/// `execution::Guarantee`. If `unsafe` ever became "turn everything off", this
/// is the test that notices.
#[test]
fn unsafe_does_not_relax_an_unlisted_limit() {
    let source = "fn* int endless() { let i = 0; while (true) { yield i; i = i + 1; } }\n\
                  unsafe { let all = endless(); out all.length(); }\n";
    let result = run_source_detailed(
        source.to_string(),
        "<unsafe-contract>",
        RunOpts {
            generator_yield_limit: Some(1_000),
            ..RunOpts::default()
        },
    );
    assert_ne!(
        result.exit_code, 0,
        "the generator ceiling was waived inside unsafe; it is not a listed \
         waivable guarantee"
    );
}

// ── 9, 10, 11: the context is restored, and nests ───────────────────────────

/// Leaving the block restores the ordinary context immediately.
#[test]
fn leaving_unsafe_restores_the_normal_context() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\nunsafe {{ OS.exec({0}); }}\nOS.exec({0});\n",
        echo()
    ));
    assert_ne!(code, 0, "the unsafe context outlived its block");
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "failed for some other reason: {message}"
    );
}

/// Nested blocks compose, and the inner one's exit does not clear the outer.
#[test]
fn nested_unsafe_blocks_compose() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         unsafe {{\n\
         \x20 unsafe {{ OS.exec({0}); }}\n\
         \x20 OS.exec({0});\n\
         }}\nout \"both ran\";\n",
        echo()
    ));
    assert_eq!(
        code, 0,
        "the inner block's exit cleared the outer context: {message}"
    );
}

/// A `throw` out of an `unsafe` block does not leave the context set.
#[test]
fn a_throw_out_of_unsafe_restores_the_context() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         try {{ unsafe {{ throw \"boom\"; }} }} catch (e) {{ }}\n\
         OS.exec({});\n",
        echo()
    ));
    assert_ne!(code, 0, "a throw left the evaluator in an unsafe context");
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "failed for some other reason: {message}"
    );
}

/// And neither does a `return`.
#[test]
fn a_return_out_of_unsafe_restores_the_context() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         fn void f() {{ unsafe {{ return; }} }}\n\
         f();\n\
         OS.exec({});\n",
        echo()
    ));
    assert_ne!(code, 0, "a return left the evaluator in an unsafe context");
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "failed for some other reason: {message}"
    );
}

/// The expression form behaves like the statement form.
#[test]
fn the_expression_form_restores_too() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         let r = unsafe {{ OS.exec({0}) }};\n\
         OS.exec({0});\n",
        echo()
    ));
    assert_ne!(code, 0, "the expression form leaked its context");
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "failed for some other reason: {message}"
    );
}

// ── 12: propagation, measured rather than invented ──────────────────────────

/// A function called from inside `unsafe` runs inside it. **DEC-M11-001.**
///
/// This is the measured behaviour, pinned so the decision is answered rather
/// than drifted into. `spec/security.md` used to say the call had to appear
/// *lexically* inside a block; it does not, and the spec now describes what the
/// runtime does and points at the open decision.
///
/// Deliberately not asserted as *desirable*: dynamic propagation is one of two
/// defensible models and Rust's is the other one. The test says what is, and the
/// message says where the choice lives.
#[test]
fn unsafe_propagates_dynamically_into_calls() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         fn void helper() {{ OS.exec({}); }}\n\
         unsafe {{ helper(); }}\n\
         out \"called\";\n",
        echo()
    ));
    assert_eq!(
        code, 0,
        "`unsafe` no longer propagates into a called function. That may be the \
         right answer — it is DEC-M11-001 — but it is a language change and this \
         test exists so it is decided rather than drifted into: {message}"
    );
}

/// And the propagation ends with the block, not with the function.
#[test]
fn a_function_called_outside_unsafe_is_still_refused() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         fn void helper() {{ OS.exec({}); }}\n\
         helper();\n",
        echo()
    ));
    assert_ne!(
        code, 0,
        "the same function ran outside any unsafe block, so the previous test \
         measures nothing"
    );
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "failed for some other reason: {message}"
    );
}

// ── 7, 8: the waivable guarantee ────────────────────────────────────────────

/// `OS.exec` inside `unsafe` may exceed the runtime's normal output ceiling.
///
/// The ceiling is `execution::Guarantee::ProcessOutputCeiling`, 64 MiB, and
/// `unsafe` is defined to waive it. The child here emits 80 MiB, and the whole
/// of it arrives.
///
/// Slow and deliberate: the property is about bytes actually crossing a pipe,
/// and a smaller number would not distinguish a waived ceiling from a ceiling
/// that is simply higher than the test.
#[test]
fn os_exec_inside_unsafe_may_exceed_the_output_ceiling() {
    let payload = std::env::temp_dir().join(format!("serez-waiver-{}.bin", std::process::id()));
    let size = 80 * 1024 * 1024;
    std::fs::write(&payload, vec![b'z'; size]).expect("payload");

    let path = payload.display().to_string().replace('\\', "\\\\");
    let argv = if cfg!(windows) {
        format!("\"cmd\", [\"/c\", \"type\", \"{path}\"]")
    } else {
        format!("\"sh\", [\"-c\", \"cat \\\"$0\\\"\", \"{path}\"]")
    };

    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         unsafe {{ let r = OS.exec({argv}); out r.stdout.length(); }}\n"
    ));
    let _ = std::fs::remove_file(&payload);

    assert_eq!(
        code, 0,
        "an 80 MiB child was refused inside unsafe; the output ceiling is \
         defined as waivable there: {message}"
    );
}

/// And the ceiling itself is real: the same guarantee, unwaived, refuses.
///
/// `OS.exec` requires `unsafe`, so there is no safe route to a child process to
/// test through — the unwaived path is exercised where it lives, in
/// `evaluator::child_output_tests`, at the boundary (exactly 64 MiB captured,
/// 64 MiB + 1 refused). This asserts the other half: that the waiver is a
/// property of the *context* and not simply an absent limit.
#[test]
fn the_waiver_is_a_property_of_the_context() {
    use serez_code::execution::{ExecutionContext, Guarantee};

    let safe = ExecutionContext::new();
    assert!(
        !safe.waives(Guarantee::ProcessOutputCeiling),
        "ordinary code waives the output ceiling"
    );

    let mut unsafe_ctx = ExecutionContext::new();
    let _ = unsafe_ctx.enter_unsafe();
    assert!(
        unsafe_ctx.waives(Guarantee::ProcessOutputCeiling),
        "unsafe does not waive the guarantee it is defined to waive"
    );

    // Every guarantee is waived by unsafe and none outside it. Walking the list
    // rather than naming one means a variant added later cannot default to
    // waived in ordinary code without failing here.
    for guarantee in Guarantee::ALL {
        assert!(!safe.waives(*guarantee), "'{}' leaked", guarantee.name());
        assert!(
            unsafe_ctx.waives(*guarantee),
            "'{}' is listed and not waived",
            guarantee.name()
        );
    }
}
