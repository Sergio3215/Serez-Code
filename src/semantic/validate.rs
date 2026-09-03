//! The semantic phase: rules about *meaning* that reject a program.
//!
//! # Why this exists
//!
//! Serez had exactly two modes of rejection. **Syntactic** — fatal, in the parser.
//! And **type** — advisory, which by contract in `spec/types.md` rejects nothing:
//! `sz file.sz` prints an `SZ3000` and runs the program anyway. There was no
//! third.
//!
//! So every rule about what a program *means* that needs to reject it had to
//! disguise itself as a rule about what it *looks like*. `Parser::is_reserved_name`
//! is the one that did, and the disguise is visible in its output: rejecting
//! `class Task { … }` produces the real error plus two invented ones, because the
//! parser abandons a half-built declaration in order to say something that is not
//! about structure at all.
//!
//! **DEC-M4-001**, decided 2026-09-03, is the choice to have this phase.
//! `docs/maturity/ROADMAP_STATE.md` §7A carries the rationale and the alternatives
//! that were rejected.
//!
//! # Where it sits
//!
//! ```text
//! Lexer → Parser → AST → [semantic] → TypeChecker → Evaluator
//!                         ↑ here      ↑ advisory     ↑ authoritative
//! ```
//!
//! After the parser, because it needs a complete tree — that is the whole point,
//! and what makes one error out of three possible. Before the type checker,
//! because it is *fatal* and the checker is not: a program this phase rejects must
//! not reach a stage whose findings are allowed to be ignored.
//!
//! It runs only on a tree the parser accepted. Validating a broken tree would
//! report consequences of the syntax error rather than problems of its own.
//!
//! # What it is not
//!
//! Not a type checker: it answers questions with yes-or-no answers that do not
//! need inference. Not a resolver — whether an unresolved name is a diagnostic is
//! **DEC-M4-002** and is still open. Not a place to move checks that are genuinely
//! syntactic; the parser rejecting `let = 3` stays where it is.
//!
//! # Candidate rules
//!
//! §5.39 measured three gaps this phase could close, all silent today: a duplicate
//! `class` or `fn` is accepted with the last one winning, and a class inheriting
//! from something that does not exist runs clean until someone instantiates it.
//! Together with DEC-M5-003 (unknown type name), DEC-M4-002 (free variables) and
//! DEC-M7-003 (`match` exhaustiveness), that is six candidate tenants — which is
//! what makes this a boundary rather than a wrapper around one rule.
//!
//! **None of them is implemented here yet, and none may be added without its own
//! decision.** What a duplicate declaration *should* do is a contract question
//! that deciding where validation lives did not answer.

use crate::ast::{Program, Statement};
use crate::diagnostic::{Diagnostic, Phase};
use crate::span::Span;

/// Generic semantic diagnostic: a meaning-level rejection not yet given a
/// narrower code.
///
/// `SZ8000` is the generic code of its range, the way `SZ2000` and `SZ3000` are
/// of theirs. `spec/errors.md` sets the rule: a message moves to a narrower code
/// only once a test pins what that code means, so `SZ8001`+ get allocated when
/// there is something to pin, not in advance. DEC-M4-005.
pub const SZ_SEMANTIC_ERROR: &str = "SZ8000";

/// Runtime namespaces that may not be shadowed by a user declaration.
///
/// Seven of the twenty-two the runtime actually has. That the list is a subset,
/// and which subset, is **DEC-M4-003** — still open, and deliberately not
/// answered by moving the rule here. §5.31 has the measurement: a program may
/// declare `class Math`, call `new Math(42)`, and still call `Math.floor(3.7)`,
/// because `Math` is one of the fifteen this list omits.
const RESERVED_NAMESPACES: &[&str] = &["Task", "Time", "DateTime", "System", "Gui", "Dec", "Media"];

fn reserved(name: &str) -> bool {
    RESERVED_NAMESPACES.contains(&name)
}

/// Every semantic problem in `program`, in source order.
///
/// Empty means the program is semantically valid *as far as this phase checks*,
/// which today is: nothing. The phase is wired into the pipeline before it has
/// any rules, deliberately — see the module docs and M4.5.2. Introducing the
/// stage and introducing a rule are separate changes, and the first one must be
/// provably invisible before the second one is trusted.
///
/// A caller treats a non-empty result as **fatal**: the program does not run. That
/// is the distinction from `TypeChecker`, whose findings are advisory.
pub fn validate(program: &Program) -> Vec<Diagnostic> {
    let mut findings = Vec::new();
    for statement in &program.statements {
        check_statement(statement, &mut findings);
    }
    findings
}

/// Walks the top level, looking through `export`.
///
/// Top level only, matching what the parser rejected: the guard fired wherever a
/// `class`, `interface` or `enum` was parsed, and every one of those the parser
/// could reach was reachable from here. Nesting is deliberately not extended to
/// — that would change *which* programs are rejected, and DEC-M4-001 moved the
/// rule without changing its reach.
fn check_statement(statement: &Statement, findings: &mut Vec<Diagnostic>) {
    match statement {
        Statement::Export(inner) => check_statement(inner, findings),
        Statement::ClassDeclaration(c) => {
            reject_if_reserved(&c.name, "a class name", c.span, findings)
        }
        Statement::InterfaceDeclaration(i) => {
            reject_if_reserved(&i.name, "an interface name", i.span, findings)
        }
        Statement::EnumDeclaration(e) => {
            reject_if_reserved(&e.name, "an enum name", e.span, findings)
        }
        _ => {}
    }
}

fn reject_if_reserved(name: &str, what: &str, span: Span, findings: &mut Vec<Diagnostic>) {
    if reserved(name) {
        findings.push(Diagnostic::frontend(
            SZ_SEMANTIC_ERROR,
            Phase::Semantic,
            span,
            format!("'{name}' is a reserved system namespace and cannot be used as {what}"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn program(source: &str) -> Program {
        let mut parser = Parser::new(Lexer::new(source.to_string()));
        parser.set_source(source.lines().map(str::to_string).collect());
        let program = parser.parse_program();
        assert!(!parser.has_errors(), "fixture must parse cleanly");
        program
    }

    #[test]
    fn an_ordinary_program_reports_nothing() {
        assert!(validate(&program("let x = 1;\nout x;\n")).is_empty());
        assert!(
            validate(&program(
                "public class C {\n    public C() { this.v = 1; }\n}\nout new C().v;\n"
            ))
            .is_empty()
        );
    }

    #[test]
    fn a_reserved_namespace_is_rejected_as_a_class_interface_or_enum() {
        for source in [
            "class Task {}\n",
            "interface Task {}\n",
            "enum Task { A, B }\n",
            "export class Gui {}\n",
        ] {
            let findings = validate(&program(source));
            assert_eq!(findings.len(), 1, "one finding for {source:?}");
            assert_eq!(findings[0].code, SZ_SEMANTIC_ERROR);
            assert_eq!(findings[0].phase, Phase::Semantic);
            assert!(
                findings[0].span.line > 0,
                "a semantic finding points at the declaration"
            );
        }
    }

    #[test]
    fn the_list_is_the_seven_it_was_and_not_one_more() {
        // DEC-M4-003 is what may change this, and it is still open. Pinned so
        // the list cannot drift while that decision is unanswered.
        for name in ["Task", "Time", "DateTime", "System", "Gui", "Dec", "Media"] {
            assert!(reserved(name), "{name} is guarded");
        }
        // §5.31: a program may declare `class Math` and still call
        // `Math.floor(3.7)`. That is the hazard DEC-M4-003 is about, and moving
        // the rule did not fix it.
        for name in ["Math", "File", "Socket", "Crypto", "JSON", "OS", "Tensor"] {
            assert!(!reserved(name), "{name} is not guarded today");
        }
    }

    #[test]
    fn a_reserved_name_is_only_rejected_where_it_declares_a_type() {
        // A *variable* named Task was always allowed, and still is. The rule
        // moved phase; its reach did not change.
        assert!(validate(&program("let Task = 1;\nout Task;\n")).is_empty());
    }

    #[test]
    fn it_runs_on_a_tree_rather_than_on_text() {
        // The phase's whole advantage over the parser is that it sees complete
        // nodes. Pinned by construction: `validate` takes a `Program`, so a
        // future rule cannot quietly go back to scanning source.
        let p = program("enum Colour { Red, Green }\n");
        assert!(validate(&p).is_empty());
        assert_eq!(p.statements.len(), 1, "the tree is what was handed over");
    }
}
