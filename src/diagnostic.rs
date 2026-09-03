//! One shape for everything the language reports.
//!
//! Before M3 there were five: `LexError`, `ParseError`, `TypeError`,
//! `RuntimeError` and `CompilerDiagnostic`. Three of them were the same four
//! fields written out three times, and each phase rendered its own — so the
//! same conceptual failure could reach the CLI and the LSP in different shapes,
//! and adding a field to one meant remembering the other four.
//!
//! This module holds the data and **does not print**. That separation is the
//! point: `spec/errors.md` states that human diagnostics are rendered once at
//! the pipeline boundary, and a producer that *can* print is a producer that can
//! render one somewhere else too. M3.6 gives the rendering a single home; until
//! then the existing renderers keep their exact output, which
//! `tests/diagnostic_render.rs` holds them to.
//!
//! It lives in its own module, depending only on `span`, for the same reason
//! `span` does: the lexer, the parser, the checker and the evaluator all
//! produce diagnostics, and none of them should have to depend on another to do
//! it.
//!
//! # What is normative here
//!
//! - **`code`** is the stable identifier. `spec/errors.md` fixes the ranges:
//!   `SZ1xxx` lexical, `SZ2xxx` syntax, `SZ3xxx` semantic/type, `SZ4xxx`-`SZ6xxx`
//!   runtime, `SZ7xxx` the experimental compiler. Tooling classifies on this;
//!   `message` wording is explicitly not part of the contract.
//! - **`kind`** is the compatibility category a caught `Error` exposes
//!   (`TypeError`, `IndexOutOfBounds`, …). Only runtime failures have one.
//! - **`severity`** separates an advisory finding from a fatal one. The type
//!   checker is deliberately partial, so its findings are advisory and do not
//!   change the exit code — `spec/types.md` records that, and it is a contract,
//!   not an accident.
//! - **`span`** is where. `spec/errors.md` types the caught `Error.span` as
//!   `"line:column"` or null, which is why an unknown span stays representable
//!   rather than being forced to `0:0`.
//!
//! # What is deliberately not here
//!
//! **Catchability.** Whether a `try/catch` may consume a failure is a property
//! of the *failure*, not of the diagnostic describing it — a security denial and
//! an index error can render identically and still differ on this. It stays
//! where it is, on the evaluator's internal `PendingRuntimeError`, so that no
//! future diagnostic change can quietly make a fatal error catchable.

use crate::span::Span;

/// How much a diagnostic means.
///
/// Two levels, because the language has exactly two behaviours today: a finding
/// that changes the exit code and one that does not. Adding a third would be a
/// change to what `sz` does, not to how it is described, so the enum stays
/// honest about the current contract rather than anticipating one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The program will not run, or has stopped. Exit code 1.
    Error,
    /// Reported, and the program continues. The partial type checker's findings
    /// are the only ones today; `spec/types.md` states that `sz file.sz` reports
    /// them and still runs.
    Advisory,
}

/// Which phase produced a diagnostic.
///
/// Kept because the rendered form names it — `❌ PARSER ERROR [SZ2000] …` — so
/// it is part of the output contract rather than internal bookkeeping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Lexer,
    Parser,
    /// Rules about what a program *means* that reject it. Fatal, and distinct
    /// from [`Phase::Type`], which is advisory by contract in `spec/types.md`.
    /// DEC-M4-001 created the phase; DEC-M4-005 chose this label and the
    /// `SZ8xxx` range.
    Semantic,
    Type,
    Runtime,
    Compiler,
}

impl Phase {
    /// The word that appears in the rendered diagnostic.
    ///
    /// `spec/errors.md` fixes these: `❌ LEXER ERROR`, `❌ PARSER ERROR`,
    /// `❌ TYPE ERROR`. Runtime failures render as plain `❌ ERROR`, which is why
    /// this returns an `Option` rather than inventing a word for them.
    pub fn label(&self) -> Option<&'static str> {
        match self {
            Phase::Lexer => Some("LEXER"),
            Phase::Parser => Some("PARSER"),
            Phase::Semantic => Some("SEMANTIC"),
            Phase::Type => Some("TYPE"),
            Phase::Runtime => None,
            Phase::Compiler => Some("COMPILER"),
        }
    }
}

/// One frame of a runtime stack trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub name: String,
    pub span: Span,
}

/// Something the language has to tell someone about.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Stable `SZxxxx` identifier. The contract; `message` is not.
    pub code: &'static str,
    /// The phase that produced it, which the rendered form names.
    pub phase: Phase,
    /// Whether it stops the program.
    pub severity: Severity,
    /// Compatibility category for a caught runtime `Error`. `None` for
    /// everything the frontend produces, which has no `kind` to expose.
    pub kind: Option<String>,
    /// Human-readable. Wording is explicitly not a stable API.
    pub message: String,
    /// Where. May be [`Span::unknown`], which renders as no position at all
    /// rather than as `0:0`.
    pub span: Span,
    /// Innermost-first runtime frames. Empty for frontend diagnostics.
    pub stack: Vec<Frame>,
    /// Additional explanation.
    pub notes: Vec<String>,
}

impl Diagnostic {
    /// A frontend diagnostic: a code, a place, and a message.
    ///
    /// Covers the three types that were structurally identical before M3 —
    /// lexical, syntactic and type findings all have exactly this shape.
    pub fn frontend(
        code: &'static str,
        phase: Phase,
        span: Span,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            code,
            phase,
            // The type checker is partial by design and its findings do not
            // change the exit code; the lexer's, the parser's and the semantic
            // phase's do. `Phase::Semantic` falling into the `_` arm is the
            // intended reading and not an oversight: it is fatal, which is the
            // property that separates it from `Phase::Type` (DEC-M4-001).
            severity: match phase {
                Phase::Type => Severity::Advisory,
                _ => Severity::Error,
            },
            kind: None,
            message: message.into(),
            span,
            stack: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Is there a position to report?
    ///
    /// Callers that must produce `null` rather than `0:0` — the caught
    /// `Error.span` of `spec/errors.md` — ask this first.
    pub fn has_position(&self) -> bool {
        self.span.is_known()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_type_finding_is_advisory_and_the_others_are_not() {
        // Not a stylistic default: `spec/types.md` states the checker is partial
        // and that `sz file.sz` reports its findings and still runs. If this
        // ever flips, the exit-code contract flips with it.
        let checker = Diagnostic::frontend("SZ3000", Phase::Type, Span::unknown(), "x");
        assert_eq!(checker.severity, Severity::Advisory);

        for phase in [Phase::Lexer, Phase::Parser] {
            let d = Diagnostic::frontend("SZ2000", phase, Span::unknown(), "x");
            assert_eq!(
                d.severity,
                Severity::Error,
                "{phase:?} must stop the program"
            );
        }
    }

    #[test]
    fn a_runtime_diagnostic_has_no_phase_label_and_the_others_do() {
        // `spec/errors.md` renders runtime failures as plain `❌ ERROR [SZ4003]`,
        // with no phase word, while the frontend ones name their phase.
        assert_eq!(Phase::Runtime.label(), None);
        assert_eq!(Phase::Parser.label(), Some("PARSER"));
        assert_eq!(Phase::Lexer.label(), Some("LEXER"));
        assert_eq!(Phase::Type.label(), Some("TYPE"));
    }

    #[test]
    fn an_unknown_span_is_reportable_as_absent() {
        let d = Diagnostic::frontend("SZ1001", Phase::Lexer, Span::unknown(), "x");
        assert!(!d.has_position());
        let placed = Diagnostic::frontend("SZ1001", Phase::Lexer, Span::point(3, 7), "x");
        assert!(placed.has_position());
    }
}
