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

use crate::ast::Program;
use crate::diagnostic::Diagnostic;

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
    let _ = program;
    Vec::new()
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
    fn the_phase_reports_nothing_until_it_has_a_rule() {
        // Not a placeholder: it is the assertion that makes M4.5.2 a
        // behaviour-preserving change. If this ever fails, some rule was added
        // without a decision.
        assert!(validate(&program("let x = 1;\nout x;\n")).is_empty());
        assert!(
            validate(&program(
                "public class C {\n    public C() { this.v = 1; }\n}\nout new C().v;\n"
            ))
            .is_empty()
        );
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
