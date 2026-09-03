//! What a runtime service is allowed to depend on.
//!
//! # The problem this is the answer to
//!
//! **DEC-M6-001.** M6 gave five services their own state and two of them their
//! own operations. It could not give any of them their **dispatch** — the code
//! that answers `Binary.toHex(…)` or `Socket.connect(…)` — because every one of
//! those needs things that live on the evaluator: `alloc` to make a value,
//! `rt_err_kind` to raise one, `resolve` to read one, and `null_ref` to return
//! nothing. So sixteen `eval_*_namespace` functions, 12,000+ lines, stayed
//! `impl super::Evaluator`.
//!
//! The register put three options on the table. Passing `&mut Evaluator` makes
//! the dependency explicit in a signature and decouples nothing — "the same
//! coupling, written down". Returning plain Rust `Result` and adapting at the
//! edge is the right architecture and a project rather than a milestone item.
//! This is the middle: **a narrow trait**, so a service depends on four
//! operations rather than on thirty-eight fields.
//!
//! # What is in it, and what is deliberately not
//!
//! [`ValueSink`] is four methods, and the list was derived from the call sites
//! rather than designed in advance — `grep` for `self.` across the two smallest
//! namespaces gives `alloc`, `rt_err_kind`, `resolve`, `null_ref`, and then
//! evaluator internals that are not service concerns.
//!
//! **`eval_expression` is not on it, and that is the boundary.** A dispatch has
//! to evaluate its arguments, which is the evaluator's job and nothing else's; a
//! service that could evaluate an arbitrary expression would have the whole
//! interpreter back. So argument evaluation stays with the evaluator, and what
//! moves behind the trait is the **operation** — the part that takes values it
//! has already been given and produces a value or an error.
//!
//! That split is what makes the trait worth having: an operation behind
//! `ValueSink` can be exercised with a stub, which is the property the register
//! names as the reason to prefer this option, and `tests` below does exactly
//! that.
//!
//! # Why it is not a `RuntimeContext`
//!
//! Because a type that carried everything a service *might* want would be the
//! evaluator with a different name. Four methods is small enough that adding a
//! fifth is a decision someone has to make on purpose.

use crate::region::{ObjectData, ObjectRef};

use super::EvalResult;

/// The four capabilities a runtime service operation needs.
///
/// Implemented by `Evaluator` in production and by a stub in tests. A service
/// written against this cannot reach the arenas, the scope stack, the class
/// registry, the permission set or the call stack — it can make a value, read
/// one, raise an error and return nothing.
pub trait ValueSink {
    /// Place a value in the arena and hand back a reference to it.
    fn alloc(&mut self, data: ObjectData) -> ObjectRef;

    /// Read a value back. `None` when the reference does not resolve, which the
    /// caller must treat as a failure rather than as a null.
    fn resolve(&self, reference: ObjectRef) -> Option<&ObjectData>;

    /// Raise a catchable runtime error of `kind`.
    ///
    /// Returns an [`EvalResult`] rather than a value so a service can `return`
    /// it directly, which is how every existing call site is written.
    fn raise(&mut self, kind: &str, message: String) -> EvalResult;

    /// The shared null.
    fn null(&self) -> ObjectRef;
}

impl ValueSink for super::Evaluator {
    fn alloc(&mut self, data: ObjectData) -> ObjectRef {
        super::Evaluator::alloc(self, data)
    }

    fn resolve(&self, reference: ObjectRef) -> Option<&ObjectData> {
        super::Evaluator::resolve(self, reference)
    }

    fn raise(&mut self, kind: &str, message: String) -> EvalResult {
        self.rt_err_kind(kind.to_string(), message)
    }

    fn null(&self) -> ObjectRef {
        self.null_ref
    }
}

#[cfg(test)]
pub(super) mod stub {
    //! A `ValueSink` with no evaluator behind it.
    //!
    //! The point of the trait, made executable: a service operation can be run
    //! and asserted on without an arena, a scope stack or a class registry. If
    //! an operation ever needs something this cannot provide, it will not
    //! compile against the stub — which is a better boundary check than reading
    //! the signature.

    use super::*;
    use crate::region::RegionId;

    /// Records what an operation allocated and what it raised.
    #[derive(Default)]
    pub struct Recorder {
        pub allocated: Vec<ObjectData>,
        pub raised: Option<(String, String)>,
    }

    impl ValueSink for Recorder {
        fn alloc(&mut self, data: ObjectData) -> ObjectRef {
            self.allocated.push(data);
            ObjectRef {
                region: RegionId::Global,
                index: self.allocated.len() - 1,
            }
        }

        fn resolve(&self, reference: ObjectRef) -> Option<&ObjectData> {
            self.allocated.get(reference.index)
        }

        fn raise(&mut self, kind: &str, message: String) -> EvalResult {
            self.raised = Some((kind.to_string(), message));
            Err(super::super::RuntimeFailure)
        }

        fn null(&self) -> ObjectRef {
            ObjectRef {
                region: RegionId::Global,
                index: usize::MAX,
            }
        }
    }
}
