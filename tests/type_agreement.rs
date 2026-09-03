//! Does the static checker agree with the runtime about the same program?
//!
//! # The contract M5 exists to establish
//!
//! M5's charter is *"type rules that are coherent, normative and consistent
//! between checker, runtime and tooling"*. `spec/types.md` is the normative
//! half: it states, rule by rule, what a declared type accepts. The runtime
//! implements that table in `evaluator::type_matches`. The checker implements a
//! different one, in `type_checker::types_compatible`, eight lines long.
//!
//! Where the two disagree, `spec/types.md` decides which is wrong — and in every
//! disagreement found so far the spec sides with the runtime, so the checker is
//! the one at fault. That is what makes those fixes ordinary work rather than
//! product decisions.
//!
//! # The property, and why it is asymmetric
//!
//! **A false positive is the serious direction.** The checker is advisory: its
//! findings print to stderr and change neither the exit code nor whether the
//! program runs (`spec/types.md`, "The static checker"). A finding on a program
//! that runs correctly is therefore pure noise, and noise on correct code is how
//! a linter teaches people to ignore it. So:
//!
//!   * **Asserted:** the checker reports nothing about a program the runtime
//!     accepts, unless that case is in [`KNOWN_DIVERGENCES`] with a reason.
//!   * **Asserted where the gap is a defect:** a case marked `checker_must_catch`
//!     is one the checker demonstrably handles in another spelling, so a miss is
//!     an oversight rather than a boundary.
//!   * **Reported, not asserted:** every other miss. `spec/types.md` already says
//!     the checker is deliberately partial, so reaching further is an improvement
//!     rather than a contract.
//!
//! # `KNOWN_DIVERGENCES` is the point of this file
//!
//! A divergence in that list is *documented*; one that is not fails the test. So
//! the list can only shrink by fixing something or grow by deciding something,
//! and either way it happens in a diff someone reads. It is also checked for
//! staleness: an entry that no longer diverges fails, so a fixed defect cannot
//! stay recorded as intended. An empty list would be the end state; today it
//! holds one entry, and that entry is an open decision rather than a defect.

use serez_code::ast::Program;
use serez_code::evaluator::Evaluator;
use serez_code::lexer::Lexer;
use serez_code::parser::Parser;
use serez_code::type_checker::TypeChecker;

/// A program, and what the two halves of the language say about it.
struct Case {
    /// Stable identifier, used by [`KNOWN_DIVERGENCES`].
    name: &'static str,
    /// What rule of `spec/types.md` this exercises.
    rule: &'static str,
    source: &'static str,
    /// For a program the **runtime rejects**: must the checker catch it too?
    ///
    /// `false` for most cases, because the checker is deliberately partial.
    /// `true` where a miss is a defect rather than a limit — where the checker
    /// demonstrably handles the same shape written differently, so the gap is an
    /// oversight and not a boundary.
    checker_must_catch: bool,
}

/// Cases where the checker reports and the runtime does not, that are recorded
/// rather than fixed. Each entry must name why.
///
/// Anything here is a claim that the divergence is *intended*, so an entry is
/// only legitimate when it points at an open decision or a spec sentence.
const KNOWN_DIVERGENCES: &[(&str, &str)] = &[(
    "nullable_value_into_non_nullable_parameter",
    "DEC-M5-001 — a value the checker types `int?` may hold null, so reporting it \
     at an `int` parameter is defensible strict-nullability behaviour rather than \
     a mistake. The checker has no flow analysis, so it fires on every such call, \
     including where the value is provably an int. spec/types.md takes no \
     position. Open.",
)];

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "void_return_accepts_null",
            rule: "spec/types.md — `void` accepts `null`",
            source: "fn void nothing() { return null; }\nnothing();\n",
            checker_must_catch: false,
        },
        Case {
            name: "typed_array_parameter_accepts_any_array",
            rule: "spec/types.md — `[T]` accepts any array, whatever its elements",
            source: concat!(
                "fn any takes([int] xs) { return 1; }\n",
                "let strs [string] = [\"a\"];\n",
                "out takes(strs);\n"
            ),
            checker_must_catch: false,
        },
        Case {
            name: "array_parameter_accepts_a_typed_array",
            rule: "spec/types.md — `array` is recognised as a type name by the runtime matcher",
            source: concat!(
                "fn any takes(array xs) { return 1; }\n",
                "let nums [int] = [1, 2];\n",
                "out takes(nums);\n"
            ),
            checker_must_catch: false,
        },
        Case {
            name: "nullable_value_into_non_nullable_parameter",
            rule: "spec/types.md — `T?` accepts a `T` or `null`",
            source: concat!(
                "fn int? maybe() { return 5; }\n",
                "fn int wants(int n) { return n; }\n",
                "let m = maybe();\n",
                "out wants(m);\n"
            ),
            checker_must_catch: false,
        },
        // §5.29. The same program without `export` is caught statically; with it,
        // passes 1 and 2 of `TypeChecker::check` never see the declaration, so it
        // is caught only at run time. `spec/types.md` never mentions `export`, so
        // nothing documents a difference — an oversight inside one function, and
        // therefore an assertion rather than a report.
        Case {
            name: "an_exported_function_is_checked_like_any_other",
            rule: "spec/types.md — nothing makes `export` change what is checked",
            source: concat!(
                "export fn int f(int a) { return a; }\n",
                "out f(\"hello\");\n"
            ),
            checker_must_catch: true,
        },
        // The control for the case above: identical but for the keyword. It is
        // what makes the claim "the checker handles this shape" a measurement
        // rather than an assumption.
        Case {
            name: "a_plain_function_is_checked",
            rule: "spec/types.md — a declared type matches by the table and nothing else",
            source: concat!("fn int f(int a) { return a; }\n", "out f(\"hello\");\n"),
            checker_must_catch: true,
        },
        // Same cause, second symptom: an exported `let` never enters `var_types`,
        // so a call using it infers nothing and is unchecked.
        Case {
            name: "an_exported_let_gets_a_type_like_any_other",
            rule: "spec/types.md — the checker infers the type of a top-level `let`",
            source: concat!(
                "export let s = \"x\";\n",
                "fn int f(int a) { return a; }\n",
                "out f(s);\n"
            ),
            checker_must_catch: true,
        },
        // Rules the checker is right about. They belong here so that the rows
        // above cannot be satisfied by simply switching the checker off.
        Case {
            name: "no_widening_is_reported_and_enforced",
            rule: "spec/types.md — `int` does not widen to `decimal` at a parameter",
            source: concat!(
                "fn decimal half(decimal d) { return d; }\n",
                "out half(1);\n"
            ),
            checker_must_catch: true,
        },
    ]
}

fn parse(source: &str) -> Program {
    let mut parser = Parser::new(Lexer::new(source.to_string()));
    parser.set_source(source.lines().map(str::to_string).collect());
    let program = parser.parse_program();
    assert!(
        !parser.has_errors(),
        "every case must parse cleanly; got {:?}",
        parser.take_errors()
    );
    program
}

/// What the checker says: the messages, so a failure shows the finding itself.
fn checker_findings(program: &Program) -> Vec<String> {
    let mut checker = TypeChecker::new(program);
    checker.check();
    checker
        .take_errors()
        .into_iter()
        .map(|d| d.message)
        .collect()
}

/// Whether the runtime accepts the program.
fn runtime_accepts(source: &str, program: &Program) -> bool {
    let mut evaluator = Evaluator::new();
    evaluator.set_source(source.lines().map(str::to_string).collect());
    evaluator.eval_program_outcome(program).is_success()
}

#[test]
fn the_checker_and_the_runtime_agree_about_the_same_program() {
    let known: std::collections::HashMap<&str, &str> = KNOWN_DIVERGENCES.iter().copied().collect();
    let mut false_positives = Vec::new();
    let mut stale = Vec::new();
    let mut missed = Vec::new();
    let mut reported_misses = Vec::new();

    for case in cases() {
        let program = parse(case.source);
        let findings = checker_findings(&program);
        let accepted = runtime_accepts(case.source, &program);

        match (accepted, findings.is_empty()) {
            // The runtime accepts and the checker complains: a false positive.
            (true, false) if !known.contains_key(case.name) => false_positives.push(format!(
                "  {}\n    rule: {}\n    the runtime accepts this program, and the checker says:\n      {}",
                case.name,
                case.rule,
                findings.join("\n      ")
            )),
            // Listed as diverging, but it no longer does. The list must not
            // outlive the divergence, or it becomes a place where a fixed defect
            // is still recorded as intended.
            (true, true) if known.contains_key(case.name) => stale.push(case.name),
            // The runtime rejects and the checker was silent.
            (false, true) => {
                if case.checker_must_catch {
                    missed.push(format!("  {}\n    rule: {}", case.name, case.rule));
                } else {
                    reported_misses.push(case.name);
                }
            }
            _ => {}
        }
    }

    if !reported_misses.is_empty() {
        println!(
            "\nchecker misses, partial by design — reported, not asserted:\n  {}\n",
            reported_misses.join("\n  ")
        );
    }

    assert!(
        stale.is_empty(),
        "KNOWN_DIVERGENCES lists {} case(s) that no longer diverge — remove them:\n  {}",
        stale.len(),
        stale.join("\n  ")
    );

    assert!(
        missed.is_empty(),
        "{} case(s) the runtime rejects and the checker should have caught:\n\n{}\n\n\
         Each of these is a shape the checker handles in another spelling, so the \
         gap is an oversight rather than the documented partiality.",
        missed.len(),
        missed.join("\n\n")
    );

    assert!(
        false_positives.is_empty(),
        "{} case(s) where the checker reports a program the runtime accepts.\n\n{}\n\n\
         The checker is advisory, so each of these is noise printed over correct code. \
         spec/types.md is normative: if it sides with the runtime, the checker is wrong \
         and should be fixed. If the divergence is intended, add it to KNOWN_DIVERGENCES \
         with the decision or spec sentence that justifies it.",
        false_positives.len(),
        false_positives.join("\n\n")
    );
}
