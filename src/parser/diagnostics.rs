//! How the parser reports a problem, and what a caller gets back.
//!
//! Two audiences, deliberately separated at the point of production: a
//! `ParseError` is pushed onto an ordered list for tooling to classify by
//! `code`, and a rendered block goes to stderr for a person to read. The label
//! setters live here for the same reason — `source_lines` and `source_name`
//! feed the rendering and nothing else, which is why the structured payload
//! carries neither (`tests/parser_facade.rs` pins that).
//!
//! Two behaviors here are recorded rather than endorsed, both in
//! `docs/maturity/ROADMAP_STATE.md`:
//!
//!   * `take_errors` reads the list; it does not take it (§5.11).
//!   * `flush_lexer_errors` runs at the end of `parse_program`, so every
//!     `SZ2xxx` precedes every `SZ1xxx` no matter where in the file they
//!     occurred (§5.12), and `has_errors()` is false before the flush even when
//!     the source is already lexically broken (§5.13).
//!
//! And one that is a defect: nine sites in the grammar never come through here
//! at all. They set `had_error` by hand and print their own line, so
//! `take_errors` returns nothing for them and the LSP shows nothing (§5.17).
//! M1 preserves that; M3 owns the fix.

use super::Parser;
use crate::diagnostic::{Diagnostic, Phase};
use crate::render;
use crate::span::Span;

/// Generic parser diagnostic: a syntax error not yet given a narrower code.
///
/// Most of the parser's several hundred messages still land here. That is
/// deliberate — a code is a promise of stability, so they get split out one at
/// a time as each acquires a test that pins its meaning, rather than by
/// numbering every message at once and freezing distinctions nobody checked.
pub const SZ_PARSE_ERROR: &str = "SZ2000";

/// A frontend diagnostic. Parser findings use `SZ2xxx`; lexical ones are
/// forwarded as `SZ1xxx` so callers and the LSP consume one ordered shape.
///
/// M3 collapsed this into the shared [`Diagnostic`]. The alias remains because
/// `parser::ParseError` is a published path — `run.rs`, the robustness tests and
/// any embedder name it — and because "parse error" is still the right word at
/// the point of production.
pub type ParseError = Diagnostic;

impl Parser {
    /// Whether any parse error was reported while building the program.
    pub fn has_errors(&self) -> bool {
        self.had_error.get()
    }

    /// All parse errors reported so far, with positions. Used by tooling (LSP).
    pub fn take_errors(&self) -> Vec<ParseError> {
        self.errors.borrow().clone()
    }

    pub fn set_source(&mut self, lines: Vec<String>) {
        self.source_lines = lines;
    }

    pub fn set_source_name(&mut self, name: &str) {
        self.source_name = Some(name.to_string());
    }

    pub(super) fn parser_error(&self, msg: &str) {
        self.parser_error_code(SZ_PARSE_ERROR, msg);
    }

    /// Report a parse error under a specific stable diagnostic code.
    pub(super) fn parser_error_code(&self, code: &'static str, msg: &str) {
        self.had_error.set(true);
        let line = self.current_token.span.line;
        let col = self.current_token.span.column;
        let diagnostic = Diagnostic::frontend(code, Phase::Parser, Span::point(line, col), msg);
        self.print_frontend_error(&diagnostic);
        self.errors.borrow_mut().push(diagnostic);
    }

    /// Print one frontend diagnostic, via the single renderer.
    ///
    /// The parser is the only producer that knows both the file name and the
    /// source text, so it is the only one whose output carries a name in the
    /// bracket and a caret line underneath. Both follow from what it puts in
    /// the [`Context`]; `crate::render` has no parser-specific rule.
    fn print_frontend_error(&self, diagnostic: &Diagnostic) {
        eprintln!(
            "{}",
            render::render(
                diagnostic,
                &render::Context {
                    source_name: self.source_name.as_deref(),
                    source_lines: &self.source_lines,
                },
            )
        );
    }

    pub(super) fn flush_lexer_errors(&self) {
        let lexical = std::mem::take(&mut *self.lexer_errors.borrow_mut());
        if lexical.is_empty() {
            return;
        }
        self.had_error.set(true);
        for error in lexical {
            self.print_frontend_error(&error);
            self.errors.borrow_mut().push(error);
        }
    }
}
