//! The depth ceiling, and the accounting that enforces it.
//!
//! Split out of the grammar because it is a resource boundary rather than a
//! syntax rule: it exists so that ordinary source cannot exhaust the native
//! stack and kill the process without a diagnostic. `spec/errors.md` and
//! `spec/syntax.md` both name the number, and `tests/parser_facade.rs` asserts
//! it, so the constant here is a published contract rather than a tuning knob.

use super::Parser;

/// Source describes a tree deeper than [`MAX_PARSE_DEPTH`].
pub const SZ_PARSE_DEPTH_EXCEEDED: &str = "SZ2001";

/// Hard ceiling on the depth of the AST a single source file may describe.
///
/// Without a ceiling, ordinary text kills the process with no diagnostic at all
/// — no line number, no exit code the CLI chose, nothing for the LSP to
/// underline (`STATUS_STACK_OVERFLOW` on Windows, `SIGSEGV` elsewhere). Two
/// different shapes of source got there, and both are bounded here:
///
///   * **Nesting.** The parser is recursive descent, so `((((…1…))))` turns one
///     level of source into one Rust stack frame. Measured crash point in a
///     release build: between 32k and 50k levels.
///   * **Operator chains.** `1 + 1 + 1 + …` parses in a *flat* loop, so it never
///     troubles the parser — but it builds a left-leaning tree one level deeper
///     per operator, and the type checker, the evaluator and the AST's own drop
///     glue each recurse once per level of that tree. Measured crash point in a
///     release build: ~32k terms when evaluated, ~1M when only type-checked.
///
/// So the ceiling counts tree depth, not parser recursion: an operator chain
/// charges one level per operator. See [`Parser::charge_depth`].
///
/// 512 is sized against the tightest stack in play rather than the roomiest,
/// and it costs real code nothing. Across the 999 `.sz`/`.szx` files in the
/// official ecosystem the deepest nesting is 19 levels and the longest operator
/// chain is 25 — the ceiling clears both by more than 20×. Source that
/// legitimately needs more should build the structure at runtime instead of
/// spelling it out.
pub const MAX_PARSE_DEPTH: usize = 512;

/// Releases the levels one parser call charged, on the way out of that call.
///
/// Recursive-descent methods return early from dozens of places (`?`,
/// `return None`, `match` arms), so decrementing by hand would leak levels on
/// the first missed path. Holding the counter in an `Rc<Cell<_>>` rather than
/// borrowing the parser lets the guard live across `&mut self` calls in the
/// body it protects.
///
/// A guard can hold more than one level: an infix chain charges one level per
/// operator it appends and releases them all at once. See
/// [`Parser::charge_depth`].
pub(super) struct DepthGuard {
    counter: std::rc::Rc<std::cell::Cell<usize>>,
    held: usize,
}

impl DepthGuard {
    /// A guard holding nothing yet.
    pub(super) fn empty(counter: &std::rc::Rc<std::cell::Cell<usize>>) -> Self {
        DepthGuard {
            counter: std::rc::Rc::clone(counter),
            held: 0,
        }
    }
}

impl Drop for DepthGuard {
    fn drop(&mut self) {
        self.counter
            .set(self.counter.get().saturating_sub(self.held));
    }
}

impl Parser {
    /// Enter one level of recursive descent, or refuse to.
    ///
    /// `None` means the ceiling was hit and an error was already reported; the
    /// caller must propagate it like any other parse failure so `parse_program`
    /// can synchronize instead of recursing further.
    pub(super) fn enter_depth(&self) -> Option<DepthGuard> {
        let mut guard = DepthGuard::empty(&self.depth);
        self.charge_depth(&mut guard)?;
        Some(guard)
    }

    /// Charge one more level of AST depth to `guard`.
    ///
    /// The counter tracks the depth of the *tree being built*, which is not the
    /// same as how deep the parser has recursed. `a + b + c + …` parses in a
    /// flat loop but produces a left-leaning tree one level deeper per operator,
    /// and every walker downstream — the type checker, the evaluator, and the
    /// AST's own drop glue — recurses once per level of that tree. Charging the
    /// loop keeps a single ceiling covering both shapes.
    pub(super) fn charge_depth(&self, guard: &mut DepthGuard) -> Option<()> {
        let next = self.depth.get() + 1;
        if next > MAX_PARSE_DEPTH {
            self.parser_error_code(
                SZ_PARSE_DEPTH_EXCEEDED,
                &format!(
                    "Expression nests deeper than the {} level limit (an operator \
                     chain counts one level per operator)",
                    MAX_PARSE_DEPTH
                ),
            );
            return None;
        }
        self.depth.set(next);
        guard.held += 1;
        Some(())
    }
}
