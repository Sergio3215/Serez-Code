//! When a name starts existing, and what the phase says about using it earlier.
//!
//! # The disagreement this settles
//!
//! Serez hoists nothing: a binding exists from the moment its statement runs.
//! Measured against the release binary, every top-level form fails the same way
//! when used before its declaration:
//!
//! ```text
//! out x; let x = 1;                       ❌ SZ4001 Variable not found: x
//! out f(); fn int f() { return 1; }       ❌ SZ4001 Variable not found: f
//! let c = new C(); public class C {…}     ❌ SZ4001 Unknown class 'C'
//! out E.A; enum E { A, B }                ❌ SZ4001 Variable not found: E
//! out a; let [a, b] = [1, 2];             ❌ SZ4001 Variable not found: a
//! out n; native fn int n();               ❌ SZ4001 Variable not found: n
//! ```
//!
//! The semantic phase said nothing about any of them, because `seed_globals`
//! binds every top-level declaration into frame 0 before the walk starts. Inside
//! a block it reported the same thing correctly — `{ out z; let z = 1; }` is
//! `SZ8000` — so the phase's answer depended on how deeply the code was nested
//! rather than on the language.
//!
//! # No new hoisting rules
//!
//! Nothing about what Serez hoists changes here. Frame 0 still holds every
//! top-level declaration from the start, because a use that runs *later*
//! legitimately sees a declaration that comes later in the file. All of these
//! work at run time and none is reported:
//!
//! ```text
//! class Child : Parent {…}  class Parent {…}     parents resolve at instantiation
//! fn a() { return b(); }    fn b() {…}  a();     mutual recursion
//! let f = () => later;      let later = 1; f();  the lambda runs afterwards
//! fn g() { return later; }  let later = 7; g();  same
//! fn h(Later p) {}          class Later {…}      an annotation evaluates nothing
//! ```
//!
//! So the rule is not "declare before use". It is: a use that happens **when its
//! own statement runs** needs the declaration to have run.
//!
//! # The two halves of this file
//!
//! Every "must be reported" test below has a "must not be reported" twin, and
//! the second half is the one that matters: a rule that reported *every* forward
//! reference would pass the first half completely and reject working programs,
//! which is the one failure mode a fatal phase must not have.

use serez_code::run::{RunOpts, run_source_detailed};

/// Run a snippet the way `sz file.sz` does, and return (exit code, message).
fn run(source: &str) -> (i32, String) {
    let outcome = run_source_detailed(
        source.to_string(),
        "<declaration-order>",
        RunOpts::default(),
    );
    let message = match &outcome.failure {
        Some(failure) => format!("{:?}", failure),
        None => String::new(),
    };
    (outcome.exit_code, message)
}

/// The phase rejected it, and said why.
fn assert_reported(source: &str, name: &str) {
    let (code, message) = run(source);
    assert_ne!(
        code, 0,
        "accepted a use before its declaration:\n{}",
        source
    );
    assert!(
        message.contains("used before its declaration has run")
            || message.contains("cannot assign to"),
        "rejected, but not by the declaration-order rule:\n{}\n{}",
        source,
        message
    );
    assert!(
        message.contains(name),
        "the diagnostic does not name '{}':\n{}",
        name,
        message
    );
}

/// It ran, and produced what it should have.
fn assert_runs(source: &str, expected_exit: i32) {
    let (code, message) = run(source);
    assert_eq!(
        code, expected_exit,
        "a legitimate forward reference was rejected:\n{}\n{}",
        source, message
    );
}

// ── must be reported ────────────────────────────────────────────────────────

#[test]
fn a_let_used_before_it_runs() {
    assert_reported("out x;\nlet x = 1;\n", "x");
}

#[test]
fn a_const_used_before_it_runs() {
    assert_reported("out x;\nconst x = 1;\n", "x");
}

#[test]
fn a_function_called_before_it_runs() {
    assert_reported("out f();\nfn int f() { return 1; }\n", "f");
}

/// `new C()` is evaluated where it stands, whatever the use kind is called.
///
/// This one was missed at first: `UseKind::Type` sounds like an annotation and
/// was excluded on that reading. It means a **construction site**, and the
/// runtime rejects `let c = new C();` before `class C` with
/// `Unknown class or interface 'C'`. Type annotations are not walked at all,
/// which is why they need no exception — see `an_annotation_may_name_a_later_class`.
#[test]
fn a_class_constructed_before_it_runs() {
    assert_reported(
        "let c = new C();\npublic class C { public C() { this.v = 1; } }\n",
        "C",
    );
}

#[test]
fn an_enum_read_before_it_runs() {
    assert_reported("out E.A;\nenum E { A, B }\n", "E");
}

#[test]
fn a_destructured_name_used_before_it_runs() {
    assert_reported("out a;\nlet [a, b] = [1, 2];\n", "a");
}

#[test]
fn a_native_declaration_used_before_it_runs() {
    assert_reported("out n;\nnative fn int n();\n", "n");
}

/// A `let` whose own initialiser reads itself: the read happens first.
#[test]
fn a_let_that_reads_itself() {
    assert_reported("let x = x + 1;\nout x;\n", "x");
}

/// Assigning to a name whose declaration has not run reads differently and says so.
#[test]
fn an_assignment_before_the_declaration_has_its_own_wording() {
    let (code, message) = run("x = 5;\nlet x = 1;\n");
    assert_ne!(code, 0);
    assert!(
        message.contains("cannot assign to 'x' here"),
        "an out-of-order write did not get the write wording: {}",
        message
    );
}

// ── must not be reported ────────────────────────────────────────────────────

/// The control that carries the most weight: ordinary code, in order.
///
/// Without it, a rule that reported every use at all would pass every test
/// above.
#[test]
fn ordinary_code_in_order_is_untouched() {
    assert_runs(
        "let x = 1;\nfn int f() { return x; }\npublic class C { public C() { this.v = 1; } }\n\
         let c = new C();\nout f();\nout c.v;\n",
        0,
    );
}

/// A builtin is not a declaration waiting to happen.
///
/// `out abs(5);` on line 1 read as a use before a declaration when
/// `declared_so_far` started empty — 52 corpus files failed on it, every one a
/// builtin. They exist before the first statement runs.
#[test]
fn a_builtin_is_available_from_the_first_line() {
    assert_runs("out abs(5);\nout parseInt(\"42\");\n", 0);
}

/// Mutual recursion between top-level functions.
#[test]
fn mutually_recursive_functions_are_fine() {
    assert_runs(
        "fn int a() { return b(); }\nfn int b() { return 1; }\nout a();\n",
        0,
    );
}

/// A lambda body runs when the lambda is called, not where it is written.
#[test]
fn a_lambda_may_capture_a_name_declared_later() {
    assert_runs(
        "let f = () => { return later; };\nlet later = 1;\nout f();\n",
        0,
    );
}

/// And so does a function body.
#[test]
fn a_function_body_may_read_a_name_declared_later() {
    assert_runs(
        "fn int g() { return later; }\nlet later = 7;\nout g();\n",
        0,
    );
}

/// A declared parent is resolved at instantiation, so it may come later.
#[test]
fn a_parent_may_be_declared_later() {
    assert_runs(
        "public class Child : Parent { public Child() { this.a = 1; } }\n\
         public class Parent { public Parent() { this.b = 2; } }\n\
         let c = new Child();\nout \"ok\";\n",
        0,
    );
}

/// An annotation evaluates nothing, so it may name a class declared later.
#[test]
fn an_annotation_may_name_a_later_class() {
    assert_runs(
        "fn void h(Later p) { }\npublic class Later { public Later() { this.z = 1; } }\n\
         out \"ok\";\n",
        0,
    );
}

/// A name declared nowhere is still the *other* rule's finding, worded its way.
///
/// The two must not be confused: "declared nowhere" and "not declared yet" are
/// different facts and different advice.
#[test]
fn a_name_declared_nowhere_keeps_its_own_message() {
    let (code, message) = run("out totallyUndefined;\n");
    assert_ne!(code, 0);
    assert!(
        message.contains("is not declared in this scope or any enclosing one"),
        "an undeclared name got the out-of-order wording: {}",
        message
    );
    assert!(
        !message.contains("used before its declaration has run"),
        "an undeclared name got both messages: {}",
        message
    );
}

/// A block already reported this correctly, and still does.
///
/// Nested scopes bind names as the walk reaches them, so they never needed the
/// new rule. This pins that the fix did not double-report them.
#[test]
fn a_block_reports_it_once() {
    let (code, message) = run("{ out z; let z = 1; }\n");
    assert_ne!(code, 0);
    let mentions = message.matches("'z'").count();
    assert_eq!(
        mentions, 1,
        "a nested out-of-order use was reported twice: {}",
        message
    );
}
