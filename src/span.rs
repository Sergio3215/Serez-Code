//! Where something is in the source.
//!
//! One type, shared by the lexer, the parser, the AST and every diagnostic
//! producer. It lives in its own module rather than in `ast.rs` for a reason
//! that matters: `Token` carries a span too, and if the type lived with the AST
//! the lexer would have to depend on the AST to produce a token. A span is
//! neither syntax nor a value — it is a fact about text — so it depends on
//! nothing and everything may depend on it.
//!
//! # What it stores, and why both
//!
//! `line` and `column` are what gets *rendered*, and they are normative:
//! `spec/lexical-grammar.md` states they are one-based and that **columns count
//! Unicode scalar values, not UTF-8 bytes**. `spec/errors.md` states that the
//! `span` field of a caught `Error` is the string `"line:column"`, or null when
//! unavailable. Neither may drift.
//!
//! `start` and `end` are byte offsets into the source, and they are the half a
//! range needs — an editor underlines a range, not a point. They are carried
//! rather than derived because deriving them would mean re-scanning the source
//! at every site that wants one, and because deriving *line/column from offsets*
//! instead would put a conversion in the path of every diagnostic the language
//! already renders correctly. Storing both is 16 bytes and no conversion; it is
//! the cheap direction to be wrong in.
//!
//! # Byte offsets, and the empty span
//!
//! `start`/`end` are byte offsets, not character indices, because that is what
//! slicing the source needs. `end` is exclusive. A span with `start == end` is
//! a point rather than a range, and is what a node gets when its position is
//! known but its extent is not — which, during M2's migration, is most of them.
//!
//! # Migration state
//!
//! M2 is adding this type to the AST family by family. Until that finishes,
//! most nodes still carry no span at all: `docs/maturity/ROADMAP_STATE.md` §5.4
//! records that five of 48 node types had a position when M2 began, and §9B.1
//! records that nothing yet *reads* one — a runtime error takes its position
//! from the call stack, and the LSP discards the parse tree. Spans are being
//! laid down ahead of their consumers deliberately; see the decision in §9B.2.

/// A region of source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    /// One-based line. `0` means unknown.
    pub line: usize,
    /// One-based column, counted in Unicode scalar values. `0` means unknown.
    pub column: usize,
    /// Byte offset of the first byte, into the source this span refers to.
    pub start: usize,
    /// Byte offset one past the last byte. Equal to `start` for a point.
    pub end: usize,
}

impl Span {
    /// A span at a known line and column whose extent is not known.
    ///
    /// This is what the migration produces for a node built before its tokens
    /// carried offsets, and what a synthetic node gets. It renders exactly as
    /// the `line`/`column` pair it replaces.
    pub fn point(line: usize, column: usize) -> Self {
        Span {
            line,
            column,
            start: 0,
            end: 0,
        }
    }

    /// No position at all — the `null` of `spec/errors.md`'s `Error.span`.
    pub fn unknown() -> Self {
        Span::default()
    }

    /// Is there a line to report?
    ///
    /// `spec/errors.md` allows a diagnostic to have no position, and renderers
    /// check this rather than printing `0:0`.
    pub fn is_known(&self) -> bool {
        self.line > 0
    }

    /// Does this span cover a range of bytes, or only mark a point?
    pub fn has_extent(&self) -> bool {
        self.end > self.start
    }
}

impl std::fmt::Display for Span {
    /// `line:column`, the shape `spec/errors.md` promises for a caught
    /// `Error.span`. Callers that must produce `null` for an unknown position
    /// check [`Span::is_known`] first rather than relying on this.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_renders_as_the_pair_it_replaces() {
        assert_eq!(Span::point(3, 17).to_string(), "3:17");
    }

    #[test]
    fn an_unknown_span_is_distinguishable_from_the_first_character() {
        // `spec/errors.md` allows a null span, and the first character of a file
        // is 1:1 — never 0:0 — so zero is free to mean "unknown".
        assert!(!Span::unknown().is_known());
        assert!(Span::point(1, 1).is_known());
    }

    #[test]
    fn a_point_has_no_extent_and_a_range_does() {
        assert!(!Span::point(1, 1).has_extent());
        let range = Span {
            line: 1,
            column: 1,
            start: 4,
            end: 9,
        };
        assert!(range.has_extent());
    }
}
