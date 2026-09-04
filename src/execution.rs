//! Whether execution is inside `unsafe { }`, and what that is allowed to change.
//!
//! # Safe by default
//!
//! Serez has no `safe` keyword and will not grow one. Ordinary code runs under
//! the runtime's guarantees because that is the default, not because it opted
//! in. The only explicit syntax is `unsafe { }`, and it marks a *deliberate*
//! step outside **specific, named** guarantees — not outside "safety".
//!
//! # What `unsafe` is not
//!
//! It is not a permission. `use permissions { OS }` says the program is
//! authorised to reach the OS capability; `unsafe` says the author accepts a
//! named relaxation. Neither substitutes for the other, and the measurements
//! that say so live in `tests/unsafe_contract.rs`:
//!
//! ```text
//! unsafe { OS.exec(…) }                       SZ6001  no permission
//! use permissions { OS }  OS.exec(…)          SZ6003  no unsafe
//! use permissions { OS }  unsafe { OS.exec }  runs
//! ```
//!
//! Inside `unsafe`, these do **not** stop applying: permissions, lockdown,
//! argument validation, the protected-path checks, type safety, the parser's
//! guarantees, the interpreter's own invariants, and every limit not listed in
//! [`Guarantee`]. A relaxation exists only when it is named here.
//!
//! # How the context propagates, measured rather than assumed
//!
//! `unsafe` in Serez is **dynamic**: a function *called* from inside an
//! `unsafe { }` block runs with the context in effect, even though its body is
//! nowhere near an `unsafe` keyword.
//!
//! ```text
//! fn void helper() { OS.exec("cmd", ["/c","echo","hi"]); }
//! unsafe { helper(); }                        runs
//! ```
//!
//! `spec/security.md` said the call must "appear **lexically** inside an
//! `unsafe { }` block", which is not what the runtime does. The divergence is
//! **DEC-M11-001**; this module implements and documents the measured behaviour
//! and does not change it. Worth knowing before that decision is taken: every
//! one of the 20 gated calls across the eight ecosystem packages, and 145 of the
//! 159 in the corpus, is *already* lexical — the other 14 are the fixtures that
//! call outside a block on purpose. Nothing measured relies on the dynamic
//! reading.
//!
//! Leaving a block restores the previous context immediately, including when the
//! block is left by `throw` or by `return`, and nesting composes.
//!
//! # Why a type rather than a `bool`
//!
//! The flag was `Evaluator::in_unsafe_block`, read directly at one site. Asking
//! "are we in an unsafe block?" at each new call site is how a keyword acquires
//! a different meaning per namespace. The question a guard should ask is not
//! *where am I* but *may I relax this particular guarantee*, and that is
//! [`ExecutionContext::waives`] — one place to add a guarantee, one place to see
//! every guarantee there is.

/// A runtime guarantee that `unsafe { }` is defined to relax.
///
/// The list is the contract. A limit that is not here is **not** waivable, and
/// adding a variant is a product decision rather than a refactor — which is the
/// point of enumerating them instead of testing a boolean at each guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Guarantee {
    /// How much of a child process's stdout and stderr the runtime will
    /// accumulate before refusing.
    ///
    /// Waivable because the caller of `OS.exec` chose the command and is the
    /// only party who can know whether its output is bounded. The runtime
    /// cannot: the size is the child's to decide.
    ///
    /// Waiving it means the process may hold as much as the child emits — about
    /// **5×** it, measured — and that is the author's undertaking, stated in
    /// `spec/security.md` and `spec/limits.md` rather than discovered.
    ProcessOutputCeiling,
}

impl Guarantee {
    /// Every guarantee `unsafe` may relax, for documentation and tests.
    ///
    /// A test walks this so a variant added without a spec entry is caught by
    /// the suite rather than by a reader.
    pub const ALL: &'static [Guarantee] = &[Guarantee::ProcessOutputCeiling];

    /// The spec name, as it appears in `security.md`'s table.
    pub fn name(self) -> &'static str {
        match self {
            Guarantee::ProcessOutputCeiling => "process output ceiling",
        }
    }
}

/// Where execution is with respect to the runtime's guarantees.
///
/// Small on purpose. It holds one fact today and exists so that the rest of the
/// runtime asks a question instead of reading a flag.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionContext {
    inside_unsafe: bool,
}

impl ExecutionContext {
    /// The default: safe, because that is what ordinary code is.
    pub fn new() -> Self {
        ExecutionContext::default()
    }

    /// Is execution inside an `unsafe { }` block?
    ///
    /// This is the question the `unsafe` **gate** asks — "was this operation
    /// authorised at all". A guard deciding whether to relax a limit should ask
    /// [`Self::waives`] instead, so that the set of relaxations stays
    /// enumerable.
    pub fn is_unsafe(&self) -> bool {
        self.inside_unsafe
    }

    /// May `guarantee` be relaxed here?
    ///
    /// False in ordinary code, for every guarantee. `unsafe` is what makes it
    /// true, and only for the guarantees [`Guarantee`] lists.
    pub fn waives(&self, guarantee: Guarantee) -> bool {
        match guarantee {
            Guarantee::ProcessOutputCeiling => self.inside_unsafe,
        }
    }

    /// Enter an `unsafe { }` block, returning the context to restore on the way
    /// out.
    ///
    /// Returning the previous value rather than counting depth keeps the
    /// save/restore shape the evaluator already had, which is what makes
    /// nesting and an early `throw` or `return` behave identically to before.
    #[must_use = "the previous context must be restored when the block ends"]
    pub fn enter_unsafe(&mut self) -> ExecutionContext {
        let previous = *self;
        self.inside_unsafe = true;
        previous
    }

    /// Restore a context saved by [`Self::enter_unsafe`].
    pub fn restore(&mut self, previous: ExecutionContext) {
        *self = previous;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ordinary code waives nothing. This is the safe-by-default contract, and
    /// it is asserted over *every* guarantee rather than the one that exists, so
    /// a variant added later cannot quietly default to waived.
    #[test]
    fn a_default_context_waives_nothing() {
        let ctx = ExecutionContext::new();
        assert!(!ctx.is_unsafe());
        for guarantee in Guarantee::ALL {
            assert!(
                !ctx.waives(*guarantee),
                "'{}' is waived outside unsafe",
                guarantee.name()
            );
        }
    }

    /// And `unsafe` waives exactly the enumerated ones.
    #[test]
    fn an_unsafe_context_waives_the_listed_guarantees() {
        let mut ctx = ExecutionContext::new();
        let _ = ctx.enter_unsafe();
        assert!(ctx.is_unsafe());
        for guarantee in Guarantee::ALL {
            assert!(
                ctx.waives(*guarantee),
                "'{}' is listed as waivable and was not waived",
                guarantee.name()
            );
        }
    }

    /// Leaving restores, and nesting composes.
    ///
    /// The inner block's exit must not clear the outer one, which is the bug a
    /// plain `= false` on exit would have.
    #[test]
    fn nesting_restores_the_enclosing_context() {
        let mut ctx = ExecutionContext::new();

        let outer = ctx.enter_unsafe();
        assert!(ctx.is_unsafe());

        let inner = ctx.enter_unsafe();
        assert!(ctx.is_unsafe());
        ctx.restore(inner);
        assert!(
            ctx.is_unsafe(),
            "leaving the inner block cleared the outer one"
        );

        ctx.restore(outer);
        assert!(
            !ctx.is_unsafe(),
            "leaving the outer block did not restore safe"
        );
    }

    /// A saved context can be restored more than once without drifting.
    #[test]
    fn restoring_is_idempotent() {
        let mut ctx = ExecutionContext::new();
        let saved = ctx.enter_unsafe();
        ctx.restore(saved);
        ctx.restore(saved);
        assert!(!ctx.is_unsafe());
    }
}
