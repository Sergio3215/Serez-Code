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
//! # `unsafe` has lexical scope — DEC-M11-001
//!
//! **`unsafe` authority does not cross a function-call boundary.** A block
//! relaxes guarantees for the statements and expressions written inside it, and
//! for nothing else. A function called from within one starts under the
//! runtime's ordinary guarantees, whatever its caller was doing.
//!
//! ```text
//! fn dangerous() { OS.exec("git", ["status"]) }   // SZ6003 — no unsafe here
//! unsafe { dangerous() }                          // the caller's block does not reach in
//!
//! fn dangerous() { unsafe { OS.exec("git", ["status"]) } }
//! unsafe { dangerous() }                          // runs — the callee declares its own
//! ```
//!
//! The point is that a function can be **audited locally**: whether it relaxes a
//! runtime guarantee is visible in its own body, and does not depend on who
//! calls it.
//!
//! Nesting inside the *same* body is unaffected — an `if`, a loop or a nested
//! block inside `unsafe { }` is still inside it. What ends the reach is a call.
//!
//! # How that is enforced
//!
//! [`ExecutionContext`] records the **call frame** a block was opened in, not a
//! boolean. The gate asks whether the frame asking is the frame that opened the
//! block; a callee runs one frame deeper, so it never is.
//!
//! Recording the frame rather than clearing a flag on entry to every call is
//! what makes this safe to add: the evaluator has five call-frame entry points
//! and twenty-seven exits, and a model that had to reset state at each exit would
//! leak the first time one was missed. Here nothing is reset on return — the
//! caller's frame number comes back on its own, and its block applies again.
//!
//! It was **dynamic** before, measured at every boundary:
//!
//! ```text
//! fn called from unsafe            callee ran unsafe
//! method called from unsafe        method ran unsafe
//! constructor from unsafe          ctor ran unsafe
//! lambda called from unsafe        lambda ran unsafe
//! callback passed through a fn     callback ran unsafe
//! ```
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

/// Which call frame, if any, is currently inside an `unsafe { }` block.
///
/// A frame number rather than a flag, because `unsafe` is lexical: the block
/// applies to the frame that opened it and to no other. See the module docs.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionContext {
    /// The call depth at which the innermost open block was entered.
    ///
    /// `None` in ordinary code. A callee is one frame deeper than whoever called
    /// it, so `Some(caller)` never matches a callee's own depth — which is the
    /// whole of DEC-M11-001, expressed as a comparison rather than as a reset at
    /// every call site.
    unsafe_at: Option<usize>,
}

impl ExecutionContext {
    /// The default: safe, because that is what ordinary code is.
    pub fn new() -> Self {
        ExecutionContext::default()
    }

    /// Is the frame at `frame` inside its own `unsafe { }` block?
    ///
    /// This is the question the `unsafe` **gate** asks — "was this operation
    /// authorised here". A guard deciding whether to relax a limit should ask
    /// [`Self::waives`] instead, so that the set of relaxations stays
    /// enumerable.
    pub fn is_unsafe(&self, frame: usize) -> bool {
        self.unsafe_at == Some(frame)
    }

    /// May `guarantee` be relaxed by the frame at `frame`?
    ///
    /// False in ordinary code, for every guarantee, and false in a frame whose
    /// *caller* opened a block. `unsafe` written in this frame is what makes it
    /// true, and only for the guarantees [`Guarantee`] lists.
    pub fn waives(&self, guarantee: Guarantee, frame: usize) -> bool {
        match guarantee {
            Guarantee::ProcessOutputCeiling => self.is_unsafe(frame),
        }
    }

    /// Enter an `unsafe { }` block opened in the frame at `frame`, returning the
    /// context to restore on the way out.
    ///
    /// Returning the previous value rather than counting depth keeps the
    /// save/restore shape the evaluator already had, which is what makes nesting
    /// and an early `throw` or `return` behave identically.
    #[must_use = "the previous context must be restored when the block ends"]
    pub fn enter_unsafe(&mut self, frame: usize) -> ExecutionContext {
        let previous = *self;
        self.unsafe_at = Some(frame);
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

    /// The frame a block was opened in. Any number; the tests care about
    /// same-frame versus deeper.
    const CALLER: usize = 3;
    const CALLEE: usize = 4;

    /// Ordinary code waives nothing.
    ///
    /// Asserted over *every* guarantee rather than the one that exists, so a
    /// variant added later cannot quietly default to waived.
    #[test]
    fn a_default_context_waives_nothing() {
        let ctx = ExecutionContext::new();
        assert!(!ctx.is_unsafe(CALLER));
        for guarantee in Guarantee::ALL {
            assert!(
                !ctx.waives(*guarantee, CALLER),
                "'{}' is waived outside unsafe",
                guarantee.name()
            );
        }
    }

    /// A block waives the listed guarantees **in its own frame**.
    #[test]
    fn a_block_waives_the_listed_guarantees_in_its_own_frame() {
        let mut ctx = ExecutionContext::new();
        let _ = ctx.enter_unsafe(CALLER);
        assert!(ctx.is_unsafe(CALLER));
        for guarantee in Guarantee::ALL {
            assert!(
                ctx.waives(*guarantee, CALLER),
                "'{}' is listed as waivable and was not waived",
                guarantee.name()
            );
        }
    }

    /// DEC-M11-001, at the level of the type: a deeper frame is not covered.
    #[test]
    fn a_callee_frame_is_not_covered_by_its_callers_block() {
        let mut ctx = ExecutionContext::new();
        let _ = ctx.enter_unsafe(CALLER);
        assert!(
            !ctx.is_unsafe(CALLEE),
            "the caller's block reached into the callee's frame"
        );
        for guarantee in Guarantee::ALL {
            assert!(
                !ctx.waives(*guarantee, CALLEE),
                "'{}' crossed a call boundary",
                guarantee.name()
            );
        }
    }

    /// And the callee's own block covers the callee, not the caller.
    #[test]
    fn a_callee_declares_its_own_block() {
        let mut ctx = ExecutionContext::new();
        let outer = ctx.enter_unsafe(CALLER);

        // The callee opens one in its own frame.
        let inner = ctx.enter_unsafe(CALLEE);
        assert!(ctx.is_unsafe(CALLEE));
        assert!(
            !ctx.is_unsafe(CALLER),
            "the callee's block leaked back into the caller's frame"
        );

        // Returning restores the caller's context, and the caller's own block
        // applies again.
        ctx.restore(inner);
        assert!(
            ctx.is_unsafe(CALLER),
            "the caller lost its own block on return"
        );

        ctx.restore(outer);
        assert!(!ctx.is_unsafe(CALLER));
    }

    /// Nesting within one frame composes: the inner exit does not clear the
    /// outer block.
    #[test]
    fn nesting_in_one_frame_restores_the_enclosing_block() {
        let mut ctx = ExecutionContext::new();

        let outer = ctx.enter_unsafe(CALLER);
        let inner = ctx.enter_unsafe(CALLER);
        assert!(ctx.is_unsafe(CALLER));

        ctx.restore(inner);
        assert!(
            ctx.is_unsafe(CALLER),
            "leaving the inner block cleared the outer one"
        );

        ctx.restore(outer);
        assert!(
            !ctx.is_unsafe(CALLER),
            "leaving the outer block did not restore safe"
        );
    }

    /// A saved context can be restored more than once without drifting.
    #[test]
    fn restoring_is_idempotent() {
        let mut ctx = ExecutionContext::new();
        let saved = ctx.enter_unsafe(CALLER);
        ctx.restore(saved);
        ctx.restore(saved);
        assert!(!ctx.is_unsafe(CALLER));
    }
}
