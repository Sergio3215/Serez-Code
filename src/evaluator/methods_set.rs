#![allow(unused_imports)]
use super::{
    CallFrame, EvalResult, StoredClass, format_decimal, json_parse, json_stringify_owned,
    obj_data_eq, obj_data_to_key_str, operator_to_method_name, owned_to_obj_data, type_matches,
};
use crate::ast::{self, Expression, Statement};
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::region::{SET_INDEX_MIN, SetIndex, set_key_str};
use crate::scope::ScopeStack;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::rc::Rc;

impl super::Evaluator {
    pub(super) fn eval_new_set(&mut self, new_expr: &ast::NewExpression) -> EvalResult {
        let mut elements: Vec<OwnedValue> = Vec::new();
        let init_arg = match &new_expr.args {
            ast::NewArgs::Positional(pos_args) => pos_args.first().cloned(),
            ast::NewArgs::Fields(_) => None,
        };
        if let Some(init_expr) = init_arg {
            let arr_ref = match self.eval_expression(&init_expr) {
                EvalResult::Value(r) => r,
                other => return other,
            };
            // A non-array initialiser used to be dropped on the floor, so
            // `new Set(5)` quietly produced an empty set.
            let initial = match self.resolve(arr_ref).cloned() {
                Some(ObjectData::Array {
                    elements: arr_elems,
                    ..
                }) => Some(arr_elems),
                Some(other) => {
                    let actual = other.type_name().to_string();
                    return self.rt_err_kind(
                        "TypeError",
                        format!("new Set(values): values must be an array, got '{actual}'"),
                    );
                }
                None => {
                    return self.rt_err_kind(
                        "TypeError",
                        "new Set(values): values must be an array, got 'null'",
                    );
                }
            };
            if let Some(arr_elems) = initial {
                // Hash-based dedup: O(N) instead of the old O(N²) pairwise scan.
                // Elements without a fingerprint can never equal anything under
                // obj_data_eq (compounds), so they are pushed unconditionally —
                // exactly what the pairwise scan concluded for them.
                let mut seen: HashSet<String> = HashSet::new();
                for elem in arr_elems {
                    match set_key_str(&elem) {
                        Some(k) => {
                            if seen.insert(k) {
                                elements.push(elem);
                            }
                        }
                        None => elements.push(elem),
                    }
                }
            }
        }
        EvalResult::Value(self.alloc(ObjectData::Set {
            elements,
            index: Default::default(),
        }))
    }

    /// True when the set contains `vd`, honouring obj_data_eq semantics.
    /// Scalar probe on a big set → O(1) via the slot-resident index; small set
    /// or fingerprint-less probe → linear scan (same behavior as always).
    fn set_contains(
        elements: &[OwnedValue],
        index: &SetIndex,
        vd: &OwnedValue,
        key: Option<&str>,
    ) -> bool {
        match key {
            Some(k) if elements.len() >= SET_INDEX_MIN => index.lookup(elements, k).is_some(),
            Some(k) => elements
                .iter()
                .any(|e| set_key_str(e).as_deref() == Some(k)),
            // No fingerprint = compound value: obj_data_eq never matches those,
            // but keep the authoritative scan in case its semantics ever widen.
            None => elements
                .iter()
                .any(|e| obj_data_eq(&Some(owned_to_obj_data(e)), &Some(owned_to_obj_data(vd)))),
        }
    }

    /// Reject a Set call that takes no arguments but was given some. Checked
    /// before anything is evaluated, so a rejected call runs no side effects.
    fn set_no_args(&mut self, dot_call: &ast::DotCallExpression) -> Option<EvalResult> {
        if dot_call.arguments.is_empty() {
            return None;
        }
        let method = dot_call.method.as_str();
        let given = dot_call.arguments.len();
        Some(self.rt_err_kind(
            "TypeError",
            format!("Set.{method} expects 0 arguments, got {given}"),
        ))
    }

    /// The receiver was dispatched here as a Set; if the slot says otherwise an
    /// internal invariant broke. Reported rather than silently answered with an
    /// empty result.
    fn set_receiver_broken(&mut self, method: &str) -> EvalResult {
        self.rt_err_kind("TypeError", format!("Set.{method}: receiver is not a Set"))
    }

    /// All Set methods, dispatched from expr.rs BEFORE the generic dot-call
    /// clones the receiver: every method runs against the arena slot, so even
    /// `.size()` on a 20k-element set no longer pays an O(N) copy, mutations
    /// happen in place (no whole-slot rewrite), and the hash index stays warm.
    pub(super) fn eval_set_method_slot(
        &mut self,
        set_ref: ObjectRef,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {
            "size" => {
                if let Some(error) = self.set_no_args(dot_call) {
                    return error;
                }
                let n = match self.resolve(set_ref) {
                    Some(ObjectData::Set { elements, .. }) => elements.len() as i64,
                    _ => return self.set_receiver_broken("size"),
                };
                EvalResult::Value(self.alloc(ObjectData::Integer(n)))
            }

            "toArray" => {
                if let Some(error) = self.set_no_args(dot_call) {
                    return error;
                }
                let items: Vec<OwnedValue> = match self.resolve(set_ref) {
                    Some(ObjectData::Set { elements, .. }) => elements.clone(),
                    _ => return self.set_receiver_broken("toArray"),
                };
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: None,
                    elements: items,
                }))
            }

            "has" | "contains" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Set.has(val) requires 1 argument");
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let vd = self.extract(vr);
                let key = set_key_str(&vd);
                let found = match self.resolve(set_ref) {
                    Some(ObjectData::Set { elements, index }) => {
                        Self::set_contains(elements, index, &vd, key.as_deref())
                    }
                    _ => false,
                };
                EvalResult::Value(self.alloc(ObjectData::Boolean(found)))
            }

            "add" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Set.add(val) requires 1 argument");
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let vd = self.extract(vr);
                let key = set_key_str(&vd);
                let arena = match set_ref.region {
                    RegionId::Global => &mut self.global_arena,
                    RegionId::Scoped => &mut self.scopes.arena,
                };
                if let Some(ObjectData::Set { elements, index }) = arena.get_mut(set_ref.index) {
                    let already = Self::set_contains(elements, index, &vd, key.as_deref());
                    if !already {
                        elements.push(vd);
                        if let Some(k) = key.as_deref() {
                            index.record_append(k, elements.len() - 1);
                        }
                    }
                }
                EvalResult::Value(set_ref)
            }

            "delete" | "remove" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Set.delete(val) requires 1 argument");
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let vd = self.extract(vr);
                let key = set_key_str(&vd);
                let arena = match set_ref.region {
                    RegionId::Global => &mut self.global_arena,
                    RegionId::Scoped => &mut self.scopes.arena,
                };
                let removed = if let Some(ObjectData::Set { elements, index }) =
                    arena.get_mut(set_ref.index)
                {
                    // At most one occurrence can match: scalars are deduped on
                    // insert and compounds never match under obj_data_eq.
                    let pos = match key.as_deref() {
                        Some(k) if elements.len() >= SET_INDEX_MIN => index.lookup(elements, k),
                        Some(k) => elements
                            .iter()
                            .position(|e| set_key_str(e).as_deref() == Some(k)),
                        None => elements.iter().position(|e| {
                            obj_data_eq(&Some(owned_to_obj_data(e)), &Some(owned_to_obj_data(&vd)))
                        }),
                    };
                    match pos {
                        // Vec::remove keeps insertion order (observable via toArray);
                        // the length change invalidates the index stamp — it rebuilds
                        // on the next lookup.
                        Some(p) => {
                            elements.remove(p);
                            true
                        }
                        None => false,
                    }
                } else {
                    false
                };
                EvalResult::Value(self.alloc(ObjectData::Boolean(removed)))
            }

            "clear" => {
                if let Some(error) = self.set_no_args(dot_call) {
                    return error;
                }
                let arena = match set_ref.region {
                    RegionId::Global => &mut self.global_arena,
                    RegionId::Scoped => &mut self.scopes.arena,
                };
                if let Some(ObjectData::Set { elements, .. }) = arena.get_mut(set_ref.index) {
                    elements.clear();
                }
                EvalResult::Value(self.null_ref)
            }

            "union" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Set.union(other) requires 1 argument");
                }
                let or = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let other_elems: Vec<OwnedValue> = match self.resolve(or) {
                    Some(ObjectData::Set { elements, .. }) => elements.clone(),
                    _ => return self.rt_err_kind("TypeError", "Set.union requires a Set argument"),
                };
                // O(N+M): seed the fingerprint set with self, then admit each
                // element of other exactly once. Fingerprint-less elements
                // (compounds) are pushed unconditionally — obj_data_eq never
                // equates them, which is what the old pairwise scan concluded.
                let (mut result, mut seen) = match self.resolve(set_ref) {
                    Some(ObjectData::Set { elements, .. }) => {
                        let seen: HashSet<String> =
                            elements.iter().filter_map(set_key_str).collect();
                        (elements.clone(), seen)
                    }
                    _ => return self.set_receiver_broken("union"),
                };
                for elem in other_elems {
                    match set_key_str(&elem) {
                        Some(k) => {
                            if seen.insert(k) {
                                result.push(elem);
                            }
                        }
                        None => result.push(elem),
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Set {
                    elements: result,
                    index: Default::default(),
                }))
            }

            "intersection" => {
                if dot_call.arguments.len() != 1 {
                    return self
                        .rt_err_kind("TypeError", "Set.intersection(other) requires 1 argument");
                }
                let or = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let other_keys: HashSet<String> = match self.resolve(or) {
                    Some(ObjectData::Set { elements, .. }) => {
                        elements.iter().filter_map(set_key_str).collect()
                    }
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Set.intersection requires a Set argument");
                    }
                };
                let self_elems: Vec<OwnedValue> = match self.resolve(set_ref) {
                    Some(ObjectData::Set { elements, .. }) => elements.clone(),
                    _ => return self.set_receiver_broken("intersection"),
                };
                // O(N+M). Fingerprint-less elements are dropped: obj_data_eq
                // can never match a compound against anything in `other`, which
                // is exactly what the old O(N×M) scan concluded for them.
                let result: Vec<OwnedValue> = self_elems
                    .into_iter()
                    .filter(|e| set_key_str(e).map_or(false, |k| other_keys.contains(&k)))
                    .collect();
                EvalResult::Value(self.alloc(ObjectData::Set {
                    elements: result,
                    index: Default::default(),
                }))
            }

            "toString" => {
                if let Some(error) = self.set_no_args(dot_call) {
                    return error;
                }
                let s = self.display(set_ref);
                EvalResult::Value(self.alloc(ObjectData::Str(s)))
            }

            // ReferenceError, matching Array, String, Random, DateTime and Task.
            // Set was the last collection reporting an unknown member as a
            // TypeError, which made `e.kind` unusable for classifying it.
            unknown => {
                let message = format!("Unknown Set method '{unknown}'");
                self.rt_err_kind("ReferenceError", message)
            }
        }
    }
}
