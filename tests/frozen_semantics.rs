//! Behaviours that are observable, undecided, and easy to change by accident.
//!
//! # Why this file exists
//!
//! M7's charter is that *important observable behaviour should exist because it
//! was decided, not because the implementation happens to do it.* Deciding is not
//! this roadmap's to do — each behaviour below is registered in
//! `docs/maturity/ROADMAP_STATE.md` §7A with evidence and a recommendation, and is
//! waiting on an answer.
//!
//! What *is* this roadmap's to do is make sure none of them moves in the
//! meantime. Each one is currently unpinned in exactly the way that matters: the
//! conformance suite asserts what a program **prints**, and every case here is
//! either silent or produces a value no passing test looks at. A refactor could
//! flip any of them and all 499 conformance tests would still pass.
//!
//! So this is not a claim that the behaviour is right. **Several are recorded in
//! `spec/` as hazards and inconsistencies, in the spec's own words.** It is a
//! claim that it is *current*, so that when a decision lands the diff shows what
//! changed and a reviewer sees it.
//!
//! `tests/runtime_outcome.rs` already does this for one rule —
//! `a_declared_type_matches_exactly_and_never_a_subclass`, pinned "so it cannot
//! move in either direction by accident". This applies that idea to the rest of
//! the undecided set.
//!
//! # How these assert
//!
//! Through the language's own `assert`, not through captured output: `out` writes
//! to stdout with `println!` and there is no capture hook, so an integration test
//! cannot read it without spawning a process. A failed `assert` raises, the
//! program stops being `ProgramOutcome::Value`, and the test fails — which makes
//! the fixture itself the specification of what is pinned, readable by anyone who
//! knows Serez rather than only by someone who knows the evaluator's internals.
//!
//! # When one of these fails
//!
//! Do not update it to match. Find the decision it belongs to, check the decision
//! has actually been taken, and say so in the commit. One of these going green a
//! different way is either a decision being taken by accident — the thing this
//! file exists to prevent — or a regression.

use serez_code::evaluator::{Evaluator, ProgramOutcome};
use serez_code::lexer::Lexer;
use serez_code::parser::Parser;

/// Run a Serez program; panic with detail unless it completes.
///
/// `label` names the pinned behaviour, so a failure says which contract moved
/// rather than only that an assertion failed.
fn pins(label: &str, source: &str) {
    let mut parser = Parser::new(Lexer::new(source.to_string()));
    parser.set_source(source.lines().map(str::to_string).collect());
    let program = parser.parse_program();
    assert!(
        !parser.has_errors(),
        "{label}: fixture must parse cleanly, got {:?}",
        parser.take_errors()
    );

    let mut evaluator = Evaluator::new();
    evaluator.set_source(source.lines().map(str::to_string).collect());
    match evaluator.eval_program_outcome(&program) {
        ProgramOutcome::Value(_) => {}
        other => panic!(
            "{label}: this behaviour is pinned as current and it moved.\n\
             The program did not complete: {other:?}\n\n\
             Do not edit this test to match. Find the decision it belongs to in \
             ROADMAP_STATE.md §7A, confirm the decision was actually taken, and say \
             so in the commit."
        ),
    }
}

/// DEC-M7-003 — a `match` with no matching arm yields `null`, silently.
///
/// `spec/control-flow.md` states this and calls it "a hazard, not a design
/// statement", noting the `null` "is indistinguishable from an arm that
/// legitimately returned null". 50 of the corpus's 107 `match` expressions have
/// no catch-all, so it is reachable from a lot of code that passes today.
#[test]
fn a_match_with_no_matching_arm_yields_null_and_says_nothing() {
    pins(
        "DEC-M7-003 match falls through to null",
        "let r = match 99 { 1 => \"one\", 2 => \"two\" };\n\
         assert(r == null, \"no arm matched, so the match is null\");\n",
    );
}

/// DEC-M7-005 — a pattern that cannot be evaluated becomes "did not match".
///
/// `evaluator/expr.rs` discards any error raised while evaluating a literal
/// pattern and reports a non-match, so a misspelled name in a pattern falls
/// silently to the next arm. **Undocumented in `spec/`**, which is the worst of
/// the four states a behaviour can be in. See §5.24.
#[test]
fn a_pattern_that_fails_to_evaluate_falls_through_silently() {
    pins(
        "DEC-M7-005 broken pattern is a non-match",
        "enum Direction { North, South }\n\
         let d = Direction.North;\n\
         let r = match d { Nonexistent.Nope => \"typo\", _ => \"fell through\" };\n\
         assert(r == \"fell through\", \"a pattern that cannot evaluate is a non-match\");\n",
    );
}

/// DEC-M7-002 — privacy is keyed to the receiver's runtime class.
///
/// A subclass method reaches an inherited private member; the same access from
/// outside is refused. `spec/classes.md` records it under caveats "rather than
/// silently describing it as stronger than the implementation".
#[test]
fn a_subclass_reaches_an_inherited_private_method() {
    pins(
        "DEC-M7-002 private is reachable within the hierarchy",
        "public class Base {\n\
         \x20   public Base() { this.secret = 42; }\n\
         \x20   private int hidden() { return this.secret; }\n\
         }\n\
         public class Derived : Base {\n\
         \x20   public Derived() { super(); }\n\
         \x20   public int reach() { return this.hidden(); }\n\
         }\n\
         assert(new Derived().reach() == 42, \"a subclass reaches an inherited private\");\n",
    );
}

/// DEC-M7-004 — `==` compares containers by identity, and assignment copies.
///
/// So no two array values are ever equal, **including an array and its own
/// copy**. `spec/values.md`: "a documented inconsistency, not a design statement".
#[test]
fn two_structurally_identical_arrays_are_not_equal() {
    pins(
        "DEC-M7-004 containers compare by identity",
        "let a = [1, 2];\n\
         let b = a;\n\
         assert(([1, 2] == [1, 2]) == false, \"two identical literals are not equal\");\n\
         assert((a == b) == false, \"an array is not equal to its own copy\");\n",
    );
}

/// DEC-M7-004, the other half — scalars *do* compare across numeric types.
///
/// Pinned beside the container case because the pair is the whole inconsistency:
/// `1 == 1.0` is true, while `1 is decimal` is false and passing `1` to a
/// `decimal` parameter is a `TypeError`. Three different answers to one question,
/// and a decision on any of them should be taken knowing the other two.
#[test]
fn an_int_equals_a_decimal_although_it_neither_is_one_nor_may_be_passed_as_one() {
    pins(
        "DEC-M7-004 / DEC-M5-002 the three answers",
        "assert((1 == 1.0) == true,        \"equality mixes numeric types\");\n\
         assert((1 is decimal) == false,   \"`is` does not\");\n\
         assert((\"1\" == 1) == false,       \"and there is no cross-type coercion\");\n\
         assert((null == false) == false,  \"null equals only null\");\n",
    );
}

/// DEC-M7-001 — `remove` on an empty array returns null.
///
/// Every other out-of-range index raises `IndexOutOfBounds`. `spec/arrays.md`
/// carries it under "Known inconsistency".
#[test]
fn remove_on_an_empty_array_returns_null_where_every_other_index_raises() {
    pins(
        "DEC-M7-001 remove on empty is null",
        "let a = [];\n\
         assert(a.remove(0) == null, \"remove on an empty array is null, not an error\");\n",
    );
}

/// **Not** a pending decision: handles are safe, and nothing says so.
///
/// A freed handle's id is never reissued, and using one after `free` is refused
/// and catchable. That is a real safety property, it is deliberate — `handles.rs`
/// states the never-reissued rule — and **no document in `spec/` mentions it**,
/// because there is no `spec/memory.md`.
///
/// Pinned here rather than left to M8 because an unspecified guarantee is exactly
/// the kind that gets optimised away by someone who does not know it is a
/// guarantee. M8 owns writing it down; this owns it not changing first.
#[test]
fn a_freed_handle_is_never_reissued_and_cannot_be_used_again() {
    pins(
        "memory handles: no reissue, no use-after-free",
        "unsafe {\n\
         \x20   let a = Memory.alloc(8);\n\
         \x20   Memory.free(a);\n\
         \x20   let b = Memory.alloc(8);\n\
         \x20   assert((a == b) == false, \"a freed handle id is never reissued\");\n\
         \x20   let dead = false;\n\
         \x20   try { Memory.write(a, 0, \"byte\", 65); } catch (e) { dead = true; }\n\
         \x20   assert(dead, \"use-after-free is refused, and catchable\");\n\
         \x20   let live = true;\n\
         \x20   try { Memory.write(b, 0, \"byte\", 65); } catch (e) { live = false; }\n\
         \x20   assert(live, \"and a live handle still accepts the identical call\");\n\
         }\n",
    );
}
