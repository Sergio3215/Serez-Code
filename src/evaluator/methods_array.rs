#![allow(unused_imports)]
use super::{
    CallFrame, EvalResult, StoredClass, format_decimal, json_parse, json_stringify_owned,
    obj_data_eq, obj_data_to_key_str, operator_to_method_name, owned_to_obj_data, type_matches,
};
use crate::ast::{self, Expression, Statement};
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::scope::ScopeStack;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::rc::Rc;

impl super::Evaluator {
    /// Reject a call whose argument count the method cannot accept.
    ///
    /// Arity is checked *before* any argument is evaluated, so an invalid call
    /// never runs the side effects of the arguments it was going to reject.
    fn array_arity(
        &mut self,
        dot_call: &ast::DotCallExpression,
        min: usize,
        max: usize,
    ) -> Option<EvalResult> {
        let given = dot_call.arguments.len();
        if given >= min && given <= max {
            return None;
        }
        let expected = if min == max {
            format!("{min} argument{}", if min == 1 { "" } else { "s" })
        } else {
            format!("{min} to {max} arguments")
        };
        let method = dot_call.method.as_str();
        Some(self.rt_err_kind(
            "TypeError",
            format!("{method} expects {expected}, got {given}"),
        ))
    }

    /// Resolve an already-evaluated argument that must be a callback.
    ///
    /// Validation happens before iteration, so an empty receiver still rejects a
    /// non-function: `[].find(1)` is a type error, not a silent `null`.
    fn array_callback_params(
        &mut self,
        cb_ref: ObjectRef,
        method: &str,
    ) -> Result<usize, EvalResult> {
        match self.callback_param_count(cb_ref) {
            Some(count) => Ok(count),
            None => Err(self.rt_err_kind(
                "TypeError",
                format!("{method}: argument must be a function"),
            )),
        }
    }

    /// Check a value against a typed array's declared element type.
    /// Returns the offending type name when the value does not belong.
    fn array_element_mismatch(&self, value: ObjectRef, element_type: &str) -> Option<String> {
        match self.resolve(value) {
            Some(data) if type_matches(element_type, data) => None,
            Some(data) => Some(data.type_name().to_string()),
            None => Some("null".to_string()),
        }
    }

    /// Slot fast path for push/pop (dispatched from expr.rs before the generic
    /// dot-call clones the receiver). Mutates the array in place via get_mut —
    /// no O(N) copy per call, no whole-slot rewrite. Diagnostics are identical
    /// to the generic path: same codes, kinds and messages.
    pub(super) fn eval_array_fast(
        &mut self,
        arr_ref: ObjectRef,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {
            "push" => {
                if let Some(error) = self.array_arity(dot_call, 1, 1) {
                    return error;
                }
                let element_type = match self.resolve(arr_ref) {
                    Some(ObjectData::Array { element_type, .. }) => element_type.clone(),
                    _ => return self.rt_err_kind("TypeError", "push: receiver is not an array"),
                };
                let val_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                if let Some(ref et) = element_type {
                    if let Some(actual) = self.array_element_mismatch(val_ref, et) {
                        return self.rt_err_kind(
                            "TypeError",
                            format!("Cannot push '{actual}' into [{et}] array"),
                        );
                    }
                }
                let val = self.extract(val_ref);
                let arena = match arr_ref.region {
                    RegionId::Global => &mut self.global_arena,
                    RegionId::Scoped => &mut self.scopes.arena,
                };
                if let Some(ObjectData::Array { elements, .. }) = arena.get_mut(arr_ref.index) {
                    elements.push(val);
                }
                EvalResult::Value(self.null_ref)
            }
            "pop" => {
                if let Some(error) = self.array_arity(dot_call, 0, 0) {
                    return error;
                }
                let arena = match arr_ref.region {
                    RegionId::Global => &mut self.global_arena,
                    RegionId::Scoped => &mut self.scopes.arena,
                };
                let popped = match arena.get_mut(arr_ref.index) {
                    Some(ObjectData::Array { elements, .. }) => elements.pop(),
                    _ => None,
                };
                match popped {
                    Some(value) => EvalResult::Value(self.plant(value)),
                    None => self.rt_err_kind("IndexOutOfBounds", "pop() called on an empty array"),
                }
            }
            // Unreachable: the dispatcher only routes push/pop here. Kept as a
            // structured diagnostic rather than a bare sentinel so a future
            // dispatcher change fails loudly instead of silently.
            other => {
                let message = format!("Unknown array method '{other}' on the fast path");
                self.rt_err_kind("ReferenceError", message)
            }
        }
    }

    pub(super) fn eval_array_method(
        &mut self,
        arr_ref: ObjectRef,
        element_type: Option<String>,
        elems: Vec<OwnedValue>,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {
            "length" => {
                if let Some(error) = self.array_arity(dot_call, 0, 0) {
                    return error;
                }
                EvalResult::Value(self.alloc(ObjectData::Integer(elems.len() as i64)))
            }

            "push" => {
                if let Some(error) = self.array_arity(dot_call, 1, 1) {
                    return error;
                }
                let val_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                if let Some(ref et) = element_type {
                    if let Some(actual) = self.array_element_mismatch(val_ref, et) {
                        return self.rt_err_kind(
                            "TypeError",
                            format!("Cannot push '{actual}' into [{et}] array"),
                        );
                    }
                }
                let val = self.extract(val_ref);
                let mut e = elems;
                e.push(val);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.null_ref)
            }

            "pop" => {
                if let Some(error) = self.array_arity(dot_call, 0, 0) {
                    return error;
                }
                let mut e = elems;
                let last = match e.pop() {
                    Some(value) => value,
                    None => {
                        return self
                            .rt_err_kind("IndexOutOfBounds", "pop() called on an empty array");
                    }
                };
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.plant(last))
            }

            "shift" => {
                if let Some(error) = self.array_arity(dot_call, 0, 0) {
                    return error;
                }
                if elems.is_empty() {
                    return self
                        .rt_err_kind("IndexOutOfBounds", "shift() called on an empty array");
                }
                let mut e = elems;
                let first = e.remove(0);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.plant(first))
            }

            "unshift" => {
                if let Some(error) = self.array_arity(dot_call, 1, 1) {
                    return error;
                }
                let val_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                if let Some(ref et) = element_type {
                    if let Some(actual) = self.array_element_mismatch(val_ref, et) {
                        return self.rt_err_kind(
                            "TypeError",
                            format!("Cannot unshift '{actual}' into [{et}] array"),
                        );
                    }
                }
                let val = self.extract(val_ref);
                let mut e = elems;
                e.insert(0, val);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.null_ref)
            }

            "remove" => {
                if let Some(error) = self.array_arity(dot_call, 1, 1) {
                    return error;
                }
                let idx = match self.eval_int_arg(&dot_call.arguments[0], "remove", "index") {
                    Ok(v) => v,
                    Err(error) => return error,
                };
                // Historical contract, deliberately preserved: removing from an
                // empty array yields null instead of failing. It predates the
                // bounds check below and is pinned by the conformance suite.
                if elems.is_empty() {
                    return EvalResult::Value(self.null_ref);
                }
                if idx < 0 || idx as usize >= elems.len() {
                    let len = elems.len();
                    return self.rt_err_kind(
                        "IndexOutOfBounds",
                        format!("remove: index {idx} out of bounds (length {len})"),
                    );
                }
                let mut e = elems;
                let removed = e.remove(idx as usize);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.plant(removed))
            }

            "sort" => {
                if let Some(error) = self.array_arity(dot_call, 0, 1) {
                    return error;
                }
                // Evaluate the optional argument exactly once
                let arg_ref: Option<ObjectRef> = if dot_call.arguments.len() == 1 {
                    match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(r) => Some(r),
                        other => return other,
                    }
                } else {
                    None
                };

                // If the argument is a function, use it as a comparator
                let comparator = arg_ref
                    .filter(|r| matches!(self.resolve(*r), Some(ObjectData::Function { .. })));

                if let Some(cb_ref) = comparator {
                    let mut owned_vals: Vec<OwnedValue> = elems.clone();
                    let n = owned_vals.len();
                    // Bubble sort (simple, avoids borrow issues with call_function).
                    // Every failure path leaves the loop before `update_array`,
                    // so a comparator that fails cannot leave the receiver in a
                    // half-sorted state.
                    let mut i = 0;
                    let mut sort_err: Option<EvalResult> = None;
                    'outer: while i < n {
                        let mut j = 0;
                        while j < n - i - 1 {
                            let a = owned_vals[j].clone();
                            let b = owned_vals[j + 1].clone();
                            let cmp_result = self.call_function(cb_ref, vec![a, b]);
                            // Classify first: the immutable borrow of `resolve`
                            // must end before a diagnostic can be recorded.
                            let comparison = match cmp_result {
                                EvalResult::Value(r) => match self.resolve(r) {
                                    Some(ObjectData::Integer(v)) => Some(*v > 0),
                                    Some(ObjectData::Decimal(v)) => Some(*v > 0.0),
                                    _ => None,
                                },
                                other => {
                                    sort_err = Some(other);
                                    break 'outer;
                                }
                            };
                            let should_swap = match comparison {
                                Some(swap) => swap,
                                None => {
                                    sort_err = Some(self.rt_err_kind(
                                        "TypeError",
                                        "sort comparator must return a number",
                                    ));
                                    break 'outer;
                                }
                            };
                            if should_swap {
                                owned_vals.swap(j, j + 1);
                            }
                            j += 1;
                        }
                        i += 1;
                    }
                    if let Some(err) = sort_err {
                        return err;
                    }
                    self.update_array(arr_ref, element_type, owned_vals);
                    return EvalResult::Value(arr_ref);
                }

                // Any other argument names a sort order. Falling back to "asc"
                // silently would make `sort("ascending")` reorder the array in a
                // direction the program never asked for.
                let descending = match arg_ref {
                    None => false,
                    Some(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Str(order)) if order == "asc" => false,
                        Some(ObjectData::Str(order)) if order == "desc" => true,
                        Some(ObjectData::Str(order)) => {
                            return self.rt_err_kind(
                                "RangeError",
                                format!("sort: order must be \"asc\" or \"desc\", got \"{order}\""),
                            );
                        }
                        _ => {
                            return self.rt_err_kind(
                                "TypeError",
                                "sort: argument must be a comparator function or \"asc\"/\"desc\"",
                            );
                        }
                    },
                };

                let mut owned_vals: Vec<OwnedValue> = elems.clone();

                let all_ints = owned_vals
                    .iter()
                    .all(|v| matches!(v, OwnedValue::Integer(_)));
                let all_decs = owned_vals
                    .iter()
                    .all(|v| matches!(v, OwnedValue::Decimal(_)));
                let all_exact = owned_vals.iter().all(|v| matches!(v, OwnedValue::Dec(_)));
                let all_strs = owned_vals.iter().all(|v| matches!(v, OwnedValue::Str(_)));

                if !all_ints && !all_decs && !all_exact && !all_strs {
                    return self.rt_err_kind(
                        "TypeError",
                        "sort requires a homogeneous array (all int, decimal, dec, or string)",
                    );
                }

                owned_vals.sort_by(|a, b| {
                    let cmp = match (a, b) {
                        (OwnedValue::Integer(x), OwnedValue::Integer(y)) => x.cmp(y),
                        (OwnedValue::Decimal(x), OwnedValue::Decimal(y)) => {
                            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
                        }
                        (OwnedValue::Dec(x), OwnedValue::Dec(y)) => x.cmp(y),
                        (OwnedValue::Str(x), OwnedValue::Str(y)) => x.cmp(y),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if descending { cmp.reverse() } else { cmp }
                });

                self.update_array(arr_ref, element_type, owned_vals);
                EvalResult::Value(arr_ref)
            }

            "map" => {
                if let Some(error) = self.array_arity(dot_call, 1, 1) {
                    return error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let n_params = match self.array_callback_params(cb_ref, "map") {
                    Ok(n) => n,
                    Err(error) => return error,
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().cloned().collect();
                let mut results: Vec<OwnedValue> = Vec::new();
                for (i, val) in owned_elems.into_iter().enumerate() {
                    let args = if n_params >= 2 {
                        vec![val, OwnedValue::Integer(i as i64)]
                    } else {
                        vec![val]
                    };
                    match self.call_function(cb_ref, args) {
                        EvalResult::Value(r) => results.push(self.extract(r)),
                        other => return other,
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: None,
                    elements: results,
                }))
            }

            "filter" => {
                if let Some(error) = self.array_arity(dot_call, 1, 1) {
                    return error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let n_params = match self.array_callback_params(cb_ref, "filter") {
                    Ok(n) => n,
                    Err(error) => return error,
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().cloned().collect();
                let mut kept: Vec<OwnedValue> = Vec::new();
                for (i, val) in owned_elems.into_iter().enumerate() {
                    let args = if n_params >= 2 {
                        vec![val.clone(), OwnedValue::Integer(i as i64)]
                    } else {
                        vec![val.clone()]
                    };
                    let keep = match self.call_function(cb_ref, args) {
                        EvalResult::Value(r) => {
                            let d = self.resolve(r).cloned();
                            self.is_truthy(&d.unwrap_or(ObjectData::Null))
                        }
                        other => return other,
                    };
                    if keep {
                        kept.push(val);
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type,
                    elements: kept,
                }))
            }

            "reduce" => {
                if let Some(error) = self.array_arity(dot_call, 1, 2) {
                    return error;
                }
                let owned_elems: Vec<OwnedValue> = elems.clone();
                let (mut acc_ref, cb_ref, start_idx) = if dot_call.arguments.len() == 2 {
                    // reduce(initial, callback) — the initial value comes first
                    // in Serez. This order is public API; see spec/values.md.
                    let init = match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(r) => r,
                        other => return other,
                    };
                    let cb = match self.eval_expression(&dot_call.arguments[1]) {
                        EvalResult::Value(r) => r,
                        other => return other,
                    };
                    (init, cb, 0usize)
                } else {
                    // reduce(callback) — first element is the initial accumulator
                    if owned_elems.is_empty() {
                        return self.rt_err_kind(
                            "TypeError",
                            "reduce with no initial value requires a non-empty array",
                        );
                    }
                    let cb = match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(r) => r,
                        other => return other,
                    };
                    let first_ref = self.plant(owned_elems[0].clone());
                    (first_ref, cb, 1usize)
                };
                if let Err(error) = self.array_callback_params(cb_ref, "reduce") {
                    return error;
                }
                for val in owned_elems.into_iter().skip(start_idx) {
                    let acc_val = self.extract(acc_ref);
                    acc_ref = match self.call_function(cb_ref, vec![acc_val, val]) {
                        EvalResult::Value(r) => r,
                        other => return other,
                    };
                }
                EvalResult::Value(acc_ref)
            }

            "join" => {
                if let Some(error) = self.array_arity(dot_call, 0, 1) {
                    return error;
                }
                let sep = if dot_call.arguments.is_empty() {
                    ",".to_string()
                } else {
                    match self.eval_str_arg(&dot_call.arguments[0], "join", "separator") {
                        Ok(s) => s,
                        Err(error) => return error,
                    }
                };
                let parts: Vec<String> = elems.iter().map(|v| v.display_str()).collect();
                EvalResult::Value(self.alloc(ObjectData::Str(parts.join(&sep))))
            }

            "toString" => {
                if let Some(error) = self.array_arity(dot_call, 0, 0) {
                    return error;
                }
                let s = self.display(arr_ref);
                EvalResult::Value(self.alloc(ObjectData::Str(s)))
            }

            "indexOf" => {
                if let Some(error) = self.array_arity(dot_call, 1, 1) {
                    return error;
                }
                let needle_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let needle_data = self.resolve(needle_ref).cloned();
                let idx = elems
                    .iter()
                    .enumerate()
                    .find(|(_, elem)| {
                        let elem_data = Some(owned_to_obj_data(elem));
                        obj_data_eq(&elem_data, &needle_data)
                    })
                    .map(|(i, _)| i as i64)
                    .unwrap_or(-1);
                EvalResult::Value(self.alloc(ObjectData::Integer(idx)))
            }

            "includes" | "contains" => {
                if let Some(error) = self.array_arity(dot_call, 1, 1) {
                    return error;
                }
                let needle_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let needle_data = self.resolve(needle_ref).cloned();
                let found = elems.iter().any(|elem| {
                    let elem_data = Some(owned_to_obj_data(elem));
                    obj_data_eq(&elem_data, &needle_data)
                });
                EvalResult::Value(self.alloc(ObjectData::Boolean(found)))
            }

            "find" => {
                if let Some(error) = self.array_arity(dot_call, 1, 1) {
                    return error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                if let Err(error) = self.array_callback_params(cb_ref, "find") {
                    return error;
                }
                let owned_elems: Vec<OwnedValue> = elems.clone();
                for val in owned_elems {
                    let val_clone = val.clone();
                    let result = match self.call_function(cb_ref, vec![val]) {
                        EvalResult::Value(r) => r,
                        other => return other,
                    };
                    if self.is_truthy(self.resolve(result).unwrap()) {
                        return EvalResult::Value(self.plant(val_clone));
                    }
                }
                EvalResult::Value(self.null_ref)
            }

            "findIndex" => {
                if let Some(error) = self.array_arity(dot_call, 1, 1) {
                    return error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                if let Err(error) = self.array_callback_params(cb_ref, "findIndex") {
                    return error;
                }
                let owned_elems: Vec<OwnedValue> = elems.clone();
                for (i, val) in owned_elems.into_iter().enumerate() {
                    let result = match self.call_function(cb_ref, vec![val]) {
                        EvalResult::Value(r) => r,
                        other => return other,
                    };
                    if self.is_truthy(self.resolve(result).unwrap()) {
                        return EvalResult::Value(self.alloc(ObjectData::Integer(i as i64)));
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Integer(-1)))
            }

            "slice" => {
                if let Some(error) = self.array_arity(dot_call, 0, 2) {
                    return error;
                }
                let len = elems.len() as i64;
                let start_i = if !dot_call.arguments.is_empty() {
                    match self.eval_int_arg(&dot_call.arguments[0], "slice", "start") {
                        Ok(v) => v,
                        Err(error) => return error,
                    }
                } else {
                    0
                };
                let end_i = if dot_call.arguments.len() >= 2 {
                    match self.eval_int_arg(&dot_call.arguments[1], "slice", "end") {
                        Ok(v) => v,
                        Err(error) => return error,
                    }
                } else {
                    len
                };
                // Normalize negative indices (count from end) then clamp
                let start = (if start_i < 0 {
                    (len + start_i).max(0)
                } else {
                    start_i.min(len)
                }) as usize;
                let end = (if end_i < 0 {
                    (len + end_i).max(0)
                } else {
                    end_i.min(len)
                }) as usize;
                let end = end.max(start); // prevent inverted range
                let sliced: Vec<OwnedValue> = elems[start..end].iter().cloned().collect();
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: element_type.clone(),
                    elements: sliced,
                }))
            }

            "reverse" => {
                if let Some(error) = self.array_arity(dot_call, 0, 0) {
                    return error;
                }
                let mut e = elems;
                e.reverse();
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(arr_ref)
            }

            "every" => {
                if let Some(error) = self.array_arity(dot_call, 1, 1) {
                    return error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                if let Err(error) = self.array_callback_params(cb_ref, "every") {
                    return error;
                }
                let owned_elems: Vec<OwnedValue> = elems.clone();
                for val in owned_elems {
                    let result = match self.call_function(cb_ref, vec![val]) {
                        EvalResult::Value(r) => r,
                        other => return other,
                    };
                    if !self.is_truthy(self.resolve(result).unwrap()) {
                        return EvalResult::Value(self.alloc(ObjectData::Boolean(false)));
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Boolean(true)))
            }

            "some" => {
                if let Some(error) = self.array_arity(dot_call, 1, 1) {
                    return error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                if let Err(error) = self.array_callback_params(cb_ref, "some") {
                    return error;
                }
                let owned_elems: Vec<OwnedValue> = elems.clone();
                for val in owned_elems {
                    let result = match self.call_function(cb_ref, vec![val]) {
                        EvalResult::Value(r) => r,
                        other => return other,
                    };
                    if self.is_truthy(self.resolve(result).unwrap()) {
                        return EvalResult::Value(self.alloc(ObjectData::Boolean(true)));
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Boolean(false)))
            }

            "flat" => {
                if let Some(error) = self.array_arity(dot_call, 0, 1) {
                    return error;
                }
                let depth = if dot_call.arguments.is_empty() {
                    1usize
                } else {
                    // A negative depth still clamps to 0 (flatten nothing);
                    // only a non-int argument is rejected.
                    match self.eval_int_arg(&dot_call.arguments[0], "flat", "depth") {
                        Ok(d) => d.max(0) as usize,
                        Err(error) => return error,
                    }
                };

                fn flat_owned(items: Vec<OwnedValue>, depth: usize) -> Vec<OwnedValue> {
                    if depth == 0 {
                        return items;
                    }
                    let mut result = Vec::new();
                    for item in items {
                        match item {
                            OwnedValue::Array { elements, .. } => {
                                result.extend(flat_owned(elements, depth - 1));
                            }
                            other => result.push(other),
                        }
                    }
                    result
                }

                let flat = flat_owned(elems.clone(), depth);
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: None,
                    elements: flat,
                }))
            }

            unknown => {
                let message = format!("Unknown array method '{unknown}'");
                self.rt_err_kind("ReferenceError", message)
            }
        }
    }

    // ── String methods ────────────────────────────────────────────────────────
}
