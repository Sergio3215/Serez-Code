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

/// Generic parser diagnostic: a syntax error not yet given a narrower code.
///
/// Most of the parser's several hundred messages still land here. That is
/// deliberate — a code is a promise of stability, so they get split out one at
/// a time as each acquires a test that pins its meaning, rather than by
/// numbering every message at once and freezing distinctions nobody checked.
pub const SZ_PARSE_ERROR: &str = "SZ2000";

/// A frontend error with its source position (1-based line/column). Parser
/// diagnostics use `SZ2xxx`; lexical diagnostics are forwarded as `SZ1xxx` so
/// callers and the LSP consume one ordered diagnostic shape.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Stable `SZ1xxx`/`SZ2xxx` identifier. Tooling classifies on this; `message` is
    /// for humans and its wording is not part of the contract.
    pub code: &'static str,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

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
        self.errors.borrow_mut().push(ParseError {
            code,
            line,
            column: col,
            message: msg.to_string(),
        });
        self.print_frontend_error("PARSER", code, line, col, msg);
    }

    fn print_frontend_error(
        &self,
        phase: &str,
        code: &'static str,
        line: usize,
        col: usize,
        msg: &str,
    ) {
        match &self.source_name {
            Some(name) => eprintln!(
                "❌ {} ERROR [{}] [{} {}:{}]: {}",
                phase, code, name, line, col, msg
            ),
            None => eprintln!(
                "❌ {} ERROR [{}] [line {}:{}]: {}",
                phase, code, line, col, msg
            ),
        }
        if let Some(src) = self.source_lines.get(line.saturating_sub(1)) {
            let ln = line.to_string();
            eprintln!("  {} | {}", ln, src.trim_end());
            eprintln!(
                "  {}   {}^",
                " ".repeat(ln.len()),
                " ".repeat(col.saturating_sub(1))
            );
        }
    }

    pub(super) fn flush_lexer_errors(&self) {
        let lexical = std::mem::take(&mut *self.lexer_errors.borrow_mut());
        if lexical.is_empty() {
            return;
        }
        self.had_error.set(true);
        for error in lexical {
            self.print_frontend_error(
                "LEXER",
                error.code,
                error.line,
                error.column,
                &error.message,
            );
            self.errors.borrow_mut().push(ParseError {
                code: error.code,
                line: error.line,
                column: error.column,
                message: error.message,
            });
        }
    }
}
