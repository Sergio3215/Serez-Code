//! The single place a diagnostic becomes text.
//!
//! `spec/errors.md` states that human diagnostics are rendered **once at the
//! pipeline boundary**. Before M3.6 that was a rule the code merely happened to
//! follow: the parser rendered its own and the lexer's, the type checker
//! rendered inside its collector, and the evaluator had sixteen `eprintln!`
//! calls. Four producers, four slightly different formats, and no way to see
//! that they differed except by reading all four.
//!
//! This module is the one home. It takes a [`Diagnostic`] and returns a
//! `String`; it does not print. That is deliberate — a renderer that writes to
//! stderr can only be tested by capturing stderr, and one that returns a string
//! can be asserted on directly, which is why `render_tests` below can pin the
//! exact byte layout of every phase without running a binary.
//!
//! # The formats, and why they differ
//!
//! ```text
//! ❌ PARSER ERROR [SZ2000] [file.sz 3:7]: Unexpected token
//!   3 | let = 1;
//!         ^
//! ❌ LEXER ERROR [SZ1003] [line 1:9]: Unterminated string
//! ❌ TYPE ERROR [SZ3000] [line 2:6]: Wrong argument type
//! ❌ TYPE ERROR [SZ3000]: Wrong argument type
//! ❌ ERROR [SZ4003]: Index out of bounds
//! ❌ BETA LIMITATION [SZ6001]:
//! Network access is disabled in this online beta environment.
//! ```
//!
//! The last one is the only headline that is not an `ERROR`, and it is the only
//! one whose message starts on the next line. Both follow from
//! [`Diagnostic::origin`]: it names a capability an `--eval` run withholds, the
//! reader is meeting the language for the first time, and the text they need is
//! three paragraphs rather than a clause. See `crate::environment` for what it
//! says and why.
//!
//! Three further differences, all pre-existing, all preserved here rather than
//! tidied:
//!
//!   * **Runtime failures carry no position bracket**, though they have a span.
//!     The frames printed underneath carry the position instead, so a bracket
//!     would repeat it. This is the one rule that has to come from the phase;
//!     see [`Phase::shows_position`].
//!   * **The file name appears only when the caller knows it.** The parser is
//!     told the name; the type checker never is, so its diagnostics say
//!     `[line L:C]` even when `sz` was given a path. That is an inconsistency,
//!     recorded in `docs/maturity/ROADMAP_STATE.md`, not something to fix inside
//!     a refactor.
//!   * **The source line and caret appear only when the caller supplies the
//!     source.** Again only the parser does.
//!
//! Both of the last two therefore need no phase rule at all: they follow from
//! what the caller passes in [`Context`]. Only the first is a real decision, so
//! only the first is written down as one.

use crate::diagnostic::{Diagnostic, Origin, Phase};

/// What the caller knows about where the diagnostic came from.
///
/// Everything here is optional because the producers genuinely differ in what
/// they have: the parser is handed the file name and its lines, the type checker
/// and the evaluator are handed neither.
#[derive(Default)]
pub struct Context<'a> {
    /// The file name, as the user typed it. `None` renders the position as
    /// `[line L:C]` instead of `[name L:C]`.
    pub source_name: Option<&'a str>,
    /// The source, split by line, for the snippet and caret. Empty means no
    /// snippet is printed at all.
    pub source_lines: &'a [String],
}

impl Origin {
    /// The words between `❌` and the `[SZxxxx]` bracket.
    ///
    /// `None` for the ordinary case, where the headline is built from the phase
    /// — `ERROR`, `PARSER ERROR`, `TYPE ERROR`. An environment limitation
    /// replaces the whole of it, phase word included: the phase is a fact about
    /// the *runtime's* pipeline, and a reader being told that the playground
    /// does not do networking has no use for it.
    ///
    /// It deliberately does not contain the word `ERROR`. That is the entire
    /// point — see `crate::environment`.
    fn headline(&self) -> Option<&'static str> {
        match self {
            Origin::Program => None,
            Origin::EnvironmentLimit => Some("BETA LIMITATION"),
        }
    }

    /// Does the message start on the line after the headline?
    ///
    /// An ordinary diagnostic is one line and reads as one sentence, so it stays
    /// on the headline. An environment limitation is three paragraphs, and
    /// hanging the first of them off `: ` would bury the sentence that answers
    /// the reader's actual question in the middle of a line.
    fn message_on_its_own_line(&self) -> bool {
        matches!(self, Origin::EnvironmentLimit)
    }
}

impl Phase {
    /// Does this phase print a `[…]` position bracket?
    ///
    /// Every phase but the runtime. A runtime failure prints its stack frames
    /// underneath, and each frame already carries `line:column`, so a bracket on
    /// the headline would say the same thing twice. Pinned by
    /// `tests/diagnostic_render.rs` across 149 fixtures.
    pub fn shows_position(&self) -> bool {
        !matches!(self, Phase::Runtime)
    }
}

/// Render one diagnostic exactly as the user sees it, without a trailing
/// newline.
///
/// Multi-line output — the snippet and caret — is embedded, so a caller prints
/// the whole thing with a single `eprintln!("{}", …)`.
pub fn render(diagnostic: &Diagnostic, context: &Context<'_>) -> String {
    let mut out = String::from("❌ ");
    match diagnostic.origin.headline() {
        Some(headline) => out.push_str(headline),
        None => {
            if let Some(label) = diagnostic.phase.label() {
                out.push_str(label);
                out.push(' ');
            }
            out.push_str("ERROR");
        }
    }
    out.push_str(" [");
    out.push_str(diagnostic.code);
    out.push(']');

    // Absent when the phase does not use one, and absent when there is no
    // position to report — the type checker's `line == 0` case, which must stay
    // silent rather than printing `line 0:0`.
    if diagnostic.phase.shows_position() && diagnostic.has_position() {
        let (line, column) = (diagnostic.span.line, diagnostic.span.column);
        match context.source_name {
            Some(name) => out.push_str(&format!(" [{name} {line}:{column}]")),
            None => out.push_str(&format!(" [line {line}:{column}]")),
        }
    }

    out.push(':');
    out.push_str(if diagnostic.origin.message_on_its_own_line() {
        "\n"
    } else {
        " "
    });
    out.push_str(&diagnostic.message);

    if let Some(source) = context
        .source_lines
        .get(diagnostic.span.line.saturating_sub(1))
    {
        let number = diagnostic.span.line.to_string();
        out.push_str(&format!("\n  {} | {}", number, source.trim_end()));
        out.push_str(&format!(
            "\n  {}   {}^",
            " ".repeat(number.len()),
            " ".repeat(diagnostic.span.column.saturating_sub(1))
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    #[test]
    fn a_parser_diagnostic_names_its_file_and_points_at_the_column() {
        let source = lines("let = 1;\n");
        let d = Diagnostic::frontend("SZ2000", Phase::Parser, Span::point(1, 5), "Unexpected '='");
        let out = render(
            &d,
            &Context {
                source_name: Some("broken.sz"),
                source_lines: &source,
            },
        );
        // The caret sits under the `=`: `  1 | ` is six columns wide,
        // and column 5 of the source is four further along.
        assert_eq!(
            out,
            concat!(
                "❌ PARSER ERROR [SZ2000] [broken.sz 1:5]: Unexpected '='
",
                "  1 | let = 1;
",
                "          ^",
            )
        );
    }

    #[test]
    fn without_a_file_name_the_bracket_says_line() {
        let d = Diagnostic::frontend("SZ1003", Phase::Lexer, Span::point(1, 9), "Unterminated");
        assert_eq!(
            render(&d, &Context::default()),
            "❌ LEXER ERROR [SZ1003] [line 1:9]: Unterminated"
        );
    }

    #[test]
    fn an_unknown_position_prints_no_bracket_at_all() {
        // The type checker's `line == 0`. `[line 0:0]` would be worse than
        // nothing, and `spec/types.md` never promised a position.
        let d = Diagnostic::frontend("SZ3000", Phase::Type, Span::unknown(), "Wrong type");
        assert_eq!(
            render(&d, &Context::default()),
            "❌ TYPE ERROR [SZ3000]: Wrong type"
        );

        let placed = Diagnostic::frontend("SZ3000", Phase::Type, Span::point(2, 6), "Wrong type");
        assert_eq!(
            render(&placed, &Context::default()),
            "❌ TYPE ERROR [SZ3000] [line 2:6]: Wrong type"
        );
    }

    #[test]
    fn a_runtime_failure_has_no_phase_word_and_no_position() {
        // Both are contracts. The stack frames underneath carry the position.
        let mut d = Diagnostic::frontend(
            "SZ4003",
            Phase::Runtime,
            Span::point(2, 19),
            "Out of bounds",
        );
        d.kind = Some("IndexOutOfBounds".to_string());
        assert_eq!(
            render(&d, &Context::default()),
            "❌ ERROR [SZ4003]: Out of bounds"
        );
    }

    #[test]
    fn an_environment_limitation_is_not_announced_as_an_error() {
        // The shape the playground shows a first-time reader. `ERROR` is absent
        // on purpose: the capability works, this environment does not have it.
        let message = crate::environment::Capability::Network.limitation();
        let mut d = Diagnostic::frontend(
            "SZ6001",
            Phase::Runtime,
            Span::point(3, 11),
            message.clone(),
        );
        d.origin = Origin::EnvironmentLimit;
        d.kind = Some("PermissionError".to_string());

        let out = render(&d, &Context::default());
        assert_eq!(out, format!("❌ BETA LIMITATION [SZ6001]:\n{message}"));
        assert!(
            !out.contains("ERROR"),
            "the playground must not announce a working capability as an error: {out}"
        );
        // The three lines a first-time reader needs, in the order the questions
        // arrive in. `environment` owns the text; this owns the frame around it.
        assert!(out.starts_with("❌ BETA LIMITATION [SZ6001]:\n"), "{out}");
        assert!(
            out.contains("\n\nThis playground is intended for quick testing"),
            "{out}"
        );
        assert!(out.ends_with("install Serez Code locally from serezcode.org."));
    }

    #[test]
    fn the_code_and_the_bracket_survive_the_new_headline() {
        // The headline is the only thing that changes. Tooling classifies on the
        // code, and `compatibility.md` promises the code does not move.
        let mut d = Diagnostic::frontend("SZ2000", Phase::Parser, Span::point(1, 5), "m");
        d.origin = Origin::EnvironmentLimit;
        let out = render(&d, &Context::default());
        assert!(out.starts_with("❌ BETA LIMITATION [SZ2000]"), "{out}");
        assert!(out.contains("[line 1:5]"), "a position still prints: {out}");
    }

    #[test]
    fn the_caret_counts_from_one() {
        let source = lines("out x;");
        let d = Diagnostic::frontend("SZ2000", Phase::Parser, Span::point(1, 1), "m");
        let out = render(
            &d,
            &Context {
                source_name: None,
                source_lines: &source,
            },
        );
        let caret = out.lines().last().unwrap();
        assert_eq!(
            caret.find('^'),
            Some(6),
            "a column of 1 must not be indented"
        );
    }
}
