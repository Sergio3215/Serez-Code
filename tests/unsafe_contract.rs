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

// ── DEC-M11-001: `unsafe` is lexical ────────────────────────────────────────
//
// `unsafe` authority does not cross a function-call boundary. A block relaxes
// guarantees for what is written inside it and for nothing else; a function
// called from within one starts under the runtime's ordinary guarantees.
//
// The point is local auditability: whether a function relaxes a guarantee is
// visible in its own body and does not depend on its callers.
//
// Every test below distinguishes the two models. Under the previous dynamic
// implementation each "callee is refused" test passed a program that *ran*, so
// none of them can pass under both.

/// **The control for DEC-M11-001.** A caller's block does not reach into a callee.
///
/// Measured under the previous dynamic implementation, this exact program
/// printed `callee ran unsafe` and exited 0.
#[test]
fn a_callers_block_does_not_authorise_a_callee() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         fn void helper() {{ OS.exec({}); }}\n\
         unsafe {{ helper(); }}\n\
         out \"callee ran unsafe\";\n",
        echo()
    ));
    assert_ne!(
        code, 0,
        "the caller's unsafe block authorised the callee's body — that is the \
         dynamic model, and DEC-M11-001 is lexical"
    );
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "refused, but not by the unsafe gate: {message}"
    );
}

/// A callee that declares its own block runs, called from inside one.
///
/// The positive half: without it, an implementation that refused every call from
/// inside `unsafe` would satisfy the test above.
#[test]
fn a_callee_that_declares_its_own_block_runs() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         fn void helper() {{ unsafe {{ OS.exec({}); }} }}\n\
         unsafe {{ helper(); }}\n\
         out \"ok\";\n",
        echo()
    ));
    assert_eq!(
        code, 0,
        "a callee's own block did not authorise its body: {message}"
    );
}

/// And it runs when called from ordinary code too — the caller is irrelevant.
#[test]
fn a_callee_with_its_own_block_runs_from_a_safe_caller() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         fn void helper() {{ unsafe {{ OS.exec({}); }} }}\n\
         helper();\n\
         out \"ok\";\n",
        echo()
    ));
    assert_eq!(
        code, 0,
        "a self-declared block depended on the caller: {message}"
    );
}

/// A function with no block of its own is refused however it is reached.
#[test]
fn a_function_without_a_block_is_refused_from_anywhere() {
    for caller in ["helper();", "unsafe { helper(); }"] {
        let (code, message) = run(&format!(
            "use permissions {{ OS }}\n\
             fn void helper() {{ OS.exec({}); }}\n\
             {caller}\n",
            echo()
        ));
        assert_ne!(
            code, 0,
            "`{caller}` authorised a body with no block of its own"
        );
        assert!(
            message.contains("SZ6003") || message.contains("requires an `unsafe"),
            "refused for some other reason from `{caller}`: {message}"
        );
    }
}

/// The boundary is the call, not the block: nesting inside one body is covered.
///
/// An `if`, a loop and a nested block are all still inside the same frame's
/// block. Confusing "a deeper block" with "a deeper frame" is the mistake this
/// pins against.
#[test]
fn nesting_inside_one_body_stays_inside_the_block() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         unsafe {{\n\
         \x20 if (true) {{\n\
         \x20   let i = 0;\n\
         \x20   while (i < 1) {{ OS.exec({}); i = i + 1; }}\n\
         \x20 }}\n\
         }}\n\
         out \"ok\";\n",
        echo()
    ));
    assert_eq!(
        code, 0,
        "a block nested inside the same body was treated as a call boundary: {message}"
    );
}

/// A method called from a caller's block is refused. Same contract.
#[test]
fn a_method_does_not_inherit_its_callers_block() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         public class C {{\n\
         \x20 public C() {{ this.x = 1; }}\n\
         \x20 public void go() {{ OS.exec({}); }}\n\
         }}\n\
         let c = new C();\n\
         unsafe {{ c.go(); }}\n\
         out \"method ran unsafe\";\n",
        echo()
    ));
    assert_ne!(code, 0, "a method inherited its caller's block");
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "refused for some other reason: {message}"
    );
}

/// A method with its own block runs.
#[test]
fn a_method_with_its_own_block_runs() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         public class C {{\n\
         \x20 public C() {{ this.x = 1; }}\n\
         \x20 public void go() {{ unsafe {{ OS.exec({}); }} }}\n\
         }}\n\
         let c = new C();\n\
         c.go();\n\
         out \"ok\";\n",
        echo()
    ));
    assert_eq!(
        code, 0,
        "a method's own block did not authorise its body: {message}"
    );
}

/// A constructor called from a caller's block is refused.
///
/// The constructor was not a call frame at all — it pushed a scope and nothing
/// else — so `unsafe { new C() }` reached straight into its body. Making it a
/// frame is what closes this, and it also closed a native stack overflow: see
/// `a_recursive_constructor_is_refused_rather_than_crashing`.
#[test]
fn a_constructor_does_not_inherit_its_callers_block() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         public class C {{ public C() {{ OS.exec({}); }} }}\n\
         unsafe {{ let c = new C(); }}\n\
         out \"ctor ran unsafe\";\n",
        echo()
    ));
    assert_ne!(code, 0, "a constructor inherited its caller's block");
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "refused for some other reason: {message}"
    );
}

/// A constructor with its own block runs.
#[test]
fn a_constructor_with_its_own_block_runs() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         public class C {{ public C() {{ unsafe {{ OS.exec({}); }} this.x = 1; }} }}\n\
         let c = new C();\n\
         out \"ok\";\n",
        echo()
    ));
    assert_eq!(
        code, 0,
        "a constructor's own block did not authorise its body: {message}"
    );
}

/// A lambda called from a caller's block is refused.
#[test]
fn a_lambda_does_not_inherit_its_callers_block() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         let f = () => {{ OS.exec({}); }};\n\
         unsafe {{ f(); }}\n\
         out \"lambda ran unsafe\";\n",
        echo()
    ));
    assert_ne!(code, 0, "a lambda inherited its caller's block");
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "refused for some other reason: {message}"
    );
}

/// Nor does a callback, reached through another function.
///
/// Two frames between the block and the body, which is the case a
/// one-level-deep implementation would still get wrong.
#[test]
fn a_callback_does_not_inherit_the_block_two_frames_up() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         fn void apply(any g) {{ g(); }}\n\
         let f = () => {{ OS.exec({}); }};\n\
         unsafe {{ apply(f); }}\n\
         out \"callback ran unsafe\";\n",
        echo()
    ));
    assert_ne!(code, 0, "a callback inherited a block two frames up");
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "refused for some other reason: {message}"
    );
}

/// Returning from a call leaves the caller inside its own block.
///
/// The other half of the boundary: entering a callee must not *clear* the
/// caller's block either. The call happens first, then a gated operation in the
/// same block.
#[test]
fn a_caller_keeps_its_block_across_a_call() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         fn int plain() {{ return 1; }}\n\
         unsafe {{\n\
         \x20 let n = plain();\n\
         \x20 OS.exec({});\n\
         }}\n\
         out \"ok\";\n",
        echo()
    ));
    assert_eq!(
        code, 0,
        "the caller lost its own block after calling a function: {message}"
    );
}

/// A callee that itself opens a block leaves the caller's intact on return.
#[test]
fn a_callees_own_block_does_not_disturb_the_caller() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         fn void inner() {{ unsafe {{ OS.exec({0}); }} }}\n\
         unsafe {{\n\
         \x20 inner();\n\
         \x20 OS.exec({0});\n\
         }}\n\
         out \"ok\";\n",
        echo()
    ));
    assert_eq!(
        code, 0,
        "the callee's block clobbered the caller's on return: {message}"
    );
}

/// A callee that fails does not leave the caller's context disturbed.
#[test]
fn a_failing_callee_does_not_disturb_the_caller() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         fn void boom() {{ throw \"boom\"; }}\n\
         unsafe {{\n\
         \x20 try {{ boom(); }} catch (e) {{ }}\n\
         \x20 OS.exec({});\n\
         }}\n\
         out \"ok\";\n",
        echo()
    ));
    assert_eq!(
        code, 0,
        "a throw inside a called function disturbed the caller's block: {message}"
    );
}

/// And a callee that throws out of *its own* block leaks nothing to the caller.
#[test]
fn a_throw_out_of_a_callees_block_leaks_nothing() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         fn void boom() {{ unsafe {{ throw \"boom\"; }} }}\n\
         try {{ boom(); }} catch (e) {{ }}\n\
         OS.exec({});\n",
        echo()
    ));
    assert_ne!(
        code, 0,
        "a callee's block survived a throw and authorised the caller"
    );
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "failed for some other reason: {message}"
    );
}

/// A `return` out of a callee's block leaks nothing either.
#[test]
fn a_return_out_of_a_callees_block_leaks_nothing() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         fn void early() {{ unsafe {{ return; }} }}\n\
         early();\n\
         OS.exec({});\n",
        echo()
    ));
    assert_ne!(code, 0, "a callee's block survived an early return");
    assert!(
        message.contains("SZ6003") || message.contains("requires an `unsafe"),
        "failed for some other reason: {message}"
    );
}

/// `break` out of a loop inside a block leaves the block behaving normally.
#[test]
fn a_break_inside_a_block_does_not_disturb_it() {
    let (code, message) = run(&format!(
        "use permissions {{ OS }}\n\
         unsafe {{\n\
         \x20 let i = 0;\n\
         \x20 while (i < 10) {{ if (i == 0) {{ break; }} i = i + 1; }}\n\
         \x20 OS.exec({});\n\
         }}\n\
         out \"ok\";\n",
        echo()
    ));
    assert_eq!(code, 0, "a break disturbed the enclosing block: {message}");
}

/// Making the constructor a call frame closed a native stack overflow.
///
/// Measured before: `public class R { public R() { let x = new R(); } }` killed
/// the process — `thread '<unknown>' has overflowed its stack`, exit 127, no
/// diagnostic and nothing catchable — while the identical recursion through a
/// function was refused at depth 512 with `SZ6002`. The constructor was not a
/// call frame, which is the same reason it inherited its caller's block.
///
/// Driven through the binary rather than in-process: 512 nested `eval_new_class`
/// frames need more Rust stack than a test-harness thread has, so an in-process
/// run overflows the *harness* before the interpreter's own guard can refuse —
/// which is a fact about the harness, not about the fix.
#[test]
fn a_recursive_constructor_is_refused_rather_than_crashing() {
    let dir = std::env::temp_dir().join(format!("serez-ctor-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let program = dir.join("rec.sz");
    std::fs::write(
        &program,
        "public class R { public R() { let x = new R(); } }
let r = new R();
",
    )
    .expect("program");

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_sz"))
        .arg(&program)
        .output()
        .expect("run sz");
    let message = String::from_utf8_lossy(&out.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        !out.status.success(),
        "unbounded constructor recursion completed"
    );
    assert!(
        message.contains("SZ6002") || message.contains("maximum call depth"),
        "the process did not refuse the recursion the way a function's is          refused; it exited {:?} saying {message}",
        out.status.code()
    );
}

/// And an ordinary depth of constructor nesting still works.
///
/// The control on the one above: a limit that refused every constructor would
/// satisfy it.
#[test]
fn ordinary_constructor_nesting_is_unaffected() {
    let (code, message) = run("public class Leaf { public Leaf() { this.v = 1; } }\n\
         public class Mid { public Mid() { this.leaf = new Leaf(); } }\n\
         public class Top { public Top() { this.mid = new Mid(); } }\n\
         let t = new Top();\n\
         out t.mid.leaf.v;\n");
    assert_eq!(
        code, 0,
        "ordinary constructor nesting was refused: {message}"
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

    // The frame numbers stand for "the frame that opened the block" and "one
    // call deeper". Any two distinct values would do; what is being asserted is
    // same-frame versus deeper.
    const HERE: usize = 2;
    const DEEPER: usize = 3;

    let safe = ExecutionContext::new();
    assert!(
        !safe.waives(Guarantee::ProcessOutputCeiling, HERE),
        "ordinary code waives the output ceiling"
    );

    let mut opened = ExecutionContext::new();
    let _ = opened.enter_unsafe(HERE);
    assert!(
        opened.waives(Guarantee::ProcessOutputCeiling, HERE),
        "unsafe does not waive the guarantee it is defined to waive"
    );

    // DEC-M11-001: the waiver does not reach a called frame.
    assert!(
        !opened.waives(Guarantee::ProcessOutputCeiling, DEEPER),
        "the waiver crossed a call boundary"
    );

    // Every guarantee is waived in the block's own frame, in no frame outside a
    // block, and in no deeper frame. Walking the list rather than naming one
    // means a variant added later cannot default to waived without failing here.
    for guarantee in Guarantee::ALL {
        assert!(
            !safe.waives(*guarantee, HERE),
            "'{}' leaked into ordinary code",
            guarantee.name()
        );
        assert!(
            opened.waives(*guarantee, HERE),
            "'{}' is listed and was not waived in its own frame",
            guarantee.name()
        );
        assert!(
            !opened.waives(*guarantee, DEEPER),
            "'{}' crossed a call boundary",
            guarantee.name()
        );
    }
}
